// SPDX-License-Identifier: AGPL-3.0-only
//! A host started for the tests alone, on a state file of its own in a fresh folder, with nothing of this machine's
//! environment but PATH: the road every test of this crate, and the app's composer_live, takes to a host. The agent
//! is a stand-in claude first on the host's PATH that answers stream-json the way the claude adapter reads it, so no
//! real agent runs. Needs node and the packages built, the web app among them (pnpm build). The host runs with no
//! wsp-daemon unless the caller asks for one and `wsp_daemon` finds it.
#![allow(dead_code)]

use std::fmt::Debug;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use wsp_host_client::{protocol, Env, Failure, HostClient};

pub const STAND_IN: &str = r#"#!/bin/sh
case " $* " in
  *" --version "*) echo "2.1.295 (Claude Code)"; exit 0 ;;
  *" --help "*) echo '--permission-mode <mode>  (choices: "default", "bypassPermissions")'; exit 0 ;;
  *" --bare "*) read -r _; echo '{"type":"control_response","response":{"subtype":"success","request_id":"init","response":{"models":[{"value":"default","resolvedModel":"claude-haiku-5-5"}]}}}'; exit 0 ;;
  *" --input-format stream-json "*) ;;
  *) exit 1 ;;
esac
session=""; prev=""
for a in "$@"; do [ "$prev" = "--session-id" ] && session=$a; prev=$a; done
while IFS= read -r _; do
  printf '{"type":"system","subtype":"init","session_id":"%s","model":"claude-haiku-5-5","cwd":"/","tools":[]}\n' "$session"
  i=0
  while [ $i -lt 250 ]; do
    printf '{"type":"assistant","session_id":"%s","message":{"role":"assistant","content":[{"type":"text","text":"ok"}]}}\n' "$session"
    i=$((i + 1))
  done
  printf '{"type":"result","subtype":"success","is_error":false,"result":"ok","session_id":"%s"}\n' "$session"
done
"#;

pub fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..").canonicalize().unwrap()
}

/// This computer's folder under a daemon asset, as the host names it.
pub fn triple() -> String {
    let os = if cfg!(target_os = "macos") { "apple-darwin" } else { "unknown-linux-musl" };
    format!("{}-{os}", std::env::consts::ARCH)
}

/// The wsp-daemon a host may carry, which a checkout does not build: the binary WSP_TEST_DAEMON names, else the one
/// packages/wspx/scripts/daemon-binary.mjs placed, else the one wsp keeps on a cloud machine; otherwise why none.
pub fn wsp_daemon() -> Result<PathBuf, String> {
    let tried = match std::env::var_os("WSP_TEST_DAEMON").filter(|named| !named.is_empty()) {
        Some(named) => vec![PathBuf::from(named)],
        None => ["packages/wspx/daemon", "/root/wsp-daemon/wsp/assets/daemon"]
            .map(|folder| root().join(folder).join(triple()).join("wsp-daemon"))
            .to_vec(),
    };
    tried.iter().find(|binary| binary.is_file()).cloned().ok_or_else(|| {
        let tried: Vec<String> = tried.iter().map(|binary| binary.display().to_string()).collect();
        format!("no wsp-daemon at {} (WSP_TEST_DAEMON names one)", tried.join(" or "))
    })
}

/// A host on a fresh state file in a folder of its own, stopped with this.
pub struct Throwaway {
    pub dir: tempfile::TempDir,
    /// The wsp-daemon the host serves files and git through, where the caller asked for one and one was found.
    pub daemon: Option<PathBuf>,
    bin: PathBuf,
    /// The shell that runs the host and stops it once its stdin, held here, closes.
    host: Child,
}

