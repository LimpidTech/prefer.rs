//! Database configuration loader.
//!
//! Provides a simplified trait (`ConfigLoader`) for loading configuration from
//! databases, and a `DbLoader` adapter that bridges it to the `Loader` trait.
//!
//! Configuration data can be provided in three forms:
//!
//! - **Raw** — a format string (e.g., stored JSON/TOML/YAML) that gets
//!   parsed using the appropriate formatter from the provided list.
//! - **Columnar** — structured key-value pairs from database columns,
//!   converted directly to `ConfigValue`. Dotted keys are expanded into
//!   nested objects (e.g., `database.host` → `{database: {host: ...}}`).
//! - **Wide** — column names are setting keys, with a single row providing
//!   the values. Also expressed as `ConfigEntry::Columnar`.
//!
//! # URL Parameters
//!
//! Database identifiers support query parameters for controlling behavior:
//!
//! | Param | Default | Description |
//! |-------|---------|-------------|
//! | `table` | `configuration` | Table name |
//! | `strategy` | `auto` | `auto`, `kv`, `raw`, `wide` |
//! | `name_column` | `name` | Key column for kv mode |
//! | `value_column` | `value` | Value column for kv mode |
//! | `separator` | `.` | Path separator for dotted key expansion |
//! | `filter_column` | (none) | Row filter column for wide mode |
//! | `filter_value` | (none) | Row filter value for wide mode |
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
use url::Url;

const DEFAULT_TABLE: &str = "configuration";
const DEFAULT_NAME_COLUMN: &str = "name";
const DEFAULT_VALUE_COLUMN: &str = "value";
const DEFAULT_SEPARATOR: &str = ".";

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
///   The key-value pairs are converted directly to `ConfigValue`, with dotted
///   keys expanded into nested objects.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum ConfigEntry {
    Raw { format: String, content: String },
    Columnar(BTreeMap<String, ColumnValue>),
}

/// Strategy for how configuration is stored in the database table.
#[derive(Debug, Clone, PartialEq)]
pub enum SchemaStrategy {
    /// Key-value rows: one row per setting, with name and value columns.
    Kv,
    /// Raw blob: a single row with serialized config data.
    Raw { has_format_column: bool },
    /// Wide table: column names are setting keys, first row is values.
    Wide,
}

/// Parsed URL parameters that control database loading behavior.
#[derive(Debug, Clone)]
pub struct IdentifierParams {
    pub table: String,
    pub strategy: StrategyChoice,
    pub name_column: String,
    pub value_column: String,
    pub separator: String,
    pub filter_column: Option<String>,
    pub filter_value: Option<String>,
}

/// The user's choice of schema strategy.
#[derive(Debug, Clone, PartialEq)]
pub enum StrategyChoice {
    Auto,
    Kv,
    Raw,
    Wide,
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

    /// Override to customize how columnar results are expanded into nested
    /// `ConfigValue` trees. The default implementation splits keys on the
    /// separator and builds nested objects.
    ///
    /// DB crates can override this for query-level optimization (e.g.,
    /// multi-table joins, PostgreSQL's `jsonb_build_object`).
    fn expand_columnar(
        &self,
        values: BTreeMap<String, ColumnValue>,
        separator: &str,
    ) -> ConfigValue {
        expand_dotted_paths(values, separator)
    }
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
        let params = parse_identifier_params(identifier)?;
        let entry = self.0.load_config(identifier).await?;

