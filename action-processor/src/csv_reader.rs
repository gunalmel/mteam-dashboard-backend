use std::io::Read;
use csv::Reader;
use crate::action_csv_row::{COLUMN_NAMES};

pub fn initialize_csv_reader<R: Read>(reader: R) -> Result<Reader<R>, String> {
    let mut csv_reader = build_csv_reader(reader);
    validate_csv_header(&mut csv_reader).map_err(|e| format!("Header parsing errors: {:?}", e))?;
    Ok(csv_reader)
}

fn build_csv_reader<R: Read>(reader: R) -> Reader<R> {
    csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .from_reader(reader)
}

type HeaderValidatorType = fn(&[&str], &[&str]) -> Result<(), String>;

pub fn validate_header(headers: &[&str], expected_headers: &[&str]) -> Result<(), String> {
    let mut headers_iter = headers.iter().map(|h| h.to_lowercase());
    let mut expected_iter = expected_headers.iter().map(|h| h.to_lowercase());

    if expected_iter.all(|expected| headers_iter.next() == Some(expected)) {
        Ok(())
    } else {
        let err = format!(
            "Line {:?}: expected {:?} as the header row of csv but got {:?}",
            1, expected_headers, headers
        );
        Err(err)
    }
}

fn apply_validation<R: Read>(reader: &mut Reader<R>, validate: HeaderValidatorType) -> Result<(), String> {
    match reader.headers() {
        Ok(headers) => {
            let headers = headers.iter().collect::<Vec<_>>();
            validate(&headers, &COLUMN_NAMES)
        }
        Err(e) => Err(e.to_string())
    }
}

fn build_csv_header_validator<R: Read>(validate: HeaderValidatorType) -> impl Fn(Box<&mut Reader<R>>) -> Result<(), String> {
    move |mut reader| apply_validation(reader.as_mut(), validate)
}

pub fn validate_csv_header<R: Read>(reader: &mut Reader<R>) -> Result<(), String> {
    build_csv_header_validator(validate_header)(Box::new(reader)) 
}

#[cfg(test)]
mod tests {
    fn assert_header_check(headers: &[&str], actual: Result<(), String>, expected_headers: &[&str]) {
        assert!(actual.is_err());
        let message: String = actual.unwrap_err();
        assert_eq!(message, format!("Line {:?}: expected {:?} as the header row of csv but got {:?}", 1, expected_headers, headers));
    }

    mod invalid_header_tests {
        use super::assert_header_check;
        use super::super::validate_header;

        #[test]
        fn test_check_headers_missing_header() {
            let headers = ["Time Stamp[Hr:Min:Sec]", "Action/Vital Name"];
            let expected_headers = ["Time Stamp[Hr:Min:Sec]", "Action/Vital Name", "Score"];

            assert_header_check(
                &headers,
                validate_header(&headers, &expected_headers),
                &expected_headers,
            );
        }

        #[test]
        fn test_check_headers_different_order() {
            let headers = [
                "Action/Vital Name",
                "Time Stamp[Hr:Min:Sec]",
                "SubAction Time[Min:Sec]",
            ];
            let expected_headers = [
                "Time Stamp[Hr:Min:Sec]",
                "Action/Vital Name",
                "SubAction Time[Min:Sec]",
            ];

            assert_header_check(
                &headers,
                validate_header(&headers, &expected_headers),
                &expected_headers,
            );
        }

        #[test]
        fn test_check_headers_unknown_header() {
            let headers = [
                "Time Stamp[Hr:Min:Sec]",
                "Action/Vital Name",
                "Unknown Header",
                "SubAction Time[Min:Sec]",
            ];
            let expected_headers = [
                "Time Stamp[Hr:Min:Sec]",
                "Action/Vital Name",
                "SubAction Time[Min:Sec]",
            ];

            assert_header_check(
                &headers,
                validate_header(&headers, &expected_headers),
                &expected_headers,
            );
        }
    }

    mod valid_header_tests {
        use crate::csv_reader::validate_header;

        #[test]
        fn test_check_headers_matching() {
            let headers = [
                "Time Stamp[Hr:Min:Sec]",
                "Action/Vital Name",
                "SubAction Time[Min:Sec]",
            ];
            let expected_headers = [
                "Time Stamp[Hr:Min:Sec]",
                "Action/Vital Name",
                "SubAction Time[Min:Sec]",
            ];

            assert!(validate_header(&headers, &expected_headers).is_ok());
        }

        #[test]
        fn test_check_headers_matching_case_insensitive() {
            let headers = [
                "time Stamp[Hr:Min:Sec]",
                "ActioN/Vital Name",
                "subAction time[min:sec]",
            ];
            let expected_headers = [
                "Time Stamp[Hr:Min:Sec]",
                "Action/Vital Name",
                "SubAction Time[Min:Sec]",
            ];

            assert!(validate_header(&headers, &expected_headers).is_ok());
        }

        #[test]
        fn test_check_headers_matching_extra_header() {
            let headers = [
                "time Stamp[Hr:Min:Sec]",
                "ActioN/Vital Name",
                "subAction time[min:sec]",
                "Extra Column",
            ];
            let expected_headers = [
                "Time Stamp[Hr:Min:Sec]",
                "Action/Vital Name",
                "SubAction Time[Min:Sec]",
            ];

            assert!(validate_header(&headers, &expected_headers).is_ok());
        }
    }

    mod tests_apply_validation {
        use crate::csv_reader::apply_validation;
        use csv::Reader;
        use std::io::{self, Read};

        // Custom reader that always returns an error
        struct ValidReader;
        impl Read for ValidReader {
            fn read(&mut self, _buf: &mut [u8]) -> io::Result<usize> {
                Ok(0)
            }
        }
        struct ErrorReader;

        impl Read for ErrorReader {
            fn read(&mut self, _buf: &mut [u8]) -> io::Result<usize> {
                Err(io::Error::new(io::ErrorKind::Other, "Simulated read error"))
            }
        }

        #[test]
        fn test_could_not_read_headers() {
            let mut csv_reader = Reader::from_reader(ErrorReader);
            let mock_validate = |_: &[&str], _: &[&str]| -> Result<(), String> { unreachable!() };

            let result = apply_validation(&mut csv_reader, mock_validate);

            assert!(result.is_err());
            assert_eq!(result.unwrap_err(), "Simulated read error");
        }

        #[test]
        fn test_read_invalid_headers() {
            let mut csv_reader = Reader::from_reader(ValidReader);
            let mock_validate = |_: &[&str], _: &[&str]| -> Result<(), String> {
                Err("Validation error".to_owned())
            };

            let result = apply_validation(&mut csv_reader, mock_validate);

            assert!(result.is_err());
            assert_eq!(result.unwrap_err(), "Validation error");
        }

        #[test]
        fn test_read_valid_headers() {
            let mut csv_reader = Reader::from_reader(ValidReader);
            let mock_validate = |_: &[&str], _: &[&str]| -> Result<(), String> {
                Ok(())
            };

            let result = apply_validation(&mut csv_reader, mock_validate);

            assert!(result.is_ok());
        }
    }
}