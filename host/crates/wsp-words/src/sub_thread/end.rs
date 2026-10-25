// SPDX-License-Identifier: AGPL-3.0-only
//! The lines a sub-thread's end says: what it held, the delete question and the result line.

use super::*;

// The TS `.join(", ")` over any list of strings: one home for the sentences that list names.
pub(super) fn join_all(items: &[impl AsRef<str>], sep: &str) -> String {
    items.iter().map(AsRef::as_ref).collect::<Vec<&str>>().join(sep)
}

/// What a sub-thread holds that no remote has: the counts its last read found, or that the read did not answer.
// sub-thread.ts:467
// export interface SubUnpushed {
//   files: number;
//   commits: number;
//   readFailed?: string;
//   /** A turn ran since the last read, so what it left is not known yet. */
//   unread?: true;
// }
/// `unread` is the TS `true` or absent.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubUnpushed {
    pub files: f64,
    pub commits: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub read_failed: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unread: Option<bool>,
}

/// The second line of the delete's question, and the half of the lead's refusal that says what is not pushed.
// sub-thread.ts:476
// export function subUnpushedLine(held: SubUnpushed, branch: string): string {
//   if (held.unread === true) return `its machine has not been read since its last turn, so work on ${branch} may not be pushed and would go with it.`;
//   if (held.readFailed !== undefined) return `its machine could not be read (${held.readFailed}), so work on ${branch} may not be pushed and would go with it.`;
//   return `${heldWords(held)} on ${branch} ${held.files + held.commits === 1 ? "is" : "are"} not pushed and go with it.`;
// }
pub fn sub_unpushed_line(held: &SubUnpushed, branch: &str) -> String {
    if held.unread == Some(true) {
        return format!("its machine has not been read since its last turn, so work on {branch} may not be pushed and would go with it.");
    }
    if let Some(failed) = &held.read_failed {
        return format!("its machine could not be read ({failed}), so work on {branch} may not be pushed and would go with it.");
    }
    format!(
        "{} on {branch} {} not pushed and go with it.",
        held_words(held.files, held.commits),
        if held.files + held.commits == 1.0 { "is" } else { "are" }
    )
}

/// The question a person answers before a sub-thread and its machine go.
// sub-thread.ts:483
// export function subDeleteQuestion(o: { threadId: string; title: string; on: string; branch: string; held?: SubUnpushed }): string {
//   const first = `Delete sub-thread ${o.threadId.slice(0, 8)} (${o.title}) and its machine on ${o.on}?`;
//   const second = o.held === undefined ? `Its turns leave this computer and its machine is deleted; what it pushed to ${o.branch} stays on the remote.` : subUnpushedLine(o.held, o.branch);
//   return `${first}\n${second.charAt(0).toUpperCase()}${second.slice(1)}`;
// }
/// The TS object `subDeleteQuestion` reads.
#[derive(Clone, Debug, PartialEq)]
pub struct SubDeleteQuestion {
    pub thread_id: String,
    pub title: String,
    pub on: String,
    pub branch: String,
    pub held: Option<SubUnpushed>,
}

pub fn sub_delete_question(o: &SubDeleteQuestion) -> String {
    let first = format!("Delete sub-thread {} ({}) and its machine on {}?", utf16::cut(&o.thread_id, 8), o.title, o.on);
    let second = match &o.held {
        None => format!("Its turns leave this computer and its machine is deleted; what it pushed to {} stays on the remote.", o.branch),
        Some(held) => sub_unpushed_line(held, &o.branch),
    };
    format!("{first}\n{}", upper_first(&second))
}

/// What a sub-thread's machine holds that no remote has, or may: the line a remove of its cloud lists, and the half
/// of the lead's refusal that says what happened.
// sub-thread.ts:491
// export function subHeldLine(threadId: string, held: SubUnpushed, branch: string): string {
//   const id8 = threadId.slice(0, 8);
//   if (held.unread === true) return `sub-thread ${id8}'s machine has not been read since its last turn, so work on ${branch} may not be pushed`;
//   if (held.readFailed !== undefined) return `sub-thread ${id8}'s machine could not be read (${held.readFailed}), so work on ${branch} may not be pushed`;
//   return `sub-thread ${id8} holds ${heldWords(held)} not pushed on ${branch}`;
// }
pub fn sub_held_line(thread_id: &str, held: &SubUnpushed, branch: &str) -> String {
    let id8 = utf16::cut(thread_id, 8);
    if held.unread == Some(true) {
        return format!("sub-thread {id8}'s machine has not been read since its last turn, so work on {branch} may not be pushed");
    }
    if let Some(failed) = &held.read_failed {
        return format!("sub-thread {id8}'s machine could not be read ({failed}), so work on {branch} may not be pushed");
    }
    format!("sub-thread {id8} holds {} not pushed on {branch}", held_words(held.files, held.commits))
}

