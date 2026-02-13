//! YAML format support.

use crate::error::{Error, Result};
use crate::formatter::{extension_matches, Formatter};
use crate::value::ConfigValue;
use std::collections::HashMap;

/// Formatter for YAML files.
///
/// Uses the `yaml-rust2` crate (no serde dependency). Handles multi-document
/// files by using the first document. Converts non-string keys to strings.
pub struct YamlFormatter;

impl Formatter for YamlFormatter {
    fn provides(&self, identifier: &str) -> bool {
        extension_matches(identifier, self.extensions())
    }

    fn extensions(&self) -> &[&str] {
        &["yaml", "yml"]
    }

    fn deserialize(&self, content: &str) -> Result<ConfigValue> {
        use yaml_rust2::YamlLoader;

        let docs = YamlLoader::load_from_str(content).map_err(|e| Error::ParseError {
            format: "YAML".to_string(),
            path: std::path::PathBuf::from("<content>"),
            source: e.to_string().into(),
        })?;

        match docs.into_iter().next() {
            Some(doc) => Ok(yaml_to_config_value(doc)),
            None => Ok(ConfigValue::Object(HashMap::new())),
        }
    }

    fn serialize(&self, value: &ConfigValue) -> Result<String> {
        Ok(config_value_to_yaml(value, 0))
    }

    fn name(&self) -> &str {
        "yaml"
    }
}

fn yaml_to_config_value(yaml: yaml_rust2::Yaml) -> ConfigValue {
    use yaml_rust2::Yaml;

    match yaml {
        Yaml::Null | Yaml::BadValue => ConfigValue::Null,
        Yaml::Boolean(b) => ConfigValue::Bool(b),
        Yaml::Integer(i) => ConfigValue::Integer(i),
        Yaml::Real(s) => s
            .parse::<f64>()
            .map(ConfigValue::Float)
            .unwrap_or(ConfigValue::String(s)),
        Yaml::String(s) => ConfigValue::String(s),
        Yaml::Array(arr) => ConfigValue::Array(arr.into_iter().map(yaml_to_config_value).collect()),
        Yaml::Hash(map) => {
            let obj: HashMap<String, ConfigValue> = map
                .into_iter()
                .filter_map(|(k, v)| {
                    let key = match k {
                        Yaml::String(s) => s,
                        Yaml::Integer(i) => i.to_string(),
                        Yaml::Real(r) => r,
                        Yaml::Boolean(b) => b.to_string(),
                        _ => return None,
                    };
                    Some((key, yaml_to_config_value(v)))
                })
                .collect();
            ConfigValue::Object(obj)
        }
        Yaml::Alias(_) => {
            unreachable!("YAML aliases are resolved by parser before reaching this code")
        }
    }
}

fn config_value_to_yaml(value: &ConfigValue, indent: usize) -> String {
    let prefix = "  ".repeat(indent);
    match value {
        ConfigValue::Null => "null".to_string(),
        ConfigValue::Bool(b) => b.to_string(),
        ConfigValue::Integer(i) => i.to_string(),
        ConfigValue::Float(f) => f.to_string(),
        ConfigValue::String(s) => format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"")),
        ConfigValue::Array(arr) => {
            if arr.is_empty() {
                return "[]".to_string();
            }
            let items: Vec<String> = arr
                .iter()
                .map(|v| format!("{}- {}", prefix, config_value_to_yaml(v, indent + 1)))
                .collect();
            format!("\n{}", items.join("\n"))
        }
        ConfigValue::Object(map) => {
            if map.is_empty() {
                return "{}".to_string();
            }
            let entries: Vec<String> = map
                .iter()
                .map(|(k, v)| {
                    let val = config_value_to_yaml(v, indent + 1);
                    if matches!(v, ConfigValue::Object(_) | ConfigValue::Array(_))
                        && val.starts_with('\n')
                    {
                        format!("{}{}:{}", prefix, k, val)
                    } else {
                        format!("{}{}: {}", prefix, k, val)
                    }
                })
                .collect();
            format!("\n{}", entries.join("\n"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provides() {
        let f = YamlFormatter;
        assert!(f.provides("config.yaml"));
        assert!(f.provides("config.yml"));
        assert!(!f.provides("config.json"));
    }

    #[test]
    fn test_deserialize() {
        let f = YamlFormatter;
        let result = f.deserialize("name: test\nport: 8080").unwrap();
        assert_eq!(result.get("name").unwrap().as_str(), Some("test"));
        assert_eq!(result.get("port").unwrap().as_i64(), Some(8080));
    }

    #[test]
    fn test_deserialize_empty() {
        let f = YamlFormatter;
        let result = f.deserialize("").unwrap();
        assert!(result.as_object().is_some());
    }

    #[test]
    fn test_deserialize_error() {
        let f = YamlFormatter;
        assert!(f.deserialize("---\n- :\n  a: [}").is_err());
    }

    #[test]
    fn test_serialize_simple() {
        let f = YamlFormatter;
        let serialized = f.serialize(&ConfigValue::Integer(42)).unwrap();
        assert_eq!(serialized, "42");
    }
}
