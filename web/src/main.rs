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
use mteam_dashboard_cognitive_load_processor::date_parser::seconds_to_csv_row_time;
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

mod app_context;
mod config;
mod gdrive_provider;
mod utils;

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
async fn main() -> std::io::Result<()> {
    let data = r#"[
{
"time": 1727713666.6,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713666.8,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713666.9,
"object": null,
"category": null
},
{
"time": 1727713667.4,
"object": "NetworkUser2",
"category": "Team"
},
{
"time": 1727713667.7,
"object": null,
"category": null
},
{
"time": 1727713667.9,
"object": null,
"category": null
},
{
"time": 1727713668.2,
"object": "NetworkUser1",
"category": "Team"
},
{
"time": 1727713668.6,
"object": null,
"category": null
},
{
"time": 1727713669,
"object": "Pump_Mask_Interactable_R(Clone)",
"category": "Equipment"
},
{
"time": 1727713669.1,
"object": null,
"category": null
},
{
"time": 1727713669.4,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727713669.9,
"object": null,
"category": null
},
{
"time": 1727713670.5,
"object": null,
"category": null
},
{
"time": 1727713670.9,
"object": null,
"category": null
},
{
"time": 1727713671.1,
"object": "Patient Left Arm",
"category": "Patient"
},
{
"time": 1727713671.6,
"object": null,
"category": null
},
{
"time": 1727713671.9,
"object": "Patient Left Leg",
"category": "Patient"
},
{
"time": 1727713672.1,
"object": "Defibrillator(Clone)",
"category": "Equipment"
},
{
"time": 1727713672.5,
"object": "Patient Left Leg",
"category": "Patient"
},
{
"time": 1727713673.1,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727713673.8,
"object": null,
"category": null
},
{
"time": 1727713673.9,
"object": null,
"category": null
},
{
"time": 1727713674.2,
"object": null,
"category": null
},
{
"time": 1727713674.5,
"object": null,
"category": null
},
{
"time": 1727713675.1,
"object": "NetworkUser2",
"category": "Team"
},
{
"time": 1727713675.3,
"object": "Pump_Mask_Interactable_R(Clone)",
"category": "Equipment"
},
{
"time": 1727713675.6,
"object": "NetworkUser2",
"category": "Team"
},
{
"time": 1727713675.9,
"object": "NetworkHandL1",
"category": "Team"
},
{
"time": 1727713676.2,
"object": "NetworkUser2",
"category": "Team"
},
{
"time": 1727713676.3,
"object": "NetworkUser2",
"category": "Team"
},
{
"time": 1727713676.5,
"object": "NetworkUser2",
"category": "Team"
},
{
"time": 1727713676.8,
"object": null,
"category": null
},
{
"time": 1727713677,
"object": null,
"category": null
},
{
"time": 1727713677.1,
"object": null,
"category": null
},
{
"time": 1727713677.3,
"object": null,
"category": null
},
{
"time": 1727713677.7,
"object": null,
"category": null
},
{
"time": 1727713677.9,
"object": "Non_Breather_Interactable(Clone)",
"category": "Others"
},
{
"time": 1727713678.1,
"object": null,
"category": null
},
{
"time": 1727713678.9,
"object": null,
"category": null
},
{
"time": 1727713679.2,
"object": null,
"category": null
},
{
"time": 1727713679.4,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727713679.9,
"object": "NetworkHandR3",
"category": "Team"
},
{
"time": 1727713680,
"object": null,
"category": null
},
{
"time": 1727713680.2,
"object": "Patient Left Leg",
"category": "Patient"
},
{
"time": 1727713680.5,
"object": "NetworkHandR2",
"category": "Team"
},
{
"time": 1727713680.7,
"object": "Patient Left Leg",
"category": "Patient"
},
{
"time": 1727713681,
"object": "Pump_Mask_Interactable_R(Clone)",
"category": "Equipment"
},
{
"time": 1727713681.8,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727713682,
"object": null,
"category": null
},
{
"time": 1727713682.3,
"object": null,
"category": null
},
{
"time": 1727713682.5,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727713682.9,
"object": null,
"category": null
},
{
"time": 1727713683,
"object": "Patient Right Leg",
"category": "Patient"
},
{
"time": 1727713683.4,
"object": null,
"category": null
},
{
"time": 1727713684,
"object": "Down Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713684.8,
"object": null,
"category": null
},
{
"time": 1727713684.9,
"object": null,
"category": null
},
{
"time": 1727713685,
"object": null,
"category": null
},
{
"time": 1727713685.5,
"object": null,
"category": null
},
{
"time": 1727713685.9,
"object": "Down Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713686.1,
"object": null,
"category": null
},
{
"time": 1727713686.3,
"object": "Patient Left Leg",
"category": "Patient"
},
{
"time": 1727713686.6,
"object": "Patient Left Leg",
"category": "Patient"
},
{
"time": 1727713686.9,
"object": "NetworkUser1",
"category": "Team"
},
{
"time": 1727713687.1,
"object": "Pump_Mask_Interactable_R(Clone)",
"category": "Equipment"
},
{
"time": 1727713687.3,
"object": "NetworkUser2",
"category": "Team"
},
{
"time": 1727713687.6,
"object": "NetworkUser2",
"category": "Team"
},
{
"time": 1727713688,
"object": null,
"category": null
},
{
"time": 1727713688.2,
"object": "Pump_Mask_Interactable_R(Clone)",
"category": "Equipment"
},
{
"time": 1727713688.4,
"object": null,
"category": null
},
{
"time": 1727713688.9,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727713689.1,
"object": null,
"category": null
},
{
"time": 1727713689.3,
"object": "Patient Left Leg",
"category": "Patient"
},
{
"time": 1727713689.4,
"object": "Patient Left Leg",
"category": "Patient"
},
{
"time": 1727713689.5,
"object": null,
"category": null
},
{
"time": 1727713690.2,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713690.4,
"object": "Down Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713690.5,
"object": "Down Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713690.7,
"object": "Down Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713690.9,
"object": "Down Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713691.2,
"object": "Down Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713691.3,
"object": "Down Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713691.5,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713691.7,
"object": "NetworkUser1",
"category": "Team"
},
{
"time": 1727713691.9,
"object": null,
"category": null
},
{
"time": 1727713692.1,
"object": null,
"category": null
},
{
"time": 1727713692.3,
"object": null,
"category": null
},
{
"time": 1727713692.5,
"object": null,
"category": null
},
{
"time": 1727713692.8,
"object": null,
"category": null
},
{
"time": 1727713693,
"object": null,
"category": null
},
{
"time": 1727713693.3,
"object": null,
"category": null
},
{
"time": 1727713693.4,
"object": null,
"category": null
},
{
"time": 1727713693.6,
"object": null,
"category": null
},
{
"time": 1727713693.9,
"object": null,
"category": null
},
{
"time": 1727713694.1,
"object": null,
"category": null
},
{
"time": 1727713694.3,
"object": null,
"category": null
},
{
"time": 1727713694.5,
"object": null,
"category": null
},
{
"time": 1727713694.8,
"object": null,
"category": null
},
{
"time": 1727713695.1,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713695.2,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713695.4,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713695.7,
"object": null,
"category": null
},
{
"time": 1727713696.2,
"object": "Down Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713696.4,
"object": "Down Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713696.5,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713696.8,
"object": null,
"category": null
},
{
"time": 1727713698.3,
"object": null,
"category": null
},
{
"time": 1727713698.4,
"object": null,
"category": null
},
{
"time": 1727713698.5,
"object": null,
"category": null
},
{
"time": 1727713698.8,
"object": null,
"category": null
},
{
"time": 1727713699.2,
"object": null,
"category": null
},
{
"time": 1727713699.8,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727713699.9,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727713700,
"object": null,
"category": null
},
{
"time": 1727713700.7,
"object": "Pulse Point Groin - Defib Machine",
"category": "Others"
},
{
"time": 1727713701,
"object": null,
"category": null
},
{
"time": 1727713701.3,
"object": "Patient Left Leg",
"category": "Patient"
},
{
"time": 1727713701.4,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727713701.9,
"object": null,
"category": null
},
{
"time": 1727713702,
"object": "NetworkUser1",
"category": "Team"
},
{
"time": 1727713702.3,
"object": "Patient Left Leg",
"category": "Patient"
},
{
"time": 1727713702.4,
"object": "Patient Left Leg",
"category": "Patient"
},
{
"time": 1727713702.7,
"object": "Patient Left Leg",
"category": "Patient"
},
{
"time": 1727713702.9,
"object": "Patient Right Leg",
"category": "Patient"
},
{
"time": 1727713703.3,
"object": "Patient Right Leg",
"category": "Patient"
},
{
"time": 1727713704,
"object": "Patient Left Leg",
"category": "Patient"
},
{
"time": 1727713704.5,
"object": "Patient Left Leg",
"category": "Patient"
},
{
"time": 1727713704.8,
"object": "Patient Left Leg",
"category": "Patient"
},
{
"time": 1727713705.5,
"object": "Patient Left Leg",
"category": "Patient"
},
{
"time": 1727713705.7,
"object": "Patient Right Leg",
"category": "Patient"
},
{
"time": 1727713706,
"object": "Patient Left Leg",
"category": "Patient"
},
{
"time": 1727713706.9,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713707,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713707.2,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713707.6,
"object": null,
"category": null
},
{
"time": 1727713707.7,
"object": null,
"category": null
},
{
"time": 1727713708.1,
"object": "Down Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713708.2,
"object": "Down Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713708.7,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727713709,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727713709.4,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727713710,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727713710.3,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727713711.3,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727713711.6,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727713711.9,
"object": "NetworkHandR3",
"category": "Team"
},
{
"time": 1727713712.2,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727713712.4,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727713712.9,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727713713.1,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727713713.4,
"object": "Defibrillator(Clone)",
"category": "Equipment"
},
{
"time": 1727713713.5,
"object": "NetworkHandR3",
"category": "Team"
},
{
"time": 1727713713.9,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727713714,
"object": null,
"category": null
},
{
"time": 1727713714.2,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727713714.5,
"object": null,
"category": null
},
{
"time": 1727713714.9,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727713715.5,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713715.7,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727713715.8,
"object": "Defibrillator(Clone)",
"category": "Equipment"
},
{
"time": 1727713716,
"object": "Defibrillator Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713716.4,
"object": "Defibrillator(Clone)",
"category": "Equipment"
},
{
"time": 1727713717.1,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727713717.5,
"object": "Defibrillator Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713717.8,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727713718.1,
"object": "Defibrillator Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713718.4,
"object": "Sync",
"category": "Equipment"
},
{
"time": 1727713718.7,
"object": "Defibrillator(Clone)",
"category": "Equipment"
},
{
"time": 1727713719,
"object": "Defibrillator Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713719.4,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727713720.1,
"object": "Defibrillator Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713720.3,
"object": "NetworkHandR3",
"category": "Team"
},
{
"time": 1727713720.6,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727713721,
"object": "Defibrillator Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713721.2,
"object": "NetworkHandR3",
"category": "Team"
},
{
"time": 1727713721.4,
"object": "Defibrillator Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713721.6,
"object": "Defibrillator Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713721.8,
"object": "Defibrillator(Clone)",
"category": "Equipment"
},
{
"time": 1727713722,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727713723,
"object": "Defibrillator Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713723.5,
"object": null,
"category": null
},
{
"time": 1727713724.2,
"object": "Defibrillator Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713724.4,
"object": "Defibrillator Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713724.5,
"object": "Defibrillator Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713724.7,
"object": "Defibrillator(Clone)",
"category": "Equipment"
},
{
"time": 1727713725.2,
"object": "Defibrillator Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713725.5,
"object": "Defibrillator Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713726.1,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727713726.2,
"object": "Defibrillator Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713726.5,
"object": "Defibrillator(Clone)",
"category": "Equipment"
},
{
"time": 1727713726.7,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727713728.4,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713728.8,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727713729,
"object": "Defibrillator Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713729.8,
"object": "Defibrillator(Clone)",
"category": "Equipment"
},
{
"time": 1727713730.3,
"object": "Defibrillator Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713730.4,
"object": "Defibrillator(Clone)",
"category": "Equipment"
},
{
"time": 1727713730.6,
"object": "Defibrillator Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713730.9,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727713731.5,
"object": "Defibrillator Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713731.7,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727713733.6,
"object": "Defibrillator Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713734.4,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727713735.2,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727713735.9,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727713736.1,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727713737.3,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727713737.6,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727713738,
"object": null,
"category": null
},
{
"time": 1727713738.5,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727713739.1,
"object": null,
"category": null
},
{
"time": 1727713739.3,
"object": "Patient Left Arm",
"category": "Patient"
},
{
"time": 1727713739.7,
"object": null,
"category": null
},
{
"time": 1727713740,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727713740.7,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727713741,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727713741.2,
"object": "Defibrillator Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713741.4,
"object": "Defibrillator Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713741.7,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727713741.9,
"object": "Defibrillator Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713742.4,
"object": null,
"category": null
},
{
"time": 1727713742.7,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727713744.3,
"object": "Patient Right Leg",
"category": "Patient"
},
{
"time": 1727713744.5,
"object": null,
"category": null
},
{
"time": 1727713745.8,
"object": "Patient Right Leg",
"category": "Patient"
},
{
"time": 1727713746,
"object": null,
"category": null
},
{
"time": 1727713747,
"object": null,
"category": null
},
{
"time": 1727713747.4,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713747.5,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713748,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713748.2,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713748.5,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713748.8,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713749.5,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713749.6,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713749.9,
"object": "Down Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713750.1,
"object": "Down Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713750.4,
"object": "Patient Right Leg",
"category": "Patient"
},
{
"time": 1727713750.6,
"object": null,
"category": null
},
{
"time": 1727713751.5,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713751.7,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713751.8,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713752.3,
"object": null,
"category": null
},
{
"time": 1727713752.8,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727713753.1,
"object": null,
"category": null
},
{
"time": 1727713753.8,
"object": "Cpr Hands Object",
"category": "Others"
},
{
"time": 1727713754,
"object": null,
"category": null
},
{
"time": 1727713754.2,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727713754.6,
"object": null,
"category": null
},
{
"time": 1727713755.3,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727713755.5,
"object": null,
"category": null
},
{
"time": 1727713755.7,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727713755.8,
"object": "Defibrillator(Clone)",
"category": "Equipment"
},
{
"time": 1727713756.1,
"object": null,
"category": null
},
{
"time": 1727713756.6,
"object": "Down Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713756.8,
"object": null,
"category": null
},
{
"time": 1727713757.5,
"object": null,
"category": null
},
{
"time": 1727713758.2,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713758.5,
"object": null,
"category": null
},
{
"time": 1727713758.8,
"object": "Defibrillator(Clone)",
"category": "Equipment"
},
{
"time": 1727713759.9,
"object": null,
"category": null
},
{
"time": 1727713760.7,
"object": null,
"category": null
},
{
"time": 1727713761.8,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713762.1,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713762.3,
"object": null,
"category": null
},
{
"time": 1727713762.4,
"object": null,
"category": null
},
{
"time": 1727713762.6,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713763.1,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713763.4,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713763.6,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713763.7,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713763.9,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713764.2,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713764.4,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713764.7,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713764.8,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713765.2,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713765.6,
"object": "NetworkHandL2",
"category": "Team"
},
{
"time": 1727713765.8,
"object": null,
"category": null
},
{
"time": 1727713766,
"object": "NetworkHandL2",
"category": "Team"
},
{
"time": 1727713766.2,
"object": null,
"category": null
},
{
"time": 1727713766.8,
"object": null,
"category": null
},
{
"time": 1727713767.4,
"object": null,
"category": null
},
{
"time": 1727713767.8,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713768.1,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713768.6,
"object": "NetworkUser2",
"category": "Team"
},
{
"time": 1727713769.2,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713769.4,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713769.8,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713769.9,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713770.1,
"object": "Down Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713770.5,
"object": null,
"category": null
},
{
"time": 1727713771.5,
"object": "NetworkHandL2",
"category": "Team"
},
{
"time": 1727713772.1,
"object": "NetworkUser2",
"category": "Team"
},
{
"time": 1727713772.3,
"object": "NetworkUser2",
"category": "Team"
},
{
"time": 1727713772.9,
"object": "NetworkUser2",
"category": "Team"
},
{
"time": 1727713773.2,
"object": "NetworkUser2",
"category": "Team"
},
{
"time": 1727713773.4,
"object": "NetworkUser2",
"category": "Team"
},
{
"time": 1727713773.8,
"object": null,
"category": null
},
{
"time": 1727713774.2,
"object": null,
"category": null
},
{
"time": 1727713774.8,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713775.6,
"object": null,
"category": null
},
{
"time": 1727713775.8,
"object": null,
"category": null
},
{
"time": 1727713777.3,
"object": null,
"category": null
},
{
"time": 1727713777.5,
"object": "Patient Right Leg",
"category": "Patient"
},
{
"time": 1727713777.9,
"object": null,
"category": null
},
{
"time": 1727713778.5,
"object": null,
"category": null
},
{
"time": 1727713779.6,
"object": null,
"category": null
},
{
"time": 1727713779.7,
"object": null,
"category": null
},
{
"time": 1727713780.1,
"object": null,
"category": null
},
{
"time": 1727713780.4,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727713780.6,
"object": "NetworkHandR3",
"category": "Team"
},
{
"time": 1727713780.8,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713781.2,
"object": "NetworkHandR3",
"category": "Team"
},
{
"time": 1727713781.4,
"object": null,
"category": null
},
{
"time": 1727713781.8,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713782.3,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713782.4,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713782.8,
"object": "NetworkUser1",
"category": "Team"
},
{
"time": 1727713783,
"object": null,
"category": null
},
{
"time": 1727713783.3,
"object": null,
"category": null
},
{
"time": 1727713784,
"object": null,
"category": null
},
{
"time": 1727713784.1,
"object": null,
"category": null
},
{
"time": 1727713784.6,
"object": null,
"category": null
},
{
"time": 1727713785.4,
"object": null,
"category": null
},
{
"time": 1727713785.7,
"object": null,
"category": null
},
{
"time": 1727713786.7,
"object": null,
"category": null
},
{
"time": 1727713786.8,
"object": null,
"category": null
},
{
"time": 1727713787,
"object": null,
"category": null
},
{
"time": 1727713787.4,
"object": null,
"category": null
},
{
"time": 1727713787.7,
"object": null,
"category": null
},
{
"time": 1727713788,
"object": null,
"category": null
},
{
"time": 1727713788.5,
"object": "Bipap_Mask_Interactable(Clone)",
"category": "Equipment"
},
{
"time": 1727713788.8,
"object": null,
"category": null
},
{
"time": 1727713789,
"object": null,
"category": null
},
{
"time": 1727713789.2,
"object": null,
"category": null
},
{
"time": 1727713789.8,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727713790.4,
"object": null,
"category": null
},
{
"time": 1727713791.1,
"object": null,
"category": null
},
{
"time": 1727713791.3,
"object": null,
"category": null
},
{
"time": 1727713791.8,
"object": null,
"category": null
},
{
"time": 1727713792.8,
"object": null,
"category": null
},
{
"time": 1727713793.5,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727713793.8,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727713794,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727713794.4,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727713794.7,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727713795.1,
"object": null,
"category": null
},
{
"time": 1727713795.8,
"object": null,
"category": null
},
{
"time": 1727713796.1,
"object": "NetworkUser2",
"category": "Team"
},
{
"time": 1727713796.6,
"object": "Pump_Mask_Interactable_R(Clone)",
"category": "Equipment"
},
{
"time": 1727713797,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727713797.4,
"object": null,
"category": null
},
{
"time": 1727713798.3,
"object": null,
"category": null
},
{
"time": 1727713799,
"object": null,
"category": null
},
{
"time": 1727713799.2,
"object": null,
"category": null
},
{
"time": 1727713799.4,
"object": null,
"category": null
},
{
"time": 1727713800,
"object": "Pulse Point Groin - Defib Machine",
"category": "Others"
},
{
"time": 1727713800.1,
"object": "Pulse Point Groin - Defib Machine",
"category": "Others"
},
{
"time": 1727713800.6,
"object": "Pulse Point Groin - Defib Machine",
"category": "Others"
},
{
"time": 1727713800.9,
"object": null,
"category": null
},
{
"time": 1727713801,
"object": null,
"category": null
},
{
"time": 1727713801.2,
"object": null,
"category": null
},
{
"time": 1727713801.4,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727713801.5,
"object": "NetworkHandR3",
"category": "Team"
},
{
"time": 1727713801.7,
"object": "Pulse Point Groin - Defib Machine",
"category": "Others"
},
{
"time": 1727713802,
"object": "Pulse Point Groin - Defib Machine",
"category": "Others"
},
{
"time": 1727713802.3,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727713802.5,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727713802.6,
"object": "NetworkHandR3",
"category": "Team"
},
{
"time": 1727713802.8,
"object": null,
"category": null
},
{
"time": 1727713803.2,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727713803.4,
"object": "NetworkHandR3",
"category": "Team"
},
{
"time": 1727713803.5,
"object": null,
"category": null
},
{
"time": 1727713803.8,
"object": "NetworkHandR3",
"category": "Team"
},
{
"time": 1727713804.5,
"object": "Down Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713804.7,
"object": "NetworkHandR1",
"category": "Team"
},
{
"time": 1727713805.1,
"object": null,
"category": null
},
{
"time": 1727713805.4,
"object": null,
"category": null
},
{
"time": 1727713806.1,
"object": "NetworkHandR3",
"category": "Team"
},
{
"time": 1727713806.3,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727713806.6,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727713806.6,
"object": "Pulse Point Groin - Defib Machine",
"category": "Others"
},
{
"time": 1727713806.8,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727713806.9,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727713807,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727713807.4,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727713807.6,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727713808.1,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727713808.2,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727713808.3,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727713808.7,
"object": "Pulse Point Groin - Defib Machine",
"category": "Others"
},
{
"time": 1727713808.8,
"object": "Pulse Point Groin - Defib Machine",
"category": "Others"
},
{
"time": 1727713809.1,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727713809.4,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727713809.5,
"object": "Pulse Point Groin - Defib Machine",
"category": "Others"
},
{
"time": 1727713809.7,
"object": "NetworkHandR3",
"category": "Team"
},
{
"time": 1727713810.1,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727713810.4,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727713810.7,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727713810.9,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727713811.1,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727713811.2,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727713811.4,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727713811.7,
"object": null,
"category": null
},
{
"time": 1727713813.8,
"object": null,
"category": null
},
{
"time": 1727713814.7,
"object": null,
"category": null
},
{
"time": 1727713816,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713816.6,
"object": null,
"category": null
},
{
"time": 1727713817.2,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727713817.5,
"object": null,
"category": null
},
{
"time": 1727713817.8,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727713818.1,
"object": "NetworkUser1",
"category": "Team"
},
{
"time": 1727713818.5,
"object": "NetworkHandR3",
"category": "Team"
},
{
"time": 1727713818.8,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727713819.3,
"object": null,
"category": null
},
{
"time": 1727713821.2,
"object": null,
"category": null
},
{
"time": 1727713821.4,
"object": null,
"category": null
},
{
"time": 1727713822.3,
"object": null,
"category": null
},
{
"time": 1727713826.6,
"object": null,
"category": null
},
{
"time": 1727713826.9,
"object": "Defibrillator(Clone)",
"category": "Equipment"
},
{
"time": 1727713827,
"object": null,
"category": null
},
{
"time": 1727713827.2,
"object": "Defibrillator Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713827.7,
"object": null,
"category": null
},
{
"time": 1727713828.6,
"object": null,
"category": null
},
{
"time": 1727713828.8,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727713828.9,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727713829.2,
"object": null,
"category": null
},
{
"time": 1727713829.7,
"object": null,
"category": null
},
{
"time": 1727713831.4,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727713831.8,
"object": null,
"category": null
},
{
"time": 1727713832.6,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727713833.2,
"object": "NetworkUser2",
"category": "Team"
},
{
"time": 1727713833.5,
"object": "NetworkUser2",
"category": "Team"
},
{
"time": 1727713834.1,
"object": null,
"category": null
},
{
"time": 1727713834.3,
"object": null,
"category": null
},
{
"time": 1727713834.5,
"object": null,
"category": null
},
{
"time": 1727713834.7,
"object": null,
"category": null
},
{
"time": 1727713835.1,
"object": "NetworkUser2",
"category": "Team"
},
{
"time": 1727713835.2,
"object": "NetworkUser2",
"category": "Team"
},
{
"time": 1727713835.7,
"object": null,
"category": null
},
{
"time": 1727713837.5,
"object": null,
"category": null
},
{
"time": 1727713837.8,
"object": "NetworkHandL2",
"category": "Team"
},
{
"time": 1727713838.1,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727713838.3,
"object": null,
"category": null
},
{
"time": 1727713839.2,
"object": "Pulse Point Groin - Defib Machine",
"category": "Others"
},
{
"time": 1727713839.4,
"object": null,
"category": null
},
{
"time": 1727713840.1,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727713840.6,
"object": "NetworkHandL3",
"category": "Team"
},
{
"time": 1727713841.8,
"object": "Pulse Point Groin - AirWay/BVM",
"category": "Others"
},
{
"time": 1727713841.9,
"object": "Pulse Point Groin - AirWay/BVM",
"category": "Others"
},
{
"time": 1727713842.1,
"object": "Patient Head",
"category": "Patient"
},
{
"time": 1727713842.3,
"object": null,
"category": null
},
{
"time": 1727713843,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727713843.8,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727713843.9,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727713844.6,
"object": null,
"category": null
},
{
"time": 1727713844.9,
"object": "Patient Left Arm",
"category": "Patient"
},
{
"time": 1727713845.1,
"object": "Patient Left Leg",
"category": "Patient"
},
{
"time": 1727713845.5,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727713846.1,
"object": null,
"category": null
},
{
"time": 1727713846.3,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713846.6,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713846.7,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713847.1,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713847.4,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713848.1,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713848.4,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727713848.6,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727713849.5,
"object": null,
"category": null
},
{
"time": 1727713849.9,
"object": "Defibrillator(Clone)",
"category": "Equipment"
},
{
"time": 1727713850.2,
"object": null,
"category": null
},
{
"time": 1727713850.7,
"object": "Defibrillator(Clone)",
"category": "Equipment"
},
{
"time": 1727713852.3,
"object": null,
"category": null
},
{
"time": 1727713852.6,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727713852.9,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727713853,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727713853.5,
"object": "Defibrillator Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713854.2,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727713854.4,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727713854.8,
"object": null,
"category": null
},
{
"time": 1727713855.3,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727713855.5,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727713856.6,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727713856.7,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727713856.9,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727713857.5,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727713858.2,
"object": "Defibrillator Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713858.6,
"object": "Defibrillator Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713858.8,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727713859.3,
"object": "Defibrillator Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713859.8,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727713860,
"object": null,
"category": null
},
{
"time": 1727713860.2,
"object": "Defibrillator(Clone)",
"category": "Equipment"
},
{
"time": 1727713860.5,
"object": "NetworkUser2",
"category": "Team"
},
{
"time": 1727713860.6,
"object": "NetworkUser2",
"category": "Team"
},
{
"time": 1727713861,
"object": null,
"category": null
},
{
"time": 1727713861.3,
"object": "NetworkHandR3",
"category": "Team"
},
{
"time": 1727713861.4,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727713861.8,
"object": null,
"category": null
},
{
"time": 1727713861.9,
"object": null,
"category": null
},
{
"time": 1727713862.2,
"object": null,
"category": null
},
{
"time": 1727713862.4,
"object": null,
"category": null
},
{
"time": 1727713862.9,
"object": null,
"category": null
},
{
"time": 1727713863.7,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727713863.9,
"object": null,
"category": null
},
{
"time": 1727713864.4,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713864.6,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713864.7,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713865.2,
"object": null,
"category": null
},
{
"time": 1727713865.4,
"object": null,
"category": null
},
{
"time": 1727713865.6,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727713865.8,
"object": null,
"category": null
},
{
"time": 1727713866.2,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727713866.5,
"object": null,
"category": null
},
{
"time": 1727713866.6,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727713866.8,
"object": null,
"category": null
},
{
"time": 1727713867.1,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727713867.5,
"object": null,
"category": null
},
{
"time": 1727713867.7,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727713867.8,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727713868.1,
"object": null,
"category": null
},
{
"time": 1727713868.2,
"object": null,
"category": null
},
{
"time": 1727713868.5,
"object": null,
"category": null
},
{
"time": 1727713868.7,
"object": null,
"category": null
},
{
"time": 1727713869.1,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727713869.2,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727713869.9,
"object": null,
"category": null
},
{
"time": 1727713870.2,
"object": null,
"category": null
},
{
"time": 1727713870.4,
"object": "NetworkUser1",
"category": "Team"
},
{
"time": 1727713871,
"object": "Pulse Point Neck - AirWay/BVM",
"category": "Others"
},
{
"time": 1727713871.1,
"object": null,
"category": null
},
{
"time": 1727713871.2,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727713871.6,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713872,
"object": null,
"category": null
},
{
"time": 1727713872.5,
"object": null,
"category": null
},
{
"time": 1727713873.5,
"object": null,
"category": null
},
{
"time": 1727713874.1,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727713874.3,
"object": "Cpr Hands Object",
"category": "Others"
},
{
"time": 1727713874.6,
"object": "NetworkUser1",
"category": "Team"
},
{
"time": 1727713874.9,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727713875.1,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727713875.5,
"object": "NetworkHandR3",
"category": "Team"
},
{
"time": 1727713875.8,
"object": null,
"category": null
},
{
"time": 1727713876.2,
"object": null,
"category": null
},
{
"time": 1727713876.5,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727713876.9,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727713877.1,
"object": null,
"category": null
},
{
"time": 1727713877.4,
"object": null,
"category": null
},
{
"time": 1727713877.7,
"object": "NetworkHandR2",
"category": "Team"
},
{
"time": 1727713878,
"object": null,
"category": null
},
{
"time": 1727713878.3,
"object": "Patient Right Leg",
"category": "Patient"
},
{
"time": 1727713878.9,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727713879.6,
"object": "NetworkUser2",
"category": "Team"
},
{
"time": 1727713879.9,
"object": "NetworkUser2",
"category": "Team"
},
{
"time": 1727713880.1,
"object": "NetworkUser1",
"category": "Team"
},
{
"time": 1727713880.9,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727713881.1,
"object": "Patient Right Leg",
"category": "Patient"
},
{
"time": 1727713881.4,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727713881.7,
"object": "Patient Right Leg",
"category": "Patient"
},
{
"time": 1727713882,
"object": null,
"category": null
},
{
"time": 1727713882.4,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727713882.5,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727713882.7,
"object": null,
"category": null
},
{
"time": 1727713883.4,
"object": null,
"category": null
},
{
"time": 1727713883.8,
"object": null,
"category": null
},
{
"time": 1727713884,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727713884.2,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727713885,
"object": null,
"category": null
},
{
"time": 1727713885.3,
"object": null,
"category": null
},
{
"time": 1727713885.5,
"object": "NetworkUser2",
"category": "Team"
},
{
"time": 1727713885.8,
"object": null,
"category": null
},
{
"time": 1727713886.3,
"object": "NetworkUser2",
"category": "Team"
},
{
"time": 1727713886.5,
"object": "NetworkUser1",
"category": "Team"
},
{
"time": 1727713886.6,
"object": "Pulse Point Neck - AirWay/BVM",
"category": "Others"
},
{
"time": 1727713886.8,
"object": "Pulse Point Neck - AirWay/BVM",
"category": "Others"
},
{
"time": 1727713887.2,
"object": null,
"category": null
},
{
"time": 1727713888,
"object": null,
"category": null
},
{
"time": 1727713888.3,
"object": null,
"category": null
},
{
"time": 1727713888.6,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727713889,
"object": null,
"category": null
},
{
"time": 1727713889.2,
"object": "Patient Left Arm",
"category": "Patient"
},
{
"time": 1727713889.5,
"object": null,
"category": null
},
{
"time": 1727713889.6,
"object": null,
"category": null
},
{
"time": 1727713889.8,
"object": null,
"category": null
},
{
"time": 1727713890.3,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727713890.5,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727713890.8,
"object": null,
"category": null
},
{
"time": 1727713891,
"object": null,
"category": null
},
{
"time": 1727713891.4,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727713891.7,
"object": null,
"category": null
},
{
"time": 1727713892,
"object": "NetworkUser1",
"category": "Team"
},
{
"time": 1727713892.4,
"object": null,
"category": null
},
{
"time": 1727713892.6,
"object": null,
"category": null
},
{
"time": 1727713893.5,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727713893.7,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727713893.9,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727713894.2,
"object": null,
"category": null
},
{
"time": 1727713894.4,
"object": null,
"category": null
},
{
"time": 1727713894.6,
"object": "NetworkUser2",
"category": "Team"
},
{
"time": 1727713895.1,
"object": null,
"category": null
},
{
"time": 1727713895.5,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727713896,
"object": "NetworkUser2",
"category": "Team"
},
{
"time": 1727713896.5,
"object": null,
"category": null
},
{
"time": 1727713896.6,
"object": null,
"category": null
},
{
"time": 1727713897.5,
"object": "Patient Left Leg",
"category": "Patient"
},
{
"time": 1727713897.6,
"object": null,
"category": null
},
{
"time": 1727713897.9,
"object": null,
"category": null
},
{
"time": 1727713898,
"object": null,
"category": null
},
{
"time": 1727713898.3,
"object": null,
"category": null
},
{
"time": 1727713898.4,
"object": null,
"category": null
},
{
"time": 1727713898.6,
"object": null,
"category": null
},
{
"time": 1727713898.8,
"object": "Lactated_Ringers_Interactable(Clone)",
"category": "Others"
},
{
"time": 1727713899,
"object": null,
"category": null
},
{
"time": 1727713899.1,
"object": null,
"category": null
},
{
"time": 1727713899.3,
"object": null,
"category": null
},
{
"time": 1727713900.2,
"object": "NetworkUser2",
"category": "Team"
},
{
"time": 1727713900.4,
"object": null,
"category": null
},
{
"time": 1727713900.7,
"object": null,
"category": null
},
{
"time": 1727713900.9,
"object": null,
"category": null
},
{
"time": 1727713901,
"object": null,
"category": null
},
{
"time": 1727713901.2,
"object": null,
"category": null
},
{
"time": 1727713902,
"object": null,
"category": null
},
{
"time": 1727713902.2,
"object": null,
"category": null
},
{
"time": 1727713902.4,
"object": null,
"category": null
},
{
"time": 1727713902.8,
"object": "NetworkHandR3",
"category": "Team"
},
{
"time": 1727713902.9,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727713903.8,
"object": "NetworkHandR2",
"category": "Team"
},
{
"time": 1727713904,
"object": "NetworkHandR2",
"category": "Team"
},
{
"time": 1727713904.4,
"object": "NetworkHandL2",
"category": "Team"
},
{
"time": 1727713904.8,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713905,
"object": null,
"category": null
},
{
"time": 1727713905.4,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727713905.7,
"object": null,
"category": null
},
{
"time": 1727713906.5,
"object": "Patient Right Leg",
"category": "Patient"
},
{
"time": 1727713906.9,
"object": "Patient Right Leg",
"category": "Patient"
},
{
"time": 1727713907.2,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727713907.4,
"object": null,
"category": null
},
{
"time": 1727713908.1,
"object": "NetworkUser2",
"category": "Team"
},
{
"time": 1727713908.5,
"object": "NetworkHandR1",
"category": "Team"
},
{
"time": 1727713908.9,
"object": "Pulse Point Groin - AirWay/BVM",
"category": "Others"
},
{
"time": 1727713909.1,
"object": "Patient Right Leg",
"category": "Patient"
},
{
"time": 1727713909.2,
"object": null,
"category": null
},
{
"time": 1727713910.4,
"object": "Patient Right Leg",
"category": "Patient"
},
{
"time": 1727713911.1,
"object": null,
"category": null
},
{
"time": 1727713911.7,
"object": null,
"category": null
},
{
"time": 1727713912.3,
"object": null,
"category": null
},
{
"time": 1727713912.5,
"object": null,
"category": null
},
{
"time": 1727713913.1,
"object": null,
"category": null
},
{
"time": 1727713913.4,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727713913.4,
"object": "NetworkHandL2",
"category": "Team"
},
{
"time": 1727713913.6,
"object": null,
"category": null
},
{
"time": 1727713914,
"object": "NetworkUser1",
"category": "Team"
},
{
"time": 1727713914.4,
"object": null,
"category": null
},
{
"time": 1727713914.6,
"object": null,
"category": null
},
{
"time": 1727713914.7,
"object": null,
"category": null
},
{
"time": 1727713914.9,
"object": "Patient Right Leg",
"category": "Patient"
},
{
"time": 1727713915.2,
"object": "NetworkUser1",
"category": "Team"
},
{
"time": 1727713915.4,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713915.5,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713915.7,
"object": null,
"category": null
},
{
"time": 1727713915.8,
"object": null,
"category": null
},
{
"time": 1727713916,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727713916.8,
"object": null,
"category": null
},
{
"time": 1727713917.2,
"object": "Patient Right Leg",
"category": "Patient"
},
{
"time": 1727713917.4,
"object": "NetworkHandR2",
"category": "Team"
},
{
"time": 1727713917.6,
"object": null,
"category": null
},
{
"time": 1727713917.9,
"object": "NetworkHandR2",
"category": "Team"
},
{
"time": 1727713918.4,
"object": "NetworkHandR2",
"category": "Team"
},
{
"time": 1727713918.8,
"object": null,
"category": null
},
{
"time": 1727713919.6,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727713919.7,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727713920,
"object": null,
"category": null
},
{
"time": 1727713921.9,
"object": "Patient Right Leg",
"category": "Patient"
},
{
"time": 1727713922.3,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727713922.5,
"object": "NetworkUser2",
"category": "Team"
},
{
"time": 1727713923.3,
"object": null,
"category": null
},
{
"time": 1727713924.1,
"object": "NetworkUser2",
"category": "Team"
},
{
"time": 1727713924.7,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727713924.9,
"object": null,
"category": null
},
{
"time": 1727713925.2,
"object": "Patient Right Leg",
"category": "Patient"
},
{
"time": 1727713925.4,
"object": "NetworkHandR3",
"category": "Team"
},
{
"time": 1727713925.6,
"object": "NetworkHandR3",
"category": "Team"
},
{
"time": 1727713925.8,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727713926.1,
"object": null,
"category": null
},
{
"time": 1727713926.2,
"object": "Patient Right Leg",
"category": "Patient"
},
{
"time": 1727713926.7,
"object": null,
"category": null
},
{
"time": 1727713927.7,
"object": null,
"category": null
},
{
"time": 1727713928,
"object": "Patient Right Leg",
"category": "Patient"
},
{
"time": 1727713928.2,
"object": "Patient Right Leg",
"category": "Patient"
},
{
"time": 1727713928.5,
"object": null,
"category": null
},
{
"time": 1727713928.8,
"object": "Patient Right Leg",
"category": "Patient"
},
{
"time": 1727713929.4,
"object": "Patient Right Leg",
"category": "Patient"
},
{
"time": 1727713929.6,
"object": null,
"category": null
},
{
"time": 1727713930.1,
"object": "Patient Right Leg",
"category": "Patient"
},
{
"time": 1727713930.7,
"object": "Patient Right Leg",
"category": "Patient"
},
{
"time": 1727713930.8,
"object": "Patient Right Leg",
"category": "Patient"
},
{
"time": 1727713931.3,
"object": "Patient Right Leg",
"category": "Patient"
},
{
"time": 1727713931.4,
"object": "Patient Right Leg",
"category": "Patient"
},
{
"time": 1727713932.5,
"object": "Patient Right Leg",
"category": "Patient"
},
{
"time": 1727713932.7,
"object": "Patient Right Leg",
"category": "Patient"
},
{
"time": 1727713933.3,
"object": "Patient Right Leg",
"category": "Patient"
},
{
"time": 1727713934.5,
"object": null,
"category": null
},
{
"time": 1727713935.9,
"object": "Patient Right Leg",
"category": "Patient"
},
{
"time": 1727713936.1,
"object": "Patient Right Leg",
"category": "Patient"
},
{
"time": 1727713936.4,
"object": "Patient Right Leg",
"category": "Patient"
},
{
"time": 1727713936.5,
"object": "Patient Right Leg",
"category": "Patient"
},
{
"time": 1727713936.6,
"object": null,
"category": null
},
{
"time": 1727713938.2,
"object": "NetworkUser1",
"category": "Team"
},
{
"time": 1727713938.4,
"object": "NetworkHandR2",
"category": "Team"
},
{
"time": 1727713938.6,
"object": "NetworkHandR2",
"category": "Team"
},
{
"time": 1727713938.8,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727713939,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727713939.3,
"object": null,
"category": null
},
{
"time": 1727713940.7,
"object": "NetworkUser2",
"category": "Team"
},
{
"time": 1727713940.9,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727713941.4,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727713941.5,
"object": null,
"category": null
},
{
"time": 1727713942.2,
"object": "Patient Right Leg",
"category": "Patient"
},
{
"time": 1727713942.5,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713943,
"object": null,
"category": null
},
{
"time": 1727713943.7,
"object": null,
"category": null
},
{
"time": 1727713944.4,
"object": null,
"category": null
},
{
"time": 1727713944.8,
"object": null,
"category": null
},
{
"time": 1727713945.1,
"object": null,
"category": null
},
{
"time": 1727713945.5,
"object": null,
"category": null
},
{
"time": 1727713947.1,
"object": "Patient Right Arm",
"category": "Patient"
},
{
"time": 1727713947.3,
"object": null,
"category": null
},
{
"time": 1727713947.4,
"object": "Patient Right Leg",
"category": "Patient"
},
{
"time": 1727713947.7,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713947.9,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713948.1,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713948.2,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713948.4,
"object": null,
"category": null
},
{
"time": 1727713949.2,
"object": null,
"category": null
},
{
"time": 1727713949.4,
"object": "NetworkUser1",
"category": "Team"
},
{
"time": 1727713949.9,
"object": "NetworkHandR2",
"category": "Team"
},
{
"time": 1727713950.2,
"object": null,
"category": null
},
{
"time": 1727713950.8,
"object": "Patient Right Leg",
"category": "Patient"
},
{
"time": 1727713951,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713951.3,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713951.5,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713951.8,
"object": null,
"category": null
},
{
"time": 1727713954.4,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713955.1,
"object": null,
"category": null
},
{
"time": 1727713956.8,
"object": null,
"category": null
},
{
"time": 1727713957.1,
"object": null,
"category": null
},
{
"time": 1727713957.4,
"object": null,
"category": null
},
{
"time": 1727713958,
"object": null,
"category": null
},
{
"time": 1727713958.8,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713958.9,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713959.2,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713959.4,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713959.7,
"object": null,
"category": null
},
{
"time": 1727713960.4,
"object": "Patient Left Leg",
"category": "Patient"
},
{
"time": 1727713960.5,
"object": null,
"category": null
},
{
"time": 1727713961.3,
"object": null,
"category": null
},
{
"time": 1727713962.5,
"object": null,
"category": null
},
{
"time": 1727713963.3,
"object": "NetworkHandR2",
"category": "Team"
},
{
"time": 1727713963.5,
"object": null,
"category": null
},
{
"time": 1727713964.3,
"object": null,
"category": null
},
{
"time": 1727713965.3,
"object": "NetworkHandR3",
"category": "Team"
},
{
"time": 1727713965.4,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727713965.7,
"object": null,
"category": null
},
{
"time": 1727713966.3,
"object": null,
"category": null
},
{
"time": 1727713967.4,
"object": null,
"category": null
},
{
"time": 1727713968.4,
"object": null,
"category": null
},
{
"time": 1727713972.3,
"object": null,
"category": null
},
{
"time": 1727713973.5,
"object": null,
"category": null
},
{
"time": 1727713974.9,
"object": null,
"category": null
},
{
"time": 1727713975,
"object": null,
"category": null
},
{
"time": 1727713975.2,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713975.5,
"object": "Down Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713975.7,
"object": "Down Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713975.9,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713976.5,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727713976.7,
"object": null,
"category": null
},
{
"time": 1727713977.4,
"object": null,
"category": null
},
{
"time": 1727713981,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713981.2,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713981.5,
"object": null,
"category": null
},
{
"time": 1727713981.9,
"object": null,
"category": null
},
{
"time": 1727713982.4,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727713982.8,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727713982.9,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727713983.2,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727713983.7,
"object": null,
"category": null
},
{
"time": 1727713983.9,
"object": null,
"category": null
},
{
"time": 1727713984.6,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713984.8,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713985,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713985.4,
"object": null,
"category": null
},
{
"time": 1727713990.1,
"object": null,
"category": null
},
{
"time": 1727713992.3,
"object": "Patient Right Leg",
"category": "Patient"
},
{
"time": 1727713992.4,
"object": null,
"category": null
},
{
"time": 1727713993,
"object": "Patient Right Leg",
"category": "Patient"
},
{
"time": 1727713993.3,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727713993.6,
"object": null,
"category": null
},
{
"time": 1727713994.3,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713994.4,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713994.5,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713994.9,
"object": null,
"category": null
},
{
"time": 1727713995.2,
"object": "NetworkHandR2",
"category": "Team"
},
{
"time": 1727713995.4,
"object": null,
"category": null
},
{
"time": 1727713995.6,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713995.7,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713996,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713996.3,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727713996.9,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727713997.2,
"object": null,
"category": null
},
{
"time": 1727713997.8,
"object": "NetworkUser2",
"category": "Team"
},
{
"time": 1727713998.6,
"object": null,
"category": null
},
{
"time": 1727713999.3,
"object": null,
"category": null
},
{
"time": 1727714000.2,
"object": "Pulse Point Groin - Defib Machine",
"category": "Others"
},
{
"time": 1727714000.4,
"object": null,
"category": null
},
{
"time": 1727714000.5,
"object": "NetworkUser2",
"category": "Team"
},
{
"time": 1727714000.9,
"object": "Patient Left Arm",
"category": "Patient"
},
{
"time": 1727714001.1,
"object": "Patient Left Leg",
"category": "Patient"
},
{
"time": 1727714001.2,
"object": "Patient Left Leg",
"category": "Patient"
},
{
"time": 1727714001.5,
"object": "Patient Left Leg",
"category": "Patient"
},
{
"time": 1727714001.8,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714002,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714002.2,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714002.5,
"object": null,
"category": null
},
{
"time": 1727714002.8,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714003.1,
"object": null,
"category": null
},
{
"time": 1727714003.2,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714003.4,
"object": null,
"category": null
},
{
"time": 1727714003.7,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714004,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714004.2,
"object": null,
"category": null
},
{
"time": 1727714004.3,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714004.4,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714004.6,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714004.7,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714004.9,
"object": "Down Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714005.2,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714005.4,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714005.7,
"object": null,
"category": null
},
{
"time": 1727714005.9,
"object": "Patient Left Leg",
"category": "Patient"
},
{
"time": 1727714006.3,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714006.5,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714006.8,
"object": null,
"category": null
},
{
"time": 1727714006.9,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714007.2,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714007.5,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714008.3,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727714008.4,
"object": null,
"category": null
},
{
"time": 1727714008.8,
"object": "Patient Left Leg",
"category": "Patient"
},
{
"time": 1727714009.2,
"object": "Patient Left Leg",
"category": "Patient"
},
{
"time": 1727714009.9,
"object": "Patient Left Leg",
"category": "Patient"
},
{
"time": 1727714010.2,
"object": "Patient Left Leg",
"category": "Patient"
},
{
"time": 1727714010.6,
"object": null,
"category": null
},
{
"time": 1727714010.9,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727714011.2,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727714011.5,
"object": null,
"category": null
},
{
"time": 1727714011.7,
"object": null,
"category": null
},
{
"time": 1727714012.3,
"object": "Patient Left Leg",
"category": "Patient"
},
{
"time": 1727714012.5,
"object": "Patient Left Leg",
"category": "Patient"
},
{
"time": 1727714013.1,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714013.4,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727714013.5,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727714013.7,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727714014.1,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727714014.2,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727714015.2,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727714015.5,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727714015.8,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727714016.2,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727714016.8,
"object": null,
"category": null
},
{
"time": 1727714017,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727714017.4,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727714017.5,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727714017.7,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727714018.2,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727714018.6,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727714019.1,
"object": null,
"category": null
},
{
"time": 1727714019.8,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727714020.1,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727714020.8,
"object": "NetworkUser1",
"category": "Team"
},
{
"time": 1727714020.9,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727714021.7,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727714021.9,
"object": "Defibrillator Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714022.5,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727714023.1,
"object": "Defibrillator Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714023.7,
"object": null,
"category": null
},
{
"time": 1727714023.9,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727714024.3,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727714024.6,
"object": null,
"category": null
},
{
"time": 1727714025.4,
"object": null,
"category": null
},
{
"time": 1727714025.6,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714025.7,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714026,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714026.2,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714026.3,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714026.5,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714026.6,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714026.7,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714026.8,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714027,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714027.4,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714027.7,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714028.6,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714028.7,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714029.2,
"object": "NetworkUser1",
"category": "Team"
},
{
"time": 1727714029.4,
"object": "Patient Left Leg",
"category": "Patient"
},
{
"time": 1727714029.7,
"object": "Down Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714030,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714030.2,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714030.6,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714030.7,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714030.9,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714031.1,
"object": "NetworkUser1",
"category": "Team"
},
{
"time": 1727714031.2,
"object": "NetworkUser1",
"category": "Team"
},
{
"time": 1727714031.4,
"object": null,
"category": null
},
{
"time": 1727714032.8,
"object": null,
"category": null
},
{
"time": 1727714033,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727714033.4,
"object": null,
"category": null
},
{
"time": 1727714034,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714034.2,
"object": null,
"category": null
},
{
"time": 1727714034.6,
"object": null,
"category": null
},
{
"time": 1727714035.1,
"object": null,
"category": null
},
{
"time": 1727714037.2,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714037.4,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714037.6,
"object": null,
"category": null
},
{
"time": 1727714037.8,
"object": null,
"category": null
},
{
"time": 1727714039,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714039.6,
"object": null,
"category": null
},
{
"time": 1727714040,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714040.2,
"object": null,
"category": null
},
{
"time": 1727714040.4,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714041,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714041.1,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714041.4,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714042.2,
"object": null,
"category": null
},
{
"time": 1727714042.3,
"object": null,
"category": null
},
{
"time": 1727714042.6,
"object": null,
"category": null
},
{
"time": 1727714042.8,
"object": null,
"category": null
},
{
"time": 1727714043.3,
"object": null,
"category": null
},
{
"time": 1727714044.1,
"object": null,
"category": null
},
{
"time": 1727714044.2,
"object": null,
"category": null
},
{
"time": 1727714044.5,
"object": null,
"category": null
},
{
"time": 1727714046.5,
"object": null,
"category": null
},
{
"time": 1727714046.7,
"object": null,
"category": null
},
{
"time": 1727714046.9,
"object": null,
"category": null
},
{
"time": 1727714047.5,
"object": null,
"category": null
},
{
"time": 1727714047.8,
"object": "NetworkHandR3",
"category": "Team"
},
{
"time": 1727714048,
"object": "Patient Right Leg",
"category": "Patient"
},
{
"time": 1727714048.2,
"object": "Patient Right Leg",
"category": "Patient"
},
{
"time": 1727714048.5,
"object": null,
"category": null
},
{
"time": 1727714048.6,
"object": "NetworkHandR3",
"category": "Team"
},
{
"time": 1727714048.9,
"object": "NetworkHandR3",
"category": "Team"
},
{
"time": 1727714049.1,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727714049.2,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727714049.5,
"object": "NetworkHandL3",
"category": "Team"
},
{
"time": 1727714049.7,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727714049.9,
"object": "NetworkHandR3",
"category": "Team"
},
{
"time": 1727714050.1,
"object": "NetworkHandR3",
"category": "Team"
},
{
"time": 1727714051.1,
"object": "NetworkHandL3",
"category": "Team"
},
{
"time": 1727714051.5,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714051.6,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714052,
"object": "Patient Right Leg",
"category": "Patient"
},
{
"time": 1727714052.2,
"object": "Patient Right Leg",
"category": "Patient"
},
{
"time": 1727714052.6,
"object": null,
"category": null
},
{
"time": 1727714054,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714054.1,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714054.4,
"object": "Down Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714054.6,
"object": "A5PumpMask_R(Clone)",
"category": "Equipment"
},
{
"time": 1727714054.8,
"object": "Patient Left Leg",
"category": "Patient"
},
{
"time": 1727714055,
"object": null,
"category": null
},
{
"time": 1727714055.3,
"object": null,
"category": null
},
{
"time": 1727714055.7,
"object": "Defibrillator(Clone)",
"category": "Equipment"
},
{
"time": 1727714055.8,
"object": "NetworkHandL3",
"category": "Team"
},
{
"time": 1727714057.2,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727714057.4,
"object": "Defibrillator(Clone)",
"category": "Equipment"
},
{
"time": 1727714057.6,
"object": "Patient Right Leg",
"category": "Patient"
},
{
"time": 1727714057.8,
"object": "Patient Right Leg",
"category": "Patient"
},
{
"time": 1727714058.9,
"object": "Patient Right Leg",
"category": "Patient"
},
{
"time": 1727714059.7,
"object": null,
"category": null
},
{
"time": 1727714061.1,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714061.7,
"object": null,
"category": null
},
{
"time": 1727714062.1,
"object": null,
"category": null
},
{
"time": 1727714063,
"object": null,
"category": null
},
{
"time": 1727714063.7,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714064.1,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714064.3,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714064.6,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714064.8,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714065,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714065.2,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714065.7,
"object": "Patient Right Leg",
"category": "Patient"
},
{
"time": 1727714065.8,
"object": "Patient Right Leg",
"category": "Patient"
},
{
"time": 1727714066.1,
"object": null,
"category": null
},
{
"time": 1727714066.6,
"object": "NetworkUser1",
"category": "Team"
},
{
"time": 1727714066.8,
"object": "NetworkUser1",
"category": "Team"
},
{
"time": 1727714067,
"object": "NetworkHandR2",
"category": "Team"
},
{
"time": 1727714067.4,
"object": "NetworkUser1",
"category": "Team"
},
{
"time": 1727714067.8,
"object": "NetworkUser2",
"category": "Team"
},
{
"time": 1727714068.3,
"object": null,
"category": null
},
{
"time": 1727714068.6,
"object": null,
"category": null
},
{
"time": 1727714068.7,
"object": "NetworkUser2",
"category": "Team"
},
{
"time": 1727714070,
"object": "NetworkHandR2",
"category": "Team"
},
{
"time": 1727714070.3,
"object": null,
"category": null
},
{
"time": 1727714071.6,
"object": null,
"category": null
},
{
"time": 1727714072.6,
"object": "Patient Right Leg",
"category": "Patient"
},
{
"time": 1727714072.8,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727714073,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727714073.3,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727714073.5,
"object": null,
"category": null
},
{
"time": 1727714074.2,
"object": "NetworkUser2",
"category": "Team"
},
{
"time": 1727714074.8,
"object": "NetworkUser2",
"category": "Team"
},
{
"time": 1727714076.7,
"object": "NetworkUser2",
"category": "Team"
},
{
"time": 1727714076.9,
"object": "NetworkUser2",
"category": "Team"
},
{
"time": 1727714077.1,
"object": null,
"category": null
},
{
"time": 1727714077.5,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727714077.7,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727714078.6,
"object": "Patient Left Arm",
"category": "Patient"
},
{
"time": 1727714078.8,
"object": "NetworkUser2",
"category": "Team"
},
{
"time": 1727714079.2,
"object": null,
"category": null
},
{
"time": 1727714081,
"object": null,
"category": null
},
{
"time": 1727714081.5,
"object": null,
"category": null
},
{
"time": 1727714082,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714082.1,
"object": "Down Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714082.4,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714082.6,
"object": "Down Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714082.7,
"object": "Down Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714082.9,
"object": "Down Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714083.5,
"object": null,
"category": null
},
{
"time": 1727714083.8,
"object": "NetworkUser2",
"category": "Team"
},
{
"time": 1727714084.6,
"object": null,
"category": null
},
{
"time": 1727714084.8,
"object": "NetworkUser1",
"category": "Team"
},
{
"time": 1727714085,
"object": "NetworkUser1",
"category": "Team"
},
{
"time": 1727714085.1,
"object": "NetworkHandR2",
"category": "Team"
},
{
"time": 1727714085.4,
"object": "NetworkUser2",
"category": "Team"
},
{
"time": 1727714086,
"object": null,
"category": null
},
{
"time": 1727714086.8,
"object": null,
"category": null
},
{
"time": 1727714087.5,
"object": null,
"category": null
},
{
"time": 1727714087.7,
"object": null,
"category": null
},
{
"time": 1727714088.4,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714089.2,
"object": null,
"category": null
},
{
"time": 1727714090,
"object": null,
"category": null
},
{
"time": 1727714091.4,
"object": null,
"category": null
},
{
"time": 1727714093,
"object": null,
"category": null
},
{
"time": 1727714093.3,
"object": null,
"category": null
},
{
"time": 1727714093.6,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714093.7,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714094,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714094.2,
"object": "NetworkHandR2",
"category": "Team"
},
{
"time": 1727714094.6,
"object": null,
"category": null
},
{
"time": 1727714095.7,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727714095.9,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727714096,
"object": null,
"category": null
},
{
"time": 1727714096.7,
"object": null,
"category": null
},
{
"time": 1727714098.1,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727714099.1,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727714100.1,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727714100.6,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727714100.8,
"object": null,
"category": null
},
{
"time": 1727714101.4,
"object": null,
"category": null
},
{
"time": 1727714102,
"object": null,
"category": null
},
{
"time": 1727714102.3,
"object": null,
"category": null
},
{
"time": 1727714102.8,
"object": null,
"category": null
},
{
"time": 1727714103.8,
"object": null,
"category": null
},
{
"time": 1727714105.3,
"object": null,
"category": null
},
{
"time": 1727714105.7,
"object": null,
"category": null
},
{
"time": 1727714108.9,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714109,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714109.4,
"object": null,
"category": null
},
{
"time": 1727714110.4,
"object": null,
"category": null
},
{
"time": 1727714110.7,
"object": null,
"category": null
},
{
"time": 1727714111.7,
"object": null,
"category": null
},
{
"time": 1727714114.4,
"object": "Patient Right Leg",
"category": "Patient"
},
{
"time": 1727714114.5,
"object": null,
"category": null
},
{
"time": 1727714115.2,
"object": "Patient Right Leg",
"category": "Patient"
},
{
"time": 1727714115.8,
"object": "Patient Right Leg",
"category": "Patient"
},
{
"time": 1727714116.1,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727714116.5,
"object": null,
"category": null
},
{
"time": 1727714116.7,
"object": "Patient Right Leg",
"category": "Patient"
},
{
"time": 1727714117.4,
"object": "Patient Right Leg",
"category": "Patient"
},
{
"time": 1727714117.6,
"object": null,
"category": null
},
{
"time": 1727714119.5,
"object": null,
"category": null
},
{
"time": 1727714119.7,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727714120,
"object": null,
"category": null
},
{
"time": 1727714120.3,
"object": null,
"category": null
},
{
"time": 1727714120.6,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714120.8,
"object": null,
"category": null
},
{
"time": 1727714121.2,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714121.3,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714121.7,
"object": null,
"category": null
},
{
"time": 1727714122.3,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714122.9,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714123,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714123.2,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714123.5,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714123.6,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714124,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714124.3,
"object": "NetworkHandL1",
"category": "Team"
},
{
"time": 1727714124.9,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714125,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714125.6,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714125.9,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714126.3,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714126.7,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714127.3,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727714127.9,
"object": null,
"category": null
},
{
"time": 1727714129.2,
"object": "Pulse Point Groin - Defib Machine",
"category": "Others"
},
{
"time": 1727714129.3,
"object": null,
"category": null
},
{
"time": 1727714129.5,
"object": null,
"category": null
},
{
"time": 1727714129.7,
"object": "Defibrillator(Clone)",
"category": "Equipment"
},
{
"time": 1727714129.9,
"object": "NetworkHandL3",
"category": "Team"
},
{
"time": 1727714130,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727714130.3,
"object": null,
"category": null
},
{
"time": 1727714130.9,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727714131.3,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727714131.6,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727714132,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727714132.3,
"object": "Defibrillator(Clone)",
"category": "Equipment"
},
{
"time": 1727714132.6,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727714132.7,
"object": "Defibrillator(Clone)",
"category": "Equipment"
},
{
"time": 1727714132.9,
"object": "NetworkHandL3",
"category": "Team"
},
{
"time": 1727714133,
"object": "NetworkHandL3",
"category": "Team"
},
{
"time": 1727714133.4,
"object": "Defibrillator(Clone)",
"category": "Equipment"
},
{
"time": 1727714133.7,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727714134.2,
"object": "Defibrillator(Clone)",
"category": "Equipment"
},
{
"time": 1727714134.3,
"object": "Defibrillator Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714134.5,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727714134.7,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727714137.6,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727714138.2,
"object": "Down Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714138.5,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727714138.7,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727714139,
"object": null,
"category": null
},
{
"time": 1727714139.8,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727714140.3,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727714141.1,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727714141.6,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727714141.7,
"object": "NetworkHandR2",
"category": "Team"
},
{
"time": 1727714142,
"object": null,
"category": null
},
{
"time": 1727714142.1,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727714142.9,
"object": null,
"category": null
},
{
"time": 1727714143.3,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727714143.9,
"object": null,
"category": null
},
{
"time": 1727714144.4,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727714144.6,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727714144.7,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727714145.1,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727714145.3,
"object": "Defibrillator Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714145.4,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727714145.6,
"object": "Pulse Point Groin - Defib Machine",
"category": "Others"
},
{
"time": 1727714145.9,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727714146.5,
"object": "Pulse Point Groin - Defib Machine",
"category": "Others"
},
{
"time": 1727714146.6,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727714146.8,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727714147,
"object": "Defibrillator Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714147.1,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727714147.4,
"object": null,
"category": null
},
{
"time": 1727714147.7,
"object": null,
"category": null
},
{
"time": 1727714148.2,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714148.3,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714148.6,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714149.2,
"object": null,
"category": null
},
{
"time": 1727714149.7,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714150,
"object": null,
"category": null
},
{
"time": 1727714150.7,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714151.1,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714151.2,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714151.5,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714151.7,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714152,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714152.2,
"object": "NetworkHandR2",
"category": "Team"
},
{
"time": 1727714152.5,
"object": null,
"category": null
},
{
"time": 1727714153.2,
"object": null,
"category": null
},
{
"time": 1727714154.9,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714155.2,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714155.6,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714155.9,
"object": null,
"category": null
},
{
"time": 1727714156.7,
"object": null,
"category": null
},
{
"time": 1727714157,
"object": "Patient Right Leg",
"category": "Patient"
},
{
"time": 1727714157.2,
"object": null,
"category": null
},
{
"time": 1727714157.7,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714157.9,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714158,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714158.7,
"object": "Patient Right Leg",
"category": "Patient"
},
{
"time": 1727714158.9,
"object": "Patient Right Leg",
"category": "Patient"
},
{
"time": 1727714158.9,
"object": "Patient Right Leg",
"category": "Patient"
},
{
"time": 1727714159,
"object": "Patient Right Leg",
"category": "Patient"
},
{
"time": 1727714159.2,
"object": "Patient Right Leg",
"category": "Patient"
},
{
"time": 1727714159.4,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727714159.9,
"object": null,
"category": null
},
{
"time": 1727714160.5,
"object": null,
"category": null
},
{
"time": 1727714160.6,
"object": null,
"category": null
},
{
"time": 1727714161.1,
"object": null,
"category": null
},
{
"time": 1727714161.3,
"object": null,
"category": null
},
{
"time": 1727714161.5,
"object": null,
"category": null
},
{
"time": 1727714161.6,
"object": null,
"category": null
},
{
"time": 1727714161.8,
"object": null,
"category": null
},
{
"time": 1727714162.1,
"object": null,
"category": null
},
{
"time": 1727714162.2,
"object": null,
"category": null
},
{
"time": 1727714162.6,
"object": "Tablet_C3D",
"category": "Tablet"
},
{
"time": 1727714162.7,
"object": "Tablet_C3D",
"category": "Tablet"
},
{
"time": 1727714163.2,
"object": null,
"category": null
},
{
"time": 1727714163.5,
"object": null,
"category": null
},
{
"time": 1727714164.2,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714164.4,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714164.5,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714164.7,
"object": "Down Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714165.1,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714165.2,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714165.4,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714165.6,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714165.9,
"object": "Down Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714166.1,
"object": "Down Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714166.4,
"object": "Down Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714166.7,
"object": "Down Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714167,
"object": "Down Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714167.2,
"object": "Down Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714167.4,
"object": "Down Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714168.2,
"object": "Down Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714168.4,
"object": "Down Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714168.8,
"object": "Down Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714168.9,
"object": "Down Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714169.1,
"object": null,
"category": null
},
{
"time": 1727714169.5,
"object": "Down Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714169.9,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714170.3,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714170.7,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714170.9,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714171.5,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714171.9,
"object": "NetworkHandR2",
"category": "Team"
},
{
"time": 1727714172.1,
"object": null,
"category": null
},
{
"time": 1727714172.5,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727714172.8,
"object": "NetworkUser1",
"category": "Team"
},
{
"time": 1727714173.2,
"object": "NetworkUser2",
"category": "Team"
},
{
"time": 1727714173.9,
"object": null,
"category": null
},
{
"time": 1727714175.2,
"object": null,
"category": null
},
{
"time": 1727714175.7,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714176,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714176.2,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714176.3,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714176.5,
"object": null,
"category": null
},
{
"time": 1727714177.2,
"object": "Down Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714177.3,
"object": "Down Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714177.7,
"object": "Down Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714178,
"object": "Down Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714178.8,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714179.1,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714179.4,
"object": "Down Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714179.8,
"object": "NetworkHandR2",
"category": "Team"
},
{
"time": 1727714180,
"object": null,
"category": null
},
{
"time": 1727714180.3,
"object": null,
"category": null
},
{
"time": 1727714180.8,
"object": null,
"category": null
},
{
"time": 1727714180.9,
"object": "NetworkUser2",
"category": "Team"
},
{
"time": 1727714181,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714181.1,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714181.3,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714181.5,
"object": null,
"category": null
},
{
"time": 1727714182,
"object": "NetworkUser2",
"category": "Team"
},
{
"time": 1727714182.5,
"object": null,
"category": null
},
{
"time": 1727714182.7,
"object": null,
"category": null
},
{
"time": 1727714184,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714184.2,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714185,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714185.1,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714185.2,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714185.5,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714186.1,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714186.4,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714186.9,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714187.2,
"object": null,
"category": null
},
{
"time": 1727714189.1,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714189.9,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714190.1,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714190.4,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714190.6,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714190.8,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714191.4,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714192.1,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714192.7,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714193.1,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714193.3,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714193.5,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714193.7,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714194.1,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714194.3,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714194.6,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714195.1,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714195.8,
"object": null,
"category": null
},
{
"time": 1727714196.4,
"object": null,
"category": null
},
{
"time": 1727714196.7,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714196.8,
"object": null,
"category": null
},
{
"time": 1727714197.1,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727714197.3,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727714198,
"object": "NetworkUser2",
"category": "Team"
},
{
"time": 1727714198.4,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714198.7,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714198.9,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714199.6,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714199.8,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714200.1,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714200.4,
"object": null,
"category": null
},
{
"time": 1727714201.4,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714201.7,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714202.2,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714202.5,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714202.7,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714202.8,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714203.5,
"object": "Down Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714203.7,
"object": "Down Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714203.9,
"object": "Down Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714204.2,
"object": "Down Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714204.5,
"object": null,
"category": null
},
{
"time": 1727714204.7,
"object": "Down Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714205.2,
"object": "Down Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714205.5,
"object": null,
"category": null
},
{
"time": 1727714205.7,
"object": "Patient Right Leg",
"category": "Patient"
},
{
"time": 1727714206,
"object": "Patient Right Leg",
"category": "Patient"
},
{
"time": 1727714206.2,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727714207.1,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714207.6,
"object": null,
"category": null
},
{
"time": 1727714209.7,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714209.9,
"object": null,
"category": null
},
{
"time": 1727714210,
"object": "NetworkUser2",
"category": "Team"
},
{
"time": 1727714210.6,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714210.9,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714211.3,
"object": null,
"category": null
},
{
"time": 1727714213.5,
"object": "Patient Left Leg",
"category": "Patient"
},
{
"time": 1727714213.7,
"object": null,
"category": null
},
{
"time": 1727714214.2,
"object": null,
"category": null
},
{
"time": 1727714214.3,
"object": null,
"category": null
},
{
"time": 1727714214.8,
"object": null,
"category": null
},
{
"time": 1727714215.1,
"object": null,
"category": null
},
{
"time": 1727714215.3,
"object": null,
"category": null
},
{
"time": 1727714215.5,
"object": null,
"category": null
},
{
"time": 1727714216.4,
"object": "Lactated_Ringers_Interactable(Clone)",
"category": "Others"
},
{
"time": 1727714216.6,
"object": null,
"category": null
},
{
"time": 1727714216.7,
"object": null,
"category": null
},
{
"time": 1727714217.8,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714218,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714218.5,
"object": null,
"category": null
},
{
"time": 1727714218.7,
"object": null,
"category": null
},
{
"time": 1727714218.9,
"object": null,
"category": null
},
{
"time": 1727714219.1,
"object": null,
"category": null
},
{
"time": 1727714219.4,
"object": null,
"category": null
},
{
"time": 1727714219.6,
"object": "Bipap_Mask_Interactable(Clone)",
"category": "Equipment"
},
{
"time": 1727714220,
"object": null,
"category": null
},
{
"time": 1727714220.2,
"object": null,
"category": null
},
{
"time": 1727714220.4,
"object": null,
"category": null
},
{
"time": 1727714221.5,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714221.6,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714221.7,
"object": null,
"category": null
},
{
"time": 1727714222.9,
"object": null,
"category": null
},
{
"time": 1727714225.7,
"object": null,
"category": null
},
{
"time": 1727714226.6,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714227.1,
"object": null,
"category": null
},
{
"time": 1727714227.7,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714227.8,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714227.9,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714228,
"object": null,
"category": null
},
{
"time": 1727714229,
"object": null,
"category": null
},
{
"time": 1727714229.3,
"object": null,
"category": null
},
{
"time": 1727714235.3,
"object": "Patient Right Leg",
"category": "Patient"
},
{
"time": 1727714235.5,
"object": null,
"category": null
},
{
"time": 1727714236.6,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714237.2,
"object": null,
"category": null
},
{
"time": 1727714238.5,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714238.7,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714239,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714239.4,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714239.8,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714240,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714240.1,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714240.5,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714240.8,
"object": null,
"category": null
},
{
"time": 1727714241.3,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714241.6,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714242.2,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714242.4,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714242.6,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714242.8,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714243.1,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714243.2,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714243.4,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714243.6,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714245.1,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714245.2,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714245.7,
"object": null,
"category": null
},
{
"time": 1727714246,
"object": null,
"category": null
},
{
"time": 1727714246.9,
"object": null,
"category": null
},
{
"time": 1727714247.1,
"object": null,
"category": null
},
{
"time": 1727714247.2,
"object": "NetworkUser2",
"category": "Team"
},
{
"time": 1727714247.5,
"object": "NetworkUser1",
"category": "Team"
},
{
"time": 1727714247.7,
"object": null,
"category": null
},
{
"time": 1727714248,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714248.4,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714248.5,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714248.6,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714249,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714249.2,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714249.4,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714249.5,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714249.6,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714249.8,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714250.1,
"object": null,
"category": null
},
{
"time": 1727714250.4,
"object": "Patient Right Leg",
"category": "Patient"
},
{
"time": 1727714251.1,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714251.2,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714251.7,
"object": "Down Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714252.1,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714252.4,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714252.8,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714252.9,
"object": "Down Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714253.1,
"object": "Down Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714253.3,
"object": "Down Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714253.4,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714253.6,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714253.7,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714253.8,
"object": "Down Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714254,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714254.1,
"object": "Down Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714254.3,
"object": "Down Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714254.5,
"object": "NetworkUser2",
"category": "Team"
},
{
"time": 1727714255,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714255.6,
"object": null,
"category": null
},
{
"time": 1727714255.9,
"object": null,
"category": null
},
{
"time": 1727714256.2,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727714256.5,
"object": null,
"category": null
},
{
"time": 1727714256.8,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727714257.2,
"object": null,
"category": null
},
{
"time": 1727714257.7,
"object": "NetworkUser1",
"category": "Team"
},
{
"time": 1727714258.2,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727714258.9,
"object": "NetworkUser1",
"category": "Team"
},
{
"time": 1727714259.2,
"object": null,
"category": null
},
{
"time": 1727714259.7,
"object": null,
"category": null
},
{
"time": 1727714259.9,
"object": null,
"category": null
},
{
"time": 1727714260.1,
"object": null,
"category": null
},
{
"time": 1727714260.4,
"object": null,
"category": null
},
{
"time": 1727714260.9,
"object": "Pulse Point Neck - AirWay/BVM",
"category": "Others"
},
{
"time": 1727714261.1,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727714261.5,
"object": null,
"category": null
},
{
"time": 1727714265.3,
"object": null,
"category": null
},
{
"time": 1727714266.5,
"object": "Pulse Point Neck - AirWay/BVM",
"category": "Others"
},
{
"time": 1727714266.7,
"object": null,
"category": null
},
{
"time": 1727714266.9,
"object": "NetworkUser1",
"category": "Team"
},
{
"time": 1727714267.1,
"object": null,
"category": null
},
{
"time": 1727714267.2,
"object": "NetworkUser1",
"category": "Team"
},
{
"time": 1727714268.2,
"object": null,
"category": null
},
{
"time": 1727714268.6,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727714268.8,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727714269.4,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714269.5,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714269.8,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714270.2,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714270.7,
"object": "Cpr Hands Object",
"category": "Others"
},
{
"time": 1727714270.8,
"object": null,
"category": null
},
{
"time": 1727714271.7,
"object": null,
"category": null
},
{
"time": 1727714272.4,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727714272.6,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727714273.2,
"object": null,
"category": null
},
{
"time": 1727714274.2,
"object": null,
"category": null
},
{
"time": 1727714274.8,
"object": "NetworkUser1",
"category": "Team"
},
{
"time": 1727714275,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727714275.4,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727714275.5,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727714275.6,
"object": null,
"category": null
},
{
"time": 1727714277.6,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714278.3,
"object": "Down Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714278.4,
"object": null,
"category": null
},
{
"time": 1727714278.8,
"object": "Down Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714279.1,
"object": "Down Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714279.8,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714280.1,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714280.2,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714280.5,
"object": "Down Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714281.3,
"object": "Down Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714281.6,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714281.8,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714282,
"object": null,
"category": null
},
{
"time": 1727714282.3,
"object": "Pulse Point Neck - AirWay/BVM",
"category": "Others"
},
{
"time": 1727714282.6,
"object": "Cpr Hands Object",
"category": "Others"
},
{
"time": 1727714282.8,
"object": "Cpr Hands Object",
"category": "Others"
},
{
"time": 1727714282.9,
"object": "Pulse Point Neck - AirWay/BVM",
"category": "Others"
},
{
"time": 1727714283.1,
"object": "NetworkUser1",
"category": "Team"
},
{
"time": 1727714283.5,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727714283.8,
"object": "Pulse Point Neck - AirWay/BVM",
"category": "Others"
},
{
"time": 1727714284,
"object": "Pulse Point Neck - AirWay/BVM",
"category": "Others"
},
{
"time": 1727714284.2,
"object": "Pulse Point Neck - AirWay/BVM",
"category": "Others"
},
{
"time": 1727714284.5,
"object": "Pulse Point Neck - AirWay/BVM",
"category": "Others"
},
{
"time": 1727714284.7,
"object": "Pulse Point Neck - AirWay/BVM",
"category": "Others"
},
{
"time": 1727714284.8,
"object": "Pulse Point Neck - AirWay/BVM",
"category": "Others"
},
{
"time": 1727714285.2,
"object": "Pulse Point Neck - AirWay/BVM",
"category": "Others"
},
{
"time": 1727714285.3,
"object": "Cpr Hands Object",
"category": "Others"
},
{
"time": 1727714285.5,
"object": "Pulse Point Neck - AirWay/BVM",
"category": "Others"
},
{
"time": 1727714285.6,
"object": "Pulse Point Neck - AirWay/BVM",
"category": "Others"
},
{
"time": 1727714285.8,
"object": "Pulse Point Neck - AirWay/BVM",
"category": "Others"
},
{
"time": 1727714285.9,
"object": "Pulse Point Neck - AirWay/BVM",
"category": "Others"
},
{
"time": 1727714286.1,
"object": "Pulse Point Neck - AirWay/BVM",
"category": "Others"
},
{
"time": 1727714286.3,
"object": "Pulse Point Neck - AirWay/BVM",
"category": "Others"
},
{
"time": 1727714286.7,
"object": "Pulse Point Neck - AirWay/BVM",
"category": "Others"
},
{
"time": 1727714286.8,
"object": "Pulse Point Neck - AirWay/BVM",
"category": "Others"
},
{
"time": 1727714286.9,
"object": "Pulse Point Neck - AirWay/BVM",
"category": "Others"
},
{
"time": 1727714287.1,
"object": "Pulse Point Neck - AirWay/BVM",
"category": "Others"
},
{
"time": 1727714287.2,
"object": "Pulse Point Neck - AirWay/BVM",
"category": "Others"
},
{
"time": 1727714287.6,
"object": "Pulse Point Neck - AirWay/BVM",
"category": "Others"
},
{
"time": 1727714287.6,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727714288,
"object": "NetworkUser1",
"category": "Team"
},
{
"time": 1727714288.6,
"object": null,
"category": null
},
{
"time": 1727714289.2,
"object": "Pulse Point Neck - AirWay/BVM",
"category": "Others"
},
{
"time": 1727714289.4,
"object": "Pulse Point Neck - AirWay/BVM",
"category": "Others"
},
{
"time": 1727714289.6,
"object": "Pulse Point Neck - AirWay/BVM",
"category": "Others"
},
{
"time": 1727714289.8,
"object": null,
"category": null
},
{
"time": 1727714290.3,
"object": "NetworkUser2",
"category": "Team"
},
{
"time": 1727714290.5,
"object": null,
"category": null
},
{
"time": 1727714290.8,
"object": "NetworkUser1",
"category": "Team"
},
{
"time": 1727714291.3,
"object": null,
"category": null
},
{
"time": 1727714291.9,
"object": "Patient Left Leg",
"category": "Patient"
},
{
"time": 1727714292.3,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714292.8,
"object": null,
"category": null
},
{
"time": 1727714293,
"object": null,
"category": null
},
{
"time": 1727714293.3,
"object": null,
"category": null
},
{
"time": 1727714293.9,
"object": null,
"category": null
},
{
"time": 1727714294,
"object": null,
"category": null
},
{
"time": 1727714294.4,
"object": null,
"category": null
},
{
"time": 1727714294.8,
"object": "Non_Breather_Interactable(Clone)",
"category": "Others"
},
{
"time": 1727714295,
"object": null,
"category": null
},
{
"time": 1727714295.6,
"object": null,
"category": null
},
{
"time": 1727714295.9,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714296.1,
"object": "Upper Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714296.2,
"object": "Middle Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714296.4,
"object": null,
"category": null
},
{
"time": 1727714297,
"object": null,
"category": null
},
{
"time": 1727714297.2,
"object": "Patient Left Leg",
"category": "Patient"
},
{
"time": 1727714297.3,
"object": "Patient Left Leg",
"category": "Patient"
},
{
"time": 1727714297.5,
"object": "Patient Left Leg",
"category": "Patient"
},
{
"time": 1727714297.6,
"object": "Patient Left Leg",
"category": "Patient"
},
{
"time": 1727714297.9,
"object": "NetworkHandR3",
"category": "Team"
},
{
"time": 1727714298,
"object": "Patient Right Leg",
"category": "Patient"
},
{
"time": 1727714298.3,
"object": "Patient Right Leg",
"category": "Patient"
},
{
"time": 1727714298.5,
"object": "Patient Right Leg",
"category": "Patient"
},
{
"time": 1727714298.7,
"object": null,
"category": null
},
{
"time": 1727714299.2,
"object": null,
"category": null
},
{
"time": 1727714299.4,
"object": null,
"category": null
},
{
"time": 1727714299.6,
"object": null,
"category": null
},
{
"time": 1727714300,
"object": null,
"category": null
},
{
"time": 1727714300.2,
"object": "OneSkeleton_Hips1",
"category": "Others"
},
{
"time": 1727714300.3,
"object": null,
"category": null
},
{
"time": 1727714300.7,
"object": null,
"category": null
},
{
"time": 1727714300.9,
"object": "NetworkUser2",
"category": "Team"
},
{
"time": 1727714301,
"object": "NetworkUser2",
"category": "Team"
},
{
"time": 1727714301.3,
"object": null,
"category": null
},
{
"time": 1727714301.6,
"object": null,
"category": null
},
{
"time": 1727714302.7,
"object": "Patient Left Leg",
"category": "Patient"
},
{
"time": 1727714303.1,
"object": null,
"category": null
},
{
"time": 1727714303.3,
"object": null,
"category": null
},
{
"time": 1727714303.5,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727714303.8,
"object": null,
"category": null
},
{
"time": 1727714304.1,
"object": null,
"category": null
},
{
"time": 1727714304.4,
"object": "Patient Right Leg",
"category": "Patient"
},
{
"time": 1727714304.7,
"object": null,
"category": null
},
{
"time": 1727714305.2,
"object": "A5PumpMask_R(Clone)",
"category": "Equipment"
},
{
"time": 1727714305.4,
"object": null,
"category": null
},
{
"time": 1727714305.8,
"object": "Patient Left Leg",
"category": "Patient"
},
{
"time": 1727714306.2,
"object": null,
"category": null
},
{
"time": 1727714306.4,
"object": null,
"category": null
},
{
"time": 1727714306.7,
"object": "NetworkUser1",
"category": "Team"
},
{
"time": 1727714307.3,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727714307.6,
"object": null,
"category": null
},
{
"time": 1727714308,
"object": null,
"category": null
},
{
"time": 1727714308.2,
"object": null,
"category": null
},
{
"time": 1727714308.3,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727714308.8,
"object": null,
"category": null
},
{
"time": 1727714309,
"object": "NetworkUser1",
"category": "Team"
},
{
"time": 1727714309.4,
"object": "Defibrillator(Clone)",
"category": "Equipment"
},
{
"time": 1727714309.6,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727714309.7,
"object": null,
"category": null
},
{
"time": 1727714310,
"object": null,
"category": null
},
{
"time": 1727714310.3,
"object": "A5PumpMask_R(Clone)",
"category": "Equipment"
},
{
"time": 1727714310.4,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727714310.6,
"object": null,
"category": null
},
{
"time": 1727714311,
"object": "NetworkUser1",
"category": "Team"
},
{
"time": 1727714311.2,
"object": null,
"category": null
},
{
"time": 1727714312.1,
"object": null,
"category": null
},
{
"time": 1727714312.3,
"object": "NetworkHandR1",
"category": "Team"
},
{
"time": 1727714312.6,
"object": null,
"category": null
},
{
"time": 1727714312.8,
"object": null,
"category": null
},
{
"time": 1727714313.2,
"object": "NetworkUser1",
"category": "Team"
},
{
"time": 1727714313.4,
"object": "Patient Torso",
"category": "Patient"
},
{
"time": 1727714313.8,
"object": null,
"category": null
},
{
"time": 1727714314.1,
"object": "A5PumpMask_R(Clone)",
"category": "Equipment"
},
{
"time": 1727714314.4,
"object": null,
"category": null
},
{
"time": 1727714314.5,
"object": null,
"category": null
},
{
"time": 1727714314.7,
"object": null,
"category": null
},
{
"time": 1727714314.9,
"object": null,
"category": null
},
{
"time": 1727714315.1,
"object": null,
"category": null
},
{
"time": 1727714315.2,
"object": null,
"category": null
},
{
"time": 1727714315.4,
"object": null,
"category": null
},
{
"time": 1727714315.5,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727714315.7,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727714315.9,
"object": null,
"category": null
},
{
"time": 1727714316.1,
"object": "NetworkUser1",
"category": "Team"
},
{
"time": 1727714316.3,
"object": "NetworkUser1",
"category": "Team"
},
{
"time": 1727714316.8,
"object": "Defibrillator Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714316.9,
"object": null,
"category": null
},
{
"time": 1727714317.1,
"object": "NetworkUser1",
"category": "Team"
},
{
"time": 1727714317.3,
"object": "NetworkUser1",
"category": "Team"
},
{
"time": 1727714318,
"object": null,
"category": null
},
{
"time": 1727714318.2,
"object": "Patient Left Leg",
"category": "Patient"
},
{
"time": 1727714318.6,
"object": null,
"category": null
},
{
"time": 1727714318.8,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727714319,
"object": null,
"category": null
},
{
"time": 1727714319.3,
"object": "A5PumpMask_R(Clone)",
"category": "Equipment"
},
{
"time": 1727714319.4,
"object": "NetworkUser1",
"category": "Team"
},
{
"time": 1727714319.5,
"object": "A5PumpMask_R(Clone)",
"category": "Equipment"
},
{
"time": 1727714320,
"object": null,
"category": null
},
{
"time": 1727714320.4,
"object": "Pulse Point Groin - Defib Machine",
"category": "Others"
},
{
"time": 1727714320.5,
"object": null,
"category": null
},
{
"time": 1727714321,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727714321.2,
"object": "Defibrillator Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714321.5,
"object": null,
"category": null
},
{
"time": 1727714321.8,
"object": "NetworkUser1",
"category": "Team"
},
{
"time": 1727714322.1,
"object": "NetworkUser1",
"category": "Team"
},
{
"time": 1727714322.5,
"object": "Pulse Point Neck - AirWay/BVM",
"category": "Others"
},
{
"time": 1727714322.7,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727714323,
"object": null,
"category": null
},
{
"time": 1727714323.4,
"object": "NetworkUser1",
"category": "Team"
},
{
"time": 1727714323.9,
"object": null,
"category": null
},
{
"time": 1727714324.1,
"object": null,
"category": null
},
{
"time": 1727714324.3,
"object": "NetworkUser1",
"category": "Team"
},
{
"time": 1727714324.6,
"object": null,
"category": null
},
{
"time": 1727714324.7,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727714325,
"object": null,
"category": null
},
{
"time": 1727714325.1,
"object": null,
"category": null
},
{
"time": 1727714325.3,
"object": null,
"category": null
},
{
"time": 1727714325.5,
"object": null,
"category": null
},
{
"time": 1727714325.7,
"object": null,
"category": null
},
{
"time": 1727714326,
"object": null,
"category": null
},
{
"time": 1727714326.2,
"object": null,
"category": null
},
{
"time": 1727714326.4,
"object": null,
"category": null
},
{
"time": 1727714326.5,
"object": "NetworkUser1",
"category": "Team"
},
{
"time": 1727714326.6,
"object": "NetworkUser1",
"category": "Team"
},
{
"time": 1727714327.3,
"object": null,
"category": null
},
{
"time": 1727714328.3,
"object": null,
"category": null
},
{
"time": 1727714328.5,
"object": "NetworkUser3",
"category": "Team"
},
{
"time": 1727714329.3,
"object": "NetworkUser1",
"category": "Team"
},
{
"time": 1727714329.5,
"object": null,
"category": null
},
{
"time": 1727714329.8,
"object": "Down Part Vital Cognitive",
"category": "Monitors"
},
{
"time": 1727714330.2,
"object": "NetworkUser1",
"category": "Team"
}
]"#;

    let stream: Vec<Value> = from_str(data).unwrap(); // Deserialize once

    let window_duration_secs = 10;
    let cursor = Cursor::new(serde_json::to_vec(&stream).unwrap()); // Convert to Cursor

    let mut reader = cursor;
    if let Ok(normalized_data) = normalize_visual_attention_load_data(&mut reader).await {
        aggregate_category_ratios(normalized_data, window_duration_secs).for_each(
            |(category, date, ratio)| {
                println!("{} {} {}", category, date, ratio);
            },
        );
    }

    Ok(())
}

