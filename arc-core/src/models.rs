use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceKind {
    Skill,
    Mcp,
    ProviderProfile,
    #[serde(rename = "subagent", alias = "sub_agent")]
    SubAgent,
}

impl ResourceKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Skill => "skill",
            Self::Mcp => "mcp",
            Self::ProviderProfile => "provider_profile",
            Self::SubAgent => "subagent",
        }
    }

    /// Default directory name under an agent home for this kind (`as_str()`).
    ///
    /// Skills may use a per-agent subdirectory (e.g. `skills-cursor`); resolve install paths with
    /// [`crate::agent::resource_install_subdir`] instead of this alone.
    pub fn default_install_dir_name(&self) -> &'static str {
        self.as_str()
    }
}

impl std::fmt::Display for ResourceKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for ResourceKind {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "skill" => Ok(Self::Skill),
            "mcp" => Ok(Self::Mcp),
            "provider_profile" => Ok(Self::ProviderProfile),
            "subagent" | "sub_agent" => Ok(Self::SubAgent),
            _ => Err(format!("unsupported resource kind: {s}")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceInfo {
    pub id: String,
    pub kind: ResourceKind,
    pub name: String,
    pub source_id: String,
    #[serde(default)]
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarketSource {
    pub id: String,
    pub git_url: String,
    pub parser: String,
    #[serde(default)]
    pub owner: String,
    #[serde(default)]
    pub repo: String,
    #[serde(default = "default_market_status")]
    pub status: String,
    #[serde(default)]
    pub last_updated_at: String,
    #[serde(default)]
    pub resource_count: usize,
}

fn default_market_status() -> String {
    "ok".to_string()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkillOrigin {
    Market { source_id: String },
    BuiltIn,
    Local,
}

impl SkillOrigin {
    pub fn label(&self) -> &str {
        match self {
            Self::Market { .. } => "market",
            Self::BuiltIn => "built-in",
            Self::Local => "local",
        }
    }

    pub fn priority(&self) -> u8 {
        match self {
            Self::Local => 0,
            Self::Market { .. } => 1,
            Self::BuiltIn => 2,
        }
    }
}

impl std::fmt::Display for SkillOrigin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillEntry {
    pub name: String,
    pub origin: SkillOrigin,
    pub summary: String,
    pub source_path: PathBuf,
    pub installed_targets: Vec<String>,
    /// For market skills: "owner/repo" display string (e.g., "anthropics/skills")
    pub market_repo: Option<String>,
}

impl SkillEntry {
    /// Human-readable origin for lists and TUI (includes market owner/repo when known).
    pub fn origin_display(&self) -> String {
        match &self.origin {
            SkillOrigin::Market { source_id } => self
                .market_repo
                .as_ref()
                .map(|repo| format!("market ({repo})"))
                .unwrap_or_else(|| format!("market ({source_id})")),
            SkillOrigin::BuiltIn => "built-in".to_string(),
            SkillOrigin::Local => "local".to_string(),
        }
    }

    /// Stable `origin` string for JSON list output (`market:owner/repo` when known).
    pub fn origin_json(&self) -> String {
        match &self.origin {
            SkillOrigin::Market { .. } => self
                .market_repo
                .as_ref()
                .map(|repo| format!("market:{repo}"))
                .unwrap_or_else(|| "market".to_string()),
            _ => self.origin.label().to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogResource {
    pub id: String,
    pub kind: ResourceKind,
    pub name: String,
    pub source_id: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub installed: bool,
    #[serde(default)]
    pub installed_targets: Vec<String>,
}
