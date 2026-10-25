// turns-help.ts:482-870 (startDetached, follow, waitThrough, restartHost, firstEnded, the reads of a thread and its head)
// called against a scripted host. Run with TZ=UTC. Cases are listed per helper; the runs below call the TS.
import { it } from "vitest";
import { record, recorded, frame, refused, ctxOf, type Case, type Run, type Obj, type Script } from "./lib.ts";

const WS = "ws-1";
const TH = "thread-9d8c7b6a5f4e";
const OTHER = "thread-b0b0b0b0c1c1";
const sess = (extra: Obj = {}): Obj => ({ workspaceId: WS, harness: "claude", status: "running", id: "turn-9", threadId: TH, startedBy: "cli", ...extra });
const started = (outcome = "started", extra: { session?: Obj; notice?: string } = {}): string =>
  frame({ session: sess(extra.session), outcome, turnId: "turn-9", ...(extra.notice !== undefined ? { notice: extra.notice } : {}) });
const ev = (type: string, o: Obj = {}, thread = TH, turn = "turn-9"): string => JSON.stringify({ type, workspaceId: WS, sessionId: turn, threadId: thread, turnId: turn, ...o });
const start = (o: Obj = {}): string => ev("session.start", o);
const done = (result: Obj = { status: "completed", text: "done" }, thread = TH, turn = "turn-9"): string => ev("session.done", { result }, thread, turn);
const end = (o: Obj = {}, thread = TH, turn = "turn-9"): string => ev("session.end", { exitCode: 0, sawResult: true, ...o }, thread, turn);
const delta = (text: string, turn = "turn-9"): string => ev("session.delta", { kind: "text", text }, TH, turn);
const queued = (waitsFor?: string, requestId = "<request id>"): string =>
  JSON.stringify({ type: "session.queued", workspaceId: WS, threadId: TH, harness: "claude", prompt: "and then", ...(requestId !== "" ? { requestId } : {}), ...(waitsFor !== undefined ? { waitsFor } : {}) });
const steer = (o: Obj = {}, requestId = "<request id>"): string => ev("session.steer", { prompt: "and then", requestId, ...o });
const starting = (o: Obj = {}, requestId = "<request id>"): string => JSON.stringify({ type: "session.starting", workspaceId: WS, threadId: TH, harness: "claude", requestId, ...o });

const START: Obj = { workspaceId: WS, prompt: "and then", thread: TH };
const view = (id: string, status: string, extra: Obj = {}): Obj => ({ id, threadId: id, workspaceId: WS, harness: "claude", startedBy: "person", status, title: `title of ${id}`, sessionId: `sess-${id}`, turns: 1, ran: true, ...extra });
const row = (threadId: string, status: string, extra: Obj = {}): Obj => ({ workspaceId: WS, harness: "claude", status, id: `sess-${threadId}`, threadId, ...extra });
const list = (...rows: Obj[]): string => frame({ sessions: rows });
const history = (...events: string[]): string => frame({ events: events.map(e => JSON.parse(e)) });

const own = (replies: Record<string, string>, more: Partial<Script> = {}): Script => ({ replies: { "sessions.start": started(), ...replies }, ...more });

const cases: Case[] = [];
const add = (fn: string, name: string, args: Obj, script: Script, extra: Partial<Case> = {}): void => void cases.push({ fn, case: name, args, script, ...extra });

