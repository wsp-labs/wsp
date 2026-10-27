// SPDX-License-Identifier: AGPL-3.0-only
//! The second half of packages/runtime/test/folder-threads.test.ts at 65f6b89c7, case for case: where a thread goes once its worktree is
//! gone, what a delete takes with it, a second start on a pull request whose branch a worktree holds, what a thread's own token reaches,
//! the roots file of the daemon's home, and a pull request's head. The copier and the daemon are fakes; git is real. Each test names the
//! TS test above it. The TS `memoryStore()` is the sqlite store here.
mod common;

use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;

use async_trait::async_trait;
use common::daemon::git;
use common::folder_host::*;
use common::local::{Around, AroundBackend, WsClient};
use common::*;
use futures::FutureExt as _;
use serde_json::{json, Value};
use wsp_core::devices::MintMore;
use wsp_core::types::api::{DeleteThreadAsk, ForkOptions, StartFromLinkAsk, StartOptions, WorktreeRemoveAsk};
use wsp_core::types::wiring::{PackedProject, PackedState, ProjectBundler, ProjectImportOptions, StateRequest};
use wsp_serve::serve::{serve_runtime, ServeOptions};
use wsp_wire::types::projects::{WORKTREE_BUSY_LINE, WORKTREE_FORCE_LINE};
use wsp_wire::views::project_bundle::ProjectPlan;
use wsp_wire::views::workspace::Caller;
use wsp_wire::views::workspace_more::{ThreadScope, WorkspaceOrigin};
use wsp_words::exit::Kind;
use wsp_words::projects::{not_made_worktree_line, pr_behind_words, project_in_use_refusal, worktree_changed_line};

fn exists(p: &str) -> bool {
    Path::new(p).exists()
}

fn removed_paths(h: &Here) -> Vec<String> {
    h.copier.lock().unwrap().worktrees_removed.iter().map(|r| r.path.clone()).collect()
}

fn forgotten(h: &Here) -> Vec<String> {
    h.copier.lock().unwrap().worktrees_forgotten.clone()
}

fn of_kind(op: &str, h: &Here) -> Vec<Value> {
    h.frames().into_iter().filter(|f| f["op"] == op).collect()
}

fn moved(events: &[Value]) -> Vec<Value> {
    events.iter().filter(|e| e["type"] == "session.moved").cloned().collect()
}

// ===== a thread whose worktree is gone =====

// "a thread whose worktree is gone" > "runs its next turn in the project folder, its transcript says so, Claude Code keeps its session and another agent opens a fresh one"
#[test]
fn runs_its_next_turn_in_the_project_folder_its_transcript_says_so_claude_code_keeps_its_session_and_another_agent_opens_a_fresh_one() {
    drive(|| async {
        let s = Scratch::new();
        let h = here(&s, HereOpts::default());
        let folder = s.repo();
        let project = add(&h, &folder).await;
        let at = folder_for(&h, ask(&id_of(&project), Some("feat/gone"), None)).await;
        let ws = at.workspace.id.clone();
        let path = ws_json(&at)["worktree"]["path"].as_str().expect("a path").to_string();
        let one = |harness: &str| StartOptions { prompt: "one".into(), harness: Some(harness.into()), ..Default::default() };
        let claude = ran_in(&h, &ws, one("claude")).await;
        let codex = ran_in(&h, &ws, one("codex")).await;
        std::fs::remove_dir_all(&path).expect("the folder goes");
        let (claude_thread, codex_thread) = (thread_of(&claude), thread_of(&codex));
        ran_in(&h, &ws, StartOptions { prompt: "two".into(), thread: Some(claude_thread.clone()), ..Default::default() }).await;
        ran_in(&h, &ws, StartOptions { prompt: "two".into(), thread: Some(codex_thread.clone()), ..Default::default() }).await;
        let starts = h.agents.starts.borrow().clone();
        assert_eq!(starts[2..].iter().map(|s| s.cwd.clone()).collect::<Vec<_>>(), [Some(path_of(&folder)), Some(path_of(&folder))]);
        assert_eq!(starts[2].resume.as_deref(), j(&claude.view())["claudeSessionId"].as_str());
        assert!(starts[3].resume.is_none());
        let found = moved(&history_of(&h, &ws).await);
        assert_eq!(found.len(), 2);
        assert_matches(&found[0], json!({ "threadId": claude_thread, "from": path, "to": path_of(&folder), "branch": "feat/gone" }));
        assert_matches(
            &found[1],
            json!({ "threadId": codex_thread, "from": path, "to": path_of(&folder), "branch": "feat/gone", "fresh": true }),
        );
        assert!(found[0].get("fresh").is_none());
        let got = h.fx.rt.workspaces().get(&ws, None, None).await.expect("the record");
        assert_matches(&j(&got)["worktree"], json!({ "gone": true }));
        h.fx.rt.close().await.expect("a close");
    });
}

// ===== a worktree's folder =====

// "a worktree's folder" > "is the state's own: a runtime handed no state file makes none, so nothing lands under the home it was given"
#[test]
fn is_the_states_own_a_runtime_handed_no_state_file_makes_none_so_nothing_lands_under_the_home_it_was_given() {
    drive(|| async {
        let s = Scratch::new();
        let h = here(&s, HereOpts { unserved: true, ..Default::default() });
        let project = add(&h, &s.repo()).await;
        let refused = h.fx.rt.workspaces().worktree(&id_of(&project), "feat/w", None).await.expect_err("a refusal");
        assert!(refused.message.contains("this runtime was given no state file"), "{}", refused.message);
        assert_eq!(refused.kind, Some(Kind::Invalid));
        assert_eq!(h.copier.lock().unwrap().asks.len(), 0);
        h.fx.rt.close().await.expect("a close");
    });
}

// ===== the images a thread's messages carried =====

// "the images a thread's messages carried" > "go with the thread when it is deleted, and another thread's on the same folder stay"
#[test]
fn go_with_the_thread_when_it_is_deleted_and_another_threads_on_the_same_folder_stay() {
    drive(|| async {
        let s = Scratch::new();
        let h = here(&s, HereOpts::default());
        let project = add(&h, &s.repo()).await;
        let home = folder_for(&h, ask(&id_of(&project), None, None)).await;
        let ws = home.workspace.id.clone();
        use base64::Engine as _;
        let bytes = base64::engine::general_purpose::STANDARD.encode([7u8; 32]);
        let png = || wsp_wire::types::attachments::Attachment {
            media_type: "image/png".into(),
            bytes: bytes.clone(),
            name: Some("shot.png".into()),
        };
        let sent = |text: &str, request: &str| StartOptions {
            prompt: text.into(),
            attachments: Some(vec![png()]),
            request_id: Some(request.into()),
            ..Default::default()
        };
        let one = ran_in(&h, &ws, sent("look", "req_a")).await;
        let two = ran_in(&h, &ws, sent("and this", "req_b")).await;
        let (first, second) = (thread_of(&one), thread_of(&two));
        let image = |thread: String, request: &'static str| {
            let (rt, ws) = (h.fx.rt.clone(), ws.clone());
            async move { rt.sessions().attachment(&ws, &thread, request, 0, None).await }
        };
        assert_eq!(j(&image(first.clone(), "req_a").await.expect("the image")), json!({ "mediaType": "image/png", "bytes": bytes }));
        h.fx.rt.sessions().delete(&first, None, DeleteThreadAsk::default()).await.expect("a delete");
        let gone = image(first, "req_a").await.expect_err("a refusal");
        assert_eq!(gone.kind, Some(Kind::NotFound));
        assert_eq!(j(&image(second, "req_b").await.expect("the image")), json!({ "mediaType": "image/png", "bytes": bytes }));
        h.fx.rt.close().await.expect("a close");
    });
}

// ===== deleting one of two threads in a project's folder =====

