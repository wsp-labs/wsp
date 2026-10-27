// SPDX-License-Identifier: AGPL-3.0-only
//! packages/runtime/test/thread-keeps-process.test.ts at 65f6b89c7, case for case. A thread on this computer keeps its
//! agent process between turns: the shipped Claude adapter over this computer's runs, driving an agent that serves turn
//! after turn from one process and names that process in every reply. Each test names the TS test above it.
mod common;

use std::path::PathBuf;
use std::sync::Arc;

use common::stubs::{alive, claude_keeper, codex_keeper, gone, pid_of, token_of};
use common::*;
use indexmap::IndexMap;
use serde_json::{json, Value};
use wsp_core::clock::FakeClock;
use wsp_core::machine_exec::ReadingSet;
use wsp_core::types::api::{DeleteThreadAsk, SettleAsk, StartOptions};
use wsp_core::types::wiring::AgentsReach;
use wsp_slates::slates::{SlateTarget, Slates, WriteParams};
use wsp_wire::types::place_word::HERE_PLACE_ID;
use wsp_wire::wire::limits::{AGENTS_KEPT, AGENT_KEEP_MS};
use wsp_words::words::thread::NO_SUCH_TURN;

#[derive(Default)]
struct Opts {
    reading: Option<Arc<ReadingSet>>,
    clock: Option<FakeClock>,
    env: Vec<(&'static str, String)>,
    dials: bool,
}

struct Host {
    root: tempfile::TempDir,
    gate: PathBuf,
    heard: PathBuf,
    run_dir: PathBuf,
    fx: Fx,
}

impl Drop for Host {
    /// A red run leaves nothing burning a core: every process the stand-ins said they started, which this test started.
    fn drop(&mut self) {
        let heard = std::fs::read_to_string(&self.heard).unwrap_or_default();
        for line in heard.lines() {
            let mut words = line.split(' ');
            if matches!(words.next(), Some("up" | "terminal" | "command" | "child")) {
                if let Some(pid) = words.next() {
                    let _ = std::process::Command::new("kill").args(["-KILL", pid]).output();
                }
            }
        }
    }
}

/// The wiring of one host: this computer's folder, the stand-ins on PATH, their gate and their log.
fn wired(root: &std::path::Path, o: &Opts) -> (wsp_core::types::wiring::LocalWiring, PathBuf, PathBuf) {
    let (gate, heard) = (root.join("gate"), root.join("heard"));
    let path = format!("{}:{}", root.join("bin").display(), std::env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin".into()));
    let mut vars = IndexMap::from([
        ("PATH".to_string(), path),
        ("STUB_GATE".to_string(), gate.display().to_string()),
        ("STUB_HEARD".to_string(), heard.display().to_string()),
    ]);
    for (k, v) in &o.env {
        vars.insert((*k).to_string(), v.clone());
    }
    let reading = o.reading.clone();
    let local = fake_local_with(root, Some(vars), move |e| {
        e.poll_ms = Some(20);
        if let Some(reading) = &reading {
            e.reading = Some(reading.clone());
        }
    });
    (local, gate, heard)
}

fn host_at(root: tempfile::TempDir, state: &State, o: Opts) -> Host {
    let (local, gate, heard) = wired(root.path(), &o);
    let adapters = wsp_core::adapters::harness_adapters();
    let mut run = runtime(&StubBackend::new(None))
        .store(&state.store)
        .adapter("claude", adapters["claude"].clone())
        .adapter("codex", adapters["codex"].clone())
        .with(move |opts| {
            opts.local = Some(local);
            if o.dials {
                opts.agents = Some(AgentsReach { here_url: Some("http://127.0.0.1:9".into()), wsp_mcp: None });
            }
        });
    if let Some(clock) = &o.clock {
        run = run.clock(clock);
    }
    let run_dir = root.path().join("runs");
    Host { root, gate, heard, run_dir, fx: run.build() }
}

/// A first host in a folder of its own: the stand-ins in `bin`.
fn start_host(state: &State, o: Opts) -> Host {
    let root = tempfile::tempdir().expect("a folder");
    std::fs::create_dir(root.path().join("bin")).expect("a folder");
    claude_keeper(&root.path().join("bin/claude"));
    codex_keeper(&root.path().join("bin/codex"));
    host_at(root, state, o)
}

/// Another host over the same folder and state, as a restart is.
fn again(first: &Host, state: &State, o: Opts) -> Host {
    let (local, gate, heard) = wired(first.root.path(), &o);
    let adapters = wsp_core::adapters::harness_adapters();
    let mut run = runtime(&StubBackend::new(None))
        .store(&state.store)
        .adapter("claude", adapters["claude"].clone())
        .adapter("codex", adapters["codex"].clone())
        .with(move |opts| {
            opts.local = Some(local);
            if o.dials {
                opts.agents = Some(AgentsReach { here_url: Some("http://127.0.0.1:9".into()), wsp_mcp: None });
            }
        });
    if let Some(clock) = &o.clock {
        run = run.clock(clock);
    }
    let _ = std::fs::create_dir_all(first.root.path().join("bin"));
    Host { root: tempfile::tempdir().expect("a folder kept for its drop"), gate, heard, run_dir: first.run_dir.clone(), fx: run.build() }
}

fn simple() -> (State, Host) {
    let state = sqlite_state();
    let h = start_host(&state, Opts::default());
    (state, h)
}

fn start(prompt: &str) -> StartOptions {
    StartOptions { prompt: prompt.into(), ..Default::default() }
}

/// One send into a thread, run to its reply.
async fn send(
    h: &Host,
    ws: &str,
    text: &str,
    thread: Option<&str>,
    model: Option<&str>,
    harness: Option<&str>,
) -> (wsp_adapters::port::TurnResult, String) {
    let o = StartOptions {
        thread: thread.map(str::to_string),
        model: model.map(str::to_string),
        harness: harness.map(str::to_string),
        ..start(text)
    };
    let handle = h.fx.rt.sessions().start(ws, o, None).await.expect("a start");
    let result = handle.finished().await;
    (result, handle.view().thread_id.expect("a thread"))
}

async fn mac(h: &Host) -> String {
    create_on(&h.fx.rt, CreateOn::named("mac").on(HERE_PLACE_ID), None).await.expect("a workspace").view.id
}

fn heard_text(h: &Host) -> String {
    std::fs::read_to_string(&h.heard).unwrap_or_default()
}

/// The commands the Codex stub left running as background terminals, by the pids it said.
fn terminals_of(h: &Host) -> Vec<u32> {
    heard_text(h).lines().filter_map(|l| l.strip_prefix("terminal ")).filter_map(|p| p.parse().ok()).collect()
}

fn group_alive(leader: i32) -> bool {
    std::process::Command::new("kill").args(["-0", "--", &format!("-{leader}")]).output().is_ok_and(|o| o.status.success())
}

async fn rows(h: &Host, ws: &str) -> Vec<Value> {
    h.fx.rt.sessions().list(Some(ws.to_string()), None).await.expect("a list").iter().map(j).collect()
}

async fn history(h: &Host, ws: &str) -> Vec<Value> {
    h.fx.rt.sessions().history(ws, None).await.expect("a history").iter().map(j).collect()
}

fn pid(text: &Option<String>) -> u32 {
    pid_of(text.as_deref()).expect("a pid in the reply")
}

// "a thread on this computer keeps its agent process between turns" > "answers the second send from the process its first turn left up, under the same turn token"
#[test]
fn answers_the_second_send_from_the_process_its_first_turn_left_up_under_the_same_turn_token() {
    drive(|| async {
        let (_state, h) = simple();
        let ws = mac(&h).await;
        let (one, thread) = send(&h, &ws, "one", None, None, None).await;
        assert_eq!(j(&one)["status"], "completed");
        let (two, _) = send(&h, &ws, "two", Some(&thread), None, None).await;
        assert!(two.text.as_deref().is_some_and(|t| t.contains("two from")));
        assert_eq!(pid(&two.text), pid(&one.text));
        assert_eq!(token_of(two.text.as_deref()), token_of(one.text.as_deref()));
        assert!(rows(&h, &ws).await.iter().filter(|s| s["threadId"] == thread.as_str()).all(|s| s["status"] == "completed"));
        h.fx.rt.close().await.expect("a close");
        gone(pid(&one.text)).await;
    });
}

// "a slate write from the kept process, by the token it still holds, lands on its own thread's slate"
#[test]
fn a_slate_write_from_the_kept_process_by_the_token_it_still_holds_lands_on_its_own_threads_slate() {
    drive(|| async {
        let (_state, h) = simple();
        let ws = mac(&h).await;
        let (one, thread) = send(&h, &ws, "one", None, None, None).await;
        let (two, _) = send(&h, &ws, "two", Some(&thread), None, None).await;
        assert_eq!(pid(&two.text), pid(&one.text));
        // Between turns too: the process is up and resting, and its token names its thread until the keep ends.
        let slates = h.fx.rt.slates();
        let wrote = slates
            .write(
                WriteParams {
                    target: SlateTarget { thread_id: None, turn_token: Some(token_of(two.text.as_deref())) },
                    text: Some("<slate title=\"Kept\"><column><text>still here</text></column></slate>".into()),
                    ..Default::default()
                },
                None,
            )
            .await
            .expect("a write");
        assert_eq!(wrote.version, 1);
        assert_eq!(slates.get(&thread).await.expect("a slate").map(|s| s.version), Some(1));
        h.fx.rt.close().await.expect("a close");
        gone(pid(&one.text)).await;
    });
}

// "answers a Codex thread's second send on the app server its first turn left up"
#[test]
fn answers_a_codex_threads_second_send_on_the_app_server_its_first_turn_left_up() {
    drive(|| async {
        let (_state, h) = simple();
        let ws = mac(&h).await;
        let (one, thread) = send(&h, &ws, "one", None, None, Some("codex")).await;
        assert_eq!(j(&one)["status"], "completed");
        let (two, _) = send(&h, &ws, "two", Some(&thread), None, None).await;
        assert!(two.text.as_deref().is_some_and(|t| t.contains("two from")));
        assert_eq!(pid(&two.text), pid(&one.text));
        let starts: Vec<Value> = history(&h, &ws)
            .await
            .into_iter()
            .filter(|e| e["type"] == "session.start" && e["threadId"] == thread.as_str())
            .map(|e| e["prompt"].clone())
            .collect();
        assert_eq!(starts, [json!("one"), json!("two")]);
        h.fx.rt.close().await.expect("a close");
        gone(pid(&one.text)).await;
    });
}

// "keeps every line a stopped turn wrote, its call and that call's result included, and the next turn's lines come after them"
#[test]
fn keeps_every_line_a_stopped_turn_wrote_its_call_and_that_calls_result_included_and_the_next_turns_lines_come_after_them() {
    drive(|| async {
        let (_state, h) = simple();
        let ws = mac(&h).await;
        let handle = h.fx.rt.sessions().start(&ws, start("hang talk"), None).await.expect("a start");
        let thread = handle.view().thread_id.expect("a thread");
        until_for(10_000, &mut || async {
            history(&h, &ws).await.iter().any(|e| e["threadId"] == thread.as_str() && e["type"] == "session.delta" && e["text"] == "beta")
        })
        .await;
        let stopped = h.fx.rt.sessions().interrupt(&handle.id(), None, None).await.expect("an interrupt");
        assert_eq!(j(&stopped)["outcome"], "accepted");
        send(&h, &ws, "two", Some(&thread), None, None).await;
        let rows: Vec<String> = history(&h, &ws)
            .await
            .into_iter()
            .filter(|e| e["threadId"] == thread.as_str() && e["type"] != "session.checkpoint")
            .map(|e| {
                if e["type"] == "session.delta" {
                    format!(
                        "{}:{}",
                        e["kind"].as_str().unwrap_or_default(),
                        e["text"].as_str().unwrap_or_default().split(" from ").next().unwrap_or_default()
                    )
                } else {
                    e["type"].as_str().unwrap_or_default().to_string()
                }
            })
            .collect();
        assert_eq!(
            rows,
            [
                "session.start",
                "text:alpha",
                "tool_use:{\"command\":\"echo one\"}",
                "tool_result:one",
                "text:beta",
                "session.done",
                "session.end",
                "session.start",
                "text:two",
                "session.done",
                "session.end"
            ]
        );
        h.fx.rt.close().await.expect("a close");
    });
}

// "ends the kept process when the model changes between turns, and the next turn boots cold"
#[test]
fn ends_the_kept_process_when_the_model_changes_between_turns_and_the_next_turn_boots_cold() {
    drive(|| async {
        let (_state, h) = simple();
        let ws = mac(&h).await;
        let (one, thread) = send(&h, &ws, "one", None, Some("claude-opus-5-5"), None).await;
        let (two, _) = send(&h, &ws, "two", Some(&thread), Some("claude-sonnet-5"), None).await;
        assert_eq!(j(&two)["status"], "completed");
        assert_ne!(pid(&two.text), pid(&one.text));
        gone(pid(&one.text)).await;
        h.fx.rt.close().await.expect("a close");
    });
}

// "reuses the kept process after the host itself wrote the thread's title into its session file"
#[test]
fn reuses_the_kept_process_after_the_host_itself_wrote_the_threads_title_into_its_session_file() {
    drive(|| async {
        let state = sqlite_state();
        let root = tempfile::tempdir().expect("a folder");
        std::fs::create_dir(root.path().join("bin")).expect("a folder");
        claude_keeper(&root.path().join("bin/claude"));
        codex_keeper(&root.path().join("bin/codex"));
        let sessions = root.path().join(".claude/projects/here");
        std::fs::create_dir_all(&sessions).expect("a folder");
        // The title lands after the turn's end, as the real one does seconds after a quick reply.
        let env = vec![
            ("STUB_SESSIONS", sessions.display().to_string()),
            ("STUB_TITLE", "Kept thread".to_string()),
            ("STUB_TITLE_MS", "1500".to_string()),
        ];
        let h = host_at(root, &state, Opts { env, ..Opts::default() });
        let ws = mac(&h).await;
        let (one, thread) = send(&h, &ws, "one", None, None, None).await;
        let row = rows(&h, &ws).await.into_iter().find(|s| s["threadId"] == thread.as_str()).expect("a row");
        let session = row["claudeSessionId"].as_str().expect("a session").to_string();
        until_for(10_000, &mut || async {
            std::fs::read_to_string(sessions.join(format!("{session}.jsonl"))).is_ok_and(|s| s.contains("custom-title"))
        })
        .await;
        let (two, _) = send(&h, &ws, "two", Some(&thread), None, None).await;
        assert_eq!(pid(&two.text), pid(&one.text));
        // And after a rename the person makes.
        let id = row["id"].as_str().expect("an id").to_string();
        let renamed = h.fx.rt.sessions().rename(&id, "Renamed", None).await.expect("a rename");
        assert_matches(&j(&renamed), json!({ "outcome": "renamed" }));
        let (three, _) = send(&h, &ws, "three", Some(&thread), None, None).await;
        assert_eq!(pid(&three.text), pid(&one.text));
        h.fx.rt.close().await.expect("a close");
    });
}

// "ends the kept process when the session file changed since its last turn, since the person resumed it elsewhere"
#[test]
fn ends_the_kept_process_when_the_session_file_changed_since_its_last_turn_since_the_person_resumed_it_elsewhere() {
    drive(|| async {
        let (_state, h) = simple();
        let ws = mac(&h).await;
        let (one, thread) = send(&h, &ws, "one", None, None, None).await;
        let row = rows(&h, &ws).await.into_iter().find(|s| s["threadId"] == thread.as_str()).expect("a row");
        let session = row["claudeSessionId"].as_str().expect("a session").to_string();
        let elsewhere = h.root.path().join(".claude/projects/elsewhere");
        std::fs::create_dir_all(&elsewhere).expect("a folder");
        std::fs::write(elsewhere.join(format!("{session}.jsonl")), "{\"type\":\"user\"}\n").expect("a file");
        let (two, _) = send(&h, &ws, "two", Some(&thread), None, None).await;
        assert_ne!(pid(&two.text), pid(&one.text));
        gone(pid(&one.text)).await;
        // Nothing changed it since, so the next send is the cold turn's own process.
        let (three, _) = send(&h, &ws, "three", Some(&thread), None, None).await;
        assert_eq!(pid(&three.text), pid(&two.text));
        h.fx.rt.close().await.expect("a close");
    });
}

// "ends a kept process after its idle keep with no turn, and its token then names nobody"
#[test]
fn ends_a_kept_process_after_its_idle_keep_with_no_turn_and_its_token_then_names_nobody() {
    drive(|| async {
        let fc = FakeClock::at_now();
        let state = sqlite_state();
        let h = start_host(&state, Opts { clock: Some(fc.clone()), ..Opts::default() });
        let ws = mac(&h).await;
        let (one, thread) = send(&h, &ws, "one", None, None, None).await;
        let token = token_of(one.text.as_deref());
        assert!(token.len() == 32 && token.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()), "{token}");
        // Between turns the token still names its thread: the process holding it is the thread's.
        let asked = StartOptions { notify: Some(vec!["me".into()]), turn_token: Some(token.clone()), ..start("kid") };
        let kid = h.fx.rt.sessions().start(&ws, asked, None).await.expect("a start");
        kid.finished().await;
        until_for(20_000, &mut || async {
            history(&h, &ws).await.iter().filter(|e| e["type"] == "session.done" && e["threadId"] == thread.as_str()).count() == 2
        })
        .await;
        assert!(alive(pid(&one.text)));
        fc.advance(AGENT_KEEP_MS - 1_000);
        assert!(alive(pid(&one.text)));
        fc.advance(1_000);
        gone(pid(&one.text)).await;
        let late = StartOptions { notify: Some(vec!["me".into()]), turn_token: Some(token), ..start("late") };
        let said = match h.fx.rt.sessions().start(&ws, late, None).await {
            Ok(_) => panic!("a late send was not refused"),
            Err(e) => e.message,
        };
        assert!(said.contains(NO_SUCH_TURN), "{said}");
        h.fx.rt.close().await.expect("a close");
    });
}

// "keeps at most ${AGENTS_KEPT} on this computer, the one idle longest ended first"
#[test]
fn keeps_at_most_6_on_this_computer_the_one_idle_longest_ended_first() {
    drive(|| async {
        let (_state, h) = simple();
        let ws = mac(&h).await;
        let mut pids = vec![];
        for n in 0..=AGENTS_KEPT {
            pids.push(pid(&send(&h, &ws, &format!("thread {n}"), None, None, None).await.0.text));
        }
        gone(pids[0]).await;
        assert!(pids[1..].iter().all(|p| alive(*p)));
        h.fx.rt.close().await.expect("a close");
    });
}

// "keeps a Codex thread and its server past the keep while background commands its turn left run, and lets both go once the last ends"
#[test]
fn keeps_a_codex_thread_and_its_server_past_the_keep_while_background_commands_its_turn_left_run_and_lets_both_go_once_the_last_ends() {
    drive(|| async {
        let fc = FakeClock::at_now();
        let state = sqlite_state();
        let h = start_host(&state, Opts { clock: Some(fc.clone()), env: vec![("STUB_UNLOAD_MS", "200".into())], ..Opts::default() });
        let ws = mac(&h).await;
        let (one, thread) = send(&h, &ws, "background 2", None, None, Some("codex")).await;
        assert_eq!(j(&one)["status"], "completed");
        let server = pid(&one.text);
        let terminals = terminals_of(&h);
        let (first, second) = (terminals[0], terminals[1]);
        let outcome = async {
            // Past the whole keep, and past the unload a thread let go of gets.
            fc.advance(AGENT_KEEP_MS);
            sleep_ms(600).await;
            assert!([first, second, server].iter().all(|p| alive(*p)));
            // One ends between turns and the server writes that into the thread's session file, which is no other writer's.
            let _ = std::process::Command::new("kill").args(["-TERM", &first.to_string()]).output();
            until_for(10_000, &mut || async { heard_text(&h).contains(&format!("ended {first}")) }).await;
            let (two, _) = send(&h, &ws, "two", Some(&thread), None, None).await;
            assert_eq!(pid(&two.text), server);
            sleep_ms(600).await;
            assert!(alive(second));
            assert!(!heard_text(&h).contains("unsubscribe"));
            // The last ends between turns: the thread is let go, and the keep runs out as it does for any idle server.
            let _ = std::process::Command::new("kill").args(["-TERM", &second.to_string()]).output();
            until_for(10_000, &mut || async { heard_text(&h).contains("unsubscribe") }).await;
            fc.advance(AGENT_KEEP_MS);
            gone(server).await;
        };
        outcome.await;
        h.fx.rt.close().await.expect("a close");
    });
}

// "lets go of a Codex thread whose command ends in the same read as its turn's end, and the keep then runs out"
#[test]
fn lets_go_of_a_codex_thread_whose_command_ends_in_the_same_read_as_its_turns_end_and_the_keep_then_runs_out() {
    drive(|| async {
        let fc = FakeClock::at_now();
        let state = sqlite_state();
        let h = start_host(&state, Opts { clock: Some(fc.clone()), ..Opts::default() });
        let ws = mac(&h).await;
        let (one, _) = send(&h, &ws, "raced", None, None, Some("codex")).await;
        assert_eq!(j(&one)["status"], "completed");
        until_for(5_000, &mut || async { heard_text(&h).contains("unsubscribe") }).await;
        fc.advance(AGENT_KEEP_MS);
        gone(pid(&one.text)).await;
        h.fx.rt.close().await.expect("a close");
    });
}

// "ends the idle one at the cap of ${AGENTS_KEPT}, never a server a background command keeps"
#[test]
fn ends_the_idle_one_at_the_cap_of_6_never_a_server_a_background_command_keeps() {
    drive(|| async {
        let (_state, h) = simple();
        let ws = mac(&h).await;
        let server = pid(&send(&h, &ws, "background 1", None, None, Some("codex")).await.0.text);
        let terminal = terminals_of(&h)[0];
        let mut pids = vec![];
        for n in 0..AGENTS_KEPT {
            pids.push(pid(&send(&h, &ws, &format!("thread {n}"), None, None, None).await.0.text));
        }
        gone(pids[0]).await;
        let mut kept = vec![server, terminal];
        kept.extend(&pids[1..]);
        assert!(kept.iter().all(|p| alive(*p)));
        let _ = std::process::Command::new("kill").args(["-KILL", &terminal.to_string()]).output();
        h.fx.rt.close().await.expect("a close");
    });
}

// "ends the kept process when its thread is settled, and when it is deleted"
#[test]
fn ends_the_kept_process_when_its_thread_is_settled_and_when_it_is_deleted() {
    drive(|| async {
        let (_state, h) = simple();
        let ws = mac(&h).await;
        let (settled, settled_thread) = send(&h, &ws, "settle me", None, None, None).await;
        let (deleted, deleted_thread) = send(&h, &ws, "delete me", None, None, None).await;
        h.fx.rt.sessions().settle(vec![settled_thread], None, SettleAsk::default()).await.expect("a settle");
        gone(pid(&settled.text)).await;
        assert!(alive(pid(&deleted.text)));
        h.fx.rt.sessions().delete(&deleted_thread, None, DeleteThreadAsk::default()).await.expect("a delete");
        gone(pid(&deleted.text)).await;
        h.fx.rt.close().await.expect("a close");
    });
}

// "a host that goes with an agent kept" > "ends the kept agent's whole tree, its input pump included, though the agent is slow to exit on its input's end"
#[test]
fn ends_the_kept_agents_whole_tree_its_input_pump_included_though_the_agent_is_slow_to_exit_on_its_inputs_end() {
    drive(|| async {
        let state = sqlite_state();
        let h = start_host(&state, Opts { env: vec![("STUB_EXIT_MS", "8000".into())], ..Opts::default() });
        let ws = mac(&h).await;
        send(&h, &ws, "one", None, None, None).await;
        let leaders: Vec<i32> = std::fs::read_dir(&h.run_dir)
            .expect("the run folder")
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().ends_with(".pid"))
            .filter_map(|e| std::fs::read_to_string(e.path()).ok()?.trim().parse().ok())
            .collect();
        assert_eq!(leaders.len(), 1);
        assert!(group_alive(leaders[0]));
        h.fx.rt.close().await.expect("a close");
        until_for(5_000, &mut || async { !group_alive(leaders[0]) }).await;
    });
}

