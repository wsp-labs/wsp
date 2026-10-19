// SPDX-License-Identifier: AGPL-3.0-only
//! The proof a lock is the owner's own host before the token beside the state file leaves for it, as the desktop app
//! reads a lock (`desktop:host-lifecycle.ts` `lockedHost`): the lock's process is this login's, and for a host bound
//! on loopback the page on the lock's port is a wsp page carrying the digest of that very token. The page carries the
//! digest and this holds the file, so the compare sends nothing and a page a squatter serves learns nothing by being
//! read. A host bound beyond loopback serves a page with no digest by design, and the lock alone is the reading for it.

use std::path::Path;
use std::time::Duration;

use serde_json::Value;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::aim::{self, Lock};
use crate::failure::Failure;
use crate::record;
use crate::release::ReleaseRule;

/// The kind of a refusal of a lock whose port gave no answer at all, which is a host still starting as often as a
/// stuck one, and which a caller may wait on.
pub const NOT_ANSWERING: &str = "not-answering";

/// The kind of every other refusal of a live lock: another login's process, no wsp page, another token's digest, or
/// a wsp from before pages named their release.
pub const LOCK_REFUSED: &str = "lock-refused";

/// The most of an answer the page read takes: a host's page is under a kilobyte, and anything past this is no page.
const PAGE_CAP: u64 = 1 << 20;

/// The page's one inline script around the boot object, as `BOOT_SCRIPT` in packages/protocol finds it.
const BOOT_OPEN: &str = "<script>window.__WSP__ = ";
const BOOT_CLOSE: &str = "</script>";

/// What signal 0 says of a pid: a process of this login's, a process of another login's, or none.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Signalled {
    Own,
    Another,
    Gone,
}

/// What signal 0 to a pid answered, as its errno where it failed, read as `signalled` in
/// packages/host/src/host-lock.ts reads it: EPERM is a live process this login may not signal.
pub fn signalled_by(errno: Option<i32>) -> Signalled {
    match errno {
        None => Signalled::Own,
        Some(libc::EPERM) => Signalled::Another,
        Some(_) => Signalled::Gone,
    }
}

pub fn signalled(pid: i32) -> Signalled {
    // SAFETY: signal 0 checks the pid and delivers nothing.
    let sent = unsafe { libc::kill(pid, 0) };
    signalled_by((sent != 0).then(|| std::io::Error::last_os_error().raw_os_error().unwrap_or(0)))
}

/// Whose words a refusal of a lock is said in, the caller's as its rule names it: the command line's and the tool
/// server's, or the desktop app's (`desktop:host-lifecycle.ts`). What this binary is called, and what to do once
/// the process is stopped.
fn said_by(rule: ReleaseRule) -> (&'static str, &'static str) {
    match rule {
        ReleaseRule::Own => ("this line's", "run this line again"),
        ReleaseRule::OwnOrNewer => ("this app's", "open wsp again"),
    }
}

/// One sentence for every live lock the token is not sent to, whichever reason it is (`wontAttach`).
fn wont_attach(state: &Path, lock: &Lock, why: &str, kind: &str, rule: ReleaseRule) -> Failure {
    let path = state.with_file_name(&record::host().files.lock);
    let (_, again) = said_by(rule);
    Failure::of_kind(
        format!(
            "a host (pid {}) holds {} on port {} but {why}: stop that process or run wsp down, then {again}",
            lock.pid,
            path.display(),
            lock.port
        ),
        kind,
    )
}

/// The lock's process is this login's: one that answers EPERM is another login's on a number a dead host once had.
pub fn own_login(state: &Path, lock: &Lock, rule: ReleaseRule) -> Result<(), Failure> {
    own_login_by(state, lock, signalled(lock.pid), rule)
}

fn own_login_by(state: &Path, lock: &Lock, signalled: Signalled, rule: ReleaseRule) -> Result<(), Failure> {
    match signalled {
        Signalled::Own => Ok(()),
        _ => Err(wont_attach(state, lock, "that process is not this login's", LOCK_REFUSED, rule)),
    }
}

