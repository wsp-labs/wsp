// SPDX-License-Identifier: AGPL-3.0-only
//! Red tests for `projects/branch_of_record` (`spec/branch_of_record.md`): the gh and git reads of a branch of record,
//! the public reread, the marketplaces read, the op token and the land. Ports of the cases of
//! `branch-of-record.test.ts` that reach `branch-of-record.ts`'s functions, at the seam of a context that answers
//! scripted gh and git output per command; the rest are the spec's rules. The cases that check only `project-landing.ts`
//! lines or the whole runtime's start are not ported here.
mod branch_support;
mod rest_support;

use std::cell::RefCell;
use std::future::Future;
use std::rc::Rc;
use std::time::Duration;

use futures::FutureExt;
use serde_json::{json, Value};

use branch_support::*;
use rest_support::{plain_error, run};
use wsp_core::devices::MintMore;
use wsp_core::types::wiring::WorkspaceRecord;
use wsp_machines::projects::branch_of_record::*;
use wsp_wire::types::shell_quote::{shell_line, ENV_FROM_INPUT};
use wsp_wire::views::workspace_more::WorkspaceOrigin;
use wsp_words::exit::{HostError, Kind};
use wsp_words::projects::{clone_on_machine_failed_line, CloneFailed};
use wsp_words::sub_thread::{
    branch_of_record_unread_refusal, brokered_git_hub_only_refusal, clone_no_thread_refusal,
    fork_head_gone_refusal, fork_not_pushable_refusal,
};

const REPO_JQ: &str = "[.id, .owner.login, .name, .private] | @tsv";
const PULL_JQ: &str = "[.head.repo.id, .head.repo.owner.login, .head.repo.name, .head.ref, .maintainer_can_modify, .base.repo.id, .head.sha, .head.repo.private] | @tsv";
const WEB: &str = "https://github.com/acme/web.git";
const OLD: &str = "https://github.com/acme/old.git";
const HEAD_SHA: &str = "ec5c10de663bd1860925ad42e9580bab4eb1d377";
const OWN_HEAD_SHA: &str = "9c2e1d4b7a0f3e8c5d6a1b2c3d4e5f6a7b8c9d0e";
const PARENT_SHA: &str = "7a6b5c4d3e2f1a0b9c8d7e6f5a4b3c2d1e0f9a8b";
const IMAGE_SHA: &str = "5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b1c2d3e4f";
const NOW: i64 = 1_700_000_000_000;

/// One gh line as `gh_here` sends it: no prompt, then the quoted words.
/// Re-raises the panic of a spawned task that already ended, so a red run names the unit's own `todo!()` rather than
/// a later assert of the test.
macro_rules! still_running {
    ($t:expr) => {
        if $t.is_finished() {
            match (&mut $t).await {
                Err(e) if e.is_panic() => std::panic::resume_unwind(e.into_panic()),
                _ => panic!("the task ended before the test acted"),
            }
        }
    };
}

fn gh(args: &[&str]) -> String {
    let mut words = vec!["gh"];
    words.extend_from_slice(args);
    format!("GH_PROMPT_DISABLED=1 {}", shell_line(&words))
}

fn repo_cmd(repo: &str) -> String {
    gh(&["api", &format!("repos/{repo}"), "--jq", REPO_JQ])
}

fn pull_cmd(number: u64) -> String {
    gh(&[
        "api",
        &format!("repos/acme/web/pulls/{number}"),
        "--jq",
        PULL_JQ,
    ])
}

fn tip_cmd(repo: &str, base: &str) -> String {
    gh(&[
        "api",
        &format!("repos/{repo}/commits/{base}"),
        "--jq",
        ".sha",
    ])
}

fn private_cmd(repo: &str) -> String {
    gh(&["api", &format!("repos/{repo}"), "--jq", ".private"])
}

fn id_cmd(id: u64) -> String {
    gh(&["api", &format!("repositories/{id}"), "--jq", ".private"])
}

fn heads_cmd(remote: &str) -> String {
    format!(
        "GIT_TERMINAL_PROMPT=0 {}",
        shell_line(&["git", "ls-remote", "--heads", remote])
    )
}

/// A branch of record as the record stores it.
fn of_record(branch: &str) -> Value {
    json!({ "repoId": 4242, "owner": "acme", "name": "web", "branch": branch })
}

fn with(mut v: Value, key: &str, x: Value) -> Value {
    v[key] = x;
    v
}

fn public_record() -> Value {
    with(
        with(of_record("wsp/pricing-page"), "startSha", json!(PARENT_SHA)),
        "public",
        json!(true),
    )
}

fn unread(repo: &str, why: &str) -> HostError {
    branch_of_record_unread_refusal(repo, why)
}

/// An answer that must come at once: a hang fails the test instead of holding it.
async fn within<F: Future>(f: F) -> F::Output {
    tokio::time::timeout(Duration::from_secs(5), f)
        .await
        .expect("the call answered at once")
}

fn live_of(records: Vec<WorkspaceRecord>) -> Vec<Rc<RefCell<WorkspaceRecord>>> {
    records
        .into_iter()
        .map(|r| Rc::new(RefCell::new(r)))
        .collect()
}

fn sub_of(branch: &str) -> Value {
    json!({ "on": "boat", "repo": "acme/web", "base": { "branch": "main", "sha": PARENT_SHA }, "branch": branch, "folder": "/root/web" })
}

// B1: branch-of-record.test.ts:141
#[test]
fn a_brokered_start_needs_a_provider_and_a_ready_broker() {
    assert!(brokered_start(true, true));
    assert!(!brokered_start(true, false));
    assert!(!brokered_start(false, true));
    assert!(!brokered_start(false, false));
}

// B2: spec only (branch-of-record.ts:24-29)
#[test]
fn gh_runs_without_a_prompt_on_this_computer_with_the_inline_timeout() {
    run(async {
        let rig = Rig::new();
        let cmd = "GH_PROMPT_DISABLED=1 gh api repos/acme/web --jq '[.id, .owner.login, .name, .private] | @tsv'";
        rig.ctx.script(cmd, ok("4242\tacme\tweb\ttrue\n"));
        let got = rig
            .area
            .gh_here("acme/web", &["api", "repos/acme/web", "--jq", REPO_JQ])
            .await;
        assert_eq!(
            got,
            Ok(vec![
                "4242".to_string(),
                "acme".to_string(),
                "web".to_string(),
                "true".to_string()
            ])
        );
        assert_eq!(*rig.ctx.exec_log.borrow(), vec![(cmd.to_string(), 20_000)]);

        // The answer is stdout less one trailing newline, `\n` or `\r\n`, split on tabs.
        for (stdout, want) in [
            ("a\tb\r\n", vec!["a", "b"]),
            ("a\n\n", vec!["a\n"]),
            ("", vec![""]),
            ("a\r", vec!["a\r"]),
            ("a\r\n\r\n", vec!["a\r\n"]),
            ("a\n", vec!["a"]),
        ] {
            let rig = Rig::new();
            rig.ctx.script("GH_PROMPT_DISABLED=1 gh api x", ok(stdout));
            assert_eq!(
                rig.area.gh_here("acme/web", &["api", "x"]).await,
                Ok(want.iter().map(|w| w.to_string()).collect()),
                "stdout {stdout:?}"
            );
        }
    });
}

