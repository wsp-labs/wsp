// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/wire/devices.ts.

#![allow(unused_imports)]
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::helpers::ReqId;
use crate::types::{Check, JsNumber, SchemaError};
use crate::views::workspace::ThreadScope;
use crate::wire::machine_link::DaemonResponse;
use wsp_frames::{False, True};

use crate::views::workspace::WorkspaceOrigin;

/// A join or pairing token read back: the code as the host takes it, and the host key where the token carried one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JoinToken {
    pub code: String,
    pub host_key: Option<String>,
}

// devices.ts:184
// export const TICKET_ORIGIN: Record<TicketPurpose, WorkspaceOrigin> = { connect: "here", relay: "relayed" };
/// Where a socket redeeming a ticket of each purpose reached the host from: connect is here, relay is relayed.
pub fn ticket_origin(purpose: TicketPurpose) -> WorkspaceOrigin {
    todo!()
}

pub const PAIR_CODE_TTL_MS: i64 = 600000;
pub const PAIR_CODE_LENGTH: i64 = 8;
pub const PAIR_CODE_ALPHABET: &str = "0123456789ABCDEFGHJKMNPQRSTVWXYZ";
pub const PAIR_CODE_SHAPE_REFUSAL: &str =
    "that is not a pairing code, which is 8 letters and digits like ABCD-EFGH; paste only the code wsp host pair printed";
pub const ACCOUNT_TICKET_REFUSAL: &str =
    "a socket let in on a ticket cannot see the account this host is signed in to; run wsp login on the computer the host runs on";
pub const DEFAULT_RELAY: &str = "https://relay.usewsp.com";
pub const DEVICE_AUTH_REFUSAL: &str = "this host admits a computer on the account only on an admission signed by a key it already trusts; run wsp login to read this computer's id on a computer that is already in, then wsp login <id> there, and dial again";
pub const DEVICE_REVOKED_REFUSAL: &str = "this host took this computer's token away; it is admitted through the account no longer, and wsp host pair on the host is the way back in";
pub const DEVICE_ACCOUNT_UNSERVED: &str =
    "this host is on no account, so it admits no computer through one; run wsp host link on the computer it runs on to put it on yours";
pub const LOGIN_NO_KEY_REFUSAL: &str = "that word names no key for the computer signing in, so nothing here could say which key it would be admitting; run wsp login there again and copy the whole word it prints";
pub const ACCOUNT_UNSERVED: &str = "this host keeps no account records; wsp up serves them";
pub const PAIR_ISSUE_REFUSAL: &str =
    "only a socket holding this host's own token may mint a pairing code; run wsp host pair on the computer the host runs on";