// ---------------------------------------------------------------------------------------------------- startDetached
const SD = (name: string, script: Script, args: Obj = {}, extra: Partial<Case> = {}): void => add("startDetached", name, { start: START, startedBy: "cli", ...args }, script, extra);
SD("started", { replies: { "sessions.start": started() } });
SD("the fields go out in order, then startedBy, requestId and answerHeld", { replies: { "sessions.start": started() } }, { start: { workspaceId: WS, prompt: "p", harness: "codex", thread: TH, attachments: [{ mediaType: "image/png", name: "a.png", bytes: "AAAA" }], model: "m" } });
SD("started by another origin", { replies: { "sessions.start": started("started", { session: { startedBy: "agent" } }) } }, { startedBy: "agent" });
SD("a notice rides the turn", { replies: { "sessions.start": started("started", { notice: "waking attic\nsecond line" }) } });
SD("queued, hearing one wait", { replies: { "sessions.start": started("queued") }, pushed: { "sessions.start": [queued()] } }, { queued: true });
SD("queued, hearing the wait for a computer", { replies: { "sessions.start": started("queued") }, pushed: { "sessions.start": [queued("hetzner")] } }, { queued: true });
SD("queued, hearing two waits in order", { replies: { "sessions.start": started("queued") }, pushed: { "sessions.start": [queued(undefined), queued("hetzner"), queued(undefined)] } }, { queued: true });
SD("a wait with another request id is not this start's", { replies: { "sessions.start": started("queued") }, pushed: { "sessions.start": [queued(undefined, "someone-else")] } }, { queued: true });
SD("a wait with no request id is not this start's", { replies: { "sessions.start": started("queued") }, pushed: { "sessions.start": [queued(undefined, "")] } }, { queued: true });
SD("no one listening for waits", { replies: { "sessions.start": started("queued") }, pushed: { "sessions.start": [queued()] } });
SD("a steer and a starting are not waits", { replies: { "sessions.start": started("steered") }, pushed: { "sessions.start": [steer(), starting()] } }, { queued: true });
SD("held with its computer's cap", { replies: { "sessions.start": started("held", { session: { capped: { placeId: "p-hetzner", place: "hetzner", running: 2, atOnce: 2 } } }) } });
SD("steered", { replies: { "sessions.start": started("steered") } });
SD("the session names no thread", { replies: { "sessions.start": frame({ session: { workspaceId: WS, harness: "claude", status: "running", id: "turn-9" }, outcome: "started", turnId: "turn-9" }) } });
SD("a reply that is no start answer", { replies: { "sessions.start": frame({ nothing: true }) } });
SD("an outcome the runtime does not know", { replies: { "sessions.start": started("exploded") } });
SD("the host refuses", { replies: { "sessions.start": refused("no such workspace", "not-found") } });
SD("the host refuses with no kind", { replies: { "sessions.start": refused("the machine is gone") } });
SD("the subscription is refused", { replies: { "events.subscribe": refused("events are off") } });
SD("the host stops under the start, no way back", { closes: { "sessions.start": 4001 } });
SD("the host stops under the start, dialled again", { closes: { "sessions.start": 4001 }, redials: [{ replies: { "sessions.start": started() } }] }, { redial: true });
SD("the host stops under the start, dialled again, and the wait is heard", { closes: { "sessions.start": 4001 }, redials: [{ replies: { "sessions.start": started("queued") }, pushed: { "sessions.start": [queued("hetzner")] } }] }, { redial: true, queued: true });
SD("the host stops under the start, the dial is refused", { closes: { "sessions.start": 4001 }, redials: [{ refuses: "no host serving the state file" }] }, { redial: true });
SD("the host stops under the start twice, dialled twice", { closes: { "sessions.start": 4001 }, redials: [{ closes: { "sessions.start": 4001 } }, { replies: { "sessions.start": started() } }] }, { redial: true });
SD("the host stops under the start, then no second dial is left", { closes: { "sessions.start": 4001 }, redials: [{ closes: { "sessions.start": 4001 } }] }, { redial: true });
SD("the host closes with another code under the start", { closes: { "sessions.start": 1006 }, redials: [{ replies: { "sessions.start": started() } }] }, { redial: true });
SD("the host closes with no way back and another code", { closes: { "sessions.start": 1006 } });
SD("the host stops under the subscription", { closes: { "events.subscribe": 4001 }, redials: [{ replies: { "sessions.start": started() } }] }, { redial: true });
SD("the host refuses then is not asked again", { replies: { "sessions.start": refused("no such workspace", "usage") }, redials: [{ replies: { "sessions.start": started() } }] }, { redial: true });

