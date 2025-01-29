use serde_json::value::Value;

pub(crate) fn map_time_to_date(visual_attention_data: Value, first_timestamp: Option<f64>) -> Option<(f64, Option<String>, Option<f64>)> {
    if let Value::Object(map) = visual_attention_data {
        let time = map.get("time")?.as_f64()?;
        let category = map
            .get("category")
            .and_then(|v| v.as_str().map(String::from));
        let start_seconds = first_timestamp.unwrap_or(time);
        let normalized_seconds = time - start_seconds;
        Some((
            normalized_seconds,
            category,
            Some(start_seconds),
        ))
    } else {
        None
    }
}