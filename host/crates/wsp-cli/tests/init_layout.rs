// SPDX-License-Identifier: AGPL-3.0-only
//! Port of `packages/host/test/init-layout.test.ts` and the recorded values of `fixtures/layout_oracle.json`
//! (node 22, @clack/core 1.4.3, @clack/prompts 1.7.0).
use std::collections::BTreeSet;
use std::future::Future;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use regex::Regex;
use serde_json::Value;
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};
use wsp_cli::hosts::Env;
use wsp_cli::init_layout::{
    accent, card, cell_width, cells, colour_depth, confirm_prompt, ellipsize, glyphs, grey, help_line, is_tty, muted, node_color_depth,
    pad_cells, password_prompt, rewind, rows_of, style_text, styles_on, summarize, table, unicode_in, viewport, width_of, wrap, Align,
    Answer, Glyphs, HelpKey, Paint, PromptOptions, ScreenOut, Window, GREY_BRIGHT, GREY_DIM, GREY_MID,
};

fn oracle() -> Value {
    serde_json::from_str(include_str!("fixtures/layout_oracle.json")).expect("the oracle parses")
}

fn env_of(v: &Value) -> Env {
    let mut env = Env::new();
    for (k, val) in v.as_object().expect("an env is an object") {
        env.insert(k.clone(), val.as_str().expect("an env value is a string").to_string());
    }
    env
}

fn env(pairs: &[(&str, &str)]) -> Env {
    pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

fn strs(v: &Value) -> Vec<String> {
    v.as_array().expect("an array").iter().map(|s| s.as_str().expect("a string").to_string()).collect()
}

fn owned(rows: &[&[&str]]) -> Vec<Vec<String>> {
    rows.iter().map(|r| r.iter().map(|c| c.to_string()).collect()).collect()
}

fn align_of(s: &str) -> Align {
    match s {
        "left" => Align::Left,
        "right" => Align::Right,
        other => panic!("an align the oracle does not hold: {other}"),
    }
}

fn csi() -> Regex {
    Regex::new(r"\x1b\[[0-?]*[ -/]*[@-~]").unwrap()
}

fn strip(s: &str) -> String {
    csi().replace_all(s, "").into_owned()
}

fn sgr_set(raw: &str) -> BTreeSet<String> {
    Regex::new(r"\x1b\[([0-9;]*)m").unwrap().captures_iter(raw).map(|c| c[1].to_string()).collect()
}

fn set(items: &[&str]) -> BTreeSet<String> {
    items.iter().map(|s| s.to_string()).collect()
}

fn after_last<'a>(s: &'a str, needle: &str) -> &'a str {
    &s[s.rfind(needle).unwrap_or_else(|| panic!("no {needle:?} in {s:?}"))..]
}

fn unicode_glyphs() -> Glyphs {
    Glyphs {
        bar: "│",
        bar_focus: "┃",
        bar_focus_end: "┗",
        step_active: "◆",
        step_cancel: "■",
        step_submit: "◇",
        radio_active: "●",
        radio_inactive: "○",
        mask: "▪",
    }
}

fn ascii_glyphs() -> Glyphs {
    Glyphs {
        bar: "|",
        bar_focus: "|",
        bar_focus_end: "+",
        step_active: "*",
        step_cancel: "x",
        step_submit: "o",
        radio_active: ">",
        radio_inactive: " ",
        mask: "*",
    }
}

// ---------------------------------------------------------------------------------------------------------------
// A stream that records what is written to it.

struct Screen {
    buf: Mutex<String>,
    columns: Option<u16>,
    rows: Option<u16>,
    tty: bool,
}

impl ScreenOut for Screen {
    fn write(&self, text: &str) {
        self.buf.lock().unwrap().push_str(text);
    }
    fn columns(&self) -> Option<u16> {
        self.columns
    }
    fn rows(&self) -> Option<u16> {
        self.rows
    }
    fn is_tty(&self) -> bool {
        self.tty
    }
}

impl Screen {
    fn new(columns: Option<u16>) -> Arc<Screen> {
        Arc::new(Screen { buf: Mutex::new(String::new()), columns, rows: None, tty: false })
    }
    fn raw(&self) -> String {
        self.buf.lock().unwrap().clone()
    }
    fn text(&self) -> String {
        strip(&self.raw())
    }
}

fn paint(on: bool, depth: u8) -> Paint {
    Paint { on, depth, unicode: true, stdout_columns: None }
}
fn off() -> Paint {
    paint(false, 1)
}
fn force_1() -> Paint {
    paint(true, 4)
}
fn force_3() -> Paint {
    paint(true, 24)
}

const KEY_LEFT: &str = "\x1b[D";
const KEY_RIGHT: &str = "\x1b[C";
const KEY_ENTER: &str = "\r";
const KEY_ESC: &str = "\x1b";
const KEY_CTRL_C: &str = "\x03";
const SECRET: &str = "slr_live_fake_solari_key";
const CONFIRM_HELP: &str = "← → change   y n answer   enter choose   esc cancel";
const PASSWORD_HELP: &str = "enter next   esc cancel";
const SNAP: &str = "@snap";
const RAW: &str = "@raw";

/// Runs the prompt beside a driver that types the script: a key is sent then 5 ms is left; a lone escape is only an
/// escape after a pause, so 700 ms follows it. `@snap` takes the text so far (escape codes out), `@raw` the bytes.
async fn drive<T>(
    prompt: impl Future<Output = Answer<T>>,
    screen: &Arc<Screen>,
    tx: UnboundedSender<String>,
    script: &[&str],
) -> (Answer<T>, Vec<String>) {
    let driver = async {
        let mut snaps = Vec::new();
        tokio::time::sleep(Duration::from_millis(5)).await;
        for k in script {
            match *k {
                SNAP => snaps.push(screen.text()),
                RAW => snaps.push(screen.raw()),
                key => {
                    let _ = tx.send(key.to_string());
                    tokio::time::sleep(Duration::from_millis(5)).await;
                    if key == KEY_ESC {
                        tokio::time::sleep(Duration::from_millis(700)).await;
                    }
                }
            }
        }
        (snaps, tx)
    };
    let (answer, (snaps, _tx)) =
        tokio::time::timeout(Duration::from_secs(20), async { tokio::join!(prompt, driver) }).await.expect("the prompt never answered");
    (answer, snaps)
}

fn options(message: &str, hint: Option<&str>, screen: &Arc<Screen>, paint: Paint) -> (PromptOptions, UnboundedSender<String>) {
    let (tx, rx) = unbounded_channel::<String>();
    let output: Arc<dyn ScreenOut> = screen.clone();
    (PromptOptions { message: message.to_string(), hint: hint.map(str::to_string), input: rx, output, paint, signal: None }, tx)
}

