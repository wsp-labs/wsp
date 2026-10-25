// SPDX-License-Identifier: AGPL-3.0-only
//! Ported from packages/protocol/test/proc-entry.test.ts and daemon-ws.test.ts, with apply_proc_changes, by_proc_column
//! and pipe_open_of pinned against what the TS module printed for the same inputs: proc_frames (the process and
//! readings half of machine-link.ts, and daemon.ts's process rows).
mod common;
use common::*;
use serde_json::{json, Value};
use wsp_wire::wire::daemon::{by_proc_column, pipe_open_of, ProcEntry, ProcKind, ProcSort};
use wsp_wire::wire::daemon_events::{parse_daemon_event, DaemonEvent};
use wsp_wire::wire::machine_link::{apply_proc_changes, ProcChanges, ProcContainer, ProcSnapshot, WorkspaceSysEvent};

fn row(pid: i64, cpu: f64, rss: f64) -> Value {
    json!({ "pid": pid, "ppid": 1, "user": "u", "state": "S", "comm": format!("c{pid}"), "cmdline": "x", "cpu": cpu, "rss": rss, "startedAt": 1 })
}

fn snapshot_of(v: Value) -> ProcSnapshot {
    match parse_daemon_event(&v).unwrap() {
        DaemonEvent::ProcSnapshot(s) => s,
        other => panic!("{other:?}"),
    }
}

fn changes_of(v: Value) -> ProcChanges {
    match parse_daemon_event(&v).unwrap() {
        DaemonEvent::ProcChanges(c) => c,
        other => panic!("{other:?}"),
    }
}

fn base_snapshot() -> Value {
    json!({ "type": "proc.snapshot", "at": 1, "daemon": 9, "total": 3, "procs": [row(5, 0.0, 1.0), row(2, 0.0, 1.0), row(9, 0.0, 1.0)], "seq": 4,
            "cost": { "cpuMs": 1, "held": 10 }, "containers": [{ "id": "a", "name": "n", "image": "i", "state": "running" }], "cgroups": [{ "cgroup": "/x", "memory": 5 }] })
}

fn base_changes() -> Value {
    json!({ "type": "proc.changes", "at": 2, "daemon": 9, "total": 4, "procs": [row(2, 50.0, 1.0), row(7, 0.0, 1.0)], "gone": [9, 100], "seq": 5, "base": 4 })
}

fn as_event(s: ProcSnapshot) -> Value {
    serde_json::to_value(DaemonEvent::ProcSnapshot(s)).unwrap()
}

mod a_process_row_from_a_newer_daemon {
    use super::*;

    // proc-entry.test.ts:10. A row that fails its schema drops the whole frame it rides in, so a word this version
    // does not know reads as other, and an owner rung it does not know as no owner.
    #[test]
    fn reads_a_kind_it_does_not_know_as_other_and_an_owner_rung_it_does_not_know_as_no_owner() {
        let mut v = row(7, 0.0, 1.0);
        v["kind"] = json!("container");
        v["owner"] = json!({ "by": "docker", "thread": "t1" });
        let read: ProcEntry = reads_as(
            v,
            json!({ "pid": 7, "ppid": 1, "user": "u", "state": "S", "comm": "c7", "cmdline": "x", "cpu": 0, "rss": 1, "startedAt": 1, "kind": "other" }),
        );
        assert_eq!(read.kind, Some(ProcKind::Other));
        assert!(read.owner.is_none());
    }

    // proc-entry.test.ts:16
    #[test]
    fn keeps_what_it_knows_a_warm_agent_s_owner_included() {
        let mut v = row(7, 0.0, 1.0);
        v["kind"] = json!("slate");
        v["owner"] = json!({ "by": "env", "thread": "t1", "workspace": "ws_1", "warm": true });
        let read: ProcEntry = reads(v);
        assert_eq!(read.kind, Some(ProcKind::Slate));
        let owner = read.owner.unwrap();
        assert_eq!((owner.thread.as_deref(), owner.workspace.as_deref(), owner.warm), (Some("t1"), Some("ws_1"), Some(true)));
    }

