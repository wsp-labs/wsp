// SPDX-License-Identifier: AGPL-3.0-only
use super::fake_proc::{auxv, fake_passwd, fake_proc_tree, write_proc, FakeProc, BTIME};
use super::*;
use crate::sys::now_ms;
use crate::Outgoing;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use tokio::sync::mpsc;

#[test]
fn a_rows_cgroup_is_the_v2_line_of_its_cgroup_file_and_nothing_where_there_is_none() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("cgroup"), "12:pids:/legacy\n0::/wsp-threads/t_abc\n").unwrap();
    assert_eq!(read_cgroup(dir.path()).as_deref(), Some("/wsp-threads/t_abc"));
    std::fs::write(dir.path().join("cgroup"), "1:name=systemd:/\n").unwrap();
    assert_eq!(read_cgroup(dir.path()), None);
    std::fs::remove_file(dir.path().join("cgroup")).unwrap();
    assert_eq!(read_cgroup(dir.path()), None);
}

#[test]
fn reads_proc_pid_stat_around_a_comm_that_holds_spaces_and_parens() {
    let zeros = vec!["0"; 28].join(" ");
    let st =
        parse_proc_pid_stat(&format!("42 (my (odd) proc) R 7 42 42 0 -1 4194560 10 0 0 0 150 50 0 0 20 0 3 0 12345 1000000 256 {zeros}\n"))
            .unwrap();
    assert_eq!(
        st,
        ProcStat { pid: 42, comm: "my (odd) proc".into(), state: "R".into(), ppid: 7, pgrp: 42, ticks: 200, threads: 3, starttime: 12345 }
    );
}

#[test]
fn a_scan_narrowed_to_cgroups_reads_their_members_and_the_daemon_and_still_counts_every_process() {
    let procs: Vec<FakeProc> =
        [1, 5, 70, 71, 72].into_iter().map(|pid| FakeProc { pid, comm: Some(format!("p{pid}")), ..FakeProc::default() }).collect();
    let root = fake_proc_tree(&procs, 4096);
    let (_passwd_dir, passwd) = fake_passwd();
    let cgroups = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(cgroups.path().join("wsp-threads/t1")).unwrap();
    std::fs::write(cgroups.path().join("wsp-threads/t1/cgroup.procs"), "70\n71\n").unwrap();
    let source = ProcFsSource::new(root.path().to_path_buf(), passwd, numbers::PROC_CAP).with_cgroup_root(cgroups.path().to_path_buf());
    let scan = |only: Option<Vec<String>>| {
        source.scan(&ProcScanInput { at: 1, elapsed_ms: 0, pty: HashMap::new(), only, daemon: 5, scopes: Default::default() }).unwrap()
    };
    let narrowed = scan(Some(vec!["/wsp-threads/t1".into(), "/wsp-threads/gone".into()]));
    assert_eq!((narrowed.procs.iter().map(|p| p.pid).collect::<Vec<_>>(), narrowed.total), (vec![5, 70, 71], 5));
    assert_eq!(scan(None).procs.iter().map(|p| p.pid).collect::<Vec<_>>(), vec![1, 5, 70, 71, 72]);
}

#[test]
fn a_rows_memory_is_its_private_pages_and_a_thread_cgroups_is_its_anon_and_kernel_never_resident_or_page_cache() {
    // An idle node on a box: 10,150 resident pages, 8,640 of them its binary and libraries mapped from files.
    let procs = [FakeProc { pid: 70, rss: Some(10_150), shared: Some(8_640), ..FakeProc::default() }];
    let root = fake_proc_tree(&procs, 4096);
    let (_passwd_dir, passwd) = fake_passwd();
    let cgroups = tempfile::tempdir().unwrap();
    let thread = cgroups.path().join("wsp-threads/t1");
    std::fs::create_dir_all(thread.join("made-inside")).unwrap();
    std::fs::write(thread.join("cgroup.procs"), "70\n").unwrap();
    // 680 MB current, of it 435 MB page cache a grep read: the thread holds its anon and kernel bytes.
    std::fs::write(thread.join("memory.current"), "713031680\n").unwrap();
    std::fs::write(thread.join("memory.stat"), "anon 231735296\nfile 456130560\nkernel 27262976\nkernel_stack 1048576\nslab 20971520\n")
        .unwrap();
    // A thread cgroup made before its folder turned the memory controller on has no memory.stat, and no figure.
    std::fs::create_dir_all(cgroups.path().join("wsp-threads/t2")).unwrap();
    let source = ProcFsSource::new(root.path().to_path_buf(), passwd, numbers::PROC_CAP).with_cgroup_root(cgroups.path().to_path_buf());
    let scan = source
        .scan(&ProcScanInput { at: 1, elapsed_ms: 0, pty: HashMap::new(), only: None, daemon: 5, scopes: Default::default() })
        .unwrap();
    assert_eq!(scan.procs.iter().find(|p| p.pid == 70).map(|p| p.rss), Some(1_510 * 4096));
    assert_eq!(scan.cgroups, Some(vec![CgroupMemory { cgroup: "/wsp-threads/t1".into(), memory: 231_735_296 + 27_262_976 }]));
    // A computer with no thread cgroups names none.
    let bare = tempfile::tempdir().unwrap();
    let (_passwd_dir, passwd) = fake_passwd();
    let source = ProcFsSource::new(root.path().to_path_buf(), passwd, numbers::PROC_CAP).with_cgroup_root(bare.path().to_path_buf());
    let scan = source
        .scan(&ProcScanInput { at: 1, elapsed_ms: 0, pty: HashMap::new(), only: None, daemon: 5, scopes: Default::default() })
        .unwrap();
    assert_eq!(scan.cgroups, None);
}

