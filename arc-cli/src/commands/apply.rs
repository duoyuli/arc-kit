use std::env;
use std::io::{self, IsTerminal};

use arc_core::capability::{AppliedScope, CapabilityTargetState, DesiredScope};
use arc_core::detect::DetectCache;
use arc_core::error::ArcError;
use arc_core::models::ResourceKind;
use arc_core::paths::ArcPaths;
use arc_core::project::{
    ConfigSource, EffectiveConfig, ProjectApplyExecution, ProjectMarketEventStatus,
    ProjectSkillApplyStatus, execute_project_apply, find_project_config, prepare_project_apply,
};
use arc_core::provider::seed_default_providers;
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
    seed_default_providers(paths, cache);

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
        if !arc_toml_wizard::create_arc_toml_interactive(paths, cache, &cwd)? {
            return Ok(());
        }
    }

    let plan = prepare_project_apply(paths, cache, &cwd)?;

    if *fmt == OutputFormat::Json {
        return apply_json(paths, cache, &plan, args);
    }

    println!();
    println!("{}", style(&plan.effective.project_name).bold());

    let printed_requirements =
        print_project_requirements_status(&plan.effective, plan.provider_to_switch.as_deref());
    if !printed_requirements {
        println!();
    }

    render_market_events(&plan);
    println!();

    let targets = if plan.effective.missing_installable.is_empty() {
        Vec::new()
    } else {
        resolve_project_install_targets(cache, args, fmt)?
    };
    let execution = execute_project_apply(paths, cache, &plan, &targets)?;

    render_provider_execution(&execution);
    render_skill_results(&execution);

    for name in &plan.effective.missing_unavailable {
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
    render_missing_project_capability_refs(&plan.effective);

    render_capability_results(&execution);
    render_removed_capabilities(&execution);

    println!();

    if !execution.has_issues(&plan.effective) {
        println!("  {}", style("Ready.").green());
    } else {
        println!("  {}", style("Partially ready").yellow(),);
    }
    println!();

    Ok(())
}

fn print_project_requirements_status(
    effective: &EffectiveConfig,
    provider_to_switch: Option<&str>,
) -> bool {
    let mut printed_sections = 0usize;

    if print_required_provider_status(effective, provider_to_switch) {
        printed_sections += 1;
    }
    if print_required_skills_status(effective) {
        printed_sections += 1;
    }
    if print_required_named_status(
        "mcps",
        &effective.required_mcps,
        &effective.missing_mcps_unavailable,
        "will apply",
    ) {
        printed_sections += 1;
    }
    if print_required_named_status(
        "subagents",
        &effective.required_subagents,
        &effective.missing_subagents_unavailable,
        "will apply",
    ) {
        printed_sections += 1;
    }

    if printed_sections > 0 {
        println!();
    }
    printed_sections > 0
}

fn print_required_provider_status(
    effective: &EffectiveConfig,
    provider_to_switch: Option<&str>,
) -> bool {
    let Some(provider) = &effective.provider else {
        return false;
    };
    if provider.source != ConfigSource::Project {
        return false;
    }

    println!();
    println!(
        "  {} {}",
        style("provider").bold(),
        style("(required in arc.toml)").dim()
    );
    let status_line = if provider_to_switch.is_some() {
        format!("{}", style("will switch").cyan())
    } else {
        format!("{}", style("present").green())
    };
    println!("    {}  {}", style(&provider.value).bold(), status_line);
    true
}

/// Lists each `[skills] require` entry and whether it is present, installable, or unknown.
fn print_required_skills_status(effective: &EffectiveConfig) -> bool {
    if effective.required_skills.is_empty() {
        return false;
    }

    println!();
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
    true
}

