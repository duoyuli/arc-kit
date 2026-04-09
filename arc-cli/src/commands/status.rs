use arc_core::capability::{CapabilityStatusEntry, CapabilityTargetState, SourceScope};
use std::env;

use arc_core::agent::agent_specs;
use arc_core::error::ArcError;
use arc_core::status::{ProjectState, ProviderMatchState, StatusSnapshot};
use arc_core::{ArcPaths, DetectCache, collect_status};
use console::{Alignment, pad_str, style};

use crate::cli::OutputFormat;
use crate::format::{SCHEMA_VERSION, StatusOutput, print_json};

pub fn run(paths: &ArcPaths, cache: &DetectCache, fmt: &OutputFormat) -> Result<(), ArcError> {
    let cwd = env::current_dir().unwrap_or_default();
    let snapshot = collect_status(paths, &cwd, cache);

    if *fmt == OutputFormat::Json {
        print_json(&StatusOutput {
            schema_version: SCHEMA_VERSION,
            project: snapshot.project,
            agents: snapshot.agents,
            catalog: snapshot.catalog,
            mcps: snapshot.mcps,
            subagents: snapshot.subagents,
            actions: snapshot.actions,
        })?;
        return Ok(());
    }

    render_text(&snapshot);
    Ok(())
}

fn render_text(snapshot: &StatusSnapshot) {
    render_project(&snapshot.project);
    println!();
    render_agents(snapshot);
    println!();
    render_catalog(snapshot);
    println!();
    render_capabilities("MCPs", &snapshot.mcps);
    println!();
    render_capabilities("Subagents", &snapshot.subagents);
    println!();
    render_actions(&snapshot.actions);
    println!();
}

fn render_project(project: &arc_core::status::ProjectStatusSection) {
    println!("{}", style("Project").bold());

    match project.state {
        ProjectState::None => {
            println!("  arc.toml: not found");
        }
        ProjectState::Invalid => {
            if let Some(path) = &project.config_path {
                println!("  config: {}", path.display());
            }
            if let Some(error) = &project.error {
                println!("  error: {error}");
            }
        }
        ProjectState::Active => {
            println!("  repo: {}", project.name);
            if let Some(path) = &project.config_path {
                println!("  config: {}", path.display());
            }
            if let Some(summary) = &project.summary {
                println!(
                    "  skills: {} required · {} ready · {} partial · {} missing · {} unavailable",
                    summary.required_skills,
                    summary.ready_skills,
                    summary.partial_skills,
                    summary.missing_skills,
                    summary.unavailable_skills,
                );
            }
            if let Some(provider) = &project.provider {
                let mut parts = vec![format!("provider {}", provider.name)];
                if provider.matched_agents > 0 {
                    parts.push(format!("{} matched", provider.matched_agents));
                }
                if provider.mismatched_agents > 0 {
                    parts.push(format!("{} mismatch", provider.mismatched_agents));
                }
                if provider.missing_profiles > 0 {
                    parts.push(format!("{} missing profile", provider.missing_profiles));
                }
                println!("  {}", parts.join(" · "));
            }

            if !project.agents.is_empty() {
                println!("  targets:");
                let name_width = project
                    .agents
                    .iter()
                    .map(|agent| agent.name.len())
                    .max()
                    .unwrap_or(0);
                for agent in &project.agents {
                    let provider_label = agent
                        .provider_status
                        .as_ref()
                        .map(render_provider_status)
                        .unwrap_or_default();
                    if provider_label.is_empty() {
                        println!(
                            "    {}  {}/{} ready",
                            pad_str(&agent.name, name_width, Alignment::Left, None),
                            agent.ready_skill_count,
                            agent.total_available_skill_count
                        );
                    } else {
                        println!(
                            "    {}  {}/{} ready  {}",
                            pad_str(&agent.name, name_width, Alignment::Left, None),
                            agent.ready_skill_count,
                            agent.total_available_skill_count,
                            provider_label
                        );
                    }
                }
            } else if project
                .summary
                .as_ref()
                .is_some_and(|summary| summary.required_skills > 0)
            {
                println!("  targets: no detected agent currently supports project-local skills");
            }
        }
    }
}

