// SPDX-License-Identifier: AGPL-3.0-only
//! `startHost` of `server.ts`, thin: the app's HTTP server on the bound port (the page with its boot object, the
//! bundle's files, a JSON 404), the runtime's socket on `/ws` through the upgrades it hands over, and the door a computer
//! you own dials. Every other door and slot of `serveRuntime` (the account, the release, the restart, the broker, the
//! doctor, the callback relay, the folders and the rest) is asked of its `wire.rs`, which answers `None` until its packet
//! lands.
use std::cell::{Cell, RefCell};
use std::fs;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::Path;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context, Poll};
use std::time::Duration;

use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;
use wsp_cli::host_lock::door_port_path;
use wsp_core::devices::token_digest;
use wsp_core::types::api::Runtime;
use wsp_core::types::wiring::Detach;
use wsp_machines::agents_read::ForwardsHolder;
use wsp_serve::serve::wsframe::{check_upgrade, is_upgrade, Head};
use wsp_serve::serve::Released;
use wsp_serve::serve::{LogFn, OpenedDoor, PlaceDoorControl, UpgradeHead, UpgradeRule};
use wsp_serve::{serve_runtime, AssembledRuntime, RuntimeServer, ServeOptions};
use wsp_store::own_file::write_own;
use wsp_wire::types::app_ports::{LOOPBACK, PLACE_PORT_OFFSET, WILDCARD, WS_PATH};
use wsp_wire::views::desktop_bridge::{BootPayload, ProductUsageOff};
use wsp_wire::wire::place_link::{door_port_held_line, door_port_moved_line, PlaceDoorView};
use wsp_words::app_ports::{authority, is_loopback, is_wildcard, join_address_of, peer_address, served_hostname};
use wsp_words::exit::HostError;

use wsp_setup::{doctor, files, recipes, relay, relay_link};

use crate::paths::{basename, resolve};
use crate::wiring::Wiring;
use crate::{broker, commands, file_doors, serve};

/// The header the door's listener marks every upgrade with, and that any client's own is stripped for: a hand-over of
/// an upgrade carries no socket to ask which listener it came in on.
const DOOR_HEADER: &str = "x-wsp-door";
/// A request head past this is no request; node's own limit is 16 KiB.
const MAX_HEAD: usize = 16 * 1024;
/// The page's own placeholder line, which the boot object replaces.
const BOOT_PREFIX: &str = "<script>window.__WSP__ = ";
const BOOT_SUFFIX: &str = ";</script>";

pub struct HostOptions {
    pub runtime: Rc<AssembledRuntime>,
    /// What the runtime was made from, which each door's `wire.rs` is handed.
    pub wiring: Wiring,
    /// The built web app: index.html plus its assets.
    pub web_dir: String,
    pub listener: TcpListener,
    /// The address the listener is bound on.
    pub address: String,
    /// The port that was asked for, which the door's own is offset from; 0 asks the system for both.
    pub asked_port: u16,
    pub advertise: Option<String>,
    pub state_path: String,
    pub auth_token: String,
    pub version: String,
    pub road: Option<String>,
    pub log: LogFn,
    pub door_open: bool,
    pub product_usage_off: Option<ProductUsageOff>,
}

pub struct HostHandle {
    pub port: u16,
    pub auth_token: String,
    front: Rc<Front>,
    server: Rc<dyn RuntimeServer>,
    runtime: Rc<AssembledRuntime>,
    accepting: RefCell<Vec<JoinHandle<()>>>,
    /// The callback relay's hold on `rt.agents.forwards`, let go at close.
    relay: RefCell<Option<Detach>>,
}

/// Everything a request needs to be answered.
struct Front {
    web_dir: String,
    version: String,
    auth_token: String,
    state_path: String,
    bound_here: bool,
    address: String,
    advertise: Option<String>,
    door_port: u16,
    port: Cell<u16>,
    log: LogFn,
    product_usage_off: Option<ProductUsageOff>,
    server: RefCell<Option<Rc<dyn RuntimeServer>>>,
    door: tokio::sync::Mutex<Option<DoorAt>>,
}

struct DoorAt {
    port: u16,
    task: JoinHandle<()>,
}

