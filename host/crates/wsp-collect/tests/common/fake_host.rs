// SPDX-License-Identifier: AGPL-3.0-only
//! A computer described as data, port of packages/collect/test/fake-host.ts at a3b870a6e: files with sizes or text,
//! dirs as a trailing slash with a total size, binaries on PATH, and canned command output. Every read of a file's
//! text, every line read and every run is logged in `calls`, as the TS fake does.
#![allow(dead_code)]
use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;

use futures::future::LocalBoxFuture;
use futures::stream::LocalBoxStream;
use futures::{FutureExt, StreamExt};
use indexmap::IndexMap;
use wsp_collect::host::{Host, HostExec, HostFs, Platform, RunOptions, Stat, StatKind};

pub const HOME: &str = "/Users/dev";
/// The modification time every file has unless the laptop names another.
pub const MTIME: f64 = 1000.0;
/// A text file's line that starts with this makes `lines` yield an `Err` there (the TS iterator throwing part way).
pub const LINES_FAIL: &str = "\u{0}fail:";

/// A file's size, or its text (whose size is its UTF-8 length).
#[derive(Debug, Clone, PartialEq)]
pub enum FileValue {
    Size(u64),
    Text(String),
}

impl FileValue {
    fn bytes(&self) -> u64 {
        match self {
            FileValue::Size(n) => *n,
            FileValue::Text(t) => t.len() as u64,
        }
    }
}

impl From<u64> for FileValue {
    fn from(n: u64) -> FileValue {
        FileValue::Size(n)
    }
}

impl From<i32> for FileValue {
    fn from(n: i32) -> FileValue {
        FileValue::Size(n as u64)
    }
}

impl From<&str> for FileValue {
    fn from(t: &str) -> FileValue {
        FileValue::Text(t.to_string())
    }
}

impl From<String> for FileValue {
    fn from(t: String) -> FileValue {
        FileValue::Text(t)
    }
}

/// fake-host.ts:6. Paths may start with `~`, which is HOME.
#[derive(Debug, Clone, Default)]
pub struct FakeLaptop {
    pub platform: Option<Platform>,
    /// `~/.zshrc` with a size is a file of that many bytes; with text, its text; `~/.config/nvim/` is a dir.
    pub files: IndexMap<String, FileValue>,
    pub which: Vec<String>,
    /// Keyed by `cmd arg arg`, led by `NAME=value` for each variable the run adds; a value is stdout of a successful run.
    pub exec: IndexMap<String, String>,
    pub mtimes: IndexMap<String, f64>,
    pub shell: Option<String>,
    pub terminal: Option<String>,
    pub xdg_config_home: Option<String>,
}

impl FakeLaptop {
    pub fn new() -> FakeLaptop {
        FakeLaptop::default()
    }

    pub fn file(mut self, path: &str, value: impl Into<FileValue>) -> FakeLaptop {
        self.files.insert(path.to_string(), value.into());
        self
    }

    pub fn which(mut self, bins: &[&str]) -> FakeLaptop {
        self.which.extend(bins.iter().map(|b| b.to_string()));
        self
    }

    pub fn exec(mut self, key: &str, stdout: &str) -> FakeLaptop {
        self.exec.insert(key.to_string(), stdout.to_string());
        self
    }

    pub fn mtime(mut self, path: &str, ms: f64) -> FakeLaptop {
        self.mtimes.insert(path.to_string(), ms);
        self
    }

    pub fn platform(mut self, p: Platform) -> FakeLaptop {
        self.platform = Some(p);
        self
    }

    pub fn shell(mut self, s: &str) -> FakeLaptop {
        self.shell = Some(s.to_string());
        self
    }

    pub fn terminal(mut self, t: &str) -> FakeLaptop {
        self.terminal = Some(t.to_string());
        self
    }

    pub fn xdg_config_home(mut self, x: &str) -> FakeLaptop {
        self.xdg_config_home = Some(x.to_string());
        self
    }
}

fn at_home(path: &str) -> String {
    match path.strip_prefix('~') {
        Some(rest) => format!("{HOME}{rest}"),
        None => path.to_string(),
    }
}

/// JavaScript's default sort: UTF-16 code units.
fn js_sorted(mut v: Vec<String>) -> Vec<String> {
    v.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
    v
}

pub struct FakeFs {
    files: IndexMap<String, FileValue>,
    mtimes: IndexMap<String, f64>,
    calls: Rc<RefCell<Vec<String>>>,
}

impl FakeFs {
    fn mtime_ms(&self, path: &str) -> f64 {
        self.mtimes.get(path).copied().unwrap_or(MTIME)
    }
}

impl HostFs for FakeFs {
    fn stat<'a>(&'a self, path: &'a str) -> LocalBoxFuture<'a, Option<Stat>> {
        async move {
            if let Some(dir) = self.files.get(&format!("{path}/")) {
                return Some(Stat { kind: StatKind::Dir, bytes: dir.bytes(), mtime_ms: self.mtime_ms(path) });
            }
            if let Some(own) = self.files.get(path) {
                return Some(Stat { kind: StatKind::File, bytes: own.bytes(), mtime_ms: self.mtime_ms(path) });
            }
            let prefix = format!("{path}/");
            let mut bytes = 0;
            let mut found = false;
            for (k, v) in &self.files {
                if k.starts_with(&prefix) {
                    found = true;
                    bytes += v.bytes();
                }
            }
            found.then(|| Stat { kind: StatKind::Dir, bytes, mtime_ms: self.mtime_ms(path) })
        }
        .boxed_local()
    }

