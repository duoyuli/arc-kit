use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};

mod agent_config;
mod model;
mod storage;
mod validate;

use crate::agent::{
    AppliedResourceScope, McpConfigFormat, McpScopeSupport, McpTransportSupport, SubagentFormat,
    SubagentSupport, agent_mcp_path, agent_spec, agent_subagent_dir,
    ordered_agent_ids_for_resource_kind,
};
use crate::detect::DetectCache;
use crate::error::{ArcError, Result};
use crate::io::{atomic_write_bytes, atomic_write_string};
use crate::mcp_registry;
use crate::models::ResourceKind;
use crate::paths::{ArcPaths, expand_user_path};
use crate::subagent_registry;

#[cfg(test)]
use agent_config::{
    gemini_mcp_value, kimi_mcp_value, render_markdown_subagent, toml_codex_mcp_value,
};
pub use model::*;
pub use storage::{
    capability_install_present, list_tracked_capability_installs, load_global_mcps,
    load_global_subagent_prompt, load_global_subagents, remove_global_mcp, remove_global_subagent,
    remove_tracked_capability, save_global_mcp, save_global_subagent, track_capability_install,
    tracking_record_for_target, untrack_capability_install,
};
pub use validate::{
    is_shadowed, resolve_declared_targets, validate_mcp_definition, validate_subagent_definition,
    validate_subagent_targets,
};

pub fn apply_mcp_plan(
    paths: &ArcPaths,
    cache: &DetectCache,
    plan: &McpApplyPlan,
    project_root: Option<&Path>,
) -> Result<Vec<CapabilityTargetStatus>> {
    let targets = resolve_declared_targets(cache, plan.definition.targets.as_ref());
    let mut statuses = Vec::new();

    for agent in targets {
        let status = evaluate_mcp_target(paths, cache, &agent, plan, project_root, true)?;
        if let Some(tracking) = tracking_record_for_target(
            ResourceKind::Mcp,
            &plan.definition.name,
            plan.source_scope,
            &status,
            project_root,
        ) {
            track_capability_install(paths, &tracking)?;
        }
        statuses.push(status);
    }

    Ok(statuses)
}

pub fn apply_subagent_plan(
    paths: &ArcPaths,
    cache: &DetectCache,
    plan: &SubagentApplyPlan,
    project_root: Option<&Path>,
) -> Result<Vec<CapabilityTargetStatus>> {
    let targets = resolve_declared_targets(cache, plan.definition.targets.as_ref());
    let prompt_body = storage::load_subagent_prompt_body(plan)?;
    let mut statuses = Vec::new();

    for agent in targets {
        let status =
            evaluate_subagent_target(paths, cache, &agent, plan, project_root, &prompt_body, true)?;
        if let Some(tracking) = tracking_record_for_target(
            ResourceKind::SubAgent,
            &plan.definition.name,
            plan.source_scope,
            &status,
            project_root,
        ) {
            track_capability_install(paths, &tracking)?;
        }
        statuses.push(status);
    }

    Ok(statuses)
}

pub fn preview_mcp_plan(
    paths: &ArcPaths,
    cache: &DetectCache,
    plan: &McpApplyPlan,
    project_root: Option<&Path>,
) -> Result<Vec<CapabilityTargetStatus>> {
    let targets = resolve_declared_targets(cache, plan.definition.targets.as_ref());
    targets
        .into_iter()
        .map(|agent| evaluate_mcp_target(paths, cache, &agent, plan, project_root, false))
        .collect()
}

pub fn preview_subagent_plan(
    paths: &ArcPaths,
    cache: &DetectCache,
    plan: &SubagentApplyPlan,
    project_root: Option<&Path>,
) -> Result<Vec<CapabilityTargetStatus>> {
    let targets = resolve_declared_targets(cache, plan.definition.targets.as_ref());
    let prompt_body = storage::load_subagent_prompt_body(plan)?;
    targets
        .into_iter()
        .map(|agent| {
            evaluate_subagent_target(
                paths,
                cache,
                &agent,
                plan,
                project_root,
                &prompt_body,
                false,
            )
        })
        .collect()
}