/// What every process of the test runs in: the folder as its home, and its bin, holding the stand-ins and a link to
/// node, ahead of the system's own folders. Never the caller's PATH: the host runs every agent it finds on it, and a
/// real codex or opencode there reaches the network.
pub fn env_in(dir: &Path) -> Env {
    std::fs::create_dir_all(dir.join("bin")).unwrap();
    if let Err(e) = std::os::unix::fs::symlink(node(), dir.join("bin/node")) {
        assert_eq!(e.kind(), std::io::ErrorKind::AlreadyExists, "linking node into the test's bin: {e}");
    }
    let home = dir.to_string_lossy().into_owned();
    let path = format!("{home}/bin:/usr/bin:/bin:/usr/sbin:/sbin");
    [("HOME", home.as_str()), ("PATH", path.as_str()), ("ANTHROPIC_API_KEY", ""), ("SOLARI_API_KEY", ""), ("WSP_UPDATE_CHECK", "0")]
        .map(|(k, v)| (k.to_owned(), v.to_owned()))
        .into_iter()
        .collect()
}

/// Runs the command after it, ends with it, and stops it when its stdin reaches end of file or it is sent SIGTERM: a
/// test process that dies without unwinding closes the pipe, so its host stops with it. SIGTERM first, as the host
/// stops its daemon on that, then SIGKILL after 20 s. The shell waits on the host itself, so a host that exited on its
/// own is never signalled by a pid the shell has already reaped.
const STOPS_WITH_THE_TEST: &str = r#"exec 3<&0
"$@" 3<&- </dev/null & host=$!
stop() {
  kill -TERM "$host" 2>/dev/null
  i=0
  while [ $i -lt 200 ]; do sleep 0.1; i=$((i + 1)); done
  kill -KILL "$host" 2>/dev/null
}
{ cat >/dev/null; stop; } <&3 3<&- & watch=$!
trap 'kill "$watch"; stop 3<&- & watch=$!' TERM
wait "$host"
wait "$host" 2>/dev/null
kill "$watch" 2>/dev/null
"#;

/// node where the caller's PATH finds it, linked into each test's bin, since nothing else on that PATH reaches the host.
fn node() -> PathBuf {
    let path = std::env::var_os("PATH").unwrap_or_default();
    std::env::split_paths(&path).map(|dir| dir.join("node")).find(|node| node.is_file()).expect("node on PATH")
}

fn node_in(dir: &Path, bin: &Path, args: &[&str]) -> Command {
    let mut command = Command::new("node");
    command.arg(bin).args(args).env_clear().envs(env_in(dir)).current_dir(dir);
    command
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

/// A copy of the host's bundle under `dir` carrying `daemon`, or none, and its bin.js. A bundle reads each asset out of
/// the assets folder beside it before the checkout's, and finds the checkout's relative to itself, so the copy links
/// all of them there, and a copy with no daemon finds none where the checkout stages one.
fn bundle_with(dir: &Path, daemon: Option<&Path>) -> PathBuf {
    let root = root();
    let pkg = dir.join("hostpkg");
    copy_dir(&root.join("packages/host/dist"), &pkg.join("dist"));
    std::fs::copy(root.join("packages/host/package.json"), pkg.join("package.json")).unwrap();
    std::fs::create_dir_all(pkg.join("assets")).unwrap();
    let link = |from: &Path, to: &str| std::os::unix::fs::symlink(from, pkg.join(to)).unwrap();
    link(&root.join("packages/host/node_modules"), "node_modules");
    link(&root.join("apps/web/dist"), "assets/web");
    if root.join("packages/wspx/assets/cli").exists() {
        link(&root.join("packages/wspx/assets/cli"), "assets/cli");
    }
    if let Some(daemon) = daemon {
        std::fs::create_dir_all(pkg.join("assets/daemon").join(triple())).unwrap();
        link(daemon, &format!("assets/daemon/{}/wsp-daemon", triple()));
    }
    pkg.join("dist/bin.js")
}

/// `wsp up` on the state under `dir` under `STOPS_WITH_THE_TEST`, its output in up.log.
fn up(dir: &Path, bin: &Path) -> Child {
    let state = dir.join("state").join("state.json").to_string_lossy().into_owned();
    let log = std::fs::OpenOptions::new().create(true).append(true).open(dir.join("up.log")).unwrap();
    let mut host = Command::new("/bin/sh");
    host.args(["-c", STOPS_WITH_THE_TEST, "sh", "node"]).arg(bin).args(["up", "--port", "0", "--no-relay", "--state", &state]);
    host.env_clear().envs(env_in(dir)).current_dir(dir).stdin(Stdio::piped()).stdout(log.try_clone().unwrap()).stderr(log).spawn().unwrap()
}

/// Interrupts every turn the host serving `dir` runs, until none runs or 20 s pass. On a thread and a runtime of its
/// own, since the test's runtime may be the one dropping the host.
fn interrupt_turns(dir: &Path) {
    let (state, vars) = (dir.join("state/state.json"), env_in(dir));
    let _ = std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        runtime.block_on(async {
            let Ok(client) = HostClient::connect(&state, &vars).await else { return };
            let until = Instant::now() + Duration::from_secs(20);
            while Instant::now() < until {
                let list: protocol::SessionsListRequest = serde_json::from_value(json!({})).unwrap();
                let sessions = client.call(list).await.map(|listed| listed.sessions).unwrap_or_default();
                let running: Vec<String> =
                    sessions.into_iter().filter(|s| serde_json::to_value(s.status).ok() == Some(json!("running"))).map(|s| s.id).collect();
                if running.is_empty() {
                    return;
                }
                for id in running {
                    let interrupt: protocol::SessionsInterruptRequest = serde_json::from_value(json!({ "sessionId": id })).unwrap();
                    let _ = client.call(interrupt).await;
                }
                tokio::time::sleep(Duration::from_millis(300)).await;
            }
        });
    })
    .join();
}

