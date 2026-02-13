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

fn config_value_to_json(value: &ConfigValue) -> String {
    match value {
        ConfigValue::Null => "null".to_string(),
        ConfigValue::Bool(b) => b.to_string(),
        ConfigValue::Integer(i) => i.to_string(),
        ConfigValue::Float(f) => f.to_string(),
        ConfigValue::String(s) => format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"")),
        ConfigValue::Array(arr) => {
            let items: Vec<String> = arr.iter().map(config_value_to_json).collect();
            format!("[{}]", items.join(","))
        }
        ConfigValue::Object(map) => {
            let entries: Vec<String> = map
                .iter()
                .map(|(k, v)| {
                    format!(
                        "\"{}\":{}",
                        k.replace('\\', "\\\\").replace('"', "\\\""),
                        config_value_to_json(v)
                    )
                })
                .collect();
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
}
