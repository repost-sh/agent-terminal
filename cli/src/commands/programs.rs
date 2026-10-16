use std::path::PathBuf;

use clap::{Args, Subcommand, ValueEnum};

use super::Context;
use super::values::{parse_env, parse_size};
use crate::native::pty;
use crate::protocol::request;
use crate::protocol::{NAME_RULE, Name, Request, Shell, Size, TextMatch, Wait};

pub(super) const NAME_THE_TAB: &str = "name the tab after the program or app it runs, e.g. `--name api`; the name is how later calls reach it (`--tab api`)";

fn tab_name_help() -> String {
    format!("The tab's name, after the program or app it runs (e.g. api): {NAME_RULE}")
}

#[derive(Subcommand)]
pub(super) enum ProgramCommand {
    #[command(about = "Run a program in a new terminal (prints the first stable screen)")]
    Open(OpenArgs),
    #[command(about = "Open your shell with command history turned on (prints the screen at its first prompt)")]
    Shell(ShellArgs),
    #[command(about = "Run a command in the shell tab, wait for it, print its output and exit code")]
    Exec {
        #[arg(help = "The command line, as you would type it at the prompt", value_name = "COMMAND LINE")]
        line: String,
        #[arg(long, help = "Exit with the command's own exit code")]
        check: bool,
    },
    #[command(
        about = "Commands run in the shell tab, with exit codes (history show <id> for output)",
        args_conflicts_with_subcommands = true
    )]
    History {
        #[arg(long, help = "Only the last N commands", value_name = "N")]
        last: Option<usize>,
        #[arg(long, help = "Only commands that exited with a code other than 0")]
        failed: bool,
        #[command(subcommand)]
        action: Option<HistoryAction>,
    },
}

impl ProgramCommand {
    pub(super) fn request(self, context: &Context) -> Result<Request, String> {
        let target = context.target()?;
        let timeout = context.global.timeout;
        match self {
            ProgramCommand::Open(args) => args.request(target.session, timeout),
            ProgramCommand::Shell(args) => shell(target.session, args, timeout),
            ProgramCommand::Exec { line, check } => {
                if line.trim().is_empty() {
                    return Err("exec needs a command line, e.g. exec \"cargo test\"".into());
                }
                if line.contains(['\n', '\r']) {
                    return Err("exec runs one command line; join commands with ; or && instead of newlines".into());
                }
                Ok(Request::Exec(request::Exec { target, line, check, timeout_ms: timeout }))
            }
            ProgramCommand::History { action: Some(HistoryAction::Show { id }), .. } => {
                Ok(Request::HistoryShow(request::HistoryShow { target, id }))
            }
            ProgramCommand::History { last, failed, action: None } => {
                Ok(Request::History(request::History { target, last, failed }))
            }
        }
    }
}

#[derive(Args)]
pub(super) struct OpenArgs {
    #[arg(long, required = true, help = tab_name_help(), value_name = "NAME", value_parser = clap::value_parser!(Name))]
    name: Name,

    #[arg(long, help = "Terminal size", value_name = "COLSxROWS", default_value = "120x40", value_parser = parse_size)]
    size: Size,

    #[arg(long, help = "Working directory (default: the current directory)")]
    cwd: Option<PathBuf>,

    #[arg(long = "env", help = "Extra environment variable (repeatable)", value_name = "K=V", value_parser = parse_env)]
    env: Vec<(String, String)>,

    #[command(flatten)]
    wait: WaitArgs,

    #[arg(long, help = "Print the screen right away, without waiting", conflicts_with_all = ["wait", "wait_stable"])]
    no_wait: bool,

    #[arg(
        help = "The program and its arguments",
        required = true,
        trailing_var_arg = true,
        allow_hyphen_values = true,
        value_name = "PROGRAM"
    )]
    command: Vec<String>,
}

#[derive(Args)]
pub(super) struct ShellArgs {
    #[arg(long, required = true, help = tab_name_help(), value_name = "NAME", value_parser = clap::value_parser!(Name))]
    name: Name,

    #[arg(long, value_enum, help = "The shell to open (default: your SHELL)")]
    shell: Option<ShellName>,

    #[arg(long, help = "Terminal size", value_name = "COLSxROWS", default_value = "120x40", value_parser = parse_size)]
    size: Size,

    #[arg(long, help = "Working directory (default: the current directory)")]
    cwd: Option<PathBuf>,

    #[arg(long = "env", help = "Extra environment variable (repeatable)", value_name = "K=V", value_parser = parse_env)]
    env: Vec<(String, String)>,

