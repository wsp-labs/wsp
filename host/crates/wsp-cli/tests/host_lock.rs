// SPDX-License-Identifier: AGPL-3.0-only
//! The host lock: one state file, one host. Ported from packages/host/test/host-lock.test.ts and
//! host-lock-take.test.ts at 65f6b89c7, with the pure readings checked against values the TS printed
//! (tests/fixtures/hosts_oracle.json). The spec is spec/host_lock.md.
use std::cell::RefCell;
use std::fs;
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::process::{Child, Command, Stdio};
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use serde_json::Value;
use wsp_cli::host_lock::*;
use wsp_wire::types::release::HostShape;
use wsp_wire::types::JsNumber;
use wsp_words::exit::HostError;

fn oracle() -> Value {
    serde_json::from_str(include_str!("fixtures/hosts_oracle.json")).unwrap()
}

fn n(v: f64) -> JsNumber {
    JsNumber(v)
}

fn me() -> u32 {
    std::process::id()
}

/// A fork while another test holds a copied program open for writing hands the child that descriptor, and the exec
/// of the copy then fails "Text file busy"; every spawn and every copy-then-spawn of this file takes the gate.
static FORK_GATE: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn gate() -> std::sync::MutexGuard<'static, ()> {
    FORK_GATE.lock().unwrap_or_else(|e| e.into_inner())
}

/// A pid that was real a moment ago and is not alive now.
fn dead_pid() -> u32 {
    let _gate = gate();
    let mut child = Command::new("true").spawn().unwrap();
    let pid = child.id();
    child.wait().unwrap();
    pid
}

struct Sleeper(Child);

impl Sleeper {
    fn new() -> Sleeper {
        let _gate = gate();
        Sleeper(Command::new("sleep").arg("30").stdin(Stdio::null()).stdout(Stdio::null()).spawn().unwrap())
    }
    fn pid(&self) -> u32 {
        self.0.id()
    }
}

impl Drop for Sleeper {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

struct Dir {
    _tmp: tempfile::TempDir,
    path: String,
}

impl Dir {
    fn new() -> Dir {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().to_str().unwrap().to_string();
        Dir { _tmp: tmp, path }
    }
    fn at(&self, name: &str) -> String {
        format!("{}/{name}", self.path)
    }
    fn names(&self) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(&self.path).unwrap().map(|e| e.unwrap().file_name().into_string().unwrap()).collect();
        names.sort();
        names
    }
}

fn ports(port: f64) -> LockPorts {
    LockPorts { port: n(port), address: None, started_by: None }
}

fn lock_json(path: &str) -> Value {
    serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
}

fn write_lock(path: &str, pid: f64, port: f64, started_by: Option<&str>) {
    let mut lock = serde_json::json!({"pid": pid, "port": port, "startedAt": "2026-09-01T00:00:00.000Z"});
    if let Some(by) = started_by {
        lock["startedBy"] = Value::String(by.into());
    }
    fs::write(path, serde_json::to_string(&lock).unwrap()).unwrap();
}

fn held_line(pid: u32, state: &str, port: u32, by_verb: bool) -> String {
    let fix = if by_verb {
        "wsp down stops it, or point --state at a different file."
    } else {
        "Stop it first, or point --state at a different file."
    };
    format!("another wsp host (pid {pid}) is already serving {state} on port {port}. {fix}")
}

fn set_age(path: &str, age: Duration) {
    fs::OpenOptions::new().write(true).open(path).unwrap().set_modified(SystemTime::now() - age).unwrap();
}

fn umask_022() {
    unsafe {
        libc::umask(0o022);
    }
}

// ---- the paths beside the state file

/// hosts_oracle.json paths: every file a host keeps beside its state, joined as node's path.join joins.
#[test]
fn every_path_beside_the_state_is_joined_as_node_joins_it() {
    for row in oracle()["paths"].as_array().unwrap() {
        let r: Vec<&str> = row.as_array().unwrap().iter().map(|v| v.as_str().unwrap()).collect();
        let state = r[0];
        assert_eq!(
            vec![
                lock_path_for(state),
                host_token_path(state),
                host_run_dir(state),
                host_roots_path(state),
                door_port_path(state),
                host_readings_dir(state),
                host_inbox_dir(state),
                host_log_path(state)
            ],
            r[1..].to_vec(),
            "{state}"
        );
    }
}

/// host-lock.ts:143: the token file trimmed, nothing where none was written.
#[test]
fn the_token_beside_the_state_is_read_trimmed_and_nothing_where_none_was_written() {
    let dir = Dir::new();
    let state = dir.at("state.json");
    assert_eq!(host_token_for(&state), None);
    fs::write(dir.at("host-token"), "  tok-1\n\n").unwrap();
    assert_eq!(host_token_for(&state), Some("tok-1".to_string()));
    fs::write(dir.at("host-token"), "").unwrap();
    assert_eq!(host_token_for(&state), Some(String::new()));
}

