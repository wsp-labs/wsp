// SPDX-License-Identifier: AGPL-3.0-only
//! exec-detached.ts:229-399 at a4033dc34. Spec: spec/exec_run.md.
//!
//! The launch, the polls, the kill and the cleanup of one detached run, and the loop that drives them.
use std::time::{Duration, Instant};

use wsp_wire::wire::limits::EXEC_CHUNK_BYTES;
use wsp_words::js::{js_trim, to_number};

use crate::cancel::Cancel;
use crate::errors::{is_missing, EngineError, Result};
use crate::exec_detached::put::{
    put_files, said_by, GuestWrite, PutFilesOptions, UploadResult, CANCELLED_EXIT, DEADLINE_EXIT, HANDSHAKE, INLINE_EXEC_MS, RUN_DIR,
};
use crate::machine::{ExecOptions, ExecResult, Machine, OnLine, RunOptions};

const POLL_MS: u64 = 2_000;
const FIRST_POLL_MS: u64 = 250;
const NO_EXIT_NOTE: &str = "wsp: the command ended without reporting an exit code";
const SETSID_LINE: &str = r#"setsid nohup bash -c '( umask "$1"; exec bash "$0.sh" ) > "$0.out" 2> "$0.err" < /dev/null; echo $? > "$0.exit"' "$b" "$u" > /dev/null 2>&1 &"#;

/// exec-detached.ts:231. Puts `{base}.sh` on the guest and starts it in its own session, behind the `.d` claim.
pub async fn launch(machine: &dyn Machine, base: &str, script: &str) -> Result<UploadResult> {
    let before = vec![
        "u=$(umask)".to_string(),
        "umask 077".to_string(),
        format!("b={base}"),
        format!("mkdir \"$b.d\" 2>/dev/null || {{ echo {}; exit 0; }}", HANDSHAKE.launched),
    ];
    let after = vec![SETSID_LINE.to_string(), "echo $! > \"$b.pid\"".to_string(), format!("echo {}", HANDSHAKE.launched)];
    let file = GuestWrite { path: format!("{base}.sh"), text: script.to_string(), append: false };
    put_files(machine, &[file], PutFilesOptions { before, after, ..Default::default() }).await
}

