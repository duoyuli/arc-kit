use std::collections::BTreeMap;

use arc_core::ArcPaths;
use arc_core::agent::AppliedResourceScope;
use arc_core::capability::{
    CapabilityTargetState, McpApplyPlan, McpDefinition, McpTransportType, SourceScope,
    apply_mcp_plan, list_tracked_capability_installs, load_global_mcps, remove_global_mcp,
    remove_tracked_capability, save_global_mcp,
};
use arc_core::detect::DetectCache;
use arc_core::error::ArcError;
use arc_core::models::ResourceKind;
use console::style;

use crate::cli::{
    McpCommand, McpInfoArgs, McpInstallArgs, McpTransportArg, McpUninstallArgs, OutputFormat,
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
        McpCommand::Uninstall(args) => uninstall(paths, args, fmt),
    }
}

fn list(paths: &ArcPaths, fmt: &OutputFormat) -> Result<(), ArcError> {
    let mcps = load_global_mcps(paths)?;
    if *fmt == OutputFormat::Json {
        let items = mcps
            .into_iter()
            .map(|item| McpItem {
                name: item.name,
                transport: transport_label(item.transport).to_string(),
                description: item.description,
                targets: item.targets,
            })
            .collect();
        return print_json(&McpListOutput {
            schema_version: SCHEMA_VERSION,
            mcps: items,
        });
    }

    if mcps.is_empty() {
        println!("  {}", style("No global MCPs installed.").yellow());
        return Ok(());
    }

    for mcp in mcps {
        let targets = mcp
            .targets
            .map(|targets| targets.join(", "))
            .unwrap_or_else(|| "all detected agents".to_string());
        println!(
            "  {}  {}  {}",
            style(&mcp.name).bold(),
            style(transport_label(mcp.transport)).cyan(),
            style(targets).dim()
        );
        if let Some(description) = mcp.description {
            println!("      {}", style(description).dim());
        }
    }
    Ok(())
}

fn info(paths: &ArcPaths, args: McpInfoArgs, fmt: &OutputFormat) -> Result<(), ArcError> {
    let mcps = load_global_mcps(paths)?;
    let Some(mcp) = mcps.into_iter().find(|item| item.name == args.name) else {
        if *fmt == OutputFormat::Json {
            return print_json(&ErrorOutput {
                schema_version: SCHEMA_VERSION,
                ok: false,
                error: format!("mcp '{}' not found", args.name),
            });
        }
        return Err(ArcError::new(format!("mcp '{}' not found", args.name)));
    };

    if *fmt == OutputFormat::Json {
        return print_json(&McpInfoOutput {
            schema_version: SCHEMA_VERSION,
            name: mcp.name,
            transport: transport_label(mcp.transport).to_string(),
            command: mcp.command,
            args: mcp.args,
            url: mcp.url,
            env: mcp.env,
            headers: mcp.headers,
            description: mcp.description,
            targets: mcp.targets,
        });
    }

    println!("  {}", style(&mcp.name).bold());
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
    if !mcp.env.is_empty() {
        println!("  env: {:?}", mcp.env);
    }
    if !mcp.headers.is_empty() {
        println!("  headers: {:?}", mcp.headers);
    }
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

fn install(
    paths: &ArcPaths,
    cache: &DetectCache,
    args: McpInstallArgs,
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
        url: args.url,
        headers: parse_kv_pairs(args.header)?,
        description: args.description,
        scope_fallback: None,
    };

    save_global_mcp(paths, &definition)?;
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
            message: format!("MCP '{}' installed.", definition.name),
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
    remove_global_mcp(paths, &args.name)?;
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
            message: format!("MCP '{}' removed.", args.name),
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
