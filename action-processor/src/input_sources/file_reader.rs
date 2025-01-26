use std::fs::File;
use std::io::{BufReader, Error as IoError};

#[derive(Debug)]
pub enum FileReaderError {
    IoError(IoError)
}

impl std::fmt::Display for FileReaderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FileReaderError::IoError(e) => write!(f, "IO error: {}", e)
        }
    }
}

impl std::error::Error for FileReaderError {}

pub fn create_file_reader(path: &str) -> Result<BufReader<File>, FileReaderError> {
    let file = File::open(path).map_err(FileReaderError::IoError)?;
    let buf_reader = BufReader::new(file);
    Ok(buf_reader)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};

    #[test]
    fn test_create_file_reader_success() -> Result<(), Box<dyn std::error::Error>> {
        // Create a temporary test file
        let test_csv_content = "col1,col2\nval1,val2\n";
        let mut file = File::create("test.csv")?;
        write!(file, "{}", test_csv_content)?; // Now this works

        // Call the function being tested
        let reader = create_file_reader("test.csv")?;

        // Use the reader with the csv crate
        let mut csv_reader = csv::ReaderBuilder::new().has_headers(true).from_reader(reader);

        let records: Vec<csv::Result<csv::StringRecord>> = csv_reader.records().collect();
       
        assert_eq!(records.len(), 1);
        let record = records[0].as_ref().unwrap();
        assert_eq!(record.get(0).unwrap(), "val1");
        assert_eq!(record.get(1).unwrap(), "val2");

        // Clean up the test file
        std::fs::remove_file("test.csv")?;
        Ok(())
    }

    #[test]
    fn test_create_file_reader_file_not_found() -> Result<(), Box<dyn std::error::Error>> {
        let result = create_file_reader("non_existent_file.csv");
        
        assert!(result.is_err());
        match result.unwrap_err() {
            FileReaderError::IoError(e) => {
                assert_eq!(e.kind(), std::io::ErrorKind::NotFound);
            }
        }
        Ok(())
    }

    #[test]
    fn test_create_file_reader_invalid_utf8() -> Result<(), Box<dyn std::error::Error>> {
        // Create a file with invalid UTF-8
        let invalid_utf8 = b"\xFF\xFE"; // Example invalid UTF-8 sequence
        let mut file = File::create("invalid.csv")?;
        file.write_all(invalid_utf8)?;

        let mut reader = create_file_reader("invalid.csv")?;

        // Try to read the file as UTF-8
        let mut text = String::new();
        let result = reader.read_to_string(&mut text);

        // Check that reading the file as UTF-8 fails
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::InvalidData);

        std::fs::remove_file("invalid.csv")?;
        Ok(())
    }
}