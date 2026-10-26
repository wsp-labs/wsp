// SPDX-License-Identifier: AGPL-3.0-only
//! Tests of spec/slate_box.md rules 9-13: slate-box.ts at 65f6b89c7, ported from test/slate-box.test.ts:90-317 over a
//! machine whose exec and run are this computer's bash, as the TS's `bashMachine()`.

use std::cell::{Cell, RefCell};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Stdio;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use base64::Engine as _;
use futures::FutureExt;
use indexmap::IndexMap;
use tokio::io::AsyncReadExt;
use tokio::task::LocalSet;

// The tests run bashes against the one folder every box slate lives in, and a sweep of it takes another run's payload.
static ONE_AT_A_TIME: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

use wsp_core::clock::real_clock;
use wsp_core::ids::OsIds;
use wsp_engine::cancel::Cancel;
use wsp_engine::errors::{EngineError, Result};
use wsp_engine::machine::{ExecOptions, ExecResult, Machine, MachineKind, MachineLife, MachineState, RunOptions, SnapshotOptions};
use wsp_slates::slate_box::*;
use wsp_slates::slate_mcp_on::box_slate_dir;
use wsp_slates::slate_runs::{ReshapeAnswer, ReshapeStart, RoadEnd, RoadEvents, RoadStart, RunRoad, Stream};

fn plain(m: &str) -> EngineError {
    EngineError::Other { message: m.to_string(), code: None }
}

fn b64(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

fn nul(items: &[&str]) -> String {
    b64(items.iter().map(|i| format!("{i}\0")).collect::<String>().as_bytes())
}

fn sh(cmd: &str, stdin: Option<&[u8]>) -> std::process::Output {
    let mut child = std::process::Command::new("bash")
        .arg("-c")
        .arg(cmd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    {
        use std::io::Write;
        let mut input = child.stdin.take().unwrap();
        if let Some(bytes) = stdin {
            input.write_all(bytes).unwrap();
        }
    }
    child.wait_with_output().unwrap()
}

fn temp() -> (tempfile::TempDir, String) {
    let dir = tempfile::tempdir().unwrap();
    let path = std::fs::canonicalize(dir.path()).unwrap().to_string_lossy().to_string();
    (dir, path)
}

fn thread_name() -> String {
    format!("t-{}-{}", std::process::id(), rand_hex())
}

fn rand_hex() -> String {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().subsec_nanos();
    format!(
        "{nanos:08x}{:04x}",
        COUNTER.with(|c| {
            c.set(c.get() + 1);
            c.get()
        })
    )
}

thread_local!(static COUNTER: Cell<u32> = const { Cell::new(0) });

/// A slate folder under /tmp/wsp-slates removed when the test ends.
struct SlateDirGuard(String);

impl Drop for SlateDirGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

type Hook = Rc<dyn Fn(&str) -> Option<Result<ExecResult>>>;

/// This computer's bash as a machine: exec runs a command to its end; run runs a script detached in its own group,
/// streaming its lines, killed at its deadline (124) or on its cancel (130).
struct BashMachine {
    killed: Cell<bool>,
    down: Rc<Cell<bool>>,
    /// Seen before every run: may answer instead of running (the TS's patched `machine.run`).
    before_run: RefCell<Option<Hook>>,
    gate: RefCell<Option<tokio::sync::oneshot::Receiver<()>>>,
    launches: RefCell<Vec<Option<Arc<dyn Machine>>>>,
    scripts: RefCell<Vec<String>>,
    exec_answer: RefCell<Option<ExecResult>>,
}

impl BashMachine {
    fn new() -> Arc<BashMachine> {
        Arc::new(BashMachine {
            killed: Cell::new(false),
            down: Rc::new(Cell::new(false)),
            before_run: RefCell::new(None),
            gate: RefCell::new(None),
            launches: RefCell::new(vec![]),
            scripts: RefCell::new(vec![]),
            exec_answer: RefCell::new(None),
        })
    }
}

async fn run_bash(script: &str, opts: &RunOptions, killed: &Cell<bool>) -> ExecResult {
    let mut cmd = tokio::process::Command::new("bash");
    cmd.arg("-c").arg(script).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).process_group(0);
    let mut child = cmd.spawn().expect("bash");
    let pid = child.id().unwrap() as i32;
    let mut stdout = child.stdout.take().unwrap();
    let mut err = child.stderr.take().unwrap();
    let mut out = String::new();
    let mut err_text = String::new();
    let deadline = tokio::time::sleep(Duration::from_millis(opts.deadline_ms));
    tokio::pin!(deadline);
    let cancel = opts.signal.clone();
    let cancelled = async move {
        match cancel {
            Some(c) => c.token().cancelled().await,
            None => futures::future::pending::<()>().await,
        }
    };
    tokio::pin!(cancelled);
    let mut forced: Option<i64> = None;
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        tokio::select! {
            n = stdout.read(&mut buf) => match n {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    // Each chunk's lines to on_line, as the TS's bashMachine splits a chunk.
                    let text = String::from_utf8_lossy(&buf[..n]).to_string();
                    out.push_str(&text);
                    if let Some(f) = &opts.on_line {
                        for line in text.split('\n').filter(|l| !l.is_empty()) {
                            f(line);
                        }
                    }
                }
            },
            _ = &mut deadline, if forced.is_none() => {
                forced = Some(124);
                unsafe { libc::kill(-pid, libc::SIGKILL) };
            }
            _ = &mut cancelled, if forced.is_none() => {
                forced = Some(130);
                killed.set(true);
                unsafe { libc::kill(-pid, libc::SIGKILL) };
            }
        }
    }
    let _ = err.read_to_string(&mut err_text).await;
    let status = child.wait().await.unwrap();
    let code = forced.unwrap_or_else(|| status.code().map(i64::from).unwrap_or(-1));
    ExecResult { exit_code: code.into(), stdout: out, stderr: err_text }
}

