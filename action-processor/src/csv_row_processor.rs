use crate::action_csv_row::ActionCsvRow;
use crate::plot_processors::{process_action_point, process_cpr_lines, process_erroneous_action, process_stage_boundary};
use crate::plot_structures::ActionPlotPoint;
use crate::processing_state::CsvProcessingState;
use csv::StringRecord;
use std::collections::VecDeque;

fn parse_csv_row(result: Result<StringRecord, csv::Error>) -> Result<ActionCsvRow, String> {
    result
        .and_then(|raw_row| {
            let mut csv_row: ActionCsvRow = raw_row.deserialize(None)?;
            csv_row.post_deserialize();
            Ok(csv_row)
        })
        .map_err(|e| format!("Could not deserialize row: {}", e))
}

pub fn process_csv_row(row_idx: usize, result: Result<StringRecord, csv::Error>, state: &mut CsvProcessingState) -> Option<Result<ActionPlotPoint, String>> {
    let current_row = match parse_csv_row(result) {
        Ok(row) => row,
        Err(e) => return Some(Err(e)),
    };

    let point = process_stage_boundary(&mut state.stage_boundaries, &current_row)
        .or_else(|| process_cpr_lines(&mut state.cpr_points, &current_row))
        .or_else(|| process_erroneous_action(state, row_idx, &current_row))
        .or_else(|| state.recent_rows
            .pop_front()
            .and_then(|recent_row| process_action_point(&recent_row))
            );
    
    if !matches!(point, Some(Ok(ActionPlotPoint::Error(_)))) {
        update_recent_actions(&current_row, &mut state.recent_rows, state.max_rows_to_check);
    }
    point
}

fn update_recent_actions(current_row: &ActionCsvRow, recent_rows: &mut VecDeque<ActionCsvRow>, max_rows: usize) {
    if current_row.action_point {
        recent_rows.push_back(current_row.clone());
    }
    if recent_rows.len() > max_rows {
        recent_rows.pop_front();
    }
}