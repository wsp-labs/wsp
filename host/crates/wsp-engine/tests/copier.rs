// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/test/copier.test.ts at a4033dc34. No child process runs: the runner is handed in.
mod common;

use std::sync::{Arc, Mutex};

use wsp_engine::copier::{
    fake_copier, verb_copier, ChildRun, ChildRunOptions, Copier, ExecResult, RoomReading, WorktreeAt, WorktreeProject, COPY_TIMEOUT_MS,
};
use wsp_engine::copier::{CopyAsk, CopyReport, CopyRoad, WorktreeAsk, WorktreeReport};
use wsp_engine::local_backend::MemoryHere;
use wsp_wire::views::workspace_more::{Carried, CarryModule, FoundModule};

const BIN: &str = "/opt/wsp/daemon/aarch64-apple-darwin/wsp-daemon";

type Calls = Arc<Mutex<Vec<(String, Vec<String>, Option<u64>)>>>;

fn report() -> CopyReport {
    CopyReport {
        road: CopyRoad::Clonefile,
        path: "/Users/dev/repo-pricing-page".to_string(),
        base: "1".repeat(40),
        branch: "main".to_string(),
        fetched: true,
        carried: Carried::DepsAndConfig,
        excluded: vec![".next".to_string(), "node_modules/.cache".to_string()],
        skipped: None,
        bytes: 1024.into(),
        fell_back: None,
        ms: 4900.into(),
    }
}

fn s(words: &[&str]) -> Vec<String> {
    words.iter().map(|w| w.to_string()).collect()
}

/// A runner that records what it was asked to run and answers what the case wants.
fn runner(exit_code: i64, stdout: &str, stderr: &str) -> (Calls, ChildRun) {
    let calls: Calls = Arc::new(Mutex::new(Vec::new()));
    let seen = calls.clone();
    let (stdout, stderr) = (stdout.to_string(), stderr.to_string());
    let run: ChildRun = Arc::new(move |file: String, args: Vec<String>, opts: ChildRunOptions| {
        seen.lock().unwrap().push((file, args, opts.timeout_ms));
        let answer = ExecResult { exit_code: exit_code.into(), stdout: stdout.clone(), stderr: stderr.clone() };
        Box::pin(async move { answer })
    });
    (calls, run)
}

fn ask(from: &str, to: &str, exclude: &[&str], size_line_bytes: i64) -> CopyAsk {
    CopyAsk {
        from: from.to_string(),
        to: to.to_string(),
        base: None,
        exclude: s(exclude),
        size_line_bytes: size_line_bytes.into(),
        road: None,
    }
}

fn worktree_ask(branch: &str, modules: Vec<CarryModule>, checkpoint: Option<&str>) -> WorktreeAsk {
    WorktreeAsk {
        from: "/repo".to_string(),
        home: "/h".to_string(),
        project: "prj_1".to_string(),
        branch: branch.to_string(),
        modules,
        checkpoint: checkpoint.map(str::to_string),
    }
}

// TS: packages/engine/test/copier.test.ts:46
// "is run with one word per exclusion and answers the report it printed"
#[tokio::test]
async fn is_run_with_one_word_per_exclusion_and_answers_the_report_it_printed() {
    let (calls, run) = runner(0, &format!("{}\n", serde_json::to_string(&report()).unwrap()), "");
    let copier = verb_copier(BIN, Some(run));
    let answered = copier.make(&ask("/Users/dev/repo", "/Users/dev/repo-pricing-page", &[".venv", ".next"], 21474836480)).await.unwrap();
    assert_eq!(answered, report());
    let calls = calls.lock().unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].0, BIN);
    assert_eq!(
        calls[0].1,
        s(&[
            "copy",
            "make",
            "--from",
            "/Users/dev/repo",
            "--to",
            "/Users/dev/repo-pricing-page",
            "--exclude",
            ".venv",
            "--exclude",
            ".next",
            "--size-line-bytes",
            "21474836480",
        ])
    );
    assert_eq!(calls[0].2, Some(COPY_TIMEOUT_MS));
}

