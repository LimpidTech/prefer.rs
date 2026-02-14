//! Configuration value types and conversion traits.
//!
//! This module provides the core `ConfigValue` enum and the `FromValue` trait
//! for converting configuration values to Rust types.

use crate::error::{Error, Result};

#[cfg(not(feature = "std"))]
use alloc::{
    collections::BTreeMap as HashMap,
    format,
    string::{String, ToString},
    vec::Vec,
};
#[cfg(not(feature = "std"))]
use core::fmt;

#[cfg(feature = "std")]
use std::{collections::HashMap, fmt, hash::Hash};

/// A configuration value that can represent any supported type.
///
/// This is the core type used throughout prefer for representing
/// parsed configuration data.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum ConfigValue {
    /// Null/missing value
    #[default]
    Null,
    /// Boolean value
    Bool(bool),
    /// Signed 64-bit integer
    Integer(i64),
    /// 64-bit floating point number
    Float(f64),
    /// UTF-8 string
    String(String),
    /// Ordered array of values
    Array(Vec<ConfigValue>),
    /// Key-value object/map
    Object(HashMap<String, ConfigValue>),
}

impl ConfigValue {
    /// Returns true if this value is null.
    pub fn is_null(&self) -> bool {
        matches!(self, ConfigValue::Null)
    }

    /// Returns the boolean value if this is a Bool.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            ConfigValue::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// Returns the integer value if this is an Integer.
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            ConfigValue::Integer(n) => Some(*n),
            _ => None,
        }
    }

    /// Returns the value as u64 if it's a non-negative Integer.
    pub fn as_u64(&self) -> Option<u64> {
        match self {
            ConfigValue::Integer(n) if *n >= 0 => Some(*n as u64),
            _ => None,
        }
    }

    /// Returns the float value if this is a Float or Integer.
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            ConfigValue::Float(f) => Some(*f),
            ConfigValue::Integer(n) => Some(*n as f64),
            _ => None,
        }
    }

    /// Returns the string value if this is a String.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            ConfigValue::String(s) => Some(s),
            _ => None,
        }
    }

    /// Returns the array if this is an Array.
    pub fn as_array(&self) -> Option<&Vec<ConfigValue>> {
        match self {
            ConfigValue::Array(arr) => Some(arr),
            _ => None,
        }
    }

    /// Returns a mutable reference to the array if this is an Array.
    pub fn as_array_mut(&mut self) -> Option<&mut Vec<ConfigValue>> {
        match self {
            ConfigValue::Array(arr) => Some(arr),
            _ => None,
        }
    }

    /// Returns the object/map if this is an Object.
    pub fn as_object(&self) -> Option<&HashMap<String, ConfigValue>> {
        match self {
            ConfigValue::Object(obj) => Some(obj),
            _ => None,
        }
    }

    /// Returns a mutable reference to the object if this is an Object.
    pub fn as_object_mut(&mut self) -> Option<&mut HashMap<String, ConfigValue>> {
        match self {
            ConfigValue::Object(obj) => Some(obj),
            _ => None,
        }
    }

    /// Get a value from an object by key.
    pub fn get(&self, key: &str) -> Option<&ConfigValue> {
        self.as_object().and_then(|obj| obj.get(key))
    }

    /// Get a mutable value from an object by key.
    pub fn get_mut(&mut self, key: &str) -> Option<&mut ConfigValue> {
        self.as_object_mut().and_then(|obj| obj.get_mut(key))
    }

    /// Returns a human-readable type name for this value.
    pub fn type_name(&self) -> &'static str {
        match self {
            ConfigValue::Null => "null",
            ConfigValue::Bool(_) => "boolean",
            ConfigValue::Integer(_) => "integer",
            ConfigValue::Float(_) => "float",
            ConfigValue::String(_) => "string",
            ConfigValue::Array(_) => "array",
            ConfigValue::Object(_) => "object",
        }
    }
}

impl fmt::Display for ConfigValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigValue::Null => write!(f, "null"),
            ConfigValue::Bool(b) => write!(f, "{}", b),
            ConfigValue::Integer(n) => write!(f, "{}", n),
            ConfigValue::Float(n) => write!(f, "{}", n),
            ConfigValue::String(s) => write!(f, "\"{}\"", s),
            ConfigValue::Array(arr) => {
                write!(f, "[")?;
                for (i, v) in arr.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", v)?;
                }
                write!(f, "]")
            }
            ConfigValue::Object(obj) => {
                write!(f, "{{")?;
                for (i, (k, v)) in obj.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "\"{}\": {}", k, v)?;
                }
                write!(f, "}}")
            }
        }
    }
}

