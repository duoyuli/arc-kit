use std::env;
use std::io::{self, IsTerminal};

use arc_core::detect::DetectCache;
use arc_core::error::ArcError;
use arc_core::market::bootstrap::sync_market_source_resources;
use arc_core::market::sources::MarketSourceRegistry;
use arc_core::models::ResourceKind;
use arc_core::project::{
    EffectiveConfig, find_project_config, load_project_config, resolve_effective_config,
};
use arc_core::provider::{apply_provider, load_providers_for_agent, supported_provider_agents};
use arc_core::skill::SkillRegistry;
use arc_core::{ArcPaths, InstallEngine};
use arc_tui::select_agents;
use console::style;

use crate::cli::{OutputFormat, ProjectApplyArgs};
use crate::commands::arc_toml_wizard;
use crate::display::agent_display_name;
use crate::format::{SCHEMA_VERSION, WriteResult, WriteResultItem, print_json};

pub fn run(
    paths: &ArcPaths,
    cache: &DetectCache,
    fmt: &OutputFormat,
    args: &ProjectApplyArgs,
) -> Result<(), ArcError> {
    let cwd = env::current_dir()
        .map_err(|e| ArcError::new(format!("failed to get working directory: {e}")))?;

    if find_project_config(&cwd).is_none() {
        if *fmt == OutputFormat::Json {
            print_json(&WriteResult {
                schema_version: SCHEMA_VERSION,
                ok: false,
                message: "No arc.toml found. Run `arc project apply` from a terminal to create one interactively, or add arc.toml manually.".to_string(),
                items: Vec::new(),
            })?;
            return Ok(());
        }

        let is_tty = io::stdin().is_terminal() && io::stdout().is_terminal();
        if !is_tty {
            return Err(ArcError::with_hint(
                "No arc.toml found in current directory.".to_string(),
                "Run `arc project apply` from a terminal to create arc.toml, or add the file manually.".to_string(),
            ));
        }

        println!();
        println!(
            "  {}",
            style("No arc.toml found in current directory.").yellow()
        );
        println!();
        println!("  {}", style("Creating arc.toml…").dim());
        println!();
        arc_toml_wizard::create_arc_toml_interactive(paths, cache, &cwd)?;
    }

    // Load project config to process markets before building registry
    let project_cfg = if let Some(config_path) = find_project_config(&cwd) {
        load_project_config(&config_path).ok()
    } else {
        None
    };

    // Process markets from arc.toml: add any missing ones
    if let Some(ref cfg) = project_cfg {
        let market_registry = MarketSourceRegistry::new(paths.clone());
        let existing = market_registry.load();
        for entry in &cfg.markets {
            let url = &entry.url;
            let source_id = market_registry.generate_slug(url);
            if !existing.contains_key(&source_id) {
                match market_registry.add(url, "auto") {
                    Ok(source) => {
                        sync_market_source_resources(paths, &source)
                            .map_err(|e| e.with_exit_code(1))?;
                        if *fmt != OutputFormat::Json {
                            println!(
                                "  {} market {} -> {}",
                                style("+").green(),
                                style(&source_id).bold(),
                                style(url).dim()
                            );
                        }
                    }
                    Err(e) => {
                        if *fmt != OutputFormat::Json {
                            println!(
                                "  {} market {} - {}",
                                style("!").yellow(),
                                style(&source_id).bold(),
                                style(&e).dim()
                            );
                        }
                    }
                }
            }
        }
    }

    let registry = SkillRegistry::new(paths.clone(), cache.clone());
    registry
        .bootstrap_catalog()
        .map_err(|e| e.with_exit_code(1))?;

    let effective =
        resolve_effective_config(paths, &cwd, cache, &registry).map_err(|e| e.with_exit_code(1))?;

    let provider_switch = effective
        .provider_to_switch(paths)
        .map_err(|e| e.with_exit_code(1))?;

    if *fmt == OutputFormat::Json {
        return apply_json(paths, cache, &registry, &effective, provider_switch, args);
    }

    println!();
    println!("  {}", style(&effective.project_name).bold());

    print_required_skills_status(&effective);

    if effective.is_up_to_date()
        && effective.missing_unavailable.is_empty()
        && provider_switch.is_none()
    {
        println!();
        println!(
            "  {} already up to date.",
            style(&effective.project_name).bold()
        );
        println!();
        return Ok(());
    }

    println!();

    if let Some(provider_name) = provider_switch {
        println!("  provider  -> {}", style(provider_name).cyan());
        apply_provider_switch(paths, provider_name, true)?;
    }

    let skill_install_count = effective.missing_installable.len();
    if skill_install_count > 0 {
        println!(
            "  skills    -> installing {}",
            style(format!(
                "{} skill{}",
                skill_install_count,
                if skill_install_count == 1 { "" } else { "s" }
            ))
            .cyan()
        );
    }

    println!();

    let engine = InstallEngine::new(cache.clone());
    let targets = if effective.missing_installable.is_empty() {
        Vec::new()
    } else {
        resolve_project_install_targets(cache, args, fmt)?
    };
    let project_root = effective.project_root.as_ref().ok_or_else(|| {
        ArcError::new("internal error: arc.toml present but project root missing")
    })?;
    for name in &effective.missing_installable {
        let Some(skill) = registry.find(name) else {
            continue;
        };
        let source_path = match registry.resolve_source_path(&skill) {
            Ok(p) => p,
            Err(e) => {
                println!(
                    "  {} {} - {}",
                    style("x").red(),
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
                        "  {} {} -> {} (project)",
                        style("+").green(),
                        style(name).bold(),
                        agent_display_name(agent)
                    );
                }
            }
            Err(e) => {
                println!(
                    "  {} {} - {}",
                    style("x").red(),
                    name,
                    style(&e.message).dim()
                );
            }
        }
    }

    for name in &effective.missing_unavailable {
        println!(
            "  {} {} not found in any source, skipped.",
            style("!").yellow(),
            style(name).bold()
        );
        println!(
            "    {}",
            style("Run `arc market add <url>` to add a market source containing this skill.").dim()
        );
    }

    println!();

    if effective.missing_unavailable.is_empty() {
        println!("  {}", style("Ready.").green());
    } else {
        println!(
            "  {} ({} skill{} skipped)",
            style("Partially ready").yellow(),
            effective.missing_unavailable.len(),
            if effective.missing_unavailable.len() == 1 {
                ""
            } else {
                "s"
            }
        );
    }
    println!();

    Ok(())
}

