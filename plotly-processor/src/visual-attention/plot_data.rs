use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Debug)]
pub struct VisualAttentionCategory {
    pub x: Vec<String>,
    pub y: Vec<f64>,
    pub name: String,
    #[serde(rename = "type")]
    pub plot_type: String,
    pub marker: HashMap<String, String> //will only set color
}