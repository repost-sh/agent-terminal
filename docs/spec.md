# agent-terminal v1 — spec

## Problem Statement

Agents can test web apps well: agent-browser gives them a fast CLI to open a page, read it as text with refs, act, wait, screenshot, and lets a human watch every session live in a dashboard. Nothing comparable exists for terminal programs.

When an agent has to QA a TUI (lazygit-style apps, chat TUIs, installers, wizards, REPLs) or an interactive CLI, it falls back to `tmux send-keys`, `expect`, or tools that only print text. That breaks in predictable ways:

- The agent cannot reliably tell when the screen has finished changing, so it sleeps and guesses, and tests are slow and flaky.
- It cannot see what a person sees: colors, highlights, layout, the selected row. There is no screenshot, and no clip of what happened.
- It cannot tell whether a shell command failed, because it reads the screen, not the exit code.
- A human who wants to watch has to attach to tmux sessions one by one, cannot rewind, cannot see what the agent typed, and cannot safely take over without the agent typing over them.
- Existing tools (orca terminals, agent-tui) cover part of this: they create terminals and send keys, but lack PNG screenshots, refs, command history, clips, and a dashboard in the agent-browser style.

## Solution

`agent-terminal`: a single fast binary that is to terminals what agent-browser is to web pages, with the same vocabulary so an agent that knows one knows the other.

- An agent runs any program in a real terminal (`open`, `shell`), reads the screen as text with numbered rows, a status header and `@t` refs (`snapshot -i`), sends keys, text, pastes and mouse events, and waits for exactly the right moment (text appears, text disappears, screen stable, program exits, shell command finishes) — usually in the same call as the input.
- It checks what must be true (`expect`), takes PNG screenshots, and cuts GIF or MP4 clips of anything that already happened, because every tab is recorded from the start.
- In shell tabs, every command is captured with its exit code, duration, working directory and output, so the agent (and the human) has a reliable command history.
- A human runs `agent-terminal dashboard` and sees every session live in a browser, in the same layout as the agent-browser dashboard: sessions and tabs on the left, the live terminal in the middle, and Activity, Commands, Screenshots, Output and Info on the right. They can scrub back in time, watch up to four terminals at once, and take control of a tab; while they hold it the agent's input is refused with a clear exit code until they hand it back.
- The binary ships its own skills (`core`, `tui-qa`, `cli-qa`), always matching the installed version, plus an MCP server with the same commands as tools.
- It installs with `npm i -g agent-terminal` (prebuilt binary per platform, no compile step), and runs on macOS, Linux and Windows.

## User Stories

### Starting and stopping programs

1. As an agent, I want to start any program in a real terminal with one command, so that I can test it the way a user runs it.
2. As an agent, I want `open` to print the program's first stable screen, so that I can act without a separate read call.
3. As an agent, I want to choose the terminal size when I open a program, so that I can test at the sizes users have.
4. As an agent, I want to set the working directory and extra environment variables at open, so that I can run the program against a specific repo or config.
5. As an agent, I want to open my shell with command tracking turned on, so that I can run commands and know their exit codes.
6. As an agent, I want `exec` to type a command in a shell tab, wait for it to finish, and give me its output and exit code, so that running plain commands takes one call.
7. As an agent, I want `exec --check` to exit with the command's own exit code, so that I can chain it in scripts.
8. As an agent, I want to restart a program with the same options, so that I can retest after a fix without retyping the command.
9. As an agent, I want to send a signal (INT, TERM, KILL, HUP, TSTP, CONT, WINCH) to the running program, so that I can test interrupt and suspend handling.
10. As an agent, I want to resize the terminal mid-session, so that I can test how the app reflows.
11. As an agent, I want to close a tab or every session, so that I leave nothing running.
12. As an agent, I want a clear exit code when the program I am driving has already exited, so that I stop sending input into nothing.

### Sessions and tabs

13. As an agent, I want named sessions, so that my terminals never mix with another agent's.
14. As an agent, I want `session id --scope worktree --prefix <p>` to give me a name unique to my checkout, so that parallel agents in different worktrees never collide.
15. As an agent, I want several tabs in one session, so that I can run a dev server next to the TUI under test.
16. As an agent, I want to target a tab without switching to it, so that I can wait on the server while driving the app.
17. As an agent, I want to list sessions with their tabs, programs, sizes and status, so that I can see what is running.
18. As an agent, I want an unused daemon to stop itself after a configurable idle time, so that forgotten sessions do not run forever.

### Reading the screen

