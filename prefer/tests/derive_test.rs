//! Tests for the derive macro.

use prefer::{Config, ConfigValue};
// Import the derive macro for #[derive(FromValue)]
use prefer_derive::FromValue;

// Import the trait for calling from_value() method
use prefer::value::FromValue as FromValueTrait;

// Helper to create ConfigValue objects more easily
fn obj(items: Vec<(&str, ConfigValue)>) -> ConfigValue {
    ConfigValue::Object(items.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
}

fn arr(items: Vec<ConfigValue>) -> ConfigValue {
    ConfigValue::Array(items)
}

fn str(s: &str) -> ConfigValue {
    ConfigValue::String(s.to_string())
}

fn int(i: i64) -> ConfigValue {
    ConfigValue::Integer(i)
}

fn bool_val(b: bool) -> ConfigValue {
    ConfigValue::Bool(b)
}

#[derive(Debug, FromValue, PartialEq)]
struct SimpleConfig {
    host: String,
    port: u16,
}

#[derive(Debug, FromValue, PartialEq)]
struct ConfigWithDefaults {
    host: String,
    #[prefer(default = "8080")]
    port: u16,
    #[prefer(default)]
    debug: bool,
}

#[derive(Debug, FromValue, PartialEq)]
struct ConfigWithRename {
    #[prefer(rename = "server_host")]
    host: String,
    #[prefer(rename = "server_port")]
    port: u16,
}

#[derive(Debug, FromValue, PartialEq)]
struct ConfigWithOptional {
    host: String,
    port: Option<u16>,
    tags: Option<Vec<String>>,
}

#[derive(Debug, FromValue, PartialEq)]
struct ConfigWithSkip {
    host: String,
    port: u16,
    #[prefer(skip)]
    runtime_only: Option<String>,
}

#[derive(Debug, FromValue, PartialEq)]
struct NestedConfig {
    server: ServerConfig,
    database: DatabaseConfig,
}

#[derive(Debug, FromValue, PartialEq)]
struct ServerConfig {
    host: String,
    port: u16,
}

#[derive(Debug, FromValue, PartialEq)]
struct DatabaseConfig {
    host: String,
    port: u16,
    name: String,
}

#[derive(Debug, FromValue, PartialEq)]
#[prefer(tag = "type")]
enum Backend {
    #[prefer(rename = "postgresql")]
    Postgres {
        host: String,
        port: u16,
    },
    Sqlite {
        path: String,
    },
}

#[test]
fn test_simple_struct() {
    let value = obj(vec![("host", str("localhost")), ("port", int(8080))]);

    let config = <SimpleConfig as FromValueTrait>::from_value(&value).unwrap();
    assert_eq!(
        config,
        SimpleConfig {
            host: "localhost".to_string(),
            port: 8080
        }
    );
}

#[test]
fn test_struct_with_defaults() {
    let value = obj(vec![("host", str("localhost"))]);

    let config = <ConfigWithDefaults as FromValueTrait>::from_value(&value).unwrap();
    assert_eq!(config.host, "localhost");
    assert_eq!(config.port, 8080);
    assert!(!config.debug);
}

#[test]
fn test_struct_with_defaults_override() {
    let value = obj(vec![
        ("host", str("localhost")),
        ("port", int(3000)),
        ("debug", bool_val(true)),
    ]);

    let config = <ConfigWithDefaults as FromValueTrait>::from_value(&value).unwrap();
    assert_eq!(config.host, "localhost");
    assert_eq!(config.port, 3000);
    assert!(config.debug);
}

#[test]
fn test_struct_with_rename() {
    let value = obj(vec![
        ("server_host", str("localhost")),
        ("server_port", int(8080)),
    ]);

    let config = <ConfigWithRename as FromValueTrait>::from_value(&value).unwrap();
    assert_eq!(config.host, "localhost");
    assert_eq!(config.port, 8080);
}

#[test]
fn test_struct_with_optional() {
    let value = obj(vec![("host", str("localhost"))]);

    let config = <ConfigWithOptional as FromValueTrait>::from_value(&value).unwrap();
    assert_eq!(config.host, "localhost");
    assert_eq!(config.port, None);
    assert_eq!(config.tags, None);
}

#[test]
fn test_struct_with_optional_present() {
    let value = obj(vec![
        ("host", str("localhost")),
        ("port", int(8080)),
        ("tags", arr(vec![str("web"), str("api")])),
    ]);

    let config = <ConfigWithOptional as FromValueTrait>::from_value(&value).unwrap();
    assert_eq!(config.host, "localhost");
    assert_eq!(config.port, Some(8080));
    assert_eq!(
        config.tags,
        Some(vec!["web".to_string(), "api".to_string()])
    );
}

#[test]
fn test_struct_with_skip() {
    let value = obj(vec![
        ("host", str("localhost")),
        ("port", int(8080)),
        ("runtime_only", str("should be ignored")),
    ]);

    let config = <ConfigWithSkip as FromValueTrait>::from_value(&value).unwrap();
    assert_eq!(config.host, "localhost");
    assert_eq!(config.port, 8080);
    assert_eq!(config.runtime_only, None);
}

#[test]
fn test_nested_struct() {
    let value = obj(vec![
        (
            "server",
            obj(vec![("host", str("localhost")), ("port", int(8080))]),
        ),
        (
            "database",
            obj(vec![
                ("host", str("db.example.com")),
                ("port", int(5432)),
                ("name", str("myapp")),
            ]),
        ),
    ]);

    let config = <NestedConfig as FromValueTrait>::from_value(&value).unwrap();
    assert_eq!(config.server.host, "localhost");
    assert_eq!(config.server.port, 8080);
    assert_eq!(config.database.host, "db.example.com");
    assert_eq!(config.database.port, 5432);
    assert_eq!(config.database.name, "myapp");
}

#[test]
fn test_tagged_enum_postgres() {
    let value = obj(vec![
        ("type", str("postgresql")),
        ("host", str("db.example.com")),
        ("port", int(5432)),
    ]);

    let backend = <Backend as FromValueTrait>::from_value(&value).unwrap();
    assert_eq!(
        backend,
        Backend::Postgres {
            host: "db.example.com".to_string(),
            port: 5432
        }
    );
}

#[test]
fn test_tagged_enum_sqlite() {
    let value = obj(vec![
        ("type", str("Sqlite")),
        ("path", str("/var/data/app.db")),
    ]);

    let backend = <Backend as FromValueTrait>::from_value(&value).unwrap();
    assert_eq!(
        backend,
        Backend::Sqlite {
            path: "/var/data/app.db".to_string()
        }
    );
}

#[test]
fn test_missing_required_field() {
    let value = obj(vec![("host", str("localhost"))]);

    let result = <SimpleConfig as FromValueTrait>::from_value(&value);
    assert!(result.is_err());
}

#[test]
fn test_invalid_type() {
    let value = obj(vec![
        ("host", str("localhost")),
        ("port", str("not a number")),
    ]);

    let result = <SimpleConfig as FromValueTrait>::from_value(&value);
    assert!(result.is_err());
}

#[tokio::test]
async fn test_config_extract_with_derive() {
    let config = Config::new(obj(vec![(
        "server",
        obj(vec![("host", str("localhost")), ("port", int(8080))]),
    )]));

    let server: ServerConfig = config.extract("server").unwrap();
    assert_eq!(server.host, "localhost");
    assert_eq!(server.port, 8080);
}
