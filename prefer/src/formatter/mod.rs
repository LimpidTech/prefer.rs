//! Configuration format abstraction.
//!
//! The `Formatter` trait separates format parsing from source loading.
//! Each formatter declares what file extensions it handles via `provides()`
//! and `extensions()`, and is discovered automatically through the registry.
//!
//! Built-in formatters:
//! - `JsonFormatter` — `.json`, `.json5`, `.jsonc`
//! - `YamlFormatter` — `.yaml`, `.yml`
//! - `TomlFormatter` — `.toml`
//! - `IniFormatter` — `.ini` (behind `ini` feature)
//! - `XmlFormatter` — `.xml` (behind `xml` feature)

pub mod json;
pub mod toml;
pub mod yaml;

#[cfg(feature = "ini")]
pub mod ini;

#[cfg(feature = "xml")]
pub mod xml;

use crate::error::Result;
use crate::value::ConfigValue;
use std::path::Path;

/// A format parser/serializer for configuration data.
///
/// Formatters are stateless — they parse raw content strings into
/// `ConfigValue` and serialize `ConfigValue` back to strings.
///
/// # Implementing a Formatter
///
/// ```ignore
/// use prefer::formatter::Formatter;
/// use prefer::{ConfigValue, Result};
///
/// struct MyFormatter;
///
/// impl Formatter for MyFormatter {
///     fn provides(&self, identifier: &str) -> bool {
///         extension_matches(identifier, self.extensions())
///     }
///
///     fn extensions(&self) -> &[&str] {
///         &["myformat", "mf"]
///     }
///
///     fn deserialize(&self, content: &str) -> Result<ConfigValue> {
///         // parse content into ConfigValue
///         todo!()
///     }
///
///     fn serialize(&self, value: &ConfigValue) -> Result<String> {
///         // serialize ConfigValue to string
///         todo!()
///     }
///
///     fn name(&self) -> &str {
///         "my-format"
///     }
/// }
/// ```
pub trait Formatter: Send + Sync + 'static {
    /// Whether this formatter can handle the given source identifier.
    ///
    /// Typically checks the file extension against `extensions()`.
    fn provides(&self, identifier: &str) -> bool;

    /// File extensions this formatter handles (without the leading dot).
    ///
    /// For example: `["json", "json5", "jsonc"]`.
    fn extensions(&self) -> &[&str];

    /// Parse a content string into a `ConfigValue`.
    fn deserialize(&self, content: &str) -> Result<ConfigValue>;

    /// Serialize a `ConfigValue` back to a string.
    fn serialize(&self, value: &ConfigValue) -> Result<String>;

    /// Human-readable name for error messages.
    fn name(&self) -> &str;
}

/// Check whether an identifier's file extension matches any of the given extensions.
///
/// This is a utility for `Formatter::provides()` implementations.
pub fn extension_matches(identifier: &str, extensions: &[&str]) -> bool {
    let path = Path::new(identifier);
    let ext = match path.extension().and_then(|e| e.to_str()) {
        Some(e) => e,
        None => return false,
    };
    extensions.iter().any(|supported| *supported == ext)
}

/// Check whether a format hint string matches any of the given extensions.
///
/// Used when matching by format hint rather than file extension.
pub fn hint_matches(hint: &str, extensions: &[&str]) -> bool {
    extensions.iter().any(|ext| *ext == hint)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extension_matches() {
        assert!(extension_matches("config.json", &["json", "json5"]));
        assert!(extension_matches("config.json5", &["json", "json5"]));
        assert!(!extension_matches("config.toml", &["json", "json5"]));
        assert!(!extension_matches("no_extension", &["json"]));
    }

    #[test]
    fn test_hint_matches() {
        assert!(hint_matches("json", &["json", "json5", "jsonc"]));
        assert!(hint_matches("toml", &["toml"]));
        assert!(!hint_matches("bson", &["json", "toml"]));
        assert!(!hint_matches("", &["json"]));
    }
}
