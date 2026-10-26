// SPDX-License-Identifier: AGPL-3.0-only
//! spec/sub_life_timers.md: a sub-thread's machine from its done to its delete, ported from
//! packages/runtime/test/sub-lifecycle.test.ts and the done halves of sub-results, sub-resume and sub-disk-keep.
mod machines_b_support;

use std::rc::Rc;
use std::sync::Arc;

use serde_json::{json, Value};

use machines_b_support::*;
use wsp_core::types::internal::GONE_REASON;
use wsp_core::types::wiring::SubRecord;
use wsp_machines::machines::machines::Entry;
use wsp_machines::machines::sub_life::*;
use wsp_wire::types::js_number::JsNumber;
use wsp_wire::views::workspace::Caller;
use wsp_wire::views::workspace_more::WorkspaceView;
use wsp_words::exit::HostError;

const WS: &str = "ws_kid";
const THREAD: &str = "t_kid";

/// A sub-thread on Boat whose first turn ran and ended (sub-lifecycle.test.ts `doneOnce`, up to the done): its record
/// live on a machine answering the result read with `read` and the session pack with `session`.
struct Held {
    host: Rc<FakeHost>,
    area: Rc<SubLifeArea>,
    entry: Entry,
    machine: Arc<FakeMachine>,
}

fn held(read: Reply, session: Option<Vec<u8>>) -> Held {
    let host = FakeHost::new();
    let machine = FakeMachine::new("bx_1");
    answering(&machine, read, session, THREAD);
    let entry = host.put_live(sub_record(WS, "running"), machine.clone());
    host.thread_records
        .borrow_mut()
        .push((THREAD.to_string(), WS.to_string()));
    let started = host.clock.now() - 60_000.0;
    host.put_row("turn-1", WS, THREAD, started, Some("s2"));
    let area = SubLifeArea::new(host.clone());
    Held {
        host,
        area,
        entry,
        machine,
    }
}

fn sub(h: &Held) -> SubRecord {
    h.entry.borrow().record.borrow().sub.clone().unwrap()
}

fn phase(h: &Held) -> String {
    serde_json::to_value(h.entry.borrow().record.borrow().phase)
        .unwrap()
        .as_str()
        .unwrap()
        .to_string()
}

fn stored_sub(h: &Held) -> Value {
    h.host.stored(WS).expect("the record was persisted")["sub"].clone()
}

#[test]
fn reads_the_result_off_local_refs_keeps_the_session_arms_the_delete_and_naps() {
    run(async {
        let h = held(ok(PUSHED), Some(b"gzipped session bytes".to_vec()));
        h.area.sub_done(WS).await;
        let s = sub(&h);
        let result = s.result.clone().expect("the done read the machine");
        assert_eq!(result.pushed_sha.as_deref(), Some(PUSHED_SHA));
        assert_eq!(
            (
                result.commits.get(),
                result.unpushed.get(),
                result.changed.get()
            ),
            (3, 0, 0)
        );
        assert_eq!(result.on_branch.as_deref(), Some(BRANCH));
        assert!(h
            .machine
            .log()
            .contains(&sub_result_command("/root/web", BRANCH, BASE_SHA)));
        assert_eq!(s.held, None);
        // L4: armed at the default 30 minutes from now.
        assert_eq!(s.delete_at, Some(JsNumber(h.host.clock.now() + 30.0 * MIN)));
        // The one session file, packed under /tmp, read off and kept by thread, then the pack removed.
        let pack = h
            .machine
            .log()
            .into_iter()
            .find(|c| c.contains("gzip -c"))
            .unwrap();
        assert!(pack
            .contains("find '/root/.claude-cfg/projects' -maxdepth 2 -type f -name '*s2.jsonl'"));
        assert!(pack.contains(&format!("gzip -c \"$f\" > '/tmp/wsp-session-{THREAD}.gz'")));
        assert!(h
            .machine
            .log()
            .contains(&format!("rm -f '/tmp/wsp-session-{THREAD}.gz'")));
        let session = s.session.clone().unwrap();
        assert_eq!(
            (session.agent.as_str(), session.id.as_str(), session.bytes),
            ("claude", "s2", Some(JsNumber(21.0)))
        );
        assert_eq!(session.kept, Some(JsNumber(h.host.clock.now())));
        assert_eq!(
            h.host.blob(SUB_SESSIONS, THREAD).as_deref(),
            Some(&b"gzipped session bytes"[..])
        );
        // Stored with all of it, the row told, then the nap asked with the done's reason.
        assert_eq!(
            stored_sub(&h)["deleteAt"],
            json!((h.host.clock.now() + 30.0 * MIN) as i64)
        );
        assert_eq!(h.host.calls_of("push_head"), vec![vec![THREAD.to_string()]]);
        assert_eq!(
            h.host.calls_of("nap_with"),
            vec![vec![WS.to_string(), "its sub-thread is done".to_string()]]
        );
        assert_eq!(phase(&h), "napping");
    });
}

