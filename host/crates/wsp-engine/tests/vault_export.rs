// SPDX-License-Identifier: AGPL-3.0-only
//! Ports of the export cases of packages/engine/test/vault.test.ts.
mod common;

use std::collections::HashMap;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicI64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use futures::stream::{self, StreamExt};
use futures::FutureExt;
use rand::RngCore;

use common::{bash, res, write, ExecFn, FakeMachine, RunFn};
use wsp_engine::errors::{EngineError, RoadError};
use wsp_engine::fetch::{Fetch, FetchRequest, FetchResponse, FetchStream};
use wsp_engine::machine::{ExecOptions, RunOptions};
use wsp_engine::vault::*;

const CACHE: &str = r"\( \( -type d \( -name 'node_modules' -o -name 'dist' -o -name '.cache' \) \) -o \( -type f \( -name '.DS_Store' \) \) -o \( -type d -exec test -f '{}/pyvenv.cfg' \; \) -o \( -type d -exec test -f '{}/.git' \; \) \)";

// Adapted: the Node write stream's default high water mark, which the TS reads from node:stream; Rust has no such
// number, so the cases that turn on a chunk's size against it keep Node's figure.
const WATERMARK: usize = 16 * 1024;

fn tmp(prefix: &str) -> tempfile::TempDir {
    tempfile::Builder::new().prefix(prefix).tempdir().unwrap()
}

fn s(p: &Path) -> String {
    p.to_str().unwrap().to_string()
}

fn strs(list: &[&str]) -> Vec<String> {
    list.iter().map(|x| x.to_string()).collect()
}

fn rel(p: &str) -> String {
    p.strip_prefix('/').unwrap_or(p).to_string()
}

fn random_bytes(n: usize) -> Vec<u8> {
    let mut b = vec![0u8; n];
    rand::thread_rng().fill_bytes(&mut b);
    b
}

fn write_bytes(root: &Path, relative: &str, bytes: &[u8]) {
    let p = root.join(relative);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, bytes).unwrap();
}

fn path_of(url: &str) -> String {
    let q = url.split_once("path=").expect("a path= query").1;
    common::url_decode(q.split('&').next().unwrap())
}

fn tar_read(args: &[&str], archive: &[u8]) -> Vec<u8> {
    let mut child = Command::new("tar").args(args).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let data = archive.to_vec();
    let writer = std::thread::spawn(move || {
        let _ = stdin.write_all(&data);
    });
    let out = child.wait_with_output().unwrap();
    writer.join().unwrap();
    assert!(out.status.success(), "tar {args:?} did not finish whole: {}", String::from_utf8_lossy(&out.stderr));
    out.stdout
}

fn listing(tgz: &[u8]) -> Vec<String> {
    let out = String::from_utf8(tar_read(&["-tzf", "-"], tgz)).unwrap();
    let mut names: Vec<String> = out
        .trim()
        .split('\n')
        .filter(|l| !l.is_empty())
        .map(|l| l.strip_prefix("./").unwrap_or(l).trim_end_matches('/').to_string())
        .filter(|l| l != "." && !l.is_empty())
        .collect();
    names.sort();
    names
}

fn rule() -> CacheRule {
    CacheRule { dirs: strs(&["node_modules", "dist", ".cache"]), files: strs(&[".DS_Store"]), markers: strs(&["pyvenv.cfg"]) }
}

fn rule_with_git() -> CacheRule {
    CacheRule { dirs: strs(&["node_modules", "dist", ".cache"]), files: strs(&[".DS_Store"]), markers: strs(&["pyvenv.cfg", ".git"]) }
}

/// A folder with source, ignored state, a repository and every cache shape the rule names.
fn folder() -> tempfile::TempDir {
    let d = tmp("wsp-export-src-");
    let r = d.path();
    write(r, "src/a.ts", "a\n");
    write(r, "src/cache-utils.ts", "a source file whose name holds the word\n");
    write(r, ".env", "TOKEN=x\n");
    write(r, ".git/config", "[core]\n");
    write(r, ".git/build/keep", "nothing under .git is judged\n");
    write(r, "node_modules/left/index.js", "cache\n");
    write(r, "dist/out.js", "output\n");
    write(r, ".cache/x", "cache\n");
    write(r, "notes/CacheNotes.md", "the word, any case, still not a cache\n");
    write(r, "my-cache-service/README.md", "a directory whose name holds the word\n");
    write(r, "src/dist", "a file named like a cache directory\n");
    write(r, ".DS_Store", "finder");
    write(r, "myenv/pyvenv.cfg", "home = /usr/bin\n");
    write(r, "myenv/bin/python", "venv by marker\n");
    write(r, "deep/dir/.DS_Store", "finder");
    std::fs::create_dir(r.join("really-empty")).unwrap();
    d
}

/// This computer's bash runs the script and the download serves the file the URL names. With `chunk_bytes` the body
/// arrives in pieces of that size a tick apart, the way an archive off a real machine does.
#[derive(Default)]
struct GuestFetch {
    chunk_bytes: Option<usize>,
}

