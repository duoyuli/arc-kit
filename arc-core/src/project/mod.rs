pub mod discover;
pub mod file;
pub mod resolve;

pub use discover::find_project_config;
pub use file::{
    MarketEntry, ProjectConfig, ProviderSection, SkillsSection, load_project_config,
    parse_project_config, write_project_config,
};
pub use resolve::{ConfigSource, EffectiveConfig, Sourced, resolve_effective_config};
