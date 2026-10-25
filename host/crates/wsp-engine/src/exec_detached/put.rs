// SPDX-License-Identifier: AGPL-3.0-only
//! exec-detached.ts:1-227 at a4033dc34. Spec: spec/exec_put.md.
//!
//! The constants every guest road shares, many short reads in as few execs as fit, and the one way bytes go onto a
//! guest through exec.
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use indexmap::IndexMap;
use wsp_wire::types::shell_quote::shell_quote;
use wsp_wire::wire::limits::EXEC_BODY_MAX;
use wsp_words::js::js_trim;

use crate::errors::{EngineError, Result};
use crate::machine::{ExecOptions, ExecResult, Framing, Machine};

/// exec-detached.ts:18. The longest one plain exec may take.
pub const INLINE_EXEC_MS: u64 = 20_000;
/// exec-detached.ts:21. What a run past its deadline exits with.
pub const DEADLINE_EXIT: i64 = wsp_wire::wire::daemon::EXEC_DEADLINE_EXIT;
/// exec-detached.ts:23. What a run answers when its caller cancelled it.
pub const CANCELLED_EXIT: i64 = 130;
/// exec-detached.ts:28.
pub const GUEST_TMP: &str = "/tmp";
/// exec-detached.ts:32.
pub const RUN_DIR: &str = "/tmp/wsp-run";

/// exec-detached.ts:37. What a guest prints back to say a step landed.
pub struct Handshake {
    pub piece: &'static str,
    pub launched: &'static str,
    pub written: &'static str,
    pub gone: &'static str,
    pub run: &'static str,
}

/// exec-detached.ts:37.
pub const HANDSHAKE: Handshake =
    Handshake { piece: "WSP_PIECE", launched: "WSP_LAUNCHED", written: "WSP_OK", gone: "WSP_GONE", run: "WSP_RUN" };

/// exec-detached.ts:42. What the machine said, for a sentence that would otherwise end in a colon.
pub fn machine_answer(res: &ExecResult) -> String {
    said_by(res.exit_code.get(), &res.stdout, &res.stderr)
}

/// [`machine_answer`] over the parts of an answer, for a caller that holds an upload's result.
pub(crate) fn said_by(exit_code: i64, stdout: &str, stderr: &str) -> String {
    let said = [stdout, stderr].map(js_trim).into_iter().filter(|text| !text.is_empty()).collect::<Vec<_>>().join(" ");
    if said.is_empty() {
        format!("it printed nothing and exited {exit_code}")
    } else {
        format!("it exited {exit_code} and said: {said}")
    }
}

/// exec-detached.ts:50.
pub const DAEMON_ENV_FILE: &str = "/etc/wsp/daemon.env";

/// exec-detached.ts:53. Room left under the cap for what a backend wraps around the command on the wire.
pub const EXEC_ENVELOPE_BYTES: usize = 512;

/// exec-detached.ts:61. Whether one command fits an exec body, the backend's wrapper counted.
pub fn exec_fits(command: &str) -> bool {
    command.len() + EXEC_ENVELOPE_BYTES <= EXEC_BODY_MAX as usize
}

/// exec-detached.ts:68.
pub const READS_PER_EXEC: usize = 8;

/// exec-detached.ts:71. One page of reads and what the exec that carried it answered.
#[derive(Debug, Clone, PartialEq)]
pub struct ReadPage<T> {
    pub rows: Vec<T>,
    pub res: ExecResult,
}

fn page_command(head: &str, lines: &[String]) -> String {
    std::iter::once(head).chain(lines.iter().map(String::as_str)).collect::<Vec<_>>().join("\n")
}

/// Sends the page's exec and answers it with its rows; a page is emptied whether or not its exec answers.
async fn send_page<T: Clone>(machine: &dyn Machine, head: &str, page: &mut Vec<(T, String)>, pages: &mut Vec<ReadPage<T>>) -> Result<()> {
    if page.is_empty() {
        return Ok(());
    }
    let sending = std::mem::take(page);
    let lines: Vec<String> = sending.iter().map(|(_, line)| line.clone()).collect();
    let cmd = page_command(head, &lines);
    let res = machine.exec(&cmd, ExecOptions { timeout_ms: Some(INLINE_EXEC_MS), ..Default::default() }).await?;
    pages.push(ReadPage { rows: sending.into_iter().map(|(row, _)| row).collect(), res });
    Ok(())
}

