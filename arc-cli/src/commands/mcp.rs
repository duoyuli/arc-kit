use std::collections::{BTreeMap, BTreeSet};
use std::io::{self, IsTerminal};

use arc_core::agent::AppliedResourceScope;
use arc_core::capability::{
    apply_mcp_plan, list_tracked_capability_installs, remove_tracked_capability, save_global_mcp,
    CapabilityTargetState, McpApplyPlan, McpDefinition, McpOAuthConfig, McpOAuthSettings,
    McpTransportType, SourceScope, TrackedCapabilityInstall,
};
use arc_core::detect::DetectCache;
use arc_core::error::ArcError;
use arc_core::mcp_registry::{self, McpEntryOrigin};
use arc_core::models::{CatalogResource, ResourceKind};
use arc_core::paths::ArcPaths;
use arc_tui::{
    pick_mcp, run_capability_uninstall_wizard, run_install_wizard, run_mcp_browser, UninstallEntry,
};
use console::style;

use crate::cli::{
    McpCommand, McpDefineArgs, McpInfoArgs, McpInstallArgs, McpTransportArg, McpUninstallArgs,
    OutputFormat,
};
use crate::format::{
    print_json, ErrorOutput, McpInfoOutput, McpItem, McpListOutput, WriteResult, WriteResultItem,
    SCHEMA_VERSION,
};

pub fn run(
    paths: &ArcPaths,
    cache: &DetectCache,
    command: McpCommand,
    fmt: &OutputFormat,
) -> Result<(), ArcError> {
    match command {
        McpCommand::List => list(paths, fmt),
        McpCommand::Info(args) => info(paths, args, fmt),
        McpCommand::Install(args) => install(paths, cache, args, fmt),
        McpCommand::Define(args) => define(paths, cache, args, fmt),
        McpCommand::Uninstall(args) => uninstall(paths, args, fmt),
    }
}

fn list(paths: &ArcPaths, fmt: &OutputFormat) -> Result<(), ArcError> {
    let catalog = mcp_registry::load_merged_mcp_catalog(paths)?;
    if *fmt == OutputFormat::Json {
        let mut items: Vec<McpItem> = catalog
            .into_iter()
            .map(|entry| McpItem {
                name: entry.definition.name,
                origin: match entry.origin {
                    McpEntryOrigin::Builtin => "builtin".to_string(),
                    McpEntryOrigin::User => "user".to_string(),
                },
                transport: transport_label(entry.definition.transport).to_string(),
                description: entry.definition.description,
                targets: entry.definition.targets,
            })
            .collect();
        items.sort_by(|a, b| a.name.cmp(&b.name));
        return print_json(&McpListOutput {
            schema_version: SCHEMA_VERSION,
            mcps: items,
        });
    }

    if catalog.is_empty() {
        println!("  {}", style("No MCP entries.").yellow());
        return Ok(());
    }

    let mut sorted = catalog;
    sorted.sort_by(|a, b| a.definition.name.cmp(&b.definition.name));

    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        render_mcp_list(&sorted);
        return Ok(());
    }

    run_mcp_browser(&sorted, |entry| {
        render_mcp_detail(entry, true);
    })
    .map_err(|err| ArcError::new(format!("interactive browse failed: {err}")))
}

