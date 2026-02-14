//! Configuration loading abstraction.
//!
//! The `Loader` trait is the primary abstraction for loading configuration data
//! from any source. Loaders declare what identifiers they can handle via
//! `provides()`, and are discovered automatically through the registry.
//!
//! Built-in loaders:
//! - `FileLoader` — handles bare names and `file://` URLs
//! - `DbLoader` — adapter for database-backed loaders via `ConfigLoader`

pub mod db;
pub mod file;

use crate::config::Config;
use crate::error::Result;
use crate::formatter::Formatter;
use crate::value::ConfigValue;
use async_trait::async_trait;
use tokio::sync::mpsc;

/// Result of a successful load operation.
///
/// Contains parsed configuration data ready for use. Loaders are responsible
/// for parsing their source format (using the provided formatters, direct
/// conversion, etc.) before returning.
pub struct LoadResult {
    /// The resolved source identifier (e.g., "/home/user/.config/myapp.toml").
    pub source: String,

    /// The parsed configuration data.
    pub data: ConfigValue,
}

/// A source of configuration data that can be discovered via the registry.
///
/// Unlike `Source`, which requires manual construction and wiring, `Loader`
/// participates in automatic discovery: the registry calls `provides()` on
/// each registered loader to find one that can handle a given identifier.
///
/// The `load()` method receives the available formatters so that loaders
/// which deal with text-based formats (files, raw database blobs) can
/// delegate parsing. Loaders that produce structured data directly
/// (e.g., from database columns) can ignore the formatters.
///
/// # Implementing a Loader
///
/// ```ignore
/// use prefer::loader::{Loader, LoadResult};
/// use prefer::formatter::Formatter;
/// use prefer::{ConfigValue, Result};
/// use async_trait::async_trait;
///
/// struct MyLoader;
///
/// #[async_trait]
/// impl Loader for MyLoader {
///     fn provides(&self, identifier: &str) -> bool {
///         identifier.starts_with("myscheme://")
///     }
///
///     async fn load(
///         &self,
///         identifier: &str,
///         _formatters: &[&dyn Formatter],
///     ) -> Result<LoadResult> {
///         let data = fetch_and_parse(identifier).await?;
///         Ok(LoadResult {
///             source: identifier.to_string(),
///             data,
///         })
///     }
///
///     fn name(&self) -> &str {
///         "my-loader"
///     }
/// }
/// ```
#[async_trait]
pub trait Loader: Send + Sync + 'static {
    /// Whether this loader can handle the given identifier.
    ///
    /// This checks capability, not connectivity. For example, a database
    /// loader returns `true` for `postgres://` URLs without actually
    /// connecting — the connection happens in `load()`.
    fn provides(&self, identifier: &str) -> bool;

    /// Load and parse configuration from the identifier.
    ///
    /// The `formatters` slice contains all registered formatters. Loaders
    /// that deal with text-based formats should search this list (by
    /// extension or hint) to find an appropriate parser. Loaders that
    /// produce structured data directly can ignore it.
    async fn load(&self, identifier: &str, formatters: &[&dyn Formatter]) -> Result<LoadResult>;

    /// Human-readable name for error messages.
    fn name(&self) -> &str;

    /// Watch the identified source for changes.
    ///
    /// Returns `None` if this loader does not support watching.
    /// The default implementation returns `Ok(None)`.
    async fn watch(&self, _identifier: &str) -> Result<Option<mpsc::Receiver<Config>>> {
        Ok(None)
    }
}
