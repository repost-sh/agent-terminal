# agent-terminal — design

Status: ratified. The dashboard UI prototype dated 2026-10-14 at `docs/prototype/dashboard-ui-2026-10-14.html` is normative for UI: labels, copy, states, dialogs and focus behaviour carry into the implementation 1:1, and where this doc and the prototype differ on UI, the prototype wins. Calls made at ratification: on narrow screens the terminal shrinks to fit; split view has no timeline; the command is `mark <name>`, not the prototype's `mark set <name>`.

`agent-terminal` is to terminals what `agent-browser` is to web pages: a fast CLI that lets an agent start a program in a real PTY, read the screen, send keys, wait for the UI to settle, take screenshots, and leave evidence. A human can watch every session live in a dashboard and take control. It ships its own skill, version-matched to the binary.

Main job: **QA of TUIs and CLIs by agents** (lazygit-style apps, chat TUIs, installers, REPLs, `--help` output, interactive prompts).

## Principles

1. **Same vocabulary as agent-browser.** `open`, `snapshot`, `screenshot`, `press`, `type`, `click`, `wait`, `close`, `--session`, `tab`, `batch`, `diff`, `record`, `dashboard`, `skills`, `--json`. An agent that knows one CLI already knows the other.
2. **Text first, pixels when you need them.** `snapshot` is the cheap default: the screen as text, with row numbers, cursor, modes and refs. `screenshot` renders a PNG for vision models, humans and evidence.
3. **Act and wait in one call.** Every input command accepts `--wait …`, so the usual loop is one round trip: `press Enter --wait-stable`.
4. **Event driven, not polling.** A resident daemon owns the PTYs and a headless VT emulator per terminal. Waits wake on the next frame that matches.
5. **Honest about TUIs.** Refs are heuristics over cells (inverse spans, `[ OK ]` buttons, box titles, OSC 8 links). `click` refuses when the app never turned mouse reporting on, and tells the agent which keys to use instead.
6. **Humans can see everything.** Every CLI call, every shell command (exit code, duration, cwd) and every screenshot is in the dashboard, and the dashboard can rewind to any point.

## Architecture (short)

- `agent-terminal` is a single Rust binary: the CLI, the daemon and the dashboard server.
- The daemon owns, per terminal: a PTY, a VT parser and grid with scrollback (`alacritty_terminal` or `libghostty-vt`), a frame counter, an output ring buffer, and an asciicast event log.
- The CLI talks to the daemon over a unix socket. The dashboard talks over HTTP + WebSocket on `127.0.0.1:4849`.
- Each session keeps its tab recordings and the activity log of every call in a plain folder; the format is in [`session-folder.md`](session-folder.md).
- `agent-terminal mcp` is an MCP stdio server (`rmcp`) whose tools are generated from the CLI's own command definitions: one tool per command, named by its words joined with `_` (`get text` is `get_text`), taking the command's arguments and options as properties. A tool call runs that command with `--json` as a child process and returns what it prints, so a new command is a new tool with no MCP code ([ADR 0001](adr/0001-mcp-tools-run-the-cli.md)).
- PNG rendering happens on the server from the grid, with a bundled monospace font (JetBrains Mono + Symbols Nerd Font fallback, then Noto Emoji for emoji), so it needs no window server. The fonts are in `cli/assets/fonts/`, and the daemon also serves them to the dashboard; their licences, and OpenH264's (the H.264 encoder `clip` builds MP4 with), are in `licenses/`, which ships in every release archive and npm platform package.
- Shell integration (OSC 133 prompt marks, OSC 7 cwd, OSC 2 title) is injected into `bash`, `zsh` and `fish` when a session opens a shell. That is what produces command history.
- Speed targets: CLI round trip ≤ 5 ms; `snapshot` ≤ 10 ms; `screenshot` PNG ≤ 40 ms at 120×40; `open` to first frame ≤ 50 ms plus the app's own startup.

## Concepts

The vocabulary (session, terminal, tab, frame, stable, snapshot, ref, mark, command, activity, control, tool) is defined in [`GLOSSARY.md`](../GLOSSARY.md).

## Command reference