#[async_trait(?Send)]
impl Fetch for GuestFetch {
    async fn fetch(&self, req: FetchRequest) -> Result<FetchResponse, RoadError> {
        let path = path_of(&req.url);
        if req.method == "PUT" {
            std::fs::write(&path, req.body.unwrap_or_default()).unwrap();
            return Ok(FetchResponse { status: 200, headers: vec![], body: vec![] });
        }
        match std::fs::read(&path) {
            Err(_) => Ok(FetchResponse { status: 404, headers: vec![], body: vec![] }),
            Ok(body) => Ok(FetchResponse { status: 200, headers: vec![("content-length".to_string(), body.len().to_string())], body }),
        }
    }
    async fn fetch_stream(&self, req: FetchRequest) -> Result<FetchStream, RoadError> {
        let res = self.fetch(req).await?;
        let chunks: Vec<Vec<u8>> = match self.chunk_bytes {
            Some(n) if res.ok() => res.body.chunks(n).map(|c| c.to_vec()).collect(),
            _ => vec![res.body],
        };
        let paced = self.chunk_bytes.is_some();
        let body = stream::iter(chunks)
            .then(move |c| async move {
                if paced {
                    tokio::time::sleep(Duration::from_millis(1)).await;
                }
                Ok::<_, RoadError>(c)
            })
            .boxed_local();
        Ok(FetchStream { status: res.status, headers: res.headers, body: Some(body) })
    }
}

/// Serves the file its URL names in `chunks` pieces, each asked for only once the reader has taken the one before, and
/// runs `at_end` as the body closes.
struct ChunkedBody {
    chunks: usize,
    at_end: Arc<dyn Fn()>,
    size: Arc<AtomicUsize>,
    enqueued: Arc<AtomicUsize>,
}

impl ChunkedBody {
    fn new(chunks: usize, at_end: Arc<dyn Fn()>) -> Arc<Self> {
        Arc::new(ChunkedBody { chunks, at_end, size: Arc::new(AtomicUsize::new(0)), enqueued: Arc::new(AtomicUsize::new(0)) })
    }
    fn size(&self) -> usize {
        self.size.load(Ordering::SeqCst)
    }
    fn enqueued(&self) -> usize {
        self.enqueued.load(Ordering::SeqCst)
    }
}

#[async_trait(?Send)]
impl Fetch for ChunkedBody {
    async fn fetch(&self, req: FetchRequest) -> Result<FetchResponse, RoadError> {
        Ok(FetchResponse { status: 200, headers: vec![], body: std::fs::read(path_of(&req.url)).unwrap() })
    }
    async fn fetch_stream(&self, req: FetchRequest) -> Result<FetchStream, RoadError> {
        let body = std::fs::read(path_of(&req.url)).unwrap();
        let size = body.len().div_ceil(self.chunks);
        self.size.store(size, Ordering::SeqCst);
        let state = (body, 0usize, self.enqueued.clone(), self.at_end.clone());
        let stream = stream::unfold(state, move |(body, at, enqueued, at_end)| async move {
            tokio::task::yield_now().await;
            if at >= body.len() {
                at_end();
                return None;
            }
            let end = (at + size).min(body.len());
            let chunk = body[at..end].to_vec();
            enqueued.fetch_add(1, Ordering::SeqCst);
            Some((Ok::<_, RoadError>(chunk), (body, end, enqueued, at_end)))
        })
        .boxed_local();
        Ok(FetchStream { status: 200, headers: vec![], body: Some(stream) })
    }
}

/// An answer with a status and no body at all.
struct Bodyless(u16);

#[async_trait(?Send)]
impl Fetch for Bodyless {
    async fn fetch(&self, _req: FetchRequest) -> Result<FetchResponse, RoadError> {
        Ok(FetchResponse { status: self.0, headers: vec![], body: vec![] })
    }
    async fn fetch_stream(&self, _req: FetchRequest) -> Result<FetchStream, RoadError> {
        Ok(FetchStream { status: self.0, headers: vec![], body: None })
    }
}

struct Guest {
    machine: FakeMachine,
    cmds: Arc<Mutex<Vec<String>>>,
}

impl Guest {
    fn cmds(&self) -> Vec<String> {
        self.cmds.lock().unwrap().clone()
    }
}

fn guest() -> Guest {
    let cmds = Arc::new(Mutex::new(Vec::new()));
    let seen = cmds.clone();
    let exec: ExecFn = Arc::new(move |cmd: String, opts: ExecOptions| {
        seen.lock().unwrap().push(cmd.clone());
        Box::pin(async move { Ok(bash(&cmd, opts.stdin, None).await) })
    });
    let mut machine = FakeMachine::new(exec);
    machine.id = "mv".to_string();
    Guest { machine, cmds }
}

fn opts_with(fetch: Arc<dyn Fetch>) -> VaultOptions {
    VaultOptions { fetch: Some(fetch), ..Default::default() }
}

fn opts() -> VaultOptions {
    opts_with(Arc::new(GuestFetch::default()))
}

type Progress = Arc<Mutex<Vec<DownloadProgress>>>;

fn progress_log() -> Progress {
    Arc::new(Mutex::new(Vec::new()))
}

fn disk(into: &Path) -> u64 {
    std::fs::metadata(into).map(|m| m.len()).unwrap_or(0)
}

fn increasing(seen: &[DownloadProgress]) -> bool {
    seen.iter().enumerate().all(|(i, p)| i == 0 || p.bytes > seen[i - 1].bytes)
}

fn tar_bytes() -> Vec<u8> {
    format!("fake-tgz-bytes-{}", "x".repeat(64)).into_bytes()
}

struct Stub {
    machine: FakeMachine,
    exec_cmds: Arc<Mutex<Vec<String>>>,
    runs: Arc<Mutex<Vec<String>>>,
    fetch: Arc<StubFetch>,
}

struct StubFetch {
    calls: AtomicUsize,
    puts: Mutex<Vec<(String, Vec<u8>)>>,
    guest_files: Arc<Mutex<HashMap<String, Vec<u8>>>>,
}

