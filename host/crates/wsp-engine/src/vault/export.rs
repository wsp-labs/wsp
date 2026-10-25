// SPDX-License-Identifier: AGPL-3.0-only
//! vault.ts:45-287 at a4033dc34: the archive off a machine. Spec: spec/vault_export.md.
use std::future::Future;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use futures::StreamExt;
use tokio::io::{AsyncWrite, AsyncWriteExt};
use wsp_wire::types::shell_quote::shell_quote;
use wsp_words::js::{is_js_whitespace, js_trim, to_number, utf16};
use wsp_words::words::base::plural;
use wsp_words::words::thread::vault_over_cap_line;

use super::{ArchiveGroup, CacheRule, DownloadProgress, ReadRoad, VaultOptions};
use crate::errors::{EngineError, Result};
use crate::exec_detached::put::{GUEST_TMP, INLINE_EXEC_MS};
use crate::fetch::{platform_fetch, FetchRequest};
use crate::machine::{ExecOptions, Machine, RunOptions};

/// vault.ts:47. The most an archive read back through exec may weigh: 16 MiB.
pub const EXEC_READ_CAP: u64 = 16 * 1024 * 1024;

fn plain(message: String) -> EngineError {
    EngineError::Other { message, code: None }
}

fn io(e: std::io::Error) -> EngineError {
    plain(e.to_string())
}

fn last_units(text: &str, n: i64) -> String {
    utf16::slice(text, -n, None)
}

fn report(opts: &VaultOptions, progress: DownloadProgress) {
    if let Some(on_progress) = &opts.on_progress {
        on_progress(progress);
    }
}

/// vault.ts:70 `download`: the archive at `path` on the guest into `sink` as it arrives. `ReadRoad::Exec` goes to
/// [`exec_read`]. `total` is the size read on the guest when the caller has it.
pub async fn download(
    machine: &dyn Machine,
    path: &str,
    sink: &mut (dyn AsyncWrite + Unpin),
    opts: &VaultOptions,
    total: Option<u64>,
) -> Result<()> {
    if opts.read_road == ReadRoad::Exec {
        return exec_read(machine, path, sink, opts, total).await;
    }
    let fetch = opts.fetch.clone().unwrap_or_else(platform_fetch);
    let url = machine.download_url(path).await?;
    let res = fetch
        .fetch_stream(FetchRequest { method: "GET".to_string(), url, headers: Vec::new(), body: None, abort: None })
        .await
        .map_err(EngineError::Road)?;
    if !res.ok() {
        return Err(plain(format!("vault export download failed: HTTP {}", res.status)));
    }
    let length = res.header("content-length").map_or(0.0, to_number);
    let known = total.or_else(|| (length.is_finite() && length > 0.0).then_some(length as u64));
    let Some(mut body) = res.body else {
        return Err(plain(format!("vault export download failed: HTTP {} with no body", res.status)));
    };
    report(opts, DownloadProgress { bytes: 0, total: known.unwrap_or(0) });
    let mut bytes: u64 = 0;
    while let Some(chunk) = body.next().await {
        let chunk = chunk.map_err(EngineError::Road)?;
        sink.write_all(&chunk).await.map_err(io)?;
        sink.flush().await.map_err(io)?;
        bytes += chunk.len() as u64;
        report(opts, DownloadProgress { bytes, total: known.unwrap_or(bytes) });
    }
    Ok(())
}

/// vault.ts:92 `execRead`: the archive as base64 on exec's stdout, refused over [`EXEC_READ_CAP`] before any of it is
/// read.
pub async fn exec_read(
    machine: &dyn Machine,
    path: &str,
    sink: &mut (dyn AsyncWrite + Unpin),
    opts: &VaultOptions,
    total: Option<u64>,
) -> Result<()> {
    let bytes = match total {
        Some(total) => total,
        None => guest_file_size(machine, path).await?,
    };
    if bytes > EXEC_READ_CAP {
        return Err(EngineError::VaultTooLarge {
            message: format!(
                "the archive on the machine is {bytes} bytes and this provider mints no download URL, so it comes back through one command, which is capped at {EXEC_READ_CAP}"
            ),
            bytes,
        });
    }
    report(opts, DownloadProgress { bytes: 0, total: bytes });
    let read = machine
        .exec(
            &format!("base64 < {}", shell_quote(path)),
            ExecOptions { timeout_ms: Some(opts.timeout_ms.unwrap_or(120_000)), ..Default::default() },
        )
        .await?;
    if read.exit_code.get() != 0 {
        return Err(plain(format!("vault export read failed (exit {}): {}", read.exit_code.get(), last_units(&read.stderr, 500))));
    }
    let squeezed: String = read.stdout.chars().filter(|c| !is_js_whitespace(*c)).collect();
    let body = crate::node_base64::node_base64_decode(&squeezed);
    if body.len() as u64 != bytes {
        return Err(plain(format!("vault export read failed: the machine held {bytes} bytes and {} came back", body.len())));
    }
    sink.write_all(&body).await.map_err(io)?;
    report(opts, DownloadProgress { bytes: body.len() as u64, total: bytes });
    Ok(())
}

