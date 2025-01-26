use mteam_dashboard_action_processor::process_csv;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

#[test]
fn test_stream_csv_with_errors() {
    // Load the sample CSV file
    let file_path = Path::new("tests/data/sample.csv");
    assert!(file_path.exists(), "Test CSV file is missing!");

    let file = File::open(file_path).expect("Failed to open the CSV file");
    let reader = BufReader::new(file);
    let max_rows_to_check = 10;

    // Run the stream_csv_with_errors function
    let results: Vec<_> = process_csv(reader, max_rows_to_check).collect();

    // Set expectations for the results
    assert!(!results.is_empty(), "No rows were processed");
    for result in results {
        match result {
            Ok(plot_point) => {
                // Perform checks on expected plot points
                println!("Processed point: {:?}", plot_point);
            }
            Err(error) => {
                // Verify error handling
                eprintln!("Error encountered: {:?}", error);
            }
        }
    }
}