// ---------------------------------------------------------------------------------------------------- follow
const FO = (name: string, script: Script, args: Obj = {}, extra: Partial<Case> = {}): void => add("follow", name, { start: START, startedBy: "cli", ...args }, script, extra);
FO("a turn that completes", { replies: { "sessions.start": started() }, pushed: { "sessions.start": [start(), delta("hel"), delta("lo"), done(), end()] } });
FO("the follow ends at the done: the end after it is not handed on", { replies: { "sessions.start": started() }, pushed: { "sessions.start": [start(), done(), delta("late"), end()] } });
FO("the done carries a whole result", { replies: { "sessions.start": started() }, pushed: { "sessions.start": [done({ status: "failed", text: "x", error: "boom", durationMs: 1200, waitedMs: 100, costUsd: 0.5, refusal: "sign-in" })] } });
FO("the start's followed mark goes last among the start's own fields", { replies: { "sessions.start": started() }, pushed: { "sessions.start": [done()] } }, { start: { workspaceId: WS, prompt: "p", harness: "codex", thread: TH, model: "m" } });
FO("a start after a cut turn", { replies: { "sessions.start": started() }, pushed: { "sessions.start": [start({ afterCut: true }), done()] } });
FO("a start that is not after a cut", { replies: { "sessions.start": started() }, pushed: { "sessions.start": [start({ afterCut: false }), done()] } });
FO("a start of another turn does not mark this one", { replies: { "sessions.start": started() }, pushed: { "sessions.start": [ev("session.start", { afterCut: true }, TH, "turn-8"), start(), done()] } });
FO("an end with the runtime's reason and no done", { replies: { "sessions.start": started() }, pushed: { "sessions.start": [start(), end({ exitCode: null, sawResult: false, reason: "the machine went to sleep" })] } });
FO("an end with no reason and no done", { replies: { "sessions.start": started() }, pushed: { "sessions.start": [start(), end({ exitCode: 1, sawResult: false })] } });
FO("a done after an end of the same turn is not read", { replies: { "sessions.start": started() }, pushed: { "sessions.start": [end({ reason: "cut" }), done()] } });
FO("another turn's done does not end this one", { replies: { "sessions.start": started() }, pushed: { "sessions.start": [done({ status: "completed", text: "earlier" }, TH, "turn-8"), start(), done({ status: "completed", text: "mine" })] } });
FO("another turn's end does not end this one", { replies: { "sessions.start": started() }, pushed: { "sessions.start": [end({ reason: "earlier" }, TH, "turn-8"), done()] } });
FO("a frame that is no session event is not handed on", { replies: { "sessions.start": started() }, pushed: { "sessions.start": [JSON.stringify({ type: "thread.row", turnId: "turn-9" }), JSON.stringify({ type: "sessionish", turnId: "turn-9" }), done()] } });
FO("a session frame with no type string is not handed on", { replies: { "sessions.start": started() }, pushed: { "sessions.start": [JSON.stringify({ turnId: "turn-9" }), done()] } });
FO("events live after the start's answer, in order", { replies: { "sessions.start": started() }, after: { "sessions.start": [start(), delta("a"), done()] } });
FO("queued, the wait and the turn", { replies: { "sessions.start": started("queued") }, pushed: { "sessions.start": [queued(undefined), start(), done()] } });
FO("queued for a computer", { replies: { "sessions.start": started("queued") }, pushed: { "sessions.start": [queued("hetzner"), queued(undefined)] }, after: { "sessions.start": [start(), done()] } });
FO("held", { replies: { "sessions.start": started("held", { session: { capped: { placeId: "p-hetzner", place: "hetzner", running: 2, atOnce: 2 } } }) }, after: { "sessions.start": [start(), done()] } });
FO("the message joined a running turn", { replies: { "sessions.start": started("steered") }, pushed: { "sessions.start": [steer()] }, after: { "sessions.start": [done()] } });
FO("the steer says the turn waits on a permission", { replies: { "sessions.start": started("steered") }, pushed: { "sessions.start": [steer({ waiting: true })] }, after: { "sessions.start": [done()] } });
FO("a steer with another request id is not this start's", { replies: { "sessions.start": started("steered") }, pushed: { "sessions.start": [steer({ waiting: true }, "other")] }, after: { "sessions.start": [done()] } });
FO("the agent is starting", { replies: { "sessions.start": started() }, pushed: { "sessions.start": [starting()] }, after: { "sessions.start": [start(), done()] } });
FO("the agent installs on its first run", { replies: { "sessions.start": started() }, pushed: { "sessions.start": [starting({ installs: true })] }, after: { "sessions.start": [start(), done()] } });
FO("a starting after the session started is not heard", { replies: { "sessions.start": started() }, pushed: { "sessions.start": [starting(), start(), starting()] }, after: { "sessions.start": [done()] } });
FO("a starting after the message steered is not heard", { replies: { "sessions.start": started() }, pushed: { "sessions.start": [starting(), steer(), starting()] }, after: { "sessions.start": [done()] } });
FO("a starting with another request id is not heard", { replies: { "sessions.start": started() }, pushed: { "sessions.start": [starting({}, "other")] }, after: { "sessions.start": [done()] } });
FO("a starting after the turn's own end is not heard", { replies: { "sessions.start": started() }, after: { "sessions.start": [starting(), end({ reason: "cut" })] } });
FO("a notice rides the start", { replies: { "sessions.start": started("started", { notice: "waking attic" }) }, pushed: { "sessions.start": [done()] } });
FO("the session names no thread", { replies: { "sessions.start": frame({ session: { workspaceId: WS, harness: "claude", status: "running", id: "turn-9" }, outcome: "started", turnId: "turn-9" }) } });
FO("a reply that is no start answer", { replies: { "sessions.start": frame({ nothing: true }) } });
FO("the host refuses the start", { replies: { "sessions.start": refused("no such workspace", "not-found") } });
FO("the host stops under the start, no way back", { closes: { "sessions.start": 4001 } });
FO("the host stops under the start, dialled again: the transcript is read there although the start just landed", { closes: { "sessions.start": 4001 }, redials: [{ replies: { "sessions.start": started(), "sessions.history": history() }, pushed: { "sessions.start": [start(), done()] } }] }, { redial: true });
FO("the host stops under the start, dialled again: a done the transcript holds is taken", { closes: { "sessions.start": 4001 }, redials: [{ replies: { "sessions.start": started(), "sessions.history": history(start(), done({ status: "completed", text: "in the transcript" })) } }] }, { redial: true });
FO("the host stops under the start, dialled again, and the transcript read is refused", { closes: { "sessions.start": 4001 }, redials: [{ replies: { "sessions.start": started() } }] }, { redial: true });
FO("the host stops under the start, the dial is refused", { closes: { "sessions.start": 4001 }, redials: [{ refuses: "no host serving the state file" }] }, { redial: true });
FO("the host stops under the turn, the host's transcript holds its done", {
  replies: { "sessions.start": started() },
  pushed: { "sessions.start": [start()] },
  closesAfter: { "sessions.start": 4001 },
  redials: [{ replies: { "sessions.history": history(start(), done({ status: "completed", text: "from the transcript" })) } }],
}, { redial: true });
FO("the host stops under the turn, the transcript holds its end", {
  replies: { "sessions.start": started() },
  closesAfter: { "sessions.start": 4001 },
  redials: [{ replies: { "sessions.history": history(start(), end({ reason: "the machine went to sleep", sawResult: false })) } }],
}, { redial: true });
FO("the host stops under the turn, the transcript holds only another turn's done", {
  replies: { "sessions.start": started() },
  closesAfter: { "sessions.start": 4001 },
  redials: [{ replies: { "sessions.history": history(done({ status: "completed", text: "earlier" }, TH, "turn-8")) }, after: { "sessions.history": [done({ status: "completed", text: "mine" })] } }],
}, { redial: true });
FO("the host stops under the turn, the new host pushes the done while the transcript is read", {
  replies: { "sessions.start": started() },
  closesAfter: { "sessions.start": 4001 },
  redials: [{ replies: { "sessions.history": history(start()) }, pushed: { "sessions.history": [done({ status: "completed", text: "pushed" })] } }],
}, { redial: true });
FO("the host stops under the turn, the new host's transcript has nothing and the turn goes on until its done", {
  replies: { "sessions.start": started() },
  closesAfter: { "sessions.start": 4001 },
  redials: [{ replies: { "sessions.history": history() }, after: { "sessions.history": [start(), done({ status: "completed", text: "later" })] } }],
}, { redial: true });
FO("the host stops under the turn, no way back", { replies: { "sessions.start": started() }, closesAfter: { "sessions.start": 4001 } });
FO("the host stops under the turn, the dial is refused", { replies: { "sessions.start": started() }, closesAfter: { "sessions.start": 4001 }, redials: [{ refuses: "no host serving the state file" }] }, { redial: true });
FO("the host closes under the turn with another code", { replies: { "sessions.start": started() }, closesAfter: { "sessions.start": 1006 }, redials: [{ replies: { "sessions.history": history(done()) } }] }, { redial: true });
FO("the host closes under the turn with no code", { replies: { "sessions.start": started() }, closesAfter: { "sessions.start": 1005 } });
FO("the host stops under the turn twice", {
  replies: { "sessions.start": started() },
  closesAfter: { "sessions.start": 4001 },
  redials: [{ replies: { "sessions.history": history(start()) }, closesAfter: { "sessions.history": 4001 } }, { replies: { "sessions.history": history(start(), done({ status: "completed", text: "third host" })) } }],
}, { redial: true });
FO("the history read fails on the new host", {
  replies: { "sessions.start": started() },
  closesAfter: { "sessions.start": 4001 },
  redials: [{ replies: { "sessions.history": refused("history is gone") } }],
}, { redial: true });
FO("the end carries the runtime's reason and the host stopped under the turn", {
  replies: { "sessions.start": started() },
  pushed: { "sessions.start": [start()] },
  after: { "sessions.start": [end({ reason: "stopped by the person", sawResult: false })] },
});
FO("a turn that never ends hangs", { replies: { "sessions.start": started() }, pushed: { "sessions.start": [start(), delta("x")] } }, {}, { within: 300 });

