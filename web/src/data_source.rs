use serde_json::Value;
use std::error::Error;
use std::io::Read;
use std::pin::Pin;
use async_trait::async_trait;
use bytes::Bytes;
use futures::Stream;
use crate::config::config::DataSourceType;

#[async_trait]
pub trait DataSource: Send + Sync + 'static {
    fn data_source_type(&self) -> DataSourceType;
    async fn get_main_folder_list(&self) -> Result<Vec<Value>, Box<dyn Error + Send + Sync>>;
    async fn fetch_json_reader(&self, file_id: String) -> Result<Box<dyn Read + Send + Sync>, String>;
    async fn fetch_csv_reader(&self, date_folder_id: String) -> Result<Box<dyn Read + Send + Sync>, String>;
    async fn fetch_json_file_map(&self, date_folder_id: &str, category_folder_name: &str, priority_list_to_order: Option<&Vec<String>>) -> Result<Vec<(String, String)>, String>;
    /// Streams a video file.
    /// Returns a tuple of:
    ///   - status_code: e.g. 200 or 206,
    ///   - content_type: MIME type (e.g., "video/mp4"),
    ///   - content_length: Optionally, the total file length,
    ///   - content_range: Optionally, the Content-Range header value,
    ///   - a streaming body.
    async fn stream_video(
        &self,
        folder_id: String,
        range: Option<String>,
    ) -> Result<
        (
            u16,
            String,
            Option<u64>,
            Option<String>,
            Pin<Box<dyn Stream<Item = Result<Bytes, Box<dyn Error + Send + Sync>>> + Send>>
        ),
        String
    >;
}