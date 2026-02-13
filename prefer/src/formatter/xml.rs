//! XML format support.

use crate::error::{Error, Result};
use crate::formatter::{extension_matches, Formatter};
use crate::registry::RegisteredFormatter;
use crate::value::ConfigValue;
use std::collections::HashMap;

inventory::submit! { RegisteredFormatter(&XmlFormatter) }

/// Formatter for XML files.
///
/// Uses the `roxmltree` crate (no serde dependency). Attributes are prefixed
/// with `@`, text content goes to `#text` when mixed with elements, and
/// repeated elements become arrays.
pub struct XmlFormatter;

impl Formatter for XmlFormatter {
    fn provides(&self, identifier: &str) -> bool {
        extension_matches(identifier, self.extensions())
    }

    fn extensions(&self) -> &[&str] {
        &["xml"]
    }

    fn deserialize(&self, content: &str) -> Result<ConfigValue> {
        let doc = roxmltree::Document::parse(content).map_err(|e| Error::ParseError {
            format: "XML".to_string(),
            path: std::path::PathBuf::from("<content>"),
            source: e.to_string().into(),
        })?;

        Ok(xml_node_to_config_value(doc.root_element()))
    }

    fn serialize(&self, value: &ConfigValue) -> Result<String> {
        Ok(format!(
            "<?xml version=\"1.0\"?>\n<root>{}</root>",
            config_value_to_xml(value)
        ))
    }

    fn name(&self) -> &str {
        "xml"
    }
}

fn xml_node_to_config_value(node: roxmltree::Node) -> ConfigValue {
    let mut map: HashMap<String, ConfigValue> = HashMap::new();

    for attr in node.attributes() {
        map.insert(
            format!("@{}", attr.name()),
            ConfigValue::String(attr.value().to_string()),
        );
    }

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

    for (name, values) in children {
        if values.len() == 1 {
            map.insert(name, values.into_iter().next().unwrap());
        } else {
            map.insert(name, ConfigValue::Array(values));
        }
    }

    if map.is_empty() && !text_content.is_empty() {
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

    if !text_content.is_empty() {
        map.insert("#text".to_string(), ConfigValue::String(text_content));
    }

    ConfigValue::Object(map)
}

fn config_value_to_xml(value: &ConfigValue) -> String {
    match value {
        ConfigValue::Null => String::new(),
        ConfigValue::Bool(b) => b.to_string(),
        ConfigValue::Integer(i) => i.to_string(),
        ConfigValue::Float(f) => f.to_string(),
        ConfigValue::String(s) => s.clone(),
        ConfigValue::Array(arr) => arr.iter().map(config_value_to_xml).collect::<String>(),
        ConfigValue::Object(map) => {
            let mut parts = Vec::new();
            for (k, v) in map {
                if k.starts_with('@') || k == "#text" {
                    continue;
                }
                parts.push(format!("<{}>{}</{}>", k, config_value_to_xml(v), k));
            }
            parts.join("")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provides() {
        let f = XmlFormatter;
        assert!(f.provides("config.xml"));
        assert!(!f.provides("config.json"));
    }

    #[test]
    fn test_deserialize() {
        let f = XmlFormatter;
        let result = f
            .deserialize(r#"<root><name>test</name><port>8080</port></root>"#)
            .unwrap();
        assert_eq!(result.get("name").unwrap().as_str(), Some("test"));
        assert_eq!(result.get("port").unwrap().as_i64(), Some(8080));
    }

    #[test]
    fn test_deserialize_attributes() {
        let f = XmlFormatter;
        let result = f
            .deserialize(r#"<root id="123">text</root>"#)
            .unwrap();
        assert_eq!(result.get("@id").unwrap().as_str(), Some("123"));
        assert_eq!(result.get("#text").unwrap().as_str(), Some("text"));
    }

    #[test]
    fn test_deserialize_error() {
        let f = XmlFormatter;
        assert!(f.deserialize("<unclosed>").is_err());
    }
}