// ---------------------------------------------------------------------------------------------------- firstEnded
const FE = (name: string, script: Script, args: Obj, extra: Partial<Case> = {}): void => add("firstEnded", name, args, script, extra);
const T1 = view(TH, "running");
const T2 = view(OTHER, "running", { sessionId: "sess-2" });
FE("a named thread the listing no longer holds, nothing in its transcript", { replies: { "sessions.list": list(row(OTHER, "running")), "sessions.history": history() } }, { named: [T1] });
FE("a named thread the listing no longer holds, a done in its transcript", { replies: { "sessions.list": list(row(OTHER, "running")), "sessions.history": history(start(), done({ status: "completed", text: "kept" })) } }, { named: [T1] });
FE("a named thread already over reads its transcript", { replies: { "sessions.list": list(row(TH, "completed")), "sessions.history": history(start(), done({ status: "completed", text: "kept", durationMs: 5 })) } }, { named: [T1] });
FE("a named thread already over, nothing in its transcript, takes the listing's status", { replies: { "sessions.list": list(row(TH, "interrupted")), "sessions.history": history() } }, { named: [T1] });
FE("a named thread already over, failed", { replies: { "sessions.list": list(row(TH, "failed")), "sessions.history": history() } }, { named: [T1] });
FE("two named, the first over in the order named wins", { replies: { "sessions.list": list(row(TH, "completed"), row(OTHER, "failed")), "sessions.history": history() } }, { named: [T2, T1] });
FE("two named, one gone and one over: the gone one is answered first", { replies: { "sessions.list": list(row(TH, "completed")), "sessions.history": history() } }, { named: [T1, T2] });
FE("two named, one running and one over", { replies: { "sessions.list": list(row(TH, "running"), row(OTHER, "completed")), "sessions.history": history() } }, { named: [T1, T2] });
FE("a done pushed for a named thread", { replies: { "sessions.list": list(row(TH, "running"), row(OTHER, "running")) }, pushed: { "sessions.list": [done({ status: "interrupted", error: "stopped by the person", durationMs: 61000 }, OTHER)] } }, { named: [T1, T2] });
FE("a done pushed after the listing is read", { replies: { "sessions.list": list(row(TH, "running")) }, after: { "sessions.list": [done({ status: "completed", text: "late" })] } }, { named: [T1] });
FE("an end pushed, the transcript holds the reply", { replies: { "sessions.list": list(row(TH, "running")), "sessions.history": history(start(), done({ status: "completed", text: "the reply landed first" })) }, pushed: { "sessions.list": [end({ reason: "cut" })] } }, { named: [T1] });
FE("an end pushed with a reason, the transcript holds nothing", { replies: { "sessions.list": list(row(TH, "running")), "sessions.history": history() }, pushed: { "sessions.list": [end({ reason: "the machine went to sleep", sawResult: false })] } }, { named: [T1] });
FE("an end pushed with no reason, the transcript holds nothing", { replies: { "sessions.list": list(row(TH, "running")), "sessions.history": history() }, pushed: { "sessions.list": [end({ sawResult: false })] } }, { named: [T1] });
FE("an end pushed reads the workspace the end names", { replies: { "sessions.list": list(row(TH, "running")), "sessions.history": history() }, pushed: { "sessions.list": [JSON.stringify({ type: "session.end", workspaceId: "ws-9", sessionId: "s", threadId: TH, turnId: "t", exitCode: null, sawResult: false })] } }, { named: [T1] });
FE("a done for a thread not named is not an answer", { replies: { "sessions.list": list(row(TH, "running")) }, pushed: { "sessions.list": [done({ status: "completed" }, "thread-zzz")] } }, { named: [T1], timeoutMs: 60 });
FE("a done with no thread is not an answer", { replies: { "sessions.list": list(row(TH, "running")) }, pushed: { "sessions.list": [JSON.stringify({ type: "session.done", workspaceId: WS, sessionId: "s", turnId: "t", result: { status: "completed" } })] } }, { named: [T1], timeoutMs: 60 });
FE("a frame that is not a session frame is not an answer", { replies: { "sessions.list": list(row(TH, "running")) }, pushed: { "sessions.list": [JSON.stringify({ type: "thread.row", threadId: TH })] } }, { named: [T1], timeoutMs: 60 });
FE("the deadline passes first", { replies: { "sessions.list": list(row(TH, "running")) } }, { named: [T1], timeoutMs: 60 });
FE("a deadline of zero", { replies: { "sessions.list": list(row(TH, "running")) } }, { named: [T1], timeoutMs: 0 });
FE("an answer before the deadline", { replies: { "sessions.list": list(row(TH, "completed")), "sessions.history": history() } }, { named: [T1], timeoutMs: 5000 });
FE("no deadline and nothing to hear", { replies: { "sessions.list": list(row(TH, "running")) } }, { named: [T1] }, { within: 300 });
FE("the socket stops while waiting", { replies: { "sessions.list": list(row(TH, "running")) }, closesAfter: { "sessions.list": 4001 } }, { named: [T1] });
FE("the socket closes while waiting", { replies: { "sessions.list": list(row(TH, "running")) }, closesAfter: { "sessions.list": 1006 } }, { named: [T1] });
FE("the socket stops during the listing", { closes: { "sessions.list": 4001 } }, { named: [T1] });
FE("the listing is refused", { replies: { "sessions.list": refused("no listing") } }, { named: [T1] });
FE("the subscription is refused", { replies: { "events.subscribe": refused("events are off") } }, { named: [T1] });
FE("the transcript read is refused for a gone thread", { replies: { "sessions.list": list(), "sessions.history": refused("no transcript") } }, { named: [T1] });
FE("a named thread with only an id", { replies: { "sessions.list": list(row("t-solo", "completed")), "sessions.history": history() } }, { named: [{ id: "t-solo", workspaceId: WS, harness: "claude", startedBy: "person", status: "running", title: "solo", sessionId: "sess-solo", turns: 1, ran: false }] });
FE("no thread named, the listing empty", { replies: { "sessions.list": list() } }, { named: [] }, { within: 300 });