// "a host that crashed with an agent kept" > "leaves the next host to take away the device the kept process held, since the sweep ends that process"
#[test]
fn leaves_the_next_host_to_take_away_the_device_the_kept_process_held_since_the_sweep_ends_that_process() {
    drive(|| async {
        let state = sqlite_state();
        let reading = ReadingSet::new();
        let rt1 = start_host(&state, Opts { reading: Some(reading.clone()), dials: true, ..Opts::default() });
        let ws = mac(&rt1).await;
        send(&rt1, &ws, "one", None, None, None).await;
        let minted: Vec<String> = rt1
            .fx
            .rt
            .devices()
            .list()
            .await
            .expect("a list")
            .iter()
            .map(j)
            .filter(|d| d["name"].as_str().is_some_and(|n| n.starts_with("thread ")))
            .map(|d| d["id"].as_str().unwrap_or_default().to_string())
            .collect();
        assert_eq!(minted.len(), 1);
        // A crash runs no close: the first host's readers simply stop.
        reading.let_go_all();
        let rt2 = again(&rt1, &state, Opts { dials: true, ..Opts::default() });
        let _ = rt2.fx.rt.sessions().list(Some(ws.clone()), None).await;
        until_for(5_000, &mut || async {
            !rt2.fx.rt.devices().list().await.expect("a list").iter().any(|d| j(d)["id"] == minted[0].as_str())
        })
        .await;
        rt2.fx.rt.close().await.expect("a close");
        rt1.fx.rt.close().await.expect("a close");
    });
}

