use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{self, IsTerminal};
use std::path::PathBuf;

use arc_core::agent::AppliedResourceScope;
use arc_core::capability::{
    apply_subagent_plan, list_tracked_capability_installs, remove_global_subagent,
    remove_tracked_capability, save_global_subagent, validate_subagent_targets,
    CapabilityTargetState, SourceScope, SubagentApplyPlan, SubagentDefinition,
    TrackedCapabilityInstall,
};
use arc_core::detect::DetectCache;
use arc_core::error::ArcError;
use arc_core::models::ResourceKind;
use arc_core::paths::ArcPaths;
use arc_core::subagent_registry::{
    find_global_subagent, load_merged_subagent_catalog, SubagentEntryOrigin,
};
use arc_tui::{
    pick_subagent, run_capability_uninstall_wizard, run_subagent_browser,
    run_subagent_install_wizard, UninstallEntry,
};
use console::style;

use crate::cli::{
    OutputFormat, SubagentCommand, SubagentInfoArgs, SubagentInstallArgs, SubagentUninstallArgs,
};
use crate::format::{
    print_json, ErrorOutput, SubagentInfoOutput, SubagentItem, SubagentListOutput, WriteResult,
    WriteResultItem, SCHEMA_VERSION,
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

    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        render_subagent_list(&subagents);
        return Ok(());
    }

    run_subagent_browser(&subagents, render_subagent_detail)
        .map_err(|err| ArcError::new(format!("interactive browse failed: {err}")))
}

fn info(paths: &ArcPaths, args: SubagentInfoArgs, fmt: &OutputFormat) -> Result<(), ArcError> {
    let name = match args.name {
        Some(name) => name,
        None => {
            let is_tty = io::stdin().is_terminal() && io::stdout().is_terminal();
            if *fmt == OutputFormat::Json || !is_tty {
                return Err(ArcError::with_hint(
                    "Subagent name required in non-interactive mode.".to_string(),
                    "Usage: arc subagent info <name>".to_string(),
                ));
            }
            let catalog = load_merged_subagent_catalog(paths)?;
            if catalog.is_empty() {
                println!("  {}", style("No global subagents available.").yellow());
                return Ok(());
            }
            let Some(name) = pick_subagent(&catalog)
                .map_err(|err| ArcError::new(format!("interactive info failed: {err}")))?
            else {
                return Ok(());
            };
            name
        }
    };
    let Some(subagent) = find_global_subagent(paths, &name)? else {
        if *fmt == OutputFormat::Json {
            return print_json(&ErrorOutput {
                schema_version: SCHEMA_VERSION,
                ok: false,
                error: format!("subagent '{name}' not found"),
            });
        }
        return Err(ArcError::new(format!("subagent '{name}' not found")));
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
            prompt: subagent.prompt_body,
        });
    }

    render_subagent_detail(&subagent);
    Ok(())
}

fn render_subagent_list(entries: &[arc_core::subagent_registry::SubagentCatalogEntry]) {
    for entry in entries {
        println!(
            "  {} {}",
            style(&entry.definition.name).bold(),
            style(format!("[{}]", origin_label(&entry.origin))).dim()
        );
        if let Some(description) = &entry.definition.description {
            println!("      {}", style(description).dim());
        }
        if let Some(targets) = targets_label(entry.definition.targets.as_ref()) {
            println!("      {}", style(targets).dim());
        }
    }
}

fn render_subagent_detail(entry: &arc_core::subagent_registry::SubagentCatalogEntry) {
    println!(
        "  {} {}",
        style(&entry.definition.name).bold(),
        style(format!("[{}]", origin_label(&entry.origin))).dim()
    );
    if let Some(description) = &entry.definition.description {
        println!("  description: {}", description);
    }
    println!("  prompt_file: {}", &entry.definition.prompt_file);
    println!(
        "  targets: {}",
        targets_label(entry.definition.targets.as_ref())
            .unwrap_or_else(|| "all detected agents".to_string())
    );
    println!("  prompt:");
    for line in entry.prompt_body.lines() {
        println!("    {line}");
    }
}

fn origin_label(origin: &SubagentEntryOrigin) -> &'static str {
    match origin {
        SubagentEntryOrigin::Builtin => "built-in",
        SubagentEntryOrigin::User => "user",
    }
}

fn targets_label(targets: Option<&Vec<String>>) -> Option<String> {
    targets.map(|targets| targets.join(", "))
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
            prompt_path: Some(prompt_path),
            prompt_body: Some(prompt_body.clone()),
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
    let Some(name) = args.name else {
        let is_tty = io::stdin().is_terminal() && io::stdout().is_terminal();
        if *fmt == OutputFormat::Json || !is_tty {
            return Err(ArcError::with_hint(
                "Subagent name required in non-interactive mode.".to_string(),
                "Usage: arc subagent uninstall <name>".to_string(),
            ));
        }
        let catalog = load_merged_subagent_catalog(paths)?;
        let tracked = list_tracked_capability_installs(paths);
        let installed = build_subagent_uninstall_entries(&catalog, &tracked);
        if installed.is_empty() {
            println!("  {}", style("No global subagents installed.").yellow());
            return Ok(());
        }
        let Some(name) = run_capability_uninstall_wizard(&installed)
            .map_err(|err| ArcError::new(format!("interactive uninstall failed: {err}")))?
        else {
            return Ok(());
        };
        return uninstall_by_name(paths, &name, fmt);
    };

    uninstall_by_name(paths, &name, fmt)
}

