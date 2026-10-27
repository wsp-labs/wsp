// SPDX-License-Identifier: AGPL-3.0-only
//! kept-stubs.ts, strays.ts and stub-script.ts at 65f6b89c7: the stand-in agents the process-driven TS tests run (Claude
//! Code on stream-json and Codex's app server, each serving one turn after another from one process), and how a test
//! reads whether a process is still there. The two scripts are the TS's own text, in `stubs/`, under node.
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

/// The node this box runs scripts with, the TS's `process.execPath`.
pub fn node() -> String {
    let path = std::env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin".into());
    path.split(':').map(|d| Path::new(d).join("node")).find(|p| p.is_file()).map(|p| p.display().to_string()).expect("node on PATH")
}

/// `writeStub(path, script)`: the script, executable, at the path.
pub fn write_stub(path: &Path, script: &str) {
    std::fs::write(path, script).expect("a stub");
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).expect("a mode");
}

/// `claudeKeeper(path)`.
pub fn claude_keeper(path: &Path) {
    write_stub(path, &include_str!("stubs/claude_keeper.js").replace("@@NODE@@", &node()));
}

/// `codexKeeper(path)`.
pub fn codex_keeper(path: &Path) {
    write_stub(path, &include_str!("stubs/codex_keeper.js").replace("@@NODE@@", &node()));
}

/// `pidOf(text)`: the process a reply names, read back off its words.
pub fn pid_of(text: Option<&str>) -> Option<u32> {
    let text = text?;
    let rest = &text[text.find(" from ")? + 6..];
    rest.split(' ').next()?.parse().ok()
}

/// `tokenOf(text)`.
pub fn token_of(text: Option<&str>) -> String {
    text.and_then(|t| t.rsplit_once(" token ")).map(|(_, token)| token.to_string()).unwrap_or_default()
}

/// Whether that pid is still there; signal 0 asks the kernel and sends nothing. A zombie is not there: a process whose
/// parent has not read its exit still answers signal 0.
pub fn alive(pid: u32) -> bool {
    match std::fs::read_to_string(format!("/proc/{pid}/stat")) {
        Ok(stat) => !stat.rsplit(')').next().unwrap_or("").trim_start().starts_with('Z'),
        Err(_) => false,
    }
}

/// `gone(pid)`: waits until the pid is gone, for five seconds.
pub async fn gone(pid: u32) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while alive(pid) {
        assert!(std::time::Instant::now() <= deadline, "process {pid} is still there");
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
}

/// `grandchild(file)`: the pid a command wrote to `file`, once the file holds one.
pub async fn grandchild(file: &Path) -> u32 {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        if let Some(pid) = std::fs::read_to_string(file).ok().and_then(|s| s.trim().parse::<u32>().ok()).filter(|p| *p > 0) {
            return pid;
        }
        assert!(std::time::Instant::now() <= deadline, "{} holds no pid", file.display());
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
}

/// `gateLoop(gate, { each, limitS })` of stub-script.ts: a shell loop that waits for the gate file, running `each` on every
/// pass, and ends its shell once the gate's folder is gone or after `limit_s` seconds.
pub fn gate_loop(gate: &str, each: &str, limit_s: u32) -> String {
    let pass = if each.is_empty() { String::new() } else { format!("{each}; ") };
    format!(
        "_g=\"{gate}\"; _end=$(($(date +%s) + {limit_s})); _n=0; while [ ! -f \"$_g\" ]; do {{ [ -d \"${{_g%/*}}\" ] && {{ [ $((_n % 20)) -ne 0 ] || [ \"$(date +%s)\" -le $_end ]; }}; }} || exit 1; {pass}sleep 0.05; _n=$((_n + 1)); done"
    )
}