impl Throwaway {
    pub fn state(&self) -> PathBuf {
        self.dir.path().join("state").join("state.json")
    }

    pub fn env(&self) -> Env {
        env_in(self.dir.path())
    }

    /// A host whose claude is the shell stand-in above, with no wsp-daemon.
    pub fn start() -> Throwaway {
        Throwaway::start_with(STAND_IN, false)
    }

    /// A host whose claude is the script `claude`, run from a copy of its bundle carrying `wsp_daemon()` where
    /// `daemon` asks for one and one is found.
    pub fn start_with(claude: &str, daemon: bool) -> Throwaway {
        Throwaway::start_with_tools(claude, daemon, &[])
    }

    /// As `start_with`, with more stand-ins in the test's bin before the host starts and looks for its tools: each
    /// a program's name and its script.
    pub fn start_with_tools(claude: &str, daemon: bool, tools: &[(&str, &str)]) -> Throwaway {
        for built in ["packages/host/dist/bin.js", "apps/web/dist/index.html"].map(|path| root().join(path)) {
            assert!(built.exists(), "{} is not built: run pnpm install && pnpm build", built.display());
        }
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("bin")).unwrap();
        std::fs::create_dir_all(dir.path().join("state")).unwrap();
        for (name, body) in [("claude", claude)].into_iter().chain(tools.iter().copied()) {
            let script = dir.path().join("bin").join(name);
            std::fs::write(&script, body).unwrap();
            std::fs::set_permissions(&script, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
        }
        let daemon = if daemon { wsp_daemon().ok() } else { None };
        let bin = bundle_with(dir.path(), daemon.as_deref());
        let host = up(dir.path(), &bin);
        let throwaway = Throwaway { dir, daemon, bin, host };
        throwaway.until_listening();
        throwaway
    }

    /// Stops the host and starts it again on the same state, as a person's restart does: every socket to it drops.
    pub fn restart(&mut self) {
        self.stop();
        for file in ["host.lock", "host-token"] {
            let _ = std::fs::remove_file(self.state().with_file_name(file));
        }
        self.host = up(self.dir.path(), &self.bin);
        self.until_listening();
    }

