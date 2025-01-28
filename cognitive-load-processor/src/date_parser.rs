use chrono::{Duration, NaiveTime, Utc};
use mteam_dashboard_action_processor::plot_structures::CsvRowTime;

//TODO there are parsers in action-processor as well, those can be extracted to a separate util crate
pub fn seconds_to_csv_row_time(seconds: u32) -> CsvRowTime {
    let start_of_day = Utc::now().date_naive().and_hms_opt(0, 0, 0).unwrap_or_default();
    let timestamp = start_of_day + Duration::seconds(seconds as i64);
    let date_string = timestamp.format("%Y-%m-%d %H:%M:%S").to_string();

    // Calculate the time portion separately
    let time_part = NaiveTime::from_hms_opt(
        (seconds / 3600) % 24, // Hours (modulo 24 to handle overflow)
        (seconds / 60) % 60,   // Minutes
        seconds % 60,        // Seconds
    ).unwrap_or_default();
    let timestamp_string = time_part.format("%H:%M:%S").to_string();

    CsvRowTime {
        total_seconds: seconds,
        date_string,
        timestamp: timestamp_string,
    }
}

#[cfg(test)]
mod tests_seconds_to_csv_row_time {
    use chrono::{Duration, Utc};
    use crate::date_parser::seconds_to_csv_row_time;
    #[test]
    fn test() {
        let csv_row_time = seconds_to_csv_row_time(3661); // 1 hour, 1 minute, 1 second
        assert_eq!(csv_row_time.total_seconds, 3661);
        assert_eq!(csv_row_time.timestamp, "01:01:01");

        let start_of_day = Utc::now().date_naive().and_hms_opt(0, 0, 0).unwrap_or_default();
        let expected_date_string = (start_of_day + Duration::seconds(3661)).format("%Y-%m-%d %H:%M:%S").to_string();
        assert_eq!(csv_row_time.date_string, expected_date_string);
    }

    #[test]
    fn test_midnight() {
        let csv_row_time = seconds_to_csv_row_time(0); // Midnight
        assert_eq!(csv_row_time.total_seconds, 0);
        assert_eq!(csv_row_time.timestamp, "00:00:00");

        let start_of_day = Utc::now().date_naive().and_hms_opt(0, 0, 0).unwrap_or_default();
        let expected_date_string = start_of_day.format("%Y-%m-%d %H:%M:%S").to_string();
        assert_eq!(csv_row_time.date_string, expected_date_string);
    }

    #[test]
    fn test_end_of_day() {
        let csv_row_time = seconds_to_csv_row_time(86399); // 23 hours, 59 minutes, 59 seconds
        assert_eq!(csv_row_time.total_seconds, 86399);
        assert_eq!(csv_row_time.timestamp, "23:59:59");

        let start_of_day = Utc::now().date_naive().and_hms_opt(0, 0, 0).unwrap_or_default();
        let expected_date_string = (start_of_day + Duration::seconds(86399)).format("%Y-%m-%d %H:%M:%S").to_string();
        assert_eq!(csv_row_time.date_string, expected_date_string);
    }
}