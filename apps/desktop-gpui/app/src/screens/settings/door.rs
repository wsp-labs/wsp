// SPDX-License-Identifier: AGPL-3.0-only
//! A host that answers only what a test's rules say (`tests/settings/door.mjs`), for a test that needs a refusal, a
//! slow answer or a push on cue, and reads back every frame the app sent it. Needs node and the packages installed.

use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::Value;
use wsp_host_client::{Env, HostClient};

pub struct Door {
    dir: PathBuf,
    child: Child,
}

impl Door {
    pub fn start(rules: Value) -> Door {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!("wsp-door-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/settings/door.mjs");
        let door = Door { child: Command::new("node").arg(script).arg(&dir).env_clear().envs(env_in(&dir)).spawn().unwrap(), dir };
        door.rules(rules);
        let until = Instant::now() + Duration::from_secs(10);
        while !door.dir.join("host.lock").exists() {
            assert!(Instant::now() < until, "the door never listened");
            std::thread::sleep(Duration::from_millis(20));
        }
        door
    }

    /// What it answers from the next frame on.
    pub fn rules(&self, rules: Value) {
        std::fs::write(self.dir.join("rules.json"), rules.to_string()).unwrap();
    }

    pub fn state(&self) -> PathBuf {
        self.dir.join("state.json")
    }

    pub fn env(&self) -> Env {
        env_in(&self.dir)
    }

    /// Every frame it was sent past the token, in order.
    pub fn asked(&self) -> Vec<Value> {
        let text = std::fs::read_to_string(self.dir.join("asked.jsonl")).unwrap_or_default();
        text.lines().filter_map(|l| serde_json::from_str(l).ok()).collect()
    }

    /// The frames of one op.
    pub fn asked_of(&self, op: &str) -> Vec<Value> {
        self.asked().into_iter().filter(|f| f["op"] == op).collect()
    }

    /// A client dialled on `runtime`, as Settings dials.
    pub fn client(&self, runtime: &tokio::runtime::Runtime) -> Arc<HostClient> {
        let (state, env) = (self.state(), self.env());
        Arc::new(runtime.block_on(HostClient::connect(&state, &env)).unwrap_or_else(|f| panic!("{}", f.message)))
    }
}

impl Drop for Door {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// The folder as home and PATH, nothing else of this machine's.
fn env_in(dir: &Path) -> Env {
    let path = std::env::var("PATH").unwrap_or_default();
    [("HOME", dir.to_string_lossy().into_owned()), ("PATH", path)].map(|(k, v)| (k.to_owned(), v)).into_iter().collect()
}

pub fn runtime() -> Arc<tokio::runtime::Runtime> {
    Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap())
}
