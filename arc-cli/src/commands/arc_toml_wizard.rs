//! Interactive flows for `arc.toml` (create / edit). Project skill installation is done by
//! `arc project apply`.

use std::path::Path;

use arc_core::detect::DetectCache;
use arc_core::error::ArcError;
use arc_core::market::sources::MarketSourceRegistry;
use arc_core::models::SkillOrigin;
use arc_core::paths::ArcPaths;
use arc_core::project::{
    MarketEntry, ProjectConfig, SkillsSection, find_project_config, load_project_config,
    write_project_config,
};
use arc_core::skill::SkillRegistry;
use console::style;

/// Create `arc.toml` in `cwd` via project requirement pickers (no install; run `arc project apply`
/// after).
pub fn create_arc_toml_interactive(
    paths: &ArcPaths,
    cache: &DetectCache,
    cwd: &Path,
) -> Result<bool, ArcError> {
    let registry = SkillRegistry::new(paths.clone(), cache.clone());
    let _ = registry.bootstrap_catalog();
    let all_skills = registry.list_all();

    let Some(selection) = arc_tui::run_project_requirements_editor(&all_skills)
        .map_err(|e| ArcError::new(format!("interactive selection failed: {e}")))?
    else {
        println!("  {}", style("Canceled.").dim());
        println!();
        return Ok(false);
    };

    // Collect markets for selected skills
    let markets = collect_markets_for_skills(paths, &all_skills, &selection.skills);

    let config = ProjectConfig {
        version: 1,
        skills: SkillsSection {
            require: selection.skills,
        },
        markets,
        ..Default::default()
    };

    let arc_toml_path = cwd.join("arc.toml");
    write_project_config(&arc_toml_path, &config)?;
    println!("  + arc.toml created");
    println!();
    Ok(true)
}

/// Edit project requirement lists for the nearest `arc.toml` (preserves `[provider]` and
/// `version`).
pub fn edit_arc_toml_interactive(
    paths: &ArcPaths,
    cache: &DetectCache,
    cwd: &Path,
) -> Result<bool, ArcError> {
    let config_path = find_project_config(cwd).ok_or_else(|| {
        ArcError::with_hint(
            "No arc.toml found.".to_string(),
            "Run `arc project apply` in this directory to create one.".to_string(),
        )
    })?;

    let mut cfg = load_project_config(&config_path)?;
    let preselected = arc_tui::ProjectRequirementsSelection {
        skills: cfg.skills.require.clone(),
    };

    let registry = SkillRegistry::new(paths.clone(), cache.clone());
    let _ = registry.bootstrap_catalog();
    let all_skills = registry.list_all();

    let Some(selection) =
        arc_tui::run_project_requirements_editor_with_defaults(&all_skills, &preselected)
            .map_err(|e| ArcError::new(format!("interactive selection failed: {e}")))?
    else {
        println!("  {}", style("Canceled.").dim());
        println!();
        return Ok(false);
    };

    cfg.skills.require = selection.skills.clone();

    // Keep unrelated existing markets, but resync the markets implied by skill selection.
    let previous_markets = collect_markets_for_skills(paths, &all_skills, &preselected.skills);
    let new_markets = collect_markets_for_skills(paths, &all_skills, &cfg.skills.require);
    cfg.markets = reconcile_markets(cfg.markets, previous_markets, new_markets);

    write_project_config(&config_path, &cfg)?;
    println!("  + arc.toml updated");
    println!();
    println!(
        "  {} Run {} to sync skills if needed.",
        style("Tip:").dim(),
        style("arc project apply").bold().dim()
    );
    println!();
    Ok(true)
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

/// Keep existing markets that were not implied by the previous skill selection,
/// then append the markets implied by the current selection.
fn reconcile_markets(
    existing: Vec<MarketEntry>,
    previous_managed: Vec<MarketEntry>,
    new_managed: Vec<MarketEntry>,
) -> Vec<MarketEntry> {
    let previous_urls: std::collections::HashSet<String> =
        previous_managed.into_iter().map(|m| m.url).collect();
    let mut result: Vec<MarketEntry> = existing
        .into_iter()
        .filter(|entry| !previous_urls.contains(&entry.url))
        .collect();
    let mut seen_urls: std::collections::HashSet<String> =
        result.iter().map(|m| m.url.clone()).collect();

    for entry in new_managed {
        if seen_urls.insert(entry.url.clone()) {
            result.push(entry);
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::reconcile_markets;
    use arc_core::project::MarketEntry;

    #[test]
    fn reconcile_markets_removes_stale_skill_market() {
        let existing = vec![
            MarketEntry {
                url: "https://example.com/market1.git".to_string(),
            },
            MarketEntry {
                url: "https://example.com/market2.git".to_string(),
            },
        ];
        let previous_managed = existing.clone();
        let new_managed = vec![MarketEntry {
            url: "https://example.com/market2.git".to_string(),
        }];

        let markets = reconcile_markets(existing, previous_managed, new_managed);

        assert_eq!(
            markets,
            vec![MarketEntry {
                url: "https://example.com/market2.git".to_string(),
            }]
        );
    }

    #[test]
    fn reconcile_markets_preserves_unrelated_existing_market() {
        let existing = vec![
            MarketEntry {
                url: "https://example.com/manual.git".to_string(),
            },
            MarketEntry {
                url: "https://example.com/market1.git".to_string(),
            },
        ];
        let previous_managed = vec![MarketEntry {
            url: "https://example.com/market1.git".to_string(),
        }];
        let new_managed = vec![MarketEntry {
            url: "https://example.com/market2.git".to_string(),
        }];

        let markets = reconcile_markets(existing, previous_managed, new_managed);

        assert_eq!(
            markets,
            vec![
                MarketEntry {
                    url: "https://example.com/manual.git".to_string(),
                },
                MarketEntry {
                    url: "https://example.com/market2.git".to_string(),
                },
            ]
        );
    }
}