#[test]
fn statm_private_pages_are_resident_less_shared() {
    assert_eq!(parse_statm_private("40600 10150 8640 1200 0 9000 0\n"), Some(1_510));
    assert_eq!(parse_statm_private("1 2 5 0 0 0 0"), Some(0));
    assert_eq!(parse_statm_private("40600"), None);
}

#[test]
fn a_scan_narrowed_to_a_cgroup_reads_every_cgroup_under_it_as_a_thread_end_takes_them() {
    let procs: Vec<FakeProc> =
        [1, 5, 70, 71, 72, 73].into_iter().map(|pid| FakeProc { pid, comm: Some(format!("p{pid}")), ..FakeProc::default() }).collect();
    let root = fake_proc_tree(&procs, 4096);
    let (_passwd_dir, passwd) = fake_passwd();
    let cgroups = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(cgroups.path().join("wsp-threads/t1/made-inside")).unwrap();
    std::fs::create_dir_all(cgroups.path().join("wsp-threads/t2")).unwrap();
    std::fs::write(cgroups.path().join("wsp-threads/t1/cgroup.procs"), "70\n").unwrap();
    std::fs::write(cgroups.path().join("wsp-threads/t1/made-inside/cgroup.procs"), "71\n").unwrap();
    std::fs::write(cgroups.path().join("wsp-threads/t2/cgroup.procs"), "72\n").unwrap();
    let source = ProcFsSource::new(root.path().to_path_buf(), passwd, numbers::PROC_CAP).with_cgroup_root(cgroups.path().to_path_buf());
    let pids = |only: &str| {
        source
            .scan(&ProcScanInput {
                at: 1,
                elapsed_ms: 0,
                pty: HashMap::new(),
                only: Some(vec![only.into()]),
                daemon: 5,
                scopes: Default::default(),
            })
            .unwrap()
            .procs
            .iter()
            .map(|p| p.pid)
            .collect::<Vec<_>>()
    };
    assert_eq!(pids("/wsp-threads/t1"), vec![5, 70, 71]);
    assert_eq!(pids("/wsp-threads"), vec![5, 70, 71, 72]);
}

#[test]
fn a_tick_reads_every_process_once_any_watcher_asks_for_that_and_the_cgroups_they_name_together_otherwise() {
    let mut inner = Inner::default();
    inner.scopes.insert(1, Some(vec!["/wsp-threads/b".into(), "/wsp-threads/a".into()]));
    inner.scopes.insert(2, Some(vec!["/wsp-threads/a".into()]));
    assert_eq!(ProcSampler::scope(&inner), Some(vec!["/wsp-threads/a".to_string(), "/wsp-threads/b".to_string()]));
    inner.scopes.insert(3, None);
    assert_eq!(ProcSampler::scope(&inner), None);
}

#[test]
fn the_rows_held_count_the_owner_each_carries() {
    let row = ProcEntry {
        pid: 1,
        ppid: 0,
        user: String::new(),
        state: String::new(),
        comm: String::new(),
        cmdline: String::new(),
        cpu: 0.0,
        rss: 0,
        started_at: 0,
        pty: None,
        cgroup: None,
        energy_mw: None,
        pgid: None,
        owner: None,
        kind: None,
        id: None,
    };
    let (thread, workspace) = (String::from("t_0123456789abcdef"), String::from("w_1"));
    let owner = wsp_frames::ProcOwner {
        by: wsp_frames::ProcOwnedBy::Root,
        thread: Some(thread.clone()),
        workspace: Some(workspace.clone()),
        warm: false,
    };
    let owned = ProcEntry { owner: Some(owner), ..row.clone() };
    assert_eq!(held_bytes(&[owned]) - held_bytes(&[row]), (thread.capacity() + workspace.capacity()) as u64);
}

#[test]
fn rejects_a_stat_line_without_the_comm_parens() {
    assert!(parse_proc_pid_stat("42 broken\n").is_err());
    assert!(parse_proc_pid_stat("42 (short) R 7\n").is_err());
}

#[test]
fn btime_the_page_size_from_auxv_and_users_from_passwd() {
    assert_eq!(parse_btime(&format!("cpu 1 2 3\nbtime {BTIME}\n")).unwrap(), BTIME);
    assert!(parse_btime("cpu 1 2 3\n").is_err());
    assert_eq!(parse_auxv_page_size(&auxv(16384)), Some(16384));
    assert_eq!(parse_auxv_page_size(&[0u8; 16]), None);
    let users = parse_passwd_users("root:x:0:0:root:/root:/bin/bash\nnobody:x:65534:65534::/:/bin/false\nbad line\n");
    assert_eq!(users, HashMap::from([(0, "root".to_owned()), (65534, "nobody".to_owned())]));
}