fn uninstall_by_name(paths: &ArcPaths, name: &str, fmt: &OutputFormat) -> Result<(), ArcError> {
    remove_global_subagent(paths, name)?;
    let tracked = list_tracked_capability_installs(paths);
    let mut removed = Vec::new();
    for record in tracked.into_iter().filter(|record| {
        record.kind == ResourceKind::SubAgent
            && record.name == name
            && record.source_scope == SourceScope::Global
    }) {
        remove_tracked_capability(paths, &record, None)?;
        removed.push(record);
    }

    if *fmt == OutputFormat::Json {
        return print_json(&WriteResult {
            schema_version: SCHEMA_VERSION,
            ok: true,
            message: format!("Subagent '{name}' removed."),
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
                        AppliedResourceScope::Global => arc_core::capability::AppliedScope::Global,
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

fn build_subagent_uninstall_entries(
    catalog: &[arc_core::subagent_registry::SubagentCatalogEntry],
    tracked: &[TrackedCapabilityInstall],
) -> Vec<UninstallEntry> {
    let tracked_targets = global_tracked_targets_by_name(tracked, ResourceKind::SubAgent);

    catalog
        .iter()
        .filter(|entry| {
            entry.origin == SubagentEntryOrigin::User
                || tracked_targets.contains_key(&entry.definition.name)
        })
        .map(|entry| UninstallEntry {
            name: entry.definition.name.clone(),
            origin: match entry.origin {
                SubagentEntryOrigin::Builtin => "built-in".to_string(),
                SubagentEntryOrigin::User => "user".to_string(),
            },
            installed_targets: tracked_targets
                .get(&entry.definition.name)
                .cloned()
                .unwrap_or_default(),
        })
        .collect()
}

fn global_tracked_targets_by_name(
    tracked: &[TrackedCapabilityInstall],
    kind: ResourceKind,
) -> BTreeMap<String, Vec<String>> {
    let mut by_name: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();

    for record in tracked
        .iter()
        .filter(|record| record.kind == kind && record.source_scope == SourceScope::Global)
    {
        by_name
            .entry(record.name.clone())
            .or_default()
            .insert(record.agent.clone());
    }

    by_name
        .into_iter()
        .map(|(name, agents)| (name, agents.into_iter().collect()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn subagent_entry(
        name: &str,
        origin: SubagentEntryOrigin,
    ) -> arc_core::subagent_registry::SubagentCatalogEntry {
        arc_core::subagent_registry::SubagentCatalogEntry {
            definition: SubagentDefinition {
                name: name.to_string(),
                description: None,
                targets: None,
                prompt_file: format!("/tmp/{name}.md"),
            },
            origin,
            prompt_body: "# prompt".to_string(),
        }
    }

    fn tracked_record(
        name: &str,
        agent: &str,
        source_scope: SourceScope,
    ) -> TrackedCapabilityInstall {
        TrackedCapabilityInstall {
            kind: ResourceKind::SubAgent,
            name: name.to_string(),
            agent: agent.to_string(),
            source_scope,
            applied_scope: AppliedResourceScope::Global,
            project_root: None,
        }
    }

    #[test]
    fn build_subagent_uninstall_entries_keeps_user_and_globally_installed_items() {
        let catalog = vec![
            subagent_entry("arc-backend", SubagentEntryOrigin::Builtin),
            subagent_entry("reviewer", SubagentEntryOrigin::User),
            subagent_entry("arc-db", SubagentEntryOrigin::Builtin),
        ];
        let tracked = vec![
            tracked_record("arc-backend", "codex", SourceScope::Global),
            tracked_record("arc-backend", "claude", SourceScope::Global),
            tracked_record("arc-db", "codex", SourceScope::Project),
        ];

        let entries = build_subagent_uninstall_entries(&catalog, &tracked);

        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].name, "arc-backend");
        assert_eq!(entries[0].origin, "built-in");
        assert_eq!(entries[0].installed_targets, vec!["claude", "codex"]);
        assert_eq!(entries[1].name, "reviewer");
        assert_eq!(entries[1].origin, "user");
        assert!(entries[1].installed_targets.is_empty());
    }

    #[test]
    fn global_tracked_targets_by_name_dedupes_agents() {
        let tracked = vec![
            tracked_record("reviewer", "codex", SourceScope::Global),
            tracked_record("reviewer", "codex", SourceScope::Global),
            tracked_record("reviewer", "claude", SourceScope::Global),
        ];

        let grouped = global_tracked_targets_by_name(&tracked, ResourceKind::SubAgent);

        assert_eq!(
            grouped.get("reviewer").cloned().unwrap_or_default(),
            vec!["claude".to_string(), "codex".to_string()]
        );
    }
}
