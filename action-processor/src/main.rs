use mteam_dashboard_action_processor::debug_message::print_debug_message;
use mteam_dashboard_action_processor::plot_structures::ActionPlotPoint;
use mteam_dashboard_action_processor::process as build_csv_reader;
use std::{env, io};

fn display_menu() -> String {
    println!("No argument provided (give file url or path on command line whem running). Please select an option:");
    println!("1. Enter file with path to parse");
    println!("2. Enter URL for processing the streaming CSV text");
    println!("3. Run with hard-coded timeline-multiplayer-09182024.csv file");
    println!("4. Run with hard-coded https://dl.dropboxusercontent.com/scl/fi/6os941r9qnk19nkd22415/timeline-multiplayer-09182024.csv?rlkey=4lpfpmkf62fnua597t7bh3p17&st=1v2zw6n3&dl=0");

    let mut input = String::new();
    io::stdin().read_line(&mut input).expect("Failed to read line");

    match input.trim() {
        "1" => {
            println!("Enter file path:");
            let mut file_path = String::new();
            io::stdin().read_line(&mut file_path).expect("Failed to read line");
            file_path
        }
        "2" => {
            println!("Enter URL:");
            let mut url = String::new();
            io::stdin().read_line(&mut url).expect("Failed to read line");
            url
        }
        "3" => {
            "timeline-multiplayer-09182024.csv".to_owned()
        }
        "4" => {
            "https://dl.dropboxusercontent.com/scl/fi/6os941r9qnk19nkd22415/timeline-multiplayer-09182024.csv?rlkey=4lpfpmkf62fnua597t7bh3p17&st=1v2zw6n3&dl=0".to_owned()
        }
        _ => { println!("Invalid option"); std::process::exit(1); }
    }
}
fn process_csv_input(csv_iterator: Box<dyn Iterator<Item=Result<ActionPlotPoint, String>>>) {
    for (row_idx, result) in csv_iterator.enumerate() {
        let _item_number = row_idx + 1;
        match result {
            // Ok(_)=> { print_debug_message!("{}", item_number); },
            Ok(ActionPlotPoint::Error(_error_point)) => {
                print_debug_message!("{} Error: {:#?}", _item_number, _error_point);
            }
            Ok(ActionPlotPoint::Action(_action_point)) => {
                print_debug_message!("{} Action: {:#?}", _item_number, _action_point);
            }
            // Ok(ActionPlotPoint::Period(PeriodType::Stage, start, end)) => { print_debug_message!("{} stage_boundary: {:#?}", item_number, (start,end)); },
            // Ok(ActionPlotPoint::MissedAction(missed_action)) => { print_debug_message!("{} missed_action: {:?}", item_number, missed_action); },
            // Ok(ActionPlotPoint::Period(PeriodType::CPR, start, end)) => { print_debug_message!("{} stage_boundary: {:#?}", item_number, (start,end)); },
            Err(_e) => { print_debug_message!("{} error: {}", _item_number, _e); }
            _ => {}
        }
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() == 2 {
        process_csv_input(build_csv_reader(&args[1].trim()));
    } else {
        let src = display_menu();
        process_csv_input(build_csv_reader(src.trim()));
    }
}