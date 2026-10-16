use clap::Subcommand;

use super::programs::{OpenArgs, WaitArgs};
use super::values::parse_size;
use super::{Context, ON_ITS_OWN};
use crate::protocol::request;
use crate::protocol::{Name, Request, Size, TabId, Target};

const BYPASS_WITH_SESSION: &str = "the argument '--bypass-session' cannot be used with '--session <NAME>': close --all --session <name> closes the tabs of that session, close --all --bypass-session closes every tab in every session";

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
    #[command(about = "Current session name (new: create one; list: every session with its tabs)")]
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
    #[command(about = "Create this conversation's session, named after its task (e.g. repost-forward)")]
    New {
        #[arg(
            help = "1 to 24 characters of a-z, 0-9 and -, starting with a letter or digit",
            value_name = "NAME",
            value_parser = clap::value_parser!(Name)
        )]
        name: Name,
    },
    #[command(about = "Sessions, their tabs, programs, sizes and status")]
    List,
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

impl TabCommand {
    pub(super) fn request(self, context: &Context) -> Result<Request, String> {
        let timeout = context.timeout;
        Ok(match self {
            TabCommand::Session { action: Some(SessionAction::New { name }) } => {
                Request::NewSession(request::NewSession { session: name })
            }
            TabCommand::Session { action: Some(SessionAction::List) } => Request::Sessions,
            TabCommand::Session { action: None } => return Err(ON_ITS_OWN.into()),
            TabCommand::Tab { action: Some(TabAction::New(args)), .. } => args.request(context.session()?, timeout)?,
            TabCommand::Tab { id: Some(tab), .. } => {
                Request::SwitchTab(request::SwitchTab { session: context.session()?, tab })
            }
            TabCommand::Tab { id: None, action: None | Some(TabAction::List) } => {
                Request::Tabs(request::Tabs { session: context.session()? })
            }
            TabCommand::Tab { action: Some(TabAction::Close { id }), .. } => {
                Request::Close(request::Close::Tab(Target { tab: id.or(context.tab), session: context.session()? }))
            }
            TabCommand::Restart(wait) => Request::Restart(request::Restart {
                target: context.target()?,
                wait: wait.until(),
                timeout_ms: timeout,
            }),
            TabCommand::Resize { size, wait } => Request::Resize(request::Resize {
                target: context.target()?,
                size,
                wait: wait.until(),
                timeout_ms: timeout,
            }),
            TabCommand::Close { all, bypass_session } => Request::Close(closing(all, bypass_session, context)?),
        })
    }
}

fn closing(all: bool, bypass_session: bool, context: &Context) -> Result<request::Close, String> {
    match (all, bypass_session, context.session_typed) {
        (false, ..) => Ok(request::Close::Tab(context.target()?)),
        (true, true, true) => Err(BYPASS_WITH_SESSION.into()),
        (true, true, false) => Ok(request::Close::EverySession),
        (true, false, true) => Ok(request::Close::Session { session: context.session()? }),
        (true, false, false) => Err(SESSION_NOT_TYPED.into()),
    }
}
