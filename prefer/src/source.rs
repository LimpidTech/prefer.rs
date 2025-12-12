//! Configuration source abstraction.
//!
//! This module provides the `Source` trait for abstracting configuration sources,
//! allowing configuration to be loaded from files, environment variables, databases,
//! or any other source.

use crate::error::{Error, Result};
use crate::formats;
use crate::value::ConfigValue;
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// A source of configuration data.
///
/// Implementations of this trait can load configuration from various sources
/// such as files, environment variables, databases, or remote services.
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
#[async_trait]
pub trait Source: Send + Sync {
    /// Load configuration data from this source.
    async fn load(&self) -> Result<ConfigValue>;

    /// Get a human-readable name for this source (used in error messages).
    fn name(&self) -> &str;
}

/// A configuration source that loads from a file.
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
        formats::parse(&contents, &self.path)
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

    fn obj(items: Vec<(&str, ConfigValue)>) -> ConfigValue {
        ConfigValue::Object(items.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
    }

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
}
