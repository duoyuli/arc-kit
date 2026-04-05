use std::fs;
use std::path::PathBuf;

use log::{info, warn};

use crate::paths::ArcPaths;

const MAX_BACKUPS: usize = 20;

/// Back up a list of files before a write operation.
/// Returns the backup directory path on success, or None if nothing was backed up.
pub fn backup_files(paths: &ArcPaths, operation: &str, files: &[PathBuf]) -> Option<PathBuf> {
    let existing: Vec<&PathBuf> = files.iter().filter(|f| f.exists()).collect();
    if existing.is_empty() {
        return None;
    }

    let timestamp = chrono::Local::now().format("%Y-%m-%dT%H-%M-%S");
    let backup_dir = paths
        .home()
        .join("backups")
        .join(format!("{timestamp}_{operation}"));

    if let Err(e) = fs::create_dir_all(&backup_dir) {
        warn!(
            "failed to create backup directory {}: {e}",
            backup_dir.display()
        );
        return None;
    }

    let user_home = paths.user_home();
    for file in &existing {
        let relative = file
            .strip_prefix(user_home)
            .unwrap_or(file)
            .to_string_lossy()
            .replace('/', "_");
        let dest = backup_dir.join(&relative);
        if fs::copy(file, &dest).is_ok() {
            info!("backed up {} → {}", file.display(), dest.display());
        }
    }

    cleanup_old_backups(paths);
    Some(backup_dir)
}

/// Collect the files that would be affected by a provider switch for the given agent.
pub fn provider_backup_files(paths: &ArcPaths, agent: &str) -> Vec<PathBuf> {
    let mut files = vec![paths.providers_dir().join("active.toml")];
    match agent {
        "claude" => {
            files.push(paths.user_home().join(".claude").join("settings.json"));
        }
        "codex" => {
            files.push(paths.user_home().join(".codex").join("auth.json"));
            files.push(paths.user_home().join(".codex").join("config.toml"));
        }
        _ => {}
    }
    files
}

fn cleanup_old_backups(paths: &ArcPaths) {
    let backups_dir = paths.home().join("backups");
    let Ok(entries) = fs::read_dir(&backups_dir) else {
        return;
    };
    let mut dirs: Vec<PathBuf> = entries
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .map(|e| e.path())
        .collect();
    dirs.sort();

    if dirs.len() > MAX_BACKUPS {
        let to_remove = dirs.len() - MAX_BACKUPS;
        for dir in &dirs[..to_remove] {
            if let Err(e) = fs::remove_dir_all(dir) {
                warn!("failed to remove old backup {}: {e}", dir.display());
            } else {
                info!("cleaned up old backup: {}", dir.display());
            }
        }
    }
}