#[test]
fn listening_rows_keep_the_listen_state_only() {
    let text = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/proc-net-tcp.txt")).unwrap();
    assert_eq!(listening_rows(&text), vec![(8080, 45678), (3000, 45700)]);
    assert_eq!(listening_rows(""), vec![]);
}

fn quiet() -> SharedLog {
    Arc::new(|_| {})
}

struct Clock(Mutex<i64>);

struct Bench {
    sampler: Arc<ProcSampler>,
    rx: mpsc::UnboundedReceiver<Outgoing>,
    root: tempfile::TempDir,
    _passwd: tempfile::TempDir,
    clock: Arc<Clock>,
}

impl Bench {
    fn got(&mut self) -> Vec<Value> {
        let mut out = Vec::new();
        while let Ok(text) = self.rx.try_recv() {
            let v: Value = serde_json::from_str(text.text()).unwrap();
            assert!(serde_json::from_value::<DaemonEvent>(v.clone()).is_ok(), "the protocol parses {v}");
            out.push(v);
        }
        out
    }

    fn advance(&self, ms: i64) {
        *self.clock.0.lock().unwrap() += ms;
    }
}

fn sampler(procs: &[FakeProc], ptys: Vec<(u32, String)>, cap: Option<usize>, page_size: u64) -> Bench {
    let root = fake_proc_tree(procs, page_size);
    let (passwd_dir, passwd) = fake_passwd();
    let clock = Arc::new(Clock(Mutex::new(1_000_000)));
    let now = Arc::clone(&clock);
    let source = Arc::new(ProcFsSource::new(root.path().to_path_buf(), passwd, cap.unwrap_or(numbers::PROC_CAP)));
    let sampler = ProcSampler::new(
        source,
        ProcSamplerOptions {
            self_pid: 4242,
            ptys: Arc::new(move || ptys.clone()),
            first: Duration::from_secs(60),
            interval: Duration::from_secs(60),
            now: Arc::new(move || *now.0.lock().unwrap()),
            log: quiet(),
        },
    );
    let (tx, rx) = mpsc::unbounded_channel();
    sampler.lock().subscribers.insert(1, Outbound(tx));
    Bench { sampler, rx, root, _passwd: passwd_dir, clock }
}

struct Fake {
    scans: AtomicUsize,
    broken: AtomicBool,
    child_ms: f64,
}

impl OwnerFacts for Fake {}

impl ProcSource for Fake {
    fn scan(&self, _: &ProcScanInput) -> Result<ProcScan, String> {
        self.scans.fetch_add(1, Ordering::SeqCst);
        if self.broken.load(Ordering::SeqCst) {
            return Err("this machine cannot be read".to_owned());
        }
        Ok(ProcScan { total: 0, procs: vec![], child_ms: self.child_ms, cgroups: None })
    }
    fn inspect(&self, pid: u32, _: &[ProcEntry]) -> Result<ProcInspectReply, OpError> {
        Ok(ProcInspectReply { pid, cwd: None, ports: vec![], threads: None, children: vec![] })
    }
}

#[tokio::test]
async fn a_probe_scans_once_for_the_first_watcher_and_rides_the_running_stream_for_the_next_refusal_and_all() {
    let source = Arc::new(Fake { scans: AtomicUsize::new(0), broken: AtomicBool::new(false), child_ms: 0.0 });
    let scans = || source.scans.load(Ordering::SeqCst);
    let opts = ProcSamplerOptions {
        self_pid: 1,
        ptys: Arc::new(Vec::new),
        first: Duration::from_secs(60),
        interval: Duration::from_secs(60),
        now: Arc::new(now_ms),
        log: quiet(),
    };
    let s = ProcSampler::new(source.clone(), opts);
    s.probe().await.unwrap();
    assert_eq!(scans(), 1);
    let (tx, _rx) = mpsc::unbounded_channel();
    s.subscribe(1, Outbound(tx), None);
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(scans(), 2);
    // Already polling, so the next watcher inherits that tick rather than scanning beside it.
    s.probe().await.unwrap();
    assert_eq!(scans(), 2);
    source.broken.store(true, Ordering::SeqCst);
    s.poll().await;
    assert_eq!(s.probe().await.unwrap_err().message, "this machine cannot be read");
    assert_eq!(scans(), 3);
    s.unsubscribe(1);
    source.broken.store(false, Ordering::SeqCst);
    s.probe().await.unwrap();
    assert_eq!(scans(), 4);
}

#[tokio::test]
async fn a_frame_counts_the_cpu_of_what_its_scan_ran_beside_its_own() {
    let source = Arc::new(Fake { scans: AtomicUsize::new(0), broken: AtomicBool::new(false), child_ms: 40.0 });
    let opts = ProcSamplerOptions {
        self_pid: 1,
        ptys: Arc::new(Vec::new),
        first: Duration::from_secs(60),
        interval: Duration::from_secs(60),
        now: Arc::new(now_ms),
        log: quiet(),
    };
    let s = ProcSampler::new(source, opts);
    let (tx, mut rx) = mpsc::unbounded_channel();
    s.subscribe(1, Outbound(tx), None);
    s.poll().await;
    s.poll().await;
    let mut costs = Vec::new();
    while let Ok(text) = rx.try_recv() {
        let v: Value = serde_json::from_str(text.text()).unwrap();
        costs.push(v["cost"]["cpuMs"].as_f64().unwrap());
    }
    assert!(!costs.is_empty() && costs.iter().all(|ms| (40.0..1_000.0).contains(ms)), "{costs:?}");
    s.unsubscribe(1);
}

