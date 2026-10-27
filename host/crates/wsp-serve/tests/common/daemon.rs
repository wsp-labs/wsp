// SPDX-License-Identifier: AGPL-3.0-only
//! The fake daemon channels of the TS fixtures: `branchDaemons` (stub-backend.ts:28) and `fakeDaemon`
//! (fork.test.ts:111). Each answers the frames it is given; none dials anything.
use std::cell::RefCell;
use std::path::PathBuf;
use std::process::Command;
use std::rc::Rc;

use async_trait::async_trait;
use futures::FutureExt as _;
use serde_json::{json, Map, Value};
use wsp_core::daemon_channel::{ChannelEnd, ChannelError, DaemonChannel, DaemonChannelOptions, Frame};
use wsp_core::daemon_token::daemon_token_for;
use wsp_core::types::wiring::Local;

pub type Open = Rc<dyn Fn(DaemonChannelOptions) -> Local<'static, Rc<dyn DaemonChannel>>>;

/// A frame kept with the machine it went to, read off the dial's token.
pub type Kept = Map<String, Value>;

struct Channel {
    machine: String,
    frames: Rc<RefCell<Vec<Kept>>>,
    answer: Rc<dyn Fn(&str, &Frame) -> Frame>,
}

#[async_trait(?Send)]
impl DaemonChannel for Channel {
    async fn send(&self, frame: Frame) -> Result<Frame, ChannelError> {
        let mut kept = frame.clone();
        kept.insert("machine".into(), json!(self.machine));
        self.frames.borrow_mut().push(kept);
        let mut reply = Map::new();
        reply.insert("id".into(), json!(1));
        reply.extend((self.answer)(&self.machine, &frame));
        Ok(reply)
    }
    fn close(&self) {}
    async fn closed(&self) -> ChannelEnd {
        futures::future::pending().await
    }
}

fn opener(
    machine_of: Rc<dyn Fn(&DaemonChannelOptions) -> String>,
    frames: Rc<RefCell<Vec<Kept>>>,
    answer: Rc<dyn Fn(&str, &Frame) -> Frame>,
) -> Open {
    Rc::new(move |dial: DaemonChannelOptions| {
        let channel = Channel { machine: machine_of(&dial), frames: frames.clone(), answer: answer.clone() };
        async move { Ok(Rc::new(channel) as Rc<dyn DaemonChannel>) }.boxed_local()
    })
}

fn frame_of(doc: Value) -> Frame {
    match doc {
        Value::Object(map) => map,
        other => panic!("not an object: {other}"),
    }
}

fn text(frame: &Frame, key: &str) -> String {
    match frame.get(key) {
        Some(Value::String(s)) => s.clone(),
        Some(other) => other.to_string(),
        None => "undefined".into(),
    }
}

/// stub-backend.ts:28 `branchDaemons` options.
#[derive(Clone, Default)]
pub struct BranchOpts {
    pub seed: Option<String>,
    pub branch_of: Option<Rc<dyn Fn(&str) -> String>>,
    pub ahead_of: Option<Rc<dyn Fn(&str) -> u32>>,
    pub default_of: Option<Rc<dyn Fn(&str) -> Option<String>>>,
}

