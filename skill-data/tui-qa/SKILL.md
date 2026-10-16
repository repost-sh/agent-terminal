---
name: tui-qa
description: Systematically test a full-screen terminal app (TUI) and find bugs, broken layouts, dead keys and confusing flows. Use when asked to QA, dogfood, test, or bug hunt a TUI or interactive terminal program. Produces a report where every finding has repro steps, the exact keys pressed, the terminal size and a screenshot.
allowed-tools: Bash(agent-terminal:*), Bash(npx @repost/agent-terminal:*)
---

# TUI QA

Explore a terminal app the way a user would, find what is broken, and leave a report someone can act on without asking you anything.

Read `agent-terminal skills get core` first if you have not.

## Setup

Only the **program** is required. Use these defaults unless told otherwise:

- Session: the program name (`lazygit` → `qa-lazygit`)
- Output folder: `./tui-qa-output/`
- Sizes: `120x40`, then `80x24`, then `200x60`
- Scope: the whole app

Start right away. Ask only if the app needs credentials you do not have.

```bash
mkdir -p ./tui-qa-output/screenshots
cp "$(agent-terminal skills path tui-qa)/templates/report.md" ./tui-qa-output/report.md
agent-terminal session new qa-lazygit
export AGENT_TERMINAL_SESSION=qa-lazygit
agent-terminal open --cwd <repo> --size 120x40 lazygit
agent-terminal snapshot -i
```

## What to test

Work through these in order. Write a finding down the moment you see it, not at the end.

1. **First screen.** Does it start cleanly? Anything cut off, misaligned, or unreadable? Is it clear what to do next?
2. **Every key it advertises.** Read the key hints (often the last row, or `?` for help) and try each one. A key that does nothing or does something else is a finding.
3. **Moving around.** Arrows, Tab and Shift+Tab, PageUp/PageDown, Home/End, Esc to back out. Can you always get back to where you were? Can focus get stuck?
4. **The main tasks.** Do the two or three things the app exists for, start to finish. Check the result with `expect`, not by eye alone.
5. **Leaving.** `q`, `Esc`, `Ctrl+c`, `Ctrl+d`. Does it exit cleanly and give the terminal back in a normal state (`wait --exit`, then check `get modes` shows alt-screen off)?
6. **Sizes.** `resize 80x24` and `resize 200x60` on the main screens. Look for cut-off text, overlapping panels and lines that wrap into the wrong place.
7. **Text.** Long names, empty lists, emoji and accented characters, very long lines, a paste of 200 lines.
8. **No color.** `open --env NO_COLOR=1 …`. Is anything only understandable through color?
9. **Errors.** Wrong input, missing files, no network. Are errors shown, readable, and recoverable?
10. **Speed.** Does any key leave the screen unchanged for more than a second? Note which.

## Evidence for each finding

```bash
# … reproduce it …
agent-terminal screenshot ./tui-qa-output/screenshots/f3.png
```

A screenshot for every finding, with the exact keys pressed, in order, and the size the terminal was.

## Wrap up

Fill in the summary counts in the report, then:

```bash
agent-terminal trace export ./tui-qa-output/session.zip
agent-terminal close --all --session qa-lazygit
```

Reply with the report path, the number of findings by severity, and the three most important ones in one line each.