fn facts_of(state: &State, ws: &str, session: &str) -> Value {
    let d = rusqlite::Connection::open_with_flags(state.dir.path().join("state.db"), rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
        .expect("state.db opens");
    let json: String = d.query_row("select json from transcript_index where workspace = ?", [ws], |r| r.get(0)).expect("the index row");
    let index: Value = serde_json::from_str(&json).expect("an index");
    index["facts"].as_array().expect("the facts").iter().find(|pair| pair[0] == session).map_or(Value::Null, |pair| pair[1].clone())
}

mod forking {
    use std::cell::Cell;
    use std::rc::Rc;
    use std::sync::Arc;

    use futures::future::LocalBoxFuture;
    use futures::FutureExt as _;
    use serde_json::json;
    use wsp_adapters::port::{
        AdapterError, AttachmentRoad, ForkTurn, ForkedSession, HarnessAdapter, HarnessSession, HarnessStartOptions, SharedFuture,
        TurnResult,
    };
    use wsp_core::types::harness::HarnessAdapterFactory;

    use super::common::adapter::{event, result};

    /// A harness that forks to one fixed copy and says its model and a reply at each turn.
    pub fn factory() -> HarnessAdapterFactory {
        let n = Rc::new(Cell::new(0u32));
        Arc::new(move |_ctx| Ok(Arc::new(Forking { n: n.clone() }) as Arc<dyn HarnessAdapter>))
    }

    struct Forking {
        n: Rc<Cell<u32>>,
    }

    impl HarnessAdapter for Forking {
        fn steers(&self) -> bool {
            false
        }
        fn attachments(&self) -> Option<AttachmentRoad> {
            Some(AttachmentRoad::Inline)
        }
        fn forks(&self) -> bool {
            true
        }
        fn fork_session(&self, _session: String, _turn: ForkTurn) -> Option<LocalBoxFuture<'static, Result<ForkedSession, String>>> {
            Some(
                async {
                    Ok(ForkedSession::Resume {
                        resume: "c0c0c0c0-0000-4000-8000-000000000001".into(),
                        drop: Box::new(|| async { Ok(()) }.boxed_local()),
                    })
                }
                .boxed_local(),
            )
        }
        fn start(&self, o: HarnessStartOptions) -> Result<Arc<dyn HarnessSession>, AdapterError> {
            self.n.set(self.n.get() + 1);
            let n = self.n.get();
            let session = o.resume.clone().unwrap_or_else(|| format!("50505050-0000-4000-8000-{n:012}"));
            let done = result(json!({ "status": "completed", "text": "reply" }));
            let on_event = o.on_event.clone().expect("an event sink");
            let model = o.model.clone().unwrap_or_else(|| "default-model".into());
            let finished: LocalBoxFuture<'static, TurnResult> = {
                let (done, session) = (done.clone(), session.clone());
                async move {
                    tokio::task::yield_now().await;
                    on_event(event(json!({ "type": "session.start", "sessionId": session, "model": model })));
                    on_event(event(json!({ "type": "turn.delta", "sessionId": session, "kind": "text", "text": "reply" })));
                    on_event(event(json!({ "type": "turn.anchor", "sessionId": session, "anchor": format!("a{n}") })));
                    on_event(event(
                        json!({ "type": "turn.done", "sessionId": session, "result": serde_json::to_value(&done).expect("a result") }),
                    ));
                    on_event(event(json!({ "type": "session.end", "sessionId": session, "exitCode": 0, "sawResult": true })));
                    done
                }
                .boxed_local()
            };
            Ok(Arc::new(Run { local_id: session, finished: finished.shared() }))
        }
    }

    struct Run {
        local_id: String,
        finished: SharedFuture<TurnResult>,
    }

    impl HarnessSession for Run {
        fn local_id(&self) -> String {
            self.local_id.clone()
        }
        fn finished(&self) -> SharedFuture<TurnResult> {
            self.finished.clone()
        }
        fn interrupt(&self) -> LocalBoxFuture<'static, ()> {
            async {}.boxed_local()
        }
    }
}

// "deleting one of two threads in a project's folder" > "deleting a fork keeps its source thread's session facts"
#[test]
fn deleting_a_fork_keeps_its_source_threads_session_facts() {
    drive(|| async {
        let s = Scratch::new();
        let h = here(&s, HereOpts { adapters: vec![("claude", forking::factory())], ..Default::default() });
        let project = add(&h, &s.repo()).await;
        let home = folder_for(&h, ask(&id_of(&project), None, None)).await;
        let ws = home.workspace.id.clone();
        let first = ran_in(
            &h,
            &ws,
            StartOptions {
                prompt: "Remember ALPHA.".into(),
                harness: Some("claude".into()),
                model: Some("claude-opus-5".into()),
                ..Default::default()
            },
        )
        .await;
        sleep_ms(300).await;
        let fork =
            h.fx.rt
                .sessions()
                .fork(
                    ForkOptions {
                        start: StartOptions { prompt: "what do you remember".into(), ..Default::default() },
                        fork: wsp_wire::views::session::ForkSource { thread_id: thread_of(&first), turn_id: None, at: None },
                        branch: None,
                    },
                    None,
                )
                .await
                .expect("a fork");
        fork.finished().await;
        sleep_ms(300).await;
        let session = j(&first.view())["claudeSessionId"].as_str().expect("a session").to_string();
        let before = facts_of(&h.state, &ws, &session);
        assert_matches(&before, json!({ "model": "claude-opus-5" }));
        h.fx.rt.sessions().delete(&thread_of(&fork), None, DeleteThreadAsk::default()).await.expect("a delete");
        sleep_ms(300).await;
        assert_eq!(facts_of(&h.state, &ws, &session), before);
        h.fx.rt.close().await.expect("a close");
    });
}

// "deleting one of two threads in a project's folder" > "drops that thread's rows alone, never reading the folder's, and the other thread keeps its events"
#[test]
fn drops_that_threads_rows_alone_never_reading_the_folders_and_the_other_thread_keeps_its_events() {
    drive(|| async {
        let s = Scratch::new();
        let state = Rc::new(sqlite_state());
        let watched = common::watched::Watched::over(&state.store);
        let h = here(
            &s,
            HereOpts { again: Some(Again { root: s.dir(), state: state.clone() }), store: Some(watched.store()), ..Default::default() },
        );
        let project = add(&h, &s.repo()).await;
        let home = folder_for(&h, ask(&id_of(&project), None, None)).await;
        let ws = home.workspace.id.clone();
        let one = ran(&h, &ws, "first thread").await;
        let two = ran(&h, &ws, "second thread").await;
        let (first, second) = (thread_of(&one), thread_of(&two));
        // Each turn's checkpoint lands after its end.
        sleep_ms(50).await;
        let page = |thread: String| {
            let (rt, ws) = (h.fx.rt.clone(), ws.clone());
            async move {
                let asked = wsp_core::types::api::HistoryAsk { thread_id: Some(thread), ..Default::default() };
                rt.sessions().thread_history(&ws, asked, None).await.expect("a page").iter().map(j).collect::<Vec<_>>()
            }
        };
        let kept = page(second.clone()).await;
        watched.rows.folder_reads.set(0);
        h.fx.rt.sessions().delete(&first, None, DeleteThreadAsk::default()).await.expect("a delete");
        assert_eq!(watched.rows.folder_reads.get(), 0);
        assert!(page(first.clone()).await.is_empty());
        assert_eq!(page(second.clone()).await, kept);
        let hits = |query: &'static str| {
            let rt = h.fx.rt.clone();
            async move { j(&rt.sessions().search(query, None).await.expect("a search"))["hits"].as_array().expect("hits").clone() }
        };
        assert!(hits("first thread").await.is_empty());
        assert_eq!(hits("second thread").await.iter().map(|h| h["threadId"].clone()).collect::<Vec<_>>(), [json!(second)]);
        h.fx.rt.close().await.expect("a close");
    });
}

// ===== removing a project whose folders hold no thread =====

// "removing a project whose folders hold no thread" > "takes the records no thread names with it, a worktree wsp made and the frozen copies its worktrees sat on too, and is refused while a thread stands"
#[test]
fn takes_the_records_no_thread_names_with_it_a_worktree_wsp_made_and_the_frozen_copies_its_worktrees_sat_on_too_and_is_refused_while_a_thread_stands(
) {
    drive(|| async {
        let s = Scratch::new();
        let h = here(&s, HereOpts::default());
        let folder = s.repo();
        let project = add(&h, &folder).await;
        let id = id_of(&project);
        let home = folder_for(&h, ask(&id, None, None)).await;
        let run = ran(&h, &home.workspace.id, "one").await;
        let tree = h.fx.rt.workspaces().worktree(&id, "feat/w", None).await.expect("a worktree");
        let refused = h.fx.rt.projects().remove(&id, None, None, None).await.expect_err("a refusal");
        let home_name = ws_json(&home)["name"].as_str().expect("a name").to_string();
        assert_eq!(refused.message, project_in_use_refusal(project["name"].as_str().expect("a name"), &[home_name]));
        assert_eq!(refused.kind, Some(Kind::Conflict));
        assert!(forgotten(&h).is_empty());
        h.fx.rt.sessions().delete(&thread_of(&run), None, DeleteThreadAsk::default()).await.expect("a delete");
        h.fx.rt.projects().remove(&id, None, None, None).await.expect("a remove");
        assert_eq!(forgotten(&h), std::slice::from_ref(&id));
        assert!(h.fx.rt.projects().list(None).await.expect("a list").is_empty());
        assert!(workspaces_of(&h).await.is_empty());
        assert_eq!(removed_paths(&h), std::slice::from_ref(&tree.path));
        assert!(folder.join(".git").exists());
        h.fx.rt.close().await.expect("a close");
    });
}

// ===== taking a folder's record away =====

