// SPDX-License-Identifier: AGPL-3.0-only
//! The port and address the host listens on, read off its flags, and the lines a port already taken says. Port of the functions of packages/protocol/src/app-ports.ts at
//! a3b870a6e; its shapes and flat constants are wsp_wire::types::app_ports. See spec/app_ports.md.

use serde::Serialize;
use url::Url;

use wsp_wire::types::app_ports::{DEFAULT_PORT, LOOPBACK, WS_PATH};
use wsp_wire::wire::limits::host_of;

use crate::exit::HostError;
use crate::js::{js_trim, number_string};

// app-ports.ts:10
// in wsp_wire::types::app_ports: DEFAULT_PORT

// app-ports.ts:13
// in wsp_wire::types::app_ports: PLACE_PORT_OFFSET

// The port that door answers on for the default port.
// app-ports.ts:15
// in wsp_wire::types::app_ports: DEFAULT_PLACE_PORT

// app-ports.ts:19
// in wsp_wire::types::app_ports: LOOPBACK

// app-ports.ts:23
// in wsp_wire::types::app_ports: WS_PATH

/// `startsWith` with a case-blind ASCII match, the way the TS regexes with the /i flag read a scheme.
fn starts_with_ci(text: &str, prefix: &str) -> bool {
    text.len() >= prefix.len() && text.is_char_boundary(prefix.len()) && text[..prefix.len()].eq_ignore_ascii_case(prefix)
}

/// The scheme before "://", lower-cased, when the text has one.
fn scheme_of(text: &str) -> Option<String> {
    text.find("://").map(|at| text[..at].to_ascii_lowercase())
}

/// `Number(text)`: JavaScript's string to number, with white space trimmed, "" as 0, hex, octal and binary prefixes, and
/// NaN for anything else.
fn js_number(text: &str) -> f64 {
    let t = js_trim(text);
    if t.is_empty() {
        return 0.0;
    }
    for (prefix, radix) in [("0x", 16), ("0X", 16), ("0o", 8), ("0O", 8), ("0b", 2), ("0B", 2)] {
        if let Some(digits) = t.strip_prefix(prefix) {
            let plain = digits.bytes().all(|b| b.is_ascii_alphanumeric());
            return match u64::from_str_radix(digits, radix) {
                Ok(n) if plain => n as f64,
                _ => f64::NAN,
            };
        }
    }
    let (negative, unsigned) = match t.as_bytes()[0] {
        b'-' => (true, &t[1..]),
        b'+' => (false, &t[1..]),
        _ => (false, t),
    };
    if unsigned == "Infinity" {
        return if negative { f64::NEG_INFINITY } else { f64::INFINITY };
    }
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    let (mantissa, exponent) = unsigned.split_at(unsigned.find(['e', 'E']).unwrap_or(unsigned.len()));
    let mantissa_ok = match mantissa.split_once('.') {
        Some((whole, fraction)) => {
            (whole.is_empty() || digits(whole)) && (fraction.is_empty() || digits(fraction)) && (digits(whole) || digits(fraction))
        }
        None => digits(mantissa),
    };
    let exponent_ok = exponent.is_empty() || digits(exponent[1..].trim_start_matches(['+', '-']));
    if !mantissa_ok || !exponent_ok {
        return f64::NAN;
    }
    t.parse::<f64>().unwrap_or(f64::NAN)
}

/// The hostname of a host as URL gives it: the host with its port taken off, brackets kept for an IPv6 literal.
fn hostname_of(host: &str) -> String {
    if host.starts_with('[') {
        host.find(']').map_or(host, |end| &host[..=end]).to_string()
    } else {
        host.split(':').next().unwrap_or_default().to_string()
    }
}

/// `isUrl`: a word that starts with one of the four schemes and "://", in any case.
fn is_url_word(word: &str) -> bool {
    scheme_of(word).is_some_and(|scheme| matches!(scheme.as_str(), "http" | "https" | "ws" | "wss"))
}

/// Whether a word is a name and a port the join screen takes as a bare address.
fn is_bare_address(word: &str) -> bool {
    let port_ok = |p: &str| (1..=5).contains(&p.len()) && p.bytes().all(|b| b.is_ascii_digit());
    if let Some(rest) = word.strip_prefix('[') {
        let Some(end) = rest.find(']') else { return false };
        let inner = &rest[..end];
        !inner.is_empty()
            && inner.bytes().all(|b| b.is_ascii_hexdigit() || b == b':')
            && rest[end + 1..].strip_prefix(':').is_some_and(port_ok)
    } else {
        let Some(colon) = word.find(':') else { return false };
        let name = &word[..colon];
        !name.is_empty() && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-') && port_ok(&word[colon + 1..])
    }
}

