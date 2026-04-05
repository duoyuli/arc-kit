use std::fs;
use std::io::{self, IsTerminal};

use arc_core::detect::DetectCache;
use arc_core::error::ArcError;
use arc_core::market::bootstrap::MarketSyncReport;
use arc_core::models::{ResourceKind, SkillEntry};
use arc_core::skill::SkillRegistry;
use arc_core::skill::tracking::{track_global_skill_install, untrack_global_skill_install};
use arc_core::{ArcPaths, InstallEngine};
use arc_tui::{run_skill_browser, run_skill_install_wizard, run_skill_uninstall_wizard};
use console::style;

use crate::cli::{
    OutputFormat, SkillCommand, SkillInfoArgs, SkillInstallArgs, SkillListArgs, SkillUninstallArgs,
};
use crate::display::{agent_display_name, agent_display_names};
use crate::format::{
    SCHEMA_VERSION, SkillInfoOutput, SkillItem, SkillListOutput, WriteResult, WriteResultItem,
    print_json,
};

pub fn run(
    paths: &ArcPaths,
    cache: &DetectCache,
    command: SkillCommand,
    fmt: &OutputFormat,
) -> Result<(), ArcError> {
    match command {
        SkillCommand::List(args) => list(paths, cache, args, fmt),
        SkillCommand::Install(args) => install(paths, cache, args, fmt),
        SkillCommand::Uninstall(args) => uninstall(paths, cache, args, fmt),
        SkillCommand::Info(args) => info(paths, cache, args, fmt),
    }
}

// ── list ─────────────────────────────────────────────────

fn list(
    paths: &ArcPaths,
    cache: &DetectCache,
    args: SkillListArgs,
    fmt: &OutputFormat,
) -> Result<(), ArcError> {
    let registry = SkillRegistry::new(paths.clone(), cache.clone());
    if *fmt != OutputFormat::Json {
        print_bootstrap_report(&registry.bootstrap_catalog()?);
    } else {
        let _ = registry.bootstrap_catalog();
    }
    let mut skills = registry.list_all();

    if args.installed {
        skills.retain(|s| !s.installed_targets.is_empty());
    }
    // Sort: installed first, uninstalled last
    skills.sort_by_key(|s| s.installed_targets.is_empty());

    if *fmt == OutputFormat::Json {
        let items: Vec<SkillItem> = skills
            .iter()
            .map(|s| SkillItem {
                name: s.name.clone(),
                origin: s.origin_json(),
                summary: s.summary.clone(),
                installed_targets: s.installed_targets.clone(),
            })
            .collect();
        print_json(&SkillListOutput {
            schema_version: SCHEMA_VERSION,
            skills: items,
        })?;
        return Ok(());
    }

    if skills.is_empty() {
        if args.installed {
            println!("  {}", style("No skills installed.").yellow());
        } else {
            println!("  {}", style("No skills found.").yellow());
        }
        return Ok(());
    }

    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        render_skill_list(&skills);
        return Ok(());
    }

    run_skill_browser(&skills, |skill| {
        render_skill_detail(&registry, skill);
    })
    .map_err(|e| ArcError::new(format!("interactive browse failed: {e}")))
}

// ── install ──────────────────────────────────────────────

