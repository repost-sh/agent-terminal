use std::collections::{HashMap, VecDeque};

const KEPT_BYTES: usize = 4 * 1024 * 1024;

#[derive(Clone, Copy)]
struct Point {
    frame: u64,
    raw: u64,
    text: u64,
}

#[derive(Default)]
struct Kept {
    bytes: VecDeque<u8>,
    dropped: u64,
}

impl Kept {
    fn end(&self) -> u64 {
        self.dropped + self.bytes.len() as u64
    }

    fn push(&mut self, bytes: &[u8]) {
        self.bytes.extend(bytes);
        let excess = self.bytes.len().saturating_sub(KEPT_BYTES);
        self.bytes.drain(..excess);
        self.dropped += excess as u64;
    }

    fn since(&self, offset: u64) -> Option<Vec<u8>> {
        let start = offset.checked_sub(self.dropped)?;
        Some(self.bytes.range(start as usize..).copied().collect())
    }

    fn between(&self, from: u64, to: u64) -> (Vec<u8>, u64) {
        let start = from.max(self.dropped);
        let end = to.clamp(start, self.end());
        let kept = self.bytes.range((start - self.dropped) as usize..(end - self.dropped) as usize).copied().collect();
        (kept, start - from)
    }
}

#[derive(Clone, Copy)]
pub struct Offsets {
    pub text: u64,
    pub styled: u64,
}

#[derive(Default)]
pub struct OutputLog {
    raw: Kept,
    text: Kept,
    styled: Kept,
    chunks: VecDeque<Point>,
    forgot_chunks: bool,
    marks: HashMap<String, Point>,
}

impl OutputLog {
    pub fn record(&mut self, frame: u64, raw: &[u8], text: &[u8], styled: &[u8]) {
        self.chunks.push_back(self.here(frame));
        self.raw.push(raw);
        self.text.push(text);
        self.styled.push(styled);
        while self.chunks.front().is_some_and(|chunk| chunk.raw < self.raw.dropped) {
            self.chunks.pop_front();
            self.forgot_chunks = true;
        }
    }

    pub fn mark(&mut self, name: String, frame: u64) {
        self.marks.insert(name, self.here(frame));
    }

    pub fn since(&self, since: Option<&str>, raw: bool, frame: u64) -> Result<String, String> {
        let (point, described) = match since {
            None => (Some(Point { frame: 0, raw: 0, text: 0 }), "the start".to_string()),
            Some(name) => match (self.marks.get(name), name.parse::<u64>()) {
                (Some(point), _) => (Some(*point), name.to_string()),
                (None, Ok(since_frame)) => (self.at_frame(since_frame, frame), format!("frame {since_frame}")),
                (None, Err(_)) => {
                    return Err(format!("no mark named {name:?}; name a point with `agent-terminal mark {name}`"));
                }
            },
        };
        let kept = point.and_then(|point| if raw { self.raw.since(point.raw) } else { self.text.since(point.text) });
        let bytes = kept.ok_or_else(|| {
            format!(
                "the output since {described} is no longer kept (agent-terminal keeps the last {} MiB); mark a later point",
                KEPT_BYTES / (1024 * 1024)
            )
        })?;
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }

    pub fn end(&self) -> Offsets {
        Offsets { text: self.text.end(), styled: self.styled.end() }
    }

    pub fn text_between(&self, from: Offsets, to: Option<Offsets>) -> String {
        kept_between(&self.text, from.text, to.map(|to| to.text))
    }

    pub fn styled_between(&self, from: Offsets, to: Option<Offsets>) -> String {
        kept_between(&self.styled, from.styled, to.map(|to| to.styled))
    }

    fn here(&self, frame: u64) -> Point {
        Point { frame, raw: self.raw.end(), text: self.text.end() }
    }

    fn at_frame(&self, since: u64, frame: u64) -> Option<Point> {
        match self.chunks.iter().position(|chunk| chunk.frame >= since) {
            Some(0) if self.forgot_chunks && self.chunks[0].frame > since => None,
            Some(i) => Some(self.chunks[i]),
            None => Some(self.here(frame)),
        }
    }
}

fn kept_between(kept: &Kept, from: u64, to: Option<u64>) -> String {
    let (bytes, lost) = kept.between(from, to.unwrap_or_else(|| kept.end()));
    let text = String::from_utf8_lossy(&bytes).into_owned();
    if lost == 0 {
        return text;
    }
    format!(
        "… (the first {lost} bytes are no longer kept; agent-terminal keeps the last {} MiB of output)\n{text}",
        KEPT_BYTES / (1024 * 1024)
    )
}
