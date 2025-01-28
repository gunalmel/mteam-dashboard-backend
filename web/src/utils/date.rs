use chrono::{Duration, NaiveDate, NaiveDateTime, NaiveTime, Utc};
use mteam_dashboard_action_processor::plot_structures::CsvRowTime;

pub(crate) fn seconds_to_csv_row_time(seconds: u32) -> CsvRowTime {
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
pub(crate) fn parse_date(date_str: &str) -> Result<NaiveDateTime, String> {
    if date_str.len() != 8 {
        return Err("Invalid date string length. Expected 8 digits (MMDDYYYY).".to_string());
    }

    let month_str = &date_str[0..2];
    let day_str = &date_str[2..4];
    let year_str = &date_str[4..8];

    let month: u32 = month_str.parse().map_err(|_| "Invalid month".to_string())?;
    let day: u32 = day_str.parse().map_err(|_| "Invalid day".to_string())?;
    let year: i32 = year_str.parse().map_err(|_| "Invalid year".to_string())?;

    NaiveDate::from_ymd_opt(year, month, day)
        .map(|date| date.and_hms_opt(0, 0, 0).unwrap())
        .ok_or_else(|| "Invalid date".to_string())
}

#[cfg(test)]
mod tests_seconds_to_csv_row_time {
    use super::*;

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

#[cfg(test)]
mod tests_parse_date {
    use super::*;
    use chrono::NaiveDateTime;

    #[test]
    fn test_valid_date() {
        assert_eq!(
            parse_date("09302024").unwrap(),
            NaiveDateTime::parse_from_str("2024-09-30 00:00:00", "%Y-%m-%d %H:%M:%S").unwrap()
        );
        assert_eq!(
            parse_date("12312023").unwrap(),
            NaiveDateTime::parse_from_str("2023-12-31 00:00:00", "%Y-%m-%d %H:%M:%S").unwrap()
        );
        assert_eq!(
            parse_date("01012000").unwrap(),
            NaiveDateTime::parse_from_str("2000-01-01 00:00:00", "%Y-%m-%d %H:%M:%S").unwrap()
        );
    }

    #[test]
    fn test_invalid_date_length() {
        assert_eq!(
            parse_date("12345").unwrap_err(),
            "Invalid date string length. Expected 8 digits (MMDDYYYY).".to_string()
        );
        assert_eq!(
            parse_date("123456789").unwrap_err(),
            "Invalid date string length. Expected 8 digits (MMDDYYYY).".to_string()
        );
        assert_eq!(
            parse_date("123456").unwrap_err(),
            "Invalid date string length. Expected 8 digits (MMDDYYYY).".to_string()
        );
    }

    #[test]
    fn test_invalid_date_values() {
        assert_eq!(parse_date("02302024").unwrap_err(), "Invalid date".to_string()); // Invalid day
        assert_eq!(parse_date("13012024").unwrap_err(), "Invalid date".to_string()); // Invalid month
        assert_eq!(parse_date("00000000").unwrap_err(), "Invalid date".to_string());
    }

    #[test]
    fn test_invalid_date_parse() {
        assert_eq!(parse_date("aa012024").unwrap_err(), "Invalid month".to_string());
        assert_eq!(parse_date("01aa2024").unwrap_err(), "Invalid day".to_string());
        assert_eq!(parse_date("0101aaaa").unwrap_err(), "Invalid year".to_string());
    }
}