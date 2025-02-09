use google_drive3::common::Client;
use google_drive3::yup_oauth2::authenticator::Authenticator;
use google_drive3::yup_oauth2::{ServiceAccountAuthenticator, ServiceAccountKey};
use hyper_rustls::HttpsConnector;
use hyper_util::client::legacy::connect::HttpConnector;
use rustls::crypto::ring::default_provider;
use rustls::crypto::CryptoProvider;
use std::error::Error;
use std::sync::Once;

static INIT: Once = Once::new();

pub(crate) fn build_connection_client() -> Client<HttpsConnector<HttpConnector>> {
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

pub(crate) async fn create_auth_authenticator(secret_json: String) -> Result<Authenticator<HttpsConnector<HttpConnector>>, Box<dyn Error>> {
    let secret: ServiceAccountKey = serde_json::from_str(&secret_json)
        .map_err(|e| format!("Failed to parse service account key: {:?}", e))?;

    Ok(ServiceAccountAuthenticator::builder(secret)
        .build()
        .await
        .map_err(|e| format!("Failed to create authenticator: {}", e))?)
}

pub(crate) fn build_drive_query(folder_id: &str, query_filter: &str) -> String {
    format!("'{}' in parents and trashed=false {}", folder_id, query_filter)
}