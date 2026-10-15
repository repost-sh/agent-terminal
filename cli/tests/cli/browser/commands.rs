use std::path::PathBuf;

use super::{Browser, Dashboard};
use crate::support::{AgentTerminal, Run};

const COMMANDS: &str = "ol[aria-label='Commands in default t1']";

struct ShellTab {
    at: AgentTerminal,
    home: PathBuf,
    work: PathBuf,
}

impl ShellTab {
    fn opened() -> Self {
        let at = AgentTerminal::new();
        let home = at.home().join("user");
        let work = home.join("projects").join("demo-app");
        std::fs::create_dir_all(&work).expect("create the work folder");
        let home = home.canonicalize().expect("canonical home");
        let work = work.canonicalize().expect("canonical work folder");
        let shell = Self { at, home, work };
        let cwd = shell.work.to_string_lossy().into_owned();
        shell.run(&["shell", "--shell", "bash", "--size", "60x12", "--cwd", &cwd]).success();
        shell
    }

    fn run(&self, args: &[&str]) -> Run {
        self.at.run_with_env(args, &[("HOME", self.home.as_path())])
    }
}

fn wait_for_listed(browser: &Browser, text: &str) {
    browser.wait_until(&format!("document.querySelector(\"{COMMANDS}\")?.innerText.includes({text:?})"));
}

fn open_commands(browser: &Browser) {
    browser.select_panel("Commands");
}

fn rows(browser: &Browser) -> Vec<Vec<String>> {
    let script = format!(
        "JSON.stringify([...document.querySelectorAll(\"{COMMANDS} > li > div > button\")].map((row) => row.innerText.split('\\n').map((part) => part.trim()).filter(Boolean)))"
    );
    let time = regex::Regex::new(r" · (\d+ ms|\d+\.\d s|running) · \d{2}:\d{2}:\d{2}$").unwrap();
    let rows: Vec<Vec<String>> = serde_json::from_str(&eval_string(browser, &script)).expect("rows as JSON");
    rows.into_iter()
        .map(|row| {
            row.into_iter()
                .map(|part| {
                    time.replace(&part, |found: &regex::Captures| {
                        let took = if &found[1] == "running" { "running" } else { "<duration>" };
                        format!(" · {took} · <time>")
                    })
                    .into_owned()
                })
                .collect()
        })
        .collect()
}

fn expanded_output(browser: &Browser, row: usize) -> String {
    browser.text(&format!("{COMMANDS} > li:nth-child({row}) > div > button + div"))
}

fn eval_string(browser: &Browser, script: &str) -> String {
    let value = browser.eval(script);
    serde_json::from_str(&value).unwrap_or_else(|e| panic!("{script} gave {value}, not a string: {e}"))
}

fn output_color(browser: &Browser, row: usize) -> String {
    let output = format!("{COMMANDS} > li:nth-child({row}) > div > button + div");
    eval_string(browser, &format!("getComputedStyle(document.querySelector(\"{output}\")).color"))
}

fn color_of(browser: &Browser, row: usize, text: &str) -> String {
    let output = format!("{COMMANDS} > li:nth-child({row}) > div > button + div");
    eval_string(
        browser,
        &format!(
            "getComputedStyle([...document.querySelectorAll(\"{output} span\")].find((span) => span.textContent === {text:?})).color"
        ),
    )
}

#[test]
fn the_commands_tab_lists_each_command_with_its_exit_status_and_expands_to_its_output() {
    let tab = ShellTab::opened();
    let colored = "printf '\\033[32mgreen\\033[0m passing\\n'";
    tab.run(&["exec", colored]).success();
    tab.run(&["exec", "echo failing; false"]).success();
    let dashboard = Dashboard::start(&tab.at);
    let browser = Browser::open(&dashboard.url("/s/default/t1"));
    open_commands(&browser);

    wait_for_listed(&browser, "echo failing; false");
    assert_eq!(
        rows(&browser),
        [
            vec!["✓", "exit 0", colored, "~/p/demo-app · <duration> · <time>", "+"],
            vec!["✗ 1", "exit 1", "echo failing; false", "~/p/demo-app · <duration> · <time>", "+"],
        ]
    );
    assert_eq!(browser.text("#ppanel-commands [data-slot=badge]").trim(), "2");

    browser.run(&["click", &format!("{COMMANDS} > li:nth-child(1) button")]);
    browser.wait_until(&format!(
        "document.querySelector(\"{COMMANDS} > li:nth-child(1) > div > button + div\")?.innerText.trim() === 'green passing'"
    ));
    assert_eq!(expanded_output(&browser, 1).trim(), "green passing");
    assert_eq!(color_of(&browser, 1, "green"), "rgb(74, 222, 128)");
    assert_ne!(output_color(&browser, 1), "rgb(74, 222, 128)");
    assert_eq!(rows(&browser)[0].last().map(String::as_str), Some("-"));

    browser.run(&["click", &format!("{COMMANDS} > li:nth-child(2) button")]);
    browser.wait_until(&format!(
        "document.querySelector(\"{COMMANDS} > li:nth-child(2) > div > button + div\")?.innerText.trim() === 'failing'"
    ));
    assert_eq!(expanded_output(&browser, 2).trim(), "failing");
    assert_eq!(
        eval_string(
            &browser,
            &format!(
                "[...document.querySelectorAll(\"{COMMANDS} > li > div > button\")].map((row) => row.getAttribute('aria-expanded')).join(' ')"
            )
        ),
        "false true"
    );
}

