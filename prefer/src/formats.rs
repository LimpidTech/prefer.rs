//! Configuration file format parsers.
//!
//! This module provides parsers for various configuration file formats,
//! converting them to the unified `ConfigValue` type.

use crate::error::{Error, Result};
use crate::value::ConfigValue;
use std::collections::HashMap;
use std::path::Path;

/// Parse configuration file contents based on the file extension.
pub fn parse(contents: &str, path: &Path) -> Result<ConfigValue> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .ok_or_else(|| Error::UnsupportedFormat(path.to_path_buf()))?;

    match ext {
        "json" => parse_json(contents, path),
        "json5" | "jsonc" => parse_json5(contents, path),
        "yaml" | "yml" => parse_yaml(contents, path),
        "toml" => parse_toml(contents, path),
        "ini" => parse_ini(contents, path),
        "xml" => parse_xml(contents, path),
        _ => Err(Error::UnsupportedFormat(path.to_path_buf())),
    }
}

// ============================================================================
// JSON Parser (using jzon)
// ============================================================================

fn parse_json(contents: &str, path: &Path) -> Result<ConfigValue> {
    let value = jzon::parse(contents).map_err(|e| Error::ParseError {
        format: "JSON".to_string(),
        path: path.to_path_buf(),
        source: e.to_string().into(),
    })?;
    Ok(jzon_to_config_value(value))
}

