//! JSON and JSON5 format support.

use crate::error::{Error, Result};
use crate::formatter::{extension_matches, Formatter};
use crate::registry::RegisteredFormatter;
use crate::value::ConfigValue;
use std::collections::HashMap;

inventory::submit! { RegisteredFormatter(&JsonFormatter) }

/// Formatter for JSON, JSON5, and JSONC files.
///
/// Uses the `jzon` crate (no serde dependency). JSON5/JSONC support is
/// best-effort — `jzon` handles many JSON5 cases but is not a full parser.
pub struct JsonFormatter;

impl Formatter for JsonFormatter {
    fn provides(&self, identifier: &str) -> bool {
        extension_matches(identifier, self.extensions())
    }

    fn extensions(&self) -> &[&str] {
        &["json", "json5", "jsonc"]
    }

    fn deserialize(&self, content: &str) -> Result<ConfigValue> {
        let value = jzon::parse(content).map_err(|e| Error::ParseError {
            format: "JSON".to_string(),
            path: std::path::PathBuf::from("<content>"),
            source: e.to_string().into(),
        })?;
        Ok(jzon_to_config_value(value))
    }

    fn serialize(&self, value: &ConfigValue) -> Result<String> {
        Ok(config_value_to_json(value))
    }

    fn name(&self) -> &str {
        "json"
    }
}

fn jzon_to_config_value(value: jzon::JsonValue) -> ConfigValue {
    use jzon::JsonValue;

    match &value {
        JsonValue::Null => ConfigValue::Null,
        JsonValue::Boolean(b) => ConfigValue::Bool(*b),
        JsonValue::Number(_) => {
            if let Some(i) = value.as_i64() {
                ConfigValue::Integer(i)
            } else if let Some(f) = value.as_f64() {
                ConfigValue::Float(f)
            } else {
                unreachable!("jzon Number should always be parseable as i64 or f64")
            }
        }
        JsonValue::Short(s) => ConfigValue::String(s.to_string()),
        JsonValue::String(s) => ConfigValue::String(s.clone()),
        JsonValue::Array(arr) => {
            ConfigValue::Array(arr.iter().cloned().map(jzon_to_config_value).collect())
        }
        JsonValue::Object(obj) => {
            let map: HashMap<String, ConfigValue> = obj
                .iter()
                .map(|(k, v)| (k.to_string(), jzon_to_config_value(v.clone())))
                .collect();
            ConfigValue::Object(map)
        }
    }
}

fn json_entry((k, v): (&String, &ConfigValue)) -> String {
    format!(
        "\"{}\":{}",
        super::escape_quotes(k),
        config_value_to_json(v)
    )
}