fn install(
    paths: &ArcPaths,
    cache: &DetectCache,
    args: SkillInstallArgs,
    fmt: &OutputFormat,
) -> Result<(), ArcError> {
    paths
        .ensure_arc_home()
        .map_err(|err| ArcError::new(err.to_string()))?;
    let registry = SkillRegistry::new(paths.clone(), cache.clone());
    if *fmt != OutputFormat::Json {
        print_bootstrap_report(&registry.bootstrap_catalog()?);
    } else {
        let _ = registry.bootstrap_catalog();
    }
    let engine = InstallEngine::new(cache.clone());
    let mut skills = registry.list_all();
    if skills.is_empty() {
        if *fmt == OutputFormat::Json {
            print_json(&WriteResult {
                schema_version: SCHEMA_VERSION,
                ok: false,
                message: "No skills available.".to_string(),
                items: Vec::new(),
            })?;
            return Ok(());
        }
        println!("  {}", style("No skills available.").yellow());
        return Ok(());
    }
    // Sort: installed first, uninstalled last
    skills.sort_by_key(|s| s.installed_targets.is_empty());

    if args.name.is_none() {
        let is_tty = io::stdin().is_terminal() && io::stdout().is_terminal();
        if !is_tty {
            return Err(ArcError::with_hint(
                "Skill name required in non-interactive mode.".to_string(),
                "Usage: arc skill install <name> [--agent <agent>]".to_string(),
            ));
        }
        let agents = cache.agents_for_install(&ResourceKind::Skill);
        let (selected_names, selected_agents) = run_skill_install_wizard(&skills, &agents)
            .map_err(|err| ArcError::new(format!("interactive install failed: {err}")))?;
        if selected_names.is_empty() || selected_agents.is_empty() {
            return Ok(());
        }
        for name in &selected_names {
            let Some(skill) = skills.iter().find(|s| &s.name == name) else {
                continue;
            };
            install_one(paths, cache, &registry, &engine, skill, &selected_agents)?;
        }
        return Ok(());
    }

    let name = args.name.expect("checked optional name");
    let Some(skill) = skills.into_iter().find(|s| s.name == name) else {
        return Err(ArcError::new(format!("skill '{name}' not found.")));
    };
    let targets = if args.agent.is_empty() {
        cache.agents_for_install(&ResourceKind::Skill)
    } else {
        args.agent
    };

    if *fmt == OutputFormat::Json {
        return install_one_json(paths, cache, &registry, &engine, &skill, &targets);
    }
    install_one(paths, cache, &registry, &engine, &skill, &targets)
}

fn install_one(
    paths: &ArcPaths,
    cache: &DetectCache,
    registry: &SkillRegistry,
    engine: &InstallEngine,
    skill: &SkillEntry,
    targets: &[String],
) -> Result<(), ArcError> {
    let mut new_targets = Vec::new();
    let mut existing_targets = Vec::new();
    for t in targets {
        if engine.is_installed_for(&skill.name, &ResourceKind::Skill, t) {
            existing_targets.push(t.clone());
        } else {
            new_targets.push(t.clone());
        }
    }

    for t in &existing_targets {
        let agent_name = agent_display_name(t);
        println!(
            "  {} {} → {}",
            style("·").dim(),
            style(&skill.name).bold().dim(),
            style(format!("{agent_name} already installed")).dim()
        );
    }

    if new_targets.is_empty() {
        return Ok(());
    }

    let source_path = registry.resolve_source_path(skill)?;
    paths
        .ensure_arc_home()
        .map_err(|err| ArcError::new(err.to_string()))?;
    let installed = engine.install_named(
        &skill.name,
        &ResourceKind::Skill,
        &source_path,
        &new_targets,
    )?;
    for agent in &installed {
        record_global_skill_install(cache, agent, &skill.name, &source_path)?;
        let agent_name = agent_display_name(agent);
        println!(
            "  {} {} → {}",
            style("✓").green(),
            style(&skill.name).bold(),
            agent_name
        );
    }
    Ok(())
}

fn install_one_json(
    paths: &ArcPaths,
    cache: &DetectCache,
    registry: &SkillRegistry,
    engine: &InstallEngine,
    skill: &SkillEntry,
    targets: &[String],
) -> Result<(), ArcError> {
    let mut items = Vec::new();

    for t in targets {
        if engine.is_installed_for(&skill.name, &ResourceKind::Skill, t) {
            items.push(WriteResultItem {
                name: skill.name.clone(),
                agent: t.clone(),
                status: "already_installed".to_string(),
            });
        }
    }

    let new_targets: Vec<String> = targets
        .iter()
        .filter(|t| !engine.is_installed_for(&skill.name, &ResourceKind::Skill, t))
        .cloned()
        .collect();

    if !new_targets.is_empty() {
        let source_path = registry.resolve_source_path(skill)?;
        paths
            .ensure_arc_home()
            .map_err(|err| ArcError::new(err.to_string()))?;
        match engine.install_named(
            &skill.name,
            &ResourceKind::Skill,
            &source_path,
            &new_targets,
        ) {
            Ok(installed) => {
                for agent in &installed {
                    record_global_skill_install(cache, agent, &skill.name, &source_path)?;
                    items.push(WriteResultItem {
                        name: skill.name.clone(),
                        agent: agent.clone(),
                        status: "installed".to_string(),
                    });
                }
            }
            Err(e) => {
                items.push(WriteResultItem {
                    name: skill.name.clone(),
                    agent: "".to_string(),
                    status: format!("error: {}", e.message),
                });
            }
        }
    }

    let ok = !items.iter().any(|i| i.status.starts_with("error"));
    print_json(&WriteResult {
        schema_version: SCHEMA_VERSION,
        ok,
        message: if ok {
            format!("Skill '{}' installed.", skill.name)
        } else {
            format!("Failed to install skill '{}'.", skill.name)
        },
        items,
    })?;
    Ok(())
}