fn print_required_named_status(
    title: &str,
    required: &[String],
    unavailable: &[String],
    install_status: &str,
) -> bool {
    if required.is_empty() {
        return false;
    }

    println!();
    println!(
        "  {} {}",
        style(title).bold(),
        style("(required in arc.toml)").dim()
    );
    for name in required {
        let status_line = if unavailable.contains(name) {
            format!("{}", style("not in catalog").yellow())
        } else {
            format!("{}", style(install_status).cyan())
        };
        println!("    {}  {}", style(name).bold(), status_line);
    }
    true
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
    plan: &arc_core::project::ProjectApplyPlan,
    args: &ProjectApplyArgs,
) -> Result<(), ArcError> {
    let targets = if plan.effective.missing_installable.is_empty() {
        Vec::new()
    } else {
        resolve_project_install_targets(cache, args, &OutputFormat::Json)?
    };
    let execution = execute_project_apply(paths, cache, plan, &targets)?;
    let items = execution_to_write_items(&execution, &plan.effective);
    let ok = !execution.has_issues(&plan.effective) && !items.iter().any(item_has_issue);

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

fn render_market_events(plan: &arc_core::project::ProjectApplyPlan) {
    for event in &plan.market_events {
        match event.status {
            ProjectMarketEventStatus::Added => {
                println!(
                    "  {} market {} -> {}",
                    style("+").green(),
                    style(&event.source_id).bold(),
                    style(&event.url).dim()
                );
            }
            ProjectMarketEventStatus::Failed => {
                println!(
                    "  {} market {} - failed to add",
                    style("!").yellow(),
                    style(&event.source_id).bold()
                );
            }
        }
    }
}

fn render_provider_execution(execution: &ProjectApplyExecution) {
    let Some(provider_switch) = &execution.provider_switch else {
        return;
    };
    println!("  provider  -> {}", style(&provider_switch.name).cyan());
    for agent in &provider_switch.agents {
        println!(
            "  {} provider {} -> {}",
            style("+").green(),
            style(&provider_switch.name).bold(),
            agent_display_name(agent)
        );
    }
}

fn render_skill_results(execution: &ProjectApplyExecution) {
    for item in &execution.skill_results {
        match &item.status {
            ProjectSkillApplyStatus::Installed { agents } => {
                for agent in agents {
                    println!(
                        "  {} {} -> {} (project)",
                        style("+").green(),
                        style(&item.name).bold(),
                        agent_display_name(agent)
                    );
                }
            }
            ProjectSkillApplyStatus::NotFound => {
                println!(
                    "  {} {} - not found",
                    style("x").red(),
                    style(&item.name).bold()
                );
            }
            ProjectSkillApplyStatus::Failed { message } => {
                println!(
                    "  {} {} - {}",
                    style("x").red(),
                    style(&item.name).bold(),
                    style(message).dim()
                );
            }
        }
    }
}

fn render_capability_results(execution: &ProjectApplyExecution) {
    for item in &execution.capability_results {
        let kind = match item.kind {
            ResourceKind::Mcp => "mcp",
            ResourceKind::SubAgent => "subagent",
            _ => "resource",
        };
        render_capability_target(kind, &item.name, &item.status);
    }
}

fn render_missing_project_capability_refs(effective: &EffectiveConfig) {
    for name in &effective.missing_mcps_unavailable {
        println!(
            "  {} mcp {} not found in merged catalog, skipped.",
            style("!").yellow(),
            style(name).bold()
        );
        println!(
            "    {}",
            style("Add it to the global registry with `arc mcp define` or `arc mcp install`.")
                .dim()
        );
    }

    for name in &effective.missing_subagents_unavailable {
        println!(
            "  {} subagent {} not found in merged catalog, skipped.",
            style("!").yellow(),
            style(name).bold()
        );
        println!(
            "    {}",
            style("Add it to the global registry with `arc subagent install`.").dim()
        );
    }
}

fn render_removed_capabilities(execution: &ProjectApplyExecution) {
    for record in &execution.removed_capabilities {
        println!(
            "  {} {} -> {} (removed)",
            style("-").green(),
            style(&record.name).bold(),
            agent_display_name(&record.agent)
        );
    }
}

fn execution_to_write_items(
    execution: &ProjectApplyExecution,
    effective: &EffectiveConfig,
) -> Vec<WriteResultItem> {
    let mut items = Vec::new();

    if let Some(provider_switch) = &execution.provider_switch {
        items.push(WriteResultItem {
            resource_kind: None,
            name: provider_switch.name.clone(),
            agent: if provider_switch.agents.is_empty() {
                "all".to_string()
            } else {
                provider_switch.agents.join(",")
            },
            status: "provider_switched".to_string(),
            desired_scope: None,
            applied_scope: None,
            reason: None,
        });
    }

    for item in &execution.skill_results {
        match &item.status {
            ProjectSkillApplyStatus::Installed { agents } => {
                for agent in agents {
                    items.push(WriteResultItem {
                        resource_kind: None,
                        name: item.name.clone(),
                        agent: agent.clone(),
                        status: "installed".to_string(),
                        desired_scope: None,
                        applied_scope: None,
                        reason: None,
                    });
                }
            }
            ProjectSkillApplyStatus::NotFound => items.push(WriteResultItem {
                resource_kind: None,
                name: item.name.clone(),
                agent: String::new(),
                status: "not_found".to_string(),
                desired_scope: None,
                applied_scope: None,
                reason: None,
            }),
            ProjectSkillApplyStatus::Failed { message } => items.push(WriteResultItem {
                resource_kind: None,
                name: item.name.clone(),
                agent: String::new(),
                status: format!("error: {message}"),
                desired_scope: None,
                applied_scope: None,
                reason: None,
            }),
        }
    }

    items.extend(
        execution
            .capability_results
            .iter()
            .map(|item| WriteResultItem {
                resource_kind: Some(match item.kind {
                    ResourceKind::Mcp => "mcp".to_string(),
                    ResourceKind::SubAgent => "subagent".to_string(),
                    _ => "resource".to_string(),
                }),
                name: item.name.clone(),
                agent: item.status.agent.clone(),
                status: format!("{:?}", item.status.status).to_ascii_lowercase(),
                desired_scope: Some(item.status.desired_scope),
                applied_scope: Some(item.status.applied_scope),
                reason: item.status.reason.clone(),
            }),
    );

    items.extend(
        execution
            .removed_capabilities
            .iter()
            .map(|record| WriteResultItem {
                resource_kind: Some(match record.kind {
                    ResourceKind::Mcp => "mcp".to_string(),
                    ResourceKind::SubAgent => "subagent".to_string(),
                    _ => "resource".to_string(),
                }),
                name: record.name.clone(),
                agent: record.agent.clone(),
                status: "removed".to_string(),
                desired_scope: None,
                applied_scope: Some(match record.applied_scope {
                    arc_core::agent::AppliedResourceScope::Project => {
                        arc_core::capability::AppliedScope::Project
                    }
                    arc_core::agent::AppliedResourceScope::Global => {
                        arc_core::capability::AppliedScope::Global
                    }
                }),
                reason: None,
            }),
    );

    items.extend(
        effective
            .missing_mcps_unavailable
            .iter()
            .map(|name| WriteResultItem {
                resource_kind: Some("mcp".to_string()),
                name: name.clone(),
                agent: String::new(),
                status: "not_found".to_string(),
                desired_scope: Some(DesiredScope::Project),
                applied_scope: Some(AppliedScope::None),
                reason: Some("not_in_catalog".to_string()),
            }),
    );

    items.extend(
        effective
            .missing_subagents_unavailable
            .iter()
            .map(|name| WriteResultItem {
                resource_kind: Some("subagent".to_string()),
                name: name.clone(),
                agent: String::new(),
                status: "not_found".to_string(),
                desired_scope: Some(DesiredScope::Project),
                applied_scope: Some(AppliedScope::None),
                reason: Some("not_in_catalog".to_string()),
            }),
    );

    items
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