fn info(paths: &ArcPaths, args: McpInfoArgs, fmt: &OutputFormat) -> Result<(), ArcError> {
    let catalog = mcp_registry::load_merged_mcp_catalog(paths)?;
    let name = match args.name {
        Some(name) => name,
        None => {
            let is_tty = io::stdin().is_terminal() && io::stdout().is_terminal();
            if *fmt == OutputFormat::Json || !is_tty {
                return Err(ArcError::with_hint(
                    "MCP name required in non-interactive mode.".to_string(),
                    "Usage: arc mcp info <name> [--show-secrets]".to_string(),
                ));
            }
            if catalog.is_empty() {
                println!("  {}", style("No MCP entries.").yellow());
                return Ok(());
            }
            let Some(name) = pick_mcp(&catalog)
                .map_err(|err| ArcError::new(format!("interactive info failed: {err}")))?
            else {
                return Ok(());
            };
            name
        }
    };
    let Some(entry) = catalog.into_iter().find(|e| e.definition.name == name) else {
        if *fmt == OutputFormat::Json {
            return print_json(&ErrorOutput {
                schema_version: SCHEMA_VERSION,
                ok: false,
                error: format!("mcp '{name}' not found"),
            });
        }
        return Err(ArcError::new(format!("mcp '{name}' not found")));
    };
    let redact = !args.show_secrets;

    if *fmt == OutputFormat::Json {
        let (env, headers) = if redact {
            (
                redact_map(&entry.definition.env),
                redact_map(&entry.definition.headers),
            )
        } else {
            (
                entry.definition.env.clone(),
                entry.definition.headers.clone(),
            )
        };
        return print_json(&McpInfoOutput {
            schema_version: SCHEMA_VERSION,
            name: entry.definition.name,
            transport: transport_label(entry.definition.transport).to_string(),
            command: entry.definition.command,
            args: entry.definition.args,
            url: entry.definition.url,
            env,
            headers,
            description: entry.definition.description,
            targets: entry.definition.targets,
        });
    }

    render_mcp_detail(&entry, redact);
    Ok(())
}

fn render_mcp_list(entries: &[mcp_registry::McpCatalogEntry]) {
    let name_width = entries
        .iter()
        .map(|entry| entry.definition.name.len())
        .max()
        .unwrap_or(0);
    let origin_width = entries
        .iter()
        .map(|entry| format!("[{}]", origin_label(&entry.origin)).len())
        .max()
        .unwrap_or(0);

    for entry in entries {
        let origin_label = format!("[{}]", origin_label(&entry.origin));
        let origin = match entry.origin {
            McpEntryOrigin::Builtin => style(format!("{origin_label:<origin_width$}")).magenta(),
            McpEntryOrigin::User => style(format!("{origin_label:<origin_width$}")).green(),
        };
        let base = format!("  {:<name_width$}  {}", entry.definition.name, origin);
        if let Some(targets) = targets_label(entry.definition.targets.as_ref()) {
            println!("{base}  {}", style(targets).dim());
        } else {
            println!("{base}");
        }
    }
}

fn render_mcp_detail(entry: &mcp_registry::McpCatalogEntry, redact: bool) {
    let mcp = &entry.definition;
    println!("  {}", style(&mcp.name).bold());
    println!("  origin: {}", origin_label(&entry.origin));
    println!("  transport: {}", transport_label(mcp.transport));
    if let Some(command) = &mcp.command {
        println!("  command: {}", command);
    }
    if !mcp.args.is_empty() {
        println!("  args: {}", mcp.args.join(" "));
    }
    if let Some(url) = &mcp.url {
        println!("  url: {}", url);
    }
    print_redacted_map("env", &mcp.env, redact);
    print_redacted_map("headers", &mcp.headers, redact);
    if let Some(description) = &mcp.description {
        println!("  description: {}", description);
    }
    println!(
        "  targets: {}",
        targets_label(mcp.targets.as_ref()).unwrap_or_else(|| "all detected agents".to_string())
    );
}

fn redact_map(m: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    m.keys()
        .map(|k| (k.clone(), "<redacted>".to_string()))
        .collect()
}

fn print_redacted_map(label: &str, m: &BTreeMap<String, String>, redact: bool) {
    if m.is_empty() {
        return;
    }
    if redact {
        let keys: Vec<_> = m.keys().map(String::as_str).collect();
        println!("  {}: [redacted] keys={:?}", label, keys);
    } else {
        println!("  {}: {:?}", label, m);
    }
}

