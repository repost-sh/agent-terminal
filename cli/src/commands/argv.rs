use std::ffi::OsString;
use std::iter::Peekable;
use std::slice::Iter;

use clap::{ArgAction, ArgMatches};

enum Takes {
    Nothing,
    Value,
    Number,
}

impl Takes {
    fn value<'a>(self, rest: &mut Peekable<Iter<'_, &'a str>>) -> Option<&'a str> {
        match self {
            Takes::Nothing => None,
            Takes::Value => rest.next().copied(),
            Takes::Number => {
                rest.next_if(|next| !next.is_empty() && next.bytes().all(|byte| byte.is_ascii_digit())).copied()
            }
        }
    }
}

pub(super) fn parse(parser: &mut clap::Command, argv: &[OsString]) -> Result<ArgMatches, clap::Error> {
    let mut unnamed = tab_name_optional(parser);
    let reordered =
        unnamed.try_get_matches_from_mut(argv).ok().and_then(|matches| open_options_first(&unnamed, &matches, argv));
    parser.try_get_matches_from_mut(reordered.as_deref().unwrap_or(argv))
}

fn tab_name_optional(parser: &clap::Command) -> clap::Command {
    let optional = |opening: clap::Command| opening.mut_arg("name", |name| name.required(false));
    parser.clone().mut_subcommand("open", optional).mut_subcommand("tab", |tab| tab.mut_subcommand("new", optional))
}

fn open_options_first(parser: &clap::Command, matches: &ArgMatches, argv: &[OsString]) -> Option<Vec<OsString>> {
    let (path, opened) = opened(matches)?;
    let command: Vec<&str> = opened.get_many::<String>("command")?.map(String::as_str).collect();
    let program_at = argv.len().checked_sub(command.len())?;
    let (before, _) = argv.split_at_checked(program_at)?;
    if before.last().is_some_and(|word| word == "--") {
        return None;
    }
    let mut parser = parser.clone();
    parser.build();
    let open = path.iter().try_fold(&parser, |command, name| command.find_subcommand(name))?;
    let (program, rest) = command.split_first()?;
    let split = Split::of(open, program, rest);
    if split.options.is_empty() && !split.separated {
        return None;
    }
    let mut reordered = before.to_vec();
    reordered.extend(split.options);
    reordered.push("--".into());
    reordered.extend(split.program);
    Some(reordered)
}

fn opened(matches: &ArgMatches) -> Option<(&'static [&'static str], &ArgMatches)> {
    match matches.subcommand()? {
        ("open", opened) => Some((&["open"], opened)),
        ("tab", tab) => match tab.subcommand()? {
            ("new", opened) => Some((&["tab", "new"], opened)),
            _ => None,
        },
        _ => None,
    }
}

struct Split {
    options: Vec<OsString>,
    program: Vec<OsString>,
    separated: bool,
}

impl Split {
    fn of(open: &clap::Command, program: &str, rest: &[&str]) -> Self {
        let mut split = Split { options: Vec::new(), program: vec![OsString::from(program)], separated: false };
        let mut rest = rest.iter().peekable();
        while let Some(&token) = rest.next() {
            if token == "--" {
                split.separated = true;
                split.program.extend(rest.map(OsString::from));
                break;
            }
            match takes(open, token) {
                Some(takes) => {
                    split.options.push(OsString::from(token));
                    split.options.extend(takes.value(&mut rest).map(OsString::from));
                }
                None => split.program.push(OsString::from(token)),
            }
        }
        split
    }
}

fn takes(open: &clap::Command, token: &str) -> Option<Takes> {
    let name = token.strip_prefix("--")?.split('=').next()?;
    let arg = open.get_arguments().find(|arg| arg.get_long() == Some(name))?;
    let inline = token.contains('=');
    Some(match arg.get_action() {
        ArgAction::Set | ArgAction::Append if inline => Takes::Nothing,
        ArgAction::Set | ArgAction::Append => {
            if arg.get_num_args().is_some_and(|range| range.min_values() == 0) {
                Takes::Number
            } else {
                Takes::Value
            }
        }
        _ => Takes::Nothing,
    })
}