pub const PAIR_CODE_REFUSAL: &str = "that pairing code is not one this host is waiting for; run wsp host pair on the host for a fresh one";
pub const DEVICES_TICKET_REFUSAL: &str = "a socket let in on a ticket cannot see or change the devices paired with this host; run wsp host devices on the computer the host runs on";
pub const API_UNAUTHORIZED: &str = "this route needs a token in an Authorization header, the host's own from the token file beside its state or a paired device's; run wsp host pair on the computer the host runs on for one";
pub const JOIN_TOKEN_MARK: &str = ".";
pub const JOIN_NO_KEY_REFUSAL: &str = "that join line names no key for the host, so this computer cannot tell which host it is joining; run wsp add on the host again and copy the whole code it prints";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TicketPurpose {
    #[serde(rename = "connect")]
    Connect,
    #[serde(rename = "relay")]
    Relay,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceVia {
    pub kind: String,
    pub relay_device_id: String,
    pub fingerprint: String,
    pub public_key: String,
    pub admitted_by: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceView {
    pub id: String,
    pub name: String,
    pub created_at: String,
    pub last_seen_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub scope: Option<ThreadScope>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub here: Option<True>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub via: Option<DeviceVia>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountView {
    pub signed_in: bool,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub login: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Admission {
    pub device: String,
    pub by: String,
    pub issued_at: String,
    pub signature: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountDeviceAdmission {
    pub by: String,
    pub issued_at: String,
    pub signature: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountDevice {
    pub id: String,
    pub name: String,
    pub fingerprint: String,
    pub admissions: Vec<AccountDeviceAdmission>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountDevices {
    pub devices: Vec<AccountDevice>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DaemonFrame {
    pub op: String,
    /// zod passthrough: unknown keys kept, in the order read.
    #[serde(flatten, serialize_with = "crate::types::check::js_record")]
    pub rest: IndexMap<String, Value>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DaemonOpenReply {
    pub channel: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DaemonSendReply {
    pub reply: DaemonResponse,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DaemonEventEvent {
    pub r#type: String,
    /// zod passthrough: unknown keys kept, in the order read.
    #[serde(flatten, serialize_with = "crate::types::check::js_record")]
    pub rest: IndexMap<String, Value>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum DaemonChannelEvent {
    #[serde(rename = "daemon.event", rename_all = "camelCase")]
    DaemonEvent { channel: String, event: DaemonEventEvent },
    #[serde(rename = "daemon.closed", rename_all = "camelCase")]
    DaemonClosed { channel: String, code: i64, reason: String },
}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `kind`: literal "account"
impl Check for DeviceVia {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for DeviceView {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for AccountView {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for Admission {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for AccountDevice {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for AccountDeviceAdmission {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for AccountDevices {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `op`: refine: (val, ctx) => { const result = check(val); const setError = () => ctx.addIssue({ code: ZodIssueCode.custom, ...getIssueProperties(val) }); if (typeof Promise !=
impl Check for DaemonFrame {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for DaemonOpenReply {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for DaemonSendReply {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for DaemonChannelEvent {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for DaemonEventEvent {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

// devices.ts:68
// /** A pairing code as every screen shows it: the alphabet's letters in two halves, which is how a person reads one
//  * across a room. The one grouping, so the sheet that shows a code and the field that takes one agree. */
// export function shownPairCode(code: string): string {
//   const letters = code.replace(/-/g, "").toUpperCase().slice(0, PAIR_CODE_LENGTH);
//   const half = Math.ceil(PAIR_CODE_LENGTH / 2);
//   return letters.length <= half ? letters : `${letters.slice(0, half)}-${letters.slice(half)}`;
// }
pub fn shown_pair_code(code: &str) -> String {
    todo!()
}
// devices.ts:76
// /** A pairing code as the host takes it, whichever screen it was copied off: the letters alone, upper case. A
//  * person copies the code they can read, so the dash the screens put in it is one this reading takes back out. */
// export const sentPairCode = (shown: string): string => shown.replace(/-/g, "").toUpperCase();
pub fn sent_pair_code(shown: &str) -> String {
    todo!()
}
// devices.ts:85
// /** Whether a code as the host takes it could be one a host is holding: the alphabet's characters at the length a code
//  * is. What tells a code from a paste that caught a label, a space for the dash or a character too many. */
// export const isPairCode = (code: string): boolean => code.length === PAIR_CODE_LENGTH && [...code].every(c => PAIR_CODE_ALPHABET.includes(c));
pub fn is_pair_code(code: &str) -> bool {
    todo!()
}
// devices.ts:137
// /** What a device signs to prove it may come in through the account: the key admitted, the key that signed for it
//  * and the moment, built by one function so the wsp that signs and the host that verifies cannot drift. The host
//  * is not inside it: one admission stands at every host on the account whose trust the signer already has, which
//  * is what saves a person a code per host. */
// export function deviceAdmissionTranscript(device: string, by: string, issuedAt: string): Uint8Array {
//   return new TextEncoder().encode(`wsp device admission v1\n${device}\n${by}\n${issuedAt}\n`);
// }
pub fn device_admission_transcript(device: &str, by: &str, issued_at: &str) -> Vec<u8> {
    todo!()
}
// devices.ts:160
// /** What a computer reads when the host it dialled answered device.auth with its own request schema's refusal: a
//  * host of an older wsp, whose door knows no road in for a computer on the account. Told apart from a refusal of this build by
//  * the kind on the frame, which an older host's schema refusal carries none of, so a token this computer never sent
//  * is never read as one that was taken away. */
// export const deviceAuthOldHostLine = (where: string): string =>
//   `the host at ${where} runs an older wsp, whose door does not know how a computer on the account comes in; update wsp on that computer and run wsp up there again`;
pub fn device_auth_old_host_line(r#where: &str) -> String {
    todo!()
}
// devices.ts:191
// /** The refusal a write route and a browser's upgrade answer with when the page that sent them was loaded at
//  * another name than the one this host was reached at. A page may only drive the host it was served by, and the
//  * hostname is the whole of the reading: a page on the app's port dialling the runtime's is the same page. */
// export function crossOriginRefusal(origin: string, host: string): string {
//   return `this request came from ${origin} and this host was reached at ${host}; the page and its socket open from the address the host answers at`;
// }
pub fn cross_origin_refusal(origin: &str, host: &str) -> String {
    todo!()
}
// devices.ts:225
// /** The one word a person copies off a join line: the single-use code and the fingerprint of the key the host will
//  * prove, as one string, so a join stays two things to copy and the screens keep the fields they have. */
// export const joinToken = (code: string, hostKey: string): string => `${shownPairCode(code)}${JOIN_TOKEN_MARK}${hostKey}`;
pub fn join_token(code: &str, host_key: &str) -> String {
    todo!()
}
// devices.ts:229
// /** The same token read back on the computer being joined, whichever road it came by: a person's paste, the flag,
//  * or the file an install over ssh landed. The code is taken as any screen's code is taken; the fingerprint is left
//  * exactly as it was written, since its own alphabet is case sensitive. A token that carries no fingerprint answers
//  * none, and the caller refuses rather than dialling. */
// export function readJoinToken(typed: string): { code: string; hostKey?: string } {
//   const trimmed = typed.trim();
//   const at = trimmed.indexOf(JOIN_TOKEN_MARK);
//   if (at === -1) return { code: sentPairCode(trimmed) };
//   const hostKey = trimmed.slice(at + 1).trim();
//   const code = sentPairCode(trimmed.slice(0, at));
//   return hostKey === "" ? { code } : { code, hostKey };
// }
pub fn read_join_token(typed: &str) -> JoinToken {
    todo!()
}
// devices.ts:246
// /** The refusal a join gets when the host at that address proved a key that is not the one the join line named:
//  * something answered where the host was expected. Nothing of this computer's went to it. */
// export const joinKeyRefusal = (url: string): string => `the host at ${url} proved a key the join line did not name, so it is not the host that printed that line; nothing was sent to it`;
pub fn join_key_refusal(url: &str) -> String {
    todo!()
}
// devices.ts:250
// /** The one word a person copies off wsp host pair, which is the join line's token under the name the pairing road
//  * reads it by: the code and the fingerprint of the key the host will prove, so a client pins that key before it
//  * spends the code. `readJoinToken` reads both roads' tokens, since they are one shape. */
// export const pairToken = joinToken;
pub fn pair_token(code: &str, host_key: &str) -> String {
    todo!()
}
// devices.ts:255
// /** The refusal a dial gets when whatever answered at that address did not prove the key this computer holds that
//  * host to, whether it proved another one or signed nothing this computer could verify: it is not that host,
//  * whichever check caught it, and no token of this computer's went to it. */
// export const pairKeyRefusal = (url: string): string => `the host at ${url} did not prove the key this computer holds for it, so it is not that host; nothing was sent to it`;
pub fn pair_key_refusal(url: &str) -> String {
    todo!()
}

// A word list zod reads with z.enum: serde refuses any other word, so nothing is left to check.
impl Check for TicketPurpose {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}