/// Lists each `[skills] require` entry and whether it is present, installable, or unknown.
fn print_required_skills_status(effective: &EffectiveConfig) {
    if effective.required_skills.is_empty() {
        return;
    }

    println!(
        "  {} {}",
        style("skills").bold(),
        style("(required in arc.toml)").dim()
    );
    for name in &effective.required_skills {
        let status_line = if effective.installed_skills.contains(name) {
            format!("{}", style("present (project)").green())
        } else if effective.missing_installable.contains(name) {
            format!("{}", style("will install").cyan())
        } else {
            format!("{}", style("not in catalog").yellow())
        };
        println!("    {}  {}", style(name).bold(), status_line);
    }
    println!();
}

fn resolve_project_install_targets(
    cache: &DetectCache,
    args: &ProjectApplyArgs,
    fmt: &OutputFormat,
) -> Result<Vec<String>, ArcError> {
    let candidates = cache.agents_for_project_skill_install(&ResourceKind::Skill);
    if candidates.is_empty() {
        return Err(ArcError::with_hint(
            "No coding agents with project-local skill support are detected.".to_string(),
            "Install a supported agent (e.g. Claude Code, Codex) or check PATH.".to_string(),
        ));
    }

    if args.all_agents && !args.agent.is_empty() {
        return Err(ArcError::with_hint(
            "Use either --all-agents or --agent <name>, not both.".to_string(),
            "Example: arc project apply --all-agents".to_string(),
        ));
    }

    if args.all_agents {
        return Ok(candidates);
    }

    if !args.agent.is_empty() {
        let mut out = Vec::new();
        for id in &args.agent {
            if !candidates.iter().any(|c| c == id) {
                return Err(ArcError::with_hint(
                    format!(
                        "Agent '{id}' is not available for project skill install (not detected or no project-local support)."
                    ),
                    format!("Available: {}", candidates.join(", ")),
                ));
            }
            if !out.contains(id) {
                out.push(id.clone());
            }
        }
        return Ok(out);
    }

    if *fmt == OutputFormat::Json {
        return Err(ArcError::with_hint(
            "Specify --agent <name> (repeatable) or --all-agents for project skill install."
                .to_string(),
            "Example: arc project apply --format json --agent claude".to_string(),
        ));
    }

    let is_tty = io::stdin().is_terminal() && io::stdout().is_terminal();
    if !is_tty {
        return Err(ArcError::with_hint(
            "Choose target agent(s): pass --agent <name> (repeatable) or --all-agents, or run from a TTY for interactive selection.".to_string(),
            "Example: arc project apply --agent claude".to_string(),
        ));
    }

    let installed: Vec<&String> = Vec::new();
    let selected = select_agents(&candidates, &installed)
        .map_err(|e| ArcError::new(format!("agent selection failed: {e}")))?;
    if selected.is_empty() {
        return Err(ArcError::new("No agents selected; canceled."));
    }
    Ok(selected)
}

