// SPDX-License-Identifier: AGPL-3.0-only
//! What one computer's daemon sends Activity over its link, after `wire/daemon.ts` and `wire/machine-link.ts`: the
//! process frames (a whole `proc.snapshot`, then `proc.changes` naming the frame each follows), the ports it listens
//! on and the readings of its memory. Read off JSON by hand, so a row a newer daemon words its own way drops that word
//! and not the frame, as the web's `.catch` reads do.

use serde_json::Value;
use wsp_host_client::protocol::{AppProcList, AppProcListItemKind};

pub type Pid = i64;

/// Every kind a process row names (`PROC_KINDS`), in the order the Kinds view and the kind select list them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    Agent,
    Tool,
    Dev,
    Test,
    Build,
    Slate,
    Terminal,
    Tab,
    App,
    Wsp,
    Other,
}

impl Kind {
    pub const ALL: [Kind; 11] = [
        Kind::Agent,
        Kind::Tool,
        Kind::Dev,
        Kind::Test,
        Kind::Build,
        Kind::Slate,
        Kind::Terminal,
        Kind::Tab,
        Kind::App,
        Kind::Wsp,
        Kind::Other,
    ];

    /// A word a newer daemon adds reads as other.
    pub fn parse(word: &str) -> Kind {
        match word {
            "agent" => Kind::Agent,
            "tool" => Kind::Tool,
            "dev" => Kind::Dev,
            "test" => Kind::Test,
            "build" => Kind::Build,
            "slate" => Kind::Slate,
            "terminal" => Kind::Terminal,
            "tab" => Kind::Tab,
            "app" => Kind::App,
            "wsp" => Kind::Wsp,
            _ => Kind::Other,
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Kind::Agent => "agent",
            Kind::Tool => "tool",
            Kind::Dev => "dev",
            Kind::Test => "test",
            Kind::Build => "build",
            Kind::Slate => "slate",
            Kind::Terminal => "terminal",
            Kind::Tab => "tab",
            Kind::App => "app",
            Kind::Wsp => "wsp",
            Kind::Other => "other",
        }
    }

    /// `ACTIVITY_KIND_WORDS`: the kind as a row's tag says it.
    pub fn word(self) -> &'static str {
        match self {
            Kind::Agent => "Agent",
            Kind::Tool => "Tool server",
            Kind::Dev => "Dev server",
            Kind::Test => "Test run",
            Kind::Build => "Build",
            Kind::Slate => "Slate run",
            Kind::Terminal => "Terminal",
            Kind::Tab => "Browser tab",
            Kind::App => "App",
            Kind::Wsp => "wsp",
            Kind::Other => "Other",
        }
    }

    /// `KIND_HEADS`: the kind as the Kinds view heads its group.
    pub fn head(self) -> &'static str {
        match self {
            Kind::Agent => "Agents",
            Kind::Tool => "Tool servers",
            Kind::Dev => "Dev servers",
            Kind::Test => "Test runs",
            Kind::Build => "Builds",
            Kind::Slate => "Slate runs",
            Kind::Terminal => "Terminals",
            Kind::Tab => "Browser tabs",
            Kind::App => "The app",
            Kind::Wsp => "wsp",
            Kind::Other => "Other",
        }
    }

    /// `UNTAGGED_KINDS`: a row of these carries no tag, its name says it already.
    pub fn untagged(self) -> bool {
        matches!(self, Kind::Agent | Kind::Other | Kind::Wsp | Kind::App)
    }
}

/// `ProcOwnedBy`: the rung of the daemon's ladder that named a row's owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OwnedBy {
    Root,
    Fork,
    Group,
    Env,
    Cgroup,
    Folder,
    Coalition,
}

