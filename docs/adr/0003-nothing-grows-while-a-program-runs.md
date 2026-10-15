---
status: accepted
---

# Nothing grows while a program runs: no recording, screenshots are the evidence

The spec (46H-19) and the first-draft changelog in `docs/commands.md` have every tab recorded from open to close as an asciicast file in its session folder, "because it costs a few KB", and ADR 0002 has the dashboard's timeline replay that recording so a human can seek to any earlier frame. Instead, nothing is recorded (spec 46H-60). A terminal's steady-state cost is its screen (the daemon's grid), its frame counter and the bounded output log it already keeps, and no file grows while a program runs, whether anyone is watching or not. Evidence is what the agent explicitly captures, which is screenshots: `screenshot`, `wait --screenshot`, `diff screenshot` and the dashboard's screenshot button. The session folder keeps only what an agent or a human produced: the activity log, the screenshot list and the command logs. The per-tab asciicast writer, `record`, `clip`, the timeline, the daemon's replay of a recording and its per-frame index are removed, not capped.

We chose this because agent-terminal is meant to stay on for days as the terminal of up to fifty agents at once, and the recording made a terminal's cost grow with how long it had run. The numbers are in `docs/research/2026-10-15-fast-output-and-recording-budget.md` (sections 1 to 3 and 6). A recording grows 2.9 MB an hour for a tab that changes once a second, 217 MB an hour for repost's `/forward` view waiting for webhooks, 2.6 GB an hour for a full-screen program repainting at 60 Hz, and 53 GB an hour for twenty of them. Everything built on it inherited that growth: replaying the timeline took 34% of the daemon's busy time with 20 tabs; opening the timeline of a tab that had run 3.6 minutes sent 185 MB of JSON and took 5.7 s; the browser held 439 MB a minute after opening a tab that had run 13 minutes; the daemon's per-frame index grew 1.7 MB an hour per 60 Hz tab and was never trimmed. Without them, an unwatched 60 Hz terminal costs the daemon about 0.5% of a core for a repost-like program and 3% for a full-screen repaint, and a watched one adds 1.3 to 2.4%.

## Prior art

Read from source (research note, section 4). tmux keeps a bounded history per pane (2,000 lines by default), gives a slow client the latest screen instead of the missed output, and records nothing. asciinema records only when asked, one event per read, and applies its idle limit at playback. ttyrec writes one record per read, with no size limit, for the session it was started for. Zellij's session resurrection serialises layouts and commands, not output. None of them records every terminal all the time.

## Considered options

- **A ring buffer per tab**, keeping the last minutes of output for a short clip after something went wrong. Rejected: its cost still grows with activity up to the cap, and no cap fits both cases. The research note's proposal of 256 MiB of compressed output per tab holds about 2.4 hours of a repost-like tab but only about 6 minutes of a full-screen 60 Hz tab, and fifty tabs at that cap are 12.5 GiB of disk.
- **A compressed archive**: closed segments compressed with zstd, measured 18 to 33 times smaller at about 1.3 ms of one core per MB. Rejected: it slows the growth but does not stop it. Twenty full-screen tabs still write about 2.5 GB an hour.
- **A retention policy** that removes session folders by age, count or a global quota. Rejected: the quota the research note proposes (2 GiB) holds under three minutes of twenty full-screen tabs uncompressed, and the policy exists only to serve a replay the product does not need once screenshots are the evidence.

## Consequences

- Supersedes ADR 0002's timeline paragraph and its consequence that the daemon replays a tab's whole run for a timeline viewer. The live view still draws the daemon's screen, as ADR 0002 decided; it no longer replays a recording, and the browser holds only the screen it shows.
- Retracts the first-draft changelog's "every tab is recorded all the time (it costs a few KB)" in `docs/commands.md`.
- Supersedes the timeline and the record button of the dashboard prototype (`docs/prototype/dashboard-ui-2026-10-14.html`), which stays as it was ratified.
- Nothing shows what happened between two calls unless the agent captured it. The guides tell agents to take a screenshot for every finding and to name a `mark` before a step whose output they may want to read with `output --since`. Taking a screenshot automatically when `expect` or `wait --assert` fails is left to its own ticket.
- What scrolled off the screen stays readable through the bounded output log (`output`, `output --since`, the dashboard's Output tab) and, in shell tabs, the command log.
- A session folder holds a few KB of logs plus the screenshot list. It starts with the first tab and stays after `close` until the name is reused, as before; no retention policy is needed.