/// exec-detached.ts:248. One read of the run's exit file and both streams from these offsets.
pub fn poll_command(base: &str, out_offset: u64, err_offset: u64) -> String {
    [
        format!("b={base}"),
        "echo WSP_POLL".to_string(),
        r#"printf '%s\n' "$(cat "$b.exit" 2>/dev/null)""#.to_string(),
        format!(r#"printf '%s\n' "$(tail -c +{} "$b.out" 2>/dev/null | head -c {} | base64 -w0)""#, out_offset + 1, EXEC_CHUNK_BYTES),
        format!(r#"printf '%s\n' "$(tail -c +{} "$b.err" 2>/dev/null | head -c {} | base64 -w0)""#, err_offset + 1, EXEC_CHUNK_BYTES),
        r#"p=$(cat "$b.pid" 2>/dev/null); if [ -n "$p" ] && kill -0 "$p" 2>/dev/null; then echo up; else echo down; fi"#.to_string(),
        "echo WSP_POLL_END".to_string(),
    ]
    .join("\n")
}

/// exec-detached.ts:260. TERM, two seconds, KILL, to the recorded group and pid.
pub fn kill_command(base: &str) -> String {
    format!(
        r#"b={base}; p=$(cat "$b.pid" 2>/dev/null); if [ -n "$p" ]; then kill -TERM -- "-$p" "$p" 2>/dev/null; sleep 2; kill -KILL -- "-$p" "$p" 2>/dev/null; fi; true"#
    )
}

/// exec-detached.ts:266. Every file of one run.
pub fn clean_command(base: &str) -> String {
    format!("rm -rf {base}.*")
}

/// exec-detached.ts:270.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Poll {
    pub exit: String,
    pub out: Vec<u8>,
    pub err: Vec<u8>,
    pub up: bool,
}

/// `Buffer.from(s, "base64")`: the sextets of every letter in the alphabet, url-safe letters too; any other character
/// is skipped; decoding stops at the first `=`.
fn node_base64(s: &str) -> Vec<u8> {
    let mut out = Vec::new();
    let (mut bits, mut nbits) = (0u32, 0u32);
    for c in s.chars().take_while(|&c| c != '=') {
        let v = match c {
            'A'..='Z' => c as u32 - 'A' as u32,
            'a'..='z' => c as u32 - 'a' as u32 + 26,
            '0'..='9' => c as u32 - '0' as u32 + 52,
            '+' | '-' => 62,
            '/' | '_' => 63,
            _ => continue,
        };
        bits = (bits << 6) | v;
        nbits += 6;
        if nbits >= 8 {
            nbits -= 8;
            out.push((bits >> nbits) as u8);
            bits &= (1 << nbits) - 1;
        }
    }
    out
}

/// exec-detached.ts:277. The first whole `WSP_POLL` .. `WSP_POLL_END` block, or `None`.
pub fn parse_poll(stdout: &str) -> Option<Poll> {
    let lines: Vec<&str> = stdout.split('\n').collect();
    let at = lines.iter().position(|line| *line == "WSP_POLL")?;
    if lines.get(at + 5) != Some(&"WSP_POLL_END") {
        return None;
    }
    Some(Poll {
        exit: js_trim(lines[at + 1]).to_string(),
        out: node_base64(lines[at + 2]),
        err: node_base64(lines[at + 3]),
        up: lines[at + 4] == "up",
    })
}

/// exec-detached.ts:285. Bytes in, lines out per stream: every chunk is kept for the stream's text and counted into
/// `offset`; with a listener, each complete line is handed over unless empty, and `flush` hands over the rest.
struct LineStream {
    all: Vec<u8>,
    pending: Vec<u8>,
    offset: u64,
    on_line: Option<OnLine>,
}

impl LineStream {
    fn new(on_line: Option<OnLine>) -> Self {
        LineStream { all: Vec::new(), pending: Vec::new(), offset: 0, on_line }
    }

    fn push(&mut self, chunk: &[u8]) {
        if chunk.is_empty() {
            return;
        }
        self.all.extend_from_slice(chunk);
        self.offset += chunk.len() as u64;
        let Some(on_line) = self.on_line.clone() else { return };
        self.pending.extend_from_slice(chunk);
        while let Some(nl) = self.pending.iter().position(|&b| b == b'\n') {
            let line = String::from_utf8_lossy(&self.pending[..nl]).into_owned();
            self.pending.drain(..=nl);
            if !line.is_empty() {
                on_line(&line);
            }
        }
    }

    fn flush(&mut self) {
        if let Some(on_line) = &self.on_line {
            if !self.pending.is_empty() {
                on_line(&String::from_utf8_lossy(&self.pending));
            }
        }
        self.pending.clear();
    }

    fn text(&self) -> String {
        String::from_utf8_lossy(&self.all).into_owned()
    }
}

/// Both streams flushed (stdout first) and the answer shaped like one exec's, with the note on its own line.
fn finish(out: &mut LineStream, err: &mut LineStream, exit_code: i64, note: Option<&str>) -> ExecResult {
    out.flush();
    err.flush();
    let stderr = err.text();
    let stderr = match note {
        None => stderr,
        Some(note) => {
            let sep = if stderr.is_empty() || stderr.ends_with('\n') { "" } else { "\n" };
            format!("{stderr}{sep}{note}\n")
        }
    };
    ExecResult { exit_code: exit_code.into(), stdout: out.text(), stderr }
}

/// Feeds one poll's bytes to both streams; true where a stream's chunk was full, so the next read comes at once.
fn read(out: &mut LineStream, err: &mut LineStream, poll: &Poll) -> bool {
    out.push(&poll.out);
    err.push(&poll.err);
    let full = EXEC_CHUNK_BYTES as usize;
    poll.out.len() == full || poll.err.len() == full
}

async fn send(machine: &dyn Machine, cmd: &str) -> Result<ExecResult> {
    machine.exec(cmd, ExecOptions { timeout_ms: Some(INLINE_EXEC_MS), ..Default::default() }).await
}

/// A pause that a cancel cuts short.
async fn pause(ms: u64, signal: Option<&Cancel>) {
    let sleep = tokio::time::sleep(Duration::from_millis(ms));
    match signal {
        Some(signal) => tokio::select! {
            _ = sleep => {}
            _ = signal.cancelled() => {}
        },
        None => sleep.await,
    }
}

/// exec-detached.ts:319. Runs the script on the machine as run() promises, under [`crate::exec_detached::RUN_DIR`].
pub async fn exec_detached(machine: &dyn Machine, script: &str, opts: RunOptions) -> Result<ExecResult> {
    exec_detached_in(machine, script, opts, RUN_DIR).await
}

/// exec-detached.ts:319 with its `runDir`: a machine somebody owns names its own folder, a test a local one.
pub async fn exec_detached_in(machine: &dyn Machine, script: &str, opts: RunOptions, run_dir: &str) -> Result<ExecResult> {
    let poll_ms = opts.poll_ms.unwrap_or(POLL_MS);
    let base = format!("{run_dir}/{}", hex::encode(rand::random::<[u8; 6]>()));
    let started = Instant::now();
    let cancelled = || opts.signal.as_ref().is_some_and(Cancel::is_cancelled);
    if cancelled() {
        return Ok(ExecResult { exit_code: CANCELLED_EXIT.into(), stdout: String::new(), stderr: "cancelled\n".into() });
    }

    let target: &dyn Machine = match &opts.launch_on {
        Some(launch_on) => launch_on.as_ref(),
        None => machine,
    };
    let launched = match launch(target, &base, script).await {
        Ok(launched) => launched,
        Err(e) => {
            let _ = send(machine, &clean_command(&base)).await;
            return Err(e);
        }
    };
    if launched.exit_code != 0 || !launched.stdout.contains(HANDSHAKE.launched) {
        let _ = send(machine, &clean_command(&base)).await;
        return Err(EngineError::Other {
            message: format!(
                "launch failed on {}: nothing came back saying {}, the word the guest prints once the run is up; {}",
                machine.id(),
                HANDSHAKE.launched,
                said_by(launched.exit_code, &launched.stdout, &launched.stderr)
            ),
            code: None,
        });
    }

    let mut out = LineStream::new(opts.on_line.clone());
    let mut err = LineStream::new(opts.on_line.clone());
    let mut wait = FIRST_POLL_MS.min(poll_ms);
    let mut downs = 0;
    loop {
        if cancelled() {
            let _ = send(machine, &kill_command(&base)).await;
            let _ = send(machine, &clean_command(&base)).await;
            return Ok(finish(&mut out, &mut err, CANCELLED_EXIT, Some("cancelled")));
        }
        if started.elapsed().as_millis() > u128::from(opts.deadline_ms) {
            let _ = send(machine, &kill_command(&base)).await;
            let last = send(machine, &poll_command(&base, out.offset, err.offset)).await.ok().and_then(|r| parse_poll(&r.stdout));
            if let Some(last) = last {
                read(&mut out, &mut err, &last);
            }
            let _ = send(machine, &clean_command(&base)).await;
            return Ok(finish(&mut out, &mut err, DEADLINE_EXIT, None));
        }
        let poll = match send(machine, &poll_command(&base, out.offset, err.offset)).await {
            Ok(answer) => parse_poll(&answer.stdout),
            Err(e) if matches!(e, EngineError::GuestUnusable { .. } | EngineError::MachineStopped { .. }) || is_missing(&e) => {
                return Err(e);
            }
            Err(_) => None,
        };
        let Some(poll) = poll else {
            pause(poll_ms, opts.signal.as_ref()).await;
            continue;
        };
        if read(&mut out, &mut err, &poll) {
            continue;
        }
        if !poll.exit.is_empty() {
            let _ = send(machine, &clean_command(&base)).await;
            let code = to_number(&poll.exit);
            return Ok(if code.is_finite() && code.fract() == 0.0 {
                finish(&mut out, &mut err, code as i64, None)
            } else {
                finish(&mut out, &mut err, -1, Some(NO_EXIT_NOTE))
            });
        }
        if !poll.up {
            downs += 1;
            if downs > 1 {
                let _ = send(machine, &clean_command(&base)).await;
                return Ok(finish(&mut out, &mut err, -1, Some(NO_EXIT_NOTE)));
            }
            continue;
        }
        pause(wait, opts.signal.as_ref()).await;
        wait = poll_ms.min(wait * 2);
    }
}