// B2: branch-of-record.test.ts:132
#[test]
fn a_gh_failure_is_refused_naming_the_repo_with_its_last_line() {
    run(async {
        let signed_out = "gh: To get started with GitHub CLI, please run: gh auth login";
        let rig = Rig::new();
        rig.ctx.script(
            "GH_PROMPT_DISABLED=1 gh api x",
            fails(4, &format!("{signed_out}\n")),
        );
        let err = rig
            .area
            .gh_here("acme/web", &["api", "x"])
            .await
            .unwrap_err();
        assert_eq!(err, unread("acme/web", signed_out));
        assert_eq!(
            err.message,
            "gh on this computer could not read acme/web (gh: To get started with GitHub CLI, please run: gh auth login), and a machine whose credentials this host keeps pushes only to a repository GitHub names. Run gh auth status on this computer, then start it again."
        );

        // The last non-blank line, its white space collapsed; no line at all reads as the exit code.
        for (code, stderr, why) in [
            (1, "warning: x\n  the  real\tline  \n\n", "the real line"),
            (7, "", "gh exited 7"),
            (2, " \n\t\n", "gh exited 2"),
        ] {
            let rig = Rig::new();
            rig.ctx
                .script("GH_PROMPT_DISABLED=1 gh api x", fails(code, stderr));
            assert_eq!(
                rig.area.gh_here("acme/web", &["api", "x"]).await,
                Err(unread("acme/web", why)),
                "stderr {stderr:?}"
            );
        }
    });
}

// B2: spec only (branch-of-record.ts:26)
#[test]
fn a_rejected_gh_exec_reads_as_exit_minus_one() {
    run(async {
        let rig = Rig::new();
        rig.ctx
            .script("GH_PROMPT_DISABLED=1 gh api x", rejects("spawn gh ENOENT"));
        assert_eq!(
            rig.area.gh_here("acme/web", &["api", "x"]).await,
            Err(unread("acme/web", "spawn gh ENOENT"))
        );
    });
}

// B3: spec only (branch-of-record.ts:31-34)
#[test]
fn a_commit_is_forty_lowercase_hex() {
    let good = "0123456789abcdef0123456789abcdef01234567";
    assert_eq!(sha_of("acme/web", Some(good)), Ok(good.to_string()));
    let said = |word: &str| {
        unread(
            "acme/web",
            &format!("no commit in {}", serde_json::to_string(word).unwrap()),
        )
    };
    for bad in [
        good.to_uppercase(),
        good[..39].to_string(),
        format!("{good}8"),
        String::new(),
        "no commit here".to_string(),
        "ab\"c".to_string(),
    ] {
        assert_eq!(
            sha_of("acme/web", Some(&bad)),
            Err(said(&bad)),
            "word {bad:?}"
        );
    }
    // JavaScript's `$` takes no trailing newline.
    assert_eq!(
        sha_of("acme/web", Some(&format!("{good}\n"))),
        Err(unread("acme/web", &format!("no commit in \"{good}\\n\"")))
    );
    assert_eq!(
        sha_of("acme/web", None),
        Err(unread("acme/web", "no commit in \"\""))
    );
}

// B3: spec only (branch-of-record.ts:36-40)
#[test]
fn a_repository_id_reads_as_javascript_number_does() {
    for (word, id) in [
        (" 12", 12),
        ("12\r", 12),
        ("12.0", 12),
        ("0x10", 16),
        ("1e3", 1000),
        ("0b11", 3),
        ("0o7", 7),
        ("+5", 5),
        ("\t5\n", 5),
        ("00012", 12),
        ("9007199254740991", 9_007_199_254_740_991),
    ] {
        assert_eq!(id_of("acme/web", Some(word)), Ok(id), "word {word:?}");
    }
    for word in [
        "",
        "  ",
        "-1",
        "0",
        "-0",
        "1.5",
        "9007199254740992",
        "9007199254740993",
        "Infinity",
        "1_0",
        "12abc",
        "1e21",
        "0x",
    ] {
        let said = unread(
            "acme/web",
            &format!(
                "no repository id in {}",
                serde_json::to_string(word).unwrap()
            ),
        );
        assert_eq!(id_of("acme/web", Some(word)), Err(said), "word {word:?}");
    }
    assert_eq!(
        id_of("acme/web", None),
        Err(unread("acme/web", "no repository id in \"\""))
    );
}

// B4: spec only (branch-of-record.ts:43-47)
#[test]
fn a_remote_off_github_is_refused_naming_its_host() {
    run(async {
        for (remote, host) in [
            ("https://gitlab.com/acme/web.git", "gitlab.com"),
            ("git@gitlab.com:acme/web.git", "gitlab.com"),
            ("https://GitLab.com/acme/web.git", "gitlab.com"),
            ("https://git.example.com/acme/web.git", "git.example.com"),
            ("/srv/git/web.git", "/srv/git/web.git"),
        ] {
            assert_eq!(
                github_repo_of(remote, "web", "Boat"),
                Err(brokered_git_hub_only_refusal("web", host, "Boat")),
                "remote {remote}"
            );
        }
        assert_eq!(
            github_repo_of("https://gitlab.com/acme/web.git", "web", "Boat").unwrap_err().message,
            "web's remote is on gitlab.com, and a machine on Boat reaches GitHub alone, through this host. Run this work on this computer."
        );
        for remote in [
            WEB,
            "git@github.com:acme/web.git",
            "https://github.com/acme/web",
            "ssh://git@github.com/acme/web.git",
        ] {
            assert_eq!(
                github_repo_of(remote, "web", "Boat"),
                Ok("acme/web".to_string()),
                "remote {remote}"
            );
        }
        assert_eq!(
            github_repo_of("https://GitHub.com/Acme/Web.git", "web", "Boat"),
            Ok("Acme/Web".to_string())
        );

        // Nothing is asked of gh for a remote that is refused.
        let rig = Rig::new();
        let err = rig
            .area
            .repo_of_record("https://gitlab.com/acme/web.git", "web", "Boat")
            .await
            .unwrap_err();
        assert_eq!(
            err,
            brokered_git_hub_only_refusal("web", "gitlab.com", "Boat")
        );
        assert_eq!(rig.ctx.commands(), Vec::<String>::new());
    });
}