fn evaluate_mcp_target(
    paths: &ArcPaths,
    cache: &DetectCache,
    agent: &str,
    plan: &McpApplyPlan,
    project_root: Option<&Path>,
    perform_write: bool,
) -> Result<CapabilityTargetStatus> {
    let Some(agent_info) = cache.get_agent(agent) else {
        return Ok(CapabilityTargetStatus {
            agent: agent.to_string(),
            status: CapabilityTargetState::Skipped,
            desired_scope: if plan.source_scope == SourceScope::Project {
                DesiredScope::Project
            } else {
                DesiredScope::Global
            },
            applied_scope: AppliedScope::None,
            reason: Some("agent_not_detected".to_string()),
        });
    };
    if agent_info.root.is_none() {
        return Ok(CapabilityTargetStatus {
            agent: agent.to_string(),
            status: CapabilityTargetState::Skipped,
            desired_scope: if plan.source_scope == SourceScope::Project {
                DesiredScope::Project
            } else {
                DesiredScope::Global
            },
            applied_scope: AppliedScope::None,
            reason: Some("agent_not_detected".to_string()),
        });
    }

    let Some(spec) = agent_spec(agent) else {
        return Ok(CapabilityTargetStatus {
            agent: agent.to_string(),
            status: CapabilityTargetState::Skipped,
            desired_scope: if plan.source_scope == SourceScope::Project {
                DesiredScope::Project
            } else {
                DesiredScope::Global
            },
            applied_scope: AppliedScope::None,
            reason: Some("unsupported_mcp".to_string()),
        });
    };

    if !validate::supports_mcp_transport(spec.mcp_transport_support, plan.definition.transport) {
        return Ok(CapabilityTargetStatus {
            agent: agent.to_string(),
            status: CapabilityTargetState::Skipped,
            desired_scope: if plan.source_scope == SourceScope::Project {
                DesiredScope::Project
            } else {
                DesiredScope::Global
            },
            applied_scope: AppliedScope::None,
            reason: Some("unsupported_transport".to_string()),
        });
    }

    let (desired_scope, applied_scope) = match plan.source_scope {
        SourceScope::Global => (DesiredScope::Global, AppliedResourceScope::Global),
        SourceScope::Project => match spec.mcp_scope_support {
            McpScopeSupport::ProjectNative => {
                (DesiredScope::Project, AppliedResourceScope::Project)
            }
            McpScopeSupport::GlobalOnly => {
                return Ok(CapabilityTargetStatus {
                    agent: agent.to_string(),
                    status: CapabilityTargetState::Skipped,
                    desired_scope: DesiredScope::Project,
                    applied_scope: AppliedScope::None,
                    reason: Some("global_only_agent".to_string()),
                });
            }
            McpScopeSupport::Unsupported => {
                return Ok(CapabilityTargetStatus {
                    agent: agent.to_string(),
                    status: CapabilityTargetState::Skipped,
                    desired_scope: DesiredScope::Project,
                    applied_scope: AppliedScope::None,
                    reason: Some("unsupported_mcp".to_string()),
                });
            }
        },
    };

    if perform_write {
        agent_config::write_agent_mcp(paths, agent, &plan.definition, applied_scope, project_root)?;
    }
    Ok(CapabilityTargetStatus {
        agent: agent.to_string(),
        status: CapabilityTargetState::Applied,
        desired_scope,
        applied_scope: AppliedScope::from_tracking(applied_scope),
        reason: None,
    })
}

