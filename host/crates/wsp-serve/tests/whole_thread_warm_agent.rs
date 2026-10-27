// SPDX-License-Identifier: AGPL-3.0-only
//! packages/runtime/test/thread-warm-agent.test.ts at 65f6b89c7, case for case. A new thread's agent process started
//! ahead of its first send, as the composer asks when it takes focus: the shipped Claude adapter over this computer's
//! runs, driving a stand-in agent that says its pid and arguments as it starts and names the process and the turn token
//! in every reply. Each test names the TS test above it.
mod common;

use std::path::PathBuf;

use common::stubs::{alive, claude_keeper, gone, pid_of, token_of};
use common::*;
use indexmap::IndexMap;
use serde_json::{json, Value};
use wsp_core::clock::FakeClock;
use wsp_core::types::api::{OutsideOpening, StartOptions, WarmAsk};
use wsp_core::types::wiring::AgentsReach;
use wsp_wire::types::place_word::HERE_PLACE_ID;
use wsp_wire::views::session::ForkSource;
use wsp_wire::views::workspace::Caller;
use wsp_wire::views::workspace_more::WorkspaceOrigin;

/// The window the owner ruled on 2026-10-11, written out so a change to the constant fails here.
const WARM_MS: i64 = 3 * 60_000;

struct Host {
    root: tempfile::TempDir,
    heard: PathBuf,
    fx: Fx,
}

impl Drop for Host {
    /// A red run leaves nothing burning a core: every process the stand-in said it started, which this test started.
    fn drop(&mut self) {
        for up in ups(&self.heard) {
            let _ = std::process::Command::new("kill").args(["-KILL", &up.0.to_string()]).output();
        }
    }
}