// B5: branch-of-record.test.ts:111
#[test]
fn the_repo_of_record_is_read_by_id_and_public_only_on_false() {
    run(async {
        let rig = Rig::new();
        let cmd = "GH_PROMPT_DISABLED=1 gh api repos/acme/web --jq '[.id, .owner.login, .name, .private] | @tsv'";
        rig.ctx.script(cmd, ok("4242\tacme\tweb\ttrue\n"));
        let hidden = rig.area.repo_of_record(WEB, "web", "Boat").await;
        assert_eq!(
            hidden,
            Ok(RepoOfRecord {
                repo_id: 4242,
                owner: "acme".into(),
                name: "web".into(),
                public: false
            })
        );
        assert_eq!(*rig.ctx.exec_log.borrow(), vec![(cmd.to_string(), 20_000)]);

        let repo = |id, owner: &str, name: &str, public| {
            Ok(RepoOfRecord {
                repo_id: id,
                owner: owner.into(),
                name: name.into(),
                public,
            })
        };
        for (stdout, want) in [
            ("4242\tacme\tweb\tfalse\n", repo(4242, "acme", "web", true)),
            ("4242\tacme\tweb\t\n", repo(4242, "acme", "web", false)),
            ("4242\tacme\tweb\n", repo(4242, "acme", "web", false)),
            ("4242\n", repo(4242, "", "", false)),
        ] {
            let rig = Rig::new();
            rig.ctx.script(cmd, ok(stdout));
            assert_eq!(
                rig.area.repo_of_record(WEB, "web", "Boat").await,
                want,
                "stdout {stdout:?}"
            );
        }

        // A renamed repo answers with its name now, and the id GitHub holds it by.
        let rig = Rig::new();
        rig.ctx
            .script(&repo_cmd("acme/old"), ok("4242\tacme\tweb\ttrue\n"));
        assert_eq!(
            rig.area.repo_of_record(OLD, "web", "Boat").await,
            repo(4242, "acme", "web", false)
        );

        let rig = Rig::new();
        rig.ctx
            .script(&repo_cmd("acme/web"), ok("abc\tacme\tweb\tfalse\n"));
        assert_eq!(
            rig.area.repo_of_record(WEB, "web", "Boat").await,
            Err(unread("acme/web", "no repository id in \"abc\""))
        );
    });
}

// B6: spec only (branch-of-record.ts:62-72)
#[test]
fn a_public_repo_turned_private_drops_public_and_persists() {
    run(async {
        let rig = Rig::new();
        let (shared, entry) = entry_of(
            &rig.ctx,
            json!({ "branchOfRecord": public_record() }),
            None,
            false,
        );
        rig.ctx.script(
            "GH_PROMPT_DISABLED=1 gh api repositories/4242 --jq .private",
            ok("true\n"),
        );
        rig.area.reread_public("ws_1", &entry).await.unwrap();
        assert_eq!(
            *rig.ctx.exec_log.borrow(),
            vec![(
                "GH_PROMPT_DISABLED=1 gh api repositories/4242 --jq .private".to_string(),
                20_000
            )]
        );
        let kept = branch_of(with(
            of_record("wsp/pricing-page"),
            "startSha",
            json!(PARENT_SHA),
        ));
        assert_eq!(shared.borrow().branch_of_record, Some(kept));
        assert_eq!(rig.ctx.persisted(), vec![shared.borrow().clone()]);

        // A record that is not public is not read at all.
        let rig = Rig::new();
        let (shared, entry) = entry_of(
            &rig.ctx,
            json!({ "branchOfRecord": of_record("wsp/pricing-page") }),
            None,
            false,
        );
        let before = shared.borrow().clone();
        rig.area.reread_public("ws_1", &entry).await.unwrap();
        assert_eq!(rig.ctx.commands(), Vec::<String>::new());
        assert_eq!(*shared.borrow(), before);
        assert_eq!(rig.ctx.persisted(), Vec::<WorkspaceRecord>::new());
    });
}

// B6: spec only (branch-of-record.ts:60-66)
#[test]
fn a_reread_waits_five_minutes() {
    run(async {
        let rig = Rig::at(NOW);
        let (_, entry) = entry_of(
            &rig.ctx,
            json!({ "branchOfRecord": public_record() }),
            None,
            false,
        );
        rig.ctx.script(&id_cmd(4242), ok("false\n"));
        let asked = || rig.ctx.commands().len();
        rig.area.reread_public("ws_1", &entry).await.unwrap();
        assert_eq!(asked(), 1);
        rig.area.reread_public("ws_1", &entry).await.unwrap();
        assert_eq!(asked(), 1);
        rig.clock.advance(299_999);
        rig.area.reread_public("ws_1", &entry).await.unwrap();
        assert_eq!(asked(), 1);
        rig.clock.advance(1);
        rig.area.reread_public("ws_1", &entry).await.unwrap();
        assert_eq!(asked(), 2);

        // The stamp is per workspace.
        let (_, other) = entry_of(
            &rig.ctx,
            json!({ "id": "ws_2", "branchOfRecord": public_record() }),
            None,
            false,
        );
        rig.area.reread_public("ws_2", &other).await.unwrap();
        assert_eq!(asked(), 3);

        // The stamp is taken before the read: a call while one is out reads nothing more.
        let (_, slow) = entry_of(
            &rig.ctx,
            json!({ "id": "ws_3", "branchOfRecord": public_record() }),
            None,
            false,
        );
        let gate = rig.ctx.script_held(&id_cmd(4242), ok("false\n"));
        let first = tokio::task::spawn_local({
            let (area, slow) = (rig.area.clone(), slow.clone());
            async move { area.reread_public("ws_3", &slow).await }
        });
        settle().await;
        assert_eq!(asked(), 4);
        within(rig.area.reread_public("ws_3", &slow)).await.unwrap();
        assert_eq!(asked(), 4);
        rig.ctx.open(&gate);
        first.await.unwrap().unwrap();
        assert_eq!(asked(), 4);
    });
}

// B6: spec only (branch-of-record.ts:67)
#[test]
fn a_reread_that_fails_keeps_it_public() {
    run(async {
        for (id, reply) in [
            ("ws_1", fails(1, "gh: Not Found (HTTP 404)")),
            ("ws_2", rejects("spawn gh ENOENT")),
            ("ws_3", ok("maybe\n")),
        ] {
            let rig = Rig::new();
            let (shared, entry) = entry_of(
                &rig.ctx,
                json!({ "id": id, "branchOfRecord": public_record() }),
                None,
                false,
            );
            rig.ctx.script(&id_cmd(4242), reply);
            let before = shared.borrow().clone();
            rig.area.reread_public(id, &entry).await.unwrap();
            assert_eq!(rig.ctx.commands(), vec![id_cmd(4242)], "{id}");
            assert_eq!(*shared.borrow(), before, "{id}");
            assert_eq!(rig.ctx.persisted(), Vec::<WorkspaceRecord>::new(), "{id}");
        }
    });
}

