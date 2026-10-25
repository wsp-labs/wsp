// SPDX-License-Identifier: AGPL-3.0-only
//! One thread read off the transcript the host holds: the messages a reader sees and the result its latest turn ended with. Port of the functions of packages/protocol/src/thread-read.ts at
//! the cut (383708c78); its shapes and flat constants are wsp_wire::types::thread_read. See spec/thread_read.md.

use indexmap::IndexMap;

use wsp_wire::types::js_number::JsNumber;
use wsp_wire::types::thread_read::{ThreadMessage, ThreadVoice, THREAD_SEED_CHARS};
use wsp_wire::views::session_events::{
    DeltaKind, PlanStepState, SessionChangesEvent, SessionDeltaEvent, SessionDoneEvent, SessionEarlierEvent, SessionEarlierEventWho,
    SessionEndEvent, SessionEvent, SessionPermissionEvent, SessionPlanEvent, SessionStartEvent, TurnResult, TurnStatus,
};

use crate::words::tools::{tool_activity_line, tool_done_line};
use crate::words::turn::{
    fmt_clock, notify_body, notify_reply, plan_steps_line, turn_changes_line, turn_end_line, FileLines, NotifyLength, StepState,
    TurnChanges, NEWER_TURN_LINE, NO_RESULT_LINE,
};

// Who a row of a read is: the message that opened or steered a turn, the agent's own words, one tool call, or the
// turn's own end as the chat's footer states it.
// thread-read.ts:18
// in wsp_wire::types::thread_read: ThreadVoice

// thread-read.ts:19
// in wsp_wire::types::thread_read: ThreadVoice

// thread-read.ts:21
// in wsp_wire::types::thread_read: ThreadMessage

// thread-read.ts:27
// in wsp_wire::types::thread_read: ThreadMessage

// Whether the event is this thread's. The runtime stamps every event with the thread it belongs to; a row from
// before the stamp carries none and is read by no thread id, which is what a wait on such a thread already does.
// thread-read.ts:31
// private in the TS: port it as a private helper
// const inThread = (event: { threadId?: string }, threadId: string): boolean => event.threadId === threadId;

// thread-read.ts:35
// in wsp_wire::types::thread_read: THREAD_SEED_CHARS

/// The thread as a new session is handed it, the Markdown a person would paste, its newest rows kept within the cap
/// and the count of the rows left out said first.
// thread-read.ts:39
// export function threadSeed(messages: readonly ThreadMessage[]): string {
//   let at = messages.length;
//   for (let size = 0; at > 0 && size + messages[at - 1]!.text.length <= THREAD_SEED_CHARS; at--) size += messages[at - 1]!.text.length;
//   const kept = threadMarkdown(messages.slice(at));
//   return at === 0 ? kept : `*${at} earlier ${at === 1 ? "message" : "messages"} left out*\n\n${kept}`;
// }
pub fn thread_seed(messages: &[ThreadMessage]) -> String {
    let mut at = messages.len();
    let mut size = 0usize;
    while at > 0 {
        let units = messages[at - 1].text.encode_utf16().count();
        if size + units > THREAD_SEED_CHARS as usize {
            break;
        }
        size += units;
        at -= 1;
    }
    let kept = thread_markdown(&messages[at..]);
    if at == 0 {
        return kept;
    }
    let noun = if at == 1 { "message" } else { "messages" };
    format!("*{} earlier {noun} left out*\n\n{kept}", crate::js::number_string(at as f64))
}

// thread-read.ts:46
// private in the TS: port it as a private helper
// const message = (who: ThreadVoice, at: number | undefined, text: string): ThreadMessage => ({ who, ...(at !== undefined ? { at } : {}), text });