async fn run_confirm(
    message: &str,
    hint: Option<&str>,
    initial: Option<bool>,
    columns: Option<u16>,
    paint: Paint,
    script: &[&str],
) -> (Answer<bool>, Arc<Screen>, Vec<String>) {
    let screen = Screen::new(columns);
    let (o, tx) = options(message, hint, &screen, paint);
    let (answer, snaps) = drive(confirm_prompt(o, initial), &screen, tx, script).await;
    (answer, screen, snaps)
}

async fn run_password(
    message: &str,
    hint: Option<&str>,
    columns: Option<u16>,
    paint: Paint,
    script: &[&str],
) -> (Answer<String>, Arc<Screen>, Vec<String>) {
    let screen = Screen::new(columns);
    let (o, tx) = options(message, hint, &screen, paint);
    let (answer, snaps) = drive(password_prompt(o), &screen, tx, script).await;
    (answer, screen, snaps)
}

fn run_card(title: &str, lines: &[String], columns: Option<u16>, paint: &Paint) -> String {
    let screen = Screen::new(columns);
    card(title, lines, screen.as_ref(), paint);
    screen.raw()
}

// ---------------------------------------------------------------------------------------------------------------
// Values recorded from the TS.

/// oracle
#[test]
fn colour_depth_matches_node_for_every_recorded_env_on_and_off_a_terminal() {
    let cases = oracle()["depth"].as_array().unwrap().clone();
    let got: Vec<(u8, u8)> = cases
        .iter()
        .map(|c| {
            let e = env_of(&c["env"]);
            (colour_depth(true, &e), colour_depth(false, &e))
        })
        .collect();
    let want: Vec<(u8, u8)> = cases.iter().map(|c| (c["tty"].as_u64().unwrap() as u8, c["notty"].as_u64().unwrap() as u8)).collect();
    assert_eq!(got, want);
}

/// oracle: the terminal column is node's own reading of the env.
#[test]
fn node_color_depth_matches_every_recorded_terminal_depth() {
    let cases = oracle()["depth"].as_array().unwrap().clone();
    let got: Vec<u8> = cases.iter().map(|c| node_color_depth(&env_of(&c["env"]))).collect();
    let want: Vec<u8> = cases.iter().map(|c| c["tty"].as_u64().unwrap() as u8).collect();
    assert_eq!(got, want);
}

/// oracle
#[test]
fn cell_width_matches_every_recorded_text() {
    let cases = oracle()["cellWidth"].as_array().unwrap().clone();
    let got: Vec<usize> = cases.iter().map(|c| cell_width(c[0].as_str().unwrap())).collect();
    let want: Vec<usize> = cases.iter().map(|c| c[1].as_u64().unwrap() as usize).collect();
    assert_eq!(got, want);
}

/// oracle
#[test]
fn ellipsize_matches_every_recorded_cut() {
    let cases = oracle()["ellipsize"].as_array().unwrap().clone();
    let got: Vec<String> = cases.iter().map(|c| ellipsize(c[0].as_str().unwrap(), c[1].as_i64().unwrap())).collect();
    let want: Vec<String> = cases.iter().map(|c| c[2].as_str().unwrap().to_string()).collect();
    assert_eq!(got, want);
}

/// oracle
#[test]
fn wrap_matches_every_recorded_break_with_the_default_and_a_given_indent() {
    let cases = oracle()["wrap"].as_array().unwrap().clone();
    let got: Vec<Vec<String>> = cases
        .iter()
        .map(|c| {
            let indent = c[2].as_str().unwrap_or("  ");
            wrap(c[0].as_str().unwrap(), c[1].as_u64().unwrap() as usize, indent)
        })
        .collect();
    let want: Vec<Vec<String>> = cases.iter().map(|c| strs(&c[3])).collect();
    assert_eq!(got, want);
}

/// oracle
#[test]
fn summarize_matches_every_recorded_summary() {
    let cases = oracle()["summarize"].as_array().unwrap().clone();
    let got: Vec<String> = cases.iter().map(|c| summarize(&strs(&c[0]), c[1].as_i64().unwrap(), 3)).collect();
    let want: Vec<String> = cases.iter().map(|c| c[2].as_str().unwrap().to_string()).collect();
    assert_eq!(got, want);
}

/// oracle
#[test]
fn viewport_matches_every_recorded_window() {
    let cases = oracle()["viewport"].as_array().unwrap().clone();
    let got: Vec<Window> =
        cases.iter().map(|c| viewport(c[0].as_u64().unwrap() as usize, c[1].as_u64().unwrap() as usize, c[2].as_i64().unwrap())).collect();
    let want: Vec<Window> = cases
        .iter()
        .map(|c| Window { start: c[3]["start"].as_u64().unwrap() as usize, end: c[3]["end"].as_u64().unwrap() as usize })
        .collect();
    assert_eq!(got, want);
}

/// oracle
#[test]
fn rewind_matches_every_recorded_block() {
    let cases = oracle()["rewind"].as_array().unwrap().clone();
    let got: Vec<String> = cases
        .iter()
        .map(|c| {
            let drawn: Vec<usize> = c[0].as_array().unwrap().iter().map(|n| n.as_u64().unwrap() as usize).collect();
            rewind(&drawn, c[1].as_u64().unwrap() as u16)
        })
        .collect();
    let want: Vec<String> = cases.iter().map(|c| c[2].as_str().unwrap().to_string()).collect();
    assert_eq!(got, want);
}

/// oracle
#[test]
fn table_matches_every_recorded_table() {
    let cases = oracle()["table"].as_array().unwrap().clone();
    let got: Vec<Vec<String>> = cases
        .iter()
        .map(|c| {
            let rows: Vec<Vec<String>> = c[0].as_array().unwrap().iter().map(strs).collect();
            let aligns: Vec<Align> = strs(&c[1]).iter().map(|a| align_of(a)).collect();
            table(&rows, &aligns)
        })
        .collect();
    let want: Vec<Vec<String>> = cases.iter().map(|c| strs(&c[2])).collect();
    assert_eq!(got, want);
}

/// oracle
#[test]
fn pad_cells_matches_every_recorded_pad() {
    let cases = oracle()["pad"].as_array().unwrap().clone();
    let got: Vec<String> = cases
        .iter()
        .map(|c| pad_cells(c[0].as_str().unwrap(), c[1].as_u64().unwrap() as usize, align_of(c[2].as_str().unwrap())))
        .collect();
    let want: Vec<String> = cases.iter().map(|c| c[3].as_str().unwrap().to_string()).collect();
    assert_eq!(got, want);
}

