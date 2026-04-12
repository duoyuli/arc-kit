pub mod adapters;
pub mod agent;
pub mod backup;
pub mod capability;
pub mod detect;
pub mod engine;
pub mod error;
pub mod git;
pub mod io;
pub mod market;
pub mod mcp_registry;
pub mod models;
pub mod paths;
pub mod project;
pub mod provider;
pub mod skill;
pub mod status;

// Re-exports for convenience.
// Prefer module-scoped access (e.g. `arc_core::models::SkillEntry`) for new code;
// these are kept for backward compatibility only.
pub use agent::{
    AGENT_SPECS, AgentConfig, AgentSpec, AppliedResourceScope, McpConfigFormat, McpScopeSupport,
    McpTransportSupport, ProviderKind, SkillInstallStrategy, SubagentSupport, agent_mcp_path,
    agent_spec, agent_specs, agent_subagent_dir, default_install_targets,
    ordered_agent_ids_for_resource_kind, project_skill_path, resource_install_subdir,
};
pub use detect::{
    AgentInfo, DetectCache, detect_agent, detect_agents_for_install, detect_all_agents,
    get_detected_agents, project_skills_satisfied_all, project_skills_satisfied_any,
};
pub use engine::InstallEngine;
pub use error::{ArcError, Result};
pub use models::{
    CatalogResource, MarketSource, ResourceInfo, ResourceKind, SkillEntry, SkillOrigin,
};
pub use paths::ArcPaths;
pub use provider::seed_default_providers;
pub use skill::{
    GlobalSkillCleanupReport, GlobalSkillMaintenanceReport, InstalledSkillSyncFailure,
    InstalledSkillSyncReport, SkillRegistry, run_global_skill_maintenance,
};
pub use status::collect_status;