/// The thread's messages, oldest first: the prompt of every turn and every steer, the agent's text as it arrived,
/// one row per tool call, and each turn's end. Text runs into the open agent row until a call, a person's message or
/// the turn's end closes it, so a reply broken by tool calls reads as the pieces the agent wrote. A turn whose reply
/// never arrived as text (a transcript capped mid-turn, a harness that reports the message only at the end) takes it
/// from the result, the same rule the app's timeline follows.
// thread-read.ts:53
// export function threadMessages(events: ReadonlyArray<SessionEvent>, threadId: string): ThreadMessage[] {
//   const rows: ThreadMessage[] = [];
//   /** The agent row text deltas append to and the harness message it is a piece of; nothing when the last row is
//    * not one of the agent's. */
//   let open: { row: number; messageId?: string } | undefined;
//   /** The running turn's tool rows by the id of their call, since a call's JSON may arrive in pieces and the line is
//    * drawn from the whole of it. */
//   const calls = new Map<string, { row: number; name: string; input: string }>();
//   let sawText = false;
//   let replied = false;
//   /** The running turn's step list row and proposed plan row, which a later one of each rewrites where it stands. */
//   let stepsRow: number | undefined;
//   let planRow: number | undefined;
//   const say = (who: ThreadVoice, at: number | undefined, text: string): number => {
//     open = undefined;
//     rows.push(message(who, at, text));
//     return rows.length - 1;
//   };
//   const turn = (at: number | undefined, result: TurnResult): void => {
//     if (!sawText && result.text !== undefined && result.text !== "") say("agent", at, result.text);
//     say("turn", at, turnEndLine(result));
//     calls.clear();
//     sawText = false;
//   };
//   for (const event of events) {
//     if (!inThread(event, threadId)) continue;
//     switch (event.type) {
//       case "session.start":
//         open = undefined;
//         calls.clear();
//         sawText = false;
//         replied = false;
//         stepsRow = undefined;
//         planRow = undefined;
//         if (event.prompt !== undefined) say("person", event.at, event.prompt);
//         continue;
//       case "session.steer":
//         say("person", event.at, event.prompt);
//         continue;
//       case "session.moved":
//         say("turn", event.at, threadMovedLine(event.from, event.to, event.fresh === true));
//         continue;
//       case "session.behind":
//       case "session.notice":
//         say("turn", event.at, event.text);
//         continue;
//       case "thread.waited":
//         say("turn", event.at, event.said ?? waitedLine(event.ms));
//         continue;
//       case "session.earlier":
//         say(event.who === "note" ? "turn" : event.who, event.at, event.text);
//         continue;
//       case "session.delta": {
//         if (event.kind === "text") {
//           // An empty piece of text opens no row: an adapter that splits a reply hands over the tail of a short one
//           // as nothing at all, and a row of no words is not a message anybody sent.
//           if (event.text === "") continue;
//           // A piece of another of the harness's messages opens a row of its own, the rule messageId carries.
//           if (open?.messageId !== undefined && event.messageId !== undefined && event.messageId !== open.messageId) open = undefined;
//           if (open === undefined) open = { row: rows.push(message("agent", event.at, event.text)) - 1, ...(event.messageId !== undefined ? { messageId: event.messageId } : {}) };
//           else rows[open.row]!.text += event.text;
//           sawText = true;
//         } else if (event.kind === "tool_result") {
//           // The call's own row turns to the past here and nowhere else: what the transcript holds a result for is
//           // what actually ran, and a call still waiting on a person has no result and keeps its present.
//           const answered = event.toolUseId === undefined ? undefined : calls.get(event.toolUseId);
//           if (answered !== undefined && event.isError !== true) {
//             const did = toolDoneLine(answered.name, answered.input);
//             if (did !== undefined) rows[answered.row]!.text = did;
//           }
//         } else if (event.kind === "tool_use") {
//           open = undefined;
//           const key = event.toolUseId;
//           const call = key === undefined ? undefined : calls.get(key);
//           if (call !== undefined) {
//             call.name = event.toolName ?? call.name;
//             call.input += event.text;
//             rows[call.row]!.text = toolActivityLine(call.name, call.input);
//           } else {
//             // A call the harness named an id for takes the pieces that follow it; one it named none for keys
//             // nothing, so it is a row of its own rather than words appended to whichever call came before.
//             const row = say("tool", event.at, toolActivityLine(event.toolName, event.text));
//             if (key !== undefined) calls.set(key, { row, name: event.toolName ?? "tool", input: event.text });
//           }
//         }
//         continue;
//       }
//       case "session.done":
//         replied = true;
//         turn(event.at, event.result);
//         continue;
//       case "session.end":
//         // A turn that never launched has no start of its own, so its end carries what was sent and is a turn of its own.
//         if (event.unstarted === true) {
//           if (event.prompt !== undefined) say("person", event.at, event.prompt);
//           turn(event.at, { status: "failed", error: event.reason ?? NO_RESULT_LINE });
//           continue;
//         }
//         // A turn the runtime ended before the harness replied: the read says so where the transcript has no result.
//         if (!replied) turn(event.at, { status: "failed", error: event.reason ?? NO_RESULT_LINE });
//         continue;
//       case "session.compacted":
//         say("tool", event.at, compactedLine(event.before, event.after));
//         continue;
//       case "session.plan":
//         if (event.steps !== undefined) {
//           if (stepsRow === undefined) stepsRow = say("tool", event.at, planStepsLine(event.steps));
//           else rows[stepsRow]!.text = planStepsLine(event.steps);
//         }
//         if (event.text !== undefined) {
//           if (planRow === undefined) planRow = say("agent", event.at, event.text);
//           else rows[planRow]!.text = event.text;
//         }
//         continue;
//       case "session.changes": {
//         // The count lands after the turn's end, and reads with the turn's work above that end.
//         const row = message("tool", event.at, turnChangesLine(event));
//         if (rows.at(-1)?.who === "turn") rows.splice(rows.length - 1, 0, row);
//         else rows.push(row);
//         open = undefined;
//         continue;
//       }
//       default:
//         continue;
//     }
//   }
//   return rows;
// }
pub fn thread_messages(events: &[SessionEvent], thread_id: &str) -> Vec<ThreadMessage> {
    let mut reader = Reader::default();
    for event in events {
        if thread_of(event) != Some(thread_id) {
            continue;
        }
        let at = at_of(event);
        match event {
            SessionEvent::SessionStartEvent(e) => reader.start(e, at),
            SessionEvent::SessionSteerEvent(e) => {
                reader.say(ThreadVoice::Person, at, e.prompt.clone());
            }
            SessionEvent::SessionMovedEvent(e) => {
                let line = crate::projects::thread_moved_line(&e.from, &e.to, e.fresh.is_some());
                reader.say(ThreadVoice::Turn, at, line);
            }
            SessionEvent::SessionBehindEvent(e) => {
                reader.say(ThreadVoice::Turn, at, e.text.clone());
            }
            SessionEvent::SessionNoticeEvent(e) => {
                reader.say(ThreadVoice::Turn, at, e.text.clone());
            }
            SessionEvent::ThreadWaitedEvent(e) => {
                let line = e.said.clone().unwrap_or_else(|| crate::workspace_state::waited_line(e.ms.as_f64()));
                reader.say(ThreadVoice::Turn, at, line);
            }
            SessionEvent::SessionEarlierEvent(e) => reader.earlier(e, at),
            SessionEvent::SessionDeltaEvent(e) => reader.delta(e, at),
            SessionEvent::SessionDoneEvent(e) => reader.done(e, at),
            SessionEvent::SessionEndEvent(e) => reader.end(e, at),
            SessionEvent::SessionCompactedEvent(e) => {
                let line = crate::words::turn::compacted_line(e.before.as_ref().map(|n| n.0), e.after.as_ref().map(|n| n.0));
                reader.say(ThreadVoice::Tool, at, line);
            }
            SessionEvent::SessionPlanEvent(e) => reader.plan(e, at),
            SessionEvent::SessionChangesEvent(e) => reader.changes(e, at),
            _ => {}
        }
    }
    reader.rows
}

