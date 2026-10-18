// SPDX-License-Identifier: AGPL-3.0-only
//! A host for the Pull request pane's tests: a throwaway host carrying a wsp-daemon, a project whose folder is a git
//! repository with a GitHub remote on a branch of its own, and the stub gh of `pr_pane/gh/` first on the host's PATH, so
//! every read and write of the pull request answers from a fixture and none reaches GitHub.

use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tokio::runtime::Runtime;
use wsp_host_client::{protocol, HostClient};

use super::common::Throwaway;

#[path = "stand_in.rs"]
mod stand_in;

pub const WAIT: Duration = Duration::from_secs(30);

pub struct PrHost {
    pub server: Throwaway,
    pub client: Arc<HostClient>,
    pub runtime: Arc<Runtime>,
    pub workspace_id: String,
    pub thread_id: String,
    pub folder: PathBuf,
}

/// The stub's folder in the test's HOME: its scenario, its state and the calls it was given.
fn stub_box(server: &Throwaway) -> PathBuf {
    server.dir.path().join("gh-stub")
}

impl PrHost {
    /// A host whose project `repo` stands on branch `work` off `main`, its pull request read from `scenario`.
    pub fn start(scenario: &str) -> PrHost {
        let server = Throwaway::start_with(stand_in::STAND_IN, true);
        assert!(server.daemon.is_some(), "the pane's tests need a wsp-daemon: {:?}", super::common::wsp_daemon());
        let stub = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/pr_pane/gh/gh");
        std::os::unix::fs::symlink(&stub, server.dir.path().join("bin/gh")).unwrap();
        std::fs::create_dir_all(stub_box(&server)).unwrap();
        std::fs::write(stub_box(&server).join("scenario"), scenario).unwrap();
        // The host's own git fetches the remote on some roads; with http refused it never dials GitHub.
        let gitconfig = server.dir.path().join(".gitconfig");
        let mut held = std::fs::read_to_string(&gitconfig).unwrap_or_default();
        held.push_str("[protocol \"https\"]\n\tallow = never\n[protocol \"http\"]\n\tallow = never\n");
        std::fs::write(&gitconfig, held).unwrap();
        let folder = server.repo_with("repo", &[("src/pane.rs", "fn main() {}\n")]);
        let git = |args: &[&str]| {
            let ok = Command::new("git").args(args).current_dir(&folder).env_clear().envs(server.env()).status().unwrap().success();
            assert!(ok, "git {args:?}");
        };
        git(&["branch", "-M", "main"]);
        git(&["remote", "add", "origin", "https://github.com/wsp-test/repo.git"]);
        git(&["checkout", "-q", "-b", "work"]);
        std::fs::write(folder.join("src/pane.rs"), "fn main() {\n    println!(\"pane\");\n}\n").unwrap();
        git(&["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "-am", "Teach the pane to read checks"]);
        server.wsp(&["add", &folder.to_string_lossy()]);
        server.wsp(&["run", "repo", "first", "--agent", "claude"]);
        let runtime = Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap());
        let client = runtime.block_on(server.client());
        let threads = runtime.block_on(client.threads()).unwrap_or_else(|f| panic!("{}", f.message));
        let row = threads.iter().find(|t| t.prompt.as_deref() == Some("first")).expect("the thread wsp run opened");
        PrHost {
            workspace_id: row.workspace_id.clone(),
            thread_id: row.thread_id.clone().expect("a thread id"),
            server,
            client: Arc::new(client),
            runtime,
            folder,
        }
    }

    /// The host as a screen holds it.
    pub fn host(&self) -> desktop_gpui::screens::Host {
        desktop_gpui::screens::Host::new(self.client.clone(), self.runtime.clone())
    }

    /// Every gh line the stub was given, oldest first.
    pub fn calls(&self) -> Vec<Value> {
        let text = std::fs::read_to_string(stub_box(&self.server).join("calls.jsonl")).unwrap_or_default();
        text.lines().filter_map(|l| serde_json::from_str(l).ok()).collect()
    }

    /// The gh lines given whose argv holds every word of `words`.
    pub fn calls_with(&self, words: &[&str]) -> Vec<Value> {
        self.calls()
            .into_iter()
            .filter(|c| words.iter().all(|w| c["argv"].as_array().is_some_and(|a| a.iter().any(|x| x.as_str() == Some(w)))))
            .collect()
    }

    /// Makes the stub refuse the writes named, one word each: `merge`, `reply`, `resolve`, `react`.
    pub fn refuse(&self, words: &str) {
        std::fs::write(stub_box(&self.server).join("refuse"), words).unwrap();
    }

    pub fn scenario(&self, scenario: &str) {
        std::fs::write(stub_box(&self.server).join("scenario"), scenario).unwrap();
    }

    /// The workspace's status as the host holds it now.
    pub fn status(&self) -> Value {
        let client = self.client.clone();
        let ask: protocol::StatusSubscribeRequest = serde_json::from_value(json!({})).unwrap();
        let statuses = self.runtime.block_on(async move { client.call(ask).await }).unwrap_or_else(|f| panic!("{}", f.message));
        let all = serde_json::to_value(statuses.statuses).unwrap();
        all.as_array().unwrap().iter().find(|s| s["id"] == self.workspace_id.as_str()).cloned().unwrap_or(Value::Null)
    }

    /// Asks the host to read the branch, which reads its pull request, until the status names one, or `WAIT` passes.
    pub fn pull_request(&self) -> Value {
        let until = Instant::now() + WAIT;
        loop {
            let client = self.client.clone();
            let ask: protocol::WorkspacesCheckoutRequest = serde_json::from_value(json!({ "workspaceId": self.workspace_id })).unwrap();
            let _ = self.runtime.block_on(async move { client.call(ask).await });
            let pr = self.status()["pr"].clone();
            if !pr.is_null() || Instant::now() > until {
                return pr;
            }
            std::thread::sleep(Duration::from_millis(300));
        }
    }
}
