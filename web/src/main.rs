use crate::app_context::AppContext;
use crate::config::config::{DataSourceType, PlotType};
use actix_files as fs;
use actix_web::web::{Data, Path};
use actix_web::{guard, middleware, web, App, HttpRequest, HttpResponse, HttpServer, Responder};
use async_stream::stream;
use bytes::{Bytes, BytesMut};
use config::config::AppConfig;
use data_source::DataSource;
use futures::{StreamExt, TryStreamExt};
use futures::{stream, Stream};
use mteam_dashboard_action_processor::process_csv;
use mteam_dashboard_cognitive_load_processor::file_processor::process_cognitive_load_data;
use mteam_dashboard_plotly_processor::actions::plot_data::ActionsPlotData;
use mteam_dashboard_plotly_processor::{actions, visual_attention};
use serde::Deserialize;
use serde_json::{json, to_string};
use std::error::Error;
use std::io;
use std::io::Read;
use std::pin::Pin;
use std::sync::Arc;

mod app_context;
mod config;
pub mod data_source;
mod data_providers;

#[derive(Deserialize)]
struct RangeQuery {
    range: Option<String>,
}
async fn stream_video_handler(
    path: Path<String>,
    query: web::Query<RangeQuery>,
    data: Data<AppContext>,
) -> Result<HttpResponse, Box<dyn Error>> {
    let folder_id = path.into_inner();
    let range = query.range.clone();

    let (status_code, content_type, content_length, content_range, stream) = data
        .datasource_provider
        .stream_video(folder_id, range)
        .await
        .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;

    let mut response = HttpResponse::build(actix_web::http::StatusCode::from_u16(status_code).unwrap());
    response.content_type(content_type);
    response.insert_header(("Accept-Ranges", "bytes"));
    response.insert_header(("Access-Control-Allow-Origin", "*"));
    if let Some(len) = content_length {
        response.insert_header(("Content-Length", len.to_string()));
    }
    if let Some(cr) = content_range {
        response.insert_header(("Content-Range", cr));
    }

    Ok(response.streaming(stream.map_err(|e| actix_web::error::ErrorInternalServerError(e))))
}

async fn data_sources(context: Data<AppContext>) -> impl Responder {
    match context.datasource_provider.get_main_folder_list().await {
        Ok(folders) => HttpResponse::Ok().json(folders),
        Err(e) => HttpResponse::NotFound().json(
            json!({"Data sources not found": format!("Failed to get data sources: {:#?}", e)}),
        ),
    }
}
async fn plot_sources(path: Path<(String, String)>, context: Data<AppContext>) -> impl Responder {
    let source_order = &context.plotly_config.team_member_filter_settings.filter_selection_order;
    let (data_source_id, plot_name) = path.into_inner();
    match context.datasource_provider.fetch_json_file_map(data_source_id.as_str(), plot_name.as_str(), Some(source_order)).await {
        Ok(file_name_vec) => {
            HttpResponse::Ok() .content_type("application/json")
                .body(convert_vec_to_ordered_json_string(file_name_vec)) }
        Err(e) => HttpResponse::NotFound().json(json!({"Not found": format!("Failed to get file list for the selected data source and plot name: {:#?}", e)}))
    }
}

/**
    * Converts a vector of tuples to a JSON map whose keys are ordered by the order of the tuples in the vector
    * Serde is not helping because under the hood it uses BTreeMap which orders the resulting json maps keys ordered alphabetically.
    * The other option is to use a dependency such as IndexMap to preserve insert order, but it's not worth it for this simple use case.
    * @param vec: Vec<(String, String)> - The vector of tuples to convert
    * @return String - The ordered JSON string
 */