#[tokio::test]
async fn a_zero_interval_reads_as_the_shortest_tick_and_never_ends_the_daemon() {
    let source = Arc::new(Fake { scans: AtomicUsize::new(0), broken: AtomicBool::new(false), child_ms: 0.0 });
    let opts = ProcSamplerOptions {
        self_pid: 1,
        ptys: Arc::new(Vec::new),
        first: Duration::ZERO,
        interval: Duration::ZERO,
        now: Arc::new(now_ms),
        log: quiet(),
    };
    let s = ProcSampler::new(source.clone(), opts);
    let (tx, mut rx) = mpsc::unbounded_channel();
    s.subscribe(1, Outbound(tx), None);
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(s.running());
    assert!(source.scans.load(Ordering::SeqCst) >= 2);
    assert!(rx.try_recv().is_ok());
    s.unsubscribe(1);
}

#[tokio::test]
async fn emits_nothing_on_the_first_poll_and_a_snapshot_of_every_process_with_cpu_over_the_interval_on_the_next() {
    let mut b = sampler(
        &[
            FakeProc {
                pid: 1,
                comm: Some("init".into()),
                cmdline: Some(vec!["/sbin/init".into(), "splash".into()]),
                starttime: Some(100),
                rss: Some(300),
                ..FakeProc::default()
            },
            FakeProc {
                pid: 4242,
                ppid: Some(1),
                comm: Some("node".into()),
                cmdline: Some(vec!["node".into(), "/opt/wsp/daemon.js".into()]),
                ticks: Some((100, 50)),
                starttime: Some(500),
                rss: Some(12_000),
                ..FakeProc::default()
            },
            FakeProc {
                pid: 77,
                ppid: Some(4242),
                comm: Some("bash".into()),
                cmdline: Some(vec!["bash".into(), "-l".into()]),
                state: Some("R".into()),
                threads: Some(2),
                ..FakeProc::default()
            },
            FakeProc { pid: 2, comm: Some("kthreadd".into()), cmdline: Some(vec![]), ..FakeProc::default() },
        ],
        vec![(77, "pty_1".to_owned())],
        None,
        4096,
    );
    b.sampler.poll().await;
    assert!(b.got().is_empty());
    // Two seconds later the daemon used 100 more ticks: one core's worth at 100 ticks a second is 50 percent.
    write_proc(
        b.root.path(),
        &FakeProc {
            pid: 4242,
            ppid: Some(1),
            comm: Some("node".into()),
            cmdline: Some(vec!["node".into(), "/opt/wsp/daemon.js".into()]),
            ticks: Some((180, 70)),
            starttime: Some(500),
            rss: Some(12_500),
            ..FakeProc::default()
        },
    );
    b.advance(2_000);
    b.sampler.poll().await;
    let got = b.got();
    assert_eq!(got.len(), 1);
    let snap = &got[0];
    assert_eq!(
        (&snap["type"], &snap["at"], &snap["daemon"], &snap["total"]),
        (&json!("proc.snapshot"), &json!(1_002_000), &json!(4242), &json!(4))
    );
    let procs = snap["procs"].as_array().unwrap();
    assert_eq!(procs.iter().map(|p| p["pid"].as_u64().unwrap()).collect::<Vec<_>>(), vec![1, 2, 77, 4242]);
    let by_pid = |pid: u64| procs.iter().find(|p| p["pid"] == json!(pid)).unwrap().clone();
    assert_eq!(
        by_pid(4242),
        json!({"pid": 4242, "ppid": 1, "user": "tester", "state": "S", "comm": "node", "cmdline": "node /opt/wsp/daemon.js", "cpu": 50.0, "rss": 12_500 * 4096, "startedAt": (BTIME + 5) * 1000, "pgid": 1, "id": "500"})
    );
    let p77 = by_pid(77);
    assert_eq!(
        (&p77["ppid"], &p77["state"], &p77["cmdline"], &p77["cpu"], &p77["pty"]),
        (&json!(4242), &json!("R"), &json!("bash -l"), &json!(0.0), &json!("pty_1"))
    );
    assert_eq!((&by_pid(2)["comm"], &by_pid(2)["cmdline"]), (&json!("kthreadd"), &json!("")));
    assert_eq!(
        (&by_pid(1)["cmdline"], &by_pid(1)["startedAt"], &by_pid(1)["rss"]),
        (&json!("/sbin/init splash"), &json!((BTIME + 1) * 1000), &json!(300 * 4096))
    );
    assert!(by_pid(1).get("pty").is_none());
}

