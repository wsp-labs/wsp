// SPDX-License-Identifier: AGPL-3.0-only
//! Hard tests for copier: the empty and odd inputs the TS cases leave out.
mod common;

use wsp_engine::copier::*;

#[test]
fn a_refusal_that_said_nothing_is_the_empty_sentence_and_stdout_stands_in_for_an_empty_stderr() {
    // copier.ts:71: `"".split("\n").at(-1)` is `""`, which `??` keeps, so the TS's fallback sentence never fires.
    assert_eq!(refusal_line("/bin/wsp-daemon", &common::res(1, "", "  \n")), "");
    assert_eq!(refusal_line("/bin/wsp-daemon", &common::res(1, "", "a\r\nb\r\n")), "b");
    assert_eq!(refusal_line("/bin/wsp-daemon", &common::res(1, "first\nthe last word\n", "")), "the last word");
    assert_eq!(refusal_line("/bin/wsp-daemon", &common::res(1, "ignored", "only stderr counts\n")), "only stderr counts");
    // Exit 127, a child that never started, is not the verb's refusal.
    assert_eq!(
        refusal_line("/bin/wsp-daemon", &common::res(127, "", "")),
        "the daemon's copy verb failed (exit 127); the host log has its output"
    );
}

#[test]
fn a_copy_with_no_exclusion_base_or_road_is_the_bare_line() {
    let ask = CopyAsk { from: "/a b".into(), to: "/c'd".into(), base: None, exclude: vec![], size_line_bytes: 0, road: None };
    assert_eq!(argv_for(&ask), vec!["copy", "make", "--from", "/a b", "--to", "/c'd", "--size-line-bytes", "0"]);
}

#[tokio::test]
async fn the_stand_ins_worktree_folder_replaces_every_char_outside_the_safe_set() {
    let home = tempfile::tempdir().unwrap();
    let copier = fake_copier(None, None);
    let ask = WorktreeAsk {
        from: "/repo".into(),
        home: home.path().to_str().unwrap().into(),
        project: "prj_1".into(),
        branch: "feat/x y@é.1_-".into(),
        modules: vec![],
        checkpoint: None,
    };
    let made = copier.worktree(&ask).await.unwrap();
    let want = home.path().join("worktrees/prj_1/feat-x-y--.1_-");
    assert_eq!(made.path, want.to_str().unwrap());
    assert!(want.is_dir());
    assert_eq!(made.branch, "feat/x y@é.1_-");
    assert!(made.made && made.fresh && made.carried.is_empty() && made.modules.is_empty() && made.ms == 1);
    assert_eq!(copier.log.lock().unwrap().worktrees, vec![ask]);
}

// ---- K-rules (spec/copier.md), values from running the TS (scratch oracle, 2026-10-25)

use std::sync::{Arc, Mutex};
use wsp_engine::machine::ExecResult;

type Seen = Arc<Mutex<Vec<(String, Vec<String>, Option<u64>)>>>;

fn answering(exit_code: i64, stdout: &str, stderr: &str) -> (Seen, VerbCopier) {
    let seen: Seen = Arc::default();
    let log = seen.clone();
    let (stdout, stderr) = (stdout.to_string(), stderr.to_string());
    let run: ChildRun = Arc::new(move |file: String, args: Vec<String>, o: ChildRunOptions| {
        log.lock().unwrap().push((file, args, o.timeout_ms));
        let answer = ExecResult { exit_code, stdout: stdout.clone(), stderr: stderr.clone() };
        Box::pin(async move { answer })
    });
    (seen, verb_copier("/bin/d", Some(run)))
}

fn removal(force: bool) -> WorktreeRemove {
    WorktreeRemove { from: "/r".into(), home: "/h".into(), path: "/w".into(), force }
}

fn report_json() -> serde_json::Value {
    serde_json::json!({ "road": "clonefile", "path": "/p", "base": "1".repeat(40), "branch": "main", "fetched": true, "carried": "deps-and-config", "excluded": [], "bytes": 1024, "ms": 1 })
}