#[async_trait(?Send)]
impl Fetch for StubFetch {
    async fn fetch(&self, req: FetchRequest) -> Result<FetchResponse, RoadError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if req.method == "PUT" {
            self.puts.lock().unwrap().push((req.url.clone(), req.body.clone().unwrap_or_default()));
            return Ok(FetchResponse { status: 200, headers: vec![], body: vec![] });
        }
        match self.guest_files.lock().unwrap().get(&path_of(&req.url)) {
            None => Ok(FetchResponse { status: 404, headers: vec![], body: b"no such guest file".to_vec() }),
            Some(bytes) => Ok(FetchResponse { status: 200, headers: vec![], body: bytes.clone() }),
        }
    }
}

fn vault_stub() -> Stub {
    let exec_cmds = Arc::new(Mutex::new(Vec::new()));
    let runs = Arc::new(Mutex::new(Vec::new()));
    let guest_files = Arc::new(Mutex::new(HashMap::new()));
    let (ec, gf) = (exec_cmds.clone(), guest_files.clone());
    let exec: ExecFn = Arc::new(move |cmd: String, _opts: ExecOptions| {
        ec.lock().unwrap().push(cmd.clone());
        if let Some(rest) = cmd.strip_prefix("tar czf '") {
            if let Some((path, _)) = rest.split_once('\'') {
                gf.lock().unwrap().insert(path.to_string(), tar_bytes());
            }
        }
        Box::pin(async { Ok(common::ok()) })
    });
    let base = exec.clone();
    let run_log = runs.clone();
    let run: RunFn = Arc::new(move |script: String, _opts: RunOptions| {
        run_log.lock().unwrap().push(script.clone());
        base(script, ExecOptions::default())
    });
    let mut machine = FakeMachine::new(exec);
    machine.id = "mv".to_string();
    machine.run = Some(run);
    let fetch = Arc::new(StubFetch { calls: AtomicUsize::new(0), puts: Mutex::new(Vec::new()), guest_files });
    Stub { machine, exec_cmds, runs, fetch }
}

/// A home holding two sign-ins beside the work around them, one a file in a folder that also holds config and one a
/// whole folder, with a sibling whose name only starts like it.
fn signed_in_home() -> (tempfile::TempDir, Vec<String>) {
    let d = tmp("wsp-vault-logins-");
    let r = d.path();
    write(r, "home/.codex/auth.json", "{\"OPENAI_API_KEY\":\"x\"}\n");
    write(r, "home/.codex/config.toml", "model = \"x\"\n");
    write(r, "home/.aws/credentials", "[default]\n");
    write(r, "home/.aws-notes/x", "kept\n");
    write(r, "home/notes.md", "kept\n");
    let leave = vec![s(&r.join("home/.codex/auth.json")), s(&r.join("home/.aws"))];
    (d, leave)
}

const KEPT: [&str; 6] = ["home", "home/.aws-notes", "home/.aws-notes/x", "home/.codex", "home/.codex/config.toml", "home/notes.md"];

// TS: packages/engine/test/vault.test.ts:54
// exportPaths tars the paths in the guest and returns the bytes
#[tokio::test]
async fn export_paths_tars_the_paths_in_the_guest_and_returns_the_bytes() {
    let stub = vault_stub();
    let buf = export_paths(&stub.machine, &strs(&["/root/.claude-cfg"]), &opts_with(stub.fetch.clone())).await.unwrap();
    assert_eq!(buf, tar_bytes());
    let cmds = stub.exec_cmds.lock().unwrap().clone();
    let tar_cmd = cmds.iter().find(|c| c.starts_with("tar czf")).expect("a tar command");
    assert!(tar_cmd.contains("-C '/' 'root/.claude-cfg'"), "{tar_cmd}");
    assert!(cmds.iter().any(|c| c.starts_with("rm -f")), "guest temp cleaned");
}

// TS: packages/engine/test/vault.test.ts:63
// the tar and the untar run detached; the size read and the cleanup stay inline
#[tokio::test]
async fn the_tar_and_the_untar_run_detached_the_size_read_and_the_cleanup_stay_inline() {
    let stub = vault_stub();
    let o = VaultOptions { max_bytes: Some(1_000_000), ..opts_with(stub.fetch.clone()) };
    // The stub's wc answers an empty stdout, which the TS reads as 0 (Number("") is 0).
    export_paths(&stub.machine, &strs(&["/root/.zshrc"]), &o).await.unwrap();
    import_into(&stub.machine, &tar_bytes(), "/root", &opts_with(stub.fetch.clone())).await.unwrap();
    let words = |c: &String| c.split('\n').next().unwrap().split(' ').take(2).collect::<Vec<_>>().join(" ");
    let runs = stub.runs.lock().unwrap().clone();
    assert_eq!(runs.iter().map(words).collect::<Vec<_>>(), vec!["tar czf", "set -eo"]);
    assert!(runs[1].contains("tar xzf"));
    let exec_cmds = stub.exec_cmds.lock().unwrap().clone();
    let inline: Vec<String> =
        exec_cmds.iter().filter(|c| !runs.contains(c)).map(|c| c.split(' ').take(2).collect::<Vec<_>>().join(" ")).collect();
    assert_eq!(inline, vec!["wc -c", "rm -f"]);
}

// TS: packages/engine/test/vault.test.ts:91
// surfaces a failing tar instead of returning garbage
#[tokio::test]
async fn surfaces_a_failing_tar_instead_of_returning_garbage() {
    let stub = vault_stub();
    let machine = FakeMachine::new(Arc::new(|_cmd, _| Box::pin(async { Ok(res(2, "", "tar: /root/nope: No such file")) })));
    let err = export_paths(&machine, &strs(&["/root/nope"]), &opts_with(stub.fetch.clone())).await.unwrap_err();
    assert!(err.to_string().contains("tar"), "{err}");
}

