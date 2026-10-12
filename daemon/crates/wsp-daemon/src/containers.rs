// SPDX-License-Identifier: AGPL-3.0-only
//! The containers the computer's engine runs, for Activity, read while some socket watches the processes. The reader
//! holds one engine `/events` subscription, lists the containers once when it opens, and inspects one only on its
//! create, start, die and destroy: the engine's list costs it about 6 ms a container (7.6% of a core polled every 2 s
//! with 13), so nothing lists on a tick. A container's figures come from its own cgroup's files on Linux, the
//! cgroup found off `/proc/<pid>/cgroup`, and where those are out of reach (a Mac, whose engine runs in a VM) from
//! the engine's one-shot stats every STATS_EVERY.

use std::collections::BTreeMap;
use std::io::Read;
use std::os::fd::OwnedFd;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::Value;
use tokio::task::AbortHandle;
use tokio::time::MissedTickBehavior;
use wsp_frames::{ContainerPlacedBy, DaemonErrorCode, ProcContainer};

use crate::engine_http::{ask, engine_socket, Events};
use crate::paths::OpError;
use crate::proc::Clock;
use crate::SharedLog;

/// The label the docker shim puts on a container a thread makes, its value the thread's id.
const THREAD_LABEL: &str = "wsp.thread";
/// The label a fork's engine fence puts on every container it makes; `wsp_runtime::engine::LABEL` is Linux only.
const WORKSPACE_LABEL: &str = "wsp.workspace";
const COMPOSE_FOLDER: &str = "com.docker.compose.project.working_dir";
const COMPOSE_PROJECT: &str = "com.docker.compose.project";
/// The four events that change what the reader holds; the engine filters out every other.
const EVENTS_QUERY: &str =
    "filters=%7B%22type%22%3A%5B%22container%22%5D%2C%22event%22%3A%5B%22create%22%2C%22start%22%2C%22die%22%2C%22destroy%22%5D%7D";
/// Between one-shot stats passes where a container's cgroup is out of reach.
pub(crate) const STATS_EVERY: Duration = Duration::from_secs(10);
/// Between tries at an engine that is not there or closed its events.
pub(crate) const RETRY_EVERY: Duration = Duration::from_secs(10);
/// The most of one thread's notes file read; the shim keeps it under 32 KB.
const NOTES_CAP: u64 = 64 << 10;
/// The longest cgroup path read off /proc.
const CGROUP_LINE_CAP: u64 = 4096;

pub(crate) struct ReaderOptions {
    pub(crate) home: Option<PathBuf>,
    pub(crate) docker_host: Option<String>,
    /// The folder the docker shim writes each thread's compose notes in.
    pub(crate) notes: PathBuf,
    pub(crate) proc_root: PathBuf,
    pub(crate) cgroup_root: PathBuf,
    /// Whether a container's cgroup can be on this computer at all, Linux; elsewhere every figure is the engine's stats.
    pub(crate) cgroups: bool,
    pub(crate) stats_every: Duration,
    pub(crate) retry: Duration,
    pub(crate) now: Clock,
    pub(crate) log: SharedLog,
}

/// A compose note the shim wrote: the thread that ran `compose up|create`, its folder, the project it named or none, and
/// when, in epoch seconds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Note {
    pub(crate) thread: String,
    pub(crate) folder: String,
    pub(crate) project: String,
    pub(crate) at: i64,
}