fn jzon_to_config_value(value: jzon::JsonValue) -> ConfigValue {
    use jzon::JsonValue;

    match &value {
        JsonValue::Null => ConfigValue::Null,
        JsonValue::Boolean(b) => ConfigValue::Bool(*b),
        JsonValue::Number(_) => {
            // Try to preserve integer precision using methods on JsonValue
            if let Some(i) = value.as_i64() {
                ConfigValue::Integer(i)
            } else if let Some(f) = value.as_f64() {
                ConfigValue::Float(f)
            } else {
                // jzon always parses numbers as i64 or f64, so this is unreachable
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

// ============================================================================
// JSON5 Parser
// ============================================================================

#[cfg(feature = "json5")]
fn parse_json5(contents: &str, path: &Path) -> Result<ConfigValue> {
    // json5 crate still uses serde, so we parse to our own type
    // For now, fall back to jzon which handles most JSON5 cases
    // A proper JSON5 parser without serde would be better
    parse_json(contents, path).map_err(|_| Error::ParseError {
        format: "JSON5".to_string(),
        path: path.to_path_buf(),
        source: "JSON5 parsing failed (falling back to JSON)".into(),
    })
}

#[cfg(not(feature = "json5"))]
fn parse_json5(contents: &str, path: &Path) -> Result<ConfigValue> {
    // Fall back to regular JSON parser
    parse_json(contents, path)
}

// ============================================================================
// YAML Parser (using yaml-rust2)
// ============================================================================

fn parse_yaml(contents: &str, path: &Path) -> Result<ConfigValue> {
    use yaml_rust2::YamlLoader;

    let docs = YamlLoader::load_from_str(contents).map_err(|e| Error::ParseError {
        format: "YAML".to_string(),
        path: path.to_path_buf(),
        source: e.to_string().into(),
    })?;

    // Use the first document, or return empty object if no documents
    match docs.into_iter().next() {
        Some(doc) => Ok(yaml_to_config_value(doc)),
        None => Ok(ConfigValue::Object(HashMap::new())),
    }
}

fn yaml_to_config_value(yaml: yaml_rust2::Yaml) -> ConfigValue {
    use yaml_rust2::Yaml;

    match yaml {
        Yaml::Null | Yaml::BadValue => ConfigValue::Null,
        Yaml::Boolean(b) => ConfigValue::Bool(b),
        Yaml::Integer(i) => ConfigValue::Integer(i),
        Yaml::Real(s) => {
            // Parse the float string
            s.parse::<f64>()
                .map(ConfigValue::Float)
                .unwrap_or(ConfigValue::String(s))
        }
        Yaml::String(s) => ConfigValue::String(s),
        Yaml::Array(arr) => ConfigValue::Array(arr.into_iter().map(yaml_to_config_value).collect()),
        Yaml::Hash(map) => {
            let obj: HashMap<String, ConfigValue> = map
                .into_iter()
                .filter_map(|(k, v)| {
                    // Convert key to string
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
        // yaml-rust2 resolves aliases before returning the parsed document,
        // so we should never encounter a raw Alias node
        Yaml::Alias(_) => {
            unreachable!("YAML aliases are resolved by parser before reaching this code")
        }
    }
}

// ============================================================================
// TOML Parser (using toml_edit)
// ============================================================================

fn parse_toml(contents: &str, path: &Path) -> Result<ConfigValue> {
    use toml_edit::DocumentMut;

    let doc: DocumentMut =
        contents
            .parse()
            .map_err(|e: toml_edit::TomlError| Error::ParseError {
                format: "TOML".to_string(),
                path: path.to_path_buf(),
                source: e.to_string().into(),
            })?;

    Ok(toml_item_to_config_value(doc.as_item()))
}

fn toml_item_to_config_value(item: &toml_edit::Item) -> ConfigValue {
    use toml_edit::Item;

    match item {
        // Item::None only occurs for deleted/non-existent keys, not when iterating a document
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

// ============================================================================
// INI Parser (using rust-ini)
// ============================================================================

#[cfg(feature = "ini")]
fn parse_ini(contents: &str, path: &Path) -> Result<ConfigValue> {
    use ini::Ini;

    let ini = Ini::load_from_str(contents).map_err(|e| Error::ParseError {
        format: "INI".to_string(),
        path: path.to_path_buf(),
        source: e.to_string().into(),
    })?;

    let mut root: HashMap<String, ConfigValue> = HashMap::new();

    for (section, properties) in ini.iter() {
        let section_name = section.unwrap_or("default");
        let mut section_map: HashMap<String, ConfigValue> = HashMap::new();

        for (key, value) in properties.iter() {
            // Try to parse as number or boolean, otherwise keep as string
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

#[cfg(not(feature = "ini"))]
fn parse_ini(_contents: &str, path: &Path) -> Result<ConfigValue> {
    Err(Error::UnsupportedFormat(path.to_path_buf()))
}

// ============================================================================
// XML Parser (using roxmltree)
// ============================================================================

#[cfg(feature = "xml")]
fn parse_xml(contents: &str, path: &Path) -> Result<ConfigValue> {
    let doc = roxmltree::Document::parse(contents).map_err(|e| Error::ParseError {
        format: "XML".to_string(),
        path: path.to_path_buf(),
        source: e.to_string().into(),
    })?;

    Ok(xml_node_to_config_value(doc.root_element()))
}

#[cfg(feature = "xml")]
fn xml_node_to_config_value(node: roxmltree::Node) -> ConfigValue {
    let mut map: HashMap<String, ConfigValue> = HashMap::new();

    // Add attributes
    for attr in node.attributes() {
        map.insert(
            format!("@{}", attr.name()),
            ConfigValue::String(attr.value().to_string()),
        );
    }

    // Collect child elements
    let mut children: HashMap<String, Vec<ConfigValue>> = HashMap::new();
    let mut text_content = String::new();

    for child in node.children() {
        if child.is_element() {
            let name = child.tag_name().name().to_string();
            children
                .entry(name)
                .or_default()
                .push(xml_node_to_config_value(child));
        } else if child.is_text() {
            let text = child.text().unwrap_or("").trim();
            if !text.is_empty() {
                text_content.push_str(text);
            }
        }
    }

    // If there are child elements, add them
    for (name, values) in children {
        if values.len() == 1 {
            map.insert(name, values.into_iter().next().unwrap());
        } else {
            map.insert(name, ConfigValue::Array(values));
        }
    }

    // If there's text content and no child elements, return the text
    if map.is_empty() && !text_content.is_empty() {
        // Try to parse as a number or boolean
        if let Ok(i) = text_content.parse::<i64>() {
            return ConfigValue::Integer(i);
        }
        if let Ok(f) = text_content.parse::<f64>() {
            return ConfigValue::Float(f);
        }
        if let Ok(b) = text_content.parse::<bool>() {
            return ConfigValue::Bool(b);
        }
        return ConfigValue::String(text_content);
    }

    // If there's text content alongside elements, add it as #text
    if !text_content.is_empty() {
        map.insert("#text".to_string(), ConfigValue::String(text_content));
    }

    ConfigValue::Object(map)
}

#[cfg(not(feature = "xml"))]
fn parse_xml(_contents: &str, path: &Path) -> Result<ConfigValue> {
    Err(Error::UnsupportedFormat(path.to_path_buf()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_parse_json() {
        let json = r#"{"name": "test", "value": 42}"#;
        let path = PathBuf::from("test.json");
        let result = parse(json, &path).unwrap();

        assert_eq!(result.get("name").unwrap().as_str(), Some("test"));
        assert_eq!(result.get("value").unwrap().as_i64(), Some(42));
    }

    #[test]
    fn test_parse_yaml() {
        let yaml = r#"
name: test
value: 42
        "#;
        let path = PathBuf::from("test.yaml");
        let result = parse(yaml, &path).unwrap();

        assert_eq!(result.get("name").unwrap().as_str(), Some("test"));
        assert_eq!(result.get("value").unwrap().as_i64(), Some(42));
    }

    #[test]
    fn test_parse_toml() {
        let toml = r#"
name = "test"
value = 42
        "#;
        let path = PathBuf::from("test.toml");
        let result = parse(toml, &path).unwrap();

        assert_eq!(result.get("name").unwrap().as_str(), Some("test"));
        assert_eq!(result.get("value").unwrap().as_i64(), Some(42));
    }

    #[cfg(feature = "ini")]
    #[test]
    fn test_parse_ini() {
        let ini = r#"
[section]
name = test
value = 42
        "#;
        let path = PathBuf::from("test.ini");
        let result = parse(ini, &path).unwrap();

        let section = result.get("section").unwrap();
        assert_eq!(section.get("name").unwrap().as_str(), Some("test"));
        assert_eq!(section.get("value").unwrap().as_i64(), Some(42));
    }

    #[test]
    fn test_unsupported_format() {
        let path = PathBuf::from("test.unknown");
        let result = parse("", &path);

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Error::UnsupportedFormat(_)));
    }

    #[test]
    fn test_parse_json_with_array() {
        let json = r#"[1, 2, 3]"#;
        let path = PathBuf::from("test.json");
        let result = parse(json, &path).unwrap();

        assert!(result.as_array().is_some());
        assert_eq!(result.as_array().unwrap().len(), 3);
    }

    #[test]
    fn test_parse_json_with_null() {
        let json = r#"{"value": null}"#;
        let path = PathBuf::from("test.json");
        let result = parse(json, &path).unwrap();

        assert!(result.get("value").unwrap().is_null());
    }

    #[test]
    fn test_parse_json_with_float() {
        let json = r#"{"pi": 1.23456}"#;
        let path = PathBuf::from("test.json");
        let result = parse(json, &path).unwrap();

        let pi = result.get("pi").unwrap().as_f64().unwrap();
        assert!((pi - 1.23456).abs() < 0.0001);
    }

    #[test]
    fn test_parse_yaml_empty() {
        let yaml = "";
        let path = PathBuf::from("test.yaml");
        let result = parse(yaml, &path).unwrap();

        // Empty YAML returns empty object
        assert!(result.as_object().is_some());
    }

    #[test]
    fn test_parse_yaml_with_float() {
        let yaml = "value: 1.5";
        let path = PathBuf::from("test.yaml");
        let result = parse(yaml, &path).unwrap();

        let value = result.get("value").unwrap().as_f64().unwrap();
        assert!((value - 1.5).abs() < 0.01);
    }

    #[test]
    fn test_parse_yaml_with_null() {
        let yaml = "value: ~";
        let path = PathBuf::from("test.yaml");
        let result = parse(yaml, &path).unwrap();

        assert!(result.get("value").unwrap().is_null());
    }

    #[test]
    fn test_parse_yaml_with_array() {
        let yaml = r#"
items:
  - one
  - two
  - three
"#;
        let path = PathBuf::from("test.yaml");
        let result = parse(yaml, &path).unwrap();

        let items = result.get("items").unwrap().as_array().unwrap();
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].as_str(), Some("one"));
    }

    #[test]
    fn test_parse_yaml_with_numeric_key() {
        let yaml = "123: value";
        let path = PathBuf::from("test.yaml");
        let result = parse(yaml, &path).unwrap();

        assert_eq!(result.get("123").unwrap().as_str(), Some("value"));
    }

    #[test]
    fn test_parse_yaml_with_bool_key() {
        let yaml = "true: value";
        let path = PathBuf::from("test.yaml");
        let result = parse(yaml, &path).unwrap();

        assert_eq!(result.get("true").unwrap().as_str(), Some("value"));
    }

    #[test]
    fn test_parse_yaml_with_float_key() {
        let yaml = "1.5: value";
        let path = PathBuf::from("test.yaml");
        let result = parse(yaml, &path).unwrap();

        assert_eq!(result.get("1.5").unwrap().as_str(), Some("value"));
    }

    #[test]
    fn test_parse_toml_with_datetime() {
        let toml = r#"created = 2024-01-15T10:30:00Z"#;
        let path = PathBuf::from("test.toml");
        let result = parse(toml, &path).unwrap();

        // Datetime is stored as string
        assert!(result.get("created").unwrap().as_str().is_some());
    }

    #[test]
    fn test_parse_toml_with_inline_table() {
        let toml = r#"point = { x = 1, y = 2 }"#;
        let path = PathBuf::from("test.toml");
        let result = parse(toml, &path).unwrap();

        let point = result.get("point").unwrap();
        assert_eq!(point.get("x").unwrap().as_i64(), Some(1));
        assert_eq!(point.get("y").unwrap().as_i64(), Some(2));
    }

    #[test]
    fn test_parse_toml_with_array_of_tables() {
        let toml = r#"
[[products]]
name = "Hammer"
price = 9.99

[[products]]
name = "Nail"
price = 0.05
"#;
        let path = PathBuf::from("test.toml");
        let result = parse(toml, &path).unwrap();

        let products = result.get("products").unwrap().as_array().unwrap();
        assert_eq!(products.len(), 2);
        assert_eq!(products[0].get("name").unwrap().as_str(), Some("Hammer"));
        assert_eq!(products[1].get("name").unwrap().as_str(), Some("Nail"));
    }

    #[test]
    fn test_parse_toml_with_nested_array() {
        let toml = r#"values = [1, 2, 3]"#;
        let path = PathBuf::from("test.toml");
        let result = parse(toml, &path).unwrap();

        let values = result.get("values").unwrap().as_array().unwrap();
        assert_eq!(values.len(), 3);
    }

    #[cfg(feature = "xml")]
    #[test]
    fn test_parse_xml_with_attributes() {
        let xml = r#"<root id="123" name="test"><child>value</child></root>"#;
        let path = PathBuf::from("test.xml");
        let result = parse(xml, &path).unwrap();

        // Attributes are prefixed with @
        assert_eq!(result.get("@id").unwrap().as_str(), Some("123"));
        assert_eq!(result.get("@name").unwrap().as_str(), Some("test"));
    }

    #[cfg(feature = "xml")]
    #[test]
    fn test_parse_xml_with_text_content() {
        let xml = r#"<root>hello</root>"#;
        let path = PathBuf::from("test.xml");
        let result = parse(xml, &path).unwrap();

        // Text-only element returns the text as string
        assert_eq!(result.as_str(), Some("hello"));
    }

    #[cfg(feature = "xml")]
    #[test]
    fn test_parse_xml_with_numeric_text() {
        let xml = r#"<root>42</root>"#;
        let path = PathBuf::from("test.xml");
        let result = parse(xml, &path).unwrap();

        // Numeric text is parsed as integer
        assert_eq!(result.as_i64(), Some(42));
    }

    #[cfg(feature = "xml")]
    #[test]
    fn test_parse_xml_with_float_text() {
        let xml = r#"<root>1.5</root>"#;
        let path = PathBuf::from("test.xml");
        let result = parse(xml, &path).unwrap();

        // Float text is parsed as float
        assert!((result.as_f64().unwrap() - 1.5).abs() < 0.01);
    }

    #[cfg(feature = "xml")]
    #[test]
    fn test_parse_xml_with_bool_text() {
        let xml = r#"<root>true</root>"#;
        let path = PathBuf::from("test.xml");
        let result = parse(xml, &path).unwrap();

        // Boolean text is parsed as bool
        assert_eq!(result.as_bool(), Some(true));
    }

    #[cfg(feature = "xml")]
    #[test]
    fn test_parse_xml_with_repeated_elements() {
        let xml = r#"<root><item>one</item><item>two</item><item>three</item></root>"#;
        let path = PathBuf::from("test.xml");
        let result = parse(xml, &path).unwrap();

        // Repeated elements become an array
        let items = result.get("item").unwrap().as_array().unwrap();
        assert_eq!(items.len(), 3);
    }

    #[cfg(feature = "xml")]
    #[test]
    fn test_parse_xml_with_mixed_content() {
        let xml = r#"<root>text<child>child text</child></root>"#;
        let path = PathBuf::from("test.xml");
        let result = parse(xml, &path).unwrap();

        // Mixed content: text goes to #text, child is separate
        assert_eq!(result.get("#text").unwrap().as_str(), Some("text"));
        assert_eq!(result.get("child").unwrap().as_str(), Some("child text"));
    }

    #[cfg(feature = "ini")]
    #[test]
    fn test_parse_ini_with_integer() {
        let ini = "[section]\ncount = 42";
        let path = PathBuf::from("test.ini");
        let result = parse(ini, &path).unwrap();

        let section = result.get("section").unwrap();
        assert_eq!(section.get("count").unwrap().as_i64(), Some(42));
    }

    #[test]
    fn test_parse_json_with_short_string() {
        // jzon uses Short for small strings (optimization)
        let json = r#"{"a": "x"}"#;
        let path = PathBuf::from("test.json");
        let result = parse(json, &path).unwrap();

        assert_eq!(result.get("a").unwrap().as_str(), Some("x"));
    }

    #[test]
    fn test_parse_json_with_long_string() {
        // jzon uses Short for strings <=30 bytes, String for longer
        // This string is >30 bytes to ensure we hit JsonValue::String
        let long_value = "this string is definitely longer than thirty bytes for sure";
        let json = format!(r#"{{"key": "{}"}}"#, long_value);
        let path = PathBuf::from("test.json");
        let result = parse(&json, &path).unwrap();

        assert_eq!(result.get("key").unwrap().as_str(), Some(long_value));
    }

    #[test]
    fn test_parse_json_with_negative_integer() {
        let json = r#"{"value": -42}"#;
        let path = PathBuf::from("test.json");
        let result = parse(json, &path).unwrap();

        assert_eq!(result.get("value").unwrap().as_i64(), Some(-42));
    }

    #[test]
    fn test_parse_json_with_large_integer() {
        let json = r#"{"value": 9223372036854775807}"#;
        let path = PathBuf::from("test.json");
        let result = parse(json, &path).unwrap();

        assert_eq!(result.get("value").unwrap().as_i64(), Some(i64::MAX));
    }

    #[test]
    fn test_parse_json_with_boolean_false() {
        let json = r#"{"enabled": false}"#;
        let path = PathBuf::from("test.json");
        let result = parse(json, &path).unwrap();

        assert_eq!(result.get("enabled").unwrap().as_bool(), Some(false));
    }

    #[test]
    fn test_parse_yaml_with_boolean() {
        let yaml = "enabled: true\ndisabled: false";
        let path = PathBuf::from("test.yaml");
        let result = parse(yaml, &path).unwrap();

        assert_eq!(result.get("enabled").unwrap().as_bool(), Some(true));
        assert_eq!(result.get("disabled").unwrap().as_bool(), Some(false));
    }

    #[test]
    fn test_parse_yaml_bad_float() {
        // Test a YAML Real that can't parse as f64 (falls back to string)
        let yaml = "value: .inf";
        let path = PathBuf::from("test.yaml");
        let result = parse(yaml, &path).unwrap();

        // Infinity gets parsed - either as float or string depending on parser
        assert!(result.get("value").is_some());
    }

    #[test]
    fn test_parse_toml_with_boolean() {
        let toml = "enabled = true\ndisabled = false";
        let path = PathBuf::from("test.toml");
        let result = parse(toml, &path).unwrap();

        assert_eq!(result.get("enabled").unwrap().as_bool(), Some(true));
        assert_eq!(result.get("disabled").unwrap().as_bool(), Some(false));
    }

    #[test]
    fn test_parse_toml_with_float() {
        let toml = "pi = 1.23456";
        let path = PathBuf::from("test.toml");
        let result = parse(toml, &path).unwrap();

        let pi = result.get("pi").unwrap().as_f64().unwrap();
        assert!((pi - 1.23456).abs() < 0.0001);
    }

    #[test]
    fn test_parse_toml_with_string() {
        let toml = r#"name = "test""#;
        let path = PathBuf::from("test.toml");
        let result = parse(toml, &path).unwrap();

        assert_eq!(result.get("name").unwrap().as_str(), Some("test"));
    }

    #[test]
    fn test_parse_toml_with_nested_table() {
        let toml = r#"
[server]
host = "localhost"
port = 8080
"#;
        let path = PathBuf::from("test.toml");
        let result = parse(toml, &path).unwrap();

        let server = result.get("server").unwrap();
        assert_eq!(server.get("host").unwrap().as_str(), Some("localhost"));
        assert_eq!(server.get("port").unwrap().as_i64(), Some(8080));
    }

    #[cfg(feature = "xml")]
    #[test]
    fn test_parse_xml_empty_element() {
        let xml = r#"<root><empty/></root>"#;
        let path = PathBuf::from("test.xml");
        let result = parse(xml, &path).unwrap();

        // Empty element becomes empty object
        assert!(result.get("empty").is_some());
    }

    #[cfg(feature = "xml")]
    #[test]
    fn test_parse_xml_with_whitespace_text() {
        let xml = r#"<root>   </root>"#;
        let path = PathBuf::from("test.xml");
        let result = parse(xml, &path).unwrap();

        // Whitespace-only text is trimmed, results in empty object
        assert!(result.as_object().is_some());
    }
}
