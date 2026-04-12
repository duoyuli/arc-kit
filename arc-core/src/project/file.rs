use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{ArcError, Result};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectConfig {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default)]
    pub provider: ProviderSection,
    #[serde(default)]
    pub skills: SkillsSection,
    #[serde(default)]
    pub markets: Vec<MarketEntry>,
    #[serde(default)]
    pub mcps: McpsSection,
    #[serde(default)]
    pub subagents: SubagentsSection,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MarketEntry {
    pub url: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderSection {
    pub name: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SkillsSection {
    #[serde(default)]
    pub require: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct McpsSection {
    #[serde(default)]
    pub require: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SubagentsSection {
    #[serde(default)]
    pub require: Vec<String>,
}

fn default_version() -> u32 {
    1
}

pub fn load_project_config(path: &Path) -> Result<ProjectConfig> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| ArcError::new(format!("failed to read arc.toml: {e}")))?;
    parse_project_config(&content)
}

pub fn parse_project_config(content: &str) -> Result<ProjectConfig> {
    let value = toml::from_str::<toml::Value>(content)
        .map_err(|e| ArcError::new(format!("arc.toml parse error: {e}")))?;
    reject_inline_project_capabilities(&value)?;
    value.try_into::<ProjectConfig>().map_err(|e| {
        let msg = e.to_string();
        if msg.contains("unknown field") {
            let field = extract_unknown_field(&msg).unwrap_or("unknown");
            ArcError::with_hint(
                format!("arc.toml contains unknown field \"{field}\""),
                "Project MCPs and subagents are name-only references. Move full definitions into the global registry and keep secrets in environment variables.",
            )
        } else {
            ArcError::new(format!("arc.toml parse error: {msg}"))
        }
    })
}

fn reject_inline_project_capabilities(value: &toml::Value) -> Result<()> {
    let Some(table) = value.as_table() else {
        return Ok(());
    };

    for (key, resource_kind, command, section) in [
        (
            "mcps",
            "MCP",
            "arc mcp define",
            "[mcps] require = [\"name\"]",
        ),
        (
            "subagents",
            "subagent",
            "arc subagent install",
            "[subagents] require = [\"name\"]",
        ),
    ] {
        if matches!(table.get(key), Some(toml::Value::Array(_))) {
            return Err(ArcError::with_hint(
                format!("project-level {resource_kind} inline definitions are no longer supported"),
                format!(
                    "Move the definition into the global registry with `{command}`, then reference it from `{section}`."
                ),
            ));
        }
    }

    Ok(())
}

fn extract_unknown_field(msg: &str) -> Option<&str> {
    // toml error: "unknown field `api_key`, ..."
    let start = msg.find('`')? + 1;
    let end = msg[start..].find('`')?;
    Some(&msg[start..start + end])
}

pub fn write_project_config(path: &Path, config: &ProjectConfig) -> Result<()> {
    let content = toml::to_string_pretty(config)
        .map_err(|e| ArcError::new(format!("failed to serialize arc.toml: {e}")))?;
    let header =
        "# arc.toml — arc-kit project configuration\n# Safe to commit. Contains no secrets.\n\n";
    std::fs::write(path, format!("{header}{content}"))
        .map_err(|e| ArcError::new(format!("failed to write arc.toml: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_minimal_arc_toml() {
        let cfg = parse_project_config(
            r#"[skills]
require = ["architecture-review", "db-migration"]
"#,
        )
        .unwrap();
        assert_eq!(
            cfg.skills.require,
            vec!["architecture-review", "db-migration"]
        );
        assert_eq!(cfg.provider.name, None);
    }

    #[test]
    fn parses_full_arc_toml() {
        let cfg = parse_project_config(
            r#"
[provider]
name = "aicodemirror"

[skills]
require = ["architecture-review", "db-migration"]
"#,
        )
        .unwrap();
        assert_eq!(cfg.provider.name.as_deref(), Some("aicodemirror"));
        assert_eq!(cfg.skills.require.len(), 2);
        assert!(cfg.mcps.require.is_empty());
        assert!(cfg.subagents.require.is_empty());
    }

    #[test]
    fn empty_file_is_valid() {
        let cfg = parse_project_config("").unwrap();
        assert_eq!(cfg.version, 1);
        assert_eq!(cfg.provider.name, None);
        assert!(cfg.skills.require.is_empty());
        assert!(cfg.mcps.require.is_empty());
        assert!(cfg.subagents.require.is_empty());
    }

    #[test]
    fn rejects_unknown_fields() {
        let err = parse_project_config(r#"api_key = "secret""#).unwrap_err();
        assert!(err.message.contains("unknown field") || err.message.contains("api_key"));
    }

    #[test]
    fn rejects_invalid_version() {
        // version must be a u32, not a string
        let err = parse_project_config(r#"version = "abc""#).unwrap_err();
        assert!(err.message.contains("parse error") || err.message.contains("invalid"));
    }

    #[test]
    fn parses_markets_section() {
        let cfg = parse_project_config(
            r#"
[[markets]]
url = "https://github.com/team/skills.git"

[[markets]]
url = "https://github.com/anthropics/skills.git"
"#,
        )
        .unwrap();
        assert_eq!(cfg.markets.len(), 2);
        assert_eq!(cfg.markets[0].url, "https://github.com/team/skills.git");
        assert_eq!(
            cfg.markets[1].url,
            "https://github.com/anthropics/skills.git"
        );
    }

    #[test]
    fn empty_markets_is_valid() {
        let cfg = parse_project_config("").unwrap();
        assert!(cfg.markets.is_empty());
    }

    #[test]
    fn parses_capability_resources() {
        let cfg = parse_project_config(
            r#"
[mcps]
require = ["github"]

[subagents]
require = ["reviewer"]
"#,
        )
        .unwrap();
        assert_eq!(cfg.mcps.require, vec!["github"]);
        assert_eq!(cfg.subagents.require, vec!["reviewer"]);
    }

    #[test]
    fn rejects_inline_project_mcps() {
        let err = parse_project_config(
            r#"
[[mcps]]
name = "github"
transport = "streamable_http"
url = "https://example.com/mcp"
"#,
        )
        .unwrap_err();
        assert!(err.message.contains("no longer supported"));
        assert!(err.hint.as_deref().unwrap_or("").contains("[mcps] require"));
    }

    #[test]
    fn rejects_inline_project_subagents() {
        let err = parse_project_config(
            r#"
[[subagents]]
name = "reviewer"
prompt_file = "reviewer.md"
"#,
        )
        .unwrap_err();
        assert!(err.message.contains("no longer supported"));
        assert!(
            err.hint
                .as_deref()
                .unwrap_or("")
                .contains("[subagents] require")
        );
    }
}