// ── uninstall ────────────────────────────────────────────

fn uninstall(
    paths: &ArcPaths,
    cache: &DetectCache,
    args: SkillUninstallArgs,
    fmt: &OutputFormat,
) -> Result<(), ArcError> {
    let engine = InstallEngine::new(cache.clone());

    let Some(name) = args.name else {
        let is_tty = io::stdin().is_terminal() && io::stdout().is_terminal();
        if !is_tty {
            return Err(ArcError::with_hint(
                "Skill name required in non-interactive mode.".to_string(),
                "Usage: arc skill uninstall <name> [--agent <agent>] [--all]".to_string(),
            ));
        }
        let registry = SkillRegistry::new(paths.clone(), cache.clone());
        print_bootstrap_report(&registry.bootstrap_catalog()?);
        let installed: Vec<SkillEntry> = registry
            .list_all()
            .into_iter()
            .filter(|s| !s.installed_targets.is_empty())
            .collect();
        if installed.is_empty() {
            println!("  {}", style("No skills installed.").yellow());
            return Ok(());
        }
        let Some((name, targets)) = run_skill_uninstall_wizard(&installed)
            .map_err(|err| ArcError::new(format!("interactive uninstall failed: {err}")))?
        else {
            return Ok(());
        };
        let removed = engine.uninstall(&name, &ResourceKind::Skill, Some(&targets))?;
        clear_global_skill_tracking(cache, &name, Some(&targets))?;
        if removed {
            println!("  {} {} removed.", style("✓").green(), name);
        } else {
            println!("  {} {} not installed.", style("─").dim(), name);
        }
        return Ok(());
    };

    let targets = if args.all {
        None
    } else if args.agent.is_empty() {
        Some(engine.get_installed_targets(&name, &ResourceKind::Skill))
    } else {
        Some(args.agent)
    };
    let removed = engine.uninstall(&name, &ResourceKind::Skill, targets.as_deref())?;
    clear_global_skill_tracking(cache, &name, targets.as_deref())?;

    if *fmt == OutputFormat::Json {
        print_json(&WriteResult {
            schema_version: SCHEMA_VERSION,
            ok: true,
            message: if removed {
                format!("Skill '{name}' removed.")
            } else {
                format!("Skill '{name}' not installed.")
            },
            items: Vec::new(),
        })?;
        return Ok(());
    }

    if removed {
        println!("  {} {} removed.", style("✓").green(), name);
    } else {
        println!("  {} {} not installed.", style("─").dim(), name);
    }
    Ok(())
}

fn record_global_skill_install(
    cache: &DetectCache,
    agent: &str,
    skill: &str,
    source_path: &std::path::Path,
) -> Result<(), ArcError> {
    let Some(agent_info) = cache.get_agent(agent) else {
        return Ok(());
    };
    let Some(root) = &agent_info.root else {
        return Ok(());
    };
    let Some(spec) = arc_core::detect::coding_agent_spec(agent) else {
        return Ok(());
    };
    let skills_dir = root.join(spec.skills_subdir);
    track_global_skill_install(&skills_dir, agent, skill, source_path)
        .map_err(|err| ArcError::new(err.message))
}

