use std::env;
use std::io::{self, IsTerminal};
use std::path::Path;

use arc_core::capability::{
    CapabilityTargetState, McpApplyPlan, SourceScope, SubagentApplyPlan, apply_mcp_plan,
    apply_subagent_plan, list_tracked_capability_installs, remove_tracked_capability,
    tracking_record_for_target, validate_mcp_definition, validate_subagent_definition,
};
use arc_core::detect::DetectCache;
use arc_core::error::ArcError;
use arc_core::market::bootstrap::sync_market_source_resources;
use arc_core::market::sources::MarketSourceRegistry;
use arc_core::models::ResourceKind;
use arc_core::project::{
    EffectiveConfig, ProjectConfig, find_project_config, load_project_config,
    resolve_effective_config,
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
    arc_core::seed_default_providers(paths, cache);

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
        return apply_json(
            paths,
            cache,
            &registry,
            &effective,
            project_cfg.as_ref(),
            provider_switch,
            args,
        );
    }

    println!();
    println!("  {}", style(&effective.project_name).bold());

    print_required_skills_status(&effective);

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

    let capability_issues = if let Some(cfg) = &project_cfg {
        apply_project_capabilities_text(paths, cache, cfg, project_root, args)?
    } else {
        false
    };

    println!();

    if effective.missing_unavailable.is_empty() && !capability_issues {
        println!("  {}", style("Ready.").green());
    } else {
        println!("  {}", style("Partially ready").yellow(),);
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
    project_cfg: Option<&ProjectConfig>,
    provider_switch: Option<&str>,
    args: &ProjectApplyArgs,
) -> Result<(), ArcError> {
    let mut items = Vec::new();

    if let Some(provider_name) = provider_switch {
        apply_provider_switch(paths, provider_name, false)?;
        items.push(WriteResultItem {
            resource_kind: None,
            name: provider_name.to_string(),
            agent: "all".to_string(),
            status: "provider_switched".to_string(),
            desired_scope: None,
            applied_scope: None,
            reason: None,
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
                resource_kind: None,
                name: name.clone(),
                agent: "".to_string(),
                status: "not_found".to_string(),
                desired_scope: None,
                applied_scope: None,
                reason: None,
            });
            continue;
        };
        let source_path = match registry.resolve_source_path(&skill) {
            Ok(p) => p,
            Err(e) => {
                items.push(WriteResultItem {
                    resource_kind: None,
                    name: name.clone(),
                    agent: "".to_string(),
                    status: format!("error: {}", e.message),
                    desired_scope: None,
                    applied_scope: None,
                    reason: None,
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
                        resource_kind: None,
                        name: name.clone(),
                        agent: agent.clone(),
                        status: "installed".to_string(),
                        desired_scope: None,
                        applied_scope: None,
                        reason: None,
                    });
                }
            }
            Err(e) => {
                items.push(WriteResultItem {
                    resource_kind: None,
                    name: name.clone(),
                    agent: "".to_string(),
                    status: format!("error: {}", e.message),
                    desired_scope: None,
                    applied_scope: None,
                    reason: None,
                });
            }
        }
    }

    if let Some(cfg) = project_cfg {
        apply_project_capabilities_json(paths, cache, cfg, project_root, args, &mut items)?;
    }

    let ok = effective.missing_unavailable.is_empty() && !items.iter().any(item_has_issue);

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

fn apply_project_capabilities_text(
    paths: &ArcPaths,
    cache: &DetectCache,
    cfg: &ProjectConfig,
    project_root: &Path,
    args: &ProjectApplyArgs,
) -> Result<bool, ArcError> {
    let mut had_issues = false;
    for definition in &cfg.mcps {
        let mut definition = definition.clone();
        validate_mcp_definition(&mut definition, SourceScope::Project)?;
        let statuses = apply_mcp_plan(
            paths,
            cache,
            &McpApplyPlan {
                definition: definition.clone(),
                source_scope: SourceScope::Project,
            },
            Some(project_root),
            args.allow_global_fallback,
        )?;
        for item in statuses {
            had_issues |= item.status != CapabilityTargetState::Applied;
            render_capability_target("mcp", &definition.name, &item);
        }
    }

    for definition in &cfg.subagents {
        let mut definition = definition.clone();
        let prompt_path =
            validate_subagent_definition(&mut definition, SourceScope::Project, project_root)?;
        let statuses = apply_subagent_plan(
            paths,
            cache,
            &SubagentApplyPlan {
                definition: definition.clone(),
                prompt_path,
                source_scope: SourceScope::Project,
            },
            Some(project_root),
        )?;
        for item in statuses {
            had_issues |= item.status != CapabilityTargetState::Applied;
            render_capability_target("subagent", &definition.name, &item);
        }
    }

    cleanup_removed_project_capabilities(
        paths,
        cache,
        cfg,
        project_root,
        args.allow_global_fallback,
        true,
        None,
    )?;
    Ok(had_issues)
}

