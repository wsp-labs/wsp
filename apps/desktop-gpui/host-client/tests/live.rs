// SPDX-License-Identifier: AGPL-3.0-only
//! Against a host started for this test alone, on a state file of its own in a fresh folder: a project, a thread whose
//! first turn ran before the client looked, and a second turn started after the client subscribed, whose events have
//! to come through as they happen. The agent is a stand-in claude first on the host's PATH that answers stream-json
//! the way the claude adapter reads it, so no real agent runs. Needs node and the packages built, the web app among them
//! (pnpm build); the host runs its turns with no wsp-daemon staged.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use wsp_host_client::protocol::{SessionEvent, SessionEventResultStatus};
use wsp_host_client::{Env, HostClient, Live};

const STAND_IN: &str = r#"#!/bin/sh
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

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..").canonicalize().unwrap()
}

/// A host on a fresh state file in a folder of its own, stopped with this.
struct Throwaway {
    dir: tempfile::TempDir,
    host: Child,
}

/// What every process of the test runs in: the folder as its home, and the stand-in ahead of anything on PATH.
fn env_in(dir: &Path) -> Env {
    let home = dir.to_string_lossy().into_owned();
    let path = format!("{}/bin:{}", home, std::env::var("PATH").unwrap_or_default());
    [("HOME", home.as_str()), ("PATH", path.as_str()), ("ANTHROPIC_API_KEY", ""), ("SOLARI_API_KEY", "")]
        .map(|(k, v)| (k.to_owned(), v.to_owned()))
        .into_iter()
        .collect()
}

fn node_in(dir: &Path, args: &[&str]) -> Command {
    let mut command = Command::new("node");
    command.arg(repo().join("packages/host/dist/bin.js")).args(args).env_clear().envs(env_in(dir)).current_dir(dir);
    command
}

impl Throwaway {
    fn state(&self) -> PathBuf {
        self.dir.path().join("state").join("state.json")
    }

    fn env(&self) -> Env {
        env_in(self.dir.path())
    }

    fn start() -> Throwaway {
        for built in ["packages/host/dist/bin.js", "apps/web/dist/index.html"].map(|path| repo().join(path)) {
            assert!(built.exists(), "{} is not built: run pnpm install && pnpm build", built.display());
        }
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("bin")).unwrap();
        std::fs::create_dir_all(dir.path().join("state")).unwrap();
        let claude = dir.path().join("bin/claude");
        std::fs::write(&claude, STAND_IN).unwrap();
        std::fs::set_permissions(&claude, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
        let state = dir.path().join("state").join("state.json").to_string_lossy().into_owned();
        let log = std::fs::File::create(dir.path().join("up.log")).unwrap();
        let host =
            node_in(dir.path(), &["up", "--port", "0", "--state", &state]).stdout(log.try_clone().unwrap()).stderr(log).spawn().unwrap();
        let throwaway = Throwaway { dir, host };
        let until = Instant::now() + Duration::from_secs(30);
        while !throwaway.state().with_file_name("host-token").exists() || !throwaway.state().with_file_name("host.lock").exists() {
            assert!(Instant::now() < until, "no host came up: {}", throwaway.log());
            std::thread::sleep(Duration::from_millis(100));
        }
        throwaway
    }

    fn log(&self) -> String {
        std::fs::read_to_string(self.dir.path().join("up.log")).unwrap_or_default()
    }

    /// One wsp line against this host, which has to succeed; what it printed.
    fn wsp(&self, args: &[&str]) -> String {
        let state = self.state().to_string_lossy().into_owned();
        let out = node_in(self.dir.path(), args).args(["--state", &state]).stdin(Stdio::null()).output().unwrap();
        let said = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
        assert!(out.status.success(), "wsp {args:?}: {said}\nhost log: {}", self.log());
        said
    }
}

impl Drop for Throwaway {
    /// A terminated host stops its daemon and every turn's process on its way out; a killed one leaves them running.
    fn drop(&mut self) {
        // SAFETY: a signal to the pid of the child this test spawned and has not yet reaped.
        unsafe { libc::kill(self.host.id() as i32, libc::SIGTERM) };
        let until = Instant::now() + Duration::from_secs(20);
        while matches!(self.host.try_wait(), Ok(None)) && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(100));
        }
        let _ = self.host.kill();
        let _ = self.host.wait();
    }
}