// B6: spec only (branch-of-record.ts:68)
#[test]
fn a_branch_changed_meanwhile_is_not_written() {
    run(async {
        let rig = Rig::new();
        let (shared, entry) = entry_of(
            &rig.ctx,
            json!({ "branchOfRecord": public_record() }),
            None,
            false,
        );
        let gate = rig.ctx.script_held(&id_cmd(4242), ok("true\n"));
        let mut reread = tokio::task::spawn_local({
            let (area, entry) = (rig.area.clone(), entry.clone());
            async move { area.reread_public("ws_1", &entry).await }
        });
        settle().await;
        still_running!(reread);
        assert_eq!(rig.ctx.commands(), vec![id_cmd(4242)]);
        let moved = branch_of(with(of_record("wsp/other"), "public", json!(true)));
        shared.borrow_mut().branch_of_record = Some(moved.clone());
        rig.ctx.open(&gate);
        reread.await.unwrap().unwrap();
        assert_eq!(shared.borrow().branch_of_record, Some(moved));
        assert_eq!(rig.ctx.persisted(), Vec::<WorkspaceRecord>::new());
    });
}

fn market_rig() -> Rig {
    let rig = Rig::at(NOW);
    rig.ctx
        .marketplace("~/.claude/plugins/known_marketplaces.json");
    rig.ctx.put_file(
        "/home/zing/.claude/plugins/known_marketplaces.json",
        "acme/web",
    );
    rig
}

// B7: spec only (branch-of-record.ts:74-107)
#[test]
fn the_first_launch_waits_at_most_three_seconds_for_the_marketplaces() {
    run(async {
        let rig = market_rig();
        assert_eq!(rig.area.public_marketplaces(), Vec::<String>::new());
        let gate = rig.ctx.script_held(&private_cmd("acme/web"), ok("false\n"));
        let launch = tokio::task::spawn_local({
            let area = rig.area.clone();
            async move { area.read_public_marketplaces().await }
        });
        settle().await;
        assert_eq!(rig.ctx.commands(), vec![private_cmd("acme/web")]);
        assert!(!launch.is_finished());
        rig.clock.advance(2_999);
        settle().await;
        assert!(!launch.is_finished());
        assert_eq!(*rig.clock.asked.borrow(), vec![3_000]);
        rig.clock.advance(1);
        settle().await;
        assert!(launch.is_finished());
        launch.await.unwrap();
        assert_eq!(rig.area.public_marketplaces(), Vec::<String>::new());

        // The read is still the one held, so a second call waits for it too, at most as long.
        let second = tokio::task::spawn_local({
            let area = rig.area.clone();
            async move { area.read_public_marketplaces().await }
        });
        settle().await;
        assert!(!second.is_finished());
        rig.clock.advance(3_000);
        settle().await;
        second.await.unwrap();
        assert_eq!(rig.ctx.commands(), vec![private_cmd("acme/web")]);

        // The read lands for the launches after it, and a call that finds it landed leaves no timer armed.
        rig.ctx.open(&gate);
        settle().await;
        assert_eq!(rig.area.public_marketplaces(), vec!["acme/web".to_string()]);
        within(rig.area.read_public_marketplaces()).await;
        assert_eq!(rig.clock.armed(), 0);
    });
}

// B7: spec only (branch-of-record.ts:80-91)
#[test]
fn later_launches_answer_at_once_and_reread_after_ten_minutes() {
    run(async {
        let rig = market_rig();
        rig.ctx.script(&private_cmd("acme/web"), ok("false\n"));
        within(rig.area.read_public_marketplaces()).await;
        assert_eq!(rig.ctx.commands(), vec![private_cmd("acme/web")]);
        assert_eq!(rig.area.public_marketplaces(), vec!["acme/web".to_string()]);
        within(rig.area.read_public_marketplaces()).await;
        rig.clock.advance(599_999);
        within(rig.area.read_public_marketplaces()).await;
        settle().await;
        assert_eq!(rig.ctx.commands().len(), 1);

        // Ten minutes after the read began, the next call starts one beside it and answers at once.
        rig.clock.advance(1);
        let gate = rig.ctx.script_held(&private_cmd("acme/web"), ok("true\n"));
        within(rig.area.read_public_marketplaces()).await;
        settle().await;
        assert_eq!(
            rig.ctx.commands(),
            vec![private_cmd("acme/web"), private_cmd("acme/web")]
        );
        // It keeps the repos last read until the new read lands.
        assert_eq!(rig.area.public_marketplaces(), vec!["acme/web".to_string()]);
        rig.ctx.open(&gate);
        settle().await;
        assert_eq!(rig.area.public_marketplaces(), Vec::<String>::new());
    });
}

// B7: spec only (branch-of-record.ts:86-107)
#[test]
fn only_public_marketplace_repos_are_kept() {
    run(async {
        let rig = Rig::at(NOW);
        for path in [
            "~/.claude/plugins/known_marketplaces.json",
            "~/.codex/marketplaces.json",
            "~/.cursor/missing.json",
        ] {
            rig.ctx.marketplace(path);
        }
        rig.ctx.put_file(
            "/home/zing/.claude/plugins/known_marketplaces.json",
            "acme/web acme/private-tools acme/web",
        );
        rig.ctx.put_file(
            "/home/zing/.codex/marketplaces.json",
            "acme/web other/market gone/repo",
        );
        let read = vec![
            "/home/zing/.claude/plugins/known_marketplaces.json",
            "/home/zing/.codex/marketplaces.json",
            "/home/zing/.cursor/missing.json",
        ];
        let unique = [
            "acme/web",
            "acme/private-tools",
            "other/market",
            "gone/repo",
        ]
        .map(String::from)
        .to_vec();

        // Each file once under the home with its `~/` cut, an unreadable one reading none, the repos unique.
        assert_eq!(rig.area.marketplace_repos().await, unique);
        assert_eq!(*rig.ctx.read_log.borrow(), read);
        rig.ctx.read_log.borrow_mut().clear();

        rig.ctx.script(&private_cmd("acme/web"), ok("false\n"));
        rig.ctx
            .script(&private_cmd("acme/private-tools"), ok("true\n"));
        rig.ctx.script(&private_cmd("other/market"), ok("false\n"));
        within(rig.area.read_public_marketplaces()).await;
        assert_eq!(*rig.ctx.read_log.borrow(), read);
        assert_eq!(
            rig.ctx.commands(),
            unique.iter().map(|r| private_cmd(r)).collect::<Vec<_>>()
        );
        // gone/repo answers a failed read, which drops it.
        assert_eq!(
            rig.area.public_marketplaces(),
            vec!["acme/web".to_string(), "other/market".to_string()]
        );
    });
}

