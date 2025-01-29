use std::io::Read;
use mteam_dashboard_utils::json::parse_json_array_root;
use crate::data_point_parser::map_time_to_date;

pub async fn process_cognitive_load_data(reader: &mut dyn Read) -> Result<impl Iterator<Item = (String, Option<f64>)>, String> {
    let root_array = parse_json_array_root(reader)?;

    Ok(root_array.into_iter().scan(None, |state, item| {
        map_time_to_date(item, *state).map(|(date_time, cognitive_load, first_timestamp)| {
            *state = first_timestamp;
            (date_time, cognitive_load)
        })
    }))
}