fn make_ask() -> CopyAsk {
    CopyAsk { from: "/a".into(), to: "/c".into(), base: None, exclude: vec![], size_line_bytes: 1, road: None }
}

/// K1: every call's own bound: the mount alone is 30 s, forget and remove-worktree are the copy's ten minutes.
#[tokio::test]
async fn each_call_asks_its_own_timeout_the_mount_alone_the_short_one() {
    let (seen, c) = answering(0, "{\"path\":\"/w\"}", "");
    c.remove("/a", "/b", CopyRoad::Clonefile).await.unwrap();
    c.worktree_mount(&WorktreeAt { from: "/r".into(), home: "/h".into(), path: "/w".into() }).await.unwrap();
    c.worktree_forget(&WorktreeProject { from: "/r".into(), home: "/h".into(), project: "pp".into() }).await.unwrap();
    c.worktree_remove(&removal(false)).await.unwrap();
    let bounds: Vec<Option<u64>> = seen.lock().unwrap().iter().map(|c| c.2).collect();
    assert_eq!(bounds, vec![Some(600_000), Some(30_000), Some(600_000), Some(600_000)]);
    assert!(seen.lock().unwrap().iter().all(|c| c.0 == "/bin/d"));
}

/// K2: worktree_remove's argv, `--force` only when asked, and its answer read through the wire type.
#[tokio::test]
async fn worktree_remove_names_force_only_when_asked_and_reads_its_answer() {
    let (seen, c) = answering(0, "{\"path\":\"/w\",\"rescued\":\"refs/rescue/x\"}", "");
    let got = c.worktree_remove(&removal(true)).await.unwrap();
    assert_eq!(got.path, "/w");
    assert_eq!(got.rescued.as_deref(), Some("refs/rescue/x"));
    assert_eq!(seen.lock().unwrap()[0].1, ["copy", "worktree-remove", "--from", "/r", "--home", "/h", "--path", "/w", "--force"]);
    let (seen, c) = answering(0, "{\"path\":\"/w\"}", "");
    assert_eq!(c.worktree_remove(&removal(false)).await.unwrap().rescued, None);
    assert_eq!(seen.lock().unwrap()[0].1, ["copy", "worktree-remove", "--from", "/r", "--home", "/h", "--path", "/w"]);
    let (_, c) = answering(1, "", "refused: files no commit holds\n");
    assert_eq!(c.worktree_remove(&removal(false)).await.unwrap_err().to_string(), "refused: files no commit holds");
}

/// K3: a null where the TS's zod schema says optional is refused, as zod refuses it; a negative or fractional size and a
/// missing field are refused; an unknown extra key is dropped. Every refusal is the exact not-read sentence with the
/// trimmed stdout as it came.
#[tokio::test]
async fn a_null_optional_a_bad_size_or_a_missing_field_is_refused_and_an_extra_key_is_dropped() {
    let refused = |what: &str, said: &str| format!("the {what} verb answered something this host does not read: {said}");
    for (key, value) in [("skipped", serde_json::Value::Null), ("fellBack", serde_json::Value::Null), ("bytes", serde_json::json!(-1)), ("bytes", serde_json::json!(1.5))] {
        let mut r = report_json();
        r[key] = value;
        let said = serde_json::to_string(&r).unwrap();
        let (_, c) = answering(0, &format!("{said}\n"), "");
        assert_eq!(c.make(&make_ask()).await.unwrap_err().to_string(), refused("copy", &said), "{key}");
    }
    let mut r = report_json();
    r.as_object_mut().unwrap().shift_remove("fetched");
    let said = serde_json::to_string(&r).unwrap();
    let (_, c) = answering(0, &said, "");
    assert_eq!(c.make(&make_ask()).await.unwrap_err().to_string(), refused("copy", &said));
    let (_, c) = answering(0, "{\"path\":\"/w\",\"rescued\":null}", "");
    assert_eq!(c.worktree_remove(&removal(false)).await.unwrap_err().to_string(), refused("worktree removal", "{\"path\":\"/w\",\"rescued\":null}"));
    let mut r = report_json();
    r["extra"] = serde_json::json!(1);
    let (_, c) = answering(0, &serde_json::to_string(&r).unwrap(), "");
    assert_eq!(c.make(&make_ask()).await.unwrap().bytes, 1024);
}

