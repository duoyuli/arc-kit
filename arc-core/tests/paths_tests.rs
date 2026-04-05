use arc_core::models::MarketSource;
use arc_core::paths::{ARC_CLI_HOME, ArcPaths, expand_user_path};

#[test]
fn with_user_home_builds_arc_layout() {
    let home = std::path::PathBuf::from("/tmp/test-home");
    let paths = ArcPaths::with_user_home(&home);

    assert_eq!(paths.user_home(), home.as_path());
    assert_eq!(paths.home(), home.join(ARC_CLI_HOME).as_path());
    assert_eq!(paths.markets_dir(), home.join(ARC_CLI_HOME).join("markets"));
    assert_eq!(
        paths.providers_dir(),
        home.join(ARC_CLI_HOME).join("providers")
    );
}

#[test]
fn with_arc_home_uses_parent_as_user_home() {
    let arc_home = std::path::PathBuf::from("/tmp/custom/.arc-cli");
    let paths = ArcPaths::with_arc_home(&arc_home);

    assert_eq!(paths.home(), arc_home.as_path());
    assert_eq!(paths.user_home(), std::path::Path::new("/tmp/custom"));
}

#[test]
fn market_checkout_uses_owner_and_repo_when_available() {
    let paths = ArcPaths::with_user_home("/tmp/test-home");
    let source = MarketSource {
        id: "owner-repo".to_string(),
        git_url: "https://github.com/owner/repo.git".to_string(),
        parser: "auto".to_string(),
        owner: "owner".to_string(),
        repo: "repo".to_string(),
        status: "ok".to_string(),
        last_updated_at: String::new(),
        resource_count: 0,
    };

    assert_eq!(
        paths.market_checkout(&source),
        std::path::PathBuf::from("/tmp/test-home/.arc-cli/markets/repo/owner/repo")
    );
}

#[test]
fn market_checkout_falls_back_to_incomplete_for_unparseable_url() {
    let paths = ArcPaths::with_user_home("/tmp/test-home");
    let source = MarketSource {
        id: "custom".to_string(),
        git_url: "not-a-git-url".to_string(),
        parser: "auto".to_string(),
        owner: String::new(),
        repo: String::new(),
        status: "ok".to_string(),
        last_updated_at: String::new(),
        resource_count: 0,
    };

    assert_eq!(
        paths.market_checkout(&source),
        std::path::PathBuf::from("/tmp/test-home/.arc-cli/markets/repo/_incomplete/custom")
    );
}

#[test]
fn expand_user_path_expands_tilde_prefix() {
    let expanded = expand_user_path("~/demo");
    assert!(expanded.ends_with("demo"));
}

#[test]
fn local_skills_dir_under_arc_home() {
    let home = std::path::PathBuf::from("/tmp/test-home");
    let paths = ArcPaths::with_user_home(&home);
    assert_eq!(
        paths.local_skills_dir(),
        home.join(ARC_CLI_HOME).join("skills")
    );
}

#[test]
fn builtin_cache_dir_under_arc_home() {
    let home = std::path::PathBuf::from("/tmp/test-home");
    let paths = ArcPaths::with_user_home(&home);
    assert_eq!(
        paths.builtin_cache_dir(),
        home.join(ARC_CLI_HOME).join("cache").join("built-in")
    );
}

#[test]
fn ensure_arc_home_creates_state_directory() {
    let temp = tempfile::tempdir().unwrap();
    let paths = ArcPaths::with_user_home(temp.path());
    assert!(!paths.home().exists());
    paths.ensure_arc_home().unwrap();
    assert!(paths.home().exists());
    assert!(paths.markets_dir().exists());
}
