use serde_json::Value;
use crate::date_parser::seconds_to_csv_row_time;

pub fn map_time_to_date(item: Value, first_timestamp: Option<f64>) -> Option<(String, Option<f64>, Option<f64>)> {
    if let Value::Array(time_and_load) = item {
        if let (Some(elapsed_seconds), cognitive_load) = (time_and_load.get(0).and_then(Value::as_f64), time_and_load.get(1).and_then(Value::as_f64)) {
            let start_seconds = first_timestamp.unwrap_or(elapsed_seconds);
            let normalized_seconds = (elapsed_seconds - start_seconds) as u32;
           
            let csv_row_time = seconds_to_csv_row_time(normalized_seconds);
           
            return Some((csv_row_time.date_string, cognitive_load, Some(start_seconds)));
        }
    }
    None
}