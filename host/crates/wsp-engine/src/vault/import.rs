// SPDX-License-Identifier: AGPL-3.0-only
//! vault.ts:289-539 at a4033dc34: a tar onto a machine. Spec: spec/vault_import.md.
use std::io::Write;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use flate2::write::GzEncoder;
use flate2::Compression;
use serde_json::Value;
use sha2::{Digest, Sha256};
use wsp_wire::types::shell_quote::shell_quote;
use wsp_words::js::{js_trim, number_string, utf16};

use super::export::{base36_fraction, guest_tmp_path, leave_flags, tar_path};
use super::{UploadProgress, VaultOptions};
use crate::errors::{backoff_ms, classify, should_retry, EngineError, RealRetryClock, Result, RetryClock};
use crate::exec_detached::put::INLINE_EXEC_MS;
use crate::fetch::{platform_fetch, Fetch, FetchRequest};
use crate::land_bytes::{has_byte_road, land_bytes, LandBytesOptions};
use crate::machine::{ExecOptions, Machine, RunOptions};

/// vault.ts:291. The signed upload URL takes one PUT of at most 32 MiB.
pub const UPLOAD_PART_BYTES: u64 = 32 * 1024 * 1024;

fn plain(message: String) -> EngineError {
    EngineError::Other { message, code: None }
}

fn last_units(text: &str, n: i64) -> String {
    utf16::slice(text, -n, None)
}

/// vault.ts:304 `parseBody`: the body as JSON, nothing where it does not parse.
fn parse_body(body: &str) -> Option<Value> {
    serde_json::from_str(body).ok()
}

/// vault.ts:304 `uploadFailure`: the sentence for a refused part. `where` is empty for a one-part archive, else ` on
/// part <part> of <parts>`; the reason is the body's JSON `error` where it is a string, else the trimmed body's first
/// 300 UTF-16 units; ` after <attempts> attempts` only past one attempt; `; the upload takes at most <limit> bytes per
/// PUT` only where the body's `limit` is a number.
pub fn upload_failure(status: u16, body: &str, part: u32, parts: u32, attempts: u32) -> String {
    let at = if parts == 1 { String::new() } else { format!(" on part {part} of {parts}") };
    let parsed = parse_body(body);
    let reason = match parsed.as_ref().and_then(|p| p.get("error")).and_then(Value::as_str) {
        Some(error) => error.to_string(),
        None => utf16::cut(js_trim(body), 300),
    };
    let limit = match parsed.as_ref().and_then(|p| p.get("limit")).and_then(Value::as_f64) {
        Some(limit) => format!("; the upload takes at most {} bytes per PUT", number_string(limit)),
        None => String::new(),
    };
    let tries = if attempts > 1 { format!(" after {attempts} attempts") } else { String::new() };
    let reason = if reason.is_empty() { String::new() } else { format!(" {reason}") };
    format!("vault import upload failed{at}: HTTP {status}{reason}{tries}{limit}")
}

async fn pause(attempt: u32) {
    let clock = RealRetryClock;
    clock.sleep(backoff_ms(attempt, clock.random())).await;
}

/// vault.ts:315 `putPart`: one part PUT until it lands, at most three attempts, sleeping
/// `crate::errors::backoff_ms(attempt, random)` between them.
pub async fn put_part(fetch: &dyn Fetch, url: &str, bytes: &[u8], part: u32, parts: u32) -> Result<()> {
    let at = if parts == 1 { String::new() } else { format!(" on part {part} of {parts}") };
    let mut attempt: u32 = 1;
    loop {
        let req = FetchRequest { method: "PUT".to_string(), url: url.to_string(), headers: Vec::new(), body: Some(bytes.to_vec()), abort: None };
        let res = match fetch.fetch(req).await {
            Ok(res) => res,
            Err(e) => {
                if attempt >= 3 {
                    return Err(plain(format!("vault import upload failed{at}: {} after {attempt} attempts", EngineError::Road(e))));
                }
                pause(attempt).await;
                attempt += 1;
                continue;
            }
        };
        if res.ok() {
            return Ok(());
        }
        let body = res.text();
        let error = parse_body(&body).and_then(|p| p.get("error").and_then(Value::as_str).map(str::to_string));
        let kind = classify(res.status, None, error.as_deref(), None);
        if !should_retry(&kind, attempt) {
            return Err(plain(upload_failure(res.status, &body, part, parts, attempt)));
        }
        pause(attempt).await;
        attempt += 1;
    }
}