#[test]
fn arms_the_delete_at_the_delay_the_person_set_read_at_the_done() {
    run(async {
        let h = held(ok(PUSHED), None);
        h.host
            .settings_delete_after
            .borrow_mut()
            .insert("box".to_string(), 2.0 * MIN);
        h.area.sub_done(WS).await;
        assert_eq!(
            sub(&h).delete_at,
            Some(JsNumber(h.host.clock.now() + 2.0 * MIN))
        );
    });
}

#[test]
fn holds_the_machine_over_work_not_pushed_arms_no_delete_and_naps_all_the_same() {
    run(async {
        let h = held(ok(UNPUSHED), None);
        h.area.sub_done(WS).await;
        let s = sub(&h);
        let kept = s.held.clone().expect("held over work not pushed");
        assert_eq!(
            (kept.held.files.get(), kept.held.commits.get(), kept.at),
            (3, 2, JsNumber(h.host.clock.now()))
        );
        assert_eq!(s.delete_at, None);
        let r = s.result.unwrap();
        assert_eq!(
            (r.pushed_sha, r.unpushed.get(), r.changed.get()),
            (None, 2, 3)
        );
        assert_eq!(
            stored_sub(&h)["held"],
            json!({ "files": 3, "commits": 2, "at": h.host.clock.now() as i64 })
        );
        assert_eq!(h.host.calls_of("nap_with").len(), 1);
        assert_eq!(phase(&h), "napping");
        // No timer stands for a held machine.
        assert_eq!(h.host.clock.pending(), 0);
    });
}

#[test]
fn treats_a_read_that_fails_as_work_not_pushed() {
    run(async {
        let h = held(exit(128, "fatal: not a git repository"), None);
        h.area.sub_done(WS).await;
        let s = sub(&h);
        assert_eq!(
            s.result.unwrap().read_failed.as_deref(),
            Some("exit 128: fatal: not a git repository")
        );
        let kept = s.held.unwrap();
        assert_eq!((kept.held.files.get(), kept.held.commits.get()), (0, 0));
        assert_eq!(s.delete_at, None);
    });
}

#[test]
fn arms_the_delete_for_a_sub_thread_that_never_fetched_the_code() {
    run(async {
        let h = held(ok("norepo\n"), None);
        h.area.sub_done(WS).await;
        let s = sub(&h);
        let r = s.result.unwrap();
        assert_eq!(
            (
                r.norepo.is_some(),
                r.unpushed.get(),
                r.changed.get(),
                r.pushed_sha
            ),
            (true, 0, 0, None)
        );
        assert!(s.delete_at.is_some());
        assert_eq!(s.held, None);
    });
}

/// Claude Code's push of the branch and what the remote answered, as a transcript holds it (sub-lifecycle.test.ts:24).
fn push_refused(output: &str) -> Vec<Value> {
    vec![
        json!({ "type": "session.delta", "kind": "tool_use", "toolName": "Bash", "toolUseId": "push-1", "text": json!({ "command": format!("git push -u origin {BRANCH}") }).to_string() }),
        json!({ "type": "session.delta", "kind": "tool_result", "toolName": "Bash", "toolUseId": "push-1", "isError": true, "exitCode": 128, "text": output }),
    ]
}

const DENIED: &str = "remote: Permission to acme/web.git denied to wsp-bot.\nfatal: unable to access 'https://github.com/acme/web.git/': The requested URL returned error: 403";

#[test]
fn names_why_the_push_was_refused_off_the_agents_own_push() {
    run(async {
        let h = held(ok(UNPUSHED), None);
        h.host
            .script("read_transcript", Ok(Value::Array(push_refused(DENIED))));
        h.area.sub_done(WS).await;
        assert_eq!(
            sub(&h).result.unwrap().push_refused.as_deref(),
            Some("Permission to acme/web.git denied to wsp-bot")
        );
        assert_eq!(
            h.host.calls_of("read_transcript"),
            vec![vec![
                WS.to_string(),
                THREAD.to_string(),
                "400".to_string(),
                "2000000".to_string()
            ]]
        );
    });
}

