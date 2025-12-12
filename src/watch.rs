//! File watching functionality for configuration files.

use crate::config::Config;
use crate::discovery;
use crate::error::Result;
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::PathBuf;
use tokio::sync::mpsc;

/// Watch a configuration file for changes.
///
/// Returns a receiver that yields new Config instances when the file is modified.
pub async fn watch(name: &str) -> Result<mpsc::Receiver<Config>> {
    let path = discovery::find_config_file(name).await?;
    watch_path(path).await
}

/// Watch a specific configuration file path for changes.
pub async fn watch_path(path: PathBuf) -> Result<mpsc::Receiver<Config>> {
    let (tx, rx) = mpsc::channel(32);

    let path_clone = path.clone();
    tokio::spawn(async move {
        let (notify_tx, mut notify_rx) = mpsc::channel(32);

        let mut watcher: RecommendedWatcher = match Watcher::new(
            move |result: notify::Result<Event>| {
                if let Ok(event) = result {
                    let _ = notify_tx.blocking_send(event);
                }
            },
            notify::Config::default(),
        ) {
            Ok(w) => w,
            Err(_) => return,
        };

        if watcher
            .watch(&path_clone, RecursiveMode::NonRecursive)
            .is_err()
        {
            return;
        }

        while let Some(event) = notify_rx.recv().await {
            match event.kind {
                EventKind::Modify(_) | EventKind::Create(_) => {
                    if let Ok(config) = Config::load_from_path(&path_clone).await {
                        if tx.send(config).await.is_err() {
                            break;
                        }
                    }
                }
                _ => {}
            }
        }
    });

    Ok(rx)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::NamedTempFile;
    use tokio::time::{sleep, Duration};

    #[tokio::test]
    async fn test_watch_file_changes() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path().with_extension("json");

        // Write initial config
        let initial = json!({"value": 1});
        tokio::fs::write(&path, initial.to_string()).await.unwrap();

        let mut receiver = watch_path(path.clone()).await.unwrap();

        // Give watcher time to set up
        sleep(Duration::from_millis(100)).await;

        // Modify the file
        let updated = json!({"value": 2});
        tokio::fs::write(&path, updated.to_string()).await.unwrap();

        // Wait for change notification
        if let Ok(config) = tokio::time::timeout(Duration::from_secs(2), receiver.recv()).await {
            if let Some(config) = config {
                let value: i32 = config.get("value").await.unwrap();
                assert_eq!(value, 2);
            }
        }

        // Cleanup
        let _ = tokio::fs::remove_file(&path).await;
    }
}