// ---------------------------------------------------------------------------------------------------- waitThrough
const WT = (name: string, script: Script, args: Obj, extra: Partial<Case> = {}): void => add("waitThrough", name, args, script, extra);
WT("one already over", { replies: { "sessions.list": list(row(TH, "completed")), "sessions.history": history(start(), done()) } }, { named: [T1] });
WT("a done pushed", { replies: { "sessions.list": list(row(TH, "running")) }, pushed: { "sessions.list": [done({ status: "completed", text: "hi" })] } }, { named: [T1] });
WT("the deadline passes", { replies: { "sessions.list": list(row(TH, "running")) } }, { named: [T1], timeoutMs: 60 });
WT("a sub-thread's end carries what its machine was read to hold", {
  replies: { "sessions.list": list(row(TH, "completed", { sub: { state: "napping", on: "boat", machine: "fix it", size: { cpu: 2, memMb: 4096 }, rateUsdPerHour: 0.018, branch: "wsp/fix-it" } })), "sessions.history": history(start(), done()), "sessions.head": JSON.stringify({ id: 1, ok: true, facts: { id: TH, threadId: TH, workspaceId: WS, harness: "claude", startedBy: "agent", status: "completed", turns: 1, ran: true, sub: { state: "napping", on: "boat", machine: "fix it", size: { cpu: 2, memMb: 4096 }, rateUsdPerHour: 0.018, branch: "wsp/fix-it", result: { branch: "wsp/fix-it", pushedSha: "9c2e1d4b7a0f3e8c5d6a1b2c3d4e5f6a7b8c9d0e", commits: 3, unpushed: 0, changed: 0, onBranch: "wsp/fix-it", readAt: "2026-10-12T09:44:10.512Z" } } }, events: [], pos: 0, total: 0 }) },
}, { named: [view(TH, "completed", { sub: { state: "napping", on: "boat", machine: "fix it", size: { cpu: 2, memMb: 4096 }, rateUsdPerHour: 0.018, branch: "wsp/fix-it" } })] });
WT("a sub-thread's end, its machine has no result yet", {
  replies: { "sessions.list": list(row(TH, "completed", { sub: { state: "napping", on: "boat", machine: "fix it", size: { cpu: 2, memMb: 4096 }, rateUsdPerHour: 0.018, branch: "wsp/fix-it" } })), "sessions.history": history(start(), done()), "sessions.head": JSON.stringify({ id: 1, ok: true, facts: { id: TH, threadId: TH, workspaceId: WS, harness: "claude", startedBy: "agent", status: "completed", turns: 1, ran: true }, events: [], pos: 0, total: 0 }) },
}, { named: [view(TH, "completed", { sub: { state: "napping", on: "boat", machine: "fix it", size: { cpu: 2, memMb: 4096 }, rateUsdPerHour: 0.018, branch: "wsp/fix-it" } })] });
WT("a sub-thread that is not named a sub-thread asks nothing more", { replies: { "sessions.list": list(row(TH, "completed")), "sessions.history": history(start(), done()) } }, { named: [T1] });
WT("the host stops under the wait and the one that comes back answers", { closes: { "sessions.list": 4001 }, redials: [{ replies: { "sessions.list": list(row(TH, "completed")), "sessions.history": history(start(), done({ status: "completed", text: "after the restart" })) } }] }, { named: [T1] });
WT("the host stops under the wait twice", { closes: { "sessions.list": 4001 }, redials: [{ closes: { "sessions.list": 4001 } }, { replies: { "sessions.list": list(row(TH, "completed")), "sessions.history": history() } }] }, { named: [T1] });
WT("the host stops after listing, the wait goes on at the next host", { replies: { "sessions.list": list(row(TH, "running")) }, closesAfter: { "sessions.list": 4001 }, redials: [{ replies: { "sessions.list": list(row(TH, "running")) }, pushed: { "sessions.list": [done({ status: "completed", text: "there" })] } }] }, { named: [T1] });
WT("the host closes under the wait with another code", { closes: { "sessions.list": 1006 }, redials: [{ replies: { "sessions.list": list(row(TH, "completed")), "sessions.history": history() } }] }, { named: [T1] });
WT("the host stops and the dial fails once, then answers", { closes: { "sessions.list": 4001 }, redials: [{ refuses: "no host serving the state file" }, { replies: { "sessions.list": list(row(TH, "completed")), "sessions.history": history() } }] }, { named: [T1] }, { within: 5000 });
WT("the host stops and never comes back, no deadline", { closes: { "sessions.list": 4001 } }, { named: [T1] }, { within: 5000 });
WT("the host stops and never comes back before the deadline", { closes: { "sessions.list": 4001 } }, { named: [T1], timeoutMs: 100 }, { within: 5000 });
WT("the host stops and the next host runs out the deadline", { closes: { "sessions.list": 4001 }, redials: [{ replies: { "sessions.list": list(row(TH, "running")) } }] }, { named: [T1], timeoutMs: 100 }, { within: 5000 });
WT("the list is refused", { replies: { "sessions.list": refused("no listing") } }, { named: [T1] });
WT("no thread named", { replies: { "sessions.list": list() } }, { named: [], timeoutMs: 60 });