// TS: packages/engine/test/vault.test.ts:99
// exportPaths refuses an archive over maxBytes before downloading it, and removes it
#[tokio::test]
async fn export_paths_refuses_an_archive_over_max_bytes_before_downloading_it_and_removes_it() {
    let stub = vault_stub();
    let base = stub.machine.exec.clone();
    let sized_exec: ExecFn = Arc::new(move |cmd: String, opts: ExecOptions| {
        if cmd.starts_with("wc -c <") {
            return async { Ok(res(0, "300000000\n", "")) }.boxed_local();
        }
        base(cmd, opts)
    });
    let sized = FakeMachine { exec: sized_exec, ..stub.machine.clone() };
    let o = VaultOptions { max_bytes: Some(200_000_000), ..opts_with(stub.fetch.clone()) };
    let err = export_paths(&sized, &strs(&["/root/big"]), &o).await.unwrap_err();
    assert_eq!(err, EngineError::VaultTooLarge { message: "the export was 286 MB, over the 191 MB cap".to_string(), bytes: 300_000_000 });
    assert_eq!(stub.fetch.calls.load(Ordering::SeqCst), 0);
    assert!(stub.exec_cmds.lock().unwrap().iter().any(|c| c.starts_with("rm -f")));
}

// TS: packages/engine/test/vault.test.ts:667
// the script tars the folder from its root, leaves every cache root the rule names behind, names them on stdout, and judges nothing under .git
#[tokio::test(flavor = "multi_thread")]
async fn the_script_tars_the_folder_from_its_root_leaves_every_cache_root_behind_and_judges_nothing_under_git() {
    let root = folder();
    let outdir = tmp("wsp-export-out-");
    let out = s(&outdir.path().join("folder.tgz"));
    let script = folder_export_script(root.path().to_str().unwrap(), &rule(), &out);
    let ran = bash(&script, None, None).await;
    assert_eq!(ran.stderr, "");
    assert_eq!(ran.exit_code, 0.into());
    let mut roots: Vec<String> = ran.stdout.trim().split('\n').map(str::to_string).collect();
    roots.sort();
    assert_eq!(roots, strs(&[".DS_Store", ".cache", "deep/dir/.DS_Store", "dist", "myenv", "node_modules"]));
    assert_eq!(
        listing(&std::fs::read(&out).unwrap()),
        strs(&[
            ".env",
            ".git",
            ".git/build",
            ".git/build/keep",
            ".git/config",
            "deep",
            "deep/dir",
            "my-cache-service",
            "my-cache-service/README.md",
            "notes",
            "notes/CacheNotes.md",
            "really-empty",
            "src",
            "src/a.ts",
            "src/cache-utils.ts",
            "src/dist"
        ])
    );
    assert!(!Path::new(&format!("{out}.list")).exists());
}

// TS: packages/engine/test/vault.test.ts:680
// exportPaths under a cache rule leaves installs, build output and nested worktrees behind at every path, keeps the repository, and the cap is read after they are gone
#[tokio::test(flavor = "multi_thread")]
async fn export_paths_under_a_cache_rule_leaves_installs_build_output_and_nested_worktrees_behind() {
    let g = guest();
    let dir = tmp("wsp-vault-home-");
    let root = dir.path();
    write(root, "proj/src/a.ts", "a\n");
    write(root, "proj/apps/web/src/lib/lruCache.ts", "a source file whose name holds the word\n");
    write(root, "proj/packages/collect/src/cache-names.ts", "another\n");
    write(root, ".claude/projects/-root-my-cache-service/S1.jsonl", "a session folder whose name holds the word\n");
    write(root, "proj/.git/config", "[core]\n");
    write(root, "proj/.git/node_modules/keep", "nothing under .git is judged\n");
    write_bytes(root, "proj/node_modules/big/blob", &random_bytes(400_000));
    write(root, "proj/dist/out.js", "output\n");
    write(root, "proj/.claude/worktrees/wt/.git", "gitdir: /elsewhere/.git/worktrees/wt\n");
    write(root, "proj/.claude/worktrees/wt/src/b.ts", "a nested checkout\n");
    write(root, "notes.md", "kept\n");
    let paths = vec![s(&root.join("proj")), s(&root.join("notes.md")), s(&root.join(".claude"))];
    let capped = VaultOptions { max_bytes: Some(100_000), ..opts() };
    let refused = export_paths(&g.machine, &paths, &capped).await.unwrap_err();
    assert!(matches!(refused, EngineError::VaultTooLarge { .. }), "{refused}");
    let tar = export_paths(&g.machine, &paths, &VaultOptions { exclude: Some(rule_with_git()), ..capped }).await.unwrap();
    let rroot = rel(&s(root));
    let names: Vec<String> = listing(&tar)
        .iter()
        .map(|l| if l.starts_with(&rroot) { l.get(rroot.len() + 1..).unwrap_or("").to_string() } else { l.clone() })
        .collect();
    assert_eq!(
        names,
        strs(&[
            ".claude",
            ".claude/projects",
            ".claude/projects/-root-my-cache-service",
            ".claude/projects/-root-my-cache-service/S1.jsonl",
            "notes.md",
            "proj",
            "proj/.claude",
            "proj/.claude/worktrees",
            "proj/.git",
            "proj/.git/config",
            "proj/.git/node_modules",
            "proj/.git/node_modules/keep",
            "proj/apps",
            "proj/apps/web",
            "proj/apps/web/src",
            "proj/apps/web/src/lib",
            "proj/apps/web/src/lib/lruCache.ts",
            "proj/packages",
            "proj/packages/collect",
            "proj/packages/collect/src",
            "proj/packages/collect/src/cache-names.ts",
            "proj/src",
            "proj/src/a.ts"
        ])
    );
    let cmds = g.cmds();
    let script = cmds.iter().find(|c| c.contains("find ")).expect("a find script");
    assert!(script.contains("cd '/'"));
    let wh = paths.iter().map(|p| format!("'{}'", rel(p))).collect::<Vec<_>>().join(" ");
    assert!(script.contains(&format!("find {wh} -mindepth 1 -path '*/.git' -prune -o {CACHE} -prune -print > ")), "{script}");
    assert!(script.contains(&format!(r"printf '%s\0' {wh} > ")), "{script}");
    assert!(
        script
            .contains(&format!(r"find {wh} -mindepth 1 \( -path '*/.git' -o -path '*/.git/*' \) -print0 -o {CACHE} -prune -o -print0 >> ")),
        "{script}"
    );
    let tar_at = script.find("tar czf '/tmp/wsp-vault-").expect("the tar line");
    let out = script[tar_at + "tar czf '".len()..].split('\'').next().unwrap();
    assert!(out.ends_with(".tgz"));
    assert!(script.contains(&format!("tar czf '{out}' --no-recursion --null -T '{out}.keep'\n")), "{script}");
    assert_eq!(cmds.iter().filter(|c| c.starts_with("rm -f")).count(), 2);
}

