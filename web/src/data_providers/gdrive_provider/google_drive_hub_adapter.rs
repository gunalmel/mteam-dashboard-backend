use google_drive3::DriveHub;
use hyper_rustls::HttpsConnector;
use hyper_util::client::legacy::connect::HttpConnector;
use std::sync::Arc;
use std::pin::Pin;
use std::future::Future;
use google_drive3::api::File;
use hyper::StatusCode;
use hyper::header::AUTHORIZATION;
use crate::data_providers::gdrive_provider::drive_hub_adapter::DriveHubAdapter;

pub struct GoogleDriveHubAdapter {
    hub: Arc<DriveHub<HttpsConnector<HttpConnector>>>,
    scope: String
}

impl GoogleDriveHubAdapter {
    pub fn new(hub: DriveHub<HttpsConnector<HttpConnector>>, scope: String) -> Self {
        Self {
            hub: Arc::new(hub),
            scope
        }
    }
}

impl DriveHubAdapter for GoogleDriveHubAdapter {
    fn fetch_files(&self, query: String) -> Pin<Box<dyn Future<Output=Result<Vec<File>, String>> + Send + '_>> {
        let hub = Arc::clone(&self.hub);
        Box::pin(async move {
            let result = hub.files().list().add_scope(&self.scope).q(&query).doit().await;
            match result {
                Err(e) => Err(format!("HTTP error: {:?}", e)),
                Ok((response, file_list)) => match response.status() {
                    StatusCode::OK => Ok(file_list.files.unwrap_or_default()),
                    _ => Err(format!(
                        "Failed to fetch file list. Response status: {}, body: {:?}",
                        response.status(),
                        response.body()
                    )),
                },
            }
        })
    }

    fn fetch_file_data(&self, file_id: String) -> Pin<Box<dyn Future<Output=Result<Vec<u8>, String>> + Send + '_>> {
        let hub = Arc::clone(&self.hub);
        Box::pin(async move {
            let access_token = hub.auth.get_token(&[&self.scope]).await.map_err(|e| format!("Token error: {}", e))?.ok_or("Missing access token")?;
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