    // Printed by the TS module: catch("other") catches null and a number too, and catch(undefined) a null owner; an
    // absent kind stays absent.
    #[test]
    fn a_null_or_wrong_typed_kind_reads_as_other_and_an_absent_one_stays_absent() {
        let mut v = row(1, 0.0, 1.0);
        v["kind"] = Value::Null;
        v["owner"] = Value::Null;
        let read: ProcEntry = reads_as(
            v,
            json!({ "pid": 1, "ppid": 1, "user": "u", "state": "S", "comm": "c1", "cmdline": "x", "cpu": 0, "rss": 1, "startedAt": 1, "kind": "other" }),
        );
        assert_eq!((read.kind, read.owner.is_none()), (Some(ProcKind::Other), true));
        let mut v = row(1, 0.0, 1.0);
        v["kind"] = json!(5);
        assert_eq!(reads_as::<ProcEntry>(v, json!({ "pid": 1, "ppid": 1, "user": "u", "state": "S", "comm": "c1", "cmdline": "x", "cpu": 0, "rss": 1, "startedAt": 1, "kind": "other" })).kind, Some(ProcKind::Other));
        assert_eq!(reads::<ProcEntry>(row(1, 0.0, 1.0)).kind, None);
    }

    // machine-link.ts:647: a container placed by a word a newer daemon adds reads as unplaced, the row kept.
    #[test]
    fn a_container_placed_by_a_word_it_does_not_know_reads_as_unplaced() {
        let read: ProcContainer = reads_as(
            json!({ "id": "a", "name": "n", "image": "i", "state": "s", "placedBy": "fork" }),
            json!({ "id": "a", "name": "n", "image": "i", "state": "s" }),
        );
        assert!(read.placed_by.is_none());
    }

    // daemon.ts:359: pid, ppid and the frame's daemon and total are integers; a float refuses the frame at that field.
    #[test]
    fn integer_fields_refuse_a_fraction_and_plain_numbers_keep_one() {
        let e = refused_with(parse_daemon_event, json!({ "type": "proc.snapshot", "at": 1, "daemon": 1.5, "total": 0, "procs": [] }));
        assert_eq!(e.path, path!["daemon"]);
        let read = snapshot_of(json!({ "type": "proc.snapshot", "at": 1.25, "daemon": 1, "total": 0, "procs": [] }));
        assert_eq!(read.at.0, 1.25);
        let e = refused_with(
            parse_daemon_event,
            json!({ "type": "proc.snapshot", "at": 1, "daemon": 1, "total": 0, "procs": [{ "pid": 1.5, "ppid": 1, "user": "u", "state": "S", "comm": "c", "cmdline": "x", "cpu": 0, "rss": 1, "startedAt": 1 }] }),
        );
        assert_eq!(e.path, path!["procs", 0, "pid"]);
    }
}

mod apply_proc_changes_leaves_the_snapshot_the_changes_describe {
    use super::*;

    // machine-link.ts:696-702, printed by the TS module: gone rows leave, new and changed rows land whole, rows sort
    // by pid, the frame's own at, total, seq and daemon stand; the snapshot's containers stay where the changes carry
    // none; cost and cgroups are the changes' or nothing, never an older tick's.
    #[test]
    fn rows_by_pid_and_the_containers_held_where_the_changes_carry_none() {
        let next = apply_proc_changes(&snapshot_of(base_snapshot()), &changes_of(base_changes())).unwrap();
        let want = json!({ "type": "proc.snapshot", "at": 2, "daemon": 9, "total": 4,
            "procs": [row(2, 50.0, 1.0), row(5, 0.0, 1.0), row(7, 0.0, 1.0)], "seq": 5,
            "containers": [{ "id": "a", "name": "n", "image": "i", "state": "running" }] });
        assert_json_eq(&as_event(next.clone()), &want, "the snapshot the changes leave");
        assert_eq!(serde_json::to_string(&DaemonEvent::ProcSnapshot(next)).unwrap(), serde_json::to_string(&as_js(&want)).unwrap());
    }

    #[test]
    fn the_changes_own_cost_containers_and_cgroups_replace_the_snapshot_s_even_empty() {
        let mut changes = base_changes();
        changes["cost"] = json!({ "cpuMs": 2, "held": 3 });
        changes["containers"] = json!([]);
        changes["cgroups"] = json!([]);
        let next = apply_proc_changes(&snapshot_of(base_snapshot()), &changes_of(changes)).unwrap();
        assert_json_eq(
            &as_event(next),
            &json!({ "type": "proc.snapshot", "at": 2, "daemon": 9, "total": 4,
            "procs": [row(2, 50.0, 1.0), row(5, 0.0, 1.0), row(7, 0.0, 1.0)], "seq": 5,
            "cost": { "cpuMs": 2, "held": 3 }, "containers": [], "cgroups": [] }),
            "the changes' own cost, containers and cgroups",
        );
    }

