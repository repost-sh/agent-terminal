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
agent-terminal open htop --wait "Load average"
agent-terminal snapshot
agent-terminal close
```

Agents start with `agent-terminal skills get core`. Every command is listed in [`docs/commands.md`](https://github.com/repost-sh/agent-terminal/blob/main/docs/commands.md).

## Skills

The usage guides ship inside the binary, so `agent-terminal skills get` always matches the installed version. What your agent needs is the small skill stub that tells it agent-terminal exists and where those guides are. Install it one of three ways.

For Claude Code, into `~/.claude/skills/agent-terminal`:

```sh
agent-terminal install --skills
```

For Claude Code, Codex, Cursor and the other assistants the [`skills`](https://github.com/vercel-labs/skills) tool supports:

```sh
npx skills add repost-sh/agent-terminal
```

From the Claude Code plugin marketplace, inside Claude Code:

```text
/plugin marketplace add repost-sh/agent-terminal
/plugin install agent-terminal@agent-terminal
```

## Keep it current

```sh
agent-terminal upgrade
```

`upgrade` finds what installed agent-terminal (npm, pnpm, yarn, bun or Homebrew), asks it for the latest version, and says so when you already have it. Otherwise it reinstalls with that same tool and refreshes the skill stub if one is installed. `upgrade --json` reports the installer, the old and new versions, and whether the stub was refreshed. When it can't tell what installed agent-terminal, it prints the upgrade command of each tool instead. A source build is upgraded by running its `cargo install` command again.

The daemon that holds your terminals keeps running the old version after an upgrade. The next command notices: if the old daemon has no open terminals, the command stops it, starts a daemon of the new version and carries on; if it still has open terminals, the command leaves them running and exits with code 6 and an error naming both versions and the number of terminals. `agent-terminal daemon stop` ends that daemon and its terminals, and the next command starts the new one.

```sh
agent-terminal doctor
```

`doctor` checks PTY support, the built-in fonts, shell integration, the daemon and its version, and the installed skill stub's version, and prints the fix for each problem; it exits 1 when it finds one. `agent-terminal doctor --fix` applies the fixes it can: it removes a stale socket, updates old shell integration scripts and rewrites an older skill stub.

## Links

- [Releases](https://github.com/repost-sh/agent-terminal/releases): the archive for each platform with its `.sha256`, and the notes for each version.
- [Changelog](https://github.com/repost-sh/agent-terminal/blob/main/CHANGELOG.md)
- [Commands](https://github.com/repost-sh/agent-terminal/blob/main/docs/commands.md)

## License

Apache-2.0
