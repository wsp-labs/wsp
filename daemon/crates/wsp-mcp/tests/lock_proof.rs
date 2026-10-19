// SPDX-License-Identifier: AGPL-3.0-only
//! The token beside the state file leaves only for the owner's own host: whatever listens on the lock's port is read
//! first, and one that serves no wsp page, or a wsp page carrying another token's digest, is refused in the caller's
//! words and never sees the token. A port that answers nothing is told apart, as a host still starting.

mod common;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;
use wsp_mcp::host::Host;
use wsp_mcp::{Args, Env, ReleaseRule, LOCK_REFUSED, NOT_ANSWERING};

const TOKEN: &str = "the-owners-token";

/// The desktop app's words, which a refusal said to the command line or the tool server never carries.
const APP_WORDS: [&str; 2] = ["this app", "open wsp again"];

/// What a listener on the lock's port was sent: every frame of every socket it upgraded, and whether any asked.
#[derive(Default)]
struct Heard {
    frames: Vec<String>,
    upgrades: usize,
}

/// A listener on loopback that answers a plain request with `page` and upgrades any socket, taking whatever token
/// it is handed and noting every frame.
async fn squatter(page: String) -> (u16, Arc<Mutex<Heard>>) {
    squatter_after(page, Duration::ZERO).await
}

/// The same, answering a plain request only after `delay`.
async fn squatter_after(page: String, delay: Duration) -> (u16, Arc<Mutex<Heard>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let heard = Arc::new(Mutex::new(Heard::default()));
    let noted = heard.clone();
    tokio::spawn(async move {
        while let Ok((tcp, _)) = listener.accept().await {
            let (page, noted) = (page.clone(), noted.clone());
            tokio::spawn(async move {
                let mut head = [0u8; 2048];
                let read = tcp.peek(&mut head).await.unwrap_or(0);
                if !String::from_utf8_lossy(&head[..read]).to_ascii_lowercase().contains("upgrade: websocket") {
                    tokio::time::sleep(delay).await;
                }
                let Some(tcp) = common::upgrade_or_page(tcp, &page).await else { return };
                noted.lock().unwrap().upgrades += 1;
                let Ok(mut ws) = tokio_tungstenite::accept_async(tcp).await else { return };
                while let Some(Ok(Message::Text(text))) = ws.next().await {
                    noted.lock().unwrap().frames.push(text.to_string());
                    let asked: Value = serde_json::from_str(&text).unwrap();
                    let answer = json!({ "id": asked["id"], "ok": true, "version": common::own_release(), "places": [] });
                    if ws.send(Message::text(answer.to_string())).await.is_err() {
                        break;
                    }
                }
            });
        }
    });
    (port, heard)
}

/// A state folder whose lock names this process, alive and this login's, on `port` at `address`, with the token
/// beside it.
fn locked(dir: &Path, port: u16, address: &str) -> PathBuf {
    let lock = json!({ "pid": std::process::id(), "port": port, "address": address, "startedAt": "2026-10-19T00:00:00.000Z" });
    std::fs::write(dir.join("host.lock"), lock.to_string()).unwrap();
    std::fs::write(dir.join("host-token"), format!("{TOKEN}\n")).unwrap();
    dir.join("state.json")
}

fn host_on(state: &Path, dir: &Path) -> Host {
    let env: Env = [("WSP_HOME".to_owned(), dir.join("home").to_string_lossy().into_owned())].into_iter().collect();
    Host::new(&Args { state: state.to_owned(), ..Args::default() }, &env, None)
}

fn wont_attach(dir: &Path, port: u16, why: &str) -> String {
    format!(
        "a host (pid {}) holds {} on port {port} but {why}: stop that process or run wsp down, then run this line again",
        std::process::id(),
        dir.join("host.lock").display()
    )
}

