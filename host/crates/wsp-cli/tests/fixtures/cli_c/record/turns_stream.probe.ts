// turns-help.ts:832-1173 (why a turn failed, what a watched turn prints, the answers to its prompts, followVerb,
// detachVerb) called against a scripted host. Run with TZ=UTC.
import { it } from "vitest";
import { record, frame, refused, ctxOf, type Case, type Run, type Obj, type Script, type IoOpts } from "./lib.ts";

const WS = "ws-1";
const TH = "thread-9d8c7b6a5f4e";
const sess = (extra: Obj = {}): Obj => ({ workspaceId: WS, harness: "claude", status: "running", id: "turn-9", threadId: TH, startedBy: "cli", ...extra });
const started = (outcome = "started", extra: { session?: Obj; notice?: string } = {}): string =>
  frame({ session: sess(extra.session), outcome, turnId: "turn-9", ...(extra.notice !== undefined ? { notice: extra.notice } : {}) });
const ev = (type: string, o: Obj = {}, turn = "turn-9"): string => JSON.stringify({ type, workspaceId: WS, sessionId: turn, threadId: TH, turnId: turn, ...o });
const start = (o: Obj = {}): string => ev("session.start", o);
const done = (result: Obj = { status: "completed", text: "done" }): string => ev("session.done", { result });
const end = (o: Obj = {}): string => ev("session.end", { exitCode: 0, sawResult: true, ...o });
const text = (t: string, messageId?: string): string => ev("session.delta", { kind: "text", text: t, ...(messageId !== undefined ? { messageId } : {}) });
const note = (t: string): string => ev("session.delta", { kind: "note", text: t });
const toolUse = (t: string, id: string, name = "Bash"): string => ev("session.delta", { kind: "tool_use", text: t, toolUseId: id, toolName: name });
const toolResult = (t: string, id?: string, isError?: boolean): string => ev("session.delta", { kind: "tool_result", text: t, ...(id !== undefined ? { toolUseId: id } : {}), ...(isError !== undefined ? { isError } : {}) });
const queued = (waitsFor?: string): string => JSON.stringify({ type: "session.queued", workspaceId: WS, threadId: TH, harness: "claude", prompt: "p", requestId: "<request id>", ...(waitsFor !== undefined ? { waitsFor } : {}) });
const steer = (o: Obj = {}): string => ev("session.steer", { prompt: "p", requestId: "<request id>", ...o });
const starting = (o: Obj = {}): string => JSON.stringify({ type: "session.starting", workspaceId: WS, threadId: TH, harness: "claude", requestId: "<request id>", ...o });
const OPT_ALLOW = { id: "opt-allow", label: "Allow", effect: "allow" };
const OPT_DENY = { id: "opt-deny", label: "Deny", effect: "deny" };
const OPT_MODE = { id: "opt-mode", label: "Accept edits for the rest of the turn", effect: "mode", mode: "acceptEdits" };
const OPT_ANSWER = { id: "opt-a", label: "Use the blue one", effect: "answer" };
const permission = (options: Obj[], o: Obj = {}): string => ev("session.permission", { askId: "ask-1", toolName: "Bash", input: JSON.stringify({ command: "rm -rf build" }), detail: "rm -rf build", options, ...o });
const closed = (askId = "ask-1"): string => ev("session.permission.closed", { askId, outcome: "allowed", optionId: "opt-allow" });
const history = (...events: string[]): string => frame({ events: events.map(e => JSON.parse(e)) });
const row = (threadId: string, status: string, extra: Obj = {}): Obj => ({ workspaceId: WS, harness: "claude", status, id: `sess-${threadId}`, threadId, ...extra });
const list = (...rows: Obj[]): string => frame({ sessions: rows });
const START: Obj = { workspaceId: WS, prompt: "and then", thread: TH };
const view = (o: Obj = {}): Obj => ({ session: sess(), threadId: TH, outcome: "started", ...o });

const cases: Case[] = [];
const add = (fn: string, name: string, args: Obj, script: Script = {}, extra: Partial<Case> = {}): void => void cases.push({ fn, case: name, args, script, ...extra });