19. As an agent, I want `snapshot` to give me the screen as text with numbered rows, so that I can reason about layout cheaply.
20. As an agent, I want the snapshot header to show session, tab, program, size, alt-screen, mouse mode, cursor position and frame number, so that I know the app's state at a glance.
21. As an agent, I want trailing blank rows folded, so that snapshots cost fewer tokens.
22. As an agent, I want `snapshot -i` to list refs for selected rows, buttons, panels, links, key hints and the input under the cursor, so that I can name what I act on.
23. As an agent, I want refs to say which frame they belong to and to be rejected once that part of the screen changed, so that I never click something that moved.
24. As an agent, I want to snapshot only a range of rows, or include scrollback, so that I read just what I need.
25. As an agent, I want a snapshot with colors kept, so that I can see styling when it matters.
26. As an agent, I want `get text` without the header, so that I can pipe screen text into other tools.
27. As an agent, I want `get` for cursor, size, title, cwd, pid, exit code, modes, frame and program, so that I can check one fact cheaply.
28. As an agent, I want `get modes` to tell me alt-screen, mouse tracking, bracketed paste, application cursor keys, focus events and keyboard protocol flags, so that I can debug why keys behave differently.
29. As an agent, I want `get cell` to give a character and its style, so that I can verify highlights and error colors.
30. As an agent, I want `find` to give me row and column of text or a regex, optionally including scrollback, so that I can locate things before clicking or asserting.
31. As an agent, I want `output --since <mark>` to give me everything printed since a point, with `--raw` to keep escape codes, so that I can read line-oriented output that scrolled away.
32. As an agent, I want to name a point in the output with `mark`, so that I can read or clip from it later.
33. As an agent, I want `history` to list shell commands with id, command, cwd, exit code and duration, and `--failed` to filter, so that I know what failed without reading the screen.
34. As an agent, I want `history show <id>` to print one command's output, so that I can read a failure in full.
35. As an agent, I want `diff snapshot` to show what changed since my last snapshot, so that I can see the effect of a key without rereading everything.

### Sending input

36. As an agent, I want to type text key by key, with an optional delay, so that I can test apps that drop fast input.
37. As an agent, I want to paste text as a real paste when the app supports bracketed paste, so that I can test paste handling.
38. As an agent, I want to press named keys and chords (Enter, Tab, Shift+Tab, Esc, arrows, F-keys, Ctrl+, Alt+, Shift+), and sequences like `g g`, with repeat, so that I can drive any keyboard UI.
39. As an agent, I want keys encoded correctly for the app's current mode (application cursor keys, Kitty keyboard protocol), so that what I press is what the app receives.
40. As an agent, I want to click a ref, a cell, or the first match of a text, including right and double click, so that I can drive mouse-enabled TUIs.
41. As an agent, I want `click` to refuse with an explanation when the app has no mouse support, so that I switch to keys instead of thinking the app is broken.
42. As an agent, I want to scroll with the wheel when the app tracks the mouse and with PageUp/PageDown otherwise, so that scrolling just works.
43. As an agent, I want low-level mouse move, down, up and wheel events, so that I can test drag and hover.
44. As an agent, I want to send raw bytes as a last resort, and see it flagged in the activity log, so that unusual sequences are possible but visible.
45. As an agent, I want every input command to accept `--wait <text>`, `--wait-gone <text>`, `--wait-stable [ms]`, `--snapshot` and `--screenshot`, so that act-wait-read is one call.

### Waiting and checking

46. As an agent, I want `wait <text>` (with regex, gone, row range and timeout), so that I continue exactly when the UI is ready.
47. As an agent, I want `wait --stable` to resolve when no frame changed for a given time, so that I can wait when I do not know what will appear.
48. As an agent, I want `wait --idle` to resolve when the program produced no output for a given time, so that I can wait on quiet programs that redraw nothing.
49. As an agent, I want `wait --exit` with an optional required code, so that I can test that a program finishes correctly.
50. As an agent, I want `wait --command` to resolve when the running shell command finishes, so that I can wait on long builds.
51. As an agent, I want `wait --cursor` and `wait --frame`, so that I can synchronize precisely when text is not enough.
52. As an agent, I want waits to wake on the next matching frame, not on a polling interval, so that tests are as fast as the app.
53. As an agent, I want a timeout to exit with its own code and print the screen it last saw, so that I can see why it failed.
54. As an agent, I want `expect` for text, absence of text, exit code and cell style, exiting 2 with the relevant screen excerpt, so that assertions are explicit and fail fast.

### Evidence