// Convenient From implementations
impl From<bool> for ConfigValue {
    fn from(v: bool) -> Self {
        ConfigValue::Bool(v)
    }
}

impl From<i64> for ConfigValue {
    fn from(v: i64) -> Self {
        ConfigValue::Integer(v)
    }
}

impl From<i32> for ConfigValue {
    fn from(v: i32) -> Self {
        ConfigValue::Integer(v as i64)
    }
}

impl From<f64> for ConfigValue {
    fn from(v: f64) -> Self {
        ConfigValue::Float(v)
    }
}

impl From<String> for ConfigValue {
    fn from(v: String) -> Self {
        ConfigValue::String(v)
    }
}

impl From<&str> for ConfigValue {
    fn from(v: &str) -> Self {
        ConfigValue::String(v.to_string())
    }
}

impl<T: Into<ConfigValue>> From<Vec<T>> for ConfigValue {
    fn from(v: Vec<T>) -> Self {
        ConfigValue::Array(v.into_iter().map(Into::into).collect())
    }
}

impl<T: Into<ConfigValue>> From<HashMap<String, T>> for ConfigValue {
    fn from(v: HashMap<String, T>) -> Self {
        ConfigValue::Object(v.into_iter().map(|(k, v)| (k, v.into())).collect())
    }
}

impl From<()> for ConfigValue {
    fn from(_: ()) -> Self {
        ConfigValue::Null
    }
}

// ============================================================================
// FromValue trait and implementations
// ============================================================================

/// Trait for types that can be constructed from a `ConfigValue`.
///
/// This trait provides a simple way to convert configuration values to Rust types
/// without requiring serde.
///
/// # Examples
///
/// ```
/// use prefer::{ConfigValue, FromValue, Result};
///
/// struct MyConfig {
///     name: String,
///     count: i64,
/// }
///
/// impl FromValue for MyConfig {
///     fn from_value(value: &ConfigValue) -> Result<Self> {
///         let obj = value.as_object()
///             .ok_or_else(|| prefer::Error::ConversionError {
///                 key: String::new(),
///                 type_name: "MyConfig".into(),
///                 source: "expected object".into(),
///             })?;
///
///         Ok(Self {
///             name: String::from_value(obj.get("name").unwrap_or(&ConfigValue::Null))?,
///             count: i64::from_value(obj.get("count").unwrap_or(&ConfigValue::Null))?,
///         })
///     }
/// }
/// ```
pub trait FromValue: Sized {
    /// Attempt to construct Self from a configuration value.
    fn from_value(value: &ConfigValue) -> Result<Self>;
}

// Primitive type implementations

impl FromValue for bool {
    fn from_value(value: &ConfigValue) -> Result<Self> {
        value.as_bool().ok_or_else(|| Error::ConversionError {
            key: String::new(),
            type_name: "bool".into(),
            source: format!("expected boolean, found {}", value.type_name()).into(),
        })
    }
}

impl FromValue for i8 {
    fn from_value(value: &ConfigValue) -> Result<Self> {
        value
            .as_i64()
            .and_then(|n| i8::try_from(n).ok())
            .ok_or_else(|| Error::ConversionError {
                key: String::new(),
                type_name: "i8".into(),
                source: format!("expected i8, found {}", value.type_name()).into(),
            })
    }
}

impl FromValue for i16 {
    fn from_value(value: &ConfigValue) -> Result<Self> {
        value
            .as_i64()
            .and_then(|n| i16::try_from(n).ok())
            .ok_or_else(|| Error::ConversionError {
                key: String::new(),
                type_name: "i16".into(),
                source: format!("expected i16, found {}", value.type_name()).into(),
            })
    }
}

impl FromValue for i32 {
    fn from_value(value: &ConfigValue) -> Result<Self> {
        value
            .as_i64()
            .and_then(|n| i32::try_from(n).ok())
            .ok_or_else(|| Error::ConversionError {
                key: String::new(),
                type_name: "i32".into(),
                source: format!("expected i32, found {}", value.type_name()).into(),
            })
    }
}

