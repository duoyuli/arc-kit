use dialoguer::Input;

use crate::select_agents;
use crate::theme::theme;

pub fn run_subagent_install_wizard(
    available_agents: &[String],
    default_name: Option<&str>,
    default_description: Option<&str>,
    default_prompt_file: Option<&str>,
    default_agents: &[String],
) -> dialoguer::Result<(String, Option<String>, String, Vec<String>)> {
    let name = Input::<String>::with_theme(&theme())
        .with_prompt("Subagent name")
        .with_initial_text(default_name.unwrap_or_default())
        .validate_with(|input: &String| -> Result<(), &'static str> {
            if input.trim().is_empty() {
                Err("Subagent name is required.")
            } else {
                Ok(())
            }
        })
        .interact_text()?;

    let description = Input::<String>::with_theme(&theme())
        .with_prompt("Description (optional)")
        .with_initial_text(default_description.unwrap_or_default())
        .allow_empty(true)
        .interact_text()?;

    let prompt_file = Input::<String>::with_theme(&theme())
        .with_prompt("Prompt file")
        .with_initial_text(default_prompt_file.unwrap_or_default())
        .validate_with(|input: &String| -> Result<(), &'static str> {
            if input.trim().is_empty() {
                Err("Prompt file is required.")
            } else {
                Ok(())
            }
        })
        .interact_text()?;

    let installed: Vec<&String> = default_agents.iter().collect();
    let agents = select_agents(available_agents, &installed)?;

    Ok((
        name.trim().to_string(),
        if description.trim().is_empty() {
            None
        } else {
            Some(description.trim().to_string())
        },
        prompt_file.trim().to_string(),
        agents,
    ))
}