// TS: packages/engine/test/copier.test.ts:73
// "names the base and the road on the line only where the caller named them"
#[tokio::test]
async fn names_the_base_and_the_road_on_the_line_only_where_the_caller_named_them() {
    let printed = CopyReport { road: CopyRoad::Worktree, branch: String::new(), ..report() };
    let (calls, run) = runner(0, &serde_json::to_string(&printed).unwrap(), "");
    let copier = verb_copier(BIN, Some(run));
    let mut named = ask("/a", "/b", &[], 1);
    named.base = Some("release".to_string());
    named.road = Some(CopyRoad::Worktree);
    copier.make(&named).await.unwrap();
    assert_eq!(
        calls.lock().unwrap()[0].1,
        s(&["copy", "make", "--from", "/a", "--to", "/b", "--base", "release", "--size-line-bytes", "1", "--road", "worktree"])
    );
}

// TS: packages/engine/test/copier.test.ts:81
// "throws the verb's own last line when it refused, so a person reads why the copy could not be made"
#[tokio::test]
async fn throws_the_verbs_own_last_line_when_it_refused_so_a_person_reads_why() {
    let (_, run) = runner(1, "", "warming up\n/Users/dev/repo-pricing-page is already there; a copy is made at a path of its own\n");
    let copier = verb_copier(BIN, Some(run));
    let err = copier.make(&ask("/Users/dev/repo", "/Users/dev/repo-pricing-page", &[], 1)).await.unwrap_err();
    assert!(err.to_string().contains("/Users/dev/repo-pricing-page is already there; a copy is made at a path of its own"), "{err}");
}

// TS: packages/engine/test/copier.test.ts:90
// "turns any other exit into one sentence naming the verb, so a parse error's usage text never reaches a person"
#[tokio::test]
async fn turns_any_other_exit_into_one_sentence_naming_the_verb() {
    let (_, stale) = runner(2, "", "error: unrecognized subcommand 'copy'\n\nUsage: wsp-daemon [OPTIONS] --root <ROOT>\n");
    let err = verb_copier(BIN, Some(stale.clone())).make(&ask("/a", "/b", &[], 1)).await.unwrap_err();
    assert!(err.to_string().contains("the daemon's copy verb failed (exit 2); the host log has its output"), "{err}");
    let err = verb_copier(BIN, Some(stale)).make(&ask("/a", "/b", &[], 1)).await.unwrap_err();
    assert!(!err.to_string().contains("Usage:"), "{err}");

    let (_, killed) = runner(-1, "", "");
    let err = verb_copier(BIN, Some(killed)).make(&ask("/a", "/b", &[], 1)).await.unwrap_err();
    assert!(err.to_string().contains("the daemon's copy verb failed (exit -1)"), "{err}");

    let (_, refused) = runner(1, "", "/b is already there; a copy is made at a path of its own\n");
    let err = verb_copier(BIN, Some(refused)).make(&ask("/a", "/b", &[], 1)).await.unwrap_err();
    assert!(err.to_string().contains("/b is already there; a copy is made at a path of its own"), "{err}");
}

// TS: packages/engine/test/copier.test.ts:106
// "refuses a line this host does not read rather than recording half a copy"
#[tokio::test]
async fn refuses_a_line_this_host_does_not_read_rather_than_recording_half_a_copy() {
    let mut printed = serde_json::to_value(report()).unwrap();
    printed["road"] = serde_json::json!("rsync");
    let (_, run) = runner(0, &printed.to_string(), "");
    let err = verb_copier(BIN, Some(run)).make(&ask("/a", "/b", &[], 1)).await.unwrap_err();
    assert!(err.to_string().contains("the copy verb answered something this host does not read"), "{err}");
}

