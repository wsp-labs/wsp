// SPDX-License-Identifier: AGPL-3.0-only
//! The one interface a replay talks to a host through, and the host behind it today: a wsp command started fresh for
//! each tape, on a temp home and state folder. The TS host is `node packages/host/dist/bin.js`; the Rust host is the
//! same command line in one binary, so it plugs in as another program for the same `ProcessHost`.

use crate::normalize::{minted, Facts};
use crate::seed::Order;
use crate::tape::{Door, Tape};
use serde_json::{json, Value};
use std::collections::{HashMap, VecDeque};
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError};
use std::sync::OnceLock;
use std::time::{Duration, Instant};
use tungstenite::{Message, WebSocket};

/// What one tape's replay answered: one reply a request, in the order the requests were sent, and every other frame
/// the host sent, with the facts of that run the normalizer takes out.
#[derive(Clone, Debug, Default)]
pub struct Replay {
    pub replies: Vec<Value>,
    pub events: Vec<Value>,
    pub facts: Facts,
}

pub trait Host: Sync {
    fn name(&self) -> String;
    fn replay(&self, tape: &Tape, order: &mut Order) -> Result<Replay, String>;
}

/// Where a reply that never came is written, so a host that hangs on a frame reads as a difference, not a stall.
const NO_REPLY: &str = "no reply";
/// The page a host serves, which no replay reads: the host refuses to start without one.
const PAGE: &str = "<!doctype html>\n<html><head><script>window.__WSP__ = null;</script></head><body></body></html>\n";
/// How long the host is given to say where it listens.
const START_WAIT: Duration = Duration::from_secs(30);
/// How long a quiet socket is listened to after the last reply for the events that follow it.
const SETTLE_QUIET: Duration = Duration::from_millis(250);
const SETTLE_MAX: Duration = Duration::from_secs(2);

pub struct ProcessHost {
    /// The program and the words before the verb: `node .../bin.js`, or a Rust binary alone.
    pub program: Vec<String>,
    /// How long one request is given its reply, and one command line its exit.
    pub reply_wait: Duration,
    /// Values of the computer the replay runs on that an answer may name, as the checkout's folder.
    pub known: Facts,
    version: OnceLock<Option<String>>,
}

impl ProcessHost {
    pub fn new(program: Vec<String>) -> ProcessHost {
        ProcessHost { program, reply_wait: Duration::from_secs(20), known: Facts::default(), version: OnceLock::new() }
    }

    /// The version the host's own `--version` prints, read once: a bump is not a difference in what it answers.
    fn version(&self) -> Option<String> {
        self.version
            .get_or_init(|| {
                let out = Command::new(&self.program[0]).args(&self.program[1..]).arg("--version").env_clear().output().ok()?;
                let said = String::from_utf8_lossy(&out.stdout);
                said.split_whitespace()
                    .last()
                    .map(|v| v.trim_start_matches('v').to_string())
                    .filter(|v| v.chars().next().is_some_and(|c| c.is_ascii_digit()))
            })
            .clone()
    }
}

impl Host for ProcessHost {
    fn name(&self) -> String {
        self.program.join(" ")
    }

    fn replay(&self, tape: &Tape, order: &mut Order) -> Result<Replay, String> {
        let tmp = tempfile::Builder::new().prefix("wsp-parity-").tempdir().map_err(|e| format!("temp folder: {e}"))?;
        let place = Place::make(tmp.path(), &self.program)?;
        let scratch = tmp.path().join("scratch").display().to_string();
        let host = place.serve(&self.program)?;
        let marks = [(RUN_MARK, scratch.as_str()), (TMP_MARK, &tmp.path().display().to_string()), (TOKEN_MARK, host.token.as_str())];
        let tape = &Tape { ins: tape.ins.iter().map(|f| placed(f, &marks)).collect(), ..tape.clone() };
        let mut facts = place.facts();
        for (value, placeholder) in &self.known.literals {
            facts.add(value.clone(), placeholder);
        }
        if let Some(version) = self.version() {
            facts.add(version, "<version>");
        }
        facts.add(host.port.to_string(), "<port>");
        facts.add(host.pid.to_string(), "<pid>");
        facts.add(host.token.clone(), TOKEN_MARK);
        let (replies, events) = match tape.door {
            Door::Ws => ws(tape, order, &host, self.reply_wait),
            Door::Cli => Ok(cli(tape, &place, &self.program, self.reply_wait)),
            Door::Mcp => mcp(tape, &place, &self.program, self.reply_wait),
        }?;
        drop(host);
        sweep(tmp.path());
        Ok(Replay { replies, events, facts })
    }
}

