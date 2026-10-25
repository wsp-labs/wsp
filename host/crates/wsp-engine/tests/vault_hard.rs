// SPDX-License-Identifier: AGPL-3.0-only
//! vault's V-rules (spec/vault_export.md V1 to V7, spec/vault_import.md V8 to V13): the exec read road, the progress a
//! download reports, a failed download over a file that was there, the upload's sleeps and final statuses, the tar
//! path spellings and tar_path. Every expected value is the TS's, from running vault.ts at a4033dc34.
mod common;

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use futures::stream::{self, StreamExt};

use common::{res, ExecFn, FakeMachine, RunFn, UrlFn};
use wsp_engine::errors::{EngineError, RoadError};
use wsp_engine::fetch::{Fetch, FetchRequest, FetchResponse, FetchStream};
use wsp_engine::machine::{ExecOptions, ExecResult, RunOptions};
use wsp_engine::vault::*;

type Calls = Arc<Mutex<Vec<String>>>;

/// A guest that answers `wc -c` and `base64` reads from the table and everything else with exit 0, and records each
/// call as `<exec|run>[<bound>] <command>`.
fn scripted(wc: &str, base64: ExecResult) -> (FakeMachine, Calls) {
    let calls: Calls = Arc::default();
    let seen = calls.clone();
    let wc = wc.to_string();
    let exec: ExecFn = Arc::new(move |cmd: String, o: ExecOptions| {
        seen.lock().unwrap().push(format!("exec[{}] {}", o.timeout_ms.map(|t| t.to_string()).unwrap_or_default(), cmd));
        let answer = if cmd.starts_with("wc -c < ") {
            res(0, &wc, "")
        } else if cmd.starts_with("base64 < ") {
            base64.clone()
        } else {
            res(0, "", "")
        };
        Box::pin(async move { Ok(answer) })
    });
    let ran = calls.clone();
    let run: RunFn = Arc::new(move |script: String, o: RunOptions| {
        ran.lock().unwrap().push(format!("run[{}] {}", o.deadline_ms, script));
        Box::pin(async { Ok(res(0, "", "")) })
    });
    let minted = calls.clone();
    let url: UrlFn = Arc::new(move |path: String| {
        minted.lock().unwrap().push(format!("download_url {path}"));
        Box::pin(async { Ok("https://signed.example/dl".to_string()) })
    });
    let mut m = FakeMachine::new(exec);
    m.run = Some(run);
    m.download_url = Some(url);
    (m, calls)
}

fn shapes(calls: &Calls) -> Vec<String> {
    calls.lock().unwrap().iter().map(|c| c.split(' ').take(2).collect::<Vec<_>>().join(" ")).collect()
}

type Seen = Arc<Mutex<Vec<DownloadProgress>>>;

fn exec_road(seen: &Seen) -> VaultOptions {
    let log = seen.clone();
    VaultOptions { read_road: ReadRoad::Exec, on_progress: Some(Arc::new(move |p| log.lock().unwrap().push(p))), ..Default::default() }
}

fn p(bytes: u64, total: u64) -> DownloadProgress {
    DownloadProgress { bytes, total }
}

/// V1: the exec road is the tar run, the size read, one base64 exec under the export's own bound, then the cleanup;
/// no URL is minted; progress is 0 of the size, then the size of the size.
#[tokio::test]
async fn the_exec_road_reads_the_size_then_one_base64_exec_and_mints_no_url() {
    let (m, calls) = scripted("5\n", res(0, "BwcH\nBwc=\n", ""));
    let seen: Seen = Arc::default();
    let bytes = export_paths(&m, &["/r/a".to_string()], &exec_road(&seen)).await.unwrap();
    assert_eq!(bytes, vec![7u8; 5]);
    assert_eq!(*seen.lock().unwrap(), vec![p(0, 5), p(5, 5)]);
    assert_eq!(shapes(&calls), ["run[120000] tar", "exec[20000] wc", "exec[120000] base64", "exec[20000] rm"]);
}

/// V2: the cap is inclusive: 16 MiB exactly is read (and an empty answer is then a short read), one byte more is
/// `VaultTooLarge` before any base64 exec, the archive still removed.
#[tokio::test]
async fn sixteen_mib_exactly_is_read_and_one_byte_more_is_refused_before_the_read() {
    let (m, calls) = scripted("16777216\n", res(0, "", ""));
    let err = export_paths(&m, &["/r/a".to_string()], &exec_road(&Seen::default())).await.unwrap_err();
    assert_eq!(err.to_string(), "vault export read failed: the machine held 16777216 bytes and 0 came back");
    assert!(shapes(&calls).contains(&"exec[120000] base64".to_string()));
    let (m, calls) = scripted("16777217\n", res(0, "", ""));
    let seen: Seen = Arc::default();
    let err = export_paths(&m, &["/r/a".to_string()], &exec_road(&seen)).await.unwrap_err();
    assert_eq!(
        err,
        EngineError::VaultTooLarge {
            message: "the archive on the machine is 16777217 bytes and this provider mints no download URL, so it comes back through one command, which is capped at 16777216".into(),
            bytes: 16777217
        }
    );
    assert_eq!(shapes(&calls), ["run[120000] tar", "exec[20000] wc", "exec[20000] rm"]);
    assert!(seen.lock().unwrap().is_empty());
}

