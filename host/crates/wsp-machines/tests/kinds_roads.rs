// SPDX-License-Identifier: AGPL-3.0-only
//! spec/kinds_roads.md: the reads and roads every kind of workspace shares, ported from projects.test.ts (the clone
//! cases), terminal-folder.test.ts, agents-brokered.test.ts, places-agents.test.ts and sub-codex-models.test.ts.
#![allow(clippy::await_holding_refcell_ref)]
mod machines_b_support;

use std::path::Path;
use std::process::Command;
use std::rc::Rc;
use std::sync::Arc;

use indexmap::IndexMap;
use serde_json::{json, Value};

use machines_b_support::kinds_ctx::*;
use machines_b_support::*;
use wsp_adapters::codex::command::{BrokerMode, CodexBroker};
use wsp_core::types::harness::{Brokered, BrokeredKind};
use wsp_core::types::pending::MachineExecOptions;
use wsp_core::types::wiring::{StageReport, WorkspaceRecord};
use wsp_engine::machine::MachineBackend;
use wsp_machines::machines::kinds::{BrokeredLaunch, KindsArea, KindsCtx};
use wsp_machines::machines::machines::Entry;
use wsp_wire::types::agents_report::AgentSignInState;
use wsp_wire::types::shell_quote::shell_line;
use wsp_wire::views::workspace::ProjectSource;
use wsp_wire::views::workspace::ProjectView;
use wsp_wire::views::workspace_kind::WorkspaceKind;
use wsp_words::exit::{HostError, Kind};
use wsp_words::projects::{clone_into_file_line, clone_into_taken_line, clone_url_refusal};

const REPO: &str = "https://github.com/spoo-me/frontend.git";

fn s(p: &Path) -> &str {
    p.to_str().unwrap()
}

fn git_source(url: &str) -> ProjectSource {
    serde_json::from_value(json!({ "kind": "git", "url": url })).unwrap()
}

fn gh_source(repo: &str) -> ProjectSource {
    serde_json::from_value(json!({ "kind": "github", "repo": repo })).unwrap()
}

fn invalid(e: &HostError, message: &str) {
    assert_eq!(e.message, message);
    assert_eq!(e.kind, Some(Kind::Invalid));
}

fn names(dir: &Path) -> Vec<String> {
    let mut found: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    found.sort();
    found
}

fn canonical(p: &Path) -> std::path::PathBuf {
    std::fs::canonicalize(p).unwrap()
}