// ------------------------------------------------------------------------------------------------ turnFailure, turnRefusal
const TURNS: [string, Obj][] = [
  ["completed", view({ result: { status: "completed", text: "ok" } })],
  ["completed with an error text", view({ result: { status: "completed", error: "odd" } })],
  ["failed with the agent's error", view({ result: { status: "failed", error: "boom" } })],
  ["failed with an empty error", view({ result: { status: "failed", error: "" } })],
  ["failed with no error, the runtime's reason", view({ result: { status: "failed" }, reason: "the machine went to sleep" })],
  ["failed with no error and no reason", view({ result: { status: "failed" } })],
  ["interrupted", view({ result: { status: "interrupted" } })],
  ["interrupted with a reason", view({ result: { status: "interrupted" }, reason: "stopped by the person" })],
  ["no result, a reason", view({ reason: "cut" })],
  ["no result, no reason", view()],
  ["no result, an empty reason", view({ reason: "" })],
  ["refused for a sign-in", view({ result: { status: "failed", error: "not logged in", refusal: "sign-in" } })],
  ["refused for a sign-in with no words", view({ result: { status: "failed", refusal: "sign-in" } })],
  ["refused for a sign-in, interrupted", view({ result: { status: "interrupted", error: "gone", refusal: "sign-in" } })],
];
for (const [name, turn] of TURNS) {
  add("turnFailure", name, { turn });
  add("turnRefusal", name, { turn });
}

// ------------------------------------------------------------------------------------------------ the answer lines
const OPTS: [string, Obj[]][] = [
  ["allow, deny and a mode", [OPT_ALLOW, OPT_DENY, OPT_MODE]],
  ["the roads' order, not the options'", [OPT_MODE, OPT_DENY, OPT_ALLOW]],
  ["allow only", [OPT_ALLOW]],
  ["deny only", [OPT_DENY]],
  ["a mode only", [OPT_MODE]],
  ["a mode with a label that starts in a capital outside ASCII", [{ id: "m", label: "Édit freely", effect: "mode" }]],
  ["a mode with an empty label", [{ id: "m", label: "", effect: "mode" }]],
  ["a mode with a label already lower", [{ id: "m", label: "never ask again", effect: "mode" }]],
  ["a mode whose label starts with a capital outside the first plane: JS lowers one UTF-16 unit", [{ id: "m", label: "\u{10400}bc", effect: "mode" }]],
  ["a mode whose label starts with a capital that lowers to two characters", [{ id: "m", label: "\u0130stanbul", effect: "mode" }]],
  ["a mode whose label starts with a digit", [{ id: "m", label: "3 more", effect: "mode" }]],
  ["two allows, the first stands", [{ id: "a1", label: "Allow once", effect: "allow" }, { id: "a2", label: "Allow always", effect: "allow" }]],
  ["no road at all: the prompt's own answers", [OPT_ANSWER]],
  ["no option", []],
];
for (const [name, options] of OPTS) add("answerKeysLine", name, { options });
for (const id of ["thread-9d8c7b6a5f4e", "abc", "", "t-1111aaaa", "thread-b0b0b0b0c1c1"]) add("answerVerbsLine", `thread ${id === "" ? "(empty)" : id}`, { threadId: id });
for (const [name, ask, said] of [
  ["a command", { toolName: "Bash", input: JSON.stringify({ command: "pnpm test" }), detail: "pnpm test" }, "allowed"],
  ["a command with no detail", { toolName: "Bash", input: JSON.stringify({ command: "pnpm test" }) }, "denied"],
  ["an edit", { toolName: "Edit", input: JSON.stringify({ file_path: "/src/a.ts", old_string: "a", new_string: "b" }) }, "allowed"],
  ["a tool the words do not know", { toolName: "mcp__linear__search", input: JSON.stringify({ query: "bug" }) }, "allowed"],
  ["an input that is no JSON", { toolName: "Bash", input: "not json" }, "denied"],
] as [string, Obj, string][]) add("answeredLine", name, { threadId: TH, ask, said });
for (const id of [TH, "short"]) {
  add("noOpenAskLine", id, { threadId: id });
  add("noSuchAnswerLine", `${id} allow`, { threadId: id, verb: "allow" });
  add("noSuchAnswerLine", `${id} deny`, { threadId: id, verb: "deny" });
}
add("answerRoads", "the table", {});
add("answerInTheApp", "the words", {});

