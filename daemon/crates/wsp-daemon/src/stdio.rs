// SPDX-License-Identifier: AGPL-3.0-only
//! Commands held open on pipes for the socket that opened them: an MCP server a slate calls, which talks JSON lines
//! on stdin and stdout for as long as it stands. A pty would cut a line at the kernel's 4095 bytes, turn \n into
//! \r\n and fold stderr into stdout; exec answers once at the exit. Bytes go each way as base64 frames, stdout and
//! stderr apart, and the command's whole process group is killed at stdio.close or when its socket closes.

use std::collections::HashMap;
use std::ffi::OsString;
use std::path::Path;
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use base64::Engine;
use nix::sys::signal::{killpg, Signal};
use nix::unistd::Pid;
use serde_json::Value;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;
use tokio::sync::{mpsc, oneshot};
use wsp_frames::{DaemonErrorCode, DaemonEvent, DaemonOp, Empty, RequestId};

use crate::ops::{answer, not_built, ok, refuse, Conn};
use crate::paths::OpError;
use crate::Ctx;

/// The three ops, typed and refused as every other frame the daemon serves. A command runs in the daemon's root with
/// the daemon's environment, as exec runs one.
pub(crate) fn serve(conn: &Arc<Conn>, ctx: &Ctx, id: Option<RequestId>, name: &str, frame: &Value) -> String {
    // Read off the raw frame, since the typed one has no such field and would drop it: a command a machine's own
    // config chose never runs here, outside that machine, whatever the host sent.
    if frame.get("machineId").is_some() {
        return refuse(id, DaemonErrorCode::Forbidden, IN_A_MACHINE);
    }
    match serde_json::from_value::<DaemonOp>(frame.clone()) {
        Ok(DaemonOp::StdioOpen { stdio_id, cmd }) => {
            let env: Vec<_> = std::env::vars_os().collect();
            answer(id, Stdios::open(conn, stdio_id, Path::new(&ctx.root), &env, &cmd).map(|()| Empty {}))
        }
        Ok(DaemonOp::StdioWrite { stdio_id, data }) => {
            answer(id, conn.stdios.write(&stdio_id, crate::tunnel::lenient_base64(&data)).map(|()| Empty {}))
        }
        Ok(DaemonOp::StdioClose { stdio_id }) => {
            conn.stdios.close(&stdio_id);
            ok(id)
        }
        Ok(_) => refuse(id, DaemonErrorCode::Unsupported, not_built(name)),
        Err(e) => refuse(id, DaemonErrorCode::BadRequest, e.to_string()),
    }
}

/// The most commands one socket holds open: a host keeps one per thread and server.
const STDIO_CAP: usize = 64;

/// Why a frame naming a machine is refused.
const IN_A_MACHINE: &str = "a held command runs on this computer alone, never for a machine on it";

type Writes = mpsc::UnboundedSender<Vec<u8>>;

/// One command's writes and its end: dropping the entry drops `_end`, which is what tells the pump to kill it, so a
/// writer blocked on a server that stopped reading holds nothing up.
struct Held {
    generation: u64,
    writes: Writes,
    _end: oneshot::Sender<()>,
}

/// The commands one socket holds open; they die with it.
#[derive(Default)]
pub(crate) struct Stdios {
    open: Mutex<HashMap<String, Held>>,
    generation: AtomicU64,
}

fn lock(map: &Mutex<HashMap<String, Held>>) -> std::sync::MutexGuard<'_, HashMap<String, Held>> {
    map.lock().unwrap_or_else(|e| e.into_inner())
}

fn room_for(map: &HashMap<String, Held>, stdio_id: &str) -> Result<(), OpError> {
    if map.contains_key(stdio_id) {
        return Err(OpError::coded(DaemonErrorCode::BadRequest, format!("stdio {stdio_id} is already open")));
    }
    if map.len() >= STDIO_CAP {
        return Err(OpError::coded(DaemonErrorCode::BadRequest, format!("too many commands open ({STDIO_CAP})")));
    }
    Ok(())
}