impl OwnedBy {
    fn parse(word: &str) -> Option<OwnedBy> {
        Some(match word {
            "root" => OwnedBy::Root,
            "fork" => OwnedBy::Fork,
            "group" => OwnedBy::Group,
            "env" => OwnedBy::Env,
            "cgroup" => OwnedBy::Cgroup,
            "folder" => OwnedBy::Folder,
            "coalition" => OwnedBy::Coalition,
            _ => return None,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Owner {
    pub by: OwnedBy,
    pub thread: Option<String>,
}

/// One process as /proc shows it (`ProcEntry`).
#[derive(Clone, Debug, PartialEq)]
pub struct ProcEntry {
    pub pid: Pid,
    pub ppid: Pid,
    pub user: String,
    pub state: String,
    pub comm: String,
    pub cmdline: String,
    pub cpu: f64,
    pub rss: f64,
    pub started_at: f64,
    pub pty: Option<String>,
    pub cgroup: Option<String>,
    pub energy_mw: Option<f64>,
    pub pgid: Option<Pid>,
    pub owner: Option<Owner>,
    pub kind: Option<Kind>,
    pub id: Option<String>,
}

fn text(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(str::to_owned)
}

fn num(v: &Value, key: &str) -> Option<f64> {
    v.get(key).and_then(Value::as_f64)
}

fn int(v: &Value, key: &str) -> Option<Pid> {
    v.get(key).and_then(|n| n.as_i64().or_else(|| n.as_f64().filter(|f| f.fract() == 0.0).map(|f| f as i64)))
}

impl ProcEntry {
    pub fn read(v: &Value) -> Option<ProcEntry> {
        let owner = v.get("owner").and_then(|o| {
            let by = OwnedBy::parse(o.get("by")?.as_str()?)?;
            Some(Owner { by, thread: text(o, "thread") })
        });
        Some(ProcEntry {
            pid: int(v, "pid")?,
            ppid: int(v, "ppid")?,
            user: text(v, "user").unwrap_or_default(),
            state: text(v, "state")?,
            comm: text(v, "comm")?,
            cmdline: text(v, "cmdline")?,
            cpu: num(v, "cpu")?,
            rss: num(v, "rss")?,
            started_at: num(v, "startedAt")?,
            pty: text(v, "pty"),
            cgroup: text(v, "cgroup"),
            energy_mw: num(v, "energyMw"),
            pgid: int(v, "pgid"),
            owner,
            kind: v.get("kind").and_then(Value::as_str).map(Kind::parse),
            id: text(v, "id"),
        })
    }

    #[cfg(test)]
    pub fn new(pid: Pid, ppid: Pid, comm: &str, cmdline: &str) -> ProcEntry {
        ProcEntry {
            pid,
            ppid,
            user: String::new(),
            state: "S".into(),
            comm: comm.into(),
            cmdline: cmdline.into(),
            cpu: 0.,
            rss: 0.,
            started_at: 0.,
            pty: None,
            cgroup: None,
            energy_mw: None,
            pgid: None,
            owner: None,
            kind: None,
            id: None,
        }
    }
}

/// `ContainerPlacedBy`: how a container came to stand under its thread.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlacedBy {
    Label,
    Compose,
}

/// One running container on the computer's engine (`ProcContainer`).
#[derive(Clone, Debug, PartialEq)]
pub struct Container {
    pub id: String,
    pub name: String,
    pub image: String,
    pub started_at: Option<f64>,
    pub thread: Option<String>,
    pub placed_by: Option<PlacedBy>,
    pub folder: Option<String>,
    pub memory: Option<f64>,
    pub cpu: Option<f64>,
}

impl Container {
    fn read(v: &Value) -> Option<Container> {
        Some(Container {
            id: text(v, "id")?,
            name: text(v, "name")?,
            image: text(v, "image")?,
            started_at: num(v, "startedAt"),
            thread: text(v, "thread"),
            placed_by: match v.get("placedBy").and_then(Value::as_str) {
                Some("label") => Some(PlacedBy::Label),
                Some("compose") => Some(PlacedBy::Compose),
                _ => None,
            },
            folder: text(v, "folder"),
            memory: num(v, "memory"),
            cpu: num(v, "cpu"),
        })
    }
}

/// What one tick of the sampler cost the daemon (`ProcCost`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cost {
    pub cpu_ms: f64,
    pub held: f64,
}

/// Every process the daemon read one tick (`ProcSnapshot`).
#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    pub at: f64,
    pub daemon: Pid,
    pub procs: Vec<ProcEntry>,
    /// How many processes the computer had, past the cap the frame lists.
    pub total: Option<i64>,
    pub seq: Option<i64>,
    pub cost: Option<Cost>,
    pub containers: Option<Vec<Container>>,
    /// Each thread cgroup's memory on a box.
    pub cgroups: Option<Vec<(String, f64)>>,
}

/// What moved since frame `base` (`ProcChanges`).
#[derive(Clone, Debug, PartialEq)]
pub struct Changes {
    pub at: f64,
    pub daemon: Pid,
    pub procs: Vec<ProcEntry>,
    pub total: Option<i64>,
    pub gone: Vec<Pid>,
    pub seq: i64,
    pub base: i64,
    pub cost: Option<Cost>,
    pub containers: Option<Vec<Container>>,
    pub cgroups: Option<Vec<(String, f64)>>,
}

pub enum Frame {
    Snapshot(Snapshot),
    Changes(Changes),
}

fn rows(v: &Value) -> Vec<ProcEntry> {
    v.get("procs").and_then(Value::as_array).map(|a| a.iter().filter_map(ProcEntry::read).collect()).unwrap_or_default()
}

fn cost(v: &Value) -> Option<Cost> {
    let c = v.get("cost")?;
    Some(Cost { cpu_ms: num(c, "cpuMs")?, held: num(c, "held")? })
}

fn containers(v: &Value) -> Option<Vec<Container>> {
    Some(v.get("containers")?.as_array()?.iter().filter_map(Container::read).collect())
}

fn cgroups(v: &Value) -> Option<Vec<(String, f64)>> {
    Some(v.get("cgroups")?.as_array()?.iter().filter_map(|c| Some((text(c, "cgroup")?, num(c, "memory")?))).collect())
}

impl Frame {
    /// A `proc.snapshot` or `proc.changes` event, or nothing for any other.
    pub fn read(v: &Value) -> Option<Frame> {
        match v.get("type")?.as_str()? {
            "proc.snapshot" => Some(Frame::Snapshot(Snapshot {
                at: num(v, "at")?,
                daemon: int(v, "daemon")?,
                procs: rows(v),
                total: int(v, "total"),
                seq: int(v, "seq"),
                cost: cost(v),
                containers: containers(v),
                cgroups: cgroups(v),
            })),
            "proc.changes" => Some(Frame::Changes(Changes {
                at: num(v, "at")?,
                daemon: int(v, "daemon")?,
                procs: rows(v),
                total: int(v, "total"),
                gone: v.get("gone").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_i64).collect()).unwrap_or_default(),
                seq: int(v, "seq")?,
                base: int(v, "base")?,
                cost: cost(v),
                containers: containers(v),
                cgroups: cgroups(v),
            })),
            _ => None,
        }
    }
}