#[tokio::test]
async fn a_list_taken_with_nobody_watching_names_each_rows_owner_off_the_threads_the_host_named() {
    // Settle and delete read their thread's processes through proc.list with no watch running: a process whose
    // parent is init, 400, is the thread's only by its owner, so the list carries it.
    let b = sampler(
        &[
            FakeProc { pid: 1, comm: Some("init".into()), ..FakeProc::default() },
            FakeProc { pid: 300, ppid: Some(1), comm: Some("claude".into()), ..FakeProc::default() },
            FakeProc { pid: 301, ppid: Some(300), comm: Some("bash".into()), ..FakeProc::default() },
            FakeProc { pid: 400, ppid: Some(1), comm: Some("node".into()), ..FakeProc::default() },
        ],
        vec![],
        None,
        4096,
    );
    let scope = ProcScope {
        thread: "t1".into(),
        workspace: Some("ws_1".into()),
        agents: vec![300],
        groups: vec![],
        folder: None,
        warm: false,
        cgroup: None,
        pids: vec![wsp_frames::ProcScopePid { pid: 400, kind: wsp_frames::ProcKind::Slate }],
    };
    b.sampler.set_scopes(vec![scope]);
    assert!(!b.sampler.running());
    let list = b.sampler.list().await.unwrap();
    let owned: Vec<(u32, Option<Option<String>>)> =
        list.procs.iter().map(|p| (p.pid, p.owner.as_ref().map(|o| o.thread.clone()))).collect();
    let t1 = Some(Some("t1".to_owned()));
    assert_eq!(owned, vec![(1, None), (300, t1.clone()), (301, t1.clone()), (400, t1)]);
}

#[tokio::test]
async fn the_threads_the_host_names_stamp_each_rows_owner_and_a_new_list_moves_the_rows_it_changes() {
    let mut b = sampler(
        &[
            FakeProc { pid: 1, comm: Some("init".into()), ..FakeProc::default() },
            FakeProc { pid: 300, ppid: Some(1), comm: Some("claude".into()), ..FakeProc::default() },
            FakeProc { pid: 301, ppid: Some(300), comm: Some("bash".into()), ..FakeProc::default() },
            FakeProc { pid: 400, ppid: Some(1), comm: Some("python3".into()), ..FakeProc::default() },
        ],
        vec![],
        None,
        4096,
    );
    let scope = |thread: &str, agents: Vec<u32>, pids: Vec<wsp_frames::ProcScopePid>| ProcScope {
        thread: thread.into(),
        workspace: Some("ws_1".into()),
        agents,
        groups: vec![],
        folder: None,
        warm: false,
        cgroup: None,
        pids,
    };
    b.sampler.set_scopes(vec![scope("t1", vec![300], vec![])]);
    b.sampler.poll().await;
    b.advance(2_000);
    b.sampler.poll().await;
    let snap = b.got().remove(0);
    let owned = |frame: &Value| -> Vec<(u64, Value, Value)> {
        frame["procs"].as_array().unwrap().iter().map(|p| (p["pid"].as_u64().unwrap(), p["owner"].clone(), p["kind"].clone())).collect()
    };
    let t1 = json!({"by": "root", "thread": "t1", "workspace": "ws_1"});
    assert_eq!(
        owned(&snap),
        vec![
            (1, Value::Null, Value::Null),
            (300, t1.clone(), json!("agent")),
            (301, t1.clone(), Value::Null),
            (400, Value::Null, Value::Null)
        ]
    );
    b.sampler.set_scopes(vec![scope("t1", vec![300], vec![wsp_frames::ProcScopePid { pid: 400, kind: wsp_frames::ProcKind::Slate }])]);
    b.advance(2_000);
    b.sampler.poll().await;
    let moved = b.got().remove(0);
    assert_eq!((&moved["type"], owned(&moved)), (&json!("proc.changes"), vec![(400, t1, json!("slate"))]));
}

#[tokio::test]
async fn caps_cmdline_at_200_bytes_reads_it_again_only_after_an_exec_changed_the_comm_and_takes_the_page_size_from_auxv() {
    let long = "x".repeat(500);
    let mut b = sampler(
        &[FakeProc { pid: 9, comm: Some("sh".into()), cmdline: Some(vec!["sh".into(), "-c".into(), long]), ..FakeProc::default() }],
        vec![],
        None,
        16384,
    );
    b.sampler.poll().await;
    b.advance(2_000);
    b.sampler.poll().await;
    let first = b.got().remove(0);
    assert_eq!(first["procs"][0]["cmdline"].as_str().unwrap().len(), 200);
    assert!(first["procs"][0]["cmdline"].as_str().unwrap().starts_with("sh -c xxx"));
    assert_eq!(first["procs"][0]["rss"], json!(10 * 16384));
    // The same pid, comm and start time: the cached cmdline stands even though the file changed underneath, so
    // the row has not changed.
    std::fs::write(b.root.path().join("9/cmdline"), b"ignored\0").unwrap();
    b.advance(2_000);
    b.sampler.poll().await;
    assert_eq!(b.got().remove(0)["procs"], json!([]));
    // An exec changes the comm; the cmdline is read afresh.
    write_proc(
        b.root.path(),
        &FakeProc { pid: 9, comm: Some("node".into()), cmdline: Some(vec!["node".into(), "app.js".into()]), ..FakeProc::default() },
    );
    b.advance(2_000);
    b.sampler.poll().await;
    let third = b.got().remove(0);
    assert_eq!((&third["procs"][0]["comm"], &third["procs"][0]["cmdline"]), (&json!("node"), &json!("node app.js")));
}

