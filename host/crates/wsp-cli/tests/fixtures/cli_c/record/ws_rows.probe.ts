import { it } from "vitest";
import { frame, record, refused, type Case, type Obj, type Run } from "./lib.ts";

// Unit ws_rows: workspaces-help.ts:602-881 at 65f6b89c7 (the rows a delete, a forget, a thread listing and a create read).
// Values the Rust side writes as typed structs are built in the key order of the Rust type, so a reply the TS passes on
// whole serialises the same way on both sides.

const HOME = "/home/zed";
const FUTURE = 32503680000000;

/** A WorkspaceOut in the order the schema writes it (id first, worktree last). `view` writes WorkspaceView's order. */
const ws = (o: Obj = {}, view = false): Obj => {
  const a: Obj = {
    id: o["id"] ?? "ws-1",
    name: o["name"] ?? "alpha",
    machineId: o["machineId"] ?? "m-1",
    phase: o["phase"] ?? "running",
    ...("kind" in o ? (o["kind"] === undefined ? {} : { kind: o["kind"] }) : { kind: "cloud" }),
    golden: "snap-g1",
    createdAt: "2026-09-27T01:02:03.004Z",
    project: o["project"] ?? { id: "proj-1", name: "alpha", path: "/root/alpha", computer: "place-9" },
    ...(o["folder"] !== undefined ? { folder: o["folder"] } : {}),
    home: "/root",
  };
  const place = "place" in o ? (o["place"] === undefined ? {} : { place: o["place"] }) : { place: "place-9" };
  const provider = { provider: "solari" };
  const worktree = o["worktree"] !== undefined ? { worktree: o["worktree"] } : {};
  return view ? { ...a, ...place, ...worktree, ...provider } : { ...a, ...place, ...provider, ...worktree };
};
const resolved = (o: Obj = {}): string => frame({ workspace: ws(o) });
const sessions = (...rows: Obj[]): string => frame({ sessions: rows });
const sess = (id: string, thread?: string): Obj => ({ id, workspaceId: "ws-1", harness: "claude", status: "completed", ...(thread !== undefined ? { threadId: thread } : {}) });
const places = (...rows: Obj[]): string => frame({ places: rows });
const place = (id: string, name: string, kind = "computer"): Obj => ({ id, kind, name, default: false });
const WT = { path: "/Users/me/.wsp/worktrees/proj-1/work", branch: "work", made: true };

/** A ThreadRow with the Rust ThreadView's field order, then the four a row adds. */
const row = (o: Obj = {}): Obj => {
  const { projectName = "wsp", folder = "/root/wsp", branch = "main", computerName = "attic", ...rest } = o;
  return {
    id: "t-1",
    workspaceId: "ws-1",
    harness: "claude",
    startedBy: "person",
    status: "completed",
    title: "fix the parser",
    sessionId: "s-1",
    turns: 1,
    ran: true,
    ...rest,
    projectName,
    folder,
    branch,
    computerName,
  };
};
const sub = (state: string, extra: Obj = {}): Obj => ({ state, on: "boat", machine: "Fix it", size: { cpu: 2, memMb: 4096 }, branch: "wsp/fix-it", ...extra });
const agent = (id: string, title: string, state: string): Obj => ({ id, title, state, startedAt: 1727431200000 });
const prompt = { askId: "a-1", toolName: "Bash", input: "{}", options: [] };
const waitingOn = { threadId: "t-9", workspaceId: "ws-9", sessionId: "s-9", title: "other", prompt };

const proj = (o: Obj = {}): Obj => ({
  id: "proj-1",
  name: "wsp",
  computer: "here",
  source: { kind: "folder", path: "/root/wsp" },
  path: "/root/wsp",
  remote: "",
  defaultBranch: "main",
  memoryKey: "-root-wsp",
  memoryDir: "/m/-root-wsp",
  ...(o["base"] !== undefined ? { base: o["base"] } : {}),
  createdAt: "2026-09-27T00:00:00.000Z",
  ...Object.fromEntries(Object.entries(o).filter(([k]) => k !== "base")),
});

const cases: Case[] = [];
const add = (c: Case): void => void cases.push(c);

// ---- constants
add({ fn: "constThreadHead", case: "the head of the threads table" });
add({ fn: "constNoProjectYet", case: "the line with no project" });

// ---- madeWorktree
add({ fn: "madeWorktree", case: "no worktree", args: { workspace: ws() } });
add({ fn: "madeWorktree", case: "a worktree wsp made", args: { workspace: ws({ worktree: WT }) } });
add({ fn: "madeWorktree", case: "a worktree wsp did not make", args: { workspace: ws({ worktree: { ...WT, made: false } }) } });
add({ fn: "madeWorktree", case: "a worktree wsp made and removed", args: { workspace: ws({ worktree: { ...WT, gone: true } }) } });
add({ fn: "madeWorktree", case: "a made worktree with gone absent keeps its path as given", args: { workspace: ws({ worktree: { path: "/x y/é 🧪", made: true } }) } });

// ---- dropping
const drop = (name: string, o: Obj, thread: Obj[] | null, ref = "alpha", more: Partial<Case> = {}): void => {
  add({ fn: "dropping", case: name, args: { ref }, script: { replies: { "workspaces.resolve": resolved(o), ...(thread !== null ? { "sessions.list": sessions(...thread) } : {}) } }, ...more });
};
drop("a cloud workspace with no thread", {}, []);
drop("one thread of two sessions counts once", {}, [sess("s-1", "t-1"), sess("s-2", "t-1")]);
drop("two threads and a session with no thread id", {}, [sess("s-1", "t-1"), sess("s-2", "t-2"), sess("s-3")]);
drop("a box folder is refused", { kind: "place" }, null);
drop("a local folder is refused", { kind: "local" }, null);
drop("a local worktree is refused with the worktree words", { kind: "local", worktree: WT }, null);
drop("a local worktree already gone is still refused", { kind: "local", worktree: { ...WT, gone: true } }, null);
drop("a worktree on a box folder is refused as a folder", { kind: "place", worktree: WT }, null);
drop("a cloud workspace with a worktree record goes on", { worktree: WT }, [sess("s-1", "t-1")]);
drop("a record with no kind is a cloud workspace", { kind: undefined }, []);
drop("a ref that is a name", {}, [], "alpha \"one\"");
drop("an empty ref", {}, [], "");
add({ fn: "dropping", case: "the host refuses the ref", args: { ref: "nope" }, script: { replies: { "workspaces.resolve": refused("no workspace nope", "not-found") } } });
add({ fn: "dropping", case: "the sessions listing is refused", args: { ref: "alpha" }, script: { replies: { "workspaces.resolve": resolved(), "sessions.list": refused("the listing broke") } } });
add({ fn: "dropping", case: "a workspace that does not read is another version", args: { ref: "alpha" }, script: { replies: { "workspaces.resolve": frame({ workspace: { id: "ws-1" } }) } } });
add({ fn: "dropping", case: "no workspace in the reply is another version", args: { ref: "alpha" }, script: { replies: { "workspaces.resolve": frame({}) } } });

