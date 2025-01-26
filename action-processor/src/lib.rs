mod input_sources;
mod parsing;
mod detection;
mod csv_reader;
mod csv_row_processor;
mod processing_state;
mod plot_processors;
mod action_csv_row;
mod utils;
pub mod debug_message;
pub mod plot_structures;
pub(crate) mod csv_processor;
pub use csv_processor::process_csv;
use crate::plot_structures::ActionPlotPoint;

pub fn process(src: &str) -> Box<dyn Iterator<Item = Result<ActionPlotPoint, String>>>{
    process_csv(input_sources::create_reader(src).unwrap(), 5)
}