/// vault.ts:335 `TarEntry`: a file, a directory of its own, or a symbolic link carried as one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TarEntry {
    File { path: String, mode: u32, content: Vec<u8> },
    Dir { path: String, mode: u32 },
    Link { path: String, target: String },
}

/// vault.ts:355. The ustar size field is eleven octal digits.
pub const TAR_MAX_FILE_BYTES: u64 = 0o77777777777;
/// vault.ts:356.
pub const TAR_LINK_MAX: usize = 100;
const TAR_BLOCK: usize = 512;

/// vault.ts:363 `splitName`: the path whole when it is at most 100 bytes; else the last slash (right to left, never
/// at index 0) that leaves a prefix of at most 155 bytes and a name of at most 100. `None` when no cut fits. Answers
/// `(prefix, name)`.
pub fn split_name(path: &str) -> Option<(String, String)> {
    if path.len() <= 100 {
        return Some((String::new(), path.to_string()));
    }
    path.rmatch_indices('/').map(|(cut, _)| cut).filter(|&cut| cut > 0).find_map(|cut| {
        let (prefix, name) = (&path[..cut], &path[cut + 1..]);
        (prefix.len() <= 155 && name.len() <= 100).then(|| (prefix.to_string(), name.to_string()))
    })
}

/// vault.ts:380 `fitsTar`: leading slashes cut, the path splits, and the target (where given) is at most 100 bytes.
pub fn fits_tar(path: &str, target: Option<&str>) -> bool {
    split_name(path.trim_start_matches('/')).is_some() && target.is_none_or(|t| t.len() <= TAR_LINK_MAX)
}

fn put(header: &mut [u8], at: usize, len: usize, value: &[u8]) {
    let n = value.len().min(len);
    header[at..at + n].copy_from_slice(&value[..n]);
}