/// V3: base64 is read as node reads it: junk characters skipped, padding optional, the URL-safe letters taken; a
/// failed read is the last 500 UTF-16 units of its stderr.
#[tokio::test]
async fn base64_is_decoded_as_leniently_as_node_and_a_failed_read_names_its_exit() {
    for (wc, said, want) in [("3\n", "BwcH!!!!\n", vec![7u8, 7, 7]), ("3\n", "-_-_", vec![0xfb, 0xff, 0xbf]), ("1\n", "Bw", vec![7u8])] {
        let (m, _) = scripted(wc, res(0, said, ""));
        assert_eq!(export_paths(&m, &["/r/a".to_string()], &exec_road(&Seen::default())).await.unwrap(), want, "{said}");
    }
    let (m, _) = scripted("3\n", res(0, "!!!!", ""));
    let err = export_paths(&m, &["/r/a".to_string()], &exec_road(&Seen::default())).await.unwrap_err();
    assert_eq!(err.to_string(), "vault export read failed: the machine held 3 bytes and 0 came back");
    let stderr = "no such file ".repeat(60);
    let (m, _) = scripted("5\n", res(3, "", &stderr));
    let err = export_paths(&m, &["/r/a".to_string()], &exec_road(&Seen::default())).await.unwrap_err();
    assert_eq!(err.to_string(), format!("vault export read failed (exit 3): {}", &stderr[stderr.len() - 500..]));
}

/// V4: the size on the guest is JS `Number(stdout)`: hex and exponent forms read, blanks around read, blank is 0;
/// anything else is the unknown-size sentence with the last 200 units of stderr.
#[tokio::test]
async fn the_guests_size_reads_as_js_number_reads_it() {
    for (said, want) in [("0x10", 16u64), ("1e3", 1000), ("  7  ", 7), ("", 0)] {
        let (m, _) = scripted(said, res(0, "", ""));
        assert_eq!(guest_file_size(&m, "/tmp/a.tgz").await.unwrap(), want, "{said:?}");
    }
    for said in ["abc", "Infinity", "12\n34\n"] {
        let exec: ExecFn = Arc::new(move |_, _| Box::pin(async move { Ok(res(0, said, "wc: odd")) }));
        let err = guest_file_size(&FakeMachine::new(exec), "/tmp/a.tgz").await.unwrap_err();
        assert_eq!(err.to_string(), "the archive's size on the machine is unknown: wc: odd", "{said:?}");
    }
}

/// A download whose body arrives as the chunks given, with the content-length header given, recording each request.
struct Chunks {
    status: u16,
    length: Option<&'static str>,
    chunks: Vec<Vec<u8>>,
    asked: Arc<Mutex<Vec<FetchRequest>>>,
}

#[async_trait]
impl Fetch for Chunks {
    async fn fetch(&self, _req: FetchRequest) -> Result<FetchResponse, RoadError> {
        unreachable!("a download streams")
    }
    async fn fetch_stream(&self, req: FetchRequest) -> Result<FetchStream, RoadError> {
        self.asked.lock().unwrap().push(req);
        let headers = self.length.map(|l| vec![("content-length".to_string(), l.to_string())]).unwrap_or_default();
        let body = stream::iter(self.chunks.clone().into_iter().map(Ok)).boxed();
        Ok(FetchStream { status: self.status, headers, body: Some(body) })
    }
}

fn chunked(status: u16, length: Option<&'static str>) -> (Arc<Chunks>, Seen, VaultOptions) {
    let fetch = Arc::new(Chunks { status, length, chunks: vec![vec![1; 3], vec![], vec![2; 4]], asked: Arc::default() });
    let seen: Seen = Arc::default();
    let log = seen.clone();
    let opts = VaultOptions { fetch: Some(fetch.clone()), on_progress: Some(Arc::new(move |p| log.lock().unwrap().push(p))), ..Default::default() };
    (fetch, seen, opts)
}

