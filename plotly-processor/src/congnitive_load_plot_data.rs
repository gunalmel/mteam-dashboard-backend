use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug)]
pub struct CognitiveLoadPlotData {
    pub x: Vec<String>,
    pub y: Vec<Option<f64>>,
    pub mode: String,
    #[serde(rename = "type")]
    pub series_type: String
}