use prefer::{Config, Error};
use std::path::PathBuf;

fn fixture_path(filename: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(filename)
}

#[tokio::test]
async fn test_load_json() {
    let config = Config::load_from_path(&fixture_path("test.json"))
        .await
        .unwrap();

    let name: String = config.get("app.name").await.unwrap();
    assert_eq!(name, "test-app");

    let port: u16 = config.get("server.port").await.unwrap();
    assert_eq!(port, 8080);

    let enabled: bool = config.get("database.enabled").await.unwrap();
    assert!(enabled);
}

#[tokio::test]
async fn test_load_yaml() {
    let config = Config::load_from_path(&fixture_path("test.yaml"))
        .await
        .unwrap();

    let name: String = config.get("app.name").await.unwrap();
    assert_eq!(name, "test-app");

    let port: u16 = config.get("server.port").await.unwrap();
    assert_eq!(port, 8080);
}

#[tokio::test]
async fn test_load_toml() {
    let config = Config::load_from_path(&fixture_path("test.toml"))
        .await
        .unwrap();

    let name: String = config.get("app.name").await.unwrap();
    assert_eq!(name, "test-app");

    let port: u16 = config.get("server.port").await.unwrap();
    assert_eq!(port, 8080);
}

#[tokio::test]
async fn test_nested_access() {
    let config = Config::load_from_path(&fixture_path("test.json"))
        .await
        .unwrap();

    let host: String = config.get("database.connection.host").await.unwrap();
    assert_eq!(host, "db.example.com");

    let port: u16 = config.get("database.connection.port").await.unwrap();
    assert_eq!(port, 5432);
}

#[tokio::test]
async fn test_key_not_found() {
    let config = Config::load_from_path(&fixture_path("test.json"))
        .await
        .unwrap();

    let result: Result<String, Error> = config.get("nonexistent.key").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_has_key() {
    let config = Config::load_from_path(&fixture_path("test.json"))
        .await
        .unwrap();

    assert!(config.has_key("app.name"));
    assert!(config.has_key("server.port"));
    assert!(config.has_key("database.connection.host"));
    assert!(!config.has_key("nonexistent"));
}

#[tokio::test]
async fn test_type_conversion() {
    let config = Config::load_from_path(&fixture_path("test.json"))
        .await
        .unwrap();

    let port_u16: u16 = config.get("server.port").await.unwrap();
    assert_eq!(port_u16, 8080);

    let port_u32: u32 = config.get("server.port").await.unwrap();
    assert_eq!(port_u32, 8080);

    let port_i32: i32 = config.get("server.port").await.unwrap();
    assert_eq!(port_i32, 8080);
}

#[tokio::test]
async fn test_load_json5() {
    let config = Config::load_from_path(&fixture_path("test.json5"))
        .await
        .unwrap();

    let name: String = config.get("app.name").await.unwrap();
    assert_eq!(name, "test-app");

    let port: u16 = config.get("server.port").await.unwrap();
    assert_eq!(port, 8080);
}

#[tokio::test]
async fn test_load_ini() {
    let config = Config::load_from_path(&fixture_path("test.ini"))
        .await
        .unwrap();

    let name: String = config.get("app.name").await.unwrap();
    assert_eq!(name, "test-app");

    let port: u16 = config.get("server.port").await.unwrap();
    assert_eq!(port, 8080);
}

#[tokio::test]
async fn test_load_xml() {
    let config = Config::load_from_path(&fixture_path("test.xml"))
        .await
        .unwrap();

    // XML parsing works differently, so just verify it loads
    assert!(config.has_key("app"));
    assert!(config.has_key("server"));
}

#[tokio::test]
async fn test_config_source_path() {
    let path = fixture_path("test.json");
    let config = Config::load_from_path(&path).await.unwrap();

    assert_eq!(config.source_path(), Some(&path));
}

#[tokio::test]
async fn test_config_data_access() {
    let config = Config::load_from_path(&fixture_path("test.json"))
        .await
        .unwrap();

    let data = config.data();
    assert!(data.is_object());
}

#[tokio::test]
async fn test_invalid_file_format() {
    let invalid_path = fixture_path("nonexistent.txt");
    let result = Config::load_from_path(&invalid_path).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_conversion_error() {
    let config = Config::load_from_path(&fixture_path("test.json"))
        .await
        .unwrap();

    let result: Result<bool, Error> = config.get("app.name").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_get_value_raw() {
    let config = Config::load_from_path(&fixture_path("test.json"))
        .await
        .unwrap();

    let value = config.get_value("app.name").unwrap();
    assert_eq!(value.as_str(), Some("test-app"));
}
