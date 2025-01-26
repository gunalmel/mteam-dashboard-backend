use crate::line::Line;
use mteam_dashboard_action_processor::plot_structures::PlotLocation;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Shape {
    pub x0: String,
    pub x1: String,
    pub fillcolor: String,
    pub name: String,
    pub y0: String,
    pub y1: String,
    #[serde(rename = "type")]
    pub shape_type: String,
    pub xref: String,
    pub yref: String,
    pub line: Line,
    pub layer: String,
    #[serde(skip)]
    pub location: (PlotLocation, PlotLocation)
}