fn config_value_to_json(value: &ConfigValue) -> String {
    match value {
        ConfigValue::Null => "null".to_string(),
        ConfigValue::Bool(b) => b.to_string(),
        ConfigValue::Integer(i) => i.to_string(),
        ConfigValue::Float(f) => f.to_string(),
        ConfigValue::String(s) => format!("\"{}\"", super::escape_quotes(s)),
        ConfigValue::Array(arr) => {
            let items: Vec<String> = arr.iter().map(config_value_to_json).collect();
            format!("[{}]", items.join(","))
        }
        ConfigValue::Object(map) => {
            let entries: Vec<String> = map.iter().map(json_entry).collect();
            format!("{{{}}}", entries.join(","))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provides() {
        let f = JsonFormatter;
        assert!(f.provides("config.json"));
        assert!(f.provides("config.json5"));
        assert!(f.provides("config.jsonc"));
        assert!(!f.provides("config.toml"));
        assert!(!f.provides("config"));
    }

    #[test]
    fn test_deserialize_object() {
        let f = JsonFormatter;
        let result = f.deserialize(r#"{"name": "test", "port": 8080}"#).unwrap();
        assert_eq!(result.get("name").unwrap().as_str(), Some("test"));
        assert_eq!(result.get("port").unwrap().as_i64(), Some(8080));
    }

    #[test]
    fn test_deserialize_array() {
        let f = JsonFormatter;
        let result = f.deserialize("[1, 2, 3]").unwrap();
        assert_eq!(result.as_array().unwrap().len(), 3);
    }

    #[test]
    fn test_deserialize_error() {
        let f = JsonFormatter;
        assert!(f.deserialize("{invalid}").is_err());
    }

    #[test]
    fn test_serialize_roundtrip() {
        let f = JsonFormatter;
        let original = f.deserialize(r#"{"key": "value"}"#).unwrap();
        let serialized = f.serialize(&original).unwrap();
        let restored = f.deserialize(&serialized).unwrap();
        assert_eq!(original, restored);
    }

    #[test]
    fn test_serialize_all_types() {
        let f = JsonFormatter;
        assert_eq!(f.serialize(&ConfigValue::Null).unwrap(), "null");
        assert_eq!(f.serialize(&ConfigValue::Bool(true)).unwrap(), "true");
        assert_eq!(f.serialize(&ConfigValue::Integer(42)).unwrap(), "42");
        assert_eq!(f.serialize(&ConfigValue::Float(1.5)).unwrap(), "1.5");
        assert_eq!(
            f.serialize(&ConfigValue::String("hi".into())).unwrap(),
            "\"hi\""
        );
    }

    #[test]
    fn test_serialize_string_escaping() {
        let f = JsonFormatter;
        assert_eq!(
            f.serialize(&ConfigValue::String("say \"hi\"".into()))
                .unwrap(),
            "\"say \\\"hi\\\"\""
        );
        assert_eq!(
            f.serialize(&ConfigValue::String("back\\slash".into()))
                .unwrap(),
            "\"back\\\\slash\""
        );
    }

    #[test]
    fn test_serialize_array() {
        let f = JsonFormatter;
        let arr = ConfigValue::Array(vec![
            ConfigValue::Integer(1),
            ConfigValue::Bool(true),
            ConfigValue::Null,
        ]);
        assert_eq!(f.serialize(&arr).unwrap(), "[1,true,null]");
    }

    #[test]
    fn test_serialize_object() {
        let f = JsonFormatter;
        let mut map = HashMap::new();
        map.insert("key".to_string(), ConfigValue::String("value".into()));
        let obj = ConfigValue::Object(map);
        assert_eq!(f.serialize(&obj).unwrap(), "{\"key\":\"value\"}");
    }

    #[test]
    fn test_serialize_object_key_escaping() {
        let f = JsonFormatter;
        let mut map = HashMap::new();
        map.insert("k\"ey".to_string(), ConfigValue::Integer(1));
        let obj = ConfigValue::Object(map);
        let serialized = f.serialize(&obj).unwrap();
        assert!(serialized.contains("\"k\\\"ey\""));
    }

    #[test]
    fn test_deserialize_null() {
        let f = JsonFormatter;
        let result = f.deserialize("null").unwrap();
        assert!(matches!(result, ConfigValue::Null));
    }

    #[test]
    fn test_deserialize_boolean() {
        let f = JsonFormatter;
        let result = f.deserialize(r#"{"flag": true}"#).unwrap();
        assert_eq!(result.get("flag").unwrap().as_bool(), Some(true));

        let result = f.deserialize(r#"{"flag": false}"#).unwrap();
        assert_eq!(result.get("flag").unwrap().as_bool(), Some(false));
    }

    #[test]
    fn test_deserialize_long_string() {
        let f = JsonFormatter;
        // jzon uses String (heap) for strings longer than ~30 chars
        let long = "a]".repeat(50);
        let json = format!(r#"{{"val": "{}"}}"#, long);
        let result = f.deserialize(&json).unwrap();
        assert_eq!(result.get("val").unwrap().as_str(), Some(long.as_str()));
    }

    #[test]
    fn test_deserialize_float() {
        let f = JsonFormatter;
        let result = f.deserialize("3.15").unwrap();
        assert_eq!(result.as_f64(), Some(3.15));
    }

    #[test]
    fn test_deserialize_nested_object() {
        let f = JsonFormatter;
        let result = f.deserialize(r#"{"a": {"b": {"c": 1}}}"#).unwrap();
        assert_eq!(
            result
                .get("a")
                .unwrap()
                .get("b")
                .unwrap()
                .get("c")
                .unwrap()
                .as_i64(),
            Some(1)
        );
    }
}