// ---------------------------------------------------------------------------------------------------- restartHost
const RH = (name: string, script: Script, extra: Partial<Case> = {}): void => add("restartHost", name, {}, script, extra);
RH("the host restarts and nothing runs", { replies: { "host.restart": frame({}) }, closesAfter: { "host.restart": 4001 }, redials: [{ replies: { "sessions.list": list(row(TH, "completed")) } }] });
RH("the host restarts and one thread runs", { replies: { "host.restart": frame({}) }, closesAfter: { "host.restart": 4001 }, redials: [{ replies: { "sessions.list": list(row(TH, "running"), row(OTHER, "completed")) } }] });
RH("the host restarts and two threads run, in the listing's order", { replies: { "host.restart": frame({}) }, closesAfter: { "host.restart": 4001 }, redials: [{ replies: { "sessions.list": list(row(OTHER, "running"), row(TH, "running")) } }] });
RH("the host closes with another code on its way down", { replies: { "host.restart": frame({}) }, closesAfter: { "host.restart": 1006 }, redials: [{ replies: { "sessions.list": list() } }] });
RH("the host closes before it answers", { closes: { "host.restart": 4001 }, redials: [{ replies: { "sessions.list": list() } }] });
RH("the restart is refused in the road's own words", { replies: { "host.restart": refused("this host was not started by a service", "usage") } });
RH("the host comes back after a refused dial", { replies: { "host.restart": frame({}) }, closesAfter: { "host.restart": 4001 }, redials: [{ refuses: "no host serving the state file" }, { replies: { "sessions.list": list(row(TH, "running")) } }] }, { within: 5000 });
RH("the host never comes back", { replies: { "host.restart": frame({}) }, closesAfter: { "host.restart": 4001 } }, { within: 5000 });
RH("the new host's listing is refused", { replies: { "host.restart": frame({}) }, closesAfter: { "host.restart": 4001 }, redials: [{ replies: { "sessions.list": refused("no listing") } }] });
RH("the host took the restart and kept serving", { replies: { "host.restart": frame({}) } }, { within: 30000 });