55. As an agent, I want PNG screenshots rendered from the terminal grid with a bundled font, so that screenshots work on headless machines and look the same everywhere.
56. As an agent, I want `screenshot --annotate` to draw ref boxes with a legend, so that a vision model or human can match refs to what they see.
57. As an agent, I want screenshot options for scale, theme, cursor on/off, row crop, and SVG, HTML, text and ANSI output, so that I get the format I need.
58. As an agent, I want screenshots to default to a named file in a configurable folder, so that evidence lands in a predictable place.
59. As an agent, I want every tab recorded from the moment it opens, so that I never have to remember to start a recording.
60. As an agent, I want `clip` to cut a GIF or MP4 for the last N seconds, between two marks or activity entries, or for one shell command, so that I can show exactly what went wrong after the fact.
61. As an agent, I want clips to skip unchanged frames and cap the frame rate, so that they are small and quick to produce.
62. As an agent, I want MP4 clips to work without ffmpeg installed, so that evidence never depends on the machine's setup.
63. As an agent, I want `record start/stop` to save a tab as an asciicast file, so that I can share a replayable, searchable recording.
64. As an agent, I want `diff screenshot --baseline` to produce a pixel diff image with a threshold, so that I can catch visual regressions.
65. As an agent, I want `trace export` to bundle the session's calls, frames, commands, screenshots and recording, so that a bug report has everything.

### Human watching and control

66. As a developer, I want `agent-terminal dashboard` to serve a live dashboard on a fixed local port, so that I can watch agents work.
67. As a developer, I want the dashboard to look and behave like the agent-browser dashboard, so that I already know how to use it.
68. As a developer, I want the sidebar to group tabs by session with program name and a status (running, exited with code, human control), so that I see the state of everything at once.
69. As a developer, I want the toolbar to show the tab's command line and working directory, with restart, screenshot, record, split view and Take control buttons, so that common actions are one click.
70. As a developer, I want the live terminal rendered faithfully in the browser at its real size, so that I see exactly what the agent sees.
71. As a developer, I want a status bar with live/reconnecting state, stream address, size, modes and frame, so that I know whether what I see is current.
72. As a developer, I want an Activity tab listing every CLI call with time, command, duration and result, with failures and refusals highlighted and screenshots as thumbnails, so that I can follow the agent's reasoning.
73. As a developer, I want Activity to follow new entries and offer "jump to latest" when I scroll up, so that I can read history without losing the live view.
74. As a developer, I want a Commands tab with every shell command, its exit status, duration and cwd, expandable to its output, so that I can see what ran and what failed.
75. As a developer, I want a Screenshots tab with a gallery, a lightbox, copy path, and compare two screenshots with changed cells highlighted, so that I can review evidence.
76. As a developer, I want an Output tab with the raw stream, marks as dividers, a toggle to show escape sequences, and search, so that I can debug rendering problems.
77. As a developer, I want an Info tab with program, args, pid, cwd, TERM, size, shell integration, modes, recording state, start time and environment with secrets masked, so that I can understand the setup without leaking tokens.
78. As a developer, I want a timeline under the terminal that I can scrub, with ticks for agent calls, commands and screenshots, so that I can rewind to any moment.
79. As a developer, I want clicking an Activity or Commands entry to show the terminal at that moment, and a Live button to return, so that cause and effect are easy to connect.
80. As a developer, I want input disabled while I view the past, so that I never type into the present by mistake.
81. As a developer, I want to take control of a tab and type into it from the dashboard, so that I can help or unblock the agent.
82. As a developer, I want a clear visual state while I hold control and a notice when the agent tried to type, so that I know the agent is waiting on me.
83. As an agent, I want my input refused with exit code 4 and a message while a human holds control, and `wait --control` to resume me, so that I never fight the human for the keyboard.
84. As a developer, I want my own keystrokes logged in Activity in a distinct style, so that the audit trail of the tab is complete.
85. As a developer, I want a split view of two or four terminals from any sessions, with a picker for empty cells, so that I can watch several agents or a server and its client together.
86. As a developer, I want to start a new session or tab from the dashboard (name, command, cwd, size, shell integration), with the equivalent CLI line shown live and copyable, so that I learn the CLI from the UI.
87. As a developer, I want clear refusals in that dialog (name taken, empty command, command not found), so that I can correct mistakes immediately.
88. As a developer, I want "Close all sessions" to confirm how many sessions and running processes it will kill, so that I do not lose work by accident.
89. As a developer, I want an exited tab to keep its last screen visible with the exit code and Restart / Close tab actions, so that I can see how it ended.
90. As a developer, I want an empty state that shows the three commands to get started, and a different one when the daemon is not running, so that the dashboard is never a blank page.
91. As a developer, I want light and dark themes, so that the dashboard matches my setup.
92. As a developer, I want `attach` to watch or drive a session from my own terminal, read-only or not, with a detach key, so that I do not need a browser.

