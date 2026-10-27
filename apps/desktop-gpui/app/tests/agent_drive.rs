// SPDX-License-Identifier: AGPL-3.0-only
//! The app driven through gpui-agent alone, as an agent drives it: a build with the `agent` feature, launched under the
//! tool's own Xvfb against a throwaway host holding two threads, opens a thread when its sidebar row is clicked, takes a
//! message typed into the composer, sends it from the send button and shows it in the thread, and an annotated
//! screenshot of that is kept under the target's tmp folder. Needs `gpui-agent` on PATH (scripts/box-warm.sh) and Xvfb.

#[path = "../../host-client/tests/common/mod.rs"]
mod common;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use serde_json::Value;

const MESSAGE: &str = "sent through gpui-agent";

/// One gpui-agent session, in a folder of the test's own, closed with this.
struct Agent {
    bin: PathBuf,
    host: common::Throwaway,
}

impl Agent {
    fn run(&self, args: &[&str]) -> Result<String, String> {
        let out = Command::new(&self.bin)
            .args(args)
            .env_clear()
            .envs(self.host.env())
            .env("GPUI_AGENT_HOME", self.host.dir.path().join("gpui-agent"))
            .current_dir(self.host.dir.path())
            .output()
            .unwrap();
        let said = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
        if out.status.success() {
            Ok(said)
        } else {
            Err(said)
        }
    }

    fn must(&self, args: &[&str]) -> String {
        self.run(args).unwrap_or_else(|said| panic!("gpui-agent {args:?}: {said}\nhost log: {}", self.host.log()))
    }

    /// What takes input on screen, as `snapshot -i` lists it.
    fn snapshot(&self) -> Vec<Value> {
        let said = self.must(&["snapshot", "-i", "--json"]);
        let snapshot: Value = serde_json::from_str(&said).unwrap_or_else(|e| panic!("snapshot --json: {e}: {said}"));
        snapshot["entries"].as_array().cloned().unwrap_or_default()
    }

    fn probe(&self, name: &str) -> Value {
        serde_json::from_str(self.must(&["get", "probe", name]).trim()).unwrap_or(Value::Null)
    }

    fn until(&self, what: &str, done: impl Fn(&Self) -> bool) {
        let end = Instant::now() + Duration::from_secs(25);
        while !done(self) {
            assert!(Instant::now() < end, "timed out waiting for {what}");
            std::thread::sleep(Duration::from_millis(200));
        }
    }
}

impl Drop for Agent {
    fn drop(&mut self) {
        let _ = self.run(&["close"]);
    }
}

fn on_path(name: &str) -> PathBuf {
    let path = std::env::var_os("PATH").unwrap_or_default();
    std::env::split_paths(&path)
        .map(|dir| dir.join(name))
        .find(|bin| bin.is_file())
        .unwrap_or_else(|| panic!("no {name} on PATH: scripts/box-warm.sh installs it"))
}

/// The ref of the one entry `pick` takes, named for the failure.
fn find(entries: &[Value], what: &str, pick: impl Fn(&Value) -> bool) -> String {
    let found: Vec<&Value> = entries.iter().filter(|e| pick(e)).collect();
    assert_eq!(found.len(), 1, "one {what} on screen, among {}", serde_json::to_string_pretty(entries).unwrap());
    format!("@{}", found[0]["ref"].as_str().unwrap())
}

fn text(entry: &Value, field: &str) -> String {
    entry[field].as_str().unwrap_or_default().to_owned()
}

#[test]
fn an_agent_opens_a_thread_by_its_row_and_sends_from_its_composer() {
    let host = common::Throwaway::start();
    let folder = host.repo("repo");
    host.wsp(&["add", &folder.to_string_lossy()]);
    host.wsp(&["run", "repo", "first", "--agent", "claude"]);
    host.wsp(&["run", "repo", "second", "--agent", "claude"]);
    let state = host.state().to_string_lossy().into_owned();
    let agent = Agent { bin: on_path("gpui-agent"), host };

    agent.must(&["launch", "--headless", "--", env!("CARGO_BIN_EXE_desktop-gpui"), "--state", &state]);
    agent.must(&["wait", "--text", "second"]);
    let row = find(&agent.snapshot(), "sidebar row of the thread first", |e| {
        text(e, "role") == "button" && text(e, "name").contains("first") && text(e, "source").contains("sidebar")
    });
    agent.must(&["click", &row]);
    agent.until("the thread first on screen", |a| a.probe("thread")["thread"].is_string());

    agent.must(&["wait", "--idle"]);
    let entries = agent.snapshot();
    let field = find(&entries, "composer field", |e| text(e, "role") == "textbox" && text(e, "name").starts_with("Ask anything"));
    agent.must(&["fill", &field, MESSAGE]);
    assert_eq!(agent.probe("composer"), MESSAGE, "the composer holds what was typed");
    let send = find(&agent.snapshot(), "send button", |e| text(e, "role") == "button" && text(e, "name") == "Send message");
    agent.must(&["click", &send]);
    agent.until("the composer to empty as it sends", |a| a.probe("composer") == "");
    agent.must(&["wait", "--text", MESSAGE]);

    let shot = Path::new(env!("CARGO_TARGET_TMPDIR")).join("agent_drive.png");
    println!("{}", agent.must(&["screenshot", "--annotate", &shot.to_string_lossy()]));
    assert!(shot.is_file(), "the annotated screenshot at {}", shot.display());
}