/// hosts_oracle.json addressLines: the lines a host prints at start and wsp status prints while it runs.
#[test]
fn the_address_lines_name_the_bound_address_the_token_file_the_state_and_the_public_name() {
    for row in oracle()["addressLines"].as_array().unwrap() {
        let at = BoundAt { port: n(row[0]["port"].as_f64().unwrap()), address: row[0]["address"].as_str().map(String::from) };
        let want: Vec<String> = serde_json::from_value(row[2].clone()).unwrap();
        assert_eq!(address_lines("/s/state.json", &at, row[1].as_str()), want);
    }
    assert_eq!(state_line("/s/state.json"), "state       /s/state.json");
    assert_eq!(public_address_line("h5aab7a97cd80893f.usewsp.com"), "public      https://h5aab7a97cd80893f.usewsp.com");
}

/// hosts_oracle.json dial: loopback for no address and for a wildcard alone; every other spelling as given.
#[test]
fn a_tool_dials_the_bound_address_and_loopback_only_for_the_wildcard() {
    for row in oracle()["dial"].as_array().unwrap() {
        assert_eq!(dial_address(row[0].as_str()), row[1].as_str().unwrap(), "{row}");
    }
}

/// host-lock.ts:35: only verb and service mark a host; up is read off the line.
#[test]
fn the_environment_marks_a_host_started_by_a_verb_or_the_service_and_nothing_else() {
    for row in oracle()["startedBy"].as_array().unwrap() {
        let env: indexmap::IndexMap<String, String> = serde_json::from_value(row[0].clone()).unwrap();
        let want: Option<HostShape> = serde_json::from_value(row[1].clone()).unwrap();
        assert_eq!(started_by_env(&env), want, "{row}");
    }
    assert_eq!(STARTED_BY_ENV, "WSP_STARTED_BY");
    assert_eq!((POLL_MS, SERVICE_WAIT_MS, MARKER_MS), (200, 20_000, 10_000.0));
}

// ---- what a pid says

/// host-lock.test.ts:340.
#[test]
fn a_pid_of_this_login_is_its_own_and_a_pid_that_is_gone_is_neither_own_nor_alive() {
    assert!(own_pid(n(me() as f64)));
    assert!(pid_alive(n(me() as f64)));
    let gone = dead_pid();
    assert!(!own_pid(n(gone as f64)));
    assert!(!pid_alive(n(gone as f64)));
    assert_eq!(signalled(n(gone as f64)), Signalled::Gone);
}

/// host-lock.test.ts:348: process 1 is root's; as root it is this login's own.
#[test]
fn process_one_is_another_logins_unless_this_login_is_root_and_held_either_way() {
    let root = unsafe { libc::getuid() } == 0;
    assert_eq!(own_pid(n(1.0)), root);
    assert!(pid_alive(n(1.0)));
    assert_eq!(signalled(n(1.0)), if root { Signalled::Own } else { Signalled::Another });
}

/// Node's process.kill(pid, 0), probed on node 22: 0 and -1 signal a group and succeed; a fraction or a pid past
/// 2^31 - 1 is refused before any signal, which reads as gone.
#[test]
fn a_pid_node_refuses_reads_as_gone_and_a_group_pid_reads_as_own() {
    assert_eq!(signalled(n(0.0)), Signalled::Own);
    assert_eq!(signalled(n(-1.0)), Signalled::Own);
    assert_eq!(signalled(n(1.5)), Signalled::Gone);
    assert_eq!(signalled(n(2_147_483_648.0)), Signalled::Gone);
    assert_eq!(signalled(n(1_099_511_627_776.0)), Signalled::Gone);
    assert_eq!(signalled(n(2_147_483_647.0)), Signalled::Gone);
}

/// host-lock.ts:84: one trailing " (deleted)" comes off before the path is looked at.
#[test]
fn a_program_is_gone_only_where_nothing_stands_at_its_path() {
    let dir = Dir::new();
    let bin = dir.at("prog");
    fs::write(&bin, "x").unwrap();
    assert!(!program_gone(&bin));
    assert!(!program_gone(&format!("{bin} (deleted)")));
    assert!(program_gone(&format!("{bin} (deleted) (deleted)")));
    assert!(program_gone(&dir.at("nothing")));
    assert!(program_gone(&format!("{} (deleted)", dir.at("nothing"))));
}

// ---- serving, vanished, held or started

