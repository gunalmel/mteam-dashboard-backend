use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Line {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<i8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>
}