use crate::action_csv_row::ActionCsvRow;
use crate::plot_structures::PlotLocation;
use std::cell::RefCell;
use std::collections::VecDeque;

pub struct CsvProcessingState {
    pub max_rows_to_check: usize,
    pub recent_rows: VecDeque<ActionCsvRow>,
    pub stage_boundaries: Vec<PlotLocation>,
    pub cpr_points: Vec<(PlotLocation, PlotLocation)>,
    pub pending_error_marker: RefCell<Option<(usize, ActionCsvRow)>>,
}

impl CsvProcessingState {
    pub fn new(max_rows_to_check: usize) -> Self {
        Self {
            max_rows_to_check,
            recent_rows: VecDeque::with_capacity(max_rows_to_check),
            stage_boundaries: vec![PlotLocation::default()],
            cpr_points: Vec::new(),
            pending_error_marker: RefCell::new(None),
        }
    }
}