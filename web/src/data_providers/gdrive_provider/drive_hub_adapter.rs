use std::pin::Pin;
use std::future::Future;
use google_drive3::api::File;
pub trait DriveHubAdapter {
    fn fetch_files(&self, query: String) -> Pin<Box<dyn Future<Output = Result<Vec<File>, String>> + Send + '_>>;
    fn fetch_file_data(&self, file_id: String) -> Pin<Box<dyn Future<Output=Result<Vec<u8>, String>> + Send + '_>>;
    fn get_access_token(&self) -> Pin<Box<dyn Future<Output = Result<String, String>> + Send + '_>>;
}