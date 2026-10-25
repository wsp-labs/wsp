// SPDX-License-Identifier: AGPL-3.0-only
//! The live entry both halves of the tests build, over a machine the half's own fake supplies: what `attach` makes.
//! The engine's `Workspace::new` is a stub today, so building an entry stops there until it lands; the test is red
//! either way.
#![allow(dead_code)]

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use wsp_core::types::wiring::{LiveWorkspace, WorkspaceRecord};
use wsp_engine::lifecycle::{Workspace, WorkspaceHooks, WorkspaceInitial};
use wsp_engine::machine::Machine;
use wsp_threads::threads::threads::Entry;

/// A live entry over a record and a machine, every in-memory field at its start.
pub fn entry_over(record: WorkspaceRecord, machine: Arc<dyn Machine>) -> Entry {
    let hooks = WorkspaceHooks {
        golden_snapshot: String::new(),
        wake_attempts: 1,
        resurrect: None,
        vault_export: None,
        vault_import: None,
        stash_vault: None,
        restore_vault: None,
        wake_check: None,
        move_hook: None,
        retire: None,
    };
    let ws = Rc::new(Workspace::new(machine.clone(), hooks, WorkspaceInitial::default()));
    Rc::new(RefCell::new(LiveWorkspace {
        record: Rc::new(RefCell::new(record)),
        opens_thread: None,
        of_record_held: false,
        ws,
        machine,
        generation: 0,
        waking: None,
        unchecked: false,
        napping: None,
        adopting: None,
        deleting: None,
        late_read: None,
        wake_stop: None,
        resize_to: None,
        nap_wait_ms: None,
        wake_said: None,
        delete_said: None,
        wake_ask: None,
        creating: false,
        checkout: None,
        checkout_reading: None,
        tree: None,
        tree_reading: None,
        pr: None,
        pr_poll: None,
        pr_reading: None,
        pr_poll_ms: None,
        pr_none_at: None,
    }))
}
