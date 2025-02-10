use std::env;
use std::sync::Arc;
use log::debug;
use serde::Deserialize;
use mteam_dashboard_plotly_processor::config::init::init_plot_config;
use mteam_dashboard_plotly_processor::config::plotly_mappings::PlotlyConfig;
use crate::config::resolve_file_path::{resolve_config_file_path, resolve_first_path};
use crate::CREDENTIALS_FILE_HOME;
use crate::data_providers::file_provider::LocalFileDataSource;
use crate::data_providers::gdrive_provider::google_data_source::GoogleDriveDataSource;
use crate::data_providers::gdrive_provider::google_drive_hub_adapter_builder::GoogleDriveHubAdapterBuilder;
use crate::data_source::DataSource;

pub enum PlotType{
    CognitiveLoad,
    VisualAttention
}

impl PlotType {
    pub fn as_str(&self) -> &str {
        match self {
            PlotType::CognitiveLoad => "cognitive-load",
            PlotType::VisualAttention => "visual-attention"
        }
    }
}

#[derive(Debug, Deserialize)]
pub enum DataSourceType {
    LocalFile,
    GoogleDrive
}

#[derive(Debug, Deserialize)]
pub struct AppConfig {
    #[serde(rename = "gdriveCredentialsFile")]
    pub gdrive_credentials_file: String,
    #[serde(rename = "gdriveRootFolderId")]
    pub gdrive_root_folder_id: String,
    #[serde(rename = "plotConfigPath")]
    pub plot_config_path: String,
    #[serde(rename = "fileSystemPath")]
    pub file_system_path: String,
    #[serde(rename = "dataSourceType")]
    pub data_source_type: DataSourceType,
    pub port: u16,
    #[serde(rename = "staticFilesPath")]
    pub static_files_path: String
}

impl AppConfig {
    pub(crate) fn new(config_file: &str) -> Result<AppConfig, std::io::Error> {
        let args: Vec<String> = env::args().collect();
        // Get the configuration file path
        let config_path = resolve_config_file_path(&args, &vec![config_file])?;
        debug!("Using configuration file: {:?}", config_path);

        // Load the configuration file as a struct
        let config: AppConfig = serde_json::from_reader(std::fs::File::open(config_path)?)?;
        debug!("Loaded config: {:#?}", config);
        Ok(config)
    }
    pub(crate) async fn get_data_provider(&self) -> Arc<dyn DataSource> {
        match self.data_source_type {
            DataSourceType::LocalFile => get_local_file_datasource_provider(&self).await,
            DataSourceType::GoogleDrive => get_gdrive_datasource_provider(&self).await
        }
    }

    pub(crate) fn get_plotly_config(&self) -> &'static PlotlyConfig {
        let plot_config_path = resolve_first_path(&[self.plot_config_path.as_str()]).unwrap();
        debug!("Using plot config path: {:#?}", plot_config_path);
        let plot_config = init_plot_config(plot_config_path).unwrap().unwrap();
        debug!("Loaded plot config: {:#?}", plot_config);
        plot_config
    }
}

async fn get_local_file_datasource_provider(config: &AppConfig) -> Arc<dyn DataSource> {
    Arc::new(LocalFileDataSource::new(config.file_system_path.clone()))
}
async fn get_gdrive_datasource_provider(config: &AppConfig) -> Arc<dyn DataSource> {
    let gdrive_credentials_file = resolve_first_path(&[
        config.gdrive_credentials_file.as_str(),
        CREDENTIALS_FILE_HOME,
    ])
        .unwrap();
    debug!(
        "Using gdrive credentials file: {:?}",
        gdrive_credentials_file
    );

    let builder = GoogleDriveHubAdapterBuilder::new()
        .with_credentials(gdrive_credentials_file)
        .with_scope("https://www.googleapis.com/auth/drive.readonly".to_string());

    let hub_adapter = builder
        .build()
        .await
        .expect("Failed to build GoogleDriveHubAdapter");

    Arc::new(
        GoogleDriveDataSource::new(config.gdrive_root_folder_id.clone(), hub_adapter)
            .await
            .expect("Failed to initialize GoogleDriveDataSource"),
    )
}