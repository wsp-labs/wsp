// SPDX-License-Identifier: AGPL-3.0-only
//! The host a verb brings up for itself (`host-start.ts`): a line that needs the host serving this state file and finds
//! none starts one detached and waits for its lock, so nobody has to type `wsp up` to get going.
use std::fs::{self, OpenOptions};
use std::io::{Read, Seek, SeekFrom};
use std::net::TcpStream;
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use wsp_cli::host_lock::{host_log_path, serving_host, HostLock, HostStarter, POLL_MS, SERVICE_WAIT_MS, STARTED_BY_ENV};
use wsp_words::app_ports::authority;
use wsp_words::exit::HostError;
use wsp_words::words::units::fmt_duration;

use crate::paths::dirname;

/// How many lines of a host's log a refusal that never served carries.
const LOG_TAIL_LINES: usize = 5;

/// The one line a verb prints before it waits, on stderr whatever the line prints on stdout.
pub fn starting_host_line(state_path: &str, log_path: &str) -> String {
    format!("starting the host for {state_path}; its log is {log_path}, and wsp down stops it")
}

fn no_host_answered_line(state_path: &str, wait_ms: u64) -> String {
    format!("no host answered for {state_path} within {}", fmt_duration(wait_ms as f64, None))
}

fn host_exited_line(state_path: &str, ended: &str, log_path: &str) -> String {
    format!("the host for {state_path} exited with {ended} before it served; its log is {log_path}")
}

/// The log from the byte a start began at, as non-empty trimmed lines.
fn log_since(path: &str, from: u64) -> Vec<String> {
    let Ok(mut file) = fs::File::open(path) else { return Vec::new() };
    if file.seek(SeekFrom::Start(from)).is_err() {
        return Vec::new();
    }
    let mut text = String::new();
    let _ = file.read_to_string(&mut text);
    text.lines().map(str::trim).filter(|l| !l.is_empty()).map(str::to_string).collect()
}

fn log_tail(path: &str) -> Vec<String> {
    let lines = log_since(path, 0);
    let from = lines.len().saturating_sub(LOG_TAIL_LINES);
    lines[from..].to_vec()
}

/// Whether the lock's host answers on its port.
fn answers(lock: &HostLock) -> bool {
    let address = wsp_cli::host_lock::dial_address(lock.address.as_deref());
    let target = authority(&address, lock.port.0);
    TcpStream::connect_timeout(&target.parse().unwrap_or_else(|_| ([127, 0, 0, 1], lock.port.0 as u16).into()), Duration::from_secs(1))
        .is_ok()
}

/// Starts a host serving the state file by running this same wsp's `up`, and answers its lock once it answers.
pub struct SpawnStarter {
    /// The words that start this wsp: its program, then whatever comes before the verb.
    pub wsp: Vec<String>,
    pub env: Vec<(String, String)>,
}

#[async_trait]
impl HostStarter for SpawnStarter {
    async fn start(&self, state_path: &str, say: &(dyn Fn(&str) + Send + Sync)) -> Result<Option<HostLock>, HostError> {
        let log_path = host_log_path(state_path);
        fs::create_dir_all(dirname(&log_path)).map_err(|e| HostError::new(e.to_string()))?;
        let wrote = fs::metadata(&log_path).map(|m| m.len()).unwrap_or(0);
        let log = OpenOptions::new().append(true).create(true).open(&log_path).map_err(|e| HostError::new(e.to_string()))?;
        let err = log.try_clone().map_err(|e| HostError::new(e.to_string()))?;
        let (program, before) = self.wsp.split_first().ok_or_else(|| HostError::new("no wsp command to start a host with"))?;
        let mut child = Command::new(program)
            .args(before)
            .args(["up", "--state", state_path, "--port", "0"])
            .env_clear()
            .envs(self.env.iter().cloned())
            .env(STARTED_BY_ENV, "verb")
            .stdin(Stdio::null())
            .stdout(Stdio::from(log))
            .stderr(Stdio::from(err))
            .process_group(0)
            .spawn()
            .map_err(|e| HostError::new(format!("{program}: {e}")))?;
        // `async_trait` fixes the lifetime of the `&str` inside `say`'s type to the call's own, so the line outlives it.
        say(Box::leak(starting_host_line(state_path, &log_path).into_boxed_str()));
        let deadline = Instant::now() + Duration::from_millis(SERVICE_WAIT_MS);
        loop {
            if let Some(lock) = serving_host(state_path).filter(answers) {
                return Ok(Some(lock));
            }
            if let Ok(Some(status)) = child.try_wait() {
                let said = log_since(&log_path, wrote);
                let ended = status.code().map_or_else(|| "no code".to_string(), |code| code.to_string());
                return Err(HostError::new(if said.is_empty() {
                    host_exited_line(state_path, &ended, &log_path)
                } else {
                    said.join("\n")
                }));
            }
            if Instant::now() >= deadline {
                let mut lines = vec![no_host_answered_line(state_path, SERVICE_WAIT_MS)];
                lines.extend(log_tail(&log_path));
                return Err(HostError::new(lines.join("\n")));
            }
            tokio::time::sleep(Duration::from_millis(POLL_MS)).await;
        }
    }
}
