use std::fs;

use arc_core::detect::DetectCache;
use arc_core::error::ArcError;
use arc_core::market::bootstrap::MarketSyncReport;
use arc_core::models::{ResourceKind, SkillEntry};
use arc_core::paths::ArcPaths;
use arc_core::skill::SkillRegistry;
use arc_core::skill::install::uninstall_global_skill;
use arc_tui::{run_skill_browser, run_skill_install_wizard, run_skill_uninstall_wizard};
use console::style;

use crate::cli::{
    OutputFormat, SkillCommand, SkillInfoArgs, SkillInstallArgs, SkillListArgs, SkillUninstallArgs,
};
use crate::commands::common::{
    CommandMode, command_mode, print_not_found_json, render_install_report, require_name_arg,
};
use crate::display::agent_display_names;
use crate::format::{SCHEMA_VERSION, SkillInfoOutput, SkillItem, SkillListOutput, print_json};

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
    let report = registry.bootstrap_catalog()?;
    if *fmt != OutputFormat::Json {
        print_bootstrap_report(&report);
    }
    let mut skills = registry.list_all();
    let tracked = registry.tracked_global_installs(None)?;

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
            scope: "global",
            skills: items,
            installations: tracked,
        })?;
        return Ok(());
    }

    if !tracked.is_empty() {
        println!("Tracked global installations:");
        for install in &tracked {
            println!(
                "  {} -> {}: {}",
                install.skill,
                install.agent,
                install.target_path.display()
            );
        }
    }

    if skills.is_empty() {
        if args.installed {
            println!("  {}", style("No skills installed.").yellow());
        } else {
            println!("  {}", style("No skills found.").yellow());
        }
        return Ok(());
    }

    if !matches!(command_mode(fmt), CommandMode::Interactive) {
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
    // 参数错误应先于目录和 catalog 初始化处理，空来源也不能绕过校验。
    if args.name.is_none() && !matches!(command_mode(fmt), CommandMode::Interactive) {
        require_name_arg(fmt, "Skill", "arc skill install <name> [--agent <agent>]")?;
    }
    paths
        .ensure_arc_home()
        .map_err(|err| ArcError::new(err.to_string()))?;
    let registry = SkillRegistry::new(paths.clone(), cache.clone());
    let report = registry.bootstrap_catalog()?;
    if *fmt != OutputFormat::Json {
        print_bootstrap_report(&report);
    }
    let mut skills = registry.list_all();
    if skills.is_empty() && args.name.is_none() {
        println!("  {}", style("No skills available.").yellow());
        return Ok(());
    }
    // Sort: installed first, uninstalled last
    skills.sort_by_key(|s| s.installed_targets.is_empty());

    if args.name.is_none() {
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
            install_one(&registry, skill, &selected_agents)?;
        }
        return Ok(());
    }

    let name = args.name.expect("checked optional name");
    let Some(skill) = skills.into_iter().find(|s| s.name == name) else {
        let message = format!("skill '{name}' not found.");
        if *fmt == OutputFormat::Json {
            print_not_found_json(&message)?;
        }
        return Err(ArcError::new(message));
    };
    let targets = if args.agent.is_empty() {
        cache.agents_for_install(&ResourceKind::Skill)
    } else {
        args.agent
    };

    if *fmt == OutputFormat::Json {
        return install_one_json(&registry, &skill, &targets);
    }
    install_one(&registry, &skill, &targets)
}

fn install_one(
    registry: &SkillRegistry,
    skill: &SkillEntry,
    targets: &[String],
) -> Result<(), ArcError> {
    render_install_report(
        &registry.install_global(skill, targets),
        &OutputFormat::Text,
    )
}

fn install_one_json(
    registry: &SkillRegistry,
    skill: &SkillEntry,
    targets: &[String],
) -> Result<(), ArcError> {
    render_install_report(
        &registry.install_global(skill, targets),
        &OutputFormat::Json,
    )
}

// ── uninstall ────────────────────────────────────────────

fn uninstall(
    paths: &ArcPaths,
    cache: &DetectCache,
    args: SkillUninstallArgs,
    fmt: &OutputFormat,
) -> Result<(), ArcError> {
    let Some(name) = args.name else {
        if !matches!(command_mode(fmt), CommandMode::Interactive) {
            require_name_arg(
                fmt,
                "Skill",
                "arc skill uninstall <name> [--agent <agent>] [--all]",
            )?;
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
        return render_install_report(
            &uninstall_global_skill(paths, cache, &name, Some(&targets)),
            fmt,
        );
    };

    let targets = if args.all || args.agent.is_empty() {
        None
    } else {
        Some(args.agent)
    };
    render_install_report(
        &uninstall_global_skill(paths, cache, &name, targets.as_deref()),
        fmt,
    )
}

// ── info ─────────────────────────────────────────────────

fn info(
    paths: &ArcPaths,
    cache: &DetectCache,
    args: SkillInfoArgs,
    fmt: &OutputFormat,
) -> Result<(), ArcError> {
    let registry = SkillRegistry::new(paths.clone(), cache.clone());
    let report = registry.bootstrap_catalog()?;
    if *fmt != OutputFormat::Json {
        print_bootstrap_report(&report);
    }

    let Some(skill) = registry.find(&args.name) else {
        if *fmt == OutputFormat::Json {
            return print_not_found_json(format!("skill '{}' not found.", args.name));
        }
        return Err(ArcError::new(format!("skill '{}' not found.", args.name)));
    };

    if *fmt == OutputFormat::Json {
        let resolved = registry
            .resolve_source_path(&skill)
            .unwrap_or_else(|_| skill.source_path.clone());
        print_json(&SkillInfoOutput {
            schema_version: SCHEMA_VERSION,
            scope: "global",
            name: skill.name.clone(),
            origin: skill.origin_display(),
            summary: skill.summary.clone(),
            installed_targets: skill.installed_targets.clone(),
            source_path: resolved.display().to_string(),
            installations: registry.tracked_global_installs(Some(&skill.name))?,
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