/// A lead's own token asking the delete of a sub-thread whose work is not pushed: losing it is the person's call.
// sub-thread.ts:499
// export const subHeldDeleteRefusal = (threadId: string, held: SubUnpushed, branch: string): Error =>
//   usage(subHeldLine(threadId, held, branch), "Send it a push, or ask the person to delete it.");
pub fn sub_held_delete_refusal(thread_id: &str, held: &SubUnpushed, branch: &str) -> crate::exit::HostError {
    usage(&sub_held_line(thread_id, held, branch), "Send it a push, or ask the person to delete it.")
}

/// A sub-thread whose cloud this host no longer holds, as its row says it.
// sub-thread.ts:503
// export const subCloudGoneLine = (on: string): string => `${on} is no longer on this host`;
pub fn sub_cloud_gone_line(on: &str) -> String {
    format!("{on} is no longer on this host")
}

/// A send into a sub-thread whose cloud this host no longer holds.
// sub-thread.ts:506
// export const subCloudGoneRefusal = (on: string): Error => usage(subCloudGoneLine(on), `Add it with wsp add ${on.toLowerCase()} and send again.`);
pub fn sub_cloud_gone_refusal(on: &str) -> crate::exit::HostError {
    usage(&sub_cloud_gone_line(on), &format!("Add it with wsp add {} and send again.", on.to_lowercase()))
}

/// A wake of a sub-thread whose machine was deleted after it was done: there is none to wake, and a send makes one.
// sub-thread.ts:509
// export const subReleasedRefusal = (threadId: string, on: string): Error =>
//   usage(`sub-thread ${threadId.slice(0, 8)}'s machine on ${on} was deleted after it was done, so there is none to wake`, `Send to it with wsp send ${threadId.slice(0, 8)} "<message>", which starts a new one.`);
pub fn sub_released_refusal(thread_id: &str, on: &str) -> crate::exit::HostError {
    let id8 = utf16::cut(thread_id, 8);
    usage(
        &format!("sub-thread {id8}'s machine on {on} was deleted after it was done, so there is none to wake"),
        &format!("Send to it with wsp send {id8} \"<message>\", which starts a new one."),
    )
}

/// What a delete Boat refused says, and when it is asked again.
// sub-thread.ts:513
// export const subDeleteRefusedLine = (on: string, machine: string, said: string, retryMinutes: number): string =>
//   `${on} would not delete ${machine} (${said}); it is tried again in ${retryMinutes} minutes`;
pub fn sub_delete_refused_line(on: &str, machine: &str, said: &str, retry_minutes: f64) -> String {
    format!("{on} would not delete {machine} ({said}); it is tried again in {} minutes", number_string(retry_minutes))
}

/// The line a cloud's remove adds to its question for sub-threads whose machines hold work no remote has: it goes
/// with them. Names five and counts the rest.
// sub-thread.ts:518
// export function subHeldOnRemoveLine(held: readonly string[]): string | undefined {
//   if (held.length === 0) return undefined;
//   const rest = held.length > 5 ? `; and ${held.length - 5} more` : "";
//   return `Work not pushed goes with ${held.length === 1 ? "its machine" : "their machines"}: ${held.slice(0, 5).join("; ")}${rest}.`;
// }
pub fn sub_held_on_remove_line(held: &[impl AsRef<str>]) -> Option<String> {
    if held.is_empty() {
        return None;
    }
    let rest = if held.len() > 5 { format!("; and {} more", held.len() - 5) } else { String::new() };
    let named: Vec<&str> = held.iter().take(5).map(AsRef::as_ref).collect();
    let whose = if held.len() == 1 { "its machine" } else { "their machines" };
    Some(format!("Work not pushed goes with {whose}: {}{rest}.", named.join("; ")))
}

/// A sub-thread's cloud as a line names it, off the word its record keeps: `boat` is Boat.
// sub-thread.ts:525
// export function cloudSaid(on: string): string {
//   const id = cloudNamed(Object.keys(PROVIDER_KEY_WORDS), on);
//   return id === undefined ? on : providerKeyName(id);
// }
pub fn cloud_said(on: &str) -> String {
    match cloud_named(&PROVIDER_KEY_IDS[..], on) {
        Some(id) => provider_key_name(&id),
        None => on.to_string(),
    }
}

/// What a sub-thread's delete did.
// sub-thread.ts:531
// export const subDeletedLine = (threadId: string, on: string): string => `deleted sub-thread ${threadId.slice(0, 8)} and its machine on ${on}`;
pub fn sub_deleted_line(thread_id: &str, on: &str) -> String {
    format!("deleted sub-thread {} and its machine on {on}", utf16::cut(thread_id, 8))
}

// The UTC day of a moment, for a line about a cloud's keep of a disk: the keep is counted from the cloud's own stop,
// so the time of day would claim more than wsp knows.
// sub-thread.ts:535
// private in the TS: port it as a private helper
// const utcDay = (at: number): string => `${new Date(at).toISOString().slice(0, 10)} UTC`;
pub(super) fn utc_day(at: f64) -> String {
    let day = jiff::Timestamp::from_millisecond(at as i64).expect("a cloud's keep is a moment jiff holds");
    format!("{} UTC", day.strftime("%Y-%m-%d"))
}

