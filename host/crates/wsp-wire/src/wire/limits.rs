// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/wire/limits.ts.

#![allow(unused_imports)]
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::helpers::ReqId;
use crate::types::js_number::JsInt;
use crate::types::{Check, JsNumber, SchemaError};
use wsp_frames::{False, True};

/// /^https?:\/\/[^\s\x00-\x1f\x7f]+$/i
pub const HTTP_URL_RE: &str = "^https?:\\/\\/[^\\s\\x00-\\x1f\\x7f]+$";
pub const HTTP_URL_MAX: i64 = 8192;
pub const EXEC_BODY_MAX: i64 = 16384;
pub const TRANSCRIPT_DIRS_MAX: i64 = 64;
pub const GUEST_TOKEN_MAX: i64 = 512;
pub const GUEST_ARGV_MAX: i64 = 256;
pub const GUEST_CWD_MAX: i64 = 4096;
pub const USAGE_STORES_MAX: i64 = 32;
pub const EXEC_CHUNK_BYTES: i64 = 262144;
pub const EXEC_LINE_MAX_BYTES: i64 = 16777216;
pub const TURN_IDLE_MS: i64 = 600000;
pub const TURN_WORK_TICKS_PER_S: i64 = 5;
pub const AGENT_KEEP_MS: i64 = 1800000;
pub const AGENTS_KEPT: i64 = 6;
pub const KEPT_AGENT_MB: i64 = 400;
pub const KEPT_MEMORY_SHARE: f64 = 0.25;
pub const AGENT_WARM_MS: i64 = 180000;
pub const AGENT_WARM_TYPED_MS: i64 = 30000;
pub const AGENTS_WARM: i64 = 2;
pub const RUN_EXIT_MS: i64 = 10000;
pub const ASIDE_WALL_MS: i64 = 180000;
pub const RUN_STOP_MS: i64 = 2000;
pub const INTERRUPT_GRACE_MS: i64 = 5000;
pub const STEER_JOIN_MS: i64 = 10000;
pub const STOP_REACH_MS: i64 = 3000;
pub const LINK_RETRY_WINDOW_MS: i64 = 60000;
pub const HOST_STOPPING_CLOSE: i64 = 4001;
pub const AUTH_DEADLINE_CLOSE: i64 = 4408;
pub const FORWARD_MAX_PER_TARGET: i64 = 16;

/// A callback port the laptop binds without root, 1024 to 65535 (limits.ts:153): `.int().min(1024).max(65535)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RelayPort(pub JsInt);

impl RelayPort {
    pub fn get(self) -> i64 {
        self.0.get()
    }
}