/// vault.ts:107 `downloadBuffer`: a download held in memory.
pub async fn download_buffer(machine: &dyn Machine, path: &str, opts: &VaultOptions, total: Option<u64>) -> Result<Vec<u8>> {
    let mut chunks: Vec<u8> = Vec::new();
    download(machine, path, &mut chunks, opts, total).await?;
    Ok(chunks)
}

/// vault.ts:115 `downloadFile`: a download streamed into a file on this computer; a download that fails leaves no
/// file behind. Answers the bytes the file holds (its size on disk once closed).
pub async fn download_file(machine: &dyn Machine, path: &str, into: &Path, opts: &VaultOptions, total: Option<u64>) -> Result<u64> {
    let mut out = tokio::fs::File::create(into).await.map_err(io)?;
    if let Err(e) = download(machine, path, &mut out, opts, total).await {
        drop(out);
        let _ = tokio::fs::remove_file(into).await;
        return Err(e);
    }
    out.flush().await.map_err(io)?;
    drop(out);
    Ok(tokio::fs::metadata(into).await.map_err(io)?.len())
}

/// vault.ts:145 `excludingArchiveScript`: the script that archives `targets`, relative to `cwd`, into `out` with
/// the rule's caches left behind and named on stdout. Byte for byte the TS's text (tests/fixtures/oracle).
pub fn excluding_archive_script(cwd: &str, targets: &[String], rule: &CacheRule, out: &str, leave: &[String]) -> String {
    let names = |list: &[String]| list.iter().map(|n| format!("-name {}", shell_quote(n))).collect::<Vec<_>>().join(" -o ");
    let mut named: Vec<String> = Vec::new();
    if !rule.dirs.is_empty() {
        named.push(format!("\\( -type d \\( {} \\) \\)", names(&rule.dirs)));
    }
    if !rule.files.is_empty() {
        named.push(format!("\\( -type f \\( {} \\) \\)", names(&rule.files)));
    }
    named.extend(rule.markers.iter().map(|m| format!("\\( -type d -exec test -f {} \\; \\)", shell_quote(&format!("{{}}/{m}")))));
    let cache = format!("\\( {} \\)", if named.is_empty() { "-false".to_string() } else { named.join(" -o ") });
    let where_ = targets.iter().map(|t| shell_quote(t)).collect::<Vec<_>>().join(" ");
    let list = format!("{out}.list");
    let keep = format!("{out}.keep");
    [
        "set -eo pipefail".to_string(),
        format!("cd {}", shell_quote(cwd)),
        format!("find {where_} -mindepth 1 -path '*/.git' -prune -o {cache} -prune -print > {}", shell_quote(&list)),
        format!("printf '%s\\0' {} > {}", targets.iter().map(|t| shell_quote(t)).collect::<Vec<_>>().join(" "), shell_quote(&keep)),
        format!(
            "find {where_} -mindepth 1 \\( -path '*/.git' -o -path '*/.git/*' \\) -print0 -o {cache} -prune -o -print0 >> {}",
            shell_quote(&keep)
        ),
        format!("tar czf {}{} --no-recursion --null -T {}", shell_quote(out), leave_flags(cwd, leave), shell_quote(&keep)),
        format!("sed 's|^\\./||' {}", shell_quote(&list)),
        format!("rm -f {} {}", shell_quote(&list), shell_quote(&keep)),
    ]
    .join("\n")
}

/// vault.ts:169.
pub fn folder_export_script(dir: &str, rule: &CacheRule, out: &str) -> String {
    excluding_archive_script(dir, &[".".to_string()], rule, out, &[])
}

/// vault.ts:177 `guestTmpPath(what, dir = GUEST_TMP)`: `<dir with trailing slashes cut>/<what>-<ms since epoch>-<six
/// base-36 chars>`.
pub fn guest_tmp_path(what: &str, dir: Option<&str>) -> String {
    let dir = dir.unwrap_or(GUEST_TMP).trim_end_matches('/');
    let ms = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
    format!("{dir}/{what}-{ms}-{}", base36_fraction(rand::random::<f64>(), 6))
}

/// The first `digits` base-36 digits of a fraction in [0, 1).
pub(super) fn base36_fraction(mut x: f64, digits: usize) -> String {
    const ALPHABET: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let mut out = String::new();
    for _ in 0..digits {
        x *= 36.0;
        let d = (x.floor() as usize).min(35);
        out.push(ALPHABET[d] as char);
        x -= d as f64;
    }
    out
}

