use super::{Chunk, KEPT_BYTES, Offsets, OutputLog};
use crate::protocol::{Target, request};

fn asked(since: Option<&str>, raw: bool) -> request::Output {
    request::Output { target: Target { session: "s".to_string(), tab: None }, since: since.map(str::to_string), raw }
}

fn chunk<'a>(raw: &'a str, text: &'a str, styled: &'a str) -> Chunk<'a> {
    Chunk { raw: raw.as_bytes(), text: text.as_bytes(), styled: styled.as_bytes() }
}

fn log_of(chunks: &[(u64, &str)]) -> OutputLog {
    let mut log = OutputLog::default();
    for (frame, text) in chunks {
        let raw = format!("\x1b[1m{text}");
        log.record(*frame, &chunk(&raw, text, &format!("<{text}>")));
    }
    log
}

#[test]
fn output_since_the_start_a_mark_or_a_frame_reads_text_or_raw_bytes() {
    let mut log = log_of(&[(1, "one "), (2, "two ")]);
    log.mark("here".to_string(), 2);
    log.record(3, &chunk("\x1b[1mthree", "three", "<three>"));
    let read = |since, raw| log.since(&asked(since, raw), 3);
    assert_eq!(
        [
            read(None, false),
            read(Some("here"), false),
            read(Some("here"), true),
            read(Some("2"), false),
            read(Some("3"), false),
            read(Some("9"), false)
        ],
        [
            Ok("one two three".to_string()),
            Ok("three".to_string()),
            Ok("\x1b[1mthree".to_string()),
            Ok("two three".to_string()),
            Ok("three".to_string()),
            Ok(String::new())
        ]
    );
    assert_eq!(
        read(Some("there"), false),
        Err("no mark named \"there\"; name a point with `agent-terminal mark there`".to_string())
    );
}

#[test]
fn output_older_than_the_kept_four_mib_is_refused_by_mark_and_by_frame() {
    let mut log = log_of(&[(1, "early")]);
    log.mark("early".to_string(), 1);
    let filler = "x".repeat(KEPT_BYTES);
    log.record(2, &chunk(&filler, &filler, &filler));
    log.record(3, &chunk("late", "late", "late"));
    let refused = |since: &str| {
        format!("the output since {since} is no longer kept (agent-terminal keeps the last 4 MiB); mark a later point")
    };
    assert_eq!(log.since(&asked(Some("early"), false), 3), Err(refused("early")));
    assert_eq!(log.since(&asked(Some("1"), true), 3), Err(refused("frame 1")));
    assert_eq!(log.since(&asked(None, false), 3), Err(refused("the start")));
    assert_eq!(log.since(&asked(Some("3"), false), 3), Ok("late".to_string()));
    assert_eq!(log.since(&asked(Some("2"), false), 3), Err(refused("frame 2")));
}

#[test]
fn the_log_ends_where_its_text_and_styled_bytes_end_and_reads_between_two_ends() {
    let mut log = log_of(&[(1, "ab")]);
    let start = log.end();
    log.record(2, &chunk("cd", "cd", "<cd>"));
    let middle = log.end();
    log.record(3, &chunk("ef", "ef", "<ef>"));
    assert_eq!((start.text, start.styled, middle.text, middle.styled), (2, 4, 4, 8));
    assert_eq!(
        (
            log.text_between(start, Some(middle)),
            log.styled_between(start, Some(middle)),
            log.text_between(middle, None),
            log.styled_between(middle, None)
        ),
        ("cd".to_string(), "<cd>".to_string(), "ef".to_string(), "<ef>".to_string())
    );
    assert_eq!(log.text_between(middle, Some(start)), String::new());
}

#[test]
fn reading_between_ends_says_how_much_was_dropped_from_its_front() {
    let mut log = OutputLog::default();
    let start = Offsets { text: 0, styled: 0 };
    let filler = "y".repeat(KEPT_BYTES);
    log.record(1, &chunk("ab", "ab", "ab"));
    log.record(2, &chunk(&filler, &filler, &filler));
    let read = log.text_between(start, None);
    let notice = "… (the first 2 bytes are no longer kept; agent-terminal keeps the last 4 MiB of output)\n";
    assert_eq!((read.starts_with(notice), read.len()), (true, notice.len() + KEPT_BYTES));
    assert!(log.styled_between(start, None).starts_with(notice));
}

#[test]
fn output_since_two_offsets_gives_the_kept_bytes_from_each_and_the_marks_in_order() {
    let mut log = log_of(&[(1, "ab")]);
    log.mark("zed".to_string(), 1);
    log.record(2, &chunk("cd", "cd", "<cd>"));
    log.mark("alpha".to_string(), 2);
    let output = log.output_since(3, 99);
    let marks: Vec<_> = output.marks.iter().map(|mark| (mark.raw, mark.styled, mark.name.as_str())).collect();
    assert_eq!(
        (output.raw.start, output.raw.bytes, output.styled.start, output.styled.bytes, marks),
        (3, b"mabcd".to_vec(), 8, Vec::new(), vec![(6, 4, "zed"), (8, 8, "alpha")])
    );
    let filler = "z".repeat(KEPT_BYTES);
    log.record(3, &chunk(&filler, "", ""));
    assert_eq!(log.output_since(0, 0).raw.start, 8);
}

#[test]
fn a_frame_before_the_first_kept_chunk_is_refused_only_once_chunks_were_forgotten() {
    let mut log = log_of(&[(5, "five")]);
    assert_eq!(log.since(&asked(Some("2"), false), 5), Ok("five".to_string()));
    let filler = "w".repeat(KEPT_BYTES);
    log.record(6, &chunk(&filler, &filler, &filler));
    log.record(7, &chunk("x", "x", "x"));
    assert!(log.since(&asked(Some("6"), false), 7).is_err());
    assert_eq!(log.since(&asked(Some("7"), false), 7), Ok("x".to_string()));
}
