use std::path::Path;
use std::process::Command;

use arc_core::market::bootstrap::ensure_local_catalog;
use arc_core::market::catalog::CatalogManager;
use arc_core::market::sources::MarketSourceRegistry;
use arc_core::paths::ArcPaths;

#[test]
fn ensure_local_catalog_clones_and_scans_builtin_repos() {
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path().join("example-market");
    let home = temp.path().join("home");

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

    let paths = ArcPaths::with_user_home(&home);
    std::fs::create_dir_all(paths.markets_dir()).unwrap();
    std::fs::write(
        paths.market_index_cache(),
        format!(
            "version = 1\nupdated_at = \"2026-03-26\"\n\n[[repo]]\ngit_url = \"file://{}\"\n",
            repo.display()
        ),
    )
    .unwrap();

    let report = ensure_local_catalog(&paths).unwrap();

    assert_eq!(report.source_count, 1);
    assert_eq!(report.cloned_count, 1);
    assert_eq!(report.resource_count, 1);
    let resources = CatalogManager::new(paths).get_resources(None);
    assert_eq!(resources.len(), 1);
    assert_eq!(resources[0].name, "demo-skill");
}

#[test]
fn ensure_local_catalog_resyncs_when_a_new_market_source_is_added() {
    let temp = tempfile::tempdir().unwrap();
    let repo_a = temp.path().join("market-a");
    let repo_b = temp.path().join("market-b");
    let home = temp.path().join("home");

    for repo in [&repo_a, &repo_b] {
        run_git(
            &["init", "--initial-branch=main", repo.to_str().unwrap()],
            temp.path(),
        );
        run_git(&["config", "user.name", "Arc Test"], repo);
        run_git(&["config", "user.email", "arc-test@example.com"], repo);
    }

    std::fs::create_dir_all(repo_a.join("skill-alpha")).unwrap();
    std::fs::write(
        repo_a.join("skill-alpha").join("SKILL.md"),
        "---\ndescription: alpha summary\n---\n# Alpha\n",
    )
    .unwrap();
    run_git(&["add", "."], &repo_a);
    run_git(&["commit", "-m", "a"], &repo_a);

    std::fs::create_dir_all(repo_b.join("skill-beta")).unwrap();
    std::fs::write(
        repo_b.join("skill-beta").join("SKILL.md"),
        "---\ndescription: beta summary\n---\n# Beta\n",
    )
    .unwrap();
    run_git(&["add", "."], &repo_b);
    run_git(&["commit", "-m", "b"], &repo_b);

    let paths = ArcPaths::with_user_home(&home);
    std::fs::create_dir_all(paths.markets_dir()).unwrap();
    std::fs::write(
        paths.market_index_cache(),
        format!(
            "version = 1\nupdated_at = \"2026-03-26\"\n\n[[repo]]\ngit_url = \"file://{}\"\n",
            repo_a.display()
        ),
    )
    .unwrap();

    let report_first = ensure_local_catalog(&paths).unwrap();
    assert_eq!(report_first.resource_count, 1);

    let registry = MarketSourceRegistry::new(paths.clone());
    registry
        .add(format!("file://{}", repo_b.display()).as_str(), "auto")
        .unwrap();

    let report_second = ensure_local_catalog(&paths).unwrap();
    assert_eq!(report_second.resource_count, 2);

    let names: Vec<_> = CatalogManager::new(paths.clone())
        .get_resources(None)
        .into_iter()
        .map(|r| r.name)
        .collect();
    assert!(names.contains(&"skill-alpha".to_string()));
    assert!(names.contains(&"skill-beta".to_string()));
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
