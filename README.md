# agent-terminal

Terminal automation CLI for AI agents, the terminal sibling of [agent-browser](https://github.com/vercel-labs/agent-browser). It runs any program in a real terminal, reads the screen as text, sends keys, waits for the screen to settle, and keeps evidence.

## Install

```sh
npm i -g @repost/agent-terminal
```

The package is `@repost/agent-terminal` and the command it installs is `agent-terminal`. The unscoped `agent-terminal` package on npm and the `agent-terminal` crate on crates.io belong to other projects.

To run it without installing, for example in a sandbox where you can't install globally, put `npx @repost/agent-terminal` where the examples below say `agent-terminal`:

```sh
npx @repost/agent-terminal --version
```

With pnpm or bun:

```sh
pnpm add -g @repost/agent-terminal
bun add -g @repost/agent-terminal
```

With Homebrew:

```sh
brew install repost-sh/tap/agent-terminal
```

Each of these installs a prebuilt binary, so no compiler is needed; npm, npx, pnpm and bun need Node.js 18 or later. To build from source instead, with a current stable Rust from [rustup.rs](https://rustup.rs) and your platform's compiler tools (the Xcode Command Line Tools on macOS, `build-essential` or its equivalent on Linux):

```sh
cargo install --locked --git https://github.com/repost-sh/agent-terminal agent-terminal
```

## Supported platforms

- macOS 11 or later, on Apple silicon (arm64) and Intel (x64).
- Linux on x64 and arm64, with glibc 2.28 or later (tested on Debian 11, Ubuntu 20.04 and Rocky Linux 8) or with musl (Alpine). npm, pnpm and bun pick the glibc or musl build for you.
- Windows is not supported yet.

## Use

```sh
agent-terminal session new try-htop
export AGENT_TERMINAL_SESSION=try-htop
agent-terminal open --name htop htop --wait "Load average"
agent-terminal snapshot
agent-terminal close
```

A session is one conversation, and every program in it runs in a tab with a name you give it (`--name`); later calls reach a tab by that name (`--tab htop`).

Agents start with `agent-terminal skills get core`. `agent-terminal --help` lists every command.

## Skills

The usage guides ship inside the binary, so `agent-terminal skills get` always matches the installed version. What your agent needs is the small skill stub that tells it agent-terminal exists and where those guides are. Install it with the [`skills`](https://github.com/vercel-labs/skills) tool, which works for Claude Code, Codex, Cursor, Gemini CLI, Copilot, OpenCode and other agents:

```sh
npx skills add repost-sh/agent-terminal
```

It writes the stub into `.agents/skills` and links it into the folder of each agent you pick. Run it inside a project to install the stub there, or add `-g` to install it in your home folder for every project; an agent loads the nearest one, so a project's stub wins over yours.

Claude Code users can take it from the plugin marketplace instead, inside Claude Code:

```text
/plugin marketplace add repost-sh/agent-terminal
/plugin install agent-terminal@agent-terminal
```

## Keep it current

```sh
agent-terminal upgrade
```

`upgrade` finds what installed agent-terminal (npm, pnpm, yarn, bun or Homebrew), asks it for the latest version, and says so when you already have it. Otherwise it reinstalls with that same tool. `upgrade --json` reports the installer and the old and new versions. It leaves the skill stub alone: the stub only points at the guides, and the new version carries its own. When it can't tell what installed agent-terminal, it prints the upgrade command of each tool instead. A source build is upgraded by running its `cargo install` command again.

Each user account has one daemon. It holds every agent's terminals, serves the one dashboard, and keeps its socket and files in `.agent-terminal` in the home folder the user database gives your account, whatever `HOME` or any other variable says; an installed agent-terminal has no setting that points it at another daemon. A second daemon for the same user refuses to start and names the one that runs: its pid, its build and its socket.

A daemon is known by its build: its version and the commit it was built from, which `agent-terminal version` prints. After an upgrade, or whenever a command meets a daemon of another build, the command notices: if that daemon has no open terminals, the command stops it, starts a daemon of its own build and carries on; if it has open terminals, the command leaves them running and exits with code 6 and an error naming both builds and the number of terminals. Those terminals may be other agents', so a human restarts the daemon when it suits them: `agent-terminal daemon stop` ends it and every terminal it holds, and the next command starts the new one.

## Links

- [Releases](https://github.com/repost-sh/agent-terminal/releases): the archive for each platform with its `.sha256`, and the notes for each version.
- [Changelog](https://github.com/repost-sh/agent-terminal/blob/main/CHANGELOG.md)

## License

Apache-2.0
