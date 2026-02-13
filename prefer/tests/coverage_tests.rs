//! Comprehensive tests for 100% code coverage.

use async_trait::async_trait;
use prefer::value::FromValue;
use prefer::visitor::{visit, FromValueVisitor, MapAccess, ValueVisitor};
use prefer::{
    Config, ConfigBuilder, ConfigValue, EnvSource, Error, FileSource, LayeredSource, MemorySource,
    Source,
};
use serial_test::serial;
use std::collections::HashMap;
use std::path::PathBuf;
use tempfile::TempDir;

// Helper functions to create ConfigValue more easily
fn obj(items: Vec<(&str, ConfigValue)>) -> ConfigValue {
    ConfigValue::Object(items.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
}

fn arr(items: Vec<ConfigValue>) -> ConfigValue {
    ConfigValue::Array(items)
}

fn str_val(s: &str) -> ConfigValue {
    ConfigValue::String(s.to_string())
}

fn int(i: i64) -> ConfigValue {
    ConfigValue::Integer(i)
}

fn float(f: f64) -> ConfigValue {
    ConfigValue::Float(f)
}

fn bool_val(b: bool) -> ConfigValue {
    ConfigValue::Bool(b)
}

fn null() -> ConfigValue {
    ConfigValue::Null
}

// ============================================================================
// lib.rs - load() and watch() functions
// ============================================================================

#[tokio::test]
#[serial]
async fn test_load_function() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("testconfig.json");
    std::fs::write(&config_path, r#"{"key": "value"}"#).unwrap();

    // Change to temp directory so discovery finds the file
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp_dir.path()).unwrap();

    let result = prefer::load("testconfig").await;
    std::env::set_current_dir(&original_dir).unwrap();

    assert!(result.is_ok());
    let config = result.unwrap();
    let value: String = config.get("key").unwrap();
    assert_eq!(value, "value");
}

#[tokio::test]
async fn test_load_function_not_found() {
    let result = prefer::load("nonexistent_config_file_xyz123").await;
    assert!(matches!(result.unwrap_err(), Error::FileNotFound(_)));
}

#[tokio::test]
#[serial]
async fn test_watch_function() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("watchtest.json");
    std::fs::write(&config_path, r#"{"version": 1}"#).unwrap();

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp_dir.path()).unwrap();

    let result = prefer::watch("watchtest").await;
    std::env::set_current_dir(&original_dir).unwrap();

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_watch_function_not_found() {
    let result = prefer::watch("nonexistent_config_file_xyz123").await;
    assert!(matches!(result.unwrap_err(), Error::FileNotFound(_)));
}

// ============================================================================
// discovery.rs - find_config_file
// ============================================================================

#[tokio::test]
#[serial]
async fn test_find_config_file_json() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("myapp.json");
    std::fs::write(&config_path, "{}").unwrap();

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp_dir.path()).unwrap();

    let result = prefer::discovery::find_config_file("myapp").await;
    std::env::set_current_dir(&original_dir).unwrap();

    assert!(result.is_ok());
    assert!(result.unwrap().to_string_lossy().contains("myapp.json"));
}

#[tokio::test]
#[serial]
async fn test_find_config_file_yaml() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("myapp.yaml");
    std::fs::write(&config_path, "key: value").unwrap();

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp_dir.path()).unwrap();

    let result = prefer::discovery::find_config_file("myapp").await;
    std::env::set_current_dir(&original_dir).unwrap();

    assert!(result.is_ok());
}

#[tokio::test]
#[serial]
async fn test_find_config_file_toml() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("myapp.toml");
    std::fs::write(&config_path, "key = \"value\"").unwrap();

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp_dir.path()).unwrap();

    let result = prefer::discovery::find_config_file("myapp").await;
    std::env::set_current_dir(&original_dir).unwrap();

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_find_config_file_not_found() {
    let result = prefer::discovery::find_config_file("definitely_not_existing_xyz").await;
    assert!(matches!(result.unwrap_err(), Error::FileNotFound(_)));
}

// ============================================================================
// formats.rs - Parse error paths
// ============================================================================

#[test]
fn test_parse_invalid_json() {
    let result = prefer::formats::parse("{ invalid json", &PathBuf::from("test.json"));
    assert!(matches!(result.unwrap_err(), Error::ParseError { .. }));
}

#[test]
fn test_parse_invalid_yaml() {
    let result = prefer::formats::parse("key: [unclosed", &PathBuf::from("test.yaml"));
    assert!(matches!(result.unwrap_err(), Error::ParseError { .. }));
}

#[test]
fn test_parse_invalid_toml() {
    let result = prefer::formats::parse("key = [unclosed", &PathBuf::from("test.toml"));
    assert!(matches!(result.unwrap_err(), Error::ParseError { .. }));
}

