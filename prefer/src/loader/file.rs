//! File-based configuration loader.
//!
//! Handles bare config names (e.g., "myapp") and `file://` URLs by searching
//! standard system paths and trying supported extensions.

use crate::config::Config;
use crate::discovery;
use crate::error::Result;
use crate::loader::{LoadResult, Loader};
use crate::registry::RegisteredLoader;
use crate::watch as watch_mod;
use async_trait::async_trait;
use std::path::PathBuf;
use tokio::sync::mpsc;

inventory::submit! { RegisteredLoader(&FileLoader) }

/// Loader for file-based configuration sources.
///
/// Handles:
/// - Bare names like `"myapp"` — searches standard system paths with all
///   supported extensions
/// - Explicit paths like `"./config.toml"` or `"/etc/myapp.toml"`
/// - `file://` URLs
///
/// File discovery and extension search logic is delegated to the existing
/// `discovery` module.
pub struct FileLoader;

impl FileLoader {
    pub fn new() -> Self {
        Self
    }

    async fn locate(&self, identifier: &str) -> Result<PathBuf> {
        let stripped = identifier.strip_prefix("file://").unwrap_or(identifier);
        discovery::find_config_file(stripped).await
    }
}

impl Default for FileLoader {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Loader for FileLoader {
    fn provides(&self, identifier: &str) -> bool {
        if identifier.starts_with("file://") {
            return true;
        }

        // If it has any other scheme, it's not ours
        if identifier.contains("://") {
            return false;
        }

        // Bare names and relative/absolute paths are file-based
        true
    }

    async fn load(&self, identifier: &str) -> Result<LoadResult> {
        let path = self.locate(identifier).await?;
        let content = tokio::fs::read_to_string(&path).await?;

        Ok(LoadResult {
            source: path.to_string_lossy().to_string(),
            content,
            format_hint: None,
        })
    }

    fn name(&self) -> &str {
        "file"
    }

    async fn watch(&self, identifier: &str) -> Result<Option<mpsc::Receiver<Config>>> {
        let path = self.locate(identifier).await?;
        let rx = watch_mod::watch_path(path).await?;
        Ok(Some(rx))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_provides_bare_name() {
        let loader = FileLoader::new();
        assert!(loader.provides("myapp"));
        assert!(loader.provides("settings"));
    }

    #[test]
    fn test_provides_file_url() {
        let loader = FileLoader::new();
        assert!(loader.provides("file:///etc/myapp.toml"));
        assert!(loader.provides("file://config.json"));
    }

    #[test]
    fn test_provides_rejects_other_schemes() {
        let loader = FileLoader::new();
        assert!(!loader.provides("postgres://localhost/db"));
        assert!(!loader.provides("sqlite:///path/to/db"));
        assert!(!loader.provides("http://example.com/config"));
    }

    #[test]
    fn test_provides_explicit_paths() {
        let loader = FileLoader::new();
        assert!(loader.provides("./config.toml"));
        assert!(loader.provides("../config.toml"));
        assert!(loader.provides("/etc/myapp.toml"));
    }

    #[tokio::test]
    #[serial]
    async fn test_load_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("testapp.json");
        let mut file = std::fs::File::create(&file_path).unwrap();
        writeln!(file, r#"{{"host": "localhost", "port": 8080}}"#).unwrap();

        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        let loader = FileLoader::new();
        let result = loader.load("testapp").await.unwrap();

        assert!(result.source.ends_with("testapp.json"));
        assert!(result.content.contains("localhost"));
        assert!(result.format_hint.is_none());

        std::env::set_current_dir(original_dir).unwrap();
    }
}
