use crate::data_source::DataSource;
use mteam_dashboard_plotly_processor::config::plotly_mappings::PlotlyConfig;
use std::sync::Arc;

pub struct AppContext {
    pub datasource_provider: Arc<dyn DataSource>,
    pub plotly_config: &'static PlotlyConfig
}