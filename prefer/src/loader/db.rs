//! Database configuration loader.
//!
//! Provides a simplified trait (`ConfigLoader`) for loading configuration from
//! databases, and a `DbLoader` adapter that bridges it to the `Loader` trait.
//!
//! Configuration data can be provided in two forms:
//!
//! - **Raw** — a format string (e.g., stored JSON/TOML/YAML) that gets
//!   parsed using the appropriate formatter from the provided list.
//! - **Columnar** — structured key-value pairs from database columns,
//!   converted directly to `ConfigValue`.
//!
//! # Example
//!
//! ```no_run
//! use prefer::loader::db::{DbLoader, ConfigLoader, ConfigEntry};
//! use prefer::registry::RegisteredLoader;
//! use async_trait::async_trait;
//!
//! struct MyDbLoader;
//!
//! #[async_trait]
//! impl ConfigLoader for MyDbLoader {
//!     fn scheme(&self) -> &str {
//!         "mydb"
//!     }
//!
//!     async fn load_config(&self, identifier: &str) -> prefer::Result<ConfigEntry> {
//!         Ok(ConfigEntry::Raw {
//!             format: "json".to_string(),
//!             content: r#"{"key": "value"}"#.to_string(),
//!         })
//!     }
//!
//!     fn name(&self) -> &str {
//!         "my_database"
//!     }
//! }
//!
//! static MY_LOADER: DbLoader<MyDbLoader> = DbLoader::new(MyDbLoader);
//! inventory::submit! { RegisteredLoader(&MY_LOADER) }
//!
//! #[tokio::main]
//! async fn main() -> prefer::Result<()> {
//!     let config = prefer::load("mydb://settings").await?;
//!     let value: String = config.get("key")?;
//!     Ok(())
//! }
//! ```

use crate::error::{Error, Result};
use crate::formatter::Formatter;
use crate::loader::{LoadResult, Loader};
use crate::value::ConfigValue;
use async_trait::async_trait;
use std::collections::{BTreeMap, HashMap};

/// A single value from a database column.
#[derive(Debug, Clone, PartialEq)]
pub enum ColumnValue {
    Null,
    Bool(bool),
    Integer(i64),
    Float(f64),
    String(String),
}

/// A configuration entry loaded from a database.
///
/// Supports two storage strategies:
///
/// - `Raw` — the database stores serialized config (JSON, TOML, YAML, etc.)
///   in a text column. The format hint tells the loader which formatter to use.
/// - `Columnar` — the database stores config values across table columns.
///   The key-value pairs are converted directly to `ConfigValue`.
#[derive(Debug, Clone)]
pub enum ConfigEntry {
    Raw { format: String, content: String },
    Columnar(BTreeMap<String, ColumnValue>),
}

/// Trait for loading configuration from a database.
///
/// Implement this trait for your specific database backend. The `scheme()`
/// method declares what URL scheme this loader handles, and `load_config()`
/// fetches the configuration for a given identifier.
#[async_trait]
pub trait ConfigLoader: Send + Sync + 'static {
    /// The URL scheme this loader handles (e.g., "postgres", "sqlite").
    fn scheme(&self) -> &str;

    /// Load configuration content for the given identifier.
    ///
    /// The identifier is the full URL string (e.g., "postgres://localhost/myapp").
    async fn load_config(&self, identifier: &str) -> Result<ConfigEntry>;

    /// Human-readable name for error messages.
    fn name(&self) -> &str;
}

/// A `Loader` that delegates to a `ConfigLoader` implementation.
///
/// Wraps any `ConfigLoader` and adapts it to the `Loader` trait,
/// routing identifiers by URL scheme and returning parsed configuration
/// data.
pub struct DbLoader<L: ConfigLoader>(pub L);

impl<L: ConfigLoader> DbLoader<L> {
    pub const fn new(loader: L) -> Self {
        Self(loader)
    }
}

#[async_trait]
impl<L: ConfigLoader> Loader for DbLoader<L> {
    fn provides(&self, identifier: &str) -> bool {
        let prefix = format!("{}://", self.0.scheme());
        identifier.starts_with(&prefix)
    }