    fn until_listening(&self) {
        let until = Instant::now() + Duration::from_secs(30);
        // The lock is written before the host listens, with port 0 until it does.
        let listening = || {
            let lock = std::fs::read_to_string(self.state().with_file_name("host.lock")).unwrap_or_default();
            serde_json::from_str::<serde_json::Value>(&lock).ok().and_then(|lock| lock["port"].as_u64()).is_some_and(|port| port != 0)
        };
        while !self.state().with_file_name("host-token").exists() || !listening() {
            assert!(Instant::now() < until, "no host came up: {}", self.log());
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    /// A terminated host stops its daemon on its way out and a killed one leaves it running. Neither stops a running
    /// turn, whose process runs apart from the host so it outlives a restart, so those are interrupted first.
    fn stop(&mut self) {
        if matches!(self.host.try_wait(), Ok(None)) {
            interrupt_turns(self.dir.path());
        }
        drop(self.host.stdin.take());
        let until = Instant::now() + Duration::from_secs(25);
        while matches!(self.host.try_wait(), Ok(None)) && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(100));
        }
        let _ = self.host.kill();
        let _ = self.host.wait();
    }

    pub fn log(&self) -> String {
        std::fs::read_to_string(self.dir.path().join("up.log")).unwrap_or_default()
    }

    /// One wsp line against this host, which has to succeed; what it printed.
    pub fn wsp(&self, args: &[&str]) -> String {
        let state = self.state().to_string_lossy().into_owned();
        let out = node_in(self.dir.path(), &self.bin, args).args(["--state", &state]).stdin(Stdio::null()).output().unwrap();
        let said = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
        assert!(out.status.success(), "wsp {args:?}: {said}\nhost log: {}", self.log());
        said
    }
}

impl Drop for Throwaway {
    fn drop(&mut self) {
        self.stop();
    }
}

/// The host one test binary shares, with a project `repo` whose first thread ran: started by the first test that asks,
/// and stopped as the binary exits.
pub struct Shared {
    pub host: Throwaway,
    /// The project's folder, a git repository with one commit.
    pub folder: PathBuf,
}

pub fn shared() -> &'static Shared {
    static SHARED: OnceLock<Shared> = OnceLock::new();
    static PID: AtomicI32 = AtomicI32::new(0);
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    extern "C" fn stop() {
        let pid = PID.swap(0, Ordering::SeqCst);
        // SAFETY: waitpid with WNOHANG answers 0 only for a child of this process that has not exited, so nothing else is signalled.
        if pid <= 0 || unsafe { libc::waitpid(pid, std::ptr::null_mut(), libc::WNOHANG) } != 0 {
            if let Some(dir) = DIR.get() {
                let _ = std::fs::remove_dir_all(dir);
            }
            return;
        }
        if let Some(dir) = DIR.get() {
            interrupt_turns(dir);
        }
        // SAFETY: a signal to the shell running the host this binary spawned, then reaping it; the binary is exiting and holds no other handle on it.
        unsafe {
            libc::kill(pid, libc::SIGTERM);
            let until = Instant::now() + Duration::from_secs(25);
            let mut reaped = false;
            while !reaped && Instant::now() < until {
                reaped = libc::waitpid(pid, std::ptr::null_mut(), libc::WNOHANG) != 0;
                std::thread::sleep(Duration::from_millis(100));
            }
            if !reaped {
                libc::kill(pid, libc::SIGKILL);
                libc::waitpid(pid, std::ptr::null_mut(), 0);
            }
        }
        if let Some(dir) = DIR.get() {
            let _ = std::fs::remove_dir_all(dir);
        }
    }
    SHARED.get_or_init(|| {
        let host = Throwaway::start();
        let folder = host.repo("repo");
        host.wsp(&["add", &folder.to_string_lossy()]);
        host.wsp(&["run", "repo", "first", "--agent", "claude"]);
        // Only once the host is whole: a panic above drops the Throwaway, which stops the host and removes its folder.
        PID.store(host.host.id() as i32, Ordering::SeqCst);
        let _ = DIR.set(host.dir.path().to_owned());
        // SAFETY: registers a function with no arguments that touches only the statics above.
        unsafe { libc::atexit(stop) };
        Shared { host, folder }
    })
}