/// JSON fit for an inline script: `<`, `>`, `&` and the two line terminators JSON allows but a script does not are
/// written as escapes, so no value can end the script or the page.
fn inline_json(value: &impl serde::Serialize) -> String {
    let text = serde_json::to_string(value).expect("a boot object writes as JSON");
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '<' | '>' | '&' | '\u{2028}' | '\u{2029}' => out.push_str(&format!("\\u{:04x}", c as u32)),
            other => out.push(other),
        }
    }
    out
}

/// The page with the dev default's boot line swapped for the real boot object.
fn load_page(web_dir: &str, boot: &BootPayload) -> Result<String, HostError> {
    let path = format!("{web_dir}/index.html");
    if !Path::new(&path).exists() {
        return Err(HostError::new(format!("web app not built: {path} is missing (pnpm --filter @wsp/web... build)")));
    }
    let html = fs::read_to_string(&path).map_err(|e| HostError::new(e.to_string()))?;
    let mut from = 0;
    while let Some(at) = html[from..].find(BOOT_PREFIX) {
        let start = from + at;
        let body = start + BOOT_PREFIX.len();
        if let Some(end) = html[body..].find('<').map(|i| body + i) {
            if html[..end].ends_with(';') && html[end..].starts_with("</script>") {
                let stop = end + "</script>".len();
                return Ok(format!("{}{BOOT_PREFIX}{}{BOOT_SUFFIX}{}", &html[..start], inline_json(boot), &html[stop..]));
            }
        }
        from = body;
    }
    Err(HostError::new(format!("{path} has no window.__WSP__ boot line to replace")))
}

fn header<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers.iter().find(|(key, _)| key.eq_ignore_ascii_case(name)).map(|(_, value)| value.as_str())
}

/// Whether the connector forwarded this request, rather than a process on the computer this host runs on.
fn through_connector(headers: &[(String, String)]) -> bool {
    header(headers, "cf-connecting-ip").is_some() || header(headers, "cf-ray").is_some()
}

fn hostname_asked(headers: &[(String, String)]) -> Option<String> {
    served_hostname(&format!("http://{}", header(headers, "host").unwrap_or("")))
}

/// Whether a request may write here or open a socket: one carrying no Origin is a tool on this computer; one carrying
/// an Origin is a page, and a page drives only the host it was served by.
fn origin_allows(headers: &[(String, String)]) -> bool {
    let Some(origin) = header(headers, "origin").filter(|o| !o.is_empty()) else { return true };
    match (served_hostname(origin), hostname_asked(headers)) {
        (Some(from), Some(at)) => from == at,
        _ => false,
    }
}

fn from_door(head: &UpgradeHead) -> bool {
    header(&head.headers, DOOR_HEADER).is_some()
}

fn own_road(head: &UpgradeHead) -> bool {
    !from_door(head)
        && !through_connector(&head.headers)
        && is_loopback(&peer_address(head.peer.map(|peer| peer.ip().to_string()).as_deref()))
}

fn peer_of(head: &UpgradeHead) -> String {
    let peer = peer_address(head.peer.map(|peer| peer.ip().to_string()).as_deref());
    match header(&head.headers, "cf-connecting-ip") {
        Some(carried) if carried.parse::<IpAddr>().is_ok() && !from_door(head) && is_loopback(&peer) => carried.to_string(),
        _ => peer,
    }
}

fn percent_decoded(path: &str) -> Result<String, ()> {
    let bytes = path.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        if bytes[at] == b'%' {
            let hex = bytes.get(at + 1..at + 3).and_then(|h| std::str::from_utf8(h).ok()).and_then(|h| u8::from_str_radix(h, 16).ok());
            out.push(hex.ok_or(())?);
            at += 3;
        } else {
            out.push(bytes[at]);
            at += 1;
        }
    }
    String::from_utf8(out).map_err(|_| ())
}

fn content_type(file: &str) -> &'static str {
    match file.rsplit_once('.').map(|(_, ext)| ext) {
        Some("html") => "text/html; charset=utf-8",
        Some("js") => "text/javascript",
        Some("css") => "text/css",
        Some("map") => "application/json",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("ico") => "image/x-icon",
        Some("woff2") => "font/woff2",
        Some("wasm") => "application/wasm",
        _ => "application/octet-stream",
    }
}

fn extension_of(path: &str) -> &str {
    let name = path.rsplit('/').next().unwrap_or("");
    match name.rfind('.') {
        Some(0) | None => "",
        Some(at) => &name[at..],
    }
}

