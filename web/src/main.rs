use crate::app_context::AppContext;
use crate::gdrive_provider::google_data_source::GoogleDriveDataSource;
use crate::gdrive_provider::google_drive_hub_adapter_builder::GoogleDriveHubAdapterBuilder;
use actix_web::{get, middleware, web, App, HttpResponse, HttpServer, Responder};
use async_stream::stream;
use bytes::{Bytes, BytesMut};
use config::config::AppConfig;
use config::resolve_file_path::{resolve_config_file_path, resolve_first_path};
use futures::StreamExt;
use futures::{stream, Stream};
use gdrive_provider::data_source::DataSource;
use log::debug;
use mteam_dashboard_action_processor::plot_structures::CsvRowTime;
use mteam_dashboard_action_processor::process_csv;
use mteam_dashboard_utils::date_parser::seconds_to_csv_row_time;
use mteam_dashboard_cognitive_load_processor::file_processor::process_cognitive_load_data;
use mteam_dashboard_plotly_processor::actions_plot_data::ActionsPlotData;
use mteam_dashboard_plotly_processor::actions_plot_data_transformers::to_plotly_data;
use mteam_dashboard_plotly_processor::config::init::init_plot_config;
use mteam_dashboard_plotly_processor::config::plotly_mappings::PlotlyConfig;
use serde::{Deserialize, Serialize};
use serde_json::{from_str, json, to_string, Deserializer, Value};
use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::error::Error;
use std::io::{BufReader, Cursor, Read};
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;
use std::{env, io};
use mteam_dashboard_utils::json::parse_json_array_root;
use mteam_dashboard_visual_attention_processor::file_processor::process_visual_attention_data;

mod app_context;
mod config;
mod gdrive_provider;

async fn data_sources(context: web::Data<AppContext>) -> impl Responder {
    match context.datasource_provider.get_main_folder_list().await {
        Ok(folders) => HttpResponse::Ok().json(folders),
        Err(e) => HttpResponse::NotFound().json(
            json!({"Data sources not found": format!("Failed to get data sources: {:#?}", e)}),
        ),
    }
}
async fn plot_sources(
    path: web::Path<(String, String)>,
    context: web::Data<AppContext>,
) -> impl Responder {
    let (data_source_id, plot_name) = path.into_inner();
    match context.datasource_provider.fetch_json_file_map(data_source_id.as_str(), plot_name.as_str()).await {
        Ok(file_name_map) => HttpResponse::Ok().json(file_name_map),
        Err(e) => HttpResponse::NotFound().json(json!({"Not found": format!("Failed to get file list for the selected data source and plot name: {:#?}", e)}))
    }
}
async fn test_actions(id: web::Path<String>, context: web::Data<AppContext>) -> impl Responder {
    let reader = context
        .datasource_provider
        .fetch_csv_reader(id.to_string())
        .await
        .unwrap();
    let actions_iterator = process_csv(reader, 10);

    // Convert the iterator to a stream of JSON strings
    let json_stream = stream::iter(actions_iterator)
        .filter_map(|result| async move {
            match result {
                Ok(item) => to_string(&item).ok(), // Serialize to JSON
                Err(err) => {
                    eprintln!("Error processing item: {}", err);
                    None // Skip errored items
                }
            }
        })
        .enumerate() // Add an index to each item
        .map(|(i, json)| {
            // Prepend a comma for all but the first item
            let prefix = if i > 0 { "," } else { "" };
            Ok(Bytes::from(format!("{}{}", prefix, json))) as Result<Bytes, actix_web::Error>
        });

    // Open and close brackets
    let open_bracket = stream::once(async { Ok(Bytes::from("[")) });
    let close_bracket = stream::once(async { Ok(Bytes::from("]")) });

    // Combine everything into a single stream
    let body = open_bracket.chain(json_stream).chain(close_bracket);

    HttpResponse::Ok()
        .content_type("application/json")
        .streaming(body)
}
async fn actions(id: web::Path<String>, context: web::Data<AppContext>) -> impl Responder {
    // Get the CSV reader
    let reader = match context
        .datasource_provider
        .fetch_csv_reader(id.to_string())
        .await
    {
        Ok(r) => r,
        Err(_) => return HttpResponse::NotFound().body("Failed to get actions reader"),
    };

    // Process the CSV data
    let actions_iterator = process_csv(reader, 10);
    let actions_plot_data: ActionsPlotData =
        to_plotly_data(context.plotly_config, actions_iterator);

    // Serialize the data
    match to_string(&actions_plot_data) {
        Ok(json) => HttpResponse::Ok()
            .content_type("application/json")
            .body(json),
        Err(_) => HttpResponse::InternalServerError().body("Failed to serialize result"),
    }
}
async fn cognitive_load(id: web::Path<String>, context: web::Data<AppContext>) -> impl Responder {
    let mut file_reader = context
        .datasource_provider
        .fetch_json_reader(id.to_string())
        .await
        .map_err(|e| e.to_string())
        .unwrap();
    match process_cognitive_load_data(&mut *file_reader).await {
        Ok(iterator) => {
            let stream = stream! { // Start the JSON object
                yield Ok(Bytes::from("{\"x\":[".to_string()));
                let mut y_bytes = BytesMut::new();
                y_bytes.extend_from_slice(b"],\"y\":[");

                let mut first = true;

                for (x, y) in iterator {
                    if !first {
                        y_bytes.extend_from_slice(b",");
                        yield Ok(Bytes::from(",".to_string()));
                    }
                    first = false;
                    let x_point = json!(x);
                    let y_point = json!(y);
                    y_bytes.extend_from_slice(to_string(&y_point).unwrap().as_bytes());
                    yield Ok(Bytes::from(to_string(&x_point).unwrap()));
                }

                yield Ok(y_bytes.freeze());

                // Close the `y` array and add other fields
                yield Ok(Bytes::from("],\"mode\":\"lines\",\"type\":\"scatter\"}".to_string()));
            };
            let body: Pin<Box<dyn Stream<Item = Result<Bytes, io::Error>> + Send>> =
                Box::pin(stream);
            HttpResponse::Ok()
                .content_type("application/json")
                .streaming(body)
        }
        Err(err) => HttpResponse::InternalServerError()
            .json(json!({"error": "Failed to process cognitive load data", "details": err})),
    }
}
#[get("/")]
async fn hello() -> impl Responder {
    HttpResponse::Ok().body("Hello world!")
}
const CREDENTIALS_FILE_HOME: &str =
    "/Users/gunalmel/Downloads/mteam-dashboard-447216-9836ce4f74a2.json";

