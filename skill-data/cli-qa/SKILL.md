---
name: cli-qa
description: Test a command-line tool the way users run it, from a real terminal. Use when asked to QA or review a CLI's help text, errors, exit codes, prompts, colors, or behavior when piped. Produces a report with the exact commands, their output and exit codes.
allowed-tools: Bash(agent-terminal:*), Bash(npx agent-terminal:*)
---

# CLI QA

Run a command-line tool in a real terminal, check that it behaves well for people and for scripts, and report what does not.

Read `agent-terminal skills get core` first if you have not.

## Setup

```bash
mkdir -p ./cli-qa-output/screenshots
cp "$(agent-terminal skills path cli-qa)/templates/report.md" ./cli-qa-output/report.md
export AGENT_TERMINAL_SESSION=qa-<tool>
agent-terminal shell --cwd <project>
```

Use `exec` for every command so each one lands in `history` with its exit code.

## What to test

1. **Help.** `<tool> --help`, `<tool> help <cmd>`, `<tool>` with no arguments. Is it accurate, readable at 80 columns, and does every listed command exist?
2. **Version.** `--version` prints one line and exits 0.
3. **Exit codes.** Success is 0. Every failure is not 0. Check with `exec "…"` and `history`.
4. **Errors.** Unknown command, missing argument, bad value, missing file. Does the message say what went wrong and how to fix it? Does it go to stderr?
5. **Prompts.** Interactive questions: answer them with `type` and `press`, then try `Ctrl+c` halfway. Does a `--yes` or `--no-input` flag exist for scripts?
6. **Pipes.** `exec "<tool> … | cat"`: no colors, spinners or prompts when output is not a terminal. `NO_COLOR=1` is respected.
7. **Width.** `resize 80x24` and run the widest output (tables, lists). Nothing should break mid-word or overflow.
8. **Slow work.** Long operations show progress, and `Ctrl+c` stops them cleanly.

## Evidence

For each finding: the exact command, its exit code (`history show <id>`), and a screenshot when the problem is visual.

## Wrap up

Fill in `./cli-qa-output/report.md`: one entry per finding (command, exit code, expected, actual, screenshot) and the summary counts. Then `agent-terminal close --all`.
