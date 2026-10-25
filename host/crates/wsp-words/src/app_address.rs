// SPDX-License-Identifier: AGPL-3.0-only
//! The app's addresses: the hash a thread, a workspace or a setting is opened by, and the pairing code a first open carries. Port of the functions of packages/protocol/src/app-address.ts at
//! a3b870a6e; its shapes and flat constants are wsp_wire::types::app_address. See spec/app_address.md.

use serde::Serialize;

/// The one address rule the app and the host share: which workspace a page
/// opens on, which thread of it or of its subagents, and the screen a
/// workspace's next thread is written on. The address is the app's one record of what a person is
/// reading, so it is written on every pick and read on every refresh; wsp init
/// writes the workspace form after its first fork, and a thread row's
/// copy-link action the thread form. A wsp:// link the desktop takes from the
/// system is read here too, down to the kind and id it names. One writer and
/// one reader live here, so no surface grows a parser of its own.
// app-address.ts:11
const PREFIX: &str = "#w/";

// app-address.ts:12
const THREAD: &str = "/t/";

/// A subagent of the thread, by the call that launched it, after the thread's own segment.
// app-address.ts:14
const SUBAGENT: &str = "/a/";

// app-address.ts:15
const NEW: &str = "/new";

/// The pairing code wsp init minted for the browser it opens, as the last segment of the hash: spent on the page's
/// first paint and written back out of the address, so what the person is reading never carries it.
// app-address.ts:18
const CODE: &str = "/c/";

/// The same code on a page opened on no workspace.
// app-address.ts:20
const CODE_ALONE: &str = "#c/";

/// What a page's address names.
// app-address.ts:23
// export interface AppAddress {
//   readonly workspaceId: string;
//   /** A thread of it, when the address names one. */
//   readonly threadId?: string;
//   /** Whether it names the screen the workspace's next thread is written on, which has no thread of its own yet. */
//   readonly fresh?: boolean;
//   /** One of the thread's own subagents, by the id of the call that launched it (its lines' parentToolUseId); only
//    * ever beside a thread, whose transcript holds those lines. */
//   readonly subagent?: string;
// }
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppAddress {
    pub workspace_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fresh: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subagent: Option<String>,
}

/// `encodeURIComponent`: the unreserved marks and ASCII letters and digits stay, every other UTF-8 byte is "%XX".
fn encode_uri_component(text: &str) -> String {
    let mut out = String::new();
    for b in text.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// `decodeURIComponent`; None where JavaScript throws URIError (a bad escape, or bytes that are not UTF-8).
fn decode_uri_component(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        if bytes[at] == b'%' {
            let hex = bytes.get(at + 1..at + 3)?;
            if !hex.iter().all(u8::is_ascii_hexdigit) {
                return None;
            }
            out.push(u8::from_str_radix(std::str::from_utf8(hex).ok()?, 16).ok()?);
            at += 3;
        } else {
            out.push(bytes[at]);
            at += 1;
        }
    }
    String::from_utf8(out).ok()
}

/// `pairingCodeOf`, with a malformed escape told apart from a hash that carries no code: Err where the TS throws.
fn pairing_of(hash: &str) -> Result<Option<PairingCode>, ()> {
    let alone = hash.starts_with(CODE_ALONE);
    let cut = if alone {
        0
    } else if hash.starts_with(PREFIX) {
        match hash.rfind(CODE) {
            Some(cut) => cut,
            None => return Ok(None),
        }
    } else {
        return Ok(None);
    };
    let marker = if alone { CODE_ALONE.len() } else { CODE.len() };
    let code = decode_uri_component(&hash[cut + marker..]).ok_or(())?;
    Ok(if code.is_empty() { None } else { Some(PairingCode { code, rest: hash[..cut].to_string() }) })
}

/// The one id rule a link's id meets: letters and digits first, then letters, digits, dashes and underscores, 128 at most.
fn link_id_ok(id: &str) -> bool {
    let bytes = id.as_bytes();
    (1..=128).contains(&bytes.len())
        && bytes[0].is_ascii_alphanumeric()
        && bytes[1..].iter().all(|b| b.is_ascii_alphanumeric() || *b == b'_' || *b == b'-')
}

fn link_kind_of(kind: &str) -> Option<LinkKind> {
    match kind {
        "thread" => Some(LinkKind::Thread),
        "workspace" => Some(LinkKind::Workspace),
        "project" => Some(LinkKind::Project),
        "settings" => Some(LinkKind::Settings),
        _ => None,
    }
}

fn link_kind_word(kind: LinkKind) -> &'static str {
    match kind {
        LinkKind::Thread => "thread",
        LinkKind::Workspace => "workspace",
        LinkKind::Project => "project",
        LinkKind::Settings => "settings",
    }
}

