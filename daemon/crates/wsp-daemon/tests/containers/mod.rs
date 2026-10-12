// SPDX-License-Identifier: AGPL-3.0-only
use super::*;
use crate::proc::{ProcSampler, ProcSamplerOptions};
use crate::Outbound;
use serde_json::json;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixListener;
use tokio::sync::broadcast;

/// An engine on a unix socket that answers the reader's five requests from what the case put in it, holds each events
/// request open for the lines the case sends, and writes down every request line it was sent.
struct Engine {
    _dir: tempfile::TempDir,
    sock: PathBuf,
    asked: Arc<Mutex<Vec<String>>>,
    inspects: Arc<Mutex<HashMap<String, Value>>>,
    stats: Arc<Mutex<HashMap<String, Value>>>,
    events: broadcast::Sender<String>,
    /// Events requests whose connection is still open.
    following: Arc<AtomicUsize>,
}

fn id(n: char) -> String {
    std::iter::repeat_n(n, 64).collect()
}

fn inspect(id: &str, name: &str, running: bool, pid: u32, labels: Value) -> Value {
    json!({
        "Id": id,
        "Name": format!("/{name}"),
        "Config": { "Image": "busybox:1.37", "Labels": labels },
        "State": { "Status": if running { "running" } else { "exited" }, "Running": running, "Pid": pid, "StartedAt": "2026-10-12T04:02:48.524742805Z" },
    })
}

fn event(action: &str, id: &str) -> String {
    json!({ "Type": "container", "Action": action, "Actor": { "ID": id, "Attributes": {} } }).to_string()
}

