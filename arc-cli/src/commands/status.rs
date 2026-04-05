use std::collections::BTreeMap;
use std::env;
use std::io::{self, IsTerminal};

use arc_core::detect::{CODING_AGENTS, DetectCache, coding_agent_spec};
use arc_core::engine::InstalledResource;
use arc_core::error::ArcError;
use arc_core::models::ResourceKind;
use arc_core::project::{EffectiveConfig, resolve_effective_config};
use arc_core::provider::{
    load_providers_for_agent, read_active_provider, supported_provider_agents,
};
use arc_core::skill::SkillRegistry;
use arc_core::{ArcPaths, InstallEngine};
use console::{Alignment, measure_text_width, pad_str, style};

use crate::cli::OutputFormat;
use crate::display::agent_display_name;
use crate::format::{AgentStatus, MarketsSummary, SCHEMA_VERSION, StatusOutput, print_json};

pub fn run(paths: &ArcPaths, cache: &DetectCache, fmt: &OutputFormat) -> Result<(), ArcError> {
    let agents = cache.detected_agents();
    let engine = InstallEngine::new(cache.clone());
    let installed = engine.list_installed(None);
    let skill_counts = count_skills_by_agent(&installed);
    let providers_dir = paths.providers_dir();
    let provider_agents: Vec<&str> = supported_provider_agents();

    let mut active_providers: BTreeMap<String, String> = BTreeMap::new();
    let mut active_provider_names: BTreeMap<String, String> = BTreeMap::new();
    for agent in &provider_agents {
        if let Some(active_name) = read_active_provider(&providers_dir, agent) {
            let providers = load_providers_for_agent(&providers_dir, agent);
            if let Some(p) = providers.iter().find(|p| p.name == active_name) {
                active_providers.insert(agent.to_string(), p.display_name.clone());
                active_provider_names.insert(agent.to_string(), p.name.clone());
            }
        }
    }

    // ── JSON output ───────────────────────────────────────
    if *fmt == OutputFormat::Json {
        let cwd = env::current_dir().unwrap_or_default();
        let registry = SkillRegistry::new(paths.clone(), cache.clone());
        let effective = resolve_effective_config(paths, &cwd, cache, &registry).ok();

        let registry2 = arc_core::market::sources::MarketSourceRegistry::new(paths.clone());
        let sources = registry2.list_all();
        let total_resources: usize = sources.iter().map(|s| s.resource_count).sum();

        let agent_list: Vec<AgentStatus> = agents
            .iter()
            .map(|(agent_id, info)| {
                let name = coding_agent_spec(agent_id)
                    .map(|s| s.display_name.to_string())
                    .unwrap_or_else(|| agent_id.clone());
                let count = skill_counts.get(agent_id.as_str()).copied().unwrap_or(0);
                AgentStatus {
                    id: agent_id.clone(),
                    name,
                    version: info.version.clone(),
                    provider: active_provider_names.get(agent_id).cloned(),
                    skill_count: count,
                }
            })
            .collect();

        let project = effective.and_then(|eff| {
            eff.config_path.as_ref()?;
            Some(crate::format::ProjectStatus {
                name: eff.project_name.clone(),
                config_path: eff.config_path.as_ref().map(|p| p.display().to_string()),
                required_skills: eff.required_skills.clone(),
                installed_skills: eff.installed_skills.clone(),
                missing_skills: eff.missing_installable.clone(),
                unavailable_skills: eff.missing_unavailable.clone(),
            })
        });

        print_json(&StatusOutput {
            schema_version: SCHEMA_VERSION,
            agents: agent_list,
            markets: MarketsSummary {
                count: sources.len(),
                resource_count: total_resources,
            },
            installed_skills: installed.len(),
            project,
        })?;
        return Ok(());
    }

    if agents.is_empty() {
        println!(
            "  {} {}",
            style("!").yellow(),
            style("No coding agents detected.").yellow()
        );
        println!();
        let names: Vec<&str> = CODING_AGENTS.iter().map(|s| s.display_name).collect();
        println!(
            "  {}  {}",
            style("Hint:").dim(),
            style(format!(
                "Install a supported agent to get started: {}",
                names.join(", ")
            ))
            .dim()
        );
        println!();
        return Ok(());
    }

    // ── Project context ───────────────────────────────────────

    let is_tty = io::stdin().is_terminal() && io::stdout().is_terminal();
    let cwd = env::current_dir().unwrap_or_default();
    let registry = SkillRegistry::new(paths.clone(), cache.clone());
    let effective = resolve_effective_config(paths, &cwd, cache, &registry).ok();

    let mut missing_skill_action_taken = false;
    let mut exit_code_1 = false;

    if let Some(ref eff) = effective
        && eff.config_path.is_some()
    {
        render_project_box(eff);

        if !is_tty {
            // Non-interactive: set exit code 1 if missing installable skills.
            if !eff.missing_installable.is_empty() {
                exit_code_1 = true;
            }
        } else {
            // Interactive: offer to install missing skills.
            if !eff.missing_installable.is_empty() {
                println!();
                let label = if eff.missing_installable.len() == 1 {
                    format!("Required skill missing: {}", eff.missing_installable[0])
                } else {
                    format!(
                        "Required skills missing: {}",
                        eff.missing_installable.join(", ")
                    )
                };
                println!("  {}", style(&label).yellow());
                let ok = arc_tui::confirm("Install now?", true).unwrap_or(false);
                if ok {
                    install_missing(paths, cache, &registry, eff)?;
                    missing_skill_action_taken = true;
                }
            }
        }
    }

    // ── Agent table ───────────────────────────────────────────

    let name_width = agents
        .keys()
        .filter_map(|id| coding_agent_spec(id))
        .map(|s| measure_text_width(s.display_name))
        .max()
        .unwrap_or(10);
    let ver_width = agents
        .values()
        .filter_map(|info| info.version.as_ref())
        .map(|v| measure_text_width(v))
        .max()
        .unwrap_or(0);
    let provider_width = active_providers
        .values()
        .map(|v| measure_text_width(v))
        .max()
        .unwrap_or(0);

    println!();
    for (agent_id, info) in agents {
        let display = coding_agent_spec(agent_id)
            .map(|s| s.display_name)
            .unwrap_or(agent_id);
        let count = skill_counts.get(agent_id.as_str()).copied().unwrap_or(0);

        let ver_str = info
            .version
            .as_deref()
            .map(|v| pad_str(v, ver_width, Alignment::Left, None).into_owned())
            .unwrap_or_else(|| " ".repeat(ver_width));

        let provider_str = active_providers
            .get(agent_id)
            .map(|name| pad_str(name, provider_width, Alignment::Left, None).into_owned())
            .unwrap_or_else(|| " ".repeat(provider_width));

        let skill_label = if count == 1 { "skill" } else { "skills" };

        println!(
            "  {}  {}  {}  {} {}",
            pad_str(display, name_width, Alignment::Left, None),
            style(&ver_str).dim(),
            style(&provider_str).cyan(),
            count,
            style(skill_label).dim(),
        );
    }
    println!();

    let registry2 = arc_core::market::sources::MarketSourceRegistry::new(paths.clone());
    let sources = registry2.list_all();
    let total_resources: usize = sources.iter().map(|s| s.resource_count).sum();
    let total_installed = installed.len();

    let skill_label = if total_installed == 1 {
        "skill"
    } else {
        "skills"
    };
    println!(
        "  {} · {} · {} {} installed",
        style(format!("{} markets", sources.len())).dim(),
        style(format!("{} resources", total_resources)).dim(),
        total_installed,
        style(skill_label).dim(),
    );
    println!();

    // If non-interactive and missing skills, exit with code 1.
    if exit_code_1 {
        return Err(ArcError::with_hint(
            "Missing required skills.".to_string(),
            "Run `arc project apply` to install missing skills.".to_string(),
        ));
    }

    // If user declined interactive install, show hint.
    if is_tty
        && let Some(ref eff) = effective
        && eff.config_path.is_some()
        && !missing_skill_action_taken
        && !eff.missing_installable.is_empty()
    {
        println!(
            "  {}",
            style("Run `arc project apply` to install missing skills.").dim()
        );
        println!();
    }

    Ok(())
}