// ------------------------------------------------------------------------------------------------ answerOpenAsk
const AO = (name: string, script: Script, args: Obj): void => add("answerOpenAsk", name, { ref: TH, road: "allow", ...args }, script);
const ASKED = (options: Obj[], extra: Obj = {}): Record<string, string> => ({ "sessions.list": list(row(TH, "running")), "sessions.history": history(start(), permission(options, extra)) });
AO("allow", { replies: { ...ASKED([OPT_ALLOW, OPT_DENY, OPT_MODE]), "sessions.answer": frame({ outcome: "answered" }) } }, {});
AO("deny with the person's words", { replies: { ...ASKED([OPT_ALLOW, OPT_DENY]), "sessions.answer": frame({ outcome: "answered" }) } }, { road: "deny", reason: "use the other folder" });
AO("deny with no words", { replies: { ...ASKED([OPT_ALLOW, OPT_DENY]), "sessions.answer": frame({ outcome: "answered" }) } }, { road: "deny" });
AO("deny with an empty reason", { replies: { ...ASKED([OPT_ALLOW, OPT_DENY]), "sessions.answer": frame({ outcome: "answered" }) } }, { road: "deny", reason: "" });
AO("allow by a prefix of the thread", { replies: { ...ASKED([OPT_ALLOW]), "sessions.answer": frame({ outcome: "answered" }) } }, { ref: "thread-9d8c" });
AO("the first allow option is the one picked", { replies: { ...ASKED([{ id: "a1", label: "Allow once", effect: "allow" }, { id: "a2", label: "Allow always", effect: "allow" }]), "sessions.answer": frame({ outcome: "answered" }) } }, {});
AO("the thread waits on no prompt", { replies: { "sessions.list": list(row(TH, "running")), "sessions.history": history(start()) } }, {});
AO("the prompt was closed", { replies: { "sessions.list": list(row(TH, "running")), "sessions.history": history(start(), permission([OPT_ALLOW]), closed()) } }, {});
AO("a prompt of another thread is not this one's", { replies: { "sessions.list": list(row(TH, "running")), "sessions.history": history(JSON.stringify({ type: "session.permission", workspaceId: WS, sessionId: "s", threadId: "thread-other", askId: "ask-9", toolName: "Bash", input: "{}", options: [OPT_ALLOW] })) } }, {});
AO("two prompts open, one closed", { replies: { "sessions.list": list(row(TH, "running")), "sessions.history": history(permission([OPT_ALLOW], { askId: "ask-1" }), permission([{ ...OPT_ALLOW, id: "second-allow" }], { askId: "ask-2" }), closed("ask-1")), "sessions.answer": frame({ outcome: "answered" }) } }, {});
AO("two prompts open", { replies: { "sessions.list": list(row(TH, "running")), "sessions.history": history(permission([OPT_ALLOW], { askId: "ask-1" }), permission([{ ...OPT_ALLOW, id: "second-allow" }], { askId: "ask-2" })), "sessions.answer": frame({ outcome: "answered" }) } }, {});
AO("the prompt takes no deny", { replies: ASKED([OPT_ALLOW]) }, { road: "deny" });
AO("the prompt carries only its own answers", { replies: ASKED([OPT_ANSWER]) }, {});
AO("the answer lands too late", { replies: { ...ASKED([OPT_ALLOW]), "sessions.answer": frame({ outcome: "gone" }) } }, {});
AO("the agent raises no prompt the host can answer", { replies: { ...ASKED([OPT_ALLOW]), "sessions.answer": frame({ outcome: "unsupported" }) } }, {});
AO("the host holds no turn of that thread", { replies: { ...ASKED([OPT_ALLOW]), "sessions.answer": frame({ outcome: "not-found" }) } }, {});
AO("the prompt has no option by that id", { replies: { ...ASKED([OPT_ALLOW]), "sessions.answer": frame({ outcome: "no-option" }) } }, {});
AO("the host refuses the answer", { replies: { ...ASKED([OPT_ALLOW]), "sessions.answer": refused("not yours", "auth") } }, {});
AO("no such thread", { replies: { "sessions.list": list(row("thread-zzz", "running")) } }, {});
AO("the transcript is refused", { replies: { "sessions.list": list(row(TH, "running")), "sessions.history": refused("no transcript") } }, {});

