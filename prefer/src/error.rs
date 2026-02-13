//! Error types for the prefer library.

#[cfg(not(feature = "std"))]
use alloc::string::{String, ToString};
#[cfg(feature = "std")]
use std::path::PathBuf;
#[cfg(feature = "std")]
use thiserror::Error;

/// Result type alias for prefer operations.
pub type Result<T> = core::result::Result<T, Error>;

/// Errors that can occur when loading or parsing configuration files.
#[cfg_attr(feature = "std", derive(Error))]
#[derive(Debug)]
pub enum Error {
    /// Configuration file was not found in any search path.
    #[cfg(feature = "std")]
    #[cfg_attr(
        feature = "std",
        error("Configuration file '{0}' not found in any search path")
    )]
    FileNotFound(String),

    /// Failed to read configuration file.
    #[cfg(feature = "std")]
    #[cfg_attr(feature = "std", error("Failed to read configuration file: {0}"))]
    IoError(#[cfg_attr(feature = "std", from)] std::io::Error),

    /// Failed to parse configuration file.
    #[cfg(feature = "std")]
    #[cfg_attr(
        feature = "std",
        error("Failed to parse {format} file at {path}: {source}")
    )]
    ParseError {
        format: String,
        path: PathBuf,
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    /// Requested configuration key was not found.
    #[cfg_attr(feature = "std", error("Configuration key '{0}' not found"))]
    KeyNotFound(String),

    /// Failed to convert configuration value to requested type.
    #[cfg_attr(
        feature = "std",
        error("Failed to convert value at '{key}' to type {type_name}: {source}")
    )]
    ConversionError {
        key: String,
        type_name: String,
        #[cfg(feature = "std")]
        source: Box<dyn std::error::Error + Send + Sync>,
        #[cfg(not(feature = "std"))]
        source: String,
    },

    /// File watching error.
    #[cfg(feature = "std")]
    #[cfg_attr(feature = "std", error("File watching error: {0}"))]
    WatchError(#[cfg_attr(feature = "std", from)] notify::Error),

    /// Invalid configuration format.
    #[cfg(feature = "std")]
    #[cfg_attr(
        feature = "std",
        error("Invalid or unsupported configuration format for file: {0}")
    )]
    UnsupportedFormat(PathBuf),

    /// A configuration source failed to load.
    #[cfg(feature = "std")]
    #[cfg_attr(
        feature = "std",
        error("Source '{source_name}' failed to load: {source}")
    )]
    SourceError {
        source_name: String,
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    /// No registered loader can handle the given identifier.
    #[cfg(feature = "std")]
    #[cfg_attr(feature = "std", error("No loader found for identifier: {0}"))]
    NoLoaderFound(String),

    /// No registered formatter can handle the given source.
    #[cfg(feature = "std")]
    #[cfg_attr(feature = "std", error("No formatter found for source: {0}"))]
    NoFormatterFound(String),

    /// The loader does not support watching for changes.
    #[cfg(feature = "std")]
    #[cfg_attr(feature = "std", error("Watching is not supported for: {0}"))]
    WatchNotSupported(String),
}

// Manual Display implementation for no_std
#[cfg(not(feature = "std"))]
impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Error::KeyNotFound(key) => write!(f, "Configuration key '{}' not found", key),
            Error::ConversionError {
                key,
                type_name,
                source,
            } => {
                write!(
                    f,
                    "Failed to convert value at '{}' to type {}: {}",
                    key, type_name, source
                )
            }
        }
    }
}

impl Error {
    /// Add key context to a ConversionError.
    ///
    /// If this is a ConversionError, returns a new ConversionError with the
    /// specified key. Otherwise returns self unchanged.
    #[rustfmt::skip] // Keep single-line for consistent LLVM coverage instrumentation
    pub fn with_key(self, key: impl Into<String>) -> Self {
        if let Error::ConversionError { type_name, source, .. } = self {
            Error::ConversionError { key: key.into(), type_name, source }
        } else {
            self
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_with_key_conversion_error() {
        let err = Error::ConversionError {
            key: String::new(),
            type_name: "i32".into(),
            source: "test".into(),
        };
        let result = err.with_key("my.key");
        match result {
            Error::ConversionError { key, .. } => assert_eq!(key, "my.key"),
            _ => panic!("expected ConversionError"),
        }
    }

    #[test]
    fn test_with_key_other_error() {
        let err = Error::FileNotFound("test.json".into());
        let result = err.with_key("ignored");
        assert!(matches!(result, Error::FileNotFound(s) if s == "test.json"));
    }

    #[test]
    fn test_display_no_loader_found() {
        let err = Error::NoLoaderFound("postgres://localhost".into());
        let msg = err.to_string();
        assert!(msg.contains("No loader found"));
        assert!(msg.contains("postgres://localhost"));
    }

    #[test]
    fn test_display_no_formatter_found() {
        let err = Error::NoFormatterFound("config.bson".into());
        let msg = err.to_string();
        assert!(msg.contains("No formatter found"));
        assert!(msg.contains("config.bson"));
    }

    #[test]
    fn test_display_watch_not_supported() {
        let err = Error::WatchNotSupported("redis://localhost".into());
        let msg = err.to_string();
        assert!(msg.contains("not supported"));
        assert!(msg.contains("redis://localhost"));
    }

    #[test]
    fn test_display_file_not_found() {
        let err = Error::FileNotFound("missing.toml".into());
        assert!(err.to_string().contains("missing.toml"));
    }

    #[test]
    fn test_display_key_not_found() {
        let err = Error::KeyNotFound("server.port".into());
        assert!(err.to_string().contains("server.port"));
    }

    #[test]
    fn test_display_conversion_error() {
        let err = Error::ConversionError {
            key: "port".into(),
            type_name: "u16".into(),
            source: "out of range".into(),
        };
        let msg = err.to_string();
        assert!(msg.contains("port"));
        assert!(msg.contains("u16"));
    }

    #[test]
    fn test_display_unsupported_format() {
        let err = Error::UnsupportedFormat(PathBuf::from("config.bson"));
        assert!(err.to_string().contains("config.bson"));
    }
}