/// oracle: painted, as the recording ran with FORCE_COLOR=3.
#[test]
fn muted_matches_every_recorded_depth() {
    let cases = oracle()["muted"].as_array().unwrap().clone();
    let got: Vec<String> = cases.iter().map(|c| muted("$ git status", c[0].as_u64().unwrap() as u8, true)).collect();
    let want: Vec<String> = cases.iter().map(|c| c[1].as_str().unwrap().to_string()).collect();
    assert_eq!(got, want);
}

/// oracle: painted, as the recording ran with FORCE_COLOR=3.
#[test]
fn help_line_matches_every_recorded_depth() {
    let keys = [
        HelpKey { key: "space".into(), does: "tick".into() },
        HelpKey { key: "← →".into(), does: "fold".into() },
        HelpKey { key: "esc".into(), does: "back".into() },
    ];
    let cases = oracle()["help"].as_array().unwrap().clone();
    let got: Vec<String> = cases.iter().map(|c| help_line(&keys, c[0].as_u64().unwrap() as u8, true)).collect();
    let want: Vec<String> = cases.iter().map(|c| c[1].as_str().unwrap().to_string()).collect();
    assert_eq!(got, want);
}

/// oracle
#[test]
fn help_line_with_no_keys_is_empty_at_every_depth() {
    assert_eq!(oracle()["help0"].as_str().unwrap(), "");
    let got: Vec<String> = [1u8, 4, 8, 24].iter().map(|d| help_line(&[], *d, true)).collect();
    assert_eq!(got, vec![String::new(); 4]);
}