/// The page on the lock's port read within `window`: a wsp page naming its release, and on loopback one carrying the
/// digest of `token` where it is the token file's.
pub async fn page_carries(state: &Path, lock: &Lock, token: Option<&str>, window: Duration, rule: ReleaseRule) -> Result<(), Failure> {
    let at = aim::authority(&aim::dial_address(lock), lock.port);
    let page = page_at(&at, window).await;
    let script = page.as_deref().and_then(boot_script);
    let boot =
        script.and_then(|line| serde_json::from_str::<Value>(line).ok()).filter(|boot| boot.get("wsPath").is_some_and(Value::is_string));
    // A boot line this release does not read, or one naming no release, is a wsp from before pages named theirs.
    if script.is_some() && boot.as_ref().and_then(|boot| boot.get("version")).is_none() {
        let (whose, again) = said_by(rule);
        return Err(Failure::of_kind(
            format!(
                "a wsp older than {whose} {} (pid {}) serves {} on port {}, and its page names no release: stop that process, then {again}",
                record::server().version,
                lock.pid,
                state.display(),
                lock.port
            ),
            LOCK_REFUSED,
        ));
    }
    if !aim::is_loopback(lock.address.as_deref().unwrap_or(&record::host().loopback)) {
        return Ok(());
    }
    if page.is_none() {
        return Err(wont_attach(state, lock, "no wsp host answers there", NOT_ANSWERING, rule));
    }
    let Some(boot) = boot else {
        return Err(wont_attach(state, lock, "no wsp host answers there", LOCK_REFUSED, rule));
    };
    let Some(token) = token else { return Ok(()) };
    match boot.get("tokenHash").and_then(Value::as_str) {
        Some(digest) if digest_matches(token, digest) => Ok(()),
        _ => Err(wont_attach(
            state,
            lock,
            "the page it serves carries another token's digest than the file beside this state",
            LOCK_REFUSED,
            rule,
        )),
    }
}

/// Whether a digest is the sha256 of the token, hex, compared in a time that does not say where the two first
/// differ (`hostTokenMatches`).
fn digest_matches(token: &str, digest: &str) -> bool {
    let held: String = ring::digest::digest(&ring::digest::SHA256, token.as_bytes()).as_ref().iter().map(|b| format!("{b:02x}")).collect();
    held.len() == digest.len() && bool::from(subtle::ConstantTimeEq::ct_eq(held.as_bytes(), digest.as_bytes()))
}

/// The page served at this authority: its body where it answered with a success, empty where it answered otherwise,
/// none where nothing answered as a web server within the window (`pageAt`). An answer past the cap is no page.
async fn page_at(at: &str, window: Duration) -> Option<String> {
    let asked = async {
        let mut tcp = TcpStream::connect(at).await.ok()?;
        tcp.write_all(format!("GET / HTTP/1.1\r\nHost: {at}\r\nConnection: close\r\n\r\n").as_bytes()).await.ok()?;
        let (mut bytes, mut chunk) = (Vec::new(), [0u8; 16 * 1024]);
        // The answer ends where its own length says, so a socket kept open past it (a proxy, keep-alive) holds nothing up.
        loop {
            match tcp.read(&mut chunk).await {
                Ok(0) => break,
                Ok(n) => bytes.extend_from_slice(&chunk[..n]),
                // A server that closes on a request it never read resets the socket after its answer: what came is it.
                Err(_) if !bytes.is_empty() => break,
                Err(_) => return None,
            }
            if bytes.len() as u64 > PAGE_CAP || whole_answer(&bytes) {
                break;
            }
        }
        Some(bytes)
    };
    let bytes = tokio::time::timeout(window, asked).await.ok()??;
    if bytes.len() as u64 > PAGE_CAP {
        return Some(String::new());
    }
    let text = String::from_utf8_lossy(&bytes);
    let (head, body) = text.split_once("\r\n\r\n")?;
    let status: u16 = head.strip_prefix("HTTP/1.")?.get(2..5)?.parse().ok()?;
    if !(200..300).contains(&status) {
        return Some(String::new());
    }
    let chunked = head.lines().any(|line| {
        line.split_once(':').is_some_and(|(name, value)| {
            name.trim().eq_ignore_ascii_case("transfer-encoding") && value.trim().eq_ignore_ascii_case("chunked")
        })
    });
    Some(if chunked { dechunked(body) } else { body.to_owned() })
}

