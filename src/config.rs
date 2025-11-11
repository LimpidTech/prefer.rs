//! Core configuration types and methods.

use crate::discovery;
use crate::error::{Error, Result};
use crate::formats;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::path::PathBuf;

/// A configuration value that can be any JSON-compatible type.
pub type ConfigValue = JsonValue;

/// The main configuration struct that holds parsed configuration data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// The underlying configuration data stored as JSON values.
    data: ConfigValue,
    /// The path to the file this configuration was loaded from.
    #[serde(skip)]
    source_path: Option<PathBuf>,
}

impl Config {
    /// Create a new Config from a JSON value.
    pub fn new(data: ConfigValue) -> Self {
        Self {
            data,
            source_path: None,
        }
    }

    /// Create a new Config with a source path.
    pub fn with_source(data: ConfigValue, path: PathBuf) -> Self {
        Self {
            data,
            source_path: Some(path),
        }
    }

    /// Load a configuration file by name.
    ///
    /// Searches standard system paths for a configuration file matching
    /// the given name with any supported extension.
    pub async fn load(name: &str) -> Result<Self> {
        let path = discovery::find_config_file(name).await?;
        Self::load_from_path(&path).await
    }

    /// Load a configuration from a specific file path.
    pub async fn load_from_path(path: &PathBuf) -> Result<Self> {
        let contents = tokio::fs::read_to_string(path).await?;
        let data = formats::parse(&contents, path)?;

        Ok(Self::with_source(data, path.clone()))
    }

    /// Get the source path of this configuration, if available.
    pub fn source_path(&self) -> Option<&PathBuf> {
        self.source_path.as_ref()
    }

    /// Get a configuration value by key using dot notation.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use prefer::Config;
    /// # async fn example(config: &Config) -> anyhow::Result<()> {
    /// let username: String = config.get("auth.username").await?;
    /// let port: u16 = config.get("server.port").await?;
    /// let enabled: bool = config.get("features.logging.enabled").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get<T>(&self, key: &str) -> Result<T>
    where
        T: for<'de> Deserialize<'de>,
    {
        let value = self.get_value(key)?;

        serde_json::from_value(value.clone()).map_err(|e| Error::ConversionError {
            key: key.to_string(),
            type_name: std::any::type_name::<T>().to_string(),
            source: Box::new(e),
        })
    }

    /// Get a raw configuration value by key using dot notation.
    ///
    /// Returns a reference to the `ConfigValue` at the specified key path.
    pub fn get_value(&self, key: &str) -> Result<&ConfigValue> {
        let parts: Vec<&str> = key.split('.').collect();
        let mut current = &self.data;

        for part in parts {
            match current {
                ConfigValue::Object(map) => {
                    current = map
                        .get(part)
                        .ok_or_else(|| Error::KeyNotFound(key.to_string()))?;
                }
                _ => return Err(Error::KeyNotFound(key.to_string())),
            }
        }

        Ok(current)
    }

    /// Get the entire configuration data as a reference.
    pub fn data(&self) -> &ConfigValue {
        &self.data
    }

    /// Get the entire configuration data as a mutable reference.
    pub fn data_mut(&mut self) -> &mut ConfigValue {
        &mut self.data
    }

    /// Check if a key exists in the configuration.
    pub fn has_key(&self, key: &str) -> bool {
        self.get_value(key).is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_get_simple_value() {
        let config = Config::new(json!({
            "name": "test",
            "port": 8080
        }));

        let name: String = config.get("name").await.unwrap();
        assert_eq!(name, "test");

        let port: u16 = config.get("port").await.unwrap();
        assert_eq!(port, 8080);
    }

    #[tokio::test]
    async fn test_get_nested_value() {
        let config = Config::new(json!({
            "auth": {
                "username": "admin",
                "password": "secret"
            }
        }));

        let username: String = config.get("auth.username").await.unwrap();
        assert_eq!(username, "admin");
    }

    #[tokio::test]
    async fn test_get_deeply_nested_value() {
        let config = Config::new(json!({
            "server": {
                "database": {
                    "connection": {
                        "host": "localhost"
                    }
                }
            }
        }));

        let host: String = config.get("server.database.connection.host").await.unwrap();
        assert_eq!(host, "localhost");
    }

    #[tokio::test]
    async fn test_key_not_found() {
        let config = Config::new(json!({
            "name": "test"
        }));

        let result: Result<String> = config.get("nonexistent").await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Error::KeyNotFound(_)));
    }

    #[test]
    fn test_has_key() {
        let config = Config::new(json!({
            "auth": {
                "username": "admin"
            }
        }));

        assert!(config.has_key("auth.username"));
        assert!(!config.has_key("auth.password"));
        assert!(!config.has_key("nonexistent"));
    }
}