// ---------------------------------------------------------------------------------------------------- the reads of a thread
const msgs = recorded("thread_read", "messages");
const readOf = (name: string): Record<string, string> => recorded("thread_read", name);
// The TS casts a history's events without reading them, and the recorded MCP replies hold events with no scope fields; a real
// host's events always carry workspaceId and sessionId, so they are put in (the TS reads the same words either way).
const scoped = (events: Obj[]): Obj[] =>
  events.map(e => ({
    workspaceId: "ws-1",
    sessionId: "s-1",
    ...(e["type"] === "session.delta" ? { text: "" } : {}),
    ...(e["type"] === "session.end" ? { exitCode: null, sawResult: false } : {}),
    ...e,
  }));
const evs = (name: string): Obj[] => scoped((JSON.parse(readOf(name)["sessions.history"]!) as { events: Obj[] }).events);
const scopedHist = (name: string): string => frame({ events: evs(name) });
const RT = (name: string, script: Script, args: Obj): void => add("readThread", name, args, script);
const T_READ = view("t-1111aaaa", "completed", { workspaceId: "ws-1" });
RT("the messages", { replies: { "sessions.history": scopedHist("messages"), "sessions.read": frame({}) } }, { thread: T_READ, last: false });
RT("the final reply", { replies: { "sessions.history": scopedHist("last"), "sessions.read": frame({}) } }, { thread: T_READ, last: true });
RT("the final reply of a thread with a newer turn", { replies: { "sessions.history": scopedHist("last with a newer turn"), "sessions.read": frame({}) } }, { thread: T_READ, last: true });
RT("the final reply the runtime cut", { replies: { "sessions.history": scopedHist("last cut by the runtime"), "sessions.read": frame({}) } }, { thread: T_READ, last: true });
RT("no words in the reply", { replies: { "sessions.history": scopedHist("last with no words"), "sessions.read": frame({}) } }, { thread: T_READ, last: true });
RT("an empty transcript", { replies: { "sessions.history": history(), "sessions.read": frame({}) } }, { thread: T_READ, last: false });
RT("an empty transcript, the last reply", { replies: { "sessions.history": history(), "sessions.read": frame({}) } }, { thread: T_READ, last: true });
RT("the transcript is read before the stamp moves", { replies: { "sessions.history": history(), "sessions.read": refused("no stamp") } }, { thread: T_READ, last: false });
RT("the transcript read is refused", { replies: { "sessions.history": refused("no transcript"), "sessions.read": frame({}) } }, { thread: T_READ, last: false });
RT("a thread with only an id", { replies: { "sessions.history": history(), "sessions.read": frame({}) } }, { thread: { id: "t-solo", workspaceId: WS, harness: "claude", startedBy: "person", status: "completed", title: "solo", sessionId: "sess-solo", turns: 1, ran: false }, last: false });

const RL = (name: string, args: Obj): void => add("readLine", name, args, {});
RL("messages", { threadId: "t-1111aaaa", events: evs("messages"), last: false });
RL("the final reply", { threadId: "t-1111aaaa", events: evs("last"), last: true });
RL("no messages", { threadId: "t-1111aaaa", events: [], last: false });
RL("no reply", { threadId: "t-1111aaaa", events: [], last: true });
RL("messages but no reply", { threadId: "t-1111aaaa", events: evs("messages"), last: true, otherThread: true });

const heads = (name: string): Record<string, string> => recorded("thread_head", name);
const TH_OF = (name: string): Obj => {
  const h = JSON.parse(heads(name)["sessions.head"]!) as Obj;
  return { facts: h["facts"], events: h["events"], pos: h["pos"], total: h["total"] };
};
add("threadHead", "a head", { thread: T_READ }, { replies: { "sessions.head": heads("head")["sessions.head"]! } });
add("threadHead", "a head keeps only its four parts", { thread: T_READ }, { replies: { "sessions.head": JSON.stringify({ id: 1, ok: true, ...TH_OF("head"), extra: "dropped" }) } });
add("threadHead", "a head with no events", { thread: T_READ }, { replies: { "sessions.head": heads("no model, access or folder, and no events")["sessions.head"]! } });
add("threadHead", "the host refuses", { thread: T_READ }, { replies: { "sessions.head": refused("no such thread", "not-found") } });
for (const name of ["head", "a sub-thread on Boat", "no model, access or folder, and no events"]) add("headLine", name, { head: TH_OF(name) }, {});

