//! Test that core types work in no_std mode

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(not(feature = "std"))]
extern crate alloc;

#[cfg(not(feature = "std"))]
use alloc::{
    collections::BTreeMap,
    string::{String, ToString},
    vec,
    vec::Vec,
};
#[cfg(feature = "std")]
use std::{
    string::{String, ToString},
    vec,
    vec::Vec,
};

use prefer::value::FromValue as FromValueTrait;
use prefer::ConfigValue;

#[cfg(feature = "derive")]
use prefer_derive::FromValue;

#[cfg(feature = "derive")]
#[derive(Debug, FromValue, PartialEq)]
struct NoStdConfig {
    name: String,
    value: i64,
}

#[test]
fn test_config_value_creation_no_std() {
    let value = ConfigValue::String("hello".to_string());
    assert_eq!(value.as_str(), Some("hello"));

    let number = ConfigValue::Integer(42);
    assert_eq!(number.as_i64(), Some(42));
}

#[test]
fn test_from_value_no_std() {
    let value = ConfigValue::Integer(100);
    let result: i64 = FromValueTrait::from_value(&value).unwrap();
    assert_eq!(result, 100);

    let string_value = ConfigValue::String("test".to_string());
    let result: String = FromValueTrait::from_value(&string_value).unwrap();
    assert_eq!(result, "test");
}

#[test]
#[cfg(feature = "derive")]
fn test_derive_macro_no_std() {
    #[cfg(not(feature = "std"))]
    use alloc::collections::BTreeMap as HashMap;
    #[cfg(feature = "std")]
    use std::collections::HashMap;

    let mut map = HashMap::new();
    map.insert("name".to_string(), ConfigValue::String("test".to_string()));
    map.insert("value".to_string(), ConfigValue::Integer(42));

    let value = ConfigValue::Object(map);
    let config: NoStdConfig = FromValueTrait::from_value(&value).unwrap();

    assert_eq!(config.name, "test");
    assert_eq!(config.value, 42);
}

#[test]
fn test_nested_values_no_std() {
    let inner = vec![
        ConfigValue::Integer(1),
        ConfigValue::Integer(2),
        ConfigValue::Integer(3),
    ];
    let array = ConfigValue::Array(inner);

    let values: Vec<i64> = FromValueTrait::from_value(&array).unwrap();
    assert_eq!(values, vec![1, 2, 3]);
}