/// Catches: the token sent to a port whose page is no wsp page, as the dial did when a live pid was the whole proof.
#[tokio::test]
async fn a_port_serving_no_wsp_page_is_refused_and_never_sent_the_token() {
    let dir = tempfile::tempdir().unwrap();
    let (port, heard) = squatter("<html><body>a dev server</body></html>".to_owned()).await;
    let state = locked(dir.path(), port, "127.0.0.1");
    let Err(refused) = host_on(&state, dir.path()).client().await else { panic!("the dial attached to a page no wsp host serves") };
    assert_eq!(refused.message, wont_attach(dir.path(), port, "no wsp host answers there"));
    assert!(APP_WORDS.iter().all(|word| !refused.message.contains(word)), "{}", refused.message);
    assert_eq!(refused.kind.as_deref(), Some(LOCK_REFUSED));
    let heard = heard.lock().unwrap();
    assert!(heard.frames.iter().all(|f| !f.contains(TOKEN)), "the squatter read the token: {:?}", heard.frames);
    assert_eq!(heard.upgrades, 0, "a socket was opened to a port that serves no wsp page");
}

/// Catches: the desktop app refused in the command line's words, or in anything but today's app's
/// (`desktop:host-lifecycle.ts` `wontAttach` and `lockedHost`).
#[tokio::test]
async fn the_desktop_is_refused_in_the_apps_own_words() {
    let dir = tempfile::tempdir().unwrap();
    let (port, _) = squatter("<html><body>a dev server</body></html>".to_owned()).await;
    let state = locked(dir.path(), port, "127.0.0.1");
    let desktop = host_on(&state, dir.path()).with_release_rule(ReleaseRule::OwnOrNewer);
    let Err(refused) = desktop.client().await else { panic!("the dial attached to a page no wsp host serves") };
    let want = format!(
        "a host (pid {}) holds {} on port {port} but no wsp host answers there: stop that process or run wsp down, then open wsp again",
        std::process::id(),
        dir.path().join("host.lock").display()
    );
    assert_eq!(refused.message, want);

    let dir = tempfile::tempdir().unwrap();
    let (port, _) =
        squatter(format!("<script>window.__WSP__ = {};</script>", json!({ "tokenHash": common::digest(TOKEN), "wsPath": "/ws" }))).await;
    let state = locked(dir.path(), port, "127.0.0.1");
    let desktop = host_on(&state, dir.path()).with_release_rule(ReleaseRule::OwnOrNewer);
    let Err(refused) = desktop.client().await else { panic!("a page naming no release was attached") };
    let want = format!(
        "a wsp older than this app's {} (pid {}) serves {} on port {port}, and its page names no release: stop that process, then open wsp again",
        common::own_release(),
        std::process::id(),
        state.display()
    );
    assert_eq!(refused.message, want);
}

/// Catches: the token sent to a wsp page that carries another token's digest, or the digest compared to anything but
/// the sha256 of the token file beside the state.
#[tokio::test]
async fn a_wsp_page_carrying_another_tokens_digest_is_refused_and_never_sent_the_token() {
    let dir = tempfile::tempdir().unwrap();
    let (port, heard) = squatter(common::wsp_page("another-token")).await;
    let state = locked(dir.path(), port, "127.0.0.1");
    let Err(refused) = host_on(&state, dir.path()).client().await else { panic!("the dial attached to another token's page") };
    assert_eq!(
        refused.message,
        wont_attach(dir.path(), port, "the page it serves carries another token's digest than the file beside this state")
    );
    assert_eq!(refused.kind.as_deref(), Some(LOCK_REFUSED));
    let heard = heard.lock().unwrap();
    assert!(heard.frames.iter().all(|f| !f.contains(TOKEN)), "the squatter read the token: {:?}", heard.frames);
    assert_eq!(heard.upgrades, 0, "a socket was opened to a page carrying another token's digest");
}

/// Catches: the digest check refusing the owner's own page, as a compare of the token itself or of a trimmed file would.
#[tokio::test]
async fn the_page_carrying_the_digest_of_the_token_file_is_attached_and_sent_the_token() {
    let dir = tempfile::tempdir().unwrap();
    let (port, heard) = squatter(common::wsp_page(TOKEN)).await;
    let state = locked(dir.path(), port, "127.0.0.1");
    host_on(&state, dir.path()).client().await.unwrap_or_else(|f| panic!("{}", f.message));
    assert!(heard.lock().unwrap().frames[0].contains(TOKEN));
}

