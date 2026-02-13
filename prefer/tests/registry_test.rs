//! Tests for the registry-based load/watch pipeline.

use prefer::loader::file::FileLoader;
use prefer::loader::Loader;
use prefer::registry;
use serial_test::serial;
use std::io::Write;
use tempfile::TempDir;

#[test]
fn test_file_loader_provides_bare_names() {
    let loader = FileLoader::new();
    assert!(loader.provides("myapp"));
    assert!(loader.provides("settings"));
    assert!(loader.provides("config"));
}

#[test]
fn test_file_loader_rejects_db_urls() {
    let loader = FileLoader::new();
    assert!(!loader.provides("postgres://localhost/mydb"));
    assert!(!loader.provides("sqlite:///tmp/db.sqlite"));
    assert!(!loader.provides("mysql://root@localhost/db"));
}

#[test]
fn test_find_loader_returns_file_loader_for_bare_name() {
    let loader = registry::find_loader("myapp");
    assert!(loader.is_some());
    assert_eq!(loader.unwrap().name(), "file");
}

#[test]
fn test_find_loader_returns_none_for_unknown_scheme() {
    // No loader registered for postgres:// in prefer itself
    let loader = registry::find_loader("postgres://localhost/db");
    assert!(loader.is_none());
}

#[test]
fn test_find_formatter_by_extension() {
    let fmt = registry::find_formatter("config.json");
    assert!(fmt.is_some());
    assert_eq!(fmt.unwrap().name(), "json");

    let fmt = registry::find_formatter("config.yaml");
    assert!(fmt.is_some());
    assert_eq!(fmt.unwrap().name(), "yaml");

    let fmt = registry::find_formatter("config.yml");
    assert!(fmt.is_some());
    assert_eq!(fmt.unwrap().name(), "yaml");

    let fmt = registry::find_formatter("config.toml");
    assert!(fmt.is_some());
    assert_eq!(fmt.unwrap().name(), "toml");

    let fmt = registry::find_formatter("config.ini");
    assert!(fmt.is_some());
    assert_eq!(fmt.unwrap().name(), "ini");

    let fmt = registry::find_formatter("config.xml");
    assert!(fmt.is_some());
    assert_eq!(fmt.unwrap().name(), "xml");
}

#[test]
fn test_find_formatter_returns_none_for_unknown() {
    let fmt = registry::find_formatter("config.bson");
    assert!(fmt.is_none());

    let fmt = registry::find_formatter("no_extension");
    assert!(fmt.is_none());
}

#[test]
fn test_find_formatter_by_hint() {
    let fmt = registry::find_formatter_by_hint("json");
    assert!(fmt.is_some());
    assert_eq!(fmt.unwrap().name(), "json");

    let fmt = registry::find_formatter_by_hint("toml");
    assert!(fmt.is_some());
    assert_eq!(fmt.unwrap().name(), "toml");

    let fmt = registry::find_formatter_by_hint("yaml");
    assert!(fmt.is_some());
    assert_eq!(fmt.unwrap().name(), "yaml");
}

#[test]
fn test_find_formatter_by_hint_returns_none_for_unknown() {
    let fmt = registry::find_formatter_by_hint("bson");
    assert!(fmt.is_none());
}

#[tokio::test]
#[serial]
async fn test_load_routes_to_file_loader() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("testcfg.json");
    let mut file = std::fs::File::create(&file_path).unwrap();
    writeln!(file, r#"{{"host": "localhost", "port": 8080}}"#).unwrap();

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp_dir.path()).unwrap();

    let config = prefer::load("testcfg").await.unwrap();

    let host: String = config.get("host").unwrap();
    assert_eq!(host, "localhost");

    let port: u16 = config.get("port").unwrap();
    assert_eq!(port, 8080);

    // Verify metadata was populated
    assert_eq!(config.loader_name(), Some("file"));
    assert_eq!(config.formatter_name(), Some("json"));
    assert!(config.source().is_some());

    std::env::set_current_dir(original_dir).unwrap();
}

#[tokio::test]
#[serial]
async fn test_load_toml_routes_correctly() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("myapp.toml");
    let mut file = std::fs::File::create(&file_path).unwrap();
    writeln!(file, r#"name = "test""#).unwrap();

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp_dir.path()).unwrap();

    let config = prefer::load("myapp").await.unwrap();

    let name: String = config.get("name").unwrap();
    assert_eq!(name, "test");
    assert_eq!(config.formatter_name(), Some("toml"));

    std::env::set_current_dir(original_dir).unwrap();
}

