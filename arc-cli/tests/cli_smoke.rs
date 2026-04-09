use std::fs;
use std::path::Path;
use std::process::Command;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

fn arc_cmd() -> Command {
    Command::new(env!("CARGO_BIN_EXE_arc"))
}

fn arc_cmd_with_home(home: &Path) -> Command {
    let mut cmd = arc_cmd();
    cmd.env("ARC_KIT_USER_HOME", home);
    cmd.env("ARC_KIT_BUILTIN_MANIFEST_URL", empty_builtin_manifest(home));
    cmd
}

fn empty_builtin_manifest(home: &Path) -> String {
    let builtin_dir = home.join("built-in");
    let market_dir = builtin_dir.join("market");
    let manifest = builtin_dir.join("manifest.toml");
    let index = market_dir.join("index.toml");
    fs::create_dir_all(&market_dir).unwrap();
    fs::write(
        &manifest,
        "version = 1\n\n[index.market]\npath = \"market/index.toml\"\n",
    )
    .unwrap();
    fs::write(&index, "version = 1\nupdated_at = \"2026-04-09\"\n").unwrap();
    format!("file://{}", manifest.display())
}

#[test]
fn help_command_exposes_primary_commands() {
    let output = arc_cmd().arg("--help").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("market"));
    assert!(stdout.contains("skill"));
    assert!(stdout.contains("provider"));
    assert!(stdout.contains("project"));
    assert!(stdout.contains("mcp"));
    assert!(stdout.contains("subagent"));
    assert!(stdout.contains("completion"));
    assert!(stdout.contains("status"));
    assert!(
        !stdout
            .lines()
            .any(|line| line.trim_start().starts_with("sync"))
    );
}

#[test]
fn provider_test_runs_without_error() {
    let temp = tempfile::tempdir().unwrap();
    let output = arc_cmd()
        .args(["provider", "test"])
        .env("ARC_KIT_USER_HOME", temp.path())
        .output()
        .unwrap();
    assert!(output.status.success());
}

#[test]
fn provider_test_json_output() {
    let temp = tempfile::tempdir().unwrap();
    let output = arc_cmd()
        .args(["provider", "test", "--format", "json"])
        .env("ARC_KIT_USER_HOME", temp.path())
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("invalid JSON: {e}, output: {stdout}"));
    assert_eq!(json["schema_version"], "4");
}