#[test]
fn names_no_cause_where_the_last_push_went_through_or_all_is_pushed() {
    run(async {
        let mut through = push_refused(DENIED);
        through.push(json!({ "type": "session.delta", "kind": "tool_use", "toolName": "Bash", "toolUseId": "push-2", "text": json!({ "command": "git push" }).to_string() }));
        through.push(json!({ "type": "session.delta", "kind": "tool_result", "toolName": "Bash", "toolUseId": "push-2", "text": "Everything up-to-date" }));
        let h = held(ok(UNPUSHED), None);
        h.host.script("read_transcript", Ok(Value::Array(through)));
        h.area.sub_done(WS).await;
        let r = sub(&h).result.unwrap();
        assert_eq!(r.push_refused, None);
        assert!(
            r.unpushed.get() > 0,
            "the read itself still says what is not pushed"
        );
        // All pushed: the transcript is not read at all.
        let pushed = held(ok(PUSHED), None);
        pushed
            .host
            .script("read_transcript", Ok(Value::Array(push_refused(DENIED))));
        pushed.area.sub_done(WS).await;
        assert_eq!(sub(&pushed).result.unwrap().push_refused, None);
        assert!(pushed.host.calls_of("read_transcript").is_empty());
    });
}

#[test]
fn leaves_a_machine_whose_nap_was_refused_running_with_the_backstop_at_delete_plus_10_minutes() {
    run(async {
        let h = held(ok(PUSHED), None);
        h.host.script(
            "nap_with",
            Err(failed(
                "the provider will not stop this machine while the snapshot of its disk fails",
            )),
        );
        h.area.sub_done(WS).await;
        assert_eq!(phase(&h), "running");
        let at = sub(&h).delete_at.unwrap();
        let backstops = h.host.backend.lifecycle.backstops.lock().unwrap().clone();
        assert_eq!(
            backstops,
            vec![("bx_1".to_string(), (at.0 + 10.0 * MIN) as i64, true)]
        );
        assert!(h
            .host
            .warns
            .borrow()
            .iter()
            .any(|w| w.starts_with("sub-thread machine bx_1 of ws_kid did not nap (")));
    });
}

#[test]
fn a_held_machine_whose_nap_failed_moves_no_backstop() {
    run(async {
        let h = held(ok(UNPUSHED), None);
        h.host.script("nap_with", Err(failed("refused")));
        h.area.sub_done(WS).await;
        assert!(h
            .host
            .backend
            .lifecycle
            .backstops
            .lock()
            .unwrap()
            .is_empty());
        assert!(sub(&h).held.is_some());
    });
}

/// The record as a done leaves it on a napped machine, armed at `deleteAt` (sub-lifecycle.test.ts `settled`).
async fn napped(read: Reply) -> Held {
    let h = held(read, Some(b"gzipped session bytes".to_vec()));
    h.area.sub_done(WS).await;
    h
}

#[test]
fn deletes_the_napped_box_with_no_wake_and_keeps_the_record_released() {
    run(async {
        let h = napped(ok(PUSHED)).await;
        let before = sub(&h);
        h.host.clock.advance(31.0 * MIN).await;
        settle().await;
        let now = h.host.live.borrow().get(WS).cloned().unwrap();
        let r = now.borrow().record.borrow().clone();
        assert_eq!(serde_json::to_value(r.phase).unwrap(), json!("released"));
        assert_eq!(r.machine_id, "");
        let s = r.sub.unwrap();
        assert_eq!(s.released_at, Some(before.delete_at.unwrap()));
        assert_eq!(s.delete_at, None);
        assert_eq!(s.result, before.result);
        assert_eq!(s.session, before.session);
        assert!(h.machine.killed());
        // The broker cut it off before the cloud was asked.
        let calls: Vec<String> = h
            .host
            .calls
            .borrow()
            .iter()
            .map(|(n, _)| n.clone())
            .collect();
        let cut = calls.iter().position(|n| n == "cut_off").unwrap();
        let unfork = calls.iter().position(|n| n == "unfork").unwrap();
        assert!(cut < unfork);
        // The release's order: sessions ended, marks forgotten, re-attached dead, persisted, released, status gone.
        assert_eq!(
            h.host.calls_of("end_sessions"),
            vec![vec![WS.to_string(), GONE_REASON.to_string()]]
        );
        assert_eq!(
            h.host.events_of("workspace.released"),
            vec![json!({ "type": "workspace.released", "workspaceId": WS })]
        );
        assert_eq!(
            h.host.calls_of("emit_status").last().unwrap(),
            &vec![WS.to_string(), "gone".to_string()]
        );
        assert_eq!(
            h.host.blob(SUB_SESSIONS, THREAD).as_deref(),
            Some(&b"gzipped session bytes"[..])
        );
        assert_eq!(h.host.stored(WS).unwrap()["phase"], json!("released"));
    });
}