/// host-lock.ts:225 and :116-123: a lock is any JSON object with numeric pid and port and a string startedAt.
#[test]
fn a_lock_serves_only_while_it_parses_and_its_pid_lives() {
    let dir = Dir::new();
    let state = dir.at("state.json");
    assert_eq!(serving_host(&state), None);
    fs::write(dir.at("host.lock"), "{ not json").unwrap();
    assert_eq!(serving_host(&state), None);
    fs::write(dir.at("host.lock"), r#"{"pid":"1","port":1,"startedAt":"x"}"#).unwrap();
    assert_eq!(serving_host(&state), None);
    fs::write(dir.at("host.lock"), format!(r#"{{"pid":{},"port":4400}}"#, me())).unwrap();
    assert_eq!(serving_host(&state), None, "no startedAt string is no lock");
    fs::write(dir.at("host.lock"), format!(r#"{{"pid":{},"port":4400,"startedAt":"x"}}"#, me())).unwrap();
    assert_eq!(
        serving_host(&state).map(|l| (l.pid, l.port, l.address, l.started_at)),
        Some((n(me() as f64), n(4400.0), None, "x".to_string()))
    );
    fs::write(
        dir.at("host.lock"),
        format!(
            r#"{{"pid":{},"port":4400.5,"address":"::1","startedBy":"elsewhere","startedAt":"2026-01-01T00:00:00.000Z","extra":1}}"#,
            me()
        ),
    )
    .unwrap();
    let held = serving_host(&state).expect("a live pid's lock serves");
    assert_eq!(held.pid, n(me() as f64));
    assert_eq!(held.port, n(4400.5));
    assert_eq!(held.address.as_deref(), Some("::1"));
    assert_eq!(held.started_by, Some(Value::String("elsewhere".into())));
    assert_eq!(held.started_by_shape(), None);
    write_lock(&dir.at("host.lock"), dead_pid() as f64, 4400.0, Some("service"));
    assert_eq!(serving_host(&state), None);
}

/// host-lock.test.ts:354: a lock whose process runs a program file that is gone holds nothing, and the next host takes it.
#[cfg(target_os = "linux")]
#[test]
fn a_lock_whose_program_file_is_gone_holds_nothing_and_the_next_host_takes_it() {
    let dir = Dir::new();
    let bin = dir.at("sleep");
    let gate = gate();
    fs::copy("/bin/sleep", &bin).unwrap();
    let held = Sleeper(Command::new(&bin).arg("30").spawn().unwrap());
    drop(gate);
    fs::remove_file(&bin).unwrap();
    let state = dir.at("state.json");
    write_lock(&lock_path_for(&state), held.pid() as f64, 4400.0, Some("service"));
    assert!(pid_alive(n(held.pid() as f64)));
    assert_eq!(serving_host(&state), None);
    assert_eq!(vanished_host(&state).map(|l| l.pid), Some(n(held.pid() as f64)));
    let taken = take_lock(&lock_path_for(&state), &state, &ports(0.0)).unwrap();
    assert_eq!(taken.pid, n(me() as f64));
    assert_eq!(lock_json(&lock_path_for(&state))["pid"], serde_json::json!(me()));
    assert_eq!(vanished_host(&state), None);
}

/// host-lock.test.ts:374: a program upgraded in place still holds its lock.
#[cfg(target_os = "linux")]
#[test]
fn a_lock_whose_program_was_upgraded_in_place_still_holds_and_refuses_a_second_host() {
    let dir = Dir::new();
    let bin = dir.at("sleep");
    let gate = gate();
    fs::copy("/bin/sleep", &bin).unwrap();
    let held = Sleeper(Command::new(&bin).arg("30").spawn().unwrap());
    drop(gate);
    fs::remove_file(&bin).unwrap();
    fs::copy("/bin/sleep", &bin).unwrap();
    let state = dir.at("state.json");
    write_lock(&lock_path_for(&state), held.pid() as f64, 4400.0, Some("service"));
    assert_eq!(serving_host(&state).map(|l| l.pid), Some(n(held.pid() as f64)));
    assert_eq!(vanished_host(&state), None);
    let refused = take_lock(&lock_path_for(&state), &state, &ports(0.0)).unwrap_err();
    assert_eq!(refused.message, held_line(held.pid(), &state, 4400, true));
    assert_eq!(lock_json(&lock_path_for(&state))["pid"], serde_json::json!(held.pid()));
}

struct CountingStarter {
    calls: AtomicUsize,
    answer: Option<HostLock>,
    seen: std::sync::Mutex<Vec<String>>,
}

#[async_trait::async_trait]
impl HostStarter for CountingStarter {
    async fn start(&self, state_path: &str, say: &(dyn Fn(&str) + Send + Sync)) -> Result<Option<HostLock>, HostError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.seen.lock().unwrap().push(state_path.to_string());
        say("starting a host");
        Ok(self.answer.clone())
    }
}

/// host-lock.ts:240: the serving host wins and the starter is never called; with none, the starter's answer.
#[tokio::test]
async fn held_or_started_takes_the_serving_host_and_starts_one_only_where_none_serves() {
    let dir = Dir::new();
    let state = dir.at("state.json");
    let started = HostLock {
        pid: n(42.0),
        port: n(4401.0),
        address: None,
        started_by: Some(Value::String("verb".into())),
        started_at: "2026-10-25T00:00:00.000Z".into(),
    };
    let starter = Arc::new(CountingStarter { calls: AtomicUsize::new(0), answer: Some(started.clone()), seen: Default::default() });
    let as_dyn: Arc<dyn HostStarter> = starter.clone();
    let said = std::sync::Mutex::new(Vec::<String>::new());
    let say = |line: &str| said.lock().unwrap().push(line.to_string());

    assert_eq!(held_or_started(&state, None, &say).await.unwrap(), None);
    assert_eq!(held_or_started(&state, Some(&as_dyn), &say).await.unwrap(), Some(started));
    assert_eq!(starter.calls.load(Ordering::SeqCst), 1);
    assert_eq!(*starter.seen.lock().unwrap(), vec![state.clone()]);
    assert_eq!(*said.lock().unwrap(), vec!["starting a host".to_string()]);

    write_lock(&dir.at("host.lock"), me() as f64, 4400.0, None);
    let serving = held_or_started(&state, Some(&as_dyn), &say).await.unwrap().unwrap();
    assert_eq!((serving.pid, serving.port), (n(me() as f64), n(4400.0)));
    assert_eq!(starter.calls.load(Ordering::SeqCst), 1, "the starter is not called while a host serves");
}

/// host-lock.ts:249.
#[test]
fn refuse_if_served_refuses_a_held_lock_and_passes_a_stale_or_missing_one() {
    let dir = Dir::new();
    let (lock, state) = (dir.at("host.lock"), dir.at("state.json"));
    refuse_if_served(&lock, &state).unwrap();
    write_lock(&lock, dead_pid() as f64, 1.0, None);
    refuse_if_served(&lock, &state).unwrap();
    write_lock(&lock, me() as f64, 4400.0, None);
    let refused = refuse_if_served(&lock, &state).unwrap_err();
    assert_eq!(refused.message, held_line(me(), &state, 4400, false));
    assert_eq!(refused.kind, None);
}

// ---- taking the lock

/// host-lock.test.ts:121 and host-lock.ts:261: pid, the ports in their order, then startedAt as an ISO string.
#[test]
fn a_taken_lock_holds_pid_the_ports_in_order_and_the_start_time() {
    let dir = Dir::new();
    let (lock, state) = (dir.at("state/host.lock"), dir.at("state/state.json"));
    let before = SystemTime::now();
    let taken =
        take_lock(&lock, &state, &LockPorts { port: n(4400.0), address: Some("0.0.0.0".into()), started_by: Some(HostShape::Service) })
            .unwrap();
    let text = fs::read_to_string(&lock).unwrap();
    let at = taken.started_at.clone();
    assert_eq!(text, format!(r#"{{"pid":{},"port":4400,"address":"0.0.0.0","startedBy":"service","startedAt":"{at}"}}"#, me()));
    assert!(regex::Regex::new(r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}Z$").unwrap().is_match(&at), "{at}");
    let _ = before;
    assert_eq!(taken.pid, n(me() as f64));
    assert_eq!(dir.names(), vec!["state".to_string()]);
    let mut left: Vec<String> = fs::read_dir(dir.at("state")).unwrap().map(|e| e.unwrap().file_name().into_string().unwrap()).collect();
    left.sort();
    assert_eq!(left, vec!["host.lock".to_string()], "the taker's own file is gone");
}

/// host-lock.test.ts:144: the state folder is the owner's when the lock is taken, and one left wider is repaired.
#[test]
fn the_state_folder_is_the_owners_once_the_lock_is_taken() {
    umask_022();
    let dir = Dir::new();
    fs::create_dir(dir.at("state")).unwrap();
    fs::set_permissions(dir.at("state"), fs::Permissions::from_mode(0o755)).unwrap();
    take_lock(&dir.at("state/host.lock"), &dir.at("state/state.json"), &ports(1.0)).unwrap();
    assert_eq!(fs::metadata(dir.at("state")).unwrap().permissions().mode() & 0o777, 0o700);
}

/// host-lock.test.ts:154: a second host is refused naming the running pid and port, and the loser takes nothing.
#[test]
fn a_second_host_is_refused_naming_the_pid_and_port_and_the_lock_stays_the_first_ones() {
    let dir = Dir::new();
    let (lock, state) = (dir.at("host.lock"), dir.at("state.json"));
    take_lock(&lock, &state, &ports(4400.0)).unwrap();
    let before = fs::read_to_string(&lock).unwrap();
    let refused = take_lock(&lock, &state, &ports(4500.0)).unwrap_err();
    assert_eq!(refused.message, held_line(me(), &state, 4400, false));
    assert_eq!(refused.kind, None);
    assert_eq!(fs::read_to_string(&lock).unwrap(), before);
}

/// host-lock.test.ts:164 and :171: any startedBy at all names wsp down.
#[test]
fn a_lock_with_any_started_by_names_wsp_down() {
    for by in ["verb", "service", "up", "elsewhere"] {
        let dir = Dir::new();
        let (lock, state) = (dir.at("host.lock"), dir.at("state.json"));
        write_lock(&lock, me() as f64, 4400.0, Some(by));
        let refused = take_lock(&lock, &state, &ports(1.0)).unwrap_err();
        assert_eq!(refused.message, held_line(me(), &state, 4400, true), "{by}");
    }
}

/// host-lock.test.ts:180: a lock whose pid is gone gives way.
#[test]
fn a_lock_whose_pid_is_gone_gives_way_and_the_new_lock_names_this_process() {
    let dir = Dir::new();
    let (lock, state) = (dir.at("host.lock"), dir.at("state.json"));
    write_lock(&lock, dead_pid() as f64, 1.0, None);
    let taken = take_lock(&lock, &state, &ports(4400.0)).unwrap();
    assert_eq!(taken.pid, n(me() as f64));
    assert_eq!(lock_json(&lock)["pid"], serde_json::json!(me()));
    assert_eq!(lock_json(&lock)["port"], serde_json::json!(4400));
    assert_eq!(dir.names(), vec!["host.lock".to_string()]);
}

/// host-lock.ts:244-246: a lock this process cannot parse is treated as a crash's.
#[test]
fn a_lock_that_does_not_parse_gives_way() {
    for text in ["{ not json", "", "[]", r#"{"pid":1}"#, "null"] {
        let dir = Dir::new();
        let (lock, state) = (dir.at("host.lock"), dir.at("state.json"));
        fs::write(&lock, text).unwrap();
        take_lock(&lock, &state, &ports(1.0)).unwrap();
        assert_eq!(lock_json(&lock)["pid"], serde_json::json!(me()), "{text:?}");
    }
}

/// host-lock.test.ts:135 and host-lock.ts:350: a host only ever removes its own lock.
#[test]
fn release_takes_away_only_this_processes_lock() {
    let dir = Dir::new();
    let (lock, state) = (dir.at("host.lock"), dir.at("state.json"));
    take_lock(&lock, &state, &ports(4400.0)).unwrap();
    let winner = format!(r#"{{"pid":{},"port":4401,"startedAt":"2026-10-25T00:00:00.000Z","startedBy":"service"}}"#, me() + 1);
    fs::write(&lock, &winner).unwrap();
    release_lock(&lock).unwrap();
    assert_eq!(fs::read_to_string(&lock).unwrap(), winner);
    take_lock(&dir.at("other.lock"), &state, &ports(1.0)).unwrap();
    release_lock(&dir.at("other.lock")).unwrap();
    assert!(!std::path::Path::new(&dir.at("other.lock")).exists());
    release_lock(&dir.at("never.lock")).unwrap();
}

/// host-lock.ts:356: the lock is swapped in whole and nothing is left beside it.
#[test]
fn rewrite_swaps_the_whole_lock_in_and_leaves_nothing_beside_it() {
    let dir = Dir::new();
    let (lock, state) = (dir.at("host.lock"), dir.at("state.json"));
    let mut taken = take_lock(&lock, &state, &ports(0.0)).unwrap();
    taken.port = n(4410.0);
    rewrite_lock(&lock, &taken).unwrap();
    assert_eq!(fs::read_to_string(&lock).unwrap(), format!(r#"{{"pid":{},"port":4410,"startedAt":"{}"}}"#, me(), taken.started_at));
    assert_eq!(dir.names(), vec!["host.lock".to_string()]);
}

// ---- the take-over, with the file calls hooked (host-lock-take.test.ts)

#[derive(Default)]
struct Hooks {
    read: RefCell<Option<Box<dyn FnMut(&str, &str)>>>,
    link: RefCell<Option<Box<dyn FnMut() -> Option<io::Error>>>>,
    changed: RefCell<Option<Box<dyn FnMut()>>>,
}

/// The file system with host-lock-take.test.ts's hooks: `read` after a read, `link` before a link (an error it
/// returns is the link's), `changed` after every write, link, rm and rename whether it failed or not.
struct HookFs {
    hooks: Rc<Hooks>,
}

impl HookFs {
    fn after<T>(&self, r: io::Result<T>) -> io::Result<T> {
        if let Some(f) = self.hooks.changed.borrow_mut().as_mut() {
            f();
        }
        r
    }
}

impl LockFs for HookFs {
    fn exists(&self, path: &str) -> bool {
        RealFs.exists(path)
    }
    fn read(&self, path: &str) -> io::Result<String> {
        let text = RealFs.read(path)?;
        if let Some(f) = self.hooks.read.borrow_mut().as_mut() {
            f(path, &text);
        }
        Ok(text)
    }
    fn write(&self, path: &str, text: &str) -> io::Result<()> {
        let r = RealFs.write(path, text);
        self.after(r)
    }
    fn write_new(&self, path: &str, text: &str) -> io::Result<()> {
        let r = RealFs.write_new(path, text);
        self.after(r)
    }
    fn link(&self, from: &str, to: &str) -> io::Result<()> {
        let refused = self.hooks.link.borrow_mut().as_mut().and_then(|f| f());
        let r = match refused {
            Some(e) => Err(e),
            None => RealFs.link(from, to),
        };
        self.after(r)
    }
    fn remove(&self, path: &str) -> io::Result<()> {
        let r = RealFs.remove(path);
        self.after(r)
    }
    fn rename(&self, from: &str, to: &str) -> io::Result<()> {
        let r = RealFs.rename(from, to);
        self.after(r)
    }
    fn names(&self, dir: &str) -> io::Result<Vec<String>> {
        RealFs.names(dir)
    }
    fn modified_ms(&self, path: &str) -> io::Result<f64> {
        RealFs.modified_ms(path)
    }
    fn now_ms(&self) -> f64 {
        RealFs.now_ms()
    }
}

/// The file system with only the link hook, which a start standing in for another one keeps (`as`, :68-79).
struct LinkOnlyFs {
    hooks: Rc<Hooks>,
}

impl LockFs for LinkOnlyFs {
    fn exists(&self, path: &str) -> bool {
        RealFs.exists(path)
    }
    fn read(&self, path: &str) -> io::Result<String> {
        RealFs.read(path)
    }
    fn write(&self, path: &str, text: &str) -> io::Result<()> {
        RealFs.write(path, text)
    }
    fn write_new(&self, path: &str, text: &str) -> io::Result<()> {
        RealFs.write_new(path, text)
    }
    fn link(&self, from: &str, to: &str) -> io::Result<()> {
        match self.hooks.link.borrow_mut().as_mut().and_then(|f| f()) {
            Some(e) => Err(e),
            None => RealFs.link(from, to),
        }
    }
    fn remove(&self, path: &str) -> io::Result<()> {
        RealFs.remove(path)
    }
    fn rename(&self, from: &str, to: &str) -> io::Result<()> {
        RealFs.rename(from, to)
    }
    fn names(&self, dir: &str) -> io::Result<Vec<String>> {
        RealFs.names(dir)
    }
    fn modified_ms(&self, path: &str) -> io::Result<f64> {
        RealFs.modified_ms(path)
    }
    fn now_ms(&self) -> f64 {
        RealFs.now_ms()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Said {
    Won,
    Refused,
}

fn tried(r: Result<HostLock, HostError>) -> Said {
    if r.is_ok() {
        Said::Won
    } else {
        Said::Refused
    }
}

struct Take {
    dir: Dir,
    lock: String,
    state: String,
    hooks: Rc<Hooks>,
}

impl Take {
    fn new() -> Take {
        let dir = Dir::new();
        let (lock, state) = (dir.at("host.lock"), dir.at("state.json"));
        Take { dir, lock, state, hooks: Rc::new(Hooks::default()) }
    }
    fn stale(&self) {
        fs::write(&self.lock, r#"{"pid":999999,"port":1,"startedAt":"2026-01-01T00:00:00.000Z"}"#).unwrap();
    }
    fn holder(&self) -> Value {
        lock_json(&self.lock)["pid"].clone()
    }
    fn take(&self) -> Result<HostLock, HostError> {
        take_lock_with(&HookFs { hooks: self.hooks.clone() }, me(), &self.lock, &self.state, &ports(1.0))
    }
    fn marker(&self, k: usize, pid: u32, age: Duration) {
        let path = format!("{}.taking.{k}", self.lock);
        fs::write(&path, format!(r#"{{"pid":{pid}}}"#)).unwrap();
        set_age(&path, age);
    }
}

fn take_as(hooks: &Rc<Hooks>, pid: u32, lock: &str, state: &str) -> Said {
    tried(take_lock_with(&LinkOnlyFs { hooks: hooks.clone() }, pid, lock, state, &ports(1.0)))
}

/// host-lock-take.test.ts:90: another start takes over after this one's read number k of the stale lock; exactly one
/// holds the lock, it is the one the file names, and no third start slips in while the path stands empty.
#[test]
fn a_start_that_takes_over_after_any_read_of_the_stale_lock_keeps_it_and_no_third_start_slips_in() {
    for k in 1..=3 {
        let t = Take::new();
        let (a, c) = (Sleeper::new(), Sleeper::new());
        t.stale();
        let stale_text = fs::read_to_string(&t.lock).unwrap();
        let a_said: Rc<RefCell<Option<Said>>> = Rc::default();
        let c_said: Rc<RefCell<Option<Said>>> = Rc::default();
        let reads = Rc::new(RefCell::new(0));
        {
            let (hooks, lock, state) = (t.hooks.clone(), t.lock.clone(), t.state.clone());
            let (a_said, c_said, reads) = (a_said.clone(), c_said.clone(), reads.clone());
            let (a_pid, c_pid) = (a.pid(), c.pid());
            *t.hooks.read.borrow_mut() = Some(Box::new(move |path: &str, text: &str| {
                if path != lock || text != stale_text {
                    return;
                }
                *reads.borrow_mut() += 1;
                if *reads.borrow() != k {
                    return;
                }
                *a_said.borrow_mut() = Some(take_as(&hooks, a_pid, &lock, &state));
                let (hooks2, lock2, state2, a_said2, c_said2) =
                    (hooks.clone(), lock.clone(), state.clone(), a_said.clone(), c_said.clone());
                *hooks.changed.borrow_mut() = Some(Box::new(move || {
                    if *a_said2.borrow() == Some(Said::Won) && c_said2.borrow().is_none() && !std::path::Path::new(&lock2).exists() {
                        *c_said2.borrow_mut() = Some(take_as(&hooks2, c_pid, &lock2, &state2));
                    }
                }));
            }));
        }
        let b_said = tried(t.take());
        let mut holders = Vec::new();
        if b_said == Said::Won {
            holders.push(me());
        }
        if *a_said.borrow() == Some(Said::Won) {
            holders.push(a.pid());
        }
        if *c_said.borrow() == Some(Said::Won) {
            holders.push(c.pid());
        }
        assert_eq!(holders.len(), 1, "read {k}: {holders:?}");
        assert_eq!(t.holder(), serde_json::json!(holders[0]), "read {k}");
        *t.hooks.read.borrow_mut() = None;
        *t.hooks.changed.borrow_mut() = None;
    }
}

/// host-lock-take.test.ts:120: a live holder's marker is never read as a crash's, however old.
#[test]
fn a_start_stalled_past_marker_ms_while_it_holds_the_marker_keeps_it() {
    let t = Take::new();
    let c = Sleeper::new();
    t.stale();
    let stale_text = fs::read_to_string(&t.lock).unwrap();
    let c_said: Rc<RefCell<Option<Said>>> = Rc::default();
    {
        let (hooks, lock, state, c_said) = (t.hooks.clone(), t.lock.clone(), t.state.clone(), c_said.clone());
        let c_pid = c.pid();
        let mut reads = 0;
        *t.hooks.read.borrow_mut() = Some(Box::new(move |path: &str, text: &str| {
            if path != lock || text != stale_text {
                return;
            }
            reads += 1;
            if reads != 3 {
                return;
            }
            set_age(&format!("{lock}.taking.0"), Duration::from_millis(MARKER_MS as u64 + 1_000));
            *c_said.borrow_mut() = Some(take_as(&hooks, c_pid, &lock, &state));
        }));
    }
    let b_said = tried(t.take());
    let mut holders = Vec::new();
    if b_said == Said::Won {
        holders.push(me());
    }
    if *c_said.borrow() == Some(Said::Won) {
        holders.push(c.pid());
    }
    assert_eq!(holders.len(), 1, "{holders:?}");
    assert_eq!(t.holder(), serde_json::json!(holders[0]));
    *t.hooks.read.borrow_mut() = None;
}

/// host-lock-take.test.ts:152.
#[test]
fn a_marker_whose_pid_is_gone_is_a_crashs_and_the_next_start_takes_over_and_clears_every_marker() {
    let t = Take::new();
    t.stale();
    t.marker(0, 999_999, Duration::ZERO);
    assert_eq!(t.take().unwrap().pid, n(me() as f64));
    assert_eq!(t.holder(), serde_json::json!(me()));
    assert_eq!(t.dir.names(), vec!["host.lock".to_string()]);
}

/// host-lock-take.test.ts:160.
#[test]
fn a_marker_held_by_a_live_pid_refuses_this_start_however_old_it_is() {
    let t = Take::new();
    let other = Sleeper::new();
    t.stale();
    t.marker(0, other.pid(), Duration::from_millis(MARKER_MS as u64 + 60_000));
    let refused = t.take().unwrap_err();
    assert_eq!(
        refused.message,
        format!("another wsp host (pid {}) is starting on {} right now; it serves in a moment.", other.pid(), t.state)
    );
    assert_eq!(t.holder(), serde_json::json!(999_999));
}

/// host-lock-take.test.ts:167.
#[test]
fn a_marker_that_names_no_pid_holds_until_it_is_older_than_marker_ms() {
    let t = Take::new();
    t.stale();
    fs::write(format!("{}.taking.0", t.lock), "").unwrap();
    let refused = t.take().unwrap_err();
    assert_eq!(refused.message, format!("another wsp host is starting on {} right now; it serves in a moment.", t.state));
    assert_eq!(t.holder(), serde_json::json!(999_999));
    set_age(&format!("{}.taking.0", t.lock), Duration::from_millis(MARKER_MS as u64 + 1_000));
    assert_eq!(t.take().unwrap().pid, n(me() as f64));
    assert_eq!(t.dir.names(), vec!["host.lock".to_string()]);
}

/// host-lock-take.test.ts:177.
#[test]
fn the_words_a_crash_left_beside_a_marker_go_and_a_live_starts_stay() {
    let t = Take::new();
    let other = Sleeper::new();
    t.stale();
    fs::write(format!("{}.999999.taking", t.lock), r#"{"pid":999999}"#).unwrap();
    fs::write(format!("{}.{}.taking", t.lock, other.pid()), format!(r#"{{"pid":{}}}"#, other.pid())).unwrap();
    assert_eq!(t.take().unwrap().pid, n(me() as f64));
    assert_eq!(t.dir.names(), vec!["host.lock".to_string(), format!("host.lock.{}.taking", other.pid())]);
}

/// host-lock-take.test.ts:185.
#[test]
fn a_marker_held_by_a_live_start_a_moment_old_refuses_and_leaves_the_stale_lock_to_it() {
    let t = Take::new();
    let other = Sleeper::new();
    t.stale();
    t.marker(0, 999_999, Duration::ZERO);
    t.marker(1, other.pid(), Duration::ZERO);
    let refused = t.take().unwrap_err();
    assert_eq!(
        refused.message,
        format!("another wsp host (pid {}) is starting on {} right now; it serves in a moment.", other.pid(), t.state)
    );
    assert_eq!(t.holder(), serde_json::json!(999_999));
    assert_eq!(t.dir.names(), vec!["host.lock".to_string(), "host.lock.taking.0".to_string(), "host.lock.taking.1".to_string()]);
}

/// host-lock-take.test.ts:195: exclusive create where link() says EPERM, ENOTSUP or EXDEV.
#[test]
fn the_lock_is_taken_by_exclusive_create_on_a_file_system_with_no_hard_links() {
    for code in [libc::EPERM, libc::EOPNOTSUPP, libc::EXDEV] {
        let t = Take::new();
        let other = Sleeper::new();
        *t.hooks.link.borrow_mut() = Some(Box::new(move || Some(io::Error::from_raw_os_error(code))));
        assert_eq!(t.take().unwrap().pid, n(me() as f64), "{code}");
        assert_eq!(t.holder(), serde_json::json!(me()));
        t.stale();
        assert_eq!(t.take().unwrap().pid, n(me() as f64), "{code} over a stale lock");
        assert_eq!(t.holder(), serde_json::json!(me()));
        fs::write(&t.lock, format!(r#"{{"pid":{},"port":2,"startedAt":"2026-10-25T00:00:00.000Z"}}"#, other.pid())).unwrap();
        let refused = t.take().unwrap_err();
        assert_eq!(refused.message, held_line(other.pid(), &t.state, 2, false));
        assert!(!std::path::Path::new(&format!("{}.{}", t.lock, me())).exists());
    }
}

/// host-lock-take.test.ts:209: any other link() error escapes, and no lock is left.
#[test]
fn any_other_link_error_escapes_with_its_code_and_leaves_no_lock() {
    let t = Take::new();
    *t.hooks.link.borrow_mut() = Some(Box::new(|| Some(io::Error::from_raw_os_error(libc::EACCES))));
    let failed = t.take().unwrap_err();
    assert!(failed.message.starts_with("EACCES"), "{}", failed.message);
    assert_eq!(failed.kind, None);
    assert!(!std::path::Path::new(&t.lock).exists());
    assert!(!std::path::Path::new(&format!("{}.{}", t.lock, me())).exists());
}

/// host-lock.test.ts:271, cut to threads: six takers at once over a stale lock, thirty rounds; exactly one wins each
/// round, the lock names it, and its token is the one written. Each taker is its own pid through take_lock_with.
#[test]
fn exactly_one_of_six_takers_at_once_wins_every_round_and_the_lock_names_it() {
    let dir = Dir::new();
    let (lock, state, token) = (dir.at("host.lock"), dir.at("state.json"), dir.at("host-token"));
    let takers: Vec<Sleeper> = (0..6).map(|_| Sleeper::new()).collect();
    for round in 0..30 {
        write_lock(&lock, dead_pid() as f64, 1.0, None);
        let _ = fs::remove_file(&token);
        let gate = Arc::new(std::sync::Barrier::new(takers.len()));
        let handles: Vec<_> = takers
            .iter()
            .map(|s| {
                let (pid, gate, lock, state, token) = (s.pid(), gate.clone(), lock.clone(), state.clone(), token.clone());
                std::thread::spawn(move || {
                    gate.wait();
                    match take_lock_with(&RealFs, pid, &lock, &state, &ports(1.0)) {
                        Ok(_) => {
                            fs::write(&token, pid.to_string()).unwrap();
                            (pid, Said::Won, String::new())
                        }
                        Err(e) => (pid, Said::Refused, e.message),
                    }
                })
            })
            .collect();
        let said: Vec<(u32, Said, String)> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        let won: Vec<u32> = said.iter().filter(|s| s.1 == Said::Won).map(|s| s.0).collect();
        assert_eq!(won.len(), 1, "round {round}: {said:?}");
        for refused in said.iter().filter(|s| s.1 == Said::Refused) {
            assert!(refused.2.starts_with("another wsp host"), "round {round}: {refused:?}");
        }
        assert_eq!(lock_json(&lock)["pid"], serde_json::json!(won[0]), "round {round}");
        assert_eq!(fs::read_to_string(&token).unwrap(), won[0].to_string(), "round {round}");
    }
}
