use std::io::Write;

fn main() -> std::io::Result<()> {
    let path = std::env::args().nth(1).expect("usage: cast-replay <file.cast>");
    let cast = std::fs::read_to_string(&path)?;
    let mut lines = cast.lines();
    let header: serde_json::Value = serde_json::from_str(lines.next().expect("a header line")).expect("header JSON");
    assert_eq!(header["version"], 2, "not an asciicast v2 file");
    crossterm::terminal::enable_raw_mode()?;
    let mut out = std::io::stdout();
    for line in lines {
        let (_, kind, data): (f64, String, String) = serde_json::from_str(line).expect("event JSON");
        if kind == "o" {
            out.write_all(data.as_bytes())?;
        }
    }
    out.flush()
}
