// SPDX-License-Identifier: AGPL-3.0-only
//! The pty relay against a scripted link: bytes both ways, raw mode set and restored, size followed, a printed URL
//! shown once as a hyperlink and opened here on o, the watched pty, the quiet status run. Ported from
//! packages/host/test/signin-relay.test.ts at 65f6b89c7. The spec is spec/signin_relay.md.
#![allow(
    clippy::result_large_err,
    clippy::type_complexity,
    clippy::too_many_arguments,
    clippy::large_enum_variant,
    clippy::regex_creation_in_loops,
    non_snake_case,
    dead_code
)]
mod common;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use common::*;
use futures::FutureExt;
use serde_json::json;
use wsp_catalog::js_regex::JsRegex;
use wsp_catalog::signin::{Question, Settle};
use wsp_cli::signin_relay::*;

const GH_WEB: &str = "! First copy your one-time code: 72F3-072B\nPress Enter to open https://github.com/login/device in your browser... ";
const GH_CREDENTIALS: &str = "? Authenticate Git with your GitHub credentials? (Y/n) ";
const AWS_SSO: &str = "SSO session name (Recommended): ";

fn enter_q() -> Question {
    Question { asks: JsRegex::new(r"Press \[?Enter\]? to (?:open|continue)"), settle: Settle::Answer("\r".into()) }
}

fn theirs_q() -> Question {
    Question { asks: JsRegex::new(r"SSO session name \(Recommended\)"), settle: Settle::Person(true) }
}

fn answers(asked: &[Asked]) -> Vec<Option<String>> {
    asked
        .iter()
        .map(|a| match &a.question.settle {
            Settle::Answer(s) => Some(s.clone()),
            _ => None,
        })
        .collect()
}

type Opened = Arc<Mutex<Vec<String>>>;

fn opener(ok: bool) -> (Opened, Arc<dyn Fn(String) -> futures::future::BoxFuture<'static, bool> + Send + Sync>) {
    let opened: Opened = Arc::default();
    let o = opened.clone();
    (
        opened,
        Arc::new(move |u: String| {
            o.lock().unwrap().push(u);
            async move { ok }.boxed()
        }),
    )
}

fn relay_opts(
    link: Arc<dyn PtyLink>,
    command: Option<&str>,
    open: Arc<dyn Fn(String) -> futures::future::BoxFuture<'static, bool> + Send + Sync>,
    timeout_ms: u64,
) -> RelayOptions {
    RelayOptions {
        link,
        command: command.map(String::from),
        open,
        callback_url: None,
        on_consent: None,
        env: None,
        timeout_ms,
        flush_ms: None,
        now: None,
    }
}

type Running = tokio::task::JoinHandle<(Result<RelayOutcome, wsp_words::exit::HostError>, RelayTerminal)>;

fn spawn_relay(o: RelayOptions, mut rt: RelayTerminal) -> Running {
    tokio::spawn(async move {
        let r = relay_pty(o, &mut rt).await;
        (r, rt)
    })
}

fn writes_after_line(p: &FakePty) -> Vec<String> {
    p.writes.iter().skip(1).cloned().collect()
}

fn pty(link: &FakeLink, i: usize) -> FakePty {
    link.ptys()[i].clone()
}

// ---- URL detection

/// signin-relay.test.ts:68.
#[test]
fn osc8_wrappers_come_off_trailing_punctuation_drops_and_a_match_at_the_chunks_end_waits() {
    let wrapped = "visit \x1b]8;;https://example.com/a?x=1\x1b\\https://example.com/a?x=1\x1b]8;;\x1b\\ now";
    assert_eq!(strip_osc8(wrapped), "visit https://example.com/a?x=1 now");
    assert_eq!(urls_in(wrapped), vec!["https://example.com/a?x=1"]);
    assert_eq!(urls_in("Open this URL (https://github.com/login/device).\r\n"), vec!["https://github.com/login/device"]);
    assert_eq!(urls_in("Open https://github.com/login/dev"), Vec::<String>::new());
    assert_eq!(
        urls_in("http://localhost:8976/oauth/callback and https://example.com/a and https://example.com/a again"),
        vec!["http://localhost:8976/oauth/callback", "https://example.com/a"]
    );
}

/// signin-relay.ts:73 and :85: every trailing run of .,;:!?)] goes, quotes and angle brackets end a URL, and a URL that
/// is all punctuation after the scheme is kept as what is left.
#[test]
fn a_url_ends_at_space_quotes_angles_and_controls_and_sheds_its_trailing_punctuation() {
    assert_eq!(
        urls_in("see \"https://a.test/x\" and <https://b.test/y> then https://c.test/z?!).]\n"),
        vec!["https://a.test/x", "https://b.test/y", "https://c.test/z"]
    );
    assert_eq!(urls_in("ftp://x.test/a https://d.test\x07tail"), vec!["https://d.test"]);
    assert_eq!(urls_in("http://e.test/(paren) \n"), vec!["http://e.test/(paren"]);
}

/// signin-relay.test.ts:77.
#[test]
fn a_url_split_across_chunks_is_reported_once_and_the_same_url_again_not_at_all() {
    let mut s = UrlScanner::new();
    assert_eq!(s.feed("Press Enter to open https://github.com/lo"), Vec::<String>::new());
    assert_eq!(s.feed("gin/device in your browser...\r\n"), vec!["https://github.com/login/device"]);
    assert_eq!(s.feed("Open this URL to continue: https://github.com/login/device\r\n"), Vec::<String>::new());
    assert_eq!(s.feed("\x1b]8;;https://claude.com/x\x07https://claude.com/x\x1b]8;;\x07\r\n"), vec!["https://claude.com/x"]);
}

/// signin-relay.test.ts:85.
#[test]
fn flush_settles_a_url_that_ended_the_last_chunk_once() {
    let mut s = UrlScanner::new();
    assert_eq!(s.feed("Open https://x.test/abc"), Vec::<String>::new());
    assert_eq!(s.flush(), vec!["https://x.test/abc"]);
    assert_eq!(s.flush(), Vec::<String>::new());
    assert_eq!(s.feed("\r\n"), Vec::<String>::new());
}

