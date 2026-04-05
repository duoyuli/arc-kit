use serde::Serialize;

use arc_core::error::ArcError;

// ── Schema version ────────────────────────────────────────
// Bump when a breaking change is made to any JSON schema.
pub const SCHEMA_VERSION: &str = "1";

// ── status ────────────────────────────────────────────────

#[derive(Serialize)]
pub struct StatusOutput {
    pub schema_version: &'static str,
    pub agents: Vec<AgentStatus>,
    pub markets: MarketsSummary,
    pub installed_skills: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project: Option<ProjectStatus>,
}

#[derive(Serialize)]
pub struct AgentStatus {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    pub skill_count: usize,
}

#[derive(Serialize)]
pub struct MarketsSummary {
    pub count: usize,
    pub resource_count: usize,
}

#[derive(Serialize)]
pub struct ProjectStatus {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config_path: Option<String>,
    pub required_skills: Vec<String>,
    pub installed_skills: Vec<String>,
    pub missing_skills: Vec<String>,
    pub unavailable_skills: Vec<String>,
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
    pub name: String,
    pub agent: String,
    pub status: String,
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
    use super::*;

    #[test]
    fn status_output_serializes_correctly() {
        let out = StatusOutput {
            schema_version: SCHEMA_VERSION,
            agents: vec![AgentStatus {
                id: "claude".to_string(),
                name: "Claude".to_string(),
                version: Some("1.0".to_string()),
                provider: Some("aicodemirror".to_string()),
                skill_count: 3,
            }],
            markets: MarketsSummary {
                count: 1,
                resource_count: 10,
            },
            installed_skills: 3,
            project: None,
        };
        let json = serde_json::to_value(&out).unwrap();
        assert_eq!(json["schema_version"], SCHEMA_VERSION);
        assert_eq!(json["agents"][0]["id"], "claude");
        assert_eq!(json["agents"][0]["skill_count"], 3);
        assert!(json["project"].is_null());
    }

    #[test]
    fn status_output_with_project() {
        let out = StatusOutput {
            schema_version: SCHEMA_VERSION,
            agents: vec![],
            markets: MarketsSummary {
                count: 0,
                resource_count: 0,
            },
            installed_skills: 0,
            project: Some(ProjectStatus {
                name: "my-project".to_string(),
                config_path: Some("/path/arc.toml".to_string()),
                required_skills: vec!["arch-review".to_string()],
                installed_skills: vec!["arch-review".to_string()],
                missing_skills: vec![],
                unavailable_skills: vec![],
            }),
        };
        let json = serde_json::to_value(&out).unwrap();
        assert_eq!(json["project"]["name"], "my-project");
        assert_eq!(
            json["project"]["missing_skills"].as_array().unwrap().len(),
            0
        );
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
                name: "my-skill".to_string(),
                agent: "claude".to_string(),
                status: "installed".to_string(),
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
