use arc_core::capability::{AppliedScope, CapabilityStatusEntry, DesiredScope};
use serde::Serialize;

use arc_core::error::ArcError;
use arc_core::status::{
    AgentRuntimeStatus, CatalogStatus, ProjectStatusSection, RecommendedAction,
};

// ── Schema version ────────────────────────────────────────
// Bump when a breaking change is made to any JSON schema.
pub const SCHEMA_VERSION: &str = "5";

// ── status ────────────────────────────────────────────────

#[derive(Serialize)]
pub struct StatusOutput {
    pub schema_version: &'static str,
    pub project: ProjectStatusSection,
    pub agents: Vec<AgentRuntimeStatus>,
    pub catalog: CatalogStatus,
    pub mcps: Vec<CapabilityStatusEntry>,
    pub subagents: Vec<CapabilityStatusEntry>,
    pub actions: Vec<RecommendedAction>,
}

// ── skill list ────────────────────────────────────────────

#[derive(Serialize)]
pub struct SkillListOutput {
    pub schema_version: &'static str,
    pub skills: Vec<SkillItem>,
}

#[derive(Serialize)]
pub struct SkillItem {
    pub name: String,
    pub origin: String,
    pub summary: String,
    pub installed_targets: Vec<String>,
}

// ── skill info ────────────────────────────────────────────

#[derive(Serialize)]
pub struct SkillInfoOutput {
    pub schema_version: &'static str,
    pub name: String,
    pub origin: String,
    pub summary: String,
    pub installed_targets: Vec<String>,
    pub source_path: String,
}

// ── mcp ───────────────────────────────────────────────────

#[derive(Serialize)]
pub struct McpListOutput {
    pub schema_version: &'static str,
    pub mcps: Vec<McpItem>,
}

#[derive(Serialize)]
pub struct McpItem {
    pub name: String,
    /// "builtin" or "user"
    pub origin: String,
    pub transport: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub targets: Option<Vec<String>>,
}

#[derive(Serialize)]
pub struct McpInfoOutput {
    pub schema_version: &'static str,
    pub name: String,
    pub transport: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    pub args: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    pub env: std::collections::BTreeMap<String, String>,
    pub headers: std::collections::BTreeMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub targets: Option<Vec<String>>,
}

// ── subagent ──────────────────────────────────────────────

#[derive(Serialize)]
pub struct SubagentListOutput {
    pub schema_version: &'static str,
    pub subagents: Vec<SubagentItem>,
}

#[derive(Serialize)]
pub struct SubagentItem {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub targets: Option<Vec<String>>,
    pub prompt_file: String,
}

#[derive(Serialize)]
pub struct SubagentInfoOutput {
    pub schema_version: &'static str,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub targets: Option<Vec<String>>,
    pub prompt_file: String,
}

// ── provider list ─────────────────────────────────────────

#[derive(Serialize)]
pub struct ProviderListOutput {
    pub schema_version: &'static str,
    pub providers: Vec<ProviderItem>,
}

#[derive(Serialize)]
pub struct ProviderItem {
    pub agent: String,
    pub agent_name: String,
    pub name: String,
    pub display_name: String,
    pub description: String,
    pub active: bool,
}

// ── market list ───────────────────────────────────────────

#[derive(Serialize)]
pub struct MarketListOutput {
    pub schema_version: &'static str,
    pub markets: Vec<MarketItem>,
}

#[derive(Serialize)]
pub struct MarketItem {
    pub id: String,
    pub git_url: String,
    pub status: String,
    pub resource_count: usize,
    pub last_updated_at: String,
}

// ── provider test ────────────────────────────────────────

#[derive(Serialize)]
pub struct ProviderTestOutput {
    pub schema_version: &'static str,
    pub results: Vec<ProviderTestItem>,
}

#[derive(Serialize)]
pub struct ProviderTestItem {
    pub provider: String,
    pub agent: String,
    pub display_name: String,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latency_ms: Option<u64>,
    pub message: String,
}

// ── write command results ────────────────────────────────

#[derive(Serialize)]
pub struct WriteResult {
    pub schema_version: &'static str,
    pub ok: bool,
    pub message: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub items: Vec<WriteResultItem>,
}

#[derive(Serialize)]
pub struct WriteResultItem {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_kind: Option<String>,
    pub name: String,
    pub agent: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub desired_scope: Option<DesiredScope>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub applied_scope: Option<AppliedScope>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Serialize)]
pub struct ErrorOutput {
    pub schema_version: &'static str,
    pub ok: bool,
    pub error: String,
}

// ── helper ────────────────────────────────────────────────

