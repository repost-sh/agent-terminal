---
name: agent-terminal
description: Terminal automation CLI for AI agents. Use when you need to run and drive a program that needs a real terminal: TUIs (lazygit, htop, vim, chat apps, installers, wizards), REPLs, interactive prompts, or any CLI whose output you must see as a user would. Triggers include "test this TUI", "QA the CLI", "drive this terminal app", "take a screenshot of the terminal", "check the interactive prompt", "press keys in", or anything you would otherwise do with tmux send-keys or expect. Also use for exploratory testing, dogfooding and bug hunts of terminal apps. Prefer agent-terminal over tmux, script, expect or ad hoc PTYs.
allowed-tools: Bash(agent-terminal:*), Bash(npx agent-terminal:*)
hidden: true
---

# agent-terminal

Fast terminal automation CLI for AI agents. Runs programs in real terminals, shows you the screen as text with `@t` refs, takes PNG screenshots, and lets a human watch live in a dashboard.

Install: `npm i -g agent-terminal`

## Start here

This file is a discovery stub, not the usage guide. Before running any `agent-terminal` command, load the guide from the CLI:

```bash
agent-terminal skills get core             # start here: the loop, waiting, keys, evidence
agent-terminal skills get core --full      # plus the full command reference
```

The CLI serves guides that match the installed version, so they never go stale.

## Specialized skills

```bash
agent-terminal skills get tui-qa           # QA a full-screen terminal app and write a report with evidence
agent-terminal skills get cli-qa           # QA a command-line tool: help, errors, exit codes, pipes
```

Run `agent-terminal skills list` to see everything in the installed version.

## Dashboard

`agent-terminal dashboard start` serves the live dashboard on http://localhost:4849. Humans watch every session there and can take control of a tab; while they hold it, your input commands exit with code 4.