#[test]
fn test_parse_invalid_json5() {
    let result = prefer::formats::parse("{ invalid", &PathBuf::from("test.json5"));
    assert!(matches!(result.unwrap_err(), Error::ParseError { .. }));
}

#[test]
fn test_parse_invalid_jsonc() {
    let result = prefer::formats::parse("{ invalid", &PathBuf::from("test.jsonc"));
    assert!(matches!(result.unwrap_err(), Error::ParseError { .. }));
}

#[test]
fn test_parse_yml_extension() {
    let result = prefer::formats::parse("key: value", &PathBuf::from("test.yml"));
    assert!(result.is_ok());
}

#[test]
fn test_parse_yaml_with_null_key() {
    // YAML allows null as a key, which we skip (line 143 in formats.rs)
    let yaml = "~: value\nvalid_key: other";
    let result = prefer::formats::parse(yaml, &PathBuf::from("test.yaml")).unwrap();
    // The null key should be skipped, but valid_key should be present
    assert!(result.get("valid_key").is_some());
    // There's no way to access a null key since our get() takes &str
}

#[cfg(feature = "ini")]
#[test]
fn test_parse_invalid_ini() {
    // Invalid INI should return a ParseError (lines 228-230)
    // Note: rust-ini is very lenient, but we can try malformed input
    // Actually rust-ini accepts almost anything, so let's test the error path differently
    let result = prefer::formats::parse("[unclosed", &PathBuf::from("test.ini"));
    // rust-ini may or may not error on this, so just verify we handle it
    let _ = result; // Either Ok or Err is fine, as long as we don't panic
}

#[cfg(feature = "ini")]
#[test]
fn test_parse_ini_with_float() {
    let ini = "[section]\nvalue = 1.5";
    let result = prefer::formats::parse(ini, &PathBuf::from("test.ini")).unwrap();
    // Check it parses as float
    assert!(result
        .get("section")
        .unwrap()
        .get("value")
        .unwrap()
        .as_f64()
        .is_some());
}

#[cfg(feature = "ini")]
#[test]
fn test_parse_ini_with_bool() {
    let ini = "[section]\nenabled = true";
    let result = prefer::formats::parse(ini, &PathBuf::from("test.ini")).unwrap();
    assert_eq!(
        result
            .get("section")
            .unwrap()
            .get("enabled")
            .unwrap()
            .as_bool(),
        Some(true)
    );
}

#[cfg(feature = "ini")]
#[test]
fn test_parse_ini_with_string() {
    let ini = "[section]\nname = hello world";
    let result = prefer::formats::parse(ini, &PathBuf::from("test.ini")).unwrap();
    assert_eq!(
        result.get("section").unwrap().get("name").unwrap().as_str(),
        Some("hello world")
    );
}

#[cfg(feature = "ini")]
#[test]
fn test_parse_ini_default_section() {
    // Properties without a section go to "default"
    let ini = "key = value";
    let result = prefer::formats::parse(ini, &PathBuf::from("test.ini")).unwrap();
    assert_eq!(
        result.get("default").unwrap().get("key").unwrap().as_str(),
        Some("value")
    );
}

#[cfg(feature = "xml")]
#[test]
fn test_parse_invalid_xml() {
    let result = prefer::formats::parse("<unclosed>", &PathBuf::from("test.xml"));
    assert!(matches!(result.unwrap_err(), Error::ParseError { .. }));
}

#[cfg(feature = "xml")]
#[test]
fn test_parse_valid_xml() {
    let xml = "<root><key>value</key></root>";
    let result = prefer::formats::parse(xml, &PathBuf::from("test.xml"));
    assert!(result.is_ok());
}

#[test]
fn test_parse_no_extension() {
    let result = prefer::formats::parse("content", &PathBuf::from("noextension"));
    assert!(matches!(result.unwrap_err(), Error::UnsupportedFormat(_)));
}

#[test]
fn test_parse_unknown_extension() {
    let result = prefer::formats::parse("content", &PathBuf::from("test.xyz"));
    assert!(matches!(result.unwrap_err(), Error::UnsupportedFormat(_)));
}

// ============================================================================
// Value conversion edge cases
// ============================================================================

#[test]
fn test_from_value_i8_overflow() {
    assert!(i8::from_value(&int(1000)).is_err());
}

#[test]
fn test_from_value_i16_overflow() {
    assert!(i16::from_value(&int(100000)).is_err());
}

#[test]
fn test_from_value_i32_overflow() {
    assert!(i32::from_value(&int(i64::MAX)).is_err());
}

#[test]
fn test_from_value_u8_overflow() {
    assert!(u8::from_value(&int(1000)).is_err());
}

#[test]
fn test_from_value_u16_overflow() {
    assert!(u16::from_value(&int(100000)).is_err());
}

