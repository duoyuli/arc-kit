//! Interactive flows for `arc.toml` (create / edit). Skill installation is done by `arc project apply`.

use std::path::Path;

use arc_core::ArcPaths;
use arc_core::detect::DetectCache;
use arc_core::error::ArcError;
use arc_core::market::sources::MarketSourceRegistry;
use arc_core::models::SkillOrigin;
use arc_core::project::{
    MarketEntry, ProjectConfig, SkillsSection, find_project_config, load_project_config,
    write_project_config,
};
use arc_core::skill::SkillRegistry;
use console::style;

/// Create `arc.toml` in `cwd` via skill picker (no install; run `arc project apply` after).
pub fn create_arc_toml_interactive(
    paths: &ArcPaths,
    cache: &DetectCache,
    cwd: &Path,
) -> Result<(), ArcError> {
    let registry = SkillRegistry::new(paths.clone(), cache.clone());
    let _ = registry.bootstrap_catalog();
    let all_skills = registry.list_all();

    let selected_names = arc_tui::run_skill_require_pick_wizard(&all_skills)
        .map_err(|e| ArcError::new(format!("interactive selection failed: {e}")))?;

    // Collect markets for selected skills
    let markets = collect_markets_for_skills(paths, &all_skills, &selected_names);

    let config = ProjectConfig {
        version: 1,
        skills: SkillsSection {
            require: selected_names,
        },
        markets,
        ..Default::default()
    };

    let arc_toml_path = cwd.join("arc.toml");
    write_project_config(&arc_toml_path, &config)?;
    println!("  + arc.toml created");
    println!();
    Ok(())
}

/// Edit `[skills] require` for the nearest `arc.toml` (preserves `[provider]` and version).
pub fn edit_arc_toml_interactive(
    paths: &ArcPaths,
    cache: &DetectCache,
    cwd: &Path,
) -> Result<(), ArcError> {
    let config_path = find_project_config(cwd).ok_or_else(|| {
        ArcError::with_hint(
            "No arc.toml found.".to_string(),
            "Run `arc project apply` in this directory to create one.".to_string(),
        )
    })?;

    let mut cfg = load_project_config(&config_path)?;
    let preselected = cfg.skills.require.clone();

    let registry = SkillRegistry::new(paths.clone(), cache.clone());
    let _ = registry.bootstrap_catalog();
    let all_skills = registry.list_all();

    let selected_names =
        arc_tui::run_skill_require_pick_wizard_with_defaults(&all_skills, &preselected)
            .map_err(|e| ArcError::new(format!("interactive selection failed: {e}")))?;

    cfg.skills.require = selected_names.clone();

    // Collect markets for selected skills and merge with existing
    let new_markets = collect_markets_for_skills(paths, &all_skills, &selected_names);
    cfg.markets = merge_markets(cfg.markets, new_markets);

    write_project_config(&config_path, &cfg)?;
    println!("  + arc.toml updated");
    println!();
    println!(
        "  {} Run {} to sync skills if needed.",
        style("Tip:").dim(),
        style("arc project apply").bold().dim()
    );
    println!();
    Ok(())
}

/// Collect market sources for the given skill names.
/// Returns MarketEntry list for market-origin skills.
/// Warns user for local-origin skills.
fn collect_markets_for_skills(
    paths: &ArcPaths,
    all_skills: &[arc_core::models::SkillEntry],
    selected_names: &[String],
) -> Vec<MarketEntry> {
    let market_registry = MarketSourceRegistry::new(paths.clone());
    let all_markets = market_registry.load();
    let mut markets: Vec<MarketEntry> = Vec::new();
    let mut seen_urls: std::collections::HashSet<String> = std::collections::HashSet::new();

    for name in selected_names {
        let Some(skill) = all_skills.iter().find(|s| &s.name == name) else {
            continue;
        };

        match &skill.origin {
            SkillOrigin::Market { source_id } => {
                if let Some(source) = all_markets.get(source_id)
                    && seen_urls.insert(source.git_url.clone())
                {
                    markets.push(MarketEntry {
                        url: source.git_url.clone(),
                    });
                }
            }
            SkillOrigin::Local => {
                println!(
                    "  {} Skill '{}' is from local directory and cannot be added to arc.toml markets.",
                    style("!").yellow(),
                    style(name).bold()
                );
            }
            SkillOrigin::BuiltIn => {
                // Built-in skills don't need market entries
            }
        }
    }

    markets
}

/// Merge new markets with existing, avoiding duplicates by URL.
fn merge_markets(existing: Vec<MarketEntry>, new: Vec<MarketEntry>) -> Vec<MarketEntry> {
    let mut result = existing;
    let mut seen_urls: std::collections::HashSet<String> =
        result.iter().map(|m| m.url.clone()).collect();

    for entry in new {
        if seen_urls.insert(entry.url.clone()) {
            result.push(entry);
        }
    }

    result
}