#[test]
fn a_refused_delete_is_asked_again_ten_minutes_on_and_stays_cut_off() {
    run(async {
        let h = napped(ok(PUSHED)).await;
        *h.machine.kill_fails.lock().unwrap() = Some("Boat answered 503".to_string());
        h.host.clock.advance(31.0 * MIN).await;
        settle().await;
        let s = sub(&h);
        assert_ne!(phase(&h), "released");
        let again = s.delete_at.expect("the delete moved on");
        assert!(again.0 > h.host.clock.now());
        let said = h
            .entry
            .borrow()
            .delete_said
            .clone()
            .expect("the refusal is said on the entry");
        assert_eq!(
            said.line,
            wsp_words::sub_thread::sub_delete_refused_line(
                "Boat",
                "Fix the flaky upload test",
                "Boat answered 503",
                10.0
            )
        );
        assert_eq!(h.host.calls_of("broker_lift"), Vec::<Vec<String>>::new());
        assert_eq!(h.host.clock.pending(), 1, "the retry is armed");
    });
}

#[test]
fn is_cancelled_by_a_turn_that_starts_and_armed_again_at_its_done() {
    run(async {
        let h = napped(ok(PUSHED)).await;
        h.host.running.borrow_mut().insert(THREAD.to_string());
        h.host
            .emit_now(json!({ "type": "session.start", "workspaceId": WS, "threadId": THREAD }));
        assert_eq!(sub(&h).delete_at, None);
        assert_eq!(sub(&h).held, None);
        assert_eq!(h.host.calls_of("broker_lift"), vec![vec![WS.to_string()]]);
        assert_eq!(h.host.clock.pending(), 0);
        h.host.clock.advance(31.0 * MIN).await;
        assert!(!h.machine.killed());
        // The turn ends: its own done arms the delete again, from then.
        h.host.running.borrow_mut().shift_remove(THREAD);
        h.entry.borrow().record.borrow_mut().phase =
            serde_json::from_value(json!("running")).unwrap();
        h.host
            .put_row("turn-2", WS, THREAD, h.host.clock.now(), Some("s2"));
        h.host
            .emit_now(json!({ "type": "session.end", "workspaceId": WS, "threadId": THREAD }));
        settle().await;
        assert_eq!(
            sub(&h).delete_at,
            Some(JsNumber(h.host.clock.now() + 30.0 * MIN))
        );
    });
}

#[test]
fn turns_into_a_hold_where_the_machine_still_runs_and_its_read_finds_work() {
    run(async {
        let h = held(ok(PUSHED), None);
        h.host.script(
            "nap_with",
            Err(failed("the stop passed its 10 minute budget")),
        );
        h.area.sub_done(WS).await;
        assert!(sub(&h).delete_at.is_some());
        h.machine.answer(
            "git rev-list --count --branches --not --remotes",
            ok(UNPUSHED),
        );
        h.host.clock.advance(31.0 * MIN).await;
        settle().await;
        let s = sub(&h);
        assert_eq!(s.delete_at, None);
        assert_eq!(
            s.held.map(|x| (x.held.files.get(), x.held.commits.get())),
            Some((3, 2))
        );
        assert_eq!(phase(&h), "running");
        assert!(!h.machine.killed());
        assert!(
            h.host.calls_of("push_head").len() >= 2,
            "every window is told the hold"
        );
    });
}

#[test]
fn arm_subs_fires_a_delete_at_passed_while_the_host_was_down() {
    run(async {
        let host = FakeHost::new();
        let machine = FakeMachine::new("bx_1");
        let mut rec = sub_record(WS, "napping");
        rec["sub"]["deleteAt"] = json!(host.clock.now() - MIN);
        rec["sub"]["result"] = json!({ "branch": BRANCH, "pushedSha": PUSHED_SHA, "commits": 3, "unpushed": 0, "changed": 0, "readAt": "2026-10-12T06:00:00.000Z" });
        let entry = host.put_live(rec, machine.clone());
        host.thread_records
            .borrow_mut()
            .push((THREAD.to_string(), WS.to_string()));
        let area = SubLifeArea::new(host.clone());
        area.arm_subs();
        host.clock.advance(0.0).await;
        settle().await;
        assert!(machine.killed());
        let now = host.live.borrow().get(WS).cloned().unwrap();
        assert_eq!(
            serde_json::to_value(now.borrow().record.borrow().phase).unwrap(),
            json!("released")
        );
        assert_eq!(
            entry
                .borrow()
                .record
                .borrow()
                .sub
                .as_ref()
                .unwrap()
                .result
                .as_ref()
                .unwrap()
                .commits
                .get(),
            3
        );
    });
}

