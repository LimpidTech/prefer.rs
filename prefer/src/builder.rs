//! Configuration builder for composing multiple sources.
//!
//! The `ConfigBuilder` provides a fluent API for creating configurations
//! from multiple sources with layered overrides.

#![allow(deprecated)] // Builder still uses Source/FileSource internally during transition

use crate::config::Config;
use crate::error::Result;
use crate::source::{EnvSource, FileSource, LayeredSource, MemorySource, Source};
use crate::value::ConfigValue;
use std::collections::HashMap;
use std::path::Path;

/// A builder for creating `Config` instances from multiple sources.
///
/// Sources are layered in the order they are added, with later sources
/// overriding earlier ones.
///
/// # Examples
///
/// ```no_run
/// use prefer::ConfigBuilder;
///
/// #[tokio::main]
/// async fn main() -> prefer::Result<()> {
///     let config = ConfigBuilder::new()
///         .add_file("config/default.toml")
///         .add_file("config/local.toml")
///         .add_env("MYAPP")
///         .build()
///         .await?;
///
///     let host: String = config.get("server.host")?;
///     Ok(())
/// }
/// ```
pub struct ConfigBuilder {
    sources: Vec<Box<dyn Source>>,
}

impl ConfigBuilder {
    /// Create a new empty builder.
    pub fn new() -> Self {
        Self {
            sources: Vec::new(),
        }
    }

    /// Add a source to the configuration.
    ///
    /// Sources added later override values from sources added earlier.
    pub fn add_source<S: Source + 'static>(mut self, source: S) -> Self {
        self.sources.push(Box::new(source));
        self
    }

    /// Add a file source by path.
    ///
    /// The file format is determined by its extension.
    pub fn add_file(self, path: impl AsRef<Path>) -> Self {
        self.add_source(FileSource::new(path))
    }

    /// Add a file source that may or may not exist.
    ///
    /// If the file doesn't exist, it will be skipped without error.
    pub fn add_optional_file(mut self, path: impl AsRef<Path>) -> Self {
        let path = path.as_ref();
        self.sources.push(Box::new(OptionalFileSource {
            name: path.to_string_lossy().into_owned(),
            path: path.to_path_buf(),
        }));
        self
    }

    /// Add environment variables with the given prefix.
    ///
    /// Variables are converted to nested structure using "__" as separator.
    /// For example, `MYAPP__DATABASE__HOST` becomes `database.host`.
    pub fn add_env(self, prefix: impl Into<String>) -> Self {
        self.add_source(EnvSource::new(prefix))
    }

    /// Add environment variables with a custom separator.
    pub fn add_env_with_separator(
        self,
        prefix: impl Into<String>,
        separator: impl Into<String>,
    ) -> Self {
        self.add_source(EnvSource::with_separator(prefix, separator))
    }

    /// Add in-memory default values.
    pub fn add_defaults(self, defaults: ConfigValue) -> Self {
        self.add_source(MemorySource::with_name(defaults, "defaults"))
    }

    /// Build the configuration by loading and merging all sources.
    pub async fn build(self) -> Result<Config> {
        let layered = LayeredSource {
            sources: self.sources,
        };

        let data = layered.load().await?;
        Ok(Config::new(data))
    }
}

impl Default for ConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// A file source that silently ignores missing files.
struct OptionalFileSource {
    path: std::path::PathBuf,
    name: String,
}

#[async_trait::async_trait]
impl Source for OptionalFileSource {
    async fn load(&self) -> Result<ConfigValue> {
        match tokio::fs::metadata(&self.path).await {
            Ok(_) => FileSource::new(&self.path).load().await,
            Err(_) => Ok(ConfigValue::Object(HashMap::new())),
        }
    }

    fn name(&self) -> &str {
        &self.name
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::test_helpers::obj;

    #[tokio::test]
    async fn test_builder_with_defaults() {
        let config = ConfigBuilder::new()
            .add_defaults(obj(vec![
                ("host", ConfigValue::String("localhost".to_string())),
                ("port", ConfigValue::Integer(8080)),
            ]))
            .build()
            .await
            .unwrap();

        let host: String = config.get("host").unwrap();
        assert_eq!(host, "localhost");

        let port: u16 = config.get("port").unwrap();
        assert_eq!(port, 8080);
    }

    #[tokio::test]
    async fn test_builder_layered_override() {
        let config = ConfigBuilder::new()
            .add_defaults(obj(vec![(
                "database",
                obj(vec![
                    ("host", ConfigValue::String("localhost".to_string())),
                    ("port", ConfigValue::Integer(5432)),
                ]),
            )]))
            .add_source(MemorySource::with_name(
                obj(vec![(
                    "database",
                    obj(vec![(
                        "host",
                        ConfigValue::String("production.example.com".to_string()),
                    )]),
                )]),
                "production",
            ))
            .build()
            .await
            .unwrap();

        // Host should be overridden
        let host: String = config.get("database.host").unwrap();
        assert_eq!(host, "production.example.com");

        // Port should still be from defaults
        let port: u16 = config.get("database.port").unwrap();
        assert_eq!(port, 5432);
    }
}
