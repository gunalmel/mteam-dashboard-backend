mod file_reader;
mod http_reader;

use file_reader::create_file_reader;
use http_reader::create_http_reader;
use std::io::Read;
use url::Url;

pub fn create_reader(src: &str) -> Result<Box<dyn Read>, String> {
    // Check if the input is a valid URL
    if let Ok(url) = Url::parse(src) {
        if url.scheme() == "http" || url.scheme() == "https" {
            // Create an HTTP reader
            match create_http_reader(src) {
                Ok(reader) => Ok(Box::new(reader)),
                Err(err) => Err(format!("Error creating HTTP reader: {}", err)),
            }
        } else {
            // If the URL scheme is not HTTP or HTTPS, treat it as a file path
            create_file_reader(src)
                .map_err(|err| format!("Error creating file reader: {}", err))
                .map(|reader| Box::new(reader) as Box<dyn Read>)
        }
    } else {
        // If the input is not a valid URL, treat it as a file path
        create_file_reader(src)
            .map_err(|err| format!("Error creating file reader: {}", err))
            .map(|reader| Box::new(reader) as Box<dyn Read>)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;
    use std::fs::File;
    use std::io::Write;

    #[test]
    fn test_create_reader_file() -> Result<(), Box<dyn std::error::Error>> {
        // Create a temporary test file
        let test_content = "Hello, world!";
        let mut file = File::create("test.txt")?;
        write!(file, "{}", test_content)?;

        let mut reader = create_reader("test.txt")?;

        let mut buffer = String::new();
        reader.read_to_string(&mut buffer)?;
        assert_eq!(buffer, test_content);

        // Clean up the test file
        std::fs::remove_file("test.txt")?;
        Ok(())
    }

    #[test]
    fn test_create_reader_http() -> Result<(), Box<dyn std::error::Error>> {
        let mut reader = create_reader("https://example.com")?;

        let mut buffer = String::new();
        reader.read_to_string(&mut buffer)?;
        assert!(buffer.contains("Example Domain"));

        Ok(())
    }

    #[test]
    fn test_create_reader_invalid_url() {
        let result = create_reader("invalid_url");

        assert!(result.is_err());
    }

    #[test]
    fn test_create_reader_file_not_found() {
        let result = create_reader("non_existent_file.txt");

        assert!(result.is_err());
    }
}