// ---- deleting
const del = (name: string, o: Obj, list: string | undefined, thread: Obj[] = [], more: Partial<Case> = {}): void => {
  add({ fn: "deleting", case: name, args: { ref: "alpha" }, script: { replies: { "workspaces.resolve": resolved(o), "sessions.list": sessions(...thread), ...(list !== undefined ? { "places.list": list } : {}) } }, ...more });
};
del("a workspace on a joined computer is named with its computer", {}, places(place("place-9", "attic")), [sess("s-1", "t-1")]);
del("the place is a provider, so the delete takes the cloud words", {}, places(place("place-9", "box", "provider")));
del("the place is a provider with a key name", {}, places(place("place-9", "solari", "provider")));
del("the place is not in the list", {}, places(place("place-2", "attic")));
del("the places list is empty", {}, places());
del("places.list is refused so the computer has no name", {}, refused("places are not readable"));
del("places.list is refused with an auth kind", {}, refused("sign in first", "auth"));
del("no place on the record reads no places", { place: undefined }, undefined);
del("two places, the second is the one named", {}, places(place("place-1", "other"), place("place-9", "attic")));
del("two rows with the one id take the first", {}, places(place("place-9", "first"), place("place-9", "second")));
del("a name with an apostrophe and a space", {}, places(place("place-9", "zed's box 🧪")));
// verbs-lifecycle.test.ts: "a copy on a computer somebody joined is deleted from that computer by the names a person knows"
const SPOO = { id: "ws_fix", name: "fix-login", machineId: "wsp-workspace-ws_fix", phase: "running", kind: "cloud", place: "p_spoo", golden: "", createdAt: "2026-09-27T00:00:00.000Z", project: { id: "pr_1", name: "api", path: "/root/api", computer: "p_spoo" } };
const spooCase = (name: string, list: string): void => void add({ fn: "deleting", case: name, args: { ref: "fix-login" }, script: { replies: { "workspaces.resolve": frame({ workspace: SPOO }), "sessions.list": sessions(), "places.list": list } } });
spooCase("a copy on a computer somebody joined is named by the computer's name", places({ id: "p_spoo", kind: "computer", name: "spoo", default: false }));
spooCase("a caller that may not read the computers' names still never reads the machine's id", refused("not yours to read"));
spooCase("a fork at another provider's account takes the cloud's words", places({ id: "p_spoo", kind: "provider", name: "ascii", default: false }));
del("a made worktree on a cloud record", { worktree: WT }, places(place("place-9", "attic")));
add({ fn: "deleting", case: "a local folder is refused before any place is read", args: { ref: "alpha" }, script: { replies: { "workspaces.resolve": resolved({ kind: "local" }) } } });
add({ fn: "deleting", case: "a local worktree is refused before any place is read", args: { ref: "alpha" }, script: { replies: { "workspaces.resolve": resolved({ kind: "local", worktree: WT }) } } });
add({ fn: "deleting", case: "the sessions listing is refused before places are read", args: { ref: "alpha" }, script: { replies: { "workspaces.resolve": resolved(), "sessions.list": refused("broke"), "places.list": places(place("place-9", "attic")) } } });
add({ fn: "deleting", case: "the ref names nothing", args: { ref: "zzz" }, script: { replies: { "workspaces.resolve": refused("no workspace zzz", "not-found") } } });
add({ fn: "deleting", case: "the socket goes while the places are read", args: { ref: "alpha" }, script: { replies: { "workspaces.resolve": resolved(), "sessions.list": sessions() }, closes: { "places.list": 4001 } } });

// ---- forgetQuestion, forgotLine, deleteQuestion, deletedLine
const dropped = (o: Obj, threads: number, on?: Obj): Obj => ({ workspace: ws(o), threads, ...(on !== undefined ? { on } : {}) });
const ON = { name: "alpha", computer: "attic" };
// verbs-lifecycle.test.ts: "the delete question and its line say what the delete does to this kind's machine"
{
  const mine = { id: "ws_mine", name: "box", machineId: "m_ab12", phase: "running", kind: "cloud", golden: "", createdAt: "2026-09-08T00:00:00.000Z", project: { id: "pr_1", name: "api", path: "/root/api", computer: "default" } };
  const here = { ...mine, kind: "local", machineId: "local" };
  const copied = { ...here, worktree: { path: "/Users/dev/api-fix", branch: "fix", made: true } };
  for (const [label, workspace, threads] of [["this computer's own", here, 1], ["a worktree wsp made", copied, 1], ["a fork", mine, 0]] as [string, Obj, number][]) {
    add({ fn: "deleteQuestion", case: `lifecycle test: ${label}`, args: { d: { workspace, threads } } });
    add({ fn: "deletedLine", case: `lifecycle test: ${label}`, args: { d: { workspace, threads } } });
  }
  const SPOO2 = { id: "ws_fix", name: "fix-login", machineId: "wsp-workspace-ws_fix", phase: "running", kind: "cloud", place: "p_spoo", golden: "", createdAt: "2026-09-27T00:00:00.000Z", project: { id: "pr_1", name: "api", path: "/root/api", computer: "p_spoo" } };
  for (const computer of ["spoo", "that computer"]) {
    const d = { workspace: SPOO2, threads: 0, on: { name: "fix-login", computer } };
    add({ fn: "deleteQuestion", case: `lifecycle test: a copy on ${computer}`, args: { d } });
    add({ fn: "deletedLine", case: `lifecycle test: a copy on ${computer}`, args: { d } });
  }
}
for (const [label, d] of [
  ["no thread", dropped({}, 0)],
  ["one thread", dropped({}, 1)],
  ["two threads", dropped({}, 2)],
  ["a thousand threads", dropped({}, 1000)],
  ["a name with quotes and an emoji", dropped({ name: "alpha \"one\" 🧪" }, 3)],
  ["a gone machine", dropped({ phase: "gone", machineId: "m-1" }, 1)],
  ["a made worktree", dropped({ worktree: WT }, 1)],
  ["a worktree wsp did not make", dropped({ worktree: { ...WT, made: false } }, 1)],
  ["a worktree made and gone", dropped({ worktree: { ...WT, gone: true } }, 1)],
  ["a create that made no machine", dropped({ machineId: "" }, 0)],
  ["a computer somebody joined", dropped({}, 2, ON)],
  ["a computer somebody joined that has no name", dropped({}, 1, { name: "alpha", computer: "that computer" })],
  ["a joined computer with a worktree record", dropped({ worktree: WT }, 1, ON)],
  ["a joined computer and no machine made", dropped({ machineId: "" }, 1, ON)],
  ["a box folder kind", dropped({ kind: "place" }, 1)],
  ["a local kind", dropped({ kind: "local" }, 1)],
] as [string, Obj][]) {
  add({ fn: "forgetQuestion", case: label, args: { d } });
  add({ fn: "forgotLine", case: label, args: { d } });
  add({ fn: "deleteQuestion", case: label, args: { d } });
  add({ fn: "deletedLine", case: label, args: { d } });
}
add({ fn: "deleteQuestion", case: "a cloud with the cloud on", args: { d: dropped({}, 1) }, cloud: true });
add({ fn: "deletedLine", case: "a cloud with the cloud on", args: { d: dropped({}, 1) }, cloud: true });