/// V5: every chunk is a progress call, an empty one too; the total is the caller's where given (0 included, which then
/// stays 0), else a content-length that reads as a number above 0 (`0x10` is 16), else the bytes so far. The request
/// is a bare GET: no header, no body, no deadline.
#[tokio::test]
async fn progress_counts_every_chunk_and_a_total_of_zero_stays_zero() {
    for (length, want) in [
        (None, vec![p(0, 0), p(3, 3), p(3, 3), p(7, 7)]),
        (Some("abc"), vec![p(0, 0), p(3, 3), p(3, 3), p(7, 7)]),
        (Some("0"), vec![p(0, 0), p(3, 3), p(3, 3), p(7, 7)]),
        (Some("7"), vec![p(0, 7), p(3, 7), p(3, 7), p(7, 7)]),
        (Some("0x10"), vec![p(0, 16), p(3, 16), p(3, 16), p(7, 16)]),
    ] {
        let (fetch, seen, opts) = chunked(200, length);
        let (m, _) = scripted("", res(0, "", ""));
        assert_eq!(download_buffer(&m, "/tmp/a.tgz", &opts, None).await.unwrap().len(), 7);
        assert_eq!(*seen.lock().unwrap(), want, "{length:?}");
        let asked = fetch.asked.lock().unwrap();
        assert_eq!((asked[0].method.as_str(), asked[0].headers.len(), asked[0].body.is_none(), asked[0].abort.is_none()), ("GET", 0, true, true));
    }
    let (_, seen, opts) = chunked(200, Some("7"));
    let (m, _) = scripted("", res(0, "", ""));
    download_buffer(&m, "/tmp/a.tgz", &opts, Some(0)).await.unwrap();
    assert_eq!(*seen.lock().unwrap(), vec![p(0, 0), p(3, 0), p(3, 0), p(7, 0)]);
    let (_, seen, opts) = chunked(301, None);
    let err = download_buffer(&m, "/tmp/a.tgz", &opts, None).await.unwrap_err();
    assert_eq!(err.to_string(), "vault export download failed: HTTP 301");
    assert!(seen.lock().unwrap().is_empty());
}

/// V6: a download into a file that is already there truncates it, and one that fails removes it, the old bytes with it.
#[tokio::test]
async fn a_failed_download_removes_the_file_that_was_there_and_a_good_one_truncates_it() {
    let dir = tempfile::tempdir().unwrap();
    let into = dir.path().join("out.tgz");
    let (m, _) = scripted("", res(0, "", ""));
    std::fs::write(&into, "OLD CONTENT OLD CONTENT OLD").unwrap();
    let (_, _, opts) = chunked(200, None);
    assert_eq!(download_file(&m, "/tmp/a.tgz", &into, &opts, None).await.unwrap(), 7);
    assert_eq!(std::fs::read(&into).unwrap(), [1, 1, 1, 2, 2, 2, 2]);
    std::fs::write(&into, "OLD CONTENT").unwrap();
    let (_, _, opts) = chunked(500, None);
    let err = download_file(&m, "/tmp/a.tgz", &into, &opts, None).await.unwrap_err();
    assert_eq!(err.to_string(), "vault export download failed: HTTP 500");
    assert!(!into.exists());
}

