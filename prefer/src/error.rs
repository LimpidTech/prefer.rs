//! Error types for the prefer library.

use std::path::PathBuf;
use thiserror::Error;

/// Result type alias for prefer operations.
pub type Result<T> = std::result::Result<T, Error>;

/// Errors that can occur when loading or parsing configuration files.
#[derive(Error, Debug)]
pub enum Error {
    /// Configuration file was not found in any search path.
    #[error("Configuration file '{0}' not found in any search path")]
    FileNotFound(String),

    /// Failed to read configuration file.
    #[error("Failed to read configuration file: {0}")]
    IoError(#[from] std::io::Error),

    /// Failed to parse configuration file.
    #[error("Failed to parse {format} file at {path}: {source}")]
    ParseError {
        format: String,
        path: PathBuf,
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    /// Requested configuration key was not found.
    #[error("Configuration key '{0}' not found")]
    KeyNotFound(String),

    /// Failed to convert configuration value to requested type.
    #[error("Failed to convert value at '{key}' to type {type_name}: {source}")]
    ConversionError {
        key: String,
        type_name: String,
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    /// File watching error.
    #[error("File watching error: {0}")]
    WatchError(#[from] notify::Error),

    /// Invalid configuration format.
    #[error("Invalid or unsupported configuration format for file: {0}")]
    UnsupportedFormat(PathBuf),

    /// A configuration source failed to load.
    #[error("Source '{source_name}' failed to load: {source}")]
    SourceError {
        source_name: String,
        source: Box<dyn std::error::Error + Send + Sync>,
    },
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
}
