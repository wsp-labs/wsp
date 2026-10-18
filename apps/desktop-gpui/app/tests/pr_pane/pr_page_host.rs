// SPDX-License-Identifier: AGPL-3.0-only
//! A host for the Pull request pane's tests: a throwaway host carrying a wsp-daemon, a project whose folder is a git
//! repository with a GitHub remote on a branch of its own, and the stub gh of `pr_pane/gh/` first on the host's PATH, so
//! every read and write of the pull request answers from a fixture and none reaches GitHub.

use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, Mutex};
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
    /// A host whose project `repo` stands on branch `work` off `main`, its pull request read from `scenario`; none,
    /// with a line saying so, where no wsp-daemon is staged.
    pub fn start(scenario: &str) -> Option<PrHost> {
        let server = Throwaway::start_with(stand_in::STAND_IN, true);
        if server.daemon.is_none() {
            println!("skipped: the host runs gh through this computer's wsp-daemon: {}", super::common::wsp_daemon().unwrap_err());
            return None;
        }
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
        Some(PrHost {
            workspace_id: row.workspace_id.clone(),
            thread_id: row.thread_id.clone().expect("a thread id"),
            server,
            client: Arc::new(client),
            runtime,
            folder,
        })
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

    /// One computer as the host lists it.
    pub fn place(&self, id: &str) -> Value {
        let client = self.client.clone();
        let ask: protocol::PlacesListRequest = serde_json::from_value(json!({})).unwrap();
        let listed = self.runtime.block_on(async move { client.call(ask).await }).unwrap_or_else(|f| panic!("{}", f.message));
        let places = serde_json::to_value(listed.places).unwrap();
        places.as_array().unwrap().iter().find(|p| p["id"] == id).cloned().unwrap_or(Value::Null)
    }

    /// Sets a computer's thread cap back to its default, which the host pushes as `place.changed`.
    pub fn touch_place(&self, id: &str) {
        let client = self.client.clone();
        let ask: protocol::PlacesSetRequest = serde_json::from_value(json!({ "placeId": id, "reset": ["threads"] })).unwrap();
        self.runtime.block_on(async move { client.call(ask).await }).unwrap_or_else(|f| panic!("{}", f.message));
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

/// A door in front of the host that writes down the op of every request a client sends through it, read off the
/// client's WebSocket frames; a client dials it through `state`, whose lock names the door's port.
pub struct Door {
    pub state: PathBuf,
    ops: Arc<Mutex<Vec<String>>>,
    _dir: tempfile::TempDir,
}

impl Door {
    pub fn open(server: &Throwaway) -> Door {
        let dir = tempfile::tempdir().unwrap();
        let served = server.state();
        let mut lock: Value = serde_json::from_str(&std::fs::read_to_string(served.with_file_name("host.lock")).unwrap()).unwrap();
        let upstream = format!("127.0.0.1:{}", lock["port"]);
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        lock["port"] = json!(listener.local_addr().unwrap().port());
        std::fs::write(dir.path().join("host.lock"), lock.to_string()).unwrap();
        std::fs::copy(served.with_file_name("host-token"), dir.path().join("host-token")).unwrap();
        let ops = Arc::new(Mutex::new(Vec::new()));
        let heard = ops.clone();
        std::thread::spawn(move || {
            for client in listener.incoming() {
                let (Ok(mut client), Ok(mut upstream)) = (client, std::net::TcpStream::connect(&upstream)) else { return };
                let (mut back, mut to) = (upstream.try_clone().unwrap(), client.try_clone().unwrap());
                std::thread::spawn(move || _ = std::io::copy(&mut back, &mut to));
                let heard = heard.clone();
                std::thread::spawn(move || {
                    let (mut seen, mut upgraded, mut chunk) = (Vec::new(), false, vec![0u8; 1 << 16]);
                    while let Ok(n @ 1..) = client.read(&mut chunk) {
                        if upstream.write_all(&chunk[..n]).is_err() {
                            return;
                        }
                        seen.extend_from_slice(&chunk[..n]);
                        if !upgraded {
                            let Some(end) = seen.windows(4).position(|w| w == b"\r\n\r\n") else { continue };
                            seen.drain(..end + 4);
                            upgraded = true;
                        }
                        while let Some((payload, used)) = client_frame(&seen) {
                            let op = serde_json::from_slice::<Value>(&payload).ok().and_then(|v| v["op"].as_str().map(str::to_owned));
                            heard.lock().unwrap().extend(op);
                            seen.drain(..used);
                        }
                    }
                });
            }
        });
        Door { state: dir.path().join("state.json"), ops, _dir: dir }
    }

    /// How many requests for `op` went through the door.
    pub fn sent(&self, op: &str) -> usize {
        self.ops.lock().unwrap().iter().filter(|o| *o == op).count()
    }
}

/// The first whole frame of a client's WebSocket bytes: its payload unmasked and how many bytes it took.
fn client_frame(bytes: &[u8]) -> Option<(Vec<u8>, usize)> {
    let second = *bytes.get(1)?;
    let (len, mut at) = match second & 0x7f {
        126 => (u16::from_be_bytes(bytes.get(2..4)?.try_into().ok()?) as usize, 4),
        127 => (u64::from_be_bytes(bytes.get(2..10)?.try_into().ok()?) as usize, 10),
        n => (n as usize, 2),
    };
    let mut mask = [0u8; 4];
    if second & 0x80 != 0 {
        mask = bytes.get(at..at + 4)?.try_into().ok()?;
        at += 4;
    }
    let payload = bytes.get(at..at + len)?;
    Some((payload.iter().enumerate().map(|(i, b)| b ^ mask[i % 4]).collect(), at + len))
}