#[test]
fn new_commands_appear_as_they_finish_and_a_running_one_shows_as_running() {
    let tab = ShellTab::opened();
    let dashboard = Dashboard::start(&tab.at);
    let browser = Browser::open(&dashboard.url("/s/default/t1"));
    open_commands(&browser);
    browser.wait_for_text("No commands yet");
    assert_eq!(browser.text("#ppanel-commands [data-slot=badge]").trim(), "0");

    let hold = tab.work.join("hold");
    let status = std::process::Command::new("mkfifo").arg(&hold).status().expect("run mkfifo");
    assert!(status.success(), "mkfifo failed");
    assert_eq!(tab.run(&["exec", "cat hold", "--timeout", "300"]).code(), 3);
    wait_for_listed(&browser, "cat hold");
    assert_eq!(rows(&browser), [vec!["running", "cat hold", "~/p/demo-app · running · <time>", "+"]]);

    std::fs::write(&hold, "released\n").expect("release the command");
    tab.run(&["wait", "--command"]).success();
    browser.wait_until(&format!(
        "document.querySelector(\"{COMMANDS} > li:nth-child(1) button\")?.innerText.includes('✓')"
    ));
    assert_eq!(rows(&browser), [vec!["✓", "exit 0", "cat hold", "~/p/demo-app · <duration> · <time>", "+"]]);

    tab.run(&["exec", "echo later"]).success();
    wait_for_listed(&browser, "echo later");
    assert_eq!(
        rows(&browser),
        [
            vec!["✓", "exit 0", "cat hold", "~/p/demo-app · <duration> · <time>", "+"],
            vec!["✓", "exit 0", "echo later", "~/p/demo-app · <duration> · <time>", "+"],
        ]
    );
    assert_eq!(browser.text("#ppanel-commands [data-slot=badge]").trim(), "2");
}

#[test]
fn tabs_without_commands_say_why_and_how_to_get_them() {
    let at = AgentTerminal::new();
    let home = at.home().join("user");
    std::fs::create_dir_all(&home).expect("create the home folder");
    let user = [("HOME", home.as_path())];
    at.run_with_env(&["--session", "qa", "open", "--size", "40x6", "cat"], &user).success();
    at.run_with_env(&["--session", "qa", "tab", "new", "--size", "40x6", "--wait", "$", "bash"], &user).success();
    let dashboard = Dashboard::start(&at);

    let browser = Browser::open(&dashboard.url("/s/qa/t1"));
    open_commands(&browser);
    browser.wait_for_text("cat is not a shell");
    assert_eq!(
        browser.text("#ppanel-commands").lines().map(str::trim).filter(|line| !line.is_empty()).collect::<Vec<_>>(),
        [
            "Commands",
            "cat is not a shell",
            "Commands come from shell tabs, opened with",
            "agent-terminal shell",
            "Copy agent-terminal shell"
        ]
    );

    browser.run(&["open", &dashboard.url("/s/qa/t2")]);
    open_commands(&browser);
    browser.wait_for_text("Shell integration is off for this tab");
    assert_eq!(
        browser.text("#ppanel-commands").lines().map(str::trim).filter(|line| !line.is_empty()).collect::<Vec<_>>(),
        [
            "Commands",
            "Shell integration is off for this tab",
            "Commands are recorded in shells opened with integration:",
            "agent-terminal --session qa shell",
            "Copy agent-terminal --session qa shell"
        ]
    );
}