#[tokio::test]
async fn a_snapshot_after_one_changed_process_carries_only_that_change() {
    let mut b = sampler(
        &[
            FakeProc { pid: 1, comm: Some("init".into()), rss: Some(300), ..FakeProc::default() },
            FakeProc { pid: 77, ppid: Some(1), comm: Some("bash".into()), rss: Some(500), ..FakeProc::default() },
            FakeProc { pid: 78, ppid: Some(1), comm: Some("sleep".into()), rss: Some(100), ..FakeProc::default() },
        ],
        vec![],
        None,
        4096,
    );
    b.sampler.poll().await;
    b.advance(5_000);
    b.sampler.poll().await;
    let first = b.got();
    assert_eq!((first.len(), &first[0]["type"]), (1, &json!("proc.snapshot")));
    assert_eq!(first[0]["procs"].as_array().unwrap().len(), 3);

    write_proc(b.root.path(), &FakeProc { pid: 77, ppid: Some(1), comm: Some("bash".into()), rss: Some(900), ..FakeProc::default() });
    b.advance(5_000);
    b.sampler.poll().await;
    let changed = b.got();
    assert_eq!(changed.len(), 1);
    assert_eq!(
        (&changed[0]["type"], &changed[0]["at"], &changed[0]["total"], &changed[0]["gone"]),
        (&json!("proc.changes"), &json!(1_010_000), &json!(3), &json!([]))
    );
    assert_eq!(
        changed[0]["procs"].as_array().unwrap().iter().map(|p| (p["pid"].clone(), p["rss"].clone())).collect::<Vec<_>>(),
        [(json!(77), json!(900 * 4096))]
    );

    std::fs::remove_dir_all(b.root.path().join("78")).unwrap();
    b.advance(5_000);
    b.sampler.poll().await;
    let gone = b.got();
    assert_eq!(
        (&gone[0]["type"], &gone[0]["procs"], &gone[0]["gone"], &gone[0]["total"]),
        (&json!("proc.changes"), &json!([]), &json!([78]), &json!(2))
    );
}

#[tokio::test]
async fn every_frame_says_what_its_tick_cost_the_daemon_in_cpu_and_in_rows_held() {
    let mut b = sampler(
        &[
            FakeProc { pid: 1, comm: Some("init".into()), ..FakeProc::default() },
            FakeProc { pid: 5, comm: Some("node".into()), cmdline: Some(vec!["node".into(), "server.js".into()]), ..FakeProc::default() },
        ],
        vec![],
        None,
        4096,
    );
    b.sampler.poll().await;
    b.advance(5_000);
    b.sampler.poll().await;
    b.advance(5_000);
    b.sampler.poll().await;
    let frames = b.got();
    assert_eq!(frames.iter().map(|f| f["type"].clone()).collect::<Vec<_>>(), [json!("proc.snapshot"), json!("proc.changes")]);
    for frame in &frames {
        let cpu = frame["cost"]["cpuMs"].as_f64().unwrap();
        assert!((0.0..1_000.0).contains(&cpu), "{cpu} ms");
        // Two copies of two rows: each row's own size and its text, the cmdline among it.
        let held = frame["cost"]["held"].as_u64().unwrap();
        assert!(held >= 2 * (2 * size_of::<ProcEntry>() as u64 + "node server.js".len() as u64), "{held}");
    }
}

#[tokio::test]
async fn a_socket_joining_mid_stream_is_sent_the_whole_list_at_once_and_the_same_changes_after() {
    let mut b = sampler(
        &[FakeProc { pid: 1, comm: Some("init".into()), ..FakeProc::default() }, FakeProc { pid: 5, rss: Some(10), ..FakeProc::default() }],
        vec![],
        None,
        4096,
    );
    b.sampler.poll().await;
    b.advance(5_000);
    b.sampler.poll().await;
    b.got();
    let (tx, mut joined) = mpsc::unbounded_channel();
    b.sampler.subscribe(2, Outbound(tx), None);
    let first: Value = serde_json::from_str(joined.try_recv().unwrap().text()).unwrap();
    assert_eq!((&first["type"], &first["at"], first["procs"].as_array().unwrap().len()), (&json!("proc.snapshot"), &json!(1_005_000), 2));
    write_proc(b.root.path(), &FakeProc { pid: 5, rss: Some(20), ..FakeProc::default() });
    b.advance(5_000);
    b.sampler.poll().await;
    let after: Value = serde_json::from_str(joined.try_recv().unwrap().text()).unwrap();
    assert_eq!(b.got(), std::slice::from_ref(&after));
    assert_eq!(
        (&after["type"], &after["procs"][0]["pid"], after["procs"].as_array().unwrap().len()),
        (&json!("proc.changes"), &json!(5), 1)
    );
}

