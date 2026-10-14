# The session folder

Every session keeps plain files in `<home>/sessions/<session>/`, where `<home>` is `AGENT_TERMINAL_HOME` or `~/.agent-terminal`. The dashboard, clips and `trace export` read them; nothing else writes them.

```
~/.agent-terminal/
  sessions/qa/
    activity.jsonl     every CLI call to the session, one JSON object per line
    t1.cast            the recording of tab t1 (asciicast v2), from open to close
    t1.commands.jsonl  the command log of shell tab t1, one JSON object per finished command
    t2.cast
  recordings/          `record start` files when no path is given: <session>-<tab>-<frame>.cast
  screenshots/         screenshots and clips when no path and no --screenshot-dir is given: <session>-<tab>-<frame>.png
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
  - `m`: a mark set with `mark <name>`; the data is the name. `clip --from <name>` starts at the last mark of that name.
- Times count from `timestamp`, not from the first event, so `timestamp + seconds` is the wall-clock time of the event and lines up with the activity log.
- Replaying the `o` and `r` events through a fresh terminal of the header's size gives the screen the tab showed.
- `restart` keeps writing to the same file: it adds an `o` event with a terminal reset (`ESC c`) before the new program's output, because the restarted program starts on a fresh screen.

## Activity log: `activity.jsonl`

One line per CLI call addressed to the session, written before the CLI gets its answer:

```json
{"time":"2026-10-14T14:01:22.051Z","tab":"t1","command":"press 2 --wait 'Unstaged changes'","duration_ms":38,"exit":0,"frame":19}
{"time":"2026-10-14T14:01:31.110Z","tab":"t1","command":"wait Pushed --timeout 5000","duration_ms":5003,"exit":3,"error":"timed out after 5000 ms waiting for ...","frame":176}
{"time":"2026-10-14T14:01:46.400Z","tab":"t1","command":"screenshot","duration_ms":30,"exit":0,"frame":212,"screenshots":["/home/me/.agent-terminal/screenshots/qa-t1-212.png"]}
```

- `time`: when the daemon received the call, UTC to the millisecond.
- `tab`: the tab the call acted on (the one it opened, for `open` and `tab new`); left out for calls about the whole session (`tab`, `close --all`, `trace export`).
- `command`: the arguments as typed, without the binary name, quoted for a POSIX shell the way `batch` prints them. A `batch` logs each command it runs as an entry of its own, in the session that command addresses.
- `duration_ms`: from receiving the call to answering it, in the daemon.
- `exit`: the CLI's exit code (0 ok, 1 error, 3 timeout, 5 exited, ...); `error`: the message when it is not 0.
- `frame`: the tab's frame when the call ended, when the tab still exists.
- `screenshots`: absolute paths of the files the call saved: the screenshot of `screenshot` or of an input command's `--screenshot`, the diff image of `diff screenshot`. Left out when the call saved none. The files live where the call put them, outside the session folder.
- `clips`: absolute path of the GIF or MP4 a `clip` call saved, in a list like `screenshots`. Left out for every other call. `clip --from @activity:N` counts the lines of this file from 1: it starts when entry N was received, and `--to @activity:N` ends when it was answered (`time` + `duration_ms`, both cut to the millisecond, so the clip runs up to 2 ms past it).
- Not logged: calls that name no session (`session list`, `skills`, `daemon stop`) and calls to a session that is not open.

## Command log: `<tab>.commands.jsonl`

Only tabs opened with `shell`, which have shell integration. One line per command, written when the command finishes, in the order the commands finish:

```json
{"id":2,"command":"cargo test","cwd":"/Users/me/agent-terminal","started_at_ms":1791999590006,"ended_at_ms":1791999597418,"duration_ms":7412,"exit_code":101,"first_frame":170,"last_frame":231,"output":"   Compiling agent-terminal v0.1.0\n...\nerror: test failed\n"}
```

- Each line is what `history show <id> --json` prints for the command, without `session` and `tab`.
- `id`: the command's number in `history`. `restart` starts the history again at 1 and keeps appending to the same file, so after a restart an id can appear twice; the earlier line is from before the restart.
- `started_at_ms`, `ended_at_ms`: wall-clock Unix time in milliseconds, so `started_at_ms / 1000 - timestamp` is where the command starts in the tab's recording. `duration_ms`: from start to finish.
- `exit_code`: what the shell reported; `null` when it started a new prompt without reporting one.
- `first_frame`, `last_frame`: the tab's frame when the command started and after its last output.
- `output`: what the command printed, as text without escape codes, exactly as `history show` prints it. A tab keeps the last 4 MiB of its output, so the output of a command that printed more starts with a line saying how many bytes are no longer kept.
- A command still running when its tab closes is not in the file.

## `trace export <path>`

A zip of the folder with every entry under `<session folder name>/`. Recordings, the activity log and the command logs are copied up to their last complete line, so an export taken while a program prints never holds half an event. The `trace export` call itself is logged after the zip is written, so it is not in the zip.