fn evaluate_subagent_target(
    paths: &ArcPaths,
    cache: &DetectCache,
    agent: &str,
    plan: &SubagentApplyPlan,
    project_root: Option<&Path>,
    prompt_body: &str,
    perform_write: bool,
) -> Result<CapabilityTargetStatus> {
    let Some(agent_info) = cache.get_agent(agent) else {
        return Ok(CapabilityTargetStatus {
            agent: agent.to_string(),
            status: CapabilityTargetState::Skipped,
            desired_scope: if plan.source_scope == SourceScope::Project {
                DesiredScope::Project
            } else {
                DesiredScope::Global
            },
            applied_scope: AppliedScope::None,
            reason: Some("agent_not_detected".to_string()),
        });
    };
    if agent_info.root.is_none() {
        return Ok(CapabilityTargetStatus {
            agent: agent.to_string(),
            status: CapabilityTargetState::Skipped,
            desired_scope: if plan.source_scope == SourceScope::Project {
                DesiredScope::Project
            } else {
                DesiredScope::Global
            },
            applied_scope: AppliedScope::None,
            reason: Some("agent_not_detected".to_string()),
        });
    }
    let Some(spec) = agent_spec(agent) else {
        return Ok(CapabilityTargetStatus {
            agent: agent.to_string(),
            status: CapabilityTargetState::Skipped,
            desired_scope: if plan.source_scope == SourceScope::Project {
                DesiredScope::Project
            } else {
                DesiredScope::Global
            },
            applied_scope: AppliedScope::None,
            reason: Some("unsupported_subagent".to_string()),
        });
    };
    if spec.subagent_support != SubagentSupport::Native {
        return Ok(CapabilityTargetStatus {
            agent: agent.to_string(),
            status: CapabilityTargetState::Skipped,
            desired_scope: if plan.source_scope == SourceScope::Project {
                DesiredScope::Project
            } else {
                DesiredScope::Global
            },
            applied_scope: AppliedScope::None,
            reason: Some("unsupported_subagent".to_string()),
        });
    }
    if spec.id == "codex"
        && plan
            .definition
            .description
            .as_deref()
            .map(str::trim)
            .unwrap_or("")
            .is_empty()
    {
        return Ok(CapabilityTargetStatus {
            agent: agent.to_string(),
            status: CapabilityTargetState::Failed,
            desired_scope: if plan.source_scope == SourceScope::Project {
                DesiredScope::Project
            } else {
                DesiredScope::Global
            },
            applied_scope: AppliedScope::None,
            reason: Some("description_required".to_string()),
        });
    }
    let applied_scope = if plan.source_scope == SourceScope::Project {
        AppliedResourceScope::Project
    } else {
        AppliedResourceScope::Global
    };
    if perform_write {
        agent_config::write_agent_subagent(
            paths,
            agent,
            &plan.definition,
            prompt_body,
            applied_scope,
            project_root,
        )?;
    }
    Ok(CapabilityTargetStatus {
        agent: agent.to_string(),
        status: CapabilityTargetState::Applied,
        desired_scope: if plan.source_scope == SourceScope::Project {
            DesiredScope::Project
        } else {
            DesiredScope::Global
        },
        applied_scope: AppliedScope::from_tracking(applied_scope),
        reason: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secret_validation_accepts_placeholders() {
        let mut definition = McpDefinition {
            name: "github".to_string(),
            targets: None,
            transport: McpTransportType::StreamableHttp,
            command: None,
            args: Vec::new(),
            env: BTreeMap::new(),
            cwd: None,
            env_file: None,
            url: Some("https://example.com/mcp".to_string()),
            headers: BTreeMap::from([(
                "Authorization".to_string(),
                "Bearer ${GITHUB_TOKEN}".to_string(),
            )]),
            timeout: None,
            startup_timeout_sec: None,
            tool_timeout_sec: None,
            enabled: None,
            required: None,
            trust: None,
            include_tools: Vec::new(),
            exclude_tools: Vec::new(),
            oauth: None,
            description: None,
        };

        assert!(validate_mcp_definition(&mut definition).is_ok());
    }

    #[test]
    fn secret_validation_rejects_plaintext() {
        let mut definition = McpDefinition {
            name: "github".to_string(),
            targets: None,
            transport: McpTransportType::StreamableHttp,
            command: None,
            args: Vec::new(),
            env: BTreeMap::new(),
            cwd: None,
            env_file: None,
            url: Some("https://example.com/mcp".to_string()),
            headers: BTreeMap::from([(
                "Authorization".to_string(),
                "Bearer ghp_secret".to_string(),
            )]),
            timeout: None,
            startup_timeout_sec: None,
            tool_timeout_sec: None,
            enabled: None,
            required: None,
            trust: None,
            include_tools: Vec::new(),
            exclude_tools: Vec::new(),
            oauth: None,
            description: None,
        };

        assert!(validate_mcp_definition(&mut definition).is_err());
    }

    #[test]
    fn secret_validation_accepts_agent_specific_placeholder_syntax() {
        for value in [
            "$GITHUB_TOKEN",
            "${env:GITHUB_TOKEN}",
            "{env:GITHUB_TOKEN}",
            "Bearer $GITHUB_TOKEN",
            "Bearer ${env:GITHUB_TOKEN}",
            "Bearer {env:GITHUB_TOKEN}",
        ] {
            let mut definition = McpDefinition {
                name: "github".to_string(),
                targets: None,
                transport: McpTransportType::StreamableHttp,
                command: None,
                args: Vec::new(),
                env: BTreeMap::new(),
                cwd: None,
                env_file: None,
                url: Some("https://example.com/mcp".to_string()),
                headers: BTreeMap::from([("Authorization".to_string(), value.to_string())]),
                timeout: None,
                startup_timeout_sec: None,
                tool_timeout_sec: None,
                enabled: None,
                required: None,
                trust: None,
                include_tools: Vec::new(),
                exclude_tools: Vec::new(),
                oauth: None,
                description: None,
            };
            assert!(
                validate_mcp_definition(&mut definition).is_ok(),
                "{value} should be accepted"
            );
        }
    }

    #[test]
    fn codex_remote_toml_maps_headers_to_codex_fields() {
        let definition = McpDefinition {
            name: "figma".to_string(),
            targets: None,
            transport: McpTransportType::StreamableHttp,
            command: None,
            args: Vec::new(),
            env: BTreeMap::new(),
            cwd: None,
            env_file: None,
            url: Some("https://mcp.figma.com/mcp".to_string()),
            headers: BTreeMap::from([
                (
                    "Authorization".to_string(),
                    "Bearer ${FIGMA_OAUTH_TOKEN}".to_string(),
                ),
                ("X-Region".to_string(), "us-east-1".to_string()),
                ("X-Trace".to_string(), "${TRACE_ID}".to_string()),
            ]),
            timeout: None,
            startup_timeout_sec: Some(20),
            tool_timeout_sec: Some(45),
            enabled: Some(true),
            required: Some(true),
            trust: None,
            include_tools: vec!["open".to_string()],
            exclude_tools: vec!["delete".to_string()],
            oauth: None,
            description: None,
        };
        let value = toml_codex_mcp_value(&definition).unwrap();
        let table = value.as_table().unwrap();
        assert_eq!(
            table.get("url").and_then(toml::Value::as_str),
            Some("https://mcp.figma.com/mcp")
        );
        assert_eq!(
            table
                .get("bearer_token_env_var")
                .and_then(toml::Value::as_str),
            Some("FIGMA_OAUTH_TOKEN")
        );
        assert!(table.contains_key("http_headers"));
        assert!(table.contains_key("env_http_headers"));
        assert!(table.contains_key("enabled_tools"));
        assert!(table.contains_key("disabled_tools"));
    }

    #[test]
    fn gemini_http_uses_http_url_field() {
        let definition = McpDefinition {
            name: "remote".to_string(),
            targets: None,
            transport: McpTransportType::StreamableHttp,
            command: None,
            args: Vec::new(),
            env: BTreeMap::new(),
            cwd: None,
            env_file: None,
            url: Some("https://example.com/mcp".to_string()),
            headers: BTreeMap::new(),
            timeout: Some(5_000),
            startup_timeout_sec: None,
            tool_timeout_sec: None,
            enabled: None,
            required: None,
            trust: Some(true),
            include_tools: vec!["safe_tool".to_string()],
            exclude_tools: Vec::new(),
            oauth: None,
            description: None,
        };
        let value = gemini_mcp_value(&definition);
        assert_eq!(value["httpUrl"], "https://example.com/mcp");
        assert!(value.get("url").is_none());
    }

    #[test]
    fn kimi_remote_uses_transport_field() {
        let definition = McpDefinition {
            name: "remote".to_string(),
            targets: None,
            transport: McpTransportType::StreamableHttp,
            command: None,
            args: Vec::new(),
            env: BTreeMap::new(),
            cwd: None,
            env_file: None,
            url: Some("https://example.com/mcp".to_string()),
            headers: BTreeMap::new(),
            timeout: None,
            startup_timeout_sec: None,
            tool_timeout_sec: None,
            enabled: None,
            required: None,
            trust: None,
            include_tools: Vec::new(),
            exclude_tools: Vec::new(),
            oauth: None,
            description: None,
        };
        let value = kimi_mcp_value(&definition);
        assert_eq!(value["transport"], "http");
        assert_eq!(value["url"], "https://example.com/mcp");
    }

    #[test]
    fn opencode_subagent_frontmatter_includes_mode() {
        let definition = SubagentDefinition {
            name: "reviewer".to_string(),
            description: Some("Repository reviewer".to_string()),
            targets: None,
            prompt_file: "reviewer.md".to_string(),
        };
        let body = render_markdown_subagent("opencode", &definition, "# Prompt").unwrap();
        assert!(body.contains("mode: subagent"));
    }

    #[test]
    fn claude_subagent_frontmatter_omits_mode() {
        let definition = SubagentDefinition {
            name: "reviewer".to_string(),
            description: Some("Repository reviewer".to_string()),
            targets: None,
            prompt_file: "reviewer.md".to_string(),
        };
        let body = render_markdown_subagent("claude", &definition, "# Prompt").unwrap();
        assert!(!body.contains("mode: subagent"));
    }
}