#[test]
fn arm_subs_arms_a_delete_at_still_ahead_and_fires_it_then() {
    run(async {
        let host = FakeHost::new();
        let machine = FakeMachine::new("bx_1");
        let mut rec = sub_record(WS, "napping");
        rec["sub"]["deleteAt"] = json!(host.clock.now() + 30.0 * MIN);
        host.put_live(rec, machine.clone());
        host.thread_records
            .borrow_mut()
            .push((THREAD.to_string(), WS.to_string()));
        let area = SubLifeArea::new(host.clone());
        area.arm_subs();
        assert_eq!(host.clock.due(), vec![host.clock.now() + 30.0 * MIN]);
        host.clock.advance(29.0 * MIN).await;
        assert!(!machine.killed());
        host.clock.advance(2.0 * MIN).await;
        settle().await;
        assert!(machine.killed());
    });
}

#[test]
fn arm_subs_runs_a_done_a_restart_cut_short_and_skips_cloud_gone_and_released() {
    run(async {
        let host = FakeHost::new();
        let cut = FakeMachine::new("bx_1");
        answering(&cut, ok(PUSHED), None, THREAD);
        host.put_live(sub_record(WS, "running"), cut.clone());
        host.thread_records
            .borrow_mut()
            .push((THREAD.to_string(), WS.to_string()));
        host.put_row("turn-1", WS, THREAD, host.clock.now() - MIN, Some("s2"));
        let mut gone = sub_record("ws_gone", "napping");
        gone["sub"]["cloudGone"] = json!(true);
        host.put_live(gone, FakeMachine::new("bx_2"));
        host.put_live(sub_record("ws_rel", "released"), FakeMachine::new("bx_3"));
        let area = SubLifeArea::new(host.clone());
        area.arm_subs();
        settle().await;
        let s = host
            .live
            .borrow()
            .get(WS)
            .unwrap()
            .borrow()
            .record
            .borrow()
            .sub
            .clone()
            .unwrap();
        assert!(s.delete_at.is_some(), "the done ran at boot");
        assert!(host
            .live
            .borrow()
            .get("ws_gone")
            .unwrap()
            .borrow()
            .record
            .borrow()
            .sub
            .as_ref()
            .unwrap()
            .result
            .is_none());
        assert_eq!(host.clock.pending(), 1);
    });
}

#[test]
fn keeps_the_dones_word_when_a_start_a_restart_failed_ends_on_the_napped_machine() {
    run(async {
        let h = napped(ok(PUSHED)).await;
        let before = sub(&h);
        // The read's time is now; the latest turn started before it. A done with the machine napping reads nothing.
        let ran = h.machine.log().len();
        h.area.sub_done(WS).await;
        assert_eq!(h.machine.log().len(), ran);
        assert_eq!(sub(&h).result, before.result);
        assert_eq!(sub(&h).delete_at, before.delete_at);
    });
}

#[test]
fn a_lost_box_reads_released_with_lost_and_no_gone() {
    run(async {
        let h = napped(ok(PUSHED)).await;
        h.entry.borrow().record.borrow_mut().gone = Some("machine gone".to_string());
        let out = h.area.sub_lost(&h.entry, "Boat lost bx_1").await.unwrap();
        assert_eq!(
            out,
            wsp_machines::machines::sub_life::timers::GoneOutcome::Settled
        );
        let r = h.entry.borrow().record.borrow().clone();
        assert_eq!(serde_json::to_value(r.phase).unwrap(), json!("released"));
        assert_eq!(r.sub.unwrap().lost.as_deref(), Some("Boat lost bx_1"));
        assert_eq!(r.gone, None);
    });
}

fn woken_view() -> WorkspaceView {
    serde_json::from_value(json!({ "id": WS, "name": "Fix the flaky upload test", "machineId": "bx_1", "phase": "running", "golden": "", "createdAt": "", "project": { "id": "p", "name": "p", "path": "", "computer": "" } })).expect("a workspace view parses")
}

fn lead() -> Caller {
    caller("t_lead", "ws_lead", "t_lead")
}

#[test]
fn refuses_a_leads_token_over_work_not_pushed_and_lets_the_person() {
    run(async {
        let h = napped(ok(UNPUSHED)).await;
        let e = h
            .area
            .sub_delete(&h.entry, THREAD, Some(&lead()), false)
            .await
            .unwrap_err();
        assert_eq!(e.message, "sub-thread t_kid holds 2 commits and 3 changed files not pushed on wsp/fix-the-flaky-upload-test. Send it a push, or ask the person to delete it.");
        assert!(h.host.calls_of("delete_workspace").is_empty());
        let done = h
            .area
            .sub_delete(&h.entry, THREAD, None, false)
            .await
            .unwrap();
        assert_eq!(
            text(&done),
            text(&json!({ "workspaceId": WS, "threads": 1, "machine": "deleted", "on": "Boat" }))
        );
        assert_eq!(
            h.host.calls_of("delete_workspace"),
            vec![vec![WS.to_string()]]
        );
    });
}