    #[arg(long, help = "Wait until this text is on screen instead of the first prompt", value_name = "TEXT")]
    wait: Option<String>,

    #[arg(
        long,
        help = "Wait until the screen has not changed for this long instead of the first prompt",
        value_name = "MS",
        num_args = 0..=1,
        default_missing_value = "300",
        conflicts_with = "wait"
    )]
    wait_stable: Option<u64>,

    #[arg(long, help = "Print the screen right away, without waiting", conflicts_with_all = ["wait", "wait_stable"])]
    no_wait: bool,
}

#[derive(Clone, Copy, ValueEnum)]
enum ShellName {
    Zsh,
    Bash,
    Fish,
}

impl ShellName {
    fn shell(self) -> Shell {
        match self {
            ShellName::Zsh => Shell::Zsh,
            ShellName::Bash => Shell::Bash,
            ShellName::Fish => Shell::Fish,
        }
    }
}

#[derive(Subcommand)]
pub(super) enum HistoryAction {
    #[command(about = "One command's output, exit code, duration and working directory")]
    Show {
        #[arg(help = "The command's id, as listed by history")]
        id: u32,
    },
}

#[derive(Args)]
pub(super) struct WaitArgs {
    #[arg(long, help = "Wait until this text is on screen, not just until the screen is stable", value_name = "TEXT")]
    wait: Option<String>,

    #[arg(
        long,
        help = "How long the screen must stay unchanged to count as stable",
        value_name = "MS",
        num_args = 0..=1,
        default_value_t = 300,
        default_missing_value = "300",
        conflicts_with = "wait"
    )]
    wait_stable: u64,
}

impl WaitArgs {
    pub(super) fn until(self) -> Wait {
        match self.wait {
            Some(text) => Wait::Text(TextMatch::plain(text)),
            None => Wait::Stable { quiet_ms: self.wait_stable },
        }
    }
}

fn environment(extra: Vec<(String, String)>) -> Vec<(String, String)> {
    let mut env = pty::inherited(std::env::vars().filter(|(key, _)| !extra.iter().any(|(set, _)| set == key)));
    env.extend(extra);
    env
}

fn shell(session: Name, args: ShellArgs, timeout_ms: u64) -> Result<Request, String> {
    let tab = args.name;
    let current = std::env::current_dir().map_err(|e| format!("current directory: {e}"))?;
    let cwd = args.cwd.map_or_else(|| current.clone(), |cwd| current.join(cwd));
    let env = environment(args.env);
    let (shell, program) = match args.shell {
        Some(name) => (name.shell(), name.shell().name().to_string()),
        None => your_shell(&env)?,
    };
    let wait = match (args.no_wait, args.wait, args.wait_stable) {
        (true, _, _) => None,
        (false, Some(text), _) => Some(Wait::Text(TextMatch::plain(text))),
        (false, None, Some(quiet_ms)) => Some(Wait::Stable { quiet_ms }),
        (false, None, None) => Some(Wait::Command),
    };
    Ok(Request::Shell(request::OpenShell { session, tab, shell, program, size: args.size, cwd, env, wait, timeout_ms }))
}

fn your_shell(env: &[(String, String)]) -> Result<(Shell, String), String> {
    let program = env.iter().find(|(key, _)| key == "SHELL").map(|(_, value)| value.clone()).ok_or(
        "SHELL is not set, so agent-terminal does not know your shell; choose one with --shell zsh, --shell bash or --shell fish",
    )?;
    let shell = Shell::of_program(&program).ok_or_else(|| {
        format!(
            "your SHELL is {program}, which has no shell integration; choose --shell zsh, --shell bash or --shell fish"
        )
    })?;
    Ok((shell, program))
}

impl OpenArgs {
    pub(super) fn request(self, session: Name, timeout_ms: u64) -> Result<Request, String> {
        let current = std::env::current_dir().map_err(|e| format!("current directory: {e}"))?;
        let cwd = self.cwd.map_or_else(|| current.clone(), |cwd| current.join(cwd));
        let env = environment(self.env);
        let mut command = self.command.into_iter();
        Ok(Request::Open(request::Open {
            session,
            tab: self.name,
            program: command.next().ok_or("open needs a program to run, e.g. open --name top htop")?,
            args: command.collect(),
            size: self.size,
            cwd,
            env,
            wait: (!self.no_wait).then(|| self.wait.until()),
            timeout_ms,
        }))
    }
}