/// The thread as Markdown, for a person to paste elsewhere: each message a person sent under a bold header with its
/// clock, the agent's words as it wrote them under one header until the person speaks again, every tool row an item
/// of a list, a step list nested under its count, and each turn's end in italics.
// thread-read.ts:185
// export function threadMarkdown(rows: readonly ThreadMessage[]): string {
//   const blocks: string[][] = [];
//   let speaker: ThreadVoice | undefined;
//   for (const [index, row] of rows.entries()) {
//     if (row.who === "person") {
//       const clock = fmtClock(row.at);
//       blocks.push([clock === "" ? "**You**" : `**You** ${clock}`], row.text.split("\n"));
//       speaker = "person";
//       continue;
//     }
//     if (row.who === "turn") {
//       blocks.push(row.text.split("\n").map(line => (line === "" ? line : `*${line}*`)));
//       speaker = "turn";
//       continue;
//     }
//     if (speaker !== "agent") blocks.push(["**Agent**"]);
//     speaker = "agent";
//     if (row.who === "agent") {
//       blocks.push(row.text.split("\n"));
//       continue;
//     }
//     const [head = "", ...rest] = row.text.split("\n");
//     const item = [`- ${head}`, ...rest.map(line => `  ${line}`)];
//     if (rows[index - 1]?.who === "tool") blocks.at(-1)!.push(...item);
//     else blocks.push(item);
//   }
//   return blocks.map(block => block.join("\n")).join("\n\n");
// }
pub fn thread_markdown(rows: &[ThreadMessage]) -> String {
    let zone = crate::js_date::DateZone::system();
    let offset = |at: f64| crate::js_date::time_value(at).map_or(0, |t| zone.offset_seconds(t));
    let mut blocks: Vec<Vec<String>> = Vec::new();
    let mut speaker: Option<ThreadVoice> = None;
    for (index, row) in rows.iter().enumerate() {
        match row.who {
            ThreadVoice::Person => {
                let clock = fmt_clock(row.at.as_ref().map(|at| at.0), &offset);
                let head = if clock.is_empty() { "**You**".to_string() } else { format!("**You** {clock}") };
                blocks.push(vec![head]);
                blocks.push(row.text.split('\n').map(str::to_string).collect());
                speaker = Some(ThreadVoice::Person);
                continue;
            }
            ThreadVoice::Turn => {
                blocks
                    .push(row.text.split('\n').map(|line| if line.is_empty() { line.to_string() } else { format!("*{line}*") }).collect());
                speaker = Some(ThreadVoice::Turn);
                continue;
            }
            ThreadVoice::Agent | ThreadVoice::Tool => {}
        }
        if speaker != Some(ThreadVoice::Agent) {
            blocks.push(vec!["**Agent**".to_string()]);
        }
        speaker = Some(ThreadVoice::Agent);
        if row.who == ThreadVoice::Agent {
            blocks.push(row.text.split('\n').map(str::to_string).collect());
            continue;
        }
        let mut lines = row.text.split('\n');
        let head = lines.next().unwrap_or("");
        let mut item = vec![format!("- {head}")];
        item.extend(lines.map(|line| format!("  {line}")));
        match blocks.last_mut() {
            Some(last) if index > 0 && rows[index - 1].who == ThreadVoice::Tool => last.extend(item),
            _ => blocks.push(item),
        }
    }
    blocks.iter().map(|block| block.join("\n")).collect::<Vec<_>>().join("\n\n")
}

