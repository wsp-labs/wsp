// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/slate-image-fetch.ts at 65f6b89c7. Spec: spec/slate_images.md (rules 7-10).
//!
//! A slate's remote image, fetched by the host for the window. Every address a name resolves to is checked before
//! the socket opens and the socket dials the address checked; a redirect is followed only within the domain the
//! person allowed and checked again; no cookie or login goes with it; the bytes stop at the cap as they arrive.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::rc::Rc;
use std::str::FromStr;
use std::time::Duration;

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use futures::future::LocalBoxFuture;
use futures::FutureExt;
use serde_json::json;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use url::Url;

use wsp_words::attachments::{image_type_of, IMAGE_TYPES};
use wsp_words::js::{js_round, number_string};

use crate::pending::slate::{
    slate_address_refused, slate_image_too_big, slate_looks_svg, slate_not_an_image, slate_problem, SLATE_IMAGE_MAX_BYTES,
};
use crate::pending::wire::SlatesImageAnswer;

pub const REDIRECTS_MAX: u32 = 5;
pub const FETCH_MS: u64 = 15_000;

const LOCAL_WORDS: &str = "is this computer or its network";

/// IPv4 blocks: where this computer and its network answer (slate-image-fetch.ts:20).
const LOCAL_V4: [(u32, u32); 11] = [
    (0x0000_0000, 8),
    (0x0a00_0000, 8),
    (0x6440_0000, 10),
    (0x7f00_0000, 8),
    (0xa9fe_0000, 16),
    (0xac10_0000, 12),
    (0xc000_0000, 24),
    (0xc0a8_0000, 16),
    (0xc612_0000, 15),
    (0xe000_0000, 4),
    (0xf000_0000, 4),
];

/// IPv6 blocks, the NAT64 prefix a network picks for itself among them (slate-image-fetch.ts:23).
const LOCAL_V6: [(u128, u32); 8] = [
    (0, 128),
    (1, 128),
    (0xfc00 << 112, 7),
    (0xfe80 << 112, 10),
    (0xfec0 << 112, 10),
    (0xff00 << 112, 8),
    (0x0100 << 112, 64),
    (0x0064_ff9b_0001 << 80, 48),
];

fn in_v4(addr: u32, net: u32, bits: u32) -> bool {
    let mask = u32::MAX << (32 - bits);
    addr & mask == net & mask
}

fn in_v6(addr: u128, net: u128, bits: u32) -> bool {
    let mask = u128::MAX << (128 - bits);
    addr & mask == net & mask
}

/// Node's `isIP`: 4 for a dotted IPv4 address, 6 for an IPv6 one (an IPv6 zone such as `%eth0` allowed), else 0.
fn ip_family(text: &str) -> u8 {
    if Ipv4Addr::from_str(text).is_ok() {
        return 4;
    }
    let (addr, zone) = match text.split_once('%') {
        Some((addr, zone)) => (addr, Some(zone)),
        None => (text, None),
    };
    if zone.is_some_and(|z| z.is_empty() || !z.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '.' | ':'))) {
        return 0;
    }
    if Ipv6Addr::from_str(addr).is_ok() {
        6
    } else {
        0
    }
}

fn v6_of(text: &str) -> Option<Ipv6Addr> {
    Ipv6Addr::from_str(text.split('%').next().unwrap_or("")).ok()
}

/// The IPv4 address an IPv6 one carries inside it, where it lands: the old compatible form, mapped, the translated
/// form, the well-known NAT64 prefix and 6to4 (slate-image-fetch.ts:353).
fn carried(v6: Ipv6Addr) -> Option<Ipv4Addr> {
    let g = v6.segments();
    let zeros = |n: usize| g[..n].iter().all(|x| *x == 0);
    let v4 = |hi: u16, lo: u16| Ipv4Addr::new((hi >> 8) as u8, hi as u8, (lo >> 8) as u8, lo as u8);
    if zeros(6) && !(g[6] == 0 && g[7] <= 1) {
        return Some(v4(g[6], g[7]));
    }
    if zeros(5) && g[5] == 0xffff {
        return Some(v4(g[6], g[7]));
    }
    if zeros(4) && g[4] == 0xffff && g[5] == 0 {
        return Some(v4(g[6], g[7]));
    }
    if g[0] == 0x64 && g[1] == 0xff9b && g[2..6].iter().all(|x| *x == 0) {
        return Some(v4(g[6], g[7]));
    }
    if g[0] == 0x2002 {
        return Some(v4(g[1], g[2]));
    }
    None
}