struct Reply {
    status: u16,
    content_type: String,
    body: Vec<u8>,
}

impl Reply {
    fn json(status: u16, body: serde_json::Value) -> Reply {
        Reply { status, content_type: "application/json".into(), body: body.to_string().into_bytes() }
    }

    fn bytes(self) -> Vec<u8> {
        let phrase = match self.status {
            200 => "OK",
            404 => "Not Found",
            500 => "Internal Server Error",
            _ => "Error",
        };
        let mut out = format!(
            "HTTP/1.1 {} {phrase}\r\ncontent-type: {}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
            self.status,
            self.content_type,
            self.body.len()
        )
        .into_bytes();
        out.extend(self.body);
        out
    }
}

impl Front {
    fn page(&self, here: bool) -> Result<String, HostError> {
        let boot = BootPayload {
            token_hash: here.then(|| token_digest(&self.auth_token)),
            ws_path: WS_PATH.to_string(),
            paired: here,
            version: self.version.clone(),
            state_path: here.then(|| self.state_path.clone()),
            product_usage_off: self.product_usage_off,
        };
        load_page(&self.web_dir, &boot)
    }

    fn asset(&self, path: &str) -> Result<Option<Reply>, ()> {
        let decoded = percent_decoded(path)?;
        let file = resolve(&self.web_dir, &format!(".{decoded}"));
        let inside = file.starts_with(&format!("{}/", self.web_dir));
        if !inside || !Path::new(&file).is_file() {
            return Ok(None);
        }
        Ok(fs::read(&file).ok().map(|body| Reply { status: 200, content_type: content_type(&file).to_string(), body }))
    }

    fn answer(&self, head: &Head, here: bool) -> Reply {
        let path = head.target.split(['?', '#']).next().unwrap_or("/");
        let page = |here: bool| match self.page(here) {
            Ok(html) => Reply { status: 200, content_type: "text/html; charset=utf-8".into(), body: html.into_bytes() },
            Err(e) => Reply::json(500, serde_json::json!({"error": e.message})),
        };
        if head.method == "GET" && path == "/" {
            return page(here);
        }
        if head.method == "GET" {
            match self.asset(path) {
                Ok(Some(reply)) => return reply,
                Ok(None) => {}
                Err(()) => return Reply::json(500, serde_json::json!({"error": "URI malformed"})),
            }
            if extension_of(path).is_empty() && !path.ends_with('/') {
                return page(here);
            }
        }
        Reply::json(404, serde_json::json!({"error": format!("no route: {} {path}", head.method)}))
    }
}

/// An upgrade's socket with the bytes already read past its head put back in front.
struct Prefixed {
    prefix: Vec<u8>,
    at: usize,
    inner: TcpStream,
}