/// vault.ts:189 `tarPath`: a path as tar names it from its group's root. Root `/` cuts one leading slash and keeps
/// a relative path as it is; any other root is `path.relative(root, path)` (posix, lexical, no filesystem read).
pub fn tar_path(root: &str, path: &str) -> String {
    if root == "/" {
        return path.strip_prefix('/').unwrap_or(path).to_string();
    }
    relative(root, path)
}

/// Node's `path.resolve(from)` and `path.resolve(to)` joined as `path.relative` reads them, with no filesystem read.
fn relative(from: &str, to: &str) -> String {
    let from = normalized(from);
    let to = normalized(to);
    let common = from.iter().zip(&to).take_while(|(a, b)| a == b).count();
    let mut parts: Vec<&str> = vec![".."; from.len() - common];
    parts.extend(to[common..].iter().map(String::as_str));
    parts.join("/")
}

/// The absolute, normalised segments of a path: relative input resolves against this process's folder.
fn normalized(path: &str) -> Vec<String> {
    let absolute = if path.starts_with('/') {
        path.to_string()
    } else {
        format!("{}/{path}", std::env::current_dir().map(|d| d.to_string_lossy().into_owned()).unwrap_or_default())
    };
    let mut out: Vec<String> = Vec::new();
    for seg in absolute.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                out.pop();
            }
            other => out.push(other.to_string()),
        }
    }
    out
}

/// vault.ts:193 `leaveFlags`: ` --exclude=<quoted tar_path(root, p)>` per left path, each with its leading space.
pub fn leave_flags(root: &str, leave: &[String]) -> String {
    leave.iter().map(|p| format!(" --exclude={}", shell_quote(&tar_path(root, p)))).collect()
}

/// vault.ts:196 `groupArchiveScript`.
pub fn group_archive_script(groups: &[ArchiveGroup], out: &str, leave: &[String]) -> String {
    let members = groups
        .iter()
        .map(|g| {
            format!(
                "-C {} {}",
                shell_quote(&g.root),
                g.paths.iter().map(|p| shell_quote(&tar_path(&g.root, p))).collect::<Vec<_>>().join(" ")
            )
        })
        .collect::<Vec<_>>()
        .join(" ");
    format!("tar czf {}{} {members}", shell_quote(out), leave_flags("/", leave))
}

/// vault.ts:200 `guestFileSize`: `wc -c < <path>` inline, read as JS `Number(stdout.trim())`: an empty stdout is 0,
/// `0x10` is 16. A non-zero exit or a non-finite number fails.
pub async fn guest_file_size(machine: &dyn Machine, path: &str) -> Result<u64> {
    let size = machine
        .exec(&format!("wc -c < {}", shell_quote(path)), ExecOptions { timeout_ms: Some(INLINE_EXEC_MS), ..Default::default() })
        .await?;
    let bytes = to_number(js_trim(&size.stdout));
    if size.exit_code.get() != 0 || !bytes.is_finite() {
        return Err(plain(format!("the archive's size on the machine is unknown: {}", last_units(&size.stderr, 200))));
    }
    Ok(bytes as u64)
}

/// vault.ts:235 `keptPaths`: the paths whose last name (trailing slashes cut) is in neither `rule.dirs` nor
/// `rule.files`, in order; refuses when none is kept.
pub fn kept_paths(paths: &[String], rule: &CacheRule) -> Result<Vec<String>> {
    let kept: Vec<String> = paths
        .iter()
        .filter(|p| {
            let name = p.trim_end_matches('/').rsplit('/').next().unwrap_or_default();
            !rule.dirs.iter().any(|d| d == name) && !rule.files.iter().any(|f| f == name)
        })
        .cloned()
        .collect();
    if kept.is_empty() {
        return Err(plain(format!(
            "vault export: every path named is a cache under the rule ({}), so there is nothing to archive",
            paths.join(", ")
        )));
    }
    Ok(kept)
}

/// vault.ts:248 `emptyArchiveScript`.
pub fn empty_archive_script(out: &str) -> String {
    format!("tar czf {} --no-recursion --null -T /dev/null", shell_quote(out))
}

/// The archive's remove line: the archive and its two working lists.
fn cleanup_script(tmp: &str) -> String {
    format!("rm -f {} {} {}", shell_quote(tmp), shell_quote(&format!("{tmp}.list")), shell_quote(&format!("{tmp}.keep")))
}

async fn cleanup(machine: &dyn Machine, tmp: &str) {
    let _ = machine.exec(&cleanup_script(tmp), ExecOptions { timeout_ms: Some(INLINE_EXEC_MS), ..Default::default() }).await;
}