// The thread's latest turn as its transcript ended it, with the time of the row it came off: the done's result
// when the turn replied, else failed with the runtime's reason when the runtime ended it. Nothing when the
// transcript holds no turn of the thread. Read newest first, since an end may follow its done by minutes and an
// older turn's done must not stand in for a newer turn's; a start met before that turn is one the thread began
// afterwards and has not ended, which makes the turn below it the one before the thread's newest.
// thread-read.ts:219
// private in the TS: port it as a private helper
// function latestTurn(events: ReadonlyArray<SessionEvent>, threadId: string): { at?: number; result: TurnResult; running: boolean } | undefined {
//   let end: Extract<SessionEvent, { type: "session.end" }> | undefined;
//   let running = false;
//   for (let i = events.length - 1; i >= 0; i--) {
//     const event = events[i]!;
//     if (!inThread(event, threadId)) continue;
//     if (event.type === "session.end") {
//       if (end !== undefined) break;
//       end = event;
//       continue;
//     }
//     if (event.type === "session.done" && (end === undefined || event.turnId === end.turnId)) return { ...(event.at !== undefined ? { at: event.at } : {}), result: event.result, running };
//     if (event.type === "session.start") {
//       if (end === undefined) running = true;
//       else if (event.turnId === end.turnId) break;
//     }
//   }
//   if (end === undefined) return undefined;
//   return { ...(end.at !== undefined ? { at: end.at } : {}), result: { status: "failed", error: end.reason ?? NO_RESULT_LINE }, running };
// }

