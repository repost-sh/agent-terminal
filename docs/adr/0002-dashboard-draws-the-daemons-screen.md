---
status: accepted
---

# The dashboard draws the daemon's screen, not the program's bytes

The spec (46H-19, Architecture) has the dashboard server send each tab's current screen when a viewer connects and then the live byte stream, and serve the timeline as keyframes plus the recording for the browser to seek through. Instead the daemon sends repaints: for each frame, the escape sequences that redraw the rows that changed since the frame the viewer last got (every row on the first frame and after a resize), with the frame number, size and modes. The timeline replays the tab's recording through the daemon's own screen engine and sends every frame the same way, with every 32nd frame a full repaint, so the browser seeks by drawing the nearest full frame and the changed rows after it.

We chose this because the spec also asks that the human see exactly what the agent sees (story 70) and that an Activity or Commands entry show the terminal at that moment (story 79). The agent sees the daemon's grid. Replaying the program's bytes in xterm.js would have a second terminal emulator interpret them, and wherever the two differ (character widths, sequences one of them does not support, reflow on resize, state set before the viewer joined such as scroll regions, character sets or the primary screen behind the alternate one) the dashboard would show a screen the agent never saw. A repaint uses only cursor moves, colours and attributes, and text, which xterm.js draws the same way. Frames are also what everything else is keyed by (snapshot headers, refs, waits, Activity and Commands entries), so the timeline seeks by frame number directly.

## Considered options

- **Bytes, as the spec has it.** Cheaper for the daemon and keeps every intermediate state, but the browser's screen can drift from the agent's for the reasons above, and seeking means replaying the recording from a keyframe in xterm.js, whose state at that keyframe the daemon cannot fully describe.

## Consequences

- The browser shows frames, not every byte: what a program prints between two frames shows only as the frame that ends it, the same coalescing the agent sees.
- The browser terminal has no scrollback (xterm.js runs with `scrollback: 0`); what scrolled off the top is in the Output tab, which shows the raw stream.
- Only what a frame carries is drawn: text, colours, attributes and the cursor. Hyperlinks (OSC 8), inline images and the window title are not drawn in the terminal.
- The daemon does the work: it compares rows for every viewer of every tab, and when a viewer opens a timeline it replays the tab's whole run from the recording. The browser keeps every frame of the run in memory.