// #[actix_web::main]
// async fn main() -> io::Result<()> {
//     env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));
//
//     let config = get_app_config().unwrap();
//     let plotly_config = get_plotly_config(&config);
//     let datasource_provider = get_datasource_provider(&config).await;
//
//     HttpServer::new(move || {
//         App::new()
//             .wrap(middleware::Compress::default())
//             .app_data(web::Data::new(AppContext {
//                 datasource_provider: datasource_provider.clone(),
//                 plotly_config
//             }))
//             .service(hello)
//             .route("/data-sources", web::get().to(data_sources))
//             .route("/data-sources/{data_source_id}/{plot_name}", web::get().to(plot_sources))
//             .route("/actions/raw/{id}", web::get().to(test_actions))
//             .route("/actions/plotly/{id}", web::get().to(actions))
//             .route("/cognitive-load/plotly/{id}", web::get().to(cognitive_load))
//     })
//         .bind(("0.0.0.0", 8080))?
//         .run()
//         .await
// }

async fn get_datasource_provider(config: &AppConfig) -> Arc<GoogleDriveDataSource> {
    let gdrive_credentials_file = resolve_first_path(&[
        config.gdrive_credentials_file.as_str(),
        CREDENTIALS_FILE_HOME,
    ])
    .unwrap();
    debug!(
        "Using gdrive credentials file: {:?}",
        gdrive_credentials_file
    );

    let builder = GoogleDriveHubAdapterBuilder::new()
        .with_credentials(gdrive_credentials_file)
        .with_scope("https://www.googleapis.com/auth/drive.readonly".to_string());

    let hub_adapter = builder
        .build()
        .await
        .expect("Failed to build GoogleDriveHubAdapter");

    Arc::new(
        GoogleDriveDataSource::new(config.gdrive_root_folder_id.clone(), hub_adapter)
            .await
            .expect("Failed to initialize GoogleDriveDataSource"),
    )
}

fn get_plotly_config(app_config: &AppConfig) -> &'static PlotlyConfig {
    let plot_config_path = resolve_first_path(&[app_config.plot_config_path.as_str()]).unwrap();
    debug!("Using plot config path: {:#?}", plot_config_path);
    let plot_config = init_plot_config(plot_config_path).unwrap().unwrap();
    debug!("Loaded plot config: {:#?}", plot_config);
    plot_config
}

fn get_app_config() -> Result<AppConfig, Box<dyn Error>> {
    let args: Vec<String> = env::args().collect();
    // Get the configuration file path
    let config_path = resolve_config_file_path(&args, &vec!["config.json"])?;
    debug!("Using configuration file: {:?}", config_path);

    // Load the configuration file as a struct
    let config: AppConfig = serde_json::from_reader(std::fs::File::open(config_path)?)?;
    debug!("Loaded config: {:#?}", config);
    Ok(config)
}

#[actix_web::main]
async fn main() -> io::Result<()> {
    let data = r#"[{
"time": 0.8,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 0.8,
"object": "Middle Part Vital Cognitive",
"category": "Team"
},
{
"time": 0.9,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 5.8,
"object": "Middle Part Vital Cognitive",
"category": "Team"
},
{
"time": 6.6,
"object": "Middle Part Vital Cognitive",
"category": "Patient"
},
{
"time": 7.8,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 8.6,
"object": "Middle Part Vital Cognitive",
"category": "Tablet"
},
{
"time": 8.7,
"object": "Middle Part Vital Cognitive",
"category": "Patient"
},
{
"time": 9.8,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 10,
"object": "Middle Part Vital Cognitive",
"category": "Equipment"
},
{
"time": 10.9,
"object": "Middle Part Vital Cognitive",
"category": "Equipment"
}]"#;

    let stream: Vec<Value> = from_str(data).unwrap(); // Deserialize once

    let window_duration_secs = 10;
    let cursor = Cursor::new(serde_json::to_vec(&stream).unwrap()); // Convert to Cursor

    let mut reader = cursor;
    process_visual_attention_data(&mut reader, window_duration_secs)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))? // Convert String error to io::Error
        .for_each(|(category, time, ratio)| {
            println!("Time: {}, Category: {}, Ratio: {}", time, category, ratio);
        });


    Ok(())
}