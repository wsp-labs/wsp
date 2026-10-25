// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/slate-box.ts at 65f6b89c7. Spec: spec/slate_box.md (rules 9-13).
//!
//! A slate's command on the thread's own machine through the engine's `Machine`: the slate's files and the command's
//! values go up first, a fixed launcher decodes the values, deletes them, and runs the agent's text as `bash -c`'s one
//! argument with the values as its environment, positional parameters and stdin. The ledger keeps what this host
//! still owes each box.
#![allow(unused_variables, dead_code)]

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use futures::future::LocalBoxFuture;
use indexmap::{IndexMap, IndexSet};

pub use wsp_engine::machine::Machine;
pub use wsp_engine::place_machine::PlaceFolderMachine;

use wsp_core::clock::{Cancel, Clock};
use wsp_core::ids::Ids;

use crate::slate_runs::{Reshape, ReshapeStart, RoadEvents, RoadStart, RoadStarted, RunRoad};

/// How long a removal a box did not answer waits before it is tried again (slate-box.ts:16).
pub const OWED_RETRY_MS: i64 = 60_000;

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

// slate-box.ts:30 boxLedger(retryMs)
pub fn box_ledger(retry_ms: Option<i64>, clock: Rc<dyn Clock>) -> Rc<BoxLedger> {
    todo!()
}

impl BoxLedger {
    /// Removes a path there, answering whether the box said it is gone; until it does, the removal is owed.
    pub async fn remove(self: &Rc<Self>, machine: Arc<dyn Machine>, path: &str) -> bool {
        todo!()
    }

    /// The owed paths of this machine, oldest first.
    pub fn owed(&self, machine: &dyn Machine) -> Vec<String> {
        todo!()
    }

    /// The box answered for these removals at a contact of its own.
    pub fn paid(&self, machine: &dyn Machine, paths: &[String]) {
        todo!()
    }

    pub fn close(&self) {
        todo!()
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

// slate-box.ts:79 boxLauncher(o)
/// The script the machine runs for one command (spec rule 10).
pub fn box_launcher(o: &Launch) -> String {
    todo!()
}

// slate-box.ts:76 nulSeparated(items)
pub fn nul_separated(items: &[String]) -> String {
    todo!()
}

// slate-box.ts:104 the default launchOn
/// The machine a launch goes through: a joined computer's folder machine standing in the thread's cgroup, else the
/// machine itself (spec Decision 1).
pub fn launch_on_for(machine: &Arc<dyn Machine>, place: Option<&Arc<PlaceFolderMachine>>, thread_id: &str) -> Arc<dyn Machine> {
    todo!()
}

/// The slate's files as they are now.
pub type SlateFilesNow = Rc<dyn Fn() -> IndexMap<String, String>>;
/// Wakes the machine where it naps, before anything goes up.
pub type Ready = Rc<dyn Fn() -> LocalBoxFuture<'static, ()>>;

/// slate-box.ts:98 boxRoad: the thread's machine as the road its slate's commands run on.
pub struct BoxRoad {
    machine: Arc<dyn Machine>,
    thread_id: String,
    dir: String,
    files: SlateFilesNow,
    ready: Ready,
    ledger: Rc<BoxLedger>,
    launch_on: Arc<dyn Machine>,
    ids: Rc<dyn Ids>,
}

// slate-box.ts:98 boxRoad(machine, threadId, files, ready, ledger, launchOn)
pub fn box_road(machine: Arc<dyn Machine>, thread_id: &str, files: SlateFilesNow, ready: Ready, ledger: Rc<BoxLedger>, launch_on: Arc<dyn Machine>, ids: Rc<dyn Ids>) -> Rc<BoxRoad> {
    todo!()
}

/// slate-box.ts:107-172; `start` needs a `LocalSet`.
impl RunRoad for BoxRoad {
    /// SLATE_DIR there.
    fn slate_dir(&self) -> String {
        todo!()
    }

    /// spec rule 12.
    fn start(&self, o: RoadStart, on: Rc<dyn RoadEvents>) -> RoadStarted {
        todo!()
    }

    /// spec rule 13.
    fn reshape(&self, o: ReshapeStart) -> Reshape {
        todo!()
    }
}
