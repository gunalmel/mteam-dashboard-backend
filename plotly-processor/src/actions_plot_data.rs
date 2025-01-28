use crate::actions_plot_compression_line::CompressionLine;
use crate::config::plotly_mappings::PlotlyConfig;
use crate::font::Font;
use crate::image::Image;
use crate::layout::Layout;
use crate::missed_action_coordinates_calculator::{seconds_to_date_time_string, MissedActionsCoordinatesIterator, Rectangle};
use crate::actions_plot_builders::create_compression_line;
use crate::actions_plot_data::ActionsPlotDataItem::{Lines, Points};
use crate::shape::Shape;
use mteam_dashboard_action_processor::plot_structures::PlotLocation;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

#[derive(Serialize, Deserialize, Debug)]
pub struct Marker {
    pub size: i8,
    pub symbol: String,
    pub color: Vec<String>
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ActionsPlotSeries {
    pub x: Vec<String>,
    pub y: Vec<String>,
    pub text: Vec<String>,
    pub mode: String,
    #[serde(rename = "type")]
    pub series_type: String,
    pub hoverinfo: String,
    pub textposition: String,
    pub textfont: Font,
    pub hovertext: Vec<String>,
    pub customdata: Vec<String>,
    pub marker: Marker,
    #[serde(skip)]
    pub images: Vec<Image>,
    #[serde(skip)]
    pub stages: Vec<(u32, String)>,
    #[serde(skip)]
    pub stage_action_counts: HashMap<String, u16>
}

impl ActionsPlotSeries {
    pub fn new() -> Self {
        ActionsPlotSeries {
            customdata: Vec::new(),
            hoverinfo: "text".to_owned(),
            hovertext: Vec::new(),
            marker: Marker{
                size: 24,
                symbol: "square".to_owned(),
                color: Vec::new()
            },
            mode: "text+markers".to_owned(),
            x: Vec::new(),
            y: Vec::new(),
            series_type: "text".to_owned(),
            text: Vec::new(),
            textfont: Font {
                size: 10, color: None, family: None, weight: None
            },
            textposition: "bottom center".to_owned(),
            images: Vec::new(),
            stages: Vec::new(),
            stage_action_counts: HashMap::new()
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(untagged)] //ScatterPlotDataItem enum should be serialized and deserialized without tags.
pub enum ActionsPlotDataItem {
    Points(ActionsPlotSeries),
    Lines(CompressionLine),
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ActionsPlotData {
    pub data: Vec<ActionsPlotDataItem>,
    pub layout: Layout,
    #[serde(rename = "actionGroupIcons")]
    pub action_group_icons: BTreeMap<String, String>,
}

#[derive(Clone)]
pub struct ActionGroup {
    pub(crate) group_name: String,
    pub(crate) icon: String,
    pub(crate) y_value: f32,
}

pub struct ActionsPlotDataCollector <'a> {
    pub actions_series: ActionsPlotSeries,
    pub missed_actions_series: ActionsPlotSeries,
    pub scatter_data: Vec<ActionsPlotDataItem>,
    pub layout: Layout,
    pub performed_action_groups: BTreeMap<String, ActionGroup>,
    pub x_max_seconds: usize,
    pub y_max: f32,
    pub plotly_config: &'a PlotlyConfig
}

impl<'a> ActionsPlotDataCollector<'a> {
    pub fn new(plotly_config: &'a PlotlyConfig) -> Self {
        Self {
            actions_series: ActionsPlotSeries::new(),
            missed_actions_series: ActionsPlotSeries::new(),
            scatter_data: Vec::new(),
            layout: Layout::new(),
            performed_action_groups: BTreeMap::new(),
            x_max_seconds: 0, // will be calculated while processing
            y_max: plotly_config.action_plot_settings.y_increment*2.0, //this is y_max value of actions, will be used to assign the starting y value for the first action group points, immutable
            plotly_config
        }
    }
    pub fn increment_y_max(&mut self) {
        self.y_max += self.plotly_config.action_plot_settings.y_increment;
    }

    pub fn select_stage_color(&self, index: usize) -> String {
        let colors = &self.plotly_config.stages.colors;
        colors.get(index % colors.len()).cloned().unwrap_or_else(|| "#ffffff".to_owned())
    }

    pub fn map_stage_name(&self, stage_name: &str) -> String {
        self.plotly_config.stages.names.get(stage_name).cloned().unwrap_or_else(|| stage_name.to_owned())
    }
    
    pub fn add_compression_line(&mut self, start: PlotLocation, end: PlotLocation){
        let compression_line = create_compression_line(start, end, self.plotly_config.action_plot_settings.y_increment.to_string());
        self.scatter_data.push(Lines(compression_line));   
    }
    
    pub fn add_action_stage(&mut self, stage: &(u32, String)) {
        let mapped_stage_name = self.map_stage_name(&stage.1);
        self.actions_series.stages.push((stage.0, mapped_stage_name));
    }

    pub fn add_missed_action_stage(&mut self, stage: &(u32, String)) {
        let mapped_stage_name = self.map_stage_name(&stage.1);
        self.missed_actions_series.stages.push((stage.0, mapped_stage_name.clone()));
        *self.missed_actions_series.stage_action_counts.entry(mapped_stage_name).or_insert(0) += 1;
    }

    pub fn get_y_for_action_group(&mut self, group_name: &String) -> f32{
        let y_value = if let Some(ActionGroup{y_value: existing_y ,..}) = self.performed_action_groups.get(group_name) {
            *existing_y
        } else {
            self.increment_y_max();
            self.y_max - self.plotly_config.action_plot_settings.y_increment
        };
        y_value
    }

    pub fn create_action_group(&mut self, action_name: &str) -> ActionGroup {
        let action_group_name = self.plotly_config.get_action_group_name(action_name);
        let action_group_icon = self.plotly_config.get_action_group_icon(&action_group_name);
        let action_group = ActionGroup {
            group_name: action_group_name.clone(),
            icon: action_group_icon,
            y_value: 0.0,
        };
        action_group
    }

    pub fn update_y_coordinates(&mut self) {
        let max_missed_actions_count_per_stage = self.missed_actions_series.stage_action_counts.values().max().unwrap();
        let missed_actions_y_max = self.plotly_config.action_plot_settings.missed_actions.calculate_y_max(*max_missed_actions_count_per_stage); // TODO: need to work on keeping in sync with y-axis range values

        self.layout.yaxis.range.push(missed_actions_y_max.to_string());
        self.layout.yaxis.range.push((self.y_max + 2.0*self.plotly_config.action_plot_settings.y_increment).to_string());
        
        let mut missed_actions_stages_shapes: Vec<Shape> = Vec::new();
        let mut missed_actions_rectangles: HashMap<String, Rectangle> = HashMap::new();
        
        self.layout.shapes.iter_mut().enumerate().for_each(|(action_shape_index, action_shape)| {
            action_shape.y0 = self.plotly_config.action_plot_settings.y_min.to_string();
            action_shape.y1 = (self.y_max + self.plotly_config.action_plot_settings.y_increment).to_string();
            
            let mut missed_actions_shape = action_shape.clone();
            missed_actions_shape.y0 = self.plotly_config.action_plot_settings.missed_actions.y_min.to_string();
            missed_actions_shape.y1 = (missed_actions_y_max+2.5*self.plotly_config.action_plot_settings.missed_actions.y_increment).to_string();
            missed_actions_stages_shapes.push(missed_actions_shape.clone());

            let missed_actions_rectangle = Rectangle{
                name: missed_actions_shape.name,
                x0: missed_actions_shape.location.0.timestamp.total_seconds as f32,
                x1: missed_actions_shape.location.1.timestamp.total_seconds as f32,
                y0: self.plotly_config.action_plot_settings.missed_actions.y_min,
                y1: missed_actions_y_max
            };
            missed_actions_rectangles.insert(missed_actions_rectangle.name.clone(), missed_actions_rectangle);
            
            self.layout.annotations[action_shape_index].y = self.plotly_config.action_plot_settings.y_annotation;
        });
        
        let missed_action_calculation_iterator = MissedActionsCoordinatesIterator::new(&self.missed_actions_series.hovertext, &self.missed_actions_series.stages ,&missed_actions_rectangles, &self.missed_actions_series.stage_action_counts, self.plotly_config.action_plot_settings.missed_actions.max_count_per_row as usize);
        missed_action_calculation_iterator.enumerate().for_each(|(action_index, missed_action)| {
            self.missed_actions_series.x.push(missed_action.1.clone());
            self.missed_actions_series.y.push(missed_action.2.to_string());
            self.missed_actions_series.images[action_index].x = missed_action.1.clone();
            self.missed_actions_series.images[action_index].y = missed_action.2.to_string();
        });
        self.layout.images.extend(self.actions_series.images.clone());
        self.layout.images.extend(self.missed_actions_series.images.clone());
        self.layout.shapes.extend(missed_actions_stages_shapes);
        
        self.layout.xaxis.range.push(seconds_to_date_time_string(0f32));
        self.layout.xaxis.range.push(seconds_to_date_time_string((self.x_max_seconds + self.plotly_config.action_plot_settings.x_axis_padding_secs) as f32));
    }
    
    pub fn to_plot_data(mut self) -> ActionsPlotData {
        self. update_y_coordinates();
        self.scatter_data.push(Points(self.actions_series));
        self.scatter_data.push(Points(self.missed_actions_series));
        let action_groups = self.performed_action_groups.into_iter()
            .map(|(key, value)| (key, value.icon))
            .collect();
        ActionsPlotData {
            data: self.scatter_data,
            layout: self.layout,
            action_group_icons: action_groups
        }
    }
}