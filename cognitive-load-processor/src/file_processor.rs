use std::io::{BufReader, Read};
use serde_json::de::Deserializer;
use serde_json::value::Value;
use crate::point_parser::map_time_to_date;

pub async fn process_cognitive_load_data(reader: &mut dyn Read) -> Result<impl Iterator<Item = (String, Option<f64>)>, String> {
    let root_array = parse_json_root(reader)?;

    Ok(root_array.into_iter().scan(None, |state, item| {
        map_time_to_date(item, *state).map(|(date_time, cognitive_load, first_timestamp)| {
            *state = first_timestamp;
            (date_time, cognitive_load)
        })
    }))
}

fn parse_json_root<R: Read>(reader: R) -> Result<Vec<Value>, String> {
    let buf_reader = BufReader::new(reader);
    let mut stream = Deserializer::from_reader(buf_reader).into_iter::<Value>();

    match stream.next() {
        Some(Ok(Value::Array(root))) => Ok(root),
        Some(Ok(_)) => Err("JSON root is not an array".to_string()),
        Some(Err(e)) => Err(format!("Error deserializing JSON root: {}", e)),
        None => Err("JSON is empty".to_string()),
    }
}

