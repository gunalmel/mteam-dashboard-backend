use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct AppConfig {
    #[serde(rename = "gdriveCredentialsFile")]
    pub gdrive_credentials_file: String,
    #[serde(rename = "gdriveRootFolderId")]
    pub gdrive_root_folder_id: String,
    #[serde(rename = "plotConfigPath")]
    pub plot_config_path: String,
    #[serde(rename = "fileSystemPath")]
    pub file_system_path: String
}