fn git(dir: &Path, args: &[&str]) {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

// ---------------------------------------------------------------------------------------------------------------
// Cloning on this computer (rule 7).

#[test]
fn clones_an_https_url_into_the_folder_named_by_argv_with_no_prompt() {
    run(async {
        let rig = CloneRig::new();
        let (_host, area) = kinds();
        let shell = rig.shell();
        world(|w| w.local = Some(shell.clone()));
        let dest = rig.folder_at(&["clones", "spoo.me"]);
        let url = "https://github.com/spoo-me/spoo.me";
        let got = area
            .clone_here(url, &git_source(url), s(&dest))
            .await
            .unwrap();
        assert_eq!(got, s(&dest));
        assert_eq!(
            std::fs::read_to_string(dest.join("README.md")).unwrap(),
            "spoo\n"
        );
        let clones: Vec<Run> = rig
            .calls()
            .into_iter()
            .filter(|c| c.argv.get(1).map(String::as_str) == Some("clone"))
            .collect();
        assert_eq!(
            clones,
            vec![Run {
                prompt: "0".to_string(),
                argv: ["git", "clone", "--", url, s(&dest)]
                    .map(String::from)
                    .to_vec()
            }]
        );
        let line = format!(
            "{} {}",
            shell_line(&["env", "GIT_TERMINAL_PROMPT=0"]),
            shell_line(&["git", "clone", "--", url, s(&dest)])
        );
        assert_eq!(shell.ran(), vec![line]);
        assert_eq!(shell.timeouts(), vec![Some(300_000)]);
    });
}

#[test]
fn clones_owner_repo_through_gh_into_an_empty_folder_of_its_name() {
    run(async {
        let rig = CloneRig::new();
        let (_host, area) = kinds();
        let shell = rig.shell();
        world(|w| w.local = Some(shell.clone()));
        let dest = rig.folder_at(&["spoo.me"]);
        std::fs::create_dir(&dest).unwrap();
        let got = area
            .clone_here("spoo-me/spoo.me", &gh_source("spoo-me/spoo.me"), s(&dest))
            .await
            .unwrap();
        assert_eq!(got, s(&dest));
        assert_eq!(
            std::fs::read_to_string(dest.join("README.md")).unwrap(),
            "spoo\n"
        );
        let gh: Vec<Run> = rig
            .calls()
            .into_iter()
            .filter(|c| c.argv[0] == "gh")
            .collect();
        assert_eq!(
            gh,
            vec![Run {
                prompt: "0".to_string(),
                argv: ["gh", "repo", "clone", "spoo-me/spoo.me", s(&dest)]
                    .map(String::from)
                    .to_vec()
            }]
        );
        let line = format!(
            "{} {}",
            shell_line(&["env", "GIT_TERMINAL_PROMPT=0"]),
            shell_line(&["gh", "repo", "clone", "spoo-me/spoo.me", s(&dest)])
        );
        assert_eq!(shell.ran(), vec![line]);
    });
}

#[test]
fn clones_into_a_folder_of_the_repos_name_inside_a_picked_folder() {
    run(async {
        let rig = CloneRig::new();
        let (_host, area) = kinds();
        world(|w| w.local = Some(rig.shell()));
        let downloads = rig.folder_at(&["Downloads"]);
        std::fs::create_dir(&downloads).unwrap();
        std::fs::write(downloads.join("invoice.pdf"), "mine").unwrap();
        let lab = "https://github.com/acme/lab.git";
        let got = area
            .clone_here(lab, &git_source(lab), s(&downloads))
            .await
            .unwrap();
        let dest = downloads.join("lab");
        assert_eq!(got, s(&dest));
        assert_eq!(
            std::fs::read_to_string(dest.join("README.md")).unwrap(),
            "spoo\n"
        );
        assert_eq!(names(&downloads), vec!["invoice.pdf", "lab"]);
        // An empty folder of another name is a folder picked too, so the clone lands inside it, and so does one of
        // another name that does not stand yet, which the clone makes.
        let empty = rig.folder_at(&["code"]);
        std::fs::create_dir(&empty).unwrap();
        assert_eq!(
            area.clone_here("acme/site", &gh_source("acme/site"), s(&empty))
                .await
                .unwrap(),
            s(&empty.join("site"))
        );
        let fresh = rig.folder_at(&["fresh", "work"]);
        let kit = "https://github.com/acme/kit";
        assert_eq!(
            area.clone_here(kit, &git_source(kit), s(&fresh))
                .await
                .unwrap(),
            s(&fresh.join("kit"))
        );
        assert_eq!(names(&fresh), vec!["kit"]);
        // A folder named for the repo, or for it with a number, is the clone's own folder; a longer name is a parent.
        let shelf = rig.folder_at(&["lab-4-shelf"]);
        let lab4 = "https://github.com/acme/lab-4.git";
        assert_eq!(
            area.clone_here(lab4, &git_source(lab4), s(&shelf))
                .await
                .unwrap(),
            s(&shelf.join("lab-4"))
        );
        let seventh = rig.folder_at(&["kit-7"]);
        let kit_git = "https://github.com/acme/kit.git";
        assert_eq!(
            area.clone_here(kit_git, &git_source(kit_git), s(&seventh))
                .await
                .unwrap(),
            s(&seventh)
        );
        assert_eq!(
            rig.clone_targets(),
            vec![
                s(&dest),
                s(&empty.join("site")),
                s(&fresh.join("kit")),
                s(&shelf.join("lab-4")),
                s(&seventh)
            ]
        );
    });
}

#[test]
fn refuses_a_full_folder_of_the_repos_name_offering_the_next_free() {
    run(async {
        let rig = CloneRig::new();
        let (_host, area) = kinds();
        world(|w| w.local = Some(rig.shell()));
        let downloads = rig.folder_at(&["Downloads"]);
        let lab = downloads.join("lab");
        std::fs::create_dir_all(&lab).unwrap();
        std::fs::write(lab.join("notes.txt"), "mine").unwrap();
        let url = "https://github.com/acme/lab";
        let src = git_source(url);
        let e = area.clone_here(url, &src, s(&downloads)).await.unwrap_err();
        invalid(
            &e,
            &clone_into_taken_line(s(&lab), s(&downloads.join("lab-2"))),
        );
        std::fs::create_dir(downloads.join("lab-2")).unwrap();
        let e = area.clone_here(url, &src, s(&downloads)).await.unwrap_err();
        invalid(
            &e,
            &clone_into_taken_line(s(&lab), s(&downloads.join("lab-3"))),
        );
        // The folder of the repo's name picked itself, full, is refused the same way and never cloned into.
        let e = area.clone_here(url, &src, s(&lab)).await.unwrap_err();
        invalid(
            &e,
            &clone_into_taken_line(s(&lab), s(&downloads.join("lab-3"))),
        );
        // A file of the repo's name is named as a file.
        let kit = downloads.join("kit");
        std::fs::write(&kit, "a file").unwrap();
        let kit_url = "https://github.com/acme/kit";
        let kit_src = git_source(kit_url);
        let e = area
            .clone_here(kit_url, &kit_src, s(&downloads))
            .await
            .unwrap_err();
        invalid(
            &e,
            &clone_into_file_line(s(&kit), Some(s(&downloads.join("kit-2")))),
        );
        let e = area
            .clone_here(kit_url, &kit_src, s(&kit))
            .await
            .unwrap_err();
        invalid(
            &e,
            &clone_into_file_line(s(&kit), Some(s(&downloads.join("kit-2")))),
        );
        // The name offered is a folder that does not stand yet, which is itself the clone's folder.
        let third = downloads.join("lab-3");
        assert_eq!(
            area.clone_here(url, &src, s(&third)).await.unwrap(),
            s(&third)
        );
        assert_eq!(names(&lab), vec!["notes.txt"]);
        assert_eq!(rig.clone_targets(), vec![s(&third)]);
    });
}

#[test]
fn refuses_a_file_a_url_no_clone_should_run_cloning_nothing() {
    run(async {
        let rig = CloneRig::new();
        let (_host, area) = kinds();
        let shell = rig.shell();
        world(|w| w.local = Some(shell.clone()));
        let full = rig.folder_at(&["full"]);
        std::fs::create_dir(&full).unwrap();
        std::fs::write(full.join("notes.txt"), "mine").unwrap();
        let notes = full.join("notes.txt");
        let e = area
            .clone_here(REPO, &git_source(REPO), s(&notes))
            .await
            .unwrap_err();
        invalid(&e, &clone_into_file_line(s(&notes), None));
        for word in [
            "file:///etc",
            "ssh://-oProxyCommand=touch/x",
            "ssh://-oProxyCommand=true@github.com/a/b",
        ] {
            let into = rig.folder_at(&["x"]);
            let e = area
                .clone_here(word, &git_source(word), s(&into))
                .await
                .unwrap_err();
            invalid(
                &e,
                &clone_url_refusal(word).expect("a refusal for this word"),
            );
            assert!(!into.exists());
        }
        let e = area
            .clone_here(
                "ssh://-oProxyCommand=touch/x",
                &git_source("ssh://-oProxyCommand=touch/x"),
                s(&rig.folder_at(&["y"])),
            )
            .await
            .unwrap_err();
        assert!(
            e.message.contains("is not a repo address wsp clones"),
            "{}",
            e.message
        );
        assert!(rig.clone_targets().is_empty());
        assert!(shell.ran().is_empty(), "no command ran: {:?}", shell.ran());
        assert_eq!(names(&full), vec!["notes.txt"]);
    });
}

#[test]
fn says_gits_last_line_when_the_clone_fails_and_leaves_no_folder_it_made() {
    run(async {
        let rig = CloneRig::new();
        let (_host, area) = kinds();
        world(|w| w.local = Some(rig.shell()));
        // A folder picked that does not stand yet is made by the clone, and a clone that fails leaves none of it.
        let top = rig.folder_at(&["gone"]);
        let dest = top.join("code");
        let missing = "https://github.com/me/missing.git";
        let e = area
            .clone_here(missing, &git_source(missing), s(&dest))
            .await
            .unwrap_err();
        invalid(
            &e,
            "fatal: repository 'https://github.com/me/missing.git' does not exist",
        );
        let private = "https://github.com/me/private.git";
        let e = area
            .clone_here(private, &git_source(private), s(&dest))
            .await
            .unwrap_err();
        invalid(&e, "fatal: repository 'https://github.com/me/private.git' not found; sign in with gh auth login, or use the repo's ssh url");
        assert!(!top.exists());
        assert_eq!(
            rig.clone_targets(),
            vec![s(&dest.join("missing")), s(&dest.join("private"))]
        );
    });
}

// ---------------------------------------------------------------------------------------------------------------
// The clone and install inside a fork (rules 2 and 3).

/// A fork of a project on `default`, its machine answering `ls -A` with `listing`.
fn fork(
    listing: &str,
    project_extra: Value,
    record_extra: Value,
) -> (
    Rc<FakeHost>,
    Rc<KindsArea>,
    Entry,
    Arc<FakeMachine>,
    ProjectView,
) {
    let (host, area) = kinds();
    let p = project("pr_1", "/root/spoo-landing", project_extra);
    hold(&p);
    let machine = FakeMachine::new("bx_7");
    machine.answer("ls -A", ok(listing));
    let mut extra = json!({ "project": "pr_1" });
    for (k, v) in record_extra.as_object().unwrap() {
        extra[k] = v.clone();
    }
    let entry = host.put_live(record_of("ws_7", extra, &["place", "sub"]), machine.clone());
    (host, area, entry, machine, p)
}

async fn land(
    area: &KindsArea,
    entry: &Entry,
    p: &ProjectView,
    report: StageReport,
) -> Result<(), HostError> {
    land_project_of(
        &*area
            .module_of(WorkspaceKind::Cloud)
            .expect("cloud is wired"),
        entry,
        p.clone(),
        report,
    )
    .await
}

fn of_record() -> Value {
    json!({ "repoId": 1, "owner": "spoo-me", "name": "frontend", "branch": "wsp/x" })
}

#[test]
fn clones_the_repo_into_a_fork_before_ready_with_its_base() {
    run(async {
        let (host, area, entry, machine, p) = fork("", json!({ "base": "main" }), json!({}));
        let (seen, report) = reports();
        land(&area, &entry, &p, report).await.unwrap();
        assert_eq!(
            *seen.borrow(),
            vec![(
                "project-cloned".to_string(),
                format!("Cloning {REPO} into /root/spoo-landing.")
            )]
        );
        let calls = host.calls_of("clone_lines");
        assert_eq!(calls.len(), 1);
        let o: Value = serde_json::from_str(&calls[0][0]).unwrap();
        assert_eq!(
            (
                o["remote"].clone(),
                o["checkout"].clone(),
                o["computer"].clone(),
                o["branch"].clone()
            ),
            (
                json!(REPO),
                json!("/root/spoo-landing"),
                json!("default"),
                json!("main")
            )
        );
        assert!(o.get("brokered").is_none());
        let clone = format!(
            "set -e\nmkdir -p '/root'\ngit clone --branch main -- {REPO} /root/spoo-landing"
        );
        assert_eq!(
            machine.log(),
            vec![clone, "ls -A '/root/spoo-landing'".to_string()]
        );

        // A copy the computer copied the checkout into holds it already: nothing is cloned, listed or installed.
        let (host, area, entry, machine, p) = fork(
            "package-lock.json\n",
            json!({ "base": "main", "checkout": "/mnt/copy" }),
            json!({}),
        );
        let (seen, report) = reports();
        land(&area, &entry, &p, report).await.unwrap();
        assert!(
            seen.borrow().is_empty()
                && machine.log().is_empty()
                && host.calls_of("land_on_record").is_empty()
        );
        let (host, area, entry, machine, p) = fork(
            "",
            json!({ "checkout": "/mnt/copy" }),
            json!({ "branchOfRecord": of_record() }),
        );
        land(&area, &entry, &p, reports().1).await.unwrap();
        assert_eq!(
            host.calls_of("land_on_record"),
            vec![vec![
                "ws_7".to_string(),
                "pr_1".to_string(),
                "null".to_string()
            ]]
        );
        assert!(machine.log().is_empty());

        // A fork of a thread's workspace starts from the branch its record names, cloned through the host's credentials.
        let (host, area, entry, machine, p) = fork(
            "",
            json!({ "base": "main" }),
            json!({ "branchOfRecord": of_record(), "base": "feat/x" }),
        );
        entry.borrow_mut().of_record_held = true;
        let (seen, report) = reports();
        land(&area, &entry, &p, report).await.unwrap();
        assert_eq!(seen.borrow().len(), 1);
        let o: Value = serde_json::from_str(&host.calls_of("clone_lines")[0][0]).unwrap();
        assert_eq!(o["branch"], json!("feat/x"));
        assert_eq!(
            (
                o["brokered"]["origin"].clone(),
                o["brokered"]["held"].clone(),
                o["brokered"]["ofRecord"]["branch"].clone()
            ),
            (json!("spoo-me/frontend"), json!(true), json!("wsp/x"))
        );
        let lines = vec![
            "set -e".to_string(),
            "mkdir -p '/root'".to_string(),
            format!("git clone --depth 1 --branch feat/x -- {REPO} /root/spoo-landing"),
        ];
        assert_eq!(
            host.calls_of("land_on_record"),
            vec![vec![
                "ws_7".to_string(),
                "pr_1".to_string(),
                serde_json::to_string(&Some(lines)).unwrap()
            ]]
        );
        assert_eq!(
            machine.log(),
            vec!["ls -A '/root/spoo-landing'".to_string()],
            "the host's clone ran through land_on_record, not on the machine"
        );
    });
}

#[test]
fn with_no_base_the_clone_takes_the_remotes_default() {
    run(async {
        let (host, area, entry, machine, p) = fork("", json!({}), json!({}));
        land(&area, &entry, &p, reports().1).await.unwrap();
        let o: Value = serde_json::from_str(&host.calls_of("clone_lines")[0][0]).unwrap();
        assert!(o.get("branch").is_none(), "no --branch without a base: {o}");
        assert_eq!(
            machine.log()[0],
            format!("set -e\nmkdir -p '/root'\ngit clone -- {REPO} /root/spoo-landing")
        );
    });
}

#[test]
fn a_clone_that_fails_ends_with_gits_last_line() {
    run(async {
        let (_host, area, entry, machine, p) = fork("", json!({}), json!({}));
        machine.answer("git clone", exit(128, "Cloning into '/root/x'...\nfatal: could not read Username for 'https://github.com'"));
        let e = land(&area, &entry, &p, reports().1).await.unwrap_err();
        assert_eq!(e.message, "the clone of spoo-me/frontend on default failed: fatal: could not read Username for 'https://github.com'");
        assert_eq!(
            machine.log().len(),
            1,
            "nothing is listed or installed after a failed clone"
        );

        // Where git said nothing on stderr, stdout is what it said.
        let (_host, area, entry, machine, p) = fork("", json!({}), json!({}));
        machine.answer(
            "git clone",
            Reply::Out {
                code: 128,
                stdout: "fatal: said on stdout\n".to_string(),
                stderr: String::new(),
            },
        );
        let e = land(&area, &entry, &p, reports().1).await.unwrap_err();
        assert_eq!(
            e.message,
            "the clone of spoo-me/frontend on default failed: fatal: said on stdout"
        );
    });
}

#[test]
fn installs_the_lockfiles_row_once_and_names_the_log_on_failure() {
    run(async {
        let clone = format!("set -e\nmkdir -p '/root'\ngit clone -- {REPO} /root/spoo-landing");
        let install =
            "mkdir -p '/tmp' && cd '/root/spoo-landing' && npm ci >> '/tmp/install-pr_1.log' 2>&1"
                .to_string();

        let (_host, area, entry, machine, p) =
            fork("package-lock.json\nsrc\n", json!({}), json!({}));
        let (seen, report) = reports();
        land(&area, &entry, &p, report).await.unwrap();
        assert_eq!(
            *seen.borrow(),
            vec![
                (
                    "project-cloned".to_string(),
                    format!("Cloning {REPO} into /root/spoo-landing.")
                ),
                (
                    "project-cloned".to_string(),
                    "npm ci in /root/spoo-landing.".to_string()
                )
            ]
        );
        assert_eq!(
            machine.log(),
            vec![
                clone.clone(),
                "ls -A '/root/spoo-landing'".to_string(),
                install.clone()
            ]
        );

        // Two lockfiles name two rows; only the first runs.
        let (_host, area, entry, machine, p) =
            fork("Cargo.lock\npackage-lock.json\n", json!({}), json!({}));
        land(&area, &entry, &p, reports().1).await.unwrap();
        assert_eq!(machine.log().len(), 3);
        assert_eq!(machine.log()[2], install);

        // A repo no row names an install for installs nothing.
        let (_host, area, entry, machine, p) = fork("README.md\n", json!({}), json!({}));
        let (seen, report) = reports();
        land(&area, &entry, &p, report).await.unwrap();
        assert_eq!((seen.borrow().len(), machine.log().len()), (1, 2));

        let failing = |reply: Reply| {
            let (_host, area, entry, machine, p) =
                fork("package-lock.json\n", json!({}), json!({}));
            machine.answer(">> '/tmp/install-pr_1.log'", reply);
            async move {
                land(&area, &entry, &p, reports().1)
                    .await
                    .unwrap_err()
                    .message
            }
        };
        let said = |last: &str| {
            format!("npm ci in /root/spoo-landing: {last}; its whole output is /tmp/install-pr_1.log on the machine")
        };
        assert_eq!(
            failing(exit(1, "npm ERR! x\nnpm ERR! boom\n")).await,
            said("npm ERR! boom")
        );
        assert_eq!(
            failing(Reply::Out {
                code: 1,
                stdout: "first\nlast out\n".to_string(),
                stderr: String::new()
            })
            .await,
            said("last out")
        );
        assert_eq!(failing(exit(3, "")).await, said("exit 3"));
    });
}

// ---------------------------------------------------------------------------------------------------------------
// The folder a turn or a command starts in (rule 25).

#[test]
fn thread_folder_takes_cwd_then_the_kinds_folder_then_the_project() {
    run(async {
        let (host, area) = kinds();
        hold(&project("pr_1", "/root/spoo-landing", json!({})));
        let local_project = project("pr_l", "/home/z/lab", json!({ "computer": "here" }));
        hold(&local_project);
        // A sub-thread's folder is the sub's own, on Boat.
        let sub = host.put_live(sub_record("ws_sub", "running"), FakeMachine::new("bx_s"));
        assert_eq!(
            area.thread_folder(&sub.borrow(), None).await.unwrap(),
            "/root/web"
        );
        assert_eq!(
            area.thread_folder(&sub.borrow(), Some("/tmp"))
                .await
                .unwrap(),
            "/tmp"
        );
        assert_eq!(
            area.thread_folder(&sub.borrow(), Some("")).await.unwrap(),
            ""
        );
        // A fork that is no sub-thread has no folder of the kind's own: its project's path is the answer.
        let fork = host.put_live(
            record_of("ws_fork", json!({ "project": "pr_1" }), &["place", "sub"]),
            FakeMachine::new("bx_f"),
        );
        assert_eq!(
            area.thread_folder(&fork.borrow(), None).await.unwrap(),
            "/root/spoo-landing"
        );
        // A folder here is the checkout of its record: the project's own, or the worktree wsp made.
        let local = host.put_live(
            record_of(
                "ws_loc",
                json!({ "kind": "local", "project": "pr_l" }),
                &["place", "sub"],
            ),
            FakeMachine::new("loc"),
        );
        assert_eq!(
            area.thread_folder(&local.borrow(), None).await.unwrap(),
            "/home/z/lab"
        );
        set_worktree(&local, json!({ "path": "/home/z/lab-wt", "made": true }));
        assert_eq!(
            area.thread_folder(&local.borrow(), None).await.unwrap(),
            "/home/z/lab-wt"
        );
        assert_eq!(
            area.thread_folder(&local.borrow(), Some("/home/z/lab/sub"))
                .await
                .unwrap(),
            "/home/z/lab/sub"
        );
    });
}

// ---------------------------------------------------------------------------------------------------------------
// What the broker holds and the launch a brokered workspace carries (rules 16 to 19).

fn brokered_entry(host: &FakeHost, id: &str) -> Entry {
    host.put_live(
        record_of(id, json!({ "branchOfRecord": of_record() }), &[]),
        FakeMachine::new(&format!("bx_{id}")),
    )
}

/// A launch as its words read: the env, the vault, the Claude placeholder and the codex broker's mode.
type Shape = (
    String,
    String,
    Option<(String, BrokeredKind)>,
    Option<String>,
);

fn codex_word(codex: &Option<CodexBroker>) -> Option<String> {
    codex.as_ref().map(|c| match &c.mode {
        BrokerMode::Apikey => "apikey".to_string(),
        BrokerMode::Chatgpt { claims, .. } => format!("chatgpt:{}", claims.account_id),
    })
}

fn shape_of(launch: Option<BrokeredLaunch>) -> Option<Shape> {
    launch.map(|l| {
        (
            text(&l.env),
            text(&l.vault),
            l.claude.map(|c| (c.token, c.kind)),
            codex_word(&l.codex),
        )
    })
}

fn empty_shape() -> Shape {
    ("{}".to_string(), "{}".to_string(), None, None)
}

#[test]
fn a_brokered_launch_carries_the_token_env_and_placeholders() {
    run(async {
        let (host, area) = kinds();
        let entry = brokered_entry(&host, "ws_b");
        host.script(
            "broker_status",
            Ok(json!({ "claude": "key", "codex": "key" })),
        );
        area.read_broker().await;
        assert!(
            host.calls_of("broker_codex_claims").is_empty(),
            "claims are read only for a ChatGPT login"
        );

        let launch = area
            .brokered_launch(&entry.borrow(), "claude", Some("tok"))
            .unwrap()
            .unwrap();
        assert_eq!(
            text(&launch.env),
            r#"{"GH_TOKEN":"tok","WSP_BRANCH_OF_RECORD":"spoo-me/frontend@wsp/x"}"#
        );
        assert_eq!(text(&launch.vault), r#"{"OPENAI_API_KEY":"sk-wsp-tok"}"#);
        assert_eq!(
            (launch.claude, codex_word(&launch.codex)),
            (
                Some(Brokered {
                    token: "tok".to_string(),
                    kind: BrokeredKind::Key
                }),
                None
            )
        );

        let codex = area
            .brokered_launch(&entry.borrow(), "codex", Some("tok"))
            .unwrap()
            .unwrap();
        assert_eq!(codex_word(&codex.codex), Some("apikey".to_string()));

        // A launch with no token carries only what codex reads.
        assert_eq!(
            shape_of(
                area.brokered_launch(&entry.borrow(), "claude", None)
                    .unwrap()
            ),
            Some(empty_shape())
        );
        assert_eq!(
            shape_of(
                area.brokered_launch(&entry.borrow(), "codex", None)
                    .unwrap()
            ),
            Some((
                empty_shape().0,
                empty_shape().1,
                None,
                Some("apikey".to_string())
            ))
        );

        // An agent the broker adds no key for is refused with a token and carries nothing without one.
        let e = area
            .brokered_launch(&entry.borrow(), "gemini", Some("tok"))
            .err()
            .unwrap();
        let said = unbrokered("gemini").unwrap();
        assert_eq!((e.message, e.kind), (said.message, Some(Kind::Usage)));
        assert_eq!(
            shape_of(
                area.brokered_launch(&entry.borrow(), "gemini", None)
                    .unwrap()
            ),
            Some(empty_shape())
        );

        // A workspace on a joined computer, or any while the broker is not ready, is not brokered.
        let joined = host.put_live(
            record_of("ws_j", json!({ "place": "srv" }), &["sub"]),
            FakeMachine::new("bx_j"),
        );
        assert!(!area.brokered(&held_record(&joined)));
        assert!(area
            .brokered_launch(&joined.borrow(), "claude", Some("tok"))
            .unwrap()
            .is_none());
        assert!(area.brokered(&held_record(&entry)));
        world(|w| w.broker_ready = false);
        assert!(!area.brokered(&held_record(&entry)));
        assert!(area
            .brokered_launch(&entry.borrow(), "claude", Some("tok"))
            .unwrap()
            .is_none());
    });
}

#[test]
fn a_codex_launch_refuses_where_the_broker_named_no_login() {
    run(async {
        // The host holds a ChatGPT login and its claims: codex runs on a placeholder of it.
        let (host, area) = kinds();
        let entry = brokered_entry(&host, "ws_b");
        host.script("broker_status", Ok(json!({ "codex": "chatgpt" })));
        host.script(
            "broker_codex_claims",
            Ok(json!({ "accountId": "acct-acme" })),
        );
        area.read_broker().await;
        let launch = area
            .brokered_launch(&entry.borrow(), "codex", Some("tok"))
            .unwrap()
            .unwrap();
        assert_eq!(
            codex_word(&launch.codex),
            Some("chatgpt:acct-acme".to_string())
        );
        assert_eq!(
            text(&launch.env),
            r#"{"GH_TOKEN":"tok","WSP_BRANCH_OF_RECORD":"spoo-me/frontend@wsp/x"}"#
        );
        assert!(launch.vault.is_empty());

        // The claims read fails: no login is held, so codex there would start unsigned and the launch refuses.
        let (host, area) = kinds();
        let entry = brokered_entry(&host, "ws_b");
        host.script("broker_status", Ok(json!({ "codex": "chatgpt" })));
        host.script(
            "broker_codex_claims",
            Err(failed("chatgpt.com unreachable")),
        );
        area.read_broker().await;
        let unread = codex_unread();
        let e = area
            .brokered_launch(&entry.borrow(), "codex", Some("tok"))
            .err()
            .unwrap();
        assert_eq!(
            (e.message, e.kind),
            (unread.message.clone(), Some(Kind::Auth))
        );
        assert_eq!(
            shape_of(
                area.brokered_launch(&entry.borrow(), "codex", None)
                    .unwrap()
            ),
            Some(empty_shape())
        );
        assert!(
            area.brokered_launch(&entry.borrow(), "claude", Some("tok"))
                .unwrap()
                .is_some(),
            "claude needs no codex login"
        );

        // The status read fails, or names no ChatGPT login: the claims are never asked for.
        let (host, area) = kinds();
        let entry = brokered_entry(&host, "ws_b");
        host.script("broker_status", Err(failed("broker is down")));
        area.read_broker().await;
        assert!(area
            .brokered_launch(&entry.borrow(), "codex", Some("tok"))
            .is_err());
        host.script("broker_status", Ok(json!({})));
        area.read_broker().await;
        assert!(area
            .brokered_launch(&entry.borrow(), "codex", Some("tok"))
            .is_err());
        assert!(host.calls_of("broker_codex_claims").is_empty());
        host.script("broker_status", Ok(json!({ "codex": "access-token" })));
        host.script(
            "broker_codex_claims",
            Ok(json!({ "accountId": "acct-acme" })),
        );
        area.read_broker().await;
        assert_eq!(host.calls_of("broker_codex_claims").len(), 1);
        assert!(area
            .brokered_launch(&entry.borrow(), "codex", Some("tok"))
            .unwrap()
            .unwrap()
            .codex
            .is_some());

        // know_broker reads where no codex login is known, once, and refuses where there is still none.
        let (host, area) = kinds();
        let entry = brokered_entry(&host, "ws_b");
        host.script("broker_status", Ok(json!({ "codex": "chatgpt" })));
        host.script(
            "broker_codex_claims",
            Ok(json!({ "accountId": "acct-acme" })),
        );
        area.know_broker(&entry.borrow(), "claude").await.unwrap();
        let joined = host.put_live(
            record_of("ws_j", json!({ "place": "srv" }), &["sub"]),
            FakeMachine::new("bx_j"),
        );
        area.know_broker(&joined.borrow(), "codex").await.unwrap();
        assert!(
            host.calls_of("broker_status").is_empty(),
            "no read for another agent or an unbrokered workspace"
        );
        area.know_broker(&entry.borrow(), "codex").await.unwrap();
        area.know_broker(&entry.borrow(), "codex").await.unwrap();
        assert_eq!(host.calls_of("broker_status").len(), 1);

        let (host, area) = kinds();
        let entry = brokered_entry(&host, "ws_b");
        host.script("broker_status", Err(failed("broker is down")));
        let e = area
            .know_broker(&entry.borrow(), "codex")
            .await
            .unwrap_err();
        assert_eq!((e.message, e.kind), (unread.message, Some(Kind::Auth)));
        host.script("broker_status", Err(failed("broker is down")));
        assert!(area.know_broker(&entry.borrow(), "codex").await.is_err());
        assert_eq!(
            host.calls_of("broker_status").len(),
            2,
            "a read that found none is read again"
        );
    });
}

#[test]
fn read_broker_past_five_seconds_leaves_both_facts_none() {
    run(async {
        let (host, area) = kinds_with(|w| w.broker_has_status = false);
        let entry = brokered_entry(&host, "ws_b");
        area.read_broker().await;
        assert!(
            host.calls_of("broker_status").is_empty(),
            "a broker with no status is not read"
        );

        world(|w| w.broker_has_status = true);
        host.script("broker_status", Ok(json!({ "codex": "chatgpt" })));
        host.script(
            "broker_codex_claims",
            Ok(json!({ "accountId": "acct-acme" })),
        );
        area.read_broker().await;
        let held = area
            .brokered_launch(&entry.borrow(), "codex", Some("tok"))
            .unwrap()
            .unwrap();
        assert!(held.codex.is_some());

        host.script("broker_status", Ok(json!("never")));
        let reading = {
            let area = area.clone();
            tokio::task::spawn_local(async move { area.read_broker().await })
        };
        settle().await;
        host.clock.advance(4_999.0).await;
        assert!(
            !reading.is_finished(),
            "the read waits the whole 5 s on the clock"
        );
        host.clock.advance(1.0).await;
        reading.await.unwrap();
        assert_eq!(host.calls_of("broker_status").len(), 2);
        let e = area
            .brokered_launch(&entry.borrow(), "codex", Some("tok"))
            .err()
            .unwrap();
        assert_eq!(e.message, codex_unread().message);
        assert_eq!(
            shape_of(
                area.brokered_launch(&entry.borrow(), "codex", None)
                    .unwrap()
            ),
            Some(empty_shape())
        );
    });
}

// ---------------------------------------------------------------------------------------------------------------
// The agent homes and environment of a joined computer (rules 13, 14, 15, 20).

fn srv_world(set: impl FnOnce(&mut Door)) -> impl FnOnce(&mut World) {
    move |w| {
        let mut door = joined_door("Srv", "/home/srv", true);
        set(&mut door);
        w.place_door = true;
        w.doors.insert("srv".to_string(), door);
    }
}

fn srv_entry(host: &FakeHost) -> Entry {
    host.put_live(
        record_of(
            "ws_p",
            json!({ "kind": "place", "place": "srv", "project": "pr_p", "name": "lab" }),
            &["sub"],
        ),
        FakeMachine::new("srv-machine"),
    )
}

const LOGINS: &str = "/home/srv/.wsp/logins";

#[test]
fn a_joined_computers_agent_homes_and_env_follow_its_login() {
    run(async {
        // A shared login is kept in the computer's logins folder, its store variable pointed there; an agent with
        // none keeps its guest store under the login's home.
        let (host, area) = kinds_with(srv_world(|d| d.logins = Some(LOGINS.to_string())));
        let entry = srv_entry(&host);
        assert_eq!(
            area.place_agent_home(Some("srv"), "/home/srv", "codex")
                .unwrap(),
            format!("{LOGINS}/codex")
        );
        assert_eq!(
            area.place_agent_home(Some("srv"), "/home/srv", "claude")
                .unwrap(),
            "/home/srv/.claude-cfg"
        );
        assert_eq!(
            area.place_agent_home(Some("srv"), "/home/srv/", "claude")
                .unwrap(),
            "/home/srv/.claude-cfg"
        );
        assert_eq!(
            area.place_agent_home(None, "/home/srv", "codex").unwrap(),
            "/home/srv/.codex"
        );
        assert_eq!(
            area.place_agent_home(Some("srv"), "/home/srv", "nope")
                .unwrap_err()
                .message,
            "the catalog has no home for nope"
        );
        let place = area
            .module_of(WorkspaceKind::Place)
            .expect("a place door is wired");
        assert_eq!(
            place.home(&entry.borrow(), "codex"),
            format!("{LOGINS}/codex")
        );
        assert_eq!(
            place.home_dir(&held_record(&entry)),
            Some("/home/srv".to_string())
        );

        let (host, area) = kinds_with(srv_world(|_| {}));
        let entry = srv_entry(&host);
        assert_eq!(
            area.place_agent_home(Some("srv"), "/home/srv", "codex")
                .unwrap(),
            "/home/srv/.codex"
        );
        assert_eq!(
            area.module_of(WorkspaceKind::Place)
                .ok()
                .unwrap()
                .home(&entry.borrow(), "codex"),
            "/home/srv/.codex"
        );

        // A record naming a computer this host does not hold is held by nothing.
        let _place = area.module_of(WorkspaceKind::Place).ok().unwrap();
        let record = |id: &str, extra: Value, without: &[&str]| -> WorkspaceRecord {
            serde_json::from_value(record_of(id, extra, without)).unwrap()
        };
        let at_none = record(
            "ws_n",
            json!({ "kind": "place", "name": "lab" }),
            &["place", "sub"],
        );
        assert_eq!(
            area.place_of(&at_none).err().unwrap().message,
            "lab stands on no computer, which this host no longer holds"
        );
        let away = record(
            "ws_a",
            json!({ "kind": "place", "place": "away", "name": "lab" }),
            &["sub"],
        );
        assert_eq!(
            area.place_of(&away).err().unwrap().message,
            "lab stands on away, which this host no longer holds"
        );
        world(|w| {
            w.doors.insert(
                "gone".to_string(),
                Door {
                    name: "Gone".to_string(),
                    joined: true,
                    ..Door::default()
                },
            );
        });
        let gone = record(
            "ws_g",
            json!({ "kind": "place", "place": "gone", "name": "lab" }),
            &["sub"],
        );
        let refused = area.place_of(&gone).err().unwrap();
        assert_eq!(refused.message, "that computer reported no home folder for its login, so nothing on it could be reached; wsp holds a machine's home to a plain absolute path");
        assert_eq!(refused.kind, None);

        // The environment: the login's variables without USER and PATH, then HOME, then each agent's store and the
        // vault's sign-in its turn would be handed, then GitHub's token, only where the door says the vault holds it.
        let vault = [
            ("CLAUDE_CODE_OAUTH_TOKEN", "sk-ant-oat01-x"),
            ("OPENAI_API_KEY", "sk-oai"),
            ("GH_TOKEN", "ghp_x"),
        ];
        let signed_in = |id: &str| IndexMap::from([(id.to_string(), AgentSignInState::SignedIn)]);
        let env_in = |set: Box<dyn FnOnce(&mut Door)>| {
            let (host, area) = kinds_with(|w| {
                srv_world(set)(w);
                w.vault = vault
                    .iter()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect();
            });
            let entry = srv_entry(&host);
            let place = area.module_of(WorkspaceKind::Place).ok().unwrap();
            let env = place.env(&entry.borrow(), None, &|id| format!("/h/{id}"));
            (env.keys().cloned().collect::<Vec<_>>(), env)
        };
        let at = |keys: &[String], k: &str| {
            keys.iter()
                .position(|x| x == k)
                .unwrap_or_else(|| panic!("{k} is in {keys:?}"))
        };

        let (keys, env) = env_in(Box::new(move |d| {
            d.sign_ins = Some(signed_in("codex"));
            d.github_from_vault = true;
        }));
        assert!(
            !keys.contains(&"USER".to_string()) && !keys.contains(&"PATH".to_string()),
            "{keys:?}"
        );
        assert!(
            at(&keys, "HOME") < at(&keys, "IS_SANDBOX")
                && at(&keys, "HOME") < at(&keys, "DISABLE_AUTOUPDATER")
        );
        assert_eq!(env["HOME"], "/home/srv");
        assert!(at(&keys, "HOME") < at(&keys, "CLAUDE_CONFIG_DIR"));
        assert!(at(&keys, "CLAUDE_CONFIG_DIR") < at(&keys, "CLAUDE_CODE_OAUTH_TOKEN"));
        assert_eq!(
            (
                env["CLAUDE_CONFIG_DIR"].as_str(),
                env["CLAUDE_CODE_OAUTH_TOKEN"].as_str(),
                env["CODEX_HOME"].as_str()
            ),
            ("/h/claude", "sk-ant-oat01-x", "/h/codex")
        );
        assert!(
            !keys.contains(&"OPENAI_API_KEY".to_string()),
            "a login that stands there gets no key beside it"
        );
        assert_eq!(
            (keys.last().map(String::as_str), env["GH_TOKEN"].as_str()),
            (Some("GH_TOKEN"), "ghp_x")
        );

        let (keys, env) = env_in(Box::new(move |d| d.sign_ins = Some(signed_in("claude"))));
        assert!(
            !keys.contains(&"CLAUDE_CODE_OAUTH_TOKEN".to_string())
                && !keys.contains(&"GH_TOKEN".to_string()),
            "{keys:?}"
        );
        assert!(at(&keys, "CODEX_HOME") < at(&keys, "OPENAI_API_KEY"));
        assert_eq!(env["OPENAI_API_KEY"], "sk-oai");

        // The vault a workspace is handed is that computer's picks of it, whole where the door answers nothing.
        let (host, area) = kinds_with(|w| {
            srv_world(|d| d.vault_drops = Some(vec!["OPENAI_API_KEY".to_string()]))(w);
            w.vault = vault
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect();
        });
        let entry = srv_entry(&host);
        let place = area.module_of(WorkspaceKind::Place).ok().unwrap();
        assert_eq!(
            text(&area.vault_on(&entry.borrow())),
            r#"{"CLAUDE_CODE_OAUTH_TOKEN":"sk-ant-oat01-x","GH_TOKEN":"ghp_x"}"#
        );
        let fork = host.put_live(
            record_of("ws_f", json!({ "project": "pr_1" }), &["place", "sub"]),
            FakeMachine::new("bx_f"),
        );
        assert_eq!(area.vault_on(&fork.borrow()).len(), 3);
        world(|w| w.doors.get_mut("srv").unwrap().vault_drops = None);
        assert_eq!(area.vault_on(&entry.borrow()).len(), 3);
        assert!(!place.login_stands(&entry.borrow(), "codex"));
        world(|w| w.doors.get_mut("srv").unwrap().sign_ins = Some(signed_in("codex")));
        assert!(
            place.login_stands(&entry.borrow(), "codex")
                && !place.login_stands(&entry.borrow(), "claude")
        );

        // A record naming a computer the door does not hold has no environment: the refusal is `place_env`'s.
        let stray = host.put_live(
            record_of(
                "ws_s",
                json!({ "kind": "place", "place": "away", "name": "lab" }),
                &["sub"],
            ),
            FakeMachine::new("stray-machine"),
        );
        let e = area
            .place_env(&stray.borrow(), &|id| format!("/h/{id}"))
            .unwrap_err();
        assert_eq!(
            e.message,
            "lab stands on away, which this host no longer holds"
        );
    });
}

// ---------------------------------------------------------------------------------------------------------------
// Git on this computer, the folder reads and the state folder (rules 4, 5, 6).

#[test]
fn remote_here_reads_two_lines_and_drops_origin() {
    run(async {
        let (_host, area) = kinds();
        let tmp = tempfile::tempdir().unwrap();
        let root = canonical(tmp.path());
        let shell = LocalShell::new(&root, None);
        world(|w| w.local = Some(shell.clone()));
        let init = |name: &str| {
            let dir = root.join(name);
            let out = Command::new("git")
                .args(["init", "-q", "-b", "main"])
                .arg(&dir)
                .output()
                .unwrap();
            assert!(out.status.success());
            dir
        };

        let lab = init("lab");
        git(
            &lab,
            &[
                "remote",
                "add",
                "origin",
                "https://example.com/acme/lab.git",
            ],
        );
        git(
            &lab,
            &[
                "symbolic-ref",
                "refs/remotes/origin/HEAD",
                "refs/remotes/origin/main",
            ],
        );
        let got = area.remote_here(s(&lab)).await.unwrap();
        assert_eq!(
            got,
            (
                "https://example.com/acme/lab.git".to_string(),
                "main".to_string()
            )
        );
        let line = format!(
            "{} || echo; {} || echo",
            shell_line(&["git", "-C", s(&lab), "remote", "get-url", "origin"]),
            shell_line(&[
                "git",
                "-C",
                s(&lab),
                "symbolic-ref",
                "--short",
                "refs/remotes/origin/HEAD"
            ])
        );
        assert_eq!(
            (shell.ran(), shell.timeouts()),
            (vec![line], vec![Some(20_000)])
        );

        // A branch with a slash keeps it; only the leading origin/ goes.
        let slashed = init("slashed");
        git(
            &slashed,
            &[
                "remote",
                "add",
                "origin",
                "git@example.com:acme/slashed.git",
            ],
        );
        git(
            &slashed,
            &[
                "symbolic-ref",
                "refs/remotes/origin/HEAD",
                "refs/remotes/origin/release/1.0",
            ],
        );
        assert_eq!(
            area.remote_here(s(&slashed)).await.unwrap(),
            (
                "git@example.com:acme/slashed.git".to_string(),
                "release/1.0".to_string()
            )
        );

        // A folder with no origin has no remote and no default branch, not one of each.
        let bare = init("bare");
        assert_eq!(
            area.remote_here(s(&bare)).await.unwrap(),
            (String::new(), String::new())
        );
        let headless = init("headless");
        git(
            &headless,
            &[
                "remote",
                "add",
                "origin",
                "https://example.com/acme/headless.git",
            ],
        );
        assert_eq!(
            area.remote_here(s(&headless)).await.unwrap(),
            (
                "https://example.com/acme/headless.git".to_string(),
                String::new()
            )
        );
        let plain = root.join("plain");
        std::fs::create_dir(&plain).unwrap();
        assert_eq!(
            area.remote_here(s(&plain)).await.unwrap(),
            (String::new(), String::new())
        );

        // The top of the repo a folder sits in, the branch it has checked out, and git's own answer with stdin.
        std::fs::create_dir(lab.join("sub")).unwrap();
        assert_eq!(
            area.git_top_of(s(&lab.join("sub"))).await.unwrap(),
            Some(s(&lab).to_string())
        );
        assert_eq!(area.git_top_of(s(&plain)).await.unwrap(), None);
        assert_eq!(
            area.branch_at(s(&lab)).await.unwrap(),
            Some("main".to_string())
        );
        std::fs::write(lab.join("a.txt"), "a").unwrap();
        git(&lab, &["add", "."]);
        git(
            &lab,
            &[
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@t",
                "commit",
                "-qm",
                "one",
            ],
        );
        git(&lab, &["checkout", "-q", "--detach"]);
        assert_eq!(
            area.branch_at(s(&lab)).await.unwrap(),
            None,
            "nothing on a detached HEAD"
        );
        assert_eq!(area.branch_at(s(&plain)).await.unwrap(), None);
        let hashed = area
            .git_here(
                s(&lab),
                &["hash-object", "--stdin"],
                Some(5_000),
                Some("hello\n"),
            )
            .await
            .unwrap();
        assert_eq!(
            (hashed.exit_code.0 .0, hashed.stdout.as_str()),
            (0.0, "ce013625030ba8dba906f756967f9e9ca394464a\n")
        );
        let ran = shell.ran();
        assert_eq!(
            ran.last().unwrap(),
            &shell_line(&["git", "-C", s(&lab), "hash-object", "--stdin"])
        );
        assert_eq!(shell.timeouts().last(), Some(&Some(5_000)));
    });
}

#[test]
fn state_folder_refuses_without_a_state_file() {
    run(async {
        let (_host, area) = kinds();
        let e = area.state_folder().unwrap_err();
        assert_eq!(
            e.message,
            "this runtime was given no state file, so it keeps no worktrees or copies"
        );
        assert_eq!(e.kind, Some(Kind::Invalid));
        world(|w| w.state_path = Some("/var/lib/wsp/state.db".to_string()));
        assert_eq!(area.state_folder().unwrap(), "/var/lib/wsp");
        world(|w| w.state_path = Some("state/wsp.db".to_string()));
        assert_eq!(
            area.state_folder().unwrap(),
            s(&std::env::current_dir().unwrap().join("state"))
        );
    });
}

// ---------------------------------------------------------------------------------------------------------------
// The backend a record reads (rules 22, 23) and the memory key (rule 24).

#[test]
fn backend_for_a_released_or_cloud_gone_record_is_the_released_backend() {
    run(async {
        let boat = FakeBackend::new();
        let (host, area) = kinds_with(|w| {
            let mut door = joined_door("Boat", "/root", false);
            door.backend = Some(boat.clone());
            w.doors.insert("box".to_string(), door);
            w.place_door = true;
        });
        let own: Arc<dyn MachineBackend> = host.backend.clone();
        let boat: Arc<dyn MachineBackend> = boat;
        let released = host.released_backend();
        let record = |extra: Value, without: &[&str]| -> WorkspaceRecord {
            serde_json::from_value(record_of("ws_r", extra, without)).unwrap()
        };

        let running = record(json!({}), &["place", "sub"]);
        assert!(same_backend(&area.backend_for(&running).unwrap(), &own));
        let on_boat = record(json!({}), &[]);
        assert!(same_backend(&area.backend_for(&on_boat).unwrap(), &boat));
        let freed = record(json!({ "phase": "released" }), &["place", "sub"]);
        assert!(same_backend(&area.backend_for(&freed).unwrap(), &released));
        let mut gone = sub_record("ws_r", "running");
        gone["sub"]["cloudGone"] = json!(true);
        let gone: WorkspaceRecord = serde_json::from_value(gone).unwrap();
        assert!(same_backend(&area.backend_for(&gone).unwrap(), &released));

        assert!(same_backend(
            &area.backend_of_kind(WorkspaceKind::Cloud, None).unwrap(),
            &own
        ));
        assert!(same_backend(
            &area
                .backend_of_kind(WorkspaceKind::Cloud, Some("box"))
                .unwrap(),
            &boat
        ));

        // The facts read off a backend, not off the kind.
        let plain = |images: bool, disk: bool, names: bool| PlainBackend {
            images,
            disk,
            names,
        };
        assert_eq!(
            (
                area.keeps_images(&*own),
                area.pause_keeps_disk(&*own),
                area.names_workspace(&*own)
            ),
            (true, true, false)
        );
        assert_eq!(
            (
                area.keeps_images(&plain(false, false, true)),
                area.pause_keeps_disk(&plain(false, false, true)),
                area.names_workspace(&plain(false, false, true))
            ),
            (false, false, true)
        );
        assert_eq!(
            (
                area.keeps_images(&plain(true, false, false)),
                area.pause_keeps_disk(&plain(false, true, false))
            ),
            (true, true)
        );

        // A released machine declares no lifecycle; a live one reads its backend's.
        let live = host.put_live(
            sub_record("ws_live", "running"),
            FakeMachine::new("bx_live"),
        );
        assert!(area.lifecycle_of(&live.borrow()).is_ok());
        let dead = host.put_live(
            record_of("ws_dead", json!({ "phase": "released" }), &["place", "sub"]),
            FakeMachine::new("bx_dead"),
        );
        assert_eq!(
            area.lifecycle_of(&dead.borrow()).err().unwrap().message,
            "cloud machines declare no lifecycle"
        );

        // The options a kind's exec stream is made with: the host's wired ones under the caller's keys.
        let reads = |o: Option<MachineExecOptions>| o.map(|o| (o.poll_ms, o.idle_ms));
        let o = MachineExecOptions {
            poll_ms: Some(1),
            ..MachineExecOptions::default()
        };
        assert_eq!(
            reads(area.exec_options_for(Some(o.clone()))),
            Some((Some(1), None))
        );
        assert_eq!(reads(area.exec_options_for(None)), None);
        world(|w| {
            w.machine_exec = Some(MachineExecOptions {
                poll_ms: Some(5),
                idle_ms: Some(9.0),
                ..MachineExecOptions::default()
            })
        });
        assert_eq!(
            reads(area.exec_options_for(Some(o))),
            Some((Some(1), Some(9.0)))
        );
        assert_eq!(
            reads(area.exec_options_for(None)),
            Some((Some(5), Some(9.0)))
        );
    });
}

#[test]
fn the_broker_read_waits_five_seconds() {
    assert_eq!(
        wsp_machines::machines::kinds::roads::BROKER_READ_MS,
        5_000.0
    );
}

#[test]
fn memory_key_follows_the_catalog_row() {
    run(async {
        let (_host, area) = kinds();
        let p = project("pr_1", "/work/lab", json!({ "memoryKey": "-recorded-key" }));
        // An agent whose row names the variable that pins a project's key takes the key recorded with the project.
        assert_eq!(
            area.project_memory_key("claude", &p),
            Some("-recorded-key".to_string())
        );
        // Every other agent keys off the folder it is worked at.
        let codex = area.project_memory_key("codex", &p);
        assert_eq!(
            codex,
            wsp_engine::project_state::project_state_key("codex", "/work/lab")
        );
        assert_ne!(codex, Some("-recorded-key".to_string()));
        assert_eq!(
            area.project_memory_key("nope", &p),
            wsp_engine::project_state::project_state_key("nope", "/work/lab")
        );
    });
}
