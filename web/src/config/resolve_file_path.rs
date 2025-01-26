use std::env;
use std::error::Error;
use std::path::Path;

fn resolve_command_line_arg(args: &[String]) -> Result<String, Box<dyn Error>> {
    if let Some(arg) = args.iter().find(|arg| arg.starts_with("--config-file=")) {
        let path = arg.trim_start_matches("--config-file=");
        if !path.is_empty() {
            return resolve_path(path).map_err(|_| {
                std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "Invalid path set by \"--config-file\" argument",
                )
                    .into()
            });
        }
    }
    Err(Box::new(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        "No \"--config-file\" argument provided or path is empty",
    )))
}

fn resolve_environment_var() -> Result<String, Box<dyn Error>> {
    if let Ok(env_path) = env::var("MTEAM_DASHBOARD_BACKEND_CONFIG") {
        return resolve_path(&env_path).map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!(
                    "Invalid path set by MTEAM_DASHBOARD_BACKEND_CONFIG: {:#?}",
                    env_path
                ),
            )
                .into()
        });
    }
    Err(Box::new(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        "Environment variable MTEAM_DASHBOARD_BACKEND_CONFIG is not set",
    )))
}

pub fn resolve_path(path_string: &str) -> Result<String, Box<dyn Error>> {
    let path = Path::new(path_string);
    if path.is_absolute() {
        if !path.exists() {
            return Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Path does not exist {}", path_string)
            )));
        }
        return Ok(path_string.to_string());
    }
    // Get the current working directory
    let current_dir = env::current_dir()?;

    let resolved_path = current_dir.join(path_string);

    if !resolved_path.exists() {
        return Err(Box::new(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Path does not exist {}", path_string)
        )));
    }
    //to_string_lossy returns a Cow<str>, so we need to call into_owned to convert it to a String.
    Ok(resolved_path.to_string_lossy().into_owned())
}

pub fn resolve_first_path(paths: &[&str]) -> Result<String, Box<dyn Error>> {
    for &path in paths {
        if let Ok(resolved) = resolve_path(path) {
            return Ok(resolved);
        }
    }
    Err(Box::new(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        format!("No valid path found: {:#?}", paths)
    )))
}

pub fn resolve_config_file_path(cmd_args: &[String], fallback_paths: &[&str]) -> Result<String, Box<dyn Error>> {
    resolve_command_line_arg(cmd_args)
        .or_else(|_| resolve_environment_var())
        .or_else(|_| resolve_first_path(fallback_paths))
}

#[cfg(test)]
mod tests_resolve_path {
    use super::*;
    use std::env;
    use std::fs;

    #[test]
    fn relative_path() {
        // Create a temporary directory
        let temp_dir = tempfile::tempdir().unwrap();
        let temp_dir_path = temp_dir.path();

        // Set the current executable path to the temporary directory
        env::set_current_dir(&temp_dir_path).unwrap();

        // Create a dummy file in the temporary directory
        let dummy_file = temp_dir_path.join("dummy.txt");
        fs::write(&dummy_file, "test").unwrap();

        // Check if the file exists
        assert!(dummy_file.exists(), "Dummy file was not created");

        // Resolve the relative path
        let resolved_path = resolve_path("dummy.txt").expect("Failed to resolve relative path");

        // Log the paths
        println!("Resolved path: {:?}", resolved_path);
        println!("Dummy file path: {:?}", dummy_file);

        // Canonicalize paths to resolve symlinks
        // let resolved_path = resolved_path.canonicalize().expect("Failed to canonicalize resolved path");
        let dummy_file = dummy_file.canonicalize().expect("Failed to canonicalize dummy file").to_string_lossy().into_owned();

        // Check if the resolved path is correct
        assert_eq!(resolved_path, dummy_file);
    }

    #[test]
    fn path_non_existent() {
        // Create a temporary directory
        let temp_dir = tempfile::tempdir().unwrap();
        let temp_dir_path = temp_dir.path();

        // Set the current executable path to the temporary directory
        env::set_current_dir(&temp_dir_path).unwrap();

        // Resolve a non-existent relative path
        let resolved_path = resolve_path("non_existent.txt");

        // Check if the function returns an error
        assert!(resolved_path.is_err());
    }

