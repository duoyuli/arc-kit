use std::io::{self, IsTerminal};

use arc_core::error::ArcError;

use crate::cli::OutputFormat;
use crate::format::{ErrorOutput, SCHEMA_VERSION, print_json};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandMode {
    Json,
    Interactive,
    Plain,
}

pub fn command_mode(fmt: &OutputFormat) -> CommandMode {
    if *fmt == OutputFormat::Json {
        CommandMode::Json
    } else if io::stdin().is_terminal() && io::stdout().is_terminal() {
        CommandMode::Interactive
    } else {
        CommandMode::Plain
    }
}

pub fn is_interactive(fmt: &OutputFormat) -> bool {
    command_mode(fmt) == CommandMode::Interactive
}

pub fn require_name_arg(
    fmt: &OutputFormat,
    resource_label: &str,
    usage: &str,
) -> Result<(), ArcError> {
    if is_interactive(fmt) {
        Ok(())
    } else {
        Err(ArcError::with_hint(
            format!("{resource_label} name required in non-interactive mode."),
            format!("Usage: {usage}"),
        ))
    }
}

pub fn print_not_found_json(error: impl Into<String>) -> Result<(), ArcError> {
    print_json(&ErrorOutput {
        schema_version: SCHEMA_VERSION,
        ok: false,
        error: error.into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_mode_short_circuits_tty_detection() {
        assert_eq!(command_mode(&OutputFormat::Json), CommandMode::Json);
    }

    #[test]
    fn missing_name_error_mentions_usage() {
        let err =
            require_name_arg(&OutputFormat::Json, "Skill", "arc skill install <name>").unwrap_err();
        assert!(err.message.contains("Skill name required"));
        assert!(err.hint.as_deref() == Some("Usage: arc skill install <name>"));
    }
}
