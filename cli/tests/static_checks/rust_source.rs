pub(crate) struct Scanned {
    pub(crate) code: String,
    pub(crate) comment_lines: Vec<usize>,
}

pub(crate) fn scan(source: &str) -> Scanned {
    let mut lexer =
        Lexer { chars: source.chars().collect(), at: 0, line: 1, code: String::new(), comment_lines: Vec::new() };
    while lexer.at < lexer.chars.len() {
        lexer.step();
    }
    Scanned { code: lexer.code, comment_lines: lexer.comment_lines }
}

struct Lexer {
    chars: Vec<char>,
    at: usize,
    line: usize,
    code: String,
    comment_lines: Vec<usize>,
}

impl Lexer {
    fn peek(&self, ahead: usize) -> Option<char> {
        self.chars.get(self.at + ahead).copied()
    }

    fn step(&mut self) {
        match (self.chars[self.at], self.peek(1)) {
            ('/', Some('/')) => self.line_comment(),
            ('/', Some('*')) => self.block_comment(),
            (c, _) => self.code_or_literal(c),
        }
    }

    fn code_or_literal(&mut self, c: char) {
        if let Some(hashes) = self.raw_string_start() {
            self.raw_string(c, hashes);
        } else if c == '"' {
            self.string();
        } else if c == '\'' && self.is_char_literal() {
            self.char_literal();
        } else {
            self.keep(c);
        }
    }

    fn keep(&mut self, c: char) {
        if c == '\n' {
            self.line += 1;
        }
        self.code.push(c);
        self.at += 1;
    }

    fn blank(&mut self) {
        let c = self.chars[self.at];
        if c == '\n' {
            self.line += 1;
        }
        self.code.push(if c == '\n' { '\n' } else { ' ' });
        self.at += 1;
    }

    fn blank_many(&mut self, count: usize) {
        for _ in 0..count {
            self.blank();
        }
    }

    fn line_comment(&mut self) {
        self.comment_lines.push(self.line);
        while self.peek(0).is_some_and(|c| c != '\n') {
            self.blank();
        }
    }

    fn block_comment(&mut self) {
        self.comment_lines.push(self.line);
        let mut depth = 0;
        while self.at < self.chars.len() {
            match (self.chars[self.at], self.peek(1)) {
                ('/', Some('*')) => depth += 1,
                ('*', Some('/')) => depth -= 1,
                _ => {
                    self.blank();
                    continue;
                }
            }
            self.blank_many(2);
            if depth == 0 {
                return;
            }
        }
    }

    fn raw_string_start(&self) -> Option<usize> {
        let before = self.at.checked_sub(1).map(|before| self.chars[before]);
        if before.is_some_and(|c| c.is_alphanumeric() || c == '_') {
            return None;
        }
        let r = self.at + usize::from(self.chars[self.at] == 'b');
        if self.chars.get(r) != Some(&'r') {
            return None;
        }
        let hashes = self.chars[r + 1..].iter().take_while(|&&c| c == '#').count();
        (self.chars.get(r + 1 + hashes) == Some(&'"')).then_some(hashes)
    }

    fn raw_string(&mut self, c: char, hashes: usize) {
        let prefix = if c == 'b' { 2 } else { 1 };
        self.blank_many(prefix + hashes + 1);
        while self.at < self.chars.len() {
            if self.chars[self.at] == '"' && self.closes_raw_string(hashes) {
                self.blank_many(1 + hashes);
                return;
            }
            self.blank();
        }
    }

    fn closes_raw_string(&self, hashes: usize) -> bool {
        self.chars[self.at + 1..].iter().take(hashes).filter(|&&h| h == '#').count() == hashes
    }

    fn string(&mut self) {
        self.blank();
        while self.at < self.chars.len() {
            let c = self.chars[self.at];
            self.blank();
            if c == '\\' && self.at < self.chars.len() {
                self.blank();
            } else if c == '"' {
                return;
            }
        }
    }

    fn is_char_literal(&self) -> bool {
        match self.peek(1) {
            Some('\\') => true,
            Some(_) => self.peek(2) == Some('\''),
            None => false,
        }
    }

    fn char_literal(&mut self) {
        self.blank();
        while self.at < self.chars.len() {
            let c = self.chars[self.at];
            self.blank();
            if c == '\\' && self.at < self.chars.len() {
                self.blank();
            } else if c == '\'' {
                return;
            }
        }
    }
}

#[test]
fn the_scanner_finds_comments_but_not_strings_or_lifetimes() {
    let source = "fn f<'a>(x: &'a str) -> char {\n    let url = \"http://x\";\n    let raw = r#\"/* no */\"#;\n    let c = '/';\n    'x' // here\n}\n/* block\n */\n/// doc\n";
    let scanned = scan(source);
    assert_eq!(scanned.comment_lines, vec![5, 7, 9]);
    assert!(!scanned.code.contains("http"));
}