/// The result above alone, for a caller that only has to say how the turn went; a thread the transcript holds no
/// turn of answers with the thread's own row instead.
// thread-read.ts:242
// export function threadResult(events: ReadonlyArray<SessionEvent>, threadId: string): TurnResult | undefined {
//   return latestTurn(events, threadId)?.result;
// }
pub fn thread_result(events: &[SessionEvent], thread_id: &str) -> Option<TurnResult> {
    latest_turn(events, thread_id).map(|turn| turn.result)
}

/// The final reply alone, as the thread's finished line carries it: the whole message the thread's latest ended turn
/// left, at the time of the row that carried it. Nothing when the transcript holds no ended turn of the thread. The
/// row is the agent's where the text is the agent's own words and the turn's where it is an error or the footer, so
/// nothing the agent did not say is handed back under its name; a second row says when the thread has since started
/// another turn, since the reply is then the report before the one being written.
// thread-read.ts:251
// export function threadReplyRows(events: ReadonlyArray<SessionEvent>, threadId: string): ThreadMessage[] {
//   const latest = latestTurn(events, threadId);
//   if (latest === undefined) return [];
//   const { result } = latest;
//   const reply = notifyReply(result, "whole");
//   const body = notifyBody(result, "whole") ?? turnEndLine(result);
//   const rows = [message(body === reply ? "agent" : "turn", latest.at, body)];
//   // The note carries no clock: it is not a row the runtime recorded, it is what the transcript says about now.
//   return latest.running ? [...rows, message("turn", undefined, NEWER_TURN_LINE)] : rows;
// }
pub fn thread_reply_rows(events: &[SessionEvent], thread_id: &str) -> Vec<ThreadMessage> {
    let Some(latest) = latest_turn(events, thread_id) else {
        return Vec::new();
    };
    let reply = notify_reply(&latest.result, Some(NotifyLength::Whole));
    let body = notify_body(&latest.result, Some(NotifyLength::Whole)).unwrap_or_else(|| turn_end_line(&latest.result));
    let who = if reply.as_deref() == Some(body.as_str()) { ThreadVoice::Agent } else { ThreadVoice::Turn };
    let mut rows = vec![message(who, latest.at, body)];
    if latest.running {
        rows.push(message(ThreadVoice::Turn, None, NEWER_TURN_LINE.to_string()));
    }
    rows
}

/// Which of the prompts a turn holds open the thread is waiting on: the oldest still open, since that is the one the
/// harness stopped at. The runtime leads a row's `asking` with it and a client answering off the transcript picks the
/// same one, so a listing and the line that answers never name two different questions.
// thread-read.ts:265
// export function leadAsk<T>(open: Iterable<T>): T | undefined {
//   return [...open][0];
// }
pub fn lead_ask<T>(open: impl IntoIterator<Item = T>) -> Option<T> {
    open.into_iter().next()
}