// B8: branch-of-record.test.ts:549
#[test]
fn a_pull_request_thread_records_the_head_of_its_fork() {
    run(async {
        let rig = Rig::new();
        let cmd = "GH_PROMPT_DISABLED=1 gh api repos/acme/web/pulls/7 --jq '[.head.repo.id, .head.repo.owner.login, .head.repo.name, .head.ref, .maintainer_can_modify, .base.repo.id, .head.sha, .head.repo.private] | @tsv'";
        rig.ctx.script(
            cmd,
            ok(&format!(
                "9001\tana\tweb-fork\tfix-typo\ttrue\t4242\t{HEAD_SHA}\ttrue\n"
            )),
        );
        let head = rig
            .area
            .pull_request_of_record(WEB, 7, "web", "Boat", true)
            .await;
        assert_eq!(
            head,
            Ok(branch_of(
                json!({ "repoId": 9001, "owner": "ana", "name": "web-fork", "branch": "fix-typo", "startSha": HEAD_SHA })
            ))
        );
        assert_eq!(*rig.ctx.exec_log.borrow(), vec![(cmd.to_string(), 20_000)]);
    });
}

// B8: branch-of-record.test.ts:591
#[test]
fn a_pull_request_on_the_projects_own_repo_is_public_as_github_says() {
    run(async {
        let rig = Rig::new();
        rig.ctx.script(
            &pull_cmd(9),
            ok(&format!(
                "4242\tacme\tweb\tlab/review-me\tfalse\t4242\t{OWN_HEAD_SHA}\tfalse\n"
            )),
        );
        let head = rig
            .area
            .pull_request_of_record(WEB, 9, "web", "Boat", true)
            .await;
        let want = json!({ "repoId": 4242, "owner": "acme", "name": "web", "branch": "lab/review-me", "startSha": OWN_HEAD_SHA, "public": true });
        assert_eq!(head, Ok(branch_of(want)));
    });
}

// B8: branch-of-record.test.ts:574
#[test]
fn a_push_to_a_fork_without_maintainer_edits_is_refused() {
    run(async {
        let rig = Rig::new();
        rig.ctx.script(
            &pull_cmd(8),
            ok(&format!(
                "9001\tana\tweb-fork\tfix-typo\tfalse\t4242\t{HEAD_SHA}\n"
            )),
        );
        let err = rig
            .area
            .pull_request_of_record(WEB, 8, "web", "Boat", true)
            .await
            .unwrap_err();
        assert_eq!(err, fork_not_pushable_refusal("ana", 8.0));
        assert_eq!(err.kind, Some(Kind::Usage));
        // Anything but `true` is no permission.
        let rig = Rig::new();
        rig.ctx.script(
            &pull_cmd(8),
            ok(&format!(
                "9001\tana\tweb-fork\tfix-typo\t\t4242\t{HEAD_SHA}\n"
            )),
        );
        assert_eq!(
            rig.area
                .pull_request_of_record(WEB, 8, "web", "Boat", true)
                .await,
            Err(fork_not_pushable_refusal("ana", 8.0))
        );
    });
}

// B8: branch-of-record.test.ts:584
#[test]
fn a_review_of_such_a_fork_passes() {
    run(async {
        let rig = Rig::new();
        rig.ctx.script(
            &pull_cmd(8),
            ok(&format!(
                "9001\tana\tweb-fork\tfix-typo\tfalse\t4242\t{HEAD_SHA}\n"
            )),
        );
        let head = rig
            .area
            .pull_request_of_record(WEB, 8, "web", "Boat", false)
            .await;
        assert_eq!(
            head,
            Ok(branch_of(
                json!({ "repoId": 9001, "owner": "ana", "name": "web-fork", "branch": "fix-typo", "startSha": HEAD_SHA })
            ))
        );
    });
}

// B8: branch-of-record.test.ts:574
#[test]
fn a_gone_fork_is_refused() {
    run(async {
        for pushes in [true, false] {
            let rig = Rig::new();
            rig.ctx
                .script(&pull_cmd(10), ok("\t\t\tfix-typo\tfalse\t4242\n"));
            let err = rig
                .area
                .pull_request_of_record(WEB, 10, "web", "Boat", pushes)
                .await
                .unwrap_err();
            assert_eq!(err, fork_head_gone_refusal(10.0));
            assert!(
                err.message.contains(
                    "pull request #10's fork was deleted, so there is no branch to push to"
                ),
                "{}",
                err.message
            );
        }
    });
}

// B8: spec only (branch-of-record.ts:118)
#[test]
fn a_pull_request_head_without_a_commit_is_refused() {
    run(async {
        let rig = Rig::new();
        rig.ctx.script(
            &pull_cmd(7),
            ok("9001\tana\tweb-fork\tfix-typo\ttrue\t4242\tshort\ttrue\n"),
        );
        let err = rig
            .area
            .pull_request_of_record(WEB, 7, "web", "Boat", true)
            .await;
        assert_eq!(err, Err(unread("acme/web", "no commit in \"short\"")));
    });
}

// B9: branch-of-record.test.ts:529
#[test]
fn names_wsp_slug_numbered_past_every_branch_taken() {
    let live = live_of(vec![
        record(json!({ "id": "a", "branchOfRecord": of_record("wsp/x") })),
        record(json!({ "id": "b", "sub": sub_of("wsp/x-2") })),
    ]);
    assert_eq!(
        free_branch_name(&live, "wsp/x", Some("abc\trefs/heads/wsp/x-3\n")),
        "wsp/x-4"
    );
    assert_eq!(free_branch_name(&live, "wsp/y", None), "wsp/y");
    assert_eq!(free_branch_name(&live, "wsp/x", Some("")), "wsp/x-3");
    assert_eq!(free_branch_name(&[], "wsp/x", None), "wsp/x");

    // Only `refs/heads/` in the second field is a branch, and exactly.
    let heads = "a\trefs/tags/wsp/y\nb\trefs/heads/wsp/y-extra\nrefs/heads/wsp/y\n";
    assert_eq!(free_branch_name(&[], "wsp/y", Some(heads)), "wsp/y");
    assert_eq!(
        free_branch_name(&[], "wsp/y", Some(&format!("{heads}c\trefs/heads/wsp/y\n"))),
        "wsp/y-2"
    );

    // The first number not taken, not the next past the last.
    let gaps = live_of(vec![
        record(json!({ "id": "a", "branchOfRecord": of_record("wsp/z") })),
        record(json!({ "id": "b", "branchOfRecord": of_record("wsp/z-3") })),
    ]);
    assert_eq!(free_branch_name(&gaps, "wsp/z", None), "wsp/z-2");
}

fn named(name: &str, remote: &str, base: Option<&str>, carried: bool) -> NamedAsk {
    NamedAsk {
        remote: remote.into(),
        name: name.into(),
        project: "web".into(),
        on: "Boat".into(),
        base: base.map(String::from),
        carried,
    }
}