/// Whether these bytes hold a whole HTTP answer: its head, then the body its Content-Length names or a chunked body
/// through its last chunk.
fn whole_answer(bytes: &[u8]) -> bool {
    let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") else { return false };
    let (head, body) = (String::from_utf8_lossy(&bytes[..end]), &bytes[end + 4..]);
    let header = |name: &str| {
        head.lines()
            .find_map(|line| line.split_once(':').filter(|(n, _)| n.trim().eq_ignore_ascii_case(name)).map(|(_, v)| v.trim().to_owned()))
    };
    if header("transfer-encoding").is_some_and(|v| v.eq_ignore_ascii_case("chunked")) {
        return chunked_end(body);
    }
    header("content-length").and_then(|v| v.parse::<usize>().ok()).is_some_and(|length| body.len() >= length)
}

/// Whether a chunked body has come through its zero-sized last chunk and the blank line after it.
fn chunked_end(mut body: &[u8]) -> bool {
    loop {
        let Some(line) = body.windows(2).position(|w| w == b"\r\n") else { return false };
        let size = String::from_utf8_lossy(&body[..line]);
        let Ok(size) = usize::from_str_radix(size.split(';').next().unwrap_or("").trim(), 16) else { return false };
        let rest = &body[line + 2..];
        if size == 0 {
            return rest.windows(2).any(|w| w == b"\r\n");
        }
        let Some(next) = rest.get(size + 2..) else { return false };
        body = next;
    }
}

/// A chunked body joined back into one, as far as it reads.
fn dechunked(mut body: &str) -> String {
    let mut whole = String::new();
    while let Some((size, rest)) = body.split_once("\r\n") {
        let Ok(size) = usize::from_str_radix(size.split(';').next().unwrap_or("").trim(), 16) else { break };
        if size == 0 || rest.len() < size {
            break;
        }
        whole.push_str(&rest[..size]);
        body = rest[size..].strip_prefix("\r\n").unwrap_or(&rest[size..]);
    }
    whole
}

