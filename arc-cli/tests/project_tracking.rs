use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use serde_json::Value;

struct Fixture {
    _temp: tempfile::TempDir,
    home: PathBuf,
    project: PathBuf,
    bin: PathBuf,
    index: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().join("home");
        let project = temp.path().join("project with spaces");
        let bin = temp.path().join("bin");
        fs::create_dir_all(home.join(".arc-cli/skills/demo")).unwrap();
        fs::write(home.join(".arc-cli/skills/demo/SKILL.md"), "# demo\n").unwrap();
        fs::create_dir_all(&project).unwrap();
        fs::create_dir_all(&bin).unwrap();
        fs::write(
            bin.join("codex"),
            "#!/bin/sh\nprintf 'codex-cli 0.116.0\\n'\n",
        )
        .unwrap();
        fs::set_permissions(bin.join("codex"), fs::Permissions::from_mode(0o755)).unwrap();
        let index = temp.path().join("empty-index.toml");
        fs::write(&index, "version = 1\nupdated_at = \"2026-09-14\"\n").unwrap();
        let fixture = Self {
            _temp: temp,
            home,
            project,
            bin,
            index,
        };
        fixture.require(&["demo"]);
        fixture
    }

    fn require(&self, names: &[&str]) {
        fs::write(
            self.project.join("arc.toml"),
            format!(
                "[skills]\nrequire = {}\n",
                serde_json::to_string(names).unwrap()
            ),
        )
        .unwrap();
    }

    fn cmd(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_arc"));
        command
            .current_dir(&self.project)
            .env_remove("ARC_KIT_HOME")
            .env("ARC_KIT_USER_HOME", &self.home)
            .env(
                "ARC_KIT_BUILTIN_MARKET_INDEX_URL",
                format!("file://{}", self.index.display()),
            )
            .env("PATH", format!("{}:/usr/bin:/bin", self.bin.display()))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        command
    }

    fn json(&self, args: &[&str], expected: i32) -> Value {
        let output = self
            .cmd()
            .args(args)
            .args(["--format", "json"])
            .output()
            .unwrap();
        decode(output, expected)
    }

    fn target(&self) -> PathBuf {
        self.project.join(".codex/skills/demo")
    }
    fn ledger(&self) -> Value {
        serde_json::from_slice(
            &fs::read(self.home.join(".arc-cli/state/skills/installs.json")).unwrap(),
        )
        .unwrap()
    }
}

fn decode(output: Output, expected: i32) -> Value {
    assert_eq!(
        output.status.code(),
        Some(expected),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!output.stdout.contains(&0x1b));
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["schema_version"], "6");
    json
}

fn inventory(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn visit(path: &Path, output: &mut BTreeMap<PathBuf, Vec<u8>>) {
        let metadata = fs::symlink_metadata(path).unwrap();
        if metadata.file_type().is_symlink() {
            output.insert(
                path.to_path_buf(),
                fs::read_link(path)
                    .unwrap()
                    .as_os_str()
                    .as_encoded_bytes()
                    .to_vec(),
            );
        } else if metadata.is_dir() {
            output.insert(path.to_path_buf(), b"directory".to_vec());
            for entry in fs::read_dir(path).unwrap() {
                visit(&entry.unwrap().path(), output);
            }
        } else {
            output.insert(path.to_path_buf(), fs::read(path).unwrap());
        }
    }
    let mut output = BTreeMap::new();
    visit(root, &mut output);
    output
}

#[test]
fn project_apply_records_paths_and_cleanup_works_after_agent_disappears() {
    let fixture = Fixture::new();
    let installed = fixture.json(&["project", "apply", "--agent", "codex"], 0);
    assert_eq!(installed["scope"], "project");
    assert_eq!(installed["items"][0]["status"], "installed");
    assert!(
        installed["items"][0]["target_path"]
            .as_str()
            .unwrap()
            .ends_with(".codex/skills/demo")
    );
    assert_eq!(fixture.ledger()["installs"].as_array().unwrap().len(), 1);
    let unchanged = fixture.json(&["project", "apply"], 0);
    assert_eq!(unchanged["has_changes"], false);
    fixture.require(&[]);
    fs::remove_file(fixture.bin.join("codex")).unwrap();
    let removed = fixture.json(&["project", "apply"], 0);
    assert_eq!(removed["items"][0]["status"], "removed");
    assert!(!fixture.target().is_symlink());
    assert!(fixture.ledger()["installs"].as_array().unwrap().is_empty());
}

