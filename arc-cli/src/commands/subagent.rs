use std::fs;
use std::io::{self, IsTerminal};
use std::path::PathBuf;

use arc_core::ArcPaths;
use arc_core::agent::AppliedResourceScope;
use arc_core::capability::{
    CapabilityTargetState, SourceScope, SubagentApplyPlan, SubagentDefinition, apply_subagent_plan,
    list_tracked_capability_installs, remove_global_subagent, remove_tracked_capability,
    save_global_subagent, validate_subagent_targets,
};
use arc_core::detect::DetectCache;
use arc_core::error::ArcError;
use arc_core::models::ResourceKind;
use arc_core::subagent_registry::{
    SubagentEntryOrigin, find_global_subagent, load_merged_subagent_catalog,
};
use arc_tui::run_subagent_install_wizard;
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
    let subagents = load_merged_subagent_catalog(paths)?;
    if *fmt == OutputFormat::Json {
        return print_json(&SubagentListOutput {
            schema_version: SCHEMA_VERSION,
            subagents: subagents
                .into_iter()
                .map(|item| SubagentItem {
                    name: item.definition.name,
                    origin: match item.origin {
                        SubagentEntryOrigin::Builtin => "builtin".to_string(),
                        SubagentEntryOrigin::User => "user".to_string(),
                    },
                    description: item.definition.description,
                    targets: item.definition.targets,
                    prompt_file: item.definition.prompt_file,
                })
                .collect(),
        });
    }

    if subagents.is_empty() {
        println!("  {}", style("No global subagents available.").yellow());
        return Ok(());
    }

    for item in subagents {
        let origin = match item.origin {
            SubagentEntryOrigin::Builtin => "built-in",
            SubagentEntryOrigin::User => "user",
        };
        println!(
            "  {} {}",
            style(&item.definition.name).bold(),
            style(format!("[{origin}]")).dim()
        );
        if let Some(description) = item.definition.description {
            println!("      {}", style(description).dim());
        }
        println!(
            "      {}",
            style(
                item.definition
                    .targets
                    .map(|targets| targets.join(", "))
                    .unwrap_or_else(|| "all detected agents".to_string())
            )
            .dim()
        );
    }
    Ok(())
}

fn info(paths: &ArcPaths, args: SubagentInfoArgs, fmt: &OutputFormat) -> Result<(), ArcError> {
    let Some(subagent) = find_global_subagent(paths, &args.name)? else {
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
            name: subagent.definition.name,
            origin: match subagent.origin {
                SubagentEntryOrigin::Builtin => "builtin".to_string(),
                SubagentEntryOrigin::User => "user".to_string(),
            },
            description: subagent.definition.description,
            targets: subagent.definition.targets,
            prompt_file: subagent.definition.prompt_file,
        });
    }

    println!(
        "  {} {}",
        style(&subagent.definition.name).bold(),
        style(match subagent.origin {
            SubagentEntryOrigin::Builtin => "[built-in]",
            SubagentEntryOrigin::User => "[user]",
        })
        .dim()
    );
    if let Some(description) = subagent.definition.description {
        println!("  description: {}", description);
    }
    println!("  prompt_file: {}", subagent.definition.prompt_file);
    println!(
        "  targets: {}",
        subagent
            .definition
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
    let resolved = resolve_install_inputs(paths, cache, args, fmt)?;
    let definition = resolved.definition;
    let prompt_path = resolved.prompt_path;
    let prompt_body = resolved.prompt_body;
    let _ = validate_subagent_targets(cache, &definition)?;
    if resolved.persist_definition {
        save_global_subagent(paths, &definition, &prompt_body)?;
    }
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

struct ResolvedInstallInputs {
    definition: SubagentDefinition,
    prompt_path: PathBuf,
    prompt_body: String,
    persist_definition: bool,
}

fn resolve_install_inputs(
    paths: &ArcPaths,
    cache: &DetectCache,
    args: SubagentInstallArgs,
    fmt: &OutputFormat,
) -> Result<ResolvedInstallInputs, ArcError> {
    let is_tty = io::stdin().is_terminal() && io::stdout().is_terminal();
    let usage = "Usage: arc subagent install <name> [--prompt-file <path>] [--agent <agent>] [--description <text>]".to_string();

    if args.name.is_none() {
        if *fmt == OutputFormat::Json || !is_tty {
            return Err(ArcError::with_hint(
                "Subagent name required in non-interactive mode.".to_string(),
                usage,
            ));
        }

        let available_agents = cache.agents_for_install(&ResourceKind::SubAgent);
        let selected_agents = if args.agent.is_empty() {
            available_agents.clone()
        } else {
            args.agent.clone()
        };
        let (name, description, prompt_file, agents) = run_subagent_install_wizard(
            &available_agents,
            args.name.as_deref(),
            args.description.as_deref(),
            args.prompt_file.as_deref(),
            &selected_agents,
        )
        .map_err(|err| ArcError::new(format!("interactive install failed: {err}")))?;
        let prompt_path = PathBuf::from(&prompt_file);
        let prompt_body = fs::read_to_string(&prompt_path)
            .map_err(|e| ArcError::new(format!("failed to read {}: {e}", prompt_path.display())))?;
        return Ok(ResolvedInstallInputs {
            definition: SubagentDefinition {
                name,
                description,
                targets: if agents.is_empty() {
                    None
                } else {
                    Some(agents)
                },
                prompt_file: prompt_path.display().to_string(),
            },
            prompt_path,
            prompt_body,
            persist_definition: true,
        });
    }

    let name = args.name.expect("checked optional name");
    if let Some(prompt_file) = args.prompt_file {
        let prompt_path = PathBuf::from(&prompt_file);
        let prompt_body = fs::read_to_string(&prompt_path)
            .map_err(|e| ArcError::new(format!("failed to read {}: {e}", prompt_path.display())))?;
        return Ok(ResolvedInstallInputs {
            definition: SubagentDefinition {
                name,
                description: args.description,
                targets: if args.agent.is_empty() {
                    None
                } else {
                    Some(args.agent)
                },
                prompt_file: prompt_path.display().to_string(),
            },
            prompt_path,
            prompt_body,
            persist_definition: true,
        });
    }

    let Some(entry) = find_global_subagent(paths, &name)? else {
        return Err(ArcError::with_hint(
            "Prompt file required in non-interactive mode unless the subagent exists in the built-in/user catalog.".to_string(),
            usage,
        ));
    };
    let mut definition = entry.definition;
    if let Some(description) = args.description {
        definition.description = Some(description);
    }
    if !args.agent.is_empty() {
        definition.targets = Some(args.agent);
    }
    let prompt_path = PathBuf::from(&definition.prompt_file);
    Ok(ResolvedInstallInputs {
        definition,
        prompt_path,
        prompt_body: entry.prompt_body,
        persist_definition: false,
    })
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