// ── Project context box ───────────────────────────────────

fn render_project_box(eff: &EffectiveConfig) {
    let total = eff.required_skills.len();
    let installed = eff.installed_skills.len();
    let missing = eff.missing_installable.len();
    let unavailable = eff.missing_unavailable.len();

    println!();
    println!("  ┌ {}", style(&eff.project_name).bold());

    if total > 0 {
        if missing == 0 && unavailable == 0 {
            println!(
                "  │ skills   {} required, {} installed {}",
                total,
                installed,
                style("✓").green()
            );
        } else if missing == 0 && unavailable > 0 {
            println!(
                "  │ skills   {} required, {} installed, {} unavailable",
                total,
                installed,
                style(unavailable.to_string()).dim()
            );
        } else {
            println!(
                "  │ skills   {} required, {} installed, {} missing",
                total,
                installed,
                style(missing.to_string()).yellow()
            );
        }
    }

    if total > 0 {
        println!(
            "  │ {}",
            style("Required skills are materialized in this repo for supported agents (e.g. .claude/skills; OpenClaw has no project-level path — use `arc skill install` globally).")
                .dim()
        );
    }

    println!("  └");
}

// ── Helpers ───────────────────────────────────────────────

fn install_missing(
    _paths: &ArcPaths,
    cache: &DetectCache,
    registry: &SkillRegistry,
    eff: &EffectiveConfig,
) -> Result<(), ArcError> {
    let Some(project_root) = eff.project_root.as_ref() else {
        return Ok(());
    };
    let engine = InstallEngine::new(cache.clone());
    let targets = cache.agents_for_project_skill_install(&ResourceKind::Skill);
    for name in &eff.missing_installable {
        let Some(skill) = registry.find(name) else {
            continue;
        };
        let source_path = match registry.resolve_source_path(&skill) {
            Ok(p) => p,
            Err(e) => {
                println!(
                    "  {} {} — {}",
                    style("✗").red(),
                    name,
                    style(&e.message).dim()
                );
                continue;
            }
        };
        match engine.install_named_project(
            name,
            &ResourceKind::Skill,
            &source_path,
            project_root,
            &targets,
        ) {
            Ok(installed) => {
                for agent in &installed {
                    println!(
                        "  {} {} → {} (project)",
                        style("✓").green(),
                        style(name).bold(),
                        agent_display_name(agent)
                    );
                }
            }
            Err(e) => {
                println!(
                    "  {} {} — {}",
                    style("✗").red(),
                    name,
                    style(&e.message).dim()
                );
            }
        }
    }
    Ok(())
}

fn count_skills_by_agent(items: &[InstalledResource]) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for item in items {
        if item.kind.as_str() != "skill" {
            continue;
        }
        for target in &item.targets {
            *counts.entry(target.clone()).or_insert(0) += 1;
        }
    }
    counts
}
