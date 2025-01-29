use std::io::{BufReader, Read};
use serde_json::de::Deserializer;
use serde_json::value::Value;

 ///If a reader returns a JSON array, this function will parse it and return it as a Vec<Value>.
 ///e.g.: Your reader has a JSON array but nothing else: [{...}, {...}, {...}]
pub fn parse_json_array_root<R: Read>(reader: R) -> Result<Vec<Value>, String> {
    let buf_reader = BufReader::new(reader);
    let mut stream = Deserializer::from_reader(buf_reader).into_iter::<Value>();

    match stream.next() {
        Some(Ok(Value::Array(root))) => Ok(root),
        Some(Ok(_)) => Err("JSON root is not an array".to_string()),
        Some(Err(e)) => Err(format!("Error deserializing JSON root: {}", e)),
        None => Err("JSON is empty".to_string()),
    }
}