//! Configuration source abstraction.
//!
//! This module provides the `Source` trait for abstracting configuration sources,
//! allowing configuration to be loaded from files, environment variables, databases,
//! or any other source.
//!
//! **Deprecated:** The `Source` trait is superseded by `Loader` + `Formatter`.
//! `EnvSource`, `MemorySource`, and `LayeredSource` remain as layering utilities.

#![allow(deprecated)] // Internal implementations still reference their own deprecated types

use crate::error::{Error, Result};
use crate::registry;
use crate::value::ConfigValue;
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// A source of configuration data.
///
/// **Deprecated:** Use the `Loader` trait instead. `Loader` participates in
/// automatic plugin discovery via the registry, while `Source` requires
/// manual construction and wiring. `Source` will be removed in a future
/// major version.
///
/// `EnvSource` and `MemorySource` are not affected — they remain as
/// layering utilities used by `ConfigBuilder`.
///
/// # Examples
///
/// ```
/// use prefer::source::Source;
/// use prefer::{ConfigValue, Result};
/// use async_trait::async_trait;
///
/// struct MyCustomSource {
///     data: ConfigValue,
/// }
///
/// #[async_trait]
/// impl Source for MyCustomSource {
///     async fn load(&self) -> Result<ConfigValue> {
///         Ok(self.data.clone())
///     }
///
///     fn name(&self) -> &str {
///         "custom"
///     }
/// }
/// ```
#[deprecated(
    since = "0.4.0",
    note = "Use the Loader trait instead. Source will be removed in a future version."
)]
#[async_trait]
pub trait Source: Send + Sync {
    /// Load configuration data from this source.
    async fn load(&self) -> Result<ConfigValue>;

    /// Get a human-readable name for this source (used in error messages).
    fn name(&self) -> &str;
}

/// A configuration source that loads from a file.
///
/// **Deprecated:** Use `FileLoader` instead, which participates in
/// automatic registry discovery.
#[deprecated(
    since = "0.4.0",
    note = "Use prefer::loader::file::FileLoader instead."
)]
pub struct FileSource {
    path: PathBuf,
}

impl FileSource {
    /// Create a new file source from a path.
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
        }
    }

    /// Get the path this source loads from.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[async_trait]
impl Source for FileSource {
    async fn load(&self) -> Result<ConfigValue> {
        let contents = tokio::fs::read_to_string(&self.path).await?;
        let source = self.path.to_string_lossy().to_string();
        let formatters = registry::collect_formatters();
        let fmt = formatters
            .iter()
            .find(|f| f.provides(&source))
            .ok_or_else(|| Error::UnsupportedFormat(self.path.clone()))?;
        fmt.deserialize(&contents)
    }

    fn name(&self) -> &str {
        // Path is always valid UTF-8 on Windows/Unix, but we store it to return &str
        // If this becomes an issue, we could change the trait to return Cow<str>
        self.path.to_str().expect("path should be valid UTF-8")
    }
}

/// A configuration source that loads from environment variables.
///
/// Environment variables are converted to a nested structure using a separator.
/// For example, with prefix "APP" and separator "__":
/// - `APP__DATABASE__HOST=localhost` becomes `{ "database": { "host": "localhost" } }`
pub struct EnvSource {
    prefix: String,
    separator: String,
}

impl EnvSource {
    /// Create a new environment source with the given prefix.
    ///
    /// Uses "__" as the default separator for nested keys.
    pub fn new(prefix: impl Into<String>) -> Self {
        Self {
            prefix: prefix.into(),
            separator: "__".to_string(),
        }
    }

    /// Create a new environment source with a custom separator.
    pub fn with_separator(prefix: impl Into<String>, separator: impl Into<String>) -> Self {
        Self {
            prefix: prefix.into(),
            separator: separator.into(),
        }
    }

    /// Convert a flat map of environment variables to a nested structure.
    fn to_nested_value(&self, vars: HashMap<String, String>) -> ConfigValue {
        let mut root: HashMap<String, ConfigValue> = HashMap::new();

        for (key, value) in vars {
            // Remove prefix and convert to lowercase
            let key = key
                .strip_prefix(&self.prefix)
                .and_then(|k| k.strip_prefix(&self.separator))
                .unwrap_or(&key)
                .to_lowercase();

            let parts: Vec<&str> = key.split(&self.separator.to_lowercase()).collect();
            insert_nested(&mut root, &parts, value);
        }

        ConfigValue::Object(root)
    }
}