The full command list lives in [`commands.md`](commands.md). The skills that teach agents to use it live in `skills/` (the installed stub) and `skill-data/` (what `agent-terminal skills get <name>` prints).

## Example: QA of a TUI in five calls

```sh
agent-terminal --session qa open lazygit --cwd ~/code/agent-terminal --size 120x36
agent-terminal snapshot -i                       # read the panels, get refs
agent-terminal press 2 --wait "Unstaged changes" --snapshot
agent-terminal press space --wait-stable --screenshot ./evidence/staged.png
agent-terminal expect --cell 4,3 --style inverse && agent-terminal close
```

Example `snapshot -i` output:

```
qa t1 · lazygit · 120x36 · alt-screen · mouse:sgr · cursor 4,3 · frame 212
 1│┌─ Status ──────────────────┐┌─ Unstaged changes ─────────────────────────────┐
 2││ agent-terminal → main     ││ M  docs/design.md                              │
 3│└───────────────────────────┘│ ?? .gitignore                                  │
 4│┌─ Files ───────────────────┐│                                                │
 …
36│ Stage: <space> | Commit: c | Push: P | Keybindings: ?
refs:
  @t1 [panel]    "Status"            rows 1-3
  @t2 [panel]    "Unstaged changes"  rows 1-18
  @t3 [selected] "M  docs/design.md" row 2 col 32
  @t4 [hint]     "Commit: c"         row 36 col 19
```

## Dashboard

`agent-terminal dashboard start` serves `http://localhost:4849`. Same layout and visual language as the agent-browser dashboard:

- **Sessions sidebar**: sessions grouped with their tabs; each tab shows program, status (running · exited 0 · exited 1 · human control) and a live dot. New session, close all, theme.
- **Center**: a toolbar showing the command line and size, then the live terminal, drawn from the daemon's grid: for each frame the daemon sends the rows that changed, not the program's bytes, so the browser shows the screen the agent sees ([ADR 0002](adr/0002-dashboard-draws-the-daemons-screen.md)). xterm.js takes character widths from the daemon's tables: `dashboard/src/lib/widths.ts` is generated from the `unicode-width` crate the screen engine uses, and a test fails when it is stale. The terminal is drawn with the fonts screenshots use, served by the daemon from the binary, so icons and emoji look the same in both. xterm.js runs in screen-reader mode so the screen is readable as text (the browser tests and agents driving agent-browser read `.xterm-accessibility-tree`); its live region, which xterm.js fills with every printed character and empties only on a key press or blur, is emptied on every render, because a program that repaints without line feeds would otherwise grow it without bound and freeze the page within a minute (`docs/research/2026-10-15-fast-output-and-recording-budget.md`). Status bar: Live/Reconnecting, stream URL, size, modes.
- **Split view**: show 2 or 4 terminals side by side, so one human can watch several agents or the server and the TUI under test at once.
- **Right panel** tabs: **Activity** (every CLI call: command, duration, result, inline thumbnail for screenshots, failed waits in red), **Commands** (shell history from OSC 133: command, exit code, duration, cwd; click to see its output), **Screenshots** (gallery with diff), **Output** (raw stream with marks), **Info** (pid, cwd, TERM, env, modes).
- **Timeline**: scrub back through frames, which the daemon replays from the tab's recording and sends the same way; clicking an Activity or Commands row jumps the terminal to that moment. "Live" returns.
- **Take control**: a human takes the keyboard. While they hold it, agent input commands exit `4` with "A human has control of qa t1. Run `agent-terminal wait --control`."

## Skills shipped

See `skills/agent-terminal/SKILL.md` (stub), `skill-data/core/SKILL.md`, `skill-data/tui-qa/SKILL.md` and `skill-data/cli-qa/SKILL.md`.

## Open points

1. Standalone dashboard on `:4849`, or terminals as a second session kind inside the agent-browser dashboard on `:4848`? Draft: standalone, same look, so it ships on its own.
2. Should the dashboard have agent-browser's Chat tab? Draft: no, it is a watch-and-take-over tool.
3. Name of the binary: `agent-terminal` (and the short alias `at`?). Draft: `agent-terminal` only, since `at` clashes with the Unix `at`.