/// The boot object's text in a page, where the page carries the one script a wsp host writes.
fn boot_script(page: &str) -> Option<&str> {
    page.match_indices(BOOT_OPEN).find_map(|(at, _)| {
        let rest = &page[at + BOOT_OPEN.len()..];
        let end = rest.find('<').unwrap_or(rest.len());
        rest[..end].strip_suffix(';').filter(|_| rest[end..].starts_with(BOOT_CLOSE))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lock() -> Lock {
        serde_json::from_str(r#"{"pid":42,"port":4000,"startedAt":"x"}"#).unwrap()
    }

    /// Catches: EPERM read as a process of this login's, as `pid_alive` reads it, or a dead pid read as live.
    #[test]
    fn reads_a_signal_as_the_host_lock_does() {
        assert_eq!(signalled_by(None), Signalled::Own);
        assert_eq!(signalled_by(Some(libc::EPERM)), Signalled::Another);
        assert_eq!(signalled_by(Some(libc::ESRCH)), Signalled::Gone);
        assert_eq!(signalled(std::process::id() as i32), Signalled::Own);
    }

    /// Catches: signal 0 from another login read as this login's process or as none. Root alone can become another
    /// login, so elsewhere this proves nothing and passes.
    #[test]
    fn this_process_is_another_logins_to_another_login() {
        // SAFETY: geteuid reads, fork's child calls only setuid, kill and _exit before it ends, and the parent reaps it.
        unsafe {
            if libc::geteuid() != 0 {
                return;
            }
            let me = std::process::id() as i32;
            let child = libc::fork();
            if child == 0 {
                let code = if libc::setuid(65534) != 0 {
                    3
                } else {
                    match signalled(me) {
                        Signalled::Another => 0,
                        Signalled::Own => 1,
                        Signalled::Gone => 2,
                    }
                };
                libc::_exit(code);
            }
            let mut status = 0;
            libc::waitpid(child, &mut status, 0);
            assert_eq!(libc::WEXITSTATUS(status), 0, "1 is read as this login's, 2 as gone, 3 could not become another login");
        }
    }

    /// Catches: a lock held by another login's process let through to the page and the token, or refused in the
    /// desktop app's words to the command line or in the command line's to the desktop.
    #[test]
    fn another_logins_process_is_refused_in_the_callers_words() {
        let state = Path::new("/s/state.json");
        assert!(own_login_by(state, &lock(), Signalled::Own, ReleaseRule::Own).is_ok());
        let refused = own_login_by(state, &lock(), Signalled::Another, ReleaseRule::Own).unwrap_err();
        assert_eq!(refused.kind.as_deref(), Some(LOCK_REFUSED));
        assert_eq!(
            refused.message,
            "a host (pid 42) holds /s/host.lock on port 4000 but that process is not this login's: stop that process or run wsp down, then run this line again"
        );
        let refused = own_login_by(state, &lock(), Signalled::Another, ReleaseRule::OwnOrNewer).unwrap_err();
        assert_eq!(
            refused.message,
            "a host (pid 42) holds /s/host.lock on port 4000 but that process is not this login's: stop that process or run wsp down, then open wsp again"
        );
    }

    #[test]
    fn finds_the_boot_line_as_the_protocol_does() {
        let line = r#"<html><script>window.__WSP__ = {"wsPath":"/ws"};</script></html>"#;
        assert_eq!(boot_script(line), Some(r#"{"wsPath":"/ws"}"#));
        assert_eq!(boot_script("<script>window.__WSP__ = {\"a\":\"<\"};</script>"), None);
        assert_eq!(boot_script("<p>nothing</p>"), None);
        assert_eq!(boot_script("<script>window.__WSP__ = x<b>;</script><script>window.__WSP__ = {};</script>"), Some("{}"));
    }

    /// Catches: a digest compared to the token itself, or to anything but its sha256 in lowercase hex.
    #[test]
    fn a_digest_is_the_sha256_of_the_token_in_hex() {
        // echo -n abc | sha256sum
        assert!(digest_matches("abc", "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"));
        assert!(!digest_matches("abc", "abc"));
        assert!(!digest_matches("abd", "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"));
    }

    #[test]
    fn joins_a_chunked_body() {
        assert_eq!(dechunked("5\r\nhello\r\n6\r\n world\r\n0\r\n\r\n"), "hello world");
    }

    /// Catches: an answer held to the socket's close, or one read as whole before its body came.
    #[test]
    fn an_answer_ends_where_its_own_length_says() {
        let chunked = "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nhello\r\n";
        assert!(!whole_answer(chunked.as_bytes()));
        assert!(whole_answer(format!("{chunked}0\r\n\r\n").as_bytes()));
        let sized = "HTTP/1.1 200 OK\r\ncontent-length: 5\r\n\r\nhell";
        assert!(!whole_answer(sized.as_bytes()));
        assert!(whole_answer(format!("{sized}o").as_bytes()));
        assert!(!whole_answer(b"HTTP/1.1 200 OK\r\n"));
    }

    /// Catches: the page read waiting for a close that a proxy in front of the host never sends after the answer.
    #[test]
    fn a_page_on_a_socket_kept_open_is_read_at_once() {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let at = listener.local_addr().unwrap().to_string();
        std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            let mut asked = [0u8; 1024];
            _ = socket.read(&mut asked);
            _ = socket.write_all(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n4\r\npage\r\n0\r\n\r\n");
            std::thread::sleep(Duration::from_secs(10));
        });
        let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        let from = std::time::Instant::now();
        let page = runtime.block_on(page_at(&at, Duration::from_secs(5)));
        assert_eq!(page.as_deref(), Some("page"));
        assert!(from.elapsed() < Duration::from_secs(2), "the read waited {:?} for the socket to close", from.elapsed());
    }
}
