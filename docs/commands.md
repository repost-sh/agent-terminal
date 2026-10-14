# agent-terminal commands

Draft for ratification. This is what `agent-terminal --help` prints. Same vocabulary as agent-browser on purpose.

```
agent-terminal - fast terminal automation CLI for AI agents

Usage: agent-terminal <command> [args] [options]

Start here (for AI agents):
  agent-terminal skills get core --full

Start and stop:
  open <program> [args...]     Run a program in a new terminal (prints the first stable screen)
                               --wait <text>  wait for this text instead of a stable screen
                               --wait-stable [ms]  how long unchanged counts as stable (default 300)
                               --no-wait  print the screen right away
  shell [--shell zsh|bash|fish]   Open your shell (default: $SHELL) with command history turned on
                               (prints the screen at its first prompt; takes --wait / --wait-stable / --no-wait like open)
  exec "<command line>"        Run a command in the shell tab, wait for it, print output + exit code
                               --check  exit with the command's own exit code
  restart                      Run the same program again with the same options (cwd, env, current size)
  signal <INT|TERM|KILL|...>   Send a signal to the program in the foreground (INT TERM KILL HUP TSTP CONT WINCH)
  resize <cols>x<rows>         Resize the terminal, e.g. resize 80x24 (the app gets SIGWINCH)
                               restart and resize print the screen and take --wait <text> / --wait-stable [ms] like open
  close [--all]                Close the current tab (--all closes every session)

Read the screen:
  snapshot                     The screen as text: header, numbered rows, cursor
                               header: session tab · program · size · alt-screen · mouse:<enc> · cursor r,c|hidden · frame n · exited [code]
  snapshot -i                  Same, plus @t refs for things on screen (panels, selected rows, buttons, links, key hints, the input)
                               each ref: @tN [kind] "label" row r col c (panels: rows a-b); it fails once its cells change
  snapshot --rows <a:b>        Only rows a to b; --scrollback [n] adds n lines (all if no n) above as rows -n..-1; --ansi keeps colors
  screenshot [path]            PNG of the screen (--annotate draws the refs)
  get text                     Screen text only, no header
  get <what>                   cursor, size, title, cwd, pid, exit-code, modes, frame, program
  get cell <row> <col>         One character and its style (color, bold, inverse...)
  find <text>                  Where the text is on screen (row,col); --regex, --all includes scrollback
  output [--since <mark|frame>]   What the program printed since a point (--raw keeps escape codes)
  history                      Commands run in the shell tab, with exit codes (history show <id> for output)
                               --last <n>  only the last n; --failed  only those that exited with a code other than 0
  mark <name>                  Name this point in the output, to read or clip from later

Send input:
  type <text>                  Type text, key by key (--delay <ms> between keys)
  paste <text>                 Paste text (bracketed paste when the app supports it)
  press <key...>               Enter, Tab, Esc, Up, F5, Ctrl+c, Alt+x, Shift+Tab; several keys run in order
                               --repeat <n>  press the whole sequence n times
                               keys follow the app's modes: application cursor keys, Kitty keyboard protocol
  click <@ref|row,col|text>    Mouse click (refused if the app has no mouse support; use keys instead)
                               --right  the right button; --double  a double click
  scroll <up|down> [n]         Scroll (mouse wheel if supported, else PageUp/PageDown; --keys always uses the keys)
  mouse <move|down|up|wheel>   Low-level mouse events: mouse move|down|up <row,col>, mouse wheel <up|down> <row,col>
                               down and up take --right or --middle
  send <bytes>                 Raw bytes with escapes (\e \xHH \n \r \t \0 \\), last resort

  Any input command also takes:
    --wait <text>              then wait until the text is on screen
    --wait-gone <text>         then wait until it is gone
    --wait-stable [ms]         then wait until the screen stops changing (default 300 ms)
    --snapshot                 then print the screen
    --screenshot [path]        then save a PNG

Wait and check:
  wait <text>                  Until the text is on screen (--regex, --gone, --rows <a:b>, --timeout <ms>)
  wait --stable [ms]           Until the screen stops changing (default 300)
  wait --idle [ms]             Until the program prints nothing, even output that leaves the screen as it was (default 300)
  wait --exit                  Until the program exits (--code <n> to require an exit code; another code exits 2)
  wait --cursor <r,c>          Until the cursor is at row r, column c
  wait --frame <n>             Until the screen reaches frame n
  wait <ms>                    Pause this long, whatever --timeout says (debugging only)
  wait --command               Until the shell command finishes
  wait --control               Until a human hands control back
  expect <text>                Fail now (exit 2) if the text is not on screen
  expect --gone <text>         Fail if it is on screen
  expect --exit-code <n>       Fail if the program did not exit with n
  expect --cell <r,c> --style <inverse|bold|fg=red|...>   Fail if that cell is not styled so

Evidence:
  clip <path> [--last 10s]     GIF or MP4 of a moment: --last <time>, --from/--to <mark|@activity>, --command <id>
  record start|stop [path]     Save the whole tab as an asciicast (.cast) file
  diff snapshot                What changed on screen since the last snapshot (--baseline <file>: a saved one)
  diff screenshot --baseline <png>   Pixel diff against a baseline image
  trace export <path>          Everything about this session in one file, for a bug report

Sessions and tabs:
  session                      Current session name
  session list                 Sessions, their tabs, programs and status
  session id --scope worktree --prefix <p>   A name unique to this checkout, for AGENT_TERMINAL_SESSION
  tab                          List tabs in this session
  tab new <program...>         Another terminal in the same session (e.g. a server next to the app)
  tab <id>                     Switch to a tab
  tab close [id]               Close a tab

Watch and take over:
  dashboard [start|stop]       The live dashboard on http://localhost:4849 (--port <n>; 0 picks a free port)
  attach [--read-only]         Watch or drive a session from your own terminal (detach: Ctrl-p Ctrl-q)

Everything else:
  batch [--bail] ["cmd" ...]   Several commands in one call (args or stdin)
  skills [list|get <name>|path]   Guides that ship with this version (core, tui-qa, cli-qa)
  mcp                          MCP stdio server with the same commands as tools
  doctor [--fix]               Check PTY, fonts, shell integration, daemon (--fix: stale socket, old integration scripts)
  install [--skills]           Install the agent skill stub into ~/.claude/skills
  upgrade                      Upgrade to the latest version
  version

Options:
  --session <name>             Use a named session (or AGENT_TERMINAL_SESSION)
  --tab <id>                   Act on a tab other than the current one
  --size <cols>x<rows>         Terminal size for open/shell/tab new (default 120x40)
  --cwd <dir>                  Working directory for open/shell/tab new
  --env <K=V>                  Extra environment variable (repeatable)
  --timeout <ms>               For waits (default 25000)
  --json                       JSON output
  --idle-timeout <time>        Stop an unused daemon after this long (default 1h, 0 = never; 30s, 10m, 1h or raw ms;
                               set by the call that starts the daemon)
  --screenshot-dir <path>      Where screenshots and clips go (or AGENT_TERMINAL_SCREENSHOT_DIR)

Exit codes:
  0  ok
  1  error
  2  expect or wait --assert failed
  3  timed out
  4  a human has control of this tab
  5  the program has exited
```

## What changed from the first draft

- Added `clip`. Every tab is recorded all the time (it costs a few KB), so the agent can cut a GIF or MP4 after something goes wrong, without starting a recording first.
- Added `session id --scope worktree`, copied from agent-browser, so parallel agents never share a session by accident.
- `mark set <name>` became `mark <name>`.
- Dropped: perf commands, a chat tab, `get` fields nobody asked for.
