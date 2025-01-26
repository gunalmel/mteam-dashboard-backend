use crate::detection::{cpr_boundary, is_action_row, is_missed_action};
use crate::parsing::{extract_stage_name, parse_time, process_action_name};
use crate::plot_structures::CsvRowTime;
// This lets us write `#[derive(Deserialize)]`.
use serde::{Deserialize, Deserializer};
use std::fmt::{Display, Formatter};
/*
 * Used by serde macros to deserialize a non-empty string from a CSV file.
 */
fn non_empty_string<'de, D>(deserializer: D) -> Result<Option<CsvRowTime>, D::Error>
where
    D: Deserializer<'de>,
{
    let value: Option<String> = Option::deserialize(deserializer)?;
    match value {
        Some(s) if !s.trim().is_empty() => Ok(parse_time(&s[..])),
        _ => Err(serde::de::Error::custom("Field cannot be empty")),
    }
}

pub const COLUMN_NAMES: [&str; 9] = [
    "Time Stamp[Hr:Min:Sec]",
    "Action/Vital Name",
    "SubAction Time[Min:Sec]",
    "SubAction Name",
    "Score",
    "Old Value",
    "New Value",
    "Username",
    "Speech Command",
];
#[derive(Default, Debug, Deserialize, Clone)]
#[serde(rename_all = "PascalCase")] //interpret each field in PascalCase, where the first letter of the field is capitalized
pub struct ActionCsvRow {
    #[serde(rename = "Time Stamp[Hr:Min:Sec]", deserialize_with = "non_empty_string")]
    pub timestamp: Option<CsvRowTime>,
    #[serde(rename = "Action/Vital Name")]
    pub action_vital_name: String,
    #[serde(default, rename = "SubAction Time[Min:Sec]")]
    pub subaction_time: String,
    #[serde(default, rename = "SubAction Name")]
    pub subaction_name: String,
    #[serde(default, rename = "Score")]
    pub score: String,
    #[serde(default, rename = "Old Value")]
    pub old_value: String,
    #[serde(default, rename = "New Value")]
    pub new_value: String,
    #[serde(default)]
    pub username: String,
    #[serde(default, rename = "Speech Command")]
    pub speech_command: String,
    
    #[serde(skip)]
    pub parsed_stage: Option<(u32, String)>,
    #[serde(skip)]
    pub action_name: String,
    #[serde(skip)]
    pub action_category: String,
    #[serde(skip)]
    pub shock_value: String,
    #[serde(skip)]
    pub action_point: bool,
    #[serde(skip)]
    pub cpr_boundary: Option<String>
}

impl Display for ActionCsvRow {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "ActionCsvRow {{ timestamp: {:?}, action_vital_name: {:?}, subaction_time: {:?}, subaction_name: {:?}, score: {:?}, old_value: {:?}, new_value: {:?}, username: {:?}, speech_command: {:?}, parsed_stage: {:?}, action_name: {:?}, action_category: {:?}, shock_value: {:?}, action_point: {:?}, cpr_boundary: {:?} }}",
            self.timestamp,
            self.action_vital_name,
            self.subaction_time,
            self.subaction_name,
            self.score,
            self.old_value,
            self.new_value,
            self.username,
            self.speech_command,
            self.parsed_stage,
            self.action_name,
            self.action_category,
            self.shock_value,
            self.action_point,
            self.cpr_boundary
        )
    }
}

impl ActionCsvRow {
    pub fn post_deserialize(&mut self) {
        self.parsed_stage = if is_missed_action(&self) {extract_stage_name(&self.username)} else { extract_stage_name(&self.action_vital_name) };
        self.cpr_boundary = cpr_boundary(&self);
        self.action_point = is_action_row(&self);
        let processed_action_name = process_action_name(&self.subaction_name);
        self.action_name = processed_action_name.0;
        self.action_category = processed_action_name.1;
        self.shock_value = processed_action_name.2;
    }
}