#[test]
fn lets_a_leads_token_delete_one_whose_work_is_all_pushed() {
    run(async {
        let h = napped(ok(PUSHED)).await;
        h.area
            .sub_delete(&h.entry, THREAD, Some(&lead()), false)
            .await
            .unwrap();
        assert_eq!(
            h.host.calls_of("delete_workspace"),
            vec![vec![WS.to_string()]]
        );
    });
}

#[test]
fn delete_check_answers_the_question_and_deletes_nothing() {
    run(async {
        let h = napped(ok(UNPUSHED)).await;
        let asked = h
            .area
            .sub_delete(&h.entry, THREAD, None, true)
            .await
            .unwrap();
        assert_eq!(asked.question.as_deref(), Some("Delete sub-thread t_kid (Fix the flaky upload test) and its machine on Boat?\n2 commits and 3 changed files on wsp/fix-the-flaky-upload-test are not pushed and go with it."));
        assert_eq!(
            (
                asked.workspace_id.as_str(),
                asked.threads,
                asked.machine.clone(),
                asked.on.as_str()
            ),
            (WS, 1, None, "Boat")
        );
        assert!(h.host.calls_of("delete_workspace").is_empty());
        assert!(!h.machine.killed());
    });
}

#[test]
fn a_delete_under_a_running_turn_is_refused_as_working() {
    run(async {
        let h = napped(ok(PUSHED)).await;
        h.host.running.borrow_mut().insert(THREAD.to_string());
        let e = h
            .area
            .sub_delete(&h.entry, THREAD, None, false)
            .await
            .unwrap_err();
        assert_eq!(e.kind, Some(wsp_words::exit::Kind::Conflict));
        assert!(h.host.calls_of("delete_workspace").is_empty());
    });
}

#[test]
fn refuses_a_leads_token_at_once_the_turn_not_read_yet() {
    run(async {
        let h = napped(ok(UNPUSHED)).await;
        // A follow-up turn started after the read: nothing has read its work.
        h.host.put_row(
            "turn-2",
            WS,
            THREAD,
            h.host.clock.now() + 1_000.0,
            Some("s2"),
        );
        h.entry.borrow().record.borrow_mut().phase =
            serde_json::from_value(json!("running")).unwrap();
        let unread = h.area.sub_unpushed(&h.entry).unwrap();
        assert_eq!(
            (unread.files, unread.commits, unread.unread),
            (0.0, 0.0, Some(true))
        );
        let e = h
            .area
            .sub_delete(&h.entry, THREAD, Some(&lead()), false)
            .await
            .unwrap_err();
        assert!(
            e.message.contains("has not been read since its last turn"),
            "{}",
            e.message
        );
    });
}

#[test]
fn waits_for_the_done_in_flight_then_refuses_over_the_work_it_found() {
    run(async {
        let h = held(ok(UNPUSHED), Some(b"s".to_vec()));
        let g = gate();
        h.machine.readable.lock().unwrap().insert(
            format!("/tmp/wsp-session-{THREAD}.gz"),
            Reply::Gated(g.clone(), Box::new(ok(""))),
        );
        let area = h.area.clone();
        tokio::task::spawn_local(async move { area.sub_done(WS).await });
        settle().await;
        let area = h.area.clone();
        let entry = h.entry.clone();
        let refused = tokio::task::spawn_local(async move {
            area.sub_delete(&entry, THREAD, Some(&lead()), false).await
        });
        settle().await;
        assert!(!refused.is_finished(), "the delete waits for the done");
        open(&g);
        let e = refused.await.unwrap().unwrap_err();
        assert!(
            e.message.contains("2 commits and 3 changed files"),
            "{}",
            e.message
        );
        assert!(!h.machine.killed());
    });
}

#[test]
fn removing_the_cloud_releases_every_sub_with_cloud_gone() {
    run(async {
        let h = napped(ok(PUSHED)).await;
        h.area.release_subs_on("box", false).await.unwrap();
        let now = h.host.live.borrow().get(WS).cloned().unwrap();
        let r = now.borrow().record.borrow().clone();
        assert_eq!(serde_json::to_value(r.phase).unwrap(), json!("released"));
        assert_eq!(r.sub.as_ref().unwrap().cloud_gone, Some(true));
        assert_eq!(h.host.clock.pending(), 0, "the delete timer went with it");
        let e = h.area.refuse_sub_send(&now).unwrap_err();
        assert_eq!(
            e.message,
            "Boat is no longer on this host. Add it with wsp add boat and send again."
        );
        assert_eq!(h.area.refuse_sub_wake(&now).unwrap_err().message, e.message);
    });
}

