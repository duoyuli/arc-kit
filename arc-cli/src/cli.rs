use clap::{ArgAction, Args, Parser, Subcommand, ValueEnum};

#[derive(Clone, Debug, Default, PartialEq, Eq, ValueEnum)]
pub enum OutputFormat {
    #[default]
    Text,
    Json,
}

#[derive(Parser)]
#[command(
    name = "arc",
    about = "Manage coding agent configuration and capabilities",
    version = env!("CARGO_PKG_VERSION"),
)]
pub struct Cli {
    #[arg(
        short,
        long,
        action = ArgAction::SetTrue,
        global = true,
        help = "Verbose logging (sets default ARC_LOG to debug when ARC_LOG is unset)"
    )]
    pub verbose: bool,
    #[arg(
        long,
        value_enum,
        global = true,
        default_value = "text",
        help = "Output format (text or json)"
    )]
    pub format: OutputFormat,
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    #[command(about = "Show current status")]
    Status,
    #[command(about = "Show version")]
    Version,
    #[command(about = "Manage market sources")]
    Market {
        #[command(subcommand)]
        command: Option<MarketCommand>,
    },
    #[command(about = "Manage skills")]
    Skill {
        #[command(subcommand)]
        command: Option<SkillCommand>,
    },
    #[command(about = "Manage providers")]
    Provider {
        #[command(subcommand)]
        command: Option<ProviderCommand>,
    },
    #[command(about = "Manage arc.toml project configuration")]
    Project {
        #[command(subcommand)]
        command: ProjectCommand,
    },
    #[command(about = "Generate shell completion script")]
    Completion {
        #[arg(help = "Target shell (bash, zsh, fish, powershell, elvish)")]
        shell: clap_complete::Shell,
    },
}

#[derive(Args, Clone, Debug, Default)]
pub struct ProjectApplyArgs {
    /// Target agent(s) for project skill install; repeat for multiple. Omit (with no --all-agents) in TTY to pick interactively when skills need installing.
    #[arg(short, long = "agent", value_name = "AGENT")]
    pub agent: Vec<String>,
    /// Install to every detected agent that supports project-local skills (previous default)
    #[arg(long)]
    pub all_agents: bool,
}

#[derive(Subcommand)]
pub enum ProjectCommand {
    #[command(
        about = "Create or update arc.toml from the catalog, switch provider, and install project skills"
    )]
    Apply(ProjectApplyArgs),
    #[command(about = "Edit [skills] require in arc.toml (interactive)")]
    Edit,
}

#[derive(Subcommand)]
pub enum MarketCommand {
    #[command(about = "Add a market source")]
    Add {
        #[arg(help = "Git repository URL")]
        git_url: String,
    },
    #[command(about = "List configured market sources")]
    List,
    #[command(about = "Remove a market source")]
    Remove {
        #[arg(help = "Git URL or source id")]
        git_url: String,
    },
    #[command(about = "Fetch and rescan all market sources")]
    Update,
}

#[derive(Subcommand)]
pub enum SkillCommand {
    #[command(about = "List all skills")]
    List(SkillListArgs),
    #[command(about = "Install a skill")]
    Install(SkillInstallArgs),
    #[command(about = "Uninstall a skill")]
    Uninstall(SkillUninstallArgs),
    #[command(about = "Show skill details")]
    Info(SkillInfoArgs),
}

#[derive(Args)]
pub struct SkillListArgs {
    #[arg(short, long, action = ArgAction::SetTrue, help = "Show installed skills only")]
    pub installed: bool,
}

#[derive(Args)]
pub struct SkillInfoArgs {
    #[arg(help = "Skill name")]
    pub name: String,
}

#[derive(Args)]
pub struct SkillInstallArgs {
    #[arg(help = "Skill name (omit for interactive mode)")]
    pub name: Option<String>,
    #[arg(short, long = "agent", help = "Target agent(s)")]
    pub agent: Vec<String>,
}

#[derive(Args)]
pub struct SkillUninstallArgs {
    #[arg(help = "Skill name (omit for interactive mode)")]
    pub name: Option<String>,
    #[arg(short, long = "agent", help = "Target agent(s)")]
    pub agent: Vec<String>,
    #[arg(long, action = ArgAction::SetTrue, help = "Uninstall from all agents")]
    pub all: bool,
}

#[derive(Subcommand)]
pub enum ProviderCommand {
    #[command(about = "List available providers")]
    List,
    #[command(about = "Switch provider")]
    Use {
        #[arg(help = "Provider name (omit for interactive mode)")]
        name: Option<String>,
        #[arg(short, long, help = "Target agent")]
        agent: Option<String>,
    },
    #[command(about = "Test provider connectivity")]
    Test {
        #[arg(help = "Provider name (omit to test active providers)")]
        name: Option<String>,
        #[arg(short, long, help = "Target agent")]
        agent: Option<String>,
    },
}
