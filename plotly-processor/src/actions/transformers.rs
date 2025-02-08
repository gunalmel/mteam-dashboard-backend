use crate::config::plotly_mappings::PlotlyConfig;
use crate::actions::builders::{
    create_stage_annotation, create_image
    , create_shape,
};
use crate::actions::plot_data::{ActionGroup, ActionsPlotData, ActionsPlotDataCollector};
use mteam_dashboard_action_processor::plot_structures::{
    Action, ActionPlotPoint, ErroneousAction, MissedAction, PeriodType, PlotLocation,
};

fn append_to_plotly_data(data_point: Result<ActionPlotPoint, String>, data_collector: &mut ActionsPlotDataCollector, stage_index: &mut usize) {
    match data_point {
        Ok(ActionPlotPoint::Action(action)) => {
            process_action(&action, data_collector);
        }
        Ok(ActionPlotPoint::Error(action)) => {
            process_error(action, data_collector);
        }
        Ok(ActionPlotPoint::MissedAction(action)) => {
            process_missed_action(action, data_collector);
        }
        Ok(ActionPlotPoint::Period(PeriodType::CPR, start, end)) => {
            process_cpr_period(start, end, data_collector);
        }
        Ok(ActionPlotPoint::Period(PeriodType::Stage, start, end)) => {
            process_stage_period((start, end), data_collector, *stage_index);
            *stage_index+=1;
        },
        Err(_) => {}
    }
}

fn add_action(data_collector: &mut ActionsPlotDataCollector, group: ActionGroup, x: String, hover_text: String, text: String, color: String, stage: &(u32, String)){
    data_collector.actions_series.customdata.push(group.group_name);

    data_collector.actions_series.x.push(x.clone());
    data_collector.actions_series.y.push(group.y_value.to_string());
    data_collector.actions_series.hovertext.push(hover_text);
    data_collector.actions_series.text.push(text);
    data_collector.actions_series.marker.color.push(color);

    data_collector.add_action_stage(stage);

    let image = create_image(x.clone(), group.y_value.to_string(), group.icon);
    data_collector.actions_series.images.push(image);
}

fn process_action(action: &Action, data_collector: &mut ActionsPlotDataCollector) {
    let mut group = data_collector.create_action_group(&action.name);
    group.y_value = data_collector.get_y_for_action_group(&group.group_name);
    data_collector.performed_action_groups.insert(group.group_name.clone(), group.clone());
    let x = action.clone().location.timestamp.date_string;
    add_action(data_collector, group, x.clone(), format!("{}, {}", action.location.timestamp.timestamp, action.name), action.clone().shock_value, "green".to_owned(), &action.location.stage);
}

fn process_error(action: ErroneousAction, data_collector: &mut ActionsPlotDataCollector) {
    let mut group = data_collector.create_action_group(&action.name);
    group.y_value = data_collector.get_y_for_action_group(&group.group_name);
    data_collector.performed_action_groups.insert(group.group_name.clone(), group.clone());
    let x = action.location.timestamp.date_string;
    
    // let timestamp = action.location.timestamp.timestamp;
    let hover_text = if action.error_info.advice.is_empty() {
        format!("{}, {}", x, action.name)
    } else {
        format!(
            "{}, {}, {}",
            action.location.timestamp.timestamp, action.name, action.error_info.advice
        )
    };
    add_action(data_collector, group, x.clone(), hover_text, action.shock_value, "red".to_owned(), &action.location.stage);
}

fn process_missed_action(action: MissedAction, data_collector: &mut ActionsPlotDataCollector) {
    let group = data_collector.create_action_group(&action.name);
    let hover_text = if action.error_info.advice.is_empty() {
        action.name.clone()
    } else {
        format!("{} - {}", action.name, action.error_info.advice)
    };

    data_collector.missed_actions_series.hovertext.push(hover_text);
    data_collector.missed_actions_series.marker.color.push("rgba(249, 105, 14, 0.8)".to_owned());

    data_collector.add_missed_action_stage(&action.location.stage);
    
    let image = create_image("".to_owned(), "".to_owned(), group.icon);
    data_collector.missed_actions_series.images.push(image);

}

fn process_cpr_period(start: PlotLocation, end: PlotLocation, data_collector: &mut ActionsPlotDataCollector) {
    data_collector.add_compression_line(start, end);
}

fn process_stage_period(period: (PlotLocation, PlotLocation), data_collector: &mut ActionsPlotDataCollector, stage_idx: usize) {
    let (start, end) = period;
    if end.timestamp.total_seconds > data_collector.x_max_seconds as u32 {
        data_collector.x_max_seconds = end.timestamp.total_seconds as usize;
    }

    let color = data_collector.select_stage_color(stage_idx);
    let stage_color = color.clone()+"33";
    let annotation_color = color+"70";
    let mapped_stage_name = data_collector.map_stage_name(&start.stage.1);

    let mut shape_normal = create_shape(&start, &end);
    shape_normal.fillcolor=stage_color;
    shape_normal.name=mapped_stage_name.clone();

    let mut annotation = create_stage_annotation(mapped_stage_name.clone());
    annotation.font.color = Some(annotation_color.clone());
    annotation.bordercolor=annotation_color;
    annotation.x = start.timestamp.date_string;
    data_collector.layout.annotations.push(annotation);
    data_collector.layout.shapes.push(shape_normal);
}

pub fn to_plotly_data(plotly_config: &PlotlyConfig, data_points: impl Iterator<Item = Result<ActionPlotPoint, String>>) -> ActionsPlotData {
    let mut data_collector = ActionsPlotDataCollector::new(&plotly_config);

    let mut stage_index = 0;

    for data_point in data_points {
            append_to_plotly_data(
                data_point,
                &mut data_collector,
                &mut stage_index
            );
    }

    data_collector.to_plot_data()
}