impl AsyncRead for Prefixed {
    fn poll_read(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<std::io::Result<()>> {
        if self.at < self.prefix.len() {
            let take = (self.prefix.len() - self.at).min(buf.remaining());
            let from = self.at;
            buf.put_slice(&self.prefix[from..from + take]);
            self.at += take;
            return Poll::Ready(Ok(()));
        }
        Pin::new(&mut self.inner).poll_read(cx, buf)
    }
}

impl AsyncWrite for Prefixed {
    fn poll_write(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<std::io::Result<usize>> {
        Pin::new(&mut self.inner).poll_write(cx, buf)
    }
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }
    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

async fn read_head(tcp: &mut TcpStream) -> Option<(Head, Vec<u8>)> {
    let mut buf: Vec<u8> = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        match Head::read(&buf) {
            Ok(Some((head, took))) => return Some((head, buf.split_off(took))),
            Ok(None) if buf.len() <= MAX_HEAD => {}
            _ => {
                let _ = tcp.write_all(b"HTTP/1.1 400 Bad Request\r\nConnection: close\r\n\r\n").await;
                return None;
            }
        }
        match tcp.read(&mut chunk).await {
            Ok(0) | Err(_) => return None,
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
        }
    }
}

async fn connection(front: Rc<Front>, mut tcp: TcpStream, peer: SocketAddr, on_door: bool) {
    let Some((head, rest)) = read_head(&mut tcp).await else { return };
    if is_upgrade(&head.headers) {
        let Some(server) = front.server.borrow().clone() else { return };
        if head.method != "GET" {
            if let Err(refusal) = check_upgrade(&head.method, &head.headers) {
                let _ = tcp.write_all(&refusal.bytes()).await;
                return;
            }
        }
        let mut headers: Vec<(String, String)> = head.headers.into_iter().filter(|(name, _)| name != DOOR_HEADER).collect();
        if on_door {
            headers.push((DOOR_HEADER.to_string(), "1".to_string()));
        }
        let upgrade = UpgradeHead { path_and_query: head.target, headers, peer: Some(peer) };
        server.upgrade(upgrade, Box::new(Prefixed { prefix: rest, at: 0, inner: tcp })).await;
        return;
    }
    let here = !on_door
        && front.bound_here
        && !through_connector(&head.headers)
        && is_loopback(&hostname_asked(&head.headers).unwrap_or_default());
    let reply = front.answer(&head, here);
    let _ = tcp.write_all(&reply.bytes()).await;
    let _ = tcp.shutdown().await;
}

fn accepting(front: &Rc<Front>, listener: TcpListener, on_door: bool) -> JoinHandle<()> {
    let front = front.clone();
    tokio::task::spawn_local(async move {
        loop {
            match listener.accept().await {
                Ok((tcp, peer)) => {
                    tokio::task::spawn_local(connection(front.clone(), tcp, peer, on_door));
                }
                Err(_) => tokio::time::sleep(Duration::from_millis(10)).await,
            }
        }
    })
}

/// The address the person named with `--advertise`, as an address: a blank and a trailing slash read one way.
pub fn advertise_word(word: Option<&str>) -> Option<String> {
    let said = word?.trim().trim_end_matches('/');
    (!said.is_empty()).then(|| said.to_string())
}

/// The addresses of this computer's interfaces that leave it: IPv4 only (the wildcard the door binds is IPv4), not
/// loopback and not link-local.
fn interface_addresses() -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    let mut list: *mut libc::ifaddrs = std::ptr::null_mut();
    // SAFETY: getifaddrs fills `list` with a chain freed below; each entry is read while the chain stands.
    unsafe {
        if libc::getifaddrs(&mut list) != 0 {
            return found;
        }
        let mut at = list;
        while !at.is_null() {
            let entry = &*at;
            let internal = entry.ifa_flags & (libc::IFF_LOOPBACK as u32) != 0;
            if !internal && !entry.ifa_addr.is_null() && i32::from((*entry.ifa_addr).sa_family) == libc::AF_INET {
                let sin = &*(entry.ifa_addr as *const libc::sockaddr_in);
                let ip = Ipv4Addr::from(u32::from_be(sin.sin_addr.s_addr));
                if !(ip.octets()[0] == 169 && ip.octets()[1] == 254) {
                    let text = ip.to_string();
                    if !found.contains(&text) {
                        found.push(text);
                    }
                }
            }
            at = entry.ifa_next;
        }
        libc::freeifaddrs(list);
    }
    found
}

/// `reachAddresses(bound)`: the wildcard stands for each address this computer answers on, loopback only where it
/// covered nothing else; anything else is itself.
fn reach_addresses(bound: &str) -> Vec<String> {
    if !is_wildcard(bound) {
        return vec![bound.to_string()];
    }
    let found = interface_addresses();
    if found.is_empty() {
        vec![LOOPBACK.to_string()]
    } else {
        found
    }
}

/// `doorAddresses(bound, port, advertise)`: the address the person named first, then what this computer answers on.
pub fn door_addresses(bound: &str, port: u16, advertise: Option<&str>) -> Vec<String> {
    let named = advertise_word(advertise).and_then(|word| join_address_of(&word));
    let own = reach_addresses(bound).into_iter().map(|address| format!("http://{}", authority(&address, f64::from(port))));
    named.iter().cloned().chain(own.filter(|at| Some(at) != named.as_ref())).collect()
}

/// The door port a host asked for any free port bound at its last start, or nothing where it bound none yet.
fn last_door_port(state_path: &str) -> Option<u16> {
    let port: u32 = fs::read_to_string(door_port_path(state_path)).ok()?.trim().parse().ok()?;
    (port > 0 && port < 65_536).then_some(port as u16)
}

impl Front {
    fn view_of(&self, at: u16, bound: &str) -> OpenedDoor {
        OpenedDoor {
            view: PlaceDoorView {
                port: (at as i64).into(),
                addresses: door_addresses(bound, at, self.advertise.as_deref()),
                host_key: String::new(),
                relay: relay_link::wire::door_relay(&self.state_path),
            },
            back_port: self.bound_here.then_some(at),
        }
    }
}