#[test]
fn provider_help_exposes_test_command() {
    let output = arc_cmd().args(["provider", "--help"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("test"),
        "expected 'test' in provider help: {stdout}"
    );
}

#[test]
fn provider_list_works_with_temp_home() {
    let temp = tempfile::tempdir().unwrap();
    let output = arc_cmd()
        .arg("provider")
        .arg("list")
        .env("ARC_KIT_USER_HOME", temp.path())
        .output()
        .unwrap();
    assert!(output.status.success());
}

#[test]
fn provider_use_json_requires_name_even_in_tty_like_environment() {
    let temp = tempfile::tempdir().unwrap();
    let output = arc_cmd_with_home(temp.path())
        .args(["provider", "use", "--format", "json"])
        .stdin(std::process::Stdio::null())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Provider name required"));
}

#[test]
fn status_auto_initializes() {
    let temp = tempfile::tempdir().unwrap();
    let output = arc_cmd()
        .arg("status")
        .env("ARC_KIT_USER_HOME", temp.path())
        .current_dir(temp.path())
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(temp.path().join(".arc-cli").exists());
}

#[test]
fn bare_command_prints_help_like_arc_help() {
    let help = arc_cmd().arg("--help").output().unwrap();
    let bare = arc_cmd().output().unwrap();
    assert!(help.status.success());
    assert!(bare.status.success());
    assert_eq!(
        help.stdout, bare.stdout,
        "bare `arc` must match `arc --help` stdout"
    );
    assert_eq!(help.stderr, bare.stderr);
}

#[test]
fn bare_skill_runs_list() {
    let temp = tempfile::tempdir().unwrap();
    let output = arc_cmd_with_home(temp.path())
        .arg("skill")
        .output()
        .unwrap();
    assert!(output.status.success());
}

#[test]
fn bare_market_runs_list() {
    let temp = tempfile::tempdir().unwrap();
    let output = arc_cmd()
        .arg("market")
        .env("ARC_KIT_USER_HOME", temp.path())
        .output()
        .unwrap();
    assert!(output.status.success());
}

#[test]
fn completion_writes_file() {
    let temp = tempfile::tempdir().unwrap();
    let output = arc_cmd()
        .args(["completion", "zsh"])
        .env("ARC_KIT_USER_HOME", temp.path())
        .output()
        .unwrap();
    assert!(output.status.success());
    let file = temp.path().join(".arc-cli/completions/arc.zsh");
    assert!(file.exists(), "completion file should be created");
    let content = std::fs::read_to_string(&file).unwrap();
    assert!(
        content.contains("arc"),
        "completion file should reference arc"
    );
}

#[test]
fn skill_list_installed_flag() {
    let temp = tempfile::tempdir().unwrap();
    let output = arc_cmd_with_home(temp.path())
        .args(["skill", "list", "--installed"])
        .output()
        .unwrap();
    assert!(output.status.success());
}

#[test]
fn skill_list_uses_builtin_market_index_without_local_catalog() {
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path().join("example-market");
    let builtin_dir = temp.path().join("built-in");
    let market_dir = builtin_dir.join("market");
    let manifest = builtin_dir.join("manifest.toml");
    let index = market_dir.join("index.toml");

    run_git(
        &["init", "--initial-branch=main", repo.to_str().unwrap()],
        temp.path(),
    );
    run_git(&["config", "user.name", "Arc Test"], &repo);
    run_git(&["config", "user.email", "arc-test@example.com"], &repo);
    std::fs::create_dir_all(repo.join("demo-skill")).unwrap();
    std::fs::write(
        repo.join("demo-skill").join("SKILL.md"),
        "---\ndescription: demo summary\n---\n# Demo\n",
    )
    .unwrap();
    run_git(&["add", "."], &repo);
    run_git(&["commit", "-m", "initial"], &repo);

    std::fs::create_dir_all(&market_dir).unwrap();
    std::fs::write(
        &manifest,
        "version = 1\n\n[index.market]\npath = \"market/index.toml\"\n",
    )
    .unwrap();
    std::fs::write(
        &index,
        format!(
            "version = 1\nupdated_at = \"2026-03-26\"\n\n[[repo]]\ngit_url = \"file://{}\"\n",
            repo.display()
        ),
    )
    .unwrap();

    let output = arc_cmd()
        .arg("skill")
        .arg("list")
        .env("ARC_KIT_USER_HOME", temp.path())
        .env(
            "ARC_KIT_BUILTIN_MANIFEST_URL",
            format!("file://{}", manifest.display()),
        )
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Bootstrapped 1 market sources and indexed 1 resources"));
    assert!(stdout.contains("demo-skill"));
}

#[test]
fn skill_install_json_requires_name() {
    let temp = tempfile::tempdir().unwrap();
    let output = arc_cmd_with_home(temp.path())
        .args(["skill", "install", "--format", "json"])
        .stdin(std::process::Stdio::null())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Skill name required"));
}

#[test]
fn skill_uninstall_json_requires_name() {
    let temp = tempfile::tempdir().unwrap();
    let output = arc_cmd_with_home(temp.path())
        .args(["skill", "uninstall", "--format", "json"])
        .stdin(std::process::Stdio::null())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Skill name required"));
}

#[test]
fn mcp_info_missing_returns_structured_json_error() {
    let temp = tempfile::tempdir().unwrap();
    let output = arc_cmd_with_home(temp.path())
        .args(["mcp", "info", "missing", "--format", "json"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(json["schema_version"], "4");
    assert_eq!(json["ok"], false);
    assert!(json["error"].as_str().unwrap().contains("not found"));
}

#[test]
fn subagent_info_missing_returns_structured_json_error() {
    let temp = tempfile::tempdir().unwrap();
    let output = arc_cmd_with_home(temp.path())
        .args(["subagent", "info", "missing", "--format", "json"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(json["schema_version"], "4");
    assert_eq!(json["ok"], false);
    assert!(json["error"].as_str().unwrap().contains("not found"));
}

#[test]
fn skill_install_symlinks_directory_for_codex() {
    let temp = tempfile::tempdir().unwrap();
    let bin_dir = temp.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    write_fake_cli(&bin_dir, "codex", "codex-cli 0.116.0");

    let local_skill = temp
        .path()
        .join(".arc-cli")
        .join("skills")
        .join("demo-skill");
    fs::create_dir_all(&local_skill).unwrap();
    fs::write(local_skill.join("SKILL.md"), "# Demo\n").unwrap();
    fs::write(local_skill.join("tool.txt"), "payload").unwrap();

    let output = arc_cmd_with_home(temp.path())
        .args(["skill", "install", "demo-skill", "--agent", "codex"])
        .env(
            "PATH",
            format!("{}:{}", bin_dir.display(), std::env::var("PATH").unwrap()),
        )
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let target = temp.path().join(".codex").join("skills").join("demo-skill");
    assert!(target.symlink_metadata().unwrap().file_type().is_symlink());
    assert_eq!(
        fs::read_to_string(target.join("tool.txt")).unwrap(),
        "payload"
    );
}

#[test]
fn mcp_install_writes_global_definition_and_codex_config() {
    let temp = tempfile::tempdir().unwrap();
    let bin_dir = temp.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    write_fake_cli(&bin_dir, "codex", "codex-cli 0.116.0");

    let output = arc_cmd()
        .args([
            "mcp",
            "install",
            "filesystem",
            "--agent",
            "codex",
            "--transport",
            "stdio",
            "--command",
            "npx",
            "--arg",
            "-y",
            "--arg",
            "@modelcontextprotocol/server-filesystem",
        ])
        .env("ARC_KIT_USER_HOME", temp.path())
        .env(
            "PATH",
            format!("{}:{}", bin_dir.display(), std::env::var("PATH").unwrap()),
        )
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let canonical = temp.path().join(".arc-cli/mcps/filesystem.toml");
    let codex_config = temp.path().join(".codex/config.toml");
    assert!(canonical.exists());
    assert!(codex_config.exists());
    let canonical_body = fs::read_to_string(canonical).unwrap();
    let codex_body = fs::read_to_string(codex_config).unwrap();
    assert!(canonical_body.contains("transport = \"stdio\""));
    assert!(codex_body.contains("[mcp_servers.filesystem]"));
    assert!(codex_body.contains("command = \"npx\""));
}

#[test]
fn mcp_install_rejects_plaintext_secret_headers() {
    let temp = tempfile::tempdir().unwrap();
    let output = arc_cmd()
        .args([
            "mcp",
            "install",
            "github",
            "--transport",
            "streamable-http",
            "--url",
            "https://api.github.com/mcp",
            "--header",
            "Authorization=Bearer secret",
        ])
        .env("ARC_KIT_USER_HOME", temp.path())
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("environment placeholder"));
}

#[test]
fn mcp_install_rejects_unknown_target_agent() {
    let temp = tempfile::tempdir().unwrap();
    let output = arc_cmd()
        .args([
            "mcp",
            "install",
            "filesystem",
            "--agent",
            "codxe",
            "--transport",
            "stdio",
            "--command",
            "npx",
        ])
        .env("ARC_KIT_USER_HOME", temp.path())
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("unsupported target agent"));
}

#[test]
fn subagent_install_writes_global_definition_and_codex_agent() {
    let temp = tempfile::tempdir().unwrap();
    let bin_dir = temp.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    write_fake_cli(&bin_dir, "codex", "codex-cli 0.116.0");
    let prompt = temp.path().join("reviewer.md");
    fs::write(&prompt, "# Reviewer\n\nReview the diff carefully.\n").unwrap();

    let output = arc_cmd()
        .args([
            "subagent",
            "install",
            "reviewer",
            "--agent",
            "codex",
            "--description",
            "Review diffs",
            "--prompt-file",
            prompt.to_str().unwrap(),
        ])
        .env("ARC_KIT_USER_HOME", temp.path())
        .env(
            "PATH",
            format!("{}:{}", bin_dir.display(), std::env::var("PATH").unwrap()),
        )
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let canonical_meta = temp.path().join(".arc-cli/subagents/reviewer.toml");
    let canonical_prompt = temp.path().join(".arc-cli/subagents/reviewer.md");
    let codex_agent = temp.path().join(".codex/agents/reviewer.toml");
    assert!(canonical_meta.exists());
    assert!(canonical_prompt.exists());
    assert!(codex_agent.exists());
    let body = fs::read_to_string(codex_agent).unwrap();
    assert!(body.contains("name = \"reviewer\""));
    assert!(body.contains("developer_instructions"));
}

#[test]
fn subagent_install_rejects_unknown_target_agent() {
    let temp = tempfile::tempdir().unwrap();
    let prompt = temp.path().join("reviewer.md");
    fs::write(&prompt, "# Reviewer\n").unwrap();

    let output = arc_cmd()
        .args([
            "subagent",
            "install",
            "reviewer",
            "--agent",
            "codxe",
            "--prompt-file",
            prompt.to_str().unwrap(),
        ])
        .env("ARC_KIT_USER_HOME", temp.path())
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("unsupported target agent"));
}

fn run_git(args: &[&str], cwd: &Path) {
    let status = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .status()
        .unwrap();
    assert!(
        status.success(),
        "git {:?} failed in {}",
        args,
        cwd.display()
    );
}

// ── bare provider now defaults to list ──────────

#[test]
fn bare_provider_runs_list() {
    let temp = tempfile::tempdir().unwrap();
    let output = arc_cmd()
        .arg("provider")
        .env("ARC_KIT_USER_HOME", temp.path())
        .output()
        .unwrap();
    assert!(output.status.success());
}

// ── JSON output smoke tests ────────────────────────────

#[test]
fn apply_json_output() {
    let temp = tempfile::tempdir().unwrap();
    let proj = tempfile::tempdir().unwrap();
    let output = arc_cmd()
        .args(["project", "apply", "--format", "json"])
        .env("ARC_KIT_USER_HOME", temp.path())
        .current_dir(proj.path())
        .stdin(std::process::Stdio::null())
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(json["schema_version"], "4");
}

// ── non-TTY guard smoke tests ──────────────────────────

#[test]
fn skill_install_no_name_noninteractive_fails() {
    let temp = tempfile::tempdir().unwrap();
    let output = arc_cmd_with_home(temp.path())
        .args(["skill", "install"])
        .output()
        .unwrap();
    // Running in test = non-TTY, so should fail with helpful error
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Skill name required"));
}

#[test]
fn skill_uninstall_no_name_noninteractive_fails() {
    let temp = tempfile::tempdir().unwrap();
    let output = arc_cmd()
        .args(["skill", "uninstall"])
        .env("ARC_KIT_USER_HOME", temp.path())
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Skill name required"));
}

#[test]
fn provider_use_no_name_noninteractive_fails() {
    let temp = tempfile::tempdir().unwrap();
    let output = arc_cmd()
        .args(["provider", "use"])
        .env("ARC_KIT_USER_HOME", temp.path())
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Provider name required"));
}

fn write_fake_cli(bin_dir: &Path, name: &str, version_output: &str) {
    let path = bin_dir.join(name);
    fs::write(&path, format!("#!/bin/sh\necho '{version_output}'\n")).unwrap();
    #[cfg(unix)]
    {
        let mut perms = fs::metadata(&path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&path, perms).unwrap();
    }
}
