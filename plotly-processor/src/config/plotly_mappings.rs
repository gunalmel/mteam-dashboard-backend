use std::{fmt, fs, io, path::Path};
use std::borrow::ToOwned;
use serde_json::Error as SerdeError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug)]
pub enum ConfigError {
    Io(io::Error),
    Serde(SerdeError),
}

impl From<io::Error> for ConfigError {
    fn from(err: io::Error) -> Self {
        ConfigError::Io(err)
    }
}

impl From<serde_json::Error> for ConfigError {
    fn from(err: serde_json::Error) -> Self {
        ConfigError::Serde(err)
    }
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct StagesConfig {
    pub names: HashMap<String, String>,
    pub colors: Vec<String>
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MissedActionsPlotSettings {
    #[serde(rename = "maxCountPerRow")]
    pub max_count_per_row: u16,
    #[serde(rename = "yIncrement")]
    pub y_increment: f32,
    #[serde(rename = "yMin")]
    pub y_min: f32
}

impl MissedActionsPlotSettings {
    pub fn calculate_y_max(&self, max_missed_action_count_per_stage: u16) -> f32 {
        self.y_min + (max_missed_action_count_per_stage as f32 / (self.max_count_per_row + 1) as f32) * self.y_increment
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ActionsPlotSettings {
    #[serde(rename = "xAxisPaddingSecs")]
    pub x_axis_padding_secs: usize,
    #[serde(rename = "yAnnotation")]
    pub y_annotation: f32,
    #[serde(rename = "yMin")]
    pub y_min: f32,
    #[serde(rename = "yIncrement")]
    pub y_increment: f32,
    #[serde(rename = "missedActions")]
    pub missed_actions: MissedActionsPlotSettings
}

const DEFAULT_ACTION_GROUP_NAME: &str = "default_group_name";
const DEFAULT_ACTION_GROUP_ICON_ATTR: &str = "default"; 
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PlotlyConfig {
    pub stages: StagesConfig,
    pub action_groups: HashMap<String, String>,
    pub action_group_icons: HashMap<String, String>,
    pub action_plot_settings: ActionsPlotSettings
}
impl PlotlyConfig {
    pub fn get_action_group_name(&self, action_name: &str) -> String{
        self.action_groups.get(&action_name.to_lowercase()).unwrap_or(&DEFAULT_ACTION_GROUP_NAME.to_owned()).to_owned()
    }

    pub fn get_action_group_icon(&self, group_name: &str) -> String {
        self.action_group_icons.get(group_name).unwrap_or(&DEFAULT_ACTION_GROUP_ICON_ATTR.to_owned()).to_owned()
    }
}
impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl std::error::Error for ConfigError {}
impl PlotlyConfig {
    pub fn load(config_dir: &Path) -> Result<Self, ConfigError> {
        let stage_names: StagesConfig =
            load_json(config_dir.join("action-plot-stages.json"))?;
        let action_groups: HashMap<String, String> =
            load_json(config_dir.join("action-groups.json"))?;
        let action_group_icons: HashMap<String, String> =
            load_json(config_dir.join("action-group-icons.json"))?;
        let action_plot_settings: ActionsPlotSettings = load_json(config_dir.join("action-plot-settings.json"))?;

        Ok(PlotlyConfig {
            stages: stage_names,
            action_groups,
            action_group_icons,
            action_plot_settings
        })
    }
}

// for<'de> Deserialize<'de> is essential for writing generic deserialization functions in Rust
// that can handle data with arbitrary lifetimes. It's a key part of how serde achieves its
// flexibility and safety. If you are ever writing a function that deserializes data using serde,
// it is almost always what you want.
fn load_json<T: for<'de> Deserialize<'de>>(path: impl AsRef<Path>) -> Result<T, ConfigError> {
    let content = fs::read_to_string(path.as_ref())?;
    let data = serde_json::from_str(&content)?;
    Ok(data)
}