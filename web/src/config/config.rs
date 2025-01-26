use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct AppConfig {
    pub gdrive_credentials_file: String,
    pub gdrive_root_folder_id: String,
    pub plot_config_path: String,
}