// ------------------------------------------------------------------------------------------------ followVerb
const FV = (name: string, script: Script, args: Obj = {}, extra: Partial<Case> = {}, both = true): void => {
  add("followVerb", name, { start: START, announce: false, json: false, ...args }, script, extra);
  if (both) add("followVerb", `${name} (--json)`, { start: START, announce: false, json: true, ...args }, script, extra);
};
const SCRIPT = (events: string[], more: Partial<Script> = {}, reply = started()): Script => ({ replies: { "sessions.start": reply }, pushed: { "sessions.start": events }, ...more });
FV("prose, then the done", SCRIPT([start(), text("Hello "), text("there.\n"), done({ status: "completed", text: "Hello there.\n", durationMs: 1200, costUsd: 0.12 })]));
FV("prose with no trailing newline", SCRIPT([start(), text("Hello"), done({ status: "completed", text: "Hello" })]));
FV("prose on the person's own screen", SCRIPT([start(), text("Hello\n"), done({ status: "completed", text: "Hello\n" })]), { io: { sameScreen: true } satisfies IoOpts });
FV("no prose streamed, on the person's own screen", SCRIPT([start(), done({ status: "completed", text: "the reply" })]), { io: { sameScreen: true } });
FV("a done with no text", SCRIPT([start(), done({ status: "completed" })]));
FV("muted asides", SCRIPT([start(), note("the harness says hi"), ev("session.compacted", { before: 90000, after: 20000 }), ev("session.retry", { attempt: 2, of: 5, why: "overloaded" }), done()]), { io: { muted: true } });
FV("asides with no ink", SCRIPT([start(), note("the harness says hi"), ev("session.compacted", {}), ev("session.retry", { attempt: 1 }), done()]));
FV("a second message opens its own paragraph", SCRIPT([start(), text("first part", "m1"), text(" more", "m1"), text("second message", "m2"), done()]));
FV("a second message after a line stands apart already", SCRIPT([start(), text("first\n", "m1"), text("second", "m2"), done()]));
FV("a line of work between two messages", SCRIPT([start(), text("before", "m1"), toolUse("{\"command\":\"ls\"}", "tu-1"), text("after", "m2"), done()]));
FV("a piece with no message id after one with", SCRIPT([start(), text("a", "m1"), text("b"), text("c", "m1"), done()]));
FV("an empty piece is nothing", SCRIPT([start(), text(""), done()]));
FV("a tool call and its result", SCRIPT([start(), toolUse("{\"command\":\"pnpm te", "tu-1"), toolUse("st\"}", "tu-1"), toolResult("all passed", "tu-1"), done()]));
FV("a failed tool result", SCRIPT([start(), toolUse("{\"command\":\"false\"}", "tu-1"), toolResult("exit 1", "tu-1", true), done()]));
FV("a result with no call open", SCRIPT([start(), toolResult("orphan", "tu-x"), toolResult("no id"), done()]));
FV("a call with no id", SCRIPT([start(), ev("session.delta", { kind: "tool_use", text: "{\"command\":\"ls\"}", toolName: "Bash" }), done()]));
FV("a result forgets its call", SCRIPT([start(), toolUse("{\"command\":\"ls\"}", "tu-1"), toolResult("a", "tu-1"), toolResult("again", "tu-1"), done()]));
FV("attachments of the person's turn", SCRIPT([start({ attachments: [{ mediaType: "image/png", bytes: 1200, name: "shot.png" }, { mediaType: "image/jpeg", bytes: 20 }] }), done()]));
FV("after a cut turn", SCRIPT([start({ afterCut: true }), done()]));
FV("a cap wait and its end", SCRIPT([ev("session.capped", { placeId: "p-1", place: "hetzner", running: 2, atOnce: 2 }), ev("session.uncapped", { why: "freed", atOnce: 2 }), start(), done()]));
FV("a notify for me", SCRIPT([start(), ev("session.notify", { notify: "me", text: "thread t-2 finished" }), done()]));
FV("a notify for another thread", SCRIPT([start(), ev("session.notify", { notify: "thread-other", text: "not for me" }), done()]));
FV("the turn's own spend word", SCRIPT([start(), done({ status: "completed", text: "x", durationMs: 3000, costUsd: 1.5 })]), { spend: "subscription" });
FV("announce with the thread's id", SCRIPT([start(), done()]), { announce: true });
FV("announce with a line of its own and the folder", SCRIPT([start(), done()], {}, started("started", { session: { cwd: "/root/work" } })), { announce: true, opened: true });
FV("announce with a line of its own and no folder", SCRIPT([start(), done()]), { announce: true, opened: true });
FV("a notice on the start", SCRIPT([start(), done()], {}, started("started", { notice: "waking attic\nthe disk was kept" })));
FV("queued behind the running turn", SCRIPT([queued(), start(), done()], {}, started("queued")));
FV("queued for a computer, then behind the turn", SCRIPT([queued("hetzner"), queued(), start(), done()], {}, started("queued")));
FV("queued for a computer alone", SCRIPT([queued("hetzner"), start(), done()], {}, started("queued")));
FV("held", SCRIPT([start(), done()], {}, started("held", { session: { capped: { placeId: "p-hetzner", place: "hetzner", running: 2, atOnce: 2 } } })));
FV("joined the running turn", SCRIPT([steer(), done()], {}, started("steered")));
FV("joined with picks dropped", SCRIPT([steer(), done()], {}, started("steered")), { picks: { model: "opus", effort: "high", access: "full", fast: true } });
FV("joined with one pick dropped", SCRIPT([steer(), done()], {}, started("steered")), { picks: { effort: "low" } });
FV("joined with fast false", SCRIPT([steer(), done()], {}, started("steered")), { picks: { fast: false } });
FV("joined a turn waiting on a permission", SCRIPT([steer({ waiting: true }), done()], {}, started("steered")));
FV("the agent is starting", SCRIPT([starting(), start(), done()]));
FV("the agent installs on first run", SCRIPT([starting({ installs: true }), start(), done()]));
FV("a failed turn", SCRIPT([start(), done({ status: "failed", text: "x", error: "the model refused" })]));
FV("a turn the runtime ended", SCRIPT([start(), end({ exitCode: null, sawResult: false, reason: "the machine went to sleep" })]));
FV("refused for a sign-in", SCRIPT([start(), done({ status: "failed", error: "not logged in", refusal: "sign-in" })]));
FV("an interrupted turn", SCRIPT([start(), done({ status: "interrupted", error: "stopped by the person" })]));
FV("the host stops under the turn: the reply is printed whole", {
  replies: { "sessions.start": started() },
  pushed: { "sessions.start": [start(), text("Hello\n")] },
  closesAfter: { "sessions.start": 4001 },
  redials: [{ replies: { "sessions.history": history(start(), done({ status: "completed", text: "Hello\n" })) } }],
}, { io: { sameScreen: true } });
FV("the host stops under the start", { closes: { "sessions.start": 4001 }, redials: [{ replies: { "sessions.start": started(), "sessions.history": history() }, pushed: { "sessions.start": [start(), done()] } }] });
FV("the host stops under the start, no way back", { closes: { "sessions.start": 4001 } });
FV("the host refuses the start", { replies: { "sessions.start": refused("no such workspace", "not-found") } });
FV("nothing ends the turn", { replies: { "sessions.start": started() }, pushed: { "sessions.start": [start(), text("x")] } }, {}, { within: 300 }, false);