impl Engine {
    async fn new() -> Engine {
        let dir = tempfile::tempdir().unwrap();
        let sock = dir.path().join("docker.sock");
        let listener = UnixListener::bind(&sock).unwrap();
        let (events, _) = broadcast::channel(64);
        let engine = Engine {
            _dir: dir,
            sock,
            asked: Arc::default(),
            inspects: Arc::default(),
            stats: Arc::default(),
            events: events.clone(),
            following: Arc::default(),
        };
        let (asked, inspects, stats, following) =
            (Arc::clone(&engine.asked), Arc::clone(&engine.inspects), Arc::clone(&engine.stats), Arc::clone(&engine.following));
        tokio::spawn(async move {
            loop {
                let Ok((mut conn, _)) = listener.accept().await else { return };
                let (asked, inspects, stats, following, mut feed) =
                    (Arc::clone(&asked), Arc::clone(&inspects), Arc::clone(&stats), Arc::clone(&following), events.subscribe());
                tokio::spawn(async move {
                    let mut head = Vec::new();
                    let mut byte = [0u8; 1];
                    while !head.ends_with(b"\r\n\r\n") {
                        if conn.read(&mut byte).await.unwrap_or(0) == 0 {
                            return;
                        }
                        head.push(byte[0]);
                    }
                    let line = String::from_utf8_lossy(&head).lines().next().unwrap_or("").trim_end_matches(" HTTP/1.1").to_owned();
                    asked.lock().unwrap().push(line.clone());
                    let (method, path) = line.split_once(' ').unwrap();
                    let answer = |status: u16, body: &Value| {
                        let text = if body.is_null() { String::new() } else { body.to_string() };
                        format!("HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{text}", text.len())
                    };
                    if path.starts_with("/events?") {
                        following.fetch_add(1, Ordering::SeqCst);
                        let _ = conn
                            .write_all(b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nTransfer-Encoding: chunked\r\n\r\n")
                            .await;
                        let mut gone = [0u8; 1];
                        loop {
                            tokio::select! {
                                line = feed.recv() => {
                                    let Ok(line) = line else { break };
                                    let body = format!("{line}\n");
                                    if conn.write_all(format!("{:x}\r\n{body}\r\n", body.len()).as_bytes()).await.is_err() { break; }
                                }
                                n = conn.read(&mut gone) => if n.unwrap_or(0) == 0 { break },
                            }
                        }
                        following.fetch_sub(1, Ordering::SeqCst);
                        return;
                    }
                    let parts: Vec<&str> = path.trim_start_matches('/').split(['/', '?']).collect();
                    let text = match (method, parts.as_slice()) {
                        ("GET", ["containers", "json"]) => {
                            let held = inspects.lock().unwrap();
                            let running: Vec<Value> = held
                                .values()
                                .filter(|i| i["State"]["Running"] == true)
                                .map(
                                    |i| json!({ "Id": i["Id"], "Names": [i["Name"]], "Labels": i["Config"]["Labels"], "State": "running" }),
                                )
                                .collect();
                            answer(200, &Value::Array(running))
                        }
                        ("GET", ["containers", id, "json"]) => match inspects.lock().unwrap().get(*id) {
                            Some(i) => answer(200, i),
                            None => answer(404, &json!({ "message": "No such container" })),
                        },
                        ("GET", ["containers", id, "stats", ..]) => match stats.lock().unwrap().get(*id) {
                            Some(s) => answer(200, s),
                            None => answer(404, &json!({ "message": "No such container" })),
                        },
                        ("POST", ["containers", id, "stop", ..]) => {
                            let mut held = inspects.lock().unwrap();
                            match held.get_mut(*id) {
                                Some(i) if i["State"]["Running"] == true => {
                                    i["State"]["Running"] = json!(false);
                                    answer(204, &Value::Null)
                                }
                                Some(_) => answer(304, &Value::Null),
                                None => answer(404, &json!({ "message": "No such container" })),
                            }
                        }
                        _ => answer(404, &json!({ "message": "page not found" })),
                    };
                    let _ = conn.write_all(text.as_bytes()).await;
                });
            }
        });
        engine
    }

    fn hold(&self, i: Value) {
        self.inspects.lock().unwrap().insert(i["Id"].as_str().unwrap().to_owned(), i);
    }

    fn asked(&self) -> Vec<String> {
        self.asked.lock().unwrap().clone()
    }

    fn count(&self, f: impl Fn(&str) -> bool) -> usize {
        self.asked().iter().filter(|l| f(l)).count()
    }
}

/// A /proc and a cgroup tree holding one container's first process and its own cgroup's two files.
struct Cgroups {
    proc_root: tempfile::TempDir,
    cgroup_root: tempfile::TempDir,
}

impl Cgroups {
    fn new() -> Cgroups {
        Cgroups { proc_root: tempfile::tempdir().unwrap(), cgroup_root: tempfile::tempdir().unwrap() }
    }

    fn container(&self, pid: u32, path: &str, memory_stat: &str, usage_usec: u64) {
        let p = self.proc_root.path().join(pid.to_string());
        std::fs::create_dir_all(&p).unwrap();
        std::fs::write(p.join("cgroup"), format!("0::{path}\n")).unwrap();
        let cg = self.cgroup_root.path().join(path.trim_start_matches('/'));
        std::fs::create_dir_all(&cg).unwrap();
        std::fs::write(cg.join("memory.stat"), memory_stat).unwrap();
        std::fs::write(cg.join("cpu.stat"), format!("usage_usec {usage_usec}\nuser_usec 1\nsystem_usec 1\n")).unwrap();
    }

