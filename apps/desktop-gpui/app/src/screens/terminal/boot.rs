// SPDX-License-Identifier: AGPL-3.0-only
//! The boot line of the page the host serves (`window.__WSP__ = {...};`, server.ts `loadPage`), read once a launch with
//! the first terminal for the family the host detected from the recipe (`detectedTerminalFont` in `fontSetting.ts`).

use std::io::{Read, Write};
use std::net::{IpAddr, SocketAddr, TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

use gpui_kit::{App, AppContext, Global, Task};
use serde_json::Value;

/// The most of the host's page read for its boot line, which stands in the page's head.
const PAGE_CAP: usize = 512 * 1024;
/// The whole read, from the dial to the boot line.
const PAGE_WAIT: Duration = Duration::from_secs(5);
const BOOT_LINE: &str = "window.__WSP__ = ";

/// `key` in the page's boot line: its value, none where the line lacks it; nothing where the page holds no boot line.
pub fn boot_key(page: &str, key: &str) -> Option<Option<Value>> {
    let at = page.find(BOOT_LINE)? + BOOT_LINE.len();
    let end = at + page[at..].find(";</script>")?;
    let mut boot: Value = serde_json::from_str(&page[at..end]).ok()?;
    Some(boot.get_mut(key).map(Value::take))
}

/// `key` in the boot line of the host's page at `page` (`http://<address>:<port>/`), one GET bounded in bytes and time.
pub fn read_boot_key(page: &str, key: &str) -> Result<Option<Value>, String> {
    read_boot_key_within(page, key, PAGE_WAIT)
}

/// `dial_address` in wsp-mcp's `aim.rs`: a wildcard bind is dialled on loopback (`loopback` in its `host.json`).
fn dial_host(address: &str) -> &str {
    if matches!(address, "0.0.0.0" | "::") {
        "127.0.0.1"
    } else {
        address
    }
}

fn read_boot_key_within(page: &str, key: &str, wait: Duration) -> Result<Option<Value>, String> {
    let deadline = Instant::now() + wait;
    let left = || deadline.checked_duration_since(Instant::now()).filter(|d| !d.is_zero()).ok_or("the host's page took too long");
    let bound = page.trim_start_matches("http://").trim_end_matches('/');
    let (address, port) = bound.rsplit_once(':').ok_or("the host's page names no port")?;
    let port: u16 = port.parse().map_err(|_| format!("the host's page names no port: {page}"))?;
    let host = dial_host(address.trim_start_matches('[').trim_end_matches(']'));
    // Only `localhost` goes to the resolver, and only a loopback answer of it is dialled.
    let address = match host {
        "localhost" => ("localhost", port).to_socket_addrs().map_err(|e| e.to_string())?.find(|a| a.ip().is_loopback()),
        _ => host.parse::<IpAddr>().ok().filter(IpAddr::is_loopback).map(|ip| SocketAddr::new(ip, port)),
    };
    let address = address.ok_or_else(|| format!("the host's page is not on this computer's loopback: {page}"))?;
    let mut socket = TcpStream::connect_timeout(&address, left()?).map_err(|e| e.to_string())?;
    let ask = format!("GET / HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\n\r\n");
    socket.set_write_timeout(Some(left()?)).map_err(|e| e.to_string())?;
    socket.write_all(ask.as_bytes()).map_err(|e| e.to_string())?;
    let mut read = Vec::new();
    let mut chunk = [0u8; 16 * 1024];
    while read.len() < PAGE_CAP {
        socket.set_read_timeout(Some(left()?)).map_err(|e| e.to_string())?;
        let n = socket.read(&mut chunk).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        read.extend_from_slice(&chunk[..n]);
        if let Some(value) = boot_key(&String::from_utf8_lossy(&read), key) {
            return Ok(value);
        }
    }
    Err("the host's page holds no boot line".into())
}

/// `terminalFont` as `detectedTerminalFont` takes it: trimmed, and an empty one is none.
pub fn family_of(value: Option<Value>) -> Option<String> {
    value.as_ref().and_then(Value::as_str).map(str::trim).filter(|f| !f.is_empty()).map(str::to_owned)
}

/// The host's page, and the family its boot line names once read.
#[derive(Default)]
struct Boot {
    page: Option<String>,
    font: Option<Option<String>>,
    reading: Option<Task<()>>,
}

impl Global for Boot {}

/// Hands the terminal the page the host serves, whose boot line [`detected_font`] reads.
pub fn read_from(page: Option<String>, cx: &mut App) {
    cx.default_global::<Boot>().page = page;
}

/// The family the host detected as the person's own terminal's, none until its page answers; the first ask reads the
/// page off the main thread and redraws the windows once it has.
pub fn detected_font(cx: &mut App) -> Option<String> {
    let boot = cx.default_global::<Boot>();
    if let Some(font) = &boot.font {
        return font.clone();
    }
    let page = boot.page.clone().filter(|_| boot.reading.is_none())?;
    let read = cx.background_spawn(async move { read_boot_key(&page, "terminalFont") });
    let reading = cx.spawn(async move |cx| {
        let font = match read.await {
            Ok(value) => family_of(value),
            Err(e) => {
                eprintln!("desktop-gpui: the host's page: {e}");
                None
            }
        };
        cx.update(|cx| {
            cx.global_mut::<Boot>().font = Some(font);
            cx.refresh_windows();
        });
    });
    cx.global_mut::<Boot>().reading = Some(reading);
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    const PAGE: &str = r#"<!doctype html><html><head><script>window.__WSP__ = {"wsPath":"/ws","terminalFont":" Berkeley Mono "};</script></head><body></body></html>"#;

    #[test]
    fn the_boot_line_gives_one_key() {
        assert_eq!(boot_key(PAGE, "terminalFont"), Some(Some(Value::from(" Berkeley Mono "))));
        assert_eq!(boot_key(PAGE, "productUsageOff"), Some(None), "a boot line without the key");
        assert_eq!(boot_key("<html><head></head></html>", "terminalFont"), None, "a page with no boot line");
        assert_eq!(family_of(boot_key(PAGE, "terminalFont").flatten()), Some("Berkeley Mono".to_owned()));
        assert_eq!(family_of(Some(Value::from("  "))), None);
    }

    /// Serves `PAGE` to each of `times` asks on loopback, handing back each ask's first read.
    fn serve(times: usize) -> (u16, std::thread::JoinHandle<Vec<String>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let serving = std::thread::spawn(move || {
            (0..times)
                .map(|_| {
                    let (mut socket, _) = listener.accept().unwrap();
                    let mut ask = [0u8; 1024];
                    let n = socket.read(&mut ask).unwrap();
                    let reply = format!("HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\r\n{PAGE}", PAGE.len());
                    socket.write_all(reply.as_bytes()).unwrap();
                    String::from_utf8_lossy(&ask[..n]).into_owned()
                })
                .collect()
        });
        (port, serving)
    }

    #[test]
    fn one_get_reads_the_key_off_the_hosts_page() {
        let (port, serving) = serve(1);
        let read = read_boot_key(&format!("http://127.0.0.1:{port}/"), "terminalFont");
        assert_eq!(read, Ok(Some(Value::from(" Berkeley Mono "))));
        assert!(serving.join().unwrap()[0].starts_with("GET / HTTP/1.1\r\n"));
    }

    #[test]
    fn a_wildcard_lock_address_is_dialled_on_loopback() {
        let (port, serving) = serve(2);
        for page in [format!("http://0.0.0.0:{port}/"), format!("http://:::{port}/")] {
            assert_eq!(read_boot_key(&page, "terminalFont"), Ok(Some(Value::from(" Berkeley Mono "))), "{page}");
        }
        assert_eq!(serving.join().unwrap().len(), 2);
    }

    #[test]
    fn an_address_off_loopback_is_refused_without_a_dial() {
        let began = Instant::now();
        // TEST-NET-1 answers nobody, so a dial would wait out its timeout or fail with another error.
        for page in [
            "http://192.0.2.1:9/",
            "http://127.999.0.1:9/",
            "http://127.0.0.1.nip.io:9/",
            "http://x@127.0.0.1:9/",
            "http://[::ffff:c000:201]:9/",
        ] {
            let read = read_boot_key(page, "terminalFont");
            assert!(read.as_ref().is_err_and(|e| e.contains("not on this computer's loopback")), "{page}: {read:?}");
        }
        assert!(began.elapsed() < Duration::from_millis(500), "refused before any dial");
    }

    #[test]
    fn a_trickling_page_is_cut_at_the_deadline() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            let mut ask = [0u8; 1024];
            _ = socket.read(&mut ask);
            for _ in 0..200 {
                if socket.write_all(b"x").is_err() {
                    return;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        });
        let began = Instant::now();
        let read = read_boot_key_within(&format!("http://127.0.0.1:{port}/"), "terminalFont", Duration::from_millis(300));
        let took = began.elapsed();
        assert!(read.is_err(), "{read:?}");
        assert!(took < Duration::from_millis(1000), "the read ran {took:?} past a 300 ms deadline");
    }
}
