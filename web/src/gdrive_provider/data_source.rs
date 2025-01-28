use serde_json::Value;
use std::error::Error;
use std::io::Read;
use std::collections::HashMap;
pub trait DataSource {
    async fn get_main_folder_list(&self) -> Result<Vec<Value>, Box<dyn Error>>;
    async fn fetch_json_reader(&self, file_id: String) -> Result<Box<dyn Read>, String>;
    async fn fetch_csv_reader(&self, date_folder_id: String) -> Result<Box<dyn Read>, String>;
    async fn fetch_json_file_map(&self, date_folder_id: &str, category_folder_name: &str) -> Result<HashMap<String, String>, String>;
}