use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug)]
#[derive(Clone)]
pub struct Image {
    pub source: String,
    pub x: String,
    pub y: String,
    pub sizex: f64,
    pub sizey: f64,
    pub xref: String,
    pub yref: String,
    pub xanchor: String,
    pub yanchor: String,
    pub layer: String,
    pub visible: bool,
    pub sizing: String,
    pub opacity: i8,
}