/// vault.ts:386 `tarOf` with its clock handed in: the ustar blocks of the entries, each header's mtime `mtime_secs`,
/// two zero blocks after, then gzipped. The gzip framing is flate2's; the tar inside is byte for byte the TS's.
pub fn tar_of_at(entries: &[TarEntry], mtime_secs: u64) -> Result<Vec<u8>> {
    let mut blocks: Vec<u8> = Vec::new();
    for e in entries {
        let (path, mode, body, target): (&str, u32, &[u8], Option<&str>) = match e {
            TarEntry::File { path, mode, content } => (path, *mode, content, None),
            TarEntry::Dir { path, mode } => (path, *mode, &[], None),
            TarEntry::Link { path, target } => (path, 0o777, &[], Some(target)),
        };
        if body.len() as u64 > TAR_MAX_FILE_BYTES {
            return Err(plain(format!("{path} is {} bytes, over what a ustar header holds", body.len())));
        }
        if target.is_some_and(|t| t.len() > TAR_LINK_MAX) {
            return Err(plain(format!("{path} links to a target too long for a ustar header")));
        }
        let stripped = path.trim_start_matches('/');
        let (prefix, name) = split_name(stripped).ok_or_else(|| plain(format!("{stripped} does not fit a ustar header")))?;
        let mut header = vec![0u8; TAR_BLOCK];
        put(&mut header, 0, 100, name.as_bytes());
        put(&mut header, 100, 8, format!("{mode:07o}\0").as_bytes());
        put(&mut header, 108, 8, b"0000000\0");
        put(&mut header, 116, 8, b"0000000\0");
        put(&mut header, 124, 12, format!("{:011o}\0", body.len()).as_bytes());
        put(&mut header, 136, 12, format!("{mtime_secs:011o}\0").as_bytes());
        put(&mut header, 148, 8, b"        ");
        let kind: &[u8] = match (target, matches!(e, TarEntry::Dir { .. })) {
            (Some(_), _) => b"2",
            (None, true) => b"5",
            (None, false) => b"0",
        };
        put(&mut header, 156, 1, kind);
        if let Some(t) = target {
            put(&mut header, 157, TAR_LINK_MAX, t.as_bytes());
        }
        put(&mut header, 257, 6, b"ustar\0");
        put(&mut header, 263, 2, b"00");
        put(&mut header, 265, 32, b"root");
        put(&mut header, 297, 32, b"root");
        put(&mut header, 345, 155, prefix.as_bytes());
        let sum: u32 = header.iter().map(|&b| u32::from(b)).sum();
        put(&mut header, 148, 8, format!("{sum:06o}\0 ").as_bytes());
        blocks.extend_from_slice(&header);
        blocks.extend_from_slice(body);
        let pad = (TAR_BLOCK - body.len() % TAR_BLOCK) % TAR_BLOCK;
        blocks.extend(std::iter::repeat_n(0u8, pad));
    }
    blocks.extend(std::iter::repeat_n(0u8, TAR_BLOCK * 2));
    let mut gz = GzEncoder::new(Vec::new(), Compression::default());
    gz.write_all(&blocks).map_err(|e| plain(e.to_string()))?;
    gz.finish().map_err(|e| plain(e.to_string()))
}

/// vault.ts:386 `tarOf`: [`tar_of_at`] at the current second (`floor(now_ms / 1000)`).
pub fn tar_of(entries: &[TarEntry]) -> Result<Vec<u8>> {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    tar_of_at(entries, now)
}

/// vault.ts:422. A hash mismatch on the guest exits 65.
pub const HASH_MISMATCH_EXIT: i64 = 65;

/// vault.ts:424 `importInto`'s answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Imported {
    pub parts: u32,
}

/// The paths of the parts quoted and joined, for a command line.
fn quoted_join(paths: &[String]) -> String {
    paths.iter().map(|p| shell_quote(p)).collect::<Vec<_>>().join(" ")
}

