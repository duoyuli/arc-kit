use std::collections::HashMap;
use std::io::{self, IsTerminal};

use arc_core::ArcPaths;
use arc_core::detect::DetectCache;
use arc_core::error::ArcError;
use arc_core::provider::test::test_provider;
use arc_core::provider::{
    ProviderInfo, apply_provider, build_provider_list_lines, load_providers_for_agent,
    read_active_provider, supported_provider_agents, supports_provider_agent,
};
use arc_tui::select_provider;
use console::{Alignment, pad_str, style};

use crate::cli::{OutputFormat, ProviderCommand};
use crate::display::agent_display_name;
use crate::format::{
    ProviderItem, ProviderListOutput, ProviderTestItem, ProviderTestOutput, SCHEMA_VERSION,
    WriteResult, print_json,
};

pub fn run(
    paths: &ArcPaths,
    cache: &DetectCache,
    command: Option<ProviderCommand>,
    fmt: &OutputFormat,
) -> Result<(), ArcError> {
    arc_core::seed_default_providers(paths, cache);
    match command {
        Some(ProviderCommand::List) | None => list(paths, fmt),
        Some(ProviderCommand::Use { name, agent }) => {
            use_provider(paths, name.as_deref(), agent.as_deref(), fmt)
        }
        Some(ProviderCommand::Test { name, agent }) => {
            test(paths, name.as_deref(), agent.as_deref(), fmt)
        }
    }
}

fn list(paths: &ArcPaths, fmt: &OutputFormat) -> Result<(), ArcError> {
    let providers_dir = paths.providers_dir();

    if *fmt == OutputFormat::Json {
        let mut items: Vec<ProviderItem> = Vec::new();
        for agent in supported_provider_agents() {
            let providers = load_providers_for_agent(&providers_dir, agent);
            let active = read_active_provider(&providers_dir, agent);
            let agent_name = agent_display_name(agent).to_string();
            for provider in &providers {
                items.push(ProviderItem {
                    agent: agent.to_string(),
                    agent_name: agent_name.clone(),
                    name: provider.name.clone(),
                    display_name: provider.display_name.clone(),
                    description: provider.description.clone(),
                    active: active.as_deref() == Some(provider.name.as_str()),
                });
            }
        }
        print_json(&ProviderListOutput {
            schema_version: SCHEMA_VERSION,
            providers: items,
        })?;
        return Ok(());
    }

    if !providers_dir.exists() {
        println!("  {}", style("No providers configured.").yellow());
        return Ok(());
    }

    let mut all_providers: Vec<ProviderInfo> = Vec::new();
    let mut active_providers: HashMap<String, String> = HashMap::new();
    for agent in supported_provider_agents() {
        let providers = load_providers_for_agent(&providers_dir, agent);
        if let Some(active) = read_active_provider(&providers_dir, agent) {
            active_providers.insert(agent.to_string(), active);
        }
        all_providers.extend(providers);
    }

    let lines = build_provider_list_lines(&all_providers, &active_providers);
    let has_any = !lines.is_empty();

    print_provider_list_lines_stdout(&lines);
    if has_any && io::stdout().is_terminal() {
        println!(
            "  {}",
            style("Run arc provider use to switch active provider.").dim()
        );
    }
    if !has_any {
        println!("  {}", style("No providers configured.").yellow());
    }
    Ok(())
}

fn print_provider_list_lines_stdout(lines: &[arc_core::provider::ProviderListLine]) {
    for line in lines {
        match line {
            arc_core::provider::ProviderListLine::AgentHeader { agent_display } => {
                println!();
                println!("  {}", style(agent_display).bold());
            }
            arc_core::provider::ProviderListLine::ProviderRow {
                is_active,
                display_name,
                description,
                name_width,
                ..
            } => {
                let marker = if *is_active { "✓" } else { " " };
                if description.is_empty() {
                    println!("    {marker} {}", display_name);
                } else {
                    let padded = pad_str(display_name, *name_width, Alignment::Left, None);
                    println!("    {marker} {padded}  {}", style(description).dim());
                }
            }
        }
    }
    println!();
}

fn use_provider(
    paths: &ArcPaths,
    name: Option<&str>,
    agent: Option<&str>,
    fmt: &OutputFormat,
) -> Result<(), ArcError> {
    let providers_dir = paths.providers_dir();

    let provider = match name {
        Some(name) => match agent {
            Some(agent) => {
                if !supports_provider_agent(agent) {
                    return Err(ArcError::new(format!("Unsupported agent '{agent}'.")));
                }
                let providers = load_providers_for_agent(&providers_dir, agent);
                providers
                    .into_iter()
                    .find(|p| p.name == name)
                    .ok_or_else(|| {
                        ArcError::new(format!("Provider '{name}' not found for agent '{agent}'."))
                    })?
            }
            None => resolve_provider_by_name(&providers_dir, name)?,
        },
        None => {
            let is_tty = io::stdin().is_terminal() && io::stdout().is_terminal();
            if *fmt == OutputFormat::Json || !is_tty {
                return Err(ArcError::with_hint(
                    "Provider name required in non-interactive mode.".to_string(),
                    "Usage: arc provider use <name> [--agent <agent>]".to_string(),
                ));
            }
            interactive_select(paths)?
        }
    };

    apply_provider(paths, &provider)?;

    if *fmt == OutputFormat::Json {
        print_json(&WriteResult {
            schema_version: SCHEMA_VERSION,
            ok: true,
            message: format!(
                "Switched {} to {}.",
                agent_display_name(&provider.agent),
                provider.display_name
            ),
            items: Vec::new(),
        })?;
        return Ok(());
    }
    show_switch_result(&provider);
    Ok(())
}