/// A PUT target answering the statuses given in turn, stamping the paused clock at each call.
struct Puts {
    answers: Vec<(u16, &'static str)>,
    at: Arc<Mutex<Vec<tokio::time::Instant>>>,
    asked: Arc<Mutex<Vec<FetchRequest>>>,
}

#[async_trait]
impl Fetch for Puts {
    async fn fetch(&self, req: FetchRequest) -> Result<FetchResponse, RoadError> {
        let n = {
            let mut at = self.at.lock().unwrap();
            at.push(tokio::time::Instant::now());
            at.len() - 1
        };
        self.asked.lock().unwrap().push(req);
        let (status, body) = self.answers[n.min(self.answers.len() - 1)];
        Ok(FetchResponse { status, headers: vec![], body: body.as_bytes().to_vec() })
    }
}

fn puts(answers: Vec<(u16, &'static str)>) -> Puts {
    Puts { answers, at: Arc::default(), asked: Arc::default() }
}

/// V8: a transient answer is tried three times, sleeping `backoff_ms(1)` then `backoff_ms(2)` (1000 to 1249, then 2000 to
/// 2249 ms) and never after the third; each PUT is the part as its body and nothing else.
#[tokio::test(start_paused = true)]
async fn a_transient_part_sleeps_the_backoff_between_its_three_tries_and_not_after() {
    let f = puts(vec![(502, "bad gateway")]);
    let err = put_part(&f, "https://signed.example/ul", b"part", 0, 1).await.unwrap_err();
    assert_eq!(err.to_string(), "vault import upload failed: HTTP 502 bad gateway after 3 attempts");
    let at = f.at.lock().unwrap().clone();
    assert_eq!(at.len(), 3);
    let gaps: Vec<u128> = at.windows(2).map(|w| (w[1] - w[0]).as_millis()).collect();
    assert!((1000..1250).contains(&gaps[0]) && (2000..2250).contains(&gaps[1]), "{gaps:?}");
    let asked = f.asked.lock().unwrap();
    assert!(asked.iter().all(|r| r.method == "PUT" && r.headers.is_empty() && r.abort.is_none() && r.body.as_deref() == Some(&b"part"[..])));
}

/// V9: only a transient status retries: a 502 the provider words as `Failed to snapshot sandbox` (snapshotUnavailable),
/// a 500 and a 429 are each final on the first answer.
#[tokio::test(start_paused = true)]
async fn a_502_that_names_the_snapshot_a_500_and_a_429_are_final_at_once() {
    for (status, body, want) in [
        (502, r#"{"error":"Failed to snapshot sandbox"}"#, "vault import upload failed: HTTP 502 Failed to snapshot sandbox"),
        (500, "boom", "vault import upload failed: HTTP 500 boom"),
        (429, r#"{"error":"slow down"}"#, "vault import upload failed: HTTP 429 slow down"),
    ] {
        let f = puts(vec![(status, body)]);
        assert_eq!(put_part(&f, "https://signed.example/ul", b"x", 0, 1).await.unwrap_err().to_string(), want);
        assert_eq!(f.at.lock().unwrap().len(), 1, "{status}");
    }
}

/// V10: the refusal's reason: a string `error` is taken whole (even empty, even past 300), a numeric `limit` adds its
/// clause (a string one does not), any other JSON body is its trimmed text.
#[test]
fn an_upload_refusal_reads_the_body_as_the_ts_does() {
    let e400 = "e".repeat(400);
    let whole = format!(r#"{{"error":"{e400}"}}"#);
    for (body, want) in [
        (r#"{"error":"","detail":"more words"}"#, "vault import upload failed: HTTP 400".to_string()),
        (whole.as_str(), format!("vault import upload failed: HTTP 400 {e400}")),
        (r#"{"error":"x","limit":1.5}"#, "vault import upload failed: HTTP 400 x; the upload takes at most 1.5 bytes per PUT".to_string()),
        (r#"{"error":"x","limit":0}"#, "vault import upload failed: HTTP 400 x; the upload takes at most 0 bytes per PUT".to_string()),
        (r#"{"error":"x","limit":"33"}"#, "vault import upload failed: HTTP 400 x".to_string()),
        ("[1,2]", "vault import upload failed: HTTP 400 [1,2]".to_string()),
        ("\"quoted\"", "vault import upload failed: HTTP 400 \"quoted\"".to_string()),
        ("null", "vault import upload failed: HTTP 400 null".to_string()),
        ("   ", "vault import upload failed: HTTP 400".to_string()),
    ] {
        assert_eq!(upload_failure(400, body, 0, 1, 1), want, "{body}");
    }
}

/// V11: a link's refusal names the path as given; a name that fits no header names it with its leading slashes cut;
/// the link is judged first.
#[test]
fn tar_refusals_spell_the_path_as_the_ts_does() {
    let link = TarEntry::Link { path: "/root/l".into(), target: "t".repeat(101) };
    assert_eq!(tar_of_at(&[link], 0).unwrap_err().to_string(), "/root/l links to a target too long for a ustar header");
    let long = "a".repeat(303);
    let file = TarEntry::File { path: format!("///{long}"), mode: 0o644, content: vec![] };
    assert_eq!(tar_of_at(&[file], 0).unwrap_err().to_string(), format!("{long} does not fit a ustar header"));
    let both = TarEntry::Link { path: long.clone(), target: "t".repeat(101) };
    assert_eq!(tar_of_at(&[both], 0).unwrap_err().to_string(), format!("{long} links to a target too long for a ustar header"));
}

/// V12: the cut is the rightmost slash that fits, never the first.
#[test]
fn a_long_name_is_cut_at_the_rightmost_slash_that_fits() {
    let (a, b, c) = ("a".repeat(50), "b".repeat(50), "c".repeat(50));
    assert_eq!(split_name(&format!("{a}/{b}/{c}/d")), Some((format!("{a}/{b}/{c}"), "d".to_string())));
    let e = "é".repeat(51);
    assert_eq!(split_name(&format!("{e}/z")), Some((e.clone(), "z".to_string())));
}

/// V13: tar_path is node's posix `path.relative`: the root itself is empty, a path outside climbs with `..`, trailing
/// and doubled slashes and `.`/`..` segments are normalised away.
#[test]
fn tar_path_is_posix_path_relative() {
    for (root, path, want) in [
        ("/root", "/root/.claude/projects", ".claude/projects"),
        ("/root", "/root", ""),
        ("/root", "/etc/x", "../etc/x"),
        ("/root", "/rootx/y", "../rootx/y"),
        ("/root/", "/root/a/", "a"),
        ("/root/", "/root//b", "b"),
        ("/root", "/root/a/../b", "b"),
        ("/root", "/root/./c", "c"),
    ] {
        assert_eq!(tar_path(root, path), want, "{root} {path}");
    }
}