fn convert_vec_to_ordered_json_string(vec: Vec<(String, String)>) -> String {
    let mut json_string = String::from("{");
    let mut iter = vec.iter();

    if let Some((key, value)) = iter.next() {
        json_string.push_str(&format!(r#""{}":"{}""#, key, value));
    }

    for (key, value) in iter {
        json_string.push_str(&format!(r#", "{}": "{}""#, key, value));
    }

    json_string.push('}');
    json_string
}

async fn test_actions(data_source_id: Path<String>, context: Data<AppContext>) -> impl Responder {
    let reader = context
        .datasource_provider
        .fetch_csv_reader(data_source_id.to_string())
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
async fn actions(data_source_id: Path<String>, context: Data<AppContext>) -> impl Responder {
    let reader = match context.datasource_provider.fetch_csv_reader(data_source_id.to_string()).await {
        Ok(r) => r,
        Err(_) => return HttpResponse::NotFound().body("Failed to get actions reader"),
    };

    let actions_iterator = process_csv(reader, 10);
    let actions_plot_data: ActionsPlotData = actions::transformers::to_plotly_data(context.plotly_config, actions_iterator);

    match to_string(&actions_plot_data) {
        Ok(json) => HttpResponse::Ok()
            .content_type("application/json")
            .body(json),
        Err(_) => HttpResponse::InternalServerError().body("Failed to serialize result"),
    }
}
async fn cognitive_load(path: Path<(String, String)>, context: Data<AppContext>) -> impl Responder {
    let mut file_reader = match get_json_file_reader(PlotType::CognitiveLoad, path, &context.datasource_provider).await{
        Ok(r) => r,
        Err(e) => return HttpResponse::NotFound().json(json!({"error": "Failed to get cognitive load data", "details": e})),
    };
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
async fn get_json_file_reader(plot_type: PlotType, path: Path<(String, String)>, datasource_provider: &Arc<dyn DataSource>) -> Result<Box<dyn Read + Send + Sync>, String> {
    let (data_source_id, id) = path.into_inner();
    let json_file_id = match datasource_provider.data_source_type() {
        DataSourceType::LocalFile => format!("{}/{}/{}",data_source_id,plot_type.as_str(),id),
        DataSourceType::GoogleDrive => id,
    };
    
    datasource_provider
        .fetch_json_reader(json_file_id)
        .await
        .map_err(|e| e.to_string())
}

async fn visual_attention(path: Path<(String, String)>, context: Data<AppContext>) -> impl Responder{
    let mut file_reader = match get_json_file_reader(PlotType::VisualAttention, path, &context.datasource_provider).await{
        Ok(r) => r,
        Err(e) => return HttpResponse::NotFound().json(json!({"error": "Failed to get visual attention data", "details": e})),
    };
    let window_duration_secs = context.plotly_config.visual_attention_plot_settings.window_size_secs;

    let visual_attention_plot_data = visual_attention::transformers::to_plotly_data(&mut file_reader, window_duration_secs, &context.plotly_config);

    match to_string(&visual_attention_plot_data) {
        Ok(json) => HttpResponse::Ok()
            .content_type("application/json")
            .body(json),
        Err(_) => HttpResponse::InternalServerError().body("Failed to serialize result"),
    }
}

const CREDENTIALS_FILE_HOME: &str =
    "/Users/gunalmel/Downloads/mteam-dashboard-447216-9836ce4f74a2.json";

#[actix_web::main]
async fn main() -> io::Result<()> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));
    
    let config = AppConfig::new("config.json")?;
    let plotly_config = config.get_plotly_config();
    let datasource_provider = config.get_data_provider().await;
    let context = Data::new(AppContext {
        datasource_provider: datasource_provider.clone(),
        plotly_config
    });
    HttpServer::new(move || {
        App::new()
            .service(
                fs::Files::new("/", &config.static_files_path)
                    .index_file("index.html")
                    .guard(guard::fn_guard(|ctx| !ctx.head().uri.path().starts_with("/api")))
            )
            .service(web::scope("/api")
            .wrap(middleware::Compress::default())
            .app_data(context.clone()) //To achieve globally shared state, it must be created outside of the closure passed to HttpServer::new and moved/cloned in. 
            .route("/data-sources/{data_source_id}/video", web::get().to(stream_video_handler))
            .route("/data-sources", web::get().to(data_sources))
            .route("/data-sources/{data_source_id}/actions", web::get().to(actions))
            .route("/data-sources/{data_source_id}/actions/raw/", web::get().to(test_actions))
            .route("/data-sources/{data_source_id}/{plot_name}", web::get().to(plot_sources))
            .route("/data-sources/{data_source_id}/cognitive-load/{id}", web::get().to(cognitive_load))
            .route("/data-sources/{data_source_id}/visual-attention/{id}", web::get().to(visual_attention))
                
        )})
        .bind(("0.0.0.0", config.port))?
        .run()
        .await
}