#[async_trait(?Send)]
impl Machine for BashMachine {
    fn id(&self) -> &str {
        "m1"
    }
    fn kind(&self) -> MachineKind {
        MachineKind::Sandbox
    }
    async fn exec(&self, cmd: &str, opts: ExecOptions) -> Result<ExecResult> {
        if self.down.get() {
            return Err(plain("the link went"));
        }
        if let Some(answer) = self.exec_answer.borrow().clone() {
            if !cmd.starts_with("rm -rf") {
                return Ok(answer);
            }
        }
        let o = sh(cmd, opts.stdin.as_deref());
        Ok(ExecResult {
            exit_code: i64::from(o.status.code().unwrap_or(1)).into(),
            stdout: String::from_utf8_lossy(&o.stdout).to_string(),
            stderr: String::from_utf8_lossy(&o.stderr).to_string(),
        })
    }
    async fn run(&self, script: &str, opts: RunOptions) -> Result<ExecResult> {
        self.scripts.borrow_mut().push(script.to_string());
        self.launches.borrow_mut().push(opts.launch_on.clone());
        let hook = self.before_run.borrow().clone();
        if let Some(h) = hook {
            if let Some(answer) = h(script) {
                return answer;
            }
        }
        let gate = self.gate.borrow_mut().take();
        if let Some(g) = gate {
            if script.contains("GATED") {
                let _ = g.await;
            } else {
                *self.gate.borrow_mut() = Some(g);
            }
        }
        Ok(run_bash(script, &opts, &self.killed).await)
    }
    async fn snapshot(&self, _name: &str, _life: MachineLife, _opts: SnapshotOptions) -> Result<String> {
        Err(plain("not in this test"))
    }
    async fn pause(&self, _wait_ms: Option<u64>) -> Result<()> {
        Err(plain("not in this test"))
    }
    async fn resume(&self, _signal: Option<&Cancel>) -> Result<()> {
        Err(plain("not in this test"))
    }
    async fn kill(&self) -> Result<()> {
        Err(plain("not in this test"))
    }
    async fn state(&self) -> Result<MachineState> {
        Err(plain("not in this test"))
    }
    async fn download_url(&self, _path: &str) -> Result<String> {
        Err(plain("not in this test"))
    }
    async fn upload_url(&self, _path: &str) -> Result<String> {
        Err(plain("not in this test"))
    }
}

