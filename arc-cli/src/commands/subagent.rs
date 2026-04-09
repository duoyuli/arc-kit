use std::fs;
use std::path::PathBuf;

use arc_core::ArcPaths;
use arc_core::agent::AppliedResourceScope;
use arc_core::capability::{
    CapabilityTargetState, SourceScope, SubagentApplyPlan, SubagentDefinition, apply_subagent_plan,
    list_tracked_capability_installs, load_global_subagents, remove_global_subagent,
    remove_tracked_capability, save_global_subagent,
};
use arc_core::detect::DetectCache;
use arc_core::error::ArcError;
use arc_core::models::ResourceKind;
use console::style;

use crate::cli::{
    OutputFormat, SubagentCommand, SubagentInfoArgs, SubagentInstallArgs, SubagentUninstallArgs,
};
use crate::format::{
    ErrorOutput, SCHEMA_VERSION, SubagentInfoOutput, SubagentItem, SubagentListOutput, WriteResult,
    WriteResultItem, print_json,
};

pub fn run(
    paths: &ArcPaths,
    cache: &DetectCache,
    command: SubagentCommand,
    fmt: &OutputFormat,
) -> Result<(), ArcError> {
    match command {
        SubagentCommand::List => list(paths, fmt),
        SubagentCommand::Info(args) => info(paths, args, fmt),
        SubagentCommand::Install(args) => install(paths, cache, args, fmt),
        SubagentCommand::Uninstall(args) => uninstall(paths, args, fmt),
    }
}

fn list(paths: &ArcPaths, fmt: &OutputFormat) -> Result<(), ArcError> {
    let subagents = load_global_subagents(paths)?;
    if *fmt == OutputFormat::Json {
        return print_json(&SubagentListOutput {
            schema_version: SCHEMA_VERSION,
            subagents: subagents
                .into_iter()
                .map(|item| SubagentItem {
                    name: item.name,
                    description: item.description,
                    targets: item.targets,
                    prompt_file: item.prompt_file,
                })
                .collect(),
        });
    }

    if subagents.is_empty() {
        println!("  {}", style("No global subagents installed.").yellow());
        return Ok(());
    }

    for item in subagents {
        println!("  {}", style(&item.name).bold());
        if let Some(description) = item.description {
            println!("      {}", style(description).dim());
        }
        println!(
            "      {}",
            style(
                item.targets
                    .map(|targets| targets.join(", "))
                    .unwrap_or_else(|| "all detected agents".to_string())
            )
            .dim()
        );
    }
    Ok(())
}

fn info(paths: &ArcPaths, args: SubagentInfoArgs, fmt: &OutputFormat) -> Result<(), ArcError> {
    let subagents = load_global_subagents(paths)?;
    let Some(subagent) = subagents.into_iter().find(|item| item.name == args.name) else {
        if *fmt == OutputFormat::Json {
            return print_json(&ErrorOutput {
                schema_version: SCHEMA_VERSION,
                ok: false,
                error: format!("subagent '{}' not found", args.name),
            });
        }
        return Err(ArcError::new(format!("subagent '{}' not found", args.name)));
    };
    if *fmt == OutputFormat::Json {
        return print_json(&SubagentInfoOutput {
            schema_version: SCHEMA_VERSION,
            name: subagent.name,
            description: subagent.description,
            targets: subagent.targets,
            prompt_file: subagent.prompt_file,
        });
    }

    println!("  {}", style(&subagent.name).bold());
    if let Some(description) = subagent.description {
        println!("  description: {}", description);
    }
    println!("  prompt_file: {}", subagent.prompt_file);
    println!(
        "  targets: {}",
        subagent
            .targets
            .map(|targets| targets.join(", "))
            .unwrap_or_else(|| "all detected agents".to_string())
    );
    Ok(())
}

fn install(
    paths: &ArcPaths,
    cache: &DetectCache,
    args: SubagentInstallArgs,
    fmt: &OutputFormat,
) -> Result<(), ArcError> {
    let prompt_path = PathBuf::from(&args.prompt_file);
    let prompt_body = fs::read_to_string(&prompt_path)
        .map_err(|e| ArcError::new(format!("failed to read {}: {e}", prompt_path.display())))?;
    let definition = SubagentDefinition {
        name: args.name,
        description: args.description,
        targets: if args.agent.is_empty() {
            None
        } else {
            Some(args.agent)
        },
        prompt_file: prompt_path.display().to_string(),
    };
    save_global_subagent(paths, &definition, &prompt_body)?;
    let statuses = apply_subagent_plan(
        paths,
        cache,
        &SubagentApplyPlan {
            definition: definition.clone(),
            prompt_path,
            source_scope: SourceScope::Global,
        },
        None,
    )?;

    if *fmt == OutputFormat::Json {
        return print_json(&WriteResult {
            schema_version: SCHEMA_VERSION,
            ok: !statuses
                .iter()
                .any(|item| item.status == CapabilityTargetState::Failed),
            message: format!("Subagent '{}' installed.", definition.name),
            items: statuses
                .into_iter()
                .map(|item| WriteResultItem {
                    resource_kind: Some("subagent".to_string()),
                    name: definition.name.clone(),
                    agent: item.agent,
                    status: format!("{:?}", item.status).to_ascii_lowercase(),
                    desired_scope: Some(item.desired_scope),
                    applied_scope: Some(item.applied_scope),
                    reason: item.reason,
                })
                .collect(),
        });
    }

    for item in statuses {
        let marker = match item.status {
            CapabilityTargetState::Applied => style("+").green(),
            CapabilityTargetState::Skipped => style("!").yellow(),
            CapabilityTargetState::Failed => style("x").red(),
        };
        let detail = item.reason.unwrap_or_else(|| "ok".to_string());
        println!(
            "  {} {} -> {} ({})",
            marker,
            style(&definition.name).bold(),
            item.agent,
            detail
        );
    }
    Ok(())
}

fn uninstall(
    paths: &ArcPaths,
    args: SubagentUninstallArgs,
    fmt: &OutputFormat,
) -> Result<(), ArcError> {
    remove_global_subagent(paths, &args.name)?;
    let tracked = list_tracked_capability_installs(paths);
    let mut removed = Vec::new();
    for record in tracked.into_iter().filter(|record| {
        record.kind == ResourceKind::SubAgent
            && record.name == args.name
            && record.source_scope == SourceScope::Global
    }) {
        remove_tracked_capability(paths, &record, None)?;
        removed.push(record);
    }

    if *fmt == OutputFormat::Json {
        return print_json(&WriteResult {
            schema_version: SCHEMA_VERSION,
            ok: true,
            message: format!("Subagent '{}' removed.", args.name),
            items: removed
                .into_iter()
                .map(|record| WriteResultItem {
                    resource_kind: Some("subagent".to_string()),
                    name: record.name,
                    agent: record.agent,
                    status: "removed".to_string(),
                    desired_scope: None,
                    applied_scope: Some(match record.applied_scope {
                        AppliedResourceScope::Project => {
                            arc_core::capability::AppliedScope::Project
                        }
                        AppliedResourceScope::Global | AppliedResourceScope::GlobalFallback => {
                            arc_core::capability::AppliedScope::Global
                        }
                    }),
                    reason: None,
                })
                .collect(),
        });
    }

    if removed.is_empty() {
        println!(
            "  {}",
            style("No tracked global subagent installs were present.").dim()
        );
    } else {
        for record in removed {
            println!(
                "  {} {} -> {}",
                style("-").green(),
                style(&record.name).bold(),
                record.agent
            );
        }
    }
    Ok(())
}
