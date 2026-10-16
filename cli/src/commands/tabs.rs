use clap::{Subcommand, ValueEnum};

use super::programs::{OpenArgs, WaitArgs};
use super::values::parse_size;
use super::{Context, ON_ITS_OWN};
use crate::protocol::request;
use crate::protocol::request::Closing;
use crate::protocol::{Request, Size, TabId, Target};

const BYPASS_WITH_SESSION: &str = "the argument '--bypass-session' cannot be used with '--session <SESSION>': close --all --session <name> closes the tabs of that session, close --all --bypass-session closes every tab in every session";

const SESSION_NOT_TYPED: &str = "close --all closes every tab of one session, so name that session on this command line: repeat it as `--session <name>` (AGENT_TERMINAL_SESSION alone is not enough); `close --all --bypass-session` closes every tab in every session";

#[derive(Subcommand)]
pub(super) enum TabCommand {
    #[command(about = "Close the current tab (--all --session <name>: every tab of that session)")]
    Close {
        #[arg(
            long,
            help = "Close every tab of the session --session names on this command line (AGENT_TERMINAL_SESSION alone is refused)"
        )]
        all: bool,
        #[arg(long, requires = "all", help = "With --all: close every tab in every session (not with --session)")]
        bypass_session: bool,
    },
    #[command(about = "Run the same program again with the same options (prints its first stable screen)")]
    Restart(WaitArgs),
    #[command(about = "Resize the terminal, e.g. resize 80x24 (the app gets SIGWINCH)")]
    Resize {
        #[arg(value_parser = parse_size, value_name = "COLSxROWS")]
        size: Size,
        #[command(flatten)]
        wait: WaitArgs,
    },
    #[command(about = "Current session name (list: every session with its tabs)")]
    Session {
        #[command(subcommand)]
        action: Option<SessionAction>,
    },
    #[command(about = "List tabs in this session (tab <id> switches to one)", args_conflicts_with_subcommands = true)]
    Tab {
        #[arg(help = "Switch to this tab", value_name = "ID", value_parser = clap::value_parser!(TabId))]
        id: Option<TabId>,
        #[command(subcommand)]
        action: Option<TabAction>,
    },
}

#[derive(Subcommand)]
pub(super) enum SessionAction {
    #[command(about = "Sessions, their tabs, programs, sizes and status")]
    List,
    #[command(about = "A name unique to this checkout, for AGENT_TERMINAL_SESSION")]
    Id {
        #[arg(long, value_enum, default_value_t = Scope::Worktree, help = "What the name is unique to")]
        scope: Scope,
        #[arg(long, help = "Put this before the name, e.g. the project or task")]
        prefix: Option<String>,
    },
}

#[derive(Subcommand)]
pub(super) enum TabAction {
    #[command(about = "List tabs in this session")]
    List,
    #[command(about = "Another terminal in the same session (e.g. a server next to the app)")]
    New(OpenArgs),
    #[command(about = "Close a tab (the current one when no id is given)")]
    Close {
        #[arg(value_parser = clap::value_parser!(TabId), value_name = "ID")]
        id: Option<TabId>,
    },
}

#[derive(Clone, Copy, ValueEnum)]
pub(super) enum Scope {
    #[value(help = "The git checkout or worktree around the current directory (else the directory itself)")]
    Worktree,
}

impl TabCommand {
    pub(super) fn request(self, context: Context) -> Result<Request, String> {
        let Context { target, session_typed, timeout, .. } = context;
        Ok(match self {
            TabCommand::Tab { action: Some(TabAction::New(args)), .. } => args.request(target.session, timeout)?,
            TabCommand::Session { action: Some(SessionAction::List) } => Request::Sessions,
            TabCommand::Session { action: None | Some(SessionAction::Id { .. }) } => return Err(ON_ITS_OWN.into()),
            TabCommand::Tab { id: Some(tab), .. } => {
                Request::SwitchTab(request::SwitchTab { session: target.session, tab })
            }
            TabCommand::Tab { id: None, action: None | Some(TabAction::List) } => {
                Request::Tabs(request::Tabs { session: target.session })
            }
            TabCommand::Tab { action: Some(TabAction::Close { id }), .. } => Request::Close(request::Close {
                target: Target { tab: id.or(target.tab), ..target },
                scope: Closing::Tab,
            }),
            TabCommand::Restart(wait) => {
                Request::Restart(request::Restart { target, wait: wait.until(), timeout_ms: timeout })
            }
            TabCommand::Resize { size, wait } => {
                Request::Resize(request::Resize { target, size, wait: wait.until(), timeout_ms: timeout })
            }
            TabCommand::Close { all, bypass_session } => {
                Request::Close(request::Close { target, scope: closing(all, bypass_session, session_typed)? })
            }
        })
    }
}

fn closing(all: bool, bypass_session: bool, session_typed: bool) -> Result<Closing, String> {
    match (all, bypass_session, session_typed) {
        (false, ..) => Ok(Closing::Tab),
        (true, true, true) => Err(BYPASS_WITH_SESSION.into()),
        (true, true, false) => Ok(Closing::EverySession),
        (true, false, true) => Ok(Closing::Session),
        (true, false, false) => Err(SESSION_NOT_TYPED.into()),
    }
}
