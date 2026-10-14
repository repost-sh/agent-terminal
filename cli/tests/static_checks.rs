use std::path::{Path, PathBuf};

enum Scope {
    Everywhere,
    ProductCode,
    OutsideOf(&'static str),
}

impl Scope {
    fn applies_to(&self, relative: &str) -> bool {
        match self {
            Scope::Everywhere => true,
            Scope::ProductCode => relative.starts_with("src/"),
            Scope::OutsideOf(home) => relative != *home,
        }
    }
}

const BANNED: &[(&str, &str, Scope)] = &[
    ("unsafe", "unsafe code", Scope::Everywhere),
    ("todo!", "todo! placeholder", Scope::Everywhere),
    ("unimplemented!", "unimplemented! placeholder", Scope::Everywhere),
    ("dbg!", "dbg! left in", Scope::Everywhere),
    ("#[allow(", "silenced lint", Scope::Everywhere),
    ("#![allow(", "silenced lint", Scope::Everywhere),
    ("#[expect(", "silenced lint", Scope::Everywhere),
    ("#![expect(", "silenced lint", Scope::Everywhere),
    ("#[ignore", "skipped test", Scope::Everywhere),
    ("thread::sleep", "sleep used as synchronization; wait on the event instead", Scope::ProductCode),
    ("let _ =", "dropped result; return it or log why it can fail", Scope::ProductCode),
    (
        "alacritty_terminal",
        "the screen engine belongs in src/native/screen.rs",
        Scope::OutsideOf("src/native/screen.rs"),
    ),
    ("portable_pty", "the PTY library belongs in src/native/pty.rs", Scope::OutsideOf("src/native/pty.rs")),
];

#[test]
fn source_has_no_comments_and_no_workarounds() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    collect_rust_files(root, &mut files);
    assert!(!files.is_empty(), "no Rust files found under {}", root.display());

    let mut violations = Vec::new();
    for file in &files {
        let source = std::fs::read_to_string(file).unwrap_or_else(|e| panic!("read {}: {e}", file.display()));
        let relative = file.strip_prefix(root).unwrap_or(file).display().to_string();
        let scanned = scan(&source);
        for line in scanned.comment_lines {
            violations.push(format!("{relative}:{line}: comment"));
        }
        for (pattern, reason, scope) in BANNED {
            if !scope.applies_to(&relative) {
                continue;
            }
            for (index, line) in scanned.code.lines().enumerate() {
                if contains_token(line, pattern) {
                    violations.push(format!("{relative}:{}: {reason} (`{pattern}`)", index + 1));
                }
            }
        }
    }
    assert!(violations.is_empty(), "comments and workarounds are not allowed:\n{}", violations.join("\n"));
}

#[test]
fn the_scanner_finds_comments_but_not_strings_or_lifetimes() {
    let source = "fn f<'a>(x: &'a str) -> char {\n    let url = \"http://x\";\n    let raw = r#\"/* no */\"#;\n    let c = '/';\n    'x' // here\n}\n/* block\n */\n/// doc\n";
    let scanned = scan(source);
    assert_eq!(scanned.comment_lines, vec![5, 7, 9]);
    assert!(!scanned.code.contains("http"));
}

#[test]
fn banned_tokens_match_whole_words_only() {
    assert!(contains_token("    unsafe { x }", "unsafe"));
    assert!(!contains_token("let not_unsafe = 1;", "unsafe"));
    assert!(contains_token("#[allow(dead_code)]", "#[allow("));
    assert!(contains_token("        let _ = writer.flush();", "let _ ="));
    assert!(contains_token("use portable_pty::CommandBuilder;", "portable_pty"));
}

#[test]
fn scopes_limit_where_a_ban_applies() {
    assert!(Scope::ProductCode.applies_to("src/native/terminal.rs"));
    assert!(!Scope::ProductCode.applies_to("tests/fixtures/src/bin/line-printer.rs"));
    assert!(!Scope::OutsideOf("src/native/pty.rs").applies_to("src/native/pty.rs"));
    assert!(Scope::OutsideOf("src/native/pty.rs").applies_to("src/native/daemon.rs"));
}

fn collect_rust_files(dir: &Path, files: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap_or_else(|e| panic!("read {}: {e}", dir.display())) {
        let path = entry.expect("directory entry").path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default();
        if path.is_dir() {
            if name != "target" && !name.starts_with('.') {
                collect_rust_files(&path, files);
            }
        } else if name.ends_with(".rs") {
            files.push(path);
        }
    }
}

