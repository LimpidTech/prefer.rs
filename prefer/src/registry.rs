//! Plugin discovery via the `inventory` crate.
//!
//! Loaders and formatters register themselves at link time using
//! `inventory::submit!`. The registry provides lookup functions that iterate
//! over all registered plugins to find one that can handle a given identifier.
//!
//! External crates (e.g., `prefer_db`) can register their own loaders and
//! formatters simply by depending on `prefer` and calling `inventory::submit!`.

use crate::formatter::Formatter;
use crate::loader::Loader;

/// Wrapper for registering a `Loader` with the inventory.
///
/// Uses a static reference since inventory items must be const-constructible.
pub struct RegisteredLoader(pub &'static dyn Loader);

/// Wrapper for registering a `Formatter` with the inventory.
///
/// Uses a static reference since inventory items must be const-constructible.
pub struct RegisteredFormatter(pub &'static dyn Formatter);

inventory::collect!(RegisteredLoader);
inventory::collect!(RegisteredFormatter);

/// Collect all registered formatters from the inventory.
pub fn collect_formatters() -> Vec<&'static dyn Formatter> {
    inventory::iter::<RegisteredFormatter>
        .into_iter()
        .map(|r| r.0)
        .collect()
}

/// Find a loader that can handle the given identifier.
///
/// Iterates over all registered loaders and returns the first one whose
/// `provides()` method returns `true`.
pub fn find_loader(identifier: &str) -> Option<&'static dyn Loader> {
    for entry in inventory::iter::<RegisteredLoader> {
        if entry.0.provides(identifier) {
            return Some(entry.0);
        }
    }
    None
}

/// Find a formatter that can handle the given source identifier.
///
/// Matches by file extension on the source path.
pub fn find_formatter(source: &str) -> Option<&'static dyn Formatter> {
    for entry in inventory::iter::<RegisteredFormatter> {
        if entry.0.provides(source) {
            return Some(entry.0);
        }
    }
    None
}

/// Find a formatter by format hint string (e.g., "json", "toml").
///
/// Used when the source has no file extension but the loader provides
/// a format hint.
pub fn find_formatter_by_hint(hint: &str) -> Option<&'static dyn Formatter> {
    for entry in inventory::iter::<RegisteredFormatter> {
        if entry.0.extensions().contains(&hint) {
            return Some(entry.0);
        }
    }
    None
}