// the prompts of a watched turn
const ASK = (options: Obj[], extra: Obj = {}): string => permission(options, extra);
const ANSWERED: Record<string, string> = { "sessions.answer": frame({ outcome: "answered" }) };
FV("a prompt, no keys: the verbs answer it", SCRIPT([start(), ASK([OPT_ALLOW, OPT_DENY, OPT_MODE]), done()]));
FV("a prompt at a keyboard: the keys answer it, y", SCRIPT([start(), ASK([OPT_ALLOW, OPT_DENY, OPT_MODE])], { replies: { "sessions.start": started(), ...ANSWERED }, after: { "sessions.start": [done()] } }), { io: { key: "y" } });
FV("a prompt at a keyboard, n", SCRIPT([start(), ASK([OPT_ALLOW, OPT_DENY, OPT_MODE])], { replies: { "sessions.start": started(), ...ANSWERED }, after: { "sessions.start": [done()] } }), { io: { key: "n" } });
FV("a prompt at a keyboard, a", SCRIPT([start(), ASK([OPT_ALLOW, OPT_DENY, OPT_MODE])], { replies: { "sessions.start": started(), ...ANSWERED }, after: { "sessions.start": [done()] } }), { io: { key: "a" } });
FV("a prompt at a keyboard, a key no road has", SCRIPT([start(), ASK([OPT_ALLOW, OPT_DENY, OPT_MODE])], { replies: { "sessions.start": started(), ...ANSWERED }, after: { "sessions.start": [done()] } }), { io: { key: "x" } });
FV("a prompt at a keyboard, a key whose option the prompt lacks", SCRIPT([start(), ASK([OPT_ALLOW])], { replies: { "sessions.start": started(), ...ANSWERED }, after: { "sessions.start": [done()] } }), { io: { key: "n" } });
FV("a prompt at a keyboard that carries only its own answers", SCRIPT([start(), ASK([OPT_ANSWER]), done()]), { io: { key: "y" } });
FV("a prompt with no options at a keyboard", SCRIPT([start(), ASK([]), done()]), { io: { key: "y" } });
FV("a key that the host refuses goes to stderr", SCRIPT([start(), ASK([OPT_ALLOW])], { replies: { "sessions.start": started(), "sessions.answer": frame({ outcome: "gone" }) }, after: { "sessions.start": [done()] } }), { io: { key: "y" } });
FV("a key that the host fails on goes to stderr", SCRIPT([start(), ASK([OPT_ALLOW])], { replies: { "sessions.start": started(), "sessions.answer": refused("no turn") }, after: { "sessions.start": [done()] } }), { io: { key: "y" } });
FV("a prompt closed by anyone releases the read", SCRIPT([start(), ASK([OPT_ALLOW]), closed(), done()]));
FV("two prompts in a row", SCRIPT([start(), ASK([OPT_ALLOW], { askId: "ask-1" }), closed("ask-1"), ASK([OPT_DENY], { askId: "ask-2", toolName: "Edit", input: JSON.stringify({ file_path: "/a" }), detail: "/a" }), done()]));
FV("a prompt for a tool with no detail", SCRIPT([start(), ASK([OPT_ALLOW, OPT_DENY], { detail: undefined, toolName: "WebFetch", input: JSON.stringify({ url: "https://example.com" }) }), done()]));