/// signin-relay.ts:97: the scanner keeps the last 2048 characters, so a URL whose start fell out of the tail is cut.
#[test]
fn the_scanner_keeps_a_tail_of_2048_characters() {
    assert_eq!(TAIL_CHARS, 2048);
    let mut s = UrlScanner::new();
    assert_eq!(s.feed(&format!("https://long.test/{}", "a".repeat(3000))), Vec::<String>::new());
    let got = s.feed(" \n");
    assert_eq!(got, Vec::<String>::new(), "the scheme fell out of the 2048 tail, so no URL is left to settle");
    let mut cut = UrlScanner::new();
    cut.feed(&format!("https://cut.test/{}", "b".repeat(3000)));
    cut.feed("c");
    let got = cut.feed(" \n");
    assert_eq!(got, Vec::<String>::new(), "the joined text only ever holds the tail and the new chunk, so the scheme fell out");
}

/// signin-relay.test.ts:92.
#[test]
fn a_terminal_hyperlink_wraps_the_url_in_osc_8() {
    assert_eq!(hyperlink("https://a.b/c"), "\x1b]8;;https://a.b/c\x1b\\https://a.b/c\x1b]8;;\x1b\\");
}

/// signin-relay.ts:185-187 and :163: the typed line and the stage command, byte for byte.
#[test]
fn the_typed_line_reads_the_staged_command_removes_its_folder_marks_the_tool_and_execs_it() {
    let staged = Staged { dir: "/tmp/wsp-line.abc".into(), file: "'/tmp/wsp-line.abc/line'".into() };
    assert_eq!(
        shell_line(&staged),
        "c=$(< '/tmp/wsp-line.abc/line'); rm -rf -- '/tmp/wsp-line.abc'; printf '\\036'; exec bash -c \"$c\"\r"
    );
    assert_eq!(STAGE, "umask 077 && d=$(mktemp -d \"${TMPDIR:-/tmp}/wsp-line.XXXXXX\") && cat > \"$d/line\" && printf '%s' \"$d\"");
    assert_eq!(
        (TOOL_STARTS, BARE_BASH, OFFER_MS, FLUSH_MS, STATUS_MARK),
        ("\x1e", "exec bash --noprofile --norc", 60_000.0, 300, "WSP_STATUS")
    );
}

/// signin-relay.ts:174-181: a stage that does not answer an absolute folder of plain text is refused.
#[tokio::test]
async fn a_stage_that_does_not_answer_a_plain_absolute_folder_is_refused_with_its_last_line() {
    let link = FakeLink::new();
    let ok = stage(&Link(link.clone()), "gh auth login").await.unwrap();
    assert_eq!(ok, Staged { dir: "/staged/1".into(), file: "'/staged/1/line'".into() });
    assert_eq!(link.staged(), vec![StagedCmd { command: "gh auth login".into(), cleared: false }]);
    let (op, extra) = link.ops()[0].clone();
    assert_eq!(op, "exec");
    assert_eq!(serde_json::Value::Object(extra), json!({"cmd": STAGE, "stdin": "Z2ggYXV0aCBsb2dpbg=="}));
    clear_staged(&Link(link.clone()), &ok).await;
    assert_eq!(link.staged(), vec![StagedCmd { command: "gh auth login".into(), cleared: true }]);

    for (reply, said) in [
        (
            json!({"exitCode": 1, "stdout": "", "stderr": "mktemp: failed\nno space left\n"}),
            "could not put the command on that computer: no space left",
        ),
        (json!({"exitCode": 0, "stdout": "relative/dir", "stderr": ""}), "could not put the command on that computer: no reason given"),
        (json!({"exitCode": 0, "stdout": "/tmp/x\u{1b}[31m", "stderr": ""}), "could not put the command on that computer: no reason given"),
        (
            json!({"ok": false, "error": "exec is not served on this channel"}),
            "could not put the command on that computer: exec is not served on this channel",
        ),
    ] {
        let refusing = FakeLink::new();
        let r = reply.clone();
        *refusing.override_op.lock().unwrap() = Some(Arc::new(move |op, _| if op == "exec" { Some(frame(r.clone())) } else { None }));
        let e = stage(&Link(refusing), "x").await.unwrap_err();
        assert_eq!(e.message, said, "{reply}");
    }
}

// ---- relayPty

/// signin-relay.test.ts:96.
#[tokio::test]
async fn a_pty_on_a_computer_somebody_owns_runs_bash_reading_no_startup_file() {
    let owned = FakeLink::with_bare(Some(true));
    let (_t, rt) = terminal(true, 120, 40);
    let (_o, open) = opener(true);
    let _run = spawn_relay(relay_opts(Arc::new(Link(owned.clone())), Some("gh auth login"), open, 60_000), rt);
    assert_eq!(
        serde_json::Value::Object(first_pty(&owned).await.created),
        json!({"cols": 120, "rows": 40, "shell": "bash", "run": BARE_BASH})
    );
    let watched = FakeLink::with_bare(Some(true));
    let _w = tokio::spawn(watch_pty(watch_opts(Arc::new(Link(watched.clone())), "codex login", 60_000)));
    let created = first_pty(&watched).await.created;
    assert_eq!((created.get("shell"), created.get("run")), (Some(&json!("bash")), Some(&json!(BARE_BASH))));
    assert_eq!((created.get("cols"), created.get("rows")), (Some(&json!(200)), Some(&json!(50))));
}

fn watch_opts(link: Arc<dyn PtyLink>, command: &str, timeout_ms: u64) -> WatchOptions {
    WatchOptions {
        link,
        command: command.into(),
        on_data: None,
        on_url: None,
        on_scanned: None,
        env: None,
        questions: vec![],
        on_question: None,
        on_typing: None,
        timeout_ms,
        stop: None,
        flush_ms: None,
    }
}

