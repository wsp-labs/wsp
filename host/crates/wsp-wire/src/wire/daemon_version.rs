// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/wire/daemon-version.ts.

#![allow(unused_imports)]
use crate::types::js_number::JsInt;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::helpers::ReqId;
use crate::types::{Check, JsNumber, SchemaError};
use wsp_frames::{False, True};

/// One process a TERM reached alone, as killTooOldOfLine names it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KilledAlone {
    pub pid: i64,
    pub command: String,
}

/// The daemon's protocol version: the length of the TS record DAEMON_CONTENTS; wsp-frames carries the same number,
/// which the landing's cut writes.
pub const DAEMON_VERSION: i64 = wsp_frames::numbers::DAEMON_VERSION as i64;
pub const DAEMON_CONTENT_SHA: &str = "ca5c18addf281365b4a68a189d212dda48f405176d2707db0bbe9f421f463120";
pub const DAEMON_ROOTS_PATH: &str = "/root/.wsp/roots";
pub const STATE_SHAPE: i64 = 2;
pub const STDIO_DAEMON_VERSION: i64 = 154;
pub const HTTP_POST_DAEMON_VERSION: i64 = 155;
pub const PIPE_DAEMON_VERSION: i64 = 159;
pub const NAMED_WATCH_DAEMON_VERSION: i64 = 136;
pub const SYS_UNWATCH_DAEMON_VERSION: i64 = 152;
pub const FS_FOLDERS_DAEMON_VERSION: i64 = 73;
pub const FORK_BRANCH_DAEMON_VERSION: i64 = 150;
pub const LEAVE_ASKS_DAEMON_VERSION: i64 = 141;
pub const LEAVE_TAKES_DAEMON_VERSION: i64 = 145;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StateShape {
    pub shape: i64,
    pub wsp: String,
    pub daemon: i64,
    pub bin: String,
    pub at: String,
}

/// No refinement of its own; checks every nested field that has one.
impl Check for StateShape {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("shape") { None => out.push(crate::types::check::missing::<Self>("shape")), Some(v) => { match crate::types::check::read::<i64>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("shape"))), } } }
        match obj.get("wsp") { None => out.push(crate::types::check::missing::<Self>("wsp")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("wsp"))), } } }
        match obj.get("daemon") { None => out.push(crate::types::check::missing::<Self>("daemon")), Some(v) => { match crate::types::check::read::<i64>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("daemon"))), } } }
        match obj.get("bin") { None => out.push(crate::types::check::missing::<Self>("bin")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("bin"))), } } }
        match obj.get("at") { None => out.push(crate::types::check::missing::<Self>("at")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("at"))), } } }
        Some(out)
    }

}

// daemon-version.ts:759
// /** How a sentence names the build that wrote a state file. */
// export const stateWriterWords = (wrote: StateShape): string => `${wrote.bin} (wsp ${wrote.wsp}, daemon ${wrote.daemon})`;
pub fn state_writer_words(wrote: &StateShape) -> String {
    todo!()
}
// daemon-version.ts:762
// /** The one word a place's row says while this wsp deploys a newer daemon than that computer runs, and nothing
//  * while it is level or ahead or has never reported. Both sides of the figure are already on the wire: the place
//  * sends its own version in every report and this host's is the record above, so nothing is asked for it. Read by
//  * `wsp places`, by the places table and by the doctor, so the three cannot word it three ways. */
// export function placeDaemonBehind(place: { daemonVersion?: number }): string | undefined {
//   const version = place.daemonVersion;
//   return version === undefined || version >= DAEMON_VERSION ? undefined : `daemon ${version}, host ${DAEMON_VERSION}`;
// }
pub fn place_daemon_behind(daemon_version: Option<i64>) -> Option<String> {
    todo!()
}
// daemon-version.ts:791
// /** Why a computer's daemon cannot take a pane's port watch, in placeDaemonBehind's words: one from before named
//  * watches, or one that never said its version. */
// export function namedWatchGap(daemonVersion: number | undefined): string | undefined {
//   if (daemonVersion === undefined) return `daemon version unknown, host ${DAEMON_VERSION}`;
//   return daemonVersion < NAMED_WATCH_DAEMON_VERSION ? placeDaemonBehind({ daemonVersion }) : undefined;
// }
pub fn named_watch_gap(daemon_version: Option<i64>) -> Option<String> {
    todo!()
}
// daemon-version.ts:809
// /** Whether the `wsp leave` a computer runs is that build or later, read off the version its own wsp says it was built
//  * with and never its daemon's: an update moves the daemon alone. A wsp that said none runs an older leave. */
// export const leaveAsks = (report: { wspDaemonVersion?: number }): boolean => (report.wspDaemonVersion ?? 0) >= LEAVE_ASKS_DAEMON_VERSION;
pub fn leave_asks(wsp_daemon_version: Option<i64>) -> bool {
    todo!()
}
// daemon-version.ts:817
// /** Whether the `wsp leave` a computer runs takes `--takes`. */
// export const leaveTakes = (report: { wspDaemonVersion?: number }): boolean => (report.wspDaemonVersion ?? 0) >= LEAVE_TAKES_DAEMON_VERSION;
pub fn leave_takes(wsp_daemon_version: Option<i64>) -> bool {
    todo!()
}
// daemon-version.ts:820
// /** The line that moves a place onto this wsp's daemon, which is the fix half of every sentence about a place that
//  * is behind. */
// export const placeUpdateLine = (name: string): string => `wsp add ${name} --update`;
pub fn place_update_line(name: &str) -> String {
    todo!()
}
// daemon-version.ts:824
// /** What a process's row says where its computer's daemon is older than a kill that ends what the process started:
//  * TERM reached the process alone, and the fix. */
// export const killTooOldLine = (name: string): string => `TERM went to this process alone: ${name} runs a daemon too old to end what it started; ${placeUpdateLine(name)} updates it`;
pub fn kill_too_old_line(name: &str) -> String {
    todo!()
}
// daemon-version.ts:828
// /** The same said away from the rows, once for a computer, naming each process TERM reached alone. */
// export const killTooOldOfLine = (name: string, procs: readonly { pid: number; command: string }[]): string =>
//   `TERM went to ${procs.map(p => `${p.pid} (${p.command})`).join(" and ")} alone: ${name} runs a daemon too old to end what ${procs.length === 1 ? "it" : "they"} started; ${placeUpdateLine(name)} updates it`;
pub fn kill_too_old_of_line(name: &str, procs: &[KilledAlone]) -> String {
    todo!()
}
// daemon-version.ts:831
// /** Why a slate's MCP server does not start on a place whose daemon cannot hold a command open. */
// export const slateServerBehindLine = (name: string): string => `${name} runs a daemon too old to start a slate's MCP server there; ${placeUpdateLine(name)} updates it`;
pub fn slate_server_behind_line(name: &str) -> String {
    todo!()
}
// daemon-version.ts:834
// /** What the doctor says about one place that is behind: the word above and the line that answers it. */
// export const placeBehindLine = (name: string, word: string): string => `${name} is behind: ${word}; ${placeUpdateLine(name)} puts this wsp's daemon on it`;
pub fn place_behind_line(name: &str, word: &str) -> String {
    todo!()
}
// daemon-version.ts:837
// /** The refusal an update gets on a place already running the daemon this wsp deploys. */
// export const placeCurrentLine = (name: string, version: number): string => `${name} already runs daemon ${version}, which is the one this wsp deploys`;
pub fn place_current_line(name: &str, version: i64) -> String {
    todo!()
}