#[test]
fn dry_run_and_status_leave_every_file_and_directory_unchanged() {
    let fixture = Fixture::new();
    let before = inventory(fixture._temp.path());
    let preview = fixture.json(&["project", "apply", "--agent", "codex", "--dry-run"], 0);
    assert_eq!(preview["dry_run"], true);
    assert_eq!(preview["items"][0]["status"], "planned");
    assert_eq!(inventory(fixture._temp.path()), before);
    fixture.json(&["status"], 0);
    assert_eq!(inventory(fixture._temp.path()), before);
    fixture.json(&["project", "apply", "--agent", "codex"], 0);
    let before = inventory(fixture._temp.path());
    let status = fixture.json(&["status"], 0);
    assert_eq!(
        status["project"]["installations"]["items"][0]["status"],
        "kept"
    );
    assert!(status["project"]["installations"]["items"][0]["target_path"].is_string());
    assert_eq!(inventory(fixture._temp.path()), before);
}

#[test]
fn dry_run_reports_missing_required_source_without_writes() {
    let fixture = Fixture::new();
    // 缺少来源的预演必须保持只读，不依赖产品是否附带内置技能。
    fixture.require(&["missing-skill"]);
    let before = inventory(fixture._temp.path());
    let result = fixture.json(&["project", "apply", "--agent", "codex", "--dry-run"], 1);
    assert_eq!(result["ok"], false);
    assert_eq!(result["items"][0]["status"], "unresolved");
    assert_eq!(inventory(fixture._temp.path()), before);

    // 来源随后出现时，普通 apply 仍可安装同一份声明。
    let source = fixture.home.join(".arc-cli/skills/missing-skill");
    fs::create_dir_all(&source).unwrap();
    fs::write(source.join("SKILL.md"), "# missing-skill\n").unwrap();
    fixture.json(&["project", "apply", "--agent", "codex"], 0);
    assert!(
        fixture
            .project
            .join(".codex/skills/missing-skill")
            .is_symlink()
    );
}

#[test]
fn missing_market_checkout_is_unresolved_without_cloning_in_read_only_commands() {
    let fixture = Fixture::new();
    fixture.require(&["market-demo"]);
    let markets = fixture.home.join(".arc-cli/markets");
    fs::create_dir_all(&markets).unwrap();
    fs::copy(&fixture.index, markets.join("index.toml")).unwrap();
    let catalog = serde_json::json!({
        "version": 3,
        "updated_at": "",
        "sources": {"fixture": {
            "id": "fixture", "git_url": "https://example.invalid/fixture/market.git",
            "owner": "fixture", "repo": "market", "parser": "auto",
            "status": "ok", "last_updated_at": "1", "resource_count": 1
        }},
        "resources": [{"id": "fixture/market-demo", "kind": "skill", "name": "market-demo", "source_id": "fixture", "summary": ""}]
    });
    fs::write(markets.join("catalog.json"), catalog.to_string()).unwrap();
    // 用假 git 记录调用，避免测试失败时访问真实网络。
    let git = fixture.bin.join("git");
    fs::write(&git, "#!/bin/sh\ntouch git-was-called\nexit 1\n").unwrap();
    fs::set_permissions(&git, fs::Permissions::from_mode(0o755)).unwrap();
    let before = inventory(fixture._temp.path());
    let preview = fixture.json(&["project", "apply", "--dry-run", "--agent", "codex"], 1);
    assert_eq!(preview["items"][0]["status"], "unresolved");
    assert_eq!(inventory(fixture._temp.path()), before);
    let status = fixture.json(&["status"], 0);
    assert_eq!(
        status["project"]["installations"]["items"][0]["status"],
        "unresolved"
    );
    assert_eq!(inventory(fixture._temp.path()), before);
}

