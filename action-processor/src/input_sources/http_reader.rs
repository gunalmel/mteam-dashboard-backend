use reqwest::blocking::Client;
use std::io::{BufReader, Cursor};

#[derive(Debug)]
pub(crate) enum HttpReaderError {
    RequestError(reqwest::Error),
    HttpStatusError(reqwest::StatusCode)
}

impl std::fmt::Display for HttpReaderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HttpReaderError::RequestError(e) => write!(f, "Request error: {}", e),
            HttpReaderError::HttpStatusError(status) => write!(f, "HTTP status error: {}", status),
        }
    }
}

impl std::error::Error for HttpReaderError {}

pub(crate) fn create_http_reader(url: &str) -> Result<BufReader<Box<dyn std::io::Read + Send + Sync>>, HttpReaderError> {
    let client = Client::new();
    let response = client.get(url).send().map_err(HttpReaderError::RequestError)?;

    if !response.status().is_success() {
        return Err(HttpReaderError::HttpStatusError(response.status()));
    }

    let body = response.bytes().map_err(HttpReaderError::RequestError)?;
    let reader: Box<dyn std::io::Read + Send + Sync> = Box::new(Cursor::new(body));
    let buf_reader = BufReader::new(reader);

    Ok(buf_reader)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::Server;
    #[test]
    fn test_create_http_reader_success() -> Result<(), Box<dyn std::error::Error>> {
        let mut server = Server::new(); // Start a mock server

        let body = "col1,col2\nval1,val2\n".to_owned();

        let mock = server.mock("GET", "/data.csv")
            .with_status(200)
            .with_body(body.clone())
            .create();

        let url = format!("{}{}", server.url(), "/data.csv"); // Correct way to build the URL

        let reader = create_http_reader(&url)?;

        let mut csv_reader = csv::ReaderBuilder::new().has_headers(true).from_reader(reader);

        let records: Vec<csv::Result<csv::StringRecord>> = csv_reader.records().collect();
        assert_eq!(records.len(), 1);
        let record = records[0].as_ref().unwrap();

        assert_eq!(record.get(0).unwrap(), "val1");
        assert_eq!(record.get(1).unwrap(), "val2");
        mock.assert();
        Ok(())
    }

    #[test]
    fn test_create_http_reader_failure() -> Result<(), Box<dyn std::error::Error>> {
        let mut server = Server::new(); // Start a mock server
        let mock = server.mock("GET", "/notfound")
            .with_status(404)
            .create();
        let url = format!("{}{}", server.url(), "/notfound");
        let result = create_http_reader(&url);
        assert!(result.is_err());
        mock.assert();
        Ok(())
    }
}