// "taking a folder's record away" > "never touches the project folder, and drops its threads' checkpoints"
#[test]
fn never_touches_the_project_folder_and_drops_its_threads_checkpoints() {
    drive(|| async {
        let s = Scratch::new();
        let h = here(&s, HereOpts::default());
        let folder = s.repo();
        let project = add(&h, &folder).await;
        let at = folder_for(&h, ask(&id_of(&project), None, None)).await;
        let run = ran(&h, &at.workspace.id, "one").await;
        h.fx.rt.workspaces().delete(&at.workspace.id, None).await.expect("a delete");
        assert!(folder.join(".git").exists());
        assert!(removed_paths(&h).is_empty());
        assert_eq!(
            of_kind("git.checkpointDrop", &h),
            [json!({ "op": "git.checkpointDrop", "cwd": path_of(&folder), "scope": at.workspace.id, "thread": thread_of(&run) })]
        );
        h.fx.rt.close().await.expect("a close");
    });
}

// "taking a folder's record away" > "refuses a worktree wsp made while it holds a file no commit has, and removes it once clean"
#[test]
fn refuses_a_worktree_wsp_made_while_it_holds_a_file_no_commit_has_and_removes_it_once_clean() {
    drive(|| async {
        let s = Scratch::new();
        let h = here(&s, HereOpts::default());
        let folder = s.repo();
        let project = add(&h, &folder).await;
        let id = id_of(&project);
        let at = folder_for(&h, ask(&id, Some("feat/keep"), None)).await;
        let path = ws_json(&at)["worktree"]["path"].as_str().expect("a path").to_string();
        std::fs::write(Path::new(&path).join("notes.md"), "mine\n").expect("a file");
        let refused = h.fx.rt.workspaces().delete(&at.workspace.id, None).await.expect_err("a refusal");
        assert!(refused.message.contains(&worktree_changed_line(1.0)), "{}", refused.message);
        let by_verb =
            h.fx.rt
                .workspaces()
                .worktree_remove(WorktreeRemoveAsk { project: id.clone(), branch: "feat/keep".into(), ..Default::default() }, None)
                .await
                .expect_err("a refusal");
        assert!(by_verb.message.contains(&worktree_changed_line(1.0)), "{}", by_verb.message);
        assert!(Path::new(&path).join("notes.md").exists());
        std::fs::remove_file(Path::new(&path).join("notes.md")).expect("the file goes");
        h.fx.rt.workspaces().delete(&at.workspace.id, None).await.expect("a delete");
        assert!(!exists(&path));
        assert!(git(&folder, &["branch", "--list", "feat/keep"]).contains("feat/keep"));
        let asked = h.copier.lock().unwrap().worktrees_removed.clone();
        assert_eq!(asked.len(), 1);
        assert_eq!(
            (asked[0].from.as_str(), asked[0].home.as_str(), asked[0].path.as_str(), asked[0].force),
            (path_of(&folder).as_str(), path_of(&h.home).as_str(), path.as_str(), false)
        );
        h.fx.rt.close().await.expect("a close");
    });
}

// "taking a folder's record away" > "never removes a worktree somebody else made, whatever is asked of it"
#[test]
fn never_removes_a_worktree_somebody_else_made_whatever_is_asked_of_it() {
    drive(|| async {
        let s = Scratch::new();
        let h = here(&s, HereOpts::default());
        let folder = s.repo();
        let project = add(&h, &folder).await;
        let id = id_of(&project);
        let theirs = s.dir().join("theirs");
        git(&folder, &["worktree", "add", "-q", "-b", "theirs", &path_of(&theirs)]);
        let held = folder_for(&h, ask(&id, Some("theirs"), None)).await;
        assert_eq!(ws_json(&held)["worktree"], json!({ "path": path_of(&theirs), "branch": "theirs", "made": false }));
        let refused =
            h.fx.rt
                .workspaces()
                .worktree_remove(WorktreeRemoveAsk { project: id.clone(), branch: "theirs".into(), ..Default::default() }, None)
                .await
                .expect_err("a refusal");
        assert!(refused.message.contains(&not_made_worktree_line("theirs")), "{}", refused.message);
        h.fx.rt.workspaces().delete(&held.workspace.id, None).await.expect("a delete");
        assert!(theirs.exists());
        assert!(removed_paths(&h).is_empty());
        h.fx.rt.close().await.expect("a close");
    });
}

// ===== a second start on a pull request whose branch a worktree holds =====

fn in_roots(roots: &Path, cwd: &str) -> bool {
    roots.exists()
        && std::fs::read_to_string(roots)
            .expect("the roots file")
            .split('\n')
            .any(|root| !root.is_empty() && (cwd == root || cwd.starts_with(&format!("{root}/"))))
}

const GIT_ENV: [(&str, &str); 4] = [
    ("GIT_AUTHOR_NAME", "t"),
    ("GIT_AUTHOR_EMAIL", "t@example.com"),
    ("GIT_COMMITTER_NAME", "t"),
    ("GIT_COMMITTER_EMAIL", "t@example.com"),
];

fn try_git(cwd: &str, args: &[&str]) -> Result<String, String> {
    let out = Command::new("git").arg("-C").arg(cwd).args(args).envs(GIT_ENV).output().map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).into_owned())
    }
}

/// A project whose origin is a GitHub repository, served from a bare repo here by this computer's daemon fake.
struct GitHubHere {
    h: Here,
    scratch: Rc<Scratch>,
    folder: PathBuf,
    author: PathBuf,
    project: Value,
    url: String,
    bare: PathBuf,
}

async fn on_github_here(sub: Option<&str>) -> GitHubHere {
    let s = Scratch::new();
    let h = here(&s, HereOpts::default());
    let root = s.dir();
    let bare = root.join("spoo.git");
    let folder = root.join("spoo");
    let plain = |args: &[&str]| assert!(Command::new("git").args(args).status().expect("git").success(), "git {args:?}");
    plain(&["init", "-q", "--bare", "-b", "main", &path_of(&bare)]);
    plain(&["clone", "-q", &path_of(&bare), &path_of(&folder)]);
    git(&folder, &["commit", "-q", "--allow-empty", "-m", "first"]);
    git(&folder, &["push", "-q", "origin", "main"]);
    let author = root.join("author");
    plain(&["clone", "-q", &path_of(&bare), &path_of(&author)]);
    git(&author, &["checkout", "-q", "-b", "feat/pr"]);
    git(&author, &["commit", "-q", "--allow-empty", "-m", "one"]);
    git(&author, &["push", "-q", "origin", "feat/pr"]);
    if let Some(sub) = sub {
        std::fs::create_dir_all(folder.join(sub)).expect("a folder");
        std::fs::write(folder.join(sub).join("README.md"), "sub\n").expect("a file");
        git(&folder, &["add", "-A"]);
        git(&folder, &["commit", "-q", "-m", "sub"]);
        git(&folder, &["push", "-q", "origin", "main"]);
        git(&author, &["pull", "-q", "--rebase", "origin", "main"]);
        git(&author, &["push", "-q", "-f", "origin", "feat/pr"]);
    }
    git(&folder, &["remote", "set-url", "origin", "https://github.com/dev/spoo.git"]);
    let url = "https://github.com/dev/spoo/pull/7".to_string();
    let fact = json!({
        "number": 7, "url": url, "state": "open", "host": "github.com", "draft": false, "base": "main", "branch": "feat/pr",
        "headOid": "a".repeat(40), "headSubject": "one", "mergeable": "mergeable", "mergeState": "clean", "review": "none", "checks": [],
        "additions": 0, "deletions": 0, "changedFiles": 0, "commits": 1,
    });
    h.daemon.answer("git.prRead", move |_| json!({ "id": 1, "ok": true, "pr": fact }));
    let issue_url = url.clone();
    h.daemon.answer("git.issueRead", move |_| {
        json!({ "id": 1, "ok": true, "issue": { "number": 7, "url": issue_url, "title": "one", "body": "", "state": "OPEN", "comments": [] } })
    });
    let roots = h.roots.clone();
    let outside = |cwd: &str| json!({ "id": 1, "ok": false, "code": "outside-root", "error": format!("{cwd} resolves outside the folders wsp serves here") });
    h.daemon.answer("git.fetchBranch", {
        let roots = roots.clone();
        move |f| {
            let text = |key: &str| f.get(key).and_then(Value::as_str).map(str::to_string);
            let cwd = text("cwd").unwrap_or_else(|| "undefined".into());
            if !in_roots(&roots, &cwd) {
                return outside(&cwd);
            }
            let branch = text("branch").unwrap_or_default();
            let into = text("into").unwrap_or_else(|| branch.clone());
            let remote = text("remote").unwrap_or_default();
            let fetched = try_git(&cwd, &["fetch", "-q", "--no-tags", &remote, &format!("refs/heads/{branch}:refs/heads/{into}")]);
            match fetched {
                Err(e) => json!({ "id": 1, "ok": false, "error": e }),
                Ok(_) => json!({ "id": 1, "ok": true, "branch": into, "oid": try_git(&cwd, &["rev-parse", &format!("refs/heads/{into}")]).unwrap_or_default() }),
            }
        }
    });
    h.daemon.answer("git.update", move |f| {
        let cwd = f.get("cwd").and_then(Value::as_str).unwrap_or("undefined").to_string();
        let outside =
            json!({ "id": 1, "ok": false, "code": "outside-root", "error": format!("{cwd} resolves outside the folders wsp serves here") });
        if !in_roots(&roots, &cwd) {
            return outside;
        }
        let base = f.get("base").and_then(Value::as_str).unwrap_or_default().to_string();
        let merged = try_git(&cwd, &["fetch", "-q", "--no-tags", "origin", &format!("refs/heads/{base}")])
            .and_then(|_| try_git(&cwd, &["merge", "-q", "--ff-only", "FETCH_HEAD"]));
        match merged {
            Err(e) => json!({ "id": 1, "ok": false, "error": e }),
            Ok(_) => json!({ "id": 1, "ok": true, "base": base, "merged": true, "commits": 1, "conflicts": [] }),
        }
    });
    let source = match sub {
        None => folder.clone(),
        Some(sub) => folder.join(sub),
    };
    let project = add(&h, &source).await;
    // Once the project holds its GitHub address, that address reads the bare repo here, for this computer's git and the daemon fake's alike.
    git(&folder, &["config", &format!("url.{}.insteadOf", path_of(&bare)), "https://github.com/dev/spoo.git"]);
    GitHubHere { h, scratch: s, folder, author, project, url, bare }
}