impl FromValue for i64 {
    fn from_value(value: &ConfigValue) -> Result<Self> {
        value.as_i64().ok_or_else(|| Error::ConversionError {
            key: String::new(),
            type_name: "i64".into(),
            source: format!("expected i64, found {}", value.type_name()).into(),
        })
    }
}

impl FromValue for u8 {
    fn from_value(value: &ConfigValue) -> Result<Self> {
        value
            .as_u64()
            .and_then(|n| u8::try_from(n).ok())
            .ok_or_else(|| Error::ConversionError {
                key: String::new(),
                type_name: "u8".into(),
                source: format!("expected u8, found {}", value.type_name()).into(),
            })
    }
}

impl FromValue for u16 {
    fn from_value(value: &ConfigValue) -> Result<Self> {
        value
            .as_u64()
            .and_then(|n| u16::try_from(n).ok())
            .ok_or_else(|| Error::ConversionError {
                key: String::new(),
                type_name: "u16".into(),
                source: format!("expected u16, found {}", value.type_name()).into(),
            })
    }
}

impl FromValue for u32 {
    fn from_value(value: &ConfigValue) -> Result<Self> {
        value
            .as_u64()
            .and_then(|n| u32::try_from(n).ok())
            .ok_or_else(|| Error::ConversionError {
                key: String::new(),
                type_name: "u32".into(),
                source: format!("expected u32, found {}", value.type_name()).into(),
            })
    }
}

impl FromValue for u64 {
    fn from_value(value: &ConfigValue) -> Result<Self> {
        value.as_u64().ok_or_else(|| Error::ConversionError {
            key: String::new(),
            type_name: "u64".into(),
            source: format!("expected u64, found {}", value.type_name()).into(),
        })
    }
}

impl FromValue for usize {
    fn from_value(value: &ConfigValue) -> Result<Self> {
        let n = value.as_i64().ok_or_else(|| Error::ConversionError {
            key: String::new(),
            type_name: "usize".into(),
            source: format!("expected integer, found {}", value.type_name()).into(),
        })?;
        usize::try_from(n).map_err(|_| Error::ConversionError {
            key: String::new(),
            type_name: "usize".into(),
            source: format!("value {} out of range for usize", n).into(),
        })
    }
}

impl FromValue for isize {
    fn from_value(value: &ConfigValue) -> Result<Self> {
        let n = value.as_i64().ok_or_else(|| Error::ConversionError {
            key: String::new(),
            type_name: "isize".into(),
            source: format!("expected integer, found {}", value.type_name()).into(),
        })?;
        isize::try_from(n).map_err(|_| Error::ConversionError {
            key: String::new(),
            type_name: "isize".into(),
            source: format!("value {} out of range for isize", n).into(),
        })
    }
}

impl FromValue for f32 {
    fn from_value(value: &ConfigValue) -> Result<Self> {
        value
            .as_f64()
            .map(|n| n as f32)
            .ok_or_else(|| Error::ConversionError {
                key: String::new(),
                type_name: "f32".into(),
                source: format!("expected f32, found {}", value.type_name()).into(),
            })
    }
}

impl FromValue for f64 {
    fn from_value(value: &ConfigValue) -> Result<Self> {
        value.as_f64().ok_or_else(|| Error::ConversionError {
            key: String::new(),
            type_name: "f64".into(),
            source: format!("expected f64, found {}", value.type_name()).into(),
        })
    }
}

impl FromValue for String {
    fn from_value(value: &ConfigValue) -> Result<Self> {
        value
            .as_str()
            .map(String::from)
            .ok_or_else(|| Error::ConversionError {
                key: String::new(),
                type_name: "String".into(),
                source: format!("expected string, found {}", value.type_name()).into(),
            })
    }
}

#[cfg(feature = "std")]
impl FromValue for std::path::PathBuf {
    fn from_value(value: &ConfigValue) -> Result<Self> {
        value
            .as_str()
            .map(std::path::PathBuf::from)
            .ok_or_else(|| Error::ConversionError {
                key: String::new(),
                type_name: "PathBuf".into(),
                source: format!("expected string, found {}", value.type_name()).into(),
            })
    }
}

impl FromValue for ConfigValue {
    fn from_value(value: &ConfigValue) -> Result<Self> {
        Ok(value.clone())
    }
}

// Collection implementations