// ---------------------------------------------------------------------------------------------------- words
for (const ms of [0, 1, 999, 1000, 20000, 61000, 3600000]) add("hostDidNotStopLine", `${ms} ms`, { waitMs: ms }, {});
for (const running of [[], ["aaaaaaaa-1111"], ["short"], ["abcdefgh12345678", "12345678abcdefgh"], ["a", "b", "c"]]) add("hostRestartedLine", `${running.length} running ${running.join(",")}`, { running }, {});

// ---------------------------------------------------------------------------------------------------- runs
type Mod = Obj;
const T = (mod: Mod): Obj => mod["turns"] as Obj;
const call = <R>(f: unknown, ...a: unknown[]): Promise<R> => (f as (...x: unknown[]) => Promise<R>)(...a);
const turnJson = (t: Obj | undefined): unknown =>
  t === undefined
    ? undefined
    : { session: t["session"], threadId: t["threadId"], outcome: t["outcome"], ...(t["result"] !== undefined ? { result: t["result"] } : {}), ...(t["reason"] !== undefined ? { reason: t["reason"] } : {}), ...(t["afterCut"] === true ? { afterCut: true } : {}), ...(t["notice"] !== undefined ? { notice: t["notice"] } : {}) };
const summary = (t: Obj): string => `thread=${String(t["threadId"])} outcome=${String(t["outcome"])} result=${String((t["result"] as Obj | undefined)?.["status"] ?? "-")} reason=${String(t["reason"] ?? "-")} afterCut=${t["afterCut"] === true}`;

const runs: Record<string, Run> = {
  startDetached: async ({ mod, client, deps, args, sinks }) =>
    turnJson(
      await call<Obj>(T(mod)["startDetached"], client, args["start"], args["startedBy"], args["queued"] === true ? (e: Obj) => sinks.said.push(`queued ${String(e["waitsFor"] ?? "-")}`) : undefined, args["redial"] === true ? () => (deps["redial"] as () => Promise<Obj>)() : undefined),
    ),
  follow: async ({ mod, client, deps, args, sinks }) => {
    const on = {
      queued: (e: Obj) => sinks.said.push(`queued ${String(e["waitsFor"] ?? "-")}`),
      starting: (e: Obj) => sinks.said.push(`starting ${String(e["harness"])} installs=${e["installs"] === true}`),
      steered: (e: Obj) => sinks.said.push(`steered waiting=${e["waiting"] === true}`),
      started: (t: Obj) => sinks.said.push(`started ${summary(t)} notice=${String(t["notice"] ?? "-")}`),
      redialed: () => sinks.said.push("redialed"),
      event: (e: Obj, t: Obj) => sinks.said.push(`event ${String(e["type"])} ${summary(t)}`),
    };
    return turnJson(await call<Obj>(T(mod)["follow"], client, args["start"], args["startedBy"], on, args["redial"] === true ? () => (deps["redial"] as () => Promise<Obj>)() : undefined));
  },
  firstEnded: ({ mod, client, args }) => call(T(mod)["firstEnded"], client, args["named"], args["timeoutMs"]),
  waitThrough: ({ mod, deps, args, sinks }) => call(T(mod)["waitThrough"], deps, args["named"], args["timeoutMs"], () => sinks.said.push("redialed")),
  restartHost: ({ mod, deps }) => call(T(mod)["restartHost"], deps),
  readThread: ({ mod, client, args }) => call(T(mod)["readThread"], client, args["thread"], args["last"]),
  readLine: async ({ mod, args, proto }) => {
    const threadId = args["otherThread"] === true ? "t-other" : (args["threadId"] as string);
    const events = args["events"] as unknown[];
    const f = (proto["threadReplyRows"] as (e: unknown[], t: string) => unknown)(events, threadId);
    const rows = args["last"] === true ? f : (proto["threadMessages"] as (e: unknown[], t: string) => unknown)(events, threadId);
    return (T(mod)["readLine"] as (r: Obj, l: boolean) => string)({ threadId, messages: rows }, args["last"] === true);
  },
  threadHead: ({ mod, client, args }) => call(T(mod)["threadHead"], client, args["thread"]),
  headLine: ({ mod, args }) => (T(mod)["headLine"] as (h: unknown) => string)(args["head"]),
  hostDidNotStopLine: ({ mod, args }) => Promise.resolve((T(mod)["hostDidNotStopLine"] as (n: number) => string)(args["waitMs"] as number)),
  hostRestartedLine: ({ mod, args }) => Promise.resolve((T(mod)["hostRestartedLine"] as (r: string[]) => string)(args["running"] as string[])),
};

it("records turns_follow", async () => {
  await record("turns_follow", cases, runs);
});
void ctxOf;
