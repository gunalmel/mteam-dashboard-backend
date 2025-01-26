use serde::{Deserialize, Serialize};
use crate::font::Font;

#[derive(Serialize, Deserialize, Debug)]
pub struct Annotation {
    pub xref: String,
    pub yref: String,
    pub x: String,
    pub y: f32,
    pub xanchor: String,
    pub yanchor: String,
    pub text: String,
    pub showarrow: bool,
    pub font: Font,
    pub bgcolor: String,
    pub bordercolor: String,
    pub borderwidth: i16,
    pub borderpad: i16
}