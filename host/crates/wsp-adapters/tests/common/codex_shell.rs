// SPDX-License-Identifier: AGPL-3.0-only
//! Real shells for the codex, OpenCode and Cursor tests that run a built line: temp folders, `writeStub`
//! (packages/protocol/test/stub-script.ts), bash with variables on its input as a machine's exec runs it, and the fake
//! `codex app-server` of packages/adapter-codex/test/fake-app-server.ts (a node script, as there). Pull it in with
//! `#[path = "common/codex_shell.rs"] mod codex_shell;`. Test code only.
#![allow(dead_code)]

use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::{Map, Value};

/// A folder removed when dropped.
pub struct TempDir(pub PathBuf);

impl TempDir {
    pub fn new(prefix: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("{prefix}{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&dir).unwrap();
        TempDir(dir)
    }
    pub fn path(&self) -> &Path {
        &self.0
    }
    pub fn join(&self, part: &str) -> PathBuf {
        self.0.join(part)
    }
    pub fn s(&self) -> String {
        self.0.to_string_lossy().into_owned()
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// An executable script at `path`, its first line naming its interpreter. Returns the path.
pub fn write_stub(path: &Path, script: &str) -> String {
    let tmp = path.with_extension("stub-tmp");
    {
        let mut f = std::fs::File::create(&tmp).unwrap();
        f.write_all(script.as_bytes()).unwrap();
        f.sync_all().unwrap();
    }
    std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o755)).unwrap();
    std::fs::rename(&tmp, path).unwrap();
    path.to_string_lossy().into_owned()
}

/// What a shell run gave back.
#[derive(Clone, Debug)]
pub struct Ran {
    pub status: i32,
    pub stdout: String,
    pub stderr: String,
}

/// `<shell> -c <line>` under exactly `env`, `input` on stdin. Retries once on ETXTBSY, which a stub written while another
/// test forks can raise.
pub fn run_shell(shell: &str, line: &str, env: &[(String, String)], input: &str) -> Ran {
    for attempt in 0..5 {
        let mut cmd = Command::new(shell);
        cmd.arg("-c").arg(line).env_clear().stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
        for (k, v) in env {
            cmd.env(k, v);
        }
        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) if e.raw_os_error() == Some(26) && attempt < 4 => {
                std::thread::sleep(std::time::Duration::from_millis(20));
                continue;
            }
            Err(e) => panic!("{shell}: {e}"),
        };
        let mut stdin = child.stdin.take().unwrap();
        let input = input.to_string();
        let writer = std::thread::spawn(move || {
            let _ = stdin.write_all(input.as_bytes());
        });
        let out = child.wait_with_output().unwrap();
        let _ = writer.join();
        return Ran {
            status: out.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        };
    }
    unreachable!()
}

pub fn bash(line: &str, env: &[(String, String)]) -> Ran {
    run_shell("bash", line, env, "")
}

/// Variables as a question reads them off its input: one NUL-ended NAME=value each.
pub fn nul_env<'a>(env: impl IntoIterator<Item = (&'a String, &'a String)>) -> String {
    env.into_iter().map(|(k, v)| format!("{k}={v}\0")).collect()
}

/// `PATH=/usr/bin:/bin HOME=<tmp>`, what the TS tests run their scripts under.
pub fn plain_env() -> Vec<(String, String)> {
    vec![("PATH".into(), "/usr/bin:/bin".into()), ("HOME".into(), std::env::temp_dir().to_string_lossy().into_owned())]
}

pub fn pairs(items: &[(&str, &str)]) -> Vec<(String, String)> {
    items.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

const FAKE: &str = r#"#!/usr/bin/env node
const fs = require("fs");
const path = require("path");
const home = process.env.CODEX_HOME;
if (process.argv[2] !== "app-server") process.exit(2);
const answers = JSON.parse(fs.readFileSync(path.join(home, "answers.json"), "utf8"));
let sent = 0;
let buf = "";
const log = m => fs.appendFileSync(path.join(home, "requests.log"), JSON.stringify({ after: sent, ...m }) + "\n");
const handle = line => {
  const m = JSON.parse(line);
  log(m);
  if (m.id === undefined) return;
  const uses = fs.existsSync(path.join(home, "uses.json")) ? JSON.parse(fs.readFileSync(path.join(home, "uses.json"), "utf8")) : {};
  const at = uses[m.method] || 0;
  fs.writeFileSync(path.join(home, "uses.json"), JSON.stringify({ ...uses, [m.method]: at + 1 }));
  const listed = answers[m.method];
  const a = Array.isArray(listed) ? listed[Math.min(at, listed.length - 1)] : listed;
  if (a === "exit") process.exit(0);
  if (a === "silent") return;
  setTimeout(() => {
    process.stdout.write(JSON.stringify({ id: m.id, ...(a ?? { result: {} }) }) + "\n");
    sent++;
  }, 30);
};
process.stdin.on("data", d => {
  buf += d;
  for (let i = buf.indexOf("\n"); i >= 0; i = buf.indexOf("\n")) {
    handle(buf.slice(0, i));
    buf = buf.slice(i + 1);
  }
});
process.stdin.on("end", () => process.exit(0));
"#;

/// A CODEX_HOME holding the fake's answers by method ("exit" ends it on that request, "silent" never answers it, a list
/// answers the nth asking with its nth entry), a PATH with the fake as codex first, and what it read. TS
/// fake-app-server.ts:51.
pub struct FakeAppServer {
    pub dir: TempDir,
    pub home: String,
    pub path: String,
}

pub fn fake_app_server(answers: Value) -> FakeAppServer {
    let dir = TempDir::new("wsp-codex-server-");
    let home = dir.join("codex-home");
    let bin = dir.join("bin");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::create_dir_all(&bin).unwrap();
    write_stub(&bin.join("codex"), FAKE);
    std::fs::write(home.join("answers.json"), serde_json::to_string(&answers).unwrap()).unwrap();
    let path = format!("{}:/usr/bin:/bin", bin.to_string_lossy());
    FakeAppServer { home: home.to_string_lossy().into_owned(), path, dir }
}

impl FakeAppServer {
    /// Every line the fake read, each with `after`: how many answers it had sent by then.
    pub fn requests(&self) -> Vec<Map<String, Value>> {
        let log = Path::new(&self.home).join("requests.log");
        let text = std::fs::read_to_string(log).unwrap_or_default();
        text.lines()
            .filter(|l| !l.is_empty())
            .map(|l| match serde_json::from_str::<Value>(l).unwrap() {
                Value::Object(m) => m,
                _ => panic!("not an object"),
            })
            .collect()
    }
}

/// `sqlite3 <db> <sql>`.
pub fn sqlite(db: &Path, sql: &str) {
    let out = Command::new("sqlite3").arg(db).arg(sql).output().unwrap();
    assert!(out.status.success(), "sqlite3: {}", String::from_utf8_lossy(&out.stderr));
}