pub fn map_time_to_date(
    visual_attention_data: Value,
    first_timestamp: Option<f64>,
) -> Option<(f64, Option<String>, Option<f64>)> {
    if let Value::Object(map) = visual_attention_data {
        let time = map.get("time")?.as_f64()?;
        let category = map
            .get("category")
            .and_then(|v| v.as_str().map(String::from));
        let start_seconds = first_timestamp.unwrap_or(time);
        let normalized_seconds = time - start_seconds;
        Some((
            normalized_seconds,
            category,
            Some(start_seconds),
        ))
    } else {
        None
    }
}

fn parse_json_root<R: Read>(reader: R) -> Result<Vec<Value>, String> {
    let buf_reader = BufReader::new(reader);
    let mut stream = Deserializer::from_reader(buf_reader).into_iter::<Value>();

    match stream.next() {
        Some(Ok(Value::Array(root))) => Ok(root),
        Some(Ok(_)) => Err("JSON root is not an array".to_string()),
        Some(Err(e)) => Err(format!("Error deserializing JSON root: {}", e)),
        None => Err("JSON is empty".to_string()),
    }
}
pub async fn normalize_visual_attention_load_data(
    reader: &mut impl Read,
) -> Result<impl Iterator<Item = (f64, Option<String>)>, String> {
    let root_array = parse_json_root(reader)?;

    Ok(root_array.into_iter().scan(None, |state, item| {
        let mapped_time =
            map_time_to_date(item, *state).map(|(date_time, cognitive_load, first_timestamp)| {
                *state = first_timestamp;
                (date_time, cognitive_load)
            });

        mapped_time
    }))
}