### Skills, MCP, setup

93. As an agent, I want `skills get core` to give me the usage guide that matches the installed binary, so that instructions are never stale.
94. As an agent, I want `skills get tui-qa` and `skills get cli-qa` checklists with report templates, so that I can run a complete QA pass without inventing the process.
95. As a developer, I want `install --skills` to put a small discovery stub in my agent's skills folder, so that agents find the tool on their own.
96. As a developer, I want an MCP server exposing the same commands as tools, so that MCP-only clients can use it.
97. As an agent, I want `batch` to run several commands in one call, with `--bail` to stop on the first failure, so that scripted flows are fast.
98. As an agent, I want `--json` on every command, so that I can parse results reliably.
99. As a developer, I want `doctor` to check PTY support, fonts, shell integration and the daemon, and `--fix` stale state, so that setup problems are easy to solve.
100. As a developer, I want `npm i -g agent-terminal` to install a prebuilt binary for my platform with no compiler, so that installation never fails.
101. As a developer, I want `upgrade` to move me to the latest version, so that staying current is one command.
102. As a developer on Windows, I want the same commands and dashboard to work, so that I can QA terminal apps on Windows too.
103. As a developer, I want each CLI call to return in a few milliseconds and the daemon to stay small in memory, so that hundreds of calls in a QA run stay fast and cheap.

## Implementation Decisions

**Product decisions (previously open, now decided)**

- Binary and package name: `agent-terminal`. No short alias (`at` clashes with the Unix `at`).
- The dashboard is standalone on port 4849, visually a sibling of the agent-browser dashboard. It is not merged into agent-browser.
- The dashboard has no Chat tab.
- Human keystrokes are logged in Activity as human entries.
- The timeline scrubber is part of v1.
- Windows is in v1. MP4 export without ffmpeg is in v1.

**Command surface**

- The command reference in the repo's docs is the contract: every command, option, default and exit code listed there ships. Exit codes: 0 ok, 1 error, 2 assertion failed, 3 timeout, 4 human has control, 5 program has exited.
- Every command supports `--json`, `--session`, `--tab`. Waits default to a 25 s timeout. Default size is 120x40. The session can come from `AGENT_TERMINAL_SESSION`.

**Architecture**

- One Rust binary contains the CLI, the daemon and the dashboard server. The CLI auto-starts the daemon on first use; the daemon exits after the idle timeout.
- CLI ↔ daemon: a Unix domain socket (a named pipe on Windows) carrying length-prefixed JSON request/response messages. The same message types back the CLI, the MCP server and the dashboard API, so all three have the same capabilities.
- Modules, each with a narrow interface:
  - **Pty**: spawn a program with size, cwd and env; read, write, resize, signal, exit status. Implemented with `portable-pty` (Unix PTYs and Windows ConPTY), behind an internal interface.
  - **Screen engine**: feed bytes; expose the grid, cursor, modes, title, cwd and scrollback; encode keys and mouse events for the current modes. Implemented with `alacritty_terminal` behind an internal interface, so the engine can be replaced without touching anything else.
  - **Terminal**: owns one Pty plus one screen engine; counts frames; keeps an in-memory scrollback of about 2,000 lines; appends every output chunk, input event and resize to the tab's recording on disk; publishes frame events that waits subscribe to.
  - **Shell integration**: small startup scripts for zsh, bash, fish and PowerShell that emit prompt and command marks (OSC 133), cwd (OSC 7) and title, injected when a shell tab opens. The Terminal turns these marks into command records (id, command, cwd, start, end, exit code, output range).
  - **Refs**: a pure function from a screen grid to a list of refs (kind, label, position, frame). Kinds: panel, selected, button, link, hint, input.
  - **Snapshot formatter**: grid + refs → the documented text format (header line, numbered rows, folded blank rows, refs list) and its JSON form.
  - **Renderer**: grid → PNG, SVG, HTML, text or ANSI. Uses bundled JetBrains Mono plus a Nerd Font symbols fallback, `swash` for glyphs, `tiny-skia` for drawing, with a glyph cache. Supports themes, scale, crop, cursor and annotation overlays.
  - **Clips**: replays a slice of a recording through a fresh screen engine, renders frames, drops unchanged ones, caps frame rate at 10–15 fps, encodes GIF with the `gif` crate and MP4 with embedded `openh264`. No AGPL encoders.
  - **Session store**: plain files per session in the user's agent-terminal folder: the recording (asciicast v2), an activity log (JSON lines), the command log, screenshots and clips. `trace export` zips this folder.
  - **Control**: per-tab owner (agent or human). Input commands check it and fail with exit 4 while a human holds it; the refusal is logged in Activity.
  - **Dashboard server**: `axum` HTTP + WebSocket, serving a web app built into the binary. Per tab it sends the current screen when a viewer connects, then the live byte stream; for the timeline it serves keyframes plus the recording, so the browser can seek.
  - **Dashboard web app**: Vite + React + Tailwind, styled after the agent-browser dashboard; terminals rendered with xterm.js and its WebGL addon. Screens and states per the dashboard UI prototype.
  - **MCP server**: `rmcp`, one tool per command, same arguments as the CLI.
  - **Skills**: Markdown built into the binary, served by `skills list/get/path`; `install --skills` writes the discovery stub.