// B10: branch-of-record.test.ts:468
#[test]
fn a_named_branch_starts_from_the_bases_tip() {
    run(async {
        // GitHub names the repo acme/web now, which the tip is read under; the heads are the remote as given.
        let rig = Rig::new();
        rig.ctx
            .script(&repo_cmd("acme/old"), ok("4242\tacme\tweb\ttrue\n"));
        rig.ctx.script(
            &tip_cmd("acme/web", "feat/parent"),
            ok(&format!("{PARENT_SHA}\n")),
        );
        rig.ctx
            .script(&heads_cmd(OLD), ok("abc\trefs/heads/main\n"));
        let got = rig
            .area
            .named_of_record(&named("pricing page", OLD, Some("feat/parent"), false))
            .await;
        assert_eq!(
            got,
            Ok(branch_of(
                json!({ "repoId": 4242, "owner": "acme", "name": "web", "branch": "wsp/pricing-page", "startSha": PARENT_SHA })
            ))
        );
        let log = rig.ctx.exec_log.borrow().clone();
        assert_eq!(
            log,
            vec![
                ("GH_PROMPT_DISABLED=1 gh api repos/acme/old --jq '[.id, .owner.login, .name, .private] | @tsv'".to_string(), 20_000),
                ("GH_PROMPT_DISABLED=1 gh api repos/acme/web/commits/feat/parent --jq .sha".to_string(), 20_000),
                ("GIT_TERMINAL_PROMPT=0 git ls-remote --heads https://github.com/acme/old.git".to_string(), 20_000),
            ]
        );

        // No base names the default branch's tip, and a public repo is recorded public.
        let rig = Rig::new();
        rig.ctx
            .script(&repo_cmd("acme/web"), ok("4242\tacme\tweb\tfalse\n"));
        rig.ctx
            .script(&tip_cmd("acme/web", "HEAD"), ok(&format!("{IMAGE_SHA}\n")));
        rig.ctx.script(&heads_cmd(WEB), ok(""));
        let got = rig
            .area
            .named_of_record(&named("Pricing  page!", WEB, None, false))
            .await;
        let want = json!({ "repoId": 4242, "owner": "acme", "name": "web", "branch": "wsp/pricing-page", "startSha": IMAGE_SHA, "public": true });
        assert_eq!(got, Ok(branch_of(want)));
        assert_eq!(
            rig.ctx.commands(),
            vec![
                repo_cmd("acme/web"),
                tip_cmd("acme/web", "HEAD"),
                heads_cmd(WEB)
            ]
        );
    });
}

// B10: branch-of-record.test.ts:468
#[test]
fn a_carried_checkout_leaves_the_start_to_the_machine() {
    run(async {
        let rig = Rig::new();
        rig.ctx
            .script(&repo_cmd("acme/web"), ok("4242\tacme\tweb\ttrue\n"));
        rig.ctx
            .script(&heads_cmd(WEB), ok("abc\trefs/heads/main\n"));
        let got = rig
            .area
            .named_of_record(&named("pricing page", WEB, Some("main"), true))
            .await;
        assert_eq!(got, Ok(branch_of(of_record("wsp/pricing-page"))));
        assert_eq!(
            rig.ctx.commands(),
            vec![repo_cmd("acme/web"), heads_cmd(WEB)]
        );
    });
}

// B10: branch-of-record.test.ts:529
#[test]
fn a_named_branch_is_numbered_past_live_records_and_the_remotes_heads() {
    run(async {
        let rig = Rig::new();
        entry_of(
            &rig.ctx,
            json!({ "id": "ws_a", "branchOfRecord": of_record("wsp/pricing-page") }),
            None,
            false,
        );
        rig.ctx
            .script(&repo_cmd("acme/web"), ok("4242\tacme\tweb\ttrue\n"));
        rig.ctx
            .script(&tip_cmd("acme/web", "main"), ok(&format!("{PARENT_SHA}\n")));
        rig.ctx
            .script(&heads_cmd(WEB), ok("abc\trefs/heads/wsp/pricing-page-2\n"));
        let got = rig
            .area
            .named_of_record(&named("Pricing  page!", WEB, Some("main"), false))
            .await
            .unwrap();
        assert_eq!(got.branch, "wsp/pricing-page-3");
    });
}

// B10: spec only (branch-of-record.ts:143-144)
#[test]
fn a_failed_ls_remote_reads_no_heads() {
    run(async {
        let said = Reply::Done(
            128,
            "abc\trefs/heads/wsp/pricing-page\n".into(),
            "fatal: could not read from remote repository".into(),
        );
        for heads in [said, rejects("spawn git ENOENT")] {
            let rig = Rig::new();
            rig.ctx
                .script(&repo_cmd("acme/web"), ok("4242\tacme\tweb\ttrue\n"));
            rig.ctx
                .script(&tip_cmd("acme/web", "main"), ok(&format!("{PARENT_SHA}\n")));
            rig.ctx.script(&heads_cmd(WEB), heads);
            let got = rig
                .area
                .named_of_record(&named("pricing page", WEB, Some("main"), false))
                .await
                .unwrap();
            assert_eq!(got.branch, "wsp/pricing-page");
        }
    });
}

// B10: spec only (branch-of-record.ts:142)
#[test]
fn a_base_tip_that_is_no_commit_is_refused_before_the_heads_are_read() {
    run(async {
        let rig = Rig::new();
        rig.ctx
            .script(&repo_cmd("acme/old"), ok("4242\tacme\tweb\ttrue\n"));
        rig.ctx
            .script(&tip_cmd("acme/web", "main"), ok("garbage\n"));
        let got = rig
            .area
            .named_of_record(&named("pricing page", OLD, Some("main"), false))
            .await;
        assert_eq!(got, Err(unread("acme/web", "no commit in \"garbage\"")));
        assert_eq!(
            rig.ctx.commands(),
            vec![
                repo_cmd("acme/old"),
                tip_cmd("acme/web", "main"),
                heads_cmd(OLD)
            ]
        );
    });
}

fn thread_scope(thread: &str, workspace: &str, root: &str) -> Value {
    json!({ "kind": "thread", "threadId": thread, "workspaceId": workspace, "rootThreadId": root })
}