#[test]
fn a_released_record_only_gains_cloud_gone() {
    run(async {
        let h = napped(ok(PUSHED)).await;
        h.host.clock.advance(31.0 * MIN).await;
        settle().await;
        let released_at = sub(&h).released_at;
        let pushes = h.host.calls_of("push_head").len();
        h.area.release_subs_on("box", false).await.unwrap();
        assert_eq!(sub(&h).released_at, released_at);
        assert_eq!(sub(&h).cloud_gone, Some(true));
        assert_eq!(h.host.calls_of("push_head").len(), pushes + 1);
        assert_eq!(
            h.host.events_of("workspace.released").len(),
            1,
            "no second release"
        );
    });
}

#[test]
fn forgetting_the_cloud_keeps_the_phase_marks_cloud_gone_and_arms_nothing() {
    run(async {
        let h = napped(ok(PUSHED)).await;
        h.area.release_subs_on("box", true).await.unwrap();
        assert_eq!(phase(&h), "napping");
        assert_eq!(sub(&h).cloud_gone, Some(true));
        assert_eq!(sub(&h).delete_at, None);
        assert_eq!(h.host.clock.pending(), 0);
        let now = h.host.live.borrow().get(WS).cloned().unwrap();
        assert_eq!(
            now.borrow().machine.id(),
            "bx_1",
            "the dead machine keeps the record's id"
        );
        h.host.clock.advance(31.0 * MIN).await;
        assert!(!h.machine.killed());
    });
}

#[test]
fn waits_for_the_wake_and_reads_the_machine_again_before_it_deletes() {
    run(async {
        let h = napped(ok(PUSHED)).await;
        let g = gate();
        let waiting = g.clone();
        let wake: futures::future::LocalBoxFuture<'static, Result<WorkspaceView, HostError>> =
            Box::pin(async move {
                let _ = waiting.acquire().await;
                Ok(woken_view())
            });
        h.entry.borrow_mut().waking = Some(futures::FutureExt::shared(wake));
        h.machine.answer(
            "git rev-list --count --branches --not --remotes",
            ok(UNPUSHED),
        );
        h.host.clock.advance(31.0 * MIN).await;
        assert!(!h.machine.killed());
        h.entry.borrow().record.borrow_mut().phase =
            serde_json::from_value(json!("running")).unwrap();
        open(&g);
        settle().await;
        assert!(!h.machine.killed());
        assert!(sub(&h).held.is_some());
        assert_ne!(phase(&h), "released");
    });
}

#[test]
fn sub_read_waits_for_the_done_after_the_turn() {
    run(async {
        let h = held(ok(PUSHED), None);
        let since = h.host.clock.now();
        let area = h.area.clone();
        let read = tokio::task::spawn_local(async move { area.sub_read(WS, since).await });
        settle().await;
        assert!(!read.is_finished());
        h.area.sub_done(WS).await;
        let r = read.await.unwrap().expect("the read the done made");
        assert_eq!(r.commits.get(), 3);
    });
}

#[test]
fn sub_read_gives_up_after_five_minutes() {
    run(async {
        let h = held(ok(PUSHED), None);
        let since = h.host.clock.now();
        let area = h.area.clone();
        let read = tokio::task::spawn_local(async move { area.sub_read(WS, since).await });
        settle().await;
        h.host.clock.advance(SUB_LINE_WAIT_MS - 1.0).await;
        assert!(!read.is_finished());
        h.host.clock.advance(1.0).await;
        assert_eq!(read.await.unwrap(), None);
        assert_eq!(h.host.clock.pending(), 0);
    });
}

#[test]
fn sub_read_with_since_equal_to_read_at_answers_at_once() {
    run(async {
        let h = napped(ok(PUSHED)).await;
        // The read's readAt is the done's now to the millisecond; asked with exactly that, it answers without waiting.
        let since = h.host.clock.now();
        let got = h.area.sub_read(WS, since).await;
        assert_eq!(got, sub(&h).result);
        assert_eq!(
            h.host.clock.pending(),
            1,
            "only the delete timer; no read wait was armed"
        );
    });
}

#[test]
fn line_ends_with_the_branch_pushed_and_the_delete_in_a_span() {
    run(async {
        let h = napped(ok(PUSHED)).await;
        let line = h.area.sub_line(WS, h.host.clock.now()).await.unwrap();
        assert_eq!(line, "branch wsp/fix-the-flaky-upload-test pushed at 9c2e1d4, 3 commits on feat/upload; merge it with wsp merge in t_lead t_kid. Its machine naps now and is deleted in 30 minutes unless you send to t_kid first.");
    });
}

#[test]
fn line_ends_with_the_work_not_pushed_and_the_machine_kept() {
    run(async {
        let h = napped(ok(UNPUSHED)).await;
        let line = h.area.sub_line(WS, h.host.clock.now()).await.unwrap();
        assert_eq!(line, "2 commits and 3 changed files on wsp/fix-the-flaky-upload-test are not pushed, so its machine is kept, napping at no cost. Send to t_kid to have them pushed, or ask the person to delete it.");
    });
}