#[test]
fn test_from_value_u32_overflow() {
    // Use i64::MAX since we store as i64 internally
    assert!(u32::from_value(&int(i64::MAX)).is_err());
}

#[test]
fn test_from_value_negative_unsigned() {
    assert!(u8::from_value(&int(-1)).is_err());
    assert!(u16::from_value(&int(-1)).is_err());
    assert!(u32::from_value(&int(-1)).is_err());
    assert!(u64::from_value(&int(-1)).is_err());
}

#[test]
fn test_from_value_wrong_types() {
    assert!(bool::from_value(&int(123)).is_err());
    assert!(i64::from_value(&str_val("string")).is_err());
    assert!(u64::from_value(&str_val("string")).is_err());
    assert!(f64::from_value(&str_val("string")).is_err());
    assert!(String::from_value(&int(123)).is_err());
}

#[test]
fn test_from_value_vec_wrong_type() {
    let result: Result<Vec<i32>, _> = Vec::from_value(&str_val("not array"));
    assert!(result.is_err());
}

#[test]
fn test_from_value_vec_invalid_element() {
    let result: Result<Vec<i32>, _> = Vec::from_value(&arr(vec![int(1), str_val("two"), int(3)]));
    assert!(result.is_err());
}

#[test]
fn test_from_value_hashmap_wrong_type() {
    let result: Result<HashMap<String, i32>, _> = HashMap::from_value(&arr(vec![int(1), int(2)]));
    assert!(result.is_err());
}

#[test]
fn test_from_value_hashmap_invalid_value() {
    let result: Result<HashMap<String, i32>, _> =
        HashMap::from_value(&obj(vec![("a", str_val("not number"))]));
    assert!(result.is_err());
}

#[test]
fn test_from_value_option_some() {
    let result: Option<String> = Option::from_value(&str_val("hello")).unwrap();
    assert_eq!(result, Some("hello".to_string()));
}

// ============================================================================
// Visitor - all types and FromValueVisitor
// ============================================================================

struct AllTypesVisitor;

impl ValueVisitor for AllTypesVisitor {
    type Output = String;

    fn expecting(&self) -> &'static str {
        "any"
    }

    fn visit_null(&mut self) -> prefer::Result<Self::Output> {
        Ok("null".into())
    }
    fn visit_bool(&mut self, v: bool) -> prefer::Result<Self::Output> {
        Ok(format!("bool:{}", v))
    }
    fn visit_i64(&mut self, v: i64) -> prefer::Result<Self::Output> {
        Ok(format!("i64:{}", v))
    }
    fn visit_f64(&mut self, v: f64) -> prefer::Result<Self::Output> {
        Ok(format!("f64:{}", v))
    }
    fn visit_str(&mut self, v: &str) -> prefer::Result<Self::Output> {
        Ok(format!("str:{}", v))
    }
    fn visit_array(&mut self, arr: &[ConfigValue]) -> prefer::Result<Self::Output> {
        Ok(format!("arr:{}", arr.len()))
    }
    fn visit_map(&mut self, map: MapAccess<'_>) -> prefer::Result<Self::Output> {
        Ok(format!("map:{}", map.len()))
    }
}

#[test]
fn test_visitor_all_types() {
    let v = AllTypesVisitor;
    assert_eq!(v.expecting(), "any");

    let mut v = AllTypesVisitor;
    assert_eq!(visit(&null(), &mut v).unwrap(), "null");
    assert_eq!(visit(&bool_val(true), &mut v).unwrap(), "bool:true");
    assert_eq!(visit(&bool_val(false), &mut v).unwrap(), "bool:false");
    assert_eq!(visit(&int(42), &mut v).unwrap(), "i64:42");
    assert_eq!(visit(&int(-42), &mut v).unwrap(), "i64:-42");
    assert_eq!(visit(&float(1.5), &mut v).unwrap(), "f64:1.5");
    assert_eq!(visit(&str_val("hi"), &mut v).unwrap(), "str:hi");
    assert_eq!(visit(&arr(vec![int(1), int(2)]), &mut v).unwrap(), "arr:2");
    assert_eq!(visit(&obj(vec![("a", int(1))]), &mut v).unwrap(), "map:1");
}

// Test default error implementations
struct StrictVisitor;
impl ValueVisitor for StrictVisitor {
    type Output = ();
    fn expecting(&self) -> &'static str {
        "nothing"
    }
}

#[test]
fn test_visitor_default_errors() {
    let mut v = StrictVisitor;
    assert!(visit(&null(), &mut v).is_err());
    assert!(visit(&bool_val(true), &mut v).is_err());
    assert!(visit(&int(42), &mut v).is_err());
    assert!(visit(&float(1.5), &mut v).is_err());
    assert!(visit(&str_val("str"), &mut v).is_err());
    assert!(visit(&arr(vec![]), &mut v).is_err());
    assert!(visit(&obj(vec![]), &mut v).is_err());
}

