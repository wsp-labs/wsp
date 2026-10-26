// SPDX-License-Identifier: AGPL-3.0-only
//! conversations (spec/conversations.md): the agents' own conversations outside wsp, listed, read and opened. Expected
//! values read off conversations.ts at 65f6b89c7 and conversations.test.ts.
#[path = "support_b/mod.rs"]
mod support;

use std::rc::Rc;
use std::sync::Arc;

use serde_json::{json, Value};
use support::conversations_ctx::{
    earlier, folder_record, lasting_error, line, set_of, store_error, stored, tool_line, ws_entry, wts, FakeConversationStore,
    FakeConversationsCtx,
};
use support::transcripts_ctx::WithState;
use support::{err, json_of, local, row, sessions_of, settle, view};
use wsp_account::projects::folders::place::FolderPlaced;
use wsp_adapters::port::{ConversationLine, ConversationOrigin, EarlierAsk, EarlierRead};
use wsp_core::console;
use wsp_core::types::api::{ConversationAsk, PlacedStart};
use wsp_threads::threads::conversations::{earlier_rows, ConversationsArea, KEPT_LIST_MS};
use wsp_threads::threads::threads::Entry;
use wsp_wire::types::conversations::{
    ConversationsAnswer, ConversationsHeld, ResumeAsk, CONVERSATIONS_AGAIN_FIX, CONVERSATION_GONE_FIX, CONVERSATION_OPEN_KIND,
    NO_CONVERSATIONS_FIX, RESUME_WHERE_FIX, RESUME_WHERE_LINE,
};
use wsp_words::conversations::{
    conversation_gone_line, conversation_is_thread_fix, conversation_is_thread_line, conversation_open_fix, conversation_open_line,
    conversations_failed_line, conversations_unread_line, conversations_update_fix, earlier_line, no_conversations_line,
};
use wsp_words::exit::{refusal, HostError, Kind};
use wsp_words::words::tools::{tool_activity_line, tool_done_line};

const ID: &str = "7414323d-e71b-4957-8b56-eefdf6bfa350";
const OTHER: &str = "e615a6ab-a2f2-4b10-8fd6-0dc38bf99c28";
const THREAD: &str = "th01abcd-9999-4000-8000-000000000001";
const LETS_GO: &str = "It lets go a minute later.";

struct Host {
    ctx: Rc<FakeConversationsCtx>,
    area: Rc<ConversationsArea>,
    entry: Entry,
    claude: Arc<FakeConversationStore>,
}

/// Project p1 at /repo/p1 (a git top), live on workspace ws1, whose folder every `folderFor` answers; Claude Code's
/// store installed.
fn host() -> Host {
    let ctx = FakeConversationsCtx::new();
    let entry = ws_entry(json!({}));
    ctx.add_entry(entry.clone());
    ctx.folder_is(entry.clone(), None);
    ctx.live_projects.borrow_mut().insert("ws1".into(), "p1".into());
    ctx.project("p1", "lab", "/repo/p1", Some("/repo/p1"));
    let claude = FakeConversationStore::new();
    ctx.install("claude", claude.clone());
    let area = ctx.area();
    Host { ctx, area, entry, claude }
}

fn same_refusal(got: &HostError, want: HostError) {
    assert_eq!(got.message, want.message);
    assert_eq!(got.fix, want.fix);
    assert_eq!(got.kind, want.kind);
}

fn refused<T: std::fmt::Debug>(got: Result<T, HostError>) -> HostError {
    got.expect_err("the call was refused")
}

fn thread_ends(ctx: &FakeConversationsCtx, ty: &str, workspace_id: &str) {
    let v = match ty {
        "session.done" => json!({ "type": ty, "workspaceId": workspace_id, "sessionId": "r1", "result": { "status": "completed" } }),
        _ => json!({ "type": ty, "workspaceId": workspace_id, "sessionId": "r1", "exitCode": 0, "sawResult": true }),
    };
    ctx.bus.emit_json(v);
}

fn kept_keys(area: &ConversationsArea) -> Vec<String> {
    area.kept.borrow().keys().cloned().collect()
}

type Fact = (String, String, ConversationOrigin, bool, Option<String>, Option<String>);

/// Each row as `(agent, id, origin, live, thread, letsGo)`.
fn facts(a: &ConversationsAnswer) -> Vec<Fact> {
    a.rows.iter().map(|r| (r.agent.clone(), r.id.clone(), r.origin, r.live, r.thread.clone(), r.lets_go.clone())).collect()
}

fn ids(a: &ConversationsAnswer) -> Vec<String> {
    a.rows.iter().map(|r| r.id.clone()).collect()
}

/// The `who`, `text` and `at` of each row a list of earlier rows holds, off its JSON.
fn shown(rows: &impl serde::Serialize) -> Vec<(String, String, Option<f64>)> {
    json_of(rows)
        .as_array()
        .expect("rows")
        .iter()
        .map(|r| (r["who"].as_str().expect("who").to_string(), r["text"].as_str().expect("text").to_string(), r["at"].as_f64()))
        .collect()
}

fn plain(who: &str, text: &str, at: Option<f64>) -> (String, String, Option<f64>) {
    (who.to_string(), text.to_string(), at)
}

