use std::collections::HashMap;
use crate::gdrive_provider::data_source_name_parser::gdrive_folder_to_location;
use crate::gdrive_provider::read_credentials::read_credentials;
use google_drive3::api::File;
use google_drive3::common::Client;
use google_drive3::yup_oauth2::authenticator::Authenticator;
use google_drive3::yup_oauth2::{ServiceAccountAuthenticator, ServiceAccountKey};
use google_drive3::DriveHub;
use hyper::header::AUTHORIZATION;
use hyper::StatusCode;
use hyper_rustls::HttpsConnector;
use hyper_util::client::legacy::connect::HttpConnector;
use rustls::crypto::ring::default_provider;
use rustls::crypto::CryptoProvider;
use serde_json::Value;
use std::error::Error;
use std::future::Future;
use std::io::Read;
use std::pin::Pin;
use std::sync::{Arc, Once};
use crate::utils::strings::snake_case_file_to_title_case;

static INIT: Once = Once::new();

const GDRIVE_SCOPE: &str = "https://www.googleapis.com/auth/drive";

pub struct GoogleDriveDataSource {
    hub: Arc<dyn DriveHubWrapper + Send + Sync>,
    main_folder_id: String,
}

pub trait DriveHubWrapper {
    fn fetch_files(&self, query: String) -> Pin<Box<dyn Future<Output = Result<Vec<File>, String>> + Send>>;
    fn fetch_file_data(&self, file_id: String) -> Pin<Box<dyn Future<Output=Result<Vec<u8>, String>> + Send>>;
}

struct GoogleDriveHubAdapter {
    hub: Arc<DriveHub<HttpsConnector<HttpConnector>>>,
}

impl DriveHubWrapper for GoogleDriveHubAdapter {
    fn fetch_files(&self, query: String) -> Pin<Box<dyn Future<Output=Result<Vec<File>, String>> + Send>> {
        let hub = Arc::clone(&self.hub);
        Box::pin(async move {
            let result = hub.files().list().add_scope(GDRIVE_SCOPE).q(query.as_str()).doit().await;
            match result {
                Err(e) => Err(format!("HTTP error: {:?}", e)),
                Ok((response, file_list)) => match response.status() {
                    StatusCode::OK => Ok(file_list.files.unwrap_or_else(Vec::new)),
                    _ => Err(format!(
                        "Failed to fetch file list. Response status: {}, body: {:?}",
                        response.status(),
                        response.body()
                    )),
                },
            }
        })
    }

    fn fetch_file_data(&self, file_id: String) -> Pin<Box<dyn Future<Output=Result<Vec<u8>, String>> + Send>> {
        let hub = Arc::clone(&self.hub);
        Box::pin(async move {
            let access_token = hub.auth.get_token(&[GDRIVE_SCOPE]).await.map_err(|e| format!("Token error: {}", e)).unwrap().unwrap();
            let url = format!("https://www.googleapis.com/drive/v3/files/{}?alt=media", file_id);

            let client = reqwest::Client::new();
            let response = client
                .get(&url)
                .header(AUTHORIZATION, format!("Bearer {}", access_token))
                .send()
                .await
                .map_err(|e| format!("Download request error: {}", e))?;

            if response.status().is_success() {
                let bytes = response.bytes().await.map_err(|e| format!("Reading bytes error: {}", e))?;
                Ok(bytes.to_vec())
            } else {
                Err(format!(
                    "Download failed with status: {}, body: {:?}",
                    response.status(),
                    response.text().await.unwrap_or_default()
                ))
            }
        })
    }
}
pub trait DataSourceProvider {
    async fn get_main_folder_list(&self) -> Result<Vec<Value>, Box<dyn Error>>;
    async fn fetch_csv_reader(&self, date_folder_id: String) -> Result<Box<dyn Read>, String>;
    async fn fetch_json_file_map(&self, date_folder_id: &str, category_folder_name: &str) -> Result<HashMap<String, String>, String>;
}

