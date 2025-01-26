use crate::config::plotly_mappings::{ConfigError, PlotlyConfig};
use once_cell::sync::OnceCell;
use std::path::Path;
use std::io;

static CONFIG: OnceCell<PlotlyConfig> = OnceCell::new();

fn read_config(config_dir: &Path) -> Result<(), ConfigError> {
    let config = PlotlyConfig::load(config_dir)?;
    Ok(CONFIG.set(config).unwrap())
}

pub fn get_config() -> Option<&'static PlotlyConfig> {
    CONFIG.get()
}

pub fn init_plot_config(path_string: String) -> Result<Option<&'static PlotlyConfig>, io::Error> {
        let path = Path::new(&path_string);
        if path.exists() {
            read_config(&path)
                .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("Error initializing plot-config at {:?}: {}", path, e)))?;
            return Ok(get_config().map(|c| c));
        } 

    // If all attempts fail, return a combined error
    Err(io::Error::new(
        io::ErrorKind::Other,
        "Reading the plot configuration files failed".to_string(),
    ))
}