pub mod adapters;
pub mod backup;
pub mod detect;
pub mod engine;
pub mod error;
pub mod git;
pub mod io;
pub mod market;
pub mod models;
pub mod paths;
pub mod project;
pub mod provider;
pub mod skill;
pub mod status;

// Re-exports for convenience.
// Prefer module-scoped access (e.g. `arc_core::models::SkillEntry`) for new code;
// these are kept for backward compatibility only.
pub use detect::{
    AgentConfig, AgentInfo, CodingAgentSpec, DetectCache, detect_agent, detect_agents_for_install,
    detect_all_agents, get_detected_agents, resource_install_subdir,
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