// ---- forget, deleteWorkspace
add({ fn: "forget", case: "sends the workspace id", args: { d: dropped({ id: "ws-7" }, 2) }, script: { replies: { "workspaces.forget": frame({}) } } });
add({ fn: "forget", case: "the host refuses a machine that still stands", args: { d: dropped({}, 0) }, script: { replies: { "workspaces.forget": refused("Only a workspace whose computer is gone can be forgotten; this one is running", "usage") } } });
add({ fn: "deleteWorkspace", case: "sends the workspace id", args: { d: dropped({ id: "ws-7" }, 2) }, script: { replies: { "workspaces.delete": frame({}) } } });
add({ fn: "deleteWorkspace", case: "the host refuses", args: { d: dropped({}, 0) }, script: { replies: { "workspaces.delete": refused("the provider said no") } } });
add({ fn: "deleteWorkspace", case: "the socket goes first", args: { d: dropped({}, 0) }, script: { closes: { "workspaces.delete": 1006 } } });

// ---- shortenedFront, shortenedEnd: under, at and over a width, with units that are one and two UTF-16 cells
const flat = (n: number, c = "a"): string => c.repeat(n);
for (const [w, n] of [[40, 39], [40, 40], [40, 41], [60, 59], [60, 60], [60, 61], [1, 0], [1, 1], [1, 2], [2, 3], [5, 4], [5, 5], [5, 6]] as [number, number][]) {
  add({ fn: "shortenedFront", case: `${n} cells in a width of ${w}`, args: { path: flat(n, "/x").slice(0, n), width: w } });
  add({ fn: "shortenedEnd", case: `${n} cells in a width of ${w}`, args: { text: flat(n, "ab").slice(0, n), width: w } });
}
add({ fn: "shortenedFront", case: "an empty path", args: { path: "", width: 40 } });
add({ fn: "shortenedEnd", case: "an empty text", args: { text: "", width: 60 } });
add({ fn: "shortenedFront", case: "accents count one cell each", args: { path: `/${flat(40, "é")}`, width: 40 } });
add({ fn: "shortenedEnd", case: "accents count one cell each", args: { text: flat(61, "é"), width: 60 } });
add({ fn: "shortenedFront", case: "an emoji counts two cells, so 40 characters are 41 cells", args: { path: `${flat(38)}🧪`, width: 40 } });
add({ fn: "shortenedFront", case: "an emoji that is cut whole", args: { path: `/🧪${flat(40)}`, width: 40 } });
add({ fn: "shortenedEnd", case: "an emoji counts two cells, so 30 emoji are 60 cells", args: { text: "🧪".repeat(30), width: 60 } });
add({ fn: "shortenedEnd", case: "an emoji early is kept whole", args: { text: `🧪${flat(80)}`, width: 60 } });
add({ fn: "shortenedFront", case: "a width wider than the path", args: { path: "/a/b", width: 400 } });
add({ fn: "shortenedEnd", case: "a width wider than the text", args: { text: "title", width: 400 } });
add({ fn: "shortenedEnd", case: "newlines and tabs are cells like any other", args: { text: `a\tb\nc${flat(70)}`, width: 60 } });

