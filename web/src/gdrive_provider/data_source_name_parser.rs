use crate::utils::date;
use chrono::NaiveDateTime;
use google_drive3::api::File;
use serde_json::{json, Value};

pub(crate) fn gdrive_folder_to_location(folder: File) -> Option<Value> {
    if let Some(name) = folder.name {
        let date_result = date::parse_date(&*name);
        match date_result {
            Ok(date) => {
                let json_result = create_json(&*folder.id.unwrap(), &*name, date);
                Some(json_result)
            }
            Err(_) => {
                println!("Debug: Folder {} does not seem to be a valid location to process for mteam data files. Skipping.", name);
                None
            }
        }
    } else {
        println!("Debug: File object passed from gdrive api doesn't have name. Skipping.");
        None
    }
}

fn create_json(id: &str, name: &str, date: NaiveDateTime) -> Value {
    json!({
        "name": name,
        "id": id,
        "date": {
            "epoch": date.and_utc().timestamp(),
            "dateString": date.format("%m/%d/%Y").to_string()
        }
    })
}