/// One replay's folders and the environment every process of it runs under, and nothing of the caller's.
struct Place {
    root: PathBuf,
    env: Vec<(String, String)>,
}

impl Place {
    fn make(root: &Path, program: &[String]) -> Result<Place, String> {
        for dir in ["home", "state", "web", "run", "tmp", "scratch"] {
            fs::create_dir_all(root.join(dir)).map_err(|e| e.to_string())?;
        }
        fs::write(root.join("web/index.html"), PAGE).map_err(|e| e.to_string())?;
        let bin_dir = Path::new(&program[0]).parent().map(|p| p.display().to_string()).filter(|p| !p.is_empty());
        let path = [bin_dir.as_deref(), Some("/usr/bin:/bin")].into_iter().flatten().collect::<Vec<_>>().join(":");
        let at = |d: &str| root.join(d).display().to_string();
        let env = vec![
            ("HOME".into(), at("home")),
            ("WSP_HOME".into(), at("state")),
            ("WSP_WEB_DIR".into(), at("web")),
            ("WSP_UPDATE_CHECK".into(), "0".into()),
            ("TMPDIR".into(), at("tmp")),
            ("PATH".into(), path),
        ];
        Ok(Place { root: root.to_path_buf(), env })
    }

    fn command(&self, program: &[String], args: &[&str]) -> Command {
        let mut cmd = Command::new(&program[0]);
        cmd.args(&program[1..]).args(args).env_clear().envs(self.env.iter().cloned()).current_dir(self.root.join("run")).process_group(0);
        #[cfg(target_os = "linux")]
        // SAFETY: prctl is async-signal-safe and touches nothing of the parent's; a harness killed mid-run takes its hosts.
        unsafe {
            cmd.pre_exec(|| {
                libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL);
                Ok(())
            });
        }
        cmd
    }

    fn serve(&self, program: &[String]) -> Result<Running, String> {
        let mut child = self
            .command(program, &["up", "--port", "0", "--no-relay"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("{}: {e}", program.join(" ")))?;
        let lines = lines_of(child.stdout.take().expect("piped"));
        let errors = drained(child.stderr.take().expect("piped"));
        let pid = child.id();
        let mut running = Running { child, pid, port: 0, token: String::new() };
        let deadline = Instant::now() + START_WAIT;
        let port = loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match lines.recv_timeout(left) {
                Ok(line) => {
                    if let Some(port) =
                        line.split("ws://127.0.0.1:").nth(1).and_then(|r| r.split('/').next()).and_then(|p| p.parse::<u16>().ok())
                    {
                        break port;
                    }
                }
                Err(_) => {
                    drop(running);
                    let said = errors.recv_timeout(Duration::from_secs(1)).unwrap_or_default();
                    return Err(format!("the host never said where it listens: {}", said.trim()));
                }
            }
        };
        // The rest of its output is read and dropped, so a host that writes a lot never blocks on a full pipe.
        std::thread::spawn(move || for _ in lines {});
        running.port = port;
        running.token = fs::read_to_string(self.root.join("state/host-token")).map_err(|e| format!("host-token: {e}"))?.trim().to_string();
        Ok(running)
    }

    fn facts(&self) -> Facts {
        let mut facts = Facts::default();
        facts.add_folder(&self.root.display().to_string(), "<tmp>");
        if let Ok(real) = fs::canonicalize(&self.root) {
            facts.add_folder(&real.display().to_string(), "<tmp>");
        }
        if let Some(name) = hostname() {
            if let Some((short, _)) = name.split_once('.') {
                facts.add(short.to_string(), "<computer>");
            }
            facts.add(name, "<computer>");
        }
        facts
    }
}

/// A host process and everything it started, all in its own process group, which goes when this does.
struct Running {
    child: Child,
    pid: u32,
    port: u16,
    token: String,
}