/// signin-relay.test.ts:108.
#[tokio::test]
async fn the_relay_opens_at_the_terminals_size_relays_both_ways_follows_a_resize_and_restores_the_terminal() {
    let link = FakeLink::new();
    let (t, rt) = terminal(true, 120, 40);
    let (opened, open) = opener(true);
    let run = spawn_relay(relay_opts(Arc::new(Link(link.clone())), Some("gh auth login"), open, 60_000), rt);
    let p = first_pty(&link).await;
    assert_eq!(serde_json::Value::Object(p.created.clone()), json!({"cols": 120, "rows": 40, "shell": "bash"}));
    tick().await;
    let p = pty(&link, 0);
    assert!(p.attached);
    assert_eq!(p.ran.as_deref(), Some("gh auth login"));
    assert_eq!(t.raw_seen(), vec![true]);

    link.data(&p.id, "? Authenticate Git with your GitHub credentials? (Y/n) ");
    assert!(t.text().contains("Authenticate Git"));
    t.type_keys("n");
    t.type_keys("\r");
    tick().await;
    assert_eq!(writes_after_line(&pty(&link, 0)), vec!["n", "\r"]);

    t.set_size(100, 30);
    t.resize.send(()).unwrap();
    tick().await;
    assert_eq!(pty(&link, 0).resizes, vec![(100, 30)]);

    link.exit(&p.id, 0);
    let (outcome, _rt) = run.await.unwrap();
    assert_eq!(outcome.unwrap(), RelayOutcome { exit_code: 0, timed_out: false, dropped: false, urls: 0, opened: 0 });
    assert_eq!(t.raw_seen(), vec![true, false]);
    assert!(pty(&link, 0).killed);
    assert_eq!(*opened.lock().unwrap(), Vec::<String>::new());
    t.type_keys("late");
    tick().await;
    assert_eq!(writes_after_line(&pty(&link, 0)), vec!["n", "\r"], "nothing is read once the relay ends");
    assert!(link.staged().iter().all(|s| s.cleared));
}

/// signin-relay.test.ts:142.
#[tokio::test]
async fn a_printed_url_is_shown_once_with_the_o_offer_and_o_opens_it_here_without_reaching_the_pty() {
    let link = FakeLink::new();
    let (t, rt) = terminal(true, 120, 40);
    let (opened, open) = opener(true);
    let consented = Arc::new(Mutex::new(0));
    let c = consented.clone();
    let mut o = relay_opts(Arc::new(Link(link.clone())), Some("claude auth login"), open, 60_000);
    o.on_consent = Some(Arc::new(move |_| *c.lock().unwrap() += 1));
    let run = spawn_relay(o, rt);
    let p = first_pty(&link).await;
    tick().await;
    let url = "https://claude.com/cai/oauth/authorize?code=true&redirect_uri=https%3A%2F%2Fplatform.claude.com%2Foauth%2Fcode%2Fcallback";
    link.data(
        &p.id,
        &format!("If the browser didn't open, visit: \x1b]8;;{url}\x1b\\{url}\x1b]8;;\x1b\\\r\nPaste code here if prompted > "),
    );
    let shown = t.text();
    assert!(shown.contains(&hyperlink(url)));
    assert_eq!(shown.matches("o opens it on this computer").count(), 1);
    assert!(
        shown.contains(&format!("\r\n  link  {}  o opens it on this computer\r\n", hyperlink(url)))
            || strip_vt(&shown).contains("  link  "),
        "the offer line"
    );

    t.type_keys("o");
    tick().await;
    assert_eq!(*opened.lock().unwrap(), vec![url.to_string()]);
    assert_eq!(*consented.lock().unwrap(), 1);
    assert_eq!(writes_after_line(&pty(&link, 0)), Vec::<String>::new());
    assert!(strip_vt(&t.text()).contains(OPENED_PRINTED));

    link.data(&p.id, &format!("visit: {url}\r\n"));
    assert_eq!(t.text().matches("o opens it on this computer").count(), 1);
    t.type_keys("\r");
    t.type_keys("o");
    tick().await;
    assert_eq!(*opened.lock().unwrap(), vec![url.to_string(), url.to_string()]);
    t.type_keys("x");
    t.type_keys("o");
    tick().await;
    assert_eq!(writes_after_line(&pty(&link, 0)), vec!["\r", "x", "o"]);
    assert_eq!(opened.lock().unwrap().len(), 2);

    link.exit(&p.id, 0);
    let (outcome, _) = run.await.unwrap();
    assert_eq!(outcome.unwrap(), RelayOutcome { exit_code: 0, timed_out: false, dropped: false, urls: 1, opened: 2 });
}

/// signin-relay.ts:220: the offer line, byte for byte with colour off (styleText off a terminal is plain).
#[tokio::test]
async fn the_offer_line_is_the_dim_link_word_the_hyperlink_and_the_dim_offer() {
    let link = FakeLink::new();
    let (t, rt) = terminal(true, 120, 40);
    let (_o, open) = opener(true);
    let run = spawn_relay(relay_opts(Arc::new(Link(link.clone())), Some("x"), open, 60_000), rt);
    let p = first_pty(&link).await;
    tick().await;
    link.data(&p.id, "go https://a.test/x now\r\n");
    let shown = strip_vt(&t.text());
    assert!(shown.ends_with("go https://a.test/x now\r\n\r\n  link  https://a.test/x  o opens it on this computer\r\n"), "{shown:?}");
    link.exit(&p.id, 0);
    run.await.unwrap().0.unwrap();
}

/// signin-relay.test.ts:183: a link in the shell's banner is text, never offered.
#[tokio::test]
async fn only_a_page_the_tool_printed_is_offered_and_the_shells_banner_is_text() {
    let link = FakeLink::new();
    *link.banner.lock().unwrap() = Some(
        "The default interactive shell is now zsh.\r\nFor more details, please visit https://support.apple.com/kb/HT208050.\r\n".into(),
    );
    let (t, rt) = terminal(true, 120, 40);
    let (_o, open) = opener(true);
    let run = spawn_relay(relay_opts(Arc::new(Link(link.clone())), Some("claude mcp login notion"), open, 60_000), rt);
    let p = first_pty(&link).await;
    tick().await;
    link.data(&p.id, "Open https://claude.ai/oauth/authorize?x=1\r\n");
    assert!(t.text().contains("https://support.apple.com/kb/HT208050"));
    assert!(!t.text().contains(&hyperlink("https://support.apple.com/kb/HT208050")));
    assert!(t.text().contains(&hyperlink("https://claude.ai/oauth/authorize?x=1")));
    link.exit(&p.id, 0);
    assert_eq!(run.await.unwrap().0.unwrap().urls, 1);
}