/// The line a sub-thread's finished line to its lead ends with, off the done's read: the branch pushed and the merge
/// that takes it, the work not pushed that keeps the machine, or nothing to merge. `deleteInMs` is how long the machine
/// stays, absent where it goes at once; a span, never a clock time, since an agent and a person read it in any zone.
/// `kept` is the day from which a cloud that deletes a paused machine's disk may delete this one's, a UTC day for the
/// same reason.
// sub-thread.ts:542
// export function subResultLine(o: { threadId: string; lead: string; result: SubResult; base: string; deleteInMs?: number; kept?: { on: string; until: number } }): string {
//   const { result: r, kept } = o;
//   const id8 = o.threadId.slice(0, 8);
//   const minutes = o.deleteInMs === undefined ? 0 : Math.round(o.deleteInMs / 60_000);
//   const goes = minutes > 0 ? `Its machine naps now and is deleted in ${plural(minutes, "minute")} unless you send to ${id8} first.` : `Its machine is deleted now; a send to ${id8} starts a new one with the conversation kept.`;
//   const naps = kept === undefined ? "its machine is kept, napping at no cost" : `its machine naps at no cost, and ${diskMayGoLine(kept.on, utcDay(kept.until))}`;
//   if (r.readFailed !== undefined) return `its machine could not be read (${r.readFailed}), so work on ${r.branch} may not be pushed and ${naps}. Send to ${id8} to have it pushed, or ask the person to delete it.`;
//   if (!subAllPushed(r)) {
//     const parts = [r.unpushed > 0 ? plural(r.unpushed, "commit") : "", r.changed > 0 ? plural(r.changed, "changed file") : ""].filter(Boolean);
//     const one = r.unpushed + r.changed === 1;
//     return `${parts.join(" and ")} on ${r.branch} ${one ? "is" : "are"} not pushed, so ${naps}. Send to ${id8} to have ${one ? "it" : "them"} pushed, or ask the person to delete it.`;
//   }
//   if (r.pushedSha === null || r.commits === 0) return `no commits on ${r.branch}, so there is nothing to merge. ${goes}`;
//   return `branch ${r.branch} pushed at ${r.pushedSha.slice(0, 7)}, ${plural(r.commits, "commit")} on ${o.base}; merge it with wsp merge in ${o.lead.slice(0, 8)} ${id8}. ${goes}`;
// }
/// The TS `{ on, until }` of a kept disk, `until` ms epoch.
#[derive(Clone, Debug, PartialEq)]
pub struct KeptUntil {
    pub on: String,
    pub until: f64,
}

/// The TS object `subResultLine` reads.
#[derive(Clone, Debug, PartialEq)]
pub struct SubResultFacts {
    pub thread_id: String,
    pub lead: String,
    pub result: SubResult,
    pub base: String,
    pub delete_in_ms: Option<f64>,
    pub kept: Option<KeptUntil>,
}

pub fn sub_result_line(o: &SubResultFacts) -> String {
    let r = &o.result;
    let id8 = utf16::cut(&o.thread_id, 8);
    let minutes = o.delete_in_ms.map_or(0.0, |ms| js_round(ms / 60_000.0));
    let goes = if minutes > 0.0 {
        format!("Its machine naps now and is deleted in {} unless you send to {id8} first.", plural(minutes, "minute"))
    } else {
        format!("Its machine is deleted now; a send to {id8} starts a new one with the conversation kept.")
    };
    let naps = match &o.kept {
        None => "its machine is kept, napping at no cost".to_string(),
        Some(kept) => format!("its machine naps at no cost, and {}", disk_may_go_line(&kept.on, &utc_day(kept.until))),
    };
    if let Some(failed) = &r.read_failed {
        return format!(
            "its machine could not be read ({failed}), so work on {} may not be pushed and {naps}. Send to {id8} to have it pushed, or ask the person to delete it.",
            r.branch
        );
    }
    if !sub_all_pushed(r) {
        let unpushed = r.unpushed.get() as f64;
        let changed = r.changed.get() as f64;
        let mut parts = Vec::new();
        if unpushed > 0.0 {
            parts.push(plural(unpushed, "commit"));
        }
        if changed > 0.0 {
            parts.push(plural(changed, "changed file"));
        }
        let one = unpushed + changed == 1.0;
        return format!(
            "{} on {} {} not pushed, so {naps}. Send to {id8} to have {} pushed, or ask the person to delete it.",
            parts.join(" and "),
            r.branch,
            if one { "is" } else { "are" },
            if one { "it" } else { "them" }
        );
    }
    let commits = r.commits.get() as f64;
    match &r.pushed_sha {
        Some(sha) if commits != 0.0 => format!(
            "branch {} pushed at {}, {} on {}; merge it with wsp merge in {} {id8}. {goes}",
            r.branch,
            utf16::cut(sha, 7),
            plural(commits, "commit"),
            o.base,
            utf16::cut(&o.lead, 8)
        ),
        _ => format!("no commits on {}, so there is nothing to merge. {goes}", r.branch),
    }
}
