//! YAML format support.

use crate::error::{Error, Result};
use crate::formatter::{extension_matches, Formatter};
use crate::registry::RegisteredFormatter;
use crate::value::ConfigValue;
use std::collections::HashMap;

inventory::submit! { RegisteredFormatter(&YamlFormatter) }

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

    #[test]
    fn test_serialize_all_scalar_types() {
        let f = YamlFormatter;
        assert_eq!(f.serialize(&ConfigValue::Null).unwrap(), "null");
        assert_eq!(f.serialize(&ConfigValue::Bool(false)).unwrap(), "false");
        assert_eq!(f.serialize(&ConfigValue::Float(2.5)).unwrap(), "2.5");
        assert_eq!(
            f.serialize(&ConfigValue::String("test".into())).unwrap(),
            "\"test\""
        );
    }

    #[test]
    fn test_serialize_string_escaping() {
        let f = YamlFormatter;
        assert_eq!(
            f.serialize(&ConfigValue::String("say \"hi\"".into()))
                .unwrap(),
            "\"say \\\"hi\\\"\""
        );
    }

    #[test]
    fn test_serialize_empty_array() {
        let f = YamlFormatter;
        let arr = ConfigValue::Array(vec![]);
        assert_eq!(f.serialize(&arr).unwrap(), "[]");
    }

    #[test]
    fn test_serialize_array() {
        let f = YamlFormatter;
        let arr = ConfigValue::Array(vec![ConfigValue::Integer(1), ConfigValue::Integer(2)]);
        let serialized = f.serialize(&arr).unwrap();
        assert!(serialized.contains("- 1"));
        assert!(serialized.contains("- 2"));
    }

    #[test]
    fn test_serialize_empty_object() {
        let f = YamlFormatter;
        let obj = ConfigValue::Object(HashMap::new());
        assert_eq!(f.serialize(&obj).unwrap(), "{}");
    }

    #[test]
    fn test_serialize_object() {
        let f = YamlFormatter;
        let mut map = HashMap::new();
        map.insert("port".to_string(), ConfigValue::Integer(8080));
        let obj = ConfigValue::Object(map);
        let serialized = f.serialize(&obj).unwrap();
        assert!(serialized.contains("port: 8080"));
    }

    #[test]
    fn test_serialize_nested_object() {
        let f = YamlFormatter;
        let mut inner = HashMap::new();
        inner.insert("host".to_string(), ConfigValue::String("localhost".into()));
        let mut outer = HashMap::new();
        outer.insert("server".to_string(), ConfigValue::Object(inner));
        let obj = ConfigValue::Object(outer);
        let serialized = f.serialize(&obj).unwrap();
        assert!(serialized.contains("server:"));
        assert!(serialized.contains("host: \"localhost\""));
    }

    #[test]
    fn test_serialize_nested_array_in_object() {
        let f = YamlFormatter;
        let mut map = HashMap::new();
        map.insert(
            "items".to_string(),
            ConfigValue::Array(vec![ConfigValue::Integer(1)]),
        );
        let obj = ConfigValue::Object(map);
        let serialized = f.serialize(&obj).unwrap();
        assert!(serialized.contains("items:"));
        assert!(serialized.contains("- 1"));
    }

    #[test]
    fn test_deserialize_all_value_types() {
        let f = YamlFormatter;
        let yaml = "null_val: null\nbool_val: true\nint_val: 42\nfloat_val: 3.15\nstr_val: hello";
        let result = f.deserialize(yaml).unwrap();
        assert!(matches!(result.get("null_val").unwrap(), ConfigValue::Null));
        assert_eq!(result.get("bool_val").unwrap().as_bool(), Some(true));
        assert_eq!(result.get("int_val").unwrap().as_i64(), Some(42));
        assert_eq!(result.get("float_val").unwrap().as_f64(), Some(3.15));
        assert_eq!(result.get("str_val").unwrap().as_str(), Some("hello"));
    }

    #[test]
    fn test_deserialize_non_string_keys() {
        let f = YamlFormatter;
        let yaml = "42: int_key\ntrue: bool_key\n3.15: float_key";
        let result = f.deserialize(yaml).unwrap();
        assert_eq!(result.get("42").unwrap().as_str(), Some("int_key"));
        assert_eq!(result.get("true").unwrap().as_str(), Some("bool_key"));
        assert_eq!(result.get("3.15").unwrap().as_str(), Some("float_key"));
    }

    #[test]
    fn test_deserialize_array() {
        let f = YamlFormatter;
        let result = f.deserialize("- 1\n- 2\n- 3").unwrap();
        assert_eq!(result.as_array().unwrap().len(), 3);
    }
}
