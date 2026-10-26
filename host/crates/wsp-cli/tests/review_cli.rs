// SPDX-License-Identifier: AGPL-3.0-only
// The #2449 review's findings on wsp-cli: each test fails on 52ef71b1e and holds the Rust to what the TS command
// line at 65f6b89c7 printed under node on the same input (the probe output is quoted at each test).
#![allow(clippy::result_large_err, clippy::type_complexity, clippy::too_many_arguments, clippy::large_enum_variant, dead_code)]

#[path = "cli_b/common.rs"]
mod common;
#[path = "common/mod.rs"]
mod pty;

use common::{case, holds, Case, Run};
use serde_json::{json, Value};

fn run(frames: &[&str], stdout: &[&str], stderr: &[&str], exit: i32) -> Run {
    Run {
        frames: frames.iter().map(|s| s.to_string()).collect(),
        stdout: stdout.iter().map(|s| s.to_string()).collect(),
        stderr: stderr.iter().map(|s| s.to_string()).collect(),
        streamed: None,
        exit,
        asked: None,
    }
}

fn line(argv: &[&str], replies: Value, text: Run, json: Run) -> Case {
    let mut c: Case = serde_json::from_value(json!({
        "case": argv.join(" "),
        "hand": true,
        "argv": argv,
        "replies": replies,
        "text": {"frames": [], "stdout": [], "stderr": [], "exit": 0},
        "json": {"frames": [], "stdout": [], "stderr": [], "exit": 0},
    }))
    .expect("a case");
    c.text = text;
    c.json = json;
    c
}

