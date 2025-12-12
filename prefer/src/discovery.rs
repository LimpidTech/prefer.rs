//! Configuration file discovery across standard system paths.

use crate::error::{Error, Result};
use std::path::PathBuf;
use tokio::fs;

/// Supported configuration file extensions.
const EXTENSIONS: &[&str] = &[
    "json", "json5", "jsonc", "yaml", "yml", "toml", "ini", "xml",
];

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
/// Returns the path to the first matching configuration file found.
pub async fn find_config_file(name: &str) -> Result<PathBuf> {
    let search_paths = get_search_paths();

    for base_path in search_paths {
        for ext in EXTENSIONS {
            let file_path = base_path.join(format!("{}.{}", name, ext));

            if fs::metadata(&file_path).await.is_ok() {
                return Ok(file_path);
            }
        }
    }

    Err(Error::FileNotFound(name.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
