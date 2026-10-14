# Glossary

**session**: a named workspace (`--session qa`, default `default`). Owns one or more terminals and isolates their history, recordings and screenshots.

**terminal**, also **tab**: one PTY running one program, addressed as `t1`, `t2`, … within its session. Say "terminal" for the thing and "tab" for its place in a session; never "pane" or "window".

**daemon**: the resident process that owns every terminal. The CLI starts it on first use and talks to it over a unix socket in the agent-terminal home folder.

**frame**: one version of a terminal's screen. The frame number goes up every time program output changes what the screen shows (text, colors, cursor); output that leaves the screen as it was makes no new frame.

**stable**: a screen whose frame has not changed for the quiet period (300 ms by default). A heuristic; when the expected content is known, wait for that text instead.

**idle**: a program that has printed nothing for the quiet period (300 ms by default), not even output that leaves the screen as it was.

**snapshot**: the screen as text: a header line, numbered rows, trailing blank rows folded. `--json` gives the same data as an object.

**ref**: `@t3`, a handle for an on-screen element found by `snapshot -i`. Valid until a later frame changes its cells.

**mark**: a named point in a terminal's output stream (`mark before-login`); `output --since before-login` reads from it.

**command**: one shell command line, delimited by OSC 133 marks. Has its text, cwd, exit code, start time, duration and output range. Not to be confused with a CLI call to agent-terminal, which is an **activity** entry.

**activity**: one CLI call to agent-terminal as recorded in the session's log: time, command as typed, duration and result.

**recording**: a tab's output, input and resizes as an asciicast file, written from the moment the tab opens. `record start` saves a copy of it from that point to a file of your choice.

**session folder**: the plain folder where a session keeps its recordings and activity log (`~/.agent-terminal/sessions/<session>/`); `trace export` zips it.

**control**: who may send input to a terminal: the agent (default) or a human who took it from the dashboard. While a human holds it, agent input exits 4.