fn apply_json(
    paths: &ArcPaths,
    cache: &DetectCache,
    registry: &SkillRegistry,
    effective: &EffectiveConfig,
    provider_switch: Option<&str>,
    args: &ProjectApplyArgs,
) -> Result<(), ArcError> {
    let mut items = Vec::new();

    if let Some(provider_name) = provider_switch {
        apply_provider_switch(paths, provider_name, false)?;
        items.push(WriteResultItem {
            name: provider_name.to_string(),
            agent: "all".to_string(),
            status: "provider_switched".to_string(),
        });
    }

    let engine = InstallEngine::new(cache.clone());
    let targets = if effective.missing_installable.is_empty() {
        Vec::new()
    } else {
        resolve_project_install_targets(cache, args, &OutputFormat::Json)?
    };
    let project_root = effective.project_root.as_ref().ok_or_else(|| {
        ArcError::new("internal error: arc.toml present but project root missing")
    })?;
    for name in &effective.missing_installable {
        let Some(skill) = registry.find(name) else {
            items.push(WriteResultItem {
                name: name.clone(),
                agent: "".to_string(),
                status: "not_found".to_string(),
            });
            continue;
        };
        let source_path = match registry.resolve_source_path(&skill) {
            Ok(p) => p,
            Err(e) => {
                items.push(WriteResultItem {
                    name: name.clone(),
                    agent: "".to_string(),
                    status: format!("error: {}", e.message),
                });
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
                    items.push(WriteResultItem {
                        name: name.clone(),
                        agent: agent.clone(),
                        status: "installed".to_string(),
                    });
                }
            }
            Err(e) => {
                items.push(WriteResultItem {
                    name: name.clone(),
                    agent: "".to_string(),
                    status: format!("error: {}", e.message),
                });
            }
        }
    }

    let ok = effective.missing_unavailable.is_empty()
        && !items.iter().any(|i| i.status.starts_with("error"));

    print_json(&WriteResult {
        schema_version: SCHEMA_VERSION,
        ok,
        message: if ok {
            "Done.".to_string()
        } else {
            "Completed with issues.".to_string()
        },
        items,
    })?;

    Ok(())
}

fn apply_provider_switch(
    paths: &ArcPaths,
    provider_name: &str,
    print: bool,
) -> Result<(), ArcError> {
    let providers_dir = paths.providers_dir();
    for agent in supported_provider_agents() {
        let providers = load_providers_for_agent(&providers_dir, agent);
        if let Some(provider) = providers.into_iter().find(|p| p.name == provider_name) {
            apply_provider(paths, &provider)?;
            if print {
                println!(
                    "  {} provider {} -> {}",
                    style("+").green(),
                    style(provider_name).bold(),
                    agent_display_name(agent)
                );
            }
        }
    }
    Ok(())
}