- Waits subscribe to frame, output, exit, command and control events from the Terminal; they never poll.
- Speed targets: CLI round trip ≤ 5 ms, `snapshot` ≤ 10 ms, PNG ≤ 40 ms at 120x40. Memory targets: daemon ≤ 10 MB idle, about 5 MB per active tab.

**Distribution**

- `cargo-dist` builds release binaries for macOS (arm64, x64), Linux (arm64, x64, glibc and musl) and Windows (x64). An npm package with one optional dependency per platform (the agent-browser and agent-tui pattern) plus a Homebrew tap.

## Testing Decisions

- A good test drives the product the way a user does and checks what the user would see: command output, exit codes, files written, and what the dashboard shows. Tests never reach into internal modules or private state. A test that would break when we swap the screen engine or PTY library for an equivalent one is testing the wrong thing.
- **Seam 1 — the CLI.** Black-box tests run the real `agent-terminal` binary against fixture programs and assert on stdout, JSON output, exit codes and produced files. Fixtures:
  - a small purpose-built ratatui app with panels, a selectable list, buttons, mouse support that can be turned off, a resize-sensitive layout, a slow action, and a crash key;
  - a plain line-printing program with controllable exit codes and timing;
  - real shells (zsh, bash, fish, PowerShell on Windows) for `shell`, `exec`, `history` and `wait --command`.
  Snapshot text and JSON are compared with `insta` snapshots. PNG output is checked for dimensions and a pixel tolerance against stored baselines. Clips are checked for frame count, duration and format.
- **Seam 2 — the dashboard.** agent-browser drives the real dashboard in a real browser against a real daemon running the fixtures: sessions appear, Activity fills as the test runs CLI calls, Commands shows exit codes, Take control refuses the agent with exit 4, the timeline shows past frames, split view shows the chosen terminals. Assertions use agent-browser snapshots and screenshots.
- Every user story above maps to at least one test at one of the two seams.
- CI runs both seams on macOS, Linux and Windows, plus the speed targets as a benchmark job that fails on regression.
- Prior art: none in this repo yet (it is new). The model is agent-browser's own approach (CLI-level tests and its dashboard), and the QA skills in this repo double as manual test scripts.

## Out of Scope

- Performance profiling of the app under test (latency tracing, flame graphs, Perfetto output). Explicitly dropped by the owner.
- A natural-language Chat tab in the dashboard.
- Merging terminals into the agent-browser dashboard.
- Controlling terminals that agent-terminal did not start (attaching to an existing tmux pane or another terminal app's window).
- Remote access to the dashboard from other machines beyond what binding to localhost and a user-run reverse proxy give.

## Further Notes

- The dashboard UI is specified by the HTML prototype dated 2026-10-14 in the repo's prototypes folder. Once the owner ratifies it, the prototype is normative for UI: labels, copy, states, dialogs and focus behavior carry into the implementation 1:1, and where this spec and the prototype differ on UI, the prototype wins.
- The skills (`core`, `tui-qa`, `cli-qa`) and the discovery stub are already drafted in the repo and ship as written, updated to match the final command behavior.
- Prior art that informed this design: agent-browser (vocabulary, dashboard, skills, packaging), agent-tui (text screenshots, `wait --stable`), orca terminals, Microsoft tui-test and ptywright (xterm-based TUI testing), asciinema/agg (cast to GIF), Charm freeze (terminal images).