/// Catches: a host bound beyond loopback held to a digest its page never carries, as the desktop app does not hold it.
#[tokio::test]
async fn a_host_bound_beyond_loopback_is_held_to_the_lock_alone() {
    let dir = tempfile::tempdir().unwrap();
    let page = json!({ "wsPath": "/ws", "paired": false, "version": common::own_release() });
    let (port, heard) = squatter(format!("<script>window.__WSP__ = {page};</script>")).await;
    // The wildcard is dialled on loopback, and is no loopback address to hold a page to.
    let state = locked(dir.path(), port, "0.0.0.0");
    host_on(&state, dir.path()).client().await.unwrap_or_else(|f| panic!("{}", f.message));
    assert!(heard.lock().unwrap().frames[0].contains(TOKEN));
}

/// Catches: a page naming no release let through, where it is a wsp from before pages named theirs.
#[tokio::test]
async fn a_page_naming_no_release_is_a_wsp_older_than_this_one() {
    let dir = tempfile::tempdir().unwrap();
    let (port, heard) =
        squatter(format!("<script>window.__WSP__ = {};</script>", json!({ "tokenHash": common::digest(TOKEN), "wsPath": "/ws" }))).await;
    let state = locked(dir.path(), port, "127.0.0.1");
    let Err(refused) = host_on(&state, dir.path()).client().await else { panic!("a page naming no release was attached") };
    let want = format!(
        "a wsp older than this line's {} (pid {}) serves {} on port {port}, and its page names no release: stop that process, then run this line again",
        common::own_release(),
        std::process::id(),
        state.display()
    );
    assert_eq!(refused.message, want);
    assert!(APP_WORDS.iter().all(|word| !refused.message.contains(word)), "{}", refused.message);
    assert_eq!(refused.kind.as_deref(), Some(LOCK_REFUSED));
    assert_eq!(heard.lock().unwrap().upgrades, 0);
}

/// Catches: a port that answers nothing refused as one that answered no wsp page, which a caller waiting on a host
/// still starting could not tell apart.
#[tokio::test]
async fn a_port_that_answers_nothing_is_not_answering() {
    let dir = tempfile::tempdir().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let sent = Arc::new(Mutex::new(Vec::new()));
    let noted = sent.clone();
    // Takes the connection and reads it, answering nothing, as a host busy before it serves.
    tokio::spawn(async move {
        while let Ok((mut tcp, _)) = listener.accept().await {
            let noted = noted.clone();
            tokio::spawn(async move {
                let mut bytes = [0u8; 4096];
                while let Ok(read @ 1..) = tcp.read(&mut bytes).await {
                    noted.lock().unwrap().extend_from_slice(&bytes[..read]);
                }
            });
        }
    });
    let state = locked(dir.path(), port, "127.0.0.1");
    let Err(refused) = host_on(&state, dir.path()).client().await else { panic!("a port that answered nothing was attached") };
    assert_eq!(refused.kind.as_deref(), Some(NOT_ANSWERING), "{}", refused.message);
    assert_eq!(refused.message, wont_attach(dir.path(), port, "no wsp host answers there"));
    assert!(!String::from_utf8_lossy(&sent.lock().unwrap()).contains(TOKEN));
}

/// Catches: the device token a pairing bought sent to whatever holds the lock's port, as the dial did when only the
/// file beside the state waited on the page.
#[tokio::test]
async fn a_paired_token_never_leaves_for_a_port_serving_no_wsp_page() {
    let dir = tempfile::tempdir().unwrap();
    let (port, heard) = squatter("<html><body>a dev server</body></html>".to_owned()).await;
    let state = locked(dir.path(), port, "127.0.0.1");
    let host = host_on(&state, dir.path());
    host.hold_token("the-paired-device-token".to_owned());
    let Err(refused) = host.client().await else { panic!("the dial attached to a page no wsp host serves") };
    assert_eq!(refused.message, wont_attach(dir.path(), port, "no wsp host answers there"));
    assert_eq!(refused.kind.as_deref(), Some(LOCK_REFUSED));
    let heard = heard.lock().unwrap();
    assert!(heard.frames.iter().all(|f| !f.contains("the-paired-device-token")), "the squatter read the token: {:?}", heard.frames);
    assert_eq!(heard.upgrades, 0, "a socket was opened to a port that serves no wsp page");
}

