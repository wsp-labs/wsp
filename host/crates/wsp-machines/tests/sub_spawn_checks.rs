// SPDX-License-Identifier: AGPL-3.0-only
//! spec/sub_spawn_checks.md: the record a sub-thread's spawn writes and every check before it, ported from
//! packages/runtime/test/sub-spawn.test.ts and sub-start-events.test.ts's first case.
mod machines_b_support;

use serde_json::{json, Value};

use machines_b_support::spawn_ctx::*;
use machines_b_support::*;
use wsp_core::clock::Cancel;
use wsp_core::types::internal::{KindModule, SessionEntry, SubStartRecord, GONE_REASON, SUB_STARTS};
use wsp_core::types::wiring::{LiveWorkspace, StageReport, SubRecord, WorkspaceRecord};
use wsp_machines::machines::area_error::{AreaError, AreaResult};
use wsp_machines::machines::machines::Entry;
use wsp_machines::machines::sub_spawn::*;
use wsp_wire::types::sub_thread::SUB_SEED_MAX_BYTES;
use wsp_words::exit::{HostError, Kind};
use wsp_words::sub_thread::*;
use wsp_words::words::thread::refusal_line;
use wsp_words::words::units::{fmt_bytes, starts_used_line, StartWindow};

fn asked() -> SpawnAsk {
    SpawnAsk { name: NAME.to_string(), project: "pr_1".to_string(), on: "boat".to_string(), ..SpawnAsk::default() }
}

fn named(name: &str) -> SpawnAsk {
    SpawnAsk { name: name.to_string(), ..asked() }
}

async fn spawn(f: &Fixture, ask: SpawnAsk) -> Result<SpawnAnswer, HostError> {
    let by = lead_caller();
    f.area.spawn_sub(ask, Some(&by)).await
}

async fn refused(f: &Fixture, ask: SpawnAsk) -> HostError {
    spawn(f, ask).await.expect_err("the spawn went ahead")
}

/// Nothing was asked of the cloud and nothing was handed to the queue; the lead's daemon was only read.
fn untouched(f: &Fixture) {
    assert!(f.host.backend.creates.lock().unwrap().is_empty(), "the cloud was asked for a machine");
    assert!(f.host.backend.machines.lock().unwrap().is_empty());
    assert!(f.host.calls_of("enqueue").is_empty() && f.host.calls_of("fork").is_empty());
    assert_eq!(f.host.store.borrow().get("workspaces").map(|c| c.len()).unwrap_or(0), 0);
    assert!(ops(f).iter().all(|op| op == "git.status"), "something besides a read went to the lead's daemon");
}

fn ops(f: &Fixture) -> Vec<String> {
    f.host.calls_of("daemon_frame").iter().map(|c| serde_json::from_str::<Value>(&c[1]).unwrap()["op"].as_str().unwrap().to_string()).collect()
}

fn enqueued(n: usize) -> Value {
    serde_json::from_str(&w().borrow().enqueued[n]).unwrap()
}

fn small() -> SubSize {
    sizes_sold(&["small"]).remove(0)
}

fn preamble_of(o: impl FnOnce(&mut SubPreamble)) -> String {
    let mut p = SubPreamble {
        on: "Boat".into(),
        size: small(),
        lead: LEAD_THREAD.into(),
        folder: FOLDER.into(),
        sent: vec![],
        setup_git: "gh auth setup-git".into(),
        repo: REPO.into(),
        base: BaseAt { branch: "feat/upload".into(), sha: BASE_SHA.into() },
        branch: BRANCH.into(),
        delete_after_minutes: 30.0,
        brokered: None,
    };
    o(&mut p);
    sub_preamble(&p)
}

struct Repo {
    _dir: tempfile::TempDir,
    repo: String,
    bare: String,
}

fn git(repo: &str, args: &[&str]) -> String {
    let o = std::process::Command::new("git").arg("-C").arg(repo).args(["-c", "user.name=t", "-c", "user.email=t@t"]).args(args).output().expect("git runs");
    assert!(o.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&o.stderr));
    String::from_utf8_lossy(&o.stdout).trim().to_string()
}

/// The lead's folder as a real repo on feat/upload with a bare remote beside it.
fn real_repo() -> Repo {
    let dir = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(dir.path()).unwrap();
    let repo = root.join("web").to_string_lossy().to_string();
    let bare = root.join("remote.git").to_string_lossy().to_string();
    std::fs::create_dir(&repo).unwrap();
    assert!(std::process::Command::new("git").args(["init", "-q", "-b", "feat/upload", &repo]).status().unwrap().success());
    assert!(std::process::Command::new("git").args(["init", "-q", "--bare", &bare]).status().unwrap().success());
    git(&repo, &["commit", "-q", "--allow-empty", "-m", "first"]);
    git(&repo, &["remote", "add", "origin", &bare]);
    Repo { _dir: dir, repo, bare }
}