// B11: branch-of-record.test.ts:495
#[test]
fn invariant_3_an_op_token_is_scoped_to_the_workspaces_thread_not_the_asker() {
    run(async {
        let rig = Rig::at(NOW);
        let ctx = rig.ctx.clone();
        // The record's own thread and root win over the thread its start opens.
        let (_, entry) = entry_of(
            &rig.ctx,
            json!({ "parentThreadId": "t_lead", "rootThreadId": "t_root", "branchOfRecord": of_record("wsp/pricing-page") }),
            Some("t_other"),
            false,
        );
        let got = rig
            .area
            .with_op_token(&entry, "clone", |env| {
                async move {
                    ctx.events.borrow_mut().push("run".to_string());
                    Ok(env)
                }
                .boxed_local()
            })
            .await;
        assert_eq!(got, Ok(env_of("tok_1", Some("wsp/pricing-page"))));
        let road = MintMore {
            road: Some(WorkspaceOrigin::Relayed),
            aside: false,
        };
        let want = Mint {
            name: "op clone".into(),
            scope: thread_scope("t_lead", "ws_1", "t_root"),
            now: NOW,
            more: road,
        };
        assert_eq!(rig.ctx.mints(), vec![want]);
        assert_eq!(
            rig.ctx.events(),
            vec!["mint op clone", "run", "revoke dev_1"]
        );

        // A workspace with no thread of its own takes the one its first start opens, and that is its root.
        rig.clock.advance(5);
        let (_, fresh) = entry_of(
            &rig.ctx,
            json!({ "id": "ws_2", "kind": "local" }),
            Some("t_new"),
            false,
        );
        let got = rig
            .area
            .with_op_token(&fresh, "clone", |env| async move { Ok(env) }.boxed_local())
            .await;
        assert_eq!(got, Ok(env_of("tok_2", None)));
        let road = MintMore {
            road: Some(WorkspaceOrigin::Here),
            aside: false,
        };
        let want = Mint {
            name: "op clone".into(),
            scope: thread_scope("t_new", "ws_2", "t_new"),
            now: NOW + 5,
            more: road,
        };
        assert_eq!(rig.ctx.mints()[1], want);
        assert_eq!(*rig.ctx.revokes.borrow(), vec!["dev_1", "dev_2"]);
    });
}

// B11: spec only (branch-of-record.ts:158-161)
#[test]
fn an_op_token_is_revoked_after_a_failure_too() {
    run(async {
        let rig = Rig::new();
        let ctx = rig.ctx.clone();
        let (_, entry) = entry_of(&rig.ctx, json!({ "parentThreadId": "t_lead" }), None, false);
        let got = rig
            .area
            .with_op_token(&entry, "clone", |_| {
                async move {
                    ctx.events.borrow_mut().push("run".to_string());
                    Err::<(), _>(plain_error("boom"))
                }
                .boxed_local()
            })
            .await;
        assert_eq!(got, Err(plain_error("boom")));
        assert_eq!(
            rig.ctx.events(),
            vec!["mint op clone", "run", "revoke dev_1"]
        );
        assert_eq!(rig.ctx.warned.lines(), Vec::<String>::new());
    });
}

// B11: spec only (branch-of-record.ts:153)
#[test]
fn no_thread_refuses_before_minting() {
    run(async {
        let rig = Rig::new();
        let ctx = rig.ctx.clone();
        let (_, entry) = entry_of(&rig.ctx, json!({}), None, false);
        let got = rig
            .area
            .with_op_token(&entry, "clone", |_| {
                async move {
                    ctx.events.borrow_mut().push("run".to_string());
                    Ok(())
                }
                .boxed_local()
            })
            .await;
        assert_eq!(got, Err(clone_no_thread_refusal("pricing page")));
        assert_eq!(rig.ctx.mints(), Vec::<Mint>::new());
        assert_eq!(rig.ctx.events(), Vec::<String>::new());
    });
}

// B11: spec only (branch-of-record.ts:160)
#[test]
fn a_failed_revoke_warns() {
    run(async {
        let rig = Rig::new();
        *rig.ctx.revoke_fails.borrow_mut() = Some("door closed".to_string());
        let (_, entry) = entry_of(&rig.ctx, json!({ "parentThreadId": "t_lead" }), None, false);
        let got = rig
            .area
            .with_op_token(&entry, "fetch", |_| async move { Ok(7) }.boxed_local())
            .await;
        assert_eq!(got, Ok(7));
        assert_eq!(
            rig.ctx.warned.lines(),
            vec!["the fetch token of pricing page was not taken away: door closed".to_string()]
        );

        // The run's own failure is what the caller reads, the warning beside it.
        let got = rig
            .area
            .with_op_token(&entry, "clone", |_| {
                async move { Err::<u32, _>(plain_error("boom")) }.boxed_local()
            })
            .await;
        assert_eq!(got, Err(plain_error("boom")));
        assert_eq!(
            rig.ctx.warned.lines().last().cloned(),
            Some("the clone token of pricing page was not taken away: door closed".to_string())
        );
        assert_eq!(rig.ctx.warned.lines().len(), 2);
    });
}

const LAND_BASE: [(&str, &str); 2] = [("parentThreadId", "t_lead"), ("rootThreadId", "t_lead")];

fn land_entry(
    rig: &Rig,
    of_record_json: Value,
    place: Option<&str>,
    held: bool,
) -> (Rc<RefCell<WorkspaceRecord>>, OnRecordEntry) {
    let mut extra = json!({ "branchOfRecord": of_record_json });
    for (k, v) in LAND_BASE {
        extra[k] = json!(v);
    }
    if let Some(place) = place {
        extra["place"] = json!(place);
    }
    entry_of(&rig.ctx, extra, None, held)
}

fn land_script(lines: &str) -> String {
    format!("{ENV_FROM_INPUT}\n{lines}")
}

// B12: branch-of-record.test.ts:468
#[test]
fn land_on_record_writes_the_start_commit_the_machine_says() {
    run(async {
        for (held, stdout) in [
            (false, format!("noise\n{IMAGE_SHA}\n")),
            (true, format!("{IMAGE_SHA}\n\n  \n")),
        ] {
            let rig = Rig::new();
            let (shared, entry) =
                land_entry(&rig, of_record("wsp/pricing-page"), Some("pl_boat"), held);
            rig.ctx.env_reply(Ok(exec_result(0, &stdout, "")));
            rig.area
                .land_on_record(&entry, &project(WEB), None)
                .await
                .unwrap();

            let lines = on_record_of("/root/web", "wsp/pricing-page", "acme/web", held);
            assert_eq!(
                *rig.ctx.on_record_calls.borrow(),
                vec![(
                    "/root/web".to_string(),
                    "wsp/pricing-page".to_string(),
                    "acme/web".to_string(),
                    held
                )]
            );
            let want = EnvRun {
                machine: "bx_1".into(),
                script: land_script(&format!("set -e\n{}", lines.join("\n"))),
                env: env_of("tok_1", Some("wsp/pricing-page")),
                timeout_ms: 300_000,
            };
            assert_eq!(rig.ctx.env_runs(), vec![want]);
            assert_eq!(rig.ctx.mints()[0].name, "op clone");
            assert_eq!(
                rig.ctx.events(),
                vec!["mint op clone", "env_exec", "revoke dev_1", "persist"]
            );
            let landed = Some(branch_of(with(
                of_record("wsp/pricing-page"),
                "startSha",
                json!(IMAGE_SHA),
            )));
            assert_eq!(shared.borrow().branch_of_record, landed);
            assert_eq!(rig.ctx.persisted(), vec![shared.borrow().clone()]);
        }

        // A remote that names no owner and repo is no origin.
        let rig = Rig::new();
        let (_, entry) = land_entry(&rig, of_record("wsp/pricing-page"), None, false);
        rig.area
            .land_on_record(&entry, &project("web"), None)
            .await
            .unwrap();
        assert_eq!(rig.ctx.on_record_calls.borrow()[0].2, "");
    });
}