// TS: packages/engine/test/copier.test.ts:113
// "hands the worktree verb each module as one JSON word and reads back the modules a new worktree holds"
#[tokio::test]
async fn hands_the_worktree_verb_each_module_as_one_json_word_and_reads_back_the_modules() {
    let printed = WorktreeReport {
        path: "/w/feat-x".to_string(),
        branch: "feat/x".to_string(),
        made: true,
        carried: s(&["web/node_modules"]),
        plain: None,
        overlaid: Some(s(&["web/node_modules"])),
        fresh: true,
        modules: vec![FoundModule { id: "pnpm".to_string(), folder: "web".to_string(), rebuild: false }],
        ms: 12.into(),
    };
    let (calls, run) = runner(0, &format!("{}\n", serde_json::to_string(&printed).unwrap()), "");
    let modules = vec![
        CarryModule {
            id: "pnpm".to_string(),
            lockfiles: s(&["pnpm-lock.yaml"]),
            carry: s(&["node_modules"]),
            never: s(&[".next"]),
            installed: Some("node_modules/.pnpm/lock.yaml".to_string()),
        },
        CarryModule { id: "uv".to_string(), lockfiles: s(&["uv.lock"]), carry: vec![], never: s(&[".venv"]), installed: None },
    ];
    let made = verb_copier(BIN, Some(run)).worktree(&worktree_ask("feat/x", modules, None)).await.unwrap();
    assert_eq!(made, printed);
    assert_eq!(
        calls.lock().unwrap()[0].1,
        s(&[
            "copy",
            "worktree",
            "--from",
            "/repo",
            "--home",
            "/h",
            "--project",
            "prj_1",
            "--branch",
            "feat/x",
            "--module",
            r#"{"id":"pnpm","lockfiles":["pnpm-lock.yaml"],"carry":["node_modules"],"never":[".next"],"installed":"node_modules/.pnpm/lock.yaml"}"#,
            "--module",
            r#"{"id":"uv","lockfiles":["uv.lock"],"carry":[],"never":[".venv"]}"#,
        ])
    );
}

// TS: packages/engine/test/copier.test.ts:132
// "hands the worktree verb the checkpoint a fork starts from, and no such flag without one"
#[tokio::test]
async fn hands_the_worktree_verb_the_checkpoint_a_fork_starts_from_and_no_such_flag_without_one() {
    let printed = WorktreeReport {
        path: "/w/fork".to_string(),
        branch: "fork".to_string(),
        made: true,
        carried: vec![],
        plain: None,
        overlaid: None,
        fresh: true,
        modules: vec![],
        ms: 3.into(),
    };
    let (calls, run) = runner(0, &format!("{}\n", serde_json::to_string(&printed).unwrap()), "");
    let checkpoint = "refs/wsp/checkpoints/ws_1/thr_1/turn_2";
    let copier = verb_copier(BIN, Some(run));
    copier.worktree(&worktree_ask("fork", vec![], Some(checkpoint))).await.unwrap();
    copier.worktree(&worktree_ask("fork", vec![], None)).await.unwrap();
    let calls = calls.lock().unwrap();
    assert_eq!(
        calls[0].1,
        s(&["copy", "worktree", "--from", "/repo", "--home", "/h", "--project", "prj_1", "--branch", "fork", "--checkpoint", checkpoint])
    );
    assert_eq!(calls[1].1, s(&["copy", "worktree", "--from", "/repo", "--home", "/h", "--project", "prj_1", "--branch", "fork"]));
}

// TS: packages/engine/test/copier.test.ts:144
// "mounts a worktree's overlays again by its path, and throws the verb's own sentence where it refused"
#[tokio::test]
async fn mounts_a_worktrees_overlays_again_by_its_path_and_throws_the_verbs_own_sentence() {
    let (calls, run) = runner(0, "", "");
    let at = WorktreeAt { from: "/repo".to_string(), home: "/h".to_string(), path: "/h/worktrees/prj_1/feat-x".to_string() };
    verb_copier(BIN, Some(run)).worktree_mount(&at).await.unwrap();
    assert_eq!(
        calls.lock().unwrap()[0].1,
        s(&["copy", "worktree-mount", "--from", "/repo", "--home", "/h", "--path", "/h/worktrees/prj_1/feat-x"])
    );
    let (_, refused) = runner(1, "", "/repo is not a worktree wsp made for this project\n");
    let at = WorktreeAt { from: "/repo".to_string(), home: "/h".to_string(), path: "/repo".to_string() };
    let err = verb_copier(BIN, Some(refused)).worktree_mount(&at).await.unwrap_err();
    assert!(err.to_string().contains("/repo is not a worktree wsp made for this project"), "{err}");
}

