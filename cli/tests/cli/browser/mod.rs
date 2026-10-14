mod activity;
mod empty;
mod info;
mod output;
mod sidebar;
mod split;
mod terminal;

use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::support::AgentTerminal;

pub struct Dashboard {
    pub port: u16,
}

impl Dashboard {
    pub fn start(at: &AgentTerminal) -> Self {
        let started = at.run(&["--json", "dashboard", "start", "--port", "0"]).success().json();
        Self { port: started["port"].as_u64().expect("the dashboard's port") as u16 }
    }

    pub fn url(&self, path: &str) -> String {
        format!("http://localhost:{}{path}", self.port)
    }
}

pub struct Browser {
    session: String,
}

static OPENED: AtomicUsize = AtomicUsize::new(0);

impl Browser {
    pub fn open(url: &str) -> Self {
        let n = OPENED.fetch_add(1, Ordering::SeqCst);
        let browser = Self { session: format!("agent-terminal-test-{}-{n}", std::process::id()) };
        browser.run(&["set", "viewport", "1440", "900"]);
        browser.run(&["open", url]);
        browser
    }

    fn command(&self, args: &[&str]) -> Output {
        Command::new("agent-browser")
            .arg("--session")
            .arg(&self.session)
            .args(args)
            .output()
            .expect("run agent-browser (install it with `npm i -g agent-browser && agent-browser install`)")
    }

    #[track_caller]
    pub fn run(&self, args: &[&str]) -> String {
        let output = self.command(args);
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        assert!(
            output.status.success(),
            "agent-browser {args:?} failed\nstdout:\n{stdout}\nstderr:\n{}\npage:\n{}",
            String::from_utf8_lossy(&output.stderr),
            String::from_utf8_lossy(&self.command(&["snapshot"]).stdout)
        );
        stdout
    }

    #[track_caller]
    pub fn wait_for_text(&self, text: &str) {
        self.run(&["wait", "--text", text]);
    }

    #[track_caller]
    pub fn wait_until(&self, condition: &str) {
        self.run(&["wait", "--fn", condition]);
    }

    #[track_caller]
    pub fn wait_for_screen(&self, text: &str) {
        self.wait_for_screen_in("main", text);
    }

    #[track_caller]
    pub fn wait_for_screen_in(&self, scope: &str, text: &str) {
        self.wait_until(&format!(
            "document.querySelector({})?.textContent.includes({})",
            js_string(&format!("{scope} .xterm-accessibility-tree")),
            js_string(text)
        ));
    }

    #[track_caller]
    pub fn wait_for_status(&self, chip: &str) {
        self.wait_until(&format!(
            "document.getElementById('status-region')?.innerText.split('\\n').map((part) => part.trim()).includes({})",
            js_string(chip)
        ));
    }

    pub fn text(&self, selector: &str) -> String {
        self.run(&["get", "text", selector])
    }

    pub fn snapshot(&self, selector: &str) -> String {
        self.run(&["snapshot", "-s", selector])
    }

    pub fn eval(&self, script: &str) -> String {
        self.run(&["eval", script]).trim().to_string()
    }

    pub fn click_button(&self, name: &str) {
        self.run(&["find", "role", "button", "click", "--name", name, "--exact"]);
    }

    pub fn screen(&self) -> Vec<String> {
        self.screen_in("main")
    }

    pub fn screen_in(&self, scope: &str) -> Vec<String> {
        let mut rows: Vec<String> = Vec::new();
        for line in self.snapshot(&format!("{scope} .xterm-accessibility-tree")).lines().map(str::trim) {
            if line.starts_with("- listitem") {
                rows.push(String::new());
            } else if let (Some(row), Some(text)) = (rows.last_mut(), line.strip_prefix("- StaticText ")) {
                *row = serde_json::from_str::<String>(text).expect("a quoted row").trim_end().to_string();
            }
        }
        let shown = rows.iter().rposition(|row| !row.is_empty()).map_or(0, |last| last + 1);
        rows.truncate(shown);
        rows
    }

    pub fn status(&self) -> Vec<String> {
        self.text("#status-region")
            .lines()
            .map(|part| part.trim().to_string())
            .filter(|part| !part.is_empty())
            .collect()
    }
}

impl Drop for Browser {
    fn drop(&mut self) {
        let _ = self.command(&["close"]);
    }
}

pub fn js_string(text: &str) -> String {
    serde_json::to_string(text).expect("a JSON string")
}

pub fn snapshot_rows(at: &AgentTerminal, args: &[&str]) -> (Vec<String>, u64) {
    let mut call = vec!["--json"];
    call.extend_from_slice(args);
    call.push("snapshot");
    let snapshot = at.run(&call).success().json();
    let mut rows: Vec<String> = snapshot["lines"]
        .as_array()
        .expect("snapshot lines")
        .iter()
        .map(|row| row.as_str().unwrap().to_string())
        .collect();
    let shown = rows.iter().rposition(|row| !row.is_empty()).map_or(0, |last| last + 1);
    rows.truncate(shown);
    (rows, snapshot["frame"].as_u64().expect("the frame number"))
}