async fn start_pr(g: &GitHubHere) -> Value {
    let ask = StartFromLinkAsk { url: g.url.clone(), agent: Some("claude".into()), ..Default::default() };
    j(&g.h.fx.rt.workspaces().start(ask, None).await.expect("a start"))
}

fn push_two(g: &GitHubHere) {
    git(&g.author, &["commit", "-q", "--allow-empty", "-m", "two"]);
    git(&g.author, &["push", "-q", "origin", "feat/pr"]);
}

/// The first start on the pull request, its worktree's path, and the author's second commit pushed.
async fn held() -> (GitHubHere, Value, String) {
    let g = on_github_here(None).await;
    let first = start_pr(&g).await;
    let path = first["workspace"]["worktree"]["path"].as_str().expect("a path").to_string();
    push_two(&g);
    (g, first, path)
}

// "a second start on a pull request whose branch a worktree holds" > "runs in that worktree, brought up to the pull request first where it is clean and only behind"
#[test]
fn runs_in_that_worktree_brought_up_to_the_pull_request_first_where_it_is_clean_and_only_behind() {
    drive(|| async {
        let (g, first, path) = held().await;
        let second = start_pr(&g).await;
        assert_eq!(second["workspace"]["id"], first["workspace"]["id"]);
        assert_eq!(git(Path::new(&path), &["rev-parse", "HEAD"]), git(&g.author, &["rev-parse", "HEAD"]));
        assert!(behind_lines(&g.h, first["workspace"]["id"].as_str().expect("an id"), None).await.is_empty());
        g.h.fx.rt.close().await.expect("a close");
    });
}

// "a second start on a pull request whose branch a worktree holds" > "runs there as it stands where it holds files no commit has, and the thread says how to update"
#[test]
fn runs_there_as_it_stands_where_it_holds_files_no_commit_has_and_the_thread_says_how_to_update() {
    drive(|| async {
        let (g, first, path) = held().await;
        std::fs::write(Path::new(&path).join("draft.md"), "mine\n").expect("a file");
        let was = git(Path::new(&path), &["rev-parse", "HEAD"]);
        let second = start_pr(&g).await;
        assert_eq!(second["workspace"]["id"], first["workspace"]["id"]);
        assert_eq!(git(Path::new(&path), &["rev-parse", "HEAD"]), was);
        assert_ne!(git(Path::new(&path), &["rev-parse", "HEAD"]), git(&g.author, &["rev-parse", "HEAD"]));
        assert_eq!(
            behind_lines(&g.h, first["workspace"]["id"].as_str().expect("an id"), None).await,
            [pr_behind_words::changed(7.0, &path)]
        );
        g.h.fx.rt.close().await.expect("a close");
    });
}

// "a second start on a pull request whose branch a worktree holds" > "runs in a worktree removed by hand and made again, brought up to the pull request"
#[test]
fn runs_in_a_worktree_removed_by_hand_and_made_again_brought_up_to_the_pull_request() {
    drive(|| async {
        let (g, first, path) = held().await;
        assert!(path.starts_with(&std::env::temp_dir().display().to_string()) || path.starts_with("/private"));
        std::fs::remove_dir_all(&path).expect("the folder goes");
        let second = start_pr(&g).await;
        assert_eq!(second["workspace"]["id"], first["workspace"]["id"]);
        assert_eq!(git(Path::new(&path), &["rev-parse", "HEAD"]), git(&g.author, &["rev-parse", "HEAD"]));
        assert!(behind_lines(&g.h, first["workspace"]["id"].as_str().expect("an id"), None).await.is_empty());
        g.h.fx.rt.close().await.expect("a close");
    });
}

// "a second start on a pull request whose branch a worktree holds" > "tells a thread of a project in a subfolder that its worktree was left behind"
#[test]
fn tells_a_thread_of_a_project_in_a_subfolder_that_its_worktree_was_left_behind() {
    drive(|| async {
        let g = on_github_here(Some("apps/web")).await;
        let first = start_pr(&g).await;
        let path = first["workspace"]["worktree"]["path"].as_str().expect("a path").to_string();
        push_two(&g);
        std::fs::write(Path::new(&path).join("draft.md"), "mine\n").expect("a file");
        start_pr(&g).await;
        assert_eq!(
            behind_lines(&g.h, first["workspace"]["id"].as_str().expect("an id"), None).await,
            [pr_behind_words::changed(7.0, &path)]
        );
        g.h.fx.rt.close().await.expect("a close");
    });
}

// "a second start on a pull request whose branch a worktree holds" > "never moves the project folder when it holds the branch: the thread runs there as it stands, told it is behind"
#[test]
fn never_moves_the_project_folder_when_it_holds_the_branch_the_thread_runs_there_as_it_stands_told_it_is_behind() {
    drive(|| async {
        let g = on_github_here(None).await;
        git(&g.folder, &["fetch", "-q", &path_of(&g.bare), "feat/pr:feat/pr"]);
        git(&g.folder, &["checkout", "-q", "feat/pr"]);
        push_two(&g);
        let was = git(&g.folder, &["rev-parse", "HEAD"]);
        let started = start_pr(&g).await;
        assert!(started["workspace"]["worktree"].is_null());
        assert_eq!(git(&g.folder, &["rev-parse", "HEAD"]), was);
        assert!(of_kind("git.update", &g.h).is_empty());
        let id = started["workspace"]["id"].as_str().expect("an id");
        assert_eq!(behind_lines(&g.h, id, None).await, [pr_behind_words::folder(7.0, &path_of(&g.folder))]);
        g.h.fx.rt.close().await.expect("a close");
    });
}

// "a second start on a pull request whose branch a worktree holds" > "runs there as it stands where its branch went another way, and the thread says how to bring them together"
#[test]
fn runs_there_as_it_stands_where_its_branch_went_another_way_and_the_thread_says_how_to_bring_them_together() {
    drive(|| async {
        let (g, first, path) = held().await;
        git(Path::new(&path), &["commit", "-q", "--allow-empty", "-m", "mine"]);
        let was = git(Path::new(&path), &["rev-parse", "HEAD"]);
        let second = start_pr(&g).await;
        assert_eq!(second["workspace"]["id"], first["workspace"]["id"]);
        assert_eq!(git(Path::new(&path), &["rev-parse", "HEAD"]), was);
        assert_eq!(
            behind_lines(&g.h, first["workspace"]["id"].as_str().expect("an id"), None).await,
            [pr_behind_words::diverged(7.0, &path)]
        );
        g.h.fx.rt.close().await.expect("a close");
    });
}

// ===== what a thread's own token reaches on this computer =====

fn thread_scope(thread: &str, workspace: &str) -> ThreadScope {
    serde_json::from_value(json!({ "kind": "thread", "threadId": thread, "workspaceId": workspace, "rootThreadId": thread }))
        .expect("a scope")
}

/// A thread on this computer, as its turn's own token reaches the host over this computer's road.
fn as_thread(scope: &ThreadScope) -> Option<Caller> {
    Some(Caller::Scoped { origin: WorkspaceOrigin::Here, by: scope.clone() })
}

