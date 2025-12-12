//! Visitor pattern for configuration value traversal.
//!
//! This module provides a simplified visitor pattern allowing for custom
//! deserialization logic when extracting values from configuration.

use crate::error::{Error, Result};
use crate::value::ConfigValue;

#[cfg(not(feature = "std"))]
use alloc::{collections::BTreeMap as HashMap, string::{String, ToString}};
#[cfg(feature = "std")]
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

    /// Visit an array value with sequential access.
    ///
    /// This provides an iterator-like interface for processing array elements
    /// one at a time, which can be more efficient than materializing the entire
    /// array. By default, this delegates to `visit_array`.
    fn visit_seq(&mut self, seq: SeqAccess<'_>) -> Result<Self::Output> {
        self.visit_array(seq.as_slice())
    }

    /// Visit an object/map value.
    fn visit_map(&mut self, _map: MapAccess<'_>) -> Result<Self::Output> {
        Err(Error::ConversionError {
            key: String::new(),
            type_name: self.expecting().into(),
            source: "unexpected object".into(),
        })
    }

    /// Visit an enum variant.
    ///
    /// This is called when deserializing enum values, providing both the
    /// variant name and associated data.
    fn visit_enum(&mut self, _variant: &str, _value: &ConfigValue) -> Result<Self::Output> {
        Err(Error::ConversionError {
            key: String::new(),
            type_name: self.expecting().into(),
            source: "unexpected enum variant".into(),
        })
    }

    /// Handle an unknown field during deserialization.
    ///
    /// This is called when encountering fields that don't match the expected
    /// structure. By default, unknown fields are silently ignored. Return an
    /// error to make unknown fields cause deserialization to fail.
    fn visit_unknown(&mut self, _key: &str, _value: &ConfigValue) -> Result<()> {
        Ok(())
    }

    /// Finish visiting and potentially transform the output.
    ///
    /// This hook is called after successful visitation, allowing for final
    /// validation or transformation of the output value.
    fn finish(&mut self, output: Self::Output) -> Result<Self::Output> {
        Ok(output)
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

/// Provides sequential access to array elements during visitation.
///
/// This allows visitors to process array elements one at a time using an
/// iterator-like pattern, which can be more efficient for large arrays.
pub struct SeqAccess<'a> {
    arr: &'a [ConfigValue],
    index: usize,
}

impl<'a> SeqAccess<'a> {
    /// Create a new sequential accessor for an array.
    pub fn new(arr: &'a [ConfigValue]) -> Self {
        Self { arr, index: 0 }
    }

    /// Get the next element in the sequence.
    ///
    /// Returns `Ok(None)` when the sequence is exhausted.
    pub fn next_element<T: crate::FromValue>(&mut self) -> Result<Option<T>> {
        if self.index >= self.arr.len() {
            return Ok(None);
        }
        let element = &self.arr[self.index];
        self.index += 1;
        T::from_value(element).map(Some)
    }

    /// Get the underlying array slice.
    pub fn as_slice(&self) -> &[ConfigValue] {
        self.arr
    }

    /// Get the total number of elements in the sequence.
    pub fn len(&self) -> usize {
        self.arr.len()
    }

    /// Check if the sequence is empty.
    pub fn is_empty(&self) -> bool {
        self.arr.is_empty()
    }

    /// Get the current position in the sequence.
    pub fn position(&self) -> usize {
        self.index
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
    _marker: core::marker::PhantomData<T>,
}

impl<T> FromValueVisitor<T> {
    pub fn new() -> Self {
        Self {
            _marker: core::marker::PhantomData,
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
        core::any::type_name::<T>()
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

    struct SeqSumVisitor {
        sum: i64,
    }

    impl ValueVisitor for SeqSumVisitor {
        type Output = i64;

        fn expecting(&self) -> &'static str {
            "an array of numbers"
        }

        fn visit_seq(&mut self, mut seq: super::SeqAccess<'_>) -> Result<Self::Output> {
            while let Some(value) = seq.next_element::<i64>()? {
                self.sum += value;
            }
            Ok(self.sum)
        }
    }

    #[test]
    fn test_visit_seq() {
        use super::SeqAccess;

        let mut visitor = SeqSumVisitor { sum: 0 };
        let arr = vec![
            ConfigValue::Integer(10),
            ConfigValue::Integer(20),
            ConfigValue::Integer(30),
        ];
        let seq = SeqAccess::new(&arr);
        let result = visitor.visit_seq(seq).unwrap();
        assert_eq!(result, 60);
    }

    struct EnumVisitor;

    impl ValueVisitor for EnumVisitor {
        type Output = String;

        fn expecting(&self) -> &'static str {
            "an enum variant"
        }

        fn visit_enum(&mut self, variant: &str, value: &ConfigValue) -> Result<Self::Output> {
            Ok(format!("{}:{}", variant, value.as_i64().unwrap_or(0)))
        }
    }

    #[test]
    fn test_visit_enum() {
        let mut visitor = EnumVisitor;
        let value = ConfigValue::Integer(42);
        let result = visitor.visit_enum("Answer", &value).unwrap();
        assert_eq!(result, "Answer:42");
    }

    struct StrictMapVisitor {
        allowed_keys: Vec<String>,
    }

    impl ValueVisitor for StrictMapVisitor {
        type Output = HashMap<String, ConfigValue>;

        fn expecting(&self) -> &'static str {
            "a map with only allowed keys"
        }

        fn visit_map(&mut self, map: MapAccess<'_>) -> Result<Self::Output> {
            for key in map.keys() {
                if !self.allowed_keys.contains(&key.to_string()) {
                    self.visit_unknown(key, map.get(key).unwrap())?;
                }
            }
            Ok(map.as_map().clone())
        }

        fn visit_unknown(&mut self, key: &str, _value: &ConfigValue) -> Result<()> {
            Err(Error::ConversionError {
                key: key.to_string(),
                type_name: "strict map".into(),
                source: format!("unknown field: {}", key).into(),
            })
        }
    }

    #[test]
    fn test_visit_unknown() {
        let mut map = HashMap::new();
        map.insert("allowed".to_string(), ConfigValue::Integer(1));
        map.insert("forbidden".to_string(), ConfigValue::Integer(2));

        let mut visitor = StrictMapVisitor {
            allowed_keys: vec!["allowed".to_string()],
        };

        let result = visit(&ConfigValue::Object(map), &mut visitor);
        assert!(result.is_err());
        match result.unwrap_err() {
            Error::ConversionError { key, .. } => assert_eq!(key, "forbidden"),
            _ => panic!("Expected ConversionError"),
        }
    }
}
