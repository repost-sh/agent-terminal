use std::collections::{HashMap, VecDeque};

use crate::protocol::request;

const KEPT_BYTES: usize = 4 * 1024 * 1024;

#[derive(Clone, Copy)]
struct Point {
    frame: u64,
    raw: u64,
    text: u64,
    styled: u64,
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
        Some(self.bytes.iter().skip(position(start)).copied().collect())
    }

    fn between(&self, from: u64, to: u64) -> (Vec<u8>, u64) {
        let start = from.max(self.dropped);
        let end = to.clamp(start, self.end());
        let kept =
            self.bytes.iter().skip(position(start - self.dropped)).take(position(end - start)).copied().collect();
        (kept, start - from)
    }

    fn kept_from(&self, offset: u64) -> (u64, Vec<u8>) {
        let start = offset.clamp(self.dropped, self.end());
        (start, self.bytes.iter().skip(position(start - self.dropped)).copied().collect())
    }
}

pub(crate) struct Slice {
    pub start: u64,
    pub bytes: Vec<u8>,
}

pub(crate) struct Output {
    pub raw: Slice,
    pub styled: Slice,
    pub marks: Vec<Marked>,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Marked {
    pub raw: u64,
    pub styled: u64,
    pub name: String,
}

#[derive(Clone, Copy)]
pub(crate) struct Offsets {
    pub text: u64,
    pub styled: u64,
}

#[derive(Default)]
pub(crate) struct OutputLog {
    raw: Kept,
    text: Kept,
    styled: Kept,
    chunks: VecDeque<Point>,
    forgot_chunks: bool,
    marks: HashMap<String, Point>,
}

pub(crate) struct Chunk<'a> {
    pub raw: &'a [u8],
    pub text: &'a [u8],
    pub styled: &'a [u8],
}

impl OutputLog {
    pub(crate) fn record(&mut self, frame: u64, chunk: &Chunk<'_>) {
        self.chunks.push_back(self.here(frame));
        self.raw.push(chunk.raw);
        self.text.push(chunk.text);
        self.styled.push(chunk.styled);
        while self.chunks.front().is_some_and(|chunk| chunk.raw < self.raw.dropped) {
            self.chunks.pop_front();
            self.forgot_chunks = true;
        }
    }

    pub(crate) fn mark(&mut self, name: String, frame: u64) {
        self.marks.insert(name, self.here(frame));
    }

    pub(crate) fn since(&self, asked: &request::Output, frame: u64) -> Result<String, String> {
        let (point, described) = match asked.since.as_deref() {
            None => (Some(Point { frame: 0, raw: 0, text: 0, styled: 0 }), "the start".to_string()),
            Some(name) => match (self.marks.get(name), name.parse::<u64>()) {
                (Some(point), _) => (Some(*point), name.to_string()),
                (None, Ok(since_frame)) => (self.at_frame(since_frame, frame), format!("frame {since_frame}")),
                (None, Err(_)) => {
                    return Err(format!("no mark named {name:?}; name a point with `agent-terminal mark {name}`"));
                }
            },
        };
        let kept =
            point.and_then(|point| if asked.raw { self.raw.since(point.raw) } else { self.text.since(point.text) });
        let bytes = kept.ok_or_else(|| {
            format!(
                "the output since {described} is no longer kept (agent-terminal keeps the last {} MiB); mark a later point",
                KEPT_BYTES / (1024 * 1024)
            )
        })?;
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }

    pub(crate) fn end(&self) -> Offsets {
        Offsets { text: self.text.end(), styled: self.styled.end() }
    }

    pub(crate) fn output_since(&self, raw: u64, styled: u64) -> Output {
        let slice = |kept: &Kept, offset: u64| {
            let (start, bytes) = kept.kept_from(offset);
            Slice { start, bytes }
        };
        let mut marks: Vec<Marked> = self
            .marks
            .iter()
            .map(|(name, point)| Marked { raw: point.raw, styled: point.styled, name: name.clone() })
            .collect();
        marks.sort();
        Output { raw: slice(&self.raw, raw), styled: slice(&self.styled, styled), marks }
    }

    pub(crate) fn text_between(&self, from: Offsets, to: Option<Offsets>) -> String {
        kept_between(&self.text, from.text, to.map(|to| to.text))
    }

    pub(crate) fn styled_between(&self, from: Offsets, to: Option<Offsets>) -> String {
        kept_between(&self.styled, from.styled, to.map(|to| to.styled))
    }

    fn here(&self, frame: u64) -> Point {
        Point { frame, raw: self.raw.end(), text: self.text.end(), styled: self.styled.end() }
    }

    fn at_frame(&self, since: u64, frame: u64) -> Option<Point> {
        match self.chunks.iter().enumerate().find(|(_, chunk)| chunk.frame >= since) {
            Some((0, first)) if self.forgot_chunks && first.frame > since => None,
            Some((_, chunk)) => Some(*chunk),
            None => Some(self.here(frame)),
        }
    }
}

fn position(offset: u64) -> usize {
    usize::try_from(offset).unwrap_or(usize::MAX)
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