/// The WebSocket address of a host at this address: the same authority over ws or wss, with the runtime's path on
/// the end of whatever path the address already carries, which is what a tunnel hostname under a prefix needs. It
/// sits beside WS_PATH because both the command line and a place's own agent turn an address into this one, and a
/// second copy of the rule would let one of them dial a path the runtime does not answer on.
// app-ports.ts:29
// export function wsUrlOf(url: string): string {
//   const parsed = new URL(url);
//   const scheme = parsed.protocol === "https:" || parsed.protocol === "wss:" ? "wss:" : "ws:";
//   const path = parsed.pathname.replace(/\/+$/, "");
//   return `${scheme}//${parsed.host}${path}${WS_PATH}`;
// }
pub fn ws_url_of(url: &str) -> Result<String, HostError> {
    let parsed = Url::parse(url).map_err(|_| HostError::new("Invalid URL"))?;
    let scheme = if matches!(parsed.scheme(), "https" | "wss") { "wss:" } else { "ws:" };
    let host = parsed.host_str().unwrap_or_default();
    let port = parsed.port().map_or(String::new(), |port| format!(":{port}"));
    Ok(format!("{scheme}//{host}{port}{}{WS_PATH}", parsed.path().trim_end_matches('/')))
}

/// An address as a person reads and types it: the host and its port, which is the short form joinAddressOf takes
/// back. The door answers with a whole URL and no screen shows one, since a line carrying `http://` is longer than
/// the row it sits in and the scheme is the one part nobody has a choice about.
// app-ports.ts:39
// export function joinAddressWord(url: string): string {
//   try {
//     return new URL(url).host;
//   } catch {
//     return url;
//   }
// }
pub fn join_address_word(url: &str) -> String {
    host_of(url).unwrap_or_else(|| url.to_string())
}

/// A peer's address as the person on the other screen would write it: an IPv4 address that arrived over a
/// dual-stack listener wears the ::ffff: prefix, which nobody typed. Read here and nowhere else, so the door that
/// shows a peer and the rule that reads one cannot disagree about what the peer is.
// app-ports.ts:50
// export function peerAddress(address: string | undefined): string {
//   return (address ?? "").replace(/^::ffff:/, "");
// }
pub fn peer_address(address: Option<&str>) -> String {
    let address = address.unwrap_or("");
    address.strip_prefix("::ffff:").unwrap_or(address).to_string()
}

/// Whether an address reaches no further than the computer it runs on. This decides whether the page is served
/// with the host token inlined and whether the JSON routes ask for a device token, so it is read once here and
/// nowhere else: two readings would let one road stay open while the other closed. A whole IPv4 literal in 127/8
/// and never a prefix, since this also reads the name a request asked for and `127.evil.example` is a name anyone
/// may register and point at this computer's own port.
// app-ports.ts:59
// export function isLoopback(address: string): boolean {
//   return address === "localhost" || address === "::1" || address === "[::1]" || /^127\.\d{1,3}\.\d{1,3}\.\d{1,3}$/.test(address);
// }
pub fn is_loopback(address: &str) -> bool {
    if matches!(address, "localhost" | "::1" | "[::1]") {
        return true;
    }
    let parts: Vec<&str> = address.split('.').collect();
    parts.len() == 4 && parts[0] == "127" && parts[1..].iter().all(|p| (1..=3).contains(&p.len()) && p.bytes().all(|b| b.is_ascii_digit()))
}

// app-ports.ts:65
// in wsp_wire::types::app_ports: WILDCARD

/// Whether an address is the wildcard, which binds every address this computer answers on, loopback included. A
/// tool on the computer itself dials such a host at loopback; a host on one named address answers only there.
// app-ports.ts:69
// export function isWildcard(address: string): boolean {
//   return address === "0.0.0.0" || address === "::";
// }
pub fn is_wildcard(address: &str) -> bool {
    matches!(address, "0.0.0.0" | "::")
}

/// Whether a word is an address rather than a name: a host is reached at an http, https, ws or wss address, and a
/// word with any of those schemes is one, wherever it is typed (a --host flag, WSP_HOST, a login address).
// app-ports.ts:75
// export function isUrl(word: string): boolean {
//   return /^(https?|wss?):\/\//i.test(word);
// }
pub fn is_url(word: &str) -> bool {
    is_url_word(word)
}

/// The name of the computer in an address a host is served at, or nothing when the word is not one. isUrl takes
/// every address a socket is reached at, the ws and wss ones the runtime's own socket is dialed at included; this is
/// the narrower reading of an address a person is pointed at, and it hands back the name it found, so the check that
/// refuses a malformed address and the alias folded out of a good one are one parse rather than two.
// app-ports.ts:83
// export function servedHostname(word: string): string | undefined {
//   if (!isUrl(word)) return undefined;
//   let parsed: URL;
//   try {
//     parsed = new URL(word);
//   } catch {
//     return undefined;
//   }
//   if (parsed.protocol !== "http:" && parsed.protocol !== "https:") return undefined;
//   return parsed.hostname === "" ? undefined : parsed.hostname;
// }
pub fn served_hostname(word: &str) -> Option<String> {
    if !is_url_word(word) {
        return None;
    }
    let scheme = scheme_of(word)?;
    if scheme != "http" && scheme != "https" {
        return None;
    }
    let hostname = hostname_of(&host_of(word)?);
    (!hostname.is_empty()).then_some(hostname)
}

