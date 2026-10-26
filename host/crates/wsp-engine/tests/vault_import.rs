// SPDX-License-Identifier: AGPL-3.0-only
//! Ports of the import cases of packages/engine/test/vault.test.ts.
mod common;

use std::collections::VecDeque;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use rand::RngCore;

use common::{bash, write, ExecFn, FakeMachine, RunFn};
use wsp_engine::errors::{EngineError, RoadError};
use wsp_engine::fetch::{Fetch, FetchRequest, FetchResponse};
use wsp_engine::machine::{BytesLanded, ExecOptions, RunOptions};
use wsp_engine::vault::*;

const MIB: usize = 1024 * 1024;
const CAP: usize = 32 * MIB;
const REFUSAL: &str = r#"{"error":"Payload Too Large","limit":33554432}"#;

fn tmp(prefix: &str) -> tempfile::TempDir {
    tempfile::Builder::new().prefix(prefix).tempdir().unwrap()
}

fn s(p: &Path) -> String {
    p.to_str().unwrap().to_string()
}

fn strs(list: &[&str]) -> Vec<String> {
    list.iter().map(|x| x.to_string()).collect()
}

fn random_bytes(n: usize) -> Vec<u8> {
    let mut b = vec![0u8; n];
    rand::thread_rng().fill_bytes(&mut b);
    b
}

fn path_of(url: &str) -> String {
    let q = url.split_once("path=").expect("a path= query").1;
    common::url_decode(q.split('&').next().unwrap())
}

fn in_order(text: &str, needles: &[&str]) -> bool {
    let mut at = 0;
    for n in needles {
        match text[at..].find(n) {
            Some(i) => at += i + n.len(),
            None => return false,
        }
    }
    true
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

fn gnu_tar() -> bool {
    Command::new("tar").arg("--version").output().map(|o| String::from_utf8_lossy(&o.stdout).contains("GNU tar")).unwrap_or(false)
}

fn exists(p: &Path) -> bool {
    p.exists()
}

/// The tar of one file `blob.bin` holding `bytes`, as this computer's tar writes it.
fn tar_of_blob(bytes: &[u8]) -> Vec<u8> {
    let src = tmp("wsp-vault-src-");
    std::fs::write(src.path().join("blob.bin"), bytes).unwrap();
    let out = Command::new("tar").args(["-czf", "-", "-C", &s(src.path()), "blob.bin"]).output().unwrap();
    assert!(out.status.success());
    out.stdout
}

/// A valid gzip of exactly `target` bytes: the tar of `blob` deflated at level 0 (stored blocks, so random bytes do not
/// grow), then padded to the length through a gzip comment field, which every reader skips.
fn gzip_of_exact_length(blob: &[u8], target: usize) -> Vec<u8> {
    let src = tmp("wsp-vault-src-");
    std::fs::write(src.path().join("blob.bin"), blob).unwrap();
    let out = tmp("wsp-vault-tar-");
    let tar_path = s(&out.path().join("blob.tar"));
    assert!(Command::new("tar").args(["-cf", &tar_path, "-C", &s(src.path()), "blob.bin"]).status().unwrap().success());
    let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::none());
    enc.write_all(&std::fs::read(&tar_path).unwrap()).unwrap();
    let gz = enc.finish().unwrap();
    assert!(gz.len() < target, "archive already {} bytes, over the {target} asked", gz.len());
    let pad = target - gz.len();
    let mut header = gz[..10].to_vec();
    header[3] |= 0x10;
    let mut out = header;
    out.extend(std::iter::repeat_n(0x61u8, pad - 1));
    out.push(0);
    out.extend_from_slice(&gz[10..]);
    out
}

fn find_all(dir: &Path) -> Vec<String> {
    let out = Command::new("find").args([".", "-mindepth", "1"]).current_dir(dir).output().unwrap();
    let mut names: Vec<String> =
        String::from_utf8(out.stdout).unwrap().trim().split('\n').map(|l| l.strip_prefix("./").unwrap_or(l).to_string()).collect();
    names.sort();
    names
}

/// This computer stands in for the guest: uploads land at the path the URL names and exec runs the script under bash,
/// so the reassembly, the hash check and the extraction are the real commands. `drop_suffix` names a part the
/// provider acknowledges but never writes, the shape of a part lost on the guest.
struct PartsFetch {
    corrupt: bool,
    drop_suffix: Option<String>,
    puts: Mutex<Vec<(String, usize)>>,
}