impl Drop for Running {
    fn drop(&mut self) {
        kill_group(self.pid);
        let _ = self.child.wait();
    }
}

/// What a recording run's own temp folder reads as on a tape: each replay points it inside its own folder, so a path a
/// test made lands there and nowhere a later run or another computer may hold something.
pub const RUN_MARK: &str = "<run>";
/// The replay's own temp folder, as a spec tape names a path under it in a frame it sends.
pub const TMP_MARK: &str = "<tmp>";
/// The host's token, as a spec tape names it in a frame it sends.
pub const TOKEN_MARK: &str = "<host-token>";

fn placed(v: &Value, marks: &[(&str, &str)]) -> Value {
    match v {
        Value::String(s) => Value::String(marks.iter().fold(s.clone(), |s, (mark, value)| s.replace(mark, value))),
        Value::Array(items) => Value::Array(items.iter().map(|i| placed(i, marks)).collect()),
        Value::Object(obj) => Value::Object(obj.iter().map(|(k, v)| (k.clone(), placed(v, marks))).collect()),
        other => other.clone(),
    }
}

/// Every process still working in a replay's folder or naming it on its command line, killed: a host a command line
/// started for a state file of its own runs in a session of its own, out of the reach of the group kill.
fn sweep(root: &Path) {
    let Ok(entries) = fs::read_dir("/proc") else { return };
    let mark = root.display().to_string();
    let real = fs::canonicalize(root).map(|p| p.display().to_string()).unwrap_or_else(|_| mark.clone());
    for entry in entries.flatten() {
        let Some(pid) = entry.file_name().to_str().and_then(|n| n.parse::<i32>().ok()) else { continue };
        let cwd = fs::read_link(entry.path().join("cwd")).map(|p| p.display().to_string()).unwrap_or_default();
        let cmdline = fs::read(entry.path().join("cmdline")).map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default();
        if [&mark, &real].iter().any(|m| cwd.starts_with(m.as_str()) || cmdline.contains(m.as_str())) && pid != std::process::id() as i32 {
            // SAFETY: kill takes plain integers.
            unsafe {
                libc::kill(pid, libc::SIGKILL);
            }
        }
    }
}

fn kill_group(pid: u32) {
    // SAFETY: kill takes plain integers; a negative pid names the process group the child leads.
    unsafe {
        libc::kill(-(pid as i32), libc::SIGKILL);
    }
}

fn hostname() -> Option<String> {
    let out = Command::new("hostname").output().ok()?;
    let name = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (!name.is_empty()).then_some(name)
}

fn lines_of(pipe: impl Read + Send + 'static) -> Receiver<String> {
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        for line in BufReader::new(pipe).lines() {
            let Ok(line) = line else { break };
            if tx.send(line).is_err() {
                break;
            }
        }
    });
    rx
}

fn drained(mut pipe: impl Read + Send + 'static) -> Receiver<String> {
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        let mut text = String::new();
        let _ = pipe.read_to_string(&mut text);
        let _ = tx.send(text);
    });
    rx
}

/// A request's id as one string, so `1` and `"1"` stay apart; none for a frame that asks for no reply.
fn request_id(frame: &Value) -> Option<String> {
    frame.get("id").filter(|id| !id.is_null()).map(Value::to_string)
}

/// The values a tape's own answers name that this host answered differently, as the ids it minted: a later frame
/// that names one of them is sent with this host's instead.
#[derive(Default)]
struct Swaps {
    map: HashMap<String, String>,
}

impl Swaps {
    fn learn(&mut self, written: &Value, live: &Value) {
        match (written, live) {
            (Value::String(a), Value::String(b)) if a != b && (minted(a) || a.starts_with('<')) => {
                self.map.insert(a.clone(), b.clone());
            }
            (Value::Object(a), Value::Object(b)) => {
                a.iter().filter_map(|(k, v)| b.get(k).map(|w| (v, w))).for_each(|(v, w)| self.learn(v, w))
            }
            (Value::Array(a), Value::Array(b)) => a.iter().zip(b).for_each(|(v, w)| self.learn(v, w)),
            _ => {}
        }
    }

