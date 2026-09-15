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
    about = "Manage coding agent providers, skills, markets, and project configuration",
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

#[allow(clippy::large_enum_variant)]
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
    /// 仅协调显式选择的项目 agent，可重复传入。
    #[arg(
        short,
        long = "agent",
        value_name = "AGENT",
        conflicts_with = "all_agents",
        help = "Reconcile only these project agents; repeat for multiple."
    )]
    pub agent: Vec<String>,
    /// 合并当前探测到的项目 agent 与历史记录中的 agent。
    #[arg(
        long,
        conflicts_with = "agent",
        help = "Include detected project agents and previously tracked project agents."
    )]
    pub all_agents: bool,
    /// 只读取本地数据生成预演，不修改文件或缓存。
    #[arg(
        long,
        help = "Preview from local data without changing files or caches."
    )]
    pub dry_run: bool,
    /// 显式接管与声明来源匹配的已有目标。
    #[arg(
        long,
        help = "Take ownership of matching existing required skill targets."
    )]
    pub adopt_existing: bool,
}

#[derive(Args, Clone, Debug, Default)]
pub struct ProjectCleanArgs {
    /// 显式指定仍存在的项目目录，允许声明文件已被移除。
    #[arg(
        long,
        value_name = "PATH",
        help = "Explicit existing project directory; usable after arc.toml has been removed."
    )]
    pub project_root: Option<std::path::PathBuf>,
    #[arg(
        short,
        long = "agent",
        value_name = "AGENT",
        conflicts_with = "all_agents"
    )]
    pub agent: Vec<String>,
    #[arg(long, conflicts_with = "agent")]
    pub all_agents: bool,
    #[arg(long, help = "Preview cleanup without changing files or caches")]
    pub dry_run: bool,
}

#[derive(Subcommand)]
pub enum ProjectCommand {
    #[command(
        about = "Reconcile tracked project skills with arc.toml and switch the required provider"
    )]
    Apply(ProjectApplyArgs),
    #[command(about = "Remove tracked project skill installations while keeping arc.toml")]
    Clean(ProjectCleanArgs),
    #[command(about = "Edit project skill requirements in arc.toml (interactive)")]
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
    #[command(about = "List global skills")]
    List(SkillListArgs),
    #[command(about = "Install a global skill")]
    Install(SkillInstallArgs),
    #[command(about = "Uninstall a global skill")]
    Uninstall(SkillUninstallArgs),
    #[command(about = "Show global skill details")]
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
    #[arg(long, action = ArgAction::SetTrue, help = "Uninstall global copies from all agents; project installs are excluded")]
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
