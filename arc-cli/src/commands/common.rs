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

#[derive(serde::Serialize)]
pub struct InstallCommandOutput<'a> {
    pub schema_version: &'static str,
    pub ok: bool,
    pub message: &'a str,
    pub has_changes: bool,
    #[serde(flatten)]
    pub report: &'a arc_core::skill::install::InstallReport,
}

pub fn render_install_report_text(report: &arc_core::skill::install::InstallReport) {
    for item in &report.items {
        let status = serde_json::to_string(&item.status).unwrap_or_default();
        let label = status.trim_matches('"').replace('_', " ");
        let scope = match item.scope {
            arc_core::skill::install::InstallScope::Global => "global",
            arc_core::skill::install::InstallScope::Project => "project",
        };
        println!("  {} {} -> {} ({})", label, item.skill, item.agent, scope);
        if let Some(path) = &item.target_path {
            println!("    {}", path.display());
        }
        if let Some(message) = &item.message {
            println!("    {message}");
        }
    }
    for message in &report.errors {
        println!("  error: {message}");
    }
}

pub fn render_install_report(
    report: &arc_core::skill::install::InstallReport,
    fmt: &OutputFormat,
) -> Result<(), ArcError> {
    if *fmt == OutputFormat::Json {
        print_json(&InstallCommandOutput {
            schema_version: SCHEMA_VERSION,
            ok: report.ok(),
            message: if report.ok() {
                "Done."
            } else {
                "Completed with issues."
            },
            has_changes: report.has_changes(),
            report,
        })?;
    } else {
        render_install_report_text(report);
    }
    if report.ok() {
        Ok(())
    } else {
        Err(ArcError::new("Skill operation completed with issues."))
    }
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