/// vault.ts:209 `archiveOf`: runs `script` (already built against `tmp`) on the guest, refuses an archive over the
/// cap, brings it with `bring`, and removes it from the guest whatever happens after the tar ran.
async fn archive_of<T, Fut>(machine: &dyn Machine, tmp: &str, script: &str, opts: &VaultOptions, bring: impl FnOnce() -> Fut) -> Result<T>
where
    Fut: Future<Output = Result<T>>,
{
    let tar = machine.run(script, RunOptions { deadline_ms: opts.timeout_ms.unwrap_or(120_000), ..Default::default() }).await?;
    if tar.exit_code.get() != 0 {
        return Err(plain(format!("vault export tar failed (exit {}): {}", tar.exit_code.get(), last_units(&tar.stderr, 500))));
    }
    let outcome = async {
        if let Some(max) = opts.max_bytes {
            let bytes = guest_file_size(machine, tmp).await?;
            if bytes > max {
                return Err(EngineError::VaultTooLarge { message: vault_over_cap_line(bytes as f64, max as f64), bytes });
            }
        }
        bring().await
    }
    .await;
    cleanup(machine, tmp).await;
    outcome
}

/// vault.ts:251 `exportPaths`: the paths archived from `/` and read back into memory.
pub async fn export_paths(machine: &dyn Machine, paths: &[String], opts: &VaultOptions) -> Result<Vec<u8>> {
    let tmp = format!("{}.tgz", guest_tmp_path("wsp-vault", None));
    let script = if paths.is_empty() {
        empty_archive_script(&tmp)
    } else {
        match &opts.exclude {
            None => group_archive_script(&[ArchiveGroup { root: "/".to_string(), paths: paths.to_vec() }], &tmp, &opts.leave),
            Some(rule) => {
                let kept: Vec<String> = kept_paths(paths, rule)?.iter().map(|p| p.strip_prefix('/').unwrap_or(p).to_string()).collect();
                excluding_archive_script("/", &kept, rule, &tmp, &opts.leave)
            }
        }
    };
    archive_of(machine, &tmp, &script, opts, || download_buffer(machine, &tmp, opts, None)).await
}

/// vault.ts:265 `exportPathsInto`: the groups into a file on this computer. Answers the bytes written.
pub async fn export_paths_into(machine: &dyn Machine, groups: &[ArchiveGroup], into: &Path, opts: &VaultOptions) -> Result<u64> {
    let tmp = format!("{}.tgz", guest_tmp_path("wsp-vault", None));
    let script = group_archive_script(groups, &tmp, &[]);
    archive_of(machine, &tmp, &script, opts, || download_file(machine, &tmp, into, opts, None)).await
}

/// vault.ts:270 `destExists`: `EngineError::Exists` whose message is `<dest> already exists on this computer with
/// <plural(files, "file")>; export with replace to overwrite it`.
pub fn dest_exists(dest: &str, files: u64) -> EngineError {
    EngineError::Exists {
        message: format!(
            "{dest} already exists on this computer with {}; export with replace to overwrite it",
            plural(files as f64, "file")
        ),
    }
}

/// vault.ts:275 `exportFolder`'s answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FolderExport {
    pub bytes: u64,
    /// The cache roots left behind, as find spelled them less a leading `./`, sorted by UTF-16 code unit.
    pub excluded: Vec<String>,
}

/// vault.ts:275 `exportFolder`.
pub async fn export_folder(machine: &dyn Machine, dir: &str, rule: &CacheRule, into: &Path, opts: &VaultOptions) -> Result<FolderExport> {
    let probe = machine
        .exec(
            &format!("test -d {} && echo yes || echo no", shell_quote(dir)),
            ExecOptions { timeout_ms: Some(INLINE_EXEC_MS), ..Default::default() },
        )
        .await?;
    if probe.exit_code.get() != 0 || js_trim(&probe.stdout) != "yes" {
        return Err(plain(format!("{dir} is not a folder on the machine")));
    }
    let tmp = format!("{}.tgz", guest_tmp_path("wsp-out", None));
    let outcome = async {
        let packed = machine
            .run(
                &folder_export_script(dir, rule, &tmp),
                RunOptions { deadline_ms: opts.timeout_ms.unwrap_or(600_000), ..Default::default() },
            )
            .await?;
        if packed.exit_code.get() != 0 {
            return Err(plain(format!(
                "packing {dir} on the machine failed (exit {}): {}",
                packed.exit_code.get(),
                last_units(&packed.stderr, 500)
            )));
        }
        let mut excluded: Vec<String> = packed.stdout.split('\n').filter(|l| !l.is_empty()).map(str::to_string).collect();
        excluded.sort_by(|a, b| utf16::cmp(a, b));
        let size = guest_file_size(machine, &tmp).await?;
        let bytes = download_file(machine, &tmp, into, opts, Some(size)).await?;
        Ok(FolderExport { bytes, excluded })
    }
    .await;
    cleanup(machine, &tmp).await;
    outcome
}