/// Why an address is one this host will not dial for an image, or `None` for one on the open internet (rule 7).
pub fn local_address(address: &str) -> Option<String> {
    match ip_family(address) {
        0 => Some("is not an address".to_string()),
        4 => {
            let addr = u32::from(Ipv4Addr::from_str(address).ok()?);
            LOCAL_V4.iter().any(|(net, bits)| in_v4(addr, *net, *bits)).then(|| LOCAL_WORDS.to_string())
        }
        _ => {
            let v6 = v6_of(address)?;
            if let Some(inner) = carried(v6) {
                return local_address(&inner.to_string());
            }
            let addr = u128::from(v6);
            LOCAL_V6.iter().any(|(net, bits)| in_v6(addr, *net, *bits)).then(|| LOCAL_WORDS.to_string())
        }
    }
}

/// One address a name resolves to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LookedUp {
    pub address: String,
    /// 4 or 6.
    pub family: u8,
}

/// slate-image-fetch.ts:376 FetchRoad.
pub trait FetchRoad {
    /// Every address a name resolves to; a failure reads as none.
    fn lookup(&self, host: &str) -> LocalBoxFuture<'static, Result<Vec<LookedUp>, String>>;
    /// Why an address may not be dialled, `None` where it may.
    fn refuses(&self, address: &str) -> Option<String>;
    fn timeout_ms(&self) -> u64;
}

/// The system resolver, every address in the order it gives them (slate-image-fetch.ts:384).
struct SystemRoad;

impl FetchRoad for SystemRoad {
    fn lookup(&self, host: &str) -> LocalBoxFuture<'static, Result<Vec<LookedUp>, String>> {
        let host = host.to_string();
        async move {
            let found = tokio::net::lookup_host((host.as_str(), 0)).await.map_err(|e| e.to_string())?;
            Ok(found.map(|a| LookedUp { address: a.ip().to_string(), family: if a.is_ipv4() { 4 } else { 6 } }).collect())
        }
        .boxed_local()
    }

    fn refuses(&self, address: &str) -> Option<String> {
        local_address(address)
    }

    fn timeout_ms(&self) -> u64 {
        FETCH_MS
    }
}

/// The system resolver, `local_address`, 15 s.
pub fn fetch_road() -> Rc<dyn FetchRoad> {
    Rc::new(SystemRoad)
}

/// A slate's answer that refuses or fails with a code (R903 unless named).
pub(crate) fn problem_answer(problem: crate::pending::slate::SlateProblem) -> SlatesImageAnswer {
    json!({ "problem": problem })
}

fn refused(href: &str, why: &str) -> SlatesImageAnswer {
    problem_answer(slate_address_refused(href, why))
}

fn failed(message: &str) -> SlatesImageAnswer {
    problem_answer(slate_problem("R903", message, None))
}

/// The one address a hop dials: the name's every address checked, the first then pinned (slate-image-fetch.ts:406).
async fn vetted(url: &Url, road: &dyn FetchRoad) -> Result<LookedUp, String> {
    let host = url.host_str().unwrap_or("").trim_start_matches('[').trim_end_matches(']').to_string();
    let family = ip_family(&host);
    let found: Vec<LookedUp> =
        if family == 0 { road.lookup(&host).await.unwrap_or_default() } else { vec![LookedUp { address: host.clone(), family }] };
    if found.is_empty() {
        return Err(format!("names {host}, which resolves to nothing"));
    }
    for a in &found {
        if let Some(why) = road.refuses(&a.address) {
            return Err(format!("resolves to {}, which {why}; a slate fetches images from the internet alone", a.address));
        }
    }
    Ok(found[0].clone())
}