// What a bare address a person types looks like: a name or an IPv4 address with a port, or an IPv6 literal in its
// brackets with one. The port is asked for because a wsp is reached at one and no default would be right for both
// the app's own and one moved by --port.
// app-ports.ts:98
// private in the TS: port it as a private helper
// const BARE_ADDRESS = /^(\[[0-9a-fA-F:]+\]|[a-zA-Z0-9.-]+):\d{1,5}$/;

/// The address typed on the join screen, as the road that dials it wants it: what a person reads off the other
/// computer is an authority (`192.168.1.20:4400`), which becomes an http address; an address that already carries
/// http or https is itself; anything else, a bare name with no port or a socket address, is nothing. One reading,
/// so the field that refuses a word and the dial that follows it cannot disagree about what an address is.
// app-ports.ts:104
// export function joinAddressOf(typed: string): string | undefined {
//   const word = typed.trim();
//   if (/^https?:\/\//i.test(word)) return servedHostname(word) === undefined ? undefined : word;
//   return BARE_ADDRESS.test(word) ? `http://${word}` : undefined;
// }
pub fn join_address_of(typed: &str) -> Option<String> {
    let word = js_trim(typed);
    if starts_with_ci(word, "http://") || starts_with_ci(word, "https://") {
        return served_hostname(word).map(|_| word.to_string());
    }
    is_bare_address(word).then(|| format!("http://{word}"))
}

/// An address and a port as the authority of a URL: an IPv6 literal needs brackets and everything else is itself.
/// The one rule, so the address wsp host pair prints and the one every local tool dials are spelled the same way.
// app-ports.ts:112
// export function authority(address: string, port: number): string {
//   const bracketed = address.includes(":") && !address.startsWith("[") ? `[${address}]` : address;
//   return `${bracketed}:${port}`;
// }
pub fn authority(address: &str, port: f64) -> String {
    let bracketed = if address.contains(':') && !address.starts_with('[') { format!("[{address}]") } else { address.to_string() };
    format!("{bracketed}:{}", number_string(port))
}

/// Where a host a relay carries answers, written from the name the relay gave it: the one home for that spelling,
/// so a line that prints the address and a turn that dials it name the same one. A tunnel is TLS from end to end,
/// which is what makes the scheme a rule rather than a choice.
// app-ports.ts:120
// export function relayUrlOf(hostname: string): string {
//   return `https://${hostname}`;
// }
pub fn relay_url_of(hostname: &str) -> String {
    format!("https://{hostname}")
}

/// The one line a host binding beyond this computer prints as it starts, so nobody learns from a stranger that the
/// page was reachable.
// app-ports.ts:126
// export function listenBeyondLoopbackLine(address: string): string {
//   return `listening on ${address}: anyone who can reach this computer there can load the page, and pairing is the gate. Run wsp host pair for a code, and wsp host devices to see who took one.`;
// }
pub fn listen_beyond_loopback_line(address: &str) -> String {
    format!("listening on {address}: anyone who can reach this computer there can load the page, and pairing is the gate. Run wsp host pair for a code, and wsp host devices to see who took one.")
}

/// The line a host bound to one address beyond loopback prints as it starts: a thread on this computer reaches the
/// host at its loopback, so on that bind it is handed no token and starts nothing under itself.
// app-ports.ts:132
// export function loopbackThreadsLine(address: string): string {
//   return `listening on ${address} alone: threads on this computer start children only when the host also listens on loopback, so run wsp up with --listen 0.0.0.0 or 127.0.0.1 for them.`;
// }
pub fn loopback_threads_line(address: &str) -> String {
    format!("listening on {address} alone: threads on this computer start children only when the host also listens on loopback, so run wsp up with --listen 0.0.0.0 or 127.0.0.1 for them.")
}

// app-ports.ts:136
// export interface AppPorts {
//   /** The port the app and the runtime's WebSocket are served on. */
//   port: number;
// }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct AppPorts {
    pub port: u16,
}

/// A port with whether a person named it: a taken port somebody asked for is a refusal, where a taken default is
/// one wsp steps over.
// app-ports.ts:143
// export interface PortsAsked extends AppPorts {
//   named: boolean;
// }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct PortsAsked {
    pub port: u16,
    pub named: bool,
}

