pub mod apply;
pub mod discover;
pub mod file;
pub mod resolve;

pub use apply::{
    ProjectApplyExecution, ProjectApplyPlan, ProjectCapabilityApplyItem, ProjectMarketEvent,
    ProjectMarketEventStatus, ProjectProviderSwitch, ProjectSkillApplyItem,
    ProjectSkillApplyStatus, execute_project_apply, prepare_project_apply,
};
pub use discover::find_project_config;
pub use file::{
    MarketEntry, McpsSection, ProjectConfig, ProviderSection, SkillsSection, SubagentsSection,
    load_project_config, parse_project_config, write_project_config,
};
pub use resolve::{
    ConfigSource, EffectiveConfig, ProjectCapabilityRequirements, ResolvedProjectSubagent, Sourced,
    resolve_effective_config, resolve_project_capability_requirements,
};
