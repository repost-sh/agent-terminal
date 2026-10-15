---
name: core
description: Core agent-terminal usage guide. Read this before running any agent-terminal commands. Covers the open-snapshot-act-wait loop, reading the screen and its @t refs, sending keys, waiting correctly, shells and command history, taking screenshots and clips as evidence, sessions and tabs, human control, and troubleshooting. Use when asked to run, drive, test or screenshot a terminal program.
allowed-tools: Bash(agent-terminal:*), Bash(npx agent-terminal:*)
---

# agent-terminal core

agent-terminal runs programs in real terminals and lets you drive them the way a person would. You read the screen as text, press keys, wait for the screen to change, and keep screenshots and clips as proof. A human may be watching in the dashboard.

## The core loop

```bash
agent-terminal open lazygit --cwd ~/code/app   # 1. start the program; prints its first stable screen
agent-terminal snapshot -i                     # 2. read the screen, with @t refs
agent-terminal press 2 --wait-stable --snapshot # 3. act, wait, and see the result in one call
agent-terminal expect "Unstaged changes"       # 4. check what must be true
agent-terminal screenshot ./evidence/files.png # 5. keep proof
agent-terminal close                           # 6. close what you opened
```

Acting and waiting in the same call (`--wait…`, `--snapshot`) is the fastest way to work. One call per step, not three.

## Always use your own session

Before your first command, name a session for the whole task:

```bash
export AGENT_TERMINAL_SESSION="$(agent-terminal session id --scope worktree --prefix task)"
```

The default session is shared with every other agent on the machine. Working in it can type into someone else's program.

## Reading the screen

```bash
agent-terminal snapshot            # the screen as text
agent-terminal snapshot -i         # plus refs
agent-terminal snapshot --rows 30:40
agent-terminal get text            # just the text, no header
agent-terminal find "Error"        # where it is: row,col
agent-terminal get cell 4 3        # one cell and its style
agent-terminal output --since before-commit   # everything printed since a mark
```

A snapshot looks like this:

```
qa t1 · lazygit · 120x36 · alt-screen · mouse:sgr · cursor 2,33 · frame 212
 1│┌─ Status ──────────────────┐┌─ Unstaged changes ──────────────────┐
 2││ app → main                ││ M  docs/design.md                   │
 3│└───────────────────────────┘│ ?? .gitignore                       │
 …
36│ Stage: <space> | Commit: c | Push: P | Keybindings: ?
refs:
  @t1 [panel]    "Status"            rows 1-3
  @t2 [panel]    "Unstaged changes"  rows 1-18
  @t3 [selected] "M  docs/design.md" row 2 col 32
  @t4 [hint]     "Commit: c"         row 36 col 19
```

The header tells you the size, whether the app took over the whole screen (`alt-screen`), whether it accepts the mouse, where the cursor is, and the `frame` number. The frame goes up every time the screen changes.

Refs are guesses made from what is on screen: box titles (`panel`), highlighted rows (`selected`), `[ OK ]` and `< Yes >` buttons, OSC 8 links, key hints, the input under the cursor. Each one says where it is: `row r col c` is where a click lands. A ref stays valid while the cells it was found on are unchanged; once they change, using it fails and tells you to take a new `snapshot -i`.

Use `snapshot` (text) to understand and decide. Use `screenshot` only for proof, or when colors and layout matter and text cannot show them.

## Sending input

```bash
agent-terminal type "hello world"
agent-terminal press Enter
agent-terminal press Ctrl+c
agent-terminal press g g                 # several keys in order
agent-terminal press Down --repeat 5
agent-terminal paste "$(cat notes.txt)"  # paste, not typed key by key
agent-terminal click @t3                 # only if the header says mouse:...
agent-terminal click 4,12 --right        # a cell (row,col); --double for a double click
agent-terminal click "Save"              # the first match of the text
agent-terminal scroll down 3             # mouse wheel if the app takes the mouse, else PageDown
agent-terminal mouse down 4,3            # low level, e.g. a drag: down, move, up
agent-terminal mouse move 4,20
agent-terminal mouse up 4,20
```

Most terminal apps are driven by keys, not the mouse. Read the key hints the app shows (often on the last row) and use them. `click` and `mouse` refuse when the app has no mouse support; that is not a bug in your command. `scroll --keys` presses PageUp/PageDown even when the app takes the mouse.

Key names: `Enter`, `Tab`, `Shift+Tab`, `Esc`, `Backspace`, `Delete`, `Up`, `Down`, `Left`, `Right`, `Home`, `End`, `PageUp`, `PageDown`, `F1`–`F12`, `Space`, and any letter with `Ctrl+`, `Alt+`, `Shift+`.

## Waiting (read this)

Most failures come from bad waits. After every key that changes the screen, wait for the right thing:

