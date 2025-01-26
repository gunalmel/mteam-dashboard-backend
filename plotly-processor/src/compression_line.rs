use serde::{Serialize, Deserialize};
use crate::font::Font;
use crate::line::Line;

#[derive(Serialize, Deserialize, Debug)]
pub struct CompressionLine {
    pub x: Vec<String>,
    pub y: Vec<String>,
    pub text: String,
    pub mode: String,
    #[serde(rename = "type")]
    pub line_type: String,
    pub hoverinfo: String,
    pub textposition: String,
    pub textfont: Font,
    pub line: Line,
    pub hovertext: Vec<String>,
}