// ---- threadLines
const lines = (name: string, t: Obj, extra: Obj = {}): void => void add({ fn: "threadLines", case: name, args: { t, home: HOME, ...extra } });
lines("a plain thread", row());
lines("a thread started by the command line", row({ startedBy: "cli" }));
lines("a thread started by an agent", row({ startedBy: "agent" }));
lines("every column empty but the id and the agent", row({ projectName: "", folder: "", branch: "", computerName: "", title: "" }));
lines("indented one step", row(), { indent: "  " });
lines("indented two steps with subagents", row({ subagents: [agent("a1", "count", "done")] }), { indent: "    " });
lines("a folder under the home", row({ folder: `${HOME}/proj/wsp` }));
lines("a folder that is the home", row({ folder: HOME }));
lines("a folder that only starts with the home's name", row({ folder: `${HOME}2/proj` }));
lines("a folder outside the home", row({ folder: "/srv/proj/wsp" }));
lines("a folder with the home and a trailing slash", row({ folder: `${HOME}/` }));
lines("the home is the root", row({ folder: "/etc/x" }), { home: "/" });
lines("the home is a deep folder", row({ folder: "/a/b/c/d" }), { home: "/a/b" });
for (const n of [39, 40, 41, 42]) lines(`a folder of ${n} cells`, row({ folder: `/${flat(n - 1)}` }));
for (const n of [37, 38, 39]) lines(`a folder of ${n + 2} cells once the home is a tilde`, row({ folder: `${HOME}/${flat(n)}` }));
lines("a long folder with an emoji whose cells cross the cap", row({ folder: `/${flat(38)}🧪` }));
for (const n of [59, 60, 61, 62]) lines(`a title of ${n} cells`, row({ title: flat(n, "t") }));
lines("a title with accents past the cap", row({ title: flat(61, "é") }));
lines("a title with an emoji at the cap", row({ title: `${flat(58)}🧪` }));
lines("a title with a newline stays as it is", row({ title: "one\ntwo\tthree" }));
lines("a thread with an empty subagent list", row({ subagents: [] }));
lines("two subagents, one of each state", row({ subagents: [agent("a1", "count", "running"), agent("b2", "read", "failed"), agent("c3", "stop", "stopped"), agent("d4", "done", "done")] }));
lines("a subagent title of 59, 60 and 61 cells", row({ subagents: [agent("a1", flat(59), "done"), agent("b2", flat(60), "done"), agent("c3", flat(61), "done")] }));
lines("subagents repeat the shortened folder and the branch", row({ folder: `${HOME}/${flat(50)}`, branch: "feat/é", subagents: [agent("a1", "x", "done")] }));
lines("subagents under an indent", row({ subagents: [agent("a1", "x", "done")] }), { indent: "  " });
lines("subagents of an agent that is not claude", row({ harness: "codex", subagents: [agent("a1", "x", "done")] }));
// the state word: waiting, done and failed
lines("running", row({ status: "running" }));
lines("completed and seen", row({ status: "completed", endedAt: 1000, readAt: 2000 }));
lines("completed and unseen reads done", row({ status: "completed", endedAt: 2000 }));
lines("interrupted reads idle", row({ status: "interrupted" }));
lines("failed", row({ status: "failed" }));
lines("failed and unseen still reads failed", row({ status: "failed", endedAt: 5 }));
lines("asking reads needs you", row({ status: "running", asking: "may I?" }));
lines("waiting on another thread reads needs you", row({ status: "running", waitingOn }));
// expired
lines("a disk kept until long ago reads expired", row({ diskKeptUntil: 1 }));
lines("a disk kept until the far future reads idle", row({ diskKeptUntil: FUTURE }));
lines("an expired disk of a running turn reads working", row({ diskKeptUntil: 1, status: "running" }));
lines("an expired disk of a failed turn reads failed", row({ diskKeptUntil: 1, status: "failed" }));
lines("an expired disk with a question reads needs you", row({ diskKeptUntil: 1, asking: "may I?" }));
lines("an expired disk behind another thread's question reads needs you", row({ diskKeptUntil: 1, waitingOn }));
lines("an expired disk of an interrupted turn", row({ diskKeptUntil: 1, status: "interrupted" }));
lines("an expired disk of an unseen finish", row({ diskKeptUntil: 1, endedAt: 5 }));
lines("an expired disk of a sub-thread wins over its own word", row({ diskKeptUntil: 1, sub: sub("waiting") }));
lines("an expired disk keeps the subagent rows", row({ diskKeptUntil: 1, subagents: [agent("a1", "x", "done")] }));
// sub-thread words
lines("a sub-thread waiting for a slot", row({ sub: sub("waiting"), status: "running" }));
lines("a sub-thread starting", row({ sub: sub("starting"), status: "running" }));
lines("a sub-thread released after a completed turn", row({ sub: sub("released"), status: "completed", endedAt: 1, readAt: 2 }));
lines("a sub-thread released after an interrupted turn", row({ sub: sub("released"), status: "interrupted" }));
lines("a sub-thread released after a failed turn", row({ sub: sub("released"), status: "failed" }));
lines("a sub-thread released and unseen reads done", row({ sub: sub("released"), status: "completed", endedAt: 1 }));
lines("a sub-thread released and still running", row({ sub: sub("released"), status: "running" }));
lines("a sub-thread napping with work not pushed", row({ sub: sub("napping", { held: { files: 2, commits: 1 } }), status: "completed", endedAt: 1, readAt: 2 }));
lines("a sub-thread running with work not pushed and the turn done", row({ sub: sub("running", { held: { files: 0, commits: 1 } }), status: "failed" }));
lines("a sub-thread napping with work not pushed while its turn runs", row({ sub: sub("napping", { held: { files: 2, commits: 1 } }), status: "running" }));
lines("a sub-thread napping with nothing held", row({ sub: sub("napping"), status: "completed", endedAt: 1, readAt: 2 }));
lines("a sub-thread running with nothing held", row({ sub: sub("running"), status: "running" }));
lines("a sub-thread waiting that asks reads needs you", row({ sub: sub("waiting"), asking: "may I?", status: "running" }));
lines("a sub-thread starting behind another thread's question reads needs you", row({ sub: sub("starting"), waitingOn, status: "running" }));
lines("a sub-thread with work not pushed that asks reads needs you", row({ sub: sub("napping", { held: { files: 1, commits: 0 } }), asking: "may I?", status: "completed" }));
lines("a sub-thread with work not pushed and an interrupted turn", row({ sub: sub("napping", { held: { files: 1, commits: 0 } }), status: "interrupted" }));
lines("a sub-thread with subagents keeps the sub-thread word on its own row only", row({ sub: sub("waiting"), status: "running", subagents: [agent("a1", "x", "running")] }));
lines("a thread with no home to shorten against", row({ folder: `${HOME}/x` }), { home: "/nowhere" });

// ---- diskGoneLines
const dg = (name: string, rows: Obj[]): void => void add({ fn: "diskGoneLines", case: name, args: { rows } });
dg("no rows", []);
dg("no row holds a gone disk", [row(), row({ id: "t-2" })]);
dg("one row", [row({ diskGone: "the provider has no machine m-1" })]);
dg("two rows in order with one between", [row({ id: "t-2", diskGone: "second" }), row({ id: "t-1" }), row({ id: "t-3", diskGone: "third" })]);
dg("an empty reason is still a line", [row({ diskGone: "" })]);
dg("a reason with a newline and an emoji", [row({ diskGone: "gone\nfor good 🧪" })]);
dg("the same id twice is two lines", [row({ diskGone: "a" }), row({ diskGone: "b" })]);

// ---- threadTree
const tree = (name: string, rows: Obj[]): void => void add({ fn: "threadTree", case: name, args: { rows } });
const t = (id: string, parent?: string, o: Obj = {}): Obj => row({ id, ...(parent !== undefined ? { parentThreadId: parent } : {}), ...o });
tree("no rows", []);
tree("one row", [t("a")]);
tree("two roots in the order given", [t("b"), t("a")]);
tree("a chain", [t("a"), t("b", "a"), t("c", "b")]);
tree("a chain given child first", [t("c", "b"), t("b", "a"), t("a")]);
tree("a parent after its child", [t("child", "lead"), t("lead")]);
tree("siblings keep their order under one parent", [t("a"), t("z", "a"), t("m", "a"), t("b", "a")]);
tree("two trees do not interleave", [t("r1"), t("r2"), t("c1", "r1"), t("c2", "r2"), t("g1", "c1")]);
tree("a parent missing from the listing stands at the top", [t("a", "gone"), t("b")]);
tree("a missing parent's children hang under it", [t("a", "gone"), t("b", "a"), t("c", "b")]);
tree("a root and an orphan keep their order", [t("orphan", "gone"), t("root"), t("kid", "root")]);
tree("a parent of the empty string stands at the top", [t("a", ""), t("b", "a")]);
tree("a row whose id is the empty string holds a parent of that name", [t("", undefined), t("k", "")]);
tree("two rows that are each other's parent stand flat at the top", [t("a", "b"), t("b", "a")]);
tree("a cycle beside a healthy tree", [t("x", "y"), t("r"), t("y", "x"), t("c", "r")]);
tree("a thread that is its own parent stands at the top", [t("a", "a"), t("b", "a")]);
tree("a cycle with a tail stands flat in the order given", [t("c1", "c2"), t("r"), t("c2", "c1"), t("c3", "c2")]);
tree("a three-cycle", [t("a", "c"), t("b", "a"), t("c", "b")]);
tree("a row with the same id twice draws the first and drops the second", [t("a"), t("a", undefined, { title: "twin" }), t("k", "a")]);
tree("a duplicate that is a child", [t("r"), t("k", "r"), t("k", "r", { title: "twin" })]);
tree("a deep chain", [t("a"), t("b", "a"), t("c", "b"), t("d", "c"), t("e", "d"), t("f", "e")]);
tree("a wide tree", [t("r"), t("a", "r"), t("b", "r"), t("c", "r"), t("d", "r"), t("e", "r")]);
tree("nested siblings interleave depth first", [t("r"), t("a", "r"), t("b", "r"), t("a1", "a"), t("b1", "b"), t("a2", "a")]);
tree("a tree listed backwards", [t("a2", "a"), t("b1", "b"), t("a1", "a"), t("b", "r"), t("a", "r"), t("r")]);
tree("rows with a root thread id are not read", [t("a", undefined, { rootThreadId: "r" }), t("b", "a", { rootThreadId: "r" })]);