#[tokio::test]
async fn drops_a_process_that_went_away_reads_a_new_one_with_cpu_0_and_never_counts_a_returned_pid_against_the_old_ticks() {
    let mut b = sampler(
        &[
            FakeProc { pid: 5, ticks: Some((100, 0)), starttime: Some(10), ..FakeProc::default() },
            FakeProc { pid: 6, ticks: Some((0, 0)), ..FakeProc::default() },
        ],
        vec![],
        None,
        4096,
    );
    b.sampler.poll().await;
    std::fs::remove_dir_all(b.root.path().join("6")).unwrap();
    write_proc(b.root.path(), &FakeProc { pid: 7, ticks: Some((900, 900)), ..FakeProc::default() });
    write_proc(b.root.path(), &FakeProc { pid: 5, ticks: Some((10, 0)), starttime: Some(900), ..FakeProc::default() });
    b.advance(2_000);
    b.sampler.poll().await;
    let snap = b.got().remove(0);
    let rows: Vec<(u64, f64)> =
        snap["procs"].as_array().unwrap().iter().map(|p| (p["pid"].as_u64().unwrap(), p["cpu"].as_f64().unwrap())).collect();
    assert_eq!(rows, vec![(5, 0.0), (7, 0.0)]);
}

#[tokio::test]
async fn stops_at_the_cap_and_says_how_many_there_were() {
    let procs: Vec<FakeProc> = (1..=12).map(|pid| FakeProc { pid, ..FakeProc::default() }).collect();
    let mut b = sampler(&procs, vec![], Some(5), 4096);
    b.sampler.poll().await;
    b.advance(2_000);
    b.sampler.poll().await;
    let snap = b.got().remove(0);
    assert_eq!(snap["total"], json!(12));
    assert_eq!(snap["procs"].as_array().unwrap().iter().map(|p| p["pid"].as_u64().unwrap()).collect::<Vec<_>>(), vec![1, 2, 3, 4, 5]);
}

#[test]
fn the_cap_reads_the_threads_cgroups_first_counts_no_kernel_thread_and_the_total_still_counts_every_pid() {
    // init, kthreadd and 84 kernel threads under it, then the person's processes up to pid 1,500; three of the newest
    // stand in thread cgroups, one a cgroup below its thread's.
    let mut procs =
        vec![FakeProc { pid: 1, ..FakeProc::default() }, FakeProc { pid: 2, comm: Some("kthreadd".into()), ..FakeProc::default() }];
    procs.extend((3..=86).map(|pid| FakeProc { pid, ppid: Some(2), ..FakeProc::default() }));
    procs.extend((87..=1_500).map(|pid| FakeProc { pid, ppid: Some(1), ..FakeProc::default() }));
    let root = fake_proc_tree(&procs, 4096);
    let (_passwd_dir, passwd) = fake_passwd();
    let cgroups = tempfile::tempdir().unwrap();
    let threads = cgroups.path().join(numbers::THREAD_CGROUPS.trim_start_matches('/'));
    std::fs::create_dir_all(threads.join("t1/run")).unwrap();
    std::fs::create_dir_all(threads.join("t2")).unwrap();
    std::fs::write(threads.join("t1/cgroup.procs"), "1450\n").unwrap();
    std::fs::write(threads.join("t1/run/cgroup.procs"), "1480\n").unwrap();
    std::fs::write(threads.join("t2/cgroup.procs"), "1499\n").unwrap();
    let read = |source: &ProcFsSource| {
        source
            .scan(&ProcScanInput { at: 0, elapsed_ms: 0, pty: HashMap::new(), only: None, daemon: 4242, scopes: Default::default() })
            .unwrap()
    };
    let source =
        ProcFsSource::new(root.path().to_path_buf(), passwd.clone(), numbers::PROC_CAP).with_cgroup_root(cgroups.path().to_path_buf());
    let scan = read(&source);
    let pids: Vec<u32> = scan.procs.iter().map(|p| p.pid).collect();
    assert_eq!(scan.total, 1_500);
    assert!([1_450, 1_480, 1_499].iter().all(|pid| pids.contains(pid)), "the threads' members are listed");
    assert!((2..=86).all(|pid| pids.contains(&pid)), "every kernel thread is listed");
    assert_eq!(scan.procs.iter().filter(|p| !kernel_thread(p)).count(), numbers::PROC_CAP);
    assert!(pids.windows(2).all(|w| w[0] < w[1]), "rows go by pid");
    // Without the thread cgroups, the newest drop first, as before.
    let bare = ProcFsSource::new(root.path().to_path_buf(), passwd, numbers::PROC_CAP).with_cgroup_root(cgroups.path().join("none"));
    let scan = read(&bare);
    assert!(!scan.procs.iter().any(|p| p.pid == 1_450));
}

#[tokio::test]
async fn runs_from_the_first_subscriber_to_the_last_and_reads_nothing_in_between_polls() {
    let b = sampler(&[FakeProc { pid: 1, ..FakeProc::default() }], vec![], None, 4096);
    b.sampler.lock().subscribers.clear();
    let (tx, _rx) = mpsc::unbounded_channel();
    let (tx2, _rx2) = mpsc::unbounded_channel();
    assert!(!b.sampler.running());
    b.sampler.subscribe(1, Outbound(tx), None);
    assert!(b.sampler.running());
    b.sampler.subscribe(2, Outbound(tx2), None);
    b.sampler.unsubscribe(1);
    b.sampler.unsubscribe(1);
    assert!(b.sampler.running());
    b.sampler.unsubscribe(2);
    assert!(!b.sampler.running());
}