/// A person's thread in the project folder, a worktree the person made beside it, and the scope the thread's token carries.
struct Lead {
    h: Here,
    _scratch: Rc<Scratch>,
    project: Value,
    theirs: Value,
    home: Value,
    scope: ThreadScope,
}

async fn lead() -> Lead {
    let s = Scratch::new();
    let h = here(&s, HereOpts::default());
    let project = add(&h, &s.repo()).await;
    let id = id_of(&project);
    let theirs = folder_for(&h, ask(&id, Some("feat/theirs"), None)).await;
    let home = folder_for(&h, ask(&id, None, None)).await;
    let run = ran(&h, &home.workspace.id, "lead").await;
    let scope = thread_scope(&thread_of(&run), &home.workspace.id);
    Lead { h, _scratch: s, project, theirs: ws_json(&theirs), home: ws_json(&home), scope }
}

// "what a thread's own token reaches on this computer" > "reaches the folder it runs in and the worktrees made for its tree, and never another tree's worktree"
#[test]
fn reaches_the_folder_it_runs_in_and_the_worktrees_made_for_its_tree_and_never_another_trees_worktree() {
    drive(|| async {
        let l = lead().await;
        let (rt, id) = (&l.h.fx.rt, id_of(&l.project));
        rt.workspaces().checkout(&id_of(&l.home), as_thread(&l.scope), None).await.expect("the folder it runs in");
        let child = rt.workspaces().folder_for(ask(&id, Some("feat/mine"), None), as_thread(&l.scope)).await.expect("a folder");
        let child_id = child.workspace.id.clone();
        let ran = rt
            .sessions()
            .start(&child_id, StartOptions { prompt: "child".into(), ..Default::default() }, as_thread(&l.scope))
            .await
            .expect("a start");
        ran.finished().await;
        rt.workspaces().checkout(&child_id, as_thread(&l.scope), None).await.expect("a worktree of its tree");
        let got = rt.workspaces().get(&child_id, None, None).await.expect("the record");
        assert_eq!(j(&got)["worktree"]["madeFor"], l.scope.root_thread_id.as_str());
        // A thread working in a worktree opens threads in the project folder every thread shares, and drives nothing there.
        let theirs_id = id_of(&l.theirs);
        let side = ran_in(&l.h, &theirs_id, StartOptions { prompt: "the person's, on a branch".into(), ..Default::default() }).await;
        let there = thread_scope(&thread_of(&side), &theirs_id);
        assert!(rt.workspaces().checkout(&id_of(&l.home), as_thread(&there), None).await.is_err());
        rt.sessions()
            .start(&id_of(&l.home), StartOptions { prompt: "in the folder".into(), ..Default::default() }, as_thread(&there))
            .await
            .expect("a start")
            .finished()
            .await;
        assert!(rt.workspaces().checkout(&theirs_id, as_thread(&l.scope), None).await.is_err());
        assert!(rt
            .sessions()
            .start(&theirs_id, StartOptions { prompt: "theirs".into(), ..Default::default() }, as_thread(&l.scope))
            .await
            .is_err());
        let remove = WorktreeRemoveAsk { project: id.clone(), branch: "feat/theirs".into(), ..Default::default() };
        assert!(rt.workspaces().worktree_remove(remove, as_thread(&l.scope)).await.is_err());
        assert!(exists(l.theirs["worktree"]["path"].as_str().expect("a path")));
        rt.close().await.expect("a close");
    });
}

// "what a thread's own token reaches on this computer" > "hears its folder's tree name only the worktrees made for its own tree, never another thread's of the same folder"
#[test]
fn hears_its_folders_tree_name_only_the_worktrees_made_for_its_own_tree_never_another_threads_of_the_same_folder() {
    drive(|| async {
        let l = lead().await;
        let (rt, id, home_id) = (&l.h.fx.rt, id_of(&l.project), id_of(&l.home));
        let other = ran(&l.h, &home_id, "another thread in the folder").await;
        let theirs = thread_scope(&thread_of(&other), &home_id);
        let served = serve_runtime(
            rt.clone(),
            ServeOptions { port: Some(0), auth_token: "secret".into(), devices: Some(rt.devices()), ..Default::default() },
        )
        .await
        .expect("the runtime serves");
        let minted = rt
            .devices()
            .mint("thread a", l.scope.clone(), common::now_ms(), MintMore { road: Some(WorkspaceOrigin::Here), ..Default::default() })
            .await
            .expect("a device");
        let mut mine = WsClient::connect(served.port(), &minted.device_token).await;
        assert_eq!(mine.request("events.subscribe", json!({})).await["ok"], true);
        let child = |by: ThreadScope, branch: &'static str| {
            let (rt, id) = (rt.clone(), id.clone());
            let l = &l;
            async move {
                let made = rt.workspaces().folder_for(ask(&id, Some(branch), None), as_thread(&by)).await.expect("a folder");
                let started = rt
                    .sessions()
                    .start(&made.workspace.id, StartOptions { prompt: branch.into(), ..Default::default() }, as_thread(&by))
                    .await
                    .expect("a start");
                started.finished().await;
                let _ = l;
                made.workspace.id
            }
        };
        let ours = child(l.scope.clone(), "feat/ours").await;
        child(theirs, "feat/not-ours").await;
        let events = mine.events.clone();
        let tree = || {
            events
                .borrow()
                .iter()
                .rfind(|e| e["type"] == "workspace.status" && e["status"]["id"] == home_id.as_str())
                .map(|e| e["status"].clone())
        };
        until(|| async { tree().is_some_and(|s| !s["tree"].is_null()) }).await;
        sleep_ms(300).await;
        let kids: Vec<Value> =
            tree().expect("a status")["tree"]["children"].as_array().expect("children").iter().map(|c| c["workspaceId"].clone()).collect();
        assert_eq!(kids, [json!(ours)]);
        mine.close();
        served.close().await.expect("a close");
        rt.close().await.expect("a close");
    });
}

// "what a thread's own token reaches on this computer" > "never removes a worktree over files no commit holds, which only the person may force"
#[test]
fn never_removes_a_worktree_over_files_no_commit_holds_which_only_the_person_may_force() {
    drive(|| async {
        let l = lead().await;
        let (rt, id) = (&l.h.fx.rt, id_of(&l.project));
        let child = rt.workspaces().folder_for(ask(&id, Some("feat/mine"), None), as_thread(&l.scope)).await.expect("a folder");
        rt.sessions()
            .start(&child.workspace.id, StartOptions { prompt: "child".into(), ..Default::default() }, as_thread(&l.scope))
            .await
            .expect("a start")
            .finished()
            .await;
        let path = ws_json(&child)["worktree"]["path"].as_str().expect("a path").to_string();
        std::fs::write(Path::new(&path).join("draft.md"), "mine\n").expect("a file");
        let remove = |force: Option<bool>, by: Option<Caller>| {
            let (rt, id) = (rt.clone(), id.clone());
            async move {
                rt.workspaces().worktree_remove(WorktreeRemoveAsk { project: id, branch: "feat/mine".into(), force, check: None }, by).await
            }
        };
        let forced = remove(Some(true), as_thread(&l.scope)).await.expect_err("a refusal");
        assert!(forced.message.contains(WORKTREE_FORCE_LINE), "{}", forced.message);
        let plain = remove(None, as_thread(&l.scope)).await.expect_err("a refusal");
        assert!(plain.message.contains(&worktree_changed_line(1.0)), "{}", plain.message);
        assert!(Path::new(&path).join("draft.md").exists());
        remove(Some(true), None).await.expect("the person forces it");
        assert!(!exists(&path));
        rt.close().await.expect("a close");
    });
}

// ===== the worktrees a thread asks for itself =====

async fn lead_alone() -> (Here, Rc<Scratch>, Value, PathBuf, ThreadScope) {
    let s = Scratch::new();
    let h = here(&s, HereOpts::default());
    let folder = s.repo();
    let project = add(&h, &folder).await;
    let home = folder_for(&h, ask(&id_of(&project), None, None)).await;
    let run = ran(&h, &home.workspace.id, "lead").await;
    let scope = thread_scope(&thread_of(&run), &home.workspace.id);
    (h, s, project, folder, scope)
}

