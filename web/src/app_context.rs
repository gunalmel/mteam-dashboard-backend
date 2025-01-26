use std::sync::Arc;
use mteam_dashboard_plotly_processor::config::plotly_mappings::PlotlyConfig;
use crate::gdrive_provider::provider::GoogleDriveDataSource;

pub struct AppContext {
    pub datasource_provider: Arc<GoogleDriveDataSource>,
    pub plotly_config: &'static PlotlyConfig
}