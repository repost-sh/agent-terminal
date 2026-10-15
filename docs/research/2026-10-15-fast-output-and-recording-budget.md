# Fast output and the recording budget

Research note, 2026-10-15. Status: findings and a proposal for review. Nothing in sections 5 and 6 is implemented.

Question: one program that redraws about 60 times a second (repost's `/forward` view) made the dashboard unusable and its recording heavy. agent-terminal is becoming a multiplexer for many concurrent agent sessions. Where exactly does it break, what does every consumer of a tab's output cost, and what budget should the design meet?

Machine for every number below: Apple M4 Max, 14 cores, 36 GiB, macOS 26.4, release build of this branch (`cargo build --release --workspace --bins`), headless Chrome for Testing 151 driven by agent-browser 0.39.0. CPU percentages are of one core (`ps -o time`), so 100% is one full core.

## Summary

- **The dashboard breaks in the browser, not in the daemon, the WebSocket or the row-diff stream.** xterm.js runs with `screenReaderMode: true` (`dashboard/src/lib/xterm.ts:60`). Its accessibility manager appends every printed character to an `aria-live` region (`dashboard/node_modules/@xterm/xterm/src/browser/AccessibilityManager.ts:139-158`, `:202-207`) and only empties it on a key press in xterm or a blur (`:115-116`, `:160-163`), neither of which happens in a read-only dashboard terminal. Line feeds stop the growth after 20 lines (`:15`, `:150-155`), but a full-screen TUI moves the cursor instead of printing line feeds, so the region grows forever. With the 60 Hz fixture it held 417,000 characters after 30 s and 1.29 million after 70 s. The browser's long tasks rose from 139 to 1,132 ms per second, painting fell from 50 to 6 frames a second, and after about 5 minutes a single `eval` took more than 120 s. Emptying that region every 250 ms (a page-side prototype, nothing changed in the repo) and changing nothing else gave a steady 60 paints a second, no long tasks, 2 to 16 ms from key press to echo, and a timeline scrub of 17 to 46 ms.
- **Second, slower failure: the timeline.** Every viewed tab also streams every frame into a timeline the browser keeps for the whole run (`dashboard/src/lib/timeline.ts:62`). For a full-screen 60 Hz tab the browser heap grew about 27 MB a minute (63 to 156 MB in 3.5 min). Opening the timeline of a tab that had run 3.6 minutes made the daemon replay the whole recording: 185 MB of JSON, 5.7 s, 5.8 s of CPU. In the 20-tab profile, replaying the timeline took 34% of the daemon's busy time.
- **repost itself was measured** (section 1): its `/forward` view redraws at 29 frames a second, not 60, writing 217 MB an hour to the recording and 329 KiB a second to the dashboard's live stream, and it froze the dashboard the same way the fixture does (0.2 paints a second after a minute). With the live region emptied it runs at 60 paints a second.
- **Recording is lossless, uncompressed and unbounded.** A repost-like partial repaint at 60 Hz writes 173 MB an hour per tab, and a full-screen repaint 2.6 GB an hour. Twenty such tabs write 3.5 GB or 53 GB an hour. `docs/commands.md:148` says recording "costs a few KB". zstd level 3 shrinks these recordings 21 to 33 times at about 1.3 ms per MB, so lossless capture can stay the default.
- **The daemon is cheap without viewers.** 20 tabs at 60 Hz cost 10% of a core (partial repaint) or 58% (full screen). Viewers dominate: the same 20 full-screen tabs cost 118% with a live stream each and 183% with the dashboard's live stream plus timeline each.

## 1. Reproducing the dashboard failure

### repost itself

`which repost` gives `/Users/schulzf/.local/bin/repost` (Repost 1.6.0, a chat TUI). `repost forward` without `--headless` refuses and points at the interactive view: `✗ repost forward needs --headless: the interactive forward view is the chat TUI's /forward (run repost, then type /forward)`. Without credentials `repost` stops at a login screen (`Login with Repost  l`, `Quit  q`); typing `/forward` there opens a token prompt that says `A browser window will open for authentication.` The animated screen was reached with the user's own `~/.repost` (approved), the daemon's home still under `.superpowers/`:

```sh
AT=cli/target/release/agent-terminal; export AGENT_TERMINAL_HOME=$PWD/.superpowers/ath
$AT --session rp2 open --size 120x40 --cwd $PWD/.superpowers/rp-cwd -- env HOME=/Users/schulzf repost
$AT --session rp2 type "/forward"; $AT --session rp2 press Enter      # Forward Session Creation, step 1/2: select bucket
$AT --session rp2 press Enter                                          # bucket nuvera-dev; step 2/2: select forwarders
$AT --session rp2 press Space; $AT --session rp2 press Enter           # toggle forwarder schulzfel@mbp (localhost:5000), start
```

The resulting screen is `Live forwards … Waiting for webhooks...`, framed on the left by a `┃` bar whose grey shimmers down all 40 rows. That bar is the animation: every frame rewrites the 40 bar cells with cursor moves and truecolor SGR, about 840 bytes, inside synchronized output. Measured over 13.5 minutes with no webhook traffic (`.superpowers/ath/sessions/rp2/t1.cast`, 45.9 MB, 43,802 output events; the fixture used below copies this byte shape):

| | repost `/forward`, 120x40 | `redraw --hz 60 --partial` (fixture) |
|---|---|---|
| Frames a second (`get frame`, 60 s) | 28.8 | 60 |
| Output events a second in the recording | 53.9 | 60 |
| Bytes an event, median / max | 837 / 1,025 | about 670 |
| Program bytes a second | 47 KB | 40 KB |
| Recording growth | 3.62 MB a minute, **217 MB an hour** (203 MB an hour over the whole run) | 173 MB an hour |
| Daemon CPU, no viewer (`ps -o time`, 60 s) | 0.8% of a core, RSS 9.4 MB, 12 fds | 0.6%, 7 MB |
| Dashboard live stream | 28.8 messages, **329 KiB a second** | 60 messages, 78 KiB a second |

repost redraws at 29 frames a second, not 60, but its frames are heavier for the dashboard than the fixture's: every frame changes all 40 rows (one cell each), and the live stream sends whole changed rows (ADR 0002), so each message carries 40 rows of SGR text, 4.2 times the bytes of the fixture's 7-row logo.

The dashboard failure reproduced with repost exactly as with the fixture (`.superpowers/bench/browser-run.sh` on `/s/rp2/t1`, after the tab had run about 9 minutes; `.superpowers/bench/browser-rp-asis.jsonl` and `browser-rp-clearlive.jsonl`):

| Variant | Window | Paints/s | Long tasks ms/s | Longest frame gap | Heap MB | Live region | Scrub ms |
|---|---|---|---|---|---|---|---|
| repost, as is | 13 s | 24.3 | 593 | 1.2 s | 121 | 1,474,200 | |
| | 26 s | 9.5 | 513 | 2.6 s | 131 | 2,669,400 | |
| | 44 s | 3.6 | 717 | 4.3 s | 150 | 3,825,000 | |
| | 60 s | 0.2 | 1,495 | 8.2 s | 165 | 3,886,200 | none of 5 returned |
| repost, live region emptied | each of 4 | 56 to 60 | 0 | 58 ms | 291 → 376 | 0 | 13 to 31 |

The live region is already 1.5 million characters 13 s after the page opens, because the timeline catches up on the whole run first (11 MB a second of timeline frames in the first window) and every replayed frame also feeds the region. Emptying the region restores 60 paints a second, no long tasks and a 13 to 31 ms scrub, with the same 329 KiB a second live stream; the browser's heap then shows the second problem alone: 291 MB after the catch-up, 439 MB after a minute, because the timeline keeps every frame of a 13-minute run. The daemon spent 13.6 s of CPU during the first run and 29.3 s during the second, almost all of it replaying the recording for the timeline (the tab alone costs 0.8% of a core).

### The fixture: `redraw`

`cli/tests/fixtures/src/bin/redraw.rs`, new:

- `redraw --hz N --partial|--full [--stats FILE]`. It switches to the alternate screen, prints `ready`, then draws N frames a second on a fixed schedule, each wrapped in synchronized output.
- `--partial` is repost-like: a 7-row braille logo whose truecolor shade changes every frame, plus a spinner and status line. About 670 bytes a frame, close to repost's 835.
- `--full` rewrites all 40 rows of coloured words, shifted every frame. About 8.4 KB a frame.
- Every byte it reads is echoed as `seen N.` on row 1, so key-to-echo latency can be measured.
- `--stats` keeps `frames bytes` written in a file for the harness.

### Measurement method

Everything below is reproducible:

- `.superpowers/bench/probe.js` is an agent-browser init script. It wraps `WebSocket.prototype.onmessage` to count messages and bytes per endpoint and to time the handler. It also records `requestAnimationFrame` deltas, long tasks (`PerformanceObserver`), event-loop lag (a `MessageChannel` ping every 50 ms), `performance.memory.usedJSHeapSize` and the live-region length.
- Key-to-echo latency: dispatch a keydown on the focused, controlled terminal, wait for the stream message that carries `seen N+1.`, then wait one animation frame.
- Scrub latency: set the timeline range, wait two animation frames.
- The URL hash switches the prototypes on: `#clear-live` empties `.live-region` every 250 ms, and `#no-timeline` stubs out the `/timeline` WebSocket.
- Commands, with every home under `.superpowers/`:

```sh
cargo build --release --workspace --bins            # from cli/
. .superpowers/env.sh                                # HOME, TMPDIR, AGENT_BROWSER_SOCKET_DIR, AGENT_TERMINAL_HOME under .superpowers/
F=$PWD/cli/target/release
.superpowers/bench/variants.sh "p60-asis:x;$F/redraw --hz 60 --partial" "p60-clearlive:clear-live;$F/redraw --hz 60 --partial" \
  "p60-notimeline:no-timeline;$F/redraw --hz 60 --partial" "p60-clearlive-notimeline:clear-live,no-timeline;$F/redraw --hz 60 --partial" \
  "f60-asis:x;$F/redraw --hz 60 --full" "f60-clearlive:clear-live;$F/redraw --hz 60 --full" "p1-asis:x;$F/redraw --hz 1 --partial"
WIN=45 NWIN=4 .superpowers/bench/variants.sh "f60-heap:clear-live;$F/redraw --hz 60 --full"
```

Each variant opens a fresh 120x40 tab, opens `/s/rd/t1` in a 1440x900 headless browser, discards 3 s, then reads four 10 s windows (`.superpowers/bench/browser-<label>.jsonl`). After that it takes control and measures 5 echoes and 5 scrubs.

### Results: one viewed tab

The window column is the time the page had been open when the window ended. "Live region" is the characters in xterm's `aria-live` region.

| Variant | Window | Paints/s | Long tasks ms/s | Longest frame gap | Heap MB | Live region | Stream msgs/s, KiB/s | Echo ms | Scrub ms |
|---|---|---|---|---|---|---|---|---|---|
| idle busy-screen (baseline) | 3 windows | 30 to 60* | 0 | 34 ms | 5 to 10 | 0 | 0.6, 0.4 | 18 to 34 | 17 to 37 |
| 60 Hz partial, as is | 13 s | 50.3 | 139 | 316 ms | 15 | 417,067 | 57.7, 75 | | |
| | 24 s | 19.1 | 638 | 1.05 s | 15 | 709,207 | 56.2, 74 | | |
| | 36 s | 10.9 | 713 | 1.84 s | 26 | 1,086,127 | 53.1, 70 | | |
| | 46 s | 6.3 | 1,133 | 2.96 s | 22 | 1,290,247 | 45.1, 59 | none returned | none returned |
| 60 Hz partial, first manual run | 66 s | 3.8 | 751 | 6.35 s | 51 | | 44.2, 58 | | |
| 60 Hz partial, live region emptied | each of 4 | 60.0 | 0 | 17.7 ms | 14 to 33 | 0 | 60, 78 | 1.6 to 10.3 | 17 to 41 |
| 60 Hz partial, **the fix built in** (`dropAnnouncementsOnceDrawn`, no page-side script; `browser-p60-fixed.jsonl`) | each of 4 | 60.0 | 0 | 17.7 ms | 15 to 42 | 0 | 60, 78 | 7.5 to 12.9 | 26 to 33 |
| 60 Hz partial, no timeline, as is | 46 s | 0.7 | 877 | 2.9 s | 15 | 1,280,259 | 53, 69 | 2 of 5 returned | none returned |
| 60 Hz partial, emptied, no timeline | each of 4 | 60.0 | 0 | 17.7 ms | 8 to 19 | 0 | 60, 78 | 3.5 to 16 | 19 to 32 |
| 60 Hz full, as is | 13 s → 46 s | 43.1 → 13.9 | 250 → 776 | 0.4 → 1.2 s | 53 → 92 | 3.7 M → 10.4 M | 57 → 33, 793 → 463 | none returned | none returned |
| 60 Hz full, emptied | each of 4 | 59.1 to 60 | 0 | 42 ms | 27 → 77 | 0 | 60, 832 | 2 to 16 | 26 to 41 |
| 60 Hz full, emptied, 3.5 min | 48 s → 3.5 min | 59 | 0 | | 63 → 137 → 157 | 0 | 60, 833 | 4 to 16 | 20 to 33 |
| 1 Hz partial, as is | 13 s → 46 s | 60 | 0 | 18 ms | 7 | 7,504 → 23,584 | 1, 1.3 | 3 to 17 | 20 to 46 |
| 20 tabs at 60 Hz, one viewed, emptied | each of 4 | 60 | 0 | | 15 to 43 | 0 | 60, 78 | 6 to 15 | 18 to 41 |

\* Headless Chrome paints at 30 a second while nothing on the page changes and at 60 once something does; the idle baseline shows both.

Where it breaks, layer by layer:

- **Daemon producing a row diff per frame: fine.** For the one viewed 60 Hz tab the daemon used 1.8% of a core (`.superpowers/bench/daemon-rd60-viewer.jsonl`) and 1.0% without the browser. With 19 more busy tabs and one viewed it used 9.9% (`daemon-20tabs-1viewed.jsonl`).
- **WebSocket: fine.** 60 messages and 78 KiB a second for the live stream and the same again for the timeline. The browser spent under 1.2 ms a second in all `onmessage` handlers together, and the longest handler took 0.3 ms. When the page fell behind, the live stream sent fewer messages (60 down to 44 a second) because `follow_tab` awaits each send and reads the latest picture after it (`cli/src/native/dashboard/stream.rs:71-93`, `changes.rs:15-17`). It already coalesces to the latest frame.
- **xterm.js applying the diffs: fine on its own.** With the live region emptied, the same diffs paint at 60 a second with no long tasks, for both partial and full repaints.
- **The accessibility live region: breaks it.** It is the only thing removed between the failing and the healthy runs. The cost grows with the region's length, not with the frame rate. xterm refreshes its accessibility tree, and appends to the region, at most once a second (`TimeBasedDebouncer.ts:6`, `:49-64`). Each append re-sets `textContent` of an ever longer string. At 1 Hz the region grew 536 characters a second, so by extrapolation (not measured) a 1 Hz tab reaches the 417,000 characters that already cost 139 ms a second after about 13 minutes. Slow tabs only take longer to get there. Caveat: agent-browser drives Chrome over CDP with accessibility snapshots, which may make Chrome's accessibility work heavier than in a desktop browser with no screen reader. The user's own report that the dashboard became unusable fits the same mechanism.
- **Timeline replay: costs memory and daemon CPU, not responsiveness.** Turning the timeline off with the live region still growing did not help (0.7 paints a second at 46 s). With the region emptied, the timeline cost nothing visible in a minute. But it keeps every frame: the heap grew about 27 MB a minute for a full-screen tab (63 → 157 MB in 3.5 min), which projects to about 1.6 GB an hour. Catching up a 3.6-minute run costs the daemon 5.7 s and the browser 185 MB of JSON (`.superpowers/bench/catchup.mjs`, run as `node .superpowers/bench/catchup.mjs ws://localhost:$PORT/api/session/rd/t1/timeline $(agent-terminal --session rd get frame)` → `{"frames":13056,"msgs":51,"MB":185.4,"firstMs":119,"caughtUpMs":5658}`).

Key-to-echo latency in the failing runs: none of the 5 probes returned within the 25 s agent-browser call limit, or 2 of 5 returned (17 and 61 ms) while the page was still between long tasks. Note that the accessibility tree is refreshed at most once a second, so tests that read the screen from `.xterm-accessibility-tree` see it up to 1 s late. The first echo probe, which read that tree, measured about 680 to 980 ms for exactly this reason. The numbers above come from the stream instead.

### The browser test: failing before the fix, passing after

`cli/tests/cli/browser/fast_output.rs` (new, registered in `cli/tests/cli/browser/mod.rs`): `a_tab_that_redraws_sixty_times_a_second_leaves_the_dashboard_responsive_and_on_the_latest_frame`.

- **Setup:** opens `redraw --hz 60 --partial`, opens the dashboard on it and waits for the screen. After 20 s it measures 5 s in the page.
- **Assertions:**
  - at least 30 paints a second;
  - at most 200 ms a second of long tasks;
  - the status bar's frame no more than 60 frames (1 s) behind `agent-terminal get frame`.
- **It failed against the dashboard as it was.** `cargo test --features browser-tests --test cli browser::fast_output` (test folders under `cli/target/tmp`, agent-browser state under `.superpowers/`): `after 20s of a 60 Hz tab the dashboard painted 3.0 times a second (at least 30), its main thread was blocked 747 ms a second (at most 200), and it showed frame 1682 while the tab was at frame 1839 (at most 60 behind)`. The same 5-second script run by hand earlier (`.superpowers/bench/measure.js`) gave 2.4 paints, 751 ms blocked, 80 frames behind.
- **The fix.** `dropAnnouncementsOnceDrawn` in `dashboard/src/lib/xterm.ts` empties xterm's `.live-region` on every `onRender`, so the region holds at most one debounce window (up to 1 s) of announced characters instead of the whole run. Screen-reader mode stays on: the rows tree is what the browser tests and agents read. Nothing else changed.
- **It passes with the fix,** and so do the other 48 browser tests, which read the screen through the same accessibility tree: `cargo test --features browser-tests --test cli browser::` → `49 passed; 0 failed`, 41.9 s.

## 2. Consumers of a tab's output

Every PTY read (up to 64 KiB, `cli/src/native/terminal.rs:261`) goes through `Terminal::output` (`terminal.rs:293-339`) under the tab's state lock. A *frame* is a read that changes the screen's fingerprint (`terminal.rs:161-169`). Repaint programs like repost write one frame per read, so reads ≈ frames here; a `cat` of a big file is many reads per frame.

| # | Consumer | Where | Runs | CPU | Memory | Disk / network |
|---|---|---|---|---|---|---|
| 1 | Recording `.cast` writer | `recording.rs:129-136`, `:99-111` | every read | UTF-8 decode, JSON encode, one unbuffered `write(2)` per read | none | 1.18x program bytes (partial) to 1.43x (full): every ESC becomes `\u001b`; plus about 20 bytes per event. Never capped, never compressed. |
| 2 | VT grid (alacritty) | `screen.rs:457-461` | every read | parse | 120x40 grid + 2,000 scrollback lines (`screen.rs:428`); about 9 MB per tab with scrollback full (`CONTRIBUTING.md`, "Speed and memory") | |
| 3 | Second parser for printed text, styled text and shell reports | `screen.rs:459`, `take_printed` `:562` | every read | parse again | | |
| 4 | Output log (Output panel, `output`, command output) | `output_log.rs:84-93` | every read | copy into three `VecDeque<u8>` | raw, text and styled, each capped at 4 MiB (`output_log.rs:3`): up to 12 MiB per tab | |
| 5 | Frame detection: fingerprint of every cell | `screen.rs:538-556`, `terminal.rs:161-169` | every read | O(rows×cols) hash; 7% of the daemon's busy time in the 20-tab profile | | |
| 6 | Frame index `frame_ends` | `terminal.rs:166` | every frame | | 8 bytes a frame, never trimmed: 1.7 MB an hour per 60 Hz tab, 35 MB an hour for 20; cloned whole by `capture_at` (`:364`) and `continued` (`:465`) | |
| 7 | Waits and conditions | `terminal.rs:171-183`, `:633-659` | every read while a wait is pending | judge each watch | | |
| 8 | Wake-ups | `terminal.rs:328-332` | every read, changed or not | `Condvar::notify_all` and a `watch` tick that wakes every follower below | | |
| 9 | Live stream per viewer | `dashboard/stream.rs:71-120` | per frame, latest-wins | `Picture::of` builds all rows as SGR strings under the state lock (`terminal.rs:75-88`, `:451-454`), compares rows, serializes JSON. 26% of busy time in the profile. | one `Shown` (rows) per viewer | 1.5 KB (partial) to 14 KB (full) per frame per viewer |
| 10 | Timeline per viewer | `dashboard/timeline.rs:51-131` | every frame, none skipped | Replays the recording in a second `Screen` from the start of the run, `Picture::of` and paint for each frame, a full repaint every 32nd frame (`:17`, `:59`). 34% of busy time in the profile. On connect it replays the whole run. | | Same as the live stream again; the browser keeps all of it |
| 11 | Output panel stream | `dashboard/details.rs:187-230` | wakes on every read | copies new bytes of the output log | | raw + styled bytes since last send |
| 12 | Info panel stream | `dashboard/details.rs:242-293` | wakes on every read | rebuilds the environment and JSON on every wake, sends on change | | |
| 13 | `attach` mirror | `daemon.rs:760-790` | per frame | `Picture::of` | | to the attached terminal |
| 14 | Past screens (`capture_at`) | `terminal.rs:361-380` | on demand | replays the run from its start: O(run) | clones `frame_ends` | reads the cast |
| 15 | Clips | `clips/mod.rs:192-204`, `:206-241` | on demand | replays from the start of the cast | holds every event up to the clip's end in memory | reads the cast |
| 16 | `record start` copy | `recording.rs:175-191`, `:211-216` | every read while on | the same encode | | a second full write |
| 17 | `trace export` | `store.rs:325` | on demand | | | copies the cast into the zip |

Browser side, per viewed tab:

- the xterm accessibility live region (above);
- the timeline array of every frame (`dashboard/src/lib/timeline.ts:62`);
- a React state update per live frame (`dashboard/src/lib/stream.ts:44-46`) and per timeline message (`timeline.ts:63`).

Bytes per second land in the cast (1, 16), the output log (4) and the parsers (2, 3). Frames per second land in the frame index (6), the viewers (9, 10, 13) and the browser. Wake-ups land per read (8, 11, 12).

Where the daemon's time goes: `sample` of the daemon for 10 s with 20 full-screen 60 Hz tabs and a dashboard viewer (live + timeline) each, 182% CPU. The binary was built with symbols into `.superpowers/target-prof`. Inclusive samples, of about 18,200 busy (`.superpowers/bench/daemon-sample.txt`, `python3 -I .superpowers/bench/inclusive.py`):

| Part | Samples | Share |
|---|---|---|
| Timeline replay (`timeline::Cursor`) | 6,168 | 34% |
| Live stream `Terminal::picture` | 4,756 | 26% |
| `Terminal::output`, all of the per-read work | 4,086 | 22% |
| … `Screen::feed`, both parsers | 1,895 | 10% |
| … `Screen::fingerprint` | 1,239 | 7% |
| … `Recording::output` | 640 | 3.5% |

## 3. Measurements at scale

Harness: `at-perf load` (`cli/perf/src/load.rs`, wired in `cli/perf/src/main.rs`; adds `tungstenite` to `cli/perf/Cargo.toml`). For every combination it:

- starts a fresh daemon in its own home under `--work`;
- opens N sessions with one 120x40 `redraw` tab each;
- for `live`, connects one `/stream` WebSocket per tab; for `dashboard`, one `/stream` and one `/timeline` per tab (what the dashboard opens for a viewed tab);
- waits 2 s, then samples over `--seconds`:
  - program bytes and frames (the fixture's stats file);
  - the daemon's frames (`get frame`), CPU (`ps -o time`), RSS (`ps -o rss`) and fds (`lsof`);
  - the `.cast` growth;
  - the bytes and messages the WebSockets received.

One command, from `cli/`:

```sh
cargo build --release --workspace --bins && target/release/at-perf load --agent target/release/agent-terminal \
  --fixtures target/release --work ../.superpowers/bench/load --out ../.superpowers/bench/load.json
```

Defaults: 10 s per combination, rates 1,30,60,120 Hz, 1,10,20 tabs, partial and full repaint, viewers none, live and dashboard: 72 combinations in about 20 minutes. It prints a Markdown table and writes `load.json`. Narrow it with `--rates`, `--tabs`, `--repaints`, `--viewers`, `--seconds`.

Profiling tools: neither `samply` nor `cargo flamegraph` is installed, and none was installed. The profile above used macOS `sample`.

Results (`.superpowers/bench/load.md`, 10 s windows; `.cast MB/hour` extrapolates the measured window):

| Rate | Repaint | Tabs | Viewer | Program KiB/s | Daemon frames/s | Daemon CPU % | RSS MB | fds | .cast MB/hour | Stream KiB/s | Timeline KiB/s |
|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 Hz | partial | 20 | none | 13 | 20 | 0.2 | 16 | 87 | 57 | 0 | 0 |
| 1 Hz | partial | 20 | dashboard | 13 | 20 | 0.8 | 21 | 155 | 57 | 30 | 31 |
| 30 Hz | partial | 20 | none | 396 | 600 | 7.2 | 37 | 87 | 1,734 | 0 | 0 |
| 30 Hz | partial | 20 | dashboard | 397 | 600 | 22.1 | 35 | 155 | 1,734 | 899 | 953 |
| 60 Hz | partial | 1 | none | 40 | 60 | 0.6 | 7 | 11 | 174 | 0 | 0 |
| 60 Hz | partial | 1 | live | 40 | 60 | 1.3 | 11 | 20 | 174 | 90 | 0 |
| 60 Hz | partial | 1 | dashboard | 40 | 60 | 2.4 | 12 | 22 | 173 | 90 | 95 |
| 60 Hz | partial | 10 | none | 396 | 600 | 6.1 | 31 | 47 | 1,734 | 0 | 0 |
| 60 Hz | partial | 10 | dashboard | 395 | 600 | 21.4 | 39 | 85 | 1,735 | 898 | 954 |
| 60 Hz | partial | 20 | none | 780 | 1,200 | 10.2 | 52 | 87 | 3,468 | 0 | 0 |
| 60 Hz | partial | 20 | live | 780 | 1,200 | 19.7 | 51 | 115 | 3,467 | 1,797 | 0 |
| 60 Hz | partial | 20 | dashboard | 785 | 1,200 | 45.2 | 68 | 155 | 3,468 | 1,797 | 1,907 |
| 120 Hz | partial | 20 | none | 1,566 | 2,399 | 21.6 | 88 | 87 | 6,942 | 0 | 0 |
| 120 Hz | partial | 20 | dashboard | 1,569 | 2,401 | 66.8 | 102 | 155 | 6,947 | 3,597 | 3,817 |
| 1 Hz | full | 20 | none | 164 | 20 | 2.0 | 26 | 87 | 876 | 0 | 0 |
| 30 Hz | full | 20 | none | 4,959 | 600 | 34.0 | 272 | 87 | 26,420 | 0 | 0 |
| 60 Hz | full | 1 | none | 502 | 60 | 3.5 | 29 | 11 | 2,644 | 0 | 0 |
| 60 Hz | full | 1 | live | 501 | 60 | 10.1 | 22 | 20 | 2,643 | 833 | 0 |
| 60 Hz | full | 1 | dashboard | 505 | 60 | 11.9 | 23 | 22 | 2,639 | 832 | 837 |
| 60 Hz | full | 20 | none | 9,849 | 1,200 | 58.0 | 133 | 87 | 52,852 | 0 | 0 |
| 60 Hz | full | 20 | live | 9,805 | 1,200 | 117.7 | 284 | 115 | 52,838 | 16,654 | 0 |
| 60 Hz | full | 20 | dashboard | 9,833 | 1,199 | 182.5 | 276 | 155 | 52,815 | 16,623 | 16,730 |
| 120 Hz | full | 20 | none | 19,563 | 2,399 | 109.3 | 491 | 87 | 105,655 | 0 | 0 |
| 120 Hz | full | 20 | dashboard | 19,559 | 2,399 | 363.3 | 489 | 155 | 105,662 | 25,593 | 33,465 |

All 72 rows are in `.superpowers/bench/load.md`.

Readings:

- **Frames are not dropped anywhere in the daemon.** Daemon frames/s equals program frames/s in every row, up to 2,400 frames a second. `wait` and `snapshot` see every frame today and nothing below changes that.
- **Disk grows with bytes, linearly.**
  - Partial: 48 KB/s of cast per 40 KiB/s of output (1.18x).
  - Full: 734 KB/s per 502 KiB/s (1.43x).
  - Per-event overhead is small: 60 events a second × about 20 bytes = 1.2 KB/s, 2.5% of the partial cast. Coalescing events would save little.
- **CPU without viewers is per byte and per frame:**
  - 0.5% of a core per partial 60 Hz tab (10.2% / 20);
  - 2.9% per full 60 Hz tab (58% / 20).
- **CPU with viewers is per frame per viewer:**
  - Partial 60 Hz: +0.5% per tab for the live stream and +1.3% for the timeline.
  - Full 60 Hz: +3% per tab for the live stream and +3.2% for the timeline.
  - A viewer roughly doubles a busy tab's cost, and the dashboard (live + timeline) roughly triples it.
- **RSS:**
  - 7 MB with one partial tab, 52 MB with 20 after 12 s.
  - 146 MB after about 3.5 minutes for 20 partial tabs with one viewer (`daemon-20tabs-1viewed.jsonl`), as the per-tab output logs (up to 12 MiB, item 4) and scrollback fill.
  - Full-screen rows vary between 133 and 491 MB with allocator behaviour.
  - On top of that, `frame_ends` grows without bound (1.7 MB an hour per 60 Hz tab).
- **File descriptors:** 4 per tab (PTY master, cast, and more), 1.5 per WebSocket: 87 for 20 tabs, 155 with 40 sockets. `launchctl limit maxfiles` on this machine gives a soft limit of 256 for launchd-started processes (`ulimit -n` in this shell: 1048576). 20 viewed tabs use 155, so a daemon started by launchd with about 35 viewed busy tabs would hit that limit.

Compression of the recording, from `zstd -T1 -<level>` over `.superpowers/bench/partial30.cast` (30 s of partial 60 Hz, 1.45 MB) and the first 50 MB of a full 60 Hz cast:

| Cast | zstd -1 | zstd -3 | zstd -9 | zstd -19 |
|---|---|---|---|---|
| partial 60 Hz | 34.5x, 36 ms | 32.6x, 39 ms | 61.6x, 38 ms | 79.8x, 315 ms |
| full 60 Hz, 50 MB | 20.8x, 62 ms | 21.3x, 63 ms | 55.0x, 126 ms | 198.6x, 4.97 s |

Caveat: the fixtures repeat with a short period (64 shades, 8 words), so these ratios are an upper bound for synthetic output. The real repost `/forward` recording (45.9 MB, 13.5 minutes, `.superpowers/ath/sessions/rp2/t1.cast`) compresses 17.9x at level 3 (2.57 MB, 80 ms, 1.7 ms per MB) and 21.6x at level 9 (2.13 MB, 630 ms): a real animation lands close to the fixture, because an animation by nature repeats its own bytes. Even at a tenth of these ratios, compression shrinks the recording 2 to 3 times at about 1.3 to 1.7 ms of one core per MB at level 3.

## 4. Prior art, from source

Shallow clones in `.superpowers/prior-art/` (read only, nothing built or run). Paths are relative to each clone.

### tmux (`30cf692c2241440efad9a3212443a45aaa719a10`)

High-rate output:
- A pane does one read per event-loop pass and then stops reading: `window.c:1711-1712` `input_parse_pane(wp); bufferevent_disable(wp->event, EV_READ);`.
- Reading is turned back on in `server_client_check_pane_buffer` (`server-client.c:2003-2006`).
- Parsing only marks damage. Redraw happens once per loop per client (`server-client.c:1795-1800`).

Slow viewers:
- The backlog limit is 8 bytes per cell: `tty.c:76-78`, `TTY_BLOCK_INTERVAL (100000 /* 100 milliseconds */)`, `TTY_BLOCK_START(tty) (1 + ((tty)->sx * (tty)->sy) * 8)`, `TTY_BLOCK_STOP(tty) (1 + ((tty)->sx * (tty)->sy) / 8)`.
- Past the limit, `tty_block_maybe` (`tty.c:223-248`) **discards the queued output** (`evbuffer_drain(tty->out, size); c->discarded += size;`) and keeps discarding (`tty.c:663-665`).
- Every 100 ms it decides whether to unblock and then **redraws the whole client** (`tty.c:201-219`, `c->flags |= CLIENT_ALLREDRAWFLAGS;`). A slow viewer gets the latest screen, not the missed stream.
- Redraw is deferred while output is still queued, with a 1 ms retry timer (`server-client.c:2383`, `:2414`, `:2421-2422`).
- Control-mode clients: 5 minutes behind, or 64 MiB of reply buffer, ends the client as "too far behind" (`control.c:134`, `:137`, `:439-447`, `:601-613`).

Retention:
- `history-limit` default 2000 lines (`options-table.c:859-864`).
- Lines are dropped 10% of the limit at a time (`grid.c:405-430`, `ny = gd->hlimit / 10;`). No recording.

### asciinema 3.2.1 (`77498061982889f68ba1f6e8b666dd8d13246250`)

High-rate output:
- One event per read: `src/session.rs:148-152`, `:238-245`, buffer 128 KiB (`:22`).
- No batching or coalescing, no flush timer.
- The file writer writes each event straight to the file, raw or through zstd level 6 (`file_output.rs:155`, `output_writer.rs:8-14`).
- Events pass a bounded channel of 1024 (`session.rs:81`). A slow output blocks the recorder rather than dropping (`:106-115`).

Slow viewers:
- The live stream has a broadcast channel of 1024 (`stream.rs:82`).
- New viewers get a dump of the current terminal state, not a replay (`stream.rs:139-154`).
- A viewer that lags ends its stream (`server.rs:317-321`, `:341-345`; this is my reading of the stream combinators, not a quoted line).

Retention and format:
- `idle_time_limit` never changes the stored times. It is metadata for players (`cli.rs:281`), applied at playback (`asciicast.rs:209-231`).
- asciicast v2: event times count from the start of the recording; required header fields `version`, `width`, `height` (https://docs.asciinema.org/manual/asciicast/v2/).
- asciicast v3, the default output (`cmd/session.rs:254`): times are intervals since the previous event (`v3.rs:227-229`); the header nests `term.cols`/`term.rows`; it adds an `x` exit event (https://docs.asciinema.org/manual/asciicast/v3/).

### ttyrec 1.0.8 (`c4fd0f2bc69dba852526df3610f98d867ba5ab24`, mirror github.com/mjording/ttyrec, `Makefile:3` `VERSION = 1.0.8`)

- The format is a header `{ struct timeval tv; int len; }` (`ttyrec.h`), written as three little-endian ints (`io.c:98-110`) with absolute wall-clock time, then the raw bytes.
- **One record per `read()`**, no batching (`ttyrec.c:289-302`), unbuffered file (`ttyrec.c:155`).
- No escaping, so the payload costs exactly the program's bytes plus 12 per record.
- No size limit, rotation or idle limit.

### zellij (`519a23715c6b0f65a3ffada64e738ed6ec9380fc`)

High-rate output:
- Reads of 64 KiB (`zellij-server/src/terminal_bytes.rs:13`) go through a screen channel bounded at 50 (`lib.rs:5074-5076`), so a busy screen thread slows PTY reading.
- Rendering is debounced: `RENDER_QUIET_PERIOD = 1ms`, `RENDER_MAX_DELAY = 8ms` (`background_jobs.rs:172-173`, `:188-211`). At most one render per 1 ms of quiet, never later than 8 ms after the first request.

Slow viewers:
- The per-client queue is 5,000 messages (`os_input_output.rs:231`). When full the client is cut off as `ClientTooSlow` (`:259-264`, `zellij-utils/src/errors.rs:836-837`).
- A FIXME suggests "a redraw-on-backpressure mechanism" instead (`:288-296`).

Retention:
- Scrollback is 10,000 lines (`zellij-utils/src/consts.rs:14`), evicted one line at a time (`panes/grid.rs:575-576`).
- Session resurrection serializes a KDL layout (tabs, panes, commands, cwd; `session_serialization.rs:45-84`) every 60 s (`background_jobs.rs:171`), only when the layout changed (`pty.rs:830`).
- Pane contents are serialized only with `serialize_pane_viewport`, capped at the scrollback size (`default.kdl:211-216`, `panes/grid.rs:2091-2125`).
- Files go to `<cache>/contract_version_1/<session>/` (`consts.rs:39-48`). No recording of output over time.

### What this means for us

- Every multiplexer here gives a slow viewer the **latest screen**, not the missed stream:
  - tmux discards and redraws;
  - zellij debounces renders to 1 to 8 ms and drops a client that is too far behind;
  - asciinema's live stream starts viewers from a state dump.
- No recorder here coalesces or drops output: asciinema and ttyrec write one event per read. asciinema compresses with zstd.
- No live view replays history from the start.

## 5. Design within a budget (proposal, not implemented)

Constraints kept from the brief:
- the daemon's grid sees every byte;
- `wait` and `snapshot` see every frame;
- the recording stays lossless.

Nothing measured forces dropping lossless capture: zstd recovers 21 to 33x at about 1.3 ms per MB, below 2% of a core for the worst row measured (20 full-screen tabs, 14.7 MB/s of cast).

### Targets

| Target | Value | Measured today | Why this number |
|---|---|---|---|
| Dashboard responsiveness, 20 busy tabs, up to 4 watched (split view) | ≥ 55 paints/s, ≤ 50 ms/s long tasks, key-to-echo p95 ≤ 50 ms, scrub ≤ 100 ms, for an hour | 2.4 paints/s and 751 ms/s blocked after 25 s with one tab; with the live region emptied: 60 paints/s, 0 ms/s, echo ≤ 16 ms, scrub ≤ 46 ms (20 tabs, 1 viewed) | The prototype already meets it with margin on this machine. The hour bounds the browser's memory, which grows today with the timeline. |
| Live view per watched tab: N | **N = 30 updates/s**, latest frame wins, the first frame after a quiet period sent at once, and the last frame always sent | 60/s (one per frame), already latest-wins under backpressure | Halves the daemon's live-stream cost (+3% → +1.5% of a core per full-screen tab) and the browser's work. The leading-edge send keeps echo latency (measured 2 to 16 ms) intact. zellij's 1 ms quiet / 8 ms max debounce is the same idea at a higher ceiling. |
| Browser memory per watched tab | ≤ 50 MB after an hour, any rate | +27 MB/min (full screen) from the timeline | Hold only what is drawn; fetch past frames on demand. |
| Daemon CPU, 20 tabs at 60 Hz, 4 watched | X ≤ **15%** of one core for repost-like tabs; ≤ 75% for full-screen repaint | 10.2% without viewers + 4 × (0.5 + 1.3) = 17% (partial); 58% + 4 × 6.2 = 83% (full) | Without the always-on timeline, with live at 30/s: 10.2 + 4 × 0.25 ≈ 11% and 58 + 4 × 1.5 ≈ 64%, inside the target. Removing the per-read full-grid fingerprint (7% of busy time) gives more headroom. |
| Daemon RSS, 20 tabs at 60 Hz, after 1 hour | Y ≤ **300 MB**, flat after warm-up | 146 MB after 3.5 min (partial, 1 viewer), about 250 MB when every output log is full (20 × 12 MiB), plus 35 MB/hour of `frame_ends` | Grid + scrollback (about 9 MB per tab) and the output log are bounded today. `frame_ends` must become bounded (an index per segment on disk) for RSS to stay flat. |
| Disk, 20 tabs at 60 Hz | Z ≤ **250 MB/hour** for repost-like tabs; hard cap per tab and globally for every program | 3,468 MB/hour (partial), 52,852 MB/hour (full) | zstd level 3 on closed segments: 3,468 / 32.6 ≈ 106 MB/hour (partial). Full screen: 52,852 / 21.3 ≈ 2.5 GB/hour, which only the caps below bound. |

### Options, one variable at a time

1. **Fix the accessibility live region (browser).**
   - Measured in a page-side prototype: it alone restores 60 paints/s and 0 long tasks at 60 Hz, partial or full.
   - Options:
     - keep `screenReaderMode` and empty the region on each frame message (the prototype);
     - turn `screenReaderMode` off and expose the screen to assistive technology another way (the daemon already has the text of every row);
     - or report it to xterm.js upstream.
   - The browser tests read `.xterm-accessibility-tree` (`cli/tests/cli/browser/mod.rs:99-105`, `:157-169`), so turning screen-reader mode off needs another way for the tests to read the screen.
   - This is required whatever else is chosen.
2. **Cap the live stream at N = 30/s with latest-wins and backpressure.**
   - Estimated, not prototyped: the live stream's cost scales with frames sent (rows above), so it halves at N = 30 for 60 Hz programs and drops to a quarter for 120 Hz.
   - Backpressure already works: `follow_tab` awaits each send and reads the latest picture after (measured 60 → 44 messages a second under a slow page).
   - Add the time cap with a leading-edge send.
   - Move `Picture::of`'s SGR row building out of the state lock: it holds the lock that the reader thread needs, and `__psynch_mutexwait` shows up in the profile.
3. **Stop streaming the timeline; serve frames on demand.**
   - The timeline is 34% of daemon busy time and the browser's unbounded heap.
   - Proposal:
     - the browser asks for frame k;
     - the daemon draws it from the nearest **checkpoint** (a stored screen every K frames or every T seconds, kept in a per-tab index on disk) plus the recording after it;
     - the daemon sends one picture.
   - Estimated cost of one scrub at K = 32: at most 31 frames replayed, about the cost of 31 frames in the profile (≈ 0.5 ms at the 20-tab rate), versus 5.7 s to replay a 3.6-minute run today.
   - `capture_at` and clips also replay from the run's start today (items 14 and 15) and would use the same checkpoints.
4. **Thin unwatched tabs.**
   - Unwatched tabs already cost nothing for viewers today, because streams exist only for viewed tabs (the sessions sidebar received 0.1 messages a second). Nothing to do beyond 2 and 3.
   - Not thinned: the per-read work (grid, fingerprint, output log, recording) that `wait` and `snapshot` need.
5. **Coalescing output events in the recording.**
   - Saves about 20 bytes per event: 2.5% of a 60 Hz partial cast (measured overhead per event, above).
   - Not worth a format change on its own.
   - Dropping JSON escaping (a ttyrec-like length-prefixed binary format) saves 15 to 30%, but loses `asciinema play` compatibility (`docs/session-folder.md`, "Recordings").
6. **Keyframes plus deltas for the recording.** Lossy if it samples frames (a 10 Hz sample of a full-screen tab: about 140 KB/s instead of 734 KB/s, 5x less). zstd already does 21x losslessly, so keyframes belong in the index (option 3), not in the recording.
7. **Per-tab size caps or ring buffers.** Needed as the bound for full-screen programs, where even compressed recording is 2.5 GB an hour for 20 tabs. Write the recording in segments (for example 64 MiB, or 10 minutes) and drop the oldest closed segments past a per-tab cap. The `.cast` stays valid if each segment starts with a header and a full repaint of the screen (the same paint `record start` already writes, `terminal.rs:488-494`).
8. **zstd for cold segments.** 21 to 33x at level 3, about 1.3 ms per MB of one core. Compress a segment when it closes; the live segment stays plain so `tail` and partial reads keep working (`cast.rs:98-100` already stops at an unfinished line). asciinema itself writes zstd at level 6 (`output_writer.rs:8-14`).

### DRAFT ADR (not a decision)

> **docs/adr/0003-a-viewer-sees-the-latest-screen-and-the-recording-is-segmented.md** — DRAFT for the user to accept, change or reject.
>
> Status: proposed.
>
> Context: one 60 Hz tab froze the dashboard within a minute (xterm's accessibility live region grows with every printed character) and the browser kept every frame of every viewed tab. The daemon kept up with 2,400 frames a second. Recording costs 0.17 to 2.6 GB an hour per busy tab, uncompressed and unbounded, while the docs say "a few KB". agent-terminal will run tens of sessions at once.
>
> Decision (proposed):
> 1. The daemon keeps seeing every byte and counting every frame; `wait`, `snapshot` and the recording are not thinned.
> 2. A viewer gets the latest screen of a watched tab at most 30 times a second. The first change after a quiet period goes out at once, the last one always does, and a slow viewer skips frames instead of queueing them.
> 3. The timeline is served on demand from checkpoints. The browser holds the screen it shows, not the run.
> 4. The dashboard's terminal does not accumulate printed text for assistive technology without bound.
> 5. Recordings are written in segments. Closed segments are compressed with zstd. Retention is bounded by age, size per tab, count of sessions and a global quota (section 6).
>
> Consequences: the live view can skip intermediate frames that `wait` still sees. Scrubbing to a frame costs at most one checkpoint interval of replay. A `.cast` becomes a set of segment files, so `asciinema play` plays one segment at a time and `trace export` packs them all. Old recordings disappear by policy, not only when the session name is reused.
>
> Considered: lossy recording by frame sampling (rejected: zstd is enough); a binary recording format (kept as an option, loses `asciinema play`); cutting off slow viewers like zellij (rejected: latest-screen keeps them usable, as tmux does).

## 6. Lifecycle and cleanup

### Today

- A session folder starts when the session's first tab opens. Starting a session under the same name removes the previous folder (`cli/src/native/store.rs:136-143`, `docs/session-folder.md`, "Lifetime"). Otherwise the folder is never removed: closing, daemon restarts and crashes all leave it.
- `recordings/` (`record start` without a path, `daemon.rs:253`) and `screenshots/` are never cleaned.
- Unique session names, for example `session id --scope worktree` (`docs/commands.md:149`), therefore pile up folders forever.
- The claim, `docs/commands.md:148`: "Every tab is recorded all the time (it costs a few KB), so the agent can cut a GIF or MP4 after something goes wrong". Measured:

| Tab | Cast per hour |
|---|---|
| repost's own login screen | 34 MB (7.9 KB/s) |
| repost `/forward`, waiting for webhooks | 217 MB (60 KB/s), 12 MB after zstd -3 |
| 1 Hz partial fixture | 2.9 MB |
| repost-like 60 Hz | 173 MB |
| full-screen 60 Hz | 2.6 GB |
| 20 repost-like tabs | 3.5 GB |
| 20 full-screen tabs | 53 GB |

"A few KB" holds only for a short run of a mostly idle program.

### Proposal

- **Segments:** `t1.000.cast`, `t1.001.cast`, … Each starts with a header and a full repaint. A new segment starts at 64 MiB or 10 minutes; the closed one is compressed to `.cast.zst`.
- **Per tab:** keep at most 256 MiB of compressed segments; drop the oldest closed segment first. The live segment is never dropped. At the measured ratios that is about 2.4 hours of a repost-like tab before the cap matters and about 6 minutes of a full-screen 60 Hz tab.
- **By age:** remove the folder of a closed session 7 days after its last write.
- **By count:** keep at most 50 closed session folders; remove the oldest first.
- **Global quota:** 2 GiB for `sessions/` + `recordings/` + `screenshots/`, default and configurable. Over quota, remove closed sessions oldest first, then the oldest closed segments of running tabs. Never touch the live segment or files a running `trace export` is reading.
- **When:** at daemon start, when a session closes, and every 10 minutes while the daemon runs.
- **After a daemon crash:** at the next start, every session folder without a live daemon is closed:
  - cut each segment's unfinished last line (the reader already ignores it, `cast.rs:98-100`);
  - compress segments left uncompressed;
  - apply the policy.
  - The activity log is append-only and already handles a missing last line the same way in `trace export` ("up to their last complete line", `docs/session-folder.md`, "`trace export <path>`").
- **Visible:** `session list` and the dashboard show the size of each session folder. `doctor` warns past 80% of the quota.

## Files added or changed

- `cli/tests/fixtures/src/bin/redraw.rs` (new): the 1 to 120 Hz partial and full-repaint fixture with echo and stats.
- `cli/perf/src/load.rs` (new), `cli/perf/src/main.rs`, `cli/perf/Cargo.toml`, `cli/Cargo.lock`: `at-perf load`.
- `cli/tests/cli/browser/fast_output.rs` (new), `cli/tests/cli/browser/mod.rs`: the browser test, and a shorter agent-browser session name (`at-test-<pid>-<n>`) so its socket path fits the 104-byte limit when `AGENT_BROWSER_SOCKET_DIR` is inside a checkout.
- `cli/tests/cli/support.rs` and the five tests that made folders in `/tmp`: one `scratch(prefix)` helper that puts every test folder in cargo's `CARGO_TARGET_TMPDIR` (`cli/target/tmp`), inside the checkout.
- `dashboard/src/lib/xterm.ts`, `cli/assets/dashboard/`: the fix for the live region (`dropAnnouncementsOnceDrawn`, section 1), and the rebuilt dashboard.
- `docs/design.md`: the Dashboard section says why the terminal runs in screen-reader mode and why its live region is emptied on every render.
- Scratch, gitignored, under `.superpowers/`:
  - `bench/` (probe, scripts, every result file);
  - `prior-art/`;
  - `target-prof/` (the profiling build);
  - `ath*` (daemon homes);
  - `ab-home`, `s`, `tmp` (agent-browser state);
  - `repost-home`.

Checks run from `cli/`: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --features browser-tests -- -D warnings`, `cargo test --test static_checks`, `cargo test -p at-perf`.
