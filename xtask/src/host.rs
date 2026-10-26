// SPDX-License-Identifier: AGPL-3.0-only
//! The one interface a replay talks to a host through, and the host behind it today: a wsp command started fresh for
//! each tape, on a temp home and state folder. The TS host is `node packages/host/dist/bin.js`; the Rust host is the
//! same command line in one binary, so it plugs in as another program for the same `ProcessHost`.

use crate::json;
use crate::normalize::{iso_ms, minted, Facts, EPOCH_MS};
use crate::seed::Order;
use crate::tape::{Door, Tape};
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine as _;
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
        sent_times(&Value::Array(tape.ins.clone()), &mut facts.sent);
        facts.started = Some(now_ms());
        let (replies, events) = match tape.door {
            Door::Ws => ws(tape, order, &host, self.reply_wait),
            Door::Cli => Ok(cli(tape, &place, &self.program, self.reply_wait)),
            Door::Mcp => mcp(tape, &place, &self.program, self.reply_wait),
        }?;
        // The door for computers the person owns listens on a port of its own, which the host keeps beside its state.
        if let Ok(port) = fs::read_to_string(tmp.path().join("state/door-port")) {
            facts.add(port.trim(), "<door-port>");
        }
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
            // Days begin at the same moment on every computer, so a day's bucket reads the same wherever it is replayed.
            ("TZ".into(), "UTC".into()),
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

fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