/// `linkTarget`: a kind the link names and an id that passes the id rule, or nothing.
fn link_target(kind: &str, id: &str) -> Option<LinkTarget> {
    let kind = link_kind_of(kind)?;
    link_id_ok(id).then(|| LinkTarget { kind, id: id.to_string() })
}

/// The hash that opens the app on a workspace.
// app-address.ts:35
// export const workspaceHash = (workspaceId: string): string => `${PREFIX}${encodeURIComponent(workspaceId)}`;
pub fn workspace_hash(workspace_id: &str) -> String {
    format!("{PREFIX}{}", encode_uri_component(workspace_id))
}

/// The hash for an address: a workspace, one thread of it or a subagent of that thread, or its next thread's screen.
// app-address.ts:38
// export function appHash(address: AppAddress): string {
//   const base = workspaceHash(address.workspaceId);
//   if (address.threadId !== undefined) {
//     const thread = `${base}${THREAD}${encodeURIComponent(address.threadId)}`;
//     return address.subagent === undefined ? thread : `${thread}${SUBAGENT}${encodeURIComponent(address.subagent)}`;
//   }
//   return address.fresh === true ? `${base}${NEW}` : base;
// }
pub fn app_hash(address: &AppAddress) -> String {
    let base = workspace_hash(&address.workspace_id);
    if let Some(thread_id) = &address.thread_id {
        let thread = format!("{base}{THREAD}{}", encode_uri_component(thread_id));
        return match &address.subagent {
            None => thread,
            Some(subagent) => format!("{thread}{SUBAGENT}{}", encode_uri_component(subagent)),
        };
    }
    if address.fresh == Some(true) { format!("{base}{NEW}") } else { base }
}

/// The hash wsp init opens the app on: the workspace it made, or none, with the code that lets the browser in.
// app-address.ts:48
// export function openingHash(code: string, workspaceId?: string): string {
//   return `${workspaceId === undefined ? CODE_ALONE : `${workspaceHash(workspaceId)}${CODE}`}${encodeURIComponent(code)}`;
// }
pub fn opening_hash(code: &str, workspace_id: Option<&str>) -> String {
    match workspace_id {
        None => format!("{CODE_ALONE}{}", encode_uri_component(code)),
        Some(workspace_id) => format!("{}{CODE}{}", workspace_hash(workspace_id), encode_uri_component(code)),
    }
}

/// The pairing code a hash carries and the hash left once it is taken out, or nothing when it carries none.
// app-address.ts:53
// export function pairingCodeOf(hash: string): { code: string; rest: string } | undefined {
//   const alone = hash.startsWith(CODE_ALONE);
//   const cut = alone ? 0 : hash.startsWith(PREFIX) ? hash.lastIndexOf(CODE) : -1;
//   if (cut === -1) return undefined;
//   const code = decodeURIComponent(hash.slice(cut + (alone ? CODE_ALONE.length : CODE.length)));
//   return code === "" ? undefined : { code, rest: hash.slice(0, cut) };
// }
/// The TS `{ code, rest }`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PairingCode {
    pub code: String,
    pub rest: String,
}

pub fn pairing_code_of(hash: &str) -> Option<PairingCode> {
    pairing_of(hash).ok().flatten()
}

/// What a hash names, or nothing: every other hash is the app's own (#gallery) or none at all. A code riding the
/// end of it is not part of what it names.
// app-address.ts:63
// export function addressFromHash(hash: string): AppAddress | undefined {
//   const named = pairingCodeOf(hash)?.rest ?? hash;
//   if (!named.startsWith(PREFIX)) return undefined;
//   const rest = named.slice(PREFIX.length);
//   const cut = rest.indexOf(THREAD);
//   const fresh = cut === -1 && rest.endsWith(NEW);
//   const workspaceId = decodeURIComponent(fresh ? rest.slice(0, -NEW.length) : cut === -1 ? rest : rest.slice(0, cut));
//   if (workspaceId === "") return undefined;
//   const after = cut === -1 ? "" : rest.slice(cut + THREAD.length);
//   const split = after.indexOf(SUBAGENT);
//   const threadId = decodeURIComponent(split === -1 ? after : after.slice(0, split));
//   const subagent = split === -1 || threadId === "" ? "" : decodeURIComponent(after.slice(split + SUBAGENT.length));
//   return { workspaceId, ...(threadId === "" ? {} : { threadId }), ...(subagent === "" ? {} : { subagent }), ...(fresh ? { fresh: true } : {}) };
// }
pub fn address_from_hash(hash: &str) -> Option<AppAddress> {
    let named = match pairing_of(hash) {
        Err(()) => return None,
        Ok(Some(pairing)) => pairing.rest,
        Ok(None) => hash.to_string(),
    };
    let rest = named.strip_prefix(PREFIX)?;
    let cut = rest.find(THREAD);
    let fresh = cut.is_none() && rest.ends_with(NEW);
    let workspace_part = match cut {
        _ if fresh => &rest[..rest.len() - NEW.len()],
        None => rest,
        Some(cut) => &rest[..cut],
    };
    let workspace_id = decode_uri_component(workspace_part)?;
    if workspace_id.is_empty() {
        return None;
    }
    let after = cut.map_or("", |cut| &rest[cut + THREAD.len()..]);
    let split = after.find(SUBAGENT);
    let thread_id = decode_uri_component(split.map_or(after, |split| &after[..split]))?;
    let subagent = match split {
        Some(split) if !thread_id.is_empty() => Some(decode_uri_component(&after[split + SUBAGENT.len()..])?),
        _ => None,
    };
    let thread_id = Some(thread_id).filter(|t| !t.is_empty());
    let subagent = subagent.filter(|s| !s.is_empty());
    Some(AppAddress { workspace_id, thread_id, fresh: fresh.then_some(true), subagent })
}