#[test]
fn adoption_is_explicit_and_clean_preserves_manifest_and_manual_files() {
    let fixture = Fixture::new();
    fs::create_dir_all(fixture.target().parent().unwrap()).unwrap();
    symlink(fixture.home.join(".arc-cli/skills/demo"), fixture.target()).unwrap();
    let manual = fixture.project.join(".codex/skills/manual");
    fs::create_dir_all(&manual).unwrap();
    fs::write(manual.join("notes"), "keep").unwrap();
    let refused = fixture.json(&["project", "apply", "--agent", "codex"], 1);
    assert_eq!(refused["items"][0]["status"], "unmanaged");
    let adopted = fixture.json(
        &["project", "apply", "--agent", "codex", "--adopt-existing"],
        0,
    );
    assert_eq!(adopted["items"][0]["status"], "adopted");
    let manifest = fs::read(fixture.project.join("arc.toml")).unwrap();
    let before = inventory(fixture._temp.path());
    fixture.json(&["project", "clean", "--dry-run"], 0);
    assert_eq!(inventory(fixture._temp.path()), before);
    fixture.json(&["project", "clean"], 0);
    assert_eq!(
        fs::read(fixture.project.join("arc.toml")).unwrap(),
        manifest
    );
    assert_eq!(fs::read_to_string(manual.join("notes")).unwrap(), "keep");
    fixture.json(&["project", "apply", "--agent", "codex"], 0);
    fs::remove_file(fixture.project.join("arc.toml")).unwrap();
    fixture.json(
        &[
            "project",
            "clean",
            "--project-root",
            fixture.project.to_str().unwrap(),
        ],
        0,
    );
    assert!(!fixture.target().is_symlink());
    assert_eq!(fs::read_to_string(manual.join("notes")).unwrap(), "keep");
}

#[test]
fn concurrent_projects_keep_both_records_and_global_uninstall_is_scoped() {
    let fixture = Fixture::new();
    let other = fixture._temp.path().join("second-project");
    fs::create_dir_all(&other).unwrap();
    fs::copy(fixture.project.join("arc.toml"), other.join("arc.toml")).unwrap();
    let state = fixture.home.join(".arc-cli/state/skills");
    fs::create_dir_all(&state).unwrap();
    let lock = fs::File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(state.join("installs.lock"))
        .unwrap();
    lock.lock().unwrap();
    let mut first = fixture
        .cmd()
        .args(["project", "apply", "--agent", "codex", "--format", "json"])
        .spawn()
        .unwrap();
    let mut second = fixture
        .cmd()
        .current_dir(&other)
        .args(["project", "apply", "--agent", "codex", "--format", "json"])
        .spawn()
        .unwrap();
    std::thread::sleep(std::time::Duration::from_millis(100));
    assert!(first.try_wait().unwrap().is_none());
    assert!(second.try_wait().unwrap().is_none());
    lock.unlock().unwrap();
    decode(first.wait_with_output().unwrap(), 0);
    decode(second.wait_with_output().unwrap(), 0);
    assert_eq!(fixture.ledger()["installs"].as_array().unwrap().len(), 2);
    fixture.json(&["skill", "install", "demo", "--agent", "codex"], 0);
    assert_eq!(fixture.ledger()["installs"].as_array().unwrap().len(), 3);
    let listing = fixture.json(&["skill", "list", "--installed"], 0);
    assert_eq!(listing["scope"], "global");
    assert_eq!(listing["installations"].as_array().unwrap().len(), 1);
    fixture.json(&["skill", "uninstall", "demo", "--all"], 0);
    assert_eq!(fixture.ledger()["installs"].as_array().unwrap().len(), 2);
    assert!(fixture.target().is_symlink());
    assert!(other.join(".codex/skills/demo").is_symlink());
}