/// `applyProcChanges`: the snapshot a changes frame leaves, or none where it does not follow the one held, which the
/// page answers by watching again for a whole one.
pub fn apply_changes(snapshot: &Snapshot, changes: Changes) -> Option<Snapshot> {
    if Some(changes.base) != snapshot.seq {
        return None;
    }
    let mut rows: std::collections::BTreeMap<Pid, ProcEntry> = snapshot.procs.iter().map(|p| (p.pid, p.clone())).collect();
    for pid in &changes.gone {
        rows.remove(pid);
    }
    for p in changes.procs {
        rows.insert(p.pid, p);
    }
    Some(Snapshot {
        at: changes.at,
        daemon: changes.daemon,
        procs: rows.into_values().collect(),
        total: changes.total.or(snapshot.total),
        seq: Some(changes.seq),
        cost: changes.cost,
        containers: changes.containers.or_else(|| snapshot.containers.clone()),
        cgroups: changes.cgroups,
    })
}

/// One port a process listens on (`KnownPort`).
#[derive(Clone, Debug, PartialEq)]
pub struct KnownPort {
    pub port: i64,
    pub pid: Option<Pid>,
    /// Only this computer can reach it; none where the daemon did not say.
    pub loopback: Option<bool>,
}

/// `applyPortsSnapshot`: a ports.watch reply's rows, one per port, lowest first.
pub fn ports_of_reply(reply: &Value) -> Option<Vec<KnownPort>> {
    let listed = reply.get("ports")?.as_array()?;
    let mut ports: Vec<KnownPort> = Vec::new();
    for row in listed {
        let port = int(row, "port")?;
        if ports.iter().any(|p| p.port == port) {
            continue;
        }
        ports.push(KnownPort { port, pid: int(row, "pid"), loopback: row.get("loopback").and_then(Value::as_bool) });
    }
    ports.sort_by_key(|p| p.port);
    Some(ports)
}