// ---- placeNames
const names = (name: string, reply: string, more: Partial<Case> = {}): void => void add({ fn: "placeNames", case: name, script: { replies: { "places.list": reply } }, ...more });
names("no places", places());
names("one computer", places(place("place-9", "attic")));
names("a computer, a provider and a joined box", places(place("here", "this mac"), place("place-9", "attic"), place("place-s", "solari", "provider")));
names("the box provider by its key name", places(place("p-b", "box", "provider")));
names("a label is not the table name", places({ ...place("place-9", "attic"), label: "zed's box" }));
names("two rows with one id keep the last name in the first place", places(place("a", "one"), place("b", "two"), place("a", "three")));
names("a name with an emoji and a newline", places(place("place-9", "at\ntic 🧪")));
names("places.list is refused", refused("places are not readable"));
add({ fn: "placeNames", case: "the socket is gone", script: { closes: { "places.list": 1006 } } });

// ---- projectsOf, projectOf, projectLine
add({ fn: "projectsOf", case: "no projects", script: { replies: { "projects.list": frame({ projects: [] }) } } });
add({ fn: "projectsOf", case: "two projects in the host's order", script: { replies: { "projects.list": frame({ projects: [proj({ id: "p-2", name: "site" }), proj()] }) } } });
add({ fn: "projectsOf", case: "a project with a base", script: { replies: { "projects.list": frame({ projects: [proj({ base: "release" })] }) } } });
add({ fn: "projectsOf", case: "the host refuses", script: { replies: { "projects.list": refused("projects are not readable") } } });
add({ fn: "projectOf", case: "by name", args: { ref: "wsp" }, script: { replies: { "projects.resolve": frame({ project: proj() }) } } });
add({ fn: "projectOf", case: "by id with a git source", args: { ref: "proj-9" }, script: { replies: { "projects.resolve": frame({ project: proj({ id: "proj-9", computer: "place-9", source: { kind: "git", url: "https://x/y.git" } }) }) } } });
add({ fn: "projectOf", case: "an empty ref", args: { ref: "" }, script: { replies: { "projects.resolve": refused("no project is named \"\"", "not-found") } } });
add({ fn: "projectOf", case: "a ref naming nothing", args: { ref: "zzz" }, script: { replies: { "projects.resolve": refused("no project zzz", "not-found") } } });

const pl = (name: string, p: Obj, workspaces: Obj[] = [], threadsHeld: Obj[] = [], named: [string, string][] = []): void => void add({ fn: "projectLine", case: name, args: { p, workspaces, threadsHeld, named } });
const th = (id: string, workspaceId: string): Obj => row({ id, workspaceId });
const wsOf = (id: string, projectId: string): Obj => ws({ id, project: { id: projectId, name: "n", path: "/p", computer: "here" } });
pl("a folder project on this computer", proj());
pl("a project on a computer by its name", proj({ computer: "place-9" }), [], [], [["place-9", "attic"]]);
pl("a project on a computer nobody named is shown by its id", proj({ computer: "place-9" }), [], [], [["place-2", "other"]]);
pl("a git source", proj({ source: { kind: "git", url: "https://github.com/x/y.git" }, path: "/y" }));
pl("a github source", proj({ source: { kind: "github", repo: "x/y" } }));
pl("a gitlab source", proj({ source: { kind: "gitlab", repo: "gitlab.com/x/y" } }));
pl("a base branch", proj({ base: "release" }));
pl("an empty base is kept empty", proj({ base: "" }));
for (const n of [39, 40, 41]) pl(`a path of ${n} cells`, proj({ path: `/${flat(n - 1)}` }));
pl("a path with an emoji at the cap", proj({ path: `/${flat(38)}🧪` }));
pl("no workspace and no thread counts zero", proj());
pl("a thread of a workspace of this project counts", proj(), [wsOf("w1", "proj-1")], [th("t1", "w1")]);
pl("threads of another project's workspace do not count", proj(), [wsOf("w1", "proj-2")], [th("t1", "w1")]);
pl("a thread of a workspace not listed does not count", proj(), [wsOf("w1", "proj-1")], [th("t1", "w9")]);
pl("three threads over two workspaces and one of another project", proj(), [wsOf("w1", "proj-1"), wsOf("w2", "proj-1"), wsOf("w3", "proj-2")], [th("t1", "w1"), th("t2", "w1"), th("t3", "w2"), th("t4", "w3")]);
pl("a project name with a newline and an emoji", proj({ name: "a\nb 🧪" }));
pl("two workspaces with one id count a thread once", proj(), [wsOf("w1", "proj-1"), wsOf("w1", "proj-1")], [th("t1", "w1")]);