/// signin-relay.test.ts:202 (the read file's lines 202-228).
#[tokio::test]
async fn o_opens_the_forwarded_page_when_one_arrived_and_the_printed_link_otherwise() {
    let link = FakeLink::new();
    let (t, rt) = terminal(true, 120, 40);
    let (opened, open) = opener(true);
    let consented: Opened = Arc::default();
    let page: Arc<Mutex<Option<String>>> = Arc::default();
    let mut o = relay_opts(Arc::new(Link(link.clone())), Some("claude auth login"), open, 60_000);
    let c = consented.clone();
    o.on_consent = Some(Arc::new(move |u| c.lock().unwrap().push(u.to_string())));
    let pg = page.clone();
    o.callback_url = Some(Arc::new(move || pg.lock().unwrap().clone()));
    let run = spawn_relay(o, rt);
    let p = first_pty(&link).await;
    tick().await;
    let printed =
        "https://claude.com/cai/oauth/authorize?code=true&redirect_uri=https%3A%2F%2Fplatform.claude.com%2Foauth%2Fcode%2Fcallback";
    link.data(&p.id, &format!("If the browser didn't open, visit: {printed}\r\nPaste code here if prompted > "));
    t.type_keys("o");
    tick().await;
    assert_eq!(*opened.lock().unwrap(), vec![printed.to_string()]);
    assert!(strip_vt(&t.text()).contains(OPENED_PRINTED));
    let forwarded = "https://claude.com/cai/oauth/authorize?code=true&redirect_uri=http%3A%2F%2Flocalhost%3A42485%2Fcallback";
    *page.lock().unwrap() = Some(forwarded.into());
    t.type_keys("o");
    tick().await;
    assert_eq!(*opened.lock().unwrap(), vec![printed.to_string(), forwarded.to_string()]);
    assert_eq!(*consented.lock().unwrap(), vec![printed.to_string(), forwarded.to_string()]);
    assert!(strip_vt(&t.text()).contains(OPENED_PAGE));
    assert_eq!(writes_after_line(&pty(&link, 0)), Vec::<String>::new());
    link.exit(&p.id, 0);
    assert_eq!(run.await.unwrap().0.unwrap().opened, 2);
}

/// signin-relay.test.ts:230 (read file 30-54).
#[tokio::test]
async fn arrow_keys_and_other_csi_sequences_keep_the_o_offer_and_typed_text_ends_it() {
    let link = FakeLink::new();
    let (t, rt) = terminal(true, 120, 40);
    let (opened, open) = opener(true);
    let run = spawn_relay(relay_opts(Arc::new(Link(link.clone())), Some("gh auth login"), open, 60_000), rt);
    let p = first_pty(&link).await;
    tick().await;
    link.data(&p.id, "? Where do you use GitHub?  [Use arrows to move]\r\n> GitHub.com https://github.com/login/device \r\n");
    for k in ["\x1b[B", "\x1b[A", "\x1bOA", "\x1bOB", "\r", "o"] {
        t.type_keys(k);
    }
    tick().await;
    assert_eq!(*opened.lock().unwrap(), vec!["https://github.com/login/device".to_string()]);
    assert_eq!(writes_after_line(&pty(&link, 0)), vec!["\x1b[B", "\x1b[A", "\x1bOA", "\x1bOB", "\r"]);
    t.type_keys("n");
    t.type_keys("o");
    tick().await;
    assert_eq!(writes_after_line(&pty(&link, 0)), vec!["\x1b[B", "\x1b[A", "\x1bOA", "\x1bOB", "\r", "n", "o"]);
    link.exit(&p.id, 0);
    run.await.unwrap().0.unwrap();
}

/// signin-relay.ts:252: o is the offer only as a chunk of its own; "oo" or "o\r" in one read is typed through.
#[tokio::test]
async fn o_is_the_offer_only_as_a_whole_chunk() {
    let link = FakeLink::new();
    let (t, rt) = terminal(true, 120, 40);
    let (opened, open) = opener(true);
    let run = spawn_relay(relay_opts(Arc::new(Link(link.clone())), Some("x"), open, 60_000), rt);
    let p = first_pty(&link).await;
    tick().await;
    link.data(&p.id, "open https://a.test/x \r\n");
    t.type_keys("o\r");
    tick().await;
    assert_eq!(*opened.lock().unwrap(), Vec::<String>::new());
    assert_eq!(writes_after_line(&pty(&link, 0)), vec!["o\r"]);
    link.exit(&p.id, 0);
    run.await.unwrap().0.unwrap();
}

/// signin-relay.test.ts:256 (read file 56-69).
#[tokio::test]
async fn a_url_that_ends_the_chunk_is_offered_after_a_short_quiet() {
    let link = FakeLink::new();
    let (t, rt) = terminal(true, 120, 40);
    let (_o, open) = opener(true);
    let mut o = relay_opts(Arc::new(Link(link.clone())), Some("netlify login"), open, 60_000);
    o.flush_ms = Some(20);
    let run = spawn_relay(o, rt);
    let p = first_pty(&link).await;
    tick().await;
    link.data(&p.id, "Opening https://app.netlify.com/authorize?ticket=abc");
    assert!(!t.text().contains("o opens it on this computer"));
    tokio::time::sleep(Duration::from_millis(60)).await;
    assert_eq!(t.text().matches("o opens it on this computer").count(), 1);
    assert!(t.text().contains(&hyperlink("https://app.netlify.com/authorize?ticket=abc")));
    link.exit(&p.id, 0);
    assert_eq!(run.await.unwrap().0.unwrap().urls, 1);
}

/// signin-relay.test.ts:271 (read file 71-89).
#[tokio::test]
async fn an_o_long_after_the_url_is_a_keystroke_and_a_failed_open_says_so() {
    let link = FakeLink::new();
    let (t, rt) = terminal(true, 120, 40);
    let (_o, open) = opener(false);
    let clock = Arc::new(Mutex::new(1_000_000.0));
    let mut o = relay_opts(Arc::new(Link(link.clone())), Some("netlify login"), open, 60_000);
    let c = clock.clone();
    o.now = Some(Arc::new(move || *c.lock().unwrap()));
    let run = spawn_relay(o, rt);
    let p = first_pty(&link).await;
    tick().await;
    link.data(&p.id, "Opening https://app.netlify.com/authorize?response_type=ticket&ticket=abc \r\n");
    t.type_keys("o");
    tick().await;
    assert_eq!(writes_after_line(&pty(&link, 0)), Vec::<String>::new());
    assert!(strip_vt(&t.text()).contains(OPEN_FAILED));
    *clock.lock().unwrap() += OFFER_MS + 1.0;
    t.type_keys("o");
    tick().await;
    assert_eq!(writes_after_line(&pty(&link, 0)), vec!["o"]);
    link.exit(&p.id, 130);
    let (outcome, _) = run.await.unwrap();
    assert_eq!(outcome.unwrap(), RelayOutcome { exit_code: 130, timed_out: false, dropped: false, urls: 1, opened: 0 });
}

