// SPDX-License-Identifier: AGPL-3.0-only
//! What the core-c tests share: the own-process-group runner, the strays a run leaves, the gate loop, the scripted
//! guest machine, this computer's bash as a guest, and small stream readers. Each test file includes what it uses.
#![allow(dead_code)]

pub mod guest;
pub mod own_group;
pub mod strays;

use futures::StreamExt;
use wsp_adapters::port::{ExecFailure, ExecStream};

/// Bash lines that give a Mac what a Linux machine's runs lean on; empty on Linux (protocol/test/linux-shell.ts).
pub fn linux_shell_prelude() -> &'static str {
    if std::path::Path::new("/proc/1/stat").exists() {
        ""
    } else {
        "setsid() { exec perl -e 'setpgrp(0, 0); exec @ARGV or die $!' -- \"$@\"; }\nbase64() { local a=(); for x in \"$@\"; do [ \"$x\" = \"-w0\" ] || a+=(\"$x\"); done; /usr/bin/base64 \"${a[@]}\"; }\nexport -f setsid base64\n"
    }
}

/// A shell loop that waits for the gate file, running `each` on every pass, and ends its shell once the gate's folder
/// is gone or after `limit_s` seconds (protocol/test/stub-script.ts `gateLoop`).
pub fn gate_loop(gate: &str, each: &str, limit_s: u64) -> String {
    let pass = if each.is_empty() { String::new() } else { format!("{each}; ") };
    format!(
        "_g=\"{gate}\"; _end=$(($(date +%s) + {limit_s})); _n=0; while [ ! -f \"$_g\" ]; do {{ [ -d \"${{_g%/*}}\" ] && {{ [ $((_n % 20)) -ne 0 ] || [ \"$(date +%s)\" -le $_end ]; }}; }} || exit 1; {pass}sleep 0.05; _n=$((_n + 1)); done"
    )
}

/// `gate_loop` with no pass and the 30 s limit.
pub fn gate(gate: &str) -> String {
    gate_loop(gate, "", 30)
}

/// Every line of a stream, or the lines before its failure and the failure.
pub async fn collect(stream: &dyn ExecStream) -> (Vec<String>, Option<ExecFailure>) {
    let mut lines = stream.lines();
    let mut out = Vec::new();
    while let Some(next) = lines.next().await {
        match next {
            Ok(line) => out.push(line),
            Err(e) => return (out, Some(e)),
        }
    }
    (out, None)
}

/// Every line of a stream that must end without a failure.
pub async fn lines_ok(stream: &dyn ExecStream) -> Vec<String> {
    let (lines, failure) = collect(stream).await;
    assert!(failure.is_none(), "the stream failed after {lines:?}: {failure:?}");
    lines
}

/// The failure a stream must end on, with the lines before it.
pub async fn lines_err(stream: &dyn ExecStream) -> (Vec<String>, ExecFailure) {
    let (lines, failure) = collect(stream).await;
    (lines.clone(), failure.unwrap_or_else(|| panic!("the stream ended without a failure after {lines:?}")))
}

/// Waits until `check` holds, polling every 20 ms, failing after `timeout_ms` with `what`.
pub async fn wait_for(what: &str, timeout_ms: u64, mut check: impl FnMut() -> bool) {
    let until = std::time::Instant::now() + std::time::Duration::from_millis(timeout_ms);
    while !check() {
        assert!(std::time::Instant::now() < until, "waited {timeout_ms} ms for {what}");
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
}

/// A real sleep.
pub async fn sleep_ms(ms: u64) {
    tokio::time::sleep(std::time::Duration::from_millis(ms)).await;
}

/// Races `fut` against `ms`; `None` when the time ran out first.
pub async fn within<T>(ms: u64, fut: impl std::future::Future<Output = T>) -> Option<T> {
    tokio::time::timeout(std::time::Duration::from_millis(ms), fut).await.ok()
}

/// The text of a file, or "" where it cannot be read.
pub fn read(path: &str) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

/// A file's permission bits as octal text, `600` say.
pub fn mode_of(path: &str) -> String {
    use std::os::unix::fs::PermissionsExt;
    format!("{:o}", std::fs::metadata(path).expect(path).permissions().mode() & 0o777)
}

/// The names in a folder, sorted.
pub fn names(dir: &str) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir).map(|d| d.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect()).unwrap_or_default();
    v.sort();
    v
}

/// Every file under `dir`, at any depth, whose bytes hold `value`.
pub fn holding(dir: &str, value: &str) -> Vec<String> {
    fn walk(dir: &std::path::Path, value: &str, out: &mut Vec<String>) {
        for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, value, out);
            } else if std::fs::read(&p).map(|b| String::from_utf8_lossy(&b).contains(value)).unwrap_or(false) {
                out.push(p.to_string_lossy().into_owned());
            }
        }
    }
    let mut out = Vec::new();
    walk(std::path::Path::new(dir), value, &mut out);
    out.sort();
    out
}

/// An exec's exit code as the wire carries it.
pub fn code(n: i64) -> wsp_wire::types::js_number::JsInt {
    wsp_wire::types::js_number::JsInt(wsp_wire::types::js_number::JsNumber::from(n))
}

/// `shell_quote` as the protocol writes it, for test text only: `'...'`, an inner `'` as `'\''`.
pub fn sq(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}
