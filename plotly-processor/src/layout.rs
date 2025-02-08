use crate::annotation::Annotation;
use crate::image::Image;
use crate::shape::Shape;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct Layout {
    pub title: Title,
    pub margin: Margin,
    pub xaxis: XAxis,
    pub yaxis: YAxis,
    pub modebar: ModeBar,
    pub autosize: bool,
    pub showlegend: bool,
    pub legend: Legend,
    pub shapes: Vec<Shape>,
    pub annotations: Vec<Annotation>,
    pub images: Vec<Image>
}

impl Layout{
    pub fn new() -> Self{
       Self {
            annotations:Vec::new(),
            autosize: true,
            images: Vec::new(),
            legend: Legend {
            x: 1.0,
            y: 1.0,
            xanchor: "right".to_owned()
        },
            margin: Margin {
            t: 0.0,
            l: 50.0,
            r: 50.0,
            b: 50.0,
        },
            modebar: ModeBar {
            orientation: "v".to_owned(),
        },
            shapes: Vec::new(),
            showlegend: false,
            title: Title { text: "Clinical Review Timeline".to_owned(), y: 0.99 },
            xaxis: XAxis {
            range: Vec::new(),
            title: "Time".to_owned(),
            showgrid: false,
            tickformat: "%H:%M:%S".to_owned(),
        },
            yaxis: YAxis { visible: false, range: Vec::new() },
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Title {
    pub text: String,
    pub y: f64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Margin {
    pub t: f64,
    pub l: f64,
    pub r: f64,
    pub b: f64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct XAxis {
    pub range: Vec<String>,
    pub title: String,
    pub showgrid: bool,
    pub tickformat: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct YAxis {
    pub visible: bool,
    pub range: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ModeBar {
    pub orientation: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Legend {
    pub x: f64,
    pub y: f64,
    pub xanchor: String
}