/// signin-relay.test.ts:291 (read file 91-100).
#[tokio::test]
async fn a_tool_that_never_finishes_times_out_and_its_pty_is_killed() {
    let link = FakeLink::new();
    let (t, rt) = terminal(true, 120, 40);
    let (_o, open) = opener(true);
    let run = spawn_relay(relay_opts(Arc::new(Link(link.clone())), Some("gcloud auth login"), open, 20), rt);
    first_pty(&link).await;
    let (outcome, _) = run.await.unwrap();
    assert_eq!(outcome.unwrap(), RelayOutcome { exit_code: -1, timed_out: true, dropped: false, urls: 0, opened: 0 });
    assert!(pty(&link, 0).killed);
    assert_eq!(t.raw_seen(), vec![true, false]);
}

/// signin-relay.test.ts:302 (read file 102-114).
#[tokio::test]
async fn a_link_that_drops_under_the_pty_ends_the_relay_with_the_terminal_restored() {
    let link = FakeLink::new();
    let drop = link.closable();
    let (t, rt) = terminal(true, 120, 40);
    let (_o, open) = opener(true);
    let run = spawn_relay(relay_opts(Arc::new(Link(link.clone())), Some("codex login"), open, 60_000), rt);
    first_pty(&link).await;
    tick().await;
    drop.send(true).unwrap();
    let (outcome, _) = run.await.unwrap();
    assert_eq!(outcome.unwrap(), RelayOutcome { exit_code: -1, timed_out: false, dropped: true, urls: 0, opened: 0 });
    assert_eq!(t.raw_seen(), vec![true, false]);
    assert!(pty(&link, 0).killed);
}

/// signin-relay.test.ts:316 (read file 116-129).
#[tokio::test]
async fn a_bare_shell_types_no_command_line_and_leaves_raw_mode_alone_off_a_tty() {
    let link = FakeLink::new();
    let (t, rt) = terminal(false, 120, 40);
    let (_o, open) = opener(true);
    let run = spawn_relay(relay_opts(Arc::new(Link(link.clone())), None, open, 60_000), rt);
    first_pty(&link).await;
    tick().await;
    assert_eq!(pty(&link, 0).writes, Vec::<String>::new());
    assert_eq!(t.raw_seen(), Vec::<bool>::new());
    t.type_keys("exit\r");
    tick().await;
    assert_eq!(pty(&link, 0).writes, vec!["exit\r"]);
    assert_eq!(link.staged(), vec![], "a bare shell stages nothing");
    let id = pty(&link, 0).id;
    link.exit(&id, 0);
    run.await.unwrap().0.unwrap();
}

/// signin-relay.ts:208: env rides pty.create, after cols, rows, shell and run.
#[tokio::test]
async fn the_environment_rides_the_pty_create_after_the_shell() {
    let link = FakeLink::with_bare(Some(true));
    let (_t, rt) = terminal(true, 90, 20);
    let (_o, open) = opener(true);
    let mut o = relay_opts(Arc::new(Link(link.clone())), Some("codex login"), open, 60_000);
    o.env = Some([("CODEX_HOME".to_string(), "/wsp/logins/codex".to_string())].into_iter().collect());
    let _run = spawn_relay(o, rt);
    let created = first_pty(&link).await.created;
    assert_eq!(
        serde_json::to_string(&created).unwrap(),
        r#"{"cols":90,"rows":20,"shell":"bash","run":"exec bash --noprofile --norc","env":{"CODEX_HOME":"/wsp/logins/codex"}}"#
    );
}

/// signin-relay.ts:205-206: a terminal that knows no size opens the pty at 80 by 24.
#[tokio::test]
async fn a_terminal_with_no_size_opens_the_pty_at_80_by_24() {
    let link = FakeLink::new();
    let (t, rt) = terminal(true, 1, 1);
    *t.out.size.lock().unwrap() = (None, None);
    let (_o, open) = opener(true);
    let _run = spawn_relay(relay_opts(Arc::new(Link(link.clone())), Some("x"), open, 60_000), rt);
    assert_eq!(serde_json::Value::Object(first_pty(&link).await.created), json!({"cols": 80, "rows": 24, "shell": "bash"}));
}

// ---- the questions a row declares

/// signin-relay.test.ts:137 (read file 137-146).
#[test]
fn each_question_is_reported_once_past_the_colours_and_reading_stops_once_every_one_is_seen() {
    let creds = Question { asks: JsRegex::new(r"Authenticate Git with your GitHub credentials\?"), settle: Settle::Answer("y\r".into()) };
    let mut scanner = QuestionScanner::new(vec![enter_q(), creds]);
    assert_eq!(scanner.feed("\x1b[0;33m!\x1b[0m First copy your one-time code: \x1b[0;1;39m72F3-072B\x1b[0m\r\n"), vec![]);
    assert_eq!(
        answers(&scanner.feed("\x1b[0;1;39mPress Enter\x1b[0m to open https://github.com/login/device in your browser... ")),
        vec![Some("\r".to_string())]
    );
    assert_eq!(scanner.feed(GH_WEB), vec![]);
    assert_eq!(answers(&scanner.feed(GH_CREDENTIALS)), vec![Some("y\r".to_string())]);
    assert_eq!(scanner.feed(GH_CREDENTIALS), vec![]);
}

/// signin-relay.test.ts:148.
#[test]
fn a_question_split_across_chunks_and_an_escape_split_with_it_are_seen() {
    let mut split = QuestionScanner::new(vec![enter_q()]);
    assert_eq!(split.feed("Press Ent"), vec![]);
    assert_eq!(answers(&split.feed("er to open https://github.com/login/device in your browser... ")), vec![Some("\r".to_string())]);
    let mut escaped = QuestionScanner::new(vec![enter_q()]);
    assert_eq!(escaped.feed("\x1b[0;1;39mPress Enter\x1b["), vec![]);
    assert_eq!(answers(&escaped.feed("0m to open https://github.com/login/device in your browser... ")), vec![Some("\r".to_string())]);
    let mut quiet = QuestionScanner::new(vec![]);
    assert_eq!(quiet.feed(GH_WEB), vec![]);
}

