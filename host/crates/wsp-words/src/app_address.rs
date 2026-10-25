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

/// The hash that opens the app on a workspace.
// app-address.ts:35
// export const workspaceHash = (workspaceId: string): string => `${PREFIX}${encodeURIComponent(workspaceId)}`;
pub fn workspace_hash(workspace_id: &str) -> String {
    todo!()
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
    todo!()
}

/// The hash wsp init opens the app on: the workspace it made, or none, with the code that lets the browser in.
// app-address.ts:48
// export function openingHash(code: string, workspaceId?: string): string {
//   return `${workspaceId === undefined ? CODE_ALONE : `${workspaceHash(workspaceId)}${CODE}`}${encodeURIComponent(code)}`;
// }
pub fn opening_hash(code: &str, workspace_id: Option<&str>) -> String {
    todo!()
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
    todo!()
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
    todo!()
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
    todo!()
}

/// The hash the app is opened on for a link the shell took before the page was up.
// app-address.ts:108
// export const linkHash = (target: LinkTarget): string => `${LINK_PREFIX}${target.kind}/${target.id}`;
pub fn link_hash(target: &LinkTarget) -> String {
    todo!()
}

/// The link a hash carries, or nothing.
// app-address.ts:111
// export function linkFromHash(hash: string): LinkTarget | undefined {
//   if (!hash.startsWith(LINK_PREFIX)) return undefined;
//   const parts = hash.slice(LINK_PREFIX.length).split("/");
//   return parts.length === 2 ? linkTarget(parts[0]!, parts[1]!) : undefined;
// }
pub fn link_from_hash(hash: &str) -> Option<LinkTarget> {
    todo!()
}

