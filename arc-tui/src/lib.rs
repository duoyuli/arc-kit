mod agent;
mod fuzzy;
mod install;
mod mcp;
mod project;
mod provider;
mod skill;
mod subagent;
mod theme;
mod uninstall;

use console::style;
use dialoguer::MultiSelect;

pub use agent::{select_agent, select_agents};
pub use install::run_install_wizard;
pub use mcp::{pick_mcp, run_mcp_browser};
pub use project::{
    ProjectRequirementsSelection, run_project_requirements_editor,
    run_project_requirements_editor_with_defaults,
};
pub use provider::select_provider;
pub use skill::{
    run_skill_browser, run_skill_install_wizard, run_skill_require_pick_wizard,
    run_skill_require_pick_wizard_with_defaults, run_skill_uninstall_wizard,
};
pub use subagent::{pick_subagent, run_subagent_browser, run_subagent_install_wizard};
pub use uninstall::{UninstallEntry, run_capability_uninstall_wizard};

/// Simple yes/no confirmation prompt. Returns the user's choice.
pub fn confirm(prompt: &str, default: bool) -> std::io::Result<bool> {
    Ok(dialoguer::Confirm::new()
        .with_prompt(prompt)
        .default(default)
        .interact()?)
}

pub(crate) const REQUIRED_MULTI_SELECT_MESSAGE: &str =
    "Select at least one item before continuing.";

pub(crate) fn validate_required_multi_selection<T>(selected: &[T]) -> Result<(), &'static str> {
    if selected.is_empty() {
        Err(REQUIRED_MULTI_SELECT_MESSAGE)
    } else {
        Ok(())
    }
}

pub(crate) fn interact_required_multi_select(
    prompt: &str,
    items: &[String],
    defaults: Option<&[bool]>,
) -> dialoguer::Result<Vec<usize>> {
    loop {
        let theme = theme::theme();
        let prompt = MultiSelect::with_theme(&theme)
            .with_prompt(prompt)
            .items(items);
        let prompt = if let Some(defaults) = defaults {
            prompt.defaults(defaults)
        } else {
            prompt
        };
        let selected = prompt.interact()?;
        if let Err(message) = validate_required_multi_selection(&selected) {
            eprintln!("{}", style(message).yellow().for_stderr());
            continue;
        }
        return Ok(selected);
    }
}

#[cfg(test)]
mod tests {
    use super::validate_required_multi_selection;

    #[test]
    fn validate_required_multi_selection_rejects_empty_selection() {
        let selected: Vec<usize> = Vec::new();

        assert!(validate_required_multi_selection(&selected).is_err());
    }

    #[test]
    fn validate_required_multi_selection_accepts_non_empty_selection() {
        let selected = vec![1usize];

        assert!(validate_required_multi_selection(&selected).is_ok());
    }
}