// "the worktrees a thread asks for itself" > "are its tree's when it asks wsp worktree for one, so a start by its path or its branch runs there"
#[test]
fn are_its_trees_when_it_asks_wsp_worktree_for_one_so_a_start_by_its_path_or_its_branch_runs_there() {
    drive(|| async {
        let (h, _s, project, _folder, scope) = lead_alone().await;
        let (rt, id) = (&h.fx.rt, id_of(&project));
        let made = rt.workspaces().worktree(&id, "feat/mine", as_thread(&scope)).await.expect("a worktree");
        let record = workspaces_of(&h).await.into_iter().find(|w| w["worktree"]["path"] == made.path.as_str()).expect("the record");
        assert_eq!(record["worktree"]["madeFor"], scope.root_thread_id.as_str());
        let by_path = rt.workspaces().folder_for(ask(&id, None, Some(Path::new(&made.path))), as_thread(&scope)).await.expect("a folder");
        assert_eq!(by_path.workspace.id, id_of(&record));
        let by = StartOptions { prompt: "by path".into(), cwd: Some(made.path.clone()), ..Default::default() };
        rt.sessions().start(&by_path.workspace.id, by, as_thread(&scope)).await.expect("a start").finished().await;
        let by_branch = rt.workspaces().folder_for(ask(&id, Some("feat/mine"), None), as_thread(&scope)).await.expect("a folder");
        assert_eq!(by_branch.workspace.id, id_of(&record));
        rt.sessions().start(&by_branch.workspace.id, prompt("by branch"), as_thread(&scope)).await.expect("a start").finished().await;
        assert_eq!(h.agents.cwds()[1..], [Some(made.path.clone()), Some(made.path.clone())]);
        rt.close().await.expect("a close");
    });
}

// "the worktrees a thread asks for itself" > "are its tree's when it made one with plain git worktree add and starts a thread there by its path"
#[test]
fn are_its_trees_when_it_made_one_with_plain_git_worktree_add_and_starts_a_thread_there_by_its_path() {
    drive(|| async {
        let (h, s, project, _folder, scope) = lead_alone().await;
        let (rt, id) = (&h.fx.rt, id_of(&project));
        let plain = s.dir().join("plain");
        git(Path::new(project["path"].as_str().expect("a path")), &["worktree", "add", "-q", "-b", "feat/plain", &path_of(&plain)]);
        let at = rt.workspaces().folder_for(ask(&id, None, Some(&plain)), as_thread(&scope)).await.expect("a folder");
        assert_matches(&ws_json(&at)["worktree"], json!({ "path": path_of(&plain), "made": false, "madeFor": scope.root_thread_id }));
        let own = StartOptions { prompt: "in my own".into(), cwd: Some(path_of(&plain)), ..Default::default() };
        rt.sessions().start(&at.workspace.id, own, as_thread(&scope)).await.expect("a start").finished().await;
        assert_eq!(h.agents.cwds().last().cloned().flatten(), Some(path_of(&plain)));
        rt.close().await.expect("a close");
    });
}

// "the worktrees a thread asks for itself" > "stay its tree's when one is removed by hand and made again"
#[test]
fn stay_its_trees_when_one_is_removed_by_hand_and_made_again() {
    drive(|| async {
        let (h, _s, project, _folder, scope) = lead_alone().await;
        let (rt, id) = (&h.fx.rt, id_of(&project));
        let first = rt.workspaces().folder_for(ask(&id, Some("feat/again"), None), as_thread(&scope)).await.expect("a folder");
        let path = ws_json(&first)["worktree"]["path"].as_str().expect("a path").to_string();
        git(Path::new(project["path"].as_str().expect("a path")), &["worktree", "remove", "--force", &path]);
        let again = rt.workspaces().folder_for(ask(&id, Some("feat/again"), None), as_thread(&scope)).await.expect("a folder");
        assert_eq!(again.workspace.id, first.workspace.id);
        assert_eq!(ws_json(&again)["worktree"]["madeFor"], scope.root_thread_id.as_str());
        rt.workspaces().checkout(&again.workspace.id, as_thread(&scope), None).await.expect("its tree's");
        rt.sessions().start(&again.workspace.id, prompt("again"), as_thread(&scope)).await.expect("a start").finished().await;
        rt.close().await.expect("a close");
    });
}

// ===== a worktree removed by hand before any thread ran in it =====

// "a worktree removed by hand before any thread ran in it" > "takes its first thread in the project folder with the moved line, and keeps the record and the thread"
#[test]
fn takes_its_first_thread_in_the_project_folder_with_the_moved_line_and_keeps_the_record_and_the_thread() {
    drive(|| async {
        let s = Scratch::new();
        let h = here(&s, HereOpts::default());
        let folder = s.repo();
        let project = add(&h, &folder).await;
        let made = h.fx.rt.workspaces().worktree(&id_of(&project), "feat/bare", None).await.expect("a worktree");
        let record = workspaces_of(&h).await.into_iter().find(|w| w["worktree"]["path"] == made.path.as_str()).expect("the record");
        std::fs::remove_dir_all(&made.path).expect("the folder goes");
        let run = ran(&h, &id_of(&record), "hello").await;
        assert_eq!(h.agents.cwds(), [Some(path_of(&folder))]);
        let after = workspaces_of(&h).await.into_iter().find(|w| w["id"] == record["id"]).expect("the record");
        assert_matches(&after["worktree"], json!({ "gone": true }));
        let threads: Vec<Value> =
            h.fx.rt.sessions().list(None, None).await.expect("a list").iter().map(j).map(|t| t["threadId"].clone()).collect();
        assert!(threads.contains(&json!(thread_of(&run))));
        let found = moved(&history_of(&h, &id_of(&record)).await);
        assert_eq!(found.len(), 1);
        assert_matches(&found[0], json!({ "threadId": thread_of(&run), "from": made.path, "to": path_of(&folder), "branch": "feat/bare" }));
        h.fx.rt.close().await.expect("a close");
    });
}

// ===== deleting a thread that runs in a worktree wsp made =====

// "deleting a thread that runs in a worktree wsp made" > "is refused while a turn runs there, and once it ended takes the worktree and the thread's checkpoint refs"
#[test]
fn is_refused_while_a_turn_runs_there_and_once_it_ended_takes_the_worktree_and_the_threads_checkpoint_refs() {
    drive(|| async {
        let s = Scratch::new();
        let h = here(&s, HereOpts::default());
        let project = add(&h, &s.repo()).await;
        let at = folder_for(&h, ask(&id_of(&project), Some("feat/busy"), None)).await;
        let path = ws_json(&at)["worktree"]["path"].as_str().expect("a path").to_string();
        h.agents.holding.set(true);
        let run = start(&h, &at.workspace.id, "working").await;
        let thread = thread_of(&run);
        let refused = h.fx.rt.sessions().delete(&thread, None, DeleteThreadAsk::default()).await.expect_err("a refusal");
        assert!(refused.message.contains(WORKTREE_BUSY_LINE), "{}", refused.message);
        assert!(exists(&path));
        h.agents.holding.set(false);
        h.agents.release();
        run.finished().await;
        until(|| async { h.ops().iter().any(|o| o == "git.checkpoint") }).await;
        h.fx.rt.sessions().delete(&thread, None, DeleteThreadAsk::default()).await.expect("a delete");
        assert_eq!(removed_paths(&h), [path]);
        let drops = of_kind("git.checkpointDrop", &h);
        assert_eq!(drops.len(), 1);
        assert_matches(&drops[0], json!({ "scope": at.workspace.id, "thread": thread }));
        h.fx.rt.close().await.expect("a close");
    });
}

// ===== deleting one of two threads in a worktree wsp made =====

// "deleting one of two threads in a worktree wsp made" > "takes that thread alone and leaves the worktree to the other, and the last one takes the worktree"
#[test]
fn takes_that_thread_alone_and_leaves_the_worktree_to_the_other_and_the_last_one_takes_the_worktree() {
    drive(|| async {
        let s = Scratch::new();
        let h = here(&s, HereOpts::default());
        let project = add(&h, &s.repo()).await;
        let at = folder_for(&h, ask(&id_of(&project), Some("feat/two"), None)).await;
        let path = ws_json(&at)["worktree"]["path"].as_str().expect("a path").to_string();
        let c = ran(&h, &at.workspace.id, "thread C").await;
        let d = ran(&h, &at.workspace.id, "thread D").await;
        let (first, second) = (thread_of(&c), thread_of(&d));
        let rt = &h.fx.rt;
        let checked =
            rt.sessions().delete(&second, None, DeleteThreadAsk { check: Some(true), ..Default::default() }).await.expect("a check");
        assert_eq!((checked.workspace_id.as_str(), checked.worktree.clone(), checked.threads), (at.workspace.id.as_str(), None, 1));
        let listed = |rt: &Rc<wsp_serve::AssembledRuntime>| {
            let rt = rt.clone();
            async move {
                let mut ids: Vec<String> =
                    rt.sessions().list(None, None).await.expect("a list").iter().map(|v| v.thread_id.clone().expect("a thread")).collect();
                ids.sort();
                ids
            }
        };
        let mut both = vec![first.clone(), second.clone()];
        both.sort();
        assert_eq!(listed(rt).await, both);
        let one = rt.sessions().delete(&first, None, DeleteThreadAsk::default()).await.expect("a delete");
        assert_eq!((one.workspace_id.as_str(), one.worktree.clone(), one.threads), (at.workspace.id.as_str(), None, 1));
        assert!(removed_paths(&h).is_empty());
        assert!(exists(&path));
        assert_eq!(listed(rt).await, std::slice::from_ref(&second));
        let last = rt.sessions().delete(&second, None, DeleteThreadAsk::default()).await.expect("a delete");
        assert_eq!((last.workspace_id.as_str(), last.worktree.clone(), last.threads), (at.workspace.id.as_str(), Some(path.clone()), 1));
        assert_eq!(removed_paths(&h), [path]);
        rt.close().await.expect("a close");
    });
}