        let data = match entry {
            ConfigEntry::Raw { format, content } => {
                let fmt = formatters
                    .iter()
                    .find(|f| f.extensions().contains(&format.as_str()))
                    .ok_or_else(|| Error::NoFormatterFound(format))?;
                fmt.deserialize(&content)?
            }
            ConfigEntry::Columnar(values) => self.0.expand_columnar(values, &params.separator),
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

/// Parse URL query parameters from a database identifier.
///
/// Extracts `table`, `strategy`, `name_column`, `value_column`, `separator`,
/// `filter_column`, and `filter_value` from the URL query string.
pub fn parse_identifier_params(identifier: &str) -> Result<IdentifierParams> {
    let parsed = Url::parse(identifier).map_err(|e| Error::SourceError {
        source_name: "db".to_string(),
        source: format!("invalid URL: {}", e).into(),
    })?;

    let get_param = |key: &str| -> Option<String> {
        parsed
            .query_pairs()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.into_owned())
    };

    let strategy = match get_param("strategy").as_deref() {
        Some("kv") => StrategyChoice::Kv,
        Some("raw") => StrategyChoice::Raw,
        Some("wide") => StrategyChoice::Wide,
        Some("auto") | None => StrategyChoice::Auto,
        Some(other) => {
            return Err(Error::SourceError {
                source_name: "db".to_string(),
                source: format!(
                    "unknown strategy '{}': expected auto, kv, raw, or wide",
                    other
                )
                .into(),
            });
        }
    };

    let table = get_param("table").unwrap_or_else(|| DEFAULT_TABLE.to_string());
    validate_identifier_name(&table, "table")?;

    let name_column = get_param("name_column").unwrap_or_else(|| DEFAULT_NAME_COLUMN.to_string());
    let value_column =
        get_param("value_column").unwrap_or_else(|| DEFAULT_VALUE_COLUMN.to_string());
    validate_identifier_name(&name_column, "name_column")?;
    validate_identifier_name(&value_column, "value_column")?;

    let filter_column = get_param("filter_column");
    let filter_value = get_param("filter_value");
    if let Some(ref col) = filter_column {
        validate_identifier_name(col, "filter_column")?;
    }

    Ok(IdentifierParams {
        table,
        strategy,
        name_column,
        value_column,
        separator: get_param("separator").unwrap_or_else(|| DEFAULT_SEPARATOR.to_string()),
        filter_column,
        filter_value,
    })
}

/// Strip prefer-specific query parameters from a URL, returning the
/// cleaned URL string suitable for passing to a database driver.
pub fn strip_prefer_params(identifier: &str) -> Result<String> {
    let mut parsed = Url::parse(identifier).map_err(|e| Error::SourceError {
        source_name: "db".to_string(),
        source: format!("invalid URL: {}", e).into(),
    })?;

    let prefer_params = [
        "table",
        "strategy",
        "name_column",
        "value_column",
        "separator",
        "filter_column",
        "filter_value",
    ];

    let remaining: Vec<(String, String)> = parsed
        .query_pairs()
        .filter(|(k, _)| !prefer_params.contains(&k.as_ref()))
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();

    if remaining.is_empty() {
        parsed.set_query(None);
    } else {
        let query = remaining
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join("&");
        parsed.set_query(Some(&query));
    }

    Ok(parsed.to_string())
}

/// Validate that a name (table, column) contains only safe characters.
///
/// Accepts alphanumeric characters and underscores. Rejects empty strings,
/// SQL injection attempts, and names with dots, hyphens, or spaces.
pub fn validate_identifier_name(name: &str, label: &str) -> Result<()> {
    let is_valid = !name.is_empty() && name.chars().all(|c| c.is_alphanumeric() || c == '_');

    if is_valid {
        Ok(())
    } else {
        Err(Error::SourceError {
            source_name: "db".to_string(),
            source: format!("invalid {}: '{}'", label, name).into(),
        })
    }
}

/// Detect the schema strategy based on table column names and user params.
///
/// If the user explicitly chose a strategy (not `auto`), returns that.
/// Otherwise, auto-detects:
/// 1. Has `name_column` + `value_column` → `Kv`
/// 2. Has `data` column → `Raw` (with optional `format` column)
/// 3. Anything else → `Wide`
pub fn detect_strategy(columns: &[String], params: &IdentifierParams) -> Result<SchemaStrategy> {
    match params.strategy {
        StrategyChoice::Kv => return Ok(SchemaStrategy::Kv),
        StrategyChoice::Raw => {
            let has_format = columns.iter().any(|c| c == "format");
            return Ok(SchemaStrategy::Raw {
                has_format_column: has_format,
            });
        }
        StrategyChoice::Wide => return Ok(SchemaStrategy::Wide),
        StrategyChoice::Auto => {}
    }

    let has_name = columns.iter().any(|c| c == &params.name_column);
    let has_value = columns.iter().any(|c| c == &params.value_column);
    let has_data = columns.iter().any(|c| c == "data");
    let has_format = columns.iter().any(|c| c == "format");

    if has_name && has_value {
        return Ok(SchemaStrategy::Kv);
    }

    if has_data {
        return Ok(SchemaStrategy::Raw {
            has_format_column: has_format,
        });
    }

    Ok(SchemaStrategy::Wide)
}

/// Convert a single `ColumnValue` to `ConfigValue`.
pub fn column_to_config_value(value: ColumnValue) -> ConfigValue {
    match value {
        ColumnValue::Null => ConfigValue::Null,
        ColumnValue::Bool(b) => ConfigValue::Bool(b),
        ColumnValue::Integer(i) => ConfigValue::Integer(i),
        ColumnValue::Float(f) => ConfigValue::Float(f),
        ColumnValue::String(s) => ConfigValue::String(s),
    }
}

/// Expand columnar key-value pairs into a nested `ConfigValue` tree.
///
/// Keys are split on the given separator, and intermediate objects are
/// created as needed. For example, with separator `"."`:
///
/// ```text
/// database.host = "localhost"  →  { database: { host: "localhost" } }
/// database.port = 5432         →  { database: { ..., port: 5432 } }
/// ```
///
/// Keys without the separator produce flat top-level entries.
pub fn expand_dotted_paths(values: BTreeMap<String, ColumnValue>, separator: &str) -> ConfigValue {
    let mut root = ConfigValue::Object(HashMap::new());

    for (key, value) in values {
        let config_value = column_to_config_value(value);
        let parts: Vec<&str> = key.split(separator).collect();
        set_nested_value(&mut root, &parts, config_value);
    }

    root
}

/// Set a value at a nested path within a `ConfigValue` tree.
///
/// Creates intermediate `Object` nodes as needed. If a non-object value
/// exists at an intermediate path, it is replaced with an object.
pub fn set_nested_value(root: &mut ConfigValue, path: &[&str], value: ConfigValue) {
    if path.is_empty() {
        return;
    }

    if path.len() == 1 {
        if let ConfigValue::Object(map) = root {
            map.insert(path[0].to_string(), value);
        }
        return;
    }

    if let ConfigValue::Object(map) = root {
        let child = map
            .entry(path[0].to_string())
            .or_insert_with(|| ConfigValue::Object(HashMap::new()));

        if !matches!(child, ConfigValue::Object(_)) {
            *child = ConfigValue::Object(HashMap::new());
        }

        set_nested_value(child, &path[1..], value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry;

    #[test]
    fn test_parse_identifier_params_defaults() {
        let params = parse_identifier_params("testdb://localhost/db").unwrap();
        assert_eq!(params.table, DEFAULT_TABLE);
        assert_eq!(params.strategy, StrategyChoice::Auto);
        assert_eq!(params.name_column, DEFAULT_NAME_COLUMN);
        assert_eq!(params.value_column, DEFAULT_VALUE_COLUMN);
        assert_eq!(params.separator, DEFAULT_SEPARATOR);
        assert!(params.filter_column.is_none());
        assert!(params.filter_value.is_none());
    }

    #[test]
    fn test_parse_identifier_params_custom() {
        let params = parse_identifier_params(
            "postgres://host/db?table=settings&strategy=kv&name_column=k&value_column=v&separator=__",
        )
        .unwrap();
        assert_eq!(params.table, "settings");
        assert_eq!(params.strategy, StrategyChoice::Kv);
        assert_eq!(params.name_column, "k");
        assert_eq!(params.value_column, "v");
        assert_eq!(params.separator, "__");
    }

    #[test]
    fn test_parse_identifier_params_filter() {
        let params =
            parse_identifier_params("postgres://host/db?filter_column=env&filter_value=prod")
                .unwrap();
        assert_eq!(params.filter_column, Some("env".to_string()));
        assert_eq!(params.filter_value, Some("prod".to_string()));
    }

    #[test]
    fn test_parse_identifier_params_invalid_strategy() {
        let result = parse_identifier_params("testdb://host/db?strategy=unknown");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_identifier_params_invalid_url() {
        let result = parse_identifier_params("not a url");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_identifier_params_invalid_table_name() {
        let result = parse_identifier_params("testdb://host/db?table=bad;name");
        assert!(result.is_err());
    }

    #[test]
    fn test_strip_prefer_params() {
        let cleaned = strip_prefer_params("postgres://host/db?table=cfg&sslmode=require").unwrap();
        assert!(!cleaned.contains("table="));
        assert!(cleaned.contains("sslmode=require"));
    }

    #[test]
    fn test_strip_prefer_params_all_removed() {
        let cleaned = strip_prefer_params("postgres://host/db?table=cfg&strategy=kv").unwrap();
        assert!(!cleaned.contains('?'));
    }

    #[test]
    fn test_strip_prefer_params_preserves_non_prefer() {
        let cleaned =
            strip_prefer_params("postgres://host/db?table=cfg&sslmode=require&connect_timeout=10")
                .unwrap();
        assert!(cleaned.contains("sslmode=require"));
        assert!(cleaned.contains("connect_timeout=10"));
        assert!(!cleaned.contains("table="));
    }

    #[test]
    fn test_validate_identifier_name_valid() {
        assert!(validate_identifier_name("prefer_config", "table").is_ok());
        assert!(validate_identifier_name("Config123", "table").is_ok());
        assert!(validate_identifier_name("a", "table").is_ok());
    }

    #[test]
    fn test_validate_identifier_name_invalid() {
        assert!(validate_identifier_name("", "table").is_err());
        assert!(validate_identifier_name("table; DROP TABLE users", "table").is_err());
        assert!(validate_identifier_name("my-table", "table").is_err());
        assert!(validate_identifier_name("schema.table", "table").is_err());
    }

    #[test]
    fn test_detect_strategy_auto_kv() {
        let columns = vec!["id".into(), "name".into(), "value".into()];
        let params = default_params();
        assert_eq!(
            detect_strategy(&columns, &params).unwrap(),
            SchemaStrategy::Kv
        );
    }

    #[test]
    fn test_detect_strategy_auto_raw_with_format() {
        let columns = vec!["id".into(), "data".into(), "format".into()];
        let params = default_params();
        assert_eq!(
            detect_strategy(&columns, &params).unwrap(),
            SchemaStrategy::Raw {
                has_format_column: true
            }
        );
    }

    #[test]
    fn test_detect_strategy_auto_raw_without_format() {
        let columns = vec!["id".into(), "data".into()];
        let params = default_params();
        assert_eq!(
            detect_strategy(&columns, &params).unwrap(),
            SchemaStrategy::Raw {
                has_format_column: false
            }
        );
    }

    #[test]
    fn test_detect_strategy_auto_wide() {
        let columns = vec!["host".into(), "port".into(), "debug".into()];
        let params = default_params();
        assert_eq!(
            detect_strategy(&columns, &params).unwrap(),
            SchemaStrategy::Wide
        );
    }

    #[test]
    fn test_detect_strategy_kv_wins_over_data() {
        let columns = vec!["name".into(), "value".into(), "data".into()];
        let params = default_params();
        assert_eq!(
            detect_strategy(&columns, &params).unwrap(),
            SchemaStrategy::Kv
        );
    }

    #[test]
    fn test_detect_strategy_explicit_overrides() {
        let columns = vec!["name".into(), "value".into()];

        let mut params = default_params();
        params.strategy = StrategyChoice::Raw;
        assert_eq!(
            detect_strategy(&columns, &params).unwrap(),
            SchemaStrategy::Raw {
                has_format_column: false
            }
        );

        params.strategy = StrategyChoice::Wide;
        assert_eq!(
            detect_strategy(&columns, &params).unwrap(),
            SchemaStrategy::Wide
        );
    }

    #[test]
    fn test_detect_strategy_custom_column_names() {
        let columns = vec!["setting".into(), "val".into()];
        let mut params = default_params();
        params.name_column = "setting".to_string();
        params.value_column = "val".to_string();
        assert_eq!(
            detect_strategy(&columns, &params).unwrap(),
            SchemaStrategy::Kv
        );
    }

    #[test]
    fn test_expand_dotted_paths_flat() {
        let mut values = BTreeMap::new();
        values.insert("host".to_string(), ColumnValue::String("localhost".into()));
        values.insert("port".to_string(), ColumnValue::Integer(8080));

        let result = expand_dotted_paths(values, ".");
        assert_eq!(result.get("host").unwrap().as_str(), Some("localhost"));
        assert_eq!(result.get("port").unwrap().as_i64(), Some(8080));
    }

    #[test]
    fn test_expand_dotted_paths_nested() {
        let mut values = BTreeMap::new();
        values.insert(
            "database.host".to_string(),
            ColumnValue::String("localhost".into()),
        );
        values.insert("database.port".to_string(), ColumnValue::Integer(5432));
        values.insert(
            "server.bind".to_string(),
            ColumnValue::String("0.0.0.0".into()),
        );

        let result = expand_dotted_paths(values, ".");
        let db = result.get("database").unwrap();
        assert_eq!(db.get("host").unwrap().as_str(), Some("localhost"));
        assert_eq!(db.get("port").unwrap().as_i64(), Some(5432));
        assert_eq!(
            result.get("server").unwrap().get("bind").unwrap().as_str(),
            Some("0.0.0.0")
        );
    }

    #[test]
    fn test_expand_dotted_paths_deep() {
        let mut values = BTreeMap::new();
        values.insert("a.b.c.d".to_string(), ColumnValue::Integer(42));

        let result = expand_dotted_paths(values, ".");
        assert_eq!(
            result
                .get("a")
                .unwrap()
                .get("b")
                .unwrap()
                .get("c")
                .unwrap()
                .get("d")
                .unwrap()
                .as_i64(),
            Some(42)
        );
    }

    #[test]
    fn test_expand_dotted_paths_custom_separator() {
        let mut values = BTreeMap::new();
        values.insert(
            "database__host".to_string(),
            ColumnValue::String("localhost".into()),
        );

        let result = expand_dotted_paths(values, "__");
        assert_eq!(
            result
                .get("database")
                .unwrap()
                .get("host")
                .unwrap()
                .as_str(),
            Some("localhost")
        );
    }

    #[test]
    fn test_expand_dotted_paths_empty() {
        let values = BTreeMap::new();
        let result = expand_dotted_paths(values, ".");
        assert!(result.as_object().unwrap().is_empty());
    }

    #[test]
    fn test_expand_dotted_paths_all_types() {
        let mut values = BTreeMap::new();
        values.insert("null_val".to_string(), ColumnValue::Null);
        values.insert("bool_val".to_string(), ColumnValue::Bool(true));
        values.insert("int_val".to_string(), ColumnValue::Integer(42));
        values.insert("float_val".to_string(), ColumnValue::Float(1.5));
        values.insert("str_val".to_string(), ColumnValue::String("hello".into()));

        let result = expand_dotted_paths(values, ".");
        assert!(matches!(result.get("null_val").unwrap(), ConfigValue::Null));
        assert_eq!(result.get("bool_val").unwrap().as_bool(), Some(true));
        assert_eq!(result.get("int_val").unwrap().as_i64(), Some(42));
        assert_eq!(result.get("float_val").unwrap().as_f64(), Some(1.5));
        assert_eq!(result.get("str_val").unwrap().as_str(), Some("hello"));
    }

    #[test]
    fn test_set_nested_value_empty_path() {
        let mut root = ConfigValue::Object(HashMap::new());
        set_nested_value(&mut root, &[], ConfigValue::Integer(1));
        assert!(root.as_object().unwrap().is_empty());
    }

    #[test]
    fn test_set_nested_value_replaces_non_object() {
        let mut root = ConfigValue::Object(HashMap::new());
        set_nested_value(
            &mut root,
            &["a"],
            ConfigValue::String("initial".to_string()),
        );
        set_nested_value(&mut root, &["a", "b"], ConfigValue::Integer(42));
        assert_eq!(root.get("a").unwrap().get("b").unwrap().as_i64(), Some(42));
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

    // --- DbLoader tests ---

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

    struct DottedColumnarLoader;

    #[async_trait]
    impl ConfigLoader for DottedColumnarLoader {
        fn scheme(&self) -> &str {
            "dotted"
        }

        async fn load_config(&self, _identifier: &str) -> Result<ConfigEntry> {
            let mut values = BTreeMap::new();
            values.insert(
                "database.host".to_string(),
                ColumnValue::String("localhost".into()),
            );
            values.insert("database.port".to_string(), ColumnValue::Integer(5432));
            Ok(ConfigEntry::Columnar(values))
        }

        fn name(&self) -> &str {
            "dotted"
        }
    }

    struct CustomExpanderLoader;

    #[async_trait]
    impl ConfigLoader for CustomExpanderLoader {
        fn scheme(&self) -> &str {
            "custom"
        }

        async fn load_config(&self, _identifier: &str) -> Result<ConfigEntry> {
            let mut values = BTreeMap::new();
            values.insert("a.b".to_string(), ColumnValue::Integer(1));
            Ok(ConfigEntry::Columnar(values))
        }

        fn name(&self) -> &str {
            "custom"
        }

        fn expand_columnar(
            &self,
            values: BTreeMap<String, ColumnValue>,
            _separator: &str,
        ) -> ConfigValue {
            let map: HashMap<String, ConfigValue> = values
                .into_iter()
                .map(|(k, v)| (k, column_to_config_value(v)))
                .collect();
            ConfigValue::Object(map)
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

    fn default_params() -> IdentifierParams {
        IdentifierParams {
            table: DEFAULT_TABLE.to_string(),
            strategy: StrategyChoice::Auto,
            name_column: DEFAULT_NAME_COLUMN.to_string(),
            value_column: DEFAULT_VALUE_COLUMN.to_string(),
            separator: DEFAULT_SEPARATOR.to_string(),
            filter_column: None,
            filter_value: None,
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
    async fn test_load_columnar_expands_dotted_keys() {
        let formatters = registry::collect_formatters();
        let loader = DbLoader::new(DottedColumnarLoader);
        let result = loader.load("dotted://settings", &formatters).await.unwrap();

        let db = result.data.get("database").unwrap();
        assert_eq!(db.get("host").unwrap().as_str(), Some("localhost"));
        assert_eq!(db.get("port").unwrap().as_i64(), Some(5432));
    }

    #[tokio::test]
    async fn test_load_columnar_custom_expander() {
        let formatters = registry::collect_formatters();
        let loader = DbLoader::new(CustomExpanderLoader);
        let result = loader.load("custom://settings", &formatters).await.unwrap();

        assert_eq!(result.data.get("a.b").unwrap().as_i64(), Some(1));
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
}
