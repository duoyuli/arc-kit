use std::fs;

use arc_core::backup::backup_files;
use arc_core::paths::ArcPaths;

#[test]
fn backup_dir_is_year_month_day_then_hhmmss_operation() {
    let temp = tempfile::tempdir().unwrap();
    let paths = ArcPaths::with_user_home(temp.path());
    let f = temp.path().join("note.txt");
    fs::write(&f, b"x").unwrap();

    let backup_dir = backup_files(&paths, "provider-use", &[f]).unwrap();
    let backups_root = paths.home().join("backups");
    let rel = backup_dir.strip_prefix(&backups_root).unwrap();
    let parts: Vec<_> = rel
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();

    assert_eq!(parts.len(), 4, "expected year/month/day/session");
    assert_eq!(parts[0].len(), 4);
    assert!(parts[0].chars().all(|c| c.is_ascii_digit()));
    assert_eq!(parts[1].len(), 2);
    assert!(parts[1].chars().all(|c| c.is_ascii_digit()));
    assert_eq!(parts[2].len(), 2);
    assert!(parts[2].chars().all(|c| c.is_ascii_digit()));

    let session = &parts[3];
    let (time_prefix, rest) = session.split_once('_').expect("session dir name");
    assert_eq!(time_prefix.len(), 6);
    assert!(time_prefix.chars().all(|c| c.is_ascii_digit()));
    assert_eq!(rest, "provider-use");
}

#[test]
fn cleanup_removes_sessions_older_than_retention_window() {
    let temp = tempfile::tempdir().unwrap();
    let paths = ArcPaths::with_user_home(temp.path());
    let backups = paths.home().join("backups");
    let old_session = backups
        .join("2020")
        .join("01")
        .join("15")
        .join("120000_provider-use");
    fs::create_dir_all(&old_session).unwrap();
    fs::write(old_session.join("dummy.txt"), b"x").unwrap();

    let f = temp.path().join("note.txt");
    fs::write(&f, b"x").unwrap();
    backup_files(&paths, "provider-use", &[f]).unwrap();

    assert!(
        !old_session.exists(),
        "backup session from 2020 should be pruned"
    );
}