struct DoorControl {
    front: Rc<Front>,
}

#[async_trait(?Send)]
impl PlaceDoorControl for DoorControl {
    async fn open(&self) -> Result<OpenedDoor, HostError> {
        let front = &self.front;
        if !front.bound_here {
            return Ok(front.view_of(front.port.get(), &front.address));
        }
        let mut held = front.door.lock().await;
        if let Some(open) = held.as_ref() {
            return Ok(front.view_of(open.port, WILDCARD));
        }
        let last = if front.door_port == 0 { last_door_port(&front.state_path) } else { None };
        let mut at = last.unwrap_or(front.door_port);
        let listener = match TcpListener::bind((WILDCARD, at)).await {
            Ok(listener) => listener,
            Err(e) if e.kind() == std::io::ErrorKind::AddrInUse && last.is_some() => {
                (front.log)(&door_port_moved_line(i64::from(at)));
                at = 0;
                TcpListener::bind((WILDCARD, 0)).await.map_err(|e| HostError::new(e.to_string()))?
            }
            Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => return Err(HostError::new(door_port_held_line(i64::from(at)))),
            Err(e) => return Err(HostError::new(e.to_string())),
        };
        let bound = listener.local_addr().map(|a| a.port()).unwrap_or(at);
        if front.door_port == 0 && last.is_none() {
            let state = Path::new(&front.state_path);
            let _ =
                write_own(state.parent().unwrap_or(Path::new("/")), &basename(&door_port_path(&front.state_path)), format!("{bound}\n"));
        }
        *held = Some(DoorAt { port: bound, task: accepting(front, listener, true) });
        Ok(front.view_of(bound, WILDCARD))
    }
}

/// Reaps what this host owns and nothing claims, saying what it stops. A host wired to no provider has no machine to
/// stop, so a sweep that fails is the only thing to say.
async fn sweep_orphans(rt: &Rc<AssembledRuntime>, log: &LogFn) {
    let say: wsp_core::types::api::SayStops = {
        let log = log.clone();
        Rc::new(move |line: &str| log(&format!("reap: {line}")))
    };
    if let Err(e) = rt.reap(None, Some(say)).await {
        log(&format!("reap: sweep failed: {}", e.message));
    }
}

/// How long a starting host waits on its provider's start-up reads before it reports itself up without them.
pub const START_READS_MS: u64 = 2_000;

