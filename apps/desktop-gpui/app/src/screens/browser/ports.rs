// SPDX-License-Identifier: AGPL-3.0-only
//! The daemon's listening ports into the browser pane's servers list (`adapt/ports.ts`, `browser/model.ts`): the
//! ports.watch reply seeds the set and port.open and port.close keep it current, remembering why a port stopped.

use std::collections::BTreeMap;

use serde_json::Value;

/// `KnownPort`: a port the daemon says listens; the process is its /proc comm, none when it could not read one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KnownPort {
    pub port: u16,
    pub pid: Option<u32>,
    pub process: Option<String>,
    /// Only this computer can reach it; none where the daemon did not say.
    pub loopback: Option<bool>,
}

/// The daemon's `port.open` and `port.close`, as the daemon link and the runtime stream both send them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PortEvent {
    Open {
        port: u16,
        pid: Option<u32>,
        process: Option<String>,
        loopback: Option<bool>,
    },
    Close {
        port: u16,
        pid: Option<u32>,
        process: Option<String>,
        command: Option<String>,
        exited: Option<bool>,
        left: bool,
        at: Option<String>,
    },
}

impl PortEvent {
    pub fn from_json(event: &Value) -> Option<PortEvent> {
        let port = u16::try_from(event.get("port")?.as_u64()?).ok()?;
        let pid = pid_of(event.get("pid"));
        let text = |key: &str| event.get(key).and_then(Value::as_str).map(str::to_owned);
        let flag = |key: &str| event.get(key).and_then(Value::as_bool);
        match event.get("type")?.as_str()? {
            "port.open" => Some(PortEvent::Open { port, pid, process: text("process"), loopback: flag("loopback") }),
            "port.close" => Some(PortEvent::Close {
                port,
                pid,
                process: text("process"),
                command: text("command"),
                exited: flag("exited"),
                left: flag("left") == Some(true),
                at: text("at"),
            }),
            _ => None,
        }
    }

    pub fn port(&self) -> u16 {
        match self {
            PortEvent::Open { port, .. } | PortEvent::Close { port, .. } => *port,
        }
    }
}

fn pid_of(value: Option<&Value>) -> Option<u32> {
    value.and_then(Value::as_u64).and_then(|pid| u32::try_from(pid).ok())
}

/// `StoppedPort`: what is known about a port that stopped listening, from the close event that said so.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoppedPort {
    pub port: u16,
    pub pid: Option<u32>,
    pub process: Option<String>,
    pub command: Option<String>,
    pub exited: Option<bool>,
    /// It still listens, held by a process that is no longer this workspace's.
    pub left: bool,
    /// The daemon's clock at the close, for the sentence.
    pub at: Option<String>,
    /// This computer's clock when the close arrived, in ms; the moved window is measured on it, not the guest's.
    pub seen_at: i64,
    /// Another port the same process opened within the window after this one stopped.
    pub moved_to: Option<u16>,
}

pub const MOVED_WINDOW_MS: i64 = 60_000;

/// `applyPortsSnapshot`: the rows of a ports.watch reply, sorted, the first row of a port winning.
pub fn apply_ports_snapshot(rows: &[KnownPort]) -> Vec<KnownPort> {
    let mut by_port = BTreeMap::new();
    for row in rows {
        by_port.entry(row.port).or_insert_with(|| row.clone());
    }
    by_port.into_values().collect()
}

/// `applyPortEvent`: a re-open of a known port keeps the first pid, a close of an unknown port is a no-op.
pub fn apply_port_event(ports: &[KnownPort], event: &PortEvent) -> Vec<KnownPort> {
    match event {
        PortEvent::Open { port, .. } if ports.iter().any(|p| p.port == *port) => ports.to_vec(),
        PortEvent::Open { port, pid, process, loopback } => {
            let mut next = ports.to_vec();
            next.push(KnownPort { port: *port, pid: *pid, process: process.clone(), loopback: *loopback });
            next.sort_by_key(|p| p.port);
            next
        }
        PortEvent::Close { port, .. } => ports.iter().filter(|p| p.port != *port).cloned().collect(),
    }
}