fn insert_nested(obj: &mut HashMap<String, ConfigValue>, path: &[&str], value: String) {
    // path is never empty because str::split() always returns at least one element
    debug_assert!(!path.is_empty(), "path should never be empty");

    let key = path[0].to_string();

    if path.len() == 1 {
        // Try to parse as different types
        let parsed_value = if value.eq_ignore_ascii_case("true") {
            ConfigValue::Bool(true)
        } else if value.eq_ignore_ascii_case("false") {
            ConfigValue::Bool(false)
        } else if let Ok(n) = value.parse::<i64>() {
            ConfigValue::Integer(n)
        } else if let Ok(n) = value.parse::<f64>() {
            ConfigValue::Float(n)
        } else {
            ConfigValue::String(value)
        };
        obj.insert(key, parsed_value);
    } else {
        // Get or create nested object
        let nested = obj
            .entry(key)
            .or_insert_with(|| ConfigValue::Object(HashMap::new()));

        if let ConfigValue::Object(nested_obj) = nested {
            insert_nested(nested_obj, &path[1..], value);
        }
    }
}

#[async_trait]
impl Source for EnvSource {
    async fn load(&self) -> Result<ConfigValue> {
        let prefix_with_sep = format!("{}{}", self.prefix, self.separator);
        let vars: HashMap<String, String> = std::env::vars()
            .filter(|(k, _)| k.starts_with(&prefix_with_sep))
            .collect();

        Ok(self.to_nested_value(vars))
    }

    fn name(&self) -> &str {
        &self.prefix
    }
}

/// A configuration source that holds data in memory.
///
/// Useful for testing or providing default values.
pub struct MemorySource {
    data: ConfigValue,
    source_name: String,
}

impl MemorySource {
    /// Create a new memory source with the given data.
    pub fn new(data: ConfigValue) -> Self {
        Self {
            data,
            source_name: "memory".to_string(),
        }
    }

    /// Create a new memory source with a custom name.
    pub fn with_name(data: ConfigValue, name: impl Into<String>) -> Self {
        Self {
            data,
            source_name: name.into(),
        }
    }
}

#[async_trait]
impl Source for MemorySource {
    async fn load(&self) -> Result<ConfigValue> {
        Ok(self.data.clone())
    }

    fn name(&self) -> &str {
        &self.source_name
    }
}

/// A configuration source that layers multiple sources with priority.
///
/// Later sources override earlier sources when keys conflict.
pub struct LayeredSource {
    pub(crate) sources: Vec<Box<dyn Source>>,
}

impl LayeredSource {
    /// Create a new layered source.
    pub fn new() -> Self {
        Self {
            sources: Vec::new(),
        }
    }

    /// Add a source to the layer (lower priority than sources added later).
    pub fn with_source<S: Source + 'static>(mut self, source: S) -> Self {
        self.sources.push(Box::new(source));
        self
    }

    /// Add a boxed source to the layer.
    pub fn add_boxed(mut self, source: Box<dyn Source>) -> Self {
        self.sources.push(source);
        self
    }
}

impl Default for LayeredSource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Source for LayeredSource {
    async fn load(&self) -> Result<ConfigValue> {
        let mut merged = ConfigValue::Object(HashMap::new());

        for source in &self.sources {
            let value = source.load().await.map_err(|e| Error::SourceError {
                source_name: source.name().to_string(),
                source: Box::new(e),
            })?;
            merge_values(&mut merged, value);
        }

        Ok(merged)
    }

    fn name(&self) -> &str {
        "layered"
    }
}

