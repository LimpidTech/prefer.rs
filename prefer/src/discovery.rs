//! Configuration file discovery across standard system paths.

use crate::error::{Error, Result};
use std::path::{Path, PathBuf};
use tokio::fs;

/// Supported configuration file extensions.
const EXTENSIONS: &[&str] = &[
    "json", "json5", "jsonc", "yaml", "yml", "toml", "ini", "xml",
];

/// Check if a path has a supported configuration file extension.
fn has_supported_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| EXTENSIONS.contains(&ext))
}

/// Get standard configuration search paths for the current platform.
pub fn get_search_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    // Current directory
    if let Ok(cwd) = std::env::current_dir() {
        paths.push(cwd);
    }

    #[cfg(target_family = "unix")]
    {
        // XDG_CONFIG_HOME or ~/.config
        if let Some(config_home) = std::env::var_os("XDG_CONFIG_HOME") {
            paths.push(PathBuf::from(config_home));
        } else if let Some(home) = dirs::home_dir() {
            paths.push(home.join(".config"));
        }

        // XDG_CONFIG_DIRS
        if let Some(config_dirs) = std::env::var_os("XDG_CONFIG_DIRS") {
            for dir in std::env::split_paths(&config_dirs) {
                paths.push(dir);
            }
        }

        // Home directory
        if let Some(home) = dirs::home_dir() {
            paths.push(home);
        }

        // System paths
        paths.push(PathBuf::from("/usr/local/etc"));
        paths.push(PathBuf::from("/usr/etc"));
        paths.push(PathBuf::from("/etc"));
    }

    #[cfg(target_family = "windows")]
    {
        // User profile directory
        if let Some(profile) = dirs::home_dir() {
            paths.push(profile);
        }

        // APPDATA
        if let Some(appdata) = dirs::config_dir() {
            paths.push(appdata);
        }

        // ProgramData
        if let Some(program_data) = std::env::var_os("ProgramData") {
            paths.push(PathBuf::from(program_data));
        }

        // SystemRoot
        if let Some(system_root) = std::env::var_os("SystemRoot") {
            paths.push(PathBuf::from(system_root));
        }
    }

    paths
}

/// Find a configuration file by name in standard search paths.
///
/// If the name already has a supported extension and exists, it will be used directly.
/// Otherwise, searches for files with supported extensions appended.
///
/// Returns the path to the first matching configuration file found.
pub async fn find_config_file(name: &str) -> Result<PathBuf> {
    let search_paths = get_search_paths();

    for base_path in &search_paths {
        // First, check if the name as-is exists and has a supported extension
        let exact_path = base_path.join(name);
        if has_supported_extension(&exact_path) && fs::metadata(&exact_path).await.is_ok() {
            return Ok(exact_path);
        }

        // Then try appending extensions
        for ext in EXTENSIONS {
            let file_path = base_path.join(format!("{}.{}", name, ext));
            if fs::metadata(&file_path).await.is_ok() {
                return Ok(file_path);
            }
        }
    }

    // Also check if it's an absolute or explicitly relative path
    let path = Path::new(name);
    let is_explicit_path = path.is_absolute() || name.starts_with("./") || name.starts_with("../");
    if is_explicit_path && has_supported_extension(path) && fs::metadata(path).await.is_ok() {
        return Ok(path.to_path_buf());
    }

    Err(Error::FileNotFound(name.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    fn test_get_search_paths() {
        let paths = get_search_paths();
        assert!(!paths.is_empty());

        // Current directory should always be first
        if let Ok(cwd) = std::env::current_dir() {
            assert_eq!(paths[0], cwd);
        }
    }

    #[cfg(target_family = "unix")]
    #[test]
    fn test_unix_paths_included() {
        let paths = get_search_paths();
        let path_strings: Vec<String> = paths.iter().map(|p| p.display().to_string()).collect();

        assert!(path_strings.iter().any(|p| p.contains("/etc")));
    }

    #[cfg(target_family = "windows")]
    #[test]
    fn test_windows_paths_included() {
        let paths = get_search_paths();

        // Should include user profile or appdata
        assert!(paths.iter().any(|p| {
            let s = p.display().to_string();
            s.contains("Users") || s.contains("AppData")
        }));
    }

    #[test]
    fn test_has_supported_extension() {
        assert!(has_supported_extension(Path::new("config.toml")));
        assert!(has_supported_extension(Path::new("config.json")));
        assert!(has_supported_extension(Path::new("config.yaml")));
        assert!(has_supported_extension(Path::new("config.yml")));
        assert!(has_supported_extension(Path::new("/path/to/config.toml")));
        assert!(!has_supported_extension(Path::new("config")));
        assert!(!has_supported_extension(Path::new("config.txt")));
        assert!(!has_supported_extension(Path::new("config.rs")));
    }

    #[tokio::test]
    #[serial]
    async fn test_find_exact_file_with_extension() {
        use std::io::Write;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("myconfig.toml");
        let mut file = std::fs::File::create(&file_path).unwrap();
        writeln!(file, "key = \"value\"").unwrap();

        // Change to temp dir so it's in search path
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        // Should find "myconfig.toml" directly without appending extensions
        let result = find_config_file("myconfig.toml").await;
        assert!(result.is_ok());
        let found_path = result.unwrap();
        assert!(found_path.ends_with("myconfig.toml"));

        std::env::set_current_dir(original_dir).unwrap();
    }

    #[tokio::test]
    #[serial]
    async fn test_find_file_appends_extension() {
        use std::io::Write;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("myconfig.json");
        let mut file = std::fs::File::create(&file_path).unwrap();
        writeln!(file, r#"{{"key": "value"}}"#).unwrap();

        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        // Should find "myconfig" by appending .json
        let result = find_config_file("myconfig").await;
        assert!(result.is_ok());
        let found_path = result.unwrap();
        assert!(found_path.ends_with("myconfig.json"));

        std::env::set_current_dir(original_dir).unwrap();
    }
}