fn interactive_select(paths: &ArcPaths) -> Result<ProviderInfo, ArcError> {
    let providers_dir = paths.providers_dir();
    let mut all_providers: Vec<ProviderInfo> = Vec::new();
    let mut active_providers: HashMap<String, String> = HashMap::new();
    for agent in supported_provider_agents() {
        let providers = load_providers_for_agent(&providers_dir, agent);
        if let Some(active) = read_active_provider(&providers_dir, agent) {
            active_providers.insert(agent.to_string(), active);
        }
        all_providers.extend(providers);
    }
    if all_providers.is_empty() {
        return Err(ArcError::new("No providers configured.".to_string()));
    }
    select_provider(&all_providers, &active_providers)
        .map_err(|err| ArcError::new(format!("interactive selection failed: {err}")))?
        .ok_or_else(|| ArcError::new("No provider selected.".to_string()))
}

fn resolve_provider_by_name(
    providers_dir: &std::path::Path,
    name: &str,
) -> Result<ProviderInfo, ArcError> {
    let mut found: Vec<ProviderInfo> = Vec::new();
    for agent in supported_provider_agents() {
        let providers = load_providers_for_agent(providers_dir, agent);
        if let Some(p) = providers.into_iter().find(|p| p.name == name) {
            found.push(p);
        }
    }
    match found.len() {
        0 => Err(ArcError::new(format!("Provider '{name}' not found."))),
        1 => Ok(found.into_iter().next().unwrap()),
        _ => {
            let agents: Vec<&str> = found.iter().map(|p| p.agent.as_str()).collect();
            Err(ArcError::new(format!(
                "Provider '{name}' exists in multiple agents: {}. Use --agent to specify.",
                agents.join(", ")
            )))
        }
    }
}

fn test(
    paths: &ArcPaths,
    name: Option<&str>,
    agent: Option<&str>,
    fmt: &OutputFormat,
) -> Result<(), ArcError> {
    let providers_dir = paths.providers_dir();

    // Collect providers to test.
    let providers_to_test: Vec<ProviderInfo> = match (name, agent) {
        (Some(name), Some(agent)) => {
            let providers = load_providers_for_agent(&providers_dir, agent);
            match providers.into_iter().find(|p| p.name == name) {
                Some(p) => vec![p],
                None => {
                    return Err(ArcError::new(format!(
                        "Provider '{name}' not found for agent '{agent}'."
                    )));
                }
            }
        }
        (Some(name), None) => vec![resolve_provider_by_name(&providers_dir, name)?],
        (None, Some(agent)) => {
            let active = read_active_provider(&providers_dir, agent);
            match active {
                Some(name) => {
                    let providers = load_providers_for_agent(&providers_dir, agent);
                    match providers.into_iter().find(|p| p.name == name) {
                        Some(p) => vec![p],
                        None => {
                            return Err(ArcError::new("Active provider not found.".to_string()));
                        }
                    }
                }
                None => {
                    return Err(ArcError::new(format!(
                        "No active provider for agent '{agent}'."
                    )));
                }
            }
        }
        (None, None) => {
            // Test all active providers.
            let mut result = Vec::new();
            for agent in supported_provider_agents() {
                if let Some(active_name) = read_active_provider(&providers_dir, agent) {
                    let providers = load_providers_for_agent(&providers_dir, agent);
                    if let Some(p) = providers.into_iter().find(|p| p.name == active_name) {
                        result.push(p);
                    }
                }
            }
            result
        }
    };

    if providers_to_test.is_empty() {
        if *fmt == OutputFormat::Json {
            print_json(&ProviderTestOutput {
                schema_version: SCHEMA_VERSION,
                results: Vec::new(),
            })?;
            return Ok(());
        }
        println!("  {}", style("No active providers to test.").yellow());
        return Ok(());
    }

    let results: Vec<_> = providers_to_test.iter().map(test_provider).collect();

    if *fmt == OutputFormat::Json {
        let items: Vec<ProviderTestItem> = results
            .iter()
            .map(|r| ProviderTestItem {
                provider: r.provider_name.clone(),
                agent: r.agent.clone(),
                display_name: r.display_name.clone(),
                ok: r.ok,
                latency_ms: r.latency_ms,
                message: r.message.clone(),
            })
            .collect();
        print_json(&ProviderTestOutput {
            schema_version: SCHEMA_VERSION,
            results: items,
        })?;
        if results.iter().any(|r| !r.ok) {
            return Err(ArcError::new(
                "One or more provider connectivity tests failed.".to_string(),
            ));
        }
        return Ok(());
    }

    println!();
    for r in &results {
        let agent_name = agent_display_name(&r.agent);
        if r.ok {
            println!(
                "  {} {} › {}     — {}",
                style("✓").green(),
                agent_name,
                r.display_name,
                style(&r.message).dim()
            );
        } else {
            println!(
                "  {} {} › {}     — {}",
                style("✗").red(),
                agent_name,
                r.display_name,
                style(&r.message).dim()
            );
        }
    }
    println!();

    if results.iter().any(|r| !r.ok) {
        return Err(ArcError::new(
            "One or more provider connectivity tests failed.".to_string(),
        ));
    }

    Ok(())
}

fn show_switch_result(provider: &ProviderInfo) {
    println!(
        "  {} {} → {}",
        style("✓").green(),
        agent_display_name(&provider.agent),
        provider.display_name
    );
}
