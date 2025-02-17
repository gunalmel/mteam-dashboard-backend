use crate::config::plotly_mappings::PlotlyConfig;
use crate::visual_attention::plot_data::VisualAttentionCategory;
use mteam_dashboard_visual_attention_processor::file_processor::process_visual_attention_data;
use std::collections::HashMap;
use std::io;
use std::io::Read;

pub fn to_plotly_data(reader: &mut impl Read, window_duration_secs: u32, config: &PlotlyConfig) -> Result<Vec<VisualAttentionCategory>, io::Error> {
    let ref mut category_map = HashMap::new();

    process_visual_attention_data(reader, window_duration_secs)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))? // Convert String error to io::Error
        .for_each(|(category, time, ratio)| {
            let point = category_map.entry(category.clone()).or_insert_with(|| VisualAttentionCategory {
                x: vec![],
                y: vec![],
                name: category,
                plot_type: "bar".to_owned(),
                marker: HashMap::new()
            });
            point.x.push(time);
            point.y.push(ratio);
        });

    let mut ordered_categories = Vec::new();

    for (category, color) in &config.visual_attention_plot_settings.ordered_category_color_tuples {
        if let Some(mut category_data) = category_map.remove(category) {
            category_data.marker.insert("color".to_owned(), color.to_owned());
            ordered_categories.push(category_data);
        }
    }
    Ok(ordered_categories)
}