/// vault.ts:424 `importInto`.
pub async fn import_into(machine: &dyn Machine, tar: &[u8], dest_dir: &str, opts: &VaultOptions) -> Result<Imported> {
    if tar.is_empty() {
        return Err(plain("vault import: empty archive, nothing to import".to_string()));
    }
    let fetch = opts.fetch.clone().unwrap_or_else(platform_fetch);
    let tmp = format!("{}.tgz", guest_tmp_path("wsp-vault-in", opts.tmp_dir.as_deref()));
    let len = tar.len() as u64;
    let parts = len.div_ceil(UPLOAD_PART_BYTES) as u32;
    let part_paths: Vec<String> = if parts == 1 { vec![tmp] } else { (0..parts).map(|i| format!("{tmp}.part{i}")).collect() };
    let uploaded: Result<()> = async {
        for (i, path) in part_paths.iter().enumerate() {
            let start = i as u64 * UPLOAD_PART_BYTES;
            let end = ((i as u64 + 1) * UPLOAD_PART_BYTES).min(len);
            let part = &tar[start as usize..end as usize];
            let mut pieces = None;
            if has_byte_road(machine) {
                pieces = land_bytes(machine, path, part, LandBytesOptions::default()).await?.map(|landed| landed.pieces);
            } else {
                let url = machine.upload_url(path).await?;
                put_part(fetch.as_ref(), &url, part, i as u32 + 1, parts).await?;
            }
            if let Some(on_part) = &opts.on_part {
                on_part(UploadProgress { part: i as u32 + 1, parts, bytes: end, total: len, pieces });
            }
        }
        Ok(())
    }
    .await;
    if let Err(e) = uploaded {
        let _ = machine.exec(&format!("rm -f {}", quoted_join(&part_paths)), ExecOptions { timeout_ms: Some(INLINE_EXEC_MS), ..Default::default() }).await;
        return Err(e);
    }
    let digest = hex::encode(Sha256::digest(tar));
    let flags = if opts.overlay { "--no-same-owner" } else { "--recursive-unlink" };
    let joined = format!("cat {}", quoted_join(&part_paths));
    let remove_parts = format!("rm -f {}", quoted_join(&part_paths));
    let kept = kept_aside(dest_dir, &opts.leave, &guest_tmp_path("wsp-vault-keep", opts.tmp_dir.as_deref()), &remove_parts);
    let mut lines: Vec<String> = vec!["set -eo pipefail".to_string()];
    match &kept {
        None => lines.push(format!("trap \"{remove_parts}\" EXIT")),
        Some(kept) => {
            lines.push(kept.r#where.clone());
            lines.push(kept.back.clone());
            lines.push("trap back EXIT".to_string());
        }
    }
    lines.push(format!("sum=$({joined} | sha256sum | cut -d' ' -f1)"));
    lines.push(format!("test \"$sum\" = {digest} || exit {HASH_MISMATCH_EXIT}"));
    lines.push(format!("mkdir -p {}", shell_quote(dest_dir)));
    if let Some(kept) = &kept {
        lines.push(kept.aside.clone());
    }
    lines.push(format!("{joined} | tar xzf -{} -C {} {flags}", leave_flags(dest_dir, &opts.leave), shell_quote(dest_dir)));
    let script = lines.join("\n");
    let untar = machine.run(&script, RunOptions { deadline_ms: opts.timeout_ms.unwrap_or(120_000), ..Default::default() }).await?;
    if untar.exit_code.get() == HASH_MISMATCH_EXIT {
        let noun = if parts == 1 { "part" } else { "parts" };
        return Err(plain(format!("vault import: the uploaded archive ({len} bytes in {parts} {noun}) did not match its hash on the machine")));
    }
    if untar.exit_code.get() != 0 {
        return Err(plain(format!("vault import untar failed (exit {}): {}", untar.exit_code.get(), last_units(&untar.stderr, 500))));
    }
    Ok(Imported { parts })
}

/// vault.ts:478 `keptAside`'s three texts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeptAside {
    pub r#where: String,
    pub aside: String,
    /// The `back` function's definition with this `rm -f` of the parts inside it.
    pub back: String,
}

/// vault.ts:478 `keptAside(destDir, leave, keep)` with `back` already given `remove_parts`. `None` when nothing is
/// left.
pub fn kept_aside(dest_dir: &str, leave: &[String], keep: &str, remove_parts: &str) -> Option<KeptAside> {
    if leave.is_empty() {
        return None;
    }
    let rels = leave.iter().map(|p| shell_quote(&tar_path(dest_dir, p))).collect::<Vec<_>>().join(" ");
    let held = |at: &str| format!("{{ [ -e \"{at}\" ] || [ -L \"{at}\" ]; }}");
    Some(KeptAside {
        r#where: format!("d={}; k={}", shell_quote(dest_dir), shell_quote(keep)),
        aside: format!(
            "for r in {rels}; do if {}; then mkdir -p \"$k/$(dirname \"$r\")\" && mv \"$d/$r\" \"$k/$r\"; fi; done",
            held("$d/$r")
        ),
        back: format!(
            "back() {{ for r in {rels}; do if {}; then mkdir -p \"$d/$(dirname \"$r\")\" && rm -rf \"$d/$r\" && mv \"$k/$r\" \"$d/$r\"; fi; done; rm -rf \"$k\"; {remove_parts}; }}",
            held("$k/$r")
        ),
    })
}