/// The port with the address to bind it on, which is what a command line asked for and what wsp up serves; the
/// port stepping reads the port alone, so it takes the narrower shape above.
// app-ports.ts:149
// export interface ListenAsked extends PortsAsked {
//   /** The address the host binds, as `--listen` named it or this computer alone. */
//   address: string;
// }
/// `port` is the TS `Number(flags.port)`, so a flag that is no number reads NaN: f64 keeps it.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ListenAsked {
    pub port: f64,
    pub named: bool,
    pub address: String,
}

/// The one rule for the port and the address, read by wsp up and wsp init: the port as named or the default, and
/// the address as named or this computer alone. Port 0 asks the operating system for any free port.
// app-ports.ts:156
// export function portsAsked(flags: { port?: string; listen?: string }): ListenAsked {
//   return {
//     port: flags.port !== undefined ? Number(flags.port) : DEFAULT_PORT,
//     named: flags.port !== undefined,
//     address: flags.listen !== undefined && flags.listen !== "" ? flags.listen : LOOPBACK,
//   };
// }
/// `port` and `listen` are the flags as typed.
pub fn ports_asked(port: Option<&str>, listen: Option<&str>) -> ListenAsked {
    ListenAsked {
        port: port.map_or(DEFAULT_PORT as f64, js_number),
        named: port.is_some(),
        address: listen.filter(|l| !l.is_empty()).unwrap_or(LOOPBACK).to_string(),
    }
}

/// What holds a port wsp wanted: a wsp host, named by the state file it serves, the process this computer could
/// name, or nothing it could name at all.
// app-ports.ts:166
// export type PortProcess = { command: string; pid: number };
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PortProcess {
    pub command: String,
    pub pid: u32,
}

// app-ports.ts:167
// export type PortHolder = { statePath: string } | PortProcess | undefined;
/// The TS PortHolder; its `undefined` is `Option<&PortHolder>` None where a function takes one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PortHolder {
    Host { state_path: String },
    Process(PortProcess),
}

/// Who holds a port, in the words every line about it uses.
// app-ports.ts:170
// export function portHolderWords(holder: PortHolder): string {
//   if (holder === undefined) return "another process";
//   return "statePath" in holder ? `the host serving ${holder.statePath}` : `${holder.command} (pid ${holder.pid})`;
// }
pub fn port_holder_words(holder: Option<&PortHolder>) -> String {
    match holder {
        None => "another process".to_string(),
        Some(PortHolder::Host { state_path }) => format!("the host serving {state_path}"),
        Some(PortHolder::Process(process)) => format!("{} (pid {})", process.command, process.pid),
    }
}

/// The sentence for a port a run was told to take and could not have.
// app-ports.ts:176
// export function portTakenLine(port: number, holder: PortHolder): string {
//   return `Port ${port} is in use on this computer by ${portHolderWords(holder)}.`;
// }
pub fn port_taken_line(port: f64, holder: Option<&PortHolder>) -> String {
    format!("Port {} is in use on this computer by {}.", number_string(port), port_holder_words(holder))
}

/// What a run says when the default port was taken and it stepped to a free one: the port it serves on, then the
/// port it stepped over and who holds it, so a second setup beside a running host reads as a move and not a fault.
// app-ports.ts:182
// export function portsPickedLine(ports: AppPorts, taken: number, holder: PortHolder): string {
//   return `Serving on ${ports.port}; ${taken} is held by ${portHolderWords(holder)}.`;
// }
pub fn ports_picked_line(ports: &AppPorts, taken: f64, holder: Option<&PortHolder>) -> String {
    format!("Serving on {}; {} is held by {}.", ports.port, number_string(taken), port_holder_words(holder))
}

// app-ports.ts:187
// in wsp_wire::types::app_ports: PORT_TAKEN_REFUSAL

/// The same last line where the run found a free port above the held one: the line to type, spelled out, since a
/// person whose port is taken is guessing at numbers otherwise.
// app-ports.ts:191
// export function portInsteadLine(ports: AppPorts): string {
//   return `Nothing was booted. The next free port is ${ports.port}: wsp up --port ${ports.port}. Or stop what holds the one you named.`;
// }
pub fn port_instead_line(ports: &AppPorts) -> String {
    format!("Nothing was booted. The next free port is {0}: wsp up --port {0}. Or stop what holds the one you named.", ports.port)
}

/// Which state file a run sets up and the flag that starts a fresh one instead: a run on a file that already
/// carries a sealed golden offers the upgrade rather than a first setup, so a showcase or a second account is one
/// flag rather than a surprise. Said before anything is read, and again in every refusal that stops the run.
// app-ports.ts:198
// export function stateFileLine(statePath: string): string {
//   return `Setting up ${statePath}; --state <path> starts a fresh setup instead.`;
// }
pub fn state_file_line(state_path: &str) -> String {
    format!("Setting up {state_path}; --state <path> starts a fresh setup instead.")
}