    fn usage(&self, path: &str, usage_usec: u64) {
        std::fs::write(self.cgroup_root.path().join(path.trim_start_matches('/')).join("cpu.stat"), format!("usage_usec {usage_usec}\n"))
            .unwrap();
    }
}

fn reader(engine: &Engine, notes: &Path, cg: &Cgroups, cgroups: bool, stats_every: Duration) -> Arc<ContainerReader> {
    ContainerReader::new(ReaderOptions {
        home: None,
        docker_host: Some(format!("unix://{}", engine.sock.display())),
        notes: notes.to_path_buf(),
        proc_root: cg.proc_root.path().to_path_buf(),
        cgroup_root: cg.cgroup_root.path().to_path_buf(),
        cgroups,
        stats_every,
        retry: Duration::from_millis(50),
        now: Arc::new(crate::sys::now_ms),
        log: Arc::new(|_| {}),
    })
}

async fn until(what: &str, f: impl Fn() -> bool) {
    for _ in 0..200 {
        if f() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("{what} never came");
}

const MEMORY_STAT: &str = "anon 131072\nfile 86016\nkernel 319488\nkernel_stack 16384\nsock 0\nslab 224896\n";

#[tokio::test]
async fn the_reader_holds_one_subscription_lists_once_and_inspects_only_on_create_start_die_and_destroy() {
    let engine = Engine::new().await;
    let cg = Cgroups::new();
    let notes = tempfile::tempdir().unwrap();
    let (a, b, c) = (id('a'), id('b'), id('c'));
    for (n, pid) in [(&a, 101), (&b, 102)] {
        let path = format!("/system.slice/docker-{n}.scope");
        cg.container(pid, &path, MEMORY_STAT, 1_000);
        engine.hold(inspect(n, &format!("c{pid}"), true, pid, json!({})));
    }
    let r = reader(&engine, notes.path(), &cg, true, Duration::from_secs(3600));
    r.start();
    until("the first list", || r.rows(1).is_some()).await;
    // Fifty ticks with nothing moving ask the engine nothing.
    for at in 0..50 {
        assert_eq!(r.rows(1000 + at * 2000).unwrap().len(), 2);
    }
    let before = engine.asked().len();
    engine.hold(inspect(&c, "c103", false, 0, json!({})));
    engine.events.send(event("create", &c)).unwrap();
    engine.events.send(event("exec_create", &a)).unwrap();
    until("the create's inspect", || engine.asked().len() == before + 1).await;
    engine.hold(inspect(&c, "c103", true, 103, json!({})));
    engine.events.send(event("start", &c)).unwrap();
    until("the start's inspect", || r.rows(200_000).unwrap().iter().any(|row| row.id == c)).await;
    engine.hold(inspect(&a, "c101", false, 0, json!({})));
    engine.events.send(event("die", &a)).unwrap();
    until("the die's inspect", || !r.rows(202_000).unwrap().iter().any(|row| row.id == a)).await;
    engine.inspects.lock().unwrap().remove(&a);
    engine.events.send(event("destroy", &b)).unwrap();
    until("the destroy", || !r.rows(204_000).unwrap().iter().any(|row| row.id == b)).await;
    for at in 0..50 {
        r.rows(300_000 + at * 2000);
    }
    let asked = engine.asked();
    assert_eq!(engine.count(|l| l.starts_with("GET /events?")), 1, "{asked:?}");
    let events = asked.iter().find(|l| l.starts_with("GET /events?")).unwrap();
    let query = events
        .replace("%22", "\"")
        .replace("%7B", "{")
        .replace("%7D", "}")
        .replace("%5B", "[")
        .replace("%5D", "]")
        .replace("%3A", ":")
        .replace("%2C", ",");
    assert!(query.ends_with(r#"filters={"type":["container"],"event":["create","start","die","destroy"]}"#), "{query}");
    assert_eq!(engine.count(|l| l == "GET /containers/json"), 1, "{asked:?}");
    // The list's two, then create, start and die; destroy and exec_create inspect nothing.
    let inspected: Vec<&String> =
        asked.iter().filter(|l| l.ends_with("/json") && l.starts_with("GET /containers/") && *l != "GET /containers/json").collect();
    assert_eq!(inspected.len(), 5, "{asked:?}");
    assert_eq!(engine.count(|l| l.contains("/stats")), 0, "{asked:?}");
    r.stop();
    until("the subscription's close", || engine.following.load(Ordering::SeqCst) == 0).await;
}

fn sampler_opts() -> ProcSamplerOptions {
    ProcSamplerOptions {
        self_pid: 1,
        ptys: Arc::new(Vec::new),
        first: Duration::from_millis(20),
        interval: Duration::from_millis(20),
        now: Arc::new(crate::sys::now_ms),
        log: Arc::new(|_| {}),
    }
}

struct NoProcs;
impl crate::proc_owner::OwnerFacts for NoProcs {}
impl crate::proc::ProcSource for NoProcs {
    fn scan(&self, _: &crate::proc::ProcScanInput) -> Result<crate::proc::ProcScan, String> {
        Ok(crate::proc::ProcScan { total: 0, procs: vec![], child_ms: 0.0 })
    }
    fn inspect(&self, pid: u32, _: &[wsp_frames::ProcEntry]) -> Result<wsp_frames::ProcInspectReply, OpError> {
        Err(crate::proc::no_process(pid))
    }
}

#[tokio::test]
async fn a_watch_starts_the_reader_its_frames_carry_the_containers_and_the_last_unwatch_closes_the_subscription() {
    let engine = Engine::new().await;
    let cg = Cgroups::new();
    let notes = tempfile::tempdir().unwrap();
    let a = id('a');
    cg.container(101, &format!("/system.slice/docker-{a}.scope"), MEMORY_STAT, 1_000);
    engine.hold(inspect(&a, "lab-db-1", true, 101, json!({ "wsp.thread": "t_one" })));
    let s = ProcSampler::new(Arc::new(NoProcs), sampler_opts());
    s.read_containers(reader(&engine, notes.path(), &cg, true, Duration::from_secs(3600)));
    tokio::time::sleep(Duration::from_millis(60)).await;
    assert!(engine.asked().is_empty(), "nothing is read while nobody watches");
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    s.subscribe(1, Outbound(tx), Some(vec!["/wsp-threads".into()]));
    let mut carried = None;
    for _ in 0..100 {
        let Some(out) = rx.recv().await else { break };
        let frame: Value = serde_json::from_str(out.text()).unwrap();
        if let Some(rows) = frame.get("containers").filter(|c| c.as_array().is_some_and(|c| !c.is_empty())) {
            carried = Some(rows.clone());
            break;
        }
    }
    let rows = carried.expect("no frame carried the container");
    assert_eq!(rows[0]["thread"], "t_one");
    assert_eq!(rows[0]["placedBy"], "label");
    assert_eq!(rows[0]["memory"], 131072 + 319488);
    // Ticks on: the list is never read again.
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(engine.count(|l| l == "GET /containers/json"), 1);
    s.unsubscribe(1);
    until("the subscription's close", || engine.following.load(Ordering::SeqCst) == 0).await;
    // A watch that starts again subscribes and lists once more, once.
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    s.subscribe(2, Outbound(tx), None);
    until("the second list", || engine.count(|l| l == "GET /containers/json") == 2).await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!((engine.count(|l| l.starts_with("GET /events?")), engine.count(|l| l == "GET /containers/json")), (2, 2));
    s.unsubscribe(2);
}

fn note(thread: &str, folder: &str, project: &str, at: i64) -> Note {
    Note { thread: thread.into(), folder: folder.into(), project: project.into(), at }
}

#[test]
fn a_container_goes_under_its_label_else_the_newest_compose_note_inside_its_folder_else_no_thread() {
    use ContainerPlacedBy::{Compose, Label};
    let notes = vec![
        note("t_old", "/srv/acme/lab", "", 100),
        note("t_new", "/srv/acme/lab/web", "", 200),
        note("t_other", "/srv/acme/lab", "other", 300),
        note("t_sibling", "/srv/acme/lab-two", "", 400),
    ];
    let placed = |label: Option<&str>, folder: Option<&str>, project: Option<&str>| place(label, None, folder, project, &notes);
    // The label names the thread whatever the notes say.
    assert_eq!(placed(Some("t_label"), Some("/srv/acme/lab"), Some("lab")), Some(("t_label".into(), Label)));
    // The newest note inside the folder; the newer one naming another project, and the sibling folder whose name only
    // starts the same, are passed over.
    assert_eq!(placed(None, Some("/srv/acme/lab"), Some("lab")), Some(("t_new".into(), Compose)));
    // A note that named the project wins where it is the newest that matches.
    assert_eq!(placed(None, Some("/srv/acme/lab"), Some("other")), Some(("t_other".into(), Compose)));
    // A note in a folder above the project's is not inside it.
    assert_eq!(placed(None, Some("/srv/acme/lab/web/deeper"), Some("lab")), None);
    // No label and no note inside: no thread, and the row keeps its folder for the page to name.
    assert_eq!(placed(None, Some("/srv/elsewhere"), Some("x")), None);
    assert_eq!(placed(None, None, None), None);
    // A label that is no thread's word is no label.
    assert_eq!(placed(Some("../etc"), Some("/srv/acme/lab"), Some("lab")), Some(("t_new".into(), Compose)));
    // A fork's container, which its fence labelled with the workspace, is never a note's; its own thread label still is.
    assert_eq!(place(None, Some("ws_fork"), Some("/srv/acme/lab"), Some("lab"), &notes), None);
    assert_eq!(place(Some("t_label"), Some("ws_fork"), Some("/srv/acme/lab"), Some("lab"), &notes), Some(("t_label".into(), Label)));
    #[cfg(target_os = "linux")]
    assert_eq!(WORKSPACE_LABEL, wsp_runtime::engine::LABEL);
    assert_eq!(
        parse_notes(
            "t_a\t/srv/acme/lab\t\t1791777782\nbroken line\nt_b\trelative\t\t1\n../x\t/a\t\t1\nt_c\t/srv/acme/lab\tlab\t1791777790\n"
        ),
        vec![note("t_a", "/srv/acme/lab", "", 1791777782), note("t_c", "/srv/acme/lab", "lab", 1791777790)]
    );
}

#[tokio::test]
async fn rows_place_by_the_shims_notes_file_and_name_the_folder_of_one_under_no_thread() {
    let engine = Engine::new().await;
    let cg = Cgroups::new();
    let notes = tempfile::tempdir().unwrap();
    let (a, b, c) = (id('a'), id('b'), id('c'));
    let compose =
        |project: &str, folder: &str| json!({ "com.docker.compose.project": project, "com.docker.compose.project.working_dir": folder });
    engine.hold(inspect(&a, "lab-db-1", true, 0, compose("lab", "/srv/acme/lab")));
    engine.hold(inspect(&b, "scratch", true, 0, json!({ "wsp.thread": "t_two" })));
    engine.hold(inspect(&c, "other-web-1", true, 0, compose("other", "/srv/other")));
    // The shim's own line, written by its printf.
    std::fs::write(notes.path().join("t_one"), "t_one\t/srv/acme/lab\t\t1791777782\n").unwrap();
    let r = reader(&engine, notes.path(), &cg, true, Duration::from_secs(3600));
    r.start();
    until("the first list", || r.rows(1).is_some_and(|rows| rows.len() == 3)).await;
    let rows: HashMap<String, ProcContainer> = r.rows(2).unwrap().into_iter().map(|row| (row.name.clone(), row)).collect();
    assert_eq!((rows["lab-db-1"].thread.as_deref(), rows["lab-db-1"].placed_by), (Some("t_one"), Some(ContainerPlacedBy::Compose)));
    assert_eq!((rows["scratch"].thread.as_deref(), rows["scratch"].placed_by), (Some("t_two"), Some(ContainerPlacedBy::Label)));
    assert_eq!((rows["other-web-1"].thread.as_deref(), rows["other-web-1"].folder.as_deref()), (None, Some("/srv/other")));
    // A newer note from another thread moves the project on the next tick, with no event from the engine.
    std::fs::write(notes.path().join("t_three"), "t_three\t/srv/acme/lab\tlab\t1791777999\n").unwrap();
    let moved: HashMap<String, ProcContainer> = r.rows(3).unwrap().into_iter().map(|row| (row.name.clone(), row)).collect();
    assert_eq!(moved["lab-db-1"].thread.as_deref(), Some("t_three"));
    // A link in the folder is read as no file, so a root daemon never reads what it points at.
    std::fs::remove_file(notes.path().join("t_three")).unwrap();
    let elsewhere = tempfile::tempdir().unwrap();
    std::fs::write(elsewhere.path().join("x"), "t_four\t/srv/acme/lab\t\t1791779999\n").unwrap();
    std::os::unix::fs::symlink(elsewhere.path().join("x"), notes.path().join("t_four")).unwrap();
    let linked: HashMap<String, ProcContainer> = r.rows(4).unwrap().into_iter().map(|row| (row.name.clone(), row)).collect();
    assert_eq!(linked["lab-db-1"].thread.as_deref(), Some("t_one"));
    r.stop();
}

#[test]
fn memory_is_the_cgroups_anon_and_kernel_and_cpu_runs_off_usage_usec() {
    assert_eq!(cgroup_memory(MEMORY_STAT), Some(131072 + 319488));
    // A kernel before 5.18 writes no kernel line: its parts stand in.
    assert_eq!(cgroup_memory("anon 100\nkernel_stack 10\npagetables 20\nslab 30\nsock 5\nfile 999\n"), Some(165));
    assert_eq!(cgroup_memory("file 10\n"), None);
    assert_eq!(cgroup_usage_usec("usage_usec 3177725\nuser_usec 3096117\n"), Some(3177725));
    // A second of cpu over two seconds of wall is half a core.
    assert_eq!(share(Some((1_000_000_000, 0)), 2_000_000_000, 2000), Some(50.0));
    assert_eq!(share(None, 2_000_000_000, 2000), None);
}

#[tokio::test]
async fn on_a_box_figures_come_from_the_cgroup_its_pid_names_and_never_from_a_cgroup_that_is_not_its_own() {
    let engine = Engine::new().await;
    let cg = Cgroups::new();
    let notes = tempfile::tempdir().unwrap();
    let (a, b) = (id('a'), id('b'));
    let path = format!("/system.slice/docker-{a}.scope");
    cg.container(101, &path, MEMORY_STAT, 1_000_000);
    // b's pid stands in a cgroup that does not name it, as a pid inside a VM or one handed on would.
    cg.container(102, "/user.slice/user-1000.slice", MEMORY_STAT, 1_000_000);
    engine.hold(inspect(&a, "a", true, 101, json!({})));
    engine.hold(inspect(&b, "b", true, 102, json!({})));
    let r = reader(&engine, notes.path(), &cg, true, Duration::from_secs(3600));
    r.start();
    until("the first list", || r.rows(1_000).is_some_and(|rows| rows.len() == 2)).await;
    cg.usage(&path, 1_500_000);
    let rows: HashMap<String, ProcContainer> = r.rows(3_000).unwrap().into_iter().map(|row| (row.name.clone(), row)).collect();
    // Half a second of cpu over two seconds is a quarter of a core.
    assert_eq!((rows["a"].memory, rows["a"].cpu), (Some(131072 + 319488), Some(25.0)));
    assert_eq!((rows["b"].memory, rows["b"].cpu), (None, None));
    // b is read off the engine's stats instead, which this engine has none of: never a stranger's files.
    r.stop();
}

#[tokio::test]
async fn where_no_cgroup_is_reached_one_shot_stats_every_ten_seconds_give_memory_and_cpu_from_two_reads_in_a_row() {
    let engine = Engine::new().await;
    let cg = Cgroups::new();
    let notes = tempfile::tempdir().unwrap();
    let a = id('a');
    engine.hold(inspect(&a, "a", true, 4242, json!({})));
    let stats = |ns: u64| json!({ "memory_stats": { "usage": 999_999_999, "stats": { "anon": 1000, "slab": 200, "kernel_stack": 30, "sock": 4, "inactive_file": 5 } }, "cpu_stats": { "cpu_usage": { "total_usage": ns } } });
    engine.stats.lock().unwrap().insert(a.clone(), stats(1_000_000_000));
    // A Mac: no cgroup is looked for at all. The pass runs every 300 ms here in place of ten seconds.
    let r = reader(&engine, notes.path(), &cg, false, Duration::from_millis(300));
    r.start();
    until("the first stats", || r.rows(1).is_some_and(|rows| rows[0].memory.is_some())).await;
    let first = r.rows(2).unwrap();
    assert_eq!((first[0].memory, first[0].cpu), (Some(1234), None));
    engine.stats.lock().unwrap().insert(a.clone(), stats(1_150_000_000));
    until("the second stats", || r.rows(3).unwrap()[0].cpu.is_some()).await;
    let cpu = r.rows(4).unwrap()[0].cpu.unwrap();
    // 150 ms of cpu over the pass's 300 ms is about half a core.
    assert!((35.0..65.0).contains(&cpu), "{cpu}");
    // Ticks between passes ask the engine nothing: two passes' worth of stats over a second, never one a tick.
    let before = engine.count(|l| l.contains("/stats?stream=false&one-shot=true"));
    for at in 0..50 {
        r.rows(10 + at);
    }
    tokio::time::sleep(Duration::from_millis(650)).await;
    let passes = engine.count(|l| l.contains("/stats?stream=false&one-shot=true")) - before;
    assert!((1..=3).contains(&passes), "{passes} passes in 650 ms");
    assert_eq!(engine.count(|l| l.starts_with("GET /proc")), 0);
    r.stop();
}

#[test]
fn the_engines_stats_memory_is_anon_slab_kernel_stack_and_sock() {
    let stats = json!({ "memory_stats": { "usage": 536576, "stats": { "anon": 131072, "slab": 224896, "kernel_stack": 16384, "sock": 0, "file": 86016 } }, "cpu_stats": { "cpu_usage": { "total_usage": 3177725000u64 } } });
    assert_eq!(stats_figures(&stats), (Some(131072 + 224896 + 16384), Some(3177725000)));
    assert_eq!(stats_figures(&json!({})), (None, None));
}

#[tokio::test]
async fn end_stops_a_container_through_the_engines_stop_with_two_seconds_and_never_removes_it() {
    let engine = Engine::new().await;
    let cg = Cgroups::new();
    let notes = tempfile::tempdir().unwrap();
    let a = id('a');
    engine.hold(inspect(&a, "a", true, 0, json!({ "wsp.thread": "t_one" })));
    let r = reader(&engine, notes.path(), &cg, false, Duration::from_secs(3600));
    r.stop_container(&a).await.unwrap();
    // Stopped already reads as done; gone is not found.
    r.stop_container(&a).await.unwrap();
    let err = r.stop_container(&id('f')).await.unwrap_err();
    assert_eq!(err.code, Some(DaemonErrorCode::NotFound));
    let stop = format!("POST /containers/{a}/stop?t=2");
    let look = |c: &str| format!("GET /containers/{c}/json");
    assert_eq!(engine.asked(), vec![look(&a), stop.clone(), look(&a), stop, look(&id('f'))]);
    assert_eq!(engine.count(|l| l.starts_with("DELETE") || l.contains("remove") || l.contains("kill")), 0);
}

#[tokio::test]
async fn end_refuses_a_container_no_label_or_note_places_whether_the_person_or_a_forks_engine_started_it() {
    let engine = Engine::new().await;
    let cg = Cgroups::new();
    let notes = tempfile::tempdir().unwrap();
    let (by_hand, fenced, noted) = (id('a'), id('b'), id('c'));
    engine.hold(inspect(&by_hand, "scratch", true, 0, json!({})));
    // A fork's container on a box: its engine's fence labels it with the workspace, never a thread.
    engine.hold(inspect(&fenced, "wsp-ws_fork-db", true, 0, json!({ "wsp.workspace": "ws_fork" })));
    engine.hold(inspect(
        &noted,
        "lab-db-1",
        true,
        0,
        json!({ "com.docker.compose.project": "lab", "com.docker.compose.project.working_dir": "/srv/acme/lab" }),
    ));
    std::fs::write(notes.path().join("t_one"), "t_one\t/srv/acme/lab\t\t1791777782\n").unwrap();
    let r = reader(&engine, notes.path(), &cg, false, Duration::from_secs(3600));
    for (c, name) in [(&by_hand, "scratch"), (&fenced, "wsp-ws_fork-db")] {
        let err = r.stop_container(c).await.unwrap_err();
        assert_eq!(err.code, Some(DaemonErrorCode::Forbidden), "{name}");
        assert_eq!(err.message, format!("{name} was not started by a thread, so wsp does not stop it; stop it with docker stop"));
    }
    // A compose project a thread's note names is that thread's, and stops.
    r.stop_container(&noted).await.unwrap();
    assert_eq!(engine.count(|l| l.starts_with("POST")), 1);
    assert_eq!(engine.count(|l| l == format!("POST /containers/{noted}/stop?t=2")), 1);
}

#[tokio::test]
async fn a_forks_container_is_never_placed_by_a_note_so_it_stands_under_no_thread_and_its_stop_is_refused() {
    let engine = Engine::new().await;
    let cg = Cgroups::new();
    let notes = tempfile::tempdir().unwrap();
    let fork = id('a');
    // A fork's compose up on a box: the fence adds wsp.workspace and keeps compose's own labels.
    engine.hold(inspect(
        &fork,
        "wsp-ws_fork-db-1",
        true,
        0,
        json!({
            "wsp.workspace": "ws_fork",
            "com.docker.compose.project": "wsp-ws_fork",
            "com.docker.compose.project.working_dir": "/root/proj",
        }),
    ));
    // A box thread's plain compose up in the same folder, its project unnamed.
    std::fs::write(notes.path().join("t_one"), "t_one\t/root/proj\t\t1791777782\n").unwrap();
    let r = reader(&engine, notes.path(), &cg, false, Duration::from_secs(3600));
    r.start();
    until("the first list", || r.rows(1).is_some_and(|rows| rows.len() == 1)).await;
    let row = r.rows(2).unwrap().remove(0);
    assert_eq!((row.thread.as_deref(), row.placed_by, row.folder.as_deref()), (None, None, Some("/root/proj")));
    let err = r.stop_container(&fork).await.unwrap_err();
    assert_eq!(err.code, Some(DaemonErrorCode::Forbidden));
    assert_eq!(err.message, "wsp-ws_fork-db-1 was not started by a thread, so wsp does not stop it; stop it with docker stop");
    assert_eq!(engine.count(|l| l.starts_with("POST")), 0);
    r.stop();
}

#[test]
fn the_engines_start_time_reads_as_epoch_milliseconds_and_its_zero_time_as_none() {
    assert_eq!(rfc3339_ms("2026-10-12T04:02:48.524742805Z"), Some(1_791_777_768_524));
    assert_eq!(rfc3339_ms("1970-01-02T00:00:00Z"), None);
    assert_eq!(rfc3339_ms("2026-10-12T09:32:48.5+05:30"), Some(1_791_777_768_500));
    assert_eq!(rfc3339_ms("0001-01-01T00:00:00Z"), None);
}