    fn apply(&self, v: &Value) -> Value {
        match v {
            Value::String(s) => Value::String(self.map.get(s).cloned().unwrap_or_else(|| s.clone())),
            Value::Array(items) => Value::Array(items.iter().map(|i| self.apply(i)).collect()),
            Value::Object(obj) => Value::Object(obj.iter().map(|(k, v)| (k.clone(), self.apply(v))).collect()),
            other => other.clone(),
        }
    }
}

fn ws(tape: &Tape, order: &mut Order, host: &Running, wait: Duration) -> Result<(Vec<Value>, Vec<Value>), String> {
    let stream = TcpStream::connect(("127.0.0.1", host.port)).map_err(|e| format!("dial the host: {e}"))?;
    stream.set_read_timeout(Some(Duration::from_millis(50))).map_err(|e| e.to_string())?;
    let (mut socket, _) =
        tungstenite::client::client(format!("ws://127.0.0.1:{}/ws", host.port), stream).map_err(|e| format!("open the socket: {e}"))?;
    let mut written: HashMap<String, VecDeque<Value>> = HashMap::new();
    for out in &tape.outs {
        if let Some(id) = request_id(out).filter(|_| out.get("op").is_none()) {
            written.entry(id).or_default().push_back(out.clone());
        }
    }
    let mut swaps = Swaps::default();
    let mut asked: Vec<String> = Vec::new();
    let mut got: HashMap<String, VecDeque<Value>> = HashMap::new();
    let mut events = Vec::new();
    let mut open = true;
    let mut i = 0;
    while i < tape.ins.len() {
        let batch = order.window().min(tape.ins.len() - i);
        let mut waiting: Vec<String> = Vec::new();
        for frame in &tape.ins[i..i + batch] {
            let sent = swaps.apply(frame);
            if let Some(id) = request_id(&sent) {
                asked.push(id.clone());
                waiting.push(id);
            }
            let text = match &sent {
                Value::String(raw) => raw.clone(),
                other => other.to_string(),
            };
            if open && socket.send(Message::text(text)).is_err() {
                open = false;
            }
        }
        i += batch;
        let deadline = Instant::now() + wait;
        while open && !waiting.is_empty() && Instant::now() < deadline {
            match read(&mut socket) {
                Heard::Frame(frame) => {
                    let id = request_id(&frame).filter(|id| frame.get("op").is_none() && waiting.contains(id));
                    match id {
                        Some(id) => {
                            waiting.remove(waiting.iter().position(|w| *w == id).expect("waiting"));
                            if let Some(own) = written.get_mut(&id).and_then(VecDeque::pop_front) {
                                swaps.learn(&own, &frame);
                            }
                            got.entry(id).or_default().push_back(frame);
                        }
                        None => events.push(frame),
                    }
                }
                Heard::Quiet => {}
                Heard::Shut(close) => {
                    events.push(close);
                    open = false;
                }
                Heard::Closed => open = false,
            }
        }
    }
    let settle_end = Instant::now() + SETTLE_MAX;
    let mut quiet_since = Instant::now();
    while open && Instant::now() < settle_end && quiet_since.elapsed() < SETTLE_QUIET {
        match read(&mut socket) {
            Heard::Frame(frame) => {
                quiet_since = Instant::now();
                events.push(frame);
            }
            Heard::Quiet => {}
            Heard::Shut(close) => {
                events.push(close);
                open = false;
            }
            Heard::Closed => open = false,
        }
    }
    let _ = socket.close(None);
    let replies = asked
        .iter()
        .map(|id| got.get_mut(id).and_then(VecDeque::pop_front).unwrap_or_else(|| json!({ "parity": NO_REPLY, "id": id })))
        .collect();
    Ok((replies, events))
}

enum Heard {
    Frame(Value),
    /// The host closed the socket, with its code and reason, as a spec tape writes a close line.
    Shut(Value),
    Quiet,
    Closed,
}

