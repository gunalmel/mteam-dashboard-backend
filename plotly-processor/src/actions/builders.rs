use crate::annotation::Annotation;
use crate::actions::compression_line::CompressionLine;
use crate::font::Font;
use crate::image::Image;
use crate::line::Line;
use crate::shape::Shape;
use mteam_dashboard_action_processor::plot_structures::PlotLocation;

pub fn create_image(x: String, y: String, source: String)->Image{
    Image{
        x,
        y,
        source,
        sizex: 10500.0,
        sizey: 1.5,
        xref: "x".to_owned(),
        yref: "y".to_owned(),
        xanchor: "center".to_owned(),
        yanchor: "middle".to_owned(),
        layer: "above".to_owned(),
        visible: true,
        sizing: "contain".to_owned(),
        opacity: 1,
    }
}
pub fn create_compression_line(start: PlotLocation, end: PlotLocation, y:String) -> CompressionLine {
    CompressionLine {
        x: vec![start.timestamp.date_string, end.timestamp.date_string],
        y: vec![y.clone(), y.clone()],
        hovertext: vec![start.timestamp.timestamp, end.timestamp.timestamp],
        text: "".to_owned(),
        mode: "lines".to_owned(),
        series_type: "scatter".to_owned(),
        hoverinfo: "text".to_owned(),
        textposition: "top center".to_owned(),
        textfont: Font {
            size: 16,
            color: None,
            family: None,
            weight: None
        },
        line: Line{
            width: None,
            color: Some("rgb(0, 150, 0)".to_owned()),
        }
    }
}
pub fn create_annotation(stage_name: String) -> Annotation {
    Annotation {
        text: stage_name,
        xref: "x".to_owned(),
        yref: "paper".to_owned(),
        x: "".to_owned(),
        y: 0.9,
        xanchor: "left".to_owned(),
        yanchor: "middle".to_owned(),
        showarrow: false,
        font: Font {
            size: 16,
            color: Some("".to_owned()),
            family: Some("Arial, sans-serif".to_owned()),
            weight: Some(700),
        },
        bgcolor: "rgba(255, 255, 255, 0.8)".to_owned(),
        bordercolor: "".to_owned(),
        borderwidth: 1,
        borderpad: 3,
    }
}
pub fn create_shape(start: &PlotLocation, end: &PlotLocation) -> Shape {
    Shape {
        x0: start.timestamp.date_string.to_owned(),
        x1: end.timestamp.date_string.to_owned(),
        fillcolor: "#".to_owned(),
        name: start.stage.1.to_owned(),
        y0: "".to_owned(),
        y1: "".to_owned(),
        shape_type: "rect".to_owned(),
        xref: "x".to_owned(),
        yref: "y".to_owned(),
        line: Line {
            width: Some(0),
            color: None,
        },
        layer: "below".to_owned(),
        location: (start.clone(), end.clone())
    }
}