// ===== deleting a thread whose last checkpoint is still being written =====

// "deleting a thread whose last checkpoint is still being written" > "waits for the checkpoint to land before it drops the thread's refs"
#[test]
fn waits_for_the_checkpoint_to_land_before_it_drops_the_threads_refs() {
    drive(|| async {
        let s = Scratch::new();
        let h = here(&s, HereOpts::default());
        let project = add(&h, &s.repo()).await;
        let at = folder_for(&h, ask(&id_of(&project), None, None)).await;
        let landing = Gate::new();
        let (gate, frames) = (landing.clone(), h.daemon.frames.clone());
        h.daemon.answer_later("git.checkpoint", move |f| {
            let (gate, frames) = (gate.clone(), frames.clone());
            let text = |key: &str| f.get(key).and_then(Value::as_str).unwrap_or("undefined").to_string();
            let reply = json!({
                "id": 1, "ok": true, "ref": format!("refs/wsp/checkpoints/{}/{}/{}", text("scope"), text("thread"), text("turn")),
                "commit": "c", "changed": true,
            });
            async move {
                gate.wait().await;
                frames.borrow_mut().push(json!({ "op": "checkpoint landed" }).as_object().cloned().expect("an object"));
                reply
            }
            .boxed_local()
        });
        let run = ran(&h, &at.workspace.id, "one").await;
        until(|| async { h.ops().iter().any(|o| o == "git.checkpoint") }).await;
        let rt = h.fx.rt.clone();
        let thread = thread_of(&run);
        let deleting = tokio::task::spawn_local(async move { rt.sessions().delete(&thread, None, DeleteThreadAsk::default()).await });
        sleep_ms(20).await;
        landing.open();
        deleting.await.expect("the delete runs").expect("a delete");
        let ops = h.ops();
        let landed = ops.iter().position(|o| o == "checkpoint landed").expect("the checkpoint landed");
        let dropped = ops.iter().position(|o| o == "git.checkpointDrop").expect("the refs dropped");
        assert!(dropped > landed, "{ops:?}");
        h.fx.rt.close().await.expect("a close");
    });
}

// ===== a project folder outside the daemon's home =====

fn roots_listed(roots: &Path) -> Vec<String> {
    let mut lines: Vec<String> =
        std::fs::read_to_string(roots).expect("the roots file").split('\n').filter(|l| !l.is_empty()).map(str::to_string).collect();
    lines.sort();
    lines
}

fn sorted(paths: &[&PathBuf]) -> Vec<String> {
    let mut all: Vec<String> = paths.iter().map(|p| path_of(p)).collect();
    all.sort();
    all
}

// "a project folder outside the daemon's home" > "is in the roots file by its first turn, which ends with the card of what changed"
#[test]
fn is_in_the_roots_file_by_its_first_turn_which_ends_with_the_card_of_what_changed() {
    drive(|| async {
        let s = Scratch::new();
        let h = here(&s, HereOpts::default());
        let snapshots = Rc::new(Cell::new(0u32));
        let (roots, count) = (h.roots.clone(), snapshots.clone());
        h.daemon.answer("git.snapshot", move |f| {
            let cwd = f.get("cwd").and_then(Value::as_str).unwrap_or("undefined").to_string();
            if !in_roots(&roots, &cwd) {
                return json!({ "id": 1, "ok": false, "code": "outside-root", "error": format!("{cwd} resolves outside the folders wsp serves here") });
            }
            count.set(count.get() + 1);
            json!({ "id": 1, "ok": true, "commit": count.get().to_string().repeat(40) })
        });
        h.daemon.answer("git.turn", |_| {
            json!({ "id": 1, "ok": true, "base": null, "truncated": false, "moved": [], "files": [{ "path": "hello.txt", "kind": "added", "additions": 1, "deletions": 0, "patch": "" }] })
        });
        let events = h.fx.events();
        let project = add(&h, &s.repo()).await;
        let at = folder_for(&h, ask(&id_of(&project), None, None)).await;
        ran(&h, &at.workspace.id, "write hello.txt").await;
        until(|| async { events.has("session.changes") }).await;
        let cards = events.of("session.changes");
        assert_eq!(cards.len(), 1);
        assert_matches(&cards[0], json!({ "from": "1".repeat(40), "to": "2".repeat(40), "files": [{ "path": "hello.txt" }] }));
        h.fx.rt.close().await.expect("a close");
    });
}

// "a project folder outside the daemon's home" > "is in the roots file beside every other folder whose record was made at the same time"
#[test]
fn is_in_the_roots_file_beside_every_other_folder_whose_record_was_made_at_the_same_time() {
    drive(|| async {
        // The records are made 10 ms apart and each write of the roots file is held before it runs, the first longest, so a write that read
        // fewer records lands after one that read more unless the writes go one at a time.
        let s = Scratch::new();
        let writes = Rc::new(Cell::new(0i64));
        let held = writes.clone();
        let around: Around = Rc::new(move |cmd: String| {
            let wait = if cmd.contains(".next") {
                let n = held.get();
                held.set(n + 1);
                Some(((8 - n) * 15).max(0) as u64)
            } else {
                None
            };
            async move {
                if let Some(ms) = wait {
                    sleep_ms(ms).await;
                }
                Ok(cmd)
            }
            .boxed_local()
        });
        let h = here(&s, HereOpts { backend: Some(AroundBackend::new(&s.dir(), None, around)), ..Default::default() });
        let folders: Vec<PathBuf> = (0..8).map(|_| s.repo()).collect();
        let mut projects = vec![];
        for f in &folders {
            projects.push(add(&h, f).await);
        }
        let opened = futures::future::join_all(projects.iter().enumerate().map(|(i, p)| {
            let (rt, id) = (h.fx.rt.clone(), id_of(p));
            async move {
                sleep_ms(i as u64 * 10).await;
                rt.workspaces().folder_for(ask(&id, None, None), None).await
            }
        }))
        .await;
        assert!(opened.iter().all(Result::is_ok));
        assert_eq!(roots_listed(&h.roots), sorted(&folders.iter().collect::<Vec<_>>()));
        h.fx.rt.close().await.expect("a close");
    });
}

// "a project folder outside the daemon's home" > "leaves the roots file once its project is removed, and the other projects' folders stay"
#[test]
fn leaves_the_roots_file_once_its_project_is_removed_and_the_other_projects_folders_stay() {
    drive(|| async {
        let s = Scratch::new();
        let h = here(&s, HereOpts::default());
        let (kept, removed) = (s.repo(), s.repo());
        let kept_project = add(&h, &kept).await;
        let removed_project = add(&h, &removed).await;
        folder_for(&h, ask(&id_of(&kept_project), None, None)).await;
        folder_for(&h, ask(&id_of(&removed_project), None, None)).await;
        assert_eq!(roots_listed(&h.roots), sorted(&[&kept, &removed]));
        h.fx.rt.projects().remove(&id_of(&removed_project), None, None, None).await.expect("a remove");
        assert_eq!(roots_listed(&h.roots), [path_of(&kept)]);
        h.fx.rt.close().await.expect("a close");
    });
}

/// A bundler of a folder on this computer: a plan of one file, and no pack, since a folder here is never packed.
struct Unpacked {
    source: PathBuf,
    planned: Rc<Cell<bool>>,
}

#[async_trait(?Send)]
impl ProjectBundler for Unpacked {
    async fn plan(&self) -> Result<ProjectPlan, wsp_words::exit::HostError> {
        self.planned.set(true);
        Ok(ProjectPlan {
            source: path_of(&self.source),
            repo: true,
            files: 1.0.into(),
            bytes: 1.0.into(),
            secrets: vec![],
            values: None,
            excluded: vec![],
            skipped: vec![],
            agents: vec![],
        })
    }
    async fn pack(&self, _carry: &[String], _rewrite: &[String], _provider: bool) -> Result<PackedProject, wsp_words::exit::HostError> {
        panic!("a folder on this computer is never packed")
    }
    async fn pack_state(&self, _req: StateRequest) -> Result<PackedState, wsp_words::exit::HostError> {
        panic!("a folder on this computer is never packed")
    }
}

