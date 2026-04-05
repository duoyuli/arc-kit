use std::path::Path;
use std::process::Command;

use arc_core::git::{GitRepo, clone, validate_git_url};

#[test]
fn validate_git_url_accepts_supported_prefixes() {
    assert!(validate_git_url("https://github.com/openai/codex.git"));
    assert!(validate_git_url("git://example.com/repo.git"));
    assert!(validate_git_url(
        "ssh://git@git.example.com/acme/toolkit.git"
    ));
    assert!(validate_git_url("git@github.com:openai/codex.git"));
    assert!(validate_git_url("file:///tmp/repo"));
}

#[test]
fn validate_git_url_rejects_other_prefixes() {
    assert!(!validate_git_url("/tmp/repo"));
    assert!(!validate_git_url("ftp://example.com/repo.git"));
}

#[test]
fn remote_default_branch_and_pull_support_master() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source");
    let checkout = temp.path().join("checkout");

    run_git(
        &["init", "--initial-branch=master", source.to_str().unwrap()],
        temp.path(),
    );
    run_git(&["config", "user.name", "Arc Test"], &source);
    run_git(&["config", "user.email", "arc-test@example.com"], &source);

    std::fs::write(source.join("README.md"), "v1\n").unwrap();
    run_git(&["add", "README.md"], &source);
    run_git(&["commit", "-m", "initial"], &source);

    clone(source.to_str().unwrap(), &checkout, None).unwrap();

    std::fs::write(source.join("README.md"), "v2\n").unwrap();
    run_git(&["add", "README.md"], &source);
    run_git(&["commit", "-m", "update"], &source);

    let repo = GitRepo::new(&checkout);
    assert_eq!(repo.remote_default_branch("origin").unwrap(), "master");
    repo.pull_default_branch("origin").unwrap();

    assert_eq!(
        std::fs::read_to_string(checkout.join("README.md")).unwrap(),
        "v2\n"
    );
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