/// Catches: a pairing code spent on whatever holds the lock's port, which could spend it on the real host itself.
#[tokio::test]
async fn a_pairing_code_never_leaves_for_a_port_serving_no_wsp_page() {
    let dir = tempfile::tempdir().unwrap();
    let (port, heard) = squatter("<html><body>a dev server</body></html>".to_owned()).await;
    let state = locked(dir.path(), port, "127.0.0.1");
    let Err(refused) = host_on(&state, dir.path()).redeem("424242", "laptop").await else { panic!("the code was spent") };
    assert_eq!(refused.message, wont_attach(dir.path(), port, "no wsp host answers there"));
    assert_eq!(refused.kind.as_deref(), Some(LOCK_REFUSED));
    let heard = heard.lock().unwrap();
    assert!(heard.frames.iter().all(|f| !f.contains("424242")), "the squatter read the code: {:?}", heard.frames);
    assert_eq!(heard.upgrades, 0, "a socket was opened to a port that serves no wsp page");
}

/// Catches: a device token held to the digest of the file beside the state, which a pairing's token never matches.
#[tokio::test]
async fn a_paired_token_goes_to_a_wsp_page_carrying_the_host_tokens_digest() {
    let dir = tempfile::tempdir().unwrap();
    let (port, heard) = squatter(common::wsp_page(TOKEN)).await;
    let state = locked(dir.path(), port, "127.0.0.1");
    let host = host_on(&state, dir.path());
    host.hold_token("the-paired-device-token".to_owned());
    host.client().await.unwrap_or_else(|f| panic!("{}", f.message));
    assert!(heard.lock().unwrap().frames[0].contains("the-paired-device-token"));
}

/// Catches: the page read taking whatever the port sends until the window closes, as a port streaming forever would
/// hold it, or a page cut short at the cap read as far as it went.
#[tokio::test]
async fn a_port_streaming_forever_is_cut_at_the_cap_and_refused_at_once() {
    let dir = tempfile::tempdir().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        while let Ok((mut tcp, _)) = listener.accept().await {
            tokio::spawn(async move {
                let head = format!("HTTP/1.1 200 OK\r\ncontent-type: text/html\r\n\r\n{}", common::wsp_page(TOKEN));
                let filler = vec![b' '; 64 * 1024];
                if tcp.write_all(head.as_bytes()).await.is_err() {
                    return;
                }
                while tcp.write_all(&filler).await.is_ok() {}
            });
        }
    });
    let state = locked(dir.path(), port, "127.0.0.1");
    let from = Instant::now();
    let Err(refused) = host_on(&state, dir.path()).client().await else { panic!("a page past the cap was attached") };
    assert_eq!(refused.kind.as_deref(), Some(LOCK_REFUSED), "{}", refused.message);
    assert_eq!(refused.message, wont_attach(dir.path(), port, "no wsp host answers there"));
    assert!(from.elapsed() < Duration::from_secs(1), "the read ran {:?}", from.elapsed());
}

/// Catches: the page read held to a window of its own shorter than the dial's, so a first dial to a host slow to
/// serve its page fails at once.
#[tokio::test]
async fn a_page_slower_than_two_seconds_is_read_within_the_dials_window() {
    let dir = tempfile::tempdir().unwrap();
    let (port, heard) = squatter_after(common::wsp_page(TOKEN), Duration::from_millis(2_500)).await;
    let state = locked(dir.path(), port, "127.0.0.1");
    host_on(&state, dir.path()).client().await.unwrap_or_else(|f| panic!("{}", f.message));
    assert!(heard.lock().unwrap().frames[0].contains(TOKEN));
}