const WAIT_FRAMES: [&str; 3] =
    [r#"{"id":2,"op":"sessions.list"}"#, r#"{"id":3,"op":"events.subscribe"}"#, r#"{"id":4,"op":"sessions.list"}"#];

/// `threads wait` keeps `seconds * 1000` whole (io.ts:315, turns-help.ts:692) and prints it through `fmtDuration`,
/// which rounds (units.ts:327): node prints `after 2ms` for `--timeout=0.0015` and `after 100ms` for 0.0995.
#[test]
fn threads_wait_prints_the_timeout_it_was_given_not_a_truncated_one() {
    let recorded = case("projects", "threads_wait", "timed out on one");
    for (timeout, said) in [("0.0015", "2ms"), ("0.0995", "100ms")] {
        let mut c = recorded.clone();
        c.argv = vec!["threads".into(), "wait".into(), "thread-7f3a9c21e4b5".into(), format!("--timeout={timeout}")];
        let stderr = format!("thread thread-7 still running after {said}");
        c.text = run(&WAIT_FRAMES, &[], &[&stderr], 0);
        c.json = run(&WAIT_FRAMES, &[r#"{"timedOut":true}"#], &[&stderr], 0);
        holds(&c);
    }
}

/// `activity end` sends the pid as the JavaScript number `Number(word)` read (activity.ts:105,111): node sends
/// `100000000000000000000`, `1e+21` and `9223372036854776000`; the Rust saturates each to `i64::MAX`.
#[test]
fn activity_end_sends_the_pid_as_the_number_read() {
    let ended = json!({"activity.end": json!({"id": 1, "ok": true, "pid": 7, "ended": [], "forced": [], "left": []}).to_string()});
    for (word, sent) in [("1e20", "100000000000000000000"), ("1e21", "1e+21"), ("9223372036854775807", "9223372036854776000")] {
        let frame = format!(r#"{{"pid":{sent},"startedAt":5,"id":2,"op":"activity.end"}}"#);
        let said = "nothing was signalled: the process listed as 7 is gone";
        let said_json = r#"{"pid":7,"ended":[],"forced":[],"left":[]}"#;
        let c = line(
            &["activity", "end", word, "--started", "5"],
            ended.clone(),
            run(&[&frame], &[said], &[], 0),
            run(&[&frame], &[said_json], &[], 0),
        );
        holds(&c);
    }
}

/// A lock whose `startedBy` is `null` names `wsp down` in the refusal: the TS asks `lock.startedBy !== undefined`
/// (host-lock.ts:128), and node answers "... wsp down stops it, or point --state at a different file." for it.
#[test]
fn a_lock_started_by_null_names_wsp_down_as_the_ts_does() {
    let dir = tempfile::tempdir().unwrap();
    let lock = dir.path().join("host.lock");
    let state = dir.path().join("state.json");
    let pid = std::process::id();
    std::fs::write(&lock, format!(r#"{{"pid":{pid},"port":4000,"startedAt":"2026-10-26T00:00:00.000Z","startedBy":null}}"#)).unwrap();
    let refused = wsp_cli::host_lock::refuse_if_served(lock.to_str().unwrap(), state.to_str().unwrap()).expect_err("a live lock refuses");
    assert_eq!(
        refused.message,
        format!(
            "another wsp host (pid {pid}) is already serving {} on port 4000. wsp down stops it, or point --state at a different file.",
            state.display()
        )
    );
}

/// A lock is read on `pid`, `port` and `startedAt` alone (host-lock.ts:45-56), so one whose `address` is not a
/// string still holds: node refuses a second host on it and `servingHost` names it. The Rust reads no lock and
/// lets a second host take a live host's state file.
#[test]
fn a_lock_whose_address_is_not_a_string_still_holds() {
    let dir = tempfile::tempdir().unwrap();
    let lock = dir.path().join("host.lock");
    let state = dir.path().join("state.json");
    let pid = std::process::id();
    std::fs::write(&lock, format!(r#"{{"pid":{pid},"port":4000,"startedAt":"2026-10-26T00:00:00.000Z","address":5}}"#)).unwrap();
    let refused = wsp_cli::host_lock::refuse_if_served(lock.to_str().unwrap(), state.to_str().unwrap()).err().map(|e| e.message);
    assert_eq!(
        refused,
        Some(format!(
            "another wsp host (pid {pid}) is already serving {} on port 4000. Stop it first, or point --state at a different file.",
            state.display()
        ))
    );
    assert!(wsp_cli::host_lock::serving_host(state.to_str().unwrap()).is_some(), "servingHost names the live host");
}

/// `removeHost` either removes a record that reads or throws (hosts.ts:134-138, `rmSync` with only `force`): on an
/// immutable record node throws `ENOTDIR: not a directory, scandir '<home>/hosts/box.json'`. False is the TS's
/// answer for "there was no record", which a caller (relay-link.ts:348, :444) reads as nothing to say.
#[test]
fn remove_host_never_answers_false_for_a_record_it_could_not_remove() {
    use wsp_cli::hosts::{hosts_dir, read_host, remove_host, write_host, HostRecord, Via};
    let home = tempfile::tempdir().unwrap();
    let home = home.path().to_str().unwrap().to_owned();
    let record = HostRecord {
        url: "http://box.local:4400".into(),
        device_id: "d_1".into(),
        device_token: "t".into(),
        host_key: None,
        paired_at: None,
        via: Via { kind: "account".into(), host_id: "h".into() },
    };
    write_host(&home, "box", &record).unwrap();
    let file = format!("{}/box.json", hosts_dir(&home));
    struct Mutable(String);
    impl Drop for Mutable {
        fn drop(&mut self) {
            let _ = std::process::Command::new("chattr").args(["-i", &self.0]).status();
        }
    }
    assert!(std::process::Command::new("chattr").args(["+i", &file]).status().unwrap().success(), "chattr +i (run as root on ext4)");
    let _undo = Mutable(file.clone());
    assert!(read_host(&home, "box").is_some());
    let answered = remove_host(&home, "box");
    assert_eq!(answered.err().map(|e| e.message), Some(format!("ENOTDIR: not a directory, scandir '{file}'")));
}

mod relay {
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use async_trait::async_trait;
    use futures::future::BoxFuture;
    use futures::FutureExt;
    use serde_json::{json, Value};
    use tokio::sync::Notify;
    use wsp_cli::signin_relay::{relay_pty, Detach, EventSink, Frame, PtyLink, RelayOptions};
    use wsp_words::exit::HostError;

    use crate::pty;

    /// A daemon link that holds its answer to the `pty.write` of `"a"` until released, and records each op as sent.
    #[derive(Default)]
    struct SlowLink {
        sent: Mutex<Vec<String>>,
        sinks: Mutex<Vec<EventSink>>,
        release: Notify,
    }

    struct Held(Arc<SlowLink>);

    #[async_trait]
    impl PtyLink for Held {
        async fn op(&self, op: &str, extra: Frame) -> Result<Frame, HostError> {
            let link = &self.0;
            let data = extra.get("data").cloned().unwrap_or(Value::Null);
            link.sent.lock().unwrap().push(if op == "pty.write" { format!("pty.write {data}") } else { op.to_owned() });
            if op == "pty.write" && data == json!("a") {
                link.release.notified().await;
            }
            let answer = if op == "pty.create" { json!({"ok": true, "ptyId": "p1"}) } else { json!({"ok": true}) };
            Ok(answer.as_object().unwrap().clone())
        }
        fn on_event(&self, sink: EventSink) -> Detach {
            self.0.sinks.lock().unwrap().push(sink);
            Detach(Box::new(|| {}))
        }
        fn closed(&self) -> Option<BoxFuture<'static, ()>> {
            None
        }
        fn bare(&self) -> Option<bool> {
            None
        }
    }

    /// `relayPty` hands each keystroke to the daemon and goes on (`void o.link.op("pty.write", ...)`,
    /// signin-relay.ts:263): node sends `pty.write "a"` and `pty.write "b"` while the first is unanswered, and when the
    /// pty exits the relay ends at once (`{"exitCode":0,...}`, then `pty.kill`) with that write still unanswered.
    #[tokio::test]
    async fn the_relay_does_not_wait_on_a_keystrokes_answer() {
        let link = Arc::new(SlowLink::default());
        let (t, mut rt) = pty::terminal(true, 120, 40);
        let o = RelayOptions {
            link: Arc::new(Held(link.clone())),
            command: None,
            open: Arc::new(|_| async { true }.boxed()),
            callback_url: None,
            on_consent: None,
            env: None,
            timeout_ms: 60_000,
            flush_ms: None,
            now: None,
        };
        let run = tokio::spawn(async move { relay_pty(o, &mut rt).await });
        tokio::time::sleep(Duration::from_millis(50)).await;
        t.type_keys("a");
        tokio::time::sleep(Duration::from_millis(20)).await;
        t.type_keys("b");
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert_eq!(
            *link.sent.lock().unwrap(),
            ["pty.create", "pty.attach", "pty.write \"a\"", "pty.write \"b\""],
            "both keystrokes go out at once"
        );
        for sink in link.sinks.lock().unwrap().iter() {
            sink(json!({"type": "pty.exit", "ptyId": "p1", "exitCode": 0}).as_object().unwrap());
        }
        let ended = tokio::time::timeout(Duration::from_secs(1), run).await;
        link.release.notify_waiters();
        let outcome = ended.expect("the relay ends when the pty exits").unwrap().unwrap();
        assert_eq!(outcome.exit_code, 0);
    }
}

/// `wsp terminal config` prints the object `readGhosttyConfig` built, keys in the order the config named them
/// (threads.ts:658-660, :668). Node over this config writes `files, fontFamily, palette, background, fontSize,
/// foreground, cursorColor, theme` for dark; the Rust reads it through a struct and writes `fontSize, theme` early
/// and `palette` late, on the line's `--json` and the tool's answer alike.
#[test]
fn terminal_config_json_keeps_the_order_the_config_was_read_in() {
    let dir = tempfile::tempdir().unwrap();
    let g = dir.path().join("ghostty");
    std::fs::create_dir_all(g.join("themes")).unwrap();
    std::fs::write(
        g.join("config"),
        "# a comment\nfont-family = \"JetBrains Mono\"\nfont-family = Fira Code\nfont-size = 13.5\ntheme = light:Mine,dark:Night\npalette = 0=#1d1f21\npalette = 15 = #ffffff\nbackground = 101010\nforeground = #e0e0e0\ncursor-color = #ff00ff\nselection-background = #333\nconfig-file = ?extra.conf\nconfig-file = missing.conf\n",
    )
    .unwrap();
    std::fs::write(g.join("extra.conf"), "palette = 4=#0000ee\nfont-family =\nfont-family = Iosevka Term\n").unwrap();
    std::fs::write(g.join("themes/Mine"), "background = #fafafa\nforeground = #111111\npalette = 1=#cc0000\n").unwrap();
    std::fs::write(g.join("themes/Night"), "background = #000010\npalette = 2=#00cc00\n").unwrap();
    std::env::set_var("XDG_CONFIG_HOME", dir.path());
    let d = dir.path().display().to_string();
    let palette = r#"[{"r":29,"g":31,"b":33},null,{"r":0,"g":204,"b":0},null,{"r":0,"g":0,"b":238},null,null,null,null,null,null,null,null,null,null,{"r":255,"g":255,"b":255}]"#;
    let json = format!(
        r#"{{"files":["{d}/ghostty/config","{d}/ghostty/extra.conf","{d}/ghostty/themes/Night"],"fontFamily":["Iosevka Term"],"palette":{palette},"background":{{"r":16,"g":16,"b":16}},"fontSize":13.5,"foreground":{{"r":224,"g":224,"b":224}},"cursorColor":{{"r":255,"g":0,"b":255}},"theme":"Night"}}"#
    );
    let text = format!(
        "Read {d}/ghostty/config, {d}/ghostty/extra.conf, {d}/ghostty/themes/Night\nfont-family = Iosevka Term\nfont-size = 13.5\ntheme = Night\nbackground = #101010\nforeground = #e0e0e0\npalette = 4 of 16 colors\ncursor-color = #ff00ff"
    );
    let c = line(&["terminal", "config"], json!({}), run(&[], &[&text], &[], 0), run(&[], &[&json], &[], 0));
    holds(&c);
}