impl<T: FromValue> FromValue for Vec<T> {
    fn from_value(value: &ConfigValue) -> Result<Self> {
        let arr = value.as_array().ok_or_else(|| Error::ConversionError {
            key: String::new(),
            type_name: "Vec".into(),
            source: format!("expected array, found {}", value.type_name()).into(),
        })?;

        arr.iter()
            .enumerate()
            .map(|(i, v)| T::from_value(v).map_err(|e| e.with_key(format!("[{i}]"))))
            .collect()
    }
}

impl<T: FromValue> FromValue for Option<T> {
    fn from_value(value: &ConfigValue) -> Result<Self> {
        if value.is_null() {
            Ok(None)
        } else {
            T::from_value(value).map(Some)
        }
    }
}

#[cfg(feature = "std")]
impl<K, V> FromValue for HashMap<K, V>
where
    K: FromValue + Eq + Hash,
    V: FromValue,
{
    fn from_value(value: &ConfigValue) -> Result<Self> {
        let obj = value.as_object().ok_or_else(|| Error::ConversionError {
            key: String::new(),
            type_name: "HashMap".into(),
            source: format!("expected object, found {}", value.type_name()).into(),
        })?;

        obj.iter()
            .map(|(k, v)| {
                let key = K::from_value(&ConfigValue::String(k.clone()))?;
                let val = V::from_value(v).map_err(|e| e.with_key(k))?;
                Ok((key, val))
            })
            .collect()
    }
}

#[cfg(not(feature = "std"))]
impl<K, V> FromValue for HashMap<K, V>
where
    K: FromValue + Ord,
    V: FromValue,
{
    fn from_value(value: &ConfigValue) -> Result<Self> {
        let obj = value.as_object().ok_or_else(|| Error::ConversionError {
            key: String::new(),
            type_name: "BTreeMap".into(),
            source: format!("expected object, found {}", value.type_name()).into(),
        })?;

        obj.iter()
            .map(|(k, v)| {
                let key = K::from_value(&ConfigValue::String(k.clone()))?;
                let val = V::from_value(v).map_err(|e| e.with_key(k))?;
                Ok((key, val))
            })
            .collect()
    }
}

#[cfg(test)]
pub(crate) mod test_helpers {
    use super::ConfigValue;

    pub fn obj(items: Vec<(&str, ConfigValue)>) -> ConfigValue {
        ConfigValue::Object(items.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
    }

    pub fn int(n: i64) -> ConfigValue {
        ConfigValue::Integer(n)
    }

    pub fn float(n: f64) -> ConfigValue {
        ConfigValue::Float(n)
    }

    pub fn string(s: &str) -> ConfigValue {
        ConfigValue::String(s.to_string())
    }

    pub fn array(items: Vec<ConfigValue>) -> ConfigValue {
        ConfigValue::Array(items)
    }

    pub fn bool_val(b: bool) -> ConfigValue {
        ConfigValue::Bool(b)
    }
}

#[cfg(test)]
mod tests {
    use super::test_helpers::*;
    use super::*;

    #[test]
    fn test_from_value_bool() {
        assert!(bool::from_value(&ConfigValue::Bool(true)).unwrap());
        assert!(!bool::from_value(&ConfigValue::Bool(false)).unwrap());
        assert!(bool::from_value(&string("true")).is_err());
    }

    #[test]
    fn test_from_value_integers() {
        assert_eq!(i32::from_value(&int(42)).unwrap(), 42);
        assert_eq!(i64::from_value(&int(-100)).unwrap(), -100);
        assert_eq!(u16::from_value(&int(8080)).unwrap(), 8080);
        assert!(i8::from_value(&int(1000)).is_err()); // overflow
    }

