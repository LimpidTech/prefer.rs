//! Configuration loading abstraction.
//!
//! The `Loader` trait is the primary abstraction for loading configuration data
//! from any source. Loaders declare what identifiers they can handle via
//! `provides()`, and are discovered automatically through the registry.
//!
//! Built-in loaders:
//! - `FileLoader` — handles bare names and `file://` URLs
//!
//! External crates (e.g., `prefer_db`) can register additional loaders for
//! schemes like `postgres://` and `sqlite://`.

pub mod file;

use crate::config::Config;
use crate::error::Result;
use async_trait::async_trait;
use tokio::sync::mpsc;

/// Result of a successful load operation.
///
/// Contains the raw content and metadata needed for the registry to find
/// an appropriate formatter.
pub struct LoadResult {
    /// The resolved source identifier (e.g., "/home/user/.config/myapp.toml").
    pub source: String,

    /// The raw content loaded from the source.
    pub content: String,

    /// Optional format hint (e.g., "json", "toml").
    ///
    /// Used when the source identifier doesn't have a file extension that
    /// can be used to determine the format (e.g., database-backed configs).
    pub format_hint: Option<String>,
}

/// A source of configuration data that can be discovered via the registry.
///
/// Unlike `Source`, which requires manual construction and wiring, `Loader`
/// participates in automatic discovery: the registry calls `provides()` on
/// each registered loader to find one that can handle a given identifier.
///
/// # Implementing a Loader
///
/// ```ignore
/// use prefer::loader::{Loader, LoadResult};
/// use prefer::Result;
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
///     async fn load(&self, identifier: &str) -> Result<LoadResult> {
///         let content = fetch_from_my_source(identifier).await?;
///         Ok(LoadResult {
///             source: identifier.to_string(),
///             content,
///             format_hint: Some("json".to_string()),
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

    /// Load configuration content from the identifier.
    ///
    /// Returns the resolved source path/URL, raw content string, and an
    /// optional format hint for the formatter.
    async fn load(&self, identifier: &str) -> Result<LoadResult>;

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