/// The permission prompt this thread has open and nobody has answered, read off the transcript alone, by the rule
/// above; nothing where the thread is waiting on nobody. A client answering a prompt it did not watch arrive reads it
/// here, so the rule that a close ends a prompt is written once.
// thread-read.ts:272
// export function openAsk(events: ReadonlyArray<SessionEvent>, threadId: string): SessionPermissionEvent | undefined {
//   const open = new Map<string, SessionPermissionEvent>();
//   for (const e of events) {
//     if (e.threadId !== threadId) continue;
//     if (e.type === "session.permission") open.set(e.askId, e);
//     if (e.type === "session.permission.closed") open.delete(e.askId);
//   }
//   return leadAsk(open.values());
// }
pub fn open_ask(events: &[SessionEvent], thread_id: &str) -> Option<SessionPermissionEvent> {
    let mut open: IndexMap<String, SessionPermissionEvent> = IndexMap::new();
    for event in events {
        if thread_of(event) != Some(thread_id) {
            continue;
        }
        match event {
            SessionEvent::SessionPermissionEvent(e) => {
                open.insert(e.ask_id.clone(), e.clone());
            }
            SessionEvent::SessionPermissionClosedEvent(e) => {
                open.shift_remove(&e.ask_id);
            }
            _ => {}
        }
    }
    lead_ask(open.into_values())
}

macro_rules! on_each_event {
    ($event:expr, $e:ident => $body:expr) => {
        match $event {
            SessionEvent::SessionStartEvent($e) => $body,
            SessionEvent::SessionDeltaEvent($e) => $body,
            SessionEvent::SessionDoneEvent($e) => $body,
            SessionEvent::SessionEndEvent($e) => $body,
            SessionEvent::SessionSteerEvent($e) => $body,
            SessionEvent::SessionNotifyEvent($e) => $body,
            SessionEvent::SessionPermissionEvent($e) => $body,
            SessionEvent::SessionPermissionClosedEvent($e) => $body,
            SessionEvent::SessionCheckpointEvent($e) => $body,
            SessionEvent::SessionChangesEvent($e) => $body,
            SessionEvent::SessionPlanEvent($e) => $body,
            SessionEvent::SessionCompactedEvent($e) => $body,
            SessionEvent::SessionRetryEvent($e) => $body,
            SessionEvent::SessionContextEvent($e) => $body,
            SessionEvent::SessionRunEvent($e) => $body,
            SessionEvent::SessionMovedEvent($e) => $body,
            SessionEvent::SessionBehindEvent($e) => $body,
            SessionEvent::SessionNoticeEvent($e) => $body,
            SessionEvent::SessionEarlierEvent($e) => $body,
            SessionEvent::SessionCappedEvent($e) => $body,
            SessionEvent::SessionUncappedEvent($e) => $body,
            SessionEvent::SessionSubagentEvent($e) => $body,
            SessionEvent::SessionSlateEvent($e) => $body,
            SessionEvent::ThreadWaitedEvent($e) => $body,
        }
    };
}

// The one variant per event type that carries a thread id; the runtime stamps every event, a row from before the stamp has none.
fn thread_of(event: &SessionEvent) -> Option<&str> {
    on_each_event!(event, e => e.thread_id.thread_str())
}

trait ThreadStr {
    fn thread_str(&self) -> Option<&str>;
}

impl ThreadStr for Option<String> {
    fn thread_str(&self) -> Option<&str> {
        self.as_deref()
    }
}

impl ThreadStr for String {
    fn thread_str(&self) -> Option<&str> {
        Some(self.as_str())
    }
}

fn at_of(event: &SessionEvent) -> Option<f64> {
    on_each_event!(event, e => e.at.as_ref().map(|at| at.0))
}

// A turn's result the runtime wrote for it: failed, with the reason it ended on.
fn failed_result(error: &str) -> TurnResult {
    TurnResult {
        status: TurnStatus::Failed,
        duration_ms: None,
        waited_ms: None,
        cost_usd: None,
        tokens: None,
        model: None,
        models: None,
        text: None,
        error: Some(error.to_string()),
        unreached: None,
        refusal: None,
        limit: None,
    }
}