```bash
agent-terminal press Enter --wait "Saved"        # best: wait for the text you expect
agent-terminal press q --wait-gone "lazygit"     # wait for something to disappear
agent-terminal press j --wait-stable             # when you don't know what will appear
agent-terminal wait --exit --code 0              # the program should finish
agent-terminal wait --command                    # a shell command should finish
```

Prefer waiting for specific text. Use `--wait-stable` when you cannot know the text. Never use `wait <ms>` except when debugging: it is slow and flaky. Waits time out after 25 s by default (`--timeout`); a timeout exits 3 and prints the screen it saw.

## Shells and command history

For plain commands, use a shell tab and `exec`. It runs the command, waits for it, and gives you the output and the exit code:

```bash
agent-terminal shell --cwd ~/code/app
agent-terminal exec "cargo test" --check   # exits with the command's exit code
agent-terminal history --failed            # which commands failed
agent-terminal history show 3              # the output of command 3
```

`shell` opens your `$SHELL` (zsh, bash or fish; `--shell` picks one) with shell integration, which is what records each command's exit code, duration, cwd and output. A tab opened with `open` has none, so `exec` and `history` refuse there. `exec` waits for a command that is still running before it types the next one; when `exec` times out, the command keeps running: `wait --command` waits for it.

Use `type` + `press Enter` only when you need to test the typing itself, such as shell completion or a prompt.

## Checking

```bash
agent-terminal expect "Welcome"
agent-terminal expect --gone "Loading"
agent-terminal expect --exit-code 0
agent-terminal expect --cell 2,33 --style inverse   # this row is the selected one
```

`expect` checks the current screen without waiting and exits 2 with the relevant part of the screen when it fails. Wait first, then expect. To wait and check in one call, add `--assert` to `wait`: running out of time then exits 2 like a failed `expect`, instead of 3 (`agent-terminal wait "Saved" --timeout 5000 --assert`).

## Evidence

```bash
agent-terminal screenshot ./evidence/step-3.png
agent-terminal screenshot --annotate ./evidence/refs.png  # refs drawn on the image
agent-terminal diff screenshot --baseline ./evidence/step-3.png  # exits 2 if any pixel changed (--threshold 0.01 allows 1%)
agent-terminal mark before-commit
agent-terminal clip ./evidence/commit.gif --from before-commit
agent-terminal clip ./evidence/crash.gif --last 10s
agent-terminal clip ./evidence/build.mp4 --command 3      # one shell command from `history`, as MP4
agent-terminal record start ./evidence/login.cast         # save the tab as an asciicast from here on
agent-terminal record stop                                # prints the file, its length and size
agent-terminal trace export ./evidence/session.zip        # everything, for a bug report
```

Every tab is recorded from the start, so you can cut a clip after something went wrong. Name a `mark` before a step you may want to show.

## Sizes

Terminal apps break at sizes their authors never tried. The default is 120x40. Try the small and the large:

```bash
agent-terminal open myapp --size 80x24
agent-terminal resize 200x60 --wait-stable   # prints the screen at the new size
```

## Several programs at once

```bash
agent-terminal open pnpm dev                 # tab t1
agent-terminal tab new myapp-tui             # tab t2, now the current tab
agent-terminal wait "ready on" --tab t1      # act on t1 without switching to it
agent-terminal tab t1                        # switch
agent-terminal restart --tab t1              # run the server again after a fix
```

Tabs are numbered `t1`, `t2`, … in the order they open. `agent-terminal tab` lists them; `session list` lists every session.

## When a human takes control

A person can take control of a tab, from the dashboard or with `agent-terminal attach` in their own terminal. Your input commands (`type`, `press`, `paste`, `send`, `signal`, `click`, `scroll`, `mouse`) and `exec` then exit 4 with "A human has control of …". Reading the screen still works. Do not retry in a loop:

```bash
agent-terminal wait --control --timeout 600000   # resumes when they hand it back (the default timeout is 25 s)
```

## Finishing

Close what you opened: `agent-terminal close`, or `agent-terminal close --all` at the end of the task.

## Troubleshooting

- **Nothing appears after `open`.** Check `agent-terminal get exit-code` and `output`: the program may have exited at once (exit 5 on the next command).
- **Keys do nothing.** The app may want different keys in its current mode; check `get modes`, and read its own key hints. Try `--wait-stable` instead of a fixed text.
- **Text wraps strangely.** The terminal is too narrow: `resize 160x40`.
- **The screen is a mess of symbols.** Fonts in screenshots are bundled, but a missing glyph shows as `□`; run `agent-terminal doctor`.
- **A wait times out.** Read the screen it printed. The text may differ in case or spacing; use `--regex`.

## Full reference

`agent-terminal skills get core --full` adds the complete command list. `agent-terminal <command> --help` for one command.