fn clear_global_skill_tracking(
    cache: &DetectCache,
    skill: &str,
    targets: Option<&[String]>,
) -> Result<(), ArcError> {
    let target_ids: Vec<String> = targets
        .map(|items| items.to_vec())
        .unwrap_or_else(|| cache.detected_agents().keys().cloned().collect());

    for agent in target_ids {
        let Some(agent_info) = cache.get_agent(&agent) else {
            continue;
        };
        let Some(root) = &agent_info.root else {
            continue;
        };
        let Some(spec) = arc_core::detect::coding_agent_spec(&agent) else {
            continue;
        };
        untrack_global_skill_install(&root.join(spec.skills_subdir), skill)
            .map_err(|err| ArcError::new(err.message))?;
    }

    Ok(())
}

// ── info ─────────────────────────────────────────────────

fn info(
    paths: &ArcPaths,
    cache: &DetectCache,
    args: SkillInfoArgs,
    fmt: &OutputFormat,
) -> Result<(), ArcError> {
    let registry = SkillRegistry::new(paths.clone(), cache.clone());
    if *fmt != OutputFormat::Json {
        print_bootstrap_report(&registry.bootstrap_catalog()?);
    } else {
        let _ = registry.bootstrap_catalog();
    }

    let Some(skill) = registry.find(&args.name) else {
        return Err(ArcError::new(format!("skill '{}' not found.", args.name)));
    };

    if *fmt == OutputFormat::Json {
        let resolved = registry
            .resolve_source_path(&skill)
            .unwrap_or_else(|_| skill.source_path.clone());
        print_json(&SkillInfoOutput {
            schema_version: SCHEMA_VERSION,
            name: skill.name.clone(),
            origin: skill.origin_display(),
            summary: skill.summary.clone(),
            installed_targets: skill.installed_targets.clone(),
            source_path: resolved.display().to_string(),
        })?;
        return Ok(());
    }

    render_skill_detail(&registry, &skill);
    Ok(())
}

fn render_skill_detail(registry: &SkillRegistry, skill: &SkillEntry) {
    println!();
    println!("  {}", style(&skill.name).bold());
    println!();

    println!("  {}    {}", style("Origin").dim(), skill.origin_display());

    if skill.installed_targets.is_empty() {
        println!(
            "  {}    {}",
            style("Status").dim(),
            style("not installed").yellow()
        );
    } else {
        let names = agent_display_names(&skill.installed_targets);
        println!(
            "  {}    {} → {}",
            style("Status").dim(),
            style("installed").green(),
            names
        );
    }

    if !skill.summary.is_empty() {
        println!();
        println!("  {}", skill.summary);
    }

    let resolved = registry
        .resolve_source_path(skill)
        .unwrap_or(skill.source_path.clone());
    let skill_md = resolved.join("SKILL.md");
    if skill_md.is_file()
        && let Ok(content) = fs::read_to_string(&skill_md)
    {
        let body = strip_frontmatter(&content);
        if !body.is_empty() {
            println!();
            println!("  {}", style("─".repeat(40)).dim());
            for line in body.lines().take(20) {
                println!("  {line}");
            }
            if body.lines().count() > 20 {
                println!("  {}", style("…").dim());
            }
        }
    }
    println!();
}

fn render_skill_list(skills: &[SkillEntry]) {
    for skill in skills {
        let origin = skill.origin_display();
        let status = if skill.installed_targets.is_empty() {
            "not installed".to_string()
        } else {
            format!(
                "installed → {}",
                agent_display_names(&skill.installed_targets)
            )
        };
        println!("{}  [{}]  {}", skill.name, origin, status);
        if !skill.summary.is_empty() {
            println!("  {}", style(&skill.summary).dim());
        }
    }
}

fn strip_frontmatter(content: &str) -> &str {
    if !content.starts_with("---") {
        return content;
    }
    let Some(end) = content[3..].find("\n---") else {
        return content;
    };
    content[(3 + end + 4)..].trim_start_matches('\n')
}

// ── shared ───────────────────────────────────────────────

fn print_bootstrap_report(report: &MarketSyncReport) {
    if report.source_count > 0 && (report.cloned_count > 0 || report.resource_count > 0) {
        println!(
            "  {} Bootstrapped {} market sources and indexed {} resources",
            style("✓").green(),
            report.source_count,
            report.resource_count
        );
    }
}