/// A thread's id as the shim takes one.
fn thread_word(s: &str) -> bool {
    !s.is_empty() && s.len() <= 64 && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

/// One notes file's lines, each `thread`, folder, project and epoch seconds tab apart; any other line is skipped.
pub(crate) fn parse_notes(text: &str) -> Vec<Note> {
    text.lines()
        .filter_map(|line| {
            let mut f = line.split('\t');
            let (thread, folder, project, at) = (f.next()?, f.next()?, f.next()?, f.next()?);
            (f.next().is_none() && thread_word(thread) && folder.starts_with('/')).then_some(())?;
            Some(Note { thread: thread.to_owned(), folder: folder.to_owned(), project: project.to_owned(), at: at.trim().parse().ok()? })
        })
        .collect()
}

/// Whether `folder` is `top` or a folder under it.
fn inside(folder: &str, top: &str) -> bool {
    let top = top.trim_end_matches('/');
    folder.trim_end_matches('/') == top || folder.starts_with(&format!("{top}/"))
}

/// The thread a container stands under: the one its label names; else, unless a fork's fence labelled it with its
/// workspace, the newest note whose folder sits inside its compose folder and whose project, where the note named one,
/// is its own; else none. A fork's compose keeps compose's own labels, so a box thread's note in the same folder would
/// otherwise take it.
pub(crate) fn place(
    label: Option<&str>,
    workspace: Option<&str>,
    folder: Option<&str>,
    project: Option<&str>,
    notes: &[Note],
) -> Option<(String, ContainerPlacedBy)> {
    if let Some(thread) = label.filter(|t| thread_word(t)) {
        return Some((thread.to_owned(), ContainerPlacedBy::Label));
    }
    if workspace.is_some() {
        return None;
    }
    let folder = folder?;
    notes
        .iter()
        .filter(|n| inside(&n.folder, folder) && (n.project.is_empty() || Some(n.project.as_str()) == project))
        .max_by_key(|n| n.at)
        .map(|n| (n.thread.clone(), ContainerPlacedBy::Compose))
}

/// A memory.stat's `anon` and `kernel`. A kernel before 5.18 has no `kernel` line, so its parts stand in for it.
pub(crate) fn cgroup_memory(stat: &str) -> Option<u64> {
    let field = |name: &str| stat.lines().find_map(|l| l.strip_prefix(name)?.strip_prefix(' ')?.trim().parse::<u64>().ok());
    let kernel = field("kernel").or_else(|| {
        ["kernel_stack", "pagetables", "sec_pagetables", "percpu", "sock", "vmalloc", "slab"]
            .iter()
            .filter_map(|f| field(f))
            .reduce(|a, b| a + b)
    });
    Some(field("anon")? + kernel.unwrap_or(0))
}

/// A cpu.stat's `usage_usec`.
pub(crate) fn cgroup_usage_usec(stat: &str) -> Option<u64> {
    stat.lines().find_map(|l| l.strip_prefix("usage_usec ")?.trim().parse().ok())
}

/// The engine's one-shot stats as (memory, cpu nanoseconds): memory is `anon + slab + kernel_stack + sock`, the parts
/// of the cgroup's `kernel` its stats map carries.
pub(crate) fn stats_figures(stats: &Value) -> (Option<u64>, Option<u64>) {
    let map = stats.pointer("/memory_stats/stats");
    let parts: Vec<u64> =
        ["anon", "slab", "kernel_stack", "sock"].iter().filter_map(|k| map.and_then(|m| m.get(*k)).and_then(Value::as_u64)).collect();
    let memory = (!parts.is_empty()).then(|| parts.iter().sum());
    (memory, stats.pointer("/cpu_stats/cpu_usage/total_usage").and_then(Value::as_u64))
}

/// The busy share of one core between two cumulative cpu readings in nanoseconds, `ms` apart.
fn share(before: Option<(u64, i64)>, now: u64, at: i64) -> Option<f64> {
    let (was, then) = before?;
    let ms = at - then;
    (ms > 0 && now >= was).then(|| (now - was) as f64 / (ms as f64 * 1e6) * 100.0)
}

/// Epoch milliseconds off the engine's RFC 3339 time; none for its zero time, which a container never started reads.
pub(crate) fn rfc3339_ms(s: &str) -> Option<i64> {
    let (date, time) = s.split_once('T')?;
    let mut d = date.split('-').map(|n| n.parse::<i64>().ok());
    let (y, m, day) = (d.next()??, d.next()??, d.next()??);
    if y < 1971 {
        return None;
    }
    let zone_at = time.find(['Z', '+', '-']).unwrap_or(time.len());
    let (clock, zone) = time.split_at(zone_at);
    let (hms, frac) = clock.split_once('.').unwrap_or((clock, ""));
    let mut t = hms.split(':').map(|n| n.parse::<i64>().ok());
    let (h, min, sec) = (t.next()??, t.next()??, t.next()??);
    let ms: i64 = format!("{frac:0<3}")[..3].parse().ok()?;
    let offset = match zone.as_bytes().first() {
        Some(sign @ (b'+' | b'-')) => {
            let (oh, om) = zone[1..].split_once(':')?;
            let minutes = oh.parse::<i64>().ok()? * 60 + om.parse::<i64>().ok()?;
            if *sign == b'+' {
                minutes
            } else {
                -minutes
            }
        }
        _ => 0,
    };
    // Days from the civil date (Howard Hinnant's days_from_civil).
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some(((days * 86_400 + h * 3600 + min * 60 + sec - offset * 60) * 1000) + ms)
}

/// One container as the reader holds it between events.
#[derive(Debug, Clone)]
struct Known {
    name: String,
    image: String,
    state: String,
    started_at: Option<i64>,
    label: Option<String>,
    workspace: Option<String>,
    folder: Option<String>,
    project: Option<String>,
    /// Its own cgroup's folder, where the computer reaches it; the engine's stats stand in otherwise.
    cgroup: Option<PathBuf>,
    /// The last cumulative cpu reading in nanoseconds, and when it was read.
    usage: Option<(u64, i64)>,
    memory: Option<u64>,
    cpu: Option<f64>,
}

#[derive(Default)]
struct NotesCache {
    /// Each file read, its name, length and change time, which the next tick compares before reading again.
    seen: Vec<(String, u64, i64)>,
    notes: Vec<Note>,
}

#[derive(Default)]
struct State {
    /// The events are held and the list read: until then no frame names containers.
    reached: bool,
    known: BTreeMap<String, Known>,
    notes: NotesCache,
    /// The last reason the engine could not be followed, said once until it changes.
    said: Option<String>,
}

pub(crate) struct ContainerReader {
    opts: ReaderOptions,
    state: Mutex<State>,
    task: Mutex<Option<AbortHandle>>,
}

fn string_at(v: &Value, at: &str) -> Option<String> {
    v.pointer(at).and_then(Value::as_str).filter(|s| !s.is_empty()).map(str::to_owned)
}

impl ContainerReader {
    pub(crate) fn new(opts: ReaderOptions) -> Arc<ContainerReader> {
        Arc::new(ContainerReader { opts, state: Mutex::default(), task: Mutex::default() })
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn socket(&self) -> Option<PathBuf> {
        engine_socket(self.opts.home.as_deref(), self.opts.docker_host.as_deref())
    }

    /// Follows the engine until stop; a second start while it runs does nothing.
    pub(crate) fn start(self: &Arc<Self>) {
        let mut task = self.task.lock().unwrap_or_else(|e| e.into_inner());
        if task.is_some() {
            return;
        }
        let me = Arc::clone(self);
        *task = Some(tokio::spawn(async move { me.run().await }).abort_handle());
    }

    pub(crate) fn stop(&self) {
        if let Some(task) = self.task.lock().unwrap_or_else(|e| e.into_inner()).take() {
            task.abort();
        }
        *self.lock() = State::default();
    }

    async fn run(&self) {
        loop {
            if let Some(sock) = self.socket() {
                if let Err(e) = self.follow(&sock).await {
                    let mut state = self.lock();
                    if state.said.as_deref() != Some(e.as_str()) {
                        (self.opts.log)(&format!("containers are not read: {e}"));
                        state.said = Some(e);
                    }
                }
            }
            {
                let mut state = self.lock();
                state.reached = false;
                state.known.clear();
            }
            tokio::time::sleep(self.opts.retry).await;
        }
    }

    /// The events first, so nothing made between the list and the subscription is missed; then the list once, each
    /// container in it inspected for the pid its cgroup is found by; then only the events, and the stats pass where a
    /// cgroup is out of reach.
    async fn follow(&self, sock: &Path) -> Result<(), String> {
        let mut events = Events::open(sock, EVENTS_QUERY).await?;
        let (status, list) = ask(sock, "GET", "/containers/json").await?;
        if status != 200 {
            return Err(format!("the engine refused its list with {status}"));
        }
        for id in list.as_array().into_iter().flatten().filter_map(|c| c.get("Id").and_then(Value::as_str)) {
            self.look(sock, id).await?;
        }
        {
            let mut state = self.lock();
            state.reached = true;
            state.said = None;
        }
        let mut stats = tokio::time::interval(self.opts.stats_every);
        stats.set_missed_tick_behavior(MissedTickBehavior::Delay);
        loop {
            tokio::select! {
                line = events.next_line() => {
                    let Ok(event) = serde_json::from_slice::<Value>(&line?) else { continue };
                    let Some(id) = string_at(&event, "/Actor/ID") else { continue };
                    match event.get("Action").and_then(Value::as_str) {
                        Some("destroy") => {
                            self.lock().known.remove(&id);
                        }
                        // A failed inspect leaves what was held; a gone engine ends the events next.
                        Some("create" | "start" | "die") => { let _ = self.look(sock, &id).await; }
                        _ => {}
                    }
                }
                _ = stats.tick() => self.stats_pass(sock).await,
            }
        }
    }

    /// Inspects one container and holds it while it runs; one that stopped or went is let go.
    async fn look(&self, sock: &Path, id: &str) -> Result<(), String> {
        if id.len() != 64 || !id.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Ok(());
        }
        let (status, inspect) = ask(sock, "GET", &format!("/containers/{id}/json")).await?;
        let running = status == 200 && inspect.pointer("/State/Running").and_then(Value::as_bool) == Some(true);
        if !running {
            self.lock().known.remove(id);
            return Ok(());
        }
        let label = |name: &str| inspect.pointer("/Config/Labels").and_then(|l| l.get(name)).and_then(Value::as_str).map(str::to_owned);
        let pid = inspect.pointer("/State/Pid").and_then(Value::as_u64).and_then(|p| u32::try_from(p).ok()).filter(|p| *p > 0);
        let cgroup = if self.opts.cgroups { pid.and_then(|pid| self.cgroup_of(pid, id)) } else { None };
        let known = Known {
            name: string_at(&inspect, "/Name").map(|n| n.trim_start_matches('/').to_owned()).unwrap_or_else(|| id[..12].to_owned()),
            image: string_at(&inspect, "/Config/Image").unwrap_or_default(),
            state: string_at(&inspect, "/State/Status").unwrap_or_else(|| "running".to_owned()),
            started_at: string_at(&inspect, "/State/StartedAt").as_deref().and_then(rfc3339_ms),
            label: label(THREAD_LABEL),
            workspace: label(WORKSPACE_LABEL),
            folder: label(COMPOSE_FOLDER),
            project: label(COMPOSE_PROJECT),
            cgroup,
            usage: None,
            memory: None,
            cpu: None,
        };
        let mut state = self.lock();
        match state.known.get_mut(id) {
            // A start after a create, or an event for one already held: the figures run on from what was read.
            Some(held) if held.started_at == known.started_at => {
                *held = Known { usage: held.usage, memory: held.memory, cpu: held.cpu, ..known };
            }
            _ => {
                state.known.insert(id.to_owned(), known);
            }
        }
        Ok(())
    }

    /// The container's own cgroup folder, off the `0::` line of its first process; none unless the path names the
    /// container's id and holds a memory.stat, so a pid out of a VM or a pid handed on never reads another's files.
    fn cgroup_of(&self, pid: u32, id: &str) -> Option<PathBuf> {
        let mut text = String::new();
        std::fs::File::open(self.opts.proc_root.join(pid.to_string()).join("cgroup"))
            .ok()?
            .take(CGROUP_LINE_CAP)
            .read_to_string(&mut text)
            .ok()?;
        let path = text.lines().find_map(|l| l.strip_prefix("0::"))?;
        if !path.contains(id) || path.split('/').any(|part| part == "..") {
            return None;
        }
        let folder = self.opts.cgroup_root.join(path.trim_start_matches('/'));
        folder.join("memory.stat").is_file().then_some(folder)
    }

    /// One-shot stats for each running container whose cgroup is out of reach; nothing is asked where none is.
    async fn stats_pass(&self, sock: &Path) {
        let ids: Vec<String> =
            self.lock().known.iter().filter(|(_, k)| k.cgroup.is_none() && k.state == "running").map(|(id, _)| id.clone()).collect();
        for id in ids {
            let Ok((200, stats)) = ask(sock, "GET", &format!("/containers/{id}/stats?stream=false&one-shot=true")).await else { continue };
            let at = (self.opts.now)();
            let (memory, usage) = stats_figures(&stats);
            if let Some(k) = self.lock().known.get_mut(&id) {
                k.memory = memory.or(k.memory);
                if let Some(usage) = usage {
                    k.cpu = share(k.usage, usage, at).or(k.cpu);
                    k.usage = Some((usage, at));
                }
            }
        }
    }

    /// Every running container as a frame carries it at `at`, its cgroup's figures read now, or None until the engine is
    /// followed. Runs off the runtime thread, inside the process sampler's tick.
    pub(crate) fn rows(&self, at: i64) -> Option<Vec<ProcContainer>> {
        let mut state = self.lock();
        if !state.reached {
            return None;
        }
        refresh_notes(&mut state.notes, &self.opts.notes);
        let State { known, notes, .. } = &mut *state;
        Some(
            known
                .iter_mut()
                .map(|(id, k)| {
                    if let Some(folder) = &k.cgroup {
                        k.memory = std::fs::read_to_string(folder.join("memory.stat")).ok().as_deref().and_then(cgroup_memory);
                        match std::fs::read_to_string(folder.join("cpu.stat")).ok().as_deref().and_then(cgroup_usage_usec) {
                            Some(usec) => {
                                k.cpu = share(k.usage, usec * 1000, at);
                                k.usage = Some((usec * 1000, at));
                            }
                            None => k.cpu = None,
                        }
                    }
                    let placed = place(k.label.as_deref(), k.workspace.as_deref(), k.folder.as_deref(), k.project.as_deref(), &notes.notes);
                    ProcContainer {
                        id: id.clone(),
                        name: k.name.clone(),
                        image: k.image.clone(),
                        state: k.state.clone(),
                        started_at: k.started_at,
                        thread: placed.as_ref().map(|(t, _)| t.clone()),
                        placed_by: placed.map(|(_, by)| by),
                        folder: k.folder.clone(),
                        project: k.project.clone(),
                        memory: k.memory,
                        cpu: k.cpu.map(crate::sys::finite),
                    }
                })
                .collect(),
        )
    }

    /// Stops one container through the engine, two seconds from TERM to KILL, and never removes it.
    /// Only a container a label or a compose note places under a thread: one the person started, or a fork's engine,
    /// is refused, as proc.kill on a box refuses what stands outside the thread cgroups.
    pub(crate) async fn stop_container(&self, id: &str) -> Result<(), OpError> {
        let sock = self.socket().ok_or_else(|| OpError::coded(DaemonErrorCode::NotFound, "no container engine runs on this computer"))?;
        let gone = || OpError::coded(DaemonErrorCode::NotFound, format!("no container {}", &id[..12.min(id.len())]));
        let (status, inspect) = ask(&sock, "GET", &format!("/containers/{id}/json")).await.map_err(OpError::plain)?;
        if status == 404 {
            return Err(gone());
        }
        let label = |name: &str| inspect.pointer("/Config/Labels").and_then(|l| l.get(name)).and_then(Value::as_str).map(str::to_owned);
        let mut notes = NotesCache::default();
        refresh_notes(&mut notes, &self.opts.notes);
        let placed = place(
            label(THREAD_LABEL).as_deref(),
            label(WORKSPACE_LABEL).as_deref(),
            label(COMPOSE_FOLDER).as_deref(),
            label(COMPOSE_PROJECT).as_deref(),
            &notes.notes,
        );
        if placed.is_none() {
            let name =
                string_at(&inspect, "/Name").map_or_else(|| id[..12.min(id.len())].to_owned(), |n| n.trim_start_matches('/').to_owned());
            return Err(OpError::coded(
                DaemonErrorCode::Forbidden,
                format!("{name} was not started by a thread, so wsp does not stop it; stop it with docker stop"),
            ));
        }
        let (status, body) = ask(&sock, "POST", &format!("/containers/{id}/stop?t=2")).await.map_err(OpError::plain)?;
        match status {
            204 | 304 => Ok(()),
            404 => Err(gone()),
            _ => Err(OpError::plain(
                body.get("message").and_then(Value::as_str).map_or_else(|| format!("the engine answered {status}"), str::to_owned),
            )),
        }
    }
}

/// Re-reads the notes folder where a file's name, length or change time moved since the last tick. Each file is opened
/// beneath the folder with no link followed, since a thread writes that folder and a box's daemon is root.
fn refresh_notes(cache: &mut NotesCache, dir: &Path) {
    let Ok(at) = nix::fcntl::open(dir, crate::beneath::dir_flags(), nix::sys::stat::Mode::empty()) else {
        cache.seen.clear();
        cache.notes.clear();
        return;
    };
    let at: OwnedFd = at;
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| thread_word(n))
        .collect();
    names.sort();
    let mut files = Vec::new();
    let mut seen = Vec::new();
    for name in names {
        let Ok(Some(file)) = crate::beneath::file(&at, &name) else { continue };
        let Ok(meta) = file.metadata() else { continue };
        seen.push((
            name,
            meta.len(),
            std::os::unix::fs::MetadataExt::ctime_nsec(&meta) + std::os::unix::fs::MetadataExt::ctime(&meta) * 1_000_000_000,
        ));
        files.push(file);
    }
    if seen == cache.seen {
        return;
    }
    cache.notes = files
        .into_iter()
        .flat_map(|file| {
            let mut text = String::new();
            let _ = file.take(NOTES_CAP).read_to_string(&mut text);
            parse_notes(&text)
        })
        .collect();
    cache.seen = seen;
}

#[cfg(test)]
#[path = "../tests/containers/mod.rs"]
mod tests;
