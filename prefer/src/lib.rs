//! # prefer
//!
//! A lightweight library for managing application configurations with support for multiple file formats.
//!
//! `prefer` helps you manage application configurations while providing users the flexibility
//! of using whatever configuration format fits their needs. It automatically discovers
//! configuration files in standard system locations and supports JSON, JSON5, YAML, TOML,
//! INI, and XML formats.
//!
//! ## Features
//!
//! - **Format-agnostic**: Supports JSON, JSON5, YAML, TOML, INI, and XML
//! - **Automatic discovery**: Searches standard system paths for configuration files
//! - **Async by design**: Non-blocking operations for file I/O
//! - **File watching**: Monitor configuration files for changes
//! - **Dot-notation access**: Access nested values with `"auth.username"`
//! - **Cross-platform**: Works on Linux, macOS, and Windows
//! - **No serde required**: Uses a lightweight `FromValue` trait instead
//!
//! ## Examples
//!
//! ```no_run
//! use prefer::load;
//!
//! #[tokio::main]
//! async fn main() -> prefer::Result<()> {
//!     // Load configuration from any supported format
//!     let config = load("settings").await?;
//!
//!     // Access values using dot notation
//!     let username: String = config.get("auth.username")?;
//!     println!("Username: {}", username);
//!
//!     Ok(())
//! }
//! ```

pub mod builder;
pub mod config;
pub mod discovery;
pub mod error;
pub mod formats;
pub mod source;
pub mod value;
pub mod visitor;
pub mod watch;

pub use builder::ConfigBuilder;
pub use config::Config;
pub use error::{Error, Result};
pub use source::{EnvSource, FileSource, LayeredSource, MemorySource, Source};
pub use value::{ConfigValue, FromValue};
pub use visitor::ValueVisitor;

// Re-export the derive macro when the feature is enabled
#[cfg(feature = "derive")]
pub use prefer_derive::FromValue;

/// Load a configuration file by name.
///
/// This function searches standard system paths for a configuration file
/// matching the given name with any supported extension. The first file
/// found is loaded and parsed according to its format.
///
/// # Arguments
///
/// * `name` - The base name of the configuration file (without path or extension)
///
/// # Returns
///
/// A `Config` instance containing the parsed configuration data.
///
/// # Examples
///
/// ```no_run
/// use prefer::load;
///
/// #[tokio::main]
/// async fn main() -> prefer::Result<()> {
///     let config = load("myapp").await?;
///     let value: String = config.get("some.key")?;
///     Ok(())
/// }
/// ```
pub async fn load(name: &str) -> Result<Config> {
    Config::load(name).await
}

/// Watch a configuration file for changes.
///
/// Returns a stream that yields new `Config` instances whenever the
/// configuration file is modified on disk.
///
/// # Arguments
///
/// * `name` - The base name of the configuration file (without path or extension)
///
/// # Returns
///
/// A receiver channel that yields `Config` instances when the file changes.
///
/// # Examples
///
/// ```no_run
/// use prefer::watch;
///
/// #[tokio::main]
/// async fn main() -> prefer::Result<()> {
///     let mut receiver = watch("myapp").await?;
///
///     while let Some(config) = receiver.recv().await {
///         println!("Configuration updated!");
///         let value: String = config.get("some.key")?;
///     }
///
///     Ok(())
/// }
/// ```
pub async fn watch(name: &str) -> Result<tokio::sync::mpsc::Receiver<Config>> {
    watch::watch(name).await
}
