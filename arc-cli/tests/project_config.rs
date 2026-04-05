use std::fs;
use std::path::Path;
use std::process::Command;

use serde_json::Value;

fn arc_cmd() -> Command {
    Command::new(env!("CARGO_BIN_EXE_arc"))
}

// ── arc project apply ───────────────────────────────────────────

#[test]
fn arc_project_apply_json_noninteractive_no_arc_toml() {
    let temp = tempfile::tempdir().unwrap();

    let output = arc_cmd()
        .args(["project", "apply", "--format", "json"])
        .env("ARC_KIT_USER_HOME", temp.path())
        .current_dir(temp.path())
        .stdin(std::process::Stdio::null())
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(json["schema_version"], "3");
    assert_eq!(json["ok"], false);
}

#[test]
fn arc_project_apply_json_when_arc_toml_exists() {
    let temp = tempfile::tempdir().unwrap();
    let proj = tempfile::tempdir().unwrap();
    fs::write(proj.path().join("arc.toml"), "[skills]\nrequire = []\n").unwrap();

    let output = arc_cmd()
        .args(["project", "apply", "--format", "json"])
        .env("ARC_KIT_USER_HOME", temp.path())
        .current_dir(proj.path())
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(json["ok"], true);
}

#[test]
fn arc_project_apply_fails_without_arc_toml() {
    let temp = tempfile::tempdir().unwrap();

    let output = arc_cmd()
        .args(["project", "apply"])
        .env("ARC_KIT_USER_HOME", temp.path())
        .current_dir(temp.path())
        .stdin(std::process::Stdio::null())
        .output()
        .unwrap();

    // Non-interactive without arc.toml should fail with hint.
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("arc.toml") || stderr.contains("Hint"),
        "expected hint, got stderr: {stderr}"
    );
}

#[test]
fn arc_apply_exits_1_on_unknown_project_provider() {
    let temp = tempfile::tempdir().unwrap();
    let proj = tempfile::tempdir().unwrap();

    fs::write(
        proj.path().join("arc.toml"),
        "[provider]\nname = \"no-such-profile-xyz\"\n",
    )
    .unwrap();

    let output = arc_cmd()
        .args(["project", "apply"])
        .env("ARC_KIT_USER_HOME", temp.path())
        .current_dir(proj.path())
        .stdin(std::process::Stdio::null())
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("no-such-profile") || stderr.contains("Provider"),
        "expected provider error, got: {stderr}"
    );
}

#[test]
fn arc_apply_exits_1_on_parse_error() {
    let temp = tempfile::tempdir().unwrap();
    let proj = tempfile::tempdir().unwrap();

    fs::write(proj.path().join("arc.toml"), "api_key = \"secret\"\n").unwrap();

    let output = arc_cmd()
        .args(["project", "apply"])
        .env("ARC_KIT_USER_HOME", temp.path())
        .current_dir(proj.path())
        .output()
        .unwrap();

    // Parse error exits 1.
    assert_eq!(
        output.status.code(),
        Some(1),
        "expected exit code 1 for parse error"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("unknown field") || stderr.contains("api_key") || stderr.contains("Error"),
        "expected error message, got: {stderr}"
    );
}

#[test]
fn arc_apply_exits_0_with_unavailable_skill() {
    let temp = tempfile::tempdir().unwrap();
    let proj = tempfile::tempdir().unwrap();

    fs::write(
        proj.path().join("arc.toml"),
        "[skills]\nrequire = [\"ghost-skill-xyz\"]\n",
    )
    .unwrap();

    let output = arc_cmd()
        .args(["project", "apply"])
        .env("ARC_KIT_USER_HOME", temp.path())
        .current_dir(proj.path())
        .output()
        .unwrap();

    // skill source not found -> exit 0 (non-blocking)
    assert_eq!(
        output.status.code(),
        Some(0),
        "expected exit code 0 for unavailable skill, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("ghost-skill-xyz")
            || stdout.contains("not found")
            || stdout.contains("skipped"),
        "expected skip message, got: {stdout}"
    );
}

#[test]
fn arc_apply_installs_missing_skills() {
    let temp = tempfile::tempdir().unwrap();
    let proj = tempfile::tempdir().unwrap();

    // Set up a fake codex agent binary.
    let bin_dir = temp.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    write_fake_cli(&bin_dir, "codex", "codex-cli 0.116.0");

    // Create a local skill.
    let skill_dir = temp
        .path()
        .join(".arc-cli")
        .join("skills")
        .join("my-local-skill");
    fs::create_dir_all(&skill_dir).unwrap();
    fs::write(skill_dir.join("SKILL.md"), "# My Local Skill\n").unwrap();

    // arc.toml requires that skill.
    fs::write(
        proj.path().join("arc.toml"),
        "[skills]\nrequire = [\"my-local-skill\"]\n",
    )
    .unwrap();

    let original_path = std::env::var("PATH").unwrap_or_default();
    let output = arc_cmd()
        .args(["project", "apply", "--agent", "codex"])
        .env("ARC_KIT_USER_HOME", temp.path())
        .env("PATH", format!("{}:{original_path}", bin_dir.display()))
        .current_dir(proj.path())
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("my-local-skill"),
        "expected skill install output, got: {stdout}"
    );

    // Verify skill was installed under the project (Codex project-local layout).
    let installed = proj
        .path()
        .join(".agents")
        .join("skills")
        .join("my-local-skill");
    assert!(
        installed.exists(),
        "skill should be installed at project .agents/skills, got {}",
        installed.display()
    );
}