// ------------------------------------------------------------------------------------------------ detachVerb
const DV = (name: string, script: Script, args: Obj = {}, extra: Partial<Case> = {}): void => {
  add("detachVerb", name, { start: START, json: false, ...args }, script, extra);
  add("detachVerb", `${name} (--json)`, { start: START, json: true, ...args }, script, extra);
};
DV("started", { replies: { "sessions.start": started() } });
DV("started with an opened line and a folder", { replies: { "sessions.start": started("started", { session: { cwd: "/root/work" } }) } }, { opened: true });
DV("started with an opened line and no folder", { replies: { "sessions.start": started() } }, { opened: true });
DV("a notice", { replies: { "sessions.start": started("started", { notice: "waking attic\nsecond" }) } });
DV("said beside the thread", { replies: { "sessions.start": started() } }, { said: "made a machine" });
DV("a sub-thread's answer rides it", { replies: { "sessions.start": started() } }, { sub: { on: "boat", machine: "fix it", size: { cpu: 2, memMb: 4096 }, rateUsdPerHour: 0.018, repo: "https://github.com/acme/web.git", base: { branch: "main", sha: "4be1c0a9d2c4e1f07a3b9e5d6c8f2a1b0e9d7c6a" }, branch: "wsp/fix-it", sent: { paths: [], bytes: 0, missing: [] }, notSent: { uncommitted: 0 }, deleteAfterMinutes: 30 } });
DV("queued behind the running turn", { replies: { "sessions.start": started("queued") }, pushed: { "sessions.start": [queued()] } });
DV("queued for a computer", { replies: { "sessions.start": started("queued") }, pushed: { "sessions.start": [queued("hetzner")] } });
DV("queued for a computer, then behind the turn", { replies: { "sessions.start": started("queued") }, pushed: { "sessions.start": [queued("hetzner"), queued()] } });
DV("held with its cap", { replies: { "sessions.start": started("held", { session: { capped: { placeId: "p-hetzner", place: "hetzner", running: 2, atOnce: 2 } } }) } });
DV("held with its cap and a wait for a slot", { replies: { "sessions.start": started("held", { session: { capped: { placeId: "p-boat", place: "boat", running: 4, atOnce: 4, why: "slot" } } }) } });
DV("steered", { replies: { "sessions.start": started("steered") } });
DV("steered with picks dropped", { replies: { "sessions.start": started("steered") } }, { picks: { model: "opus", access: "full", fast: true } });
DV("the host stops under the start, dialled again", { closes: { "sessions.start": 4001 }, redials: [{ replies: { "sessions.start": started() } }] });
DV("the host stops under the start, no way back", { closes: { "sessions.start": 4001 } });
DV("the host refuses the start", { replies: { "sessions.start": refused("no such workspace", "not-found") } });