/// `applyPortEvent`: a re-open of a known port keeps the first pid, a close of an unknown port is nothing.
pub fn apply_port_event(ports: &mut Vec<KnownPort>, event: &Value) {
    let Some(port) = int(event, "port") else { return };
    match event.get("type").and_then(Value::as_str) {
        Some("port.open") if !ports.iter().any(|p| p.port == port) => {
            ports.push(KnownPort { port, pid: int(event, "pid"), loopback: event.get("loopback").and_then(Value::as_bool) });
            ports.sort_by_key(|p| p.port);
        }
        Some("port.close") => ports.retain(|p| p.port != port),
        _ => {}
    }
}

/// A `sys.sample`'s memory, used and total.
pub fn memory_of(sample: &Value) -> Option<(f64, f64)> {
    Some((sample.pointer("/mem/used")?.as_f64()?, sample.pointer("/mem/total")?.as_f64()?))
}

/// What the app says one of its own processes is (`AppProcKind`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppKind {
    Main,
    Gpu,
    Network,
    Renderer,
    Tab,
    Frame,
    Helper,
}

impl AppKind {
    /// `APP_PROC_WORDS`.
    pub fn word(self) -> &'static str {
        match self {
            AppKind::Main => "App",
            AppKind::Gpu => "Graphics",
            AppKind::Network => "Network",
            AppKind::Renderer => "Window",
            AppKind::Tab => "Browser tab",
            AppKind::Frame => "Frame",
            AppKind::Helper => "Helper",
        }
    }
}

/// One of the app's own processes as it names them (`AppProc`).
#[derive(Clone, Debug, PartialEq)]
pub struct AppPart {
    pub pid: Pid,
    pub kind: AppKind,
    pub origins: Vec<String>,
}