fn read(socket: &mut WebSocket<TcpStream>) -> Heard {
    match socket.read() {
        Ok(Message::Text(t)) => Heard::Frame(serde_json::from_str(t.as_str()).unwrap_or_else(|_| Value::String(t.as_str().to_string()))),
        Ok(Message::Binary(b)) => Heard::Frame(json!({ "binary": b.len() })),
        Ok(Message::Close(c)) => {
            let (code, reason) = c.map(|f| (u16::from(f.code), f.reason.to_string())).unwrap_or((1005, String::new()));
            Heard::Shut(json!({ "close": { "code": code, "reason": reason } }))
        }
        Ok(_) => Heard::Quiet,
        Err(tungstenite::Error::Io(e)) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => Heard::Quiet,
        Err(_) => Heard::Closed,
    }
}

fn cli(tape: &Tape, place: &Place, program: &[String], wait: Duration) -> (Vec<Value>, Vec<Value>) {
    let replies = tape
        .ins
        .iter()
        .map(|frame| {
            let argv: Vec<String> = frame
                .get("argv")
                .and_then(Value::as_array)
                .map(|a| a.iter().map(|w| w.as_str().map(str::to_string).unwrap_or_else(|| w.to_string())).collect())
                .unwrap_or_default();
            let words: Vec<&str> = argv.iter().map(String::as_str).collect();
            match place.command(program, &words).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn() {
                Ok(child) => finished(child, wait),
                Err(e) => json!({ "parity": format!("start: {e}") }),
            }
        })
        .collect();
    (replies, Vec::new())
}

fn finished(mut child: Child, wait: Duration) -> Value {
    let pid = child.id();
    let out = drained(child.stdout.take().expect("piped"));
    let err = drained(child.stderr.take().expect("piped"));
    let deadline = Instant::now() + wait;
    let code = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status.code(),
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(20)),
            _ => break None,
        }
    };
    kill_group(pid);
    let _ = child.wait();
    let text = |rx: Receiver<String>| rx.recv_timeout(Duration::from_secs(2)).unwrap_or_default();
    let mut answer = json!({ "exit": code, "stdout": text(out), "stderr": text(err) });
    if code.is_none() {
        answer["parity"] = Value::String("still running when its time ran out".into());
    }
    answer
}

fn mcp(tape: &Tape, place: &Place, program: &[String], wait: Duration) -> Result<(Vec<Value>, Vec<Value>), String> {
    let mut child = place
        .command(program, &["mcp"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("{}: {e}", program.join(" ")))?;
    let pid = child.id();
    let lines = lines_of(child.stdout.take().expect("piped"));
    let mut stdin: Option<ChildStdin> = child.stdin.take();
    let mut asked = Vec::new();
    let mut got: HashMap<String, VecDeque<Value>> = HashMap::new();
    let mut events = Vec::new();
    let mut take = |line: String, waiting: &mut Vec<String>, events: &mut Vec<Value>| {
        let frame: Value = serde_json::from_str(&line).unwrap_or(Value::String(line));
        match request_id(&frame).filter(|id| frame.get("method").is_none() && waiting.contains(id)) {
            Some(id) => {
                waiting.remove(waiting.iter().position(|w| *w == id).expect("waiting"));
                got.entry(id).or_default().push_back(frame);
            }
            None => events.push(frame),
        }
    };
    for frame in &tape.ins {
        let mut waiting = Vec::new();
        if let Some(id) = request_id(frame).filter(|_| frame.get("method").is_some()) {
            asked.push(id.clone());
            waiting.push(id);
        }
        if let Some(pipe) = stdin.as_mut() {
            if writeln!(pipe, "{frame}").is_err() {
                stdin = None;
            }
        }
        let deadline = Instant::now() + wait;
        while !waiting.is_empty() {
            match lines.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
                Ok(line) => take(line, &mut waiting, &mut events),
                Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => break,
            }
        }
    }
    drop(stdin);
    let deadline = Instant::now() + SETTLE_MAX;
    while let Ok(line) = lines.recv_timeout(deadline.saturating_duration_since(Instant::now()).min(SETTLE_QUIET)) {
        take(line, &mut Vec::new(), &mut events);
    }
    kill_group(pid);
    let _ = child.wait();
    let replies = asked
        .iter()
        .map(|id| got.get_mut(id).and_then(VecDeque::pop_front).unwrap_or_else(|| json!({ "parity": NO_REPLY, "id": id })))
        .collect();
    Ok((replies, events))
}