fn origin_label(origin: &McpEntryOrigin) -> &'static str {
    match origin {
        McpEntryOrigin::Builtin => "builtin",
        McpEntryOrigin::User => "user",
    }
}

fn targets_label(targets: Option<&Vec<String>>) -> Option<String> {
    targets.map(|targets| targets.join(", "))
}

fn install(
    paths: &ArcPaths,
    cache: &DetectCache,
    args: McpInstallArgs,
    fmt: &OutputFormat,
) -> Result<(), ArcError> {
    let catalog = mcp_registry::load_merged_mcp_catalog(paths)?;

    let custom = args.transport.is_some()
        || !args.arg.is_empty()
        || args.command.is_some()
        || args.url.is_some()
        || !args.env.is_empty()
        || !args.header.is_empty()
        || args.cwd.is_some()
        || args.env_file.is_some()
        || args.timeout.is_some()
        || args.startup_timeout_sec.is_some()
        || args.tool_timeout_sec.is_some()
        || args.enabled
        || args.required
        || args.trust
        || !args.include_tool.is_empty()
        || !args.exclude_tool.is_empty()
        || args.oauth_client_id.is_some()
        || args.oauth_client_secret.is_some()
        || args.oauth_scope.is_some()
        || args.oauth_callback_port.is_some()
        || args.oauth_auth_server_metadata_url.is_some()
        || args.oauth_disabled
        || args.description.is_some();

    if args.name.is_none() {
        if custom {
            return Err(ArcError::with_hint(
                "MCP name required when passing custom install options.".to_string(),
                "Usage: arc mcp install <name> [--transport <transport>] [--command <cmd> | --url <url>]".to_string(),
            ));
        }
        let is_tty = io::stdin().is_terminal() && io::stdout().is_terminal();
        if *fmt == OutputFormat::Json || !is_tty {
            return Err(ArcError::with_hint(
                "MCP name required in non-interactive mode.".to_string(),
                "Usage: arc mcp install <name> [--agent <agent>]".to_string(),
            ));
        }
        let resources: Vec<CatalogResource> = catalog
            .iter()
            .map(|entry| CatalogResource {
                id: entry.definition.name.clone(),
                kind: ResourceKind::Mcp,
                name: entry.definition.name.clone(),
                source_id: match entry.origin {
                    McpEntryOrigin::Builtin => "builtin".to_string(),
                    McpEntryOrigin::User => "user".to_string(),
                },
                summary: entry.definition.description.clone().unwrap_or_default(),
                installed: false,
                installed_targets: Vec::new(),
            })
            .collect();
        let agents = cache.agents_for_install(&ResourceKind::Mcp);
        let (selected_names, selected_agents) = run_install_wizard("mcp", &resources, &agents)
            .map_err(|err| ArcError::new(format!("interactive install failed: {err}")))?;
        if selected_names.is_empty() || selected_agents.is_empty() {
            return Ok(());
        }
        for name in selected_names {
            let Some(entry) = catalog.iter().find(|e| e.definition.name == name) else {
                continue;
            };
            let mut definition = entry.definition.clone();
            definition.targets = Some(selected_agents.clone());
            save_global_mcp(paths, &definition)?;
            apply_and_print(paths, cache, definition, fmt, "installed")?;
        }
        return Ok(());
    }

    let definition = if custom {
        let Some(t) = args.transport.clone() else {
            return Err(ArcError::with_hint(
                "When passing --command, --url, --arg, --env, or --header, --transport is required."
                    .to_string(),
                "Example: arc mcp install mysrv --transport stdio --command npx --arg \"-y\" --arg \"@scope/pkg\""
                    .to_string(),
            ));
        };
        mcp_definition_from_install_tail(&args, t)?
    } else {
        let name = args.name.as_deref().expect("checked optional name");
        let Some(entry) = catalog.iter().find(|e| e.definition.name == name) else {
            return Err(ArcError::with_hint(
                format!(
                    "mcp '{}' not found in built-in presets or user registry.",
                    name
                ),
                "Use `arc mcp define` to add a custom server, or check `arc mcp list`.".to_string(),
            ));
        };
        let mut def = entry.definition.clone();
        def.targets = if args.agent.is_empty() {
            None
        } else {
            Some(args.agent)
        };
        def
    };

    save_global_mcp(paths, &definition)?;
    apply_and_print(paths, cache, definition, fmt, "installed")
}