// ---- createFor
const CAP = (sizes: Obj[]): string => frame({ capabilities: { sizes } });
const OFFER = (cpu: number, memMb: number, rate: number): Obj => ({ cpu, memMb, rateUsdPerHour: rate });
const SIZES = [OFFER(2, 4096, 0.018), OFFER(4, 8192, 0.036)];
const cloudProject = { id: "proj-1", name: "alpha", computer: "place-9" };
const hereProject = { id: "proj-2", name: "site", computer: "here" };
const creating = (name: string, message: string, stage = "fork-requested"): string => JSON.stringify({ type: "workspace.creating", workspaceId: "ws-5", name, stage, message, elapsedMs: 10 });
const made = (o: Obj = {}, notice?: string): string => frame({ workspace: ws({ id: "ws-5", name: "fix-it", ...o }), ...(notice !== undefined ? { notice } : {}) });
const create = (name: string, args: Obj, script: Script2, more: Partial<Case> = {}): void => void add({ fn: "createFor", case: name, args: { project: cloudProject, name: "fix-it", ...args }, script: { ...script }, ...more });
type Script2 = NonNullable<Case["script"]>;
const landing = frame({ capabilities: {} });
create("a cloud project with nothing asked", {}, { replies: { "workspaces.landing": landing, "workspaces.create": made() } });
create("the folder the workspace holds the project in", {}, { replies: { "workspaces.landing": landing, "workspaces.create": made({ folder: "/work/fix-it" }) } });
create("a notice rides under the line", {}, { replies: { "workspaces.landing": landing, "workspaces.create": made({}, "the machine is a size smaller") } });
create("an empty notice is still a notice", {}, { replies: { "workspaces.landing": landing, "workspaces.create": made({}, "") } });
create("under --json the value is one line and the lines are frames", { json: true }, { replies: { "workspaces.landing": landing, "workspaces.create": made({}, "n") }, pushed: { "workspaces.create": [creating("fix-it", "asking for a machine")] } });
create("stages stream to stderr as they arrive", {}, { replies: { "workspaces.landing": landing, "workspaces.create": made() }, pushed: { "workspaces.create": [creating("fix-it", "asking for a machine"), creating("fix-it", "naming it", "hostname-set"), creating("fix-it", "ready", "ready")] } });
create("a stage of another name is not shown", {}, { replies: { "workspaces.landing": landing, "workspaces.create": made() }, pushed: { "workspaces.create": [creating("other", "not mine"), creating("fix-it", "mine")] } });
create("a frame of another type is not shown", {}, { replies: { "workspaces.landing": landing, "workspaces.create": made() }, pushed: { "workspaces.create": [JSON.stringify({ type: "workspace.ready", name: "fix-it", message: "no" }), creating("fix-it", "yes")] } });
create("stages heard while the subscription is asked are held and shown first", {}, { replies: { "workspaces.landing": landing, "workspaces.create": made() }, pushed: { "events.subscribe": [creating("fix-it", "early one")], "workspaces.create": [creating("fix-it", "later one")] } });
create("a stage heard after the reply is dropped", {}, { replies: { "workspaces.landing": landing, "workspaces.create": made() }, after: { "workspaces.create": [creating("fix-it", "too late")] } });
create("stages under --json print as frames before the result", { json: true }, { replies: { "workspaces.landing": landing, "workspaces.create": made() }, pushed: { "events.subscribe": [creating("fix-it", "early")], "workspaces.create": [creating("fix-it", "live")] } });
create("a stage with a message holding a newline and an emoji", {}, { replies: { "workspaces.landing": landing, "workspaces.create": made() }, pushed: { "workspaces.create": [creating("fix-it", "line one\nline two 🧪")] } });
create("a size on a cloud project", { asked: { size: "2x4" } }, { replies: { "workspaces.landing": CAP(SIZES), "workspaces.create": made() } });
create("a size of half a gigabyte steps", { asked: { size: "2x0.5" } }, { replies: { "workspaces.landing": CAP([OFFER(2, 512, 0.01)]), "workspaces.create": made() } });
create("a size word with spaces round it", { asked: { size: " 4x8 " } }, { replies: { "workspaces.landing": CAP(SIZES), "workspaces.create": made() } });
create("a size the provider does not offer", { asked: { size: "8x16" } }, { replies: { "workspaces.landing": CAP(SIZES), "workspaces.create": made() } });
create("a size that is no size", { asked: { size: "big" } }, { replies: { "workspaces.landing": CAP(SIZES), "workspaces.create": made() } });
create("an empty size word", { asked: { size: "" } }, { replies: { "workspaces.landing": CAP(SIZES), "workspaces.create": made() } });
create("a size when the provider offers none", { asked: { size: "2x4" } }, { replies: { "workspaces.landing": CAP([]), "workspaces.create": made() } });
create("a size with a named offer", { asked: { size: "9x9" } }, { replies: { "workspaces.landing": CAP([{ ...OFFER(2, 4096, 0.018), name: "small" }]), "workspaces.create": made() } });
create("a size on a copy of a folder is refused before anything is sent", { project: hereProject, asked: { size: "2x4" } }, { replies: { "workspaces.landing": CAP(SIZES), "workspaces.create": made() } });
create("an empty size on a copy of a folder is refused too", { project: hereProject, asked: { size: "" } }, {});
create("no size on a copy of a folder goes on", { project: hereProject }, { replies: { "workspaces.landing": landing, "workspaces.create": made({ kind: "local", folder: "/work/site", project: { id: "proj-2", name: "site", path: "/root/site", computer: "here" } }) } });
create("agents with both caps", { asked: { agents: { maxMachines: 3, maxDepth: 2 } } }, { replies: { "workspaces.landing": landing, "workspaces.create": made() } });
create("agents with one cap", { asked: { agents: { maxDepth: 1 } } }, { replies: { "workspaces.landing": landing, "workspaces.create": made() } });
create("agents with no cap named is still sent", { asked: { agents: {} } }, { replies: { "workspaces.landing": landing, "workspaces.create": made() } });
create("a parent", { asked: { parent: "ws-0" } }, { replies: { "workspaces.landing": landing, "workspaces.create": made() } });
create("a size, agents and a parent in the order sent", { asked: { size: "4x8", agents: { maxMachines: 0 }, parent: "ws-0" } }, { replies: { "workspaces.landing": CAP(SIZES), "workspaces.create": made() } });
create("an empty name is sent as it is", { name: "" }, { replies: { "workspaces.landing": landing, "workspaces.create": made({ name: "" }) } });
create("the landing is refused so nothing else is sent", {}, { replies: { "workspaces.landing": refused("this computer forks nothing", "usage"), "workspaces.create": made() } });
create("the create is refused", {}, { replies: { "workspaces.landing": landing, "workspaces.create": refused("the provider is at its cap", "provider") }, pushed: { "workspaces.create": [creating("fix-it", "asking")] } });
create("the subscription is refused so no create is sent", {}, { replies: { "workspaces.landing": landing, "events.subscribe": refused("no events"), "workspaces.create": made() } });
create("the socket goes during the create", {}, { replies: { "workspaces.landing": landing }, closes: { "workspaces.create": 4001 }, pushed: { "workspaces.create": [creating("fix-it", "asking")] } });
create("the socket goes during the landing", {}, { closes: { "workspaces.landing": 1006 } });
create("a project with a name that needs escaping", { project: { ...cloudProject, name: "a \"b\" 🧪" } }, { replies: { "workspaces.landing": landing, "workspaces.create": made({ project: { id: "proj-1", name: "a \"b\" 🧪", path: "/root/a", computer: "place-9" } }) } });

