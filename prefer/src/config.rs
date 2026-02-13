//! Core configuration types and methods.

use crate::discovery;
use crate::error::{Error, Result};
use crate::events::Emitter;
use crate::formats;
use crate::value::{ConfigValue, FromValue};
use crate::visitor::{visit, ValueVisitor};
use std::collections::HashMap;
use std::path::PathBuf;

/// The main configuration struct that holds parsed configuration data.
///
/// `Config` retains metadata about how it was loaded (source path, loader
/// name, formatter name) so it can support `save()` and `watch()` on an
/// existing instance. It also supports an event emitter for "changed"
/// events when values are set via `set()`.
pub struct Config {
    data: ConfigValue,
    source_path: Option<PathBuf>,
    source: Option<String>,
    loader_name: Option<String>,
    formatter_name: Option<String>,
    emitter: Option<Emitter>,
}

impl std::fmt::Debug for Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Config")
            .field("data", &self.data)
            .field("source_path", &self.source_path)
            .field("source", &self.source)
            .field("loader_name", &self.loader_name)
            .field("formatter_name", &self.formatter_name)
            .finish()
    }
}

impl Clone for Config {
    fn clone(&self) -> Self {
        Self {
            data: self.data.clone(),
            source_path: self.source_path.clone(),
            source: self.source.clone(),
            loader_name: self.loader_name.clone(),
            formatter_name: self.formatter_name.clone(),
            emitter: None,
        }
    }
}

impl Config {
    /// Create a new Config from a ConfigValue.
    pub fn new(data: ConfigValue) -> Self {
        Self {
            data,
            source_path: None,
            source: None,
            loader_name: None,
            formatter_name: None,
            emitter: None,
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
            source: None,
            loader_name: None,
            formatter_name: None,
            emitter: None,
        }
    }