#[tokio::test]
async fn inspect_reads_cwd_listening_ports_by_socket_inode_threads_and_children_for_one_pid() {
    let b = sampler(
        &[
            FakeProc { pid: 1, comm: Some("init".into()), ..FakeProc::default() },
            FakeProc {
                pid: 30,
                ppid: Some(1),
                comm: Some("node".into()),
                threads: Some(7),
                cwd: Some("/root/app".into()),
                socket_inodes: Some(vec![111_111, 222_222]),
                ..FakeProc::default()
            },
            FakeProc { pid: 31, ppid: Some(30), comm: Some("sh".into()), ..FakeProc::default() },
            FakeProc { pid: 32, ppid: Some(30), comm: Some("sh".into()), ..FakeProc::default() },
            FakeProc { pid: 40, ppid: Some(1), comm: Some("nginx".into()), socket_inodes: Some(vec![333_333]), ..FakeProc::default() },
        ],
        vec![],
        None,
        4096,
    );
    std::fs::write(
        b.root.path().join("net/tcp"),
        [
            "  sl  local_address rem_address   st tx_queue rx_queue tr tm->when retrnsmt   uid  timeout inode",
            "   0: 00000000:1F90 00000000:0000 0A 00000000:00000000 00:00000000 00000000     0        0 111111 1 0000000000000000 100 0 0 10 0",
            "   1: 0100007F:0BB8 00000000:0000 0A 00000000:00000000 00:00000000 00000000     0        0 333333 1 0000000000000000 100 0 0 10 0",
            "   2: 0100007F:C350 0100007F:1F90 01 00000000:00000000 00:00000000 00000000     0        0 222222 1 0000000000000000 100 0 0 10 0",
            "",
        ]
        .join("\n"),
    )
    .unwrap();
    b.sampler.poll().await;
    b.advance(2_000);
    b.sampler.poll().await;
    assert_eq!(
        b.sampler.inspect(30).await.unwrap(),
        ProcInspectReply { pid: 30, cwd: Some("/root/app".into()), ports: vec![8080], threads: Some(7), children: vec![31, 32] }
    );
    assert_eq!(
        b.sampler.inspect(40).await.unwrap(),
        ProcInspectReply { pid: 40, cwd: None, ports: vec![3000], threads: Some(1), children: vec![] }
    );
    assert_eq!(b.sampler.inspect(999).await.unwrap_err().code, Some(DaemonErrorCode::NotFound));
}

#[tokio::test]
async fn inspect_without_a_running_watch_scans_once_for_the_children() {
    let b = sampler(
        &[FakeProc { pid: 1, ..FakeProc::default() }, FakeProc { pid: 2, ppid: Some(1), ..FakeProc::default() }],
        vec![],
        None,
        4096,
    );
    assert_eq!(b.sampler.inspect(1).await.unwrap().children, vec![2]);
}

#[tokio::test]
async fn a_list_answers_every_process_at_once_and_rides_a_running_sampler_rather_than_scanning_beside_it() {
    let b = sampler(
        &[FakeProc { pid: 1, ..FakeProc::default() }, FakeProc { pid: 2, ppid: Some(1), ..FakeProc::default() }],
        vec![],
        None,
        4096,
    );
    let listed = b.sampler.list().await.unwrap();
    assert_eq!((listed.at, listed.daemon, listed.total), (1_000_000, 4242, 2));
    assert_eq!(listed.procs.iter().map(|p| (p.pid, p.ppid, p.cpu)).collect::<Vec<_>>(), vec![(1, 0, 0.0), (2, 1, 0.0)]);

    let source = Arc::new(Fake { scans: AtomicUsize::new(0), broken: AtomicBool::new(false), child_ms: 0.0 });
    let opts = || ProcSamplerOptions {
        self_pid: 1,
        ptys: Arc::new(Vec::new),
        first: Duration::from_secs(60),
        interval: Duration::from_secs(60),
        now: Arc::new(now_ms),
        log: quiet(),
    };
    let s = ProcSampler::new(source.clone(), opts());
    s.list().await.unwrap();
    assert_eq!(source.scans.load(Ordering::SeqCst), 1);
    let (tx, _rx) = mpsc::unbounded_channel();
    s.subscribe(1, Outbound(tx), None);
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(source.scans.load(Ordering::SeqCst), 2);
    s.list().await.unwrap();
    assert_eq!(source.scans.load(Ordering::SeqCst), 2);
    s.unsubscribe(1);

    // A watch narrowed to cgroups holds only their rows, so a list reads every process itself.
    let source = Arc::new(Fake { scans: AtomicUsize::new(0), broken: AtomicBool::new(false), child_ms: 0.0 });
    let s = ProcSampler::new(source.clone(), opts());
    let (tx, _rx) = mpsc::unbounded_channel();
    s.subscribe(1, Outbound(tx), Some(vec!["/wsp-threads".into()]));
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(source.scans.load(Ordering::SeqCst), 1);
    s.list().await.unwrap();
    assert_eq!(source.scans.load(Ordering::SeqCst), 2);
    s.unsubscribe(1);
}