fn message(who: ThreadVoice, at: Option<f64>, text: String) -> ThreadMessage {
    ThreadMessage { who, at: at.map(JsNumber), text }
}

struct LatestTurn {
    at: Option<f64>,
    result: TurnResult,
    running: bool,
}

// The latest turn, read newest first; see the TS comment above, kept with the reader it serves.
fn latest_turn(events: &[SessionEvent], thread_id: &str) -> Option<LatestTurn> {
    let mut end: Option<&SessionEndEvent> = None;
    let mut running = false;
    for event in events.iter().rev() {
        if thread_of(event) != Some(thread_id) {
            continue;
        }
        match event {
            SessionEvent::SessionEndEvent(e) => {
                if end.is_some() {
                    break;
                }
                end = Some(e);
            }
            SessionEvent::SessionDoneEvent(e) if end.is_none_or(|end| e.turn_id == end.turn_id) => {
                return Some(LatestTurn { at: at_of(event), result: e.result.clone(), running });
            }
            SessionEvent::SessionStartEvent(e) => match end {
                None => running = true,
                Some(end) if e.turn_id == end.turn_id => break,
                Some(_) => {}
            },
            _ => {}
        }
    }
    let end = end?;
    let reason = end.reason.as_deref().unwrap_or(NO_RESULT_LINE);
    Some(LatestTurn { at: end.at.as_ref().map(|at| at.0), result: failed_result(reason), running })
}

#[derive(Default)]
struct Reader {
    rows: Vec<ThreadMessage>,
    // The agent row text deltas append to, and the harness message it is a piece of.
    open: Option<OpenRow>,
    // The running turn's tool rows by the id of their call, since a call's input may arrive in pieces.
    calls: IndexMap<String, Call>,
    saw_text: bool,
    replied: bool,
    steps_row: Option<usize>,
    plan_row: Option<usize>,
}

struct OpenRow {
    row: usize,
    message_id: Option<String>,
}

struct Call {
    row: usize,
    name: String,
    input: String,
}

impl Reader {
    fn say(&mut self, who: ThreadVoice, at: Option<f64>, text: String) -> usize {
        self.open = None;
        self.rows.push(message(who, at, text));
        self.rows.len() - 1
    }

    fn turn(&mut self, at: Option<f64>, result: &TurnResult) {
        if !self.saw_text {
            if let Some(text) = result.text.as_deref().filter(|text| !text.is_empty()) {
                self.say(ThreadVoice::Agent, at, text.to_string());
            }
        }
        self.say(ThreadVoice::Turn, at, turn_end_line(result));
        self.calls.clear();
        self.saw_text = false;
    }

    fn start(&mut self, e: &SessionStartEvent, at: Option<f64>) {
        self.open = None;
        self.calls.clear();
        self.saw_text = false;
        self.replied = false;
        self.steps_row = None;
        self.plan_row = None;
        if let Some(prompt) = &e.prompt {
            self.say(ThreadVoice::Person, at, prompt.clone());
        }
    }

    fn earlier(&mut self, e: &SessionEarlierEvent, at: Option<f64>) {
        let who = match e.who {
            SessionEarlierEventWho::Person => ThreadVoice::Person,
            SessionEarlierEventWho::Agent => ThreadVoice::Agent,
            SessionEarlierEventWho::Tool => ThreadVoice::Tool,
            SessionEarlierEventWho::Note => ThreadVoice::Turn,
        };
        self.say(who, at, e.text.clone());
    }

    fn done(&mut self, e: &SessionDoneEvent, at: Option<f64>) {
        self.replied = true;
        self.turn(at, &e.result);
    }

    fn end(&mut self, e: &SessionEndEvent, at: Option<f64>) {
        let reason = e.reason.as_deref().unwrap_or(NO_RESULT_LINE);
        if e.unstarted.is_some() {
            if let Some(prompt) = &e.prompt {
                self.say(ThreadVoice::Person, at, prompt.clone());
            }
            self.turn(at, &failed_result(reason));
        } else if !self.replied {
            self.turn(at, &failed_result(reason));
        }
    }