/// Every time a tape's frames carry, as milliseconds or as ISO text, which a host echoing it answers the same each run.
fn sent_times(v: &Value, out: &mut std::collections::BTreeSet<i64>) {
    match v {
        Value::Number(n) => out.extend(n.as_i64().filter(|i| EPOCH_MS.contains(&(*i as f64)))),
        Value::String(s) => out.extend(iso_ms(s)),
        Value::Array(items) => items.iter().for_each(|i| sent_times(i, out)),
        Value::Object(obj) => obj.values().for_each(|i| sent_times(i, out)),
        _ => {}
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

/// The values a tape that joins as a place names by mark, which the replay makes fresh: the joining computer's key,
/// nonce and ephemeral for place.join, its signature for place.prove, and a join code a separate signed-in socket
/// mints first.
pub const JOINER_KEY: &str = "<joiner-key>";
pub const JOINER_NONCE: &str = "<joiner-nonce>";
pub const JOINER_EPHEMERAL: &str = "<joiner-ephemeral>";
pub const JOINER_SIGNATURE: &str = "<joiner-signature>";
pub const JOIN_CODE: &str = "<join-code>";

/// What a place.join reply's signature reads as once the replay checked it against the key the reply names.
pub const SIGNATURE_HOLDS: &str = "<the host's key signed this>";
pub const SIGNATURE_FAILS: &str = "<no signature of the host's key>";

/// The joining computer a replay plays, as `wsp join` and the daemon do: a key of its own, the first frame's two
/// random values, and the seal it agrees once the host answers.
struct Joiner {
    key: ring::signature::Ed25519KeyPair,
    public_der: Vec<u8>,
    nonce: String,
    ephemeral: Option<ring::agreement::EphemeralPrivateKey>,
    ephemeral_public: String,
    code: Option<(String, String)>,
    /// The place id, the host's nonce and the host's ephemeral, off its answer to place.join.
    answered: Option<(String, String, String)>,
}

impl Joiner {
    fn new(code: Option<(String, String)>) -> Result<Joiner, String> {
        use ring::signature::KeyPair as _;
        let rng = ring::rand::SystemRandom::new();
        let pkcs8 = ring::signature::Ed25519KeyPair::generate_pkcs8(&rng).map_err(|_| "make the joiner's key")?;
        let key = ring::signature::Ed25519KeyPair::from_pkcs8(pkcs8.as_ref()).map_err(|_| "read the joiner's key")?;
        // An ed25519 SubjectPublicKeyInfo is this prefix and the key's 32 bytes, as node exports one.
        let mut public_der = vec![0x30, 0x2a, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x03, 0x21, 0x00];
        public_der.extend_from_slice(key.public_key().as_ref());
        let mut nonce = [0u8; 32];
        ring::rand::SecureRandom::fill(&rng, &mut nonce).map_err(|_| "draw the joiner's nonce")?;
        let (ephemeral, public) = wsp_seal::fresh_ephemeral().ok_or("make the joiner's ephemeral")?;
        Ok(Joiner {
            key,
            public_der,
            nonce: B64.encode(nonce),
            ephemeral: Some(ephemeral),
            ephemeral_public: B64.encode(public),
            code,
            answered: None,
        })
    }

    fn marks(&self) -> Vec<(&'static str, String)> {
        let mut marks = vec![
            (JOINER_KEY, B64.encode(&self.public_der)),
            (JOINER_NONCE, self.nonce.clone()),
            (JOINER_EPHEMERAL, self.ephemeral_public.clone()),
        ];
        if let Some((code, _)) = &self.code {
            marks.push((JOIN_CODE, code.clone()));
        }
        if let Some((place, host_nonce, host_ephemeral)) = &self.answered {
            let bytes = transcript("place", place, host_nonce, &self.nonce, host_ephemeral, &self.ephemeral_public);
            marks.push((JOINER_SIGNATURE, B64.encode(self.key.sign(&bytes).as_ref())));
        }
        marks
    }

    /// Takes the host's answer to its place.join, and the seal every later frame rides inside.
    fn answered(&mut self, reply: &Value) -> Option<wsp_seal::Seal> {
        let field = |k: &str| reply.get(k).and_then(Value::as_str).map(str::to_string);
        let (place, nonce, ephemeral) = (field("placeId")?, field("nonce")?, field("ephemeral")?);
        let peer: [u8; 32] = B64.decode(&ephemeral).ok()?.try_into().ok()?;
        let secret = wsp_seal::agree(self.ephemeral.take()?, &peer)?;
        self.answered = Some((place.clone(), nonce, ephemeral));
        Some(wsp_seal::Seal::place(&wsp_seal::seal_keys(&secret, &place)))
    }
}

/// The bytes a place link's signature covers, as the protocol's placeLinkTranscript builds them.
pub fn transcript(role: &str, place: &str, challenge: &str, answer: &str, challenger: &str, answerer: &str) -> Vec<u8> {
    format!("wsp place link v2\n{role}\n{place}\n{challenge}\n{answer}\n{challenger}\n{answerer}\n").into_bytes()
}

/// A place.join reply's signature checked against the key the reply names over what the joiner sent and the host
/// answered, and the key against the fingerprint the join code carried: a port that signs the wrong bytes, or
/// answers with a key the code does not name, reads otherwise than one that does not.
pub fn checked_join(sent: &Value, reply: &mut Value, fingerprint: Option<&str>) {
    let text = |v: &Value, k: &str| v.get(k).and_then(Value::as_str).map(str::to_string);
    let (Some(key), Some(signature)) = (text(reply, "hostPublicKey"), text(reply, "signature")) else { return };
    let holds = (|| {
        let bytes = transcript(
            "host",
            &text(reply, "placeId")?,
            &text(sent, "nonce")?,
            &text(reply, "nonce")?,
            &text(sent, "ephemeral")?,
            &text(reply, "ephemeral")?,
        );
        let signature: [u8; 64] = B64.decode(&signature).ok()?.try_into().ok()?;
        Some(wsp_seal::verify(&B64.decode(&key).ok()?, &bytes, &signature))
    })()
    .unwrap_or(false);
    reply["signature"] = Value::String(if holds { SIGNATURE_HOLDS } else { SIGNATURE_FAILS }.into());
    if let Some(named) = fingerprint {
        if B64.decode(&key).map(|der| wsp_seal::fingerprint(&der)).ok().as_deref() != Some(named) {
            reply["hostPublicKey"] = Value::String("<not the key the join code names>".into());
        }
    }
}

/// A join code minted on a socket of its own, signed in, as the app's sheet asks for one: the code a prove spends and
/// the fingerprint of the host's key the line carries.
fn mint(host: &Running, wait: Duration) -> Result<(String, String), String> {
    let stream = TcpStream::connect(("127.0.0.1", host.port)).map_err(|e| format!("dial the host to mint: {e}"))?;
    stream.set_read_timeout(Some(wait)).map_err(|e| e.to_string())?;
    let (mut socket, _) = tungstenite::client::client(format!("ws://127.0.0.1:{}/ws", host.port), stream)
        .map_err(|e| format!("open a socket to mint: {e}"))?;
    let mut ask = |frame: Value| -> Result<Value, String> {
        socket.send(Message::text(json::write(&frame))).map_err(|e| format!("mint: {e}"))?;
        loop {
            match socket.read().map_err(|e| format!("mint: {e}"))? {
                Message::Text(t) => {
                    let v = json::read(t.as_str()).map_err(|e| format!("mint: {e}"))?;
                    if v.get("id") == frame.get("id") {
                        return Ok(v);
                    }
                }
                Message::Close(_) => return Err("the host closed the socket it was to mint on".into()),
                _ => {}
            }
        }
    };
    ask(json!({ "id": 1, "op": "auth", "token": host.token }))?;
    let minted = ask(json!({ "id": 2, "op": "places.mint" }))?;
    let line =
        minted.pointer("/joins/0/line").and_then(Value::as_str).ok_or_else(|| format!("places.mint answered no join line: {minted}"))?;
    let token = line.split(" --code ").nth(1).ok_or_else(|| format!("a join line with no code: {line}"))?;
    let (code, fingerprint) = token.split_once('.').ok_or_else(|| format!("a join code with no fingerprint: {token}"))?;
    let _ = socket.close(None);
    Ok((code.replace('-', "").to_uppercase(), fingerprint.to_string()))
}

fn ws(tape: &Tape, order: &mut Order, host: &Running, wait: Duration) -> Result<(Vec<Value>, Vec<Value>), String> {
    let text_of = |v: &Value| json::write(v);
    let joins = tape.ins.iter().any(|f| f.get("publicKey").and_then(Value::as_str) == Some(JOINER_KEY));
    let mints = tape.ins.iter().any(|f| text_of(f).contains(JOIN_CODE));
    let mut joiner = if joins { Some(Joiner::new(if mints { Some(mint(host, wait)?) } else { None })?) } else { None };
    let mut seal: Option<wsp_seal::Seal> = None;
    let mut sent_join: HashMap<String, Value> = HashMap::new();
    let stream = TcpStream::connect(("127.0.0.1", host.port)).map_err(|e| format!("dial the host: {e}"))?;
    // The handshake reads its answer in one blocking call, which a loaded computer can keep waiting for seconds; the
    // short timeout the reading loop polls with goes on once the socket is open.
    stream.set_read_timeout(Some(wait)).map_err(|e| e.to_string())?;
    let (mut socket, _) =
        tungstenite::client::client(format!("ws://127.0.0.1:{}/ws", host.port), stream).map_err(|e| format!("open the socket: {e}"))?;
    socket.get_ref().set_read_timeout(Some(Duration::from_millis(50))).map_err(|e| e.to_string())?;
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
            let mut sent = swaps.apply(frame);
            if let Some(j) = &joiner {
                let marks = j.marks();
                sent = placed(&sent, &marks.iter().map(|(m, v)| (*m, v.as_str())).collect::<Vec<_>>());
            }
            if let Some(id) = request_id(&sent) {
                if sent.get("op").and_then(Value::as_str) == Some("place.join") {
                    sent_join.insert(id.clone(), sent.clone());
                }
                asked.push(id.clone());
                waiting.push(id);
            }
            let text = match &sent {
                Value::String(raw) => raw.clone(),
                other => json::write(other),
            };
            let message = match seal.as_mut() {
                Some(s) => Message::binary(s.seal(&text)),
                None => Message::text(text),
            };
            if open && socket.send(message).is_err() {
                open = false;
            }
        }
        i += batch;
        let deadline = Instant::now() + wait;
        while open && !waiting.is_empty() && Instant::now() < deadline {
            match read(&mut socket, seal.as_mut()) {
                Heard::Frame(mut frame) => {
                    let id = request_id(&frame).filter(|id| frame.get("op").is_none() && waiting.contains(id));
                    match id {
                        Some(id) => {
                            waiting.remove(waiting.iter().position(|w| *w == id).expect("waiting"));
                            if let Some(own) = written.get_mut(&id).and_then(VecDeque::pop_front) {
                                swaps.learn(&own, &frame);
                            }
                            if let Some(asked) = sent_join.remove(&id) {
                                if frame.get("ok") == Some(&Value::Bool(true)) {
                                    if let Some(j) = joiner.as_mut().filter(|_| asked.get("nonce").and_then(Value::as_str).is_some()) {
                                        if asked.get("ephemeral").and_then(Value::as_str) == Some(j.ephemeral_public.as_str()) {
                                            seal = j.answered(&frame);
                                        }
                                    }
                                }
                                let fingerprint = joiner.as_ref().and_then(|j| j.code.as_ref()).map(|(_, f)| f.clone());
                                checked_join(&asked, &mut frame, fingerprint.as_deref());
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
        match read(&mut socket, seal.as_mut()) {
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

/// One frame off the socket; once a place link is sealed, a binary frame opened under the seal.
fn read(socket: &mut WebSocket<TcpStream>, seal: Option<&mut wsp_seal::Seal>) -> Heard {
    let text = |t: &str| Heard::Frame(json::read(t).unwrap_or_else(|_| Value::String(t.to_string())));
    match socket.read() {
        Ok(Message::Text(t)) => text(t.as_str()),
        Ok(Message::Binary(b)) => match seal.and_then(|s| s.unseal(&b)) {
            Some(opened) => text(&opened),
            None => Heard::Frame(json!({ "binary": b.len() })),
        },
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
        let frame = json::read(&line).unwrap_or(Value::String(line));
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
            if writeln!(pipe, "{}", json::write(frame)).is_err() {
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
