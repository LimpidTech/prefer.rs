//! TOML format support.

use crate::error::{Error, Result};
use crate::formatter::{extension_matches, Formatter};
use crate::registry::RegisteredFormatter;
use crate::value::ConfigValue;
use std::collections::HashMap;

inventory::submit! { RegisteredFormatter(&TomlFormatter) }

/// Formatter for TOML files.
///
/// Uses the `toml_edit` crate (no serde dependency). Handles datetimes
/// by converting to strings, and supports inline tables and array-of-tables.
pub struct TomlFormatter;

impl Formatter for TomlFormatter {
    fn provides(&self, identifier: &str) -> bool {
        extension_matches(identifier, self.extensions())
    }

    fn extensions(&self) -> &[&str] {
        &["toml"]
    }

    fn deserialize(&self, content: &str) -> Result<ConfigValue> {
        use toml_edit::DocumentMut;

        let doc: DocumentMut =
            content
                .parse()
                .map_err(|e: toml_edit::TomlError| Error::ParseError {
                    format: "TOML".to_string(),
                    path: std::path::PathBuf::from("<content>"),
                    source: e.to_string().into(),
                })?;

        Ok(toml_item_to_config_value(doc.as_item()))
    }

    fn serialize(&self, value: &ConfigValue) -> Result<String> {
        Ok(config_value_to_toml(value, ""))
    }

    fn name(&self) -> &str {
        "toml"
    }
}

fn toml_item_to_config_value(item: &toml_edit::Item) -> ConfigValue {
    use toml_edit::Item;

    match item {
        Item::None => unreachable!("Item::None should not occur when iterating parsed TOML"),
        Item::Value(v) => toml_value_to_config_value(v),
        Item::Table(t) => {
            let map: HashMap<String, ConfigValue> = t
                .iter()
                .map(|(k, v)| (k.to_string(), toml_item_to_config_value(v)))
                .collect();
            ConfigValue::Object(map)
        }
        Item::ArrayOfTables(arr) => ConfigValue::Array(
            arr.iter()
                .map(|t| {
                    let map: HashMap<String, ConfigValue> = t
                        .iter()
                        .map(|(k, v)| (k.to_string(), toml_item_to_config_value(v)))
                        .collect();
                    ConfigValue::Object(map)
                })
                .collect(),
        ),
    }
}

fn toml_value_to_config_value(value: &toml_edit::Value) -> ConfigValue {
    use toml_edit::Value;

    match value {
        Value::String(s) => ConfigValue::String(s.value().to_string()),
        Value::Integer(i) => ConfigValue::Integer(*i.value()),
        Value::Float(f) => ConfigValue::Float(*f.value()),
        Value::Boolean(b) => ConfigValue::Bool(*b.value()),
        Value::Datetime(dt) => ConfigValue::String(dt.to_string()),
        Value::Array(arr) => {
            ConfigValue::Array(arr.iter().map(toml_value_to_config_value).collect())
        }
        Value::InlineTable(t) => {
            let map: HashMap<String, ConfigValue> = t
                .iter()
                .map(|(k, v)| (k.to_string(), toml_value_to_config_value(v)))
                .collect();
            ConfigValue::Object(map)
        }
    }
}

fn config_value_to_toml(value: &ConfigValue, key_prefix: &str) -> String {
    match value {
        ConfigValue::Null => "\"\"".to_string(),
        ConfigValue::Bool(b) => b.to_string(),
        ConfigValue::Integer(i) => i.to_string(),
        ConfigValue::Float(f) => f.to_string(),
        ConfigValue::String(s) => format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"")),
        ConfigValue::Array(arr) => {
            let items: Vec<String> = arr
                .iter()
                .map(|v| config_value_to_toml(v, key_prefix))
                .collect();
            format!("[{}]", items.join(", "))
        }
        ConfigValue::Object(map) => {
            let mut lines = Vec::new();
            let mut tables = Vec::new();

            for (k, v) in map {
                let full_key = if key_prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{}.{}", key_prefix, k)
                };

                match v {
                    ConfigValue::Object(_) => {
                        tables.push((k.clone(), full_key, v));
                    }
                    _ => {
                        lines.push(format!("{} = {}", k, config_value_to_toml(v, &full_key)));
                    }
                }
            }

            for (_, full_key, v) in tables {
                lines.push(format!("\n[{}]", full_key));
                if let ConfigValue::Object(inner) = v {
                    for (ik, iv) in inner {
                        let inner_key = format!("{}.{}", full_key, ik);
                        lines.push(format!(
                            "{} = {}",
                            ik,
                            config_value_to_toml(iv, &inner_key)
                        ));
                    }
                }
            }

            lines.join("\n")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provides() {
        let f = TomlFormatter;
        assert!(f.provides("config.toml"));
        assert!(!f.provides("config.json"));
        assert!(!f.provides("config.yaml"));
    }

    #[test]
    fn test_deserialize() {
        let f = TomlFormatter;
        let result = f
            .deserialize("name = \"test\"\nport = 8080")
            .unwrap();
        assert_eq!(result.get("name").unwrap().as_str(), Some("test"));
        assert_eq!(result.get("port").unwrap().as_i64(), Some(8080));
    }

    #[test]
    fn test_deserialize_error() {
        let f = TomlFormatter;
        assert!(f.deserialize("[invalid").is_err());
    }

    #[test]
    fn test_serialize_simple() {
        let f = TomlFormatter;
        assert_eq!(f.serialize(&ConfigValue::Integer(42)).unwrap(), "42");
        assert_eq!(f.serialize(&ConfigValue::Bool(true)).unwrap(), "true");
    }
}