impl PartsFetch {
    fn new(corrupt: bool, drop_suffix: Option<&str>) -> Arc<Self> {
        Arc::new(PartsFetch { corrupt, drop_suffix: drop_suffix.map(str::to_string), puts: Mutex::new(Vec::new()) })
    }
    fn puts(&self) -> Vec<(String, usize)> {
        self.puts.lock().unwrap().clone()
    }
    fn paths(&self) -> Vec<String> {
        self.puts().into_iter().map(|p| p.0).collect()
    }
    fn assert_no_part_left(&self) {
        for p in self.paths() {
            assert!(!exists(Path::new(&p)), "{p} is still on the machine");
        }
    }
}

#[async_trait(?Send)]
impl Fetch for PartsFetch {
    async fn fetch(&self, req: FetchRequest) -> Result<FetchResponse, RoadError> {
        let path = path_of(&req.url);
        let mut body = req.body.unwrap_or_default();
        self.puts.lock().unwrap().push((path.clone(), body.len()));
        if body.len() > CAP {
            return Ok(FetchResponse {
                status: 413,
                headers: vec![("content-type".to_string(), "application/json".to_string())],
                body: REFUSAL.as_bytes().to_vec(),
            });
        }
        let mid = body.len() / 2;
        if self.corrupt && !body.is_empty() {
            body[mid] ^= 0xff;
        }
        if !path.ends_with(self.drop_suffix.as_deref().unwrap_or("\0")) {
            std::fs::write(&path, &body).unwrap();
        }
        let answer = format!(r#"{{"ok":true,"path":{},"bytes":{}}}"#, serde_json::to_string(&path).unwrap(), body.len());
        Ok(FetchResponse { status: 200, headers: vec![], body: answer.into_bytes() })
    }
}

struct Guest {
    machine: FakeMachine,
    fetch: Arc<PartsFetch>,
}

impl Guest {
    fn opts(&self) -> VaultOptions {
        VaultOptions { fetch: Some(self.fetch.clone()), ..Default::default() }
    }
}

fn guest_with(fetch: Arc<PartsFetch>) -> Guest {
    let exec: ExecFn = Arc::new(move |cmd: String, opts: ExecOptions| Box::pin(async move { Ok(bash(&cmd, opts.stdin, None).await) }));
    let mut machine = FakeMachine::new(exec);
    machine.id = "mv".to_string();
    Guest { machine, fetch }
}

fn guest() -> Guest {
    guest_with(PartsFetch::new(false, None))
}

fn overlay(g: &Guest) -> VaultOptions {
    VaultOptions { overlay: true, ..g.opts() }
}

struct Stub {
    machine: FakeMachine,
    exec_cmds: Arc<Mutex<Vec<String>>>,
    fetch: Arc<StubFetch>,
}

struct StubFetch {
    calls: AtomicUsize,
    puts: Mutex<Vec<(String, Vec<u8>)>>,
}

#[async_trait(?Send)]
impl Fetch for StubFetch {
    async fn fetch(&self, req: FetchRequest) -> Result<FetchResponse, RoadError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.puts.lock().unwrap().push((req.url.clone(), req.body.clone().unwrap_or_default()));
        Ok(FetchResponse { status: 200, headers: vec![], body: vec![] })
    }
}

fn vault_stub() -> Stub {
    let exec_cmds = Arc::new(Mutex::new(Vec::new()));
    let ec = exec_cmds.clone();
    let exec: ExecFn = Arc::new(move |cmd: String, _opts: ExecOptions| {
        ec.lock().unwrap().push(cmd);
        Box::pin(async { Ok(common::ok()) })
    });
    let base = exec.clone();
    let run: RunFn = Arc::new(move |script: String, _opts: RunOptions| base(script, ExecOptions::default()));
    let mut machine = FakeMachine::new(exec);
    machine.id = "mv".to_string();
    machine.run = Some(run);
    Stub { machine, exec_cmds, fetch: Arc::new(StubFetch { calls: AtomicUsize::new(0), puts: Mutex::new(Vec::new()) }) }
}

fn stub_opts(stub: &Stub) -> VaultOptions {
    VaultOptions { fetch: Some(stub.fetch.clone()), ..Default::default() }
}

type Parts = Arc<Mutex<Vec<UploadProgress>>>;

fn part_log() -> (Parts, Arc<dyn Fn(UploadProgress)>) {
    let seen: Parts = Arc::new(Mutex::new(Vec::new()));
    let log = seen.clone();
    (seen, Arc::new(move |p| log.lock().unwrap().push(p)))
}

fn fetch_failed() -> RoadError {
    RoadError { name: "TypeError".to_string(), message: "fetch failed".to_string(), code: None, cause: None }
}