fn render_agents(snapshot: &StatusSnapshot) {
    println!("{}", style("Agents").bold());

    if snapshot.agents.is_empty() {
        println!("  none detected");
        let names: Vec<&str> = agent_specs()
            .iter()
            .map(|agent| agent.display_name)
            .collect();
        println!("  hint: install a supported agent: {}", names.join(", "));
        return;
    }

    let name_width = snapshot
        .agents
        .iter()
        .map(|agent| agent.name.len())
        .max()
        .unwrap_or(0);
    let version_width = snapshot
        .agents
        .iter()
        .map(|agent| agent.version.as_deref().unwrap_or("-").len())
        .max()
        .unwrap_or(1);
    let provider_width = snapshot
        .agents
        .iter()
        .map(|agent| {
            agent
                .provider
                .as_ref()
                .map(|provider| provider.display_name.len())
                .unwrap_or(1)
        })
        .max()
        .unwrap_or(1);

    for agent in &snapshot.agents {
        let version = agent.version.as_deref().unwrap_or("-");
        let provider = agent
            .provider
            .as_ref()
            .map(|item| item.display_name.as_str())
            .unwrap_or("-");
        let project_local = if agent.supports_project_skills {
            "project-local"
        } else {
            "global-only"
        };
        let mcp_scope = &agent.mcp_scope_supported;
        let subagent = &agent.subagent_supported;
        let transports = if agent.mcp_transports_supported.is_empty() {
            "-".to_string()
        } else {
            agent.mcp_transports_supported.join(",")
        };
        println!(
            "  {}  {}  {}  {} global skills  {}  mcp:{} [{}]  subagent:{}",
            pad_str(&agent.name, name_width, Alignment::Left, None),
            style(pad_str(version, version_width, Alignment::Left, None)).dim(),
            style(pad_str(provider, provider_width, Alignment::Left, None)).cyan(),
            agent.global_skill_count,
            style(project_local).dim(),
            mcp_scope,
            transports,
            subagent,
        );
    }
}

fn render_catalog(snapshot: &StatusSnapshot) {
    println!("{}", style("Catalog").bold());
    println!(
        "  {} markets · {} resources · {} global skills",
        snapshot.catalog.market_count,
        snapshot.catalog.resource_count,
        snapshot.catalog.global_skill_count,
    );
    if snapshot.catalog.unhealthy_market_count > 0 {
        println!(
            "  warning: {} markets report a non-ok status",
            snapshot.catalog.unhealthy_market_count
        );
    }
}

fn render_provider_status(status: &arc_core::status::ProjectProviderAgentStatus) -> String {
    match status.state {
        ProviderMatchState::Matched => style("provider matched").green().to_string(),
        ProviderMatchState::Mismatch => style("provider mismatch").yellow().to_string(),
        ProviderMatchState::MissingProfile => {
            style("provider profile missing").yellow().to_string()
        }
    }
}

fn render_capabilities(title: &str, items: &[CapabilityStatusEntry]) {
    println!("{}", style(title).bold());
    if items.is_empty() {
        println!("  none");
        return;
    }

    for item in items {
        let scope = match item.source_scope {
            SourceScope::Global => "global",
            SourceScope::Project => "project",
        };
        let resolution = match item.resolution {
            arc_core::capability::ResourceResolution::Active => "active",
            arc_core::capability::ResourceResolution::Shadowed => "shadowed",
        };
        println!(
            "  {}  {}  {}",
            style(&item.name).bold(),
            style(scope).cyan(),
            style(resolution).dim()
        );
        if item.targets.is_empty() {
            println!("      {}", style("no current targets").dim());
            continue;
        }
        for target in &item.targets {
            let marker = match target.status {
                CapabilityTargetState::Applied => style("+").green(),
                CapabilityTargetState::Skipped => style("!").yellow(),
                CapabilityTargetState::Failed => style("x").red(),
            };
            let detail = target.reason.as_deref().unwrap_or("ok");
            println!(
                "      {} {}  desired:{}  applied:{}  {}",
                marker,
                target.agent,
                format!("{:?}", target.desired_scope).to_ascii_lowercase(),
                format!("{:?}", target.applied_scope).to_ascii_lowercase(),
                style(detail).dim()
            );
        }
    }
}

fn render_actions(actions: &[arc_core::status::RecommendedAction]) {
    println!("{}", style("Actions").bold());
    if actions.is_empty() {
        println!("  none");
        return;
    }
    for action in actions {
        let marker = match action.severity {
            arc_core::status::ActionSeverity::Info => style("i").cyan(),
            arc_core::status::ActionSeverity::Warn => style("!").yellow(),
        };
        if let Some(command) = &action.command {
            println!("  {} {} ({})", marker, action.message, style(command).dim());
        } else {
            println!("  {} {}", marker, action.message);
        }
    }
}
