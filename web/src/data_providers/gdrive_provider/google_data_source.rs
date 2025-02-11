use crate::data_source::DataSource;
use mteam_dashboard_utils::strings::snake_case_file_to_title_case;
use serde_json::Value;
use std::cmp::Ordering;
use std::error::Error;
use std::io::Read;
use std::pin::Pin;
use std::sync::Arc;
use async_trait::async_trait;
use bytes::Bytes;
use futures::{Stream, StreamExt, TryStreamExt};
use reqwest::Client;
use crate::config::config::DataSourceType;
use crate::data_providers::gdrive_provider::data_source_name_parser::folder_to_data_location;
use crate::data_providers::gdrive_provider::drive_hub_adapter::DriveHubAdapter;
use crate::data_providers::gdrive_provider::google_drive_utils::build_drive_query;

pub struct GoogleDriveDataSource {
    hub: Arc<dyn DriveHubAdapter + Send + Sync>,
    main_folder_id: String,
}

impl GoogleDriveDataSource {
    pub async fn new(main_folder_id: String, hub_wrapper: Arc<dyn DriveHubAdapter + Send + Sync>) -> Result<Self, Box<dyn Error>> {
        Ok(Self {
            hub: hub_wrapper,
            main_folder_id
        })
    }

    async fn get_subfolder_id(&self, parent_folder_id: &str, subfolder_name: &str) -> Result<String, String> {
        let query = format!("mimeType = 'application/vnd.google-apps.folder' and '{}' in parents and name = '{}' and trashed = false", parent_folder_id, subfolder_name);

        let subfolders = self.hub.fetch_files(query).await?;
        if subfolders.is_empty() {
            return Err(format!("Subfolder not found: {}", subfolder_name));
        }

        let subfolder_id = subfolders
            .get(0)
            .and_then(|file| file.id.as_ref())
            .ok_or_else(|| "Subfolder ID not found".to_string())?;

        Ok(subfolder_id.to_string())
    }

    async fn get_json_file_name_map(&self, folder_id: String, priority_list_to_order: Option<&Vec<String>>) -> Result<Vec<(String, String)>, String> {
        let query = format!("mimeType = 'application/json' and '{}' in parents and trashed = false", folder_id);

        let files = self.hub.fetch_files(query).await?;

        let mut file_vec: Vec<(String, String)> = files.into_iter()
            .map(|file| (snake_case_file_to_title_case(file.name.unwrap_or_default().as_str()), file.id.unwrap_or_default()))
            .collect();

        if let Some(list) = priority_list_to_order {
            if list.is_empty() {
                return Err("Priority list is empty".to_string());
            }

            let priority_list: Vec<&str> = list.iter().map(|s| s.as_str()).collect();

            // Sort the vector while preserving the priority order
            file_vec.sort_by(|(a, _), (b, _)| ordering_by_priority_list_then_alphabetically(a, b, &priority_list));
        }

        // Return Vec instead of BTreeMap to maintain order
        Ok(file_vec)
    }
}

fn ordering_by_priority_list_then_alphabetically<'a>(a: &'a str, b: &'a str, priority_list: &[&'a str]) -> Ordering {
    if let (Some(idx_a), Some(idx_b)) = (
        priority_list.iter().position(|x| *x == a),
        priority_list.iter().position(|x| *x == b)
    ) {
        idx_a.cmp(&idx_b)
    } else if priority_list.contains(&a) {
        Ordering::Less
    } else if priority_list.contains(&b) {
        Ordering::Greater
    } else {
        a.cmp(b)
    }
}

#[async_trait]
impl DataSource for GoogleDriveDataSource {
    fn data_source_type(&self) -> DataSourceType {
        DataSourceType::GoogleDrive
    }
    async fn get_main_folder_list(&self) -> Result<Vec<Value>, Box<dyn Error + Send + Sync>> {
        let query = build_drive_query(&self.main_folder_id, "and mimeType = 'application/vnd.google-apps.folder'");
        let folder_list = self.hub.fetch_files(query).await?;

        let mut files: Vec<Value> = folder_list
            .into_iter()
            .filter_map(folder_to_data_location) // Filters and maps files
            .collect();

        files.sort_by(|a, b| {
            let epoch_a = a["date"]["epoch"].as_i64().unwrap_or(0);
            let epoch_b = b["date"]["epoch"].as_i64().unwrap_or(0);
            epoch_a.cmp(&epoch_b)
        });

        Ok(files)
    }
    async fn fetch_json_reader(&self, file_id: String) -> Result<Box<dyn Read + Send + Sync>, String> {
        let data = self.hub.fetch_file_data(file_id).await?;
        Ok(Box::new(std::io::Cursor::new(data)))
    }
    async fn fetch_csv_reader(&self, folder_id: String) -> Result<Box<dyn Read + Send + Sync>, String> {
        let query = format!(
            "mimeType contains 'text/' and '{}' in parents and trashed = false",
            &folder_id
        );

        let csv_files = self.hub.fetch_files(query).await?;
        if csv_files.is_empty() {
            return Err(format!("No files found under the specified ID: {}", &folder_id));
        }

        let csv_file_id = csv_files
            .get(0)
            .and_then(|file| file.id.as_ref())
            .ok_or_else(|| "File ID not found".to_string())?;

        let data = self.hub.fetch_file_data(csv_file_id.to_string()).await?;
        Ok(Box::new(std::io::Cursor::new(data)))
    }
    async fn fetch_json_file_map(&self, source_folder_id: &str, sub_folder_name: &str, priority_list_to_order: Option<&Vec<String>>) -> Result<Vec<(String, String)>, String> {
        let folder_id = self.get_subfolder_id(source_folder_id, sub_folder_name).await?;
        self.get_json_file_name_map(folder_id, priority_list_to_order).await
    }