fn files(pairs: &[(&str, &str)]) -> SlateFilesNow {
    let map: IndexMap<String, String> = pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
    Rc::new(move || map.clone())
}

fn no_wake() -> Ready {
    Rc::new(|| async {}.boxed_local())
}

/// The events of one start: its lines, and its end sent once.
struct Heard {
    lines: RefCell<Vec<String>>,
    end: RefCell<Option<tokio::sync::oneshot::Sender<RoadEnd>>>,
}

impl RoadEvents for Heard {
    fn line(&self, _stream: Stream, text: &str) {
        self.lines.borrow_mut().push(text.to_string());
    }
    fn end(&self, end: RoadEnd) {
        let tx = self.end.borrow_mut().take().expect("the road ends once");
        let _ = tx.send(end);
    }
}

fn heard() -> (Rc<Heard>, tokio::sync::oneshot::Receiver<RoadEnd>) {
    let (tx, rx) = tokio::sync::oneshot::channel();
    (Rc::new(Heard { lines: RefCell::new(vec![]), end: RefCell::new(Some(tx)) }), rx)
}

fn road_start(cmd: &str, cwd: &str, env: &[(&str, &str)], args: &[&str], stdin: Option<&str>, timeout_s: f64, stream: bool) -> RoadStart {
    RoadStart {
        cmd: cmd.into(),
        args: args.iter().map(|s| s.to_string()).collect(),
        cwd: cwd.into(),
        env: env.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
        stdin: stdin.map(String::from),
        timeout_s,
        stream,
    }
}

/// Starts a command on the road, answering its end and the lines it streamed.
async fn ended(road: &Rc<BoxRoad>, o: RoadStart) -> (RoadEnd, Vec<String>) {
    let (on, rx) = heard();
    let _started = road.start(o, on.clone());
    let end = rx.await.expect("the road ends once");
    let got = on.lines.borrow().clone();
    (end, got)
}

fn runs_in(thread: &str) -> Vec<String> {
    std::fs::read_dir(box_slate_dir(thread))
        .map(|d| d.filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().to_string()).filter(|n| n.starts_with(".run-")).collect())
        .unwrap_or_default()
}

// ---- the launcher ----

#[test]
fn the_launcher_hands_values_whole_runs_none_and_leaves_none_on_disk() {
    // TS slate-box.test.ts:91-104.
    let (_keep, dir) = temp();
    let p = format!("{dir}/.run-x");
    let value = "two\nlines \"$HOME\" `touch pwned-env` $(touch pwned-env2)";
    std::fs::create_dir(&p).unwrap();
    std::fs::write(format!("{p}/env"), nul(&["NAME", value, "SLATE_DIR", "/tmp/s"])).unwrap();
    std::fs::write(format!("{p}/args"), nul(&["a b", "$(touch pwned-arg)", ""])).unwrap();
    std::fs::write(format!("{p}/in"), b64(b"in put\nsecond")).unwrap();
    let cmd = format!(
        "printf '%s|' \"$NAME\" \"$1\" \"$2\" \"$3\" \"$#\" \"$SLATE_DIR\"; echo; cat; echo; pwd; ls -a {} | grep -c run || true",
        serde_json::to_string(&dir).unwrap()
    );
    let script = box_launcher(&Launch { payload: p.clone(), cwd: dir.clone(), cmd });
    let o = std::process::Command::new("bash").arg("-c").arg(&script).current_dir(std::env::temp_dir()).output().unwrap();
    assert_eq!(String::from_utf8_lossy(&o.stdout), format!("{value}|a b|$(touch pwned-arg)||3|/tmp/s|\nin put\nsecond\n{dir}\n0\n"));
    for pwned in ["pwned-env", "pwned-env2", "pwned-arg"] {
        assert!(!Path::new(&dir).join(pwned).exists(), "{pwned}");
    }
}