// ---- projectsHere
const here = (name: string, script: Script2): void => void add({ fn: "projectsHere", case: name, script });
here("the host lists two projects", { replies: { "projects.list": frame({ projects: [proj({ id: "p-1", name: "wsp", computer: "here" }), proj({ id: "p-2", name: "site", computer: "place-9" })] }) } });
here("the host lists none, which is an answer", { replies: { "projects.list": frame({ projects: [] }), "workspaces.list": frame({ workspaces: [ws()] }) } });
here("the list is refused so the workspaces name them", { replies: { "projects.list": refused("not from this caller", "usage"), "workspaces.list": frame({ workspaces: [ws({ id: "w1", project: { id: "p-1", name: "wsp", path: "/p", computer: "here" } }), ws({ id: "w2", project: { id: "p-2", name: "site", path: "/s", computer: "place-9" } })] }) } });
here("the list is refused with an auth kind", { replies: { "projects.list": refused("sign in", "auth"), "workspaces.list": frame({ workspaces: [ws()] }) } });
here("a reply with no projects falls back", { replies: { "projects.list": frame({}), "workspaces.list": frame({ workspaces: [ws()] }) } });
here("two workspaces of one project name it once, the last name in the first place", { replies: { "projects.list": refused("no"), "workspaces.list": frame({ workspaces: [ws({ id: "w1", project: { id: "p-1", name: "old", path: "/p", computer: "here" } }), ws({ id: "w2", project: { id: "p-2", name: "site", path: "/s", computer: "here" } }), ws({ id: "w3", project: { id: "p-1", name: "new", path: "/p", computer: "place-9" } })] }) } });
here("no workspace either", { replies: { "projects.list": refused("no"), "workspaces.list": frame({ workspaces: [] }) } });
here("both reads are refused", { replies: { "projects.list": refused("no"), "workspaces.list": refused("nor this") } });
here("the socket goes on the second read", { replies: { "projects.list": refused("no") }, closes: { "workspaces.list": 1006 } });

// ---- agentsAsked, countAsked
const asked = (name: string, a: { maxMachines?: string | number; maxDepth?: string | number; computer?: string }): void => void add({ fn: "agentsAsked", case: name, args: a });
asked("nothing named", {});
asked("machines only", { maxMachines: "3" });
asked("depth only", { maxDepth: "2" });
asked("both, machines first", { maxMachines: "0", maxDepth: "1" });
asked("both as numbers", { maxMachines: 4, maxDepth: 3 });
asked("machines zero is a cap", { maxMachines: 0 });
asked("depth zero is refused", { maxDepth: "0" });
asked("machines below zero", { maxMachines: "-1" });
asked("machines that is no number", { maxMachines: "many" });
asked("the machines are read before the depth", { maxMachines: "x", maxDepth: "y" });
asked("a good machines and a bad depth", { maxMachines: "2", maxDepth: "y" });
asked("a computer names the flag", { maxMachines: "x", computer: "attic" });
asked("a computer names the depth flag", { maxDepth: "x", computer: "attic" });
asked("a computer and no flag is not read", { computer: "attic" });
const count = (name: string, flagName: string, word: string | number, least: number, computer?: string): void => void add({ fn: "countAsked", case: name, args: { flagName, word, least, ...(computer !== undefined ? { computer } : {}) } });
count("a plain number", "--max-machines", "3", 0);
count("zero where zero is allowed", "--max-machines", "0", 0);
count("zero where one is the least", "--max-depth", "0", 1);
count("one where one is the least", "--max-depth", "1", 1);
count("below zero", "--max-machines", "-1", 0);
count("a fraction", "--max-machines", "1.5", 0);
count("a fraction that is whole", "--max-machines", "2.0", 0);
count("an exponent that is whole", "--max-machines", "1e2", 0);
count("a hex word", "--max-machines", "0x10", 0);
count("white space around a number", "--max-machines", " 4 ", 0);
count("an empty word reads as zero", "--max-machines", "", 0);
count("an empty word where one is the least", "--max-depth", "", 1);
count("a blank word", "--max-machines", "   ", 0);
count("not a number", "--max-machines", "abc", 0);
count("a number with a unit", "--max-machines", "3x", 0);
count("a plus sign", "--max-machines", "+3", 0);
count("infinity", "--max-machines", "Infinity", 0);
count("a number given as a number", "--max-depth", 5, 1);
count("a fraction given as a number", "--max-depth", 1.5, 1);
count("zero given as a number where one is the least", "--max-depth", 0, 1);
count("a word with quotes and an emoji", "--max-machines", "a\"b🧪", 0);
count("a computer names the flag", "--max-machines", "x", 0, "attic");
count("a computer names the depth flag", "--max-depth", "x", 1, "attic");
count("a big whole number", "--max-machines", "100000", 0);
count("a leading zero", "--max-machines", "007", 0);
count("an underscore is not a digit separator", "--max-machines", "1_000", 0);
count("arabic digits are not digits", "--max-machines", "\u0663", 0);
count("an exponent past the double range is infinity", "--max-machines", "1e400", 0);
count("minus zero is zero", "--max-machines", "-0", 0);
count("a leading dot with an exponent", "--max-machines", ".5e1", 0);
count("a trailing dot", "--max-machines", "5.", 0);
count("a binary word", "--max-machines", "0b11", 0);
count("an octal word", "--max-machines", "0o17", 0);
count("tabs and newlines round a number", "--max-machines", "\t7\n", 0);
count("a comma", "--max-machines", "1,5", 0);
count("a bare hex prefix", "--max-machines", "0x", 0);
count("a signed hex word", "--max-machines", "-0x10", 0);
count("a minus where zero is the least", "--max-machines", "-1e0", 0);
count("a number as a number with a fraction that is whole", "--max-depth", 2.0, 1);

// ---- sizeChosen
const chosen = (name: string, sizes: Obj[], word: string): void => void add({ fn: "sizeChosen", case: name, args: { sizes, word } });
chosen("an offered size", SIZES, "2x4");
chosen("the second offered size", SIZES, "4x8");
chosen("a size in gigabytes with a fraction", [OFFER(1, 1536, 0.01)], "1x1.5");
chosen("a size of half a gigabyte", [OFFER(2, 512, 0.01)], "2x0.5");
chosen("white space round the word", SIZES, "  2x4  ");
chosen("a size not offered", SIZES, "8x16");
chosen("a word that is no size", SIZES, "big");
chosen("a size of zero cpus", SIZES, "0x4");
chosen("a size of zero memory", SIZES, "2x0");
chosen("a size with a capital x", SIZES, "2X4");
chosen("an empty word", SIZES, "");
chosen("no sizes offered", [], "2x4");
chosen("a named offer", [{ ...OFFER(2, 4096, 0.018), name: "small" }], "3x3");
chosen("a rate in cents and a rate in thousandths", [OFFER(2, 4096, 0.09), OFFER(4, 8192, 0.036), OFFER(8, 16384, 1)], "1x1");
chosen("a memory that rounds", [OFFER(2, 2253, 0.1)], "2x2.2");