/// A commit on the remote alone, made off the lead's head without moving it, pushed to `r#ref` there.
fn pushed_elsewhere(repo: &str, r#ref: &str, message: &str) -> String {
    let sha = git(repo, &["commit-tree", "HEAD^{tree}", "-p", "HEAD", "-m", message]);
    git(repo, &["push", "-q", "origin", &format!("{sha}:{}", r#ref)]);
    sha
}

fn lead_at(r: &Repo) -> Fixture {
    let f = lead_on_this_computer();
    lead_folder_at(&r.repo);
    real_git();
    f
}

#[test]
fn writes_the_record_before_the_cloud_and_answers_out_notice_and_preamble() {
    run(async {
        let f = lead_on_this_computer();
        f.host.pick(&["CLAUDE.local.md", "local.properties"]);
        {
            let world = w();
            let mut world = world.borrow_mut();
            world.git.uncommitted = 2;
            world.git.ignored = 3;
            world.read_ms = 40.0;
        }
        let began = f.host.clock.now();
        let made = spawn(&f, asked()).await.unwrap();
        // The record stands before the cloud is asked: the machine is made once the thread's start comes.
        assert!(f.host.backend.creates.lock().unwrap().is_empty());
        assert_eq!(
            js_of(&made.sub.out),
            js_text(&json!({
                "on": "boat", "machine": NAME, "size": { "cpu": 2, "memMb": 4096 }, "rateUsdPerHour": 0.018, "repo": REPO,
                "base": { "branch": "feat/upload", "sha": BASE_SHA }, "branch": BRANCH,
                "sent": { "paths": ["CLAUDE.local.md", "local.properties"], "bytes": 1243, "missing": [] },
                "notSent": { "uncommitted": 2 }, "deleteAfterMinutes": 30
            }))
        );
        assert_eq!((made.sub.folder.as_str(), made.sub.harness.as_str()), (FOLDER, "claude"));
        assert_eq!(made.notice.as_deref(), Some("2 uncommitted changes in web did not travel; the sub-thread starts at feat/upload as pushed (4be1c0a)."));
        let preamble = [
            "This thread runs on a machine of its own on Boat (small, 2 vCPU, 4 GB), started by thread t_lead. The code is not here yet. The folder you are in, /root/web, holds only the git-ignored files the person picked: CLAUDE.local.md, local.properties. Get the code first, in this folder:",
            "",
            "gh auth setup-git",
            "git init -q",
            "git remote add origin https://github.com/acme/web.git",
            "git fetch -q --filter=blob:none origin feat/upload",
            "git checkout -q -b wsp/fix-the-flaky-upload-test 4be1c0a9d2c4e1f07a3b9e5d6c8f2a1b0e9d7c6a",
            "",
            "Work on wsp/fix-the-flaky-upload-test, then commit and run git push -u origin wsp/fix-the-flaky-upload-test before your turn ends. Thread t_lead merges that branch. This machine is deleted 30 minutes after your last turn ends, and anything not pushed is lost with it.",
            "",
            "The task:",
        ]
        .join("\n");
        assert_eq!(made.sub.preamble, preamble);
        assert_eq!(made.view, json!({ "id": SPAWNED_ID, "name": NAME, "phase": "running", "machineId": "" }));
        // The record the queue was handed, field by field in the TS key order.
        assert_eq!(w().borrow().enqueued.len(), 1);
        assert_eq!(
            w().borrow().enqueued[0],
            js_text(&json!({
                "id": SPAWNED_ID, "name": NAME, "kind": "cloud", "machineId": "", "phase": "running", "golden": "wsp-host-default-v1", "createdAt": "2025-10-09T08:53:20.040Z",
                "project": "pr_1", "spec": {}, "place": "box", "size": { "cpu": 2, "memMb": 4096 }, "firstLife": true,
                "parentThreadId": LEAD_THREAD, "rootThreadId": LEAD_THREAD, "lead": { "threadId": LEAD_THREAD, "workspaceId": LEAD_WS },
                "sub": {
                    "on": "boat", "repo": REPO, "base": { "branch": "feat/upload", "sha": BASE_SHA }, "branch": BRANCH, "folder": FOLDER, "rateUsdPerHour": 0.018, "sizeName": "small",
                    "spawn": { "startedAt": began, "branch": { "branch": "feat/upload", "sha": BASE_SHA, "ms": 40 }, "uncommitted": 2, "log": [] }
                }
            }))
        );
        assert_eq!(f.host.stored(SPAWNED_ID).unwrap()["phase"], json!("running"));
        // The hand-off, in order, and the wait the queue reads now.
        let order: Vec<String> = f.host.names().into_iter().filter(|n| ["forking_add", "supersede_failed", "enqueue", "forking_remove"].contains(&n.as_str())).collect();
        assert_eq!(order, ["forking_add", "supersede_failed", "enqueue", "forking_remove"]);
        assert_eq!(f.host.calls_of("forking_add"), vec![vec![NAME.to_string()]]);
        assert_eq!(f.host.calls_of("supersede_failed"), vec![vec![NAME.to_string()]]);
        assert_eq!(f.host.calls_of("forking_remove"), vec![vec![NAME.to_string()]]);
        assert_eq!(f.host.calls_of("queue_wait"), vec![vec!["box".to_string(), "limits".to_string()]]);
        assert_eq!(f.host.calls_of("enqueue"), vec![vec![SPAWNED_ID.to_string(), "none".to_string()]]);
        assert_eq!(f.host.calls_of("brokered"), vec![vec!["cloud".to_string(), "box".to_string()]]);
        // wsp asked the lead's daemon one read in its folder and pushed nothing.
        let frames = f.host.calls_of("daemon_frame");
        assert_eq!(frames.len(), 1);
        assert_eq!((frames[0][0].as_str(), serde_json::from_str::<Value>(&frames[0][1]).unwrap()), (LEAD_WS, json!({ "op": "git.status", "cwd": LEAD_FOLDER })));
        assert_eq!(f.host.calls_of("copy_blocked"), vec![vec![LEAD_WS.to_string()]]);
        assert_eq!(f.host.calls_of("copy_of"), vec![vec!["box".to_string(), "default".to_string()]]);
        assert_eq!(w().borrow().held.get(), 0);
    });
}

#[test]
fn leaves_values_and_keys_here_whatever_the_picks_say() {
    run(async {
        let f = lead_on_this_computer();
        {
            let world = w();
            let mut world = world.borrow_mut();
            world.menu = vec![(".env".into(), 120, "secret".into()), ("dev.pem".into(), 900, "secret".into()), ("CLAUDE.local.md".into(), 1100, "config".into())];
            world.left = vec!["deploy/tls.key".into()];
        }
        f.host.pick(&[".env", "dev.pem", "CLAUDE.local.md"]);
        let made = spawn(&f, asked()).await.unwrap();
        assert_eq!(js_of(&made.sub.out.sent), js_text(&json!({ "paths": ["CLAUDE.local.md"], "bytes": 1100, "missing": [] })));
        // Only the pick that travels was packed; every path that stayed is named in one line, a key the pack found inside a picked folder among them.
        assert_eq!(w().borrow().packs, vec![vec!["CLAUDE.local.md".to_string()]]);
        assert_eq!(made.notice, Some(sub_stayed_line(&[".env", "dev.pem", "deploy/tls.key"])));
        assert_eq!(made.sub.out.not_sent.why, None);
    });
}

#[test]
fn says_why_nothing_travels_where_the_only_pick_is_values() {
    run(async {
        let f = lead_on_this_computer();
        w().borrow_mut().menu = vec![(".env".into(), 120, "secret".into())];
        f.host.pick(&[".env"]);
        let made = spawn(&f, asked()).await.unwrap();
        assert_eq!(js_of(&made.sub.out.sent), js_text(&json!({ "paths": [], "bytes": 0, "missing": [] })));
        assert_eq!(made.notice, Some(sub_stayed_line(&[".env"])));
        assert_eq!(made.sub.out.not_sent.why, None);
        assert!(w().borrow().packs.is_empty() && f.host.calls_of("seed_pack").is_empty(), "an empty pick was packed");
        assert!(enqueued(0)["sub"]["spawn"].get("notSent").is_none());
    });
}

#[test]
fn sends_nothing_without_picks_and_names_a_pick_no_longer_there() {
    run(async {
        let f = lead_on_this_computer();
        let made = spawn(&f, asked()).await.unwrap();
        assert_eq!(js_of(&made.sub.out.sent), js_text(&json!({ "paths": [], "bytes": 0, "missing": [] })));
        assert_eq!(js_of(&made.sub.out.not_sent), js_text(&json!({ "uncommitted": 0, "why": "nothing is picked for web" })));
        assert_eq!(made.notice.as_deref(), Some("no git-ignored files were sent: nothing is picked for web. wsp projects seed web shows what could go."));
        assert!(made.sub.preamble.contains("The folder you are in, /root/web, is empty."));
        assert_eq!(enqueued(0)["sub"]["spawn"]["notSent"], json!("nothing is picked for web"));
        assert!(f.host.calls_of("seed_pack").is_empty());

        let again = lead_on_this_computer();
        again.host.pick(&["CLAUDE.local.md", "gone.json"]);
        let sent = spawn(&again, asked()).await.unwrap();
        assert_eq!(js_of(&sent.sub.out.sent), js_text(&json!({ "paths": ["CLAUDE.local.md"], "bytes": 1100, "missing": ["gone.json"] })));
        assert_eq!(w().borrow().packs, vec![vec!["CLAUDE.local.md".to_string()]]);
    });
}

#[test]
fn takes_the_size_the_lead_names_and_numbers_the_default_branch() {
    run(async {
        let f = lead_on_this_computer();
        let big = spawn(&f, SpawnAsk { size: Some("4x8".into()), ..asked() }).await.unwrap();
        assert_eq!(js_of(&big.sub.out.size), js_text(&json!({ "cpu": 4, "memMb": 8192 })));
        assert_eq!(serde_json::to_value(&big.sub.out.rate_usd_per_hour).unwrap(), json!(0.036));
        assert!(big.sub.preamble.contains("on Boat (default, 4 vCPU, 8 GB)"));
        let record = enqueued(0);
        assert_eq!((record["size"].clone(), record["sub"]["sizeName"].clone(), record["sub"]["rateUsdPerHour"].clone()), (json!({ "cpu": 4, "memMb": 8192 }), json!("default"), json!(0.036)));
        assert!(f.host.backend.creates.lock().unwrap().is_empty());
        // The next two default to the branch its name makes, numbered past one another sub-thread of this host holds.
        let twin = spawn(&f, named(&format!("{NAME} 2"))).await.unwrap();
        assert_eq!(twin.sub.out.branch, "wsp/fix-the-flaky-upload-test-2");
        let third = spawn(&f, named(&format!("{NAME}."))).await.unwrap();
        assert_eq!(third.sub.out.branch, "wsp/fix-the-flaky-upload-test-3");
        let named_branch = spawn(&f, SpawnAsk { branch: Some("wsp/upload-suite".into()), ..named("Run the suite") }).await.unwrap();
        assert_eq!(named_branch.sub.out.branch, "wsp/upload-suite");
        // With no size asked, the size the place keeps for sub-threads is the default.
        w().borrow_mut().kept_size = Some((4.0, 8192.0));
        let kept = spawn(&f, named("Kept size")).await.unwrap();
        assert_eq!(js_of(&kept.sub.out.size), js_text(&json!({ "cpu": 4, "memMb": 8192 })));
        assert_eq!(enqueued(4)["sub"]["sizeName"], json!("default"));
    });
}

#[test]
fn numbers_the_default_branch_past_one_the_remote_holds() {
    run(async {
        let f = lead_on_this_computer();
        *w().borrow().remote_heads.lock().unwrap() = vec![BRANCH.to_string(), format!("{BRANCH}-2")];
        let made = spawn(&f, asked()).await.unwrap();
        assert_eq!(made.sub.out.branch, format!("{BRANCH}-3"));
        assert!(made.sub.preamble.contains(&format!("git checkout -q -b {BRANCH}-3 ")));
        // Read in the lead's folder with no prompt.
        assert_eq!(w().borrow().local.log(), vec![format!("GIT_TERMINAL_PROMPT=0 git -C {LEAD_FOLDER} ls-remote --heads origin")]);
    });
}

#[test]
fn takes_the_leads_own_project_and_refuses_another() {
    run(async {
        let f = lead_on_this_computer();
        let by_name = spawn(&f, SpawnAsk { project: "web".into(), ..asked() }).await.unwrap();
        let by_none = spawn(&f, SpawnAsk { project: String::new(), ..named("Another one") }).await.unwrap();
        let by_id = spawn(&f, named("A third one")).await.unwrap();
        assert_eq!((by_name.sub.out.repo.as_str(), by_none.sub.out.repo.as_str(), by_id.sub.out.repo.as_str()), (REPO, REPO, REPO));
        let records = f.host.store.borrow().get("workspaces").map(|c| c.len());
        let other = refused(&f, SpawnAsk { project: "api".into(), ..named("A fourth one") }).await;
        assert_eq!(other, sub_other_project_refusal("boat", "api", "web"));
        assert_eq!(
            other.message,
            refusal_line("run --on boat starts a sub-thread of the thread asking, on that thread's project web, and api is another project", "Leave the project out, or run the work on api without --on.")
        );
        assert_eq!(class_of(&other), "usage");
        // Refused before the record is announced, so no window draws a row for it and nothing is asked of Boat.
        assert_eq!(f.host.store.borrow().get("workspaces").map(|c| c.len()), records);
        assert_eq!(f.host.calls_of("enqueue").len(), 3);
        assert!(f.host.backend.creates.lock().unwrap().is_empty());
    });
}

#[test]
fn refuses_a_size_the_plan_does_not_sell_before_any_create() {
    run(async {
        let f = lead_on_this_computer();
        *f.host.backend.limits.lock().unwrap() = Some(trial());
        let sold = sizes_sold(&["small", "default"]);
        let big = refused(&f, SpawnAsk { size: Some("8x16".into()), ..asked() }).await;
        assert_eq!(big, sub_size_refusal("Boat", "8x16", &sold));
        assert_eq!(big.message, "Boat sells small 2x4 ($0.018/h) and default 4x8 ($0.036/h) on this plan; 8x16 is not one of them. Name one of them, or leave the size out for Boat's own.");
        assert_eq!(class_of(&big), "usage");
        let odd = refused(&f, SpawnAsk { size: Some("3x5".into()), ..asked() }).await;
        assert_eq!(odd, sub_size_refusal("Boat", "3x5", &sold));
        untouched(&f);
        assert_eq!(w().borrow().held.get(), 0);
    });
}

#[test]
fn from_starts_at_the_remotes_commit_and_says_nothing_of_uncommitted() {
    run(async {
        let r = real_repo();
        let f = lead_at(&r);
        f.host.pick(&["CLAUDE.local.md"]);
        {
            let world = w();
            let mut world = world.borrow_mut();
            world.git.ahead = 1;
            world.git.uncommitted = 2;
        }
        let main = pushed_elsewhere(&r.repo, "refs/heads/main", "the suite moved to tests/k6");
        let made = spawn(&f, SpawnAsk { from: Some("origin/main".into()), ..asked() }).await.unwrap();
        // The record stands before the cloud is asked: the machine is made once the thread's start comes.
        assert!(f.host.backend.creates.lock().unwrap().is_empty());
        assert_eq!((made.sub.out.base.branch.as_str(), made.sub.out.base.sha.as_str()), ("main", main.as_str()));
        assert_eq!(made.sub.out.branch, BRANCH);
        assert_eq!(js_of(&made.sub.out.not_sent), js_text(&json!({ "uncommitted": 0 })));
        let lines = [
            "gh auth setup-git".to_string(),
            "git init -q".to_string(),
            "git remote add origin https://github.com/acme/web.git".to_string(),
            "git fetch -q --filter=blob:none origin main".to_string(),
            format!("git checkout -q -b {BRANCH} {main}"),
        ];
        assert!(made.sub.preamble.contains(&lines.join("\n")));
        assert_eq!(made.notice, None);
        assert_eq!(ops(&f), ["git.status"]);
        assert_eq!(enqueued(0)["sub"]["base"], json!({ "branch": "main", "sha": main }));
        assert!(enqueued(0)["sub"]["spawn"].get("uncommitted").is_none());
    });
}

#[test]
fn from_takes_a_tag_by_its_commit_and_a_commit_by_its_live_branch() {
    run(async {
        let r = real_repo();
        let f = lead_at(&r);
        {
            let world = w();
            let mut world = world.borrow_mut();
            world.git.upstream = None;
            world.git.default_branch = Some("trunk".into());
        }
        let tagged = pushed_elsewhere(&r.repo, "refs/heads/trunk", "the release");
        git(&r.repo, &["tag", "-a", "-m", "v1", "v1", &tagged]);
        git(&r.repo, &["push", "-q", "origin", "v1", &format!("{tagged}:refs/heads/a-side")]);
        // A branch whose name ends in v1 is not the tag v1, though ls-remote lists it for that name.
        pushed_elsewhere(&r.repo, "refs/heads/release/v1", "not the tag");
        let by_tag = spawn(&f, SpawnAsk { from: Some("v1".into()), ..named("From the tag") }).await.unwrap();
        assert_eq!((by_tag.sub.out.base.branch.as_str(), by_tag.sub.out.base.sha.as_str()), ("v1", tagged.as_str()));
        git(&r.repo, &["fetch", "-q", "origin"]);
        let by_commit = spawn(&f, SpawnAsk { from: Some(tagged[..9].to_string()), ..named("From the commit") }).await.unwrap();
        assert_eq!((by_commit.sub.out.base.branch.as_str(), by_commit.sub.out.base.sha.as_str()), ("trunk", tagged.as_str()));
        assert!(by_commit.sub.preamble.contains(&format!("git fetch -q --filter=blob:none origin trunk\ngit checkout -q -b wsp/from-the-commit {tagged}")));
        // A branch deleted on the remote that this folder still tracks, sorting first, is no branch to fetch the commit by.
        let aside = pushed_elsewhere(&r.repo, "refs/heads/z-live", "on a live branch alone");
        git(&r.repo, &["push", "-q", "origin", &format!("{aside}:refs/heads/a-gone")]);
        git(&r.repo, &["fetch", "-q", "origin"]);
        git(&r.bare, &["update-ref", "-d", "refs/heads/a-gone"]);
        assert!(git(&r.repo, &["branch", "-r", "--contains", &aside]).contains("origin/a-gone"));
        let by_live = spawn(&f, SpawnAsk { from: Some(aside[..9].to_string()), ..named("From the live branch") }).await.unwrap();
        assert_eq!((by_live.sub.out.base.branch.as_str(), by_live.sub.out.base.sha.as_str()), ("z-live", aside.as_str()));
        assert!(f.host.backend.creates.lock().unwrap().is_empty());
    });
}

#[test]
fn from_refuses_a_commit_no_remote_branch_holds_and_a_ref_the_remote_lacks() {
    run(async {
        let r = real_repo();
        let f = lead_at(&r);
        let local = git(&r.repo, &["rev-parse", "HEAD"]);
        let commit = refused(&f, SpawnAsk { from: Some(local[..12].to_string()), ..asked() }).await;
        assert_eq!(commit, sub_from_not_on_remote_refusal(&local[..12], None));
        assert_eq!(class_of(&commit), "usage");
        let branch = refused(&f, SpawnAsk { from: Some("feat/later".into()), ..asked() }).await;
        assert_eq!(branch.message, "feat/later is not on the remote. Run git push -u origin feat/later, then start the sub-thread again.");
        // The form the refusals teach: the push line names the branch, which git takes, and not origin/, which it refuses.
        let tracked = refused(&f, SpawnAsk { from: Some("origin/feat/later".into()), ..asked() }).await;
        assert_eq!(tracked.message, "origin/feat/later is not on the remote. Run git push -u origin feat/later, then start the sub-thread again.");
        untouched(&f);
        assert_eq!(w().borrow().held.get(), 0);
    });
}

#[test]
fn names_from_beside_the_push_line_where_the_branch_is_not_pushed() {
    run(async {
        let f = lead_on_this_computer();
        {
            let world = w();
            let mut world = world.borrow_mut();
            world.git.ahead = 1;
            world.git.default_branch = Some("trunk".into());
        }
        let e = refused(&f, asked()).await;
        assert_eq!(e.message, "feat/upload has 1 commit the remote does not. Run git push and start the sub-thread again, or start it from a ref the remote holds with --from origin/trunk.");
        assert_eq!(e, sub_ahead_refusal("feat/upload", 1.0, "origin/trunk"));
        untouched(&f);
    });
}

type Setup = Box<dyn Fn(&Fixture)>;

struct Row {
    name: &'static str,
    setup: Setup,
    ask: SpawnAsk,
    terminal: bool,
    want: Option<HostError>,
    class: &'static str,
}

fn row(name: &'static str, setup: impl Fn(&Fixture) + 'static, ask: SpawnAsk, want: HostError, class: &'static str) -> Row {
    Row { name, setup: Box::new(setup), ask, terminal: false, want: Some(want), class }
}

fn broker_up(gh: bool) {
    let world = w();
    world.borrow_mut().broker_ready = true;
    world.borrow().gh_signed_in.store(gh, std::sync::atomic::Ordering::SeqCst);
}

fn git_change(f: impl Fn(&mut LeadGit) + 'static) -> impl Fn(&Fixture) {
    move |_| f(&mut w().borrow_mut().git)
}

fn remote_is(remote: &'static str) -> impl Fn(&Fixture) {
    move |_| w().borrow_mut().project = project_json("web", "here", LEAD_FOLDER, remote)
}

fn starts_used(window: &'static str) -> impl Fn(&Fixture) {
    move |f| {
        let mut l = paid();
        let s = l.starts.as_mut().unwrap();
        match window {
            "hour" => s.hour.used = 60.0,
            _ => s.day.used = 200.0,
        }
        *f.host.backend.limits.lock().unwrap() = Some(l);
    }
}

#[test]
fn every_check_refuses_before_the_cloud_is_asked() {
    run(async {
        let cloud = GitHostWords { label: "GitHub".into(), cli: "gh".into(), login: "GitHub CLI login".into() };
        let paid_sizes = sizes_sold(&["small", "default", "large"]);
        let from = |r: &str| SpawnAsk { from: Some(r.to_string()), ..asked() };
        let branch = |b: &str| SpawnAsk { branch: Some(b.to_string()), ..asked() };
        let harness = |h: &str| SpawnAsk { harness: Some(h.to_string()), ..asked() };
        let mut rows = vec![
            Row { terminal: true, ..row("S1 a line from no thread", |_| {}, asked(), sub_no_thread_refusal("boat"), "usage") },
            row("S3 a cloud this host holds no key for", |_| {}, SpawnAsk { on: "solari".into(), ..asked() }, sub_no_cloud_refusal("solari", &["Boat"]), "usage"),
            row("S4 a folder with no remote", remote_is(""), asked(), sub_no_remote_refusal("web", "Boat"), "usage"),
            row("S4 a remote on another host", remote_is("https://git.example.com/acme/web.git"), asked(), sub_other_host_refusal("web", "git.example.com", "Boat"), "usage"),
            row("S5 a lead on no branch", git_change(|g| g.head = "(detached)".into()), asked(), sub_detached_refusal("web", "origin/main"), "usage"),
            row("S5 a branch the remote lacks", git_change(|g| g.upstream = None), asked(), sub_no_upstream_refusal("feat/upload", "origin/main"), "usage"),
            row("S5 commits the remote lacks", git_change(|g| g.ahead = 2), asked(), sub_ahead_refusal("feat/upload", 2.0, "origin/main"), "usage"),
            row("S5 commits git could not count", git_change(|g| g.counts_unknown = true), asked(), sub_ahead_unknown_refusal("feat/upload", "origin/main"), "usage"),
            row("S5 a ref --from names the remote lacks", |_| {}, from("origin/nope"), sub_from_not_on_remote_refusal("origin/nope", Some("nope")), "usage"),
            row("S6 no image on Boat", |_| w().borrow_mut().image = None, asked(), sub_no_image_refusal("Boat"), "usage"),
            row("S6 an image with no GitHub sign-in", |_| w().borrow_mut().image.as_mut().unwrap().logins = vec![("GitHub CLI login".into(), "skipped".into())], asked(), sub_no_sign_in_refusal("Boat", &cloud, "acme/web"), "usage"),
            row("S6 gh on this computer signed in to nothing, the broker ready", |_| broker_up(false), asked(), sub_host_no_sign_in_refusal("Boat", "acme/web"), "usage"),
            row("S6 a remote on GitLab, the broker ready", |f| { broker_up(true); remote_is("git@gitlab.com:acme/web.git")(f) }, asked(), sub_host_other_git_refusal("Boat", "GitLab", "acme/web"), "usage"),
            row("S7 a size the plan does not sell", |_| {}, SpawnAsk { size: Some("16x32".into()), ..asked() }, sub_size_refusal("Boat", "16x32", &paid_sizes), "usage"),
            row("S8 a name git refuses", |_| {}, branch("wsp/bad..name"), sub_branch_name_refusal("wsp/bad..name"), "usage"),
            row("S8 the lead's own branch", |_| {}, branch("feat/upload"), sub_lead_branch_refusal("feat/upload"), "usage"),
            row("S8 HEAD", |_| {}, branch("HEAD"), sub_branch_name_refusal("HEAD"), "usage"),
            row("S8 a part ending in .lock", |_| {}, branch("feat/x.lock/y"), sub_branch_name_refusal("feat/x.lock/y"), "usage"),
            row("S10 the day's spend reached", |_| w().borrow_mut().spend_capped = true, asked(), HostError::new(wsp_words::words::computer::spend_cap_refusal("box", 0.0, 0.0)), "provider"),
            row(
                "S11 an agent the broker adds no key for, the broker ready",
                |_| {
                    broker_up(true);
                    w().borrow_mut().image.as_mut().unwrap().agents = vec!["claude".into(), "codex".into(), "opencode".into()];
                },
                harness("opencode"),
                wsp_words::place_name::brokered_agent_refusal("OpenCode", &["Claude Code", "Codex"]),
                "usage",
            ),
            row("S11 an agent the image does not carry", |_| w().borrow_mut().image.as_mut().unwrap().agents = vec!["codex".into()], harness("claude"), sub_no_agent_refusal("Boat", "claude", &["codex"]), "usage"),
            Row {
                want: None,
                ..row(
                    "S12 picks past what a sub-thread carries",
                    |f| {
                        w().borrow_mut().menu = vec![("data.bin".into(), SUB_SEED_MAX_BYTES + 1024, "data".into()), ("CLAUDE.local.md".into(), 100, "config".into())];
                        f.host.pick(&["data.bin", "CLAUDE.local.md"]);
                    },
                    asked(),
                    HostError::new(""),
                    "usage",
                )
            },
            row("S13 the hour's starts used", starts_used("hour"), asked(), HostError::new(starts_used_line(StartWindow::Hour, 60.0, 60.0, Some("the $20 plan"), Some(&format!("sub-thread {NAME}")))), "provider"),
            row("S13 the day's starts used", starts_used("day"), asked(), HostError::new(starts_used_line(StartWindow::Day, 200.0, 200.0, Some("the $20 plan"), Some(&format!("sub-thread {NAME}")))), "provider"),
        ];
        assert_eq!(rows.len(), 24);
        // Two more the TS covers elsewhere: a daemon that cannot read the lead's branch, and a name another thread holds.
        rows.push(row("S5 a daemon that cannot read the branch", |_| w().borrow_mut().status_error = Some("the daemon is not answering".into()), asked(), HostError::new(wsp_words::words::computer::branch_unread_refusal("web", "the daemon is not answering")), "provider"));
        rows.push(row("a name another thread holds", |_| w().borrow_mut().taken_names = vec![NAME.to_string()], asked(), HostError::with_kind(format!("a thread named {NAME} already exists"), Kind::Conflict), "provider"));

        for row in rows {
            let f = lead_on_this_computer();
            (row.setup)(&f);
            let by = lead_caller();
            let got = f.area.spawn_sub(row.ask.clone(), if row.terminal { None } else { Some(&by) }).await.expect_err(&format!("{}: the spawn went ahead", row.name));
            match &row.want {
                Some(want) => assert_eq!(&got, want, "{}", row.name),
                None => assert!(got.message.starts_with("the files picked for web come to "), "{}: {}", row.name, got.message),
            }
            assert_eq!(class_of(&got), row.class, "{}", row.name);
            untouched(&f);
            // The place is taken once for every line that has a thread, and freed on every exit.
            assert_eq!(f.host.calls_of("spawn_guard").len(), if row.terminal { 0 } else { 1 }, "{}", row.name);
            assert_eq!(w().borrow().held.get(), 0, "{}: a refused spawn holds its slot", row.name);
        }
    });
}

#[test]
fn s6_asks_gh_here_in_place_of_the_image_while_the_broker_is_ready() {
    run(async {
        let f = lead_on_this_computer();
        broker_up(true);
        w().borrow_mut().image.as_mut().unwrap().logins = vec![("GitHub CLI login".into(), "skipped".into())];
        let made = spawn(&f, asked()).await.unwrap();
        assert_eq!(w().borrow().local.log()[0], "gh auth token --hostname github.com > /dev/null 2>&1");
        // Where the host keeps the machine's credentials, the record names the repo by its id and the branch its pushes may land on.
        assert_eq!(f.host.calls_of("repo_of_record"), vec![vec!["git@github.com:acme/web.git".to_string(), "web".to_string(), "Boat".to_string()]]);
        let record = w().borrow().enqueued[0].clone();
        let at = format!(r#""branchOfRecord":{{"repoId":4242,"owner":"acme","name":"web","branch":"{BRANCH}","startSha":"{BASE_SHA}"}}"#);
        assert!(record.contains(&format!(r#""lead":{{"threadId":"{LEAD_THREAD}","workspaceId":"{LEAD_WS}"}},{at},"sub":"#)), "{record}");
        assert_eq!(made.sub.preamble, preamble_of(|p| p.brokered = Some(true)));
    });
}

#[test]
fn s6_asks_the_image_while_the_broker_is_not_ready() {
    run(async {
        let f = lead_on_this_computer();
        w().borrow().gh_signed_in.store(true, std::sync::atomic::Ordering::SeqCst);
        w().borrow_mut().image.as_mut().unwrap().logins = vec![("GitHub CLI login".into(), "skipped".into())];
        let got = refused(&f, asked()).await;
        assert_eq!(got, sub_no_sign_in_refusal("Boat", &GitHostWords { label: "GitHub".into(), cli: "gh".into(), login: "GitHub CLI login".into() }, "acme/web"));
        assert_eq!(class_of(&got), "usage");
        // gh on this computer is never asked.
        assert!(w().borrow().local.log().iter().all(|c| !c.contains("gh ")), "gh was asked: {:?}", w().borrow().local.log());
        untouched(&f);
    });
}

#[test]
fn s12_names_the_largest_picks_and_the_sizes() {
    run(async {
        let big = SUB_SEED_MAX_BYTES + 1024;
        let f = lead_on_this_computer();
        w().borrow_mut().menu = vec![("data.bin".into(), big, "data".into()), ("CLAUDE.local.md".into(), 100, "config".into())];
        f.host.pick(&["data.bin", "CLAUDE.local.md"]);
        let got = refused(&f, asked()).await;
        let tar = tar_of(&[("data.bin", big), ("CLAUDE.local.md", 100)]).len() as f64;
        let largest = [format!("data.bin ({})", fmt_bytes(big as f64)), format!("CLAUDE.local.md ({})", fmt_bytes(100.0))];
        assert_eq!(got, sub_seed_too_big_refusal("web", &fmt_bytes(tar), &fmt_bytes(SUB_SEED_MAX_BYTES as f64), &largest));
        assert_eq!(class_of(&got), "usage");
        untouched(&f);

        // Of three picks, the two largest by the plan's own bytes, the largest first.
        let g = lead_on_this_computer();
        w().borrow_mut().menu = vec![("CLAUDE.local.md".into(), 100, "config".into()), ("data.bin".into(), big, "data".into()), ("local.properties".into(), 143, "config".into())];
        g.host.pick(&["CLAUDE.local.md", "data.bin", "local.properties"]);
        let three = refused(&g, asked()).await;
        let tar = tar_of(&[("CLAUDE.local.md", 100), ("data.bin", big), ("local.properties", 143)]).len() as f64;
        let largest = [format!("data.bin ({})", fmt_bytes(big as f64)), format!("local.properties ({})", fmt_bytes(143.0))];
        assert_eq!(three, sub_seed_too_big_refusal("web", &fmt_bytes(tar), &fmt_bytes(SUB_SEED_MAX_BYTES as f64), &largest));

        // An archive of exactly the most a sub-thread carries is allowed.
        let h = lead_on_this_computer();
        w().borrow_mut().menu = vec![("d".into(), SUB_SEED_MAX_BYTES - 2, "data".into())];
        h.host.pick(&["d"]);
        let made = spawn(&h, asked()).await.unwrap();
        assert_eq!(made.sub.out.sent.paths, vec!["d".to_string()]);
    });
}

#[test]
fn refuses_a_lead_on_a_forked_machine_before_any_create() {
    run(async {
        let f = lead_on_this_computer();
        f.host.put_lead(lead_record("cloud", "cloud-lead"), FakeMachine::new("bx_9"));
        let got = refused(&f, asked()).await;
        assert_eq!(got, sub_lead_on_machine_refusal("Boat", LEAD_THREAD, "cloud-lead"));
        assert_eq!(class_of(&got), "usage");
        // The lead is never read: no daemon frame went to it.
        assert!(f.host.calls_of("daemon_frame").is_empty());
        untouched(&f);
        assert_eq!(w().borrow().held.get(), 0);
    });
}

#[test]
fn starts_from_a_lead_on_an_added_computer_and_says_no_picks_went() {
    run(async {
        let f = lead_on_a_box();
        let by = caller("t_box", LEAD_WS, "t_box");
        let made = f.area.spawn_sub(SpawnAsk { project: String::new(), ..asked() }, Some(&by)).await.unwrap();
        let why = sub_picked_elsewhere_why("lab-box", "hetzner");
        assert_eq!(js_of(&made.sub.out.not_sent), js_text(&json!({ "uncommitted": 0, "why": why })));
        assert_eq!(made.notice, Some(wsp_words::sub_thread::sub_not_sent_line(&why, "lab-box")));
        assert_eq!(made.notice.as_deref(), Some("no git-ignored files were sent: lab-box lives on hetzner, and picks are read from folders on this computer only."));
        assert_eq!(made.sub.folder, "/root/lab");
        assert_eq!(made.sub.out.repo, "https://github.com/acme/lab.git");
        // The lead's branch was read through its own daemon in its own folder, and the picks are not looked up.
        let frames = f.host.calls_of("daemon_frame");
        assert_eq!(serde_json::from_str::<Value>(&frames[0][1]).unwrap(), json!({ "op": "git.status", "cwd": "/root/lab-box" }));
        assert!(f.host.calls_of("get_seed_choice").is_empty() && f.host.calls_of("seed_pack").is_empty());
        assert_eq!(enqueued(0)["lead"], json!({ "threadId": "t_box", "workspaceId": LEAD_WS }));
    });
}

#[test]
fn numbers_past_the_remotes_branches_read_in_the_leads_folder_on_its_box() {
    run(async {
        let f = lead_on_a_box();
        *w().borrow().lead_remote_heads.lock().unwrap() = vec!["main".to_string(), BRANCH.to_string()];
        let by = caller("t_box", LEAD_WS, "t_box");
        let made = f.area.spawn_sub(SpawnAsk { project: String::new(), ..asked() }, Some(&by)).await.unwrap();
        assert_eq!(made.sub.out.branch, format!("{BRANCH}-2"));
        // Read in the lead's own folder there, through the box's own road, with no prompt; nothing was asked of this computer's git.
        let lead = w().borrow().lead_machine.clone();
        assert_eq!(lead.log(), vec!["GIT_TERMINAL_PROMPT=0 git -C /root/lab-box ls-remote --heads origin".to_string()]);
        assert!(w().borrow().local.log().is_empty());
    });
}

#[test]
fn the_record_is_queued_under_the_lead_before_any_machine() {
    run(async {
        let f = lead_on_this_computer();
        w().borrow_mut().menu = vec![(".env".into(), 120, "secret".into())];
        f.host.pick(&[".env"]);
        let made = spawn(&f, named("Kid")).await.unwrap();
        // The record comes first with its lead and its branch step, while the cloud has made nothing.
        let record = f.host.stored(&made.view["id"].as_str().unwrap().to_string()).unwrap();
        assert_eq!((record["phase"].clone(), record["machineId"].clone()), (json!("running"), json!("")));
        assert_eq!((record["parentThreadId"].clone(), record["rootThreadId"].clone()), (json!(LEAD_THREAD), json!(LEAD_THREAD)));
        assert_eq!(record["lead"], json!({ "threadId": LEAD_THREAD, "workspaceId": LEAD_WS }));
        assert_eq!(record["sub"]["spawn"]["branch"]["branch"], json!("feat/upload"));
        assert!(f.host.calls_of("fork").is_empty() && f.host.backend.creates.lock().unwrap().is_empty());
        assert!(f.host.at("forking_add") < f.host.at("enqueue"));
        // Its entry is the stand-in the queue holds it by: no machine yet.
        let entry = f.host.live.borrow().get(SPAWNED_ID).cloned().expect("the queue holds the record");
        assert_eq!(entry.borrow().machine.id(), "");
        assert_eq!(made.notice, Some(sub_stayed_line(&[".env"])));
    });
}

#[test]
fn a_refusal_after_the_guard_frees_the_place() {
    run(async {
        // A refusal after the take.
        let f = lead_on_this_computer();
        let got = refused(&f, SpawnAsk { size: Some("16x32".into()), ..asked() }).await;
        assert_eq!(class_of(&got), "usage");
        assert_eq!((f.host.calls_of("spawn_guard").len(), w().borrow().held.get()), (1, 0));
        // A spawn that goes through frees it too.
        spawn(&f, asked()).await.unwrap();
        assert_eq!((f.host.calls_of("spawn_guard").len(), w().borrow().held.get()), (2, 0));
        // The guard's own refusal comes out in the usage class where it carries no kind, and nothing after it runs.
        let g = lead_on_this_computer();
        g.host.script("spawn_guard", Err(AreaError::Host(HostError::new("the switch is off"))));
        let off = refused(&g, asked()).await;
        assert_eq!((off.message.as_str(), off.kind.clone(), class_of(&off)), ("the switch is off", Some(Kind::Usage), "usage"));
        assert!(g.host.calls_of("computer_rows").is_empty());
        assert_eq!(w().borrow().held.get(), 0);
        // A kind it carries stays.
        let h = lead_on_this_computer();
        h.host.script("spawn_guard", Err(AreaError::Host(HostError::with_kind("too many machines", Kind::Concurrency))));
        let full = refused(&h, asked()).await;
        assert_eq!((full.message.as_str(), full.kind), ("too many machines", Some(Kind::Concurrency)));
        untouched(&h);
    });
}

#[test]
fn sold_by_reads_the_plan_classes_else_the_backend_sizes() {
    run(async {
        // With the plan read: the classes it sells, by the class's own name and rate.
        let f = lead_on_this_computer();
        let planned = refused(&f, SpawnAsk { size: Some("3x5".into()), ..asked() }).await;
        assert_eq!(planned, sub_size_refusal("Boat", "3x5", &sizes_sold(&["small", "default", "large"])));
        assert_eq!(f.host.calls_of("queue_wait").len(), 0);

        // Without it: every size the backend offers, named by its figures, at the rate the backend gives it.
        let g = lead_on_this_computer();
        *g.host.backend.limits.lock().unwrap() = None;
        let offered: Vec<SubSize> = [("2x4", 2.0, 4096.0, 0.018), ("4x8", 4.0, 8192.0, 0.036), ("8x16", 8.0, 16384.0, 0.072), ("16x32", 16.0, 32768.0, 0.2)]
            .iter()
            .map(|(n, cpu, mem, rate)| SubSize { name: n.to_string(), cpu: *cpu, mem_mb: *mem, rate_usd_per_hour: *rate })
            .collect();
        let backend = refused(&g, SpawnAsk { size: Some("3x5".into()), ..asked() }).await;
        assert_eq!(backend, sub_size_refusal("Boat", "3x5", &offered));
        // The default size of the backend when none is asked, and its largest at the rate the backend itself names.
        let dflt = spawn(&g, asked()).await.unwrap();
        assert_eq!(js_of(&dflt.sub.out.size), js_text(&json!({ "cpu": 2, "memMb": 4096 })));
        let largest = spawn(&g, SpawnAsk { size: Some("16x32".into()), ..named("The largest") }).await.unwrap();
        assert_eq!(serde_json::to_value(&largest.sub.out.rate_usd_per_hour).unwrap(), json!(0.2));
        assert_eq!((enqueued(0)["sub"]["sizeName"].clone(), enqueued(1)["sub"]["sizeName"].clone()), (json!("2x4"), json!("16x32")));
        assert_eq!(g.host.calls_of("queue_wait")[0], vec!["box".to_string(), "no limits".to_string()]);
    });
}