/// vault.ts:489.
#[derive(Clone, Default)]
pub struct LandOptions {
    pub fetch: Option<Arc<dyn Fetch>>,
    pub timeout_ms: Option<u64>,
    pub tmp_dir: Option<String>,
    /// Remove what is at the destination first; without it an existing path is refused with `Exists`.
    pub replace: bool,
    pub on_part: Option<Arc<dyn Fn(UploadProgress)>>,
    /// Called once the archive is extracted, right before it is moved into place.
    pub on_landing: Option<Arc<dyn Fn()>>,
}

/// vault.ts:503. The landing refuses an existing path with exit 66.
pub const EXISTS_EXIT: i64 = 66;

/// vault.ts:505 `exists`: `EngineError::Exists` whose message is `<path> already exists on the machine; import with
/// replace to overwrite it`.
pub fn exists_on_machine(path: &str) -> EngineError {
    EngineError::Exists { message: format!("{path} already exists on the machine; import with replace to overwrite it") }
}

/// vault.ts:510 `landBundle`.
pub async fn land_bundle(machine: &dyn Machine, tar: &[u8], dest: &str, opts: &LandOptions) -> Result<Imported> {
    let target = dest.trim_end_matches('/');
    if !dest.starts_with('/') || target.is_empty() {
        return Err(plain(format!("destination must be an absolute path below /, got {dest}")));
    }
    let probe = machine.exec(&format!("test -e {} && echo yes || echo no", shell_quote(target)), ExecOptions { timeout_ms: Some(INLINE_EXEC_MS), ..Default::default() }).await?;
    if probe.exit_code.get() != 0 {
        return Err(plain(format!("could not look at {target} on the machine: {}", last_units(&probe.stderr, 200))));
    }
    if js_trim(&probe.stdout) == "yes" && !opts.replace {
        return Err(exists_on_machine(target));
    }
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
    let staging = format!("{target}.wsp-in-{now}-{}", base36_fraction(rand::random::<f64>(), 6));
    let import = VaultOptions {
        overlay: true,
        tmp_dir: opts.tmp_dir.clone(),
        fetch: opts.fetch.clone(),
        timeout_ms: opts.timeout_ms,
        on_part: opts.on_part.clone(),
        ..Default::default()
    };
    let imported = match import_into(machine, tar, &staging, &import).await {
        Ok(imported) => imported,
        Err(e) => {
            let _ = machine.exec(&format!("rm -rf {}", shell_quote(&staging)), ExecOptions { timeout_ms: Some(INLINE_EXEC_MS), ..Default::default() }).await;
            return Err(e);
        }
    };
    if let Some(on_landing) = &opts.on_landing {
        on_landing();
    }
    let parent = match target.rfind('/') {
        Some(at) if at > 0 => &target[..at],
        _ => "/",
    };
    let clear = if opts.replace {
        format!("rm -rf {}", shell_quote(target))
    } else {
        format!("test ! -e {} || exit {EXISTS_EXIT}", shell_quote(target))
    };
    let script = [
        "set -e".to_string(),
        format!("trap \"rm -rf {}\" EXIT", shell_quote(&staging)),
        format!("mkdir -p {}", shell_quote(parent)),
        clear,
        format!("mv {} {}", shell_quote(&staging), shell_quote(target)),
    ]
    .join("\n");
    let moved = machine.run(&script, RunOptions { deadline_ms: opts.timeout_ms.unwrap_or(120_000), ..Default::default() }).await?;
    if moved.exit_code.get() == EXISTS_EXIT {
        return Err(exists_on_machine(target));
    }
    if moved.exit_code.get() != 0 {
        return Err(plain(format!("landing at {target} failed (exit {}): {}", moved.exit_code.get(), last_units(&moved.stderr, 500))));
    }
    Ok(imported)
}
