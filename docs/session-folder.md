# The session folder

Every session keeps plain files in `<home>/sessions/<session>/`, where `<home>` is `AGENT_TERMINAL_HOME` or `~/.agent-terminal`. The dashboard, clips and `trace export` read them; nothing else writes them.

```
~/.agent-terminal/
  sessions/qa/
    activity.jsonl     every CLI call to the session, one JSON object per line
    t1.cast            the recording of tab t1 (asciicast v2), from open to close
    t2.cast
  recordings/          `record start` files when no path is given: <session>-<tab>-<frame>.cast
```

## Lifetime

- The folder starts when the session's first tab opens. A session that starts again under the same name (after its last tab closed, or in a new daemon) starts from an empty folder: the previous run's files are removed.
- The folder stays after the session ends, so `trace export` still works after `close`, until the name is used again.
- The folder name is the session name with every byte outside `A-Z a-z 0-9 - _ .` written as `%XX`, and a leading `.` too (`../x` becomes `%2E.%2Fx`); the empty name is `%`.

## Recordings: `<tab>.cast`

[asciicast v2](https://docs.asciinema.org/manual/asciicast/v2/), so `asciinema play t1.cast` replays it.

- Header: `{"version":2,"width":120,"height":40,"timestamp":1791999575,"command":"lazygit --debug"}`. Width and height are the size the tab opened at; `timestamp` is whole Unix seconds.
- Events: `[seconds, "o"|"i"|"r", data]`, appended as they happen, one per line.
  - `o`: program output as it reached the screen, UTF-8 (a character split across reads is written whole; invalid bytes become U+FFFD).
  - `i`: input agent-terminal wrote to the program (`type`, `press`, `paste`, `send`). Signals are not input.
  - `r`: a resize, `"COLSxROWS"`.
- Times count from `timestamp`, not from the first event, so `timestamp + seconds` is the wall-clock time of the event and lines up with the activity log.
- Replaying the `o` and `r` events through a fresh terminal of the header's size gives the screen the tab showed.
- `restart` keeps writing to the same file: it adds an `o` event with a terminal reset (`ESC c`) before the new program's output, because the restarted program starts on a fresh screen.

## Activity log: `activity.jsonl`

One line per CLI call addressed to the session, written before the CLI gets its answer:

```json
{"time":"2026-10-14T14:01:22.051Z","tab":"t1","command":"press 2 --wait \"Unstaged changes\"","duration_ms":38,"exit":0,"frame":19}
{"time":"2026-10-14T14:01:31.110Z","tab":"t1","command":"wait \"Pushed\" --timeout 5000","duration_ms":5003,"exit":3,"error":"timed out after 5000 ms waiting for ...","frame":176}
```

- `time`: when the daemon received the call, UTC to the millisecond.
- `tab`: the tab the call acted on (the one it opened, for `open` and `tab new`); left out for calls about the whole session (`tab`, `close --all`, `trace export`).
- `command`: the arguments as typed, without the binary name; arguments with spaces or shell characters are double-quoted.
- `duration_ms`: from receiving the call to answering it, in the daemon.
- `exit`: the CLI's exit code (0 ok, 1 error, 3 timeout, 5 exited, ...); `error`: the message when it is not 0.
- `frame`: the tab's frame when the call ended, when the tab still exists.
- Not logged: calls that name no session (`session list`, `skills`, `daemon stop`) and calls to a session that is not open.

## `trace export <path>`

A zip of the folder with every entry under `<session folder name>/`. Recordings and the activity log are copied up to their last complete line, so an export taken while a program prints never holds half an event. The `trace export` call itself is logged after the zip is written, so it is not in the zip.
