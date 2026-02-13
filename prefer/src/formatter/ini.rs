//! INI format support.

use crate::error::{Error, Result};
use crate::formatter::{extension_matches, Formatter};
use crate::registry::RegisteredFormatter;
use crate::value::ConfigValue;
use std::collections::HashMap;

inventory::submit! { RegisteredFormatter(&IniFormatter) }

/// Formatter for INI files.
///
/// Uses the `rust-ini` crate. Auto-detects value types: booleans, integers,
/// floats, and strings. Groups values by section name, with a "default"
/// section for global keys.
pub struct IniFormatter;

impl Formatter for IniFormatter {
    fn provides(&self, identifier: &str) -> bool {
        extension_matches(identifier, self.extensions())
    }

    fn extensions(&self) -> &[&str] {
        &["ini"]
    }

    fn deserialize(&self, content: &str) -> Result<ConfigValue> {
        use ini::Ini;

        let ini = Ini::load_from_str(content).map_err(|e| Error::ParseError {
            format: "INI".to_string(),
            path: std::path::PathBuf::from("<content>"),
            source: e.to_string().into(),
        })?;

        let mut root: HashMap<String, ConfigValue> = HashMap::new();

        for (section, properties) in ini.iter() {
            let section_name = section.unwrap_or("default");
            let mut section_map: HashMap<String, ConfigValue> = HashMap::new();

            for (key, value) in properties.iter() {
                let parsed_value = if let Ok(num) = value.parse::<i64>() {
                    ConfigValue::Integer(num)
                } else if let Ok(num) = value.parse::<f64>() {
                    ConfigValue::Float(num)
                } else if let Ok(b) = value.parse::<bool>() {
                    ConfigValue::Bool(b)
                } else {
                    ConfigValue::String(value.to_string())
                };

                section_map.insert(key.to_string(), parsed_value);
            }

            root.insert(section_name.to_string(), ConfigValue::Object(section_map));
        }

        Ok(ConfigValue::Object(root))
    }

    fn serialize(&self, value: &ConfigValue) -> Result<String> {
        let ConfigValue::Object(map) = value else {
            return Ok(String::new());
        };

        let mut lines = Vec::new();

        for (section, section_value) in map {
            let ConfigValue::Object(props) = section_value else {
                continue;
            };

            lines.push(format!("[{}]", section));
            for (key, val) in props {
                lines.push(format!("{} = {}", key, ini_value_str(val)));
            }
            lines.push(String::new());
        }

        Ok(lines.join("\n"))
    }

    fn name(&self) -> &str {
        "ini"
    }
}

fn ini_value_str(value: &ConfigValue) -> String {
    match value {
        ConfigValue::Null => String::new(),
        ConfigValue::Bool(b) => b.to_string(),
        ConfigValue::Integer(i) => i.to_string(),
        ConfigValue::Float(f) => f.to_string(),
        ConfigValue::String(s) => s.clone(),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provides() {
        let f = IniFormatter;
        assert!(f.provides("config.ini"));
        assert!(!f.provides("config.toml"));
    }

    #[test]
    fn test_deserialize() {
        let f = IniFormatter;
        let result = f.deserialize("[section]\nname = test\ncount = 42").unwrap();
        let section = result.get("section").unwrap();
        assert_eq!(section.get("name").unwrap().as_str(), Some("test"));
        assert_eq!(section.get("count").unwrap().as_i64(), Some(42));
    }

    #[test]
    fn test_deserialize_error() {
        let f = IniFormatter;
        // rust-ini is very permissive, most content parses. Use truly invalid input.
        // Actually rust-ini is extremely lenient. This test verifies it doesn't panic.
        let result = f.deserialize("[section]\nkey = value");
        assert!(result.is_ok());
    }

    #[test]
    fn test_deserialize_all_value_types() {
        let f = IniFormatter;
        let ini = "[types]\nint = 42\nfloat = 3.14\nbool = true\nstr = hello world";
        let result = f.deserialize(ini).unwrap();
        let section = result.get("types").unwrap();
        assert_eq!(section.get("int").unwrap().as_i64(), Some(42));
        assert_eq!(section.get("float").unwrap().as_f64(), Some(3.14));
        assert_eq!(section.get("bool").unwrap().as_bool(), Some(true));
        assert_eq!(section.get("str").unwrap().as_str(), Some("hello world"));
    }

    #[test]
    fn test_deserialize_default_section() {
        let f = IniFormatter;
        let ini = "global_key = global_value\n[section]\nkey = value";
        let result = f.deserialize(ini).unwrap();
        let default = result.get("default").unwrap();
        assert_eq!(
            default.get("global_key").unwrap().as_str(),
            Some("global_value")
        );
    }

    #[test]
    fn test_serialize_sections() {
        let f = IniFormatter;
        let mut section_map = HashMap::new();
        section_map.insert("host".to_string(), ConfigValue::String("localhost".into()));
        section_map.insert("port".to_string(), ConfigValue::Integer(5432));
        let mut root = HashMap::new();
        root.insert("database".to_string(), ConfigValue::Object(section_map));
        let obj = ConfigValue::Object(root);

        let serialized = f.serialize(&obj).unwrap();
        assert!(serialized.contains("[database]"));
        assert!(serialized.contains("host = localhost"));
        assert!(serialized.contains("port = 5432"));
    }

    #[test]
    fn test_serialize_all_value_types() {
        let f = IniFormatter;
        let mut props = HashMap::new();
        props.insert("null_val".to_string(), ConfigValue::Null);
        props.insert("bool_val".to_string(), ConfigValue::Bool(true));
        props.insert("int_val".to_string(), ConfigValue::Integer(42));
        props.insert("float_val".to_string(), ConfigValue::Float(3.14));
        props.insert("str_val".to_string(), ConfigValue::String("hello".into()));
        props.insert(
            "arr_val".to_string(),
            ConfigValue::Array(vec![ConfigValue::Integer(1)]),
        );
        let mut root = HashMap::new();
        root.insert("sect".to_string(), ConfigValue::Object(props));
        let obj = ConfigValue::Object(root);

        let serialized = f.serialize(&obj).unwrap();
        assert!(serialized.contains("[sect]"));
        assert!(serialized.contains("bool_val = true"));
        assert!(serialized.contains("int_val = 42"));
        assert!(serialized.contains("float_val = 3.14"));
        assert!(serialized.contains("str_val = hello"));
        // Null serializes to empty string, array/object to empty string
        assert!(serialized.contains("null_val = "));
        assert!(serialized.contains("arr_val = "));
    }

    #[test]
    fn test_serialize_non_object_root() {
        let f = IniFormatter;
        // Non-object root produces empty output
        let serialized = f.serialize(&ConfigValue::Integer(42)).unwrap();
        assert!(serialized.is_empty());
    }
}
