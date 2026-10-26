// SPDX-License-Identifier: AGPL-3.0-only
//! What the core-rest tests share: the TS oracle's answers (oracle/core_rest.oracle.ts writes
//! tests/fixtures/core_rest/<unit>.json), a machine that answers each command by needle and records it, the
//! owner-thread runner, and real git and sh runners with no identity and no config of the person's.
#![allow(dead_code)]

use std::cell::RefCell;
use std::future::Future;
use std::path::Path;
use std::process::Command;
use std::rc::Rc;

use async_trait::async_trait;
use serde::de::DeserializeOwned;
use serde_json::Value;
use wsp_engine::errors::{EngineError, Result};
use wsp_engine::machine::{ExecOptions, ExecResult, MachineKind, MachineLife, MachineOffers, MachineState, RunOptions, SnapshotOptions};
use wsp_engine::Machine;
use wsp_wire::types::js_number::JsInt;

/// One unit's TS answers.
pub fn fixture(name: &str) -> Value {
    let path = format!("{}/tests/fixtures/core_rest/{name}.json", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{path}: {e}"))
}

/// A wire value read off JSON.
pub fn wire<T: DeserializeOwned>(v: &Value) -> T {
    serde_json::from_value(v.clone()).unwrap_or_else(|e| panic!("{v} does not read: {e}"))
}

/// A value as JSON.
pub fn json<T: serde::Serialize>(v: &T) -> Value {
    serde_json::to_value(v).expect("serializes")
}

/// Runs a test body on one thread with a `LocalSet`, as the host runs.
pub fn run_local<F: Future<Output = ()>>(body: F) {
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    tokio::task::LocalSet::new().block_on(&rt, body);
}

/// Lets every spawned task that can run, run.
pub async fn settle() {
    for _ in 0..50 {
        tokio::task::yield_now().await;
    }
}

pub fn result(exit_code: i64, stdout: &str, stderr: &str) -> ExecResult {
    ExecResult { exit_code: JsInt::from(exit_code), stdout: stdout.into(), stderr: stderr.into() }
}

/// One command a `RecMachine` was asked to run, as the oracle's machine records it.
#[derive(Clone, Debug, PartialEq)]
pub struct Ran {
    pub cmd: String,
    pub timeout_ms: Option<u64>,
    pub stdin: Option<String>,
}

impl Ran {
    /// The oracle's `{cmd, timeoutMs?, stdin?}`.
    pub fn json(&self) -> Value {
        let mut o = serde_json::Map::new();
        o.insert("cmd".into(), Value::String(self.cmd.clone()));
        if let Some(t) = self.timeout_ms {
            o.insert("timeoutMs".into(), Value::from(t));
        }
        if let Some(s) = &self.stdin {
            o.insert("stdin".into(), Value::String(s.clone()));
        }
        Value::Object(o)
    }
}

/// A machine that answers each command by the first rule whose needle the command holds (exit 0 and nothing said
/// otherwise), fails any command holding `fail_on` with an engine error, and records every command. The oracle's
/// `machine(rules, fail)`.
pub struct RecMachine {
    pub id: String,
    pub rules: Vec<(String, ExecResult)>,
    pub fail_on: Option<String>,
    pub ran: RefCell<Vec<Ran>>,
    pub offers: MachineOffers,
    pub daemon_token_path: Option<String>,
}

impl RecMachine {
    pub fn new(rules: Vec<(&str, ExecResult)>) -> Rc<Self> {
        Rc::new(Self::plain(rules))
    }

    pub fn plain(rules: Vec<(&str, ExecResult)>) -> Self {
        RecMachine {
            id: "p".into(),
            rules: rules.into_iter().map(|(n, r)| (n.to_string(), r)).collect(),
            fail_on: None,
            ran: RefCell::new(vec![]),
            offers: MachineOffers::default(),
            daemon_token_path: None,
        }
    }

    pub fn failing(mut self, needle: &str) -> Self {
        self.fail_on = Some(needle.into());
        self
    }

    pub fn ran(&self) -> Vec<Ran> {
        self.ran.borrow().clone()
    }

    pub fn cmds(&self) -> Vec<String> {
        self.ran.borrow().iter().map(|r| r.cmd.clone()).collect()
    }

    pub fn ran_json(&self) -> Value {
        Value::Array(self.ran.borrow().iter().map(Ran::json).collect())
    }
}

#[async_trait(?Send)]
impl Machine for RecMachine {
    fn id(&self) -> &str {
        &self.id
    }
    fn kind(&self) -> MachineKind {
        MachineKind::Sandbox
    }
    fn takes_stdin(&self) -> bool {
        true
    }
    fn daemon_token_path(&self) -> Option<&str> {
        self.daemon_token_path.as_deref()
    }
    fn offers(&self) -> MachineOffers {
        self.offers
    }
    async fn exec(&self, cmd: &str, opts: ExecOptions) -> Result<ExecResult> {
        self.ran.borrow_mut().push(Ran {
            cmd: cmd.into(),
            timeout_ms: opts.timeout_ms,
            stdin: opts.stdin.map(|b| String::from_utf8(b).unwrap()),
        });
        if let Some(f) = &self.fail_on {
            if cmd.contains(f.as_str()) {
                return Err(EngineError::Other { message: "the link went".into(), code: None });
            }
        }
        Ok(self.rules.iter().find(|(n, _)| cmd.contains(n.as_str())).map(|(_, r)| r.clone()).unwrap_or_else(|| result(0, "", "")))
    }
    async fn run(&self, _script: &str, _opts: RunOptions) -> Result<ExecResult> {
        panic!("not on this road")
    }
    async fn snapshot(&self, _name: &str, _life: MachineLife, _opts: SnapshotOptions) -> Result<String> {
        panic!("not on this road")
    }
    async fn pause(&self, _wait_ms: Option<u64>) -> Result<()> {
        panic!("not on this road")
    }
    async fn resume(&self, _signal: Option<&wsp_engine::Cancel>) -> Result<()> {
        panic!("not on this road")
    }
    async fn kill(&self) -> Result<()> {
        panic!("not on this road")
    }
    async fn state(&self) -> Result<MachineState> {
        panic!("not on this road")
    }
    async fn download_url(&self, _path: &str) -> Result<String> {
        panic!("not on this road")
    }
    async fn upload_url(&self, _path: &str) -> Result<String> {
        panic!("not on this road")
    }
}

/// The environment a script gets on a computer the person joined: no git identity anywhere, no config of the
/// person's or the machine's, and `user.useConfigOnly` so git guesses none (project-seed-script.test.ts:34).
pub fn no_identity(root: &Path) -> Vec<(String, String)> {
    let home = root.join("empty-home");
    std::fs::create_dir_all(&home).unwrap();
    vec![
        ("PATH".into(), std::env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin".into())),
        ("HOME".into(), home.display().to_string()),
        ("GIT_CONFIG_GLOBAL".into(), "/dev/null".into()),
        ("GIT_CONFIG_NOSYSTEM".into(), "1".into()),
        ("GIT_CONFIG_COUNT".into(), "1".into()),
        ("GIT_CONFIG_KEY_0".into(), "user.useConfigOnly".into()),
        ("GIT_CONFIG_VALUE_0".into(), "true".into()),
    ]
}

/// One script under `sh -c` with exactly `env`: its exit code and what it said.
pub fn sh(script: &str, env: &[(String, String)]) -> ExecResult {
    let out = Command::new("sh").arg("-c").arg(script).env_clear().envs(env.iter().map(|(k, v)| (k, v))).output().expect("sh runs");
    result(out.status.code().unwrap_or(1) as i64, &String::from_utf8_lossy(&out.stdout), &String::from_utf8_lossy(&out.stderr))
}

/// git in `at` with a test identity, panicking on a non-zero exit; stdout as text.
pub fn git(at: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(at)
        .args(args)
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@t")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@t")
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .expect("git runs");
    assert!(out.status.success(), "git {args:?} in {}: {}", at.display(), String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).into_owned()
}
