# agent-terminal

Terminal automation CLI for AI agents, the terminal sibling of [agent-browser](https://github.com/vercel-labs/agent-browser). It runs any program in a real terminal, reads the screen as text, sends keys, waits for the screen to settle, and keeps evidence.

## Install

```sh
npm i -g agent-terminal
```

or with Homebrew:

```sh
brew install repost-sh/tap/agent-terminal
```

Both install a prebuilt binary for macOS (arm64, x64) or Linux (arm64, x64, glibc or musl); no compiler is needed. To build from source instead:

```sh
cargo install --locked --git https://github.com/repost-sh/agent-terminal agent-terminal
```

## Use

```sh
agent-terminal open htop --wait "Load average"
agent-terminal snapshot
agent-terminal close
```

Agents start with `agent-terminal skills get core`. Every command is listed in [`docs/commands.md`](docs/commands.md).

## License

Apache-2.0