impl Stdios {
    /// Starts the command and its pump; refused by name when the id is taken or the socket holds the cap.
    pub(crate) fn open(conn: &Arc<Conn>, stdio_id: String, root: &Path, env: &[(OsString, OsString)], cmd: &str) -> Result<(), OpError> {
        let mut map = lock(&conn.stdios.open);
        room_for(&map, &stdio_id)?;
        if conn.is_closed() {
            return Err(OpError::plain("client went away before the command started"));
        }
        let mut command = Command::new("bash");
        command.arg("-c").arg(cmd).current_dir(root).env_clear().envs(env.iter().map(|(k, v)| (k, v)));
        command.process_group(0).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
        let child = command.spawn().map_err(|e| OpError::plain(format!("bash could not be started: {e}")))?;
        let (writes, rx) = mpsc::unbounded_channel();
        let (end_tx, end) = oneshot::channel();
        let generation = conn.stdios.generation.fetch_add(1, Ordering::Relaxed);
        map.insert(stdio_id.clone(), Held { generation, writes, _end: end_tx });
        tokio::spawn(pump(Arc::clone(conn), stdio_id, generation, child, rx, end));
        Ok(())
    }

    pub(crate) fn write(&self, stdio_id: &str, bytes: Vec<u8>) -> Result<(), OpError> {
        let map = lock(&self.open);
        let Some(Held { writes, .. }) = map.get(stdio_id) else {
            return Err(OpError::coded(DaemonErrorCode::NotFound, format!("no such stdio: {stdio_id}")));
        };
        // One sender per command keeps the writes in the order the frames came.
        let _ = writes.send(bytes);
        Ok(())
    }

    /// Ends the command; the pump kills its group and says stdio.exit.
    pub(crate) fn close(&self, stdio_id: &str) {
        lock(&self.open).remove(stdio_id);
    }

    pub(crate) fn close_all(&self) {
        lock(&self.open).clear();
    }
}