    fn plan(&mut self, e: &SessionPlanEvent, at: Option<f64>) {
        if let Some(steps) = &e.steps {
            let steps: Vec<crate::words::turn::PlanStep> = steps
                .iter()
                .map(|step| crate::words::turn::PlanStep {
                    text: step.text.clone(),
                    state: match step.state {
                        PlanStepState::Pending => StepState::Pending,
                        PlanStepState::Working => StepState::Working,
                        PlanStepState::Done => StepState::Done,
                    },
                })
                .collect();
            let line = plan_steps_line(&steps);
            match self.steps_row {
                None => self.steps_row = Some(self.say(ThreadVoice::Tool, at, line)),
                Some(row) => self.rows[row].text = line,
            }
        }
        if let Some(text) = &e.text {
            match self.plan_row {
                None => self.plan_row = Some(self.say(ThreadVoice::Agent, at, text.clone())),
                Some(row) => self.rows[row].text = text.clone(),
            }
        }
    }

    fn changes(&mut self, e: &SessionChangesEvent, at: Option<f64>) {
        let changes = TurnChanges {
            files: e.files.iter().map(|file| FileLines { additions: file.additions.0, deletions: file.deletions.0 }).collect(),
            others: e.others.as_ref().map(|others| others.len()),
            shared: e.shared.is_some(),
        };
        let row = message(ThreadVoice::Tool, at, turn_changes_line(&changes));
        if self.rows.last().is_some_and(|last| last.who == ThreadVoice::Turn) {
            let index = self.rows.len() - 1;
            self.rows.insert(index, row);
        } else {
            self.rows.push(row);
        }
        self.open = None;
    }

    fn delta(&mut self, e: &SessionDeltaEvent, at: Option<f64>) {
        match e.kind {
            DeltaKind::Text => {
                if e.text.is_empty() {
                    return;
                }
                let split = match (self.open.as_ref().and_then(|open| open.message_id.as_ref()), e.message_id.as_ref()) {
                    (Some(open_id), Some(message_id)) => open_id != message_id,
                    _ => false,
                };
                if split {
                    self.open = None;
                }
                match &mut self.open {
                    Some(open) => self.rows[open.row].text.push_str(&e.text),
                    None => {
                        let row = self.rows.len();
                        self.rows.push(message(ThreadVoice::Agent, at, e.text.clone()));
                        self.open = Some(OpenRow { row, message_id: e.message_id.clone() });
                    }
                }
                self.saw_text = true;
            }
            DeltaKind::ToolResult => {
                let answered =
                    e.tool_use_id.as_ref().and_then(|id| self.calls.get(id)).map(|call| (call.row, call.name.clone(), call.input.clone()));
                if let Some((row, name, input)) = answered {
                    if e.is_error != Some(true) {
                        if let Some(did) = tool_done_line(Some(&name), &input) {
                            self.rows[row].text = did;
                        }
                    }
                }
            }
            DeltaKind::ToolUse => {
                self.open = None;
                let key = e.tool_use_id.clone();
                if let Some(call) = key.as_ref().and_then(|key| self.calls.get_mut(key)) {
                    if let Some(name) = &e.tool_name {
                        call.name = name.clone();
                    }
                    call.input.push_str(&e.text);
                    let (row, name, input) = (call.row, call.name.clone(), call.input.clone());
                    self.rows[row].text = tool_activity_line(Some(&name), &input);
                } else {
                    let row = self.say(ThreadVoice::Tool, at, tool_activity_line(e.tool_name.as_deref(), &e.text));
                    if let Some(key) = key {
                        let name = e.tool_name.clone().unwrap_or_else(|| "tool".to_string());
                        self.calls.insert(key, Call { row, name, input: e.text.clone() });
                    }
                }
            }
            DeltaKind::Thinking | DeltaKind::Note => {}
        }
    }
}