/// exec-detached.ts:82. Many short reads in as few execs as they fit in; `line` gets the row's place in its page.
pub async fn paged_reads<T: Clone>(
    machine: &dyn Machine,
    rows: &[T],
    line: impl Fn(&T, usize) -> String,
    head: &str,
) -> Result<Vec<ReadPage<T>>> {
    let mut pages = Vec::new();
    let mut page: Vec<(T, String)> = Vec::new();
    for row in rows {
        let next = line(row, page.len());
        if !page.is_empty() {
            let mut lines: Vec<String> = page.iter().map(|(_, l)| l.clone()).collect();
            lines.push(next.clone());
            if page.len() >= READS_PER_EXEC || !exec_fits(&page_command(head, &lines)) {
                send_page(machine, head, &mut page, &mut pages).await?;
                page.push((row.clone(), line(row, 0)));
                continue;
            }
        }
        page.push((row.clone(), next));
    }
    send_page(machine, head, &mut page, &mut pages).await?;
    Ok(pages)
}

/// exec-detached.ts:105. What one page printed, by the place in that page the marker names, in first-seen order.
pub fn markers_of(stdout: &str, marker: &str) -> IndexMap<String, String> {
    let mut found = IndexMap::new();
    for line in stdout.split('\n') {
        let words: Vec<&str> = js_trim(line).split(' ').collect();
        if words[0] == marker && words.len() > 1 {
            found.insert(words[1].to_string(), words[2..].join(" "));
        }
    }
    found
}

/// exec-detached.ts:132.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuestWrite {
    /// Where on the guest; its directory is made if missing.
    pub path: String,
    pub text: String,
    /// Add to the file's end instead of replacing it.
    pub append: bool,
}

/// exec-detached.ts:140.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PutFilesOptions {
    /// Lines the last exec runs before any file lands.
    pub before: Vec<String>,
    /// Lines the last exec runs once every file is on disk.
    pub after: Vec<String>,
    pub timeout_ms: Option<u64>,
    /// Bytes for the last exec's own input.
    pub stdin: Option<Vec<u8>>,
}

/// exec-detached.ts:154.
pub fn append_marker(path: &str) -> String {
    format!("{path}.appended")
}

/// exec-detached.ts:159.
pub const OLD_APPEND_MARKS: &str = ".a[0-9a-f]*";

/// Node's `path.posix.dirname`: `.` for a bare name, the root for a root's child, and the path up to its last
/// separator that is followed by a name, repeated separators kept inside it.
fn posix_dirname(path: &str) -> String {
    let bytes = path.as_bytes();
    if bytes.is_empty() {
        return ".".into();
    }
    let has_root = bytes[0] == b'/';
    let mut end: Option<usize> = None;
    let mut matched_slash = true;
    for i in (1..bytes.len()).rev() {
        if bytes[i] == b'/' {
            if !matched_slash {
                end = Some(i);
                break;
            }
        } else {
            matched_slash = false;
        }
    }
    match end {
        None => (if has_root { "/" } else { "." }).into(),
        Some(1) if has_root => "//".into(),
        Some(end) => path[..end].to_string(),
    }
}

/// One file of an upload as it is planned: its base64, how many numbered pieces it is cut into, its append marker and
/// the key its append carries.
struct Planned<'a> {
    file: &'a GuestWrite,
    b64: String,
    pieces: usize,
    mark: String,
    key: String,
}

/// The lines that land one planned file.
fn land(f: &Planned) -> Vec<String> {
    let path = shell_quote(&f.file.path);
    let names = format!("{path}.{{0..{}}}", f.pieces as i64 - 1);
    let source = if f.pieces == 0 { format!("printf %s {}", shell_quote(&f.b64)) } else { format!("cat {names}") };
    let redirect = if f.file.append { ">>" } else { ">" };
    let decode = format!("{source} | base64 -d {redirect} {path}");
    let write = if f.file.append {
        let (mark, key) = (shell_quote(&f.mark), shell_quote(&f.key));
        format!("[ \"$(cat {mark} 2>/dev/null)\" = {key} ] || {{ {decode} && printf %s {key} > {mark}; }} || exit 1")
    } else {
        format!("{decode} || exit 1")
    };
    if f.pieces == 0 {
        vec![write]
    } else {
        vec![write, format!("rm -f {names}")]
    }
}

/// One numbered piece of a file: written whole to its own name, so a retried exec lands it once.
fn piece(head: &str, path: &str, i: usize, part: &str) -> String {
    [head, "umask 077", &format!("printf %s {} > {}.{i} || exit 1", shell_quote(part), shell_quote(path)), "echo WSP_PIECE"].join("\n")
}

