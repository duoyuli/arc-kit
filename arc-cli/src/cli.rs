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
    #[command(about = "Manage MCP servers")]
    Mcp {
        #[command(subcommand)]
        command: Option<McpCommand>,
    },
    #[command(about = "Manage subagents")]
    Subagent {
        #[command(subcommand)]
        command: Option<SubagentCommand>,
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
    /// Allow project MCPs to fall back to global-only agent config paths
    #[arg(long)]
    pub allow_global_fallback: bool,
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

#[derive(Subcommand)]
pub enum McpCommand {
    #[command(about = "List global MCP definitions (built-in presets and user registry)")]
    List,
    #[command(about = "Show global MCP details")]
    Info(McpInfoArgs),
    #[command(
        about = "Install or update a global MCP from a preset name, or pass --transport for a custom definition"
    )]
    Install(McpInstallArgs),
    #[command(
        about = "Add or update a custom global MCP (full definition; use for servers not in presets)"
    )]
    Define(McpDefineArgs),
    #[command(about = "Remove a user MCP from the registry (built-in presets cannot be removed)")]
    Uninstall(McpUninstallArgs),
}

#[derive(Args)]
pub struct McpInfoArgs {
    #[arg(help = "MCP name")]
    pub name: String,
    #[arg(long, help = "Print secret env/header values (default: redacted)")]
    pub show_secrets: bool,
}

#[derive(Args)]
pub struct McpInstallArgs {
    #[arg(help = "MCP name (omit for interactive mode)")]
    pub name: Option<String>,
    #[arg(short, long = "agent", value_name = "AGENT")]
    pub agent: Vec<String>,
    /// Custom install: set transport and command/url (omit to use a preset by name)
    #[arg(long, value_enum)]
    pub transport: Option<McpTransportArg>,
    #[arg(long)]
    pub command: Option<String>,
    #[arg(long = "arg", allow_hyphen_values = true)]
    pub arg: Vec<String>,
    #[arg(long = "env", value_name = "KEY=VALUE")]
    pub env: Vec<String>,
    #[arg(long)]
    pub cwd: Option<String>,
    #[arg(long = "env-file")]
    pub env_file: Option<String>,
    #[arg(long)]
    pub url: Option<String>,
    #[arg(long = "header", value_name = "KEY=VALUE")]
    pub header: Vec<String>,
    #[arg(long)]
    pub timeout: Option<u64>,
    #[arg(long = "startup-timeout-sec")]
    pub startup_timeout_sec: Option<u64>,
    #[arg(long = "tool-timeout-sec")]
    pub tool_timeout_sec: Option<u64>,
    #[arg(long)]
    pub enabled: bool,
    #[arg(long)]
    pub required: bool,
    #[arg(long)]
    pub trust: bool,
    #[arg(long = "include-tool")]
    pub include_tool: Vec<String>,
    #[arg(long = "exclude-tool")]
    pub exclude_tool: Vec<String>,
    #[arg(long = "oauth-client-id")]
    pub oauth_client_id: Option<String>,
    #[arg(long = "oauth-client-secret")]
    pub oauth_client_secret: Option<String>,
    #[arg(long = "oauth-scope")]
    pub oauth_scope: Option<String>,
    #[arg(long = "oauth-callback-port")]
    pub oauth_callback_port: Option<u16>,
    #[arg(long = "oauth-auth-server-metadata-url")]
    pub oauth_auth_server_metadata_url: Option<String>,
    #[arg(long = "oauth-disabled")]
    pub oauth_disabled: bool,
    #[arg(long)]
    pub description: Option<String>,
}

#[derive(Args)]
pub struct McpDefineArgs {
    #[arg(help = "MCP name")]
    pub name: String,
    #[arg(short, long = "agent", value_name = "AGENT")]
    pub agent: Vec<String>,
    #[arg(long, value_enum)]
    pub transport: McpTransportArg,
    #[arg(long)]
    pub command: Option<String>,
    #[arg(long = "arg", allow_hyphen_values = true)]
    pub arg: Vec<String>,
    #[arg(long = "env", value_name = "KEY=VALUE")]
    pub env: Vec<String>,
    #[arg(long)]
    pub cwd: Option<String>,
    #[arg(long = "env-file")]
    pub env_file: Option<String>,
    #[arg(long)]
    pub url: Option<String>,
    #[arg(long = "header", value_name = "KEY=VALUE")]
    pub header: Vec<String>,
    #[arg(long)]
    pub timeout: Option<u64>,
    #[arg(long = "startup-timeout-sec")]
    pub startup_timeout_sec: Option<u64>,
    #[arg(long = "tool-timeout-sec")]
    pub tool_timeout_sec: Option<u64>,
    #[arg(long)]
    pub enabled: bool,
    #[arg(long)]
    pub required: bool,
    #[arg(long)]
    pub trust: bool,
    #[arg(long = "include-tool")]
    pub include_tool: Vec<String>,
    #[arg(long = "exclude-tool")]
    pub exclude_tool: Vec<String>,
    #[arg(long = "oauth-client-id")]
    pub oauth_client_id: Option<String>,
    #[arg(long = "oauth-client-secret")]
    pub oauth_client_secret: Option<String>,
    #[arg(long = "oauth-scope")]
    pub oauth_scope: Option<String>,
    #[arg(long = "oauth-callback-port")]
    pub oauth_callback_port: Option<u16>,
    #[arg(long = "oauth-auth-server-metadata-url")]
    pub oauth_auth_server_metadata_url: Option<String>,
    #[arg(long = "oauth-disabled")]
    pub oauth_disabled: bool,
    #[arg(long)]
    pub description: Option<String>,
}

#[derive(Args)]
pub struct McpUninstallArgs {
    #[arg(help = "MCP name")]
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Eq, ValueEnum)]
pub enum McpTransportArg {
    Stdio,
    Sse,
    StreamableHttp,
}

#[derive(Subcommand)]
pub enum SubagentCommand {
    #[command(about = "List global subagent definitions")]
    List,
    #[command(about = "Show global subagent details")]
    Info(SubagentInfoArgs),
    #[command(about = "Install or update a global subagent definition")]
    Install(SubagentInstallArgs),
    #[command(about = "Remove a global subagent definition")]
    Uninstall(SubagentUninstallArgs),
}

#[derive(Args)]
pub struct SubagentInfoArgs {
    #[arg(help = "Subagent name")]
    pub name: String,
}

#[derive(Args)]
pub struct SubagentInstallArgs {
    #[arg(help = "Subagent name (omit for interactive mode)")]
    pub name: Option<String>,
    #[arg(short, long = "agent", value_name = "AGENT")]
    pub agent: Vec<String>,
    #[arg(long)]
    pub description: Option<String>,
    #[arg(long = "prompt-file", value_name = "PATH")]
    pub prompt_file: Option<String>,
}

#[derive(Args)]
pub struct SubagentUninstallArgs {
    #[arg(help = "Subagent name")]
    pub name: String,
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