/// signin-relay.test.ts:160.
#[test]
fn a_question_carries_the_tools_own_words() {
    assert_eq!(
        QuestionScanner::new(vec![theirs_q()]).feed(AWS_SSO),
        vec![Asked { question: theirs_q(), matched: "SSO session name (Recommended)".into() }]
    );
}

/// signin-relay.ts:128: escapes and OSC 8 come out of the joined text before the shapes are read.
#[test]
fn a_question_painted_inside_a_hyperlink_is_still_seen() {
    let mut s = QuestionScanner::new(vec![enter_q()]);
    let got = s.feed("\x1b]8;;https://x.test\x1b\\Press Enter\x1b]8;;\x1b\\ to continue");
    assert_eq!(got, vec![Asked { question: enter_q(), matched: "Press Enter to continue".into() }]);
}

async fn watched_questions(questions: Vec<Question>) -> (Option<String>, Vec<String>, Vec<String>) {
    let link = FakeLink::new();
    link.set_script(|l, pty, line| {
        if !line.contains("; exec bash -c ") {
            return;
        }
        l.data(pty, "! First copy your one-time code: 72F3-072B\r\n");
        l.data(pty, "Press Enter to open https://github.com/login/device in your browser... ");
        l.data(pty, "SSO session name (Recommended): ");
    });
    let seen: Arc<Mutex<Vec<String>>> = Arc::default();
    let s = seen.clone();
    let mut o = watch_opts(Arc::new(Link(link.clone())), "gh auth login --web", 100);
    o.questions = questions;
    o.on_question = Some(Arc::new(move |a: &Asked| s.lock().unwrap().push(a.matched.clone())));
    watch_pty(o).await.unwrap();
    tick().await;
    let p = pty(&link, 0);
    let seen = seen.lock().unwrap().clone();
    (p.ran.clone(), writes_after_line(&p), seen)
}

/// signin-relay.test.ts:164.
#[tokio::test]
async fn an_answer_is_typed_on_the_tools_pty_as_its_line_arrives_and_nothing_for_the_persons() {
    assert_eq!(
        watched_questions(vec![enter_q()]).await,
        (Some("gh auth login --web".into()), vec!["\r".into()], vec!["Press Enter to open".into()])
    );
    assert_eq!(
        watched_questions(vec![theirs_q()]).await,
        (Some("gh auth login --web".into()), vec![], vec!["SSO session name (Recommended)".into()])
    );
    assert_eq!(watched_questions(vec![]).await, (Some("gh auth login --web".into()), vec![], vec![]));
}

// ---- watchPty

/// signin-relay.test.ts:184.
#[tokio::test]
async fn nothing_the_shell_printed_before_the_tool_started_is_answered_reported_or_handed_on() {
    let link = FakeLink::new();
    *link.banner.lock().unwrap() = Some("Press Enter to open https://evil.example/login\r\n".into());
    link.set_script(|l, pty, line| {
        if line.contains("; exec bash -c ") {
            l.exit(pty, 0);
        }
    });
    let seen: Opened = Arc::default();
    let asked: Opened = Arc::default();
    let chunks: Opened = Arc::default();
    let mut o = watch_opts(Arc::new(Link(link.clone())), "gh auth login --web", 1_000);
    o.flush_ms = Some(5);
    o.questions = vec![Question { asks: JsRegex::new("Press Enter to open"), settle: Settle::Answer("\r".into()) }];
    let (s, a, c) = (seen.clone(), asked.clone(), chunks.clone());
    o.on_question = Some(Arc::new(move |q: &Asked| a.lock().unwrap().push(q.matched.clone())));
    o.on_url = Some(Arc::new(move |u: &str| s.lock().unwrap().push(u.into())));
    o.on_data = Some(Arc::new(move |d: &str| c.lock().unwrap().push(d.into())));
    watch_pty(o).await.unwrap();
    tokio::time::sleep(Duration::from_millis(20)).await;
    assert_eq!((seen.lock().unwrap().clone(), asked.lock().unwrap().clone(), pty(&link, 0).writes.len()), (vec![], vec![], 1));
    assert!(!chunks.lock().unwrap().join("").contains("evil"));
}

/// signin-relay.test.ts:198.
#[tokio::test]
async fn a_watched_pty_is_wide_types_nothing_back_reports_each_page_once_and_ends_with_the_tools_exit() {
    let link = FakeLink::new();
    link.set_script(|l, pty, line| {
        if !line.contains("; exec bash -c ") {
            return;
        }
        l.data(pty, "visit https://github.com/login/device to sign in\r\n");
        l.data(pty, "still https://github.com/login/device, then https://github.com/settings\r\n");
        l.exit(pty, 0);
    });
    let seen: Opened = Arc::default();
    let chunks: Opened = Arc::default();
    let mut o = watch_opts(Arc::new(Link(link.clone())), "gh auth login", 60_000);
    let (s, c) = (seen.clone(), chunks.clone());
    o.on_url = Some(Arc::new(move |u: &str| s.lock().unwrap().push(u.into())));
    o.on_data = Some(Arc::new(move |d: &str| c.lock().unwrap().push(d.into())));
    let out = watch_pty(o).await.unwrap();
    assert_eq!(out, WatchOutcome { exit_code: 0, timed_out: false, dropped: false, stopped: false });
    assert_eq!(*seen.lock().unwrap(), vec!["https://github.com/login/device".to_string(), "https://github.com/settings".to_string()]);
    assert!(chunks.lock().unwrap().join("").contains("visit https://github.com/login/device"));
    let p = pty(&link, 0);
    assert_eq!(
        (p.created.get("cols"), p.created.get("rows"), p.created.get("shell")),
        (Some(&json!(200)), Some(&json!(50)), Some(&json!("bash")))
    );
    assert_eq!(p.ran.as_deref(), Some("gh auth login"));
    assert_eq!(p.writes.len(), 1);
    assert!(p.killed);
    assert!(link.staged().iter().all(|s| s.cleared));
}