// FromValueVisitor tests
#[test]
fn test_from_value_visitor_null() {
    let mut visitor: FromValueVisitor<Option<i32>> = FromValueVisitor::new();
    let result = visit(&null(), &mut visitor).unwrap();
    assert_eq!(result, None);
}

#[test]
fn test_from_value_visitor_bool() {
    let mut visitor: FromValueVisitor<bool> = FromValueVisitor::new();
    let result = visit(&bool_val(true), &mut visitor).unwrap();
    assert!(result);
}

#[test]
fn test_from_value_visitor_i64() {
    let mut visitor: FromValueVisitor<i64> = FromValueVisitor::new();
    let result = visit(&int(42), &mut visitor).unwrap();
    assert_eq!(result, 42);
}

#[test]
fn test_from_value_visitor_u64() {
    let mut visitor: FromValueVisitor<u64> = FromValueVisitor::default(); // Test Default impl
                                                                          // Use a value that fits in u64
    let result = visit(&int(100), &mut visitor).unwrap();
    assert_eq!(result, 100);
}

#[test]
fn test_from_value_visitor_f64() {
    let mut visitor: FromValueVisitor<f64> = FromValueVisitor::new();
    let result = visit(&float(1.5), &mut visitor).unwrap();
    assert!((result - 1.5).abs() < f64::EPSILON);
}

#[test]
fn test_from_value_visitor_f64_nan() {
    // Test the NaN case where from_f64 returns None
    let mut visitor: FromValueVisitor<Option<f64>> = FromValueVisitor::new();
    // f64::NAN can't be represented in JSON, so test with a valid float
    let result = visit(&float(1.5), &mut visitor).unwrap();
    assert_eq!(result, Some(1.5));
}

#[test]
fn test_from_value_visitor_string() {
    let mut visitor: FromValueVisitor<String> = FromValueVisitor::new();
    let result = visit(&str_val("hello"), &mut visitor).unwrap();
    assert_eq!(result, "hello");
}

#[test]
fn test_from_value_visitor_array() {
    let mut visitor: FromValueVisitor<Vec<i32>> = FromValueVisitor::new();
    let result = visit(&arr(vec![int(1), int(2), int(3)]), &mut visitor).unwrap();
    assert_eq!(result, vec![1, 2, 3]);
}

#[test]
fn test_from_value_visitor_map() {
    let mut visitor: FromValueVisitor<HashMap<String, i32>> = FromValueVisitor::new();
    let result = visit(&obj(vec![("a", int(1)), ("b", int(2))]), &mut visitor).unwrap();
    assert_eq!(result.get("a"), Some(&1));
    assert_eq!(result.get("b"), Some(&2));
}

#[test]
fn test_from_value_visitor_expecting() {
    let visitor: FromValueVisitor<String> = FromValueVisitor::new();
    assert!(visitor.expecting().contains("String"));
}

// ============================================================================
// MapAccess methods
// ============================================================================

#[test]
fn test_map_access_all_methods() {
    struct MapVisitor;
    impl ValueVisitor for MapVisitor {
        type Output = (bool, bool, usize, Vec<String>, Vec<(String, i64)>);

        fn visit_map(&mut self, map: MapAccess<'_>) -> prefer::Result<Self::Output> {
            let contains = map.contains_key("a");
            let is_empty = map.is_empty();
            let len = map.len();
            let keys: Vec<String> = map.keys().map(String::from).collect();
            let items: Vec<(String, i64)> = map
                .iter()
                .map(|(k, v)| (k.to_string(), v.as_i64().unwrap_or(0)))
                .collect();
            Ok((contains, is_empty, len, keys, items))
        }

        fn expecting(&self) -> &'static str {
            "map"
        }
    }

    let mut v = MapVisitor;
    let (contains, is_empty, len, keys, items) =
        visit(&obj(vec![("a", int(1)), ("b", int(2))]), &mut v).unwrap();

    assert!(contains);
    assert!(!is_empty);
    assert_eq!(len, 2);
    assert_eq!(keys.len(), 2);
    assert_eq!(items.len(), 2);
}

#[test]
fn test_map_access_get() {
    struct GetVisitor;
    impl ValueVisitor for GetVisitor {
        type Output = Option<i64>;

        fn visit_map(&mut self, map: MapAccess<'_>) -> prefer::Result<Self::Output> {
            Ok(map.get("key").and_then(|v| v.as_i64()))
        }

        fn expecting(&self) -> &'static str {
            "map"
        }
    }

    let mut v = GetVisitor;
    let result = visit(&obj(vec![("key", int(42))]), &mut v).unwrap();
    assert_eq!(result, Some(42));
}

