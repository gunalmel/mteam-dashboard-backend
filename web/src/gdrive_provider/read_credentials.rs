use std::path::Path;
use std::{env, fs};

fn read_credentials_from_file(source: &str) -> Result<String, String> {
    let secret_path = Path::new(source);

    let secret_json = match fs::read_to_string(secret_path) {
        Err(e) => return Err(format!("Failed to read service account key file: {:#?}", e)),
        Ok(secret_json) => secret_json,
    };

    Ok(secret_json)
}

fn read_credentials_from_env(variable_name: &str) -> Result<String, String> {
    match env::var(variable_name) {
        Ok(val) => Ok(val),
        Err(e) => Err(format!(
            "Environment variable {} is not set to point at gdrive credentials: {}",
            variable_name, e
        )),
    }
}

pub(crate) fn read_credentials(source: &str) -> Result<String, String> {
    if Path::new(source).exists() {
        read_credentials_from_file(source)
    } else {
        match read_credentials_from_env(source) {
            Ok(credentials) => Ok(credentials),
            Err(e) => Err(format!(
            "File not found for gdrive credentials: '{}'. {}",
            source, e
        )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    const FILE_CONTENT: &str = r#"{
    "type": "service_account",
    "project_id": "mteam-dashboard-447216",
    "private_key_id": "9836",
    "private_key": "-----BEGIN PRIVATE KEY-----\nMIIEvAIBADANBgkqhkiG9w0BAQEFAASCBKY\n-----END PRIVATE KEY-----\n",
    "client_email": "mteam-dashboard-web@mteam-dashboard-447216.iam.gserviceaccount.com",
    "client_id": "10384",
    "auth_uri": "https://accounts.google.com/o/oauth2/auth",
    "token_uri": "https://oauth2.googleapis.com/token",
    "auth_provider_x509_cert_url": "https://www.googleapis.com/oauth2/v1/certs",
    "client_x509_cert_url": "https://www.googleapis.com/robot/v1/metadata/x509/mteam-dashboard-web%40mteam-dashboard-447216.iam.gserviceaccount.com",
    "universe_domain": "googleapis.com"
    }"#;

    #[test]
    fn test_read_credentials_from_file_success() {
        // Use a mock file path and write the content directly to a string.
        let mut mock_file = std::io::Cursor::new(Vec::new());
        mock_file.write_all(FILE_CONTENT.as_bytes()).unwrap();
        mock_file.set_position(0); // Reset the cursor to the beginning.
                                   // Create a mock path that will be used.
        let mock_path = "mock_file.json";
        // Create a mock file in memory
        fs::write(mock_path, FILE_CONTENT).unwrap();

        let result = read_credentials_from_file(mock_path);

        fs::remove_file(mock_path).unwrap();
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), FILE_CONTENT);
    }

    #[test]
    fn test_read_credentials_from_file_failure() {
        let result = read_credentials_from_file("non_existent_file.json");

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .starts_with("Failed to read service account key file:"));
    }

    #[test]
    fn test_read_credentials_from_env_success() {
        env::set_var("TEST_CREDENTIALS", FILE_CONTENT);

        let result = read_credentials_from_env("TEST_CREDENTIALS");

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), FILE_CONTENT);
        env::remove_var("TEST_CREDENTIALS");
    }

    #[test]
    fn test_read_credentials_from_env_failure() {
        let result = read_credentials_from_env("NON_EXISTENT_ENV_VAR");

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .starts_with("Environment variable NON_EXISTENT_ENV_VAR not found or invalid:"));
    }

    #[test]
    fn test_read_credentials_file_success() {
        let mock_path = "mock_file.json";
        fs::write(mock_path, FILE_CONTENT).unwrap();

        let result = read_credentials(mock_path);

        fs::remove_file(mock_path).unwrap();
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), FILE_CONTENT);
    }

    #[test]
    fn test_read_credentials_env_success() {
        env::set_var("TEST_CREDENTIALS", FILE_CONTENT);

        let result = read_credentials("TEST_CREDENTIALS");

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), FILE_CONTENT);
        env::remove_var("TEST_CREDENTIALS");
    }

    #[test]
    fn test_read_credentials_both_fail() {
        let source = "non_existent_source";

        let result = read_credentials(source);

        assert!(result.is_err());
        assert!(result.unwrap_err().contains(&format!(
            "Neither a file at path '{}' nor an environment variable with that name was found",
            source
        )));
    }

    #[test]
    fn test_read_credentials_file_read_fail() {
        let source = "/path/that/does/not/exist";

        let result = read_credentials(source);

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Neither a file at path '/path/that/does/not/exist' nor an environment variable with that name was found: Environment variable /path/that/does/not/exist not found or invalid: environment variable not found"));
    }
}