/// signin-relay.test.ts:218.
#[tokio::test]
async fn the_callers_stop_a_dropped_link_and_a_timeout_each_end_it_with_no_exit_code_and_kill_the_pty() {
    let held = || {
        let link = FakeLink::new();
        link.set_script(|l, pty, line| {
            if line.contains("; exec bash -c ") {
                l.data(pty, "waiting for you...\r\n");
            }
        });
        link
    };
    let stopping = held();
    let (stop_tx, stop_rx) = tokio::sync::oneshot::channel::<()>();
    let mut o = watch_opts(stopping.dial(), "gh auth login", 60_000);
    o.stop = Some(
        async move {
            let _ = stop_rx.await;
        }
        .boxed(),
    );
    let stopped = tokio::spawn(watch_pty(o));
    first_pty(&stopping).await;
    tick().await;
    stop_tx.send(()).unwrap();
    assert_eq!(stopped.await.unwrap().unwrap(), WatchOutcome { exit_code: -1, timed_out: false, dropped: false, stopped: true });
    assert!(pty(&stopping, 0).killed);

    let dropping = held();
    let gone = tokio::spawn(watch_pty(watch_opts(dropping.dial(), "gh auth login", 60_000)));
    first_pty(&dropping).await;
    tick().await;
    dropping.drop_view();
    let out = gone.await.unwrap().unwrap();
    assert_eq!((out.exit_code, out.dropped, out.stopped, out.timed_out), (-1, true, false, false));

    let slow = held();
    assert_eq!(
        watch_pty(watch_opts(Arc::new(Link(slow.clone())), "gh auth login", 20)).await.unwrap(),
        WatchOutcome { exit_code: -1, timed_out: true, dropped: false, stopped: false }
    );
    assert!(pty(&slow, 0).killed);
}

/// signin-relay.ts:355-360: the first end wins; a stop after the exit changes nothing.
#[tokio::test]
async fn the_first_end_wins_and_a_later_one_changes_nothing() {
    let link = FakeLink::new();
    link.set_script(|l, pty, line| {
        if line.contains("; exec bash -c ") {
            l.exit(pty, 3);
        }
    });
    let mut o = watch_opts(Arc::new(Link(link.clone())), "x", 60_000);
    o.stop = Some(async {}.boxed());
    let out = watch_pty(o).await.unwrap();
    assert!(
        out == WatchOutcome { exit_code: 3, timed_out: false, dropped: false, stopped: false }
            || out == WatchOutcome { exit_code: -1, timed_out: false, dropped: false, stopped: true },
        "{out:?}"
    );
    assert!(!(out.stopped && out.exit_code != -1));
}

/// signin-relay.ts:396 and :403: on_typing gets the pty's input once attached and None once it is gone; a refused write
/// rejects with the daemon's reason.
#[tokio::test]
async fn on_typing_hands_the_ptys_input_while_attached_and_none_after() {
    let link = FakeLink::new();
    let handed: Arc<Mutex<Vec<Option<PtyTyping>>>> = Arc::default();
    let h = handed.clone();
    let mut o = watch_opts(Arc::new(Link(link.clone())), "x", 60_000);
    o.on_typing = Some(Arc::new(move |w| h.lock().unwrap().push(w)));
    let run = tokio::spawn(watch_pty(o));
    first_pty(&link).await;
    tick().await;
    let write = handed.lock().unwrap()[0].clone().expect("the input is handed once attached");
    write("CODE-1234\r".into()).await.unwrap();
    assert_eq!(pty(&link, 0).writes.last().map(String::as_str), Some("CODE-1234\r"));
    *link.override_op.lock().unwrap() =
        Some(Arc::new(|op, _| if op == "pty.write" { Some(frame(json!({"ok": false, "error": "pty gone"}))) } else { None }));
    assert_eq!(write("x".into()).await.unwrap_err().message, "pty.write refused: pty gone");
    *link.override_op.lock().unwrap() = None;
    let id = pty(&link, 0).id;
    link.exit(&id, 0);
    run.await.unwrap().unwrap();
    let handed = handed.lock().unwrap();
    assert_eq!(handed.len(), 2);
    assert!(handed[1].is_none());
}

// ---- a refused pty op

fn refusing(op: &'static str) -> Arc<FakeLink> {
    let link = FakeLink::new();
    *link.override_op.lock().unwrap() = Some(Arc::new(move |name, _| {
        if name == op {
            Some(frame(json!({"id": 1, "ok": false, "error": format!("{op} is not allowed here")})))
        } else {
            None
        }
    }));
    link
}

/// signin-relay.test.ts:261 (read file 261-269).
#[tokio::test]
async fn pty_create_refused_throws_the_daemons_reason_and_nothing_goes_raw_or_follows() {
    let link = refusing("pty.create");
    let (t, mut rt) = terminal(true, 120, 40);
    let (_o, open) = opener(true);
    let e = relay_pty(relay_opts(Arc::new(Link(link.clone())), Some("gh auth login"), open, 60_000), &mut rt).await.unwrap_err();
    assert_eq!(e.message, "pty.create refused: pty.create is not allowed here");
    assert_eq!(t.raw_seen(), Vec::<bool>::new());
    assert_eq!(link.op_names(), vec!["pty.create"]);
    let e = run_quiet(&Link(link.clone()), "gh auth status", 5_000, &Default::default()).await.unwrap_err();
    assert_eq!(e.message, "pty.create refused: pty.create is not allowed here");
    assert_eq!(link.op_names(), vec!["pty.create", "pty.create"]);
    let e = watch_pty(watch_opts(Arc::new(Link(link.clone())), "x", 5_000)).await.unwrap_err();
    assert_eq!(e.message, "pty.create refused: pty.create is not allowed here");
}

/// signin-relay.ts:152-156: a create that answers no pty id.
#[tokio::test]
async fn pty_create_with_no_pty_id_throws() {
    let link = FakeLink::new();
    *link.override_op.lock().unwrap() = Some(Arc::new(|op, _| if op == "pty.create" { Some(frame(json!({"ok": true}))) } else { None }));
    let e = run_quiet(&Link(link), "x", 5_000, &Default::default()).await.unwrap_err();
    assert_eq!(e.message, "pty.create answered without a pty id");
    let refused = FakeLink::new();
    *refused.override_op.lock().unwrap() =
        Some(Arc::new(|op, _| if op == "pty.create" { Some(frame(json!({"ok": false}))) } else { None }));
    let e = run_quiet(&Link(refused), "x", 5_000, &Default::default()).await.unwrap_err();
    assert_eq!(e.message, "pty.create refused: no reason given");
}