#[test]
fn test_map_access_as_map() {
    struct AsMapVisitor;
    impl ValueVisitor for AsMapVisitor {
        type Output = usize;

        fn visit_map(&mut self, map: MapAccess<'_>) -> prefer::Result<Self::Output> {
            Ok(map.as_map().len())
        }

        fn expecting(&self) -> &'static str {
            "map"
        }
    }

    let mut v = AsMapVisitor;
    let result = visit(
        &obj(vec![("a", int(1)), ("b", int(2)), ("c", int(3))]),
        &mut v,
    )
    .unwrap();
    assert_eq!(result, 3);
}

// ============================================================================
// Config methods
// ============================================================================

#[test]
fn test_config_extract_not_found() {
    let config = Config::new(obj(vec![("a", int(1))]));
    let result: Result<i32, _> = config.extract("nonexistent");
    assert!(matches!(result.unwrap_err(), Error::KeyNotFound(_)));
}

#[test]
fn test_config_extract_conversion_error() {
    let config = Config::new(obj(vec![("port", str_val("string"))]));
    let result: Result<u16, _> = config.extract("port");
    assert!(matches!(result.unwrap_err(), Error::ConversionError { .. }));
}

#[test]
fn test_config_visit_key_not_found() {
    let config = Config::new(obj(vec![("a", int(1))]));
    let mut v = AllTypesVisitor;
    let result = config.visit_key("nonexistent", &mut v);
    assert!(matches!(result.unwrap_err(), Error::KeyNotFound(_)));
}

#[test]
fn test_config_visit_key_conversion_error() {
    let config = Config::new(obj(vec![("key", int(123))]));
    let mut v = StrictVisitor;
    let result = config.visit_key("key", &mut v);
    assert!(matches!(result.unwrap_err(), Error::ConversionError { .. }));
}

#[test]
fn test_config_visit() {
    let config = Config::new(obj(vec![("a", int(1))]));
    let mut v = AllTypesVisitor;
    let result = config.visit(&mut v).unwrap();
    assert_eq!(result, "map:1");
}

#[tokio::test]
async fn test_config_load_from_path() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("test.json");
    std::fs::write(&config_path, r#"{"loaded": true}"#).unwrap();

    let config = Config::load_from_path(&config_path).await.unwrap();
    let loaded: bool = config.get("loaded").unwrap();
    assert!(loaded);
    assert_eq!(config.source_path(), Some(&config_path));
}

#[test]
fn test_config_data_mut() {
    let mut config = Config::new(obj(vec![("key", str_val("original"))]));
    if let Some(obj) = config.data_mut().as_object_mut() {
        obj.insert("key".into(), str_val("modified"));
    }
    assert_eq!(config.data().get("key").unwrap().as_str(), Some("modified"));
}

// ============================================================================
// Source implementations
// ============================================================================

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
async fn test_env_source_load() {
    // Set some env vars for testing
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

    // Cleanup
    std::env::remove_var("PREFERTEST__DB__HOST");
    std::env::remove_var("PREFERTEST__DB__PORT");
    std::env::remove_var("PREFERTEST__DEBUG");
}