struct Scanned {
    code: String,
    comment_lines: Vec<usize>,
}

fn scan(source: &str) -> Scanned {
    let chars: Vec<char> = source.chars().collect();
    let mut code = String::with_capacity(source.len());
    let mut comment_lines = Vec::new();
    let mut line = 1;
    let mut i = 0;

    let blank = |code: &mut String, c: char| code.push(if c == '\n' { '\n' } else { ' ' });

    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        if c == '/' && next == Some('/') {
            comment_lines.push(line);
            while i < chars.len() && chars[i] != '\n' {
                blank(&mut code, chars[i]);
                i += 1;
            }
        } else if c == '/' && next == Some('*') {
            comment_lines.push(line);
            let mut depth = 0;
            while i < chars.len() {
                if chars[i] == '/' && chars.get(i + 1) == Some(&'*') {
                    depth += 1;
                    code.push_str("  ");
                    i += 2;
                } else if chars[i] == '*' && chars.get(i + 1) == Some(&'/') {
                    depth -= 1;
                    code.push_str("  ");
                    i += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    if chars[i] == '\n' {
                        line += 1;
                    }
                    blank(&mut code, chars[i]);
                    i += 1;
                }
            }
        } else if let Some(hashes) = raw_string_start(&chars, i) {
            let prefix = if c == 'b' { 2 } else { 1 };
            let open = prefix + hashes + 1;
            for &ch in &chars[i..i + open] {
                blank(&mut code, ch);
            }
            i += open;
            while i < chars.len() {
                if chars[i] == '"' && chars[i + 1..].iter().take(hashes).filter(|&&h| h == '#').count() == hashes {
                    for &ch in &chars[i..i + 1 + hashes] {
                        blank(&mut code, ch);
                    }
                    i += 1 + hashes;
                    break;
                }
                if chars[i] == '\n' {
                    line += 1;
                }
                blank(&mut code, chars[i]);
                i += 1;
            }
        } else if c == '"' {
            blank(&mut code, c);
            i += 1;
            while i < chars.len() {
                let ch = chars[i];
                if ch == '\\' {
                    blank(&mut code, ch);
                    if let Some(&escaped) = chars.get(i + 1) {
                        if escaped == '\n' {
                            line += 1;
                        }
                        blank(&mut code, escaped);
                    }
                    i += 2;
                    continue;
                }
                if ch == '\n' {
                    line += 1;
                }
                blank(&mut code, ch);
                i += 1;
                if ch == '"' {
                    break;
                }
            }
        } else if c == '\'' && is_char_literal(&chars, i) {
            blank(&mut code, c);
            i += 1;
            while i < chars.len() {
                let ch = chars[i];
                blank(&mut code, ch);
                i += 1;
                if ch == '\\' {
                    if let Some(&escaped) = chars.get(i) {
                        blank(&mut code, escaped);
                        i += 1;
                    }
                } else if ch == '\'' {
                    break;
                }
            }
        } else {
            if c == '\n' {
                line += 1;
            }
            code.push(c);
            i += 1;
        }
    }
    Scanned { code, comment_lines }
}

fn raw_string_start(chars: &[char], i: usize) -> Option<usize> {
    let starts_ident = i > 0 && (chars[i - 1].is_alphanumeric() || chars[i - 1] == '_');
    if starts_ident {
        return None;
    }
    let mut j = i;
    if chars[j] == 'b' {
        j += 1;
    }
    if chars.get(j) != Some(&'r') {
        return None;
    }
    j += 1;
    let hashes = chars[j..].iter().take_while(|&&c| c == '#').count();
    (chars.get(j + hashes) == Some(&'"')).then_some(hashes)
}

fn is_char_literal(chars: &[char], i: usize) -> bool {
    match chars.get(i + 1) {
        Some('\\') => true,
        Some(_) => chars.get(i + 2) == Some(&'\''),
        None => false,
    }
}

fn contains_token(line: &str, pattern: &str) -> bool {
    let is_word_char = |c: char| c.is_alphanumeric() || c == '_';
    line.match_indices(pattern).any(|(start, _)| {
        let before = line[..start].chars().next_back();
        let after = line[start + pattern.len()..].chars().next();
        let starts_clean = !pattern.starts_with(is_word_char) || !before.is_some_and(is_word_char);
        let ends_clean = !pattern.ends_with(is_word_char) || !after.is_some_and(is_word_char);
        starts_clean && ends_clean
    })
}