#[test]
fn the_launcher_says_plainly_when_the_folder_is_missing() {
    // TS slate-box.test.ts:106-118.
    let (_keep, dir) = temp();
    let p = format!("{dir}/.run-y");
    std::fs::create_dir(&p).unwrap();
    for name in ["env", "args", "in"] {
        std::fs::write(format!("{p}/{name}"), "").unwrap();
    }
    let script = box_launcher(&Launch { payload: p, cwd: "/nowhere/at/all".into(), cmd: "true".into() });
    let o = std::process::Command::new("bash").arg("-c").arg(&script).output().unwrap();
    assert_eq!(String::from_utf8_lossy(&o.stderr), "the folder /nowhere/at/all does not exist\n");
    assert_eq!(o.status.code(), Some(1));
}

#[test]
fn the_launcher_text_is_the_ts_text() {
    // spec rules 10, 11: the text the TS printed.
    assert_eq!(
        box_launcher(&Launch { payload: "/tmp/wsp-slates/t/.run-ab".into(), cwd: "/root/it's".into(), cmd: "echo 'hi' $1".into() }),
        "set -u\np='/tmp/wsp-slates/t/.run-ab'\nbase64 -d < \"$p/env\" > \"$p/env0\" && base64 -d < \"$p/args\" > \"$p/args0\" && base64 -d < \"$p/in\" > \"$p/in0\" || { rm -rf \"$p\"; echo \"the run's values did not arrive\" >&2; exit 1; }\nwhile IFS= read -r -d '' k && IFS= read -r -d '' v; do export \"$k=$v\"; done < \"$p/env0\"\na=(); while IFS= read -r -d '' x; do a+=(\"$x\"); done < \"$p/args0\"\nexec 3< \"$p/in0\"\nrm -rf \"$p\"\ncd '/root/it'\\''s' 2>/dev/null || { echo 'the folder /root/it'\\''s does not exist' >&2; exit 1; }\nexec bash -c 'echo '\\''hi'\\'' $1' bash ${a[@]+\"${a[@]}\"} <&3"
    );
    assert_eq!(nul_separated(&["a".into(), "b c".into()]), b64(b"a\0b c\0"));
    assert_eq!(nul_separated(&[]), "");
}

// ---- the road ----