    #[test]
    fn test_from_value_floats() {
        assert!((f64::from_value(&float(1.5)).unwrap() - 1.5).abs() < f64::EPSILON);
        assert!((f32::from_value(&float(1.5)).unwrap() - 1.5).abs() < f32::EPSILON);
        // Integer can also be read as float
        assert!((f64::from_value(&int(42)).unwrap() - 42.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_from_value_string() {
        assert_eq!(String::from_value(&string("hello")).unwrap(), "hello");
        assert!(String::from_value(&int(123)).is_err());
    }

    #[test]
    fn test_from_value_vec() {
        let result: Vec<i32> = Vec::from_value(&array(vec![int(1), int(2), int(3)])).unwrap();
        assert_eq!(result, vec![1, 2, 3]);
    }

    #[test]
    fn test_from_value_option() {
        assert_eq!(Option::<i32>::from_value(&ConfigValue::Null).unwrap(), None);
        assert_eq!(Option::<i32>::from_value(&int(42)).unwrap(), Some(42));
    }

    #[test]
    fn test_from_value_hashmap() {
        let result: HashMap<String, i32> =
            HashMap::from_value(&obj(vec![("a", int(1)), ("b", int(2))])).unwrap();
        assert_eq!(result.get("a"), Some(&1));
        assert_eq!(result.get("b"), Some(&2));
    }

    #[test]
    fn test_from_value_config_value() {
        let value = obj(vec![("nested", ConfigValue::Bool(true))]);
        let result = ConfigValue::from_value(&value).unwrap();
        assert_eq!(result, value);
    }

    #[test]
    fn test_config_value_accessors() {
        let obj = obj(vec![
            ("name", string("test")),
            ("count", int(42)),
            ("enabled", ConfigValue::Bool(true)),
        ]);

        assert!(obj.get("name").unwrap().as_str() == Some("test"));
        assert!(obj.get("count").unwrap().as_i64() == Some(42));
        assert!(obj.get("enabled").unwrap().as_bool() == Some(true));
        assert!(obj.get("missing").is_none());
    }

    #[test]
    fn test_config_value_display() {
        assert_eq!(format!("{}", ConfigValue::Null), "null");
        assert_eq!(format!("{}", ConfigValue::Bool(true)), "true");
        assert_eq!(format!("{}", int(42)), "42");
        assert_eq!(format!("{}", string("hello")), "\"hello\"");
    }

    #[test]
    fn test_config_value_display_array() {
        let arr = array(vec![int(1), int(2), int(3)]);
        assert_eq!(format!("{}", arr), "[1, 2, 3]");

        // Empty array
        assert_eq!(format!("{}", array(vec![])), "[]");

        // Single element
        assert_eq!(format!("{}", array(vec![int(42)])), "[42]");
    }

    #[test]
    fn test_config_value_display_obj() {
        // Note: HashMap iteration order is not guaranteed, so test single-element
        let obj = obj(vec![("key", int(42))]);
        assert_eq!(format!("{}", obj), "{\"key\": 42}");

        // Empty object
        let empty = ConfigValue::Object(HashMap::new());
        assert_eq!(format!("{}", empty), "{}");
    }

    #[test]
    fn test_config_value_display_float() {
        assert_eq!(format!("{}", float(1.5)), "1.5");
    }

    #[test]
    fn test_is_null() {
        assert!(ConfigValue::Null.is_null());
        assert!(!ConfigValue::Bool(false).is_null());
        assert!(!int(0).is_null());
        assert!(!string("").is_null());
    }

    #[test]
    fn test_as_u64() {
        // Positive number
        assert_eq!(int(42).as_u64(), Some(42));
        // Zero
        assert_eq!(int(0).as_u64(), Some(0));
        // Negative number returns None
        assert_eq!(int(-1).as_u64(), None);
        // Non-integer returns None
        assert_eq!(string("42").as_u64(), None);
    }

    #[test]
    fn test_as_f64_from_integer() {
        // Float from Integer
        assert_eq!(int(42).as_f64(), Some(42.0));
        // Float from Float
        assert_eq!(float(1.5).as_f64(), Some(1.5));
        // Non-number returns None
        assert_eq!(string("1.5").as_f64(), None);
    }

    #[test]
    fn test_as_array() {
        let arr = array(vec![int(1), int(2)]);
        assert!(arr.as_array().is_some());
        assert_eq!(arr.as_array().unwrap().len(), 2);

        // Non-array returns None
        assert!(int(42).as_array().is_none());
    }

    #[test]
    fn test_as_array_mut() {
        let mut arr = array(vec![int(1)]);
        if let Some(a) = arr.as_array_mut() {
            a.push(int(2));
        }
        assert_eq!(arr.as_array().unwrap().len(), 2);

        // Non-array returns None
        let mut num = int(42);
        assert!(num.as_array_mut().is_none());
    }

    #[test]
    fn test_get_mut() {
        let mut obj = obj(vec![("key", int(1))]);
        if let Some(v) = obj.get_mut("key") {
            *v = int(42);
        }
        assert_eq!(obj.get("key").unwrap().as_i64(), Some(42));

        // Missing key returns None
        assert!(obj.get_mut("missing").is_none());

        // Non-object returns None
        let mut num = int(42);
        assert!(num.get_mut("key").is_none());
    }

    #[test]
    fn test_type_name() {
        assert_eq!(ConfigValue::Null.type_name(), "null");
        assert_eq!(ConfigValue::Bool(true).type_name(), "boolean");
        assert_eq!(int(1).type_name(), "integer");
        assert_eq!(float(1.0).type_name(), "float");
        assert_eq!(string("").type_name(), "string");
        assert_eq!(array(vec![]).type_name(), "array");
        assert_eq!(obj(vec![]).type_name(), "object");
    }

    #[test]
    fn test_default() {
        let value: ConfigValue = Default::default();
        assert!(value.is_null());
    }

    #[test]
    fn test_from_bool() {
        let value: ConfigValue = true.into();
        assert_eq!(value.as_bool(), Some(true));
    }

    #[test]
    fn test_from_i64() {
        let value: ConfigValue = 42i64.into();
        assert_eq!(value.as_i64(), Some(42));
    }

    #[test]
    fn test_from_i32() {
        let value: ConfigValue = 42i32.into();
        assert_eq!(value.as_i64(), Some(42));
    }

    #[test]
    fn test_from_f64() {
        let value: ConfigValue = 1.5f64.into();
        assert_eq!(value.as_f64(), Some(1.5));
    }

    #[test]
    fn test_from_string() {
        let value: ConfigValue = String::from("hello").into();
        assert_eq!(value.as_str(), Some("hello"));
    }

    #[test]
    fn test_from_str() {
        let value: ConfigValue = "hello".into();
        assert_eq!(value.as_str(), Some("hello"));
    }

    #[test]
    fn test_from_vec() {
        let value: ConfigValue = vec![1i64, 2i64, 3i64].into();
        assert_eq!(value.as_array().unwrap().len(), 3);
    }

    #[test]
    fn test_from_hashmap() {
        let mut map = HashMap::new();
        map.insert("key".to_string(), 42i64);
        let value: ConfigValue = map.into();
        assert_eq!(value.get("key").unwrap().as_i64(), Some(42));
    }

    #[test]
    fn test_from_unit() {
        let value: ConfigValue = ().into();
        assert!(value.is_null());
    }

    #[test]
    fn test_as_bool_non_bool() {
        assert!(int(1).as_bool().is_none());
        assert!(string("true").as_bool().is_none());
    }

    #[test]
    fn test_as_i64_non_integer() {
        assert!(string("42").as_i64().is_none());
        assert!(float(42.0).as_i64().is_none());
    }

    #[test]
    fn test_as_str_non_string() {
        assert!(int(42).as_str().is_none());
        assert!(ConfigValue::Null.as_str().is_none());
    }

    #[test]
    fn test_as_object_non_obj() {
        assert!(int(42).as_object().is_none());
        assert!(array(vec![]).as_object().is_none());
    }

    #[test]
    fn test_as_object_mut_non_obj() {
        let mut arr = array(vec![]);
        assert!(arr.as_object_mut().is_none());
    }

    #[test]
    fn test_get_on_non_obj() {
        assert!(int(42).get("key").is_none());
    }

    #[test]
    fn test_from_value_usize() {
        // Valid positive integer
        assert_eq!(usize::from_value(&int(42)).unwrap(), 42);
        // Zero
        assert_eq!(usize::from_value(&int(0)).unwrap(), 0);
        // Negative value should fail
        assert!(usize::from_value(&int(-1)).is_err());
        // Non-integer should fail
        assert!(usize::from_value(&string("42")).is_err());
    }

    #[test]
    fn test_from_value_isize() {
        // Positive integer
        assert_eq!(isize::from_value(&int(42)).unwrap(), 42);
        // Negative integer
        assert_eq!(isize::from_value(&int(-42)).unwrap(), -42);
        // Zero
        assert_eq!(isize::from_value(&int(0)).unwrap(), 0);
        // Non-integer should fail
        assert!(isize::from_value(&string("42")).is_err());
    }

    #[test]
    fn test_from_value_pathbuf() {
        use std::path::PathBuf;

        // Valid path string
        let path = PathBuf::from_value(&string("/home/user/config.toml")).unwrap();
        assert_eq!(path, PathBuf::from("/home/user/config.toml"));

        // Empty string is valid
        let empty = PathBuf::from_value(&string("")).unwrap();
        assert_eq!(empty, PathBuf::from(""));

        // Non-string should fail
        assert!(PathBuf::from_value(&int(42)).is_err());
    }
}