#[test]
fn line_names_the_day_the_cloud_may_delete_a_held_disk() {
    run(async {
        let h = held(ok(UNPUSHED), None);
        *h.host.backend.lifecycle.disk_kept_ms.lock().unwrap() = Some(14 * 24 * 60 * 60_000);
        h.area.sub_done(WS).await;
        let line = h.area.sub_line(WS, h.host.clock.now()).await.unwrap();
        assert!(
            line.contains("so its machine naps at no cost, and Boat may delete its disk from "),
            "{line}"
        );
    });
}

#[test]
fn serial_runs_a_done_and_a_delete_one_after_the_other_and_swallows_a_failure() {
    run(async {
        let h = held(ok(PUSHED), Some(b"s".to_vec()));
        let g = gate();
        h.machine.readable.lock().unwrap().insert(
            format!("/tmp/wsp-session-{THREAD}.gz"),
            Reply::Gated(g.clone(), Box::new(ok(""))),
        );
        let area = h.area.clone();
        let done = tokio::task::spawn_local(async move { area.sub_done(WS).await });
        settle().await;
        // The delete asked meanwhile waits for the done: deleteAt is not set yet, so it would do nothing if it ran now.
        let area = h.area.clone();
        let expire = tokio::task::spawn_local(async move { area.sub_expire(WS).await });
        settle().await;
        assert!(!done.is_finished() && !expire.is_finished());
        open(&g);
        done.await.unwrap();
        expire.await.unwrap();
        // The expire ran after the done had armed it, and deleted the box.
        assert!(h.machine.killed());
    });
}

#[test]
fn a_failure_in_the_chain_is_warned_and_swallowed_and_the_next_work_runs() {
    run(async {
        let h = held(ok(PUSHED), None);
        h.host.script("persist", Err(failed("disk full")));
        h.area.sub_done(WS).await;
        assert!(
            h.host
                .warns
                .borrow()
                .iter()
                .any(|w| w == "sub-thread machine ws_kid: disk full"),
            "{:?}",
            h.host.warns.borrow()
        );
        // The next done on the chain still runs.
        h.entry
            .borrow()
            .record
            .borrow_mut()
            .sub
            .as_mut()
            .unwrap()
            .result = None;
        h.area.sub_done(WS).await;
        assert!(sub(&h).result.is_some());
    });
}

#[test]
fn session_end_runs_the_done_after_a_send_queued_behind_takes_the_thread() {
    run(async {
        let h = held(ok(PUSHED), None);
        h.host
            .emit_now(json!({ "type": "session.end", "workspaceId": WS, "threadId": THREAD }));
        // The send queued behind the turn takes the thread in the same tick.
        h.host.running.borrow_mut().insert(THREAD.to_string());
        settle().await;
        assert_eq!(sub(&h).result, None, "a busy thread's done reads nothing");
        assert!(!h.machine.ran("git rev-list"));
    });
}

#[test]
fn refuse_sub_wake_names_released_and_cloud_gone() {
    run(async {
        let h = napped(ok(PUSHED)).await;
        assert!(h.area.refuse_sub_wake(&h.entry).is_ok());
        h.host.clock.advance(31.0 * MIN).await;
        settle().await;
        let now = h.host.live.borrow().get(WS).cloned().unwrap();
        let e = h.area.refuse_sub_wake(&now).unwrap_err();
        assert_eq!(
            e.message,
            wsp_words::sub_thread::sub_released_refusal(THREAD, "Boat").message
        );
        assert!(h.area.refuse_sub_send(&now).is_ok());
    });
}

#[test]
fn workspace_deleted_cancels_the_timer() {
    run(async {
        let h = napped(ok(PUSHED)).await;
        assert_eq!(h.host.clock.pending(), 1);
        h.host
            .emit_now(json!({ "type": "workspace.deleted", "workspaceId": WS }));
        assert_eq!(h.host.clock.pending(), 0);
        h.host.clock.advance(31.0 * MIN).await;
        assert!(!h.machine.killed());
    });
}

#[test]
fn lead_of_sub_is_only_the_lead_that_started_it() {
    run(async {
        let h = held(ok(PUSHED), None);
        assert!(h.area.lead_of_sub(Some(WS), Some(&lead())));
        let other: Caller = caller("t_other", "ws_lead", "t_other");
        assert!(!h.area.lead_of_sub(Some(WS), Some(&other)));
        assert!(!h.area.lead_of_sub(None, Some(&lead())));
        assert!(!h.area.lead_of_sub(Some(WS), None));
    });
}