// What a wsp:// link can name. A link opens something and never does anything to it: an act a link could start
// would be one any page or app on the computer could start by handing the shell a url.
// app-address.ts:80
// in wsp_wire::types::app_address: LINK_KINDS

// app-address.ts:81
// export type LinkKind = (typeof LINK_KINDS)[number];
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LinkKind {
    Thread,
    Workspace,
    Project,
    Settings,
}

// app-address.ts:82
// export interface LinkTarget {
//   readonly kind: LinkKind;
//   /** The thread's, workspace's or project's id, or the Settings group's. */
//   readonly id: string;
// }
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct LinkTarget {
    pub kind: LinkKind,
    pub id: String,
}

// app-address.ts:88
const LINK_SCHEME: &str = "wsp://";

// app-address.ts:89
const LINK_PREFIX: &str = "#open/";

// An id as wsp mints them: letters, digits, dashes and underscores, nothing a path or a query could hide behind.
// app-address.ts:91
// private in the TS: port it as a private helper
// const LINK_ID = /^[A-Za-z0-9][A-Za-z0-9_-]{0,127}$/;

// app-address.ts:93
// private in the TS: port it as a private helper
// function linkTarget(kind: string, id: string): LinkTarget | undefined {
//   if (!(LINK_KINDS as readonly string[]).includes(kind) || !LINK_ID.test(id)) return undefined;
//   return { kind: kind as LinkKind, id };
// }

/// What a wsp:// link names, read as `wsp://<kind>/<id>` and nothing looser: no query, no fragment, no second
/// segment, no escapes. Anything else names nothing.
// app-address.ts:100
// export function addressFromLink(url: string): LinkTarget | undefined {
//   if (url.slice(0, LINK_SCHEME.length).toLowerCase() !== LINK_SCHEME) return undefined;
//   const rest = url.slice(LINK_SCHEME.length);
//   const parts = (rest.endsWith("/") ? rest.slice(0, -1) : rest).split("/");
//   return parts.length === 2 ? linkTarget(parts[0]!, parts[1]!) : undefined;
// }
pub fn address_from_link(url: &str) -> Option<LinkTarget> {
    let rest = url.get(..LINK_SCHEME.len())?.eq_ignore_ascii_case(LINK_SCHEME).then(|| &url[LINK_SCHEME.len()..])?;
    let rest = rest.strip_suffix('/').unwrap_or(rest);
    match rest.split('/').collect::<Vec<_>>().as_slice() {
        [kind, id] => link_target(kind, id),
        _ => None,
    }
}

/// The hash the app is opened on for a link the shell took before the page was up.
// app-address.ts:108
// export const linkHash = (target: LinkTarget): string => `${LINK_PREFIX}${target.kind}/${target.id}`;
pub fn link_hash(target: &LinkTarget) -> String {
    format!("{LINK_PREFIX}{}/{}", link_kind_word(target.kind), target.id)
}

/// The link a hash carries, or nothing.
// app-address.ts:111
// export function linkFromHash(hash: string): LinkTarget | undefined {
//   if (!hash.startsWith(LINK_PREFIX)) return undefined;
//   const parts = hash.slice(LINK_PREFIX.length).split("/");
//   return parts.length === 2 ? linkTarget(parts[0]!, parts[1]!) : undefined;
// }
pub fn link_from_hash(hash: &str) -> Option<LinkTarget> {
    let rest = hash.strip_prefix(LINK_PREFIX)?;
    match rest.split('/').collect::<Vec<_>>().as_slice() {
        [kind, id] => link_target(kind, id),
        _ => None,
    }
}