impl Throwaway {
    /// A git repository with one empty commit, in a folder of that name under the test's own.
    pub fn repo(&self, name: &str) -> PathBuf {
        self.repo_with(name, &[])
    }

    /// A git repository whose one commit holds `files`, each a path under it and its text.
    pub fn repo_with(&self, name: &str, files: &[(&str, &str)]) -> PathBuf {
        let folder = self.dir.path().join(name);
        std::fs::create_dir_all(&folder).unwrap();
        for (path, text) in files {
            let file = folder.join(path);
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(file, text).unwrap();
        }
        let git = |args: &[&str]| {
            assert!(Command::new("git").args(args).current_dir(&folder).env_clear().envs(self.env()).status().unwrap().success())
        };
        git(&["init", "-q"]);
        git(&["add", "-A"]);
        git(&["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "--allow-empty", "-m", "init"]);
        folder
    }

    pub async fn client(&self) -> HostClient {
        HostClient::connect(&self.state(), &self.env()).await.unwrap_or_else(|f| panic!("{}", f.message))
    }
}

/// The ids the shared host's first thread goes by.
pub struct First {
    pub project_id: String,
    pub workspace_id: String,
    pub thread_id: String,
    pub session_id: String,
}

pub async fn first(client: &HostClient) -> First {
    let project_id = client.projects().await.unwrap().into_iter().find(|p| p.name == "repo").expect("the project repo").id;
    let row =
        client.threads().await.unwrap().into_iter().find(|t| t.prompt.as_deref() == Some("first")).expect("the thread wsp run opened");
    First { project_id, workspace_id: row.workspace_id, thread_id: row.thread_id.expect("a thread id"), session_id: row.id }
}

/// A failure that says the call itself was wrong, not that the host refused it: the host's schema refused the frame
/// (a zod error's message is its issues as JSON), or the reply did not read as the op's reply type.
fn malformed(failure: &Failure) -> bool {
    failure.message.starts_with('[') || failure.message.contains("answered what its reply type does not read")
}

/// A call the host has to answer, with a reply that reads as its type.
pub fn answered<T: Debug>(op: &str, got: Result<T, Failure>) -> T {
    match got {
        Ok(reply) => {
            println!("{op:<36} answered");
            reply
        }
        Err(f) => panic!("{op}: {}", f.message),
    }
}

/// A call the host has to refuse, in words holding `holds`: the frame read and the host said no for a reason of its own.
pub fn refused<T: Debug>(op: &str, got: Result<T, Failure>, holds: &str) {
    match got {
        Ok(reply) => panic!("{op} was answered, not refused: {reply:?}"),
        Err(f) if malformed(&f) => panic!("{op}: {}", f.message),
        Err(f) => {
            assert!(f.message.contains(holds), "{op} was refused for another reason than {holds:?}: {}", f.message);
            println!("{op:<36} refused: {}", f.message.lines().next().unwrap_or_default());
        }
    }
}

/// A request read from the JSON the web app would send, so a test names only the fields it sets. The request type has
/// to send back exactly that: a null it would drop, a key it has no field for, or a default it would add fails here. A
/// field that may be null is sent as one only from Some(<Field>OrNull(None)).
pub fn ask<T: serde::de::DeserializeOwned + serde::Serialize>(params: Value) -> T {
    let request: T = serde_json::from_value(params.clone()).unwrap_or_else(|e| panic!("{params}: {e}"));
    let sent = serde_json::to_value(&request).unwrap();
    fn same(asked: &Value, sent: &Value) -> bool {
        match (asked, sent) {
            (Value::Object(a), Value::Object(s)) => a.len() == s.len() && a.iter().all(|(key, v)| s.get(key).is_some_and(|w| same(v, w))),
            (Value::Array(a), Value::Array(s)) => a.len() == s.len() && a.iter().zip(s).all(|(v, w)| same(v, w)),
            (Value::Number(a), Value::Number(s)) => a.as_f64() == s.as_f64(),
            _ => asked == sent,
        }
    }
    assert!(same(&params, &sent), "the request would send {sent}, not {params}");
    request
}