/// K4: the not-read sentence: blank stdout is `nothing at all`, the rest is the trimmed text cut at 200 UTF-16 units,
/// never re-serialised; each verb names itself.
#[tokio::test]
async fn the_not_read_sentence_is_the_trimmed_text_cut_at_200_or_nothing_at_all() {
    let (_, c) = answering(0, "  \n", "");
    assert_eq!(c.make(&make_ask()).await.unwrap_err().to_string(), "the copy verb answered something this host does not read: nothing at all");
    let (_, c) = answering(0, "  \n", "");
    assert_eq!(c.worktree_remove(&removal(false)).await.unwrap_err().to_string(), "the worktree removal verb answered something this host does not read: nothing at all");
    let (_, c) = answering(0, "null", "");
    assert_eq!(c.make(&make_ask()).await.unwrap_err().to_string(), "the copy verb answered something this host does not read: null");
    let (_, c) = answering(0, &"x".repeat(250), "");
    assert_eq!(c.make(&make_ask()).await.unwrap_err().to_string(), format!("the copy verb answered something this host does not read: {}", "x".repeat(200)));
    let (_, c) = answering(0, "{\"road\":\"rsync\"}\n", "");
    assert_eq!(c.make(&make_ask()).await.unwrap_err().to_string(), "the copy verb answered something this host does not read: {\"road\":\"rsync\"}");
}

/// K5: an empty string is still a value: `--base ""`, `--exclude ""` and `--checkpoint ""` are passed (`!== undefined`).
#[tokio::test]
async fn an_empty_base_exclusion_or_checkpoint_is_still_passed() {
    let ask = CopyAsk { base: Some(String::new()), exclude: vec![String::new()], ..make_ask() };
    assert_eq!(argv_for(&ask), ["copy", "make", "--from", "/a", "--to", "/c", "--base", "", "--exclude", "", "--size-line-bytes", "1"]);
    let (seen, c) = answering(0, "{\"path\":\"/w\",\"modules\":[]}", "");
    let _ = c
        .worktree(&WorktreeAsk { from: "/r".into(), home: "/h".into(), project: "p".into(), branch: "b".into(), modules: vec![], checkpoint: Some(String::new()) })
        .await;
    let argv = seen.lock().unwrap()[0].1.clone();
    assert_eq!(argv[argv.len() - 2..], ["--checkpoint".to_string(), String::new()]);
}

/// K6: the stand-in's folder is node's `path.join`, which normalises: an empty or dot branch collapses, `..` climbs, and
/// the safe-set swap is per UTF-16 unit (an emoji is two dashes).
#[tokio::test]
async fn the_stand_ins_folder_is_a_normalised_join() {
    let home = tempfile::tempdir().unwrap();
    let h = std::fs::canonicalize(home.path()).unwrap().to_str().unwrap().to_string();
    let copier = fake_copier(None, None);
    for (project, branch, want) in [
        ("p", "", format!("{h}/worktrees/p")),
        ("p", ".", format!("{h}/worktrees/p")),
        ("p", "..", format!("{h}/worktrees")),
        ("p", "a/../b", format!("{h}/worktrees/p/a-..-b")),
        ("p", "x\u{1F600}y", format!("{h}/worktrees/p/x--y")),
        ("..", "b", format!("{h}/b")),
    ] {
        let ask = WorktreeAsk { from: "/r".into(), home: h.clone(), project: project.into(), branch: branch.into(), modules: vec![], checkpoint: None };
        let made = copier.worktree(&ask).await.unwrap();
        assert_eq!(made.path, want, "{project} {branch:?}");
        assert!(std::path::Path::new(&want).is_dir());
    }
}
