//! Visitor pattern for configuration value traversal.
//!
//! This module provides a simplified visitor pattern allowing for custom
//! deserialization logic when extracting values from configuration.

use crate::error::{Error, Result};
use crate::value::ConfigValue;
use std::collections::HashMap;

/// A visitor that can traverse and transform configuration values.
///
/// This trait allows implementing custom deserialization logic for types that
/// need more control over how configuration values are interpreted.
///
/// # Examples
///
/// ```
/// use prefer::{ConfigValue, ValueVisitor, Result};
/// use prefer::visitor::MapAccess;
///
/// struct StringCollector {
///     strings: Vec<String>,
/// }
///
/// impl ValueVisitor for StringCollector {
///     type Output = Vec<String>;
///
///     fn visit_str(&mut self, v: &str) -> Result<Self::Output> {
///         self.strings.push(v.to_string());
///         Ok(self.strings.clone())
///     }
///
///     fn visit_array(&mut self, arr: &[ConfigValue]) -> Result<Self::Output> {
///         for item in arr {
///             if let Some(s) = item.as_str() {
///                 self.strings.push(s.to_string());
///             }
///         }
///         Ok(self.strings.clone())
///     }
///
///     fn expecting(&self) -> &'static str {
///         "a string or array of strings"
///     }
/// }
/// ```
pub trait ValueVisitor {
    /// The type produced by this visitor.
    type Output;

    /// Description of what this visitor expects (for error messages).
    fn expecting(&self) -> &'static str {
        "any value"
    }

    /// Visit a null value.
    fn visit_null(&mut self) -> Result<Self::Output> {
        Err(Error::ConversionError {
            key: String::new(),
            type_name: self.expecting().into(),
            source: "unexpected null".into(),
        })
    }

    /// Visit a boolean value.
    fn visit_bool(&mut self, _v: bool) -> Result<Self::Output> {
        Err(Error::ConversionError {
            key: String::new(),
            type_name: self.expecting().into(),
            source: "unexpected boolean".into(),
        })
    }

    /// Visit a signed integer value.
    fn visit_i64(&mut self, _v: i64) -> Result<Self::Output> {
        Err(Error::ConversionError {
            key: String::new(),
            type_name: self.expecting().into(),
            source: "unexpected integer".into(),
        })
    }

    /// Visit a floating-point value.
    fn visit_f64(&mut self, _v: f64) -> Result<Self::Output> {
        Err(Error::ConversionError {
            key: String::new(),
            type_name: self.expecting().into(),
            source: "unexpected float".into(),
        })
    }

    /// Visit a string value.
    fn visit_str(&mut self, _v: &str) -> Result<Self::Output> {
        Err(Error::ConversionError {
            key: String::new(),
            type_name: self.expecting().into(),
            source: "unexpected string".into(),
        })
    }

    /// Visit an array value.
    fn visit_array(&mut self, _arr: &[ConfigValue]) -> Result<Self::Output> {
        Err(Error::ConversionError {
            key: String::new(),
            type_name: self.expecting().into(),
            source: "unexpected array".into(),
        })
    }

    /// Visit an object/map value.
    fn visit_map(&mut self, _map: MapAccess<'_>) -> Result<Self::Output> {
        Err(Error::ConversionError {
            key: String::new(),
            type_name: self.expecting().into(),
            source: "unexpected object".into(),
        })
    }
}

/// Provides access to object/map entries during visitation.
pub struct MapAccess<'a> {
    map: &'a HashMap<String, ConfigValue>,
}

impl<'a> MapAccess<'a> {
    fn new(map: &'a HashMap<String, ConfigValue>) -> Self {
        Self { map }
    }

    /// Get a value by key.
    pub fn get(&self, key: &str) -> Option<&ConfigValue> {
        self.map.get(key)
    }

    /// Check if a key exists.
    pub fn contains_key(&self, key: &str) -> bool {
        self.map.contains_key(key)
    }

    /// Get all keys in the map.
    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.map.keys().map(|s| s.as_str())
    }

    /// Iterate over all key-value pairs.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &ConfigValue)> {
        self.map.iter().map(|(k, v)| (k.as_str(), v))
    }

    /// Get the number of entries.
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Check if the map is empty.
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Get the underlying map reference.
    pub fn as_map(&self) -> &HashMap<String, ConfigValue> {
        self.map
    }
}

/// Drive a visitor through a configuration value.
///
/// This function dispatches to the appropriate visitor method based on the
/// value's type.
///
/// # Examples
///
/// ```
/// use prefer::{ConfigValue, ValueVisitor, Result};
/// use prefer::visitor::visit;
///
/// struct PortExtractor;
///
/// impl ValueVisitor for PortExtractor {
///     type Output = u16;
///
///     fn visit_i64(&mut self, v: i64) -> Result<Self::Output> {
///         u16::try_from(v).map_err(|_| prefer::Error::ConversionError {
///             key: String::new(),
///             type_name: "u16".into(),
///             source: "port out of range".into(),
///         })
///     }
///
///     fn expecting(&self) -> &'static str {
///         "a port number (0-65535)"
///     }
/// }
///
/// let value = ConfigValue::Integer(8080);
/// let port = visit(&value, &mut PortExtractor).unwrap();
/// assert_eq!(port, 8080);
/// ```
pub fn visit<V: ValueVisitor>(value: &ConfigValue, visitor: &mut V) -> Result<V::Output> {
    match value {
        ConfigValue::Null => visitor.visit_null(),
        ConfigValue::Bool(b) => visitor.visit_bool(*b),
        ConfigValue::Integer(n) => visitor.visit_i64(*n),
        ConfigValue::Float(f) => visitor.visit_f64(*f),
        ConfigValue::String(s) => visitor.visit_str(s),
        ConfigValue::Array(arr) => visitor.visit_array(arr),
        ConfigValue::Object(map) => visitor.visit_map(MapAccess::new(map)),
    }
}

