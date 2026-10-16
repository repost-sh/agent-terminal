use std::fmt::Debug;
use std::path::PathBuf;

use super::{
    ScreenshotPath, parse_bytes, parse_env, parse_origin, parse_position, parse_rows, parse_scrollback, parse_size,
    parse_threshold,
};

fn parsed<T: Debug>(parse: fn(&str) -> Result<T, String>, inputs: &[&str]) -> String {
    inputs.iter().map(|input| format!("{input:?} -> {:?}", parse(input))).collect::<Vec<_>>().join("\n")
}

#[test]
fn a_size_is_columns_by_rows() {
    insta::assert_snapshot!(parsed(parse_size, &["120x40", "20x5", "500x200", "x40", "120x", "120", "19x40", "120x201", "a1x2", "70000x10"]), @r##"
        "120x40" -> Ok(Size { cols: 120, rows: 40 })
        "20x5" -> Ok(Size { cols: 20, rows: 5 })
        "500x200" -> Ok(Size { cols: 500, rows: 200 })
        "x40" -> Err("invalid column count:  (cannot parse integer from empty string)")
        "120x" -> Err("invalid row count:  (cannot parse integer from empty string)")
        "120" -> Err("expected COLSxROWS, e.g. 120x40")
        "19x40" -> Ok(Size { cols: 19, rows: 40 })
        "120x201" -> Ok(Size { cols: 120, rows: 201 })
        "a1x2" -> Err("invalid column count: a1 (invalid digit found in string)")
        "70000x10" -> Err("invalid column count: 70000 (number too large to fit in target type)")
    "##);
}

#[test]
fn rows_are_a_range_from_one_that_does_not_run_backwards() {
    insta::assert_snapshot!(parsed(parse_rows, &["1:10", "3:3", "0:4", "5:4", "1", "a:2", "2:b"]), @r##"
        "1:10" -> Ok(RowRange { first: 1, last: 10 })
        "3:3" -> Ok(RowRange { first: 3, last: 3 })
        "0:4" -> Err("rows count from 1, and A must not be after B")
        "5:4" -> Err("rows count from 1, and A must not be after B")
        "1" -> Err("expected A:B, e.g. 1:10")
        "a:2" -> Err("invalid row: a (invalid digit found in string)")
        "2:b" -> Err("invalid row: b (invalid digit found in string)")
    "##);
}

#[test]
fn scrollback_is_every_line_or_the_last_n() {
    insta::assert_snapshot!(parsed(parse_scrollback, &["", "0", "500", "-1", "many"]), @r##"
        "" -> Ok(All)
        "0" -> Ok(Last(0))
        "500" -> Ok(Last(500))
        "-1" -> Err("invalid line count: -1 (invalid digit found in string)")
        "many" -> Err("invalid line count: many (invalid digit found in string)")
    "##);
}

#[test]
fn a_threshold_is_a_share_from_zero_to_one() {
    insta::assert_snapshot!(parsed(parse_threshold, &["0", "0.01", "1", "1.5", "-0.1", "half"]), @r##"
        "0" -> Ok(0.0)
        "0.01" -> Ok(0.01)
        "1" -> Ok(1.0)
        "1.5" -> Err("invalid threshold: 1.5 (expected a share of pixels from 0 to 1, e.g. 0.01)")
        "-0.1" -> Err("invalid threshold: -0.1 (expected a share of pixels from 0 to 1, e.g. 0.01)")
        "half" -> Err("invalid threshold: half (expected a share of pixels from 0 to 1, e.g. 0.01)")
    "##);
}

#[test]
fn a_position_is_a_row_and_column_from_one() {
    insta::assert_snapshot!(parsed(parse_position, &["4,3", "1,1", "0,3", "4,0", "4", "a,3", "4,b"]), @r##"
        "4,3" -> Ok((4, 3))
        "1,1" -> Ok((1, 1))
        "0,3" -> Err("rows and columns count from 1")
        "4,0" -> Err("rows and columns count from 1")
        "4" -> Err("expected ROW,COL, e.g. 4,3")
        "a,3" -> Err("invalid row: a (invalid digit found in string)")
        "4,b" -> Err("invalid column: b (invalid digit found in string)")
    "##);
}

#[test]
fn an_origin_is_an_http_host_and_port() {
    insta::assert_snapshot!(
        parsed(parse_origin, &["http://localhost:5173", "https://a.b:443", "localhost:5173", "http://:80", "http://a/b:80", "http://a:port", "http://a", "http://a:70000"]),
        @r##"
        "http://localhost:5173" -> Ok("http://localhost:5173")
        "https://a.b:443" -> Ok("https://a.b:443")
        "localhost:5173" -> Err("localhost:5173 is not an origin; an origin is http://host:port, like http://localhost:5173")
        "http://:80" -> Err("http://:80 is not an origin; an origin is http://host:port, like http://localhost:5173")
        "http://a/b:80" -> Err("http://a/b:80 is not an origin; an origin is http://host:port, like http://localhost:5173")
        "http://a:port" -> Err("http://a:port is not an origin; an origin is http://host:port, like http://localhost:5173")
        "http://a" -> Err("http://a is not an origin; an origin is http://host:port, like http://localhost:5173")
        "http://a:70000" -> Err("http://a:70000 is not an origin; an origin is http://host:port, like http://localhost:5173")
    "##
    );
}

#[test]
fn an_environment_variable_is_a_named_value() {
    insta::assert_snapshot!(parsed(parse_env, &["A=1", "A=", "A=b=c", "=1", "A"]), @r##"
        "A=1" -> Ok(("A", "1"))
        "A=" -> Ok(("A", ""))
        "A=b=c" -> Ok(("A", "b=c"))
        "=1" -> Err("expected K=V")
        "A" -> Err("expected K=V")
    "##);
}

#[test]
fn bytes_take_escapes_for_control_bytes_and_hex() {
    let bytes = |value: &str| parse_bytes(value).map(|raw| raw.0);
    insta::assert_snapshot!(
        parsed(bytes, &["ab", r"\e[A", r"\n\r\t\0\\", r"\x41\x7f", "é", r"\x4", r"\xzz", r"\q", "a\\"]),
        @r##"
        "ab" -> Ok([97, 98])
        "\\e[A" -> Ok([27, 91, 65])
        "\\n\\r\\t\\0\\\\" -> Ok([10, 13, 9, 0, 92])
        "\\x41\\x7f" -> Ok([65, 127])
        "é" -> Ok([195, 169])
        "\\x4" -> Err("\\x needs two hex digits, got \\x4")
        "\\xzz" -> Err("\\x needs two hex digits, got \\xzz")
        "\\q" -> Err("unknown escape \\q; use \\e, \\xHH, \\n, \\r, \\t, \\0 or \\\\")
        "a\\" -> Err("a trailing \\ escapes nothing; write \\\\ for a backslash")
    "##
    );
}

#[test]
fn an_empty_screenshot_path_means_the_default() {
    assert_eq!(
        (ScreenshotPath::from("").chosen(), ScreenshotPath::from("a.png").chosen()),
        (None, Some(PathBuf::from("a.png")))
    );
}