impl DataSourceProvider for GoogleDriveDataSource {
    async fn get_main_folder_list(&self) -> Result<Vec<Value>, Box<dyn Error>> {
        let folder_list = self.hub.fetch_files(build_drive_query(&self.main_folder_id, "and mimeType = 'application/vnd.google-apps.folder'")).await?;

        let mut files: Vec<Value> = folder_list
            .into_iter()
            .filter_map(gdrive_folder_to_location) // Filters and maps files
            .collect();

        files.sort_by(|a, b| {
            let epoch_a = a["date"]["epoch"].as_i64().unwrap_or(0);
            let epoch_b = b["date"]["epoch"].as_i64().unwrap_or(0);
            epoch_a.cmp(&epoch_b)
        });

        Ok(files)
    }
    async fn fetch_csv_reader(&self, folder_id: String) -> Result<Box<dyn Read>, String> {
        let query = format!(
            "mimeType contains 'text/' and '{}' in parents and trashed = false",
            &folder_id
        );

        let csv_files = self.hub.fetch_files(query).await?;
        if csv_files.is_empty() {
            return Err(format!("No files found under the specified ID: {}", &folder_id).into());
        }

        let csv_file_id = csv_files
            .get(0)
            .and_then(|file| file.id.as_ref())
            .ok_or_else(|| "File ID not found".to_string())?;

        let data = self.hub.fetch_file_data(csv_file_id.to_string()).await?;
        Ok(Box::new(std::io::Cursor::new(data)))
    }
    async fn fetch_json_file_map(&self, source_folder_id: &str, sub_folder_name: &str) -> Result<HashMap<String, String>, String> {
        let folder_id = self.get_subfolder_id(source_folder_id, sub_folder_name).await?;

        let files = self.get_json_file_name_map(folder_id).await?;

        Ok(files)
    }
}

impl GoogleDriveDataSource {
    pub async fn new(credentials_path: &str, folder_id: &str) -> Result<Self, Box<dyn Error>> {

        let secret = read_credentials(credentials_path)?;
        let client = build_connection_client();
        let auth = create_auth_authenticator(secret).await?;
        let hub = Arc::new(DriveHub::new(client, auth));

        Ok(Self {
            hub: Arc::new(GoogleDriveHubAdapter { hub }),
            main_folder_id: folder_id.to_string(),
        })
    }

    async fn get_subfolder_id(&self, parent_folder_id: &str, subfolder_name: &str) -> Result<String, String> {
        let query = format!("mimeType = 'application/vnd.google-apps.folder' and '{}' in parents and name = '{}' and trashed = false", parent_folder_id, subfolder_name);

        let subfolders = self.hub.fetch_files(query).await?;
        if subfolders.is_empty() {
            return Err(format!("Subfolder not found: {}", subfolder_name).into());
        }

        let subfolder_id = subfolders
            .get(0)
            .and_then(|file| file.id.as_ref())
            .ok_or_else(|| "Subfolder ID not found".to_string())?;

        Ok(subfolder_id.to_string())
    }

    async fn get_json_file_name_map(&self, folder_id: String) -> Result<HashMap<String, String>, String> {
        let query = format!("mimeType = 'application/json' and '{}' in parents and trashed = false", folder_id);

        let files = self.hub.fetch_files(query).await?;
        let file_map = files
            .into_iter()
            .map(|file| (snake_case_file_to_title_case(file.name.unwrap_or_default().as_str()), file.id.unwrap_or_default()))
            .collect::<HashMap<String, String>>();

        Ok(file_map)
    }
}

fn build_drive_query(folder_id: &str, query_filter: &str) -> String {
    format!("'{}' in parents and trashed=false {}", folder_id, query_filter)
}

fn build_connection_client() -> Client<HttpsConnector<HttpConnector>> {
    INIT.call_once(|| {
        CryptoProvider::install_default(default_provider())
            .expect("Failed to install the default crypto provider for rustls");
    });
    hyper_util::client::legacy::Client::builder(hyper_util::rt::TokioExecutor::new())
        .build(
            hyper_rustls::HttpsConnectorBuilder::new()
                .with_native_roots()
                .unwrap()
                .https_or_http()
                .enable_http2()
                .build(),
        )
}

async fn create_auth_authenticator(secret_json: String) -> Result<Authenticator<HttpsConnector<HttpConnector>>, Box<dyn Error>> {
    let secret: ServiceAccountKey = serde_json::from_str(&secret_json)
        .map_err(|e| format!("Failed to parse service account key: {:?}", e))?;

    Ok(ServiceAccountAuthenticator::builder(secret)
        .build()
        .await
        .map_err(|e| format!("Failed to create authenticator: {}", e))?)
}