fn define(
    paths: &ArcPaths,
    cache: &DetectCache,
    args: McpDefineArgs,
    fmt: &OutputFormat,
) -> Result<(), ArcError> {
    let definition = McpDefinition {
        name: args.name,
        targets: if args.agent.is_empty() {
            None
        } else {
            Some(args.agent)
        },
        transport: transport_from_arg(args.transport),
        command: args.command,
        args: args.arg,
        env: parse_kv_pairs(args.env)?,
        cwd: args.cwd,
        env_file: args.env_file,
        url: args.url,
        headers: parse_kv_pairs(args.header)?,
        timeout: args.timeout,
        startup_timeout_sec: args.startup_timeout_sec,
        tool_timeout_sec: args.tool_timeout_sec,
        enabled: args.enabled.then_some(true),
        required: args.required.then_some(true),
        trust: args.trust.then_some(true),
        include_tools: args.include_tool,
        exclude_tools: args.exclude_tool,
        oauth: build_oauth_config(
            args.oauth_client_id,
            args.oauth_client_secret,
            args.oauth_scope,
            args.oauth_callback_port,
            args.oauth_auth_server_metadata_url,
            args.oauth_disabled,
        )?,
        description: args.description,
    };

    save_global_mcp(paths, &definition)?;
    apply_and_print(paths, cache, definition, fmt, "saved and applied")
}

fn mcp_definition_from_install_tail(
    args: &McpInstallArgs,
    transport: McpTransportArg,
) -> Result<McpDefinition, ArcError> {
    Ok(McpDefinition {
        name: args.name.clone().expect("checked optional name"),
        targets: if args.agent.is_empty() {
            None
        } else {
            Some(args.agent.clone())
        },
        transport: transport_from_arg(transport),
        command: args.command.clone(),
        args: args.arg.clone(),
        env: parse_kv_pairs(args.env.clone())?,
        cwd: args.cwd.clone(),
        env_file: args.env_file.clone(),
        url: args.url.clone(),
        headers: parse_kv_pairs(args.header.clone())?,
        timeout: args.timeout,
        startup_timeout_sec: args.startup_timeout_sec,
        tool_timeout_sec: args.tool_timeout_sec,
        enabled: args.enabled.then_some(true),
        required: args.required.then_some(true),
        trust: args.trust.then_some(true),
        include_tools: args.include_tool.clone(),
        exclude_tools: args.exclude_tool.clone(),
        oauth: build_oauth_config(
            args.oauth_client_id.clone(),
            args.oauth_client_secret.clone(),
            args.oauth_scope.clone(),
            args.oauth_callback_port,
            args.oauth_auth_server_metadata_url.clone(),
            args.oauth_disabled,
        )?,
        description: args.description.clone(),
    })
}

fn build_oauth_config(
    client_id: Option<String>,
    client_secret: Option<String>,
    scope: Option<String>,
    callback_port: Option<u16>,
    auth_server_metadata_url: Option<String>,
    disabled: bool,
) -> Result<Option<McpOAuthConfig>, ArcError> {
    if disabled
        && (client_id.is_some()
            || client_secret.is_some()
            || scope.is_some()
            || callback_port.is_some()
            || auth_server_metadata_url.is_some())
    {
        return Err(ArcError::new(
            "--oauth-disabled cannot be combined with OAuth settings".to_string(),
        ));
    }
    if disabled {
        return Ok(Some(McpOAuthConfig::Disabled(false)));
    }
    if client_id.is_none()
        && client_secret.is_none()
        && scope.is_none()
        && callback_port.is_none()
        && auth_server_metadata_url.is_none()
    {
        return Ok(None);
    }
    Ok(Some(McpOAuthConfig::Settings(McpOAuthSettings {
        client_id,
        client_secret,
        scope,
        callback_port,
        auth_server_metadata_url,
    })))
}

