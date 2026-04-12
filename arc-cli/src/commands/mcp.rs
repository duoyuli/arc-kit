use std::collections::BTreeMap;
use std::io::{self, IsTerminal};

use arc_core::ArcPaths;
use arc_core::agent::AppliedResourceScope;
use arc_core::capability::{
    CapabilityTargetState, McpApplyPlan, McpDefinition, McpOAuthConfig, McpOAuthSettings,
    McpTransportType, SourceScope, apply_mcp_plan, list_tracked_capability_installs,
    remove_tracked_capability, save_global_mcp,
};
use arc_core::detect::DetectCache;
use arc_core::error::ArcError;
use arc_core::mcp_registry::{self, McpEntryOrigin};
use arc_core::models::{CatalogResource, ResourceKind};
use arc_tui::run_install_wizard;
use console::style;

use crate::cli::{
    McpCommand, McpDefineArgs, McpInfoArgs, McpInstallArgs, McpTransportArg, McpUninstallArgs,
    OutputFormat,
};
use crate::format::{
    ErrorOutput, McpInfoOutput, McpItem, McpListOutput, SCHEMA_VERSION, WriteResult,
    WriteResultItem, print_json,
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

    for entry in sorted {
        let mcp = entry.definition;
        let origin = match entry.origin {
            McpEntryOrigin::Builtin => style("builtin").magenta(),
            McpEntryOrigin::User => style("user").green(),
        };
        let targets = mcp
            .targets
            .as_ref()
            .map(|targets| targets.join(", "))
            .unwrap_or_else(|| "all detected agents".to_string());
        println!(
            "  {}  {}  {}  {}",
            style(&mcp.name).bold(),
            origin,
            style(transport_label(mcp.transport)).cyan(),
            style(targets).dim()
        );
    }
    Ok(())
}

fn info(paths: &ArcPaths, args: McpInfoArgs, fmt: &OutputFormat) -> Result<(), ArcError> {
    let catalog = mcp_registry::load_merged_mcp_catalog(paths)?;
    let Some(entry) = catalog.into_iter().find(|e| e.definition.name == args.name) else {
        if *fmt == OutputFormat::Json {
            return print_json(&ErrorOutput {
                schema_version: SCHEMA_VERSION,
                ok: false,
                error: format!("mcp '{}' not found", args.name),
            });
        }
        return Err(ArcError::new(format!("mcp '{}' not found", args.name)));
    };
    let mcp = entry.definition;
    let redact = !args.show_secrets;

    if *fmt == OutputFormat::Json {
        let (env, headers) = if redact {
            (redact_map(&mcp.env), redact_map(&mcp.headers))
        } else {
            (mcp.env.clone(), mcp.headers.clone())
        };
        return print_json(&McpInfoOutput {
            schema_version: SCHEMA_VERSION,
            name: mcp.name,
            transport: transport_label(mcp.transport).to_string(),
            command: mcp.command,
            args: mcp.args,
            url: mcp.url,
            env,
            headers,
            description: mcp.description,
            targets: mcp.targets,
        });
    }

    println!("  {}", style(&mcp.name).bold());
    println!(
        "  origin: {}",
        match entry.origin {
            McpEntryOrigin::Builtin => "builtin",
            McpEntryOrigin::User => "user",
        }
    );
    println!("  transport: {}", transport_label(mcp.transport));
    if let Some(command) = mcp.command {
        println!("  command: {}", command);
    }
    if !mcp.args.is_empty() {
        println!("  args: {}", mcp.args.join(" "));
    }
    if let Some(url) = mcp.url {
        println!("  url: {}", url);
    }
    print_redacted_map("env", &mcp.env, redact);
    print_redacted_map("headers", &mcp.headers, redact);
    if let Some(description) = mcp.description {
        println!("  description: {}", description);
    }
    println!(
        "  targets: {}",
        mcp.targets
            .map(|targets| targets.join(", "))
            .unwrap_or_else(|| "all detected agents".to_string())
    );
    Ok(())
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
        scope_fallback: None,
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
        scope_fallback: None,
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
        false,
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
    let catalog = mcp_registry::load_merged_mcp_catalog(paths)?;
    if !catalog.iter().any(|e| e.definition.name == args.name) {
        if *fmt == OutputFormat::Json {
            return print_json(&ErrorOutput {
                schema_version: SCHEMA_VERSION,
                ok: false,
                error: format!("mcp '{}' not found", args.name),
            });
        }
        return Err(ArcError::new(format!("mcp '{}' not found", args.name)));
    }

    let had_user_entry = mcp_registry::remove_user_registry_mcp(paths, &args.name)?;

    let tracked = list_tracked_capability_installs(paths);
    let mut removed = Vec::new();
    for record in tracked.into_iter().filter(|record| {
        record.kind == ResourceKind::Mcp
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
            message: format!(
                "MCP '{}' removed from registry and agent configs.",
                args.name
            ),
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
                        AppliedResourceScope::Global | AppliedResourceScope::GlobalFallback => {
                            arc_core::capability::AppliedScope::Global
                        }
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
