use std::io::IsTerminal;
use std::process::ExitCode;

use crate::cli::{
    Cli, Commands, McpCommand, ProjectCommand, SkillCommand, SkillListArgs, SubagentCommand,
};
use crate::commands::{apply, edit, market, mcp, provider, skill, status, subagent};
use arc_core::error::ArcError;
use arc_core::{ArcPaths, DetectCache};
use clap::{CommandFactory, Parser};
use clap_complete::{Shell, generate};
use console::style;

pub fn main_exit_code() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("{} {}", style("Error:").red().bold(), err.message);
            if let Some(hint) = err.hint {
                eprintln!("{} {}", style("Hint:").yellow().bold(), hint);
            }
            ExitCode::from(err.exit_code.unwrap_or(1))
        }
    }
}

pub fn run() -> Result<(), ArcError> {
    let cli = Cli::parse();

    // Fast path: no subcommand, completion, and version need no state initialization.
    match &cli.command {
        None => {
            let _ = Cli::command().print_help();
            return Ok(());
        }
        Some(Commands::Completion { shell }) => {
            return generate_completion(*shell);
        }
        Some(Commands::Version) => {
            println!("arc v{}", env!("CARGO_PKG_VERSION"));
            return Ok(());
        }
        _ => {}
    }

    let fmt = cli.format;
    let paths = ArcPaths::default();
    paths
        .ensure_arc_home()
        .map_err(|e| ArcError::new(format!("failed to initialize state directory: {e}")))?;
    init_logger(&paths, cli.verbose);

    if std::io::stderr().is_terminal() && !paths.home().join("completions").exists() {
        eprintln!(
            "{} Run {} to enable tab completion.",
            style("Tip:").cyan().bold(),
            style("arc completion zsh").bold(),
        );
    }

    match cli.command {
        Some(Commands::Status) => {
            let cache = DetectCache::new(&paths);
            status::run(&paths, &cache, &fmt)
        }
        Some(Commands::Market { command }) => market::run(
            &paths,
            command.unwrap_or(crate::cli::MarketCommand::List),
            &fmt,
        ),
        Some(Commands::Skill { command }) => {
            let cache = DetectCache::new(&paths);
            let cmd = command.unwrap_or(SkillCommand::List(SkillListArgs { installed: false }));
            skill::run(&paths, &cache, cmd, &fmt)
        }
        Some(Commands::Mcp { command }) => {
            let cache = DetectCache::new(&paths);
            let cmd = command.unwrap_or(McpCommand::List);
            mcp::run(&paths, &cache, cmd, &fmt)
        }
        Some(Commands::Subagent { command }) => {
            let cache = DetectCache::new(&paths);
            let cmd = command.unwrap_or(SubagentCommand::List);
            subagent::run(&paths, &cache, cmd, &fmt)
        }
        Some(Commands::Provider { command }) => {
            let cache = DetectCache::new(&paths);
            provider::run(&paths, &cache, command, &fmt)
        }
        Some(Commands::Project { command }) => match command {
            ProjectCommand::Apply(opts) => {
                let cache = DetectCache::new(&paths);
                apply::run(&paths, &cache, &fmt, &opts)
            }
            ProjectCommand::Edit => {
                let cache = DetectCache::new(&paths);
                edit::run(&paths, &cache, &fmt)
            }
        },
        // Handled in fast path above.
        None | Some(Commands::Version) | Some(Commands::Completion { .. }) => unreachable!(),
    }
}

fn generate_completion(shell: Shell) -> Result<(), ArcError> {
    let ext = match shell {
        Shell::Zsh => "zsh",
        Shell::Bash => "bash",
        Shell::Fish => "fish",
        Shell::PowerShell => "ps1",
        Shell::Elvish => "elv",
        _ => "sh",
    };

    let paths = ArcPaths::default();
    let dir = paths.home().join("completions");
    std::fs::create_dir_all(&dir)
        .map_err(|e| ArcError::new(format!("failed to create completions directory: {e}")))?;

    let file_path = dir.join(format!("arc.{ext}"));
    let mut buf = Vec::new();
    let mut cmd = Cli::command();
    generate(shell, &mut cmd, "arc", &mut buf);

    std::fs::write(&file_path, &buf)
        .map_err(|e| ArcError::new(format!("failed to write {}: {e}", file_path.display())))?;

    println!("Completion script written to {}", file_path.display());
    println!();
    match shell {
        Shell::Zsh => {
            println!("Add this to your .zshrc:");
            println!(
                "  [ -r \"{}\" ] && source \"{}\"",
                file_path.display(),
                file_path.display()
            );
        }
        Shell::Bash => {
            println!("Add this to your .bashrc:");
            println!("  source \"{}\"", file_path.display());
        }
        Shell::Fish => {
            println!("Symlink or copy to fish completions:");
            println!(
                "  ln -sf \"{}\" ~/.config/fish/completions/arc.fish",
                file_path.display()
            );
        }
        _ => {
            println!("Source the generated file in your shell config.");
        }
    }
    Ok(())
}

fn init_logger(paths: &ArcPaths, verbose: bool) {
    use env_logger::fmt::Target;
    use std::fs::OpenOptions;
    use std::io::Write;

    let log_path = paths.home().join("arc.log");
    let file = OpenOptions::new().create(true).append(true).open(&log_path);

    let Ok(file) = file else {
        return;
    };
    let file = std::sync::Mutex::new(file);

    let default_level = std::env::var("ARC_LOG").unwrap_or_else(|_| {
        if verbose {
            "debug".to_string()
        } else {
            "info".to_string()
        }
    });
    let env = env_logger::Env::new().filter_or("ARC_LOG", &default_level);

    let _ = env_logger::Builder::from_env(env)
        .format(move |_buf, record| {
            let ts = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
            let line = format!("{ts} [{}] {}\n", record.level(), record.args());
            if let Ok(mut f) = file.lock() {
                let _ = f.write_all(line.as_bytes());
            }
            Ok(())
        })
        .target(Target::Pipe(Box::new(std::io::sink())))
        .try_init();
}
