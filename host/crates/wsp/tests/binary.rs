// SPDX-License-Identifier: AGPL-3.0-only
//! The `wsp` binary as the comparison harness starts it (xtask/src/host.rs): the environment cleared to seven
//! variables, `up --port 0 --no-relay` on a temp home, the `ws://` line and the token file read back, then the
//! command line and the tool server on the same home. The oracle is the TS host at 65f6b89c7.
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tungstenite::Message;

const BIN: &str = env!("CARGO_BIN_EXE_wsp");
const PAGE: &str = "<!doctype html>\n<html><head><script>window.__WSP__ = null;</script></head><body></body></html>\n";
const VERSION: &str = "0.3.8";
const START_WAIT: Duration = Duration::from_secs(30);

/// One replay's folders, laid out as the harness lays them.
struct Place {
    _tmp: tempfile::TempDir,
    root: PathBuf,
}

impl Place {
    fn new() -> Place {
        let tmp = tempfile::Builder::new().prefix("wsp-bin-").tempdir().expect("a temp folder");
        let root = fs::canonicalize(tmp.path()).expect("a real path");
        for dir in ["home", "state", "web", "run", "tmp"] {
            fs::create_dir_all(root.join(dir)).expect("a folder");
        }
        fs::write(root.join("web/index.html"), PAGE).expect("the page");
        Place { _tmp: tmp, root }
    }

    fn state(&self) -> PathBuf {
        self.root.join("state")
    }

    fn command(&self, args: &[&str]) -> Command {
        let at = |d: &str| self.root.join(d).display().to_string();
        let bin_dir = Path::new(BIN).parent().expect("a folder").display().to_string();
        let mut cmd = Command::new(BIN);
        cmd.args(args)
            .env_clear()
            .env("HOME", at("home"))
            .env("WSP_HOME", at("state"))
            .env("WSP_WEB_DIR", at("web"))
            .env("WSP_UPDATE_CHECK", "0")
            .env("TZ", "UTC")
            .env("TMPDIR", at("tmp"))
            .env("PATH", format!("{bin_dir}:/usr/bin:/bin"))
            .current_dir(self.root.join("run"))
            .process_group(0);
        cmd
    }

    /// `wsp <args>` to its end: the exit code, stdout and stderr.
    fn run(&self, args: &[&str]) -> (i32, String, String) {
        let out = self.command(args).stdin(Stdio::null()).output().expect("the binary starts");
        (out.status.code().unwrap_or(-1), String::from_utf8_lossy(&out.stdout).into_owned(), String::from_utf8_lossy(&out.stderr).into_owned())
    }