// "a later turn re-opened by the next host" > "writes its missing start row with the message typed for that turn, not the thread's first"
#[test]
fn writes_its_missing_start_row_with_the_message_typed_for_that_turn_not_the_threads_first() {
    drive(|| async {
        let state = sqlite_state();
        let reading = ReadingSet::new();
        let rt1 = start_host(&state, Opts { reading: Some(reading.clone()), ..Opts::default() });
        let ws = mac(&rt1).await;
        let (_, thread) = send(&rt1, &ws, "the first message", None, None, None).await;
        rt1.fx
            .rt
            .sessions()
            .start(&ws, StartOptions { thread: Some(thread.clone()), ..start("the second message later") }, None)
            .await
            .expect("a start");
        // The agent holds the message and has said nothing for it when the host goes.
        until_for(10_000, &mut || async { heard_text(&rt1).contains("the second message later") }).await;
        rt1.fx.rt.close().await.expect("a close");
        reading.let_go_all();
        std::fs::write(&rt1.gate, "go\n").expect("a gate");

        let rt2 = again(&rt1, &state, Opts::default());
        until_for(20_000, &mut || async { rows(&rt2, &ws).await.iter().all(|s| s["status"] != "running") }).await;
        let all = history(&rt2, &ws).await;
        let starts: Vec<&Value> =
            all.iter().filter(|e| e["type"] == "session.start" && e["threadId"] == thread.as_str()).map(|e| &e["prompt"]).collect();
        assert_eq!(starts, [&json!("the first message"), &json!("the second message later")]);
        let done = all.iter().filter(|e| e["type"] == "session.done" && e["threadId"] == thread.as_str()).next_back().expect("a done");
        assert!(done["result"]["text"].as_str().is_some_and(|t| t.contains("the second message later from")), "{done}");
        rt2.fx.rt.close().await.expect("a close");
    });
}