fn prompt_of(event: &SessionEvent) -> Option<&str> {
    match event {
        SessionEvent::SessionStart { prompt, .. } => prompt.as_deref(),
        _ => None,
    }
}

#[tokio::test]
async fn lists_threads_and_receives_a_turn_started_after_it_subscribed() {
    let host = Throwaway::start();
    let folder = host.dir.path().join("repo");
    std::fs::create_dir_all(&folder).unwrap();
    let git = |args: &[&str]| {
        assert!(Command::new("git").args(args).current_dir(&folder).env_clear().envs(host.env()).status().unwrap().success())
    };
    git(&["init", "-q"]);
    git(&["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "--allow-empty", "-m", "init"]);
    host.wsp(&["add", &folder.to_string_lossy()]);
    host.wsp(&["run", "repo", "first", "--agent", "claude"]);

    // What a shell inside a thread carries, aimed at a closed port: the client keeps to the host serving the state file.
    let mut env = host.env();
    env.extend(
        [("WSP_HOST", "http://127.0.0.1:9"), ("WSP_HOST_URL", "http://127.0.0.1:9"), ("WSP_HOST_TOKEN", "x")]
            .map(|(k, v)| (k.to_owned(), v.to_owned())),
    );
    let client = HostClient::connect(&host.state(), &env).await.unwrap_or_else(|f| panic!("{}", f.message));

    let projects = client.projects().await.unwrap_or_else(|f| panic!("{}", f.message));
    println!("projects: {:?}", projects.iter().map(|p| (&p.id, &p.name, &p.path)).collect::<Vec<_>>());
    assert!(projects.iter().any(|p| p.name == "repo" && p.path.ends_with("/repo")), "{projects:?}");

    let threads = client.threads().await.unwrap_or_else(|f| panic!("{}", f.message));
    println!("threads: {:?}", threads.iter().map(|t| (&t.workspace_id, &t.thread_id, &t.prompt, &t.status)).collect::<Vec<_>>());
    let row = threads.iter().find(|t| t.prompt.as_deref() == Some("first")).expect("the thread wsp run opened");
    let thread_id = row.thread_id.clone().expect("a thread id");

    let mut thread = client.open(&row.workspace_id, &thread_id).await.unwrap_or_else(|f| panic!("{}", f.message));
    println!("history: {} events, types {:?}", thread.history.len(), thread.history.iter().map(event_type).collect::<Vec<_>>());
    assert!(thread.history.iter().any(|e| prompt_of(e) == Some("first")));
    let replies = thread.history.iter().filter(|e| matches!(e, SessionEvent::SessionDelta { text, .. } if text == "ok")).count();
    assert_eq!(replies, 250, "every reply of the first turn, more than the 200 events a first page held");
    assert!(!thread.history.iter().any(|e| prompt_of(e) == Some("second")));

    let sent = Instant::now();
    host.wsp(&["send", &thread_id, "--detach", "second"]);
    println!("{:>6} ms  wsp send --detach returned", sent.elapsed().as_millis());
    let mut seen = Vec::new();
    let (mut started, mut said, mut completed) = (false, 0, false);
    loop {
        let next = tokio::time::timeout(Duration::from_secs(60), thread.next()).await.expect("the turn's events within 60 s");
        let live = next.expect("the socket stays open").unwrap_or_else(|f| panic!("{}", f.message));
        let Live::Event(event) = live else {
            if let Live::Other(frame) = live {
                println!("{:>6} ms  {} (outside the transcript)", sent.elapsed().as_millis(), frame["type"]);
            }
            continue;
        };
        println!("{:>6} ms  {}", sent.elapsed().as_millis(), event_type(&event));
        seen.push(event_type(&event));
        match &*event {
            SessionEvent::SessionStart { prompt, .. } => started |= prompt.as_deref() == Some("second"),
            SessionEvent::SessionDelta { text, .. } => said += usize::from(text == "ok"),
            SessionEvent::SessionDone { result, .. } => completed |= result.status == SessionEventResultStatus::Completed,
            SessionEvent::SessionEnd { .. } => break,
            _ => {}
        }
    }
    assert!(started && said == 250 && completed, "the second turn's start, every reply once and result, saw {seen:?}");
}

fn event_type(event: &SessionEvent) -> String {
    serde_json::to_value(event).unwrap()["type"].as_str().unwrap_or_default().to_owned()
}
