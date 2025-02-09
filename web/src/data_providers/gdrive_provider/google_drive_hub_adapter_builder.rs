use google_drive3::DriveHub;
use std::error::Error;
use std::sync::Arc;
use crate::data_providers::gdrive_provider::google_drive_hub_adapter::GoogleDriveHubAdapter;
use crate::data_providers::gdrive_provider::google_drive_utils::{build_connection_client, create_auth_authenticator};
use crate::data_providers::gdrive_provider::read_credentials::read_credentials;

pub struct GoogleDriveHubAdapterBuilder {
    credentials_path: Option<String>,
    scope: Option<String>
}

impl GoogleDriveHubAdapterBuilder {
    pub fn new() -> Self {
        GoogleDriveHubAdapterBuilder {
            credentials_path: None,
            scope: None
        }
    }

    pub fn with_scope(mut self, scope: String) -> Self {
        self.scope = Some(scope);
        self
    }

    pub fn with_credentials(mut self, credentials_path: String) -> Self {
        self.credentials_path = Some(credentials_path);
        self
    }

    pub async fn build(self) -> Result<Arc<GoogleDriveHubAdapter>, Box<dyn Error>> {
        let scope = self.scope.ok_or("Scope is missing")?;
        let credentials_path = self.credentials_path.ok_or("Credentials path is missing")?;
        let secret = read_credentials(&credentials_path)?;
        let client = build_connection_client();
        let auth = create_auth_authenticator(secret).await?;
        let hub = DriveHub::new(client, auth);
        Ok(Arc::new(GoogleDriveHubAdapter::new(hub, scope)))
    }
}