pub async fn start_host(opts: HostOptions) -> Result<HostHandle, HostError> {
    let rt = opts.runtime.clone();
    let bound_here = is_loopback(&opts.address);
    let port = opts.listener.local_addr().map_err(|e| HostError::new(e.to_string()))?.port();
    let door_port = if opts.asked_port == 0 { 0 } else { (u64::from(opts.asked_port) + PLACE_PORT_OFFSET) as u16 };
    let front = Rc::new(Front {
        web_dir: resolve(&std::env::current_dir().map(|p| p.display().to_string()).unwrap_or_else(|_| "/".into()), &opts.web_dir),
        version: opts.version.clone(),
        auth_token: opts.auth_token.clone(),
        state_path: opts.state_path.clone(),
        bound_here,
        address: opts.address.clone(),
        advertise: opts.advertise.clone(),
        door_port,
        port: Cell::new(port),
        log: opts.log.clone(),
        product_usage_off: opts.product_usage_off,
        server: RefCell::new(None),
        door: tokio::sync::Mutex::new(None),
    });
    let origin_allowed: UpgradeRule = Rc::new(|head: &UpgradeHead| origin_allows(&head.headers));
    let own_road_rule: UpgradeRule = Rc::new(own_road);
    let (setup, wiring) = (opts.wiring.setup(), &opts.wiring);
    let serve = ServeOptions {
        released: Some(Released { version: Some(opts.version.clone()), road: opts.road.clone() }),
        origin_allowed: Some(origin_allowed),
        own_road: Some(own_road_rule),
        peer_of: Some(Rc::new(peer_of)),
        door: Some(Rc::new(DoorControl { front: front.clone() })),
        devices: Some(rt.devices()),
        account: relay_link::wire::account_door(&setup),
        admitted: relay_link::wire::admitted_devices(&setup),
        forwards: relay::wire::forwards_slot(&setup),
        broker: broker::wire::broker_door(wiring),
        projects: file_doors::wire::projects(wiring),
        landing: file_doors::wire::landing(wiring),
        folders: file_doors::wire::folders(wiring),
        terminal_config: file_doors::wire::terminal_config(wiring),
        editor: file_doors::wire::editor(wiring),
        ssh: file_doors::wire::ssh(wiring),
        init: serve::wire::init_door(wiring),
        recipes: recipes::wire::recipes_slot(&setup),
        doctor: doctor::wire::place_doctor(&setup),
        ssh_hosts: file_doors::wire::ssh_hosts(wiring),
        release: files::wire::release_door(&setup),
        restart: commands::wire::restart_door(wiring),
        auth_token: opts.auth_token.clone(),
        log: Some(opts.log.clone()),
        ..Default::default()
    };
    // The page is read once before anything serves: a bundle with no page is a start that is refused.
    front.page(bound_here)?;
    // Before the runtime socket: the app lists and stops the relay's forwards through it.
    let relay_hold = relay::wire::callback_forwards(&setup).map(|relay| rt.agents().forwards(Rc::new(ForwardsHolder(relay))));
    let server = serve_runtime(rt.clone(), serve).await?;
    *front.server.borrow_mut() = Some(server.clone());
    let task = accepting(&front, opts.listener, false);
    let handle = HostHandle {
        port,
        auth_token: opts.auth_token,
        front: front.clone(),
        server,
        runtime: rt.clone(),
        accepting: RefCell::new(vec![task]),
        relay: RefCell::new(relay_hold),
    };
    // Waited on for a moment, so a provider that answers says its lines before the host reports itself up; one that never
    // answers comes up all the same and its reads go on.
    let (reads_rt, reads_log, reads_state) = (rt.clone(), opts.log.clone(), opts.state_path.clone());
    let reads = tokio::task::spawn_local(async move { start_reads(&reads_rt, &reads_log, &reads_state).await });
    let _ = tokio::time::timeout(Duration::from_millis(START_READS_MS), reads).await;
    if opts.door_open {
        if let Err(e) = (DoorControl { front: front.clone() }).open().await {
            (opts.log)(&e.message);
        }
    }
    Ok(handle)
}

/// The variable Boat's key is read from, and the one other provider's a cloud build adds.
const PROVIDER_KEY_ENVS: [&str; 1] = ["BOAT_API_KEY"];

/// What the provider says at start: the sweep and the storage line. A host with no key has no account to list and
/// says that where the line would be; a key beside the state opens a place without wiring the host to it, so the
/// line names the file only where none is.
async fn start_reads(rt: &Rc<AssembledRuntime>, log: &LogFn, state_path: &str) {
    sweep_orphans(rt, log).await;
    let saved = crate::envfile::parse_env_file(&crate::envfile::env_file_for(state_path));
    let key_beside = PROVIDER_KEY_ENVS.iter().any(|name| saved.contains_key(*name));
    log(&no_provider_storage_line(if key_beside { None } else { Some(state_path) }));
}

/// `noProviderStorageLine`: what a host wired to no provider says where the storage line would have been.
pub fn no_provider_storage_line(state_path: Option<&str>) -> String {
    match state_path {
        Some(state) => format!("storage: nothing listed; this host is wired to no provider (no key beside {state})"),
        None => "storage: nothing listed; this host is wired to no provider".to_string(),
    }
}

impl HostHandle {
    pub async fn revoke_device(&self, id: &str) -> Result<bool, HostError> {
        self.server.revoke_device(id).await
    }

    /// The runtime first: the sockets it holds are this server's, and closing them is what sends a waiting client the
    /// stopping code. Then the listeners; last, the runtime's own close, so the flush it waits on is the final word.
    pub async fn close(&self) -> Result<(), HostError> {
        self.server.close().await?;
        if let Some(release) = self.relay.borrow_mut().take() {
            release();
        }
        for task in self.accepting.borrow_mut().drain(..) {
            task.abort();
        }
        if let Some(open) = self.front.door.lock().await.take() {
            open.task.abort();
        }
        self.runtime.close().await
    }

    pub fn door_port(&self) -> Option<u16> {
        self.front.door.try_lock().ok().and_then(|held| held.as_ref().map(|open| open.port))
    }
}
