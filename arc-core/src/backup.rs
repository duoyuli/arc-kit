use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use chrono::{DateTime, Duration, Local, NaiveDate};
use log::{info, warn};
use once_cell::sync::Lazy;
use regex::Regex;

use crate::paths::ArcPaths;

/// Remove backup sessions whose backup day is strictly before `today - N days` (local date).
const BACKUP_RETENTION_DAYS: i64 = 60;
/// Depth under `backups/`: year / month / day / session folder.
const NESTED_SESSION_DEPTH: usize = 4;

static LEGACY_BACKUP_DIR: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^\d{4}-\d{2}-\d{2}T\d{2}-\d{2}-\d{2}_").expect("valid regex"));

/// Back up a list of files before a write operation.
/// Returns the backup directory path on success, or None if nothing was backed up.
///
/// Layout: `backups/<year>/<month>/<day>/<HHMMSS>_<operation>/` with zero-padded month, day, and time.
/// After a successful backup, sessions whose backup day is older than the retention window (60 local calendar days) are removed.
pub fn backup_files(paths: &ArcPaths, operation: &str, files: &[PathBuf]) -> Option<PathBuf> {
    let existing: Vec<&PathBuf> = files.iter().filter(|f| f.exists()).collect();
    if existing.is_empty() {
        return None;
    }

    let now = Local::now();
    let year = now.format("%Y").to_string();
    let month = now.format("%m").to_string();
    let day = now.format("%d").to_string();
    let hhmmss = now.format("%H%M%S").to_string();
    let op = sanitize_operation_segment(operation);

    let backup_dir = paths
        .home()
        .join("backups")
        .join(year)
        .join(month)
        .join(day)
        .join(format!("{hhmmss}_{op}"));

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

fn sanitize_operation_segment(operation: &str) -> String {
    let mut s = operation
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '\0' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect::<String>();
    if s.is_empty() {
        s = "backup".to_string();
    }
    s
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
            let snapshots_dir = paths.state_dir().join("providers").join("codex");
            if let Ok(entries) = fs::read_dir(snapshots_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file() {
                        files.push(path);
                    }
                }
            }
        }
        _ => {}
    }
    files
}

fn list_backup_session_dirs(backups_dir: &Path) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    collect_nested_sessions(backups_dir, 0, &mut dirs);
    collect_legacy_flat_sessions(backups_dir, &mut dirs);
    dirs.dedup();
    dirs
}

/// Calendar day of the backup session: from `year/month/day` in the path, legacy name prefix, or mtime.
fn backup_session_date(session_dir: &Path, backups_root: &Path) -> Option<NaiveDate> {
    if let Ok(rel) = session_dir.strip_prefix(backups_root) {
        let parts: Vec<&str> = rel
            .components()
            .filter_map(|c| c.as_os_str().to_str())
            .collect();
        match parts.len() {
            4 => {
                let y: i32 = parts[0].parse().ok()?;
                let m: u32 = parts[1].parse().ok()?;
                let d: u32 = parts[2].parse().ok()?;
                return NaiveDate::from_ymd_opt(y, m, d);
            }
            1 if parts[0].len() >= 10 => {
                return NaiveDate::parse_from_str(&parts[0][..10], "%Y-%m-%d").ok();
            }
            _ => {}
        }
    }
    mtime_date(session_dir)
}

fn mtime_date(path: &Path) -> Option<NaiveDate> {
    let t: SystemTime = fs::metadata(path).ok()?.modified().ok()?;
    let dt: DateTime<Local> = t.into();
    Some(dt.date_naive())
}

fn collect_nested_sessions(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    if depth == NESTED_SESSION_DEPTH {
        out.push(dir.to_path_buf());
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.filter_map(|e| e.ok()) {
        let Ok(ft) = entry.file_type() else {
            continue;
        };
        if !ft.is_dir() {
            continue;
        }
        collect_nested_sessions(&entry.path(), depth + 1, out);
    }
}

fn collect_legacy_flat_sessions(backups_dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(backups_dir) else {
        return;
    };
    for entry in entries.filter_map(|e| e.ok()) {
        let Ok(ft) = entry.file_type() else {
            continue;
        };
        if !ft.is_dir() {
            continue;
        }
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if LEGACY_BACKUP_DIR.is_match(name) {
            out.push(entry.path());
        }
    }
}

fn prune_empty_parents(mut path: PathBuf, stop_at: &Path) {
    while path != stop_at && path.starts_with(stop_at) {
        let parent = path.parent().map(Path::to_path_buf);
        let Ok(mut read) = fs::read_dir(&path) else {
            break;
        };
        if read.next().is_some() {
            break;
        }
        drop(read);
        if fs::remove_dir(&path).is_err() {
            break;
        }
        path = match parent {
            Some(p) => p,
            None => break,
        };
    }
}

fn cleanup_old_backups(paths: &ArcPaths) {
    let backups_dir = paths.home().join("backups");
    let today = Local::now().date_naive();
    let cutoff = today - Duration::days(BACKUP_RETENTION_DAYS);

    for dir in list_backup_session_dirs(&backups_dir) {
        let Some(session_date) = backup_session_date(&dir, &backups_dir) else {
            continue;
        };
        if session_date >= cutoff {
            continue;
        }
        if let Err(e) = fs::remove_dir_all(&dir) {
            warn!("failed to remove old backup {}: {e}", dir.display());
        } else {
            info!("cleaned up old backup (before {cutoff}): {}", dir.display());
            if let Some(parent) = dir.parent() {
                prune_empty_parents(parent.to_path_buf(), &backups_dir);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_path_parses_session_date() {
        let root = Path::new("/h/.arc-cli/backups");
        let session = root.join("2020").join("01").join("15").join("120000_op");
        assert_eq!(
            backup_session_date(&session, root),
            Some(NaiveDate::from_ymd_opt(2020, 1, 15).unwrap())
        );
    }

    #[test]
    fn legacy_flat_name_parses_session_date() {
        let root = Path::new("/h/.arc-cli/backups");
        let session = root.join("2020-01-15T12-00-00_provider-use");
        assert_eq!(
            backup_session_date(&session, root),
            Some(NaiveDate::from_ymd_opt(2020, 1, 15).unwrap())
        );
    }
}
