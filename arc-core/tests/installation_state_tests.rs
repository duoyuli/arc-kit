use std::collections::BTreeMap;
use std::fs;

use arc_core::detect::DetectCache;
use arc_core::paths::ArcPaths;
use arc_core::skill::install::{
    InstallScope, inspect_install_ledger, install_global_skill, uninstall_global_skill,
};
use arc_core::skill::tracking::fingerprint_path;
use serde_json::json;

fn legacy_fixture(
    copy: bool,
) -> (
    tempfile::TempDir,
    ArcPaths,
    std::path::PathBuf,
    std::path::PathBuf,
    String,
) {
    let temp = tempfile::tempdir().unwrap();
    let paths = ArcPaths::with_user_home(temp.path());
    let source = temp.path().join("source");
    fs::create_dir_all(&source).unwrap();
    fs::write(source.join("SKILL.md"), "original").unwrap();
    let agent = if copy { "openclaw" } else { "codex" };
    let target = temp.path().join(format!(".{agent}/skills/demo"));
    fs::create_dir_all(target.parent().unwrap()).unwrap();
    if copy {
        fs::create_dir(&target).unwrap();
        fs::copy(source.join("SKILL.md"), target.join("SKILL.md")).unwrap();
    } else {
        std::os::unix::fs::symlink(&source, &target).unwrap();
    }
    // 固定为旧版本实际使用的 FNV 指纹，避免测试随新算法一起变化。
    let old = json!([{"agent":agent,"skill":"demo","source_path":source,"source_fingerprint":"c8258d5979962bf9"}]).to_string();
    fs::create_dir_all(paths.skill_tracking_file().parent().unwrap()).unwrap();
    fs::write(paths.skill_tracking_file(), &old).unwrap();
    (temp, paths, source, target, old)
}

#[test]
fn legacy_read_is_pure_and_migration_does_not_require_agent_detection() {
    let (_temp, paths, source, target, old) = legacy_fixture(false);
    let view = inspect_install_ledger(&paths).unwrap();
    assert_eq!(view.installs.len(), 1);
    assert_eq!(view.installs[0].scope, InstallScope::Global);
    assert_eq!(
        view.installs[0].target_path,
        fs::canonicalize(target.parent().unwrap())
            .unwrap()
            .join("demo")
    );
    assert_eq!(
        fs::read_to_string(paths.skill_tracking_file()).unwrap(),
        old
    );
    assert!(!paths.state_dir().join("skills/installs.lock").exists());
    let no_agents = DetectCache::from_map(BTreeMap::new());
    let report = install_global_skill(&paths, &no_agents, "demo", &source, &["codex".to_string()]);
    assert!(report.ok(), "{report:?}");
    let persisted: serde_json::Value =
        serde_json::from_slice(&fs::read(paths.skill_tracking_file()).unwrap()).unwrap();
    assert_eq!(persisted["schema_version"], 2);
    assert_eq!(persisted["installs"].as_array().unwrap().len(), 1);
    let backup = fs::read_dir(paths.skill_tracking_file().parent().unwrap())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| {
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("installs.v1.")
        })
        .unwrap();
    assert_eq!(fs::read_to_string(backup).unwrap(), old);
    assert!(uninstall_global_skill(&paths, &no_agents, "demo", None).ok());
    assert!(!target.is_symlink());
}

#[test]
fn legacy_copy_requires_source_verification_and_preserves_unresolved_records() {
    let (_temp, paths, source, target, _) = legacy_fixture(true);
    let valid = inspect_install_ledger(&paths).unwrap();
    assert_eq!(valid.installs.len(), 1);
    assert_eq!(
        valid.installs[0].target_fingerprint.as_deref(),
        Some(fingerprint_path(&target).unwrap().as_str())
    );
    // 旧指纹不能独立证明归属；当前来源不匹配时保留未解决记录。
    fs::write(source.join("SKILL.md"), "source changed").unwrap();
    let changed_source = inspect_install_ledger(&paths).unwrap();
    assert!(changed_source.installs.is_empty());
    assert_eq!(changed_source.unresolved_legacy.len(), 1);
    fs::write(target.join("SKILL.md"), "manual edit").unwrap();
    let unresolved = inspect_install_ledger(&paths).unwrap();
    assert!(unresolved.installs.is_empty());
    assert_eq!(unresolved.unresolved_legacy.len(), 1);
    let report = uninstall_global_skill(
        &paths,
        &DetectCache::from_map(BTreeMap::new()),
        "demo",
        None,
    );
    assert!(!report.ok());
    let persisted = inspect_install_ledger(&paths).unwrap();
    assert_eq!(persisted.unresolved_legacy.len(), 1);
    assert_eq!(
        fs::read_to_string(target.join("SKILL.md")).unwrap(),
        "manual edit"
    );
}

#[test]
fn verified_legacy_copy_persists_strong_fingerprints_on_migration() {
    let (_temp, paths, source, target, old) = legacy_fixture(true);
    let report = install_global_skill(
        &paths,
        &DetectCache::from_map(BTreeMap::new()),
        "demo",
        &source,
        &["openclaw".to_string()],
    );
    assert!(report.ok(), "{report:?}");
    let ledger = inspect_install_ledger(&paths).unwrap();
    assert!(ledger.installs[0].source_fingerprint.starts_with("sha256:"));
    assert_eq!(
        ledger.installs[0].target_fingerprint.as_deref(),
        Some(fingerprint_path(&target).unwrap().as_str())
    );
    // 迁移保留原始台账，便于检查兼容性与历史归属。
    assert!(
        fs::read_dir(paths.skill_tracking_file().parent().unwrap())
            .unwrap()
            .any(|entry| {
                let path = entry.unwrap().path();
                path.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("installs.v1.")
                    && fs::read_to_string(path).unwrap() == old
            })
    );
}

#[test]
fn missing_legacy_copy_fingerprint_never_establishes_ownership() {
    let (_temp, paths, source, target, _) = legacy_fixture(true);
    fs::write(
        paths.skill_tracking_file(),
        json!([{"agent":"openclaw","skill":"demo","source_path":source}]).to_string(),
    )
    .unwrap();
    let state = inspect_install_ledger(&paths).unwrap();
    assert!(state.installs.is_empty());
    assert_eq!(state.unresolved_legacy.len(), 1);
    assert!(target.is_dir());
}

#[test]
fn newer_schema_is_never_rewritten_or_quarantined() {
    let (_temp, paths, source, target, _) = legacy_fixture(false);
    let future = json!({"schema_version":99,"installs":[],"future_data":{"keep":true}}).to_string();
    fs::write(paths.skill_tracking_file(), &future).unwrap();
    assert!(inspect_install_ledger(&paths).is_err());
    let report = install_global_skill(
        &paths,
        &DetectCache::from_map(BTreeMap::new()),
        "demo",
        &source,
        &["codex".to_string()],
    );
    assert!(!report.ok());
    assert_eq!(
        fs::read_to_string(paths.skill_tracking_file()).unwrap(),
        future
    );
    assert!(!paths.state_dir().join("skills/installs.lock").exists());
    assert!(target.is_symlink());
}

#[test]
fn fingerprinting_rejects_special_files_instead_of_reading_them() {
    let temp = tempfile::tempdir().unwrap();
    let socket = temp.path().join("socket");
    let _listener = std::os::unix::net::UnixListener::bind(socket).unwrap();
    let error = fingerprint_path(temp.path()).unwrap_err();
    assert!(error.message.contains("unsupported file type"));
}