#[tokio::test(flavor = "current_thread")]
async fn writes_the_slates_files_sweeps_and_runs_with_values_folder_and_lines() {
    let _one = ONE_AT_A_TIME.lock().await;
    // TS slate-box.test.ts:124-144.
    LocalSet::new()
        .run_until(async {
            let machine = BashMachine::new();
            let thread = thread_name();
            let dir = box_slate_dir(&thread);
            let _guard = SlateDirGuard(dir.clone());
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(format!("{dir}/stale.py"), "old").unwrap();
            let (_keep, cwd) = temp();
            let m: Arc<dyn Machine> = machine.clone();
            let road = box_road(
                m.clone(),
                &thread,
                files(&[("hi.sh", "echo from the file")]),
                no_wake(),
                box_ledger(None, real_clock()),
                m,
                Rc::new(OsIds),
            );
            assert_eq!(road.slate_dir(), dir);
            let (end, lines) = ended(
                &road,
                road_start(
                    "sh \"$SLATE_DIR/hi.sh\"; printf '%s|%s\\n' \"$X\" \"$1\"; pwd; cat",
                    &cwd,
                    &[("SLATE_DIR", &dir), ("X", "x y")],
                    &["$(touch nope)"],
                    Some("IN"),
                    20.0,
                    true,
                ),
            )
            .await;
            assert_eq!((end.code, end.timed_out, end.cut), (Some(0), false, false));
            assert_eq!(end.out, format!("from the file\nx y|$(touch nope)\n{cwd}\nIN"));
            assert_eq!(lines, vec!["from the file".to_string(), "x y|$(touch nope)".into(), cwd.clone(), "IN".into()]);
            assert_eq!(std::fs::read_to_string(format!("{dir}/hi.sh")).unwrap(), "echo from the file");
            assert!(!Path::new(&format!("{dir}/stale.py")).exists());
            assert_eq!(runs_in(&thread), Vec::<String>::new());
            assert!(!Path::new(&cwd).join("nope").exists());
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn the_payload_folder_is_the_accounts_alone_before_decoding() {
    let _one = ONE_AT_A_TIME.lock().await;
    // TS slate-box.test.ts:146-162.
    LocalSet::new()
        .run_until(async {
            let machine = BashMachine::new();
            let modes = Rc::new(RefCell::new(Vec::<u32>::new()));
            let seen = modes.clone();
            *machine.before_run.borrow_mut() = Some(Rc::new(move |script: &str| {
                if let Some(p) = script.lines().find_map(|l| l.strip_prefix("p='").and_then(|r| r.strip_suffix('\''))) {
                    seen.borrow_mut().push(std::fs::metadata(p).unwrap().permissions().mode() & 0o777);
                }
                None
            }));
            let thread = thread_name();
            let _guard = SlateDirGuard(box_slate_dir(&thread));
            let (_keep, cwd) = temp();
            let m: Arc<dyn Machine> = machine.clone();
            let road = box_road(m.clone(), &thread, files(&[]), no_wake(), box_ledger(None, real_clock()), m, Rc::new(OsIds));
            let (end, _) = ended(&road, road_start("true", &cwd, &[("TOKEN", "s")], &[], Some(""), 20.0, false)).await;
            assert_eq!(end.code, Some(0));
            assert_eq!(*modes.borrow(), vec![0o700]);
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn hands_the_launch_alone_to_the_cgroup_machine() {
    let _one = ONE_AT_A_TIME.lock().await;
    // TS slate-box.test.ts:164-176: the machine runs and writes; the launch alone is handed the cgroup machine.
    LocalSet::new()
        .run_until(async {
            let machine = BashMachine::new();
            let launch_on = BashMachine::new();
            let thread = thread_name();
            let _guard = SlateDirGuard(box_slate_dir(&thread));
            let (_keep, cwd) = temp();
            let m: Arc<dyn Machine> = machine.clone();
            let l: Arc<dyn Machine> = launch_on.clone();
            let road = box_road(
                m.clone(),
                &thread,
                files(&[("a.sh", "echo a")]),
                no_wake(),
                box_ledger(None, real_clock()),
                l.clone(),
                Rc::new(OsIds),
            );
            let (end, _) = ended(&road, road_start("true", &cwd, &[], &[], Some(""), 20.0, false)).await;
            assert_eq!(end.code, Some(0));
            let handed = machine.launches.borrow().clone();
            assert_eq!(handed.len(), 1);
            assert!(Arc::ptr_eq(handed[0].as_ref().expect("launch_on handed"), &l));
            assert!(launch_on.scripts.borrow().is_empty(), "the cgroup machine runs nothing itself");
            assert_eq!(std::fs::read_to_string(format!("{}/a.sh", box_slate_dir(&thread))).unwrap(), "echo a");
            // The machine itself as its own launch machine hands none.
            let road = box_road(m.clone(), &thread, files(&[]), no_wake(), box_ledger(None, real_clock()), m.clone(), Rc::new(OsIds));
            ended(&road, road_start("true", &cwd, &[], &[], Some(""), 20.0, false)).await;
            assert!(machine.launches.borrow()[1].is_none());
            assert!(Arc::ptr_eq(&launch_on_for(&m, None, &thread), &m));
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn values_a_box_left_go_once_it_answers_and_live_values_are_spared() {
    let _one = ONE_AT_A_TIME.lock().await;
    // TS slate-box.test.ts:248-291.
    LocalSet::new()
        .run_until(async {
            let machine = BashMachine::new();
            let down = machine.down.clone();
            *machine.before_run.borrow_mut() = Some(Rc::new(move |script: &str| {
                if script.contains("LINK_GOES") {
                    down.set(true);
                    return Some(Err(plain("the link went")));
                }
                None
            }));
            let (tx, rx) = tokio::sync::oneshot::channel::<()>();
            *machine.gate.borrow_mut() = Some(rx);
            let ledger = box_ledger(Some(200), real_clock());
            let [one, two, three] = [thread_name(), thread_name(), thread_name()];
            let _guards: Vec<SlateDirGuard> = [&one, &two, &three].iter().map(|t| SlateDirGuard(box_slate_dir(t))).collect();
            let m: Arc<dyn Machine> = machine.clone();
            let go = |t: &str, cmd: &str, env: &[(&str, &str)]| {
                let road = box_road(m.clone(), t, files(&[]), no_wake(), ledger.clone(), m.clone(), Rc::new(OsIds));
                let o = road_start(cmd, "/tmp", env, &[], None, 20.0, false);
                async move { ended(&road, o).await.0 }
            };
            // The link goes after the values went up: they stay, owed, and go on the retry once the box answers.
            let gone = go(&one, "echo LINK_GOES", &[("TOKEN", "s3cret")]).await;
            assert_eq!(gone.error.as_deref(), Some("the link went"));
            assert_eq!(runs_in(&one).len(), 1);
            assert_eq!(ledger.owed(machine.as_ref()).len(), 1);
            machine.down.set(false);
            for _ in 0..200 {
                if runs_in(&one).is_empty() {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
            assert_eq!(runs_in(&one), Vec::<String>::new());
            assert!(ledger.owed(machine.as_ref()).is_empty());
            // Stale values in another thread's folder go at the next contact; a run still starting keeps its own.
            std::fs::create_dir_all(format!("{}/.run-stale", box_slate_dir(&two))).unwrap();
            std::fs::write(format!("{}/.run-stale/env", box_slate_dir(&two)), "s3cret").unwrap();
            let gated = tokio::task::spawn_local(go(&one, "echo GATED \"$TOKEN\"", &[("TOKEN", "still here")]));
            for _ in 0..200 {
                if machine.scripts.borrow().iter().any(|s| s.contains("GATED")) {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            assert_eq!(runs_in(&one).len(), 1);
            assert_eq!(go(&three, "echo hi", &[]).await.out, "hi\n");
            assert_eq!(runs_in(&two), Vec::<String>::new());
            assert_eq!(runs_in(&one).len(), 1);
            tx.send(()).unwrap();
            assert_eq!(gated.await.unwrap().out, "GATED still here\n");
            assert_eq!(runs_in(&one), Vec::<String>::new());
            assert_eq!(ledger.live.borrow().len(), 0);
            ledger.close();
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn ledger_close_cancels_retries() {
    let _one = ONE_AT_A_TIME.lock().await;
    // spec rule 9: a closed ledger arms no retry and cancels the armed one; a contact still pays.
    LocalSet::new()
        .run_until(async {
            let machine = BashMachine::new();
            machine.down.set(true);
            let ledger = box_ledger(Some(100), real_clock());
            let m: Arc<dyn Machine> = machine.clone();
            let (_keep, dir) = temp();
            let path = format!("{dir}/owed");
            std::fs::create_dir(&path).unwrap();
            assert!(!ledger.remove(m.clone(), &path).await);
            assert_eq!(ledger.owed(machine.as_ref()), vec![path.clone()]);
            ledger.close();
            machine.down.set(false);
            tokio::time::sleep(Duration::from_millis(350)).await;
            assert!(Path::new(&path).exists(), "no retry ran after close");
            assert_eq!(ledger.owed(machine.as_ref()), vec![path.clone()]);
            ledger.paid(machine.as_ref(), std::slice::from_ref(&path));
            assert!(ledger.owed(machine.as_ref()).is_empty());
            assert!(ledger.remove(m, &path).await);
            assert!(!Path::new(&path).exists());
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn a_failed_upload_ends_with_the_machines_words() {
    let _one = ONE_AT_A_TIME.lock().await;
    // spec rule 12: the upload's stderr trimmed, else its exit code.
    LocalSet::new()
        .run_until(async {
            let thread = thread_name();
            let _guard = SlateDirGuard(box_slate_dir(&thread));
            for (answer, words) in [
                (
                    ExecResult { exit_code: 3.into(), stdout: String::new(), stderr: "  disk full\n".into() },
                    "the slate's files did not reach the thread's machine: disk full",
                ),
                (
                    ExecResult { exit_code: 3.into(), stdout: String::new(), stderr: "   ".into() },
                    "the slate's files did not reach the thread's machine: exit 3",
                ),
            ] {
                let machine = BashMachine::new();
                *machine.exec_answer.borrow_mut() = Some(answer);
                let m: Arc<dyn Machine> = machine.clone();
                let road = box_road(m.clone(), &thread, files(&[]), no_wake(), box_ledger(None, real_clock()), m, Rc::new(OsIds));
                let (end, _) = ended(&road, road_start("true", "/tmp", &[], &[], None, 20.0, false)).await;
                assert_eq!(
                    (end.code, end.error.as_deref(), end.out.as_str(), end.err.as_str(), end.cut, end.timed_out),
                    (None, Some(words), "", "", false, false)
                );
                assert!(machine.scripts.borrow().is_empty(), "nothing launched");
            }
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn caps_output_at_2_mb_and_marks_it_cut() {
    let _one = ONE_AT_A_TIME.lock().await;
    // spec rule 12: stdout cut at 2 MiB, stderr given only the room left, cut set.
    LocalSet::new()
        .run_until(async {
            let machine = BashMachine::new();
            let thread = thread_name();
            let _guard = SlateDirGuard(box_slate_dir(&thread));
            let m: Arc<dyn Machine> = machine.clone();
            let road = box_road(m.clone(), &thread, files(&[]), no_wake(), box_ledger(None, real_clock()), m, Rc::new(OsIds));
            let (end, _) = ended(
                &road,
                road_start("head -c 3145728 /dev/zero | tr '\\0' x; echo; echo oops >&2", "/tmp", &[], &[], None, 20.0, false),
            )
            .await;
            assert_eq!(end.out.len(), 2_097_152);
            assert!(end.out.bytes().all(|b| b == b'x'));
            assert_eq!(end.err, "");
            assert!(end.cut);
            let (small, _) = ended(&road, road_start("printf ab; printf cd >&2", "/tmp", &[], &[], None, 20.0, false)).await;
            assert_eq!((small.out.as_str(), small.err.as_str(), small.cut), ("ab", "cd", false));
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn a_kill_cancels_the_command_and_a_then_reshapes_there() {
    let _one = ONE_AT_A_TIME.lock().await;
    // TS slate-box.test.ts:293-316.
    LocalSet::new()
        .run_until(async {
            let machine = BashMachine::new();
            let thread = thread_name();
            let _guard = SlateDirGuard(box_slate_dir(&thread));
            let m: Arc<dyn Machine> = machine.clone();
            let road = box_road(m.clone(), &thread, files(&[]), no_wake(), box_ledger(None, real_clock()), m, Rc::new(OsIds));
            let started = std::time::Instant::now();
            let (on, rx) = heard();
            let handle = road.start(road_start("sleep 30", "/tmp", &[], &[], None, 60.0, false), on);
            for _ in 0..200 {
                if !machine.scripts.borrow().is_empty() {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            (handle.kill)();
            let end = rx.await.unwrap();
            assert!(started.elapsed() < Duration::from_secs(10));
            assert!(machine.killed.get());
            assert_eq!(end.code, Some(130));
            let shaped = road.reshape(ReshapeStart {
                cmd: "printf '{\"got\":\"%s\"}' \"$(cat)\"".into(),
                input: "a b".into(),
                cwd: "/tmp".into(),
                env: IndexMap::new(),
                timeout_s: 20.0,
                scrub: Rc::new(|t: &str| t.to_string()),
            });
            assert_eq!(shaped.done.await, ReshapeAnswer::Json(serde_json::json!({ "got": "a b" })));
        })
        .await;
}