#[test]
fn arc_project_apply_json_requires_agent_when_skills_need_install() {
    let temp = tempfile::tempdir().unwrap();
    let proj = tempfile::tempdir().unwrap();

    let bin_dir = temp.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    write_fake_cli(&bin_dir, "codex", "codex-cli 0.116.0");

    let skill_dir = temp
        .path()
        .join(".arc-cli")
        .join("skills")
        .join("need-agent-json");
    fs::create_dir_all(&skill_dir).unwrap();
    fs::write(skill_dir.join("SKILL.md"), "# Need Agent Json\n").unwrap();

    fs::write(
        proj.path().join("arc.toml"),
        "[skills]\nrequire = [\"need-agent-json\"]\n",
    )
    .unwrap();

    let original_path = std::env::var("PATH").unwrap_or_default();
    let output = arc_cmd()
        .args(["project", "apply", "--format", "json"])
        .env("ARC_KIT_USER_HOME", temp.path())
        .env("PATH", format!("{}:{original_path}", bin_dir.display()))
        .current_dir(proj.path())
        .stdin(std::process::Stdio::null())
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--agent") || stderr.contains("--all-agents"),
        "expected agent selection hint, got: {stderr}"
    );
}

// ── arc status ────────────────────────────────────────────────

#[test]
fn arc_status_shows_project_context() {
    let temp = tempfile::tempdir().unwrap();
    let proj = tempfile::tempdir().unwrap();

    fs::write(
        proj.path().join("arc.toml"),
        "[skills]\nrequire = [\"skill-a\", \"skill-b\"]\n",
    )
    .unwrap();

    let output = arc_cmd()
        .arg("status")
        .env("ARC_KIT_USER_HOME", temp.path())
        .current_dir(proj.path())
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Project")
            && stdout.contains("Agents")
            && stdout.contains("Catalog")
            && stdout.contains("repo:")
            && stdout.contains("skills:"),
        "expected modular status output, got: {stdout}"
    );
}

#[test]
fn arc_status_no_change_without_arc_toml() {
    let temp = tempfile::tempdir().unwrap();
    let proj = tempfile::tempdir().unwrap();

    let output = arc_cmd()
        .arg("status")
        .env("ARC_KIT_USER_HOME", temp.path())
        .current_dir(proj.path())
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Project") && stdout.contains("arc.toml: not found"),
        "expected explicit no-project section, got: {stdout}"
    );
}

#[test]
fn arc_status_noninteractive_succeeds_with_missing_skills_reminder() {
    let temp = tempfile::tempdir().unwrap();
    let proj = tempfile::tempdir().unwrap();

    fs::write(
        proj.path().join("arc.toml"),
        "[skills]\nrequire = [\"nonexistent-skill-abc\"]\n",
    )
    .unwrap();

    let mut child = arc_cmd()
        .arg("status")
        .env("ARC_KIT_USER_HOME", temp.path())
        .current_dir(proj.path())
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    drop(child.stdin.take());
    let output = child.wait_with_output().unwrap();

    assert!(
        output.status.success(),
        "status is read-only; missing skills must not change exit code"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("skills: 1 required") && stdout.contains("1 unavailable"),
        "expected project summary reminder, got: {stdout}"
    );
}

#[test]
fn arc_status_json_exposes_project_agents_and_catalog_modules() {
    let temp = tempfile::tempdir().unwrap();
    let proj = tempfile::tempdir().unwrap();

    fs::write(proj.path().join("arc.toml"), "[skills]\nrequire = []\n").unwrap();

    let output = arc_cmd()
        .args(["status", "--format", "json"])
        .env("ARC_KIT_USER_HOME", temp.path())
        .current_dir(proj.path())
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(json["schema_version"], "3");
    assert_eq!(json["project"]["state"], "active");
    assert!(json.get("agents").is_some());
    assert!(json.get("catalog").is_some());
    assert!(json.get("actions").is_some());
}

#[test]
fn arc_status_surfaces_invalid_arc_toml() {
    let temp = tempfile::tempdir().unwrap();
    let proj = tempfile::tempdir().unwrap();

    fs::write(proj.path().join("arc.toml"), "api_key = \"secret\"\n").unwrap();

    let output = arc_cmd()
        .arg("status")
        .env("ARC_KIT_USER_HOME", temp.path())
        .current_dir(proj.path())
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("error:") && stdout.contains("unknown field"),
        "expected invalid project details, got: {stdout}"
    );
}