/// `applyStoppedEvent`: folds one port event into the stopped ports; false when nothing changed. A close always
/// changes them, since it stamps a fresh arrival.
pub fn apply_stopped_event(stopped: &mut BTreeMap<u16, StoppedPort>, event: &PortEvent, now_ms: i64) -> bool {
    match event {
        PortEvent::Close { port, pid, process, command, exited, left, at } => {
            let row = StoppedPort {
                port: *port,
                pid: *pid,
                process: process.clone(),
                command: command.clone(),
                exited: *exited,
                left: *left,
                at: at.clone(),
                seen_at: now_ms,
                moved_to: None,
            };
            stopped.insert(*port, row);
            true
        }
        PortEvent::Open { port, process, .. } => {
            let mut changed = stopped.remove(port).is_some();
            for s in stopped.values_mut() {
                if process.is_some() && s.process == *process && now_ms - s.seen_at <= MOVED_WINDOW_MS {
                    s.moved_to = Some(*port);
                    changed = true;
                }
            }
            changed
        }
    }
}

/// `stoppedSentence`: the line above a frame whose port does not answer on its computer, or whose holder is no longer
/// this workspace's; clock renders the daemon's timestamp in the person's zone. With fetched_down, a fetch of the port
/// just failed, so a holder that left is not said to still listen.
pub fn stopped_sentence(
    port: u16,
    computer: &str,
    stopped: Option<&StoppedPort>,
    clock: impl Fn(&str) -> String,
    fetched_down: bool,
) -> String {
    let down = format!(":{port} on {computer} is not answering; this tab reconnects when it does");
    let Some(stopped) = stopped else {
        return down;
    };
    let mut detail = stopped.at.as_deref().map(|at| format!(" at {}", clock(at))).unwrap_or_default();
    let holder = stopped.command.as_ref().or(stopped.process.as_ref());
    let pid = stopped.pid.map(|pid| format!("pid {pid}"));
    let who = match (holder, pid) {
        (Some(holder), Some(pid)) => Some(format!("{holder} ({pid})")),
        (Some(holder), None) => Some(holder.clone()),
        (None, pid) => pid,
    };
    if let Some(who) = who {
        detail += &format!(", held by {who}");
        if stopped.left {
            if !fetched_down {
                detail += ", which still listens";
            }
        } else if stopped.exited == Some(true) {
            detail += ", which exited";
        } else if stopped.exited == Some(false) {
            detail += ", which is still running";
        }
    }
    if let Some(moved) = stopped.moved_to {
        detail += &format!(", now on :{moved}");
    }
    if stopped.left {
        format!(":{port} is no longer this workspace's{detail}")
    } else if detail.is_empty() {
        down
    } else {
        format!("{down}. It stopped listening{detail}")
    }
}

/// `WorkspacePorts`: one workspace's ports as the daemon last said them and why each stopped one stopped. Each change
/// answers whether anything a pane reads changed, so the caller knows when to notify.
#[derive(Clone, Debug, Default)]
pub struct Directory {
    ports: Vec<KnownPort>,
    stopped: BTreeMap<u16, StoppedPort>,
    seeded: bool,
}

impl Directory {
    /// Sorted by port.
    pub fn ports(&self) -> &[KnownPort] {
        &self.ports
    }

    /// What the last close said about a port that is not listening; none once it listens again.
    pub fn stopped(&self, port: u16) -> Option<&StoppedPort> {
        self.stopped.get(&port)
    }

    /// False until the daemon has said anything about ports: an empty directory then means unknown, not silent.
    pub fn seeded(&self) -> bool {
        self.seeded
    }

    pub fn feed_event(&mut self, event: &PortEvent, now_ms: i64) -> bool {
        let next = apply_port_event(&self.ports, event);
        let stopped = apply_stopped_event(&mut self.stopped, event, now_ms);
        self.adopt(next, stopped)
    }