// TS: packages/engine/test/vault.test.ts:739
// exportPaths leaves each path it is told to leave out of the archive, everything under it too, with a cache rule and without
#[tokio::test(flavor = "multi_thread")]
async fn export_paths_leaves_each_path_it_is_told_to_leave_out_of_the_archive_with_a_cache_rule_and_without() {
    let g = guest();
    let (dir, leave) = signed_in_home();
    let root = s(dir.path());
    let rroot = rel(&root);
    let names = |tar: &[u8]| -> Vec<String> {
        listing(tar).iter().filter(|l| l.starts_with(&format!("{rroot}/"))).map(|l| l[rroot.len() + 1..].to_string()).collect()
    };
    let home = vec![s(&dir.path().join("home"))];
    let plain = export_paths(&g.machine, &home, &VaultOptions { leave: leave.clone(), ..opts() }).await.unwrap();
    assert_eq!(names(&plain), strs(&KEPT));
    let ruled_rule = CacheRule { dirs: strs(&["node_modules"]), files: vec![], markers: vec![] };
    let ruled = export_paths(&g.machine, &home, &VaultOptions { leave, exclude: Some(ruled_rule), ..opts() }).await.unwrap();
    assert_eq!(names(&ruled), strs(&KEPT));
    let home_dir = "home";
    let flags = format!("--exclude='{rroot}/{home_dir}/.codex/auth.json' --exclude='{rroot}/{home_dir}/.aws'");
    let tars: Vec<String> = g.cmds().into_iter().filter(|c| c.contains("tar czf")).collect();
    assert!(!tars.is_empty());
    assert!(tars.iter().all(|c| c.contains(&flags)), "{tars:?}");
}

// TS: packages/engine/test/vault.test.ts:816
// exportPaths judges the paths it is handed too: the install a box's worktrees share at the top of the home never travels, and what is left fits the cap
#[tokio::test(flavor = "multi_thread")]
async fn export_paths_judges_the_paths_it_is_handed_too_so_the_shared_install_never_travels() {
    let g = guest();
    let dir = tmp("wsp-vault-box-");
    let root = dir.path();
    write(root, "wsp/src/a.ts", "the checkout the person works in\n");
    write_bytes(root, "wsp/node_modules/big/blob", &random_bytes(400_000));
    write_bytes(root, "node_modules/big/blob", &random_bytes(400_000));
    write_bytes(root, ".pnpm-store/v3/files/00/deadbeef", &random_bytes(400_000));
    write(root, "notes.md", "kept\n");
    let r = CacheRule {
        dirs: strs(&["node_modules", ".pnpm-store", "dist"]),
        files: strs(&[".DS_Store"]),
        markers: strs(&["pyvenv.cfg", ".git"]),
    };
    let mut top: Vec<String> = std::fs::read_dir(root).unwrap().map(|e| e.unwrap().file_name().into_string().unwrap()).collect();
    top.sort();
    let paths: Vec<String> = top.iter().map(|n| s(&root.join(n))).collect();
    let o = VaultOptions { max_bytes: Some(100_000), exclude: Some(r), ..opts() };
    let tar = export_paths(&g.machine, &paths, &o).await.unwrap();
    let rroot = rel(&s(root));
    let names: Vec<String> = listing(&tar)
        .iter()
        .map(|l| if l.starts_with(&rroot) { l.get(rroot.len() + 1..).unwrap_or("").to_string() } else { l.clone() })
        .collect();
    assert_eq!(names, strs(&["notes.md", "wsp", "wsp/src", "wsp/src/a.ts"]));
    let cmds = g.cmds();
    let script = cmds.iter().find(|c| c.contains("find ")).expect("a find script");
    let wh = format!("'{}' '{}'", rel(&s(&root.join("notes.md"))), rel(&s(&root.join("wsp"))));
    assert!(script.contains(&format!("find {wh} -mindepth 1")), "{script}");
    assert!(script.contains(&format!(r"printf '%s\0' {wh} > ")), "{script}");
}