/// exec-detached.ts:169. The execs that put the files on the guest under upload id `upload`, every body under the cap
/// as `framing` wraps it: the pieces, then the last exec. Refuses before anything is sent.
pub fn upload_sequence(
    files: &[GuestWrite],
    before: &[String],
    after: &[String],
    upload: &str,
    framing: Option<&dyn Framing>,
) -> Result<Vec<String>> {
    let framed = |cmd: &str| framing.map_or_else(|| cmd.to_string(), |f| f.wrap(cmd));
    let refusal = |bare: &str| {
        let message = match framing.and_then(|f| f.no_room()) {
            Some(room) if exec_fits(bare) => room,
            _ => "the lines around the upload do not fit one exec body".to_string(),
        };
        EngineError::Other { message, code: None }
    };
    let mut folders: Vec<String> = Vec::new();
    for f in files {
        let folder = posix_dirname(&f.path);
        if !folders.contains(&folder) {
            folders.push(folder);
        }
    }
    let head = format!("mkdir -p {}", folders.iter().map(|d| shell_quote(d)).collect::<Vec<_>>().join(" "));
    let mut plan: Vec<Planned> = files
        .iter()
        .enumerate()
        .map(|(at, f)| Planned {
            file: f,
            b64: STANDARD.encode(f.text.as_bytes()),
            pieces: 0,
            mark: append_marker(&f.path),
            key: format!("{upload}.{at}"),
        })
        .collect();
    let last = |plan: &[Planned]| -> String {
        let mut lines = vec![head.clone()];
        lines.extend(before.iter().cloned());
        lines.push("set -o pipefail".into());
        lines.extend(plan.iter().flat_map(land));
        lines.extend(after.iter().cloned());
        lines.join("\n")
    };
    let mut execs: Vec<String> = Vec::new();
    while !exec_fits(&framed(&last(&plan))) {
        let mut pick: Option<usize> = None;
        for (at, f) in plan.iter().enumerate() {
            if f.pieces == 0 && pick.is_none_or(|p| f.b64.len() > plan[p].b64.len()) {
                pick = Some(at);
            }
        }
        let Some(at) = pick else { return Err(refusal(&last(&plan))) };
        let (path, b64_len) = (plan[at].file.path.clone(), plan[at].b64.len());
        // Base64 decodes in groups of four, so a piece boundary on a multiple of four keeps the joined text decodable.
        let room = EXEC_BODY_MAX - EXEC_ENVELOPE_BYTES as i64 - framed(&piece(&head, &path, b64_len, "")).len() as i64;
        let size = room.div_euclid(4) * 4;
        if size < 4 {
            return Err(refusal(&piece(&head, &path, b64_len, "AAAA")));
        }
        let size = size as usize;
        let pieces = b64_len.div_ceil(size).max(1);
        plan[at].pieces = pieces;
        for i in 0..pieces {
            let b64 = &plan[at].b64;
            let part = &b64[(i * size).min(b64.len())..((i + 1) * size).min(b64.len())];
            execs.push(piece(&head, &path, i, part));
        }
    }
    execs.push(last(&plan));
    Ok(execs)
}

/// exec-detached.ts:204. What one upload took: the last exec's answer and the pieces cut to fit an exec body.
#[derive(Debug, Clone, PartialEq)]
pub struct UploadResult {
    pub exit_code: i64,
    pub stdout: String,
    pub stderr: String,
    pub pieces: u32,
}

/// exec-detached.ts:215. Sends the upload's execs in order, each piece confirmed before the next goes.
pub async fn put_files(machine: &dyn Machine, files: &[GuestWrite], opts: PutFilesOptions) -> Result<UploadResult> {
    let timeout_ms = opts.timeout_ms.unwrap_or(INLINE_EXEC_MS);
    let upload = hex::encode(rand::random::<[u8; 6]>());
    let framing = if machine.offers().framed { Some(machine.framed().await?) } else { None };
    let execs = upload_sequence(files, &opts.before, &opts.after, &upload, framing.as_deref())?;
    let (last, pieces) = execs.split_last().expect("an upload always ends in its last exec");
    for (at, cmd) in pieces.iter().enumerate() {
        let key = format!("{upload}/{at}");
        let res = machine.exec(cmd, ExecOptions { timeout_ms: Some(timeout_ms), idempotency_key: Some(key), stdin: None }).await?;
        if res.exit_code.get() != 0 || !res.stdout.contains(HANDSHAKE.piece) {
            return Err(EngineError::Other {
                message: format!(
                    "a piece did not land on {}: nothing came back saying {}, the word the guest prints once a piece is written; {}",
                    machine.id(),
                    HANDSHAKE.piece,
                    machine_answer(&res)
                ),
                code: None,
            });
        }
    }
    let key = format!("{upload}/last");
    let answer = machine.exec(last, ExecOptions { timeout_ms: Some(timeout_ms), idempotency_key: Some(key), stdin: opts.stdin }).await?;
    Ok(UploadResult { exit_code: answer.exit_code.get(), stdout: answer.stdout, stderr: answer.stderr, pieces: pieces.len() as u32 })
}
