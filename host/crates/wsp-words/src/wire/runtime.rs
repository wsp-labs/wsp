// SPDX-License-Identifier: AGPL-3.0-only
//! The refusal of a thread operation on a thread that is not there. Port of the functions of packages/protocol/src/wire/runtime.ts that wsp-wire left to this crate, at
//! a3b870a6e; that file's shapes are wsp-wire's. See spec/wire_runtime.md.

/// The one sentence a thread's own token is refused an op with. It names the op rather than guessing why a caller
/// wanted it: the reasons are on the acts, and this is the door saying the op is not a thread's at all. What a thread
/// may start depends on where it runs, so the sentence claims none of it.
// runtime.ts:1247
// export function threadOpRefusal(op: string, threadId: string, on: string | undefined): string {
//   return `${op} is not a thread's to ask for; this request came in on the token of ${threadAt(threadId, on)}, and a thread's token starts work only under its own root and reads only that tree`;
// }
pub fn thread_op_refusal(op: &str, thread_id: &str, on: Option<&str>) -> String {
    todo!()
}