    /// Create a Config with full metadata from the registry loading path.
    pub(crate) fn with_metadata(
        data: ConfigValue,
        source: String,
        loader_name: String,
        formatter_name: String,
    ) -> Self {
        let source_path = PathBuf::from(&source);
        let source_path = if source_path.exists() {
            Some(source_path)
        } else {
            None
        };

        Self {
            data,
            source_path,
            source: Some(source),
            loader_name: Some(loader_name),
            formatter_name: Some(formatter_name),
            emitter: None,
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

    /// Get the source identifier (e.g., file path or URL).
    pub fn source(&self) -> Option<&str> {
        self.source.as_deref()
    }

    /// Get the name of the loader that loaded this config.
    pub fn loader_name(&self) -> Option<&str> {
        self.loader_name.as_deref()
    }

    /// Get the name of the formatter used to parse this config.
    pub fn formatter_name(&self) -> Option<&str> {
        self.formatter_name.as_deref()
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

    /// Set a configuration value by key using dot notation.
    ///
    /// Creates intermediate objects as needed. Emits a "changed" event
    /// if an emitter is attached.
    pub fn set(&mut self, key: &str, value: ConfigValue) {
        let previous = self.get_value(key).ok().cloned();
        let parts: Vec<&str> = key.split('.').collect();
        set_nested(&mut self.data, &parts, value.clone());

        if let Some(emitter) = &self.emitter {
            emitter.emit("changed", key, &value, previous.as_ref());
        }
    }

    /// Register a handler for configuration change events.
    ///
    /// The handler is called whenever `set()` is used to modify a value.
    pub fn on_change(
        &mut self,
        handler: Box<dyn Fn(&str, &ConfigValue, Option<&ConfigValue>) + Send + Sync>,
    ) {
        let emitter = self.emitter.get_or_insert_with(Emitter::new);
        emitter.bind("changed", handler);
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

/// Set a value at a nested key path, creating intermediate objects as needed.
fn set_nested(current: &mut ConfigValue, parts: &[&str], value: ConfigValue) {
    debug_assert!(!parts.is_empty(), "key parts should never be empty");

    let key = parts[0];

    if parts.len() == 1 {
        if let ConfigValue::Object(map) = current {
            map.insert(key.to_string(), value);
        } else {
            let mut map = HashMap::new();
            map.insert(key.to_string(), value);
            *current = ConfigValue::Object(map);
        }
        return;
    }

    // Ensure current is an object and get/create the nested entry
    if !matches!(current, ConfigValue::Object(_)) {
        *current = ConfigValue::Object(HashMap::new());
    }

    if let ConfigValue::Object(map) = current {
        let entry = map
            .entry(key.to_string())
            .or_insert_with(|| ConfigValue::Object(HashMap::new()));
        set_nested(entry, &parts[1..], value);
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

    #[test]
    fn test_set_simple() {
        let mut config = Config::new(obj(vec![("port", ConfigValue::Integer(8080))]));
        config.set("port", ConfigValue::Integer(9090));
        let port: i64 = config.get("port").unwrap();
        assert_eq!(port, 9090);
    }

    #[test]
    fn test_set_nested_creates_intermediates() {
        let mut config = Config::new(ConfigValue::Object(HashMap::new()));
        config.set("server.database.host", ConfigValue::String("localhost".into()));

        let host: String = config.get("server.database.host").unwrap();
        assert_eq!(host, "localhost");
    }

    #[test]
    fn test_set_emits_changed_event() {
        let mut config = Config::new(obj(vec![("port", ConfigValue::Integer(8080))]));

        let log = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let log_clone = log.clone();
        config.on_change(Box::new(move |key, value, prev| {
            log_clone.lock().unwrap().push((
                key.to_string(),
                value.clone(),
                prev.cloned(),
            ));
        }));

        config.set("port", ConfigValue::Integer(9090));

        let entries = log.lock().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].0, "port");
        assert_eq!(entries[0].1, ConfigValue::Integer(9090));
        assert_eq!(entries[0].2, Some(ConfigValue::Integer(8080)));
    }

    #[test]
    fn test_metadata_accessors() {
        let config = Config::with_metadata(
            ConfigValue::Null,
            "/etc/myapp.toml".into(),
            "file".into(),
            "toml".into(),
        );

        assert_eq!(config.source(), Some("/etc/myapp.toml"));
        assert_eq!(config.loader_name(), Some("file"));
        assert_eq!(config.formatter_name(), Some("toml"));
    }

    #[test]
    fn test_clone_drops_emitter() {
        let mut config = Config::new(ConfigValue::Null);
        config.on_change(Box::new(|_, _, _| {}));
        assert!(config.emitter.is_some());

        let cloned = config.clone();
        assert!(cloned.emitter.is_none());
    }

    #[test]
    fn test_clone_preserves_metadata() {
        let config = Config::with_metadata(
            ConfigValue::Null,
            "/nonexistent/path.toml".into(),
            "file".into(),
            "toml".into(),
        );
        let cloned = config.clone();
        assert_eq!(cloned.source(), Some("/nonexistent/path.toml"));
        assert_eq!(cloned.loader_name(), Some("file"));
        assert_eq!(cloned.formatter_name(), Some("toml"));
    }

    #[test]
    fn test_debug_output() {
        let config = Config::with_metadata(
            ConfigValue::Integer(42),
            "test.json".into(),
            "file".into(),
            "json".into(),
        );
        let debug = format!("{:?}", config);
        assert!(debug.contains("Config"));
        assert!(debug.contains("loader_name"));
        assert!(debug.contains("formatter_name"));
        // emitter should NOT appear in debug output
        assert!(!debug.contains("emitter"));
    }

    #[test]
    fn test_with_metadata_nonexistent_path() {
        let config = Config::with_metadata(
            ConfigValue::Null,
            "/this/path/does/not/exist.toml".into(),
            "file".into(),
            "toml".into(),
        );
        // source_path should be None for non-existent paths
        assert!(config.source_path().is_none());
        // but source string is still set
        assert_eq!(config.source(), Some("/this/path/does/not/exist.toml"));
    }

    #[test]
    fn test_new_config_has_no_metadata() {
        let config = Config::new(ConfigValue::Null);
        assert!(config.source_path().is_none());
        assert!(config.source().is_none());
        assert!(config.loader_name().is_none());
        assert!(config.formatter_name().is_none());
    }

    #[test]
    fn test_set_without_emitter() {
        let mut config = Config::new(ConfigValue::Object(HashMap::new()));
        // Should not panic when no emitter is attached
        config.set("key", ConfigValue::Integer(42));
        let val: i64 = config.get("key").unwrap();
        assert_eq!(val, 42);
    }

    #[test]
    fn test_set_overwrites_non_object() {
        let mut config = Config::new(ConfigValue::Integer(0));
        config.set("key", ConfigValue::String("value".into()));
        let val: String = config.get("key").unwrap();
        assert_eq!(val, "value");
    }

    #[test]
    fn test_set_overwrites_nested_non_object() {
        let mut config = Config::new(obj(vec![("a", ConfigValue::Integer(1))]));
        // Setting a.b.c should replace integer "a" with an object
        config.set("a.b.c", ConfigValue::String("deep".into()));
        let val: String = config.get("a.b.c").unwrap();
        assert_eq!(val, "deep");
    }

    #[test]
    fn test_set_new_key_fires_with_none_previous() {
        let mut config = Config::new(ConfigValue::Object(HashMap::new()));

        let log = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let log_clone = log.clone();
        config.on_change(Box::new(move |key, _value, prev| {
            log_clone
                .lock()
                .unwrap()
                .push((key.to_string(), prev.cloned()));
        }));

        config.set("new_key", ConfigValue::Integer(1));

        let entries = log.lock().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].0, "new_key");
        assert!(entries[0].1.is_none());
    }

    #[test]
    fn test_data_and_data_mut() {
        let mut config = Config::new(obj(vec![("x", ConfigValue::Integer(1))]));
        assert!(config.data().as_object().is_some());

        if let ConfigValue::Object(map) = config.data_mut() {
            map.insert("y".to_string(), ConfigValue::Integer(2));
        }
        assert!(config.has_key("y"));
    }

    #[test]
    fn test_get_value_non_object_intermediate() {
        let config = Config::new(obj(vec![("a", ConfigValue::Integer(1))]));
        // Trying to traverse through a non-object should fail
        let result = config.get_value("a.b");
        assert!(matches!(result, Err(Error::KeyNotFound(_))));
    }

    #[test]
    fn test_with_source() {
        let config = Config::with_source(
            ConfigValue::Integer(42),
            PathBuf::from("/tmp/test.json"),
        );
        assert_eq!(
            config.source_path(),
            Some(&PathBuf::from("/tmp/test.json"))
        );
    }
}
