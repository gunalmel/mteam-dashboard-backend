use crate::data_source::DataSource;
use crate::gdrive_provider::data_source_name_parser::gdrive_folder_to_location;
use crate::gdrive_provider::drive_hub_adapter::DriveHubAdapter;
use crate::gdrive_provider::google_drive_utils::build_drive_query;
use mteam_dashboard_utils::strings::snake_case_file_to_title_case;
use serde_json::Value;
use std::cmp::Ordering;
use std::error::Error;
use std::io::Read;
use std::sync::Arc;
use async_trait::async_trait;
use crate::config::config::DataSourceType;

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
            .filter_map(gdrive_folder_to_location) // Filters and maps files
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
}