/// stdout and stderr to the socket as they come, stdin fed by a task of its own so a server that stops reading never
/// holds up its output; until both outputs end or the entry goes. Then the group is killed, before its stdin is let
/// go, so a close always reads as a kill; the entry goes if it is still this pump's, and stdio.exit is said.
async fn pump(
    conn: Arc<Conn>,
    stdio_id: String,
    generation: u64,
    mut child: tokio::process::Child,
    mut writes: mpsc::UnboundedReceiver<Vec<u8>>,
    mut end: oneshot::Receiver<()>,
) {
    let pid = child.id();
    let (mut stdout, mut stderr) = (child.stdout.take(), child.stderr.take());
    let mut stdin = child.stdin.take();
    let feeder = tokio::spawn(async move {
        while let Some(bytes) = writes.recv().await {
            if let Some(pipe) = stdin.as_mut() {
                if pipe.write_all(&bytes).await.is_err() {
                    stdin = None;
                }
            }
        }
        // Holds stdin open until the pump has killed the group and aborts this task.
        std::future::pending::<()>().await;
    });
    let (mut out_buf, mut err_buf) = (vec![0u8; 16 * 1024], vec![0u8; 16 * 1024]);
    let event = |err: bool, bytes: &[u8]| {
        let data = base64::engine::general_purpose::STANDARD.encode(bytes);
        let stdio_id = stdio_id.clone();
        conn.out.send_event(&if err { DaemonEvent::StdioErr { stdio_id, data } } else { DaemonEvent::StdioOut { stdio_id, data } })
    };
    while stdout.is_some() || stderr.is_some() {
        tokio::select! {
            read = async { stdout.as_mut().unwrap().read(&mut out_buf).await }, if stdout.is_some() => match read {
                Ok(0) | Err(_) => stdout = None,
                Ok(n) => if !event(false, &out_buf[..n]) { break },
            },
            read = async { stderr.as_mut().unwrap().read(&mut err_buf).await }, if stderr.is_some() => match read {
                Ok(0) | Err(_) => stderr = None,
                Ok(n) => if !event(true, &err_buf[..n]) { break },
            },
            _ = &mut end => break,
        }
    }
    // The group, not the pid: a server's own children go with it. A group already gone is a command that exited.
    if pid.and_then(|p| i32::try_from(p).ok()).is_none_or(|g| killpg(Pid::from_raw(g), Signal::SIGKILL).is_err()) {
        let _ = child.start_kill();
    }
    feeder.abort();
    let code = child.wait().await.ok().and_then(|s| s.code());
    {
        let mut map = lock(&conn.stdios.open);
        if map.get(&stdio_id).is_some_and(|held| held.generation == generation) {
            map.remove(&stdio_id);
        }
    }
    conn.out.send_event(&DaemonEvent::StdioExit { stdio_id, code });
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use serde_json::{json, Value};

    use crate::ops::tests::{bench, conn, reply};

    fn b64(bytes: &[u8]) -> String {
        base64::Engine::encode(&base64::engine::general_purpose::STANDARD, bytes)
    }

    fn unb64(text: &Value) -> Vec<u8> {
        base64::Engine::decode(&base64::engine::general_purpose::STANDARD, text.as_str().unwrap()).unwrap()
    }

    /// An MCP server's frames are JSON lines of any length: a line past the 4095 bytes a pty's line discipline holds,
    /// every byte value besides, goes in and comes back as it went, stderr apart from it, and the exit is said.
    #[tokio::test]
    async fn carries_a_commands_stdin_and_stdout_unchanged_with_stderr_apart_and_says_its_exit() {
        let b = bench();
        let (c, mut rx) = conn(None);
        let opened =
            reply(&b, &c, json!({"id": 1, "op": "stdio.open", "stdioId": "s1", "cmd": "echo started >&2; head -c 6200; exit 3"})).await;
        assert_eq!(opened, json!({"id": 1, "ok": true}));
        let line = format!("{}\n", "x".repeat(5000));
        let mut sent: Vec<u8> = line.into_bytes();
        sent.extend((0..=255u8).cycle().take(1199));
        assert_eq!(sent.len(), 6200);
        for part in sent.chunks(1000) {
            assert_eq!(
                reply(&b, &c, json!({"id": 2, "op": "stdio.write", "stdioId": "s1", "data": b64(part)})).await,
                json!({"id": 2, "ok": true})
            );
        }
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let exit = loop {
            let said = tokio::time::timeout(Duration::from_secs(5), rx.recv()).await.unwrap().unwrap();
            let v: Value = serde_json::from_str(said.text()).unwrap();
            match v["type"].as_str() {
                Some("stdio.out") => out.extend(unb64(&v["data"])),
                Some("stdio.err") => err.extend(unb64(&v["data"])),
                Some("stdio.exit") => break v,
                _ => {}
            }
        };
        assert_eq!(out, sent);
        assert_eq!(err, b"started\n");
        assert_eq!(exit, json!({"type": "stdio.exit", "stdioId": "s1", "code": 3}));
        let gone = reply(&b, &c, json!({"id": 3, "op": "stdio.write", "stdioId": "s1", "data": "eA=="})).await;
        assert_eq!(gone["code"], "not-found");
    }

    /// A server left running is the socket's: its close, or stdio.close, kills the server and what it started.
    #[tokio::test]
    async fn a_close_and_the_sockets_close_kill_the_commands_whole_group() {
        let b = bench();
        let dir = tempfile::tempdir().unwrap();
        for by_socket in [false, true] {
            let (c, mut rx) = conn(None);
            let pid_file = dir.path().join(format!("child-{by_socket}.pid"));
            let cmd = format!("sleep 60 & echo $! > {}; cat", pid_file.display());
            reply(&b, &c, json!({"id": 1, "op": "stdio.open", "stdioId": "s1", "cmd": cmd})).await;
            let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
            while !pid_file.exists() || std::fs::read_to_string(&pid_file).unwrap().trim().is_empty() {
                assert!(tokio::time::Instant::now() < deadline, "the command never started");
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
            let pid: i32 = std::fs::read_to_string(&pid_file).unwrap().trim().parse().unwrap();
            if by_socket {
                c.close();
            } else {
                reply(&b, &c, json!({"id": 2, "op": "stdio.close", "stdioId": "s1"})).await;
            }
            let exit = loop {
                let said = tokio::time::timeout(Duration::from_secs(5), rx.recv()).await.unwrap().unwrap();
                let v: Value = serde_json::from_str(said.text()).unwrap();
                if v["type"] == "stdio.exit" {
                    break v;
                }
            };
            assert_eq!(exit, json!({"type": "stdio.exit", "stdioId": "s1"}), "a signal ends it, so no code");
            tokio::time::sleep(Duration::from_millis(100)).await;
            assert!(nix::sys::signal::kill(nix::unistd::Pid::from_raw(pid), None).is_err(), "pid {pid} outlived its command");
        }
    }

    /// Reads events until stdio.exit, failing past five seconds.
    async fn exit_of(rx: &mut tokio::sync::mpsc::UnboundedReceiver<crate::Outgoing>) -> Value {
        loop {
            let said = tokio::time::timeout(Duration::from_secs(5), rx.recv()).await.expect("no stdio.exit within 5 s").unwrap();
            let v: Value = serde_json::from_str(said.text()).unwrap();
            if v["type"] == "stdio.exit" {
                return v;
            }
        }
    }

    /// A server that stopped reading its stdin holds a writer blocked on a full pipe: a close, and its socket's
    /// close, kill it all the same, before anything of its stdin is let go.
    #[tokio::test]
    async fn a_server_that_never_reads_its_stdin_is_killed_by_a_close_and_by_its_sockets_close() {
        let b = bench();
        let dir = tempfile::tempdir().unwrap();
        for by_socket in [false, true] {
            let (c, mut rx) = conn(None);
            let pid_file = dir.path().join(format!("deaf-{by_socket}.pid"));
            let cmd = format!("echo $$ > {}; exec sleep 120", pid_file.display());
            reply(&b, &c, json!({"id": 1, "op": "stdio.open", "stdioId": "s1", "cmd": cmd})).await;
            let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
            while !pid_file.exists() || std::fs::read_to_string(&pid_file).unwrap().trim().is_empty() {
                assert!(tokio::time::Instant::now() < deadline, "the command never started");
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
            let pid: i32 = std::fs::read_to_string(&pid_file).unwrap().trim().parse().unwrap();
            let chunk = b64(&vec![b'x'; 64 * 1024]);
            for _ in 0..4 {
                reply(&b, &c, json!({"id": 2, "op": "stdio.write", "stdioId": "s1", "data": chunk})).await;
            }
            if by_socket {
                c.close();
            } else {
                reply(&b, &c, json!({"id": 3, "op": "stdio.close", "stdioId": "s1"})).await;
            }
            assert_eq!(exit_of(&mut rx).await, json!({"type": "stdio.exit", "stdioId": "s1"}));
            tokio::time::sleep(Duration::from_millis(100)).await;
            assert!(nix::sys::signal::kill(nix::unistd::Pid::from_raw(pid), None).is_err(), "pid {pid} outlived its close");
        }
    }

    /// The daemon runs a held command on its own computer alone: a frame naming a machine, whose command line the
    /// machine's own config chose, is refused before anything starts, and so are a write and a close naming one.
    #[tokio::test]
    async fn a_frame_naming_a_machine_is_refused_and_starts_nothing() {
        let b = bench();
        let (c, _rx) = conn(None);
        let dir = tempfile::tempdir().unwrap();
        let ran = dir.path().join("ran");
        let cmd = format!("touch {}", ran.display());
        for frame in [
            json!({"id": 1, "op": "stdio.open", "stdioId": "m1", "machineId": "k1", "cmd": cmd}),
            json!({"id": 1, "op": "stdio.open", "stdioId": "m2", "machineId": null, "cmd": cmd}),
            json!({"id": 1, "op": "stdio.write", "stdioId": "m1", "machineId": "k1", "data": "eA=="}),
            json!({"id": 1, "op": "stdio.close", "stdioId": "m1", "machineId": "k1"}),
        ] {
            let said = reply(&b, &c, frame.clone()).await;
            assert_eq!(said["ok"], false, "{frame} was answered {said}");
            assert_eq!(said["code"], "forbidden", "{said}");
        }
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert!(!ran.exists(), "a command named for a machine ran on this computer");
        assert!(c.stdios.open.lock().unwrap().is_empty());
    }
}