// send-waits.test.ts: a send the host holds says what it waits for, on the detached road and on the followed one
DV("send-waits: a wait behind the turn turns into a wait for a computer", { replies: { "sessions.start": started("queued") }, pushed: { "sessions.start": [queued(), queued("hetzner")] } });
DV("send-waits: a wait for a computer gives way to a wait behind the turn", { replies: { "sessions.start": started("queued") }, pushed: { "sessions.start": [queued("hetzner"), queued()] } });
FV("send-waits: a followed send names the computer at the wait's end too", SCRIPT([queued(), queued("hetzner"), done(), end()], {}, started("queued")));
FV("send-waits: a followed send that waited for a computer alone", SCRIPT([queued("hetzner"), done(), end()], {}, started("queued")));
// the keys pick an option by what picking it does, never by its id
const SWAPPED = [{ id: "1", label: "Allow", effect: "deny" }, { id: "2", label: "Deny", effect: "allow" }, { id: "3", label: "Edit freely", effect: "mode", mode: "acceptEdits" }];
FV("keys pick by effect, y", SCRIPT([start(), ASK(SWAPPED)], { replies: { "sessions.start": started(), ...ANSWERED }, after: { "sessions.start": [done()] } }), { io: { key: "y" } });
FV("keys pick by effect, n", SCRIPT([start(), ASK(SWAPPED)], { replies: { "sessions.start": started(), ...ANSWERED }, after: { "sessions.start": [done()] } }), { io: { key: "n" } });
FV("keys pick by effect, a", SCRIPT([start(), ASK(SWAPPED)], { replies: { "sessions.start": started(), ...ANSWERED }, after: { "sessions.start": [done()] } }), { io: { key: "a" } });
FV("a second prompt replaces the first read of keys", SCRIPT([start(), ASK([OPT_ALLOW], { askId: "ask-1" }), ASK([OPT_ALLOW, OPT_DENY], { askId: "ask-2" })], { replies: { "sessions.start": started(), ...ANSWERED }, after: { "sessions.start": [done()] } }), { io: { key: "n" } });

// ------------------------------------------------------------------------------------------------ beforeSending, turnView
add("beforeSending", "the read answers", {}, { replies: { "sessions.list": list(row(TH, "running")) } });
add("beforeSending", "the read fails and the host did not stop", {}, { replies: { "sessions.list": refused("no listing", "auth") } });
add("beforeSending", "the host stopped under the read", {}, { closes: { "sessions.list": 4001 } });
add("beforeSending", "the host closed under the read with another code", {}, { closes: { "sessions.list": 1006 } });
for (const [name, turn] of [
  ["started, no result", view()],
  ["completed with text", view({ result: { status: "completed", text: "the reply" } })],
  ["completed with no text", view({ result: { status: "completed" } })],
  ["failed with text", view({ result: { status: "failed", text: "partial", error: "boom" } })],
  ["held with its cap", view({ outcome: "held", session: sess({ capped: { placeId: "p-hetzner", place: "hetzner", running: 2, atOnce: 2 } }) })],
  ["held with no cap on the session", view({ outcome: "held" })],
  ["started with a cap on the session is not held", view({ session: sess({ capped: { placeId: "p-hetzner", place: "hetzner", running: 2, atOnce: 2 } }) })],
  ["after a cut turn", view({ afterCut: true, result: { status: "completed", text: "x" } })],
  ["queued", view({ outcome: "queued" })],
  ["steered with the harness named", view({ outcome: "steered", session: sess({ harness: "codex" }) })],
] as [string, Obj][]) add("turnView", name, { turn });

