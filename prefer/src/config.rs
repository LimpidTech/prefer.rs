//! Core configuration types and methods.

use crate::discovery;
use crate::error::{Error, Result};
use crate::formats;
use crate::value::{ConfigValue, FromValue};
use crate::visitor::{visit, ValueVisitor};
use std::path::PathBuf;

/// The main configuration struct that holds parsed configuration data.
#[derive(Debug, Clone)]
pub struct Config {
    /// The underlying configuration data.
    data: ConfigValue,
    /// The path to the file this configuration was loaded from.
    source_path: Option<PathBuf>,
}

impl Config {
    /// Create a new Config from a ConfigValue.
    pub fn new(data: ConfigValue) -> Self {
        Self {
            data,
            source_path: None,
        }
    }

    /// Create a new ConfigBuilder for constructing a Config from multiple sources.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use prefer::Config;
    ///
    /// #[tokio::main]
    /// async fn main() -> prefer::Result<()> {
    ///     let config = Config::builder()
    ///         .add_file("config/default.toml")
    ///         .add_env("MYAPP")
    ///         .build()
    ///         .await?;
    ///     Ok(())
    /// }
    /// ```
    pub fn builder() -> crate::builder::ConfigBuilder {
        crate::builder::ConfigBuilder::new()
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
    /// # fn example(config: &Config) -> prefer::Result<()> {
    /// let username: String = config.get("auth.username")?;
    /// let port: u16 = config.get("server.port")?;
    /// let enabled: bool = config.get("features.logging.enabled")?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn get<T: FromValue>(&self, key: &str) -> Result<T> {
        let value = self.get_value(key)?;
        T::from_value(value).map_err(|e| e.with_key(key))
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

    /// Extract a value by key using the `FromValue` trait.
    ///
    /// This is an alias for `get()` for backwards compatibility.
    pub fn extract<T: FromValue>(&self, key: &str) -> Result<T> {
        self.get(key)
    }

    /// Visit a value at the given key with a custom visitor.
    ///
    /// This method allows for complex custom deserialization logic using
    /// the visitor pattern.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use prefer::{Config, ConfigValue, ValueVisitor, Result};
    /// # use prefer::visitor::MapAccess;
    /// struct PortVisitor;
    ///
    /// impl ValueVisitor for PortVisitor {
    ///     type Output = u16;
    ///
    ///     fn visit_i64(&mut self, v: i64) -> Result<Self::Output> {
    ///         u16::try_from(v).map_err(|_| prefer::Error::ConversionError {
    ///             key: String::new(),
    ///             type_name: "u16".into(),
    ///             source: "port out of range".into(),
    ///         })
    ///     }
    ///
    ///     fn expecting(&self) -> &'static str {
    ///         "a port number"
    ///     }
    /// }
    ///
    /// # fn example(config: &Config) -> Result<()> {
    /// let port = config.visit_key("server.port", &mut PortVisitor)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn visit_key<V: ValueVisitor>(&self, key: &str, visitor: &mut V) -> Result<V::Output> {
        let value = self.get_value(key)?;
        visit(value, visitor).map_err(|e| e.with_key(key))
    }

    /// Visit the entire configuration data with a custom visitor.
    ///
    /// This is useful for transforming the entire configuration into a
    /// custom type.
    pub fn visit<V: ValueVisitor>(&self, visitor: &mut V) -> Result<V::Output> {
        visit(&self.data, visitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn obj(items: Vec<(&str, ConfigValue)>) -> ConfigValue {
        ConfigValue::Object(items.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
    }

    #[test]
    fn test_get_simple_value() {
        let config = Config::new(obj(vec![
            ("name", ConfigValue::String("test".to_string())),
            ("port", ConfigValue::Integer(8080)),
        ]));

        let name: String = config.get("name").unwrap();
        assert_eq!(name, "test");

        let port: u16 = config.get("port").unwrap();
        assert_eq!(port, 8080);
    }

    #[test]
    fn test_get_nested_value() {
        let config = Config::new(obj(vec![(
            "auth",
            obj(vec![
                ("username", ConfigValue::String("admin".to_string())),
                ("password", ConfigValue::String("secret".to_string())),
            ]),
        )]));

        let username: String = config.get("auth.username").unwrap();
        assert_eq!(username, "admin");
    }

    #[test]
    fn test_get_deeply_nested_value() {
        let config = Config::new(obj(vec![(
            "server",
            obj(vec![(
                "database",
                obj(vec![(
                    "connection",
                    obj(vec![("host", ConfigValue::String("localhost".to_string()))]),
                )]),
            )]),
        )]));

        let host: String = config.get("server.database.connection.host").unwrap();
        assert_eq!(host, "localhost");
    }

    #[test]
    fn test_key_not_found() {
        let config = Config::new(obj(vec![("name", ConfigValue::String("test".to_string()))]));

        let result: Result<String> = config.get("nonexistent");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Error::KeyNotFound(_)));
    }

    #[test]
    fn test_has_key() {
        let config = Config::new(obj(vec![(
            "auth",
            obj(vec![("username", ConfigValue::String("admin".to_string()))]),
        )]));

        assert!(config.has_key("auth.username"));
        assert!(!config.has_key("auth.password"));
        assert!(!config.has_key("nonexistent"));
    }
}