    /// Adopts the daemon's full listening set from a ports.watch reply. Anything that is not an array of port rows is
    /// ignored so a bad reply cannot empty a directory the pushes built.
    pub fn sync_ports(&mut self, reply: &Value) -> bool {
        match snapshot_rows(reply) {
            Some(rows) => self.adopt(apply_ports_snapshot(&rows), false),
            None => false,
        }
    }

    fn adopt(&mut self, next: Vec<KnownPort>, stopped_changed: bool) -> bool {
        let first = !self.seeded;
        self.seeded = true;
        let same = same_directory(&self.ports, &next);
        if !same {
            self.ports = next;
        }
        first || !same || stopped_changed
    }
}

/// `sameDirectory`: whether loopback changed alone does not count, as today.
fn same_directory(a: &[KnownPort], b: &[KnownPort]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(p, q)| p.port == q.port && p.pid == q.pid && p.process == q.process)
}

/// `snapshotRows`: none for anything but an array of objects each with a port.
fn snapshot_rows(reply: &Value) -> Option<Vec<KnownPort>> {
    reply
        .as_array()?
        .iter()
        .map(|row| {
            let row = row.as_object()?;
            Some(KnownPort {
                port: u16::try_from(row.get("port")?.as_u64()?).ok()?,
                pid: pid_of(row.get("pid")),
                process: row.get("process").and_then(Value::as_str).map(str::to_owned),
                loopback: row.get("loopback").and_then(Value::as_bool),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn known(port: u16, pid: Option<u32>, process: Option<&str>) -> KnownPort {
        KnownPort { port, pid, process: process.map(str::to_owned), loopback: None }
    }

    fn event(value: Value) -> PortEvent {
        PortEvent::from_json(&value).unwrap()
    }

    #[test]
    fn the_snapshot_seeds_sorted_and_the_first_pid_wins_on_a_duplicate() {
        let rows = [known(5173, Some(40), Some("node")), known(3000, None, None), known(5173, Some(41), None)];
        assert_eq!(apply_ports_snapshot(&rows), [known(3000, None, None), known(5173, Some(40), Some("node"))]);
    }

    #[test]
    fn each_port_event_folds_into_the_list() {
        let cases = [
            (
                "daemon port.open adds with the process name",
                vec![],
                json!({"type": "port.open", "port": 3000, "pid": 7, "process": "node"}),
                vec![known(3000, Some(7), Some("node"))],
            ),
            (
                "runtime port.open adds too",
                vec![],
                json!({"type": "port.open", "workspaceId": "ws", "port": 3000}),
                vec![known(3000, None, None)],
            ),
            (
                "re-open keeps the first pid",
                vec![known(3000, Some(7), None)],
                json!({"type": "port.open", "port": 3000, "pid": 9}),
                vec![known(3000, Some(7), None)],
            ),
            (
                "close removes",
                vec![known(3000, Some(7), None), known(5173, None, None)],
                json!({"type": "port.close", "port": 3000}),
                vec![known(5173, None, None)],
            ),
            (
                "close of an unknown port changes nothing",
                vec![known(3000, Some(7), None)],
                json!({"type": "port.close", "workspaceId": "ws", "port": 81}),
                vec![known(3000, Some(7), None)],
            ),
            (
                "open keeps the list sorted",
                vec![known(5173, None, None)],
                json!({"type": "port.open", "port": 80}),
                vec![known(80, None, None), known(5173, None, None)],
            ),
        ];
        for (name, before, raw, after) in cases {
            assert_eq!(apply_port_event(&before, &event(raw)), after, "{name}");
        }
    }

    #[test]
    fn an_event_reads_from_the_daemons_json_and_anything_else_is_none() {
        let open = event(json!({"type": "port.open", "port": 3000, "pid": 7, "process": "node", "loopback": true}));
        assert_eq!(open, PortEvent::Open { port: 3000, pid: Some(7), process: Some("node".into()), loopback: Some(true) });
        let close = event(
            json!({"type": "port.close", "port": 8412, "command": "python3 -m http.server", "exited": false, "left": true, "at": "t"}),
        );
        assert_eq!(
            close,
            PortEvent::Close {
                port: 8412,
                pid: None,
                process: None,
                command: Some("python3 -m http.server".into()),
                exited: Some(false),
                left: true,
                at: Some("t".into())
            }
        );
        for raw in [
            json!({"type": "port.open"}),
            json!({"type": "port.moved", "port": 1}),
            json!({"type": "port.open", "port": 70000}),
            json!(null),
        ] {
            assert_eq!(PortEvent::from_json(&raw), None, "{raw}");
        }
    }

    const AT: &str = "2026-09-05T12:04:00.000Z";
    const T0: i64 = 1_000_000;

    fn close_held() -> PortEvent {
        event(
            json!({"type": "port.close", "port": 8412, "pid": 53479, "process": "python3", "command": "python3 -m http.server 8412", "exited": true, "at": AT}),
        )
    }

    fn held_row() -> StoppedPort {
        StoppedPort {
            port: 8412,
            pid: Some(53479),
            process: Some("python3".into()),
            command: Some("python3 -m http.server 8412".into()),
            exited: Some(true),
            left: false,
            at: Some(AT.into()),
            seen_at: T0,
            moved_to: None,
        }
    }

    fn stopped_after(events: &[(PortEvent, i64)]) -> (BTreeMap<u16, StoppedPort>, bool) {
        let mut stopped = BTreeMap::new();
        let mut changed = false;
        for (e, at) in events {
            changed = apply_stopped_event(&mut stopped, e, *at);
        }
        (stopped, changed)
    }

    #[test]
    fn a_close_records_who_held_the_port_and_when_stamped_with_the_local_clock() {
        let (stopped, changed) = stopped_after(&[(close_held(), T0)]);
        assert!(changed);
        assert_eq!(stopped.into_values().collect::<Vec<_>>(), [held_row()]);
    }

    #[test]
    fn a_plain_close_records_the_port_with_nothing_known_about_its_holder() {
        let (stopped, _) = stopped_after(&[(event(json!({"type": "port.close", "workspaceId": "ws", "port": 3000})), T0)]);
        let empty = StoppedPort {
            port: 3000,
            pid: None,
            process: None,
            command: None,
            exited: None,
            left: false,
            at: None,
            seen_at: T0,
            moved_to: None,
        };
        assert_eq!(stopped.get(&3000), Some(&empty));
    }

    #[test]
    fn the_same_port_opening_again_forgets_the_stop_and_an_unrelated_open_changes_nothing() {
        let (stopped, changed) =
            stopped_after(&[(close_held(), T0), (event(json!({"type": "port.open", "port": 5173, "process": "node"})), T0 + 1)]);
        assert!(!changed);
        assert_eq!(stopped.len(), 1);
        let (stopped, changed) = stopped_after(&[
            (close_held(), T0),
            (event(json!({"type": "port.open", "port": 8412, "pid": 60000, "process": "python3"})), T0 + 1),
        ]);
        assert!(changed);
        assert!(stopped.is_empty());
    }

    #[test]
    fn the_same_process_opening_another_port_within_a_minute_marks_where_it_went() {
        let moved_open = || event(json!({"type": "port.open", "port": 8413, "pid": 60000, "process": "python3"}));
        let (stopped, changed) = stopped_after(&[(close_held(), T0), (moved_open(), T0 + MOVED_WINDOW_MS)]);
        assert!(changed);
        assert_eq!(stopped.get(&8412), Some(&StoppedPort { moved_to: Some(8413), ..held_row() }));
        let unchanged = [
            (moved_open(), T0 + MOVED_WINDOW_MS + 1),
            (event(json!({"type": "port.open", "port": 8413, "pid": 60000, "process": "node"})), T0 + 1),
            (event(json!({"type": "port.open", "port": 8413})), T0 + 1),
        ];
        for (open, at) in unchanged {
            let (stopped, changed) = stopped_after(&[(close_held(), T0), (open.clone(), at)]);
            assert!(!changed, "{open:?}");
            assert_eq!(stopped.get(&8412), Some(&held_row()));
        }
    }

    fn base() -> StoppedPort {
        StoppedPort { seen_at: 0, ..held_row() }
    }

    const DOWN: &str = ":8412 on acme-box is not answering; this tab reconnects when it does";

    fn sentence(stopped: Option<&StoppedPort>, fetched_down: bool) -> String {
        stopped_sentence(8412, "acme-box", stopped, |_| "12:04".to_owned(), fetched_down)
    }

    #[test]
    fn the_stopped_sentence_names_what_is_known_about_the_holder() {
        let time_only = StoppedPort { pid: None, process: None, command: None, exited: None, ..base() };
        let cases = [
            ("nothing known", None, DOWN.to_owned()),
            ("time only", Some(time_only), format!("{DOWN}. It stopped listening at 12:04")),
            (
                "holder that exited",
                Some(base()),
                format!("{DOWN}. It stopped listening at 12:04, held by python3 -m http.server 8412 (pid 53479), which exited"),
            ),
            (
                "holder still running",
                Some(StoppedPort { exited: Some(false), ..base() }),
                format!("{DOWN}. It stopped listening at 12:04, held by python3 -m http.server 8412 (pid 53479), which is still running"),
            ),
            (
                "comm when the argv is unknown",
                Some(StoppedPort { command: None, ..base() }),
                format!("{DOWN}. It stopped listening at 12:04, held by python3 (pid 53479), which exited"),
            ),
            (
                "pid when neither comm nor argv is known",
                Some(StoppedPort { process: None, command: None, ..base() }),
                format!("{DOWN}. It stopped listening at 12:04, held by pid 53479, which exited"),
            ),
            (
                "moved to another port",
                Some(StoppedPort { moved_to: Some(8413), ..base() }),
                format!(
                    "{DOWN}. It stopped listening at 12:04, held by python3 -m http.server 8412 (pid 53479), which exited, now on :8413"
                ),
            ),
        ];
        for (name, stopped, expected) in cases {
            assert_eq!(sentence(stopped.as_ref(), false), expected, "{name}");
        }
    }

    #[test]
    fn a_holder_that_is_no_longer_the_workspaces_still_listens_so_the_line_says_it_left() {
        let left = StoppedPort { exited: Some(false), left: true, ..base() };
        assert_eq!(
            sentence(Some(&left), false),
            ":8412 is no longer this workspace's at 12:04, held by python3 -m http.server 8412 (pid 53479), which still listens"
        );
    }

    #[test]
    fn a_holder_that_left_with_a_fetch_that_just_failed_is_not_said_to_still_listen() {
        let left = StoppedPort { exited: Some(false), left: true, ..base() };
        assert_eq!(
            sentence(Some(&left), true),
            ":8412 is no longer this workspace's at 12:04, held by python3 -m http.server 8412 (pid 53479)"
        );
    }

    #[test]
    fn open_adds_close_drops_and_a_close_for_an_unknown_port_drops_nothing_but_is_remembered() {
        let mut d = Directory::default();
        assert!(d.feed_event(&event(json!({"type": "port.open", "port": 80, "pid": 1, "process": "nginx"})), 0));
        assert_eq!(d.ports(), [known(80, Some(1), Some("nginx"))]);
        assert!(d.feed_event(&event(json!({"type": "port.close", "port": 81})), 0));
        assert_eq!(d.ports(), [known(80, Some(1), Some("nginx"))]);
        assert_eq!(d.stopped(81).map(|s| s.port), Some(81));
        assert!(d.feed_event(&event(json!({"type": "port.close", "port": 80})), 0));
        assert_eq!(d.ports(), []);
    }

    #[test]
    fn a_reopen_of_a_known_port_keeps_the_first_pid_and_changes_nothing() {
        let mut d = Directory::default();
        assert!(d.feed_event(&event(json!({"type": "port.open", "port": 80, "pid": 1})), 0));
        assert!(!d.feed_event(&event(json!({"type": "port.open", "port": 80, "pid": 2})), 0));
        assert_eq!(d.ports(), [known(80, Some(1), None)]);
    }

    #[test]
    fn open_close_open_for_one_port_leaves_it_listed_with_the_new_pid() {
        let mut d = Directory::default();
        d.feed_event(&event(json!({"type": "port.open", "port": 8412, "pid": 100, "process": "node"})), 0);
        d.feed_event(&event(json!({"type": "port.close", "port": 8412})), 0);
        assert_eq!(d.ports(), []);
        d.feed_event(&event(json!({"type": "port.open", "port": 8412, "pid": 200, "process": "node"})), 0);
        assert_eq!(d.ports(), [known(8412, Some(200), Some("node"))]);
        d.feed_event(&event(json!({"type": "port.close", "port": 8412})), 0);
        d.feed_event(&event(json!({"type": "port.open", "port": 8412})), 0);
        assert_eq!(d.ports(), [known(8412, None, None)]);
    }

    #[test]
    fn it_is_unseeded_until_the_daemon_says_anything_then_seeded_even_by_an_empty_snapshot() {
        let mut d = Directory::default();
        assert!(!d.seeded());
        assert!(d.sync_ports(&json!([])));
        assert!(d.seeded());
        assert!(!d.sync_ports(&json!([])));
    }

    #[test]
    fn a_snapshot_adds_the_missing_drops_the_absent_and_sorts_by_port() {
        let mut d = Directory::default();
        d.feed_event(&event(json!({"type": "port.open", "port": 80, "pid": 1})), 0);
        d.feed_event(&event(json!({"type": "port.open", "port": 81})), 0);
        assert!(d.sync_ports(&json!([{"port": 9000, "pid": 5, "process": "node"}, {"port": 80, "pid": 1}])));
        assert_eq!(d.ports(), [known(80, Some(1), None), known(9000, Some(5), Some("node"))]);
    }

    #[test]
    fn an_identical_snapshot_changes_nothing() {
        let mut d = Directory::default();
        d.sync_ports(&json!([{"port": 80, "pid": 1}]));
        assert!(!d.sync_ports(&json!([{"port": 80, "pid": 1}])));
        assert_eq!(d.ports(), [known(80, Some(1), None)]);
    }

    #[test]
    fn a_malformed_reply_is_ignored_rather_than_clearing_the_directory() {
        let mut d = Directory::default();
        d.sync_ports(&json!([{"port": 80}]));
        for reply in [Value::Null, json!("nope"), json!([{"port": "80"}]), json!([null])] {
            assert!(!d.sync_ports(&reply), "{reply}");
        }
        assert_eq!(d.ports(), [known(80, None, None)]);
    }

    #[test]
    fn a_close_keeps_the_holder_until_the_port_listens_again_changing_on_each_step() {
        let mut d = Directory::default();
        d.feed_event(&event(json!({"type": "port.open", "port": 8412, "pid": 53479, "process": "python3"})), 1_000);
        assert_eq!(d.stopped(8412), None);
        assert!(d.feed_event(&close_held(), 1_000));
        assert_eq!(d.stopped(8412), Some(&StoppedPort { seen_at: 1_000, ..held_row() }));
        assert!(d.feed_event(&event(json!({"type": "port.open", "port": 8413, "pid": 60000, "process": "python3"})), 31_000));
        assert_eq!(d.stopped(8412).and_then(|s| s.moved_to), Some(8413));
        assert_eq!(d.ports(), [known(8413, Some(60000), Some("python3"))]);
        assert!(d.feed_event(&event(json!({"type": "port.open", "port": 8412, "pid": 60001, "process": "python3"})), 31_000));
        assert_eq!(d.stopped(8412), None);
    }

    #[test]
    fn a_stop_with_nothing_new_for_the_list_still_counts_as_a_change() {
        let mut d = Directory::default();
        d.sync_ports(&json!([{"port": 80}]));
        d.feed_event(&event(json!({"type": "port.close", "port": 80})), 5);
        assert!(d.feed_event(&event(json!({"type": "port.close", "port": 80, "at": AT})), 5));
        assert_eq!(d.ports(), []);
        assert_eq!(d.stopped(80).and_then(|s| s.at.as_deref()), Some(AT));
    }
}