/// Daemons for every machine and for this computer: a lead's checkout reads as on `GUEST_BRANCH` (or the branch
/// `branch_of` names), tracked and level with its upstream unless `ahead_of` counts commits over it, and a child's
/// copy is put on whatever branch it is asked. Every frame is kept with the machine it went to.
pub fn branch_daemons(o: BranchOpts) -> (Open, Rc<RefCell<Vec<Kept>>>) {
    let seed = o.seed.clone().unwrap_or_else(|| "cafef00d".repeat(3));
    let frames: Rc<RefCell<Vec<Kept>>> = Rc::default();
    let machine_of: Rc<dyn Fn(&DaemonChannelOptions) -> String> = Rc::new(move |dial| {
        (1..40)
            .map(|n| format!("m{n}"))
            .find(|m| daemon_token_for(&seed, m).is_ok_and(|t| t == dial.token))
            .unwrap_or_else(|| "here".into())
    });
    let answer: Rc<dyn Fn(&str, &Frame) -> Frame> = Rc::new(move |machine, frame| {
        let branch = || o.branch_of.as_ref().map_or_else(|| super::stub_backend::GUEST_BRANCH.to_string(), |f| f(machine));
        match text(frame, "op").as_str() {
            "git.status" => {
                let head = branch();
                let named = o.default_of.as_ref().and_then(|f| f(machine));
                let mut doc = json!({
                    "ok": true,
                    "branch": { "oid": "abc", "head": head, "upstream": format!("origin/{head}"), "ahead": o.ahead_of.as_ref().map_or(0, |f| f(machine)), "behind": 0 },
                    "entries": [], "root": text(frame, "cwd"),
                });
                if let Some(named) = named {
                    doc["defaultBranch"] = json!(named);
                }
                frame_of(doc)
            }
            "git.push" => {
                let head = branch();
                if o.default_of.as_ref().and_then(|f| f(machine)).as_deref() == Some(head.as_str()) {
                    return frame_of(
                        json!({ "ok": false, "code": "on-default-branch", "error": format!("this workspace is on {head}, the project's default branch, and no push moves it") }),
                    );
                }
                let base = frame.get("base").map(|_| text(frame, "base")).unwrap_or_default();
                frame_of(
                    json!({ "ok": true, "branch": head, "base": base, "remote": "origin", "ahead": o.ahead_of.as_ref().map_or(0, |f| f(machine)), "uncommitted": 0, "stat": [] }),
                )
            }
            "git.startOn" => frame_of(json!({ "ok": true, "branch": text(frame, "branch"), "oid": "c0ffee" })),
            other => frame_of(json!({ "ok": false, "error": format!("{other} was not answered") })),
        }
    });
    (opener(machine_of, frames.clone(), answer), frames)
}

/// fork.test.ts:111 `fakeDaemon`: answers each checkpoint with a ref of the turn it names, or makes it for real in `repo`.
pub fn fake_daemon(repo: Option<PathBuf>) -> (Open, Rc<RefCell<Vec<Kept>>>) {
    let frames: Rc<RefCell<Vec<Kept>>> = Rc::default();
    let answer: Rc<dyn Fn(&str, &Frame) -> Frame> = Rc::new(move |_, f| match text(f, "op").as_str() {
        "git.checkpoint" => {
            let r = format!("refs/wsp/checkpoints/{}/{}/{}", text(f, "scope"), text(f, "thread"), text(f, "turn"));
            if let Some(repo) = &repo {
                checkpoint_in(repo, &r);
            }
            frame_of(json!({ "ok": true, "ref": r, "commit": "c", "changed": true }))
        }
        "git.checkpointDrop" => frame_of(json!({ "ok": true, "dropped": 0 })),
        other => frame_of(json!({ "ok": false, "error": format!("no {other} here") })),
    });
    (opener(Rc::new(|_| "here".into()), frames.clone(), answer), frames)
}

const GIT_ENV: [(&str, &str); 4] = [
    ("GIT_AUTHOR_NAME", "t"),
    ("GIT_AUTHOR_EMAIL", "t@example.com"),
    ("GIT_COMMITTER_NAME", "t"),
    ("GIT_COMMITTER_EMAIL", "t@example.com"),
];

/// `git -C cwd args`, with the identity every commit here carries; the trimmed stdout.
pub fn git(cwd: &std::path::Path, args: &[&str]) -> String {
    let out = Command::new("git").arg("-C").arg(cwd).args(args).envs(GIT_ENV).output().expect("git runs");
    assert!(out.status.success(), "git {args:?} failed: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// The folder's whole tree under the ref, its HEAD as the commit's parent, as the daemon's checkpoint takes it.
fn checkpoint_in(repo: &std::path::Path, r: &str) {
    let index = repo.join(".git").join("fork-test.index");
    let run = |args: &[&str]| {
        let out =
            Command::new("git").arg("-C").arg(repo).args(args).envs(GIT_ENV).env("GIT_INDEX_FILE", &index).output().expect("git runs");
        assert!(out.status.success(), "git {args:?} failed: {}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    };
    run(&["add", "-A"]);
    let tree = run(&["write-tree"]);
    let _ = std::fs::remove_file(&index);
    let commit = git(repo, &["commit-tree", &tree, "-p", "HEAD", "-m", "wsp checkpoint"]);
    git(repo, &["update-ref", r, &commit]);
}