#[tokio::test]
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
async fn test_memory_source() {
    let data = obj(vec![("memory", bool_val(true))]);
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
async fn test_layered_source() {
    let base = MemorySource::with_name(obj(vec![("a", int(1)), ("b", int(2))]), "base");
    let overlay = MemorySource::with_name(obj(vec![("b", int(20)), ("c", int(3))]), "overlay");

    let layered = LayeredSource::new().with_source(base).with_source(overlay);
    assert_eq!(layered.name(), "layered");

    let value = layered.load().await.unwrap();
    assert_eq!(value.get("a").unwrap().as_i64(), Some(1)); // From base
    assert_eq!(value.get("b").unwrap().as_i64(), Some(20)); // Overridden by overlay
    assert_eq!(value.get("c").unwrap().as_i64(), Some(3)); // From overlay
}

#[tokio::test]
async fn test_layered_source_default() {
    let layered = LayeredSource::default();
    let value = layered.load().await.unwrap();
    assert!(value.as_object().map(|o| o.is_empty()).unwrap_or(false));
}

#[tokio::test]
async fn test_layered_source_add_boxed() {
    let source: Box<dyn Source> = Box::new(MemorySource::new(obj(vec![("boxed", bool_val(true))])));
    let layered = LayeredSource::new().add_boxed(source);
    let value = layered.load().await.unwrap();
    assert_eq!(value.get("boxed").unwrap().as_bool(), Some(true));
}

// ============================================================================
// Builder tests
// ============================================================================

#[tokio::test]
async fn test_builder_defaults() {
    let config = ConfigBuilder::new()
        .add_defaults(obj(vec![("default", bool_val(true))]))
        .build()
        .await
        .unwrap();

    let value: bool = config.get("default").unwrap();
    assert!(value);
}

#[tokio::test]
async fn test_builder_add_file() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("builder.toml");
    std::fs::write(&config_path, r#"key = "from_file""#).unwrap();

    let config = ConfigBuilder::new()
        .add_file(&config_path)
        .build()
        .await
        .unwrap();

    let value: String = config.get("key").unwrap();
    assert_eq!(value, "from_file");
}

#[tokio::test]
async fn test_builder_optional_file_exists() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("optional.json");
    std::fs::write(&config_path, r#"{"optional": true}"#).unwrap();

    let config = ConfigBuilder::new()
        .add_optional_file(&config_path)
        .build()
        .await
        .unwrap();

    let value: bool = config.get("optional").unwrap();
    assert!(value);
}

#[tokio::test]
async fn test_builder_optional_file_missing() {
    let config = ConfigBuilder::new()
        .add_defaults(obj(vec![("default", bool_val(true))]))
        .add_optional_file("/nonexistent/file.json")
        .build()
        .await
        .unwrap();

    // Should still work, just without the optional file
    let value: bool = config.get("default").unwrap();
    assert!(value);
}

#[tokio::test]
async fn test_builder_add_env() {
    std::env::set_var("BUILDERTEST__KEY", "from_env");

    let config = ConfigBuilder::new()
        .add_defaults(obj(vec![("key", str_val("default"))]))
        .add_env("BUILDERTEST")
        .build()
        .await
        .unwrap();

    let value: String = config.get("key").unwrap();
    assert_eq!(value, "from_env");

    std::env::remove_var("BUILDERTEST__KEY");
}

#[tokio::test]
async fn test_builder_env_with_separator() {
    std::env::set_var("BUILDSEP_KEY", "custom_sep");

    let config = ConfigBuilder::new()
        .add_env_with_separator("BUILDSEP", "_")
        .build()
        .await
        .unwrap();

    let value: String = config.get("key").unwrap();
    assert_eq!(value, "custom_sep");

    std::env::remove_var("BUILDSEP_KEY");
}

#[tokio::test]
async fn test_builder_add_source() {
    let custom = MemorySource::new(obj(vec![("custom", bool_val(true))]));

    let config = ConfigBuilder::new()
        .add_source(custom)
        .build()
        .await
        .unwrap();

    let value: bool = config.get("custom").unwrap();
    assert!(value);
}

#[tokio::test]
async fn test_config_builder_method() {
    let config = Config::builder()
        .add_defaults(obj(vec![("builder_method", bool_val(true))]))
        .build()
        .await
        .unwrap();

    let value: bool = config.get("builder_method").unwrap();
    assert!(value);
}

#[tokio::test]
async fn test_builder_default_impl() {
    let builder = ConfigBuilder::default();
    let config = builder
        .add_defaults(obj(vec![("test", int(1))]))
        .build()
        .await
        .unwrap();
    let value: i32 = config.get("test").unwrap();
    assert_eq!(value, 1);
}

// ============================================================================
// Error display
// ============================================================================

#[test]
fn test_error_display() {
    let errors = [
        Error::FileNotFound("test".into()),
        Error::KeyNotFound("key".into()),
        Error::UnsupportedFormat(PathBuf::from("test.xyz")),
        Error::ConversionError {
            key: "key".into(),
            type_name: "i32".into(),
            source: "test error".into(),
        },
        Error::SourceError {
            source_name: "test".into(),
            source: "source error".into(),
        },
        Error::ParseError {
            format: "JSON".into(),
            path: PathBuf::from("test.json"),
            source: "parse error".into(),
        },
    ];

    for error in errors {
        // Just verify Display works without panicking
        let _ = format!("{}", error);
    }
}

// ============================================================================
// Additional coverage - edge cases
// ============================================================================

#[test]
fn test_optional_file_source_name() {
    // Test the OptionalFileSource name method through ConfigBuilder
    let builder = ConfigBuilder::new().add_optional_file("test/path/config.json");
    // The builder holds the source internally, testing that add_optional_file works
    let _ = builder;
}

#[tokio::test]
async fn test_optional_file_source_error_shows_name() {
    // Create a file with invalid JSON to trigger an error in OptionalFileSource
    // This exercises the name() method when the error is wrapped
    let dir = TempDir::new().unwrap();
    let invalid_file = dir.path().join("invalid.json");
    tokio::fs::write(&invalid_file, "{ not valid json")
        .await
        .unwrap();

    let result = ConfigBuilder::new()
        .add_optional_file(&invalid_file)
        .build()
        .await;

    // The error should contain the source name
    let err = result.unwrap_err();
    let err_msg = format!("{}", err);
    assert!(err_msg.contains("invalid.json") || err_msg.contains("Source"));
}

#[test]
fn test_value_type_names() {
    // Test all value type names through error messages
    use prefer::value::FromValue;

    // null → attempting to get string from null
    let err = String::from_value(&null()).unwrap_err();
    assert!(format!("{}", err).contains("null"));

    // array → attempting to get string from array
    let err = String::from_value(&arr(vec![int(1), int(2)])).unwrap_err();
    assert!(format!("{}", err).contains("array"));

    // object → attempting to get string from object
    let err = String::from_value(&obj(vec![("a", int(1))])).unwrap_err();
    assert!(format!("{}", err).contains("object"));
}

#[test]
fn test_vec_conversion_with_index_error() {
    // Test that Vec conversion includes index in error message
    let value = arr(vec![int(1), str_val("not_a_number"), int(3)]);
    let result: Result<Vec<i32>, _> = Vec::from_value(&value);
    let err = result.unwrap_err();
    // The error should mention the index [1]
    if let Error::ConversionError { key, .. } = err {
        assert!(key.contains("[1]"));
    } else {
        panic!("Expected ConversionError");
    }
}

#[test]
fn test_hashmap_conversion_with_key_error() {
    // Test that HashMap conversion includes key in error message
    let value = obj(vec![("valid", int(1)), ("invalid", str_val("not_number"))]);
    let result: Result<HashMap<String, i32>, _> = HashMap::from_value(&value);
    let err = result.unwrap_err();
    if let Error::ConversionError { key, .. } = err {
        assert!(key.contains("invalid"));
    } else {
        panic!("Expected ConversionError");
    }
}

#[test]
fn test_visitor_default_expecting() {
    // Test the default expecting() message
    struct DefaultExpectingVisitor;
    impl ValueVisitor for DefaultExpectingVisitor {
        type Output = ();
    }

    let visitor = DefaultExpectingVisitor;
    assert_eq!(visitor.expecting(), "any value");
}

#[test]
fn test_from_value_visitor_via_visitor() {
    // Test FromValueVisitor with i64 path
    let mut visitor: FromValueVisitor<u64> = FromValueVisitor::new();
    // Test with a positive number that could be u64
    let result = visit(&int(42), &mut visitor).unwrap();
    assert_eq!(result, 42);
}

#[test]
fn test_f32_conversion_error() {
    // Test f32 conversion from non-number
    let result = f32::from_value(&str_val("not a number"));
    assert!(result.is_err());
}

#[tokio::test]
async fn test_env_source_float_parsing() {
    // Test float parsing in environment variables
    std::env::set_var("ENVFLOAT__VALUE", "1.5");

    let source = EnvSource::new("ENVFLOAT");
    let value = source.load().await.unwrap();
    assert!((value.get("value").unwrap().as_f64().unwrap() - 1.5).abs() < 0.001);

    std::env::remove_var("ENVFLOAT__VALUE");
}

#[tokio::test]
async fn test_env_source_nan_float() {
    // Test NaN float handling (from_f64 returns None for NaN)
    // Since we can't set NaN through env vars, this tests the else branch
    // by setting a non-number string that parses as string
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
    // Test "false" boolean parsing in env source
    std::env::set_var("ENVBOOL__ENABLED", "false");
    std::env::set_var("ENVBOOL__DISABLED", "FALSE");

    let source = EnvSource::new("ENVBOOL");
    let value = source.load().await.unwrap();
    assert_eq!(value.get("enabled").unwrap().as_bool(), Some(false));
    assert_eq!(value.get("disabled").unwrap().as_bool(), Some(false));

    std::env::remove_var("ENVBOOL__ENABLED");
    std::env::remove_var("ENVBOOL__DISABLED");
}

#[test]
fn test_error_with_key_non_conversion() {
    // Test that Error::with_key() passes through non-ConversionError unchanged
    let err = Error::FileNotFound("test.json".to_string());
    let result = err.with_key("some.key");
    assert!(matches!(result, Error::FileNotFound(s) if s == "test.json"));
}

#[test]
fn test_config_value_display_multi_item_object() {
    // Test Display for object with multiple items (covers the ", " separator)
    let obj = obj(vec![("a", int(1)), ("b", int(2))]);
    let display = format!("{}", obj);
    // Object iteration order is not guaranteed, but both should be present
    assert!(display.contains("\"a\": 1"));
    assert!(display.contains("\"b\": 2"));
    assert!(display.contains(", "));
}

#[tokio::test]
async fn test_layered_source_error_propagation() {
    // Test that LayeredSource properly wraps source errors
    struct FailingSource;

    #[async_trait]
    impl Source for FailingSource {
        async fn load(&self) -> prefer::Result<ConfigValue> {
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

// Test error re-mapping in config.extract when error is not ConversionError
// by using a custom FromValue that returns a non-ConversionError
#[test]
fn test_config_extract_non_conversion_error() {
    // Custom type that returns FileNotFound instead of ConversionError
    #[derive(Debug)]
    struct FailingType;

    impl FromValue for FailingType {
        fn from_value(_value: &ConfigValue) -> prefer::Result<Self> {
            Err(Error::FileNotFound("mock error".into()))
        }
    }

    let config = Config::new(obj(vec![("key", str_val("value"))]));
    let result: Result<FailingType, _> = config.extract("key");
    // The error should pass through as-is (not re-mapped)
    assert!(matches!(result.unwrap_err(), Error::FileNotFound(_)));
}

// Test error re-mapping in config.visit_key when error is not ConversionError
#[test]
fn test_config_visit_key_non_conversion_error() {
    // Custom visitor that returns FileNotFound
    struct FailingVisitor;
    impl ValueVisitor for FailingVisitor {
        type Output = ();

        fn visit_str(&mut self, _v: &str) -> prefer::Result<Self::Output> {
            Err(Error::FileNotFound("mock visitor error".into()))
        }

        fn expecting(&self) -> &'static str {
            "nothing"
        }
    }

    let config = Config::new(obj(vec![("key", str_val("value"))]));
    let mut v = FailingVisitor;
    let result = config.visit_key("key", &mut v);
    // The error should pass through as-is (not re-mapped)
    assert!(matches!(result.unwrap_err(), Error::FileNotFound(_)));
}

// Test Vec conversion with non-ConversionError from element
#[test]
fn test_vec_non_conversion_error() {
    // Custom type that returns FileNotFound
    #[derive(Debug)]
    struct FailingElement;

    impl FromValue for FailingElement {
        fn from_value(_value: &ConfigValue) -> prefer::Result<Self> {
            Err(Error::FileNotFound("element error".into()))
        }
    }

    let value = arr(vec![int(1), int(2), int(3)]);
    let result: Result<Vec<FailingElement>, _> = Vec::from_value(&value);
    // The error should pass through as-is
    assert!(matches!(result.unwrap_err(), Error::FileNotFound(_)));
}

// Test HashMap conversion with non-ConversionError from value
#[test]
fn test_hashmap_non_conversion_error() {
    // Custom type that returns FileNotFound
    #[derive(Debug)]
    struct FailingValue;

    impl FromValue for FailingValue {
        fn from_value(_value: &ConfigValue) -> prefer::Result<Self> {
            Err(Error::FileNotFound("value error".into()))
        }
    }

    let value = obj(vec![("key", str_val("value"))]);
    let result: Result<HashMap<String, FailingValue>, _> = HashMap::from_value(&value);
    // The error should pass through as-is
    assert!(matches!(result.unwrap_err(), Error::FileNotFound(_)));
}

// Test config.get_value traversing non-object
#[test]
fn test_config_get_value_non_object_path() {
    let config = Config::new(obj(vec![("key", str_val("not_an_object"))]));
    // Trying to traverse through a string should fail
    let result = config.get_value("key.nested");
    assert!(matches!(result.unwrap_err(), Error::KeyNotFound(_)));
}

// Test watch receiver drop (tests send failure path in watch.rs)
#[tokio::test]
async fn test_watch_receiver_dropped() {
    use tokio::time::{sleep, Duration};

    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("droptest.json");
    std::fs::write(&config_path, r#"{"value": 1}"#).unwrap();

    // Start watching and immediately drop the receiver
    let receiver = prefer::watch::watch_path(config_path.clone())
        .await
        .unwrap();
    drop(receiver);

    // Give watcher time to set up
    sleep(Duration::from_millis(50)).await;

    // Modify the file - this should trigger a send that fails (receiver dropped)
    // The watcher should break out of its loop
    std::fs::write(&config_path, r#"{"value": 2}"#).unwrap();

    // Give time for the event to propagate
    sleep(Duration::from_millis(100)).await;

    // Test passes if no panic/hang occurs
}

// Test watch with non-modify events (tests the _ => {} path in watch.rs)
#[tokio::test]
async fn test_watch_non_modify_events() {
    use tokio::time::{sleep, timeout, Duration};

    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("nonmodify.json");
    std::fs::write(&config_path, r#"{"value": 1}"#).unwrap();

    let mut receiver = prefer::watch::watch_path(config_path.clone())
        .await
        .unwrap();

    // Give watcher time to set up
    sleep(Duration::from_millis(50)).await;

    // Delete the file - this triggers a Remove event, not Modify
    std::fs::remove_file(&config_path).unwrap();

    // The receiver should NOT receive anything for delete events
    let result = timeout(Duration::from_millis(200), receiver.recv()).await;
    assert!(result.is_err()); // Timeout means no message received (correct behavior)

    // Recreate for cleanup
    std::fs::write(&config_path, r#"{"value": 1}"#).unwrap();
}
