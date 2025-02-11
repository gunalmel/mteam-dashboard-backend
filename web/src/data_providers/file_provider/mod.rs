use crate::config::config::DataSourceType;
use crate::data_source::DataSource;
use async_trait::async_trait;
use chrono::NaiveDate;
use mteam_dashboard_utils::date_parser;
use mteam_dashboard_utils::strings::snake_case_file_to_title_case;
use serde_json::Value;
use std::cmp::Ordering;
use std::error::Error;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use bytes::Bytes;
use futures::Stream;
use tokio::task;
use tokio::task::JoinHandle;
fn ordering_by_priority_list_then_alphabetically<'a>(a: &'a str, b: &'a str, priority_list: &[&'a str]) -> Ordering {
    if let (Some(idx_a), Some(idx_b)) = (
        priority_list.iter().position(|&x| x == a),
        priority_list.iter().position(|&x| x == b),
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

/// A local file–system implementation of `DataSource`.
///
/// The expected directory structure is:
///
/// ```text
/// /a/b/topmost_dir/         <-- root_dir
///    ├── 010125/         <-- “main folder” (e.g. a date folder)
///    │      ├── cognitive-load/   <-- category folder holding JSON files
///    │      │      ├── a_file.json
///    │      │      └── b_file.json
///    │      └── some.csv     <-- CSV (or text) file in the date folder
///    └── 010225/
///           └── visual-attention/
///                  ├── ...
/// ```
///
/// In this implementation the “file id” is simply the file’s relative path (or name).
pub struct LocalFileDataSource {
    root_dir: PathBuf,
}

impl LocalFileDataSource {
    pub fn new<P: AsRef<Path>>(root_dir: P) -> Self {
        Self {
            root_dir: root_dir.as_ref().to_path_buf(),
        }
    }

    fn get_file_reader(file_path: PathBuf) -> JoinHandle<Result<Box<dyn Read + Send + Sync>, String>> {
        let reader = task::spawn_blocking(|| {
            fs::File::open(file_path)
                .map(|f| Box::new(f) as Box<dyn Read + Send + Sync>)
                .map_err(|e| e.to_string())
        });
        reader
    }
}

#[async_trait]
impl DataSource for LocalFileDataSource {
    fn data_source_type(&self) -> DataSourceType {
        DataSourceType::LocalFile
    }

    async fn get_main_folder_list(&self) -> Result<Vec<Value>, Box<dyn Error + Send + Sync>> {
        // Clone the root directory so we can move it into the blocking closure.
        let root_dir = self.root_dir.clone();
        // Use block_in_place to run the blocking code on the current thread.
        let folders = task::spawn_blocking(|| -> Result<Vec<Value>, Box<dyn Error + Send + Sync>> {
            let mut list = Vec::new();
            for entry in fs::read_dir(root_dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_dir() {
                    // Use the folder name as its ID.
                    let folder_name = path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("")
                        .to_string();

                    let date = match date_parser::parse_date(&folder_name) {
                        Ok(date_result) => date_result,
                        Err(_) => {
                            println!("Debug: Folder {} does not seem to be a valid location to process for mteam data files. Skipping.", &folder_name);
                            NaiveDate::from_ymd_opt(1970, 1, 1).unwrap().and_hms_opt(0, 0, 0).unwrap()
                        }
                    };

                    // Build a JSON object similar to your Google Drive folder mapping.
                    let json_obj = serde_json::json!({
                        "id": folder_name,
                        "name": folder_name,
                        "date": { "epoch": date.and_utc().timestamp(),
                                  "dateString": date.format("%m/%d/%Y").to_string() 
                        }
                    });
                    list.push(json_obj);
                }
            }
            // Sort folders by epoch (older first).
            list.sort_by(|a, b| {
                let epoch_a = a["date"]["epoch"].as_u64().unwrap_or(0);
                let epoch_b = b["date"]["epoch"].as_u64().unwrap_or(0);
                epoch_a.cmp(&epoch_b)
            });
            Ok(list)
        });
        folders.await.unwrap()
    }

    async fn fetch_json_reader(&self, file_id: String) -> Result<Box<dyn Read + Send + Sync>, String> {
        let file_path = self.root_dir.join(file_id);
        let reader = Self::get_file_reader(file_path);
        reader.await.unwrap()
    }

    async fn fetch_csv_reader(&self, date_folder_id: String) -> Result<Box<dyn Read + Send + Sync>, String> {
        let folder_path = self.root_dir.join(date_folder_id);
        let csv_file_path = task::spawn_blocking(move || {
            let mut csv_path: Option<PathBuf> = None;
            for entry in fs::read_dir(&folder_path).map_err(|e| e.to_string())? {
                let entry = entry.map_err(|e| e.to_string())?;
                let path = entry.path();
                if path.is_file() {
                    if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                        if ext.eq_ignore_ascii_case("csv") || ext.eq_ignore_ascii_case("txt") {
                            csv_path = Some(path);
                            break;
                        }
                    }
                }
            }
            csv_path.ok_or_else(|| format!("No CSV file found in folder {:?}", folder_path))
        }).await.unwrap()?;
        let reader = Self::get_file_reader(csv_file_path); 
        
        reader.await.unwrap()
    }

    async fn fetch_json_file_map(&self, date_folder_id: &str, category_folder_name: &str, priority_list_to_order: Option<&Vec<String>>, ) -> Result<Vec<(String, String)>, String> {
        // Convert borrowed parameters to owned values so they can be used in the closure.
        let date_folder_id = date_folder_id.to_owned();
        let category_folder_name = category_folder_name.to_owned();
        let root_dir = self.root_dir.clone();
        // Clone the priority list if provided.
        let priority_list = priority_list_to_order.cloned();

        let file_map = task::spawn_blocking(move || -> Result<Vec<(String, String)>, String> {
            let folder_path = root_dir.join(&date_folder_id).join(&category_folder_name);
            let mut file_vec = Vec::new();
            for entry in fs::read_dir(&folder_path).map_err(|e| e.to_string())? {
                let entry = entry.map_err(|e| e.to_string())?;
                let path = entry.path();
                if path.is_file() {
                    if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                        if ext.eq_ignore_ascii_case("json") {
                            if let Some(file_name_os) = path.file_name() {
                                if let Some(file_name) = file_name_os.to_str() {
                                    let display_name = snake_case_file_to_title_case(file_name);
                                    // Here we use the file name as its “ID.”
                                    file_vec.push((display_name, file_name.to_string()));
                                }
                            }
                        }
                    }
                }
            }
            if let Some(priority) = priority_list {
                if priority.is_empty() {
                    return Err("Priority list is empty".to_string());
                }
                let priority_refs: Vec<&str> = priority.iter().map(|s| s.as_str()).collect();
                file_vec.sort_by(|(a, _), (b, _)| {
                    ordering_by_priority_list_then_alphabetically(a, b, &priority_refs)
                });
            }
            Ok(file_vec)
        });
        file_map.await.unwrap()
    }

    async fn stream_video(&self, folder_id: String, range: Option<String>) -> Result<(u16, String, Option<u64>, Option<String>, Pin<Box<dyn Stream<Item=Result<Bytes, Box<dyn Error + Send + Sync>>> + Send>>), String> {
        todo!()
    }
}
