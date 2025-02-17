use serde_json::value::Value;
use std::io::{BufReader, Read};

///If a reader returns a JSON array, this function will parse it and return it as a Vec<Value>.
 ///e.g.: Your reader has a JSON array but nothing else: [{...}, {...}, {...}]
 pub fn parse_json_array_root<R: Read>(reader: R) -> Result<Vec<Value>, String> {
     // Read the entire JSON input into a String
     let mut raw_json = String::new();
     BufReader::new(reader)
         .read_to_string(&mut raw_json)
         .map_err(|e| format!("Error reading JSON: {}", e))?;

     // Replace invalid NaN tokens with valid null tokens.
     let sanitized_json = raw_json.replace("NaN", "null");

     // parse the sanitized JSON.
     let root: Value = serde_json::from_str(&sanitized_json)
         .map_err(|e| format!("Error deserializing JSON root: {}", e))?;

     match root {
         Value::Array(arr) => Ok(arr),
         _ => Err("JSON root is not an array".to_string()),
     }
 }