// TS: packages/engine/test/vault.test.ts:846
// an export that names no path at all archives nothing: no find, no walk of the root it runs in, and the refusal about caches is not what it says
#[tokio::test(flavor = "multi_thread")]
async fn an_export_that_names_no_path_at_all_archives_nothing() {
    let g = guest();
    let o = VaultOptions { exclude: Some(rule()), ..opts() };
    let tar = export_paths(&g.machine, &[], &o).await.unwrap();
    assert_eq!(listing(&tar), Vec::<String>::new());
    let cmds = g.cmds();
    let script = cmds.iter().find(|c| c.contains("tar czf")).expect("a tar command");
    let inner = script
        .strip_prefix("tar czf '/tmp/wsp-vault-")
        .and_then(|r| r.strip_suffix("' --no-recursion --null -T /dev/null"))
        .unwrap_or_else(|| panic!("{script}"));
    assert!(inner.ends_with(".tgz") && !inner.contains('\''), "{script}");
    assert!(!cmds.iter().any(|c| c.contains("find ")));
}

// TS: packages/engine/test/vault.test.ts:859
// refuses an export whose every path is a cache under the rule rather than running find with none
#[tokio::test(flavor = "multi_thread")]
async fn refuses_an_export_whose_every_path_is_a_cache_under_the_rule_rather_than_running_find_with_none() {
    let g = guest();
    let o = VaultOptions { exclude: Some(rule()), ..opts() };
    let err = export_paths(&g.machine, &strs(&["/root/node_modules", "/root/dist"]), &o).await.unwrap_err();
    assert_eq!(
        err.to_string(),
        "vault export: every path named is a cache under the rule (/root/node_modules, /root/dist), so there is nothing to archive"
    );
    assert_eq!(g.cmds(), Vec::<String>::new());
}

// TS: packages/engine/test/vault.test.ts:871
// streams the archive into a file on this computer with its size known up front, names the excluded roots, and leaves nothing on the guest
#[tokio::test(flavor = "multi_thread")]
async fn streams_the_archive_into_a_file_with_its_size_known_up_front_and_leaves_nothing_on_the_guest() {
    let g = guest();
    let root = folder();
    let dir = tmp("wsp-export-into-");
    let into = dir.path().join("folder.tgz");
    let seen = progress_log();
    let log = seen.clone();
    let o = VaultOptions { on_progress: Some(Arc::new(move |p| log.lock().unwrap().push(p))), ..opts() };
    let FolderExport { bytes, excluded } = export_folder(&g.machine, root.path().to_str().unwrap(), &rule(), &into, &o).await.unwrap();
    assert_eq!(excluded, strs(&[".DS_Store", ".cache", "deep/dir/.DS_Store", "dist", "myenv", "node_modules"]));
    assert_eq!(disk(&into), bytes);
    let names = listing(&std::fs::read(&into).unwrap());
    assert!(names.contains(&"src/a.ts".to_string()));
    assert!(!names.contains(&"dist/out.js".to_string()));
    let seen = seen.lock().unwrap().clone();
    assert_eq!(seen.last().copied(), Some(DownloadProgress { bytes, total: bytes }));
    assert!(seen.iter().all(|p| p.total == bytes && p.bytes <= bytes));
    let cmds = g.cmds();
    let packed = cmds.iter().find(|c| c.contains("tar czf")).expect("a tar command");
    let tmp_path = packed.split_once("tar czf '").unwrap().1.split('\'').next().unwrap().to_string();
    assert!(tmp_path.starts_with("/tmp/wsp-out-"), "{tmp_path}");
    assert!(!Path::new(&tmp_path).exists());
    assert!(cmds.iter().any(|c| c.starts_with("rm -f ") && c.contains(&tmp_path)));
}

// TS: packages/engine/test/vault.test.ts:895
// a 6 MB body off the guest reports bytes that only go up, never runs the file ahead of them, and ends at the size the file takes
#[tokio::test(flavor = "multi_thread")]
async fn a_6_mb_body_off_the_guest_reports_bytes_that_only_go_up_and_never_runs_the_file_ahead() {
    let g = guest();
    let root = folder();
    write_bytes(root.path(), "src/big.bin", &random_bytes(6 * 1024 * 1024));
    let dir = tmp("wsp-export-into-");
    let into = dir.path().join("folder.tgz");
    let seen = progress_log();
    let mid: Arc<Mutex<Vec<(u64, u64)>>> = Arc::new(Mutex::new(Vec::new()));
    let (log, mid_log, at) = (seen.clone(), mid.clone(), into.clone());
    let o = VaultOptions {
        on_progress: Some(Arc::new(move |p| {
            log.lock().unwrap().push(p);
            if p.bytes > 0 && p.bytes < p.total {
                mid_log.lock().unwrap().push((disk(&at), p.bytes));
            }
        })),
        ..opts_with(Arc::new(GuestFetch { chunk_bytes: Some(256 * 1024) }))
    };
    let FolderExport { bytes, .. } = export_folder(&g.machine, root.path().to_str().unwrap(), &rule(), &into, &o).await.unwrap();
    assert!(bytes > 4 * 1024 * 1024);
    let seen = seen.lock().unwrap().clone();
    assert!(seen.len() >= 4, "{}", seen.len());
    assert!(increasing(&seen));
    // The file trails the progress; it never runs ahead of it.
    let mid = mid.lock().unwrap().clone();
    assert!(mid.iter().enumerate().all(|(i, m)| m.0 < bytes && m.0 <= m.1 && (i == 0 || m.0 >= mid[i - 1].0)), "{mid:?}");
    assert_eq!(disk(&into), bytes);
    assert!(listing(&std::fs::read(&into).unwrap()).contains(&"src/big.bin".to_string()));
}