// TS: packages/engine/test/copier.test.ts:152
// "takes a project's frozen copies away by its id, and throws the verb's own sentence where it refused"
#[tokio::test]
async fn takes_a_projects_frozen_copies_away_by_its_id_and_throws_the_verbs_own_sentence() {
    let (calls, run) = runner(0, "", "");
    let of = WorktreeProject { from: "/repo".to_string(), home: "/h".to_string(), project: "prj_1".to_string() };
    verb_copier(BIN, Some(run)).worktree_forget(&of).await.unwrap();
    assert_eq!(calls.lock().unwrap()[0].1, s(&["copy", "worktree-forget", "--from", "/repo", "--home", "/h", "--project", "prj_1"]));
    let (_, refused) = runner(1, "", "/h/repo is not the top of a git repository\n");
    let of = WorktreeProject { from: "/h/repo".to_string(), home: "/h".to_string(), project: "prj_1".to_string() };
    let err = verb_copier(BIN, Some(refused)).worktree_forget(&of).await.unwrap_err();
    assert!(err.to_string().contains("/h/repo is not the top of a git repository"), "{err}");
}

// TS: packages/engine/test/copier.test.ts:160
// "takes a copy away by the road that made it"
#[tokio::test]
async fn takes_a_copy_away_by_the_road_that_made_it() {
    let (calls, run) = runner(0, "", "");
    let copier = verb_copier(BIN, Some(run));
    copier.remove("/Users/dev/repo", "/Users/dev/repo-pricing-page", CopyRoad::Worktree).await.unwrap();
    assert_eq!(
        calls.lock().unwrap()[0].1,
        s(&["copy", "remove", "--from", "/Users/dev/repo", "--to", "/Users/dev/repo-pricing-page", "--road", "worktree"])
    );
    let (_, refused) = runner(1, "", "worktree is not a road this computer has\n");
    let err = verb_copier(BIN, Some(refused)).remove("/a", "/b", CopyRoad::Worktree).await.unwrap_err();
    assert!(err.to_string().contains("worktree is not a road this computer has"), "{err}");
}

// TS: packages/engine/test/copier.test.ts:171
// "is the kernel's reading on the real road, and on the stand-in only what a test hands it"
#[tokio::test]
async fn is_the_kernels_reading_on_the_real_road_and_on_the_stand_in_only_what_a_test_hands_it() {
    if cfg!(target_os = "linux") {
        assert!(verb_copier("/nowhere/wsp-daemon", None).room().await.unwrap().total_mb > 0);
    }
    assert!(!fake_copier(None, None).has_room());
    let reading: RoomReading = Arc::new(|| Box::pin(async { Some(MemoryHere { free_mb: 900, total_mb: 16_384 }) }));
    let copier = fake_copier(None, Some(reading));
    assert!(copier.has_room());
    assert_eq!(copier.room().await, Some(MemoryHere { free_mb: 900, total_mb: 16_384 }));
}

// TS: packages/engine/test/copier.test.ts:180
// "records what it was asked for and answers a report of the road it was told to take"
#[tokio::test]
async fn records_what_it_was_asked_for_and_answers_a_report_of_the_road_it_was_told_to_take() {
    let copier = fake_copier(None, None);
    let asked = CopyAsk {
        from: "/a".to_string(),
        to: "/b".to_string(),
        base: None,
        exclude: s(&[".next"]),
        size_line_bytes: 99.into(),
        road: Some(CopyRoad::Worktree),
    };
    let made = copier.make(&asked).await.unwrap();
    assert_eq!(made.road, CopyRoad::Worktree);
    assert_eq!(made.path, "/b");
    assert_eq!(made.excluded, s(&[".next"]));
    assert_eq!(copier.log.lock().unwrap().asks, vec![asked]);
    copier.remove("/a", "/b", CopyRoad::Clonefile).await.unwrap();
    assert_eq!(copier.log.lock().unwrap().removed, vec![("/a".to_string(), "/b".to_string(), CopyRoad::Clonefile)]);
}