struct Response {
    status: u16,
    headers: Vec<(String, String)>,
    reader: BufReader<TcpStream>,
}

impl Response {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str())
    }
}

/// Sends one GET to the address the hop checked, on the socket it opens, and reads the status and the headers.
async fn send_get(url: &Url, to: &LookedUp) -> Result<Response, String> {
    if url.scheme() == "https" {
        return Err("https is not linked into this build".to_string());
    }
    let ip = IpAddr::from_str(&to.address).map_err(|e| e.to_string())?;
    let port = url.port_or_known_default().unwrap_or(80);
    let mut stream = TcpStream::connect(SocketAddr::new(ip, port)).await.map_err(|e| e.to_string())?;
    let host = url.host_str().unwrap_or("");
    let host_header = match url.port() {
        Some(p) => format!("{host}:{p}"),
        None => host.to_string(),
    };
    let path = match url.query() {
        Some(q) => format!("{}?{q}", url.path()),
        None => url.path().to_string(),
    };
    let accept: Vec<&str> = IMAGE_TYPES.iter().map(|t| t.0).collect();
    let request = format!(
        "GET {path} HTTP/1.1\r\nhost: {host_header}\r\nconnection: close\r\nuser-agent: wsp\r\naccept: {}\r\n\r\n",
        accept.join(",")
    );
    stream.write_all(request.as_bytes()).await.map_err(|e| e.to_string())?;
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line).await.map_err(|e| e.to_string())?;
    let status =
        line.split_whitespace().nth(1).and_then(|s| s.parse::<u16>().ok()).ok_or_else(|| "the answer had no status".to_string())?;
    let mut headers = Vec::new();
    loop {
        line.clear();
        let read = reader.read_line(&mut line).await.map_err(|e| e.to_string())?;
        let text = line.trim_end_matches(['\r', '\n']);
        if read == 0 || text.is_empty() {
            break;
        }
        if let Some((name, value)) = text.split_once(':') {
            headers.push((name.trim().to_lowercase(), value.trim().to_string()));
        }
    }
    Ok(Response { status, headers, reader })
}

enum Body {
    Bytes(Vec<u8>),
    Over,
}

/// Reads the body up to the cap and one byte more, decoding a chunked answer (slate-image-fetch.ts:226 `body`).
async fn read_body(res: &mut Response, cap: usize) -> Result<Body, String> {
    let io = |e: std::io::Error| e.to_string();
    let chunked = res.header("transfer-encoding").is_some_and(|v| v.to_lowercase().contains("chunked"));
    let mut out: Vec<u8> = Vec::new();
    if chunked {
        loop {
            let mut line = String::new();
            if res.reader.read_line(&mut line).await.map_err(io)? == 0 {
                return Err("the answer stopped part way".to_string());
            }
            let size_text = line.split(';').next().unwrap_or("").trim();
            let size = usize::from_str_radix(size_text, 16).map_err(|_| "a chunk had no size".to_string())?;
            if size == 0 {
                loop {
                    let mut trailer = String::new();
                    if res.reader.read_line(&mut trailer).await.map_err(io)? == 0 || trailer.trim_end_matches(['\r', '\n']).is_empty() {
                        break;
                    }
                }
                break;
            }
            if out.len().saturating_add(size) > cap {
                return Ok(Body::Over);
            }
            let start = out.len();
            out.resize(start + size, 0);
            res.reader.read_exact(&mut out[start..]).await.map_err(io)?;
            let mut end = String::new();
            res.reader.read_line(&mut end).await.map_err(io)?;
        }
    } else {
        let mut remaining = res.header("content-length").and_then(|v| v.parse::<usize>().ok()).unwrap_or(usize::MAX);
        let mut chunk = vec![0u8; 64 * 1024];
        while remaining > 0 {
            let want = remaining.min(chunk.len());
            let n = res.reader.read(&mut chunk[..want]).await.map_err(io)?;
            if n == 0 {
                break;
            }
            if out.len() + n > cap {
                return Ok(Body::Over);
            }
            out.extend_from_slice(&chunk[..n]);
            remaining -= n.min(remaining);
        }
    }
    Ok(Body::Bytes(out))
}

