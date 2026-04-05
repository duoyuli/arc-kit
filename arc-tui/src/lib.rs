mod agent;
mod fuzzy;
mod install;
mod provider;
mod skill;
mod theme;

pub use agent::{select_agent, select_agents};
pub use install::run_install_wizard;
pub use provider::select_provider;
pub use skill::{
    run_skill_browser, run_skill_install_wizard, run_skill_require_pick_wizard,
    run_skill_require_pick_wizard_with_defaults, run_skill_uninstall_wizard,
};

/// Simple yes/no confirmation prompt. Returns the user's choice.
pub fn confirm(prompt: &str, default: bool) -> std::io::Result<bool> {
    Ok(dialoguer::Confirm::new()
        .with_prompt(prompt)
        .default(default)
        .interact()?)
}