/// A visitor that collects values into a type using `FromValue`.
pub struct FromValueVisitor<T> {
    _marker: std::marker::PhantomData<T>,
}

impl<T> FromValueVisitor<T> {
    pub fn new() -> Self {
        Self {
            _marker: std::marker::PhantomData,
        }
    }
}

impl<T> Default for FromValueVisitor<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: crate::FromValue> ValueVisitor for FromValueVisitor<T> {
    type Output = T;

    fn expecting(&self) -> &'static str {
        std::any::type_name::<T>()
    }

    fn visit_null(&mut self) -> Result<Self::Output> {
        T::from_value(&ConfigValue::Null)
    }

    fn visit_bool(&mut self, v: bool) -> Result<Self::Output> {
        T::from_value(&ConfigValue::Bool(v))
    }

    fn visit_i64(&mut self, v: i64) -> Result<Self::Output> {
        T::from_value(&ConfigValue::Integer(v))
    }

    fn visit_f64(&mut self, v: f64) -> Result<Self::Output> {
        T::from_value(&ConfigValue::Float(v))
    }

    fn visit_str(&mut self, v: &str) -> Result<Self::Output> {
        T::from_value(&ConfigValue::String(v.to_string()))
    }

    fn visit_array(&mut self, arr: &[ConfigValue]) -> Result<Self::Output> {
        T::from_value(&ConfigValue::Array(arr.to_vec()))
    }

    fn visit_map(&mut self, map: MapAccess<'_>) -> Result<Self::Output> {
        T::from_value(&ConfigValue::Object(map.as_map().clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct SumVisitor {
        sum: i64,
    }

    impl ValueVisitor for SumVisitor {
        type Output = i64;

        fn expecting(&self) -> &'static str {
            "a number or array of numbers"
        }

        fn visit_i64(&mut self, v: i64) -> Result<Self::Output> {
            self.sum += v;
            Ok(self.sum)
        }

        fn visit_array(&mut self, arr: &[ConfigValue]) -> Result<Self::Output> {
            for item in arr {
                if let Some(n) = item.as_i64() {
                    self.sum += n;
                }
            }
            Ok(self.sum)
        }
    }

    #[test]
    fn test_visit_integer() {
        let mut visitor = SumVisitor { sum: 0 };
        let result = visit(&ConfigValue::Integer(42), &mut visitor).unwrap();
        assert_eq!(result, 42);
    }

    #[test]
    fn test_visit_array() {
        let mut visitor = SumVisitor { sum: 0 };
        let arr = ConfigValue::Array(vec![
            ConfigValue::Integer(1),
            ConfigValue::Integer(2),
            ConfigValue::Integer(3),
            ConfigValue::Integer(4),
            ConfigValue::Integer(5),
        ]);
        let result = visit(&arr, &mut visitor).unwrap();
        assert_eq!(result, 15);
    }

    #[test]
    fn test_visit_string_error() {
        let mut visitor = SumVisitor { sum: 0 };
        let result = visit(&ConfigValue::String("hello".to_string()), &mut visitor);
        assert!(result.is_err());
    }

    struct KeyCollector {
        keys: Vec<String>,
    }

    impl ValueVisitor for KeyCollector {
        type Output = Vec<String>;

        fn expecting(&self) -> &'static str {
            "an object"
        }

        fn visit_map(&mut self, map: MapAccess<'_>) -> Result<Self::Output> {
            self.keys = map.keys().map(String::from).collect();
            Ok(self.keys.clone())
        }
    }

    #[test]
    fn test_visit_map() {
        let mut visitor = KeyCollector { keys: vec![] };
        let mut map = HashMap::new();
        map.insert("a".to_string(), ConfigValue::Integer(1));
        map.insert("b".to_string(), ConfigValue::Integer(2));
        map.insert("c".to_string(), ConfigValue::Integer(3));

        let result = visit(&ConfigValue::Object(map), &mut visitor).unwrap();
        assert_eq!(result.len(), 3);
        assert!(result.contains(&"a".to_string()));
        assert!(result.contains(&"b".to_string()));
        assert!(result.contains(&"c".to_string()));
    }

    #[test]
    fn test_map_access_methods() {
        let mut map = HashMap::new();
        map.insert(
            "host".to_string(),
            ConfigValue::String("localhost".to_string()),
        );
        map.insert("port".to_string(), ConfigValue::Integer(8080));

        let access = MapAccess::new(&map);

        assert!(access.contains_key("host"));
        assert!(!access.contains_key("nonexistent"));
        assert_eq!(access.get("host").unwrap().as_str(), Some("localhost"));
        assert_eq!(access.len(), 2);
        assert!(!access.is_empty());
    }
}