pub fn print_json<T: Serialize>(value: &T) -> Result<(), ArcError> {
    let s = serde_json::to_string_pretty(value)
        .map_err(|e| ArcError::new(format!("JSON serialization error: {e}")))?;
    println!("{s}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use arc_core::status::{
        ActionSeverity, AgentProviderStatus, ProjectProviderAgentStatus, ProjectProviderStatus,
        ProjectSkillRollout, ProjectSkillState, ProjectState, ProjectSummary, ProviderMatchState,
    };

    use super::*;

    #[test]
    fn status_output_serializes_correctly() {
        let out = StatusOutput {
            schema_version: SCHEMA_VERSION,
            project: ProjectStatusSection {
                state: ProjectState::None,
                name: "workspace".to_string(),
                root: None,
                config_path: None,
                error: None,
                summary: None,
                skills: vec![],
                agents: vec![],
                provider: None,
            },
            agents: vec![AgentRuntimeStatus {
                id: "claude".to_string(),
                name: "Claude Code".to_string(),
                version: Some("1.0".to_string()),
                provider: Some(AgentProviderStatus {
                    name: "aicodemirror".to_string(),
                    display_name: "AiCodeMirror".to_string(),
                }),
                global_skill_count: 3,
                supports_project_skills: true,
                supports_provider: true,
                mcp_scope_supported: "project_native".to_string(),
                mcp_transports_supported: vec!["stdio".to_string(), "streamable_http".to_string()],
                subagent_supported: "native".to_string(),
            }],
            catalog: CatalogStatus {
                market_count: 1,
                resource_count: 10,
                global_skill_count: 3,
                unhealthy_market_count: 0,
            },
            mcps: vec![],
            subagents: vec![],
            actions: vec![],
        };
        let json = serde_json::to_value(&out).unwrap();
        assert_eq!(json["schema_version"], SCHEMA_VERSION);
        assert_eq!(json["agents"][0]["id"], "claude");
        assert_eq!(json["agents"][0]["global_skill_count"], 3);
        assert_eq!(json["project"]["state"], "none");
    }

    #[test]
    fn status_output_with_project() {
        let out = StatusOutput {
            schema_version: SCHEMA_VERSION,
            agents: vec![],
            catalog: CatalogStatus {
                market_count: 0,
                resource_count: 0,
                global_skill_count: 0,
                unhealthy_market_count: 0,
            },
            mcps: vec![],
            project: ProjectStatusSection {
                state: ProjectState::Active,
                name: "my-project".to_string(),
                root: Some("/path".into()),
                config_path: Some("/path/arc.toml".into()),
                error: None,
                summary: Some(ProjectSummary {
                    required_skills: 1,
                    ready_skills: 1,
                    partial_skills: 0,
                    missing_skills: 0,
                    unavailable_skills: 0,
                    target_agents: 1,
                }),
                skills: vec![ProjectSkillRollout {
                    name: "arch-review".to_string(),
                    state: ProjectSkillState::Ready,
                    ready_on_agents: vec!["claude".to_string()],
                    missing_on_agents: vec![],
                }],
                agents: vec![],
                provider: Some(ProjectProviderStatus {
                    name: "openai".to_string(),
                    matched_agents: 1,
                    mismatched_agents: 0,
                    missing_profiles: 0,
                    agents: vec![ProjectProviderAgentStatus {
                        id: "claude".to_string(),
                        name: "Claude Code".to_string(),
                        state: ProviderMatchState::Matched,
                    }],
                }),
            },
            subagents: vec![],
            actions: vec![RecommendedAction {
                severity: ActionSeverity::Info,
                message: "All good".to_string(),
                command: None,
            }],
        };
        let json = serde_json::to_value(&out).unwrap();
        assert_eq!(json["project"]["state"], "active");
        assert_eq!(json["project"]["summary"]["required_skills"], 1);
        assert_eq!(json["actions"][0]["severity"], "info");
    }

    #[test]
    fn skill_list_output_serializes_correctly() {
        let out = SkillListOutput {
            schema_version: SCHEMA_VERSION,
            skills: vec![SkillItem {
                name: "arch-review".to_string(),
                origin: "market".to_string(),
                summary: "Architecture review skill".to_string(),
                installed_targets: vec!["claude".to_string()],
            }],
        };
        let json = serde_json::to_value(&out).unwrap();
        assert_eq!(json["schema_version"], SCHEMA_VERSION);
        assert_eq!(json["skills"][0]["name"], "arch-review");
        assert_eq!(json["skills"][0]["installed_targets"][0], "claude");
    }

    #[test]
    fn provider_list_output_serializes_correctly() {
        let out = ProviderListOutput {
            schema_version: SCHEMA_VERSION,
            providers: vec![ProviderItem {
                agent: "claude".to_string(),
                agent_name: "Claude".to_string(),
                name: "aicodemirror".to_string(),
                display_name: "AiCodeMirror".to_string(),
                description: "Fast mirror".to_string(),
                active: true,
            }],
        };
        let json = serde_json::to_value(&out).unwrap();
        assert_eq!(json["providers"][0]["active"], true);
        assert_eq!(json["providers"][0]["name"], "aicodemirror");
    }

    #[test]
    fn market_list_output_serializes_correctly() {
        let out = MarketListOutput {
            schema_version: SCHEMA_VERSION,
            markets: vec![MarketItem {
                id: "abc123".to_string(),
                git_url: "https://github.com/example/market".to_string(),
                status: "ok".to_string(),
                resource_count: 42,
                last_updated_at: "2026-03-30".to_string(),
            }],
        };
        let json = serde_json::to_value(&out).unwrap();
        assert_eq!(json["markets"][0]["resource_count"], 42);
        assert_eq!(json["markets"][0]["status"], "ok");
    }

    #[test]
    fn write_result_serializes_correctly() {
        let out = WriteResult {
            schema_version: SCHEMA_VERSION,
            ok: true,
            message: "Done.".to_string(),
            items: vec![WriteResultItem {
                resource_kind: None,
                name: "my-skill".to_string(),
                agent: "claude".to_string(),
                status: "installed".to_string(),
                desired_scope: None,
                applied_scope: None,
                reason: None,
            }],
        };
        let json = serde_json::to_value(&out).unwrap();
        assert_eq!(json["ok"], true);
        assert_eq!(json["items"][0]["name"], "my-skill");
    }

    #[test]
    fn write_result_empty_items_skipped() {
        let out = WriteResult {
            schema_version: SCHEMA_VERSION,
            ok: true,
            message: "Done.".to_string(),
            items: Vec::new(),
        };
        let json = serde_json::to_value(&out).unwrap();
        assert!(json.get("items").is_none()); // skip_serializing_if Vec::is_empty
    }
}