/// signin-relay.test.ts:271 (read file 271-280).
#[tokio::test]
async fn pty_attach_refused_throws_with_the_pty_killed_and_the_terminal_untouched() {
    let link = refusing("pty.attach");
    let (t, mut rt) = terminal(true, 120, 40);
    let (_o, open) = opener(true);
    let e = relay_pty(relay_opts(Arc::new(Link(link.clone())), Some("gh auth login"), open, 60_000), &mut rt).await.unwrap_err();
    assert!(e.message.starts_with("pty.attach refused"), "{}", e.message);
    assert_eq!(t.raw_seen(), Vec::<bool>::new());
    assert!(pty(&link, 0).killed);
    assert_eq!(pty(&link, 0).writes, Vec::<String>::new());
    let e = run_quiet(&Link(link.clone()), "gh auth status", 5_000, &Default::default()).await.unwrap_err();
    assert!(e.message.starts_with("pty.attach refused"));
    assert!(pty(&link, 1).killed);
}

// ---- runQuiet

/// signin-relay.test.ts:284.
#[tokio::test]
async fn the_quiet_run_returns_only_what_sits_between_the_echo_and_the_exit_marker() {
    let link = FakeLink::new();
    link.set_script(|l, pty, line| {
        if line.starts_with("gh auth status") {
            l.data(
                pty,
                "github.com\r\n  ✓ Logged in to github.com account someone (keyring)\r\n  - Token: gho_****\r\n\r\nWSP_STATUS 1\r\n",
            );
            l.exit(pty, 1);
        }
    });
    let res = run_quiet(&Link(link.clone()), "gh auth status", 5_000, &Default::default()).await.unwrap();
    let p = pty(&link, 0);
    assert_eq!(serde_json::to_string(&p.created).unwrap(), r#"{"cols":200,"rows":50,"shell":"/bin/sh","env":{"PS1":""}}"#);
    assert_eq!(p.ran.as_deref(), Some("gh auth status"));
    assert_eq!(p.writes.len(), 1);
    assert_eq!(p.writes[0], "c=$(cat '/staged/1/line'); rm -rf -- '/staged/1'; eval \"$c\"; printf '\\nWSP_STATUS %s\\n' $?; exit\r");
    assert_eq!(link.staged(), vec![StagedCmd { command: "gh auth status".into(), cleared: true }]);
    assert_eq!(
        res,
        QuietRun {
            output: "github.com\n  ✓ Logged in to github.com account someone (keyring)\n  - Token: gho_****".into(),
            exit_code: 1,
            timed_out: false,
            dropped: false
        }
    );
    assert!(p.killed);
}

/// signin-relay.ts:425: env rides after PS1, and a PS1 in it wins.
#[tokio::test]
async fn the_quiet_runs_environment_rides_after_an_empty_ps1() {
    let link = FakeLink::new();
    let env: indexmap::IndexMap<String, String> =
        [("CODEX_HOME".to_string(), "/l".to_string()), ("PS1".to_string(), "> ".to_string())].into_iter().collect();
    let _ = run_quiet(&Link(link.clone()), "x", 20, &env).await.unwrap();
    assert_eq!(serde_json::to_string(&pty(&link, 0).created["env"]).unwrap(), r#"{"PS1":"> ","CODEX_HOME":"/l"}"#);
}

/// signin-relay.test.ts:301.
#[tokio::test]
async fn the_quiet_runs_output_is_plain_text_with_colours_taken_out() {
    let link = FakeLink::new();
    link.set_script(|l, pty, line| {
        if line.starts_with("opencode auth list") {
            l.data(pty, "\x1b[0m\r\n●  Anthropic \x1b[90mANTHROPIC_API_KEY\r\n│\r\n└  1 environment variable\r\n\r\nWSP_STATUS 0\r\n");
            l.exit(pty, 0);
        }
    });
    let res = run_quiet(&Link(link), "opencode auth list", 5_000, &Default::default()).await.unwrap();
    assert_eq!(res.output, "●  Anthropic ANTHROPIC_API_KEY\n│\n└  1 environment variable");
    assert_eq!(res.exit_code, 0);
}

/// signin-relay.ts:428-431 and :463-467: the last marker line counts, and a marker inside a line does not.
#[tokio::test]
async fn the_last_whole_marker_line_is_the_exit_and_everything_after_it_is_dropped() {
    let link = FakeLink::new();
    link.set_script(|l, pty, line| {
        if line.starts_with("x") {
            l.data(pty, "say WSP_STATUS 9\r\nWSP_STATUS 4\r\nmore\r\nWSP_STATUS 2\r\ntrailing\r\n");
            l.exit(pty, 0);
        }
    });
    let res = run_quiet(&Link(link), "x", 5_000, &Default::default()).await.unwrap();
    assert_eq!(res, QuietRun { output: "say WSP_STATUS 9\nWSP_STATUS 4\nmore".into(), exit_code: 2, timed_out: false, dropped: false });
}

/// signin-relay.test.ts:313.
#[tokio::test]
async fn a_status_command_that_never_answers_times_out() {
    let link = FakeLink::new();
    let res = run_quiet(&Link(link.clone()), "codex login status", 20, &Default::default()).await.unwrap();
    assert_eq!(res, QuietRun { output: String::new(), exit_code: -1, timed_out: true, dropped: false });
    assert!(pty(&link, 0).killed);
}

/// signin-relay.test.ts:320.
#[tokio::test]
async fn a_link_that_drops_during_the_status_command_reads_as_dropped() {
    let link = FakeLink::new();
    let view = link.dial();
    let run = tokio::spawn(async move { run_quiet(view.as_ref(), "codex login status", 5_000, &Default::default()).await });
    first_pty(&link).await;
    tick().await;
    link.drop_view();
    assert_eq!(run.await.unwrap().unwrap(), QuietRun { output: String::new(), exit_code: -1, timed_out: false, dropped: true });
}