    async fn load(&self, identifier: &str, formatters: &[&dyn Formatter]) -> Result<LoadResult> {
        let entry = self.0.load_config(identifier).await?;

        let data = match entry {
            ConfigEntry::Raw { format, content } => {
                let fmt = formatters
                    .iter()
                    .find(|f| f.extensions().contains(&format.as_str()))
                    .ok_or_else(|| Error::NoFormatterFound(format))?;
                fmt.deserialize(&content)?
            }
            ConfigEntry::Columnar(values) => columnar_to_config_value(values),
        };

        Ok(LoadResult {
            source: identifier.to_string(),
            data,
        })
    }

    fn name(&self) -> &str {
        self.0.name()
    }
}

fn column_to_config_value(value: ColumnValue) -> ConfigValue {
    match value {
        ColumnValue::Null => ConfigValue::Null,
        ColumnValue::Bool(b) => ConfigValue::Bool(b),
        ColumnValue::Integer(i) => ConfigValue::Integer(i),
        ColumnValue::Float(f) => ConfigValue::Float(f),
        ColumnValue::String(s) => ConfigValue::String(s),
    }
}

fn columnar_to_config_value(values: BTreeMap<String, ColumnValue>) -> ConfigValue {
    let map: HashMap<String, ConfigValue> = values
        .into_iter()
        .map(|(k, v)| (k, column_to_config_value(v)))
        .collect();
    ConfigValue::Object(map)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry;

    struct TestLoader;

    #[async_trait]
    impl ConfigLoader for TestLoader {
        fn scheme(&self) -> &str {
            "testdb"
        }

        async fn load_config(&self, _identifier: &str) -> Result<ConfigEntry> {
            Ok(ConfigEntry::Raw {
                format: "json".to_string(),
                content: r#"{"key": "value", "num": 42}"#.to_string(),
            })
        }

        fn name(&self) -> &str {
            "test"
        }
    }

    struct EchoLoader;

    #[async_trait]
    impl ConfigLoader for EchoLoader {
        fn scheme(&self) -> &str {
            "echo"
        }

        async fn load_config(&self, identifier: &str) -> Result<ConfigEntry> {
            let mut values = BTreeMap::new();
            values.insert(
                "received".to_string(),
                ColumnValue::String(identifier.to_string()),
            );
            Ok(ConfigEntry::Columnar(values))
        }

        fn name(&self) -> &str {
            "echo"
        }
    }

    struct ColumnarLoader;

    #[async_trait]
    impl ConfigLoader for ColumnarLoader {
        fn scheme(&self) -> &str {
            "coldb"
        }

        async fn load_config(&self, _identifier: &str) -> Result<ConfigEntry> {
            let mut values = BTreeMap::new();
            values.insert("host".to_string(), ColumnValue::String("localhost".into()));
            values.insert("port".to_string(), ColumnValue::Integer(5432));
            values.insert("debug".to_string(), ColumnValue::Bool(true));
            values.insert("timeout".to_string(), ColumnValue::Float(30.5));
            values.insert("retired".to_string(), ColumnValue::Null);
            Ok(ConfigEntry::Columnar(values))
        }

        fn name(&self) -> &str {
            "columnar"
        }
    }

    struct FailingLoader;

    #[async_trait]
    impl ConfigLoader for FailingLoader {
        fn scheme(&self) -> &str {
            "faildb"
        }

        async fn load_config(&self, identifier: &str) -> Result<ConfigEntry> {
            Err(Error::SourceError {
                source_name: "faildb".to_string(),
                source: format!("connection failed for {}", identifier).into(),
            })
        }

        fn name(&self) -> &str {
            "failing"
        }
    }

    #[test]
    fn test_provides_matching_scheme() {
        let loader = DbLoader::new(TestLoader);
        assert!(loader.provides("testdb://some/path"));
        assert!(loader.provides("testdb://localhost/config"));
    }

    #[test]
    fn test_provides_rejects_other_schemes() {
        let loader = DbLoader::new(TestLoader);
        assert!(!loader.provides("postgres://localhost/db"));
        assert!(!loader.provides("file:///etc/config.toml"));
        assert!(!loader.provides("settings"));
    }

    #[test]
    fn test_provides_rejects_partial_scheme() {
        let loader = DbLoader::new(TestLoader);
        assert!(!loader.provides("testdb"));
        assert!(!loader.provides("testdb:/missing-slash"));
    }

    #[tokio::test]
    async fn test_load_raw_parses_with_formatter() {
        let formatters = registry::collect_formatters();
        let loader = DbLoader::new(TestLoader);
        let result = loader.load("testdb://settings", &formatters).await.unwrap();

        assert_eq!(result.source, "testdb://settings");
        assert_eq!(result.data.get("key").unwrap().as_str(), Some("value"));
        assert_eq!(result.data.get("num").unwrap().as_i64(), Some(42));
    }

    #[tokio::test]
    async fn test_identifier_passed_to_config_loader() {
        let formatters = registry::collect_formatters();
        let loader = DbLoader::new(EchoLoader);
        let result = loader
            .load("echo://my/specific/path", &formatters)
            .await
            .unwrap();

        assert_eq!(result.source, "echo://my/specific/path");
        assert_eq!(
            result.data.get("received").unwrap().as_str(),
            Some("echo://my/specific/path")
        );
    }

    #[tokio::test]
    async fn test_load_columnar_converts_directly() {
        let formatters = registry::collect_formatters();
        let loader = DbLoader::new(ColumnarLoader);
        let result = loader.load("coldb://settings", &formatters).await.unwrap();

        assert_eq!(result.source, "coldb://settings");
        assert_eq!(result.data.get("host").unwrap().as_str(), Some("localhost"));
        assert_eq!(result.data.get("port").unwrap().as_i64(), Some(5432));
        assert_eq!(result.data.get("debug").unwrap().as_bool(), Some(true));
        assert_eq!(result.data.get("timeout").unwrap().as_f64(), Some(30.5));
        assert!(matches!(
            result.data.get("retired").unwrap(),
            &ConfigValue::Null
        ));
    }

    #[tokio::test]
    async fn test_load_columnar_empty_map() {
        struct EmptyColumnarLoader;

        #[async_trait]
        impl ConfigLoader for EmptyColumnarLoader {
            fn scheme(&self) -> &str {
                "emptydb"
            }
            async fn load_config(&self, _id: &str) -> Result<ConfigEntry> {
                Ok(ConfigEntry::Columnar(BTreeMap::new()))
            }
            fn name(&self) -> &str {
                "empty"
            }
        }

        let formatters = registry::collect_formatters();
        let loader = DbLoader::new(EmptyColumnarLoader);
        let result = loader.load("emptydb://x", &formatters).await.unwrap();

        assert!(result.data.as_object().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_load_error_propagation() {
        let formatters = registry::collect_formatters();
        let loader = DbLoader::new(FailingLoader);
        let result = loader.load("faildb://settings", &formatters).await;

        match result {
            Err(Error::SourceError { source_name, .. }) => {
                assert_eq!(source_name, "faildb");
            }
            Err(other) => panic!("expected SourceError, got {:?}", other),
            Ok(_) => panic!("expected error, got Ok"),
        }
    }

    #[tokio::test]
    async fn test_load_raw_unknown_format_errors() {
        struct UnknownFormatLoader;

        #[async_trait]
        impl ConfigLoader for UnknownFormatLoader {
            fn scheme(&self) -> &str {
                "unkfmt"
            }
            async fn load_config(&self, _id: &str) -> Result<ConfigEntry> {
                Ok(ConfigEntry::Raw {
                    format: "bson".to_string(),
                    content: "{}".to_string(),
                })
            }
            fn name(&self) -> &str {
                "unknown-format"
            }
        }

        let formatters = registry::collect_formatters();
        let loader = DbLoader::new(UnknownFormatLoader);
        let result = loader.load("unkfmt://x", &formatters).await;

        assert!(matches!(result, Err(Error::NoFormatterFound(_))));
    }

    #[test]
    fn test_name_delegates() {
        let loader = DbLoader::new(TestLoader);
        assert_eq!(loader.name(), "test");
    }

    #[test]
    fn test_column_to_config_value() {
        assert_eq!(column_to_config_value(ColumnValue::Null), ConfigValue::Null);
        assert_eq!(
            column_to_config_value(ColumnValue::Bool(true)),
            ConfigValue::Bool(true)
        );
        assert_eq!(
            column_to_config_value(ColumnValue::Integer(42)),
            ConfigValue::Integer(42)
        );
        assert_eq!(
            column_to_config_value(ColumnValue::Float(1.5)),
            ConfigValue::Float(1.5)
        );
        assert_eq!(
            column_to_config_value(ColumnValue::String("hello".into())),
            ConfigValue::String("hello".into())
        );
    }
}
