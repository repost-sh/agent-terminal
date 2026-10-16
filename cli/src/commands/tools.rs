use clap::{Args, Subcommand};

use super::ON_ITS_OWN;
use super::values::parse_origin;
use crate::protocol::Request;
use crate::protocol::request;
use crate::skills;

#[derive(Subcommand)]
pub(super) enum ToolCommand {
    #[command(about = "Guides that ship with this version (core, tui-qa, cli-qa)")]
    Skills {
        #[command(subcommand)]
        action: Option<skills::Action>,
    },
    #[command(
        about = "The live dashboard on http://localhost:4849 (dashboard start, dashboard stop)",
        args_conflicts_with_subcommands = true
    )]
    Dashboard {
        #[command(flatten)]
        start: DashboardArgs,
        #[command(subcommand)]
        action: Option<DashboardAction>,
    },
    #[command(about = "Watch or drive a session from your own terminal (detach: Ctrl-p Ctrl-q)")]
    Attach {
        #[arg(long, help = "Only watch: your keys are not sent and the agent keeps control")]
        read_only: bool,
    },
    #[command(about = "Install the agent skill stub into ~/.claude/skills")]
    Install {
        #[arg(long, help = "Install the agent skill stub (the default)")]
        skills: bool,
    },
    #[command(hide = true)]
    Daemon {
        #[command(subcommand)]
        action: DaemonAction,
    },
    #[command(
        about = "Several commands in one call (args or stdin)",
        long_about = "Run several commands in order in one call to the daemon.\n\nPass each command as one quoted argument, e.g. batch \"open htop\" \"snapshot\", or pipe a JSON array to stdin whose items are command lines or argument arrays, e.g. [[\"open\", \"htop\"], \"snapshot\"]. Commands use the batch's --session, --tab and --timeout unless they set their own. Every command is parsed before any of them runs. Each command's output and exit code are printed in order, and batch exits with the code of the first command that failed."
    )]
    Batch(BatchArgs),
    #[command(about = "MCP stdio server with the same commands as tools")]
    Mcp,
    #[command(
        about = "Check PTY support, the built-in fonts, shell integration, the daemon (its socket and version) and the installed skill stub's version, and say how to fix each problem"
    )]
    Doctor {
        #[arg(
            long,
            help = "Remove stale sockets and state, bring shell integration scripts and an older skill stub up to date"
        )]
        fix: bool,
    },
    #[command(
        about = "Upgrade to the latest version with what installed this one (npm, pnpm, yarn, bun or Homebrew), and refresh the installed skill stub"
    )]
    Upgrade,
    #[command(about = "Print the version")]
    Version,
}

impl ToolCommand {
    pub(super) fn request(self) -> Result<Request, String> {
        match self {
            ToolCommand::Daemon { action: DaemonAction::Stop } => Ok(Request::Stop),
            ToolCommand::Dashboard { action: Some(DashboardAction::Stop), .. } => Ok(Request::DashboardStop),
            ToolCommand::Dashboard {
                action: Some(DashboardAction::Start(DashboardArgs { port, allow_origin })),
                ..
            }
            | ToolCommand::Dashboard { start: DashboardArgs { port, allow_origin }, action: None } => {
                Ok(Request::DashboardStart(request::DashboardStart { port, origins: allow_origin }))
            }
            ToolCommand::Daemon { action: DaemonAction::Run }
            | ToolCommand::Skills { .. }
            | ToolCommand::Install { .. }
            | ToolCommand::Batch(_)
            | ToolCommand::Mcp
            | ToolCommand::Attach { .. }
            | ToolCommand::Doctor { .. }
            | ToolCommand::Upgrade
            | ToolCommand::Version => Err(ON_ITS_OWN.into()),
        }
    }
}

#[derive(Args)]
pub(super) struct BatchArgs {
    #[arg(long, help = "Stop at the first command that fails")]
    pub(super) bail: bool,

    #[arg(help = "The commands, each as one quoted argument", value_name = "COMMAND")]
    pub(super) commands: Vec<String>,
}

#[derive(Subcommand)]
pub(super) enum DashboardAction {
    #[command(about = "Serve the dashboard and print its address (the default)")]
    Start(DashboardArgs),
    #[command(about = "Stop serving the dashboard")]
    Stop,
}

#[derive(Args)]
pub(super) struct DashboardArgs {
    #[arg(long, help = "Port to serve on (0 picks a free one)", value_name = "N", default_value_t = 4849)]
    port: u16,

    #[arg(
        long = "allow-origin",
        help = "Also answer pages from this origin, such as the dashboard's dev server (repeatable)",
        value_name = "ORIGIN",
        value_parser = parse_origin
    )]
    allow_origin: Vec<String>,
}

#[derive(Subcommand)]
pub(super) enum DaemonAction {
    Run,
    Stop,
}