    #[test]
    fn absolute_path() {
        // Create a temporary directory
        let temp_dir = tempfile::tempdir().unwrap();
        let temp_dir_path = temp_dir.path();

        // Create a dummy file in the temporary directory
        let dummy_file = temp_dir_path.join("dummy.txt");
        fs::write(&dummy_file, "test").unwrap();

        // Check if the file exists
        assert!(dummy_file.exists(), "Dummy file was not created");

        // Resolve the absolute path
        let resolved_path = resolve_path(dummy_file.to_str().unwrap()).expect("Failed to resolve absolute path");

        // Check if the resolved path is correct
        assert_eq!(resolved_path, dummy_file.to_str().unwrap());
    }
    #[test]
    fn absolute_path_non_existent() {
        // Create a temporary directory
        let temp_dir = tempfile::tempdir().unwrap();
        let temp_dir_path = temp_dir.path();

        // Create a non-existent file path in the temporary directory
        let non_existent_file = temp_dir_path.join("non_existent.txt");

        // Resolve the non-existent absolute path
        let resolved_path = resolve_path(non_existent_file.to_str().unwrap());

        // Check if the function returns an error
        assert!(resolved_path.is_err());
    }
}
#[cfg(test)]
mod tests_resolve_config_file_path {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn with_command_line_argument() {
        let temp_dir = tempdir().unwrap();
        let config_file = temp_dir.path().join("../../config.json").to_string_lossy().into_owned();
        fs::write(&config_file, "{}").unwrap();

        // Mock command-line arguments
        let args = vec![
            "app".to_string(),
            format!("--config-file={}", config_file),
        ];

        // No fallback paths provided
        let fallback_paths: [&str; 0] = [];
        let resolved_path = resolve_config_file_path(&args, &fallback_paths).unwrap();
        assert_eq!(resolved_path, config_file);
    }

    #[test]
    fn with_env_variable() {
        let temp_dir = tempdir().unwrap();
        let config_file = temp_dir.path().join("env_config.json");
        fs::write(&config_file, "{}").unwrap();

        // Mock environment variable
        env::set_var("MTEAM_DASHBOARD_BACKEND_CONFIG", config_file.to_string_lossy().into_owned());

        // Clear command-line arguments
        let args = vec!["app".to_string()];

        // No fallback paths provided
        let fallback_paths: [&str; 0] = [];
        let resolved_path = resolve_config_file_path(&args, &fallback_paths).unwrap();
        assert_eq!(resolved_path, config_file.to_string_lossy().into_owned());

        // Clean up environment variable
        env::remove_var("MTEAM_DASHBOARD_BACKEND_CONFIG");
    }

    #[test]
    fn with_multiple_paths() {
        let temp_dir = tempdir().unwrap();
        let config_file = temp_dir.path().join("fallback_config.json");
        fs::write(&config_file, "{}").unwrap();

        // Mock arguments and environment
        let args = vec!["app".to_string()];
        env::remove_var("MTEAM_DASHBOARD_BACKEND_CONFIG");

        // Provide fallback paths
        let fallback_paths = config_file.to_string_lossy().into_owned();
        let resolved_path = resolve_config_file_path(&args, &[&fallback_paths]).unwrap();
        assert_eq!(resolved_path, config_file.to_string_lossy().into_owned());
    }

    #[test]
    fn path_not_found() {
        // Mock arguments and environment
        let args = vec!["app".to_string()];
        env::remove_var("MTEAM_DASHBOARD_BACKEND_CONFIG"); 

        // Provide invalid fallback paths
        let fallback_paths = ["nonexistent_file.json", "another_nonexistent.json"];
        let result = resolve_config_file_path(&args, &fallback_paths);

        // Debug prints
        println!("Result: {:?}", result);

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "No valid path found: [\n    \"nonexistent_file.json\",\n    \"another_nonexistent.json\",\n]"
        );
    }
}