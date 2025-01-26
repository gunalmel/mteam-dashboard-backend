use crate::action_csv_row::ActionCsvRow;
use chrono::{DateTime, Datelike, TimeZone, Utc};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CsvRowTime {
    pub total_seconds: u32,
    pub date_string: String,
    pub timestamp: String,
}

impl Default for CsvRowTime {
    fn default() -> Self {
        let now_utc: DateTime<Utc> = Utc::now();
        let beginning_of_day_utc: DateTime<Utc> = Utc.with_ymd_and_hms(now_utc.year(), now_utc.month(), now_utc.day(), 0, 0, 0).unwrap();

        CsvRowTime {
            total_seconds: 0,
            date_string: beginning_of_day_utc.format("%Y-%m-%d %H:%M:%S").to_string(),
            timestamp: String::from("00:00:00")
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Serialize)]
pub struct PlotLocation {
    pub timestamp: CsvRowTime,
    pub stage: (u32, String)
}

impl PlotLocation {
    pub fn new(row: &ActionCsvRow) -> Self {
        Self {
            timestamp: row.timestamp.clone().unwrap_or(CsvRowTime::default()),
            stage: row.parsed_stage.clone().unwrap_or(PlotLocation::default().stage),
        }
    }
}

#[derive(Debug, PartialEq, Serialize, Clone)]
pub struct ErrorInfo {
    pub action_rule: String,
    pub violation: String,
    pub advice: String
}

impl ErrorInfo {
    pub fn new(row: &ActionCsvRow) -> Self {
        Self {
            action_rule: row.subaction_name.clone(),
            violation: row.score.clone(),
            advice: row.speech_command.clone(),
        }
    }
}

#[derive(Debug, PartialEq, Clone, Serialize)]
pub struct Action {
    pub location: PlotLocation,
    pub name: String,
    pub action_category: String,
    pub shock_value: String
}

impl Action {
    pub fn new(row: &ActionCsvRow) -> Self {
        Self {
            location:PlotLocation {
                timestamp: row.timestamp.clone().unwrap_or(CsvRowTime::default()),
                stage: row.parsed_stage.clone().unwrap_or(PlotLocation::default().stage),
            },
            name: row.action_name.clone(),
            action_category: row.action_category.clone(),
            shock_value: row.shock_value.clone(),
        }
    }
}

#[derive(Debug, PartialEq, Clone, Serialize)]
pub struct ErroneousAction {
    pub location: PlotLocation,
    pub name: String,
    pub action_category: String,
    pub shock_value: String,
    pub error_info: ErrorInfo
}

impl ErroneousAction {
    pub fn new(action_row: &ActionCsvRow, error_marker_row: &ActionCsvRow) -> Self {
        Self {
            location: PlotLocation::new(action_row),
            name: action_row.action_name.clone(),
            action_category: action_row.action_category.clone(),
            shock_value: action_row.shock_value.clone(),
            error_info: ErrorInfo::new(error_marker_row)
        }
    }
}

#[derive(Debug, PartialEq, Clone, Serialize)]
pub struct MissedAction {
    pub location: PlotLocation,
    pub name: String,
    pub error_info: ErrorInfo
}

impl MissedAction {
    pub(crate) fn new(row: &ActionCsvRow) -> MissedAction {
        MissedAction {
            location: PlotLocation::new(row),
            name: row.action_vital_name.clone(),
            error_info: ErrorInfo::new(row)
        }
    }
}

#[derive(Debug, PartialEq, Clone, Serialize)]
pub enum PeriodType {
    CPR,
    Stage
}

#[derive(Debug, PartialEq, Clone, Serialize)]
pub enum ActionPlotPoint {
    Error(ErroneousAction),
    Action(Action),
    MissedAction(MissedAction),
    Period(PeriodType, PlotLocation, PlotLocation)
}

