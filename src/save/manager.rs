use std::{
    path::{Path, PathBuf},
    sync::mpsc,
    time::Duration,
};

use crate::emulator::EmulatorCommand;

const MAX_SAVES: usize = 48;
const CLEAN_SHUTDOWN_MARKER: &str = ".clean_shutdown";

/// Returns the most recent save file in `save_dir`, if any.
/// Used on startup to detect a crash (no `.clean_shutdown` marker) and restore.
pub fn find_latest_save(save_dir: &Path) -> Option<PathBuf> {
    let mut saves = list_saves(save_dir);
    saves.sort();
    saves.into_iter().next_back()
}

/// Returns true if the clean shutdown marker is present.
pub fn clean_shutdown_marker_exists(save_dir: &Path) -> bool {
    save_dir.join(CLEAN_SHUTDOWN_MARKER).exists()
}

/// Write the clean shutdown marker.
pub fn write_clean_shutdown_marker(save_dir: &Path) -> std::io::Result<()> {
    std::fs::write(save_dir.join(CLEAN_SHUTDOWN_MARKER), b"")
}

/// Remove the clean shutdown marker (called on startup before running).
pub fn remove_clean_shutdown_marker(save_dir: &Path) -> std::io::Result<()> {
    let path = save_dir.join(CLEAN_SHUTDOWN_MARKER);
    if path.exists() {
        std::fs::remove_file(path)
    } else {
        Ok(())
    }
}

/// Rotate old saves: delete oldest files so at most `MAX_SAVES` remain.
pub fn rotate_saves(save_dir: &Path) {
    let mut saves = list_saves(save_dir);
    saves.sort();
    while saves.len() >= MAX_SAVES {
        let oldest = saves.remove(0);
        if let Err(e) = std::fs::remove_file(&oldest) {
            tracing::warn!("failed to delete old save {}: {e}", oldest.display());
        }
    }
}

fn list_saves(save_dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(save_dir) else {
        return vec![];
    };
    entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.extension().and_then(|s| s.to_str()) == Some("state")
                && p.file_name()
                    .and_then(|s| s.to_str())
                    .is_some_and(|n| n.starts_with("save_"))
        })
        .collect()
}

/// Spawn a tokio task that triggers an auto-save every `interval`.
pub fn spawn_auto_save_task(
    cmd_tx: mpsc::SyncSender<EmulatorCommand>,
    interval: Duration,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(interval);
        ticker.tick().await; // skip first immediate tick
        loop {
            ticker.tick().await;
            if cmd_tx.try_send(EmulatorCommand::SaveState).is_err() {
                tracing::warn!("auto-save: cmd_tx full or disconnected");
            } else {
                tracing::info!("auto-save triggered");
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn make_save(dir: &Path, name: &str) {
        fs::write(dir.join(name), b"fake save data").unwrap();
    }

    #[test]
    fn test_find_latest_save_empty_dir() {
        let dir = TempDir::new().unwrap();
        assert!(find_latest_save(dir.path()).is_none());
    }

    #[test]
    fn test_find_latest_save_returns_newest_alphabetically() {
        let dir = TempDir::new().unwrap();
        make_save(dir.path(), "save_20240101_000000.state");
        make_save(dir.path(), "save_20240102_000000.state");
        make_save(dir.path(), "save_20231231_235959.state");

        let latest = find_latest_save(dir.path()).unwrap();
        assert_eq!(
            latest.file_name().unwrap().to_str().unwrap(),
            "save_20240102_000000.state"
        );
    }

    #[test]
    fn test_find_latest_save_ignores_non_state_files() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("readme.txt"), b"not a save").unwrap();
        fs::write(dir.path().join(".clean_shutdown"), b"").unwrap();
        make_save(dir.path(), "save_20240101_120000.state");

        let latest = find_latest_save(dir.path()).unwrap();
        assert_eq!(
            latest.file_name().unwrap().to_str().unwrap(),
            "save_20240101_120000.state"
        );
    }

    #[test]
    fn test_rotate_saves_keeps_max() {
        let dir = TempDir::new().unwrap();
        for i in 0..MAX_SAVES + 5 {
            make_save(
                dir.path(),
                &format!("save_20240101_{:06}.state", i * 60),
            );
        }
        assert_eq!(list_saves(dir.path()).len(), MAX_SAVES + 5);

        rotate_saves(dir.path());

        let remaining = list_saves(dir.path()).len();
        assert!(
            remaining < MAX_SAVES,
            "expected fewer than {MAX_SAVES} saves after rotation, got {remaining}"
        );
    }

    #[test]
    fn test_rotate_saves_deletes_oldest() {
        let dir = TempDir::new().unwrap();
        for i in 0..MAX_SAVES + 3 {
            make_save(
                dir.path(),
                &format!("save_202401_{:02}_000000.state", i + 1),
            );
        }

        rotate_saves(dir.path());

        // Oldest files (01, 02, 03) should be gone
        assert!(!dir.path().join("save_202401_01_000000.state").exists());
        assert!(!dir.path().join("save_202401_02_000000.state").exists());
        assert!(!dir.path().join("save_202401_03_000000.state").exists());
        // Newest should still be present
        assert!(dir
            .path()
            .join(format!("save_202401_{:02}_000000.state", MAX_SAVES + 3))
            .exists());
    }

    #[test]
    fn test_rotate_saves_noop_when_under_limit() {
        let dir = TempDir::new().unwrap();
        for i in 0..10 {
            make_save(dir.path(), &format!("save_20240101_{:06}.state", i));
        }
        rotate_saves(dir.path());
        assert_eq!(list_saves(dir.path()).len(), 10);
    }

    #[test]
    fn test_clean_shutdown_marker_lifecycle() {
        let dir = TempDir::new().unwrap();
        assert!(!clean_shutdown_marker_exists(dir.path()));

        write_clean_shutdown_marker(dir.path()).unwrap();
        assert!(clean_shutdown_marker_exists(dir.path()));

        remove_clean_shutdown_marker(dir.path()).unwrap();
        assert!(!clean_shutdown_marker_exists(dir.path()));
    }

    #[test]
    fn test_remove_marker_is_idempotent() {
        let dir = TempDir::new().unwrap();
        // Should not error when marker doesn't exist
        remove_clean_shutdown_marker(dir.path()).unwrap();
    }

    #[test]
    fn test_crash_detection_no_marker() {
        let dir = TempDir::new().unwrap();
        make_save(dir.path(), "save_20240101_120000.state");
        // No marker = crash. find_latest_save should return the save to restore.
        assert!(!clean_shutdown_marker_exists(dir.path()));
        assert!(find_latest_save(dir.path()).is_some());
    }

    #[test]
    fn test_crash_detection_with_marker() {
        let dir = TempDir::new().unwrap();
        make_save(dir.path(), "save_20240101_120000.state");
        write_clean_shutdown_marker(dir.path()).unwrap();
        // Marker present = clean exit, no restore needed.
        assert!(clean_shutdown_marker_exists(dir.path()));
    }
}