// B12: branch-of-record.test.ts:511
#[test]
fn a_failed_clone_says_why_in_one_sentence() {
    run(async {
        let started = "Cloning into '/root/web'...\n";
        let refused = "fatal: unable to access 'http://github.com/acme/web/': The requested URL returned error: 403";
        let host_said = "this call's token is not live on your computer, since the turn that held it has ended. Send the thread a new message.";
        let cases = [
            (
                128,
                String::new(),
                format!("{started}remote: wsp: {host_said}\n{refused}\n"),
                format!("failed: {host_said}"),
            ),
            (
                124,
                String::new(),
                started.to_string(),
                "did not finish in 5 minutes, and git said nothing past its start".to_string(),
            ),
            (
                128,
                String::new(),
                started.to_string(),
                "ended with exit 128, and git said nothing past its start".to_string(),
            ),
            (
                128,
                String::new(),
                format!("{started}{refused}\n"),
                format!("failed: {refused}"),
            ),
            (
                128,
                "fatal: something".to_string(),
                String::new(),
                "failed: fatal: something".to_string(),
            ),
        ];
        let clone: Vec<String> = [
            "set -e",
            "git clone --depth 1 --branch main -- https://github.com/acme/web.git /root/web",
        ]
        .map(String::from)
        .to_vec();
        for (code, stdout, stderr, said) in cases {
            let rig = Rig::new();
            let (shared, entry) =
                land_entry(&rig, of_record("wsp/pricing-page"), Some("pl_boat"), false);
            let before = shared.borrow().clone();
            rig.ctx.env_reply(Ok(exec_result(code, &stdout, &stderr)));
            let err = rig
                .area
                .land_on_record(&entry, &project(WEB), Some(&clone))
                .await
                .unwrap_err();
            assert_eq!(
                err,
                plain_error(&format!("the clone of acme/web on Boat {said}"))
            );
            let ran = rig.ctx.env_runs();
            assert_eq!(ran[0].script, land_script(&clone.join("\n")));
            assert_eq!(ran[0].timeout_ms, 300_000);
            assert_eq!(rig.ctx.on_record_calls.borrow().len(), 0);
            assert_eq!(
                rig.ctx.events(),
                vec!["mint op clone", "env_exec", "revoke dev_1"]
            );
            assert_eq!(*shared.borrow(), before);
        }

        // Where the record names no place the wired one is named, and a remote that is no owner and repo reads whole.
        let rig = Rig::new();
        let (_, entry) = land_entry(&rig, of_record("wsp/pricing-page"), None, false);
        rig.ctx.env_reply(Ok(exec_result(128, "", "fatal: nope\n")));
        let err = rig
            .area
            .land_on_record(&entry, &project("web"), Some(&clone))
            .await
            .unwrap_err();
        let said = CloneFailed {
            repo: "web".into(),
            on: "This Mac".into(),
            exit_code: 128,
            said: "fatal: nope\n".into(),
            limit_ms: 300_000.0,
        };
        assert_eq!(err, plain_error(&clone_on_machine_failed_line(&said)));
    });
}

// B12: spec only (branch-of-record.ts:171-172)
#[test]
fn a_failed_checkout_says_gits_last_line() {
    run(async {
        for (code, stdout, stderr, said) in [
            (
                1,
                "",
                "error: pathspec\nfatal: cannot checkout wsp/pricing-page\n",
                "fatal: cannot checkout wsp/pricing-page",
            ),
            (1, "one\ntwo\n", "", "two"),
            (1, "out line\n", "  \n", "out line"),
            (3, "", "", "git exited 3"),
        ] {
            let rig = Rig::new();
            let (shared, entry) =
                land_entry(&rig, of_record("wsp/pricing-page"), Some("pl_boat"), false);
            let before = shared.borrow().clone();
            rig.ctx.env_reply(Ok(exec_result(code, stdout, stderr)));
            let err = rig
                .area
                .land_on_record(&entry, &project(WEB), None)
                .await
                .unwrap_err();
            assert_eq!(
                err,
                plain_error(said),
                "stdout {stdout:?} stderr {stderr:?}"
            );
            assert_eq!(
                rig.ctx.events(),
                vec!["mint op clone", "env_exec", "revoke dev_1"]
            );
            assert_eq!(*shared.borrow(), before);
        }
    });
}

// B12: spec only (branch-of-record.ts:176-178)
#[test]
fn a_start_sha_already_recorded_is_kept() {
    run(async {
        let rig = Rig::new();
        let recorded = with(of_record("wsp/pricing-page"), "startSha", json!(PARENT_SHA));
        let (shared, entry) = land_entry(&rig, recorded.clone(), Some("pl_boat"), false);
        rig.ctx
            .env_reply(Ok(exec_result(0, &format!("{IMAGE_SHA}\n"), "")));
        rig.area
            .land_on_record(&entry, &project(WEB), None)
            .await
            .unwrap();
        assert_eq!(shared.borrow().branch_of_record, Some(branch_of(recorded)));
        assert_eq!(rig.ctx.persisted(), Vec::<WorkspaceRecord>::new());
        assert_eq!(
            rig.ctx.events(),
            vec!["mint op clone", "env_exec", "revoke dev_1"]
        );
    });
}

// B12: spec only (branch-of-record.ts:177)
#[test]
fn a_last_line_that_is_no_commit_writes_nothing() {
    run(async {
        let upper = IMAGE_SHA.to_uppercase();
        for stdout in [
            String::new(),
            "HEAD is now at 5e6f7a8\n".to_string(),
            format!("{upper}\n"),
            format!("{}\n", &IMAGE_SHA[..39]),
            format!("{IMAGE_SHA}\nclean\n"),
        ] {
            let rig = Rig::new();
            let (shared, entry) =
                land_entry(&rig, of_record("wsp/pricing-page"), Some("pl_boat"), false);
            let before = shared.borrow().clone();
            rig.ctx.env_reply(Ok(exec_result(0, &stdout, "")));
            rig.area
                .land_on_record(&entry, &project(WEB), None)
                .await
                .unwrap();
            assert_eq!(*shared.borrow(), before, "stdout {stdout:?}");
            assert_eq!(
                rig.ctx.persisted(),
                Vec::<WorkspaceRecord>::new(),
                "stdout {stdout:?}"
            );
        }
    });
}