fn import_of(workspace: &str, folder: &Path, planned: &Rc<Cell<bool>>) -> ProjectImportOptions {
    ProjectImportOptions {
        workspace_id: workspace.into(),
        source: path_of(folder),
        dest: path_of(folder),
        replace: None,
        carry: None,
        rewrite: None,
        agents: None,
        bundler: Rc::new(Unpacked { source: folder.to_path_buf(), planned: planned.clone() }),
    }
}

// "a project folder outside the daemon's home" > "keeps every other checkout's folder when a folder is imported, and the imported one stays through a host restart until its workspace goes"
#[test]
fn keeps_every_other_checkouts_folder_when_a_folder_is_imported_and_the_imported_one_stays_through_a_host_restart_until_its_workspace_goes()
{
    drive(|| async {
        let s = Scratch::new();
        let first = here(&s, HereOpts::default());
        let (other, own, landed, removed) = (s.repo(), s.repo(), s.repo(), s.repo());
        folder_for(&first, ask(&id_of(&add(&first, &other).await), None, None)).await;
        let own_project = add(&first, &own).await;
        let at = folder_for(&first, ask(&id_of(&own_project), None, None)).await;
        let removed_project = add(&first, &removed).await;
        folder_for(&first, ask(&id_of(&removed_project), None, None)).await;
        let planned = Rc::new(Cell::new(false));
        first.fx.rt.projects().import(import_of(&at.workspace.id, &landed, &planned), None).await.expect("an import");
        assert_eq!(roots_listed(&first.roots), sorted(&[&other, &own, &landed, &removed]));
        // A restarted host writes the file whole from the records it reads back, and the imported folder is one of theirs.
        first.fx.rt.close().await.expect("a close");
        let second =
            here(&s, HereOpts { again: Some(Again { root: first.root.clone(), state: first.state.clone() }), ..Default::default() });
        second.fx.rt.projects().remove(&id_of(&removed_project), None, None, None).await.expect("a remove");
        assert_eq!(roots_listed(&second.roots), sorted(&[&other, &own, &landed]));
        // The folder goes with the workspace that imported it.
        second.fx.rt.projects().remove(&id_of(&own_project), None, None, None).await.expect("a remove");
        assert_eq!(roots_listed(&second.roots), [path_of(&other)]);
        second.fx.rt.close().await.expect("a close");
    });
}

// "a project folder outside the daemon's home" > "takes back only its own folder when its roots write is refused, so an import beside it keeps its folder"
#[test]
fn takes_back_only_its_own_folder_when_its_roots_write_is_refused_so_an_import_beside_it_keeps_its_folder() {
    drive(|| async {
        // The first write of the roots file is held until the second import is queued behind it, then refused.
        let s = Scratch::new();
        let release = Gate::new();
        let refusing = Rc::new(Cell::new(false));
        let refused_folder: Rc<RefCell<String>> = Rc::default();
        let (held, flag, name) = (release.clone(), refusing.clone(), refused_folder.clone());
        let around: Around = Rc::new(move |cmd: String| {
            let target = name.borrow().clone();
            if cmd.contains(".next") && !target.is_empty() && cmd.contains(&target) && !flag.get() {
                flag.set(true);
                let held = held.clone();
                return async move {
                    held.wait().await;
                    Err(wsp_engine::machine::ExecResult {
                        exit_code: 1.into(),
                        stdout: String::new(),
                        stderr: "read-only file system".into(),
                    })
                }
                .boxed_local();
            }
            async move { Ok(cmd) }.boxed_local()
        });
        let h = here(&s, HereOpts { backend: Some(AroundBackend::new(&s.dir(), None, around)), ..Default::default() });
        let (own, first, second) = (s.repo(), s.repo(), s.repo());
        *refused_folder.borrow_mut() = path_of(&first);
        let at = folder_for(&h, ask(&id_of(&add(&h, &own).await), None, None)).await;
        let second_planned = Rc::new(Cell::new(false));
        let first_planned = Rc::new(Cell::new(false));
        let one = {
            let (rt, o) = (h.fx.rt.clone(), import_of(&at.workspace.id, &first, &first_planned));
            tokio::task::spawn_local(async move { rt.projects().import(o, None).await })
        };
        until(|| async { refusing.get() }).await;
        let two = {
            let (rt, o) = (h.fx.rt.clone(), import_of(&at.workspace.id, &second, &second_planned));
            tokio::task::spawn_local(async move { rt.projects().import(o, None).await })
        };
        until(|| async { second_planned.get() }).await;
        sleep_ms(50).await;
        release.open();
        let refusal = one.await.expect("the import runs").expect_err("a refusal");
        assert!(refusal.message.contains("read-only file system"), "{}", refusal.message);
        two.await.expect("the import runs").expect("an import");
        assert_eq!(roots_listed(&h.roots), sorted(&[&own, &second]));
        h.fx.rt.close().await.expect("a close");
    });
}

// "a project folder outside the daemon's home" > "takes an imported folder off the roots file when the workspace that imported it goes"
#[test]
fn takes_an_imported_folder_off_the_roots_file_when_the_workspace_that_imported_it_goes() {
    drive(|| async {
        let s = Scratch::new();
        let h = here(&s, HereOpts::default());
        let (other, own, landed) = (s.repo(), s.repo(), s.repo());
        folder_for(&h, ask(&id_of(&add(&h, &other).await), None, None)).await;
        let own_project = add(&h, &own).await;
        let at = folder_for(&h, ask(&id_of(&own_project), None, None)).await;
        let planned = Rc::new(Cell::new(false));
        h.fx.rt.projects().import(import_of(&at.workspace.id, &landed, &planned), None).await.expect("an import");
        h.fx.rt.projects().remove(&id_of(&own_project), None, None, None).await.expect("a remove");
        assert_eq!(roots_listed(&h.roots), [path_of(&other)]);
        h.fx.rt.close().await.expect("a close");
    });
}

// ===== a pull request's head =====

// "a pull request's head" > "of a merged pull request whose branch was deleted is still fetched, off the pull request's own head"
#[test]
fn of_a_merged_pull_request_whose_branch_was_deleted_is_still_fetched_off_the_pull_requests_own_head() {
    drive(|| async {
        let g = on_github_here(None).await;
        let tip = git(&g.author, &["rev-parse", "HEAD"]);
        git(&g.bare, &["update-ref", "refs/pull/7/head", &tip]);
        git(&g.bare, &["update-ref", "-d", "refs/heads/feat/pr"]);
        let started = start_pr(&g).await;
        assert_eq!(git(Path::new(started["workspace"]["worktree"]["path"].as_str().expect("a path")), &["rev-parse", "HEAD"]), tip);
        g.h.fx.rt.close().await.expect("a close");
    });
}

// "a pull request's head" > "is refused for a pick the agent does not take before any worktree is made"
#[test]
fn is_refused_for_a_pick_the_agent_does_not_take_before_any_worktree_is_made() {
    drive(|| async {
        let g = on_github_here(None).await;
        let ask = StartFromLinkAsk {
            url: g.url.clone(),
            agent: Some("claude".into()),
            model: Some("no-such-model".into()),
            ..Default::default()
        };
        assert!(g.h.fx.rt.workspaces().start(ask, None).await.is_err());
        assert!(g.h.copier.lock().unwrap().worktrees.is_empty());
        g.h.fx.rt.close().await.expect("a close");
    });
}

// "a pull request's head" > "is fetched through the daemon at every start, so a second start runs on what the author pushed since"
#[test]
fn is_fetched_through_the_daemon_at_every_start_so_a_second_start_runs_on_what_the_author_pushed_since() {
    drive(|| async {
        let g = on_github_here(None).await;
        let first = start_pr(&g).await;
        let path = first["workspace"]["worktree"]["path"].as_str().expect("a path").to_string();
        assert_eq!(git(Path::new(&path), &["rev-parse", "HEAD"]), git(&g.author, &["rev-parse", "HEAD"]));
        g.h.fx
            .rt
            .workspaces()
            .worktree_remove(WorktreeRemoveAsk { project: id_of(&g.project), branch: "feat/pr".into(), ..Default::default() }, None)
            .await
            .expect("a removal");
        push_two(&g);
        let second = start_pr(&g).await;
        let again = second["workspace"]["worktree"]["path"].as_str().expect("a path");
        assert_eq!(git(Path::new(again), &["rev-parse", "HEAD"]), git(&g.author, &["rev-parse", "HEAD"]));
        let fetched = of_kind("git.fetchBranch", &g.h);
        assert_eq!(fetched.len(), 2);
        for f in &fetched {
            assert_matches(f, json!({ "cwd": path_of(&g.folder), "remote": "https://github.com/dev/spoo.git", "branch": "feat/pr" }));
        }
        let _ = &g.scratch;
        g.h.fx.rt.close().await.expect("a close");
    });
}