fn host(clock: Option<&FakeClock>) -> Host {
    let root = tempfile::tempdir().expect("a folder");
    let heard = root.path().join("heard");
    std::fs::create_dir(root.path().join("bin")).expect("a folder");
    claude_keeper(&root.path().join("bin/claude"));
    let path = format!("{}:{}", root.path().join("bin").display(), std::env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin".into()));
    let vars = IndexMap::from([("PATH".to_string(), path), ("STUB_HEARD".to_string(), heard.display().to_string())]);
    let local = fake_local_with(root.path(), Some(vars), |o| o.poll_ms = Some(20));
    let claude = wsp_core::adapters::harness_adapters()["claude"].clone();
    let mut wired = runtime(&StubBackend::new(None)).adapter("claude", claude).with(move |o| {
        o.local = Some(local);
        o.agents = Some(AgentsReach { here_url: Some("http://127.0.0.1:9".into()), wsp_mcp: None });
    });
    if let Some(clock) = clock {
        wired = wired.clock(clock);
    }
    Host { root, heard, fx: wired.build() }
}

/// Every agent process the stand-in started, in the order it started them: its pid and the arguments it was given.
fn ups(heard: &PathBuf) -> Vec<(u32, String)> {
    std::fs::read_to_string(heard)
        .unwrap_or_default()
        .lines()
        .filter(|l| l.starts_with("up "))
        .map(|l| {
            let words: Vec<&str> = l.split(' ').collect();
            (words[1].parse().expect("a pid"), words[2..].join(" "))
        })
        .collect()
}

async fn up_count(h: &Host, n: usize) -> Vec<(u32, String)> {
    until_for(10_000, &mut || async { ups(&h.heard).len() == n }).await;
    ups(&h.heard)
}

#[derive(Default)]
struct More {
    thread: Option<String>,
    harness: Option<&'static str>,
    model: Option<&'static str>,
    effort: Option<&'static str>,
    permission_mode: Option<&'static str>,
}

fn prompt(p: &str) -> StartOptions {
    StartOptions { prompt: p.into(), ..Default::default() }
}

async fn send(h: &Host, ws: &str, text: &str, more: More, origin: Option<Caller>) -> (Option<String>, String) {
    let o = StartOptions {
        thread: more.thread,
        harness: more.harness.map(str::to_string),
        model: more.model.map(str::to_string),
        effort: more.effort.map(str::to_string),
        permission_mode: more.permission_mode.map(str::to_string),
        ..prompt(text)
    };
    let handle = h.fx.rt.sessions().start(ws, o, origin).await.expect("a start");
    let result = handle.finished().await;
    (result.text.clone(), handle.view().thread_id.expect("a thread"))
}

fn claude() -> More {
    More { harness: Some("claude"), ..Default::default() }
}

fn warm_ask(model: Option<&str>, effort: Option<&str>, mode: Option<&str>, fast: Option<bool>) -> WarmAsk {
    WarmAsk {
        harness: Some("claude".into()),
        model: model.map(str::to_string),
        effort: effort.map(str::to_string),
        permission_mode: mode.map(str::to_string),
        fast,
        ..Default::default()
    }
}

async fn warm(h: &Host, ws: &str, ask: WarmAsk, origin: Option<Caller>) -> Value {
    j(&h.fx.rt.sessions().warm(ws, ask, origin).await.expect("a warm"))
}

async fn plain_warm(h: &Host, ws: &str) -> Value {
    warm(h, ws, warm_ask(None, None, None, None), None).await
}

async fn mac(h: &Host, name: &str) -> String {
    create_on(&h.fx.rt, CreateOn::named(name).on(HERE_PLACE_ID), None).await.expect("a workspace").view.id
}

fn is_session_id(s: &str) -> bool {
    s.len() == 36 && s.chars().all(|c| c.is_ascii_hexdigit() || c == '-')
}

fn as_lead(ws: &str, thread: &str) -> Caller {
    let by = serde_json::from_value(json!({ "kind": "thread", "threadId": thread, "workspaceId": ws, "rootThreadId": thread }))
        .expect("a scope");
    Caller::Scoped { origin: WorkspaceOrigin::Here, by }
}

// "an agent process started ahead of a new thread's first send" > "runs the send that opens a thread on it, the process up before the send, and keeps it for the thread's next"
#[test]
fn runs_the_send_that_opens_a_thread_on_it_the_process_up_before_the_send_and_keeps_it_for_the_threads_next() {
    drive(|| async {
        let h = host(None);
        let ws = mac(&h, "mac").await;
        assert_eq!(plain_warm(&h, &ws).await, json!({ "warm": "started" }));
        let up = up_count(&h, 1).await;
        let warmed = &up[0];
        assert!(warmed.1.contains("--model claude-opus-5-5"), "{}", warmed.1);
        let (text, thread) = send(&h, &ws, "one", claude(), None).await;
        assert_eq!(text.as_deref(), Some(format!("one from {} turn 1 token {}", warmed.0, token_of(text.as_deref())).as_str()));
        assert_eq!(ups(&h.heard).len(), 1);
        // The thread's next send runs on the same process, as any thread's kept one.
        let (two, _) = send(&h, &ws, "two", More { thread: Some(thread.clone()), ..Default::default() }, None).await;
        assert_eq!(pid_of(two.as_deref()), Some(warmed.0));
        assert_eq!(token_of(two.as_deref()), token_of(text.as_deref()));
        let rows = h.fx.rt.sessions().list(Some(ws.clone()), None).await.expect("a list");
        let row = rows.iter().find(|r| r.thread_id.as_deref() == Some(thread.as_str())).expect("the row");
        assert_eq!(j(row)["status"], "completed");
        assert!(is_session_id(j(row)["claudeSessionId"].as_str().expect("a session id")));
        h.fx.rt.close().await.expect("a close");
    });
}

// "never outlives its three idle minutes, and a send after it boots a process of its own"
#[test]
fn never_outlives_its_three_idle_minutes_and_a_send_after_it_boots_a_process_of_its_own() {
    drive(|| async {
        let fc = FakeClock::at_now();
        let h = host(Some(&fc));
        let ws = mac(&h, "mac").await;
        plain_warm(&h, &ws).await;
        let first = up_count(&h, 1).await[0].0;
        fc.advance(WARM_MS - 1_000);
        assert!(alive(first));
        fc.advance(1_000);
        gone(first).await;
        // Asked for again before its window is out, the window starts over.
        assert_eq!(plain_warm(&h, &ws).await, json!({ "warm": "started" }));
        let second = up_count(&h, 2).await[1].0;
        fc.advance(WARM_MS - 1_000);
        assert_eq!(plain_warm(&h, &ws).await, json!({ "warm": "standing" }));
        fc.advance(WARM_MS - 1_000);
        assert!(alive(second));
        fc.advance(1_000);
        gone(second).await;
        let (one, _) = send(&h, &ws, "one", claude(), None).await;
        assert_ne!(pid_of(one.as_deref()), Some(second));
        assert_eq!(ups(&h.heard).len(), 3);
        h.fx.rt.close().await.expect("a close");
    });
}

// "asked for a minute apart for five minutes, as the composer does while the person types, runs the send on the process started at the first"
#[test]
fn asked_for_a_minute_apart_for_five_minutes_as_the_composer_does_while_the_person_types_runs_the_send_on_the_process_started_at_the_first()
{
    drive(|| async {
        let fc = FakeClock::at_now();
        let h = host(Some(&fc));
        let ws = mac(&h, "mac").await;
        plain_warm(&h, &ws).await;
        let warmed = up_count(&h, 1).await[0].0;
        for _ in 1..=5 {
            fc.advance(60_000);
            assert_eq!(plain_warm(&h, &ws).await, json!({ "warm": "standing" }));
        }
        fc.advance(WARM_MS - 1_000);
        assert!(alive(warmed));
        let (one, _) = send(&h, &ws, "one", claude(), None).await;
        assert_eq!(pid_of(one.as_deref()), Some(warmed));
        assert_eq!(ups(&h.heard).len(), 1);
        h.fx.rt.close().await.expect("a close");
    });
}

// "a changed model, effort or access ends the one standing and starts one launched with it"
#[test]
fn a_changed_model_effort_or_access_ends_the_one_standing_and_starts_one_launched_with_it() {
    drive(|| async {
        let h = host(None);
        let ws = mac(&h, "mac").await;
        plain_warm(&h, &ws).await;
        let first = up_count(&h, 1).await[0].0;
        assert_eq!(warm(&h, &ws, warm_ask(Some("claude-sonnet-5"), None, None, None), None).await, json!({ "warm": "started" }));
        let second = up_count(&h, 2).await[1].clone();
        gone(first).await;
        assert!(second.1.contains("--model claude-sonnet-5"), "{}", second.1);
        warm(&h, &ws, warm_ask(Some("claude-sonnet-5"), Some("low"), None, None), None).await;
        let third = up_count(&h, 3).await[2].clone();
        gone(second.0).await;
        assert!(third.1.contains("--effort low"), "{}", third.1);
        warm(&h, &ws, warm_ask(Some("claude-sonnet-5"), Some("low"), Some("acceptEdits"), None), None).await;
        let fourth = up_count(&h, 4).await[3].clone();
        gone(third.0).await;
        assert!(fourth.1.contains("--permission-mode acceptEdits"), "{}", fourth.1);
        let more = More {
            harness: Some("claude"),
            model: Some("claude-sonnet-5"),
            effort: Some("low"),
            permission_mode: Some("acceptEdits"),
            ..Default::default()
        };
        let (one, _) = send(&h, &ws, "one", more, None).await;
        assert_eq!(pid_of(one.as_deref()), Some(fourth.0));
        h.fx.rt.close().await.expect("a close");
    });
}

// "a send at other picks than the one standing boots cold and ends it, rather than run on the wrong model"
#[test]
fn a_send_at_other_picks_than_the_one_standing_boots_cold_and_ends_it_rather_than_run_on_the_wrong_model() {
    drive(|| async {
        let h = host(None);
        let ws = mac(&h, "mac").await;
        warm(&h, &ws, warm_ask(Some("claude-sonnet-5"), None, None, None), None).await;
        let warmed = up_count(&h, 1).await[0].0;
        // The send names no model, so it opens on the default, which is not the model the process was launched at.
        let (one, _) = send(&h, &ws, "one", claude(), None).await;
        assert_ne!(pid_of(one.as_deref()), Some(warmed));
        assert!(ups(&h.heard)[1].1.contains("--model claude-opus-5-5"));
        gone(warmed).await;
        h.fx.rt.close().await.expect("a close");
    });
}

// "a send that leaves out a pick the one standing was launched with boots cold too, so fast never rides unasked"
#[test]
fn a_send_that_leaves_out_a_pick_the_one_standing_was_launched_with_boots_cold_too_so_fast_never_rides_unasked() {
    drive(|| async {
        let h = host(None);
        let ws = mac(&h, "mac").await;
        warm(&h, &ws, warm_ask(None, None, None, Some(true)), None).await;
        let warmed = up_count(&h, 1).await[0].clone();
        assert!(warmed.1.contains("\"fastMode\":true"), "{}", warmed.1);
        let (one, _) = send(&h, &ws, "one", claude(), None).await;
        assert_ne!(pid_of(one.as_deref()), Some(warmed.0));
        assert!(!ups(&h.heard)[1].1.contains("fastMode"));
        gone(warmed.0).await;
        h.fx.rt.close().await.expect("a close");
    });
}

// "a send that takes it and is refused before its launch ends it, so none stands claimed beside the next"
#[test]
fn a_send_that_takes_it_and_is_refused_before_its_launch_ends_it_so_none_stands_claimed_beside_the_next() {
    drive(|| async {
        let h = host(None);
        let ws = mac(&h, "mac").await;
        plain_warm(&h, &ws).await;
        let warmed = up_count(&h, 1).await[0].0;
        let asked = StartOptions { harness: Some("claude".into()), notify: Some(vec!["no-such-thread".into()]), ..prompt("one") };
        let e = match h.fx.rt.sessions().start(&ws, asked, None).await {
            Ok(_) => panic!("the send was not refused"),
            Err(e) => e,
        };
        assert!(e.message.contains("no thread no-such-thread to notify"), "{}", e.message);
        gone(warmed).await;
        assert_eq!(plain_warm(&h, &ws).await, json!({ "warm": "started" }));
        up_count(&h, 2).await;
        h.fx.rt.close().await.expect("a close");
    });
}

// "a thread's own start neither starts one nor takes the person's"
#[test]
fn a_threads_own_start_neither_starts_one_nor_takes_the_persons() {
    drive(|| async {
        let h = host(None);
        let ws = mac(&h, "mac").await;
        let (_, lead) = send(&h, &ws, "lead", claude(), None).await;
        let lead_caller = as_lead(&ws, &lead);
        assert_eq!(warm(&h, &ws, warm_ask(None, None, None, None), Some(lead_caller.clone())).await, json!({ "warm": "none" }));
        assert_eq!(ups(&h.heard).len(), 1);
        plain_warm(&h, &ws).await;
        let warmed = up_count(&h, 2).await[1].0;
        let (child, _) = send(&h, &ws, "child", claude(), Some(lead_caller)).await;
        assert_ne!(pid_of(child.as_deref()), Some(warmed));
        assert!(alive(warmed));
        let (mine, _) = send(&h, &ws, "mine", claude(), None).await;
        assert_eq!(pid_of(mine.as_deref()), Some(warmed));
        h.fx.rt.close().await.expect("a close");
    });
}

// "a fork neither takes the one standing nor ends it: that process holds no conversation, and a fork resumes its source's"
#[test]
fn a_fork_neither_takes_the_one_standing_nor_ends_it_that_process_holds_no_conversation_and_a_fork_resumes_its_sources() {
    drive(|| async {
        let h = host(None);
        let ws = mac(&h, "mac").await;
        let (_, source) = send(&h, &ws, "source", claude(), None).await;
        let rows = h.fx.rt.sessions().list(Some(ws.clone()), None).await.expect("a list");
        let row = rows.iter().find(|r| r.thread_id.as_deref() == Some(source.as_str())).expect("the row");
        let session = j(row)["claudeSessionId"].as_str().expect("a session").to_string();
        // The stand-in writes no session file, so the source's one turn is written here as the CLI files it.
        let opener = "0000aaaa-0000-4000-8000-000000000001";
        let lines = [
            json!({ "type": "user", "uuid": opener, "parentUuid": null, "sessionId": session, "message": { "role": "user", "content": "source" } }),
            json!({ "type": "assistant", "uuid": "a1", "parentUuid": opener, "sessionId": session, "message": { "id": "msg1", "role": "assistant", "content": [{ "type": "text", "text": "said" }] } }),
        ];
        let folder = h.root.path().join(".claude/projects/-acme");
        std::fs::create_dir_all(&folder).expect("a folder");
        let body: String = lines.iter().map(|l| format!("{l}\n")).collect();
        std::fs::write(folder.join(format!("{session}.jsonl")), body).expect("a session file");
        plain_warm(&h, &ws).await;
        let warmed = up_count(&h, 2).await[1].0;
        // The stand-in announces another model than it was launched on, so the fork names the one the source ran.
        let forked =
            h.fx.rt
                .sessions()
                .start(
                    &ws,
                    StartOptions {
                        model: Some("claude-opus-5-5".into()),
                        fork: Some(ForkSource { thread_id: source.clone(), turn_id: None, at: None }),
                        ..prompt("forked")
                    },
                    None,
                )
                .await
                .expect("a start");
        let result = forked.finished().await;
        assert_eq!(j(&result)["status"], "completed");
        assert_ne!(pid_of(result.text.as_deref()), Some(warmed));
        let own = up_count(&h, 3).await[2].1.clone();
        let resumed = own.find("--resume ").map(|at| &own[at + 9..]).unwrap_or("");
        assert!(resumed.len() >= 36 && is_session_id(&resumed[..36]), "{own}");
        assert!(!own.contains(&session), "{own}");
        assert!(alive(warmed));
        let (mine, _) = send(&h, &ws, "mine", claude(), None).await;
        assert_eq!(pid_of(mine.as_deref()), Some(warmed));
        h.fx.rt.close().await.expect("a close");
    });
}

// "a start on a copy of an outside conversation launches its own copy, never the fresh one standing"
#[test]
fn a_start_on_a_copy_of_an_outside_conversation_launches_its_own_copy_never_the_fresh_one_standing() {
    drive(|| async {
        let h = host(None);
        let ws = mac(&h, "mac").await;
        plain_warm(&h, &ws).await;
        let warmed = up_count(&h, 1).await[0].0;
        let outside = OutsideOpening {
            harness: "claude".into(),
            id: "7414323d-e71b-4957-8b56-eefdf6bfa350".into(),
            copy: true,
            project: "lab".into(),
            cwd: None,
            title: "lab codewords".into(),
            earlier: vec![],
        };
        let id = outside.id.clone();
        let handle =
            h.fx.rt
                .sessions()
                .start(&ws, StartOptions { harness: Some("claude".into()), outside: Some(outside), ..prompt("one") }, None)
                .await
                .expect("a start");
        let result = handle.finished().await;
        assert_ne!(pid_of(result.text.as_deref()), Some(warmed));
        let second = ups(&h.heard)[1].1.clone();
        assert!(second.contains(&format!("--resume {id} --fork-session")), "{second}");
        h.fx.rt.close().await.expect("a close");
    });
}

// "stands for the two composers asked for last across every project: warming a third ends the one asked for longest ago"
#[test]
fn stands_for_the_two_composers_asked_for_last_across_every_project_warming_a_third_ends_the_one_asked_for_longest_ago() {
    drive(|| async {
        let h = host(None);
        let mut folders = vec![];
        for n in 0..4 {
            folders.push(mac(&h, &format!("folder{n}")).await);
        }
        let warm_in = |n: usize, count: usize| {
            let (h, folders) = (&h, &folders);
            async move {
                plain_warm(h, &folders[n]).await;
                up_count(h, count).await.last().expect("a process").0
            }
        };
        let first = warm_in(0, 1).await;
        let second = warm_in(1, 2).await;
        let third = warm_in(2, 3).await;
        gone(first).await;
        assert!([second, third].iter().all(|p| alive(*p)));
        // Asked for again, the second is the more recent of the two, so the fourth ends the third.
        assert_eq!(plain_warm(&h, &folders[1]).await, json!({ "warm": "standing" }));
        let fourth = warm_in(3, 4).await;
        gone(third).await;
        assert!([second, fourth].iter().all(|p| alive(*p)));
        // The first folder's send finds nothing standing for it and boots cold, the two standing left as they are.
        let (one, _) = send(&h, &folders[0], "one", claude(), None).await;
        let pid = pid_of(one.as_deref()).expect("a pid");
        assert!(![first, second, fourth].contains(&pid));
        assert!([second, fourth].iter().all(|p| alive(*p)));
        h.fx.rt.close().await.expect("a close");
    });
}

// "five folders asked in turn leave two standing, and those count against the cap of ${AGENTS_KEPT}"
#[test]
fn five_folders_asked_in_turn_leave_two_standing_and_those_count_against_the_cap_of_6() {
    drive(|| async {
        let h = host(None);
        let mut folders = vec![];
        for n in 0..5 {
            folders.push(mac(&h, &format!("folder{n}")).await);
        }
        for (n, ws) in folders.iter().enumerate() {
            plain_warm(&h, ws).await;
            up_count(&h, n + 1).await;
        }
        let warmed = ups(&h.heard);
        for w in &warmed[..3] {
            gone(w.0).await;
        }
        assert!(warmed[3..].iter().all(|w| alive(w.0)));
        // Five threads' kept processes in a sixth folder on top of the two: the seventh process ends the one idle
        // longest, the older warm one.
        let sixth = mac(&h, "folder5").await;
        for word in ["a", "b", "c", "d", "e"] {
            send(&h, &sixth, word, claude(), None).await;
        }
        gone(warmed[3].0).await;
        assert!(alive(warmed[4].0));
        h.fx.rt.close().await.expect("a close");
    });
}