// ------------------------------------------------------------------------------------------------ runs
const T = (mod: Obj): Obj => mod["turns"] as Obj;
const call = <R>(f: unknown, ...a: unknown[]): Promise<R> => (f as (...x: unknown[]) => Promise<R>)(...a);
const sync = <R>(f: unknown, ...a: unknown[]): Promise<R> => Promise.resolve((f as (...x: unknown[]) => R)(...a));
const turnJson = (t: Obj | undefined): unknown =>
  t === undefined
    ? undefined
    : { session: t["session"], threadId: t["threadId"], outcome: t["outcome"], ...(t["result"] !== undefined ? { result: t["result"] } : {}), ...(t["reason"] !== undefined ? { reason: t["reason"] } : {}), ...(t["afterCut"] === true ? { afterCut: true } : {}), ...(t["notice"] !== undefined ? { notice: t["notice"] } : {}) };
const summary = (t: Obj): string => `thread=${String(t["threadId"])} outcome=${String(t["outcome"])} result=${String((t["result"] as Obj | undefined)?.["status"] ?? "-")}`;
const opened = (id: string, folder?: string): string => (folder === undefined ? `thread ${id}` : `opened ${id} in ${folder}`);
const roadOf = (mod: Obj, which: string): unknown => (T(mod)["ANSWER_ROADS"] as Obj[]).find(r => r["key"] === (which === "allow" ? "y" : "n"));

const runs: Record<string, Run> = {
  turnFailure: ({ mod, args }) => sync(T(mod)["turnFailure"], args["turn"]),
  turnRefusal: async ({ mod, args }) => {
    const e = (T(mod)["turnRefusal"] as (t: unknown) => Error | undefined)(args["turn"]);
    return e;
  },
  answerKeysLine: ({ mod, args }) => sync(T(mod)["answerKeysLine"], args["options"]),
  answerVerbsLine: ({ mod, args }) => sync(T(mod)["answerVerbsLine"], args["threadId"]),
  answeredLine: ({ mod, args }) => sync(T(mod)["answeredLine"], args["threadId"], args["ask"], args["said"]),
  noOpenAskLine: ({ mod, args }) => sync(T(mod)["noOpenAskLine"], args["threadId"]),
  noSuchAnswerLine: ({ mod, args }) => sync(T(mod)["noSuchAnswerLine"], args["threadId"], args["verb"]),
  answerRoads: ({ mod }) => Promise.resolve(T(mod)["ANSWER_ROADS"]),
  answerInTheApp: ({ mod }) => Promise.resolve(T(mod)["ANSWER_IN_THE_APP"]),
  answerOpenAsk: ({ mod, client, args }) => call(T(mod)["answerOpenAsk"], client, args["ref"], roadOf(mod, args["road"] as string), args["reason"]),
  followVerb: async ({ mod, client, deps, args, sinks }) => {
    const ctx = ctxOf({ mod, deps, sinks, io: args["io"] as IoOpts | undefined, json: args["json"] === true });
    const said = { ...(args["opened"] === true ? { opened } : {}), ...(args["spend"] !== undefined ? { spend: args["spend"] } : {}) };
    const turn = await call<Obj>(T(mod)["followVerb"], ctx, client, args["start"], args["announce"] === true, args["picks"] ?? {}, said, (t: Obj) => sinks.said.push(`onTurn ${summary(t)}`));
    return turnJson(turn);
  },
  detachVerb: async ({ mod, client, deps, args, sinks }) => {
    const ctx = ctxOf({ mod, deps, sinks, json: args["json"] === true });
    await call(T(mod)["detachVerb"], ctx, client, args["start"], args["picks"] ?? {}, args["opened"] === true ? opened : undefined, args["sub"], args["said"]);
    return null;
  },
  beforeSending: ({ mod, client }) => call(T(mod)["beforeSending"], client, () => (client as unknown as { request: (op: string) => Promise<unknown> }).request("sessions.list")),
  turnView: ({ mod, args }) => sync(T(mod)["turnView"], args["turn"]),
};

it("records turns_stream", async () => {
  await record("turns_stream", cases, runs);
});