/// The answer of one fetch once it is read to its end: a shown image, a refusal, or a failure of the request itself.
async fn fetch_hops(href: &str, domain: &str, road: Rc<dyn FetchRoad>) -> Result<SlatesImageAnswer, String> {
    let cap = SLATE_IMAGE_MAX_BYTES as usize;
    let mut url = Url::parse(href).map_err(|_| "Invalid URL".to_string())?;
    for hops in 0u32.. {
        let scheme = url.scheme().to_string();
        if scheme != "http" && scheme != "https" {
            return Ok(refused(href, &format!("leads to {scheme}: by a redirect, which wsp does not follow")));
        }
        let host = url.host_str().unwrap_or("").to_string();
        if host.to_lowercase() != domain {
            return Ok(refused(href, &format!("redirects to {host}, a domain other than the {domain} you allowed")));
        }
        if !url.username().is_empty() || url.password().is_some_and(|p| !p.is_empty()) {
            return Ok(refused(href, "redirects to an address with a login, which wsp never sends"));
        }
        let to = match vetted(&url, road.as_ref()).await {
            Ok(to) => to,
            Err(why) => return Ok(refused(url.as_str(), &why)),
        };
        let mut res = send_get(&url, &to).await?;
        let status = res.status;
        let location = res.header("location").map(String::from);
        if (300..400).contains(&status) {
            if let Some(location) = location {
                if hops >= REDIRECTS_MAX {
                    return Ok(refused(href, &format!("redirects more than {REDIRECTS_MAX} times")));
                }
                url = url.join(&location).map_err(|_| "Invalid URL".to_string())?;
                continue;
            }
        }
        if !(200..300).contains(&status) {
            return Ok(failed(&format!("{domain} answered {status} for {href}")));
        }
        let declared = res.header("content-length").and_then(|v| v.trim().parse::<f64>().ok());
        if declared.is_some_and(|n| n.is_finite() && n > SLATE_IMAGE_MAX_BYTES as f64) {
            return Ok(problem_answer(slate_image_too_big(href, declared.map(|n| n as u64))));
        }
        let bytes = match read_body(&mut res, cap).await? {
            Body::Over => return Ok(problem_answer(slate_image_too_big(href, None))),
            Body::Bytes(bytes) => bytes,
        };
        let version = res.header("etag").or_else(|| res.header("last-modified")).map(String::from);
        let Some(media_type) = image_type_of(&bytes) else {
            return Ok(problem_answer(slate_not_an_image(href, slate_looks_svg(url.path(), &bytes))));
        };
        let mut answer = json!({ "mediaType": media_type, "bytes": STANDARD.encode(&bytes) });
        if let Some(version) = version {
            answer["version"] = json!(version);
        }
        return Ok(answer);
    }
    Err("no hop".to_string())
}

/// Fetches an image whose domain the person allowed (rule 9).
pub async fn fetch_slate_image(href: &str, domain: &str, road: Option<Rc<dyn FetchRoad>>) -> SlatesImageAnswer {
    let road = road.unwrap_or_else(fetch_road);
    let wait_ms = road.timeout_ms();
    match tokio::time::timeout(Duration::from_millis(wait_ms), fetch_hops(href, domain, road.clone())).await {
        Ok(Ok(answer)) => answer,
        Ok(Err(message)) => failed(&format!("{href} could not be read: {message}")),
        Err(_) => {
            let seconds = number_string(js_round(wait_ms as f64 / 1000.0));
            failed(&format!("{domain} did not answer within {seconds} s for {href}"))
        }
    }
}
