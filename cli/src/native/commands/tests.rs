use std::time::{Duration, Instant, UNIX_EPOCH};

use super::{Change, Clock, Commands, Point, epoch_ms};
use crate::native::output_log::Offsets;
use crate::native::screen::ShellReport;

fn at(text: u64, frame: u64) -> Point {
    Point { output: Offsets { text, styled: text * 2 }, frame }
}

fn started(command: &str, cwd: Option<&str>) -> ShellReport {
    ShellReport::Started { command: command.to_string(), cwd: cwd.map(String::from) }
}

fn finished(exit_code: Option<i32>) -> ShellReport {
    ShellReport::Finished { exit_code }
}

fn shown(commands: &Commands) -> String {
    let prompt = commands.prompt();
    format!(
        "ready {}, {} prompts, running {:?}, last exit {:?}, next id {}",
        prompt.ready,
        prompt.prompts,
        commands.running(),
        commands.last_exit_code(),
        commands.next_id()
    )
}

fn steps(commands: &mut Commands, reports: Vec<ShellReport>) -> String {
    let mut lines = vec![format!("at first: {}", shown(commands))];
    for (step, report) in (1..).zip(reports) {
        let named = match &report {
            ShellReport::Prompt => "prompt".to_string(),
            ShellReport::Ready => "ready".to_string(),
            ShellReport::Started { command, .. } => format!("started {command}"),
            ShellReport::Finished { exit_code } => format!("finished {exit_code:?}"),
        };
        let changed = match commands.report(report, at(step * 10, step)) {
            None => "nothing".to_string(),
            Some(Change::Started) => "a start".to_string(),
            Some(Change::Finished { id }) => format!("the end of {id}"),
        };
        lines.push(format!("{named} -> {changed}: {}", shown(commands)));
    }
    lines.join("\n")
}

#[test]
fn a_prompt_is_ready_until_a_command_is_entered() {
    let mut commands = Commands::new("/home/me/work".into(), 1);
    let reported = steps(&mut commands, vec![ShellReport::Prompt, ShellReport::Ready]);
    commands.entered();
    insta::assert_snapshot!(format!("{reported}\nentered: {}", shown(&commands)), @r##"
        at first: ready false, 0 prompts, running None, last exit None, next id 1
        prompt -> nothing: ready false, 1 prompts, running None, last exit None, next id 1
        ready -> nothing: ready true, 1 prompts, running None, last exit None, next id 1
        entered: ready false, 1 prompts, running None, last exit None, next id 1
    "##);
    let submitted = commands.submitted();
    assert_eq!((submitted.prompts, submitted.first_id), (1, 1));
}

#[test]
fn a_command_runs_from_its_start_to_its_finish_and_reports_out_of_turn_change_nothing() {
    let mut commands = Commands::new("/home/me/work".into(), 1);
    let reports = vec![
        finished(Some(1)),
        started("make", None),
        ShellReport::Ready,
        started("again", None),
        finished(Some(2)),
        finished(Some(0)),
        ShellReport::Prompt,
        started("sleep 9", Some("/tmp")),
        ShellReport::Prompt,
    ];
    insta::assert_snapshot!(steps(&mut commands, reports), @r##"
        at first: ready false, 0 prompts, running None, last exit None, next id 1
        finished Some(1) -> nothing: ready false, 0 prompts, running None, last exit None, next id 1
        started make -> a start: ready false, 0 prompts, running Some("make"), last exit None, next id 2
        ready -> nothing: ready false, 0 prompts, running Some("make"), last exit None, next id 2
        started again -> nothing: ready false, 0 prompts, running Some("make"), last exit None, next id 2
        finished Some(2) -> the end of 1: ready false, 0 prompts, running None, last exit Some(2), next id 2
        finished Some(0) -> nothing: ready false, 0 prompts, running None, last exit Some(2), next id 2
        prompt -> nothing: ready false, 1 prompts, running None, last exit Some(2), next id 2
        started sleep 9 -> a start: ready false, 1 prompts, running Some("sleep 9"), last exit None, next id 3
        prompt -> the end of 2: ready false, 2 prompts, running None, last exit None, next id 3
    "##);
}

fn kept(commands: &Commands, id: u32) -> String {
    let Some((record, from, to)) = commands.record(id) else {
        return format!("{id}: none");
    };
    let ended_after_start = record.ended_at_ms.map(|ended| ended >= record.started_at_ms);
    let mut shown = serde_json::to_value(record).unwrap();
    let fields = shown.as_object_mut().unwrap();
    for timing in ["started_at_ms", "ended_at_ms", "duration_ms"] {
        fields.remove(timing);
    }
    let to = to.map(|to| (to.text, to.styled));
    format!(
        "{id}: {shown} ended after its start {ended_after_start:?}, output {:?} to {to:?}",
        (from.text, from.styled)
    )
}

#[test]
fn each_command_is_kept_under_its_id_with_its_cwd_output_span_and_frames() {
    let mut commands = Commands::new("/home/me/work".into(), 7);
    commands.report(started("make", None), at(10, 1));
    commands.report(finished(Some(0)), at(20, 3));
    commands.report(started("vim", Some("/tmp")), at(30, 4));
    insta::assert_snapshot!([6, 7, 8, 9].map(|id| kept(&commands, id)).join("\n"), @r##"
        6: none
        7: {"command":"make","cwd":"/home/me/work","exit_code":0,"first_frame":1,"id":7,"last_frame":3} ended after its start Some(true), output (10, 20) to Some((20, 40))
        8: {"command":"vim","cwd":"/tmp","exit_code":null,"first_frame":4,"id":8,"last_frame":null} ended after its start None, output (30, 60) to None
        9: none
    "##);
    let ids: Vec<u32> = commands.records().iter().map(|record| record.id).collect();
    assert_eq!(ids, [7, 8]);
}

#[test]
fn a_command_is_stamped_with_the_wall_clock_of_the_moment_it_started() {
    let mut commands = Commands::new("/home/me/work".into(), 1);
    let origin = Instant::now().checked_sub(Duration::from_secs(5)).unwrap();
    commands.clock = Clock { wall: UNIX_EPOCH + Duration::from_secs(1_000), origin };
    commands.report(started("make", None), at(0, 0));
    let started_at_ms = commands.records().first().unwrap().started_at_ms;
    assert!((1_005_000..1_065_000).contains(&started_at_ms), "{started_at_ms}");
    assert_eq!(
        [epoch_ms(UNIX_EPOCH + Duration::from_millis(1_500)), epoch_ms(UNIX_EPOCH - Duration::from_secs(1))],
        [1_500, 0]
    );
}