    #[test]
    fn changes_that_do_not_follow_the_snapshot_held_leave_nothing() {
        let mut stale = base_changes();
        stale["base"] = json!(3);
        assert!(apply_proc_changes(&snapshot_of(base_snapshot()), &changes_of(stale)).is_none());
        // A snapshot from a daemon a version behind names no seq, so no changes follow it.
        let mut unnumbered = base_snapshot();
        unnumbered.as_object_mut().unwrap().shift_remove("seq");
        assert!(apply_proc_changes(&snapshot_of(unnumbered), &changes_of(base_changes())).is_none());
    }

    // A pid both gone and changed in one frame lands: gone is applied first, then the rows.
    #[test]
    fn a_pid_named_gone_and_changed_in_one_frame_stands_as_changed() {
        let mut changes = base_changes();
        changes["gone"] = json!([2]);
        let next = apply_proc_changes(&snapshot_of(base_snapshot()), &changes_of(changes)).unwrap();
        let pids: Vec<i64> = next.procs.iter().map(|p| p.pid.get()).collect();
        assert_eq!(pids, vec![2, 5, 7, 9]);
        assert_eq!(next.procs[0].cpu.0, 50.0);
    }
}

// daemon.ts:414-419, printed by the TS module: heaviest first, ties by pid ascending.
#[test]
fn by_proc_column_orders_heaviest_first_and_ties_by_pid() {
    let rows: Vec<ProcEntry> = [row(3, 5.0, 10.0), row(1, 5.0, 30.0), row(2, 9.0, 20.0)].into_iter().map(reads::<ProcEntry>).collect();
    let mut by_cpu = rows.clone();
    by_cpu.sort_by(|a, b| by_proc_column(ProcSort::Cpu, a, b));
    assert_eq!(by_cpu.iter().map(|p| p.pid.get()).collect::<Vec<_>>(), vec![2, 1, 3]);
    let mut by_mem = rows;
    by_mem.sort_by(|a, b| by_proc_column(ProcSort::Mem, a, b));
    assert_eq!(by_mem.iter().map(|p| p.pid.get()).collect::<Vec<_>>(), vec![1, 2, 3]);
}

// daemon-ws.test.ts:388 and daemon.ts:99-108, printed by the TS module.
#[test]
fn pipe_open_of_reads_the_one_text_frame_a_pipe_opens_with_and_nothing_else() {
    let open = pipe_open_of(r#"{"op":"pipe.open","door":"gh","heldMs":3}"#).unwrap();
    assert_eq!(serde_json::to_value(&open).unwrap(), json!({ "op": "pipe.open", "door": "gh", "heldMs": 3 }));
    // A daemon that serves no pipes greets the socket with its hello, which is no pipe.
    assert!(pipe_open_of(r#"{"type":"daemon.hello","root":"/r","version":3}"#).is_none());
    assert!(pipe_open_of("nope").is_none());
    assert!(pipe_open_of(r#"{"op":"pipe.open","door":"gh","heldMs":-1}"#).is_none());
    assert!(pipe_open_of(r#"{"op":"pipe.open","door":"ssh","heldMs":0}"#).is_none());
    let extra = pipe_open_of(r#"{"op":"pipe.open","door":"http","heldMs":0,"extra":1}"#).unwrap();
    assert_eq!(serde_json::to_value(&extra).unwrap(), json!({ "op": "pipe.open", "door": "http", "heldMs": 0 }));
}

// machine-link.ts:617: the host's own reading rides workspace.sys with the daemon's sample inside it, type and all.
#[test]
fn a_workspace_reading_carries_the_sample_with_its_own_type() {
    let v = json!({ "type": "workspace.sys", "workspaceId": "ws_1", "sample": { "type": "sys.sample", "cpu": 12.5, "load1": 0.5, "mem": { "used": 1, "total": 2 }, "disk": { "used": 3, "total": 4 }, "at": 5 } });
    let read: WorkspaceSysEvent = reads_exact(v);
    assert_eq!(read.workspace_id, "ws_1");
    let mut no_tag = json!({ "type": "workspace.sys", "workspaceId": "ws_1", "sample": { "cpu": 1, "load1": 0, "mem": { "used": 1, "total": 2 }, "disk": { "used": 3, "total": 4 }, "at": 5 } });
    assert_eq!(refused::<WorkspaceSysEvent>(no_tag.take()).path, path!["sample", "type"]);
}