fn apply_project_capabilities_json(
    paths: &ArcPaths,
    cache: &DetectCache,
    cfg: &ProjectConfig,
    project_root: &Path,
    args: &ProjectApplyArgs,
    items: &mut Vec<WriteResultItem>,
) -> Result<(), ArcError> {
    for definition in &cfg.mcps {
        let mut definition = definition.clone();
        validate_mcp_definition(&mut definition, SourceScope::Project)?;
        let statuses = apply_mcp_plan(
            paths,
            cache,
            &McpApplyPlan {
                definition: definition.clone(),
                source_scope: SourceScope::Project,
            },
            Some(project_root),
            args.allow_global_fallback,
        )?;
        items.extend(statuses.into_iter().map(|item| WriteResultItem {
            resource_kind: Some("mcp".to_string()),
            name: definition.name.clone(),
            agent: item.agent,
            status: format!("{:?}", item.status).to_ascii_lowercase(),
            desired_scope: Some(item.desired_scope),
            applied_scope: Some(item.applied_scope),
            reason: item.reason,
        }));
    }

    for definition in &cfg.subagents {
        let mut definition = definition.clone();
        let prompt_path =
            validate_subagent_definition(&mut definition, SourceScope::Project, project_root)?;
        let statuses = apply_subagent_plan(
            paths,
            cache,
            &SubagentApplyPlan {
                definition: definition.clone(),
                prompt_path,
                source_scope: SourceScope::Project,
            },
            Some(project_root),
        )?;
        items.extend(statuses.into_iter().map(|item| WriteResultItem {
            resource_kind: Some("subagent".to_string()),
            name: definition.name.clone(),
            agent: item.agent,
            status: format!("{:?}", item.status).to_ascii_lowercase(),
            desired_scope: Some(item.desired_scope),
            applied_scope: Some(item.applied_scope),
            reason: item.reason,
        }));
    }

    cleanup_removed_project_capabilities(
        paths,
        cache,
        cfg,
        project_root,
        args.allow_global_fallback,
        false,
        Some(items),
    )
}

fn cleanup_removed_project_capabilities(
    paths: &ArcPaths,
    cache: &DetectCache,
    cfg: &ProjectConfig,
    project_root: &Path,
    allow_global_fallback: bool,
    print: bool,
    items: Option<&mut Vec<WriteResultItem>>,
) -> Result<(), ArcError> {
    let desired_records =
        desired_project_capability_records(paths, cache, cfg, project_root, allow_global_fallback)?;
    let tracked = list_tracked_capability_installs(paths);
    let mut removed_items = Vec::new();
    for record in tracked.into_iter().filter(|record| {
        record.source_scope == SourceScope::Project
            && record.project_root.as_deref() == Some(project_root)
            && matches!(record.kind, ResourceKind::Mcp | ResourceKind::SubAgent)
            && !desired_records.iter().any(|desired| desired == record)
    }) {
        remove_tracked_capability(paths, &record, Some(project_root))?;
        removed_items.push(record);
    }

    if print {
        for record in &removed_items {
            println!(
                "  {} {} -> {} (removed)",
                style("-").green(),
                style(&record.name).bold(),
                agent_display_name(&record.agent)
            );
        }
    }
    if let Some(items) = items {
        items.extend(removed_items.into_iter().map(|record| WriteResultItem {
            resource_kind: Some(match record.kind {
                ResourceKind::Mcp => "mcp".to_string(),
                ResourceKind::SubAgent => "subagent".to_string(),
                _ => "resource".to_string(),
            }),
            name: record.name,
            agent: record.agent,
            status: "removed".to_string(),
            desired_scope: None,
            applied_scope: Some(match record.applied_scope {
                arc_core::agent::AppliedResourceScope::Project => {
                    arc_core::capability::AppliedScope::Project
                }
                arc_core::agent::AppliedResourceScope::Global
                | arc_core::agent::AppliedResourceScope::GlobalFallback => {
                    arc_core::capability::AppliedScope::Global
                }
            }),
            reason: None,
        }));
    }
    Ok(())
}

fn desired_project_capability_records(
    paths: &ArcPaths,
    cache: &DetectCache,
    cfg: &ProjectConfig,
    project_root: &Path,
    allow_global_fallback: bool,
) -> Result<Vec<arc_core::capability::TrackedCapabilityInstall>, ArcError> {
    let tracked = list_tracked_capability_installs(paths);
    let mut desired = Vec::new();

    for definition in &cfg.mcps {
        let mut definition = definition.clone();
        validate_mcp_definition(&mut definition, SourceScope::Project)?;
        let statuses = arc_core::capability::preview_mcp_plan(
            paths,
            cache,
            &tracked,
            &McpApplyPlan {
                definition: definition.clone(),
                source_scope: SourceScope::Project,
            },
            Some(project_root),
            allow_global_fallback,
        )?;
        desired.extend(statuses.into_iter().filter_map(|status| {
            tracking_record_for_target(
                ResourceKind::Mcp,
                &definition.name,
                SourceScope::Project,
                &status,
                Some(project_root),
            )
        }));
    }

    for definition in &cfg.subagents {
        let mut definition = definition.clone();
        let prompt_path =
            validate_subagent_definition(&mut definition, SourceScope::Project, project_root)?;
        let statuses = arc_core::capability::preview_subagent_plan(
            paths,
            cache,
            &SubagentApplyPlan {
                definition: definition.clone(),
                prompt_path,
                source_scope: SourceScope::Project,
            },
            Some(project_root),
        )?;
        desired.extend(statuses.into_iter().filter_map(|status| {
            tracking_record_for_target(
                ResourceKind::SubAgent,
                &definition.name,
                SourceScope::Project,
                &status,
                Some(project_root),
            )
        }));
    }

    Ok(desired)
}

fn render_capability_target(
    kind: &str,
    name: &str,
    item: &arc_core::capability::CapabilityTargetStatus,
) {
    let marker = match item.status {
        CapabilityTargetState::Applied => style("+").green(),
        CapabilityTargetState::Skipped => style("!").yellow(),
        CapabilityTargetState::Failed => style("x").red(),
    };
    let detail = item.reason.as_deref().unwrap_or("ok");
    println!(
        "  {} {} {} -> {} ({})",
        marker,
        style(kind).cyan(),
        style(name).bold(),
        agent_display_name(&item.agent),
        style(detail).dim()
    );
}

fn item_has_issue(item: &WriteResultItem) -> bool {
    matches!(item.status.as_str(), "failed" | "skipped" | "not_found")
        || item.status.starts_with("error")
}