// TS: packages/engine/test/vault.test.ts:925
// progress arrives while the response is still open: every chunk of the body is reported before the body ends
#[tokio::test(flavor = "multi_thread")]
async fn progress_arrives_while_the_response_is_still_open() {
    let g = guest();
    let root = folder();
    let dir = tmp("wsp-export-open-");
    let into = dir.path().join("folder.tgz");
    const CHUNKS: usize = 4;
    let seen = progress_log();
    let reported_when_body_ended = Arc::new(AtomicI64::new(-1));
    let (log, ended) = (seen.clone(), reported_when_body_ended.clone());
    let at_end_seen = seen.clone();
    let body = ChunkedBody::new(
        CHUNKS,
        Arc::new(move || ended.store(at_end_seen.lock().unwrap().iter().filter(|p| p.bytes > 0).count() as i64, Ordering::SeqCst)),
    );
    let o = VaultOptions { on_progress: Some(Arc::new(move |p| log.lock().unwrap().push(p))), ..opts_with(body.clone()) };
    let FolderExport { bytes, .. } = export_folder(&g.machine, root.path().to_str().unwrap(), &rule(), &into, &o).await.unwrap();
    // Each chunk is asked for behind a yield while the path from one arriving to its report holds no wait, so each
    // report lands before the next chunk.
    assert!(body.size() < WATERMARK);
    assert_eq!(body.enqueued(), CHUNKS);
    assert_eq!(reported_when_body_ended.load(Ordering::SeqCst), CHUNKS as i64);
    let seen = seen.lock().unwrap().clone();
    assert!(increasing(&seen));
    assert_eq!(seen.last().copied(), Some(DownloadProgress { bytes, total: bytes }));
    assert_eq!(disk(&into), bytes);
    assert!(listing(&std::fs::read(&into).unwrap()).contains(&"src/a.ts".to_string()));
}

// TS: packages/engine/test/vault.test.ts:953
// the archive lands as it arrives: a chunk over the watermark is on disk by the time its own progress line is out
#[tokio::test(flavor = "multi_thread")]
async fn the_archive_lands_as_it_arrives_a_chunk_over_the_watermark_is_on_disk_by_its_progress_line() {
    let g = guest();
    let root = folder();
    const CHUNKS: usize = 4;
    write_bytes(root.path(), "src/big.bin", &random_bytes(2 * CHUNKS * WATERMARK));
    let dir = tmp("wsp-export-drain-");
    let into = dir.path().join("folder.tgz");
    let mid: Arc<Mutex<Vec<(u64, u64)>>> = Arc::new(Mutex::new(Vec::new()));
    let (mid_log, at) = (mid.clone(), into.clone());
    let body = ChunkedBody::new(CHUNKS, Arc::new(|| {}));
    let o = VaultOptions {
        on_progress: Some(Arc::new(move |p| {
            if p.bytes > 0 && p.bytes < p.total {
                mid_log.lock().unwrap().push((disk(&at), p.bytes));
            }
        })),
        ..opts_with(body.clone())
    };
    let FolderExport { bytes, .. } = export_folder(&g.machine, root.path().to_str().unwrap(), &rule(), &into, &o).await.unwrap();
    assert!(body.size() > WATERMARK);
    let mid = mid.lock().unwrap().clone();
    assert_eq!(mid.len(), CHUNKS - 1);
    assert_eq!(mid.iter().map(|m| m.0).collect::<Vec<_>>(), mid.iter().map(|m| m.1).collect::<Vec<_>>());
    assert!(mid.last().unwrap().0 > mid[0].0);
    assert!(mid.last().unwrap().0 < bytes);
    assert_eq!(disk(&into), bytes);
    assert!(listing(&std::fs::read(&into).unwrap()).contains(&"src/big.bin".to_string()));
}

// TS: packages/engine/test/vault.test.ts:986
// exportPathsInto streams the paths into a file on this computer, and a download that fails or answers with no body leaves no half file behind
#[tokio::test(flavor = "multi_thread")]
async fn export_paths_into_streams_the_paths_and_a_failed_or_bodyless_download_leaves_no_half_file() {
    let g = guest();
    let root = folder();
    let dir = tmp("wsp-export-into-");
    let into = dir.path().join("state.tgz");
    let src = vec![ArchiveGroup { root: s(root.path()), paths: vec![s(&root.path().join("src"))] }];
    let bytes = export_paths_into(&g.machine, &src, &into, &opts()).await.unwrap();
    assert_eq!(disk(&into), bytes);
    let base: Vec<String> = listing(&std::fs::read(&into).unwrap()).iter().map(|l| l.rsplit('/').next().unwrap().to_string()).collect();
    assert_eq!(base, strs(&["src", "a.ts", "cache-utils.ts", "dist"]));
    let gone = dir.path().join("never.tgz");
    let err = export_paths_into(&g.machine, &src, &gone, &opts_with(Arc::new(Bodyless(503)))).await.unwrap_err();
    assert!(err.to_string().contains("HTTP 503"), "{err}");
    assert!(!gone.exists());
    let bodyless = dir.path().join("bodyless.tgz");
    let err = export_paths_into(&g.machine, &src, &bodyless, &opts_with(Arc::new(Bodyless(200)))).await.unwrap_err();
    assert!(err.to_string().contains("HTTP 200 with no body"), "{err}");
    assert!(!bodyless.exists());
}

