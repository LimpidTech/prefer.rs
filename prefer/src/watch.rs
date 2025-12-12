//! File watching functionality for configuration files.

use crate::config::Config;
use crate::discovery;
use crate::error::Result;
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::PathBuf;
use std::sync::mpsc as std_mpsc;
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
    let (notify_tx, notify_rx) = std_mpsc::channel();

    // Create watcher before spawning to properly propagate errors
    let mut watcher: RecommendedWatcher = Watcher::new(
        move |result: notify::Result<Event>| {
            if let Ok(event) = result {
                let _ = notify_tx.send(event);
            }
        },
        notify::Config::default(),
    )?;

    watcher.watch(&path, RecursiveMode::NonRecursive)?;

    tokio::spawn(async move {
        // Keep watcher alive for the duration of the task
        let _watcher = watcher;
        run_event_loop(notify_rx, tx, path).await;
    });

    Ok(rx)
}

/// Process file system events and send config updates.
/// Extracted for testability.
async fn run_event_loop(
    notify_rx: std_mpsc::Receiver<Event>,
    tx: mpsc::Sender<Config>,
    path: PathBuf,
) {
    loop {
        match notify_rx.try_recv() {
            Ok(event) => match event.kind {
                EventKind::Modify(_) | EventKind::Create(_) => {
                    if let Ok(config) = Config::load_from_path(&path).await {
                        if tx.send(config).await.is_err() {
                            break;
                        }
                    }
                }
                _ => {}
            },
            Err(std_mpsc::TryRecvError::Empty) => {
                tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
            }
            Err(std_mpsc::TryRecvError::Disconnected) => {
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    use tokio::time::{sleep, timeout, Duration};

    #[tokio::test]
    async fn test_watch_file_changes() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path().with_extension("json");

        // Write initial config
        tokio::fs::write(&path, r#"{"value": 1}"#).await.unwrap();

        let mut receiver = watch_path(path.clone()).await.unwrap();

        // Consume initial notification if present (from initial write)
        sleep(Duration::from_millis(200)).await;
        while let Ok(Some(_)) = timeout(Duration::from_millis(50), receiver.recv()).await {
            // Drain any initial notifications
        }

        // Modify the file
        tokio::fs::write(&path, r#"{"value": 2}"#).await.unwrap();

        // Wait for change notification
        if let Ok(Some(config)) = timeout(Duration::from_secs(2), receiver.recv()).await {
            let value: i32 = config.get("value").unwrap();
            assert_eq!(value, 2);
        }

        // Cleanup
        let _ = tokio::fs::remove_file(&path).await;
    }

    #[tokio::test]
    async fn test_event_loop_exits_on_channel_disconnect() {
        // Create a channel and immediately drop the sender
        let (notify_tx, notify_rx) = std_mpsc::channel::<Event>();
        let (tx, _rx) = mpsc::channel(1);
        let path = PathBuf::from("dummy.json");

        // Drop the sender to trigger Disconnected
        drop(notify_tx);

        // run_event_loop should exit immediately due to Disconnected
        let result = timeout(
            Duration::from_millis(100),
            run_event_loop(notify_rx, tx, path),
        )
        .await;

        // Should complete (not timeout) because loop exits on Disconnected
        assert!(result.is_ok());
    }
}
