//! Configuration file format parsers.

use crate::error::{Error, Result};
use crate::ConfigValue;
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

fn parse_json(contents: &str, path: &Path) -> Result<ConfigValue> {
    serde_json::from_str(contents).map_err(|e| Error::ParseError {
        format: "JSON".to_string(),
        path: path.to_path_buf(),
        source: Box::new(e),
    })
}

#[cfg(feature = "json5")]
fn parse_json5(contents: &str, path: &Path) -> Result<ConfigValue> {
    json5::from_str(contents).map_err(|e| Error::ParseError {
        format: "JSON5".to_string(),
        path: path.to_path_buf(),
        source: Box::new(e),
    })
}

#[cfg(not(feature = "json5"))]
fn parse_json5(contents: &str, path: &Path) -> Result<ConfigValue> {
    // Fall back to regular JSON parser
    parse_json(contents, path)
}

fn parse_yaml(contents: &str, path: &Path) -> Result<ConfigValue> {
    serde_yaml::from_str(contents).map_err(|e| Error::ParseError {
        format: "YAML".to_string(),
        path: path.to_path_buf(),
        source: Box::new(e),
    })
}

fn parse_toml(contents: &str, path: &Path) -> Result<ConfigValue> {
    let value: toml::Value = toml::from_str(contents).map_err(|e| Error::ParseError {
        format: "TOML".to_string(),
        path: path.to_path_buf(),
        source: Box::new(e),
    })?;

    // Convert TOML value to JSON value
    let json_str = serde_json::to_string(&value).map_err(|e| Error::ParseError {
        format: "TOML".to_string(),
        path: path.to_path_buf(),
        source: Box::new(e),
    })?;

    serde_json::from_str(&json_str).map_err(|e| Error::ParseError {
        format: "TOML".to_string(),
        path: path.to_path_buf(),
        source: Box::new(e),
    })
}

#[cfg(feature = "ini")]
fn parse_ini(contents: &str, path: &Path) -> Result<ConfigValue> {
    use ini::Ini;
    use serde_json::{Map, Value};

    let ini = Ini::load_from_str(contents).map_err(|e| Error::ParseError {
        format: "INI".to_string(),
        path: path.to_path_buf(),
        source: Box::new(e),
    })?;

    let mut root = Map::new();

    for (section, properties) in ini.iter() {
        let section_name = section.unwrap_or("default");
        let mut section_map = Map::new();

        for (key, value) in properties.iter() {
            // Try to parse as number or boolean, otherwise keep as string
            let parsed_value = if let Ok(num) = value.parse::<i64>() {
                Value::Number(num.into())
            } else if let Ok(num) = value.parse::<f64>() {
                Value::Number(serde_json::Number::from_f64(num).unwrap_or(0.into()))
            } else if let Ok(b) = value.parse::<bool>() {
                Value::Bool(b)
            } else {
                Value::String(value.to_string())
            };

            section_map.insert(key.to_string(), parsed_value);
        }

        root.insert(section_name.to_string(), Value::Object(section_map));
    }

    Ok(Value::Object(root))
}

#[cfg(not(feature = "ini"))]
fn parse_ini(_contents: &str, path: &Path) -> Result<ConfigValue> {
    Err(Error::UnsupportedFormat(path.to_path_buf()))
}

#[cfg(feature = "xml")]
fn parse_xml(contents: &str, path: &Path) -> Result<ConfigValue> {
    use quick_xml::de::from_str;

    from_str(contents).map_err(|e| Error::ParseError {
        format: "XML".to_string(),
        path: path.to_path_buf(),
        source: Box::new(e),
    })
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

        assert_eq!(result["name"], "test");
        assert_eq!(result["value"], 42);
    }

    #[test]
    fn test_parse_yaml() {
        let yaml = r#"
name: test
value: 42
        "#;
        let path = PathBuf::from("test.yaml");
        let result = parse(yaml, &path).unwrap();

        assert_eq!(result["name"], "test");
        assert_eq!(result["value"], 42);
    }

    #[test]
    fn test_parse_toml() {
        let toml = r#"
name = "test"
value = 42
        "#;
        let path = PathBuf::from("test.toml");
        let result = parse(toml, &path).unwrap();

        assert_eq!(result["name"], "test");
        assert_eq!(result["value"], 42);
    }

    #[cfg(feature = "json5")]
    #[test]
    fn test_parse_json5() {
        let json5 = r#"{
            // Comment
            name: "test",
            value: 42,
        }"#;
        let path = PathBuf::from("test.json5");
        let result = parse(json5, &path).unwrap();

        assert_eq!(result["name"], "test");
        assert_eq!(result["value"], 42);
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

        assert_eq!(result["section"]["name"], "test");
        assert_eq!(result["section"]["value"], 42);
    }

    #[test]
    fn test_unsupported_format() {
        let path = PathBuf::from("test.unknown");
        let result = parse("", &path);

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Error::UnsupportedFormat(_)));
    }
}