fn codewords() -> wsp_adapters::port::ConversationEarlier {
    earlier(
        Some("/work/acme/lab-wt"),
        Some("lab codewords"),
        vec![
            line("person", "Remember the codeword ALPHA."),
            tool_line("Bash", r#"{"command":"echo hi-from-tool"}"#),
            line("agent", "ALPHA"),
        ],
        3,
    )
}

fn ask(agent: &str, id: &str, preview: bool) -> ConversationAsk {
    ConversationAsk { project: None, agent: agent.into(), id: id.into(), preview: preview.then_some(true) }
}

fn resume(id: &str, copy: bool) -> ResumeAsk {
    ResumeAsk { id: id.into(), copy: copy.then_some(true) }
}

fn earlier_ask(cwds: &[&str], last: u32, probe: bool) -> EarlierAsk {
    EarlierAsk { cwds: cwds.iter().map(|s| s.to_string()).collect(), last, probe, tail: None, count: true }
}

// ---- the kept list (V1, V8) ------------------------------------------------------------------------------------

// TS: conversations.test.ts:377, :467
#[test]
fn a_kept_list_answers_a_second_visit_and_goes_when_a_thread_ends() {
    local(async {
        let h = host();
        h.claude.rows(vec![stored(ID, "/repo/p1", 100.0)]).live_answers(vec![Some(set_of(&[]))]);
        let first = h.area.list(None, None, None).await.expect("a list");
        let second = h.area.list(None, None, None).await.expect("a list");
        assert_eq!(ids(&first), vec![ID]);
        assert_eq!(ids(&second), vec![ID]);
        assert_eq!(h.claude.list_calls(), 1, "the second visit is answered off the kept read");
        assert_eq!(kept_keys(&h.area), vec!["p1\0claude".to_string()]);

        thread_ends(&h.ctx, "session.done", "ws1");
        assert_eq!(kept_keys(&h.area), Vec::<String>::new(), "a turn's done forgets the project's reads");
        h.area.list(None, None, None).await.expect("a list");
        assert_eq!(h.claude.list_calls(), 2);
        assert_eq!(kept_keys(&h.area), vec!["p1\0claude".to_string()]);

        thread_ends(&h.ctx, "session.end", "ws1");
        assert_eq!(kept_keys(&h.area), Vec::<String>::new(), "a turn's end forgets them too");
        h.area.list(None, None, None).await.expect("a list");
        assert_eq!(h.claude.list_calls(), 3);
    });
}

#[test]
fn a_thread_ending_forgets_only_its_projects_reads_and_a_workspace_not_live_forgets_nothing() {
    local(async {
        let h = host();
        let other = ws_entry(json!({ "id": "ws2", "project": "p2" }));
        h.ctx.add_entry(other.clone());
        h.ctx.folder_answers.borrow_mut().push((Some("p2".into()), None, Ok(FolderPlaced { entry: other, cwd: None, setup: None })));
        h.ctx.live_projects.borrow_mut().insert("ws2".into(), "p2".into());
        h.ctx.project("p2", "farm", "/repo/p2", Some("/repo/p2"));
        h.claude.rows(vec![stored(ID, "/repo/p1", 100.0)]);
        h.area.list(Some("p1".into()), None, None).await.expect("a list");
        h.area.list(Some("p2".into()), None, None).await.expect("a list");
        assert_eq!(kept_keys(&h.area), vec!["p1\0claude".to_string(), "p2\0claude".to_string()]);

        thread_ends(&h.ctx, "session.done", "ws9");
        assert_eq!(kept_keys(&h.area).len(), 2, "a workspace that is not live forgets nothing");

        thread_ends(&h.ctx, "session.done", "ws1");
        assert_eq!(kept_keys(&h.area), vec!["p2\0claude".to_string()]);
    });
}

#[test]
fn ten_visits_in_a_minute_read_the_store_once() {
    local(async {
        let h = host();
        h.claude.rows(vec![stored(ID, "/repo/p1", 100.0)]).stamp_answers(vec![Ok("s1".into())]);
        for _ in 0..5 {
            assert_eq!(ids(&h.area.list(None, None, None).await.expect("a list")), vec![ID]);
        }
        let visits = futures::future::join_all((0..5).map(|_| h.area.list(None, None, None))).await;
        assert_eq!(visits.len(), 5);
        for v in visits {
            assert_eq!(ids(&v.expect("a list")), vec![ID]);
        }
        assert_eq!(h.claude.list_calls(), 1);
        assert_eq!(h.claude.stamp_calls(), 10, "every visit asks the stamp");
    });
}

// TS: conversations.test.ts:467
#[test]
fn a_kept_list_goes_after_a_minute() {
    local(async {
        let h = host();
        h.claude.rows(vec![stored(ID, "/repo/p1", 100.0)]);
        h.area.list(None, None, None).await.expect("a list");
        h.ctx.clock.advance(KEPT_LIST_MS - 1);
        h.area.list(None, None, None).await.expect("a list");
        assert_eq!(h.claude.list_calls(), 1, "59.999 s on, the kept read still answers");
        h.ctx.clock.advance(1);
        h.area.list(None, None, None).await.expect("a list");
        assert_eq!(h.claude.list_calls(), 2, "a read 60 s old is read again");
    });
}

#[test]
fn a_visit_to_any_project_drops_every_read_a_minute_old() {
    local(async {
        let h = host();
        let other = ws_entry(json!({ "id": "ws2", "project": "p2" }));
        h.ctx.folder_answers.borrow_mut().push((Some("p2".into()), None, Ok(FolderPlaced { entry: other, cwd: None, setup: None })));
        h.ctx.project("p2", "farm", "/repo/p2", Some("/repo/p2"));
        h.claude.rows(vec![stored(ID, "/repo/p1", 100.0)]);
        h.area.list(Some("p1".into()), None, None).await.expect("a list");
        h.ctx.clock.advance(KEPT_LIST_MS);
        h.area.list(Some("p2".into()), None, None).await.expect("a list");
        assert_eq!(kept_keys(&h.area), vec!["p2\0claude".to_string()], "p1's read, a minute old, went with the visit to p2");
    });
}

// TS: conversations.test.ts:392
#[test]
fn a_new_stamp_reads_the_store_again() {
    local(async {
        let h = host();
        h.claude.rows(vec![stored(ID, "/repo/p1", 100.0)]).stamp_answers(vec![Ok("s1".into()), Ok("s1".into()), Ok("s2".into())]);
        h.area.list(None, None, None).await.expect("a list");
        h.area.list(None, None, None).await.expect("a list");
        assert_eq!(h.claude.list_calls(), 1, "the same stamp answers the second visit");
        h.area.list(None, None, None).await.expect("a list");
        assert_eq!(h.claude.list_calls(), 2, "a new stamp reads again");
        h.area.list(None, None, None).await.expect("a list");
        assert_eq!(h.claude.list_calls(), 2, "and the new stamp is the kept one");
        assert_eq!(h.claude.stamp_cwds(), vec![vec!["/repo/p1".to_string()]; 4]);
    });
}

#[test]
fn a_failed_stamp_reads_as_empty() {
    local(async {
        let h = host();
        h.claude.rows(vec![stored(ID, "/repo/p1", 100.0)]).stamp_answers(vec![
            Err(store_error("stat failed")),
            Err(store_error("stat failed")),
            Ok("x".into()),
        ]);
        h.area.list(None, None, None).await.expect("a list");
        h.area.list(None, None, None).await.expect("a list");
        assert_eq!(h.claude.list_calls(), 1, "a stamp that failed twice reads \"\" twice, which is the same");
        h.area.list(None, None, None).await.expect("a list");
        assert_eq!(h.claude.list_calls(), 2);
    });
}

#[test]
fn new_folders_read_the_store_again() {
    local(async {
        let h = host();
        h.claude.rows(vec![stored(ID, "/repo/p1", 100.0)]);
        h.area.list(None, None, None).await.expect("a list");
        h.area.list(None, None, None).await.expect("a list");
        assert_eq!(h.claude.list_calls(), 1);
        h.ctx.worktrees.borrow_mut().insert("/repo/p1".into(), Ok(wts(&["/wt/a"])));
        h.area.list(None, None, None).await.expect("a list");
        assert_eq!(h.claude.list_cwds(), vec![vec!["/repo/p1".to_string()], vec!["/repo/p1".to_string(), "/wt/a".to_string()]]);
    });
}

// TS: conversations.test.ts:406
#[test]
fn a_kept_list_reads_live_ids_again_when_one_of_the_persons_rows_was_open() {
    local(async {
        let h = host();
        h.claude.rows(vec![stored(ID, "/repo/p1", 100.0)]).live_answers(vec![Some(set_of(&[ID])), Some(set_of(&[]))]);
        let marks = |a: &ConversationsAnswer| a.rows.iter().map(|r| r.live).collect::<Vec<_>>();
        assert_eq!(marks(&h.area.list(None, None, None).await.expect("a list")), vec![true]);
        assert_eq!((h.claude.list_calls(), h.claude.live_calls()), (1, 1));
        assert_eq!(marks(&h.area.list(None, None, None).await.expect("a list")), vec![false], "closing the other app clears the mark");
        assert_eq!((h.claude.list_calls(), h.claude.live_calls()), (1, 2), "the list is kept, the open ones asked again");
        assert_eq!(marks(&h.area.list(None, None, None).await.expect("a list")), vec![false]);
        assert_eq!(
            (h.claude.list_calls(), h.claude.live_calls()),
            (1, 2),
            "the fresh answer is the kept one: nothing was open, nothing to ask"
        );
    });
}

#[test]
fn a_kept_row_that_is_a_wsp_thread_does_not_ask_the_open_ones_again() {
    local(async {
        let h = host();
        h.ctx.transcript_state.with(|t| {
            let mut index = wsp_core::types::internal::TranscriptIndex::default();
            index.starts.insert("t1".into(), ID.into());
            t.index.insert("ws1".into(), index);
        });
        h.claude.rows(vec![stored(ID, "/repo/p1", 100.0)]).live_answers(vec![Some(set_of(&[ID]))]);
        h.area.list(None, None, None).await.expect("a list");
        h.area.list(None, None, None).await.expect("a list");
        assert_eq!((h.claude.list_calls(), h.claude.live_calls()), (1, 1));
    });
}

// ---- wsp_sessions, road_of, folders_of, stores_on (V2 to V5) ---------------------------------------------------

#[test]
fn wsp_sessions_reads_the_index_starts_then_the_rows() {
    let h = host();
    h.ctx.transcript_state.with(|t| {
        let mut first = wsp_core::types::internal::TranscriptIndex::default();
        first.starts.insert("t1".into(), "s1".into());
        first.starts.insert("t2".into(), "s2".into());
        let mut second = wsp_core::types::internal::TranscriptIndex::default();
        second.starts.insert("t3".into(), "s2".into());
        second.starts.insert("t4".into(), "s4".into());
        t.index.insert("ws1".into(), first);
        t.index.insert("ws2".into(), second);
    });
    h.ctx.sessions.with(|s| {
        *s = sessions_of(vec![
            row(view(json!({ "id": "r1", "threadId": "t5", "claudeSessionId": "s4" })), "u1"),
            row(view(json!({ "id": "r2", "threadId": "t6" })), "u2"),
            row(view(json!({ "id": "r3", "claudeSessionId": "s9" })), "u3"),
            row(view(json!({ "id": "r4", "threadId": "t7", "claudeSessionId": "s1" })), "u4"),
        ])
        .take()
    });
    let found: Vec<(String, String)> = h.area.wsp_sessions().into_iter().collect();
    assert_eq!(
        found,
        vec![("s1".to_string(), "t7".to_string()), ("s2".to_string(), "t3".to_string()), ("s4".to_string(), "t5".to_string())],
        "each index's starts in index order, a later index and then a row overwriting, a row with no session or no thread left out"
    );
}

#[test]
fn road_runs_commands_in_thirty_seconds_and_asks_the_daemon() {
    local(async {
        let h = host();
        h.ctx.daemon_answers("transcripts.list", Ok(json!({ "ok": true, "rows": [] })));
        let road = h.area.road_of(&h.entry);
        assert_eq!(*h.ctx.exec_timeouts.borrow(), vec![30_000u64]);
        assert_eq!(road.exec.exec("echo hi", None).await.expect("an exec"), "exec-out");
        assert_eq!(*h.ctx.exec_commands.lock().expect("commands"), vec!["echo hi".to_string()]);
        let mut frame = serde_json::Map::new();
        frame.insert("op".into(), json!("transcripts.list"));
        frame.insert("cwds".into(), json!(["/repo/p1"]));
        let answered = (road.ask)(frame).await.expect("the daemon answers");
        assert_eq!(Value::Object(answered), json!({ "ok": true, "rows": [] }));
        assert_eq!(*h.ctx.frames.borrow(), vec![json!({ "op": "transcripts.list", "cwds": ["/repo/p1"] })]);
    });
}

#[test]
fn folders_of_lists_the_project_its_worktrees_and_held_folders_once() {
    local(async {
        let h = host();
        h.ctx.worktrees.borrow_mut().insert("/repo/p1".into(), Ok(wts(&["/wt/a", "/repo/p1", "/wt/b"])));
        h.ctx.held_folders.borrow_mut().insert(
            "p1".into(),
            vec![
                folder_record("f1", "p1", Some(("/wt/b", false))),
                folder_record("f2", "p1", Some(("/wt/gone", true))),
                folder_record("f3", "p1", None),
                folder_record("f4", "p1", Some(("/wt/c", false))),
            ],
        );
        let folders = h.area.folders_of(&h.entry).await.expect("folders");
        assert_eq!(
            folders,
            vec!["/repo/p1", "/wt/a", "/wt/b", "/wt/c"],
            "the path, then git's worktrees, then held folders whose worktree is not gone, each once"
        );
        assert_eq!(h.ctx.called().iter().filter(|c| *c == "worktreesOf /repo/p1").count(), 1);
    });
}

#[test]
fn folders_of_asks_git_only_on_a_kind_that_copies_a_folder_with_a_git_top() {
    local(async {
        let h = host();
        h.ctx.worktrees.borrow_mut().insert("/repo/p1".into(), Ok(wts(&["/wt/a"])));
        let cloud = ws_entry(json!({ "id": "ws3", "kind": "cloud" }));
        assert_eq!(h.area.folders_of(&cloud).await.expect("folders"), vec!["/repo/p1"], "a cloud machine copies no folder");
        h.ctx.project("p1", "lab", "/repo/p1", None);
        assert_eq!(h.area.folders_of(&h.entry).await.expect("folders"), vec!["/repo/p1"], "no git top, no worktrees");
        assert!(!h.ctx.called().iter().any(|c| c.starts_with("worktreesOf")));
    });
}

#[test]
fn folders_of_fails_where_git_cannot_list_the_worktrees() {
    local(async {
        let h = host();
        h.ctx.worktrees.borrow_mut().insert("/repo/p1".into(), Err(err("git: not a repository")));
        let failed = refused(h.area.folders_of(&h.entry).await);
        assert_eq!(failed.message, "git: not a repository");
    });
}

#[test]
fn stores_on_keeps_the_catalog_order_and_skips_what_is_off_or_reads_nothing() {
    let h = host();
    h.ctx.install("codex", FakeConversationStore::new());
    h.ctx.install_without_store("gemini");
    h.ctx.install("zz-not-in-the-catalog", FakeConversationStore::new());
    let names = |agent: Option<&str>| h.area.stores_on(&h.entry, agent).into_iter().map(|(a, _)| a).collect::<Vec<_>>();
    assert_eq!(names(None), vec!["claude", "codex"], "catalog order; gemini reads no store and the unknown id is no catalog agent");
    assert_eq!(names(Some("codex")), vec!["codex"]);
    h.ctx.off.borrow_mut().insert("codex".into());
    assert_eq!(names(None), vec!["claude"], "an agent turned off on the computer is skipped");
    assert_eq!(names(Some("codex")), Vec::<String>::new());
    assert_eq!(names(Some("gemini")), Vec::<String>::new());
}

// ---- list (V5 to V9) -----------------------------------------------------------------------------------------

// TS: conversations.test.ts:188, :205
#[test]
fn list_marks_wsp_threads_and_leaves_out_scripts() {
    local(async {
        let h = host();
        let codex = FakeConversationStore::new();
        h.ctx.install("codex", codex.clone());
        let mut mine = stored("mine-session", "/repo/p1", 50.0);
        mine.origin = ConversationOrigin::Script;
        let mut script = stored("script-session", "/repo/p1", 70.0);
        script.origin = ConversationOrigin::Script;
        h.claude.rows(vec![mine, stored(OTHER, "/repo/p1", 40.0), script]).live_answers(vec![Some(set_of(&["mine-session", OTHER]))]);
        codex.rows(vec![stored("cx-1", "/repo/p1", 60.0)]).lets_go_says(LETS_GO);
        h.ctx.sessions.with(|s| {
            *s = sessions_of(vec![row(
                view(json!({ "id": "r1", "threadId": "t1", "claudeSessionId": "mine-session", "startedAt": 5 })),
                "u1",
            )])
            .take()
        });

        let answer = h.area.list(None, None, None).await.expect("a list");
        assert_eq!(h.claude.list_cwds(), vec![vec!["/repo/p1".to_string()]]);
        assert_eq!(codex.list_cwds(), vec![vec!["/repo/p1".to_string()]]);
        assert_eq!(answer.held, Vec::<ConversationsHeld>::new());
        assert_eq!(
            facts(&answer),
            vec![
                ("codex".into(), "cx-1".into(), ConversationOrigin::Terminal, false, None, Some(LETS_GO.to_string())),
                ("claude".into(), "mine-session".into(), ConversationOrigin::Wsp, false, Some("t1".to_string()), None),
                ("claude".into(), OTHER.into(), ConversationOrigin::Terminal, true, None, None),
            ],
            "newest first; a wsp thread's session is wsp's, never open elsewhere; a script's own session is left out; another app's open one is live"
        );
        assert_eq!(answer.rows[1].title, "t-mine");
        assert_eq!(answer.rows[1].cwd, "/repo/p1");

        let only = h.area.list(None, Some("claude".into()), None).await.expect("a list");
        assert_eq!(only.rows.iter().map(|r| r.agent.clone()).collect::<Vec<_>>(), vec!["claude", "claude"]);
    });
}

// TS: conversations.test.ts:188
#[test]
fn list_asks_the_folder_of_the_named_project_and_the_default_without_one() {
    local(async {
        let h = host();
        h.claude.rows(vec![]);
        h.area.list(Some("p1".into()), None, None).await.expect("a list");
        h.area.list(None, None, None).await.expect("a list");
        let asked: Vec<String> = h.ctx.called().into_iter().filter(|c| c.starts_with("folderFor")).collect();
        assert_eq!(asked, vec!["folderFor p1 ".to_string(), "folderFor  ".to_string()]);
    });
}

// TS: conversations.test.ts:527
#[test]
fn list_refuses_a_named_agent_with_no_store() {
    local(async {
        let h = host();
        let got = refused(h.area.list(None, Some("codex".into()), None).await);
        same_refusal(&got, refusal(&no_conversations_line("Codex"), NO_CONVERSATIONS_FIX, Some(Kind::Usage)));
        h.ctx.install_without_store("codex");
        let got = refused(h.area.list(None, Some("codex".into()), None).await);
        same_refusal(&got, refusal(&no_conversations_line("Codex"), NO_CONVERSATIONS_FIX, Some(Kind::Usage)));
        h.ctx.install("codex", FakeConversationStore::new());
        h.ctx.off.borrow_mut().insert("codex".into());
        let got = refused(h.area.list(None, Some("codex".into()), None).await);
        same_refusal(&got, refusal(&no_conversations_line("Codex"), NO_CONVERSATIONS_FIX, Some(Kind::Usage)));
        assert_eq!(h.claude.list_calls(), 0, "refused before any store was read");
    });
}

#[test]
fn list_hides_a_thread_the_caller_does_not_reach() {
    local(async {
        let h = host();
        h.claude.rows(vec![stored(ID, "/repo/p1", 100.0)]).live_answers(vec![Some(set_of(&[ID]))]);
        h.ctx.sessions.with(|s| {
            *s = sessions_of(vec![row(view(json!({ "id": "r1", "threadId": "t1", "claudeSessionId": ID, "startedAt": 5 })), "u1")]).take()
        });
        let seen = h.area.list(None, None, None).await.expect("a list");
        assert_eq!(facts(&seen), vec![("claude".into(), ID.into(), ConversationOrigin::Wsp, false, Some("t1".to_string()), None)]);
        h.ctx.unreached.borrow_mut().insert("r1".into());
        let hidden = h.area.list(None, None, None).await.expect("a list");
        assert_eq!(
            facts(&hidden),
            vec![("claude".into(), ID.into(), ConversationOrigin::Wsp, false, None, None)],
            "the row stays, marked wsp's, and its thread is named only to a caller that reaches its latest row"
        );
        assert!(h.ctx.called().contains(&"reachesRow r1".to_string()));
    });
}

#[test]
fn list_sorts_newest_first_then_by_id() {
    local(async {
        let h = host();
        h.claude.rows(vec![
            stored("b", "/repo/p1", 10.0),
            stored("old", "/repo/p1", 5.0),
            stored("a", "/repo/p1", 10.0),
            stored("C", "/repo/p1", 10.0),
            stored("new", "/repo/p1", 20.0),
        ]);
        let answer = h.area.list(None, None, None).await.expect("a list");
        assert_eq!(ids(&answer), vec!["new", "a", "b", "C", "old"], "lastAt descending, then id by localeCompare: a, b, C");
    });
}

// TS: conversations.test.ts:225
#[test]
fn list_says_the_update_a_daemon_too_old_needs() {
    local(async {
        let h = host();
        let codex = FakeConversationStore::new();
        h.ctx.install("codex", codex.clone());
        h.claude.list_answers(vec![Err(store_error("unknown op: transcripts.list"))]).live_answers(vec![None]);
        codex.rows(vec![stored("cx-1", "/repo/p1", 200.0)]);
        let captured = console::capture();

        assert_eq!(
            h.area.unread_of("unknown op: transcripts.list", &h.entry, "claude"),
            Some(ConversationsHeld {
                agent: "claude".into(),
                said: conversations_unread_line("Claude Code", "ws1-computer"),
                fix: conversations_update_fix("ws1-computer"),
                why: None,
            })
        );
        assert_eq!(h.area.unread_of("unknown op", &h.entry, "claude"), None, "the prefix is `unknown op: `");
        assert_eq!(h.area.unread_of("the daemon connection closed", &h.entry, "claude"), None);

        let answer = h.area.list(None, None, None).await.expect("a list");
        assert_eq!(answer.rows.iter().map(|r| r.agent.clone()).collect::<Vec<_>>(), vec!["codex"], "the other agents' rows stand");
        assert_eq!(
            answer.held,
            vec![ConversationsHeld {
                agent: "claude".into(),
                said: conversations_unread_line("Claude Code", "ws1-computer"),
                fix: conversations_update_fix("ws1-computer"),
                why: None,
            }]
        );
        assert_eq!(captured.lines(), Vec::<String>::new(), "a daemon too old is said in the line, not warned");
        h.area.list(None, None, None).await.expect("a list");
        assert_eq!(h.claude.list_calls(), 1, "a daemon too old fails the same all minute: its read is kept");
    });
}

// TS: conversations.test.ts:240, :421
#[test]
fn a_passing_failure_is_said_and_not_kept() {
    local(async {
        let h = host();
        let codex = FakeConversationStore::new();
        h.ctx.install("codex", codex.clone());
        h.claude.rows(vec![stored(ID, "/repo/p1", 100.0)]);
        codex
            .list_answers(vec![Err(store_error("the daemon connection closed")), Ok(vec![stored("cx-1", "/repo/p1", 50.0)])])
            .live_answers(vec![None]);
        let captured = console::capture();

        let first = h.area.list(None, None, None).await.expect("a list");
        assert_eq!(ids(&first), vec![ID], "the others' rows standing");
        assert_eq!(
            first.held,
            vec![ConversationsHeld {
                agent: "codex".into(),
                said: conversations_failed_line("Codex", "the daemon connection closed"),
                fix: CONVERSATIONS_AGAIN_FIX.into(),
                why: Some("the daemon connection closed".into()),
            }]
        );
        assert_eq!(
            captured.lines(),
            vec!["the Codex conversations on ws1-computer were not read: the daemon connection closed".to_string()]
        );
        assert_eq!(codex.live_calls(), 1, "the open ones are asked beside the list, a failed list or not");
        settle().await;
        assert_eq!(kept_keys(&h.area), vec!["p1\0claude".to_string()], "the failed read dropped itself");

        let second = h.area.list(None, None, None).await.expect("a list");
        assert_eq!(ids(&second), vec![ID.to_string(), "cx-1".to_string()]);
        assert_eq!(second.held, Vec::<ConversationsHeld>::new());
        assert_eq!(
            (codex.list_calls(), h.claude.list_calls()),
            (2, 1),
            "the next visit reads the failed store again and keeps the good one"
        );
    });
}

// TS: conversations.test.ts:441
#[test]
fn a_lasting_store_error_lists_nothing_and_says_nothing() {
    local(async {
        let h = host();
        h.claude.list_answers(vec![Err(lasting_error("codex is not installed"))]);
        let captured = console::capture();
        for _ in 0..3 {
            let answer = h.area.list(None, None, None).await.expect("a list");
            assert_eq!(answer.rows.len(), 0);
            assert_eq!(answer.held, Vec::<ConversationsHeld>::new(), "no line for a failure that lasts");
        }
        settle().await;
        assert_eq!(h.claude.list_calls(), 1, "kept for the minute");
        assert_eq!(
            captured.lines(),
            vec!["the Claude Code conversations on ws1-computer were not read: codex is not installed".to_string()],
            "warned once"
        );
        assert_eq!(kept_keys(&h.area), vec!["p1\0claude".to_string()]);
    });
}

// ---- earlier_rows, read (V10, V11) -------------------------------------------------------------------------------

// TS: conversations.test.ts:586
#[test]
fn earlier_rows_put_the_note_first_at_the_first_lines_time_and_keep_each_lines_time() {
    let bash = r#"{"command":"echo hi-from-tool"}"#;
    let mut first = line("person", "Remember the codeword ALPHA.");
    first.at = Some(1_000.0);
    let mut tool = tool_line("Bash", bash);
    tool.at = Some(2_000.0);
    let last = line("agent", "ALPHA");
    let lines: Vec<ConversationLine> = vec![first, tool, last];
    let call = tool_done_line(Some("Bash"), bash).unwrap_or_else(|| tool_activity_line(Some("Bash"), bash));
    assert_eq!(
        shown(&earlier_rows(&lines, 3, "Claude Code")),
        vec![
            plain("note", &earlier_line(3.0, "Claude Code"), Some(1_000.0)),
            plain("person", "Remember the codeword ALPHA.", Some(1_000.0)),
            plain("tool", &call, Some(2_000.0)),
            plain("agent", "ALPHA", None),
        ]
    );
    assert_eq!(call, "$ echo hi-from-tool");
    assert_eq!(shown(&earlier_rows(&lines, 0, "Claude Code")).len(), 3, "nothing left out, no note");
    assert_eq!(
        shown(&earlier_rows(&[], 2, "Codex")),
        vec![plain("note", &earlier_line(2.0, "Codex"), None)],
        "a note with no first line has no time"
    );
}

// TS: conversations.test.ts:498
#[test]
fn read_answers_the_newest_rows_with_a_note_for_the_rest() {
    local(async {
        let h = host();
        h.claude.earlier_answers(vec![
            Ok(EarlierRead::Read(codewords())),
            Ok(EarlierRead::Read(earlier(None, None, vec![line("agent", "hi")], 0))),
        ]);
        let got = h.area.read(ask("claude", ID, false), None).await.expect("a read");
        assert_eq!(got.cwd, Some("/work/acme/lab-wt".to_string()));
        assert_eq!(got.title, "lab codewords");
        assert_eq!(
            shown(&got.rows),
            vec![
                plain("note", &earlier_line(3.0, "Claude Code"), None),
                plain("person", "Remember the codeword ALPHA.", None),
                plain("tool", "$ echo hi-from-tool", None),
                plain("agent", "ALPHA", None),
            ]
        );
        assert_eq!(
            h.claude.earlier_asks(),
            vec![(ID.to_string(), earlier_ask(&["/repo/p1"], 200, false))],
            "the newest 200, no probe: read only, nothing takes it as a writer"
        );

        let bare = h.area.read(ask("claude", OTHER, false), None).await.expect("a read");
        assert_eq!(bare.cwd, None);
        assert_eq!(bare.title, OTHER, "no title reads as the id");
        assert_eq!(shown(&bare.rows), vec![plain("agent", "hi", None)]);
    });
}

// TS: conversations.test.ts:517
#[test]
fn read_of_a_preview_asks_the_tail_and_counts_nothing() {
    local(async {
        let h = host();
        h.claude.earlier_answers(vec![Ok(EarlierRead::Read(codewords()))]);
        let got = h.area.read(ask("claude", ID, true), None).await.expect("a read");
        assert_eq!(
            shown(&got.rows).iter().map(|r| r.0.clone()).collect::<Vec<_>>(),
            vec!["person", "tool", "agent"],
            "no note on a preview, whatever is left out"
        );
        assert_eq!(
            h.claude.earlier_asks(),
            vec![(ID.to_string(), EarlierAsk { cwds: vec!["/repo/p1".into()], last: 4, probe: false, tail: Some(262_144), count: false })]
        );
    });
}

// TS: conversations.test.ts:527
#[test]
fn read_of_a_conversation_gone_refuses_not_found() {
    local(async {
        let h = host();
        h.claude.earlier_answers(vec![Ok(EarlierRead::Gone)]);
        let got = refused(h.area.read(ask("claude", OTHER, false), None).await);
        same_refusal(
            &got,
            refusal(&conversation_gone_line("Claude Code", OTHER, "ws1-computer"), CONVERSATION_GONE_FIX, Some(Kind::NotFound)),
        );
        h.claude.earlier_answers(vec![Ok(EarlierRead::Open)]);
        let got = refused(h.area.read(ask("claude", OTHER, false), None).await);
        same_refusal(
            &got,
            refusal(&conversation_gone_line("Claude Code", OTHER, "ws1-computer"), CONVERSATION_GONE_FIX, Some(Kind::NotFound)),
        );

        let got = refused(h.area.read(ask("codex", OTHER, false), None).await);
        same_refusal(&got, refusal(&no_conversations_line("Codex"), NO_CONVERSATIONS_FIX, Some(Kind::Usage)));
    });
}

#[test]
fn read_says_the_update_a_daemon_too_old_needs_and_lets_any_other_failure_through() {
    local(async {
        let h = host();
        h.claude.earlier_answers(vec![Err(store_error("unknown op: transcripts.read"))]);
        let got = refused(h.area.read(ask("claude", ID, false), None).await);
        same_refusal(
            &got,
            refusal(
                &conversations_unread_line("Claude Code", "ws1-computer"),
                &conversations_update_fix("ws1-computer"),
                Some(Kind::Usage),
            ),
        );
        h.claude.earlier_answers(vec![Err(store_error("the daemon connection closed"))]);
        let got = refused(h.area.read(ask("claude", ID, false), None).await);
        assert_eq!((got.message.as_str(), got.fix.clone(), got.kind.clone()), ("the daemon connection closed", None, None));
    });
}

// ---- opening, placed (V12, V13) ----------------------------------------------------------------------------------

// TS: conversations.test.ts:671
#[test]
fn opening_refuses_a_conversation_a_thread_ran() {
    local(async {
        let h = host();
        h.ctx.transcript_state.with(|t| {
            let mut index = wsp_core::types::internal::TranscriptIndex::default();
            index.starts.insert(THREAD.into(), ID.into());
            t.index.insert("ws1".into(), index);
        });
        h.claude.earlier_answers(vec![Ok(EarlierRead::Read(codewords()))]);
        let got = refused(h.area.opening("ws1", Some("claude".into()), resume(ID, false), None).await);
        same_refusal(&got, refusal(&conversation_is_thread_line(ID, THREAD), &conversation_is_thread_fix(THREAD), Some(Kind::Usage)));
        assert!(got.message.contains("is thread th01abcd already"));
        assert_eq!(h.claude.earlier_asks(), vec![], "refused before the store was asked");

        h.ctx.transcript_state.with(|t| t.index.clear());
        h.ctx
            .sessions
            .with(|s| *s = sessions_of(vec![row(view(json!({ "id": "r1", "threadId": THREAD, "claudeSessionId": ID })), "u1")]).take());
        let got = refused(h.area.opening("ws1", Some("claude".into()), resume(ID, false), None).await);
        same_refusal(&got, refusal(&conversation_is_thread_line(ID, THREAD), &conversation_is_thread_fix(THREAD), Some(Kind::Usage)));
    });
}

// TS: conversations.test.ts:610, :622
#[test]
fn opening_refuses_an_open_conversation_with_its_title() {
    local(async {
        let h = host();
        h.claude.earlier_answers(vec![Ok(EarlierRead::Read(codewords()))]).live_answers(vec![Some(set_of(&[ID]))]);
        let got = refused(h.area.opening("ws1", Some("claude".into()), resume(ID, false), None).await);
        same_refusal(
            &got,
            refusal(
                &conversation_open_line("lab codewords", "ws1-computer"),
                &conversation_open_fix(None),
                Some(Kind::parse(CONVERSATION_OPEN_KIND)),
            ),
        );
        assert_eq!(h.claude.earlier_asks(), vec![(ID.to_string(), earlier_ask(&["/repo/p1"], 200, true))]);
        assert_eq!(h.claude.live_calls(), 1, "a read that found nothing open asks the open ids");
    });
}

// TS: conversations.test.ts:622
#[test]
fn opening_refuses_where_the_agent_itself_refuses_a_second_writer_and_says_when_it_lets_go() {
    local(async {
        let h = host();
        let codex = FakeConversationStore::new();
        h.ctx.install("codex", codex.clone());
        codex.earlier_on_probe(Ok(EarlierRead::Open)).earlier_answers(vec![Ok(EarlierRead::Read(codewords()))]).lets_go_says(LETS_GO);
        let got = refused(h.area.opening("ws1", Some("codex".into()), resume(ID, false), None).await);
        same_refusal(
            &got,
            refusal(
                &conversation_open_line("lab codewords", "ws1-computer"),
                &conversation_open_fix(Some(LETS_GO)),
                Some(Kind::parse(CONVERSATION_OPEN_KIND)),
            ),
        );
        assert_eq!(
            codex.earlier_asks(),
            vec![(ID.to_string(), earlier_ask(&["/repo/p1"], 200, true)), (ID.to_string(), earlier_ask(&["/repo/p1"], 0, false))],
            "the title is read again with last 0 and no probe"
        );
    });
}

#[test]
fn an_open_conversation_whose_title_read_finds_nothing_is_named_by_its_id() {
    local(async {
        let h = host();
        h.claude.earlier_on_probe(Ok(EarlierRead::Open)).earlier_answers(vec![Ok(EarlierRead::Gone)]);
        let got = refused(h.area.opening("ws1", Some("claude".into()), resume(ID, false), None).await);
        same_refusal(
            &got,
            refusal(&conversation_open_line(ID, "ws1-computer"), &conversation_open_fix(None), Some(Kind::parse(CONVERSATION_OPEN_KIND))),
        );
    });
}

// TS: conversations.test.ts:634
#[test]
fn opening_a_copy_reads_without_a_probe_and_is_never_refused_as_open() {
    local(async {
        let h = host();
        h.claude
            .earlier_on_probe(Ok(EarlierRead::Open))
            .earlier_answers(vec![Ok(EarlierRead::Read(codewords()))])
            .live_answers(vec![Some(set_of(&[ID]))]);
        let got = h.area.opening("ws1", Some("claude".into()), resume(ID, true), None).await.expect("an opening");
        assert!(got.copy);
        assert_eq!(h.claude.earlier_asks(), vec![(ID.to_string(), earlier_ask(&["/repo/p1"], 200, false))]);
        assert_eq!(h.claude.live_calls(), 0, "a copy never asks the open ids");
    });
}

// TS: conversations.test.ts:660
#[test]
fn opening_refuses_a_conversation_gone_and_an_agent_with_no_store() {
    local(async {
        let h = host();
        h.claude.earlier_answers(vec![Ok(EarlierRead::Gone)]);
        let got = refused(h.area.opening("ws1", Some("claude".into()), resume(ID, false), None).await);
        same_refusal(
            &got,
            refusal(&conversation_gone_line("Claude Code", ID, "ws1-computer"), CONVERSATION_GONE_FIX, Some(Kind::NotFound)),
        );
        let got = refused(h.area.opening("ws1", Some("codex".into()), resume(ID, false), None).await);
        same_refusal(&got, refusal(&no_conversations_line("Codex"), NO_CONVERSATIONS_FIX, Some(Kind::Usage)));
        h.ctx.install_without_store("codex");
        let got = refused(h.area.opening("ws1", Some("codex".into()), resume(ID, false), None).await);
        same_refusal(&got, refusal(&no_conversations_line("Codex"), NO_CONVERSATIONS_FIX, Some(Kind::Usage)));
    });
}

#[test]
fn opening_says_the_update_a_daemon_too_old_needs() {
    local(async {
        let h = host();
        h.claude.earlier_answers(vec![Err(store_error("unknown op: transcripts.read"))]);
        let got = refused(h.area.opening("ws1", Some("claude".into()), resume(ID, false), None).await);
        same_refusal(
            &got,
            refusal(
                &conversations_unread_line("Claude Code", "ws1-computer"),
                &conversations_update_fix("ws1-computer"),
                Some(Kind::Usage),
            ),
        );
    });
}

// TS: conversations.test.ts:548, :565, :586
#[test]
fn opening_answers_the_earlier_rows() {
    local(async {
        let h = host();
        let mut first = line("person", "Remember the codeword ALPHA.");
        first.at = Some(1_000.0);
        let mut last = line("agent", "ALPHA");
        last.at = Some(3_000.0);
        h.claude
            .earlier_answers(vec![
                Ok(EarlierRead::Read(earlier(Some("/repo/p1-wt"), Some("lab codewords"), vec![first, last], 3))),
                Ok(EarlierRead::Read(earlier(Some("/repo/p1-wt"), None, vec![], 0))),
            ])
            .live_answers(vec![Some(set_of(&[]))]);
        let got = h.area.opening("ws1", Some("claude".into()), resume(ID, false), None).await.expect("an opening");
        assert_eq!((got.harness.as_str(), got.id.as_str(), got.copy, got.project.as_str()), ("claude", ID, false, "p1"));
        assert_eq!(got.cwd, Some("/repo/p1-wt".to_string()));
        assert_eq!(got.title, "lab codewords");
        assert_eq!(
            shown(&got.earlier),
            vec![
                plain("note", &earlier_line(3.0, "Claude Code"), Some(1_000.0)),
                plain("person", "Remember the codeword ALPHA.", Some(1_000.0)),
                plain("agent", "ALPHA", Some(3_000.0)),
            ],
            "each earlier message keeps the time it was written, the count the time of the first"
        );
        assert_eq!(h.claude.earlier_asks(), vec![(ID.to_string(), earlier_ask(&["/repo/p1"], 200, true))]);
        assert_eq!(h.claude.live_calls(), 1);

        let bare = h.area.opening("ws1", Some("claude".into()), resume(OTHER, false), None).await.expect("an opening");
        assert_eq!(bare.title, OTHER, "no title reads as the id");
    });
}

#[test]
fn opening_takes_the_default_agent_where_the_start_names_none() {
    local(async {
        let h = host();
        let codex = FakeConversationStore::new();
        h.ctx.install("codex", codex.clone());
        *h.ctx.default_agent.borrow_mut() = "codex".into();
        codex.earlier_answers(vec![Ok(EarlierRead::Read(codewords()))]);
        let got = h.area.opening("ws1", None, resume(ID, false), None).await.expect("an opening");
        assert_eq!(got.harness, "codex");
        assert_eq!(codex.earlier_asks().len(), 1);
        assert_eq!(h.claude.earlier_asks(), vec![]);
        assert!(h.ctx.called().contains(&"defaultAgentOf".to_string()));

        h.ctx.calls.borrow_mut().clear();
        h.claude.earlier_answers(vec![Ok(EarlierRead::Read(codewords()))]);
        let named = h.area.opening("ws1", Some("claude".into()), resume(OTHER, false), None).await.expect("an opening");
        assert_eq!(named.harness, "claude");
        assert!(!h.ctx.called().contains(&"defaultAgentOf".to_string()), "a named agent is not looked up");
    });
}

// TS: conversations.test.ts:548, :649
#[test]
fn placed_refuses_a_thread_branch_or_cwd_and_places_the_opening() {
    local(async {
        let h = host();
        for beside in [
            PlacedStart { project: Some("p1".into()), branch: Some("feat/x".into()), ..Default::default() },
            PlacedStart { project: Some("p1".into()), cwd: Some("/repo/p1".into()), ..Default::default() },
            PlacedStart { project: Some("p1".into()), thread: Some("th_1".into()), ..Default::default() },
        ] {
            let got = refused(h.area.placed(beside, resume(ID, false), None).await);
            same_refusal(&got, refusal(RESUME_WHERE_LINE, RESUME_WHERE_FIX, Some(Kind::Usage)));
        }
        assert_eq!(h.ctx.called(), Vec::<String>::new(), "refused before anything was asked");

        let tree =
            ws_entry(json!({ "id": "ws2", "project": "p1", "worktree": { "path": "/repo/p1-wt", "branch": "feat/x", "made": true } }));
        h.ctx.folder_answers.borrow_mut().push((
            Some("p1".into()),
            Some("/repo/p1-wt".into()),
            Ok(FolderPlaced { entry: tree, cwd: Some("/repo/p1-wt".into()), setup: None }),
        ));
        h.claude
            .earlier_answers(vec![Ok(EarlierRead::Read(earlier(
                Some("/repo/p1-wt"),
                Some("lab codewords"),
                vec![line("agent", "ALPHA")],
                0,
            )))])
            .live_answers(vec![Some(set_of(&[]))]);
        let at = h
            .area
            .placed(
                PlacedStart { project: Some("p1".into()), harness: Some("claude".into()), ..Default::default() },
                resume(ID, false),
                None,
            )
            .await
            .expect("placed");
        assert_eq!(at.workspace_id, "ws2", "the record of the folder the conversation ran in");
        assert_eq!(at.cwd, Some("/repo/p1-wt".to_string()));
        assert_eq!(at.outside.id, ID);
        assert_eq!(at.outside.harness, "claude");
        assert_eq!(at.outside.cwd, Some("/repo/p1-wt".to_string()));
        assert_eq!(at.outside.project, "p1");
        assert_eq!(
            h.ctx.called().iter().filter(|c| c.starts_with("folderFor") || c.starts_with("entryOf")).cloned().collect::<Vec<_>>(),
            vec!["folderFor p1 ", "entryOf ws1", "folderFor p1 /repo/p1-wt"]
        );
    });
}

#[test]
fn placed_takes_the_starts_workspace_as_its_base_and_places_a_conversation_with_no_folder_in_the_project_folder() {
    local(async {
        let h = host();
        let base = ws_entry(json!({ "id": "ws7", "project": "p1" }));
        h.ctx.add_entry(base);
        h.claude
            .earlier_answers(vec![Ok(EarlierRead::Read(earlier(None, Some("lab codewords"), vec![], 0)))])
            .live_answers(vec![Some(set_of(&[]))]);
        let at = h
            .area
            .placed(
                PlacedStart { workspace_id: Some("ws7".into()), project: Some("p1".into()), ..Default::default() },
                resume(ID, false),
                None,
            )
            .await
            .expect("placed");
        assert_eq!(at.workspace_id, "ws1", "folderFor's answer for the opening's project");
        assert_eq!(at.cwd, None);
        assert_eq!(at.outside.cwd, None);
        assert_eq!(
            h.ctx.called().iter().filter(|c| c.starts_with("folderFor") || c.starts_with("entryOf")).cloned().collect::<Vec<_>>(),
            vec!["entryOf ws7", "folderFor p1 "]
        );
    });
}