// TS: packages/engine/test/vault.test.ts:1010
// exportPathsInto archives each group from its own root, so a copy under a mirror root travels at the path it mirrors
#[tokio::test(flavor = "multi_thread")]
async fn export_paths_into_archives_each_group_from_its_own_root() {
    let g = guest();
    let dir = tmp("wsp-export-groups-");
    let home_dir = tmp("wsp-export-home-");
    let mirror_dir = tmp("wsp-export-mirror-");
    let (home, mirror) = (home_dir.path(), mirror_dir.path());
    let home_rel = rel(&s(home));
    write(home, "sessions/s.jsonl", "a session\n");
    write(home, "store.db", "every project's rows\n");
    write(mirror, &format!("{home_rel}/store.db"), "this project's rows\n");
    let into = dir.path().join("state.tgz");
    let groups = vec![
        ArchiveGroup { root: "/".to_string(), paths: vec![s(&home.join("sessions"))] },
        ArchiveGroup { root: s(mirror), paths: vec![s(&mirror.join(&home_rel).join("store.db"))] },
    ];
    export_paths_into(&g.machine, &groups, &into, &opts()).await.unwrap();
    let out = tmp("wsp-export-out-");
    let status = Command::new("tar").args(["-xzf", &s(&into), "-C", &s(out.path())]).status().unwrap();
    assert!(status.success());
    assert_eq!(common::read(out.path(), &format!("{home_rel}/sessions/s.jsonl")), "a session\n");
    assert_eq!(common::read(out.path(), &format!("{home_rel}/store.db")), "this project's rows\n");
}

fn scrub_out(script: &str) -> String {
    const START: &str = "/tmp/wsp-vault-";
    let mut out = String::new();
    let mut rest = script;
    while let Some(i) = rest.find(START) {
        out.push_str(&rest[..i]);
        let after = &rest[i + START.len()..];
        let digits = after.chars().take_while(|c| c.is_ascii_digit()).count();
        let tail = &after[digits..];
        let named = tail.strip_prefix('-').map(|t| t.chars().take_while(|c| c.is_ascii_lowercase() || c.is_ascii_digit()).count());
        match named.filter(|n| digits > 0 && *n > 0 && tail[1 + n..].starts_with(".tgz")) {
            Some(n) => {
                out.push_str("OUT");
                rest = &tail[1 + n + ".tgz".len()..];
            }
            None => {
                out.push_str(START);
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

// TS: packages/engine/test/vault.test.ts:1044
// spells each archive script exactly: one tar from the root with no rule, the find and tar script under one
#[tokio::test]
async fn spells_each_archive_script_exactly() {
    let paths = strs(&["/root/.claude", "/root/proj", "/root/notes.md", "/root/my [odd] *dir?"]);
    let scripts: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let log = scripts.clone();
    let exec: ExecFn = Arc::new(|_, _| panic!("this stub has no exec"));
    let run: RunFn = Arc::new(move |script: String, _| {
        log.lock().unwrap().push(scrub_out(&script));
        Box::pin(async { Ok(res(1, "", "")) })
    });
    let mut machine = FakeMachine::new(exec);
    machine.run = Some(run);
    let err = export_paths(&machine, &paths, &VaultOptions::default()).await.unwrap_err();
    assert!(err.to_string().contains("vault export tar failed"), "{err}");
    let err = export_paths(&machine, &paths, &VaultOptions { exclude: Some(rule_with_git()), ..Default::default() }).await.unwrap_err();
    assert!(err.to_string().contains("vault export tar failed"), "{err}");
    let w = "'root/.claude' 'root/proj' 'root/notes.md' 'root/my [odd] *dir?'";
    let excluding = [
        "set -eo pipefail".to_string(),
        "cd '/'".to_string(),
        format!(r"find {w} -mindepth 1 -path '*/.git' -prune -o {CACHE} -prune -print > 'OUT.list'"),
        format!(r"printf '%s\0' {w} > 'OUT.keep'"),
        format!(r"find {w} -mindepth 1 \( -path '*/.git' -o -path '*/.git/*' \) -print0 -o {CACHE} -prune -o -print0 >> 'OUT.keep'"),
        "tar czf 'OUT' --no-recursion --null -T 'OUT.keep'".to_string(),
        r"sed 's|^\./||' 'OUT.list'".to_string(),
        "rm -f 'OUT.list' 'OUT.keep'".to_string(),
    ]
    .join("\n");
    assert_eq!(*scripts.lock().unwrap(), vec![format!("tar czf 'OUT' -C '/' {w}"), excluding]);
    let folder = [
        "set -eo pipefail".to_string(),
        "cd '/root/proj'".to_string(),
        format!(r"find '.' -mindepth 1 -path '*/.git' -prune -o {CACHE} -prune -print > '/tmp/out.tgz.list'"),
        r"printf '%s\0' '.' > '/tmp/out.tgz.keep'".to_string(),
        format!(
            r"find '.' -mindepth 1 \( -path '*/.git' -o -path '*/.git/*' \) -print0 -o {CACHE} -prune -o -print0 >> '/tmp/out.tgz.keep'"
        ),
        "tar czf '/tmp/out.tgz' --no-recursion --null -T '/tmp/out.tgz.keep'".to_string(),
        r"sed 's|^\./||' '/tmp/out.tgz.list'".to_string(),
        "rm -f '/tmp/out.tgz.list' '/tmp/out.tgz.keep'".to_string(),
    ]
    .join("\n");
    assert_eq!(folder_export_script("/root/proj", &rule_with_git(), "/tmp/out.tgz"), folder);
}

// TS: packages/engine/test/vault.test.ts:1065
// a folder that is not on the machine fails with a plain sentence before any download
#[tokio::test(flavor = "multi_thread")]
async fn a_folder_that_is_not_on_the_machine_fails_with_a_plain_sentence_before_any_download() {
    let g = guest();
    let missing = format!("{}/wsp-export-none-{:08x}", std::env::temp_dir().display(), rand::random::<u32>());
    let never = std::env::temp_dir().join("wsp-never.tgz");
    let err = export_folder(&g.machine, &missing, &rule(), &never, &opts()).await.unwrap_err();
    assert!(err.to_string().contains(&format!("{missing} is not a folder on the machine")), "{err}");
    assert!(!g.cmds().iter().any(|c| c.contains("tar czf")));
}