    async fn stream_video(
        &self,
        folder_id: String,
        range: Option<String>,
    ) -> Result<
        (
            u16,    // HTTP status code (206 for ranged requests, otherwise upstream status)
            String, // content type
            Option<u64>,  // content length of the bytes being sent
            Option<String>, // Content-Range header if applicable
            Pin<Box<dyn Stream<Item = Result<Bytes, Box<dyn Error + Send + Sync>>> + Send>>
        ),
        String
    > {
        // 1. Build a query for video files in the target folder.
        let query = format!(
            "mimeType contains 'video/' and '{}' in parents and trashed = false",
            folder_id
        );
        let video_files = self.hub.fetch_files(query).await?;
        if video_files.is_empty() {
            return Err(format!("No video files found under folder: {}", folder_id));
        }
        // Use the first video file.
        let video_file = video_files.get(0).ok_or_else(|| "Video file not found".to_string())?;
        let video_file_id = video_file
            .id
            .as_ref()
            .ok_or_else(|| "Video file ID not found".to_string())?;
        // Try to get total file size from file metadata.
        let total_length_from_file = video_file.size.unwrap_or(0);

        // 2. Get an access token.
        let access_token = self.hub.get_access_token().await?;

        // 3. Build the download URL using the Drive API endpoint.
        let url = format!(
            "https://www.googleapis.com/drive/v3/files/{}?alt=media",
            video_file_id
        );

        // 4. Create a reqwest client and prepare the GET request.
        let client = Client::new();
        let mut req_builder = client.get(&url).bearer_auth(access_token);
        if let Some(ref range_header) = range {
            req_builder = req_builder.header("Range", range_header);
        }
        let response = req_builder.send().await.map_err(|e| e.to_string())?;

        // 5. Capture upstream response headers.
        let upstream_status = response.status().as_u16();
        let content_type = response
            .headers()
            .get("Content-Type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("application/octet-stream")
            .to_string();
        let upstream_content_length = response
            .headers()
            .get("Content-Length")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok());
        let upstream_content_range = response
            .headers()
            .get("Content-Range")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        // Compute effective total size (prefer file metadata if available).
        let effective_total = if total_length_from_file > 0 {
            total_length_from_file
        } else {
            upstream_content_length.unwrap_or(0) as i64
        };

        // 6. Get the streaming response body.
        let stream = response
            .bytes_stream()
            .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)
            .boxed();

        // 7. If a Range header was provided, parse it and force a 206 response.
        if let Some(range_header) = range {
            if !range_header.starts_with("bytes=") {
                return Err("Invalid range header".to_string());
            }
            if effective_total == 0 {
                // If we cannot determine the total length, just return upstream values.
                return Ok((upstream_status, content_type, upstream_content_length, upstream_content_range, stream));
            }
            let range_part = &range_header[6..]; // Skip "bytes="
            let parts: Vec<&str> = range_part.split('-').collect();
            let start: u64 = parts.get(0)
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            let end: u64 = if let Some(end_str) = parts.get(1) {
                if end_str.is_empty() {
                    (effective_total - 1) as u64
                } else {
                    end_str.parse().map_err(|_| "Invalid end range")?
                }
            } else {
                (effective_total - 1) as u64
            };

            // --- CLAMPING LOGIC ---
            // If the requested start is >= effective_total, clamp to the last byte.
            let clamped = if start >= effective_total as u64 {
                true
            } else {
                false
            };
            let start = if clamped { effective_total.saturating_sub(1) } else { start as i64 };
            let end = if clamped { effective_total - 1 } else { end as i64 };
            // Validate again.
            if start > end || end >= effective_total {
                // Instead of erroring out, we now clamp.
                // (This should rarely occur because of our clamping above.)
                return Err("Range Not Satisfiable".to_string());
            }
            let byte_count = end - start + 1;
            // Compute Content-Range header.
            let computed_content_range = format!("bytes {}-{}/{}", start, end, effective_total);
            let final_content_range = upstream_content_range.or(Some(computed_content_range));
            Ok((206, content_type, Some(byte_count as u64), final_content_range, stream))
        } else {
            // No Range header provided; return upstream values.
            Ok((upstream_status, content_type, upstream_content_length, upstream_content_range, stream))
        }
    }
}