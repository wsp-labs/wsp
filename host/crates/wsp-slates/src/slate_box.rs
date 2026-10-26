// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/slate-box.ts at 65f6b89c7. Spec: spec/slate_box.md (rules 9-13).
//!
//! A slate's command on the thread's own machine through the engine's `Machine`: the slate's files and the command's
//! values go up first, a fixed launcher decodes the values, deletes them, and runs the agent's text as `bash -c`'s one
//! argument with the values as its environment, positional parameters and stdin. The ledger keeps what this host
//! still owes each box.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{Arc, Mutex, OnceLock};

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use futures::future::LocalBoxFuture;
use indexmap::{IndexMap, IndexSet};
use tokio::sync::oneshot;

pub use wsp_engine::machine::Machine;
pub use wsp_engine::place_machine::PlaceFolderMachine;

use wsp_core::clock::{Cancel, Clock, Timer};
use wsp_core::ids::Ids;
use wsp_engine::cancel::Cancel as RunCancel;
use wsp_engine::exec_detached::put::{put_files, GuestWrite, PutFilesOptions};
use wsp_engine::machine::{ExecOptions, OnLine, RunOptions};
use wsp_wire::types::shell_quote::shell_quote;
use wsp_wire::wire::daemon::{EXEC_DEADLINE_EXIT, EXEC_OUTPUT_MAX};
use wsp_words::js::js_trim;
use wsp_words::js::utf16::{cut as js_cut, len as js_len};
use wsp_words::thread_cgroup::thread_cgroup;

use crate::slate_mcp_on::{box_slate_dir, BOX_SLATES};
use crate::slate_runs::{
    reshape_answer, Reshape, ReshapeAnswer, ReshapeStart, RoadEnd, RoadEvents, RoadStart, RoadStarted, RunRoad, Stream,
};

/// How long a removal a box did not answer waits before it is tried again (slate-box.ts:16).
pub const OWED_RETRY_MS: i64 = 60_000;

/// Every run this process has in flight, across ledgers: one host has one ledger, but a sweep must spare each live
/// payload whichever ledger started it.
fn in_flight() -> &'static Mutex<IndexSet<String>> {
    static IN_FLIGHT: OnceLock<Mutex<IndexSet<String>>> = OnceLock::new();
    IN_FLIGHT.get_or_init(|| Mutex::new(IndexSet::new()))
}

struct Owed {
    machine: Arc<dyn Machine>,
    paths: IndexSet<String>,
    timer: Option<Cancel>,
}

/// slate-box.ts:20 BoxLedger: the values of runs in flight, which no sweep may take, and removals a box never answered.
pub struct BoxLedger {
    pub live: RefCell<IndexSet<String>>,
    owed: RefCell<HashMap<String, Owed>>,
    closed: Cell<bool>,
    retry_ms: i64,
    clock: Rc<dyn Clock>,
}

pub fn box_ledger(retry_ms: Option<i64>, clock: Rc<dyn Clock>) -> Rc<BoxLedger> {
    Rc::new(BoxLedger {
        live: RefCell::new(IndexSet::new()),
        owed: RefCell::new(HashMap::new()),
        closed: Cell::new(false),
        retry_ms: retry_ms.unwrap_or(OWED_RETRY_MS),
        clock,
    })
}

impl BoxLedger {
    /// Removes a path there, answering whether the box said it is gone; until it does, the removal is owed.
    pub async fn remove(self: &Rc<Self>, machine: Arc<dyn Machine>, path: &str) -> bool {
        let cmd = format!("rm -rf {}", shell_quote(path));
        let gone = matches!(machine.exec(&cmd, ExecOptions::default()).await, Ok(r) if r.exit_code.get() == 0);
        if gone {
            self.paid(machine.as_ref(), &[path.to_string()]);
            return true;
        }
        let arm = {
            let mut owed = self.owed.borrow_mut();
            let entry = owed.entry(machine.id().to_string()).or_insert_with(|| Owed {
                machine: machine.clone(),
                paths: IndexSet::new(),
                timer: None,
            });
            entry.paths.insert(path.to_string());
            entry.timer.is_none() && !self.closed.get()
        };
        if arm {
            let cancel = self.arm_retry(machine.id().to_string());
            if let Some(o) = self.owed.borrow_mut().get_mut(machine.id()) {
                o.timer = Some(cancel);
            }
        }
        false
    }

    fn arm_retry(self: &Rc<Self>, machine_id: String) -> Cancel {
        let weak = Rc::downgrade(self);
        let fire: Timer = Box::new(move || {
            let Some(ledger) = weak.upgrade() else {
                return;
            };
            let due = {
                let mut owed = ledger.owed.borrow_mut();
                match owed.get_mut(&machine_id) {
                    Some(o) => {
                        o.timer = None;
                        Some((o.machine.clone(), o.paths.iter().cloned().collect::<Vec<String>>()))
                    }
                    None => None,
                }
            };
            let Some((machine, paths)) = due else {
                return;
            };
            for path in paths {
                let ledger = ledger.clone();
                let machine = machine.clone();
                tokio::task::spawn_local(async move {
                    ledger.remove(machine, &path).await;
                });
            }
        });
        self.clock.schedule(fire, self.retry_ms)
    }

    /// The owed paths of this machine, oldest first.
    pub fn owed(&self, machine: &dyn Machine) -> Vec<String> {
        self.owed.borrow().get(machine.id()).map(|o| o.paths.iter().cloned().collect()).unwrap_or_default()
    }

    /// The box answered for these removals at a contact of its own.
    pub fn paid(&self, machine: &dyn Machine, paths: &[String]) {
        let timer = {
            let mut owed = self.owed.borrow_mut();
            let Some(o) = owed.get_mut(machine.id()) else {
                return;
            };
            for path in paths {
                o.paths.shift_remove(path);
            }
            if !o.paths.is_empty() {
                return;
            }
            let timer = o.timer.take();
            owed.remove(machine.id());
            timer
        };
        if let Some(cancel) = timer {
            cancel();
        }
    }

    pub fn close(&self) {
        self.closed.set(true);
        let timers: Vec<Cancel> = self.owed.borrow().values().filter_map(|o| o.timer.clone()).collect();
        for cancel in timers {
            cancel();
        }
    }
}

/// What the launcher is built from (slate-box.ts:79).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Launch {
    /// The folder the values went up in.
    pub payload: String,
    pub cwd: String,
    pub cmd: String,
}

/// The script the machine runs for one command (spec rule 10).
pub fn box_launcher(o: &Launch) -> String {
    [
        "set -u".to_string(),
        format!("p={}", shell_quote(&o.payload)),
        r#"base64 -d < "$p/env" > "$p/env0" && base64 -d < "$p/args" > "$p/args0" && base64 -d < "$p/in" > "$p/in0" || { rm -rf "$p"; echo "the run's values did not arrive" >&2; exit 1; }"#.to_string(),
        r#"while IFS= read -r -d '' k && IFS= read -r -d '' v; do export "$k=$v"; done < "$p/env0""#.to_string(),
        r#"a=(); while IFS= read -r -d '' x; do a+=("$x"); done < "$p/args0""#.to_string(),
        r#"exec 3< "$p/in0""#.to_string(),
        r#"rm -rf "$p""#.to_string(),
        format!(
            "cd {} 2>/dev/null || {{ echo {} >&2; exit 1; }}",
            shell_quote(&o.cwd),
            shell_quote(&format!("the folder {} does not exist", o.cwd))
        ),
        format!(r#"exec bash -c {} bash ${{a[@]+"${{a[@]}}"}} <&3"#, shell_quote(&o.cmd)),
    ]
    .join("\n")
}

pub fn nul_separated(items: &[String]) -> String {
    let joined: String = items.iter().map(|item| format!("{item}\0")).collect();
    STANDARD.encode(joined.as_bytes())
}

/// The machine a launch goes through: a joined computer's folder machine standing in the thread's cgroup, else the
/// machine itself (spec Decision 1).
pub fn launch_on_for(machine: &Arc<dyn Machine>, place: Option<&Arc<PlaceFolderMachine>>, thread_id: &str) -> Arc<dyn Machine> {
    match place {
        Some(place) => place.in_cgroup(&thread_cgroup(thread_id), None),
        None => machine.clone(),
    }
}

/// The slate's files as they are now.
pub type SlateFilesNow = Rc<dyn Fn() -> IndexMap<String, String>>;
/// Wakes the machine where it naps, before anything goes up.
pub type Ready = Rc<dyn Fn() -> LocalBoxFuture<'static, ()>>;

/// slate-box.ts:98 boxRoad: the thread's machine as the road its slate's commands run on.
pub struct BoxRoad {
    machine: Arc<dyn Machine>,
    dir: String,
    files: SlateFilesNow,
    ready: Ready,
    ledger: Rc<BoxLedger>,
    launch_on: Arc<dyn Machine>,
    ids: Rc<dyn Ids>,
}

/// What one start needs once it is running on its own task.
struct Job {
    machine: Arc<dyn Machine>,
    launch_on: Arc<dyn Machine>,
    dir: String,
    files: SlateFilesNow,
    ready: Ready,
    ledger: Rc<BoxLedger>,
}

pub fn box_road(
    machine: Arc<dyn Machine>,
    thread_id: &str,
    files: SlateFilesNow,
    ready: Ready,
    ledger: Rc<BoxLedger>,
    launch_on: Arc<dyn Machine>,
    ids: Rc<dyn Ids>,
) -> Rc<BoxRoad> {
    Rc::new(BoxRoad { machine, dir: box_slate_dir(thread_id), files, ready, ledger, launch_on, ids })
}

impl Job {
    /// The upload, the launch and the run's end, before the values' removal (slate-box.ts:118-160).
    async fn launch(&self, o: &RoadStart, payload: &str, cancel: &RunCancel, on: &Rc<dyn RoadEvents>) -> Result<RoadEnd, String> {
        (self.ready)().await;
        let owed = self.ledger.owed(self.machine.as_ref());
        let spared: Vec<String> = in_flight().lock().unwrap().iter().map(|p| format!("! -path {}", shell_quote(p))).collect();
        let spared = spared.join(" ");
        let dir = &self.dir;
        let mut writes: Vec<GuestWrite> = (self.files)()
            .iter()
            .map(|(name, text)| GuestWrite { path: format!("{dir}/{name}"), text: text.clone(), append: false })
            .collect();
        let env_flat: Vec<String> = o.env.iter().flat_map(|(k, v)| [k.clone(), v.clone()]).collect();
        writes.push(GuestWrite { path: format!("{payload}/env"), text: nul_separated(&env_flat), append: false });
        writes.push(GuestWrite { path: format!("{payload}/args"), text: nul_separated(&o.args), append: false });
        writes.push(GuestWrite { path: format!("{payload}/in"), text: STANDARD.encode(o.stdin.as_deref().unwrap_or("")), append: false });

        let mut before = vec!["umask 077".to_string()];
        before.extend(owed.iter().map(|path| format!("rm -rf {}", shell_quote(path))));
        before.push(format!("mkdir -p {}", shell_quote(dir)));
        before.push(format!("find {} -mindepth 1 -maxdepth 1 ! -name '.run-*' -exec rm -rf {{}} +", shell_quote(dir)));
        before.push(format!("find {} -mindepth 2 -maxdepth 2 -name '.run-*' {spared} -exec rm -rf {{}} +", shell_quote(BOX_SLATES)));
        before.push(format!("mkdir -p {q} && chmod 700 {q}", q = shell_quote(payload)));

        let upload =
            put_files(self.machine.as_ref(), &writes, PutFilesOptions { before, ..Default::default() }).await.map_err(|e| e.to_string())?;
        if upload.exit_code != 0 {
            let said = js_trim(&upload.stderr);
            let detail = if said.is_empty() { format!("exit {}", upload.exit_code) } else { said.to_string() };
            return Err(format!("the slate's files did not reach the thread's machine: {detail}"));
        }
        self.ledger.paid(self.machine.as_ref(), &owed);

        // wsp-engine's `OnLine` is an `Arc` of a closure the engine runs on one thread; the type is the engine's.
        #[allow(clippy::arc_with_non_send_sync)]
        let on_line: Option<OnLine> = if o.stream {
            let on = on.clone();
            Some(Arc::new(move |line: &str| on.line(Stream::Out, line)))
        } else {
            None
        };
        let launch = box_launcher(&Launch { payload: payload.to_string(), cwd: o.cwd.clone(), cmd: o.cmd.clone() });
        let launch_on = if Arc::ptr_eq(&self.launch_on, &self.machine) { None } else { Some(self.launch_on.clone()) };
        let res = self
            .machine
            .run(
                &launch,
                RunOptions {
                    deadline_ms: (o.timeout_s * 1_000.0) as u64,
                    on_line,
                    poll_ms: None,
                    unlogged: true,
                    signal: Some(cancel.clone()),
                    launch_on,
                },
            )
            .await
            .map_err(|e| e.to_string())?;

        let code = res.exit_code.get();
        let max = EXEC_OUTPUT_MAX as usize;
        let stdout_len = js_len(&res.stdout);
        let room = max.saturating_sub(stdout_len);
        Ok(RoadEnd {
            code: Some(code),
            signal: None,
            error: None,
            out: js_cut(&res.stdout, max),
            err: js_cut(&res.stderr, room),
            cut: stdout_len + js_len(&res.stderr) > max,
            timed_out: code == EXEC_DEADLINE_EXIT,
        })
    }
}

/// The events of a reshape: the end answers its `done` once.
struct ReshapeSink {
    answer: RefCell<Option<oneshot::Sender<ReshapeAnswer>>>,
    timeout_s: f64,
    scrub: Rc<dyn Fn(&str) -> String>,
}

impl RoadEvents for ReshapeSink {
    fn line(&self, _stream: Stream, _text: &str) {}

    fn end(&self, end: RoadEnd) {
        let tx = self.answer.borrow_mut().take();
        if let Some(tx) = tx {
            let _ = tx.send(reshape_answer(&end, self.timeout_s, &*self.scrub));
        }
    }
}

/// slate-box.ts:107-172; `start` needs a `LocalSet`.
impl RunRoad for BoxRoad {
    /// SLATE_DIR there.
    fn slate_dir(&self) -> String {
        self.dir.clone()
    }

    /// spec rule 12.
    fn start(&self, o: RoadStart, on: Rc<dyn RoadEvents>) -> RoadStarted {
        let cancel = RunCancel::new();
        let payload = format!("{}/.run-{}", self.dir, hex::encode(self.ids.bytes(6)));
        self.ledger.live.borrow_mut().insert(payload.clone());
        in_flight().lock().unwrap().insert(payload.clone());
        let job = Job {
            machine: self.machine.clone(),
            launch_on: self.launch_on.clone(),
            dir: self.dir.clone(),
            files: self.files.clone(),
            ready: self.ready.clone(),
            ledger: self.ledger.clone(),
        };
        let task_cancel = cancel.clone();
        tokio::task::spawn_local(async move {
            let end = match job.launch(&o, &payload, &task_cancel, &on).await {
                Ok(end) => end,
                Err(message) => RoadEnd { error: Some(message), ..RoadEnd::default() },
            };
            job.ledger.remove(job.machine.clone(), &payload).await;
            job.ledger.live.borrow_mut().shift_remove(&payload);
            in_flight().lock().unwrap().shift_remove(&payload);
            on.end(end);
        });
        RoadStarted { kill: Rc::new(move || cancel.cancel(None)), pid: None }
    }

    /// spec rule 13.
    fn reshape(&self, o: ReshapeStart) -> Reshape {
        let (tx, rx) = oneshot::channel();
        let sink = Rc::new(ReshapeSink { answer: RefCell::new(Some(tx)), timeout_s: o.timeout_s, scrub: o.scrub });
        let started = self.start(
            RoadStart { cmd: o.cmd, args: Vec::new(), cwd: o.cwd, env: o.env, stdin: Some(o.input), timeout_s: o.timeout_s, stream: false },
            sink,
        );
        let done = Box::pin(async move {
            rx.await.unwrap_or(ReshapeAnswer::Failed { why: "then ended with no answer".into(), exit: None, err: String::new() })
        });
        Reshape { done, kill: started.kill }
    }
}