#[test]
fn dry_run_reports_an_active_writer_without_creating_any_state() {
    let fixture = Fixture::new();
    let state = fixture.home.join(".arc-cli/state/skills");
    fs::create_dir_all(&state).unwrap();
    let lock = fs::File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(state.join("installs.lock"))
        .unwrap();
    lock.lock().unwrap();
    let before = inventory(fixture._temp.path());
    let busy = fixture.json(&["project", "apply", "--dry-run", "--agent", "codex"], 1);
    assert_eq!(busy["ok"], false);
    assert!(
        busy["errors"]
            .as_array()
            .unwrap()
            .iter()
            .any(|error| error.as_str().unwrap().contains("busy"))
    );
    assert_eq!(inventory(fixture._temp.path()), before);
    lock.unlock().unwrap();
    fixture.json(&["project", "apply", "--dry-run", "--agent", "codex"], 0);
    assert_eq!(inventory(fixture._temp.path()), before);
}

#[test]
fn status_reports_corrupt_tracking_even_outside_a_project_without_mutation() {
    let fixture = Fixture::new();
    fs::remove_file(fixture.project.join("arc.toml")).unwrap();
    let state = fixture.home.join(".arc-cli/state/skills");
    fs::create_dir_all(&state).unwrap();
    fs::write(state.join("installs.json"), "corrupt metadata").unwrap();
    let before = inventory(fixture._temp.path());
    let output = fixture.json(&["status"], 0);
    assert!(
        output["tracking"]["error"]
            .as_str()
            .unwrap()
            .contains("corrupt")
    );
    assert_eq!(inventory(fixture._temp.path()), before);
    fixture.json(
        &[
            "project",
            "clean",
            "--dry-run",
            "--project-root",
            fixture.project.to_str().unwrap(),
        ],
        1,
    );
    assert_eq!(inventory(fixture._temp.path()), before);
}

#[test]
fn cleanup_rejects_unknown_agents_and_conflicting_selection_flags() {
    let fixture = Fixture::new();
    fixture.require(&[]);
    let result = fixture.json(&["project", "clean", "--agent", "typo"], 1);
    assert!(
        result["errors"].as_array().unwrap()[0]
            .as_str()
            .unwrap()
            .contains("unsupported project agent")
    );
    let rejected = fixture
        .cmd()
        .args(["project", "apply", "--agent", "codex", "--all-agents"])
        .output()
        .unwrap();
    assert_eq!(rejected.status.code(), Some(2));
    assert!(!fixture.target().exists());
}

#[test]
fn provider_failure_preserves_completed_skill_results_and_records() {
    let fixture = Fixture::new();
    fs::create_dir_all(fixture.home.join(".arc-cli/providers")).unwrap();
    fs::write(fixture.home.join(".arc-cli/providers/codex.toml"), "[proxy]\ndisplay_name = \"Proxy\"\ndescription = \"Test\"\nbase_url = \"https://example.com\"\napi_key = \"sk-test\"\n").unwrap();
    fs::create_dir_all(fixture.home.join(".codex")).unwrap();
    fs::write(
        fixture.home.join(".codex/config.toml"),
        "model_providers = \"broken\"\n",
    )
    .unwrap();
    fs::write(
        fixture.project.join("arc.toml"),
        "[provider]\nname = \"proxy\"\n[skills]\nrequire = [\"demo\"]\n",
    )
    .unwrap();
    let output = fixture.json(&["project", "apply", "--agent", "codex"], 1);
    assert_eq!(output["ok"], false);
    assert_eq!(output["items"][0]["status"], "installed");
    assert!(
        output["errors"]
            .as_array()
            .unwrap()
            .iter()
            .any(|error| error.as_str().unwrap().contains("provider switch failed"))
    );
    assert!(fixture.target().is_symlink());
    assert_eq!(fixture.ledger()["installs"].as_array().unwrap().len(), 1);
}