fn apply_and_print(
    paths: &ArcPaths,
    cache: &DetectCache,
    definition: McpDefinition,
    fmt: &OutputFormat,
    verb: &str,
) -> Result<(), ArcError> {
    let statuses = apply_mcp_plan(
        paths,
        cache,
        &McpApplyPlan {
            definition: definition.clone(),
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
            message: format!("MCP '{}' {}.", definition.name, verb),
            items: statuses
                .into_iter()
                .map(|item| WriteResultItem {
                    resource_kind: Some("mcp".to_string()),
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

fn uninstall(paths: &ArcPaths, args: McpUninstallArgs, fmt: &OutputFormat) -> Result<(), ArcError> {
    let Some(name) = args.name else {
        let is_tty = io::stdin().is_terminal() && io::stdout().is_terminal();
        if *fmt == OutputFormat::Json || !is_tty {
            return Err(ArcError::with_hint(
                "MCP name required in non-interactive mode.".to_string(),
                "Usage: arc mcp uninstall <name>".to_string(),
            ));
        }
        let catalog = mcp_registry::load_merged_mcp_catalog(paths)?;
        let tracked = list_tracked_capability_installs(paths);
        let installed = build_mcp_uninstall_entries(&catalog, &tracked);
        if installed.is_empty() {
            println!("  {}", style("No global MCPs installed.").yellow());
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
    let catalog = mcp_registry::load_merged_mcp_catalog(paths)?;
    if !catalog.iter().any(|entry| entry.definition.name == name) {
        if *fmt == OutputFormat::Json {
            return print_json(&ErrorOutput {
                schema_version: SCHEMA_VERSION,
                ok: false,
                error: format!("mcp '{name}' not found"),
            });
        }
        return Err(ArcError::new(format!("mcp '{name}' not found")));
    }

    let had_user_entry = mcp_registry::remove_user_registry_mcp(paths, name)?;

    let tracked = list_tracked_capability_installs(paths);
    let mut removed = Vec::new();
    for record in tracked.into_iter().filter(|record| {
        record.kind == ResourceKind::Mcp
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
            message: format!("MCP '{name}' removed from registry and agent configs."),
            items: removed
                .into_iter()
                .map(|record| WriteResultItem {
                    resource_kind: Some("mcp".to_string()),
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

    if !had_user_entry && removed.is_empty() {
        println!(
            "  {}",
            style("Nothing to remove (preset was not installed / no user registry entry).").dim()
        );
    } else {
        if had_user_entry {
            println!(
                "  {} {}",
                style("-").green(),
                style("removed from user registry").bold()
            );
        }
        if removed.is_empty() {
            println!(
                "  {}",
                style("No tracked global MCP installs were present.").dim()
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
    }
    Ok(())
}

fn build_mcp_uninstall_entries(
    catalog: &[mcp_registry::McpCatalogEntry],
    tracked: &[TrackedCapabilityInstall],
) -> Vec<UninstallEntry> {
    let tracked_targets = global_tracked_targets_by_name(tracked, ResourceKind::Mcp);

    catalog
        .iter()
        .filter(|entry| {
            entry.origin == McpEntryOrigin::User
                || tracked_targets.contains_key(&entry.definition.name)
        })
        .map(|entry| UninstallEntry {
            name: entry.definition.name.clone(),
            origin: match entry.origin {
                McpEntryOrigin::Builtin => "built-in".to_string(),
                McpEntryOrigin::User => "user".to_string(),
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

fn parse_kv_pairs(items: Vec<String>) -> Result<BTreeMap<String, String>, ArcError> {
    let mut out = BTreeMap::new();
    for item in items {
        let Some((key, value)) = item.split_once('=') else {
            return Err(ArcError::new(format!("invalid KEY=VALUE pair: {item}")));
        };
        out.insert(key.to_string(), value.to_string());
    }
    Ok(out)
}

fn transport_from_arg(arg: McpTransportArg) -> McpTransportType {
    match arg {
        McpTransportArg::Stdio => McpTransportType::Stdio,
        McpTransportArg::Sse => McpTransportType::Sse,
        McpTransportArg::StreamableHttp => McpTransportType::StreamableHttp,
    }
}

fn transport_label(transport: McpTransportType) -> &'static str {
    match transport {
        McpTransportType::Stdio => "stdio",
        McpTransportType::Sse => "sse",
        McpTransportType::StreamableHttp => "streamable_http",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mcp_definition(name: &str) -> McpDefinition {
        McpDefinition {
            name: name.to_string(),
            targets: None,
            transport: McpTransportType::Stdio,
            command: Some("npx".to_string()),
            args: vec!["-y".to_string(), "@demo/server".to_string()],
            env: BTreeMap::new(),
            cwd: None,
            env_file: None,
            url: None,
            headers: BTreeMap::new(),
            timeout: None,
            startup_timeout_sec: None,
            tool_timeout_sec: None,
            enabled: Some(true),
            required: Some(false),
            trust: Some(false),
            include_tools: Vec::new(),
            exclude_tools: Vec::new(),
            oauth: None,
            description: None,
        }
    }

    fn catalog_entry(name: &str, origin: McpEntryOrigin) -> mcp_registry::McpCatalogEntry {
        mcp_registry::McpCatalogEntry {
            definition: mcp_definition(name),
            origin,
        }
    }

    fn tracked_record(
        name: &str,
        agent: &str,
        source_scope: SourceScope,
    ) -> TrackedCapabilityInstall {
        TrackedCapabilityInstall {
            kind: ResourceKind::Mcp,
            name: name.to_string(),
            agent: agent.to_string(),
            source_scope,
            applied_scope: AppliedResourceScope::Global,
            project_root: None,
        }
    }

    #[test]
    fn build_mcp_uninstall_entries_keeps_user_and_globally_installed_items() {
        let catalog = vec![
            catalog_entry("filesystem", McpEntryOrigin::Builtin),
            catalog_entry("drawio", McpEntryOrigin::Builtin),
            catalog_entry("custom", McpEntryOrigin::User),
        ];
        let tracked = vec![
            tracked_record("filesystem", "codex", SourceScope::Global),
            tracked_record("filesystem", "claude", SourceScope::Global),
            tracked_record("drawio", "codex", SourceScope::Project),
        ];

        let entries = build_mcp_uninstall_entries(&catalog, &tracked);

        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].name, "filesystem");
        assert_eq!(entries[0].origin, "built-in");
        assert_eq!(entries[0].installed_targets, vec!["claude", "codex"]);
        assert_eq!(entries[1].name, "custom");
        assert_eq!(entries[1].origin, "user");
        assert!(entries[1].installed_targets.is_empty());
    }

    #[test]
    fn global_tracked_targets_by_name_dedupes_agents() {
        let tracked = vec![
            tracked_record("filesystem", "codex", SourceScope::Global),
            tracked_record("filesystem", "codex", SourceScope::Global),
            tracked_record("filesystem", "claude", SourceScope::Global),
        ];

        let grouped = global_tracked_targets_by_name(&tracked, ResourceKind::Mcp);

        assert_eq!(
            grouped.get("filesystem").cloned().unwrap_or_default(),
            vec!["claude".to_string(), "codex".to_string()]
        );
    }
}