    /// `wsp up --port 0 --no-relay`, read as the harness reads it: the port off the first line holding `ws://127.0.0.1:`.
    fn serve(&self) -> Running {
        let mut child = self
            .command(&["up", "--port", "0", "--no-relay"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("the binary starts");
        let (tx, rx) = std::sync::mpsc::channel();
        let out = child.stdout.take().expect("piped");
        std::thread::spawn(move || {
            for line in BufReader::new(out).lines().map_while(Result::ok) {
                if tx.send(line).is_err() {
                    break;
                }
            }
        });
        let err = child.stderr.take().expect("piped");
        let errors = std::thread::spawn(move || {
            let mut said = String::new();
            let _ = BufReader::new(err).read_to_string(&mut said);
            said
        });
        let deadline = Instant::now() + START_WAIT;
        let mut lines = Vec::new();
        let port = loop {
            let left = deadline.saturating_duration_since(Instant::now());
            let line = rx.recv_timeout(left).unwrap_or_else(|_| {
                let _ = child.kill();
                let _ = child.wait();
                panic!("the host never said where it listens; stdout so far: {lines:?}; stderr: {}", errors.join().unwrap_or_default());
            });
            let port = line.split("ws://127.0.0.1:").nth(1).and_then(|r| r.split('/').next()).and_then(|p| p.parse::<u16>().ok());
            lines.push(line);
            if let Some(port) = port {
                break port;
            }
        };
        std::thread::spawn(move || for _ in rx {});
        let token = fs::read_to_string(self.state().join("host-token")).expect("the token file").trim().to_string();
        Running { child, port, token, lines }
    }
}

struct Running {
    child: Child,
    port: u16,
    token: String,
    lines: Vec<String>,
}

impl Running {
    fn socket(&self) -> tungstenite::WebSocket<TcpStream> {
        let tcp = TcpStream::connect(("127.0.0.1", self.port)).expect("the port answers");
        tcp.set_read_timeout(Some(Duration::from_secs(20))).expect("a timeout");
        let (socket, _) = tungstenite::client(format!("ws://127.0.0.1:{}/ws", self.port), tcp).expect("the upgrade");
        socket
    }

    fn get(&self, path: &str) -> (u16, String) {
        let mut tcp = TcpStream::connect(("127.0.0.1", self.port)).expect("the port answers");
        tcp.set_read_timeout(Some(Duration::from_secs(20))).expect("a timeout");
        write!(tcp, "GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nConnection: close\r\n\r\n", self.port).expect("a request");
        let mut said = String::new();
        tcp.read_to_string(&mut said).expect("an answer");
        let status = said.split(' ').nth(1).and_then(|s| s.parse().ok()).expect("a status");
        (status, said.split_once("\r\n\r\n").map(|(_, body)| body.to_string()).unwrap_or_default())
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn say(socket: &mut tungstenite::WebSocket<TcpStream>, frame: Value) {
    socket.send(Message::text(frame.to_string())).expect("a send");
}

fn hear(socket: &mut tungstenite::WebSocket<TcpStream>) -> Value {
    loop {
        match socket.read().expect("a frame") {
            Message::Text(text) => return serde_json::from_str(text.as_str()).expect("json"),
            Message::Close(frame) => panic!("closed: {frame:?}"),
            _ => {}
        }
    }
}

#[test]
fn version_is_the_last_word_of_one_line() {
    let place = Place::new();
    let (code, out, _) = place.run(&["--version"]);
    assert_eq!((code, out.as_str()), (0, format!("wsp {VERSION}\n").as_str()));
}

#[test]
fn version_under_json_names_the_release_and_the_daemon() {
    let place = Place::new();
    let (code, out, _) = place.run(&["--version", "--json"]);
    assert_eq!(code, 0);
    let said: Value = serde_json::from_str(out.trim()).expect("one json line");
    assert_eq!(said["version"], VERSION);
    assert!(said["daemon"].is_number(), "{said}");
    assert_eq!(out.trim().find("\"version\""), Some(1), "version comes first: {out}");
}

#[test]
fn up_says_where_it_listens_writes_its_token_and_holds_the_lock() {
    let place = Place::new();
    let host = place.serve();
    assert!(host.lines.iter().any(|l| l.contains(&format!("ws://127.0.0.1:{}/ws", host.port))), "{:?}", host.lines);
    assert!(host.lines.iter().any(|l| l.starts_with("app         http://127.0.0.1:")), "{:?}", host.lines);
    assert!(host.lines.iter().any(|l| l == &format!("state       {}", place.state().join("state.json").display())), "{:?}", host.lines);
    assert_eq!(host.token.len(), 32, "24 random bytes as base64url");
    let mode = fs::metadata(place.state().join("host-token")).expect("the token file").permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
    let lock: Value = serde_json::from_str(&fs::read_to_string(place.state().join("host.lock")).expect("the lock")).expect("json");
    assert_eq!(lock["pid"], host.child.id());
    assert_eq!(lock["port"], host.port);
    assert_eq!(lock["startedBy"], "up");
}

#[test]
fn the_page_carries_the_boot_object_and_never_the_token() {
    let place = Place::new();
    let host = place.serve();
    let (status, page) = host.get("/");
    assert_eq!(status, 200);
    assert!(page.contains("window.__WSP__ = {\"tokenHash\":\""), "{page}");
    assert!(page.contains("\"wsPath\":\"/ws\",\"paired\":true,\"version\":\"0.3.8\",\"statePath\":\""), "{page}");
    assert!(!page.contains(&host.token));
    let (status, deep) = host.get("/threads/abc");
    assert_eq!((status, deep), (200, page));
    let (status, miss) = host.get("/missing.js");
    assert_eq!((status, miss.as_str()), (404, "{\"error\":\"no route: GET /missing.js\"}"));
}

#[test]
fn the_socket_answers_auth_with_the_release_and_the_road() {
    let place = Place::new();
    let host = place.serve();
    let mut socket = host.socket();
    say(&mut socket, json!({"token": host.token, "id": 1, "op": "auth"}));
    let reply = hear(&mut socket);
    assert_eq!(reply.to_string(), json!({"id": 1, "ok": true, "version": VERSION, "road": "up"}).to_string());
}

#[test]
fn a_wrong_token_is_refused_as_auth() {
    let place = Place::new();
    let host = place.serve();
    let mut socket = host.socket();
    say(&mut socket, json!({"token": "nope", "id": 1, "op": "auth"}));
    let reply = hear(&mut socket);
    assert_eq!(reply["ok"], false);
    assert_eq!(reply["kind"], "auth");
}

#[test]
fn a_second_host_on_one_state_is_refused_by_the_lock() {
    let place = Place::new();
    let host = place.serve();
    let (code, _, err) = place.run(&["up", "--port", "0", "--no-relay"]);
    assert_eq!(code, 1);
    assert!(err.contains(&format!("another wsp host (pid {}) is already serving", host.child.id())), "{err}");
}

#[test]
fn a_stop_signal_closes_the_host_and_frees_the_lock() {
    let place = Place::new();
    let mut host = place.serve();
    // SAFETY: SIGTERM to the child this test started.
    unsafe { libc::kill(host.child.id() as libc::pid_t, libc::SIGTERM) };
    let ended = host.child.wait().expect("the host ends");
    assert_eq!(ended.code(), Some(0));
    assert!(!place.state().join("host.lock").exists());
}

#[test]
fn an_unknown_command_is_a_usage_refusal() {
    let place = Place::new();
    let (code, out, err) = place.run(&["nope"]);
    assert_eq!((code, out.as_str(), err.as_str()), (3, "", "unknown command: nope. Run wsp --help for the list.\n"));
    let (code, out, err) = place.run(&["nope", "--json"]);
    let want = "{\"error\":\"unknown command: nope. Run wsp --help for the list.\",\"class\":\"usage\",\"exit\":3}\n";
    assert_eq!((code, out.as_str(), err.as_str()), (3, "", want));
}

#[test]
fn a_verb_prints_its_page_without_a_host() {
    let place = Place::new();
    let (code, out, err) = place.run(&["threads", "--help"]);
    assert_eq!(code, 0, "{err}");
    assert!(out.starts_with("usage: wsp threads [<project>] [--tree] [--watch]"), "{out}");
}

#[test]
fn the_tool_server_greets_and_lists_over_stdio() {
    let place = Place::new();
    let state = place.state().join("state.json").display().to_string();
    let mut child = place.command(&["mcp", "--state", &state]).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().expect("starts");
    let mut stdin = child.stdin.take().expect("piped");
    writeln!(stdin, "{}", json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2099-01-01", "capabilities": {}, "clientInfo": {"name": "t", "version": "1"}}})).expect("a line");
    writeln!(stdin, "{}", json!({"jsonrpc": "2.0", "method": "notifications/initialized"})).expect("a line");
    writeln!(stdin, "{}", json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"})).expect("a line");
    drop(stdin);
    let mut said = String::new();
    child.stdout.take().expect("piped").read_to_string(&mut said).expect("output");
    assert_eq!(child.wait().expect("ends").code(), Some(0));
    let lines: Vec<Value> = said.lines().map(|l| serde_json::from_str(l).expect("json")).collect();
    let hello = lines.iter().find(|l| l["id"] == 1).expect("initialize answered");
    assert_eq!(hello["result"]["protocolVersion"], "2025-11-25");
    assert_eq!(hello["result"]["serverInfo"], json!({"name": "wsp", "version": VERSION}));
    let tools = lines.iter().find(|l| l["id"] == 2).expect("tools/list answered");
    assert!(tools["result"]["tools"].as_array().is_some_and(|t| t.len() >= 70), "{tools}");
}

/// The first tape of chapter 10: waits on the bodies of packet 8 (`workspaces.list`) and packet 13 (`sessions.list`).
#[test]
#[ignore = "needs create_runtime packets 8 and 13"]
fn the_spec_tape_of_a_verbs_dial_reads_empty_lists() {
    let place = Place::new();
    let host = place.serve();
    let mut socket = host.socket();
    say(&mut socket, json!({"token": host.token, "id": 1, "op": "auth"}));
    assert_eq!(hear(&mut socket)["ok"], true);
    say(&mut socket, json!({"id": 2, "op": "workspaces.list"}));
    assert_eq!(hear(&mut socket).to_string(), json!({"id": 2, "ok": true, "workspaces": []}).to_string());
    say(&mut socket, json!({"id": 3, "op": "sessions.list"}));
    assert_eq!(hear(&mut socket).to_string(), json!({"id": 3, "ok": true, "sessions": []}).to_string());
}

/// `wsp threads --json` against the running host: waits on packet 13's `sessions.list`.
#[test]
#[ignore = "needs create_runtime packet 13"]
fn a_verb_dials_the_running_host_and_prints_its_json() {
    let place = Place::new();
    let _host = place.serve();
    let (code, out, err) = place.run(&["threads", "--json"]);
    assert_eq!((code, out.as_str(), err.as_str()), (0, "{\"threads\":[]}\n", ""));
}