impl Check for RelayPort {
    fn check(&self) -> Result<(), SchemaError> {
        crate::types::check::num_at_least(self.0.as_f64(), 1024.0)?;
        crate::types::check::num_at_most(self.0.as_f64(), 65535.0)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Int)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PortForwardKind {
    #[serde(rename = "url")]
    Url,
    #[serde(rename = "callback")]
    Callback,
    #[serde(rename = "editor")]
    Editor,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortForward {
    pub workspace_id: String,
    pub port: RelayPort,
    pub started_at: String,
    pub name: String,
    pub kind: PortForwardKind,
}

/// No refinement of its own; checks every nested field that has one.
impl Check for PortForward {
    fn check(&self) -> Result<(), SchemaError> {
        self.port.check().map_err(|e| e.under("port"))?;
        self.kind.check().map_err(|e| e.under("kind"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
        crate::types::keys::req_key::<RelayPort>(obj, "port", &mut out);
        crate::types::keys::req_key::<String>(obj, "startedAt", &mut out);
        crate::types::keys::req_key::<String>(obj, "name", &mut out);
        crate::types::keys::req_key::<PortForwardKind>(obj, "kind", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "workspaceId" => <String as Check>::describe(rest),
                "port" => <RelayPort as Check>::describe(rest),
                "startedAt" => <String as Check>::describe(rest),
                "name" => <String as Check>::describe(rest),
                "kind" => <PortForwardKind as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

// limits.ts:57
// /** How many agent processes a joined computer keeps between turns at once, off its total memory: a 4 GB computer
//  * keeps two, an 8 GB one five, and one under 1.6 GB none. */
// export function agentsKeptOn(memMb: number): number {
//   return Math.min(AGENTS_KEPT, Math.max(0, Math.floor((memMb * KEPT_MEMORY_SHARE) / KEPT_AGENT_MB)));
// }
pub fn agents_kept_on(mem_mb: f64) -> i64 {
    ((mem_mb * KEPT_MEMORY_SHARE / KEPT_AGENT_MB as f64).floor() as i64).clamp(0, AGENTS_KEPT)
}
// limits.ts:101
// /** The wait before dial `attempt`, half a second doubling to ten, with up to a quarter second of jitter so the
//  * turns of one machine do not all come back at the same instant after a blip. */
// export function linkBackoffMs(attempt: number): number {
//   return Math.min(10_000, 500 * 2 ** Math.max(0, attempt - 1)) + Math.floor(Math.random() * 250);
// }
pub fn link_backoff_ms(attempt: i64, random: f64) -> i64 {
    let doubled = 500i64 << attempt.saturating_sub(1).clamp(0, 5);
    doubled.min(10_000) + (random * 250.0).floor() as i64
}
// limits.ts:114
// export function isHttpUrl(url: unknown): url is string {
//   if (typeof url !== "string" || url.length > HTTP_URL_MAX || !HTTP_URL_RE.test(url)) return false;
//   try {
//     new URL(url);
//     return true;
//   } catch {
//     return false;
//   }
// }
pub fn is_http_url(url: &str) -> bool {
    if crate::types::check::utf16_len(url) > HTTP_URL_MAX as usize {
        return false;
    }
    let lower = url.to_ascii_lowercase();
    let rest = match (lower.strip_prefix("https://"), lower.strip_prefix("http://")) {
        (Some(_), _) => &url[8..],
        (None, Some(_)) => &url[7..],
        (None, None) => return false,
    };
    // The url crate's IDNA table drops U+180E from a host where node refuses it, so it is refused here too.
    let plain = !rest.is_empty() && rest.chars().all(|c| !is_js_space(c) && (c as u32) > 0x1f && c != '\x7f' && c != '\u{180e}');
    plain && url::Url::parse(url).is_ok()
}

/// JavaScript's `\s`: the white space and line terminators its trim strips, which the regex above reads.
fn is_js_space(c: char) -> bool {
    crate::types::check::js_trim(c.encode_utf8(&mut [0; 4])).is_empty()
}
// limits.ts:124
// /** Every folder a turn's paths are built from is held to this: absolute, made of what a path is made of, and
//  * never walking up out of itself. A machine can answer with anything, and what it answers lands in a mount and
//  * in the commands a turn runs there, so a home carrying a semicolon, a quote, a backtick, a glob or a `..` is
//  * refused at the one door rather than quoted or resolved at each of twenty places (the paths are quoted too;
//  * this is what keeps a machine from deciding what those paths mean). A space is a path on macOS and stays
//  * allowed, and a name that begins with a dot is a name. The one rule, read by the wire schemas here and by the
//  * ssh read. */
// export function isPlainPath(path: string): boolean {
//   if (!path.startsWith("/") || !/^[A-Za-z0-9 ._+@:,/-]+$/.test(path) || path.includes("//")) return false;
//   return !path.split("/").some(part => part === "." || part === "..");
// }
pub fn is_plain_path(path: &str) -> bool {
    wsp_frames::is_plain_path(path)
}
// limits.ts:136
// /** A name under a folder the reader already holds, never a path of its own: the far side joins one of these onto
//  * a folder here, so a leading slash, an empty part and a part that walks up out of it are refused at the wire
//  * rather than resolved at the other end. The one rule, read by the wire schema here and by the daemon's own. */
// export function isUnderPath(path: string): boolean {
//   return path.length > 0 && !path.startsWith("/") && !path.split("/").some(part => part === "" || part === "." || part === "..");
// }
pub fn is_under_path(path: &str) -> bool {
    !path.is_empty() && !path.starts_with('/') && !path.split('/').any(|part| part.is_empty() || part == "." || part == "..")
}
// limits.ts:143
// /** The host of a URL that passed isHttpUrl (with its port, without userinfo), or undefined when it does not parse: never throws. */
// export function hostOf(url: string): string | undefined {
//   try {
//     return new URL(url).host;
//   } catch {
//     return undefined;
//   }
// }
pub fn host_of(url: &str) -> Option<String> {
    let parsed = url::Url::parse(url).ok()?;
    let host = parsed.host_str().unwrap_or_default();
    Some(match parsed.port() {
        Some(port) => format!("{host}:{port}"),
        None => host.to_string(),
    })
}

/// The URL a sign-in's redirect_uri names, when the URL carries one and both parse.
fn redirect_of(url: &str) -> Option<url::Url> {
    let redirect = url::Url::parse(url).ok()?.query_pairs().find(|(key, _)| key == "redirect_uri")?.1.into_owned();
    url::Url::parse(&redirect).ok()
}

fn is_loopback(target: &url::Url) -> bool {
    matches!(target.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"))
}
// limits.ts:172
// /** Whether a sign-in's page comes back to the machine that asked for it: its redirect_uri names a loopback host,
//  * port or no port (aws registers http://127.0.0.1/oauth/callback bare and binds its port at the time). This is what
//  * tells the two roads apart, since a page that returns to the machine hands the person nothing to carry back. */
// export function redirectsToMachine(url: string): boolean {
//   const target = redirectOf(url);
//   return target !== undefined && LOOPBACK_HOSTS.has(target.hostname);
// }
pub fn redirects_to_machine(url: &str) -> bool {
    redirect_of(url).is_some_and(|target| is_loopback(&target))
}
// limits.ts:180
// /** Which port on the machine that redirect names, for the forward to bind here: the daemon reads it to name the port
//  * on a browser.open. Absent when the page does not come back to the machine at all, when the redirect names no
//  * explicit port, or when the port is one this computer could not bind. */
// export function callbackPortOf(url: string): number | undefined {
//   const target = redirectOf(url);
//   if (target === undefined || !LOOPBACK_HOSTS.has(target.hostname) || target.port === "") return undefined;
//   const port = Number(target.port);
//   return RelayPort.safeParse(port).success ? port : undefined;
// }
pub fn callback_port_of(url: &str) -> Option<u16> {
    let target = redirect_of(url)?;
    if !is_loopback(&target) {
        return None;
    }
    target.port().filter(|port| *port >= 1024)
}

// A word list zod reads with z.enum: serde refuses any other word, so nothing is left to check.
impl Check for PortForwardKind {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
}
