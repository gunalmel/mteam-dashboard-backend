use chrono::{NaiveDate, NaiveDateTime};
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