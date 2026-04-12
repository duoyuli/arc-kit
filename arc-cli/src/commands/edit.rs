use std::env;
use std::io::{self, IsTerminal};

use arc_core::detect::DetectCache;
use arc_core::error::ArcError;
use arc_core::paths::ArcPaths;

use crate::cli::OutputFormat;
use crate::commands::arc_toml_wizard;
use crate::format::{SCHEMA_VERSION, WriteResult, print_json};

pub fn run(paths: &ArcPaths, cache: &DetectCache, fmt: &OutputFormat) -> Result<(), ArcError> {
    let cwd = env::current_dir()
        .map_err(|e| ArcError::new(format!("failed to get working directory: {e}")))?;

    if *fmt == OutputFormat::Json {
        print_json(&WriteResult {
            schema_version: SCHEMA_VERSION,
            ok: false,
            message: "`arc project edit` requires an interactive terminal.".to_string(),
            items: Vec::new(),
        })?;
        return Ok(());
    }

    let is_tty = io::stdin().is_terminal() && io::stdout().is_terminal();
    if !is_tty {
        return Err(ArcError::with_hint(
            "`arc project edit` requires an interactive terminal.".to_string(),
            "Run from a TTY.".to_string(),
        ));
    }

    let _ = arc_toml_wizard::edit_arc_toml_interactive(paths, cache, &cwd)?;
    Ok(())
}