/// The app's list as the host pushes it (`AppProcList`).
pub fn app_parts(list: &AppProcList) -> Vec<AppPart> {
    list.iter()
        .map(|row| {
            let kind = match row.kind {
                AppProcListItemKind::Main => AppKind::Main,
                AppProcListItemKind::Gpu => AppKind::Gpu,
                AppProcListItemKind::Network => AppKind::Network,
                AppProcListItemKind::Renderer => AppKind::Renderer,
                AppProcListItemKind::Tab => AppKind::Tab,
                AppProcListItemKind::Frame => AppKind::Frame,
                AppProcListItemKind::Helper | AppProcListItemKind::Unknown => AppKind::Helper,
            };
            AppPart { pid: row.pid.get() as Pid, kind, origins: row.origins.clone() }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn changes_apply_only_on_the_frame_they_name() {
        let row = |pid: i64, cpu: f64| json!({ "pid": pid, "ppid": 1, "user": "u", "state": "S", "comm": "c", "cmdline": "", "cpu": cpu, "rss": 1, "startedAt": 0 });
        let Some(Frame::Snapshot(first)) =
            Frame::read(&json!({ "type": "proc.snapshot", "at": 1, "daemon": 9, "total": 2, "procs": [row(2, 0.), row(3, 0.)], "seq": 4,
                "containers": [{ "id": "c1", "name": "db", "image": "postgres", "state": "running" }] }))
        else {
            panic!("no snapshot")
        };
        let changes = |base: i64| {
            let Some(Frame::Changes(c)) = Frame::read(
                &json!({ "type": "proc.changes", "at": 3, "daemon": 9, "total": 2, "procs": [row(5, 1.)], "gone": [2], "seq": 5, "base": base }),
            ) else {
                panic!("no changes")
            };
            c
        };
        assert!(apply_changes(&first, changes(3)).is_none());
        let next = apply_changes(&first, changes(4)).unwrap();
        assert_eq!(next.procs.iter().map(|p| p.pid).collect::<Vec<_>>(), vec![3, 5]);
        assert_eq!(next.seq, Some(5));
        assert_eq!(next.at, 3., "the changes' own time");
        assert_eq!(next.containers.map(|c| c.len()), Some(1), "changes that name no containers keep the snapshot's");
    }

    #[test]
    fn a_word_a_newer_daemon_adds_drops_the_word_not_the_row() {
        let p = ProcEntry::read(&json!({ "pid": 2, "ppid": 1, "user": "u", "state": "S", "comm": "c", "cmdline": "", "cpu": 0, "rss": 1,
            "startedAt": 0, "kind": "robot", "owner": { "by": "telepathy", "thread": "t" } }))
        .unwrap();
        assert_eq!(p.kind, Some(Kind::Other));
        assert_eq!(p.owner, None);
    }

    #[test]
    fn ports_fold_as_the_web_folds_them() {
        let mut ports = ports_of_reply(&json!({ "ports": [{ "port": 5173, "pid": 7 }, { "port": 22, "loopback": false }] })).unwrap();
        assert_eq!(ports.iter().map(|p| p.port).collect::<Vec<_>>(), vec![22, 5173]);
        apply_port_event(&mut ports, &json!({ "type": "port.open", "port": 22, "pid": 99 }));
        assert_eq!(ports[0].pid, None);
        apply_port_event(&mut ports, &json!({ "type": "port.close", "port": 5173 }));
        assert_eq!(ports.len(), 1);
    }

    /// The daemon's own types, written as it writes them, read back by this file's readers: a field renamed or retyped
    /// on the daemon's side reads as nothing here and fails these.
    mod the_daemons_types {
        use super::super::*;
        use wsp_frames::{
            CgroupMemory, ContainerPlacedBy, DaemonEvent, ListeningPort, PortsWatchReply, ProcContainer, ProcCost, ProcKind, ProcOwnedBy,
            ProcOwner, Usage,
        };

        fn ours(kind: ProcKind) -> Kind {
            match kind {
                ProcKind::Agent => Kind::Agent,
                ProcKind::Tool => Kind::Tool,
                ProcKind::Dev => Kind::Dev,
                ProcKind::Test => Kind::Test,
                ProcKind::Build => Kind::Build,
                ProcKind::Slate => Kind::Slate,
                ProcKind::Terminal => Kind::Terminal,
                ProcKind::Tab => Kind::Tab,
                ProcKind::App => Kind::App,
                ProcKind::Wsp => Kind::Wsp,
                ProcKind::Other => Kind::Other,
            }
        }

        fn rung(by: ProcOwnedBy) -> OwnedBy {
            match by {
                ProcOwnedBy::Root => OwnedBy::Root,
                ProcOwnedBy::Fork => OwnedBy::Fork,
                ProcOwnedBy::Group => OwnedBy::Group,
                ProcOwnedBy::Env => OwnedBy::Env,
                ProcOwnedBy::Cgroup => OwnedBy::Cgroup,
                ProcOwnedBy::Folder => OwnedBy::Folder,
                ProcOwnedBy::Coalition => OwnedBy::Coalition,
            }
        }

        const KINDS: [ProcKind; 11] = [
            ProcKind::Agent,
            ProcKind::Tool,
            ProcKind::Dev,
            ProcKind::Test,
            ProcKind::Build,
            ProcKind::Slate,
            ProcKind::Terminal,
            ProcKind::Tab,
            ProcKind::App,
            ProcKind::Wsp,
            ProcKind::Other,
        ];
        const RUNGS: [ProcOwnedBy; 7] = [
            ProcOwnedBy::Root,
            ProcOwnedBy::Fork,
            ProcOwnedBy::Group,
            ProcOwnedBy::Env,
            ProcOwnedBy::Cgroup,
            ProcOwnedBy::Folder,
            ProcOwnedBy::Coalition,
        ];

        fn entry(pid: u32, kind: ProcKind, by: ProcOwnedBy) -> wsp_frames::ProcEntry {
            wsp_frames::ProcEntry {
                pid,
                ppid: 1,
                user: "u".into(),
                state: "R".into(),
                comm: "node".into(),
                cmdline: "node server.js".into(),
                cpu: 12.5,
                rss: 4096,
                started_at: 1_700_000_000_000,
                pty: Some("p1".into()),
                cgroup: Some("/wsp.slice/t1".into()),
                energy_mw: Some(30.5),
                pgid: Some(pid),
                owner: Some(ProcOwner { by, thread: Some("t1".into()), workspace: Some("w1".into()), warm: false }),
                kind: Some(kind),
                id: Some("tick-77".into()),
            }
        }

        fn container() -> ProcContainer {
            ProcContainer {
                id: "c1".into(),
                name: "db".into(),
                image: "postgres".into(),
                state: "running".into(),
                started_at: Some(1_700_000_000_000),
                thread: Some("t1".into()),
                placed_by: Some(ContainerPlacedBy::Compose),
                folder: Some("/src/app".into()),
                project: Some("app".into()),
                memory: Some(2048),
                cpu: Some(3.5),
            }
        }

        fn same(theirs: &wsp_frames::ProcEntry, ours_: &ProcEntry) {
            let owner = theirs.owner.as_ref().unwrap();
            let want = ProcEntry {
                pid: theirs.pid.into(),
                ppid: theirs.ppid.into(),
                user: theirs.user.clone(),
                state: theirs.state.clone(),
                comm: theirs.comm.clone(),
                cmdline: theirs.cmdline.clone(),
                cpu: theirs.cpu,
                rss: theirs.rss as f64,
                started_at: theirs.started_at as f64,
                pty: theirs.pty.clone(),
                cgroup: theirs.cgroup.clone(),
                energy_mw: theirs.energy_mw,
                pgid: theirs.pgid.map(Into::into),
                owner: Some(Owner { by: rung(owner.by), thread: owner.thread.clone() }),
                kind: theirs.kind.map(ours),
                id: theirs.id.clone(),
            };
            assert_eq!(*ours_, want);
        }

        fn wire(event: &DaemonEvent) -> Value {
            serde_json::to_value(event).unwrap()
        }

        #[test]
        fn a_snapshot_reads_every_field_the_daemon_writes() {
            let procs: Vec<_> =
                KINDS.iter().zip(RUNGS.iter().cycle()).enumerate().map(|(i, (k, b))| entry(10 + i as u32, *k, *b)).collect();
            let event = DaemonEvent::ProcSnapshot {
                at: 5_000,
                daemon: 9,
                total: procs.len() as u64,
                procs: procs.clone(),
                seq: Some(4),
                cost: Some(ProcCost { cpu_ms: 1.5, held: 640 }),
                containers: Some(vec![container()]),
                cgroups: Some(vec![CgroupMemory { cgroup: "/wsp.slice/t1".into(), memory: 8192 }]),
            };
            let Some(Frame::Snapshot(s)) = Frame::read(&wire(&event)) else { panic!("the daemon's snapshot does not read") };
            assert_eq!((s.at, s.daemon, s.seq, s.total), (5_000., 9, Some(4), Some(procs.len() as i64)));
            assert_eq!(s.cost, Some(Cost { cpu_ms: 1.5, held: 640. }));
            assert_eq!(s.cgroups, Some(vec![("/wsp.slice/t1".to_owned(), 8192.)]));
            assert_eq!(s.procs.len(), procs.len());
            for (theirs, ours_) in procs.iter().zip(&s.procs) {
                same(theirs, ours_);
            }
            let c = container();
            let want = Container {
                id: c.id,
                name: c.name,
                image: c.image,
                started_at: c.started_at.map(|t| t as f64),
                thread: c.thread,
                placed_by: Some(PlacedBy::Compose),
                folder: c.folder,
                memory: c.memory.map(|m| m as f64),
                cpu: c.cpu,
            };
            assert_eq!(s.containers, Some(vec![want]));
        }

        #[test]
        fn changes_read_every_field_the_daemon_writes() {
            let event = DaemonEvent::ProcChanges {
                at: 7_000,
                daemon: 9,
                total: 1,
                procs: vec![entry(12, ProcKind::Dev, ProcOwnedBy::Env)],
                gone: vec![10, 11],
                seq: 5,
                base: 4,
                cost: Some(ProcCost { cpu_ms: 0.5, held: 320 }),
                containers: Some(vec![]),
                cgroups: Some(vec![]),
            };
            let Some(Frame::Changes(c)) = Frame::read(&wire(&event)) else { panic!("the daemon's changes do not read") };
            assert_eq!((c.at, c.daemon, c.seq, c.base, c.gone.clone()), (7_000., 9, 5, 4, vec![10, 11]));
            assert_eq!(c.cost, Some(Cost { cpu_ms: 0.5, held: 320. }));
            assert_eq!((c.containers, c.cgroups), (Some(vec![]), Some(vec![])));
            same(&entry(12, ProcKind::Dev, ProcOwnedBy::Env), &c.procs[0]);
        }

        #[test]
        fn every_kind_reads_as_the_word_the_daemon_writes() {
            for kind in KINDS {
                let word = serde_json::to_value(kind).unwrap();
                assert_eq!(Some(ours(kind).id()), word.as_str());
                assert_eq!(Kind::parse(word.as_str().unwrap()), ours(kind));
            }
            for by in RUNGS {
                let word = serde_json::to_value(by).unwrap();
                assert_eq!(OwnedBy::parse(word.as_str().unwrap()), Some(rung(by)));
            }
        }

        #[test]
        fn ports_and_readings_read_as_the_daemon_writes_them() {
            let listening = |port: u16, pid: Option<u32>, loopback: bool| ListeningPort {
                port,
                pid,
                inode: None,
                uid: 0,
                process: None,
                command: None,
                loopback,
            };
            let reply =
                serde_json::to_value(PortsWatchReply { ports: vec![listening(5173, Some(7), true), listening(22, None, false)] }).unwrap();
            let mut ports = ports_of_reply(&reply).unwrap();
            assert_eq!(
                ports,
                vec![
                    KnownPort { port: 22, pid: None, loopback: Some(false) },
                    KnownPort { port: 5173, pid: Some(7), loopback: Some(true) }
                ]
            );
            let open = DaemonEvent::PortOpen { port: 8080, pid: Some(8), process: None, loopback: Some(false), watch: None };
            apply_port_event(&mut ports, &wire(&open));
            assert!(ports.contains(&KnownPort { port: 8080, pid: Some(8), loopback: Some(false) }));
            let close = DaemonEvent::PortClose {
                port: 22,
                pid: None,
                process: None,
                command: None,
                exited: None,
                left: None,
                at: None,
                watch: None,
            };
            apply_port_event(&mut ports, &wire(&close));
            assert_eq!(ports.iter().map(|p| p.port).collect::<Vec<_>>(), vec![5173, 8080]);
            let sample =
                DaemonEvent::SysSample { cpu: 1., load1: 0.5, mem: Usage { used: 3, total: 8 }, disk: Usage { used: 1, total: 2 }, at: 1 };
            assert_eq!(memory_of(&wire(&sample)), Some((3., 8.)));
        }
    }
}