/// oracle: the card at 40 columns with no colour.
#[test]
fn card_matches_the_recorded_bytes_at_forty_columns_without_colour() {
    let red = "\x1b[31mDisk  12.5 GB of 16.1 GB, red line\x1b[39m";
    let lines: Vec<String> = [
        "Identity  2 of 3  1.7 KB",
        "  GitHub CLI login  copy",
        "",
        "Installs  Claude Code, Codex, 89 tools plus Homebrew's toolchain",
        red,
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    let raw = run_card("Summary", &lines, Some(40), &off());
    assert_eq!(raw, oracle()["card"].as_str().unwrap());
}

/// oracle: FORCE_COLOR=3, a title and the lines "one" and "" (the recording's input is read off its output).
#[test]
fn card_matches_the_recorded_bytes_with_colour() {
    let lines = vec!["one".to_string(), String::new()];
    let raw = run_card("Summary", &lines, None, &force_3());
    assert_eq!(raw, oracle()["cardColour"].as_str().unwrap());
}

// ---------------------------------------------------------------------------------------------------------------
// The recorded prompt runs, byte for byte.

#[derive(Debug, PartialEq)]
enum Got {
    Confirm(Answer<bool>),
    Password(Answer<String>),
}

fn case_paint(case: &Value) -> Paint {
    if case.get("noColor").and_then(Value::as_bool) == Some(true) {
        return off();
    }
    match case["force"].as_str() {
        None => off(),
        Some("1") => force_1(),
        Some("3") => force_3(),
        Some(other) => panic!("a FORCE_COLOR the oracle does not hold: {other}"),
    }
}

fn want_of(case: &Value) -> Got {
    let v = &case["value"];
    match case["kind"].as_str().unwrap() {
        "confirm" => Got::Confirm(if v == "cancel" { Answer::Cancel } else { Answer::Value(v.as_bool().unwrap()) }),
        "password" => Got::Password(if v == "cancel" { Answer::Cancel } else { Answer::Value(v.as_str().unwrap().to_string()) }),
        other => panic!("a prompt kind the oracle does not hold: {other}"),
    }
}

async fn replay(case: &Value) -> (String, Got) {
    let keys = strs(&case["keys"]);
    let script: Vec<&str> = keys.iter().map(String::as_str).collect();
    let opts = &case["opts"];
    let hint = opts.get("hint").and_then(Value::as_str);
    let columns = case["columns"].as_u64().map(|n| n as u16);
    let p = case_paint(case);
    match case["kind"].as_str().unwrap() {
        "confirm" => {
            let message = opts.get("message").and_then(Value::as_str).unwrap_or("Boot it?");
            let initial = opts.get("initialValue").and_then(Value::as_bool);
            let (answer, screen, _) = run_confirm(message, hint, initial, columns, p, &script).await;
            (screen.raw(), Got::Confirm(answer))
        }
        _ => {
            let message = opts["message"].as_str().unwrap();
            let (answer, screen, _) = run_password(message, hint, columns, p, &script).await;
            (screen.raw(), Got::Password(answer))
        }
    }
}

async fn replay_group(force: Option<&str>, no_color: bool, kind: Option<&str>) {
    let all = oracle()["prompts"].as_array().unwrap().clone();
    let cases: Vec<(usize, Value)> = all
        .into_iter()
        .enumerate()
        .filter(|(_, c)| {
            c["force"].as_str() == force
                && (c.get("noColor").and_then(Value::as_bool) == Some(true)) == no_color
                && kind.map_or(true, |k| c["kind"].as_str() == Some(k))
        })
        .collect();
    assert!(!cases.is_empty(), "the oracle holds no such prompts");
    for (i, case) in cases {
        let (raw, got) = replay(&case).await;
        assert_eq!(raw, case["raw"].as_str().unwrap(), "prompt case {i}: bytes written");
        assert_eq!(got, want_of(&case), "prompt case {i}: answer");
    }
}

/// oracle: prompts 0 to 7, no colour.
#[tokio::test]
async fn confirm_prompts_write_the_recorded_bytes_and_answer_as_recorded_without_colour() {
    replay_group(None, false, Some("confirm")).await;
}

/// oracle: prompts 8 to 12, no colour.
#[tokio::test]
async fn password_prompts_write_the_recorded_bytes_and_answer_as_recorded_without_colour() {
    replay_group(None, false, Some("password")).await;
}

/// oracle: prompts 13 to 20, FORCE_COLOR=1.
#[tokio::test]
async fn confirm_prompts_write_the_recorded_bytes_and_answer_as_recorded_at_16_colours() {
    replay_group(Some("1"), false, Some("confirm")).await;
}

/// oracle: prompts 21 to 25, FORCE_COLOR=1.
#[tokio::test]
async fn password_prompts_write_the_recorded_bytes_and_answer_as_recorded_at_16_colours() {
    replay_group(Some("1"), false, Some("password")).await;
}

/// oracle: prompts 26 to 33, FORCE_COLOR=3.
#[tokio::test]
async fn confirm_prompts_write_the_recorded_bytes_and_answer_as_recorded_at_truecolor() {
    replay_group(Some("3"), false, Some("confirm")).await;
}

/// oracle: prompts 34 to 38, FORCE_COLOR=3.
#[tokio::test]
async fn password_prompts_write_the_recorded_bytes_and_answer_as_recorded_at_truecolor() {
    replay_group(Some("3"), false, Some("password")).await;
}

/// oracle: prompts 39 and 40, NO_COLOR set.
#[tokio::test]
async fn prompts_under_no_color_write_the_recorded_bytes_and_answer_as_recorded() {
    replay_group(None, true, None).await;
}

// ---------------------------------------------------------------------------------------------------------------
// init-layout.test.ts, the pure rules.

/// init-layout.test.ts:11
#[test]
fn ellipsize_keeps_text_that_fits_and_ends_cut_text_with_one_ellipsis_inside_the_width() {
    let got = vec![ellipsize("abc", 3), ellipsize("abcdef", 4), ellipsize("abcdef", 1), ellipsize("abcdef", 0)];
    assert_eq!(got, vec!["abc", "abc…", "…", ""]);
}

/// init-layout.test.ts:18
#[test]
fn table_pads_each_column_to_its_widest_cell_right_aligns_where_asked_and_leaves_no_trailing_space() {
    let rows = owned(&[&["Identity", "10", "82.9 KB", "3 can come"], &["Tools", "161", "", "92 can come"], &["Shell", "9", "1.1 MB", ""]]);
    assert_eq!(
        table(&rows, &[Align::Left, Align::Right, Align::Right]),
        vec!["Identity   10  82.9 KB  3 can come", "Tools     161           92 can come", "Shell       9   1.1 MB"]
    );
}

/// init-layout.test.ts:34
#[test]
fn width_of_reads_the_streams_columns_falls_back_to_80_and_caps_at_100() {
    let got = vec![width_of(None, 100), width_of(Some(48), 100), width_of(Some(240), 100)];
    assert_eq!(got, vec![80, 48, 100]);
}

/// init-layout.test.ts:34 (the cap argument, whose default the TS spells as 100)
#[test]
fn width_of_caps_at_the_cap_it_is_given() {
    let got = vec![width_of(None, 60), width_of(Some(240), 120), width_of(Some(90), 120)];
    assert_eq!(got, vec![60, 120, 90]);
}

/// init-layout.test.ts:41
#[test]
fn rows_of_reads_the_streams_rows_and_falls_back_to_20() {
    assert_eq!(vec![rows_of(None), rows_of(Some(30))], vec![20, 30]);
}

/// init-layout.test.ts:47
#[test]
fn viewport_keeps_the_cursor_mid_window_pins_the_window_at_both_ends_and_never_goes_under_one_row() {
    let got = vec![viewport(20, 0, 5), viewport(20, 10, 5), viewport(20, 19, 5), viewport(3, 1, 5), viewport(20, 7, 0), viewport(0, 0, 4)];
    let want = vec![
        Window { start: 0, end: 5 },
        Window { start: 8, end: 13 },
        Window { start: 15, end: 20 },
        Window { start: 0, end: 3 },
        Window { start: 7, end: 8 },
        Window { start: 0, end: 0 },
    ];
    assert_eq!(got, want);
}

/// init-layout.test.ts:56
#[test]
fn summarize_names_up_to_three_labels_then_n_more_drops_labels_when_the_width_is_short_and_cuts_the_last_one_kept() {
    let l = |xs: &[&str]| xs.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    let long = "a".repeat(30);
    let got = vec![
        summarize(&l(&["a", "b"]), 80, 3),
        summarize(&l(&["a", "b", "c", "d", "e"]), 80, 3),
        summarize(&l(&["alpha", "beta", "gamma", "delta"]), 20, 3),
        summarize(&l(&["alpha", "beta", "gamma", "delta"]), 14, 3),
        summarize(&l(&[&long, "beta"]), 20, 3),
        summarize(&[], 20, 3),
    ];
    assert_eq!(got, vec!["a, b", "a, b, c +2 more", "alpha, beta +2 more", "alpha +3 more", "aaaaaaaaaaa… +1 more", ""]);
}

/// init-layout.test.ts:56 (the `named` argument)
#[test]
fn summarize_names_as_many_labels_as_it_is_told_to() {
    let l: Vec<String> = ["a", "b", "c", "d", "e"].iter().map(|s| s.to_string()).collect();
    assert_eq!(
        vec![summarize(&l, 80, 2), summarize(&l, 80, 4), summarize(&l, 80, 5)],
        vec!["a, b +3 more", "a, b, c, d +1 more", "a, b, c, d, e"]
    );
}

/// init-layout.test.ts:65
#[test]
fn wrap_breaks_a_line_at_word_ends_inside_the_width_indents_the_rest_by_two_and_cuts_a_word_longer_than_the_width() {
    let p = "/p".repeat(50);
    let ten = " ".repeat(10);
    let got = vec![
        wrap("short line", 20, "  "),
        wrap("Installs  Claude Code, Codex, 89 tools plus Homebrew's toolchain", 40, "  "),
        wrap(&"a".repeat(30), 10, "  "),
        wrap(&format!("  {}", "a".repeat(30)), 10, "  "),
        wrap("", 10, "  "),
        wrap("  GitHub CLI login  copy", 40, "  "),
        wrap("Upload    77.0 MB, nothing has left this computer yet", 30, "  "),
        wrap("Installs  Claude Code, Codex, 89 tools plus Homebrew's toolchain", 40, &ten),
        wrap(&format!("Save the key to {p} so wsp stops asking?"), 40, "  "),
        wrap(&format!("{p} so"), 40, "  "),
        wrap(&format!("Installs  {p} plus jq"), 40, &ten),
    ];
    let want: Vec<Vec<String>> = vec![
        vec!["short line".into()],
        vec!["Installs  Claude Code, Codex, 89 tools".into(), "  plus Homebrew's toolchain".into()],
        vec!["aaaaaaaaa…".into()],
        vec!["  aaaaaaa…".into()],
        vec!["".into()],
        vec!["  GitHub CLI login  copy".into()],
        vec!["Upload    77.0 MB, nothing has".into(), "  left this computer yet".into()],
        vec!["Installs  Claude Code, Codex, 89 tools".into(), "          plus Homebrew's toolchain".into()],
        vec!["Save the key to".into(), format!("  {}…", &p[..37]), "  so wsp stops asking?".into()],
        vec![format!("{}…", &p[..39]), "  so".into()],
        vec!["Installs".into(), format!("          {}…", &p[..29]), "          plus jq".into()],
    ];
    assert_eq!(got, want);
}

/// init-layout.test.ts:83
#[test]
fn card_prints_a_bold_title_on_the_step_glyph_and_its_lines_down_the_bar_an_empty_line_as_a_bare_bar_and_long_lines_wrapped_to_the_width() {
    let red = "\x1b[31mDisk  12.5 GB of 16.1 GB, red line\x1b[39m";
    let lines: Vec<String> = [
        "Identity  2 of 3  1.7 KB",
        "  GitHub CLI login  copy",
        "",
        "Installs  Claude Code, Codex, 89 tools plus Homebrew's toolchain",
        red,
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    let raw = run_card("Summary", &lines, Some(40), &off());
    let stripped = strip(&raw);
    let got: Vec<&str> = stripped.split('\n').collect();
    assert_eq!(
        got,
        vec![
            "│",
            "◇  Summary",
            "│  Identity  2 of 3  1.7 KB",
            "│    GitHub CLI login  copy",
            "│",
            "│  Installs  Claude Code, Codex, 89",
            "│    tools plus Homebrew's toolchain",
            "│  Disk  12.5 GB of 16.1 GB, red line",
            ""
        ]
    );
    assert!(raw.contains(&format!("│  {red}\n")), "the coloured line passes whole: {raw:?}");
    let too_wide: Vec<usize> = got[..got.len() - 1].iter().map(|l| l.chars().count()).filter(|n| *n > 40).collect();
    assert_eq!(too_wide, Vec::<usize>::new());
    assert!(!raw.chars().any(|c| "╮╯─├".contains(c)), "no box edges: {raw:?}");
}

/// init-layout.test.ts:99
#[test]
fn the_focused_bar_and_its_end_are_one_cell_wide_and_ascii_off_a_unicode_terminal() {
    let (u, a) = (glyphs(true), glyphs(false));
    assert_eq!((u, a), (unicode_glyphs(), ascii_glyphs()));
    let widths: Vec<usize> = [u.bar_focus, u.bar_focus_end, a.bar_focus, a.bar_focus_end].iter().map(|g| g.chars().count()).collect();
    assert_eq!(widths, vec![1, 1, 1, 1]);
}

/// init-layout.ts:13 (clack's isUnicodeSupported off Windows: TERM is not linux)
#[test]
fn unicode_is_on_unless_the_term_is_linux() {
    let got = vec![unicode_in(&env(&[])), unicode_in(&env(&[("TERM", "xterm-256color")])), unicode_in(&env(&[("TERM", "linux")]))];
    assert_eq!(got, vec![true, true, false]);
}

/// init-layout.test.ts:105
#[test]
fn colour_depth_is_1_off_a_terminal_and_when_the_env_says_no_colour_nodes_depth_for_term_and_colorterm_otherwise_and_force_color_wins() {
    let got = vec![
        colour_depth(false, &env(&[("TERM", "xterm-256color")])),
        colour_depth(true, &env(&[("TERM", "dumb")])),
        colour_depth(true, &env(&[("TERM", "xterm-256color"), ("NO_COLOR", "1")])),
        colour_depth(true, &env(&[("TERM", "xterm")])),
        colour_depth(true, &env(&[("TERM", "linux")])),
        colour_depth(true, &env(&[("TERM", "xterm-256color")])),
        colour_depth(true, &env(&[("TERM", "xterm-256color"), ("COLORTERM", "truecolor")])),
        colour_depth(false, &env(&[("FORCE_COLOR", "1")])),
        colour_depth(false, &env(&[("FORCE_COLOR", "3")])),
        colour_depth(false, &env(&[("FORCE_COLOR", "0")])),
    ];
    assert_eq!(got, vec![1, 1, 1, 4, 4, 8, 24, 4, 24, 1]);
}

/// init-layout.ts:119 (styleText paints on FORCE_COLOR above 2 bits, else on a terminal above 2 bits)
#[test]
fn styles_are_on_for_force_color_above_two_bits_and_otherwise_only_on_a_deep_enough_terminal() {
    let got = vec![
        styles_on(&env(&[("FORCE_COLOR", "3")]), false),
        styles_on(&env(&[("FORCE_COLOR", "1")]), false),
        styles_on(&env(&[("FORCE_COLOR", "0")]), true),
        styles_on(&env(&[("TERM", "xterm-256color")]), true),
        styles_on(&env(&[("TERM", "xterm-256color")]), false),
        styles_on(&env(&[("TERM", "dumb")]), true),
        styles_on(&env(&[]), true),
    ];
    assert_eq!(got, vec![true, true, false, true, false, false, false]);
}

/// init-layout.ts:124-127 (the doc comment's open and close codes)
#[test]
fn style_text_wraps_the_text_in_each_formats_codes_and_closes_them_in_reverse_and_is_the_text_alone_when_off() {
    let got = vec![
        style_text(&["dim"], "x", true),
        style_text(&["bold"], "x", true),
        style_text(&["strikethrough"], "x", true),
        style_text(&["cyan"], "x", true),
        style_text(&["green"], "x", true),
        style_text(&["red"], "x", true),
        style_text(&["gray"], "x", true),
        style_text(&["inverse"], "x", true),
        style_text(&["hidden"], "x", true),
        style_text(&["strikethrough", "dim"], "x", true),
        style_text(&["inverse", "hidden"], "x", true),
        style_text(&["strikethrough", "dim"], "x", false),
    ];
    let want = vec![
        "\x1b[2mx\x1b[22m",
        "\x1b[1mx\x1b[22m",
        "\x1b[9mx\x1b[29m",
        "\x1b[36mx\x1b[39m",
        "\x1b[32mx\x1b[39m",
        "\x1b[31mx\x1b[39m",
        "\x1b[90mx\x1b[39m",
        "\x1b[7mx\x1b[27m",
        "\x1b[8mx\x1b[28m",
        "\x1b[9m\x1b[2mx\x1b[22m\x1b[29m",
        "\x1b[7m\x1b[8mx\x1b[28m\x1b[27m",
        "x",
    ];
    assert_eq!(got, want);
}

/// init-layout.ts:125, 128, 131
#[test]
fn grey_paints_a_256_colour_code_and_the_accent_is_cyan_only_where_styles_are_on() {
    assert_eq!(grey(GREY_BRIGHT, "k"), "\x1b[38;5;247mk\x1b[39m");
    assert_eq!(grey(GREY_MID, "k"), "\x1b[38;5;243mk\x1b[39m");
    assert_eq!(grey(GREY_DIM, "k"), "\x1b[38;5;239mk\x1b[39m");
    assert_eq!((accent("hit", true), accent("hit", false)), ("\x1b[36mhit\x1b[39m".to_string(), "hit".to_string()));
}

/// init-layout.test.ts:118
#[test]
fn muted_paints_a_grey_from_256_colours_up_dim_under_them_and_nothing_off_a_terminal() {
    let got = vec![
        muted("$ git status", 1, false),
        muted("$ git status", 1, true),
        muted("$ git status", 4, true),
        muted("$ git status", 4, false),
        muted("$ git status", 8, false),
        muted("$ git status", 24, false),
    ];
    let grey243 = "\x1b[38;5;243m$ git status\x1b[39m";
    assert_eq!(got, vec!["$ git status", "$ git status", "\x1b[2m$ git status\x1b[22m", "$ git status", grey243, grey243]);
    let back = help_line(&[HelpKey { key: "esc".into(), does: "back".into() }], 8, false);
    assert!(back.contains(&muted("back", 8, false)), "{back:?}");
}

fn tick_keys() -> Vec<HelpKey> {
    vec![
        HelpKey { key: "space".into(), does: "tick".into() },
        HelpKey { key: "← →".into(), does: "fold".into() },
        HelpKey { key: "esc".into(), does: "back".into() },
    ]
}

/// init-layout.test.ts:132
#[test]
fn help_line_is_plain_at_depth_1_and_sets_keys_and_what_they_do_three_spaces_apart() {
    assert_eq!(help_line(&tick_keys(), 1, false), "space tick   ← → fold   esc back");
    assert_eq!(help_line(&tick_keys(), 1, true), "space tick   ← → fold   esc back");
}

/// init-layout.test.ts:132
#[test]
fn help_line_under_256_colours_leaves_keys_plain_and_dims_the_rest_with_no_other_code() {
    let sixteen = help_line(&tick_keys(), 4, true);
    assert_eq!(strip(&sixteen), "space tick   ← → fold   esc back");
    assert!(sixteen.contains("space \x1b[2mtick\x1b[22m"), "{sixteen:?}");
    assert!(!sixteen.contains("38;5;"), "{sixteen:?}");
    assert_eq!(sgr_set(&sixteen), set(&["2", "22"]));
    assert_eq!(help_line(&tick_keys(), 4, false), "space tick   ← → fold   esc back");
}

/// init-layout.test.ts:132
#[test]
fn help_line_from_256_colours_up_paints_two_greys_the_key_brighter() {
    let want = "\x1b[38;5;247mspace\x1b[39m \x1b[38;5;243mtick\x1b[39m\x1b[38;5;243m   \x1b[39m\x1b[38;5;247m← →\x1b[39m \x1b[38;5;243mfold\x1b[39m\x1b[38;5;243m   \x1b[39m\x1b[38;5;247mesc\x1b[39m \x1b[38;5;243mback\x1b[39m";
    let got: Vec<String> = [8u8, 24].iter().map(|d| help_line(&tick_keys(), *d, false)).collect();
    assert_eq!(got, vec![want.to_string(), want.to_string()]);
}

/// init-layout.ts:104 (the escape codes taken out, UTF-16 length)
#[test]
fn cells_counts_a_row_without_its_escape_codes_in_utf16_units() {
    let red = "\x1b[31mDisk  12.5 GB of 16.1 GB, red line\x1b[39m";
    assert_eq!(vec![cells(""), cells("abc"), cells(red), cells("修复"), cells("🚀")], vec![0, 3, 34, 2, 2]);
}

/// init-layout.ts:116
#[test]
fn is_tty_is_true_only_for_a_stream_that_is_a_terminal() {
    let term = Screen { buf: Mutex::new(String::new()), columns: Some(80), rows: Some(24), tty: true };
    let pipe = Screen { buf: Mutex::new(String::new()), columns: None, rows: None, tty: false };
    assert_eq!(vec![is_tty(None), is_tty(Some(&term)), is_tty(Some(&pipe))], vec![false, true, false]);
}

/// init-layout.test.ts:403
#[test]
fn cell_width_counts_east_asian_wide_characters_and_emoji_as_two_and_combining_marks_as_none() {
    let got = vec![cell_width("abc"), cell_width("修复"), cell_width("🚀a"), cell_width("👩\u{200d}💻"), cell_width("e\u{301}")];
    assert_eq!(got, vec![3, 4, 3, 2, 1]);
}

/// init-layout.test.ts:410
#[test]
fn ellipsize_cuts_to_cells_and_never_through_a_character() {
    let got = vec![ellipsize("修复实验室", 5), ellipsize("🚀🔥✅x", 4), ellipsize("👩\u{200d}💻👩\u{200d}💻", 3), ellipsize("abcdef", 4)];
    assert_eq!(got, vec!["修复…", "🚀…", "👩\u{200d}💻…", "abc…"]);
    assert!(cell_width(&ellipsize("修复实验室", 6)) <= 6);
}

// ---------------------------------------------------------------------------------------------------------------
// init-layout.test.ts, the confirm prompt.

/// init-layout.test.ts:209
#[tokio::test]
async fn confirm_draws_the_question_on_the_step_glyph_the_hint_and_two_answers_down_the_thick_bar_and_enter_takes_the_marked_answer() {
    let (answer, screen, snaps) = run_confirm("Boot it?", Some("No costs nothing."), None, None, off(), &[SNAP, KEY_ENTER]).await;
    assert_eq!(snaps, vec![format!("◆  Boot it?\n┃  No costs nothing.\n┃  ○ Yes / ● No\n┗  {CONFIRM_HELP}")]);
    assert_eq!(answer, Answer::Value(false));
    assert_eq!(after_last(&screen.text(), "◇"), "◇  Boot it?\n│  No costs nothing.\n│  No\n");
}

/// init-layout.test.ts:220
#[tokio::test]
async fn confirm_left_and_right_move_the_marker_and_enter_takes_it() {
    let (answer, screen, snaps) =
        run_confirm("Boot it?", None, None, None, off(), &[SNAP, KEY_LEFT, SNAP, KEY_RIGHT, KEY_LEFT, KEY_ENTER]).await;
    assert_eq!(snaps[0], format!("◆  Boot it?\n┃  ○ Yes / ● No\n┗  {CONFIRM_HELP}"));
    assert!(after_last(&snaps[1], "◆").contains("┃  ● Yes / ○ No"), "{:?}", snaps[1]);
    assert_eq!(answer, Answer::Value(true));
    assert_eq!(after_last(&screen.text(), "◇"), "◇  Boot it?\n│  Yes\n");
}

/// init-layout.test.ts:220
#[tokio::test]
async fn confirm_y_answers_yes_at_once() {
    let (answer, _, _) = run_confirm("Boot it?", None, None, None, off(), &["y"]).await;
    assert_eq!(answer, Answer::Value(true));
}

/// init-layout.test.ts:220
#[tokio::test]
async fn confirm_n_answers_no_at_once_even_when_the_marker_started_on_yes() {
    let (answer, _, _) = run_confirm("Boot it?", None, Some(true), None, off(), &["n"]).await;
    assert_eq!(answer, Answer::Value(false));
}

/// init-layout.test.ts:220
#[tokio::test]
async fn confirm_esc_cancels_with_the_marked_answer_struck_through_and_clacks_escape_alias_does_not_flip_it_first() {
    let (answer, screen, _) = run_confirm("Boot it?", None, None, None, off(), &[KEY_ESC]).await;
    assert_eq!(answer, Answer::Cancel);
    assert_eq!(after_last(&screen.text(), "■"), "■  Boot it?\n│  No\n");
}

/// init-layout.test.ts:220
#[tokio::test]
async fn confirm_ctrl_c_cancels() {
    let (answer, _, _) = run_confirm("Boot it?", None, None, None, off(), &[KEY_CTRL_C]).await;
    assert_eq!(answer, Answer::Cancel);
}

/// init-layout.test.ts:257
#[tokio::test]
async fn confirm_wraps_a_long_question_and_hint_at_the_width_the_rest_under_the_bar_with_the_two_space_indent() {
    let message = "Boot a 2 vCPU, 4 GB builder and build this? About $0.11/hr while it runs.";
    let hint = "No costs nothing and keeps the recipe for wsp init --recipe.";
    let (answer, _, snaps) = run_confirm(message, Some(hint), None, Some(40), off(), &[SNAP, KEY_ENTER]).await;
    let lines: Vec<&str> = snaps[0].split('\n').collect();
    let help = format!("┗  {CONFIRM_HELP}");
    assert_eq!(
        lines,
        vec![
            "◆  Boot a 2 vCPU, 4 GB builder and build",
            "┃    this? About $0.11/hr while it runs.",
            "┃  No costs nothing and keeps the recipe",
            "┃    for wsp init --recipe.",
            "┃  ○ Yes / ● No",
            help.as_str()
        ]
    );
    let too_wide: Vec<usize> = lines[..lines.len() - 1].iter().map(|l| l.chars().count()).filter(|n| *n > 40).collect();
    assert_eq!(too_wide, Vec::<usize>::new());
    assert_eq!(answer, Answer::Value(false));
}

/// init-layout.test.ts:257
#[tokio::test]
async fn confirm_takes_a_newline_in_the_question_as_a_line_break() {
    let (answer, _, snaps) =
        run_confirm("Boot it?\nIt bills while it runs.", Some("No costs nothing."), None, None, off(), &[SNAP, KEY_ENTER]).await;
    assert_eq!(snaps, vec![format!("◆  Boot it?\n┃  It bills while it runs.\n┃  No costs nothing.\n┃  ○ Yes / ● No\n┗  {CONFIRM_HELP}")]);
    assert_eq!(answer, Answer::Value(false));
}

/// init-layout.test.ts:276
#[tokio::test]
async fn confirm_with_colour_on_paints_cyan_glyph_and_question_dim_hint_and_bar_cyan_marker_and_two_help_greys_and_no_other_colour() {
    let (answer, screen, snaps) = run_confirm("Boot it?", Some("No costs nothing."), None, None, force_3(), &[RAW, KEY_ENTER]).await;
    let frame = &snaps[0];
    assert!(frame.contains("\x1b[36m◆\x1b[39m  \x1b[36mBoot it?\x1b[39m"), "{frame:?}");
    assert!(frame.contains("\x1b[2m┃\x1b[22m  \x1b[2mNo costs nothing.\x1b[22m"), "{frame:?}");
    assert!(frame.contains("\x1b[2m○\x1b[22m \x1b[2mYes\x1b[22m \x1b[2m/\x1b[22m \x1b[36m●\x1b[39m No"), "{frame:?}");
    assert!(frame.contains("\x1b[2m┗\x1b[22m  \x1b[38;5;247m← →\x1b[39m \x1b[38;5;243mchange\x1b[39m"), "{frame:?}");
    assert_eq!(sgr_set(frame), set(&["2", "22", "36", "38;5;243", "38;5;247", "39"]));
    assert_eq!(answer, Answer::Value(false));
    let raw = screen.raw();
    assert_eq!(
        after_last(&raw, "\x1b[32m"),
        "\x1b[32m◇\x1b[39m  Boot it?\n\x1b[2m│\x1b[22m  \x1b[2mNo costs nothing.\x1b[22m\n\x1b[2m│\x1b[22m  \x1b[2mNo\x1b[22m\n\x1b[?25h"
    );
}

/// init-layout.test.ts:294
#[tokio::test]
async fn confirm_at_16_colours_dims_the_help_words_and_no_256_colour_code_leaves_the_frame() {
    let (answer, screen, snaps) = run_confirm("Boot it?", None, None, None, force_1(), &[RAW, KEY_ENTER]).await;
    assert!(snaps[0].contains("← → \x1b[2mchange\x1b[22m"), "{:?}", snaps[0]);
    assert!(!snaps[0].contains("38;5;"), "{:?}", snaps[0]);
    assert_eq!(sgr_set(&snaps[0]), set(&["2", "22", "36", "39"]));
    assert_eq!(answer, Answer::Value(false));
    assert!(!screen.raw().contains("38;5;"));
}

/// init-layout.test.ts:306
#[tokio::test]
async fn confirm_under_no_color_carries_no_sgr_at_all_and_keeps_its_glyphs_and_dots() {
    let (answer, _, snaps) = run_confirm("Boot it?", Some("No costs nothing."), None, None, off(), &[RAW, SNAP, KEY_ENTER]).await;
    assert_eq!(sgr_set(&snaps[0]), BTreeSet::new());
    assert_eq!(snaps[1], format!("◆  Boot it?\n┃  No costs nothing.\n┃  ○ Yes / ● No\n┗  {CONFIRM_HELP}"));
    assert_eq!(answer, Answer::Value(false));
}

/// PromptOptions.signal's doc (no TS test): a signal that fires ends the prompt as a cancel.
#[tokio::test]
async fn confirm_ends_as_a_cancel_when_its_signal_fires_before_an_answer() {
    let screen = Screen::new(None);
    let (mut o, tx) = options("Boot it?", None, &screen, off());
    o.signal = Some(Box::pin(async { tokio::time::sleep(Duration::from_millis(50)).await }));
    let answer = tokio::time::timeout(Duration::from_secs(10), confirm_prompt(o, None)).await.expect("the prompt never ended");
    drop(tx);
    assert_eq!(answer, Answer::Cancel);
}

// ---------------------------------------------------------------------------------------------------------------
// init-layout.test.ts, the password prompt.

/// init-layout.test.ts:321
#[tokio::test]
async fn password_masks_every_character_never_echoes_the_text_and_returns_it_on_enter_and_the_finished_screen_shows_the_mask_on_the_thin_bar(
) {
    let hint = "Solari API key  console.getsolari.com";
    let (answer, screen, snaps) = run_password("No Solari key found.", Some(hint), None, off(), &[SNAP, SECRET, SNAP, KEY_ENTER]).await;
    assert_eq!(snaps[0], format!("◆  No Solari key found.\n┃  {hint}\n┃  _\n┗  {PASSWORD_HELP}"));
    assert!(after_last(&snaps[1], "◆").contains(&format!("┃  {}_", "▪".repeat(SECRET.len()))), "{:?}", snaps[1]);
    assert_eq!(answer, Answer::Value(SECRET.to_string()));
    let raw = screen.raw();
    assert!(!raw.contains(SECRET), "the secret was echoed");
    assert!(!raw.contains(&SECRET[..4]), "the start of the secret was echoed");
    assert_eq!(after_last(&screen.text(), "◇"), format!("◇  No Solari key found.\n│  {hint}\n│  {}\n", "▪".repeat(SECRET.len())));
}

/// init-layout.test.ts:336
#[tokio::test]
async fn password_enter_on_nothing_returns_the_empty_string_with_no_mask_line() {
    let (answer, screen, _) = run_password("Anthropic API key", None, None, off(), &[KEY_ENTER]).await;
    assert_eq!(answer, Answer::Value(String::new()));
    assert_eq!(after_last(&screen.text(), "◇"), "◇  Anthropic API key\n");
}

/// init-layout.test.ts:336
#[tokio::test]
async fn password_esc_cancels_with_the_mask_struck_through_and_the_text_never_written() {
    let (answer, screen, _) = run_password("Anthropic API key", None, None, off(), &["abc", KEY_ESC]).await;
    assert_eq!(answer, Answer::Cancel);
    assert_eq!(after_last(&screen.text(), "■"), "■  Anthropic API key\n│  ▪▪▪\n");
    assert!(!screen.raw().contains("abc"));
}

/// init-layout.test.ts:336
#[tokio::test]
async fn password_ctrl_c_cancels() {
    let (answer, _, _) = run_password("Anthropic API key", None, None, off(), &[KEY_CTRL_C]).await;
    assert_eq!(answer, Answer::Cancel);
}

/// init-layout.test.ts:358
#[tokio::test]
async fn password_with_colour_on_paints_cyan_glyph_and_question_dim_hint_and_bar_the_inverse_cursor_and_two_help_greys_and_no_other_colour()
{
    let hint = "Solari API key  console.getsolari.com";
    let (answer, screen, snaps) = run_password("No Solari key found.", Some(hint), None, force_3(), &["ab", RAW, KEY_ENTER]).await;
    let frame = after_last(&snaps[0], "\x1b[36m◆");
    assert!(frame.contains("\x1b[36m◆\x1b[39m  \x1b[36mNo Solari key found.\x1b[39m"), "{frame:?}");
    assert!(frame.contains(&format!("\x1b[2m┃\x1b[22m  \x1b[2m{hint}\x1b[22m")), "{frame:?}");
    assert!(frame.contains("\x1b[2m┃\x1b[22m  ▪▪\x1b[7m\x1b[8m_\x1b[28m\x1b[27m"), "{frame:?}");
    assert!(frame.contains("\x1b[2m┗\x1b[22m  \x1b[38;5;247menter\x1b[39m \x1b[38;5;243mnext\x1b[39m"), "{frame:?}");
    assert_eq!(sgr_set(frame), set(&["2", "22", "27", "28", "36", "38;5;243", "38;5;247", "39", "7", "8"]));
    assert_eq!(answer, Answer::Value("ab".to_string()));
    let raw = screen.raw();
    assert!(!raw.contains("ab\x1b"), "the text was echoed");
    assert_eq!(
        after_last(&raw, "\x1b[32m"),
        format!("\x1b[32m◇\x1b[39m  No Solari key found.\n\x1b[2m│\x1b[22m  \x1b[2m{hint}\x1b[22m\n\x1b[2m│\x1b[22m  \x1b[2m▪▪\x1b[22m\n\x1b[?25h")
    );
}

/// init-layout.test.ts:376
#[tokio::test]
async fn password_at_16_colours_dims_the_help_words_and_no_256_colour_code_leaves_the_frame() {
    let (answer, screen, snaps) = run_password("Anthropic API key", None, None, force_1(), &[RAW, KEY_ENTER]).await;
    assert!(snaps[0].contains("enter \x1b[2mnext\x1b[22m"), "{:?}", snaps[0]);
    assert!(!snaps[0].contains("38;5;"), "{:?}", snaps[0]);
    assert_eq!(sgr_set(&snaps[0]), set(&["2", "22", "27", "28", "36", "39", "7", "8"]));
    assert_eq!(answer, Answer::Value(String::new()));
    assert!(!screen.raw().contains("38;5;"));
}

/// init-layout.test.ts:388
#[tokio::test]
async fn password_under_no_color_carries_no_sgr_at_all_and_keeps_its_glyphs_and_dots() {
    let (answer, _, snaps) = run_password("Anthropic API key", Some("optional, enter skips"), None, off(), &[RAW, SNAP, KEY_ENTER]).await;
    assert_eq!(sgr_set(&snaps[0]), BTreeSet::new());
    assert_eq!(snaps[1], format!("◆  Anthropic API key\n┃  optional, enter skips\n┃  _\n┗  {PASSWORD_HELP}"));
    assert_eq!(answer, Answer::Value(String::new()));
}

/// PromptOptions.signal's doc (no TS test): a signal that fires ends the prompt as a cancel.
#[tokio::test]
async fn password_ends_as_a_cancel_when_its_signal_fires_before_an_answer() {
    let screen = Screen::new(None);
    let (mut o, tx) = options("Anthropic API key", None, &screen, off());
    o.signal = Some(Box::pin(async { tokio::time::sleep(Duration::from_millis(50)).await }));
    let answer = tokio::time::timeout(Duration::from_secs(10), password_prompt(o)).await.expect("the prompt never ended");
    drop(tx);
    assert_eq!(answer, Answer::Cancel);
}