fn response(status: u16, body: &str) -> FetchResponse {
    FetchResponse { status, headers: vec![], body: body.as_bytes().to_vec() }
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

fn home_tar(root: &Path) -> Vec<u8> {
    let out = Command::new("tar").args(["-czf", "-", "-C", &s(root), "home"]).output().unwrap();
    assert!(out.status.success());
    out.stdout
}

// TS: packages/engine/test/vault.test.ts:73
// importInto uploads via uploadUrl and untars at the destination
#[tokio::test]
async fn import_into_uploads_via_upload_url_and_untars_at_the_destination() {
    let stub = vault_stub();
    let payload = b"payload-tgz".to_vec();
    import_into(&stub.machine, &payload, "/root", &stub_opts(&stub)).await.unwrap();
    let puts = stub.fetch.puts.lock().unwrap().clone();
    assert_eq!(puts.len(), 1);
    assert_eq!(puts[0].1, payload);
    let cmds = stub.exec_cmds.lock().unwrap().clone();
    let untar = cmds.iter().find(|c| c.contains("tar xzf")).expect("an untar command");
    assert!(untar.contains("-C '/root'"), "{untar}");
}

// TS: packages/engine/test/vault.test.ts:83
// an overlay import merges into the destination: no recursive unlink, files owned by the guest user
#[tokio::test]
async fn an_overlay_import_merges_into_the_destination_with_no_recursive_unlink() {
    let stub = vault_stub();
    let o = VaultOptions { overlay: true, ..stub_opts(&stub) };
    import_into(&stub.machine, b"payload-tgz", "/root", &o).await.unwrap();
    let cmds = stub.exec_cmds.lock().unwrap().clone();
    let untar = cmds.iter().find(|c| c.contains("tar xzf")).expect("an untar command");
    assert!(untar.contains("-C '/root' --no-same-owner"), "{untar}");
    assert!(!untar.contains("--recursive-unlink"));
}

/// The archive lands byte-identical at a fresh destination; the parts are gone from the machine afterwards.
async fn lands_identical(tar: &[u8], blob: &[u8]) -> (Imported, Vec<(String, usize)>) {
    let g = guest();
    let dest = tmp("wsp-vault-dest-");
    let out = import_into(&g.machine, tar, dest.path().to_str().unwrap(), &overlay(&g)).await.unwrap();
    assert!(std::fs::read(dest.path().join("blob.bin")).unwrap() == blob, "blob.bin differs");
    g.fetch.assert_no_part_left();
    (out, g.fetch.puts())
}

// TS: packages/engine/test/vault.test.ts:225
// an archive of exactly one part goes up as one PUT under the plain name; one byte more is a second part of one byte
#[tokio::test(flavor = "multi_thread")]
async fn an_archive_of_exactly_one_part_goes_up_as_one_put_and_one_byte_more_is_a_second_part() {
    let part = UPLOAD_PART_BYTES as usize;
    let blob = random_bytes(part - 20_000);
    let exact = gzip_of_exact_length(&blob, part);
    assert_eq!(exact.len(), part);
    let (out, puts) = lands_identical(&exact, &blob).await;
    assert_eq!(out.parts, 1);
    assert_eq!(puts.iter().map(|p| (p.1, p.0.ends_with(".tgz"))).collect::<Vec<_>>(), vec![(part, true)]);

    let over = gzip_of_exact_length(&blob, part + 1);
    let (out, puts) = lands_identical(&over, &blob).await;
    assert_eq!(out.parts, 2);
    assert_eq!(
        puts.iter().map(|p| (p.1, p.0[p.0.len() - 6..].to_string())).collect::<Vec<_>>(),
        vec![(part, ".part0".to_string()), (1, ".part1".to_string())]
    );
}

// TS: packages/engine/test/vault.test.ts:239
// an empty archive is refused before any upload
#[tokio::test]
async fn an_empty_archive_is_refused_before_any_upload() {
    let stub = vault_stub();
    let err = import_into(&stub.machine, &[], "/root", &stub_opts(&stub)).await.unwrap_err();
    assert!(err.to_string().contains("empty archive"), "{err}");
    assert_eq!(stub.fetch.calls.load(Ordering::SeqCst), 0);
    assert_eq!(*stub.exec_cmds.lock().unwrap(), Vec::<String>::new());
}

// TS: packages/engine/test/vault.test.ts:247
// reports each part as it lands
#[tokio::test]
async fn reports_each_part_as_it_lands() {
    let stub = vault_stub();
    let (seen, on_part) = part_log();
    let tar = vec![0u8; UPLOAD_PART_BYTES as usize + 5];
    let o = VaultOptions { on_part: Some(on_part), ..stub_opts(&stub) };
    import_into(&stub.machine, &tar, "/root", &o).await.unwrap();
    let total = tar.len() as u64;
    assert_eq!(
        *seen.lock().unwrap(),
        vec![
            UploadProgress { part: 1, parts: 2, bytes: UPLOAD_PART_BYTES, total, pieces: None },
            UploadProgress { part: 2, parts: 2, bytes: total, total, pieces: None },
        ]
    );
}

// TS: packages/engine/test/vault.test.ts:258
// says how many pieces a part was cut into on a road that cuts them, and nothing on one that takes it whole
#[tokio::test]
async fn says_how_many_pieces_a_part_was_cut_into_on_a_road_that_cuts_them() {
    let stub = vault_stub();
    let (seen, on_part) = part_log();
    let tar = b"a small archive".to_vec();
    let by_road = FakeMachine {
        put_bytes: Some(Arc::new(|_path, _bytes| Box::pin(async { Ok(Some(BytesLanded { pieces: 7 })) }))),
        ..stub.machine.clone()
    };
    let o = VaultOptions { on_part: Some(on_part), ..stub_opts(&stub) };
    import_into(&by_road, &tar, "/root", &o).await.unwrap();
    let total = tar.len() as u64;
    assert_eq!(*seen.lock().unwrap(), vec![UploadProgress { part: 1, parts: 1, bytes: total, total, pieces: Some(7) }]);
    // The signed URL road puts the part in one call, so there is no piece count to carry.
    seen.lock().unwrap().clear();
    import_into(&stub.machine, &tar, "/root", &o).await.unwrap();
    assert_eq!(*seen.lock().unwrap(), vec![UploadProgress { part: 1, parts: 1, bytes: total, total, pieces: None }]);
}

enum Answer {
    Ok,
    Status(u16),
    Drop,
}

struct Flaky {
    answers: Mutex<VecDeque<Answer>>,
    sent: Mutex<Vec<usize>>,
    inner: Arc<StubFetch>,
}

#[async_trait(?Send)]
impl Fetch for Flaky {
    async fn fetch(&self, req: FetchRequest) -> Result<FetchResponse, RoadError> {
        let answer = self.answers.lock().unwrap().pop_front().expect("an answer left");
        self.sent.lock().unwrap().push(req.body.as_ref().map(Vec::len).unwrap_or(0));
        match answer {
            Answer::Drop => Err(fetch_failed()),
            Answer::Status(status) => Ok(response(status, r#"{"error":"Bad Gateway"}"#)),
            Answer::Ok => self.inner.fetch(req).await,
        }
    }
}

// TS: packages/engine/test/vault.test.ts:271
// a part the edge answers 502 or drops is retried with a bound; the retry is per part and the parts already up are not sent again
// Spawns no process, so the backoff runs on a paused clock.
#[tokio::test(start_paused = true)]
async fn a_part_the_edge_answers_502_or_drops_is_retried_per_part_with_a_bound() {
    let stub = vault_stub();
    let flaky = Arc::new(Flaky {
        answers: Mutex::new(VecDeque::from([Answer::Ok, Answer::Status(502), Answer::Drop, Answer::Ok])),
        sent: Mutex::new(Vec::new()),
        inner: stub.fetch.clone(),
    });
    let o = VaultOptions { fetch: Some(flaky.clone()), ..Default::default() };
    let out = import_into(&stub.machine, &vec![0u8; UPLOAD_PART_BYTES as usize + 7], "/root", &o).await.unwrap();
    assert_eq!(out.parts, 2);
    assert_eq!(*flaky.sent.lock().unwrap(), vec![UPLOAD_PART_BYTES as usize, 7, 7, 7]);
}

struct Down {
    n: AtomicUsize,
}

#[async_trait(?Send)]
impl Fetch for Down {
    async fn fetch(&self, _req: FetchRequest) -> Result<FetchResponse, RoadError> {
        self.n.fetch_add(1, Ordering::SeqCst);
        Ok(response(503, r#"{"error":"Service Unavailable"}"#))
    }
}

// TS: packages/engine/test/vault.test.ts:294
// a part that keeps failing gives up after three attempts, naming the part, the status, the body and the attempts, and removes every part path
// Spawns no process, so the backoff runs on a paused clock.
#[tokio::test(start_paused = true)]
async fn a_part_that_keeps_failing_gives_up_after_three_attempts_and_removes_every_part_path() {
    let stub = vault_stub();
    let down = Arc::new(Down { n: AtomicUsize::new(0) });
    let o = VaultOptions { fetch: Some(down.clone()), ..Default::default() };
    let err = import_into(&stub.machine, &vec![0u8; UPLOAD_PART_BYTES as usize + 7], "/root", &o).await.unwrap_err();
    assert!(err.to_string().contains("part 1 of 2: HTTP 503 Service Unavailable after 3 attempts"), "{err}");
    assert_eq!(down.n.load(Ordering::SeqCst), 3);
    let cmds = stub.exec_cmds.lock().unwrap().clone();
    assert!(cmds.iter().any(|c| c.starts_with("rm -f") && c.contains(".part0") && c.contains(".part1")), "{cmds:?}");
}

// TS: packages/engine/test/vault.test.ts:313
// a 40 MB archive goes up in two parts under the measured cap and lands byte-identical
#[tokio::test(flavor = "multi_thread")]
async fn a_40_mb_archive_goes_up_in_two_parts_under_the_measured_cap_and_lands_byte_identical() {
    let blob = random_bytes(40 * MIB);
    let tar = tar_of_blob(&blob);
    assert!(tar.len() > CAP);
    let g = guest();
    let dest = tmp("wsp-vault-dest-");
    let out = import_into(&g.machine, &tar, dest.path().to_str().unwrap(), &overlay(&g)).await.unwrap();
    assert_eq!(out.parts, 2);
    assert_eq!(g.fetch.puts().iter().map(|p| p.1).collect::<Vec<_>>(), vec![CAP, tar.len() - CAP]);
    assert!(std::fs::read(dest.path().join("blob.bin")).unwrap() == blob, "blob.bin differs");
    g.fetch.assert_no_part_left();
}

struct FirstThenRefuse {
    n: AtomicUsize,
}

#[async_trait(?Send)]
impl Fetch for FirstThenRefuse {
    async fn fetch(&self, _req: FetchRequest) -> Result<FetchResponse, RoadError> {
        if self.n.fetch_add(1, Ordering::SeqCst) == 0 {
            Ok(response(200, ""))
        } else {
            Ok(response(413, REFUSAL))
        }
    }
}

// TS: packages/engine/test/vault.test.ts:331
// a part the provider refuses names the limit it answered with, and the parts already up are removed
#[tokio::test]
async fn a_part_the_provider_refuses_names_the_limit_it_answered_with_and_the_parts_already_up_are_removed() {
    let stub = vault_stub();
    let refusing = Arc::new(FirstThenRefuse { n: AtomicUsize::new(0) });
    let o = VaultOptions { fetch: Some(refusing.clone()), ..Default::default() };
    let err = import_into(&stub.machine, &vec![0u8; 33 * MIB], "/root", &o).await.unwrap_err().to_string();
    assert!(in_order(&err, &["part 2 of 2", "HTTP 413", "33554432 bytes"]), "{err}");
    assert_eq!(refusing.n.load(Ordering::SeqCst), 2);
    let cmds = stub.exec_cmds.lock().unwrap().clone();
    assert!(cmds.iter().any(|c| c.starts_with("rm -f") && c.contains(".part0")));
    assert!(!cmds.iter().any(|c| c.contains("tar xzf")));
}

// TS: packages/engine/test/vault.test.ts:341
// a part missing on the machine is reported as cat's error, not as a hash mismatch, and the other parts are removed
#[tokio::test(flavor = "multi_thread")]
async fn a_part_missing_on_the_machine_is_reported_as_cats_error_not_as_a_hash_mismatch() {
    let tar = tar_of_blob(&random_bytes(33 * MIB));
    let g = guest_with(PartsFetch::new(false, Some(".part1")));
    let dest = tmp("wsp-vault-dest-");
    let err = import_into(&g.machine, &tar, dest.path().to_str().unwrap(), &overlay(&g)).await.unwrap_err().to_string();
    assert!(in_order(&err, &["vault import untar failed", "No such file or directory"]), "{err}");
    assert!(!err.contains("did not match its hash"), "{err}");
    assert!(!exists(&dest.path().join("blob.bin")));
    g.fetch.assert_no_part_left();
}

// TS: packages/engine/test/vault.test.ts:358
// a failed extraction leaves no part behind on the machine
#[tokio::test(flavor = "multi_thread")]
async fn a_failed_extraction_leaves_no_part_behind_on_the_machine() {
    let not_a_tar = random_bytes(33 * MIB);
    let g = guest();
    let dest = tmp("wsp-vault-dest-");
    let err = import_into(&g.machine, &not_a_tar, dest.path().to_str().unwrap(), &overlay(&g)).await.unwrap_err();
    assert!(err.to_string().contains("vault import untar failed"), "{err}");
    assert_eq!(g.fetch.puts().len(), 2);
    g.fetch.assert_no_part_left();
}

// TS: packages/engine/test/vault.test.ts:372
// an archive that does not match its hash after reassembly is refused before anything is extracted
#[tokio::test(flavor = "multi_thread")]
async fn an_archive_that_does_not_match_its_hash_after_reassembly_is_refused_before_anything_is_extracted() {
    let tar = tar_of_blob(&random_bytes(4096));
    let g = guest_with(PartsFetch::new(true, None));
    let dest = tmp("wsp-vault-dest-");
    let err = import_into(&g.machine, &tar, dest.path().to_str().unwrap(), &overlay(&g)).await.unwrap_err();
    assert!(err.to_string().contains("did not match its hash"), "{err}");
    assert!(!exists(&dest.path().join("blob.bin")));
    g.fetch.assert_no_part_left();
}

fn binary() -> Vec<u8> {
    let mut b = b"#!/bin/sh\necho hi\n".to_vec();
    b.extend(random_bytes(3000));
    b.extend([0, 0xff, 0x0a]);
    b
}

fn bundle(binary: &[u8]) -> Vec<u8> {
    tar_of(&[
        TarEntry::Dir { path: "src".to_string(), mode: 0o755 },
        TarEntry::File { path: "src/run.sh".to_string(), mode: 0o755, content: binary.to_vec() },
        TarEntry::File { path: "src/plain.txt".to_string(), mode: 0o644, content: b"plain\n".to_vec() },
    ])
    .unwrap()
}

fn names_in(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().file_name().into_string().unwrap()).collect();
    names.sort();
    names
}

// TS: packages/engine/test/vault.test.ts:437
// lands the archive at the path, parents made, with the bytes, the exec bit and the stages in order, and leaves no staging directory
#[tokio::test(flavor = "multi_thread")]
async fn lands_the_archive_at_the_path_with_the_bytes_the_exec_bit_and_the_stages_in_order() {
    let g = guest();
    let root = tmp("wsp-land-");
    let dest = root.path().join("work").join("proj");
    let stages: Arc<Mutex<Vec<&'static str>>> = Arc::new(Mutex::new(Vec::new()));
    let (on_part_log, on_landing_log) = (stages.clone(), stages.clone());
    let opts = LandOptions {
        fetch: Some(g.fetch.clone()),
        on_part: Some(Arc::new(move |_| on_part_log.lock().unwrap().push("part"))),
        on_landing: Some(Arc::new(move || on_landing_log.lock().unwrap().push("landing"))),
        ..Default::default()
    };
    let bin = binary();
    let out = land_bundle(&g.machine, &bundle(&bin), dest.to_str().unwrap(), &opts).await.unwrap();
    assert_eq!(out.parts, 1);
    assert_eq!(*stages.lock().unwrap(), vec!["part", "landing"]);
    assert!(std::fs::read(dest.join("src/run.sh")).unwrap() == bin, "run.sh differs");
    assert_eq!(std::fs::metadata(dest.join("src/run.sh")).unwrap().permissions().mode() & 0o777, 0o755);
    assert_eq!(std::fs::read_to_string(dest.join("src/plain.txt")).unwrap(), "plain\n");
    assert_eq!(names_in(&root.path().join("work")), strs(&["proj"]));
    g.fetch.assert_no_part_left();
}

// TS: packages/engine/test/vault.test.ts:457
// refuses an existing destination with kind exists before any byte goes up, and replaces it when told to
#[tokio::test(flavor = "multi_thread")]
async fn refuses_an_existing_destination_with_kind_exists_before_any_byte_goes_up_and_replaces_it_when_told_to() {
    let g = guest();
    let root = tmp("wsp-land-");
    let dest = root.path().join("proj");
    std::fs::create_dir(&dest).unwrap();
    std::fs::write(dest.join("old.txt"), "old\n").unwrap();
    let bin = binary();
    let opts = LandOptions { fetch: Some(g.fetch.clone()), ..Default::default() };
    let err = land_bundle(&g.machine, &bundle(&bin), dest.to_str().unwrap(), &opts).await.unwrap_err();
    match err {
        EngineError::Exists { message } => assert!(message.contains("import with replace"), "{message}"),
        other => panic!("expected kind exists, got {other}"),
    }
    assert_eq!(g.fetch.puts(), Vec::<(String, usize)>::new());
    assert_eq!(std::fs::read_to_string(dest.join("old.txt")).unwrap(), "old\n");
    land_bundle(&g.machine, &bundle(&bin), dest.to_str().unwrap(), &LandOptions { replace: true, ..opts }).await.unwrap();
    assert!(!exists(&dest.join("old.txt")));
    assert_eq!(std::fs::read_to_string(dest.join("src/plain.txt")).unwrap(), "plain\n");
    assert_eq!(names_in(root.path()), strs(&["proj"]));
}

// TS: packages/engine/test/vault.test.ts:477
// refuses a relative destination and the root
#[tokio::test]
async fn refuses_a_relative_destination_and_the_root() {
    let stub = vault_stub();
    let tar = bundle(&binary());
    for dest in ["work/proj", "/", "//"] {
        let err = land_bundle(&stub.machine, &tar, dest, &LandOptions::default()).await.unwrap_err();
        assert!(err.to_string().contains("absolute path"), "{dest}: {err}");
    }
}

// TS: packages/engine/test/vault.test.ts:497
// packs each text at its path with its mode, so a plain tar lands them where the upload road extracts
#[test]
fn packs_each_text_at_its_path_with_its_mode_so_a_plain_tar_lands_them_where_the_upload_road_extracts() {
    let deep = format!("etc/{}/{}/{}.md", "a".repeat(60), "b".repeat(60), "c".repeat(40));
    let file = |path: &str, mode: u32, content: &str| TarEntry::File { path: path.to_string(), mode, content: content.as_bytes().to_vec() };
    let tgz = tar_of(&[
        file("/etc/wsp/machine-context.md", 0o644, "short text\n"),
        file("/root/.hermes/skills/wsp-machine/SKILL.md", 0o600, "skill\n"),
        file(&format!("/{deep}"), 0o644, "deep\n"),
        file("/etc/wsp/empty", 0o644, ""),
    ])
    .unwrap();
    let dir = tmp("wsp-tarof-");
    tar_read(&["-xzf", "-", "-C", &s(dir.path())], &tgz);
    let listed = String::from_utf8(tar_read(&["-tzf", "-"], &tgz)).unwrap();
    assert_eq!(
        listed.trim().split('\n').collect::<Vec<_>>(),
        vec!["etc/wsp/machine-context.md", "root/.hermes/skills/wsp-machine/SKILL.md", deep.as_str(), "etc/wsp/empty"]
    );
    assert_eq!(common::read(dir.path(), "etc/wsp/machine-context.md"), "short text\n");
    assert_eq!(common::read(dir.path(), "root/.hermes/skills/wsp-machine/SKILL.md"), "skill\n");
    assert_eq!(common::read(dir.path(), &deep), "deep\n");
    assert_eq!(common::read(dir.path(), "etc/wsp/empty"), "");
    let mode = |rel: &str| std::fs::metadata(dir.path().join(rel)).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode("etc/wsp/machine-context.md"), 0o644);
    assert_eq!(mode("root/.hermes/skills/wsp-machine/SKILL.md"), 0o600);
}

// TS: packages/engine/test/vault.test.ts:515
// refuses a path a ustar header cannot hold
#[test]
fn refuses_a_path_a_ustar_header_cannot_hold() {
    let long = format!("/{}", "x".repeat(120));
    let err = tar_of(&[TarEntry::File { path: long.clone(), mode: 0o644, content: vec![] }]).unwrap_err();
    assert!(err.to_string().contains("ustar"), "{err}");
    assert!(!fits_tar(&long, None));
    assert!(fits_tar("a/b.txt", None));
    assert!(!fits_tar("a/link", Some(&"t".repeat(101))));
}

// TS: packages/engine/test/vault.test.ts:522
// carries bytes as they are, a directory of its own, a symlink as a link and the exec bit
#[test]
fn carries_bytes_as_they_are_a_directory_of_its_own_a_symlink_as_a_link_and_the_exec_bit() {
    let mut tail = b"text then binary\n".to_vec();
    tail.extend(random_bytes(1500));
    tail.extend([0x00, 0xff, 0xfe]);
    let tgz = tar_of(&[
        TarEntry::Dir { path: "proj".to_string(), mode: 0o750 },
        TarEntry::Dir { path: "proj/empty".to_string(), mode: 0o755 },
        TarEntry::File { path: "proj/bin/run".to_string(), mode: 0o755, content: tail.clone() },
        TarEntry::Link { path: "proj/link".to_string(), target: "bin/run".to_string() },
    ])
    .unwrap();
    let dir = tmp("wsp-tarof-");
    tar_read(&["-xzf", "-", "-C", &s(dir.path())], &tgz);
    let at = |rel: &str| dir.path().join(rel);
    assert!(std::fs::read(at("proj/bin/run")).unwrap() == tail, "proj/bin/run differs");
    assert_eq!(std::fs::metadata(at("proj/bin/run")).unwrap().permissions().mode() & 0o777, 0o755);
    assert!(std::fs::metadata(at("proj/empty")).unwrap().is_dir());
    assert_eq!(std::fs::metadata(at("proj/empty")).unwrap().permissions().mode() & 0o777, 0o755);
    assert!(std::fs::symlink_metadata(at("proj/link")).unwrap().file_type().is_symlink());
    assert_eq!(std::fs::read_link(at("proj/link")).unwrap().to_str().unwrap(), "bin/run");
}

fn moved(root: &Path, dest: &Path, leave: &[String]) -> Vec<String> {
    leave.iter().map(|p| p.replace(&s(root), &s(dest))).collect()
}

// TS: packages/engine/test/vault.test.ts:755
// importInto leaves the same paths out of an archive that holds them, so a vault written before the rule lands without them
#[tokio::test(flavor = "multi_thread")]
async fn import_into_leaves_the_same_paths_out_of_an_archive_that_holds_them() {
    let g = guest();
    let (root, leave) = signed_in_home();
    let dest = tmp("wsp-vault-land-");
    let tar = home_tar(root.path());
    // The overlay road, since --recursive-unlink is GNU tar's alone; the excludes stand in the same place on both.
    let o = VaultOptions { leave: moved(root.path(), dest.path(), &leave), ..overlay(&g) };
    import_into(&g.machine, &tar, dest.path().to_str().unwrap(), &o).await.unwrap();
    assert_eq!(find_all(dest.path()), strs(&KEPT));
}

// TS: packages/engine/test/vault.test.ts:774
// a replacing import keeps what the destination holds at each path it leaves, while every other folder it lands is replaced whole
#[tokio::test(flavor = "multi_thread")]
async fn a_replacing_import_keeps_what_the_destination_holds_at_each_path_it_leaves() {
    // The only tar that takes the --recursive-unlink a replacing import runs is GNU's.
    if !gnu_tar() {
        return;
    }
    let g = guest();
    let (root, leave) = signed_in_home();
    let dest = tmp("wsp-vault-keep-");
    write(dest.path(), "home/.codex/auth.json", "the replacement's own\n");
    write(dest.path(), "home/.codex/stale.toml", "replaced with its folder\n");
    write(dest.path(), "home/.aws/credentials", "the replacement's own\n");
    let tar = home_tar(root.path());
    let o = VaultOptions { leave: moved(root.path(), dest.path(), &leave), ..g.opts() };
    import_into(&g.machine, &tar, dest.path().to_str().unwrap(), &o).await.unwrap();
    let mut expected = strs(&KEPT);
    expected.extend(strs(&["home/.aws", "home/.aws/credentials", "home/.codex/auth.json"]));
    expected.sort();
    assert_eq!(find_all(dest.path()), expected);
    assert_eq!(common::read(dest.path(), "home/.codex/auth.json"), "the replacement's own\n");
    assert_eq!(common::read(dest.path(), "home/.aws/credentials"), "the replacement's own\n");
    assert_eq!(common::read(dest.path(), "home/.codex/config.toml"), "model = \"x\"\n");
}

// TS: packages/engine/test/vault.test.ts:799
// an import whose extract fails still puts back what it moved aside
#[tokio::test(flavor = "multi_thread")]
async fn an_import_whose_extract_fails_still_puts_back_what_it_moved_aside() {
    let g = guest();
    let dest = tmp("wsp-vault-keep-");
    write(dest.path(), "home/.codex/auth.json", "the replacement's own\n");
    let scratch = tmp("wsp-vault-scratch-");
    let o = VaultOptions { tmp_dir: Some(s(scratch.path())), leave: vec![s(&dest.path().join("home/.codex/auth.json"))], ..g.opts() };
    let err = import_into(&g.machine, b"not a gzip at all", dest.path().to_str().unwrap(), &o).await.unwrap_err();
    assert!(err.to_string().contains("vault import untar failed"), "{err}");
    assert_eq!(common::read(dest.path(), "home/.codex/auth.json"), "the replacement's own\n");
    assert_eq!(names_in(scratch.path()), Vec::<String>::new());
}