    fn list<'a>(&'a self, dir: &'a str) -> LocalBoxFuture<'a, Vec<String>> {
        async move {
            let prefix = format!("{dir}/");
            let mut names = BTreeSet::new();
            for k in self.files.keys() {
                if let Some(rest) = k.strip_prefix(&prefix) {
                    let name = rest.split('/').next().unwrap_or("");
                    if !name.is_empty() {
                        names.insert(name.to_string());
                    }
                }
            }
            js_sorted(names.into_iter().collect())
        }
        .boxed_local()
    }

    fn read_text<'a>(&'a self, path: &'a str) -> LocalBoxFuture<'a, Option<String>> {
        async move {
            self.calls.borrow_mut().push(format!("read {path}"));
            match self.files.get(path) {
                Some(FileValue::Text(t)) => Some(t.clone()),
                _ => None,
            }
        }
        .boxed_local()
    }

    fn walk<'a>(&'a self, dir: &'a str) -> LocalBoxFuture<'a, Vec<String>> {
        async move {
            let prefix = format!("{dir}/");
            js_sorted(self.files.keys().filter(|k| k.starts_with(&prefix) && !k.ends_with('/')).cloned().collect())
        }
        .boxed_local()
    }

    /// The TS fake's lines: the text split on LF, a trailing LF giving one last empty line; `holding` is ignored.
    /// A line that starts with LINES_FAIL is the read failing there: an `Err` with the rest as its message, and no more.
    fn lines<'a>(&'a self, path: &'a str, _holding: Option<&'a str>) -> LocalBoxStream<'a, std::io::Result<String>> {
        self.calls.borrow_mut().push(format!("lines {path}"));
        let lines: Vec<std::io::Result<String>> = match self.files.get(path) {
            Some(FileValue::Text(t)) => {
                let mut out = Vec::new();
                for l in t.split('\n') {
                    if let Some(message) = l.strip_prefix(LINES_FAIL) {
                        out.push(Err(std::io::Error::other(message.to_string())));
                        break;
                    }
                    out.push(Ok(l.to_string()));
                }
                out
            }
            _ => Vec::new(),
        };
        futures::stream::iter(lines).boxed_local()
    }
}

pub struct FakeExec {
    which: Vec<String>,
    exec: IndexMap<String, String>,
    calls: Rc<RefCell<Vec<String>>>,
}

impl HostExec for FakeExec {
    fn which<'a>(&'a self, bin: &'a str) -> LocalBoxFuture<'a, bool> {
        async move { self.which.iter().any(|b| b == bin) }.boxed_local()
    }

    fn run<'a>(&'a self, cmd: &'a str, args: &'a [String], opts: RunOptions) -> LocalBoxFuture<'a, Option<String>> {
        async move {
            let mut words: Vec<String> = opts.env.iter().flatten().map(|(k, v)| format!("{k}={v}")).collect();
            words.push(cmd.to_string());
            words.extend(args.iter().cloned());
            let key = words.join(" ");
            let ms = opts.timeout_ms.map(|ms| format!(" ({ms} ms)")).unwrap_or_default();
            self.calls.borrow_mut().push(format!("run {key}{ms}"));
            self.exec.get(&key).cloned()
        }
        .boxed_local()
    }
}

/// A Host over the laptop, and the log of what was asked of it.
pub struct FakeHost {
    pub host: Host,
    calls: Rc<RefCell<Vec<String>>>,
}

impl FakeHost {
    /// Every `read <path>`, `lines <path>` and `run <key>[ (<ms> ms)]`, in order.
    pub fn calls(&self) -> Vec<String> {
        self.calls.borrow().clone()
    }
}

impl std::ops::Deref for FakeHost {
    type Target = Host;
    fn deref(&self) -> &Host {
        &self.host
    }
}

pub fn fake_host(laptop: FakeLaptop) -> FakeHost {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let files = laptop.files.iter().map(|(k, v)| (at_home(k), v.clone())).collect();
    let mtimes = laptop.mtimes.iter().map(|(k, v)| (at_home(k), *v)).collect();
    let fs = FakeFs { files, mtimes, calls: calls.clone() };
    let exec = FakeExec { which: laptop.which.clone(), exec: laptop.exec.clone(), calls: calls.clone() };
    let host = Host {
        platform: laptop.platform.unwrap_or(Platform::Darwin),
        home: HOME.to_string(),
        shell: laptop.shell.clone(),
        terminal: laptop.terminal.clone(),
        xdg_config_home: laptop.xdg_config_home.clone(),
        claude_config_dir: None,
        codex_home: None,
        stores: None,
        seeds: None,
        fs: Rc::new(fs),
        exec: Rc::new(exec),
    };
    FakeHost { host, calls }
}

/// `fakeHost()` with nothing on it.
pub fn empty_host() -> FakeHost {
    fake_host(FakeLaptop::new())
}

/// Runs a future to its end on this thread, as the TS tests await.
pub fn block_on<F: std::future::Future>(f: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(f)
}