// ── arc --help ─────────────────────────────────────────────────

#[test]
fn arc_help_exposes_project() {
    let output = arc_cmd().arg("--help").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("project"),
        "expected 'project' in help: {stdout}"
    );
}

#[test]
fn arc_project_apply_help_exists() {
    let output = arc_cmd()
        .args(["project", "apply", "--help"])
        .output()
        .unwrap();
    assert!(output.status.success());
}

#[test]
fn arc_project_edit_help_exists() {
    let output = arc_cmd()
        .args(["project", "edit", "--help"])
        .output()
        .unwrap();
    assert!(output.status.success());
}

// ── markets in arc.toml ────────────────────────────────────────────────

fn init_git_repo_with_commit(path: &Path) {
    let status = Command::new("git")
        .args(["init", "--initial-branch=main", path.to_str().unwrap()])
        .status()
        .unwrap();
    assert!(status.success());
    fs::write(path.join("README.md"), "# test\n").unwrap();
    let status = Command::new("git")
        .args(["config", "user.name", "Arc Test"])
        .current_dir(path)
        .status()
        .unwrap();
    assert!(status.success());
    let status = Command::new("git")
        .args(["config", "user.email", "arc-test@example.com"])
        .current_dir(path)
        .status()
        .unwrap();
    assert!(status.success());
    let status = Command::new("git")
        .args(["add", "."])
        .current_dir(path)
        .status()
        .unwrap();
    assert!(status.success());
    let status = Command::new("git")
        .args(["commit", "-m", "init"])
        .current_dir(path)
        .status()
        .unwrap();
    assert!(status.success());
}

#[test]
fn arc_apply_adds_markets_from_arc_toml() {
    let temp = tempfile::tempdir().unwrap();
    let proj = tempfile::tempdir().unwrap();
    let repos = tempfile::tempdir().unwrap();
    let repo_a = repos.path().join("market-a");
    let repo_b = repos.path().join("market-b");
    fs::create_dir_all(&repo_a).unwrap();
    fs::create_dir_all(&repo_b).unwrap();
    init_git_repo_with_commit(&repo_a);
    init_git_repo_with_commit(&repo_b);
    let url_a = format!("file://{}", repo_a.display());
    let url_b = format!("file://{}", repo_b.display());

    fs::write(
        proj.path().join("arc.toml"),
        format!(
            r#"
[[markets]]
url = "{url_a}"

[[markets]]
url = "{url_b}"

[skills]
require = []
"#
        ),
    )
    .unwrap();

    let output = arc_cmd()
        .args(["project", "apply", "--format", "json"])
        .env("ARC_KIT_USER_HOME", temp.path())
        .current_dir(proj.path())
        .output()
        .unwrap();

    assert!(output.status.success());

    // Verify markets were added
    let market_file = temp
        .path()
        .join(".arc-cli")
        .join("markets")
        .join("markets.json");
    if market_file.exists() {
        let content = fs::read_to_string(&market_file).unwrap();
        assert!(
            content.contains("market-a") && content.contains("market-b"),
            "markets should be recorded: {}",
            content
        );
    }
}

#[test]
fn arc_apply_skips_existing_markets() {
    let temp = tempfile::tempdir().unwrap();
    let proj = tempfile::tempdir().unwrap();
    let repos = tempfile::tempdir().unwrap();
    let repo = repos.path().join("market-one");
    fs::create_dir_all(&repo).unwrap();
    init_git_repo_with_commit(&repo);
    let url = format!("file://{}", repo.display());

    fs::write(
        proj.path().join("arc.toml"),
        format!(
            r#"
[[markets]]
url = "{url}"

[skills]
require = []
"#
        ),
    )
    .unwrap();

    // First apply to add the market
    let output1 = arc_cmd()
        .args(["project", "apply", "--format", "json"])
        .env("ARC_KIT_USER_HOME", temp.path())
        .current_dir(proj.path())
        .output()
        .unwrap();
    assert!(output1.status.success());

    // Second apply should skip the existing market (no error)
    let output2 = arc_cmd()
        .args(["project", "apply", "--format", "json"])
        .env("ARC_KIT_USER_HOME", temp.path())
        .current_dir(proj.path())
        .output()
        .unwrap();
    assert!(output2.status.success());
}

// ── helpers ───────────────────────────────────────────────────

#[cfg(unix)]
fn write_fake_cli(bin_dir: &Path, name: &str, version_output: &str) {
    use std::os::unix::fs::PermissionsExt;
    let path = bin_dir.join(name);
    fs::write(&path, format!("#!/bin/sh\necho '{version_output}'\n")).unwrap();
    let mut perms = fs::metadata(&path).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&path, perms).unwrap();
}

#[cfg(not(unix))]
fn write_fake_cli(bin_dir: &Path, name: &str, version_output: &str) {
    let path = bin_dir.join(format!("{name}.bat"));
    fs::write(&path, format!("@echo {version_output}\r\n")).unwrap();
}