/// Deep merge two ConfigValues, with `overlay` taking precedence.
fn merge_values(base: &mut ConfigValue, overlay: ConfigValue) {
    match (base, overlay) {
        (ConfigValue::Object(base_obj), ConfigValue::Object(overlay_obj)) => {
            for (key, overlay_value) in overlay_obj {
                match base_obj.get_mut(&key) {
                    Some(base_value) => merge_values(base_value, overlay_value),
                    None => {
                        base_obj.insert(key, overlay_value);
                    }
                }
            }
        }
        (base, overlay) => {
            *base = overlay;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::test_helpers::{bool_val, int, obj};
    use serial_test::serial;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_memory_source() {
        let data = obj(vec![
            ("host", ConfigValue::String("localhost".to_string())),
            ("port", ConfigValue::Integer(8080)),
        ]);
        let source = MemorySource::new(data.clone());

        let loaded = source.load().await.unwrap();
        assert_eq!(loaded, data);
    }

    #[tokio::test]
    async fn test_layered_source_merge() {
        let base = MemorySource::with_name(
            obj(vec![
                (
                    "database",
                    obj(vec![
                        ("host", ConfigValue::String("localhost".to_string())),
                        ("port", ConfigValue::Integer(5432)),
                    ]),
                ),
                ("debug", ConfigValue::Bool(false)),
            ]),
            "base",
        );

        let overlay = MemorySource::with_name(
            obj(vec![
                (
                    "database",
                    obj(vec![(
                        "host",
                        ConfigValue::String("production.db.example.com".to_string()),
                    )]),
                ),
                ("debug", ConfigValue::Bool(true)),
            ]),
            "overlay",
        );

        let layered = LayeredSource::new().with_source(base).with_source(overlay);

        let result = layered.load().await.unwrap();

        assert_eq!(
            result
                .get("database")
                .unwrap()
                .get("host")
                .unwrap()
                .as_str(),
            Some("production.db.example.com")
        );
        assert_eq!(
            result
                .get("database")
                .unwrap()
                .get("port")
                .unwrap()
                .as_i64(),
            Some(5432)
        ); // From base
        assert_eq!(result.get("debug").unwrap().as_bool(), Some(true)); // From overlay
    }

    #[test]
    fn test_env_source_nested() {
        let source = EnvSource::new("TEST");
        let vars = HashMap::from([
            ("TEST__DATABASE__HOST".to_string(), "localhost".to_string()),
            ("TEST__DATABASE__PORT".to_string(), "5432".to_string()),
            ("TEST__DEBUG".to_string(), "true".to_string()),
        ]);

        let result = source.to_nested_value(vars);

        assert_eq!(
            result
                .get("database")
                .unwrap()
                .get("host")
                .unwrap()
                .as_str(),
            Some("localhost")
        );
        assert_eq!(
            result
                .get("database")
                .unwrap()
                .get("port")
                .unwrap()
                .as_i64(),
            Some(5432)
        );
        assert_eq!(result.get("debug").unwrap().as_bool(), Some(true));
    }

    #[test]
    fn test_merge_values() {
        let mut base = obj(vec![
            ("a", ConfigValue::Integer(1)),
            (
                "b",
                obj(vec![
                    ("c", ConfigValue::Integer(2)),
                    ("d", ConfigValue::Integer(3)),
                ]),
            ),
        ]);

        let overlay = obj(vec![
            ("a", ConfigValue::Integer(10)),
            ("b", obj(vec![("c", ConfigValue::Integer(20))])),
            ("e", ConfigValue::Integer(5)),
        ]);

        merge_values(&mut base, overlay);

        assert_eq!(base.get("a").unwrap().as_i64(), Some(10));
        assert_eq!(base.get("b").unwrap().get("c").unwrap().as_i64(), Some(20));
        assert_eq!(base.get("b").unwrap().get("d").unwrap().as_i64(), Some(3));
        assert_eq!(base.get("e").unwrap().as_i64(), Some(5));
    }

    #[tokio::test]
    async fn test_file_source_load() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("source.json");
        std::fs::write(&config_path, r#"{"source": "file"}"#).unwrap();

        let source = FileSource::new(&config_path);
        assert_eq!(source.path(), config_path);
        assert!(source.name().contains("source.json"));

        let value = source.load().await.unwrap();
        assert_eq!(value.get("source").unwrap().as_str(), Some("file"));
    }

    #[tokio::test]
    async fn test_file_source_not_found() {
        let source = FileSource::new("/nonexistent/path.json");
        assert!(source.load().await.is_err());
    }

    #[tokio::test]
    #[serial]
    async fn test_env_source_load() {
        std::env::set_var("PREFERTEST__DB__HOST", "localhost");
        std::env::set_var("PREFERTEST__DB__PORT", "5432");
        std::env::set_var("PREFERTEST__DEBUG", "true");

        let source = EnvSource::new("PREFERTEST");
        assert_eq!(source.name(), "PREFERTEST");

        let value = source.load().await.unwrap();
        assert_eq!(
            value.get("db").unwrap().get("host").unwrap().as_str(),
            Some("localhost")
        );
        assert_eq!(
            value.get("db").unwrap().get("port").unwrap().as_i64(),
            Some(5432)
        );
        assert_eq!(value.get("debug").unwrap().as_bool(), Some(true));

        std::env::remove_var("PREFERTEST__DB__HOST");
        std::env::remove_var("PREFERTEST__DB__PORT");
        std::env::remove_var("PREFERTEST__DEBUG");
    }

    #[tokio::test]
    #[serial]
    async fn test_env_source_with_separator() {
        std::env::set_var("PREFERSEP_DB_HOST", "dbhost");

        let source = EnvSource::with_separator("PREFERSEP", "_");
        let value = source.load().await.unwrap();
        assert_eq!(
            value.get("db").unwrap().get("host").unwrap().as_str(),
            Some("dbhost")
        );

        std::env::remove_var("PREFERSEP_DB_HOST");
    }

    #[tokio::test]
    async fn test_env_source_empty() {
        let source = EnvSource::new("NONEXISTENT_PREFIX_XYZ123");
        let value = source.load().await.unwrap();
        assert!(value.as_object().map(|o| o.is_empty()).unwrap_or(false));
    }

    #[tokio::test]
    async fn test_memory_source_coverage() {
        let data = obj(vec![("memory", ConfigValue::Bool(true))]);
        let source = MemorySource::new(data.clone());
        assert_eq!(source.name(), "memory");

        let loaded = source.load().await.unwrap();
        assert_eq!(loaded, data);
    }

    #[tokio::test]
    async fn test_memory_source_with_name() {
        let source = MemorySource::with_name(obj(vec![]), "custom");
        assert_eq!(source.name(), "custom");
    }

    #[tokio::test]
    async fn test_layered_source_override() {
        let base = MemorySource::with_name(obj(vec![("a", int(1)), ("b", int(2))]), "base");
        let overlay = MemorySource::with_name(obj(vec![("b", int(20)), ("c", int(3))]), "overlay");

        let layered = LayeredSource::new().with_source(base).with_source(overlay);
        assert_eq!(layered.name(), "layered");

        let value = layered.load().await.unwrap();
        assert_eq!(value.get("a").unwrap().as_i64(), Some(1));
        assert_eq!(value.get("b").unwrap().as_i64(), Some(20));
        assert_eq!(value.get("c").unwrap().as_i64(), Some(3));
    }

    #[tokio::test]
    async fn test_layered_source_default() {
        let layered = LayeredSource::default();
        let value = layered.load().await.unwrap();
        assert!(value.as_object().map(|o| o.is_empty()).unwrap_or(false));
    }

    #[tokio::test]
    async fn test_layered_source_add_boxed() {
        let source: Box<dyn Source> =
            Box::new(MemorySource::new(obj(vec![("boxed", bool_val(true))])));
        let layered = LayeredSource::new().add_boxed(source);
        let value = layered.load().await.unwrap();
        assert_eq!(value.get("boxed").unwrap().as_bool(), Some(true));
    }

    #[tokio::test]
    #[serial]
    async fn test_env_source_float_parsing() {
        std::env::set_var("ENVFLOAT__VALUE", "1.5");

        let source = EnvSource::new("ENVFLOAT");
        let value = source.load().await.unwrap();
        assert!((value.get("value").unwrap().as_f64().unwrap() - 1.5).abs() < 0.001);

        std::env::remove_var("ENVFLOAT__VALUE");
    }

    #[tokio::test]
    #[serial]
    async fn test_env_source_nan_float() {
        std::env::set_var("ENVNAN__VALUE", "not_a_number_at_all");

        let source = EnvSource::new("ENVNAN");
        let value = source.load().await.unwrap();
        assert_eq!(
            value.get("value").unwrap().as_str(),
            Some("not_a_number_at_all")
        );

        std::env::remove_var("ENVNAN__VALUE");
    }

    #[tokio::test]
    #[serial]
    async fn test_env_source_false_boolean() {
        std::env::set_var("ENVBOOL__ENABLED", "false");
        std::env::set_var("ENVBOOL__DISABLED", "FALSE");

        let source = EnvSource::new("ENVBOOL");
        let value = source.load().await.unwrap();
        assert_eq!(value.get("enabled").unwrap().as_bool(), Some(false));
        assert_eq!(value.get("disabled").unwrap().as_bool(), Some(false));

        std::env::remove_var("ENVBOOL__ENABLED");
        std::env::remove_var("ENVBOOL__DISABLED");
    }

    #[tokio::test]
    async fn test_layered_source_error_propagation() {
        struct FailingSource;

        #[async_trait]
        impl Source for FailingSource {
            async fn load(&self) -> Result<ConfigValue> {
                Err(Error::FileNotFound("test".into()))
            }

            fn name(&self) -> &str {
                "failing"
            }
        }

        let layered = LayeredSource::new().with_source(FailingSource);
        let result = layered.load().await;
        assert!(matches!(result.unwrap_err(), Error::SourceError { .. }));
    }
}
