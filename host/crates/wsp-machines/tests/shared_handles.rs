// SPDX-License-Identifier: AGPL-3.0-only
//! The maps the machines area keeps and other areas of the runtime must reach are `Rc<RefCell<..>>` handles on the
//! area itself, so the one the runtime hands out is the one the area writes: `MachinesArea.rereading`,
//! `ReachArea.nap_refusals` and `stop_refusals`, `DaemonArea.revived_at` and `unread_at`.
mod common;
#[path = "support/daemon.rs"]
mod daemon_support;
mod machines_b_support;
#[path = "support/machines.rs"]
mod machines_support;

use std::cell::RefCell;
use std::rc::Rc;

use indexmap::{IndexMap, IndexSet};
use serde_json::json;

use machines_b_support::reach_ctx::*;
use wsp_core::types::internal::MachineMoment;
use wsp_machines::machines::reach::ForMachine;
use wsp_wire::types::js_number::JsNumber;

type Refusals = Rc<RefCell<IndexMap<String, ForMachine<String>>>>;

fn held(machine: &str, said: &str) -> ForMachine<String> {
    ForMachine {
        machine_id: machine.into(),
        value: said.into(),
    }
}

#[test]
fn the_reach_areas_refusals_are_handles_the_runtime_can_hold() {
    common::run_local(async {
        let host = reach_host();
        let area = reach(&host);
        let nap: Refusals = area.nap_refusals.clone();
        let stop: Refusals = area.stop_refusals.clone();
        nap.borrow_mut()
            .insert("ws_1".into(), held("bx_1", "no pause"));
        stop.borrow_mut()
            .insert("ws_1".into(), held("bx_1", "no stop"));
        assert_eq!(
            area.nap_refusals.borrow().get("ws_1"),
            Some(&held("bx_1", "no pause"))
        );
        assert_eq!(
            area.stop_refusals.borrow().get("ws_1"),
            Some(&held("bx_1", "no stop"))
        );
        area.nap_refusals.borrow_mut().shift_remove("ws_1");
        assert!(
            nap.borrow().is_empty(),
            "a delete through the area is seen through the handle"
        );
    });
}

#[test]
fn the_machines_area_hands_out_the_set_it_rereads_by() {
    common::run_local(async {
        let w = machines_support::World::new(machines_support::Opts::default());
        let handle: Rc<RefCell<IndexSet<String>>> = w.area.rereading_held();
        handle.borrow_mut().insert("ws_1".into());
        assert_eq!(w.area.rereading(), vec!["ws_1".to_string()]);
        w.area.rereading_held().borrow_mut().shift_remove("ws_1");
        assert!(handle.borrow().is_empty());
    });
}

#[test]
fn the_daemon_area_hands_out_its_revived_and_unread_marks() {
    common::run_local(async {
        let w = daemon_support::World::new(&json!({"entries": []}));
        let moment = MachineMoment {
            machine_id: "m1".into(),
            at: JsNumber(5.0),
        };
        let revived = w.area.revived_at_held();
        revived.borrow_mut().insert("a".into(), moment.clone());
        assert_eq!(w.area.revived_at().get("a"), Some(&moment));
        let unread = w.area.unread_at();
        unread.borrow_mut().insert("a".into(), moment.clone());
        w.area.unread_at().borrow_mut().shift_remove("a");
        assert!(unread.borrow().is_empty());
        w.area.revived_at_held().borrow_mut().shift_remove("a");
        assert!(w.area.revived_at().is_empty());
    });
}