#[tokio::test]
async fn test_load_no_loader_found() {
    let result = prefer::load("postgres://localhost/db").await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(err, prefer::Error::NoLoaderFound(_)));
}

#[tokio::test]
async fn test_watch_no_loader_found() {
    let result = prefer::watch("postgres://localhost/db").await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(err, prefer::Error::NoLoaderFound(_)));
}

#[test]
fn test_formatter_deserialize_serialize_roundtrip() {
    let fmt = registry::find_formatter("test.json").unwrap();
    let data = fmt.deserialize(r#"{"key": "value", "num": 42}"#).unwrap();
    let serialized = fmt.serialize(&data).unwrap();
    let restored = fmt.deserialize(&serialized).unwrap();
    assert_eq!(data, restored);
}

#[test]
fn test_config_set_and_get() {
    let mut config = prefer::Config::new(prefer::ConfigValue::Object(Default::default()));

    config.set(
        "server.host",
        prefer::ConfigValue::String("localhost".into()),
    );
    config.set("server.port", prefer::ConfigValue::Integer(8080));

    let host: String = config.get("server.host").unwrap();
    assert_eq!(host, "localhost");

    let port: u16 = config.get("server.port").unwrap();
    assert_eq!(port, 8080);
}

#[test]
fn test_config_on_change_fires() {
    let mut config = prefer::Config::new(prefer::ConfigValue::Object(Default::default()));

    let changes = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let changes_clone = changes.clone();

    config.on_change(Box::new(move |key, _value, _prev| {
        changes_clone.lock().unwrap().push(key.to_string());
    }));

    config.set("a", prefer::ConfigValue::Integer(1));
    config.set("b.c", prefer::ConfigValue::String("hello".into()));

    let log = changes.lock().unwrap();
    assert_eq!(log.len(), 2);
    assert_eq!(log[0], "a");
    assert_eq!(log[1], "b.c");
}

#[tokio::test]
#[serial]
async fn test_load_yaml_routes_correctly() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("settings.yaml");
    let mut file = std::fs::File::create(&file_path).unwrap();
    writeln!(file, "host: localhost\nport: 3000").unwrap();

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp_dir.path()).unwrap();

    let config = prefer::load("settings").await.unwrap();

    let host: String = config.get("host").unwrap();
    assert_eq!(host, "localhost");
    assert_eq!(config.formatter_name(), Some("yaml"));
    assert_eq!(config.loader_name(), Some("file"));

    std::env::set_current_dir(original_dir).unwrap();
}

#[tokio::test]
#[serial]
async fn test_load_file_not_found() {
    let temp_dir = TempDir::new().unwrap();
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp_dir.path()).unwrap();

    let result = prefer::load("nonexistent_config").await;
    assert!(result.is_err());

    std::env::set_current_dir(original_dir).unwrap();
}

#[test]
fn test_formatter_toml_roundtrip() {
    let fmt = registry::find_formatter("test.toml").unwrap();
    let data = fmt.deserialize("name = \"test\"\nport = 8080").unwrap();
    let serialized = fmt.serialize(&data).unwrap();
    let restored = fmt.deserialize(&serialized).unwrap();
    assert_eq!(data, restored);
}

#[test]
fn test_formatter_yaml_roundtrip() {
    let fmt = registry::find_formatter("test.yaml").unwrap();
    let data = fmt.deserialize("name: test\nport: 8080").unwrap();
    let serialized = fmt.serialize(&data).unwrap();
    let restored = fmt.deserialize(&serialized).unwrap();
    assert_eq!(data, restored);
}

#[test]
fn test_find_formatter_by_hint_ini() {
    let fmt = registry::find_formatter_by_hint("ini");
    assert!(fmt.is_some());
    assert_eq!(fmt.unwrap().name(), "ini");
}

#[test]
fn test_find_formatter_by_hint_xml() {
    let fmt = registry::find_formatter_by_hint("xml");
    assert!(fmt.is_some());
    assert_eq!(fmt.unwrap().name(), "xml");
}

#[test]
fn test_file_loader_provides_file_url() {
    let loader = FileLoader::new();
    assert!(loader.provides("file:///etc/myapp.toml"));
    assert!(loader.provides("file://config.json"));
}

#[test]
fn test_error_display_messages() {
    let err = prefer::Error::NoLoaderFound("redis://host".into());
    assert!(err.to_string().contains("redis://host"));

    let err = prefer::Error::NoFormatterFound("config.bson".into());
    assert!(err.to_string().contains("config.bson"));

    let err = prefer::Error::WatchNotSupported("scheme://x".into());
    assert!(err.to_string().contains("scheme://x"));
}