pub fn aggregate_category_ratios(
    data_iter: impl Iterator<Item = (f64, Option<String>)>,
    window_size: u32,
) -> impl Iterator<Item = (String, String, f64)> {
    let sliding_window = SlidingWindow {
        data_iter,
        window_start: 0,
        window_end: 0,
        window_size,
        category_count: Default::default(),
        total_count: 0,
    };

    sliding_window.flat_map(|results| results.into_iter())
}

struct SlidingWindow<I: Iterator<Item = (f64, Option<String>)>> {
    data_iter: I,
    window_start: u32,
    window_end: u32,
    window_size: u32,
    category_count: HashMap<String, usize>,
    total_count: usize,
}

impl<I: Iterator<Item = (f64, Option<String>)>> Iterator for SlidingWindow<I> {
    type Item = Vec<(String, String, f64)>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut results = Vec::new();
        while let Some((time, category)) = self.data_iter.next() {
            if time >= self.window_end as f64 {
                for (cat, count) in self.category_count.drain() {
                    let window_end_date = seconds_to_csv_row_time(self.window_end).date_string;
                    results.push((cat, window_end_date, count as f64 / self.total_count as f64));
                }
                self.window_start = self.window_end;
                self.window_end += self.window_size;
                self.total_count = 0;
                self.category_count.clear();
            }

            if let Some(cat) = category {
                *self.category_count.entry(cat.clone()).or_insert(0) += 1;
                self.total_count += 1;
            }
        }

        if self.total_count > 0 {
            for (cat, count) in self.category_count.drain() {
                let window_end_date = seconds_to_csv_row_time(self.window_end).date_string;
                results.push((cat, window_end_date, count as f64 / self.total_count as f64));
            }
        }

        if results.is_empty() {
            None
        } else {
            Some(results)
        }
    }
}