// ---- sshAsked
const ssh = (name: string, a: { name?: string; port?: string; keyPath?: string }): void => void add({ fn: "sshAsked", case: name, args: a });
ssh("nothing", {});
ssh("a name", { name: "box" });
ssh("an empty name is kept", { name: "" });
ssh("a port", { port: "22" });
ssh("the lowest port", { port: "1" });
ssh("the highest port", { port: "65535" });
ssh("one past the highest port", { port: "65536" });
ssh("port zero", { port: "0" });
ssh("a negative port", { port: "-22" });
ssh("a fraction", { port: "22.5" });
ssh("not a number", { port: "ssh" });
ssh("an empty port reads as zero", { port: "" });
ssh("a port with white space round it", { port: " 22 " });
ssh("a port in exponent form", { port: "1e3" });
ssh("a binary port", { port: "0b1010" });
ssh("an exponent port of eighty", { port: "8e1" });
ssh("a port with a leading plus", { port: "+22" });
ssh("a port with tabs round it", { port: "\t22\n" });
ssh("a port past the exponent range", { port: "1e400" });
ssh("a hex port", { port: "0x16" });
ssh("a port with quotes and an emoji", { port: "2\"2🧪" });
ssh("an absolute key", { keyPath: "/home/zed/.ssh/id_ed25519" });
ssh("a relative key is refused", { keyPath: "id_ed25519" });
ssh("a key that starts with a tilde is refused", { keyPath: "~/.ssh/id" });
ssh("an empty key is refused", { keyPath: "" });
ssh("all three in the order name, port, key", { name: "box", port: "2222", keyPath: "/k" });
ssh("a bad port is read before the key", { port: "x", keyPath: "rel" });
ssh("a good port and a bad key", { port: "22", keyPath: "rel" });

// ---- the width constants, read through the rows they cap
add({ fn: "constFolderWidth", case: "the folder width" });
add({ fn: "constTitleWidth", case: "the title width" });

const runs: Record<string, Run> = {
  constThreadHead: ({ mod }) => (mod["workspaces"] as any).THREAD_HEAD,
  constNoProjectYet: ({ mod }) => (mod["workspaces"] as any).NO_PROJECT_YET,
  constFolderWidth: ({ mod }) => (mod["workspaces"] as any).shortenedFront("x".repeat(100), 40).length,
  constTitleWidth: ({ mod }) => (mod["workspaces"] as any).shortenedEnd("x".repeat(100), 60).length,
  madeWorktree: ({ mod, args }) => (mod["workspaces"] as any).madeWorktree(args["workspace"]) ?? null,
  dropping: ({ mod, client, args }) => (mod["workspaces"] as any).dropping(client, args["ref"]),
  deleting: ({ mod, client, args }) => (mod["workspaces"] as any).deleting(client, args["ref"]),
  forgetQuestion: ({ mod, args }) => (mod["workspaces"] as any).forgetQuestion(args["d"]),
  forgotLine: ({ mod, args }) => (mod["workspaces"] as any).forgotLine(args["d"]),
  deleteQuestion: ({ mod, args }) => (mod["workspaces"] as any).deleteQuestion(args["d"]),
  deletedLine: ({ mod, args }) => (mod["workspaces"] as any).deletedLine(args["d"]),
  forget: async ({ mod, client, args }) => {
    await (mod["workspaces"] as any).forget(client, args["d"]);
    return null;
  },
  deleteWorkspace: async ({ mod, client, args }) => {
    await (mod["workspaces"] as any).deleteWorkspace(client, args["d"]);
    return null;
  },
  shortenedFront: ({ mod, args }) => (mod["workspaces"] as any).shortenedFront(args["path"], args["width"]),
  shortenedEnd: ({ mod, args }) => (mod["workspaces"] as any).shortenedEnd(args["text"], args["width"]),
  threadLines: ({ mod, args }) => {
    process.env["HOME"] = args["home"] as string;
    return (mod["workspaces"] as any).threadLines(args["t"], args["indent"]);
  },
  diskGoneLines: ({ mod, args }) => (mod["workspaces"] as any).diskGoneLines(args["rows"]),
  threadTree: ({ mod, args }) => {
    const rows = args["rows"] as Obj[];
    return (mod["workspaces"] as any).threadTree(rows).map((n: { row: Obj; depth: number }) => ({ index: rows.indexOf(n.row), id: n.row["id"], depth: n.depth }));
  },
  placeNames: ({ mod, client }) => (mod["workspaces"] as any).placeNames(client),
  projectsOf: ({ mod, client }) => (mod["workspaces"] as any).projectsOf(client),
  projectOf: ({ mod, client, args }) => (mod["workspaces"] as any).projectOf(client, args["ref"]),
  projectLine: ({ mod, args }) => (mod["workspaces"] as any).projectLine(args["p"], args["workspaces"], args["threadsHeld"], new Map(args["named"] as [string, string][])),
  createFor: ({ mod, client, args, sinks }) => {
    const io = { log: (l: string) => sinks.stdout.push(l), error: (l: string) => sinks.stderr.push(l), stream: (t: string) => (sinks.streamed += t) };
    const out = (mod["client"] as any).formatter(io, args["json"] === true);
    return (mod["workspaces"] as any).createFor(client, out, args["project"], args["name"], args["asked"] ?? {});
  },
  projectsHere: async ({ mod, client }) => ((await (mod["workspaces"] as any).projectsHere(client)) as Obj[]).map(p => ({ id: p["id"], name: p["name"], computer: p["computer"] })),
  agentsAsked: ({ mod, args }) => (mod["workspaces"] as any).agentsAsked(args["maxMachines"], args["maxDepth"], args["computer"]) ?? null,
  countAsked: ({ mod, args }) => (mod["workspaces"] as any).countAsked(args["flagName"], args["word"], args["least"], args["computer"]),
  sizeChosen: ({ mod, args }) => (mod["workspaces"] as any).sizeChosen(args["sizes"], args["word"]),
  sshAsked: ({ mod, args }) => (mod["workspaces"] as any).sshAsked(args["name"], args["port"], args["keyPath"]),
};

it("ws_rows", async () => {
  await record("ws_rows", cases, runs);
});
