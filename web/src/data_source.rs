use serde_json::Value;
use std::error::Error;
use std::io::Read;
use async_trait::async_trait;

#[async_trait]
pub trait DataSource: Send + Sync + 'static {
    async fn get_main_folder_list(&self) -> Result<Vec<Value>, Box<dyn Error + Send + Sync>>;
    async fn fetch_json_reader(&self, file_id: String) -> Result<Box<dyn Read + Send + Sync>, String>;
    async fn fetch_csv_reader(&self, date_folder_id: String) -> Result<Box<dyn Read + Send + Sync>, String>;
    async fn fetch_json_file_map(&self, date_folder_id: &str, category_folder_name: &str, priority_list_to_order: Option<&Vec<String>>) -> Result<Vec<(String, String)>, String>;
}