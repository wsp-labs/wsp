// ws_reads: workspaces-help.ts:111-296 (listings, picking a thread or a workspace, waking, napping, resizing,
// rebuilding, the image, ssh) and threadRows / branchHere (:158-184, :869-881). Recorded by lib.ts against the TS at 65f6b89c7.
import { it } from "vitest";
import { frame, record, refused, type Case, type Obj, type Run } from "./lib.ts";

const ORDER = ["id", "name", "machineId", "phase", "kind", "golden", "createdAt", "project", "folder", "home", "claudeSessionId", "gone", "theme", "glyph", "daemonNote", "daemonRefusedAt", "vaultedAt", "vaultRefused", "wakeRefused", "diskGone", "agents", "parentThreadId", "rootThreadId", "parentWorkspaceId", "place", "provider", "pauseMode", "worktree", "from", "review", "machineState", "size", "rateUsdPerHour", "reason", "idleAt", "facts", "reach"];
const inOrder = (o: Obj): Obj => Object.fromEntries(Object.entries(o).filter(([, v]) => v !== undefined).sort(([a], [b]) => ORDER.indexOf(a) - ORDER.indexOf(b)));
const PROJECT = { id: "proj-1", name: "alpha", path: "/root/alpha", computer: "place-9" };
const ws = (o: Obj = {}): Obj => inOrder({ id: "ws-1", name: "alpha", machineId: "m-1", phase: "running", kind: "cloud", golden: "snap-g1", createdAt: "2026-09-27T01:02:03.004Z", project: PROJECT, ...o });
const listing = (o: Obj = {}): Obj => ws({ machineState: "running", size: { cpu: 2, memMb: 4096 }, rateUsdPerHour: 0.018, reach: { state: "reachable" }, ...o });
const sess = (id: string, thread: string | undefined, o: Obj = {}): Obj => ({ id, workspaceId: "ws-1", harness: "claude", status: "completed", ...(thread !== undefined ? { threadId: thread } : {}), ...o });
const PROJECT_ROW = { id: "proj-1", name: "alpha", computer: "place-9", source: { kind: "folder", path: "/root/alpha" }, path: "/root/alpha", remote: "", defaultBranch: "main", memoryKey: "-root-alpha", memoryDir: "/m/-root-alpha", createdAt: "2026-09-27T00:00:00.000Z" };
const wsList = (...w: Obj[]): string => frame({ workspaces: w });
const statuses = (...w: Obj[]): string => frame({ statuses: w });
const sessions = (...s: Obj[]): string => frame({ sessions: s });
const resolved = (w: Obj): string => frame({ workspace: w });
// The cloud setting changes none of these words; a copy of the first case of each list and of every refusal pins that.
const both = (cs: Case[]): Case[] => [...cs, ...cs.filter((c, i) => i === 0 || /refused by the host|^gone, with the provider/.test(c.case)).map(c => ({ ...c, case: `${c.case} (cloud)`, cloud: true }))];

const HASH = "a".repeat(64);
const IMAGE = { name: "default", version: 2, hash: HASH, recipeHash: "r-1", logins: [{ name: "claude", state: "signed-in" }, { name: "gh", state: "copied" }], sealedAt: "2026-09-20T00:00:00.000Z", sealedFrom: "zingzy's Mac" };
const VAULT = { sha256: "c".repeat(64), bytes: 4096, paths: 1, takenAt: "2026-09-20T00:00:00.000Z" };
const COPY = { place: "solari", version: 2, hash: HASH, snapshotId: "snap-c1", builtAt: "2026-09-20T00:00:00.000Z", sizeBytes: 1288490188 };
const view = (image: Obj | null, copies: Obj[] = []): Obj => ({ image, copies, projects: [] });
const place = { id: "place-9", kind: "computer", name: "attic", default: false };
const stage = (stageName: string, at = "place-9", extra: Obj = {}): string => JSON.stringify({ type: "golden.stage", name: "default", stage: stageName, place: at, ...extra });
const built = (c: Obj = COPY, did = true): string => frame({ build: { built: did, copy: c } });
const dump = (over: Obj): string => frame({ view: over });
const err = (text: string, kind?: string): string => refused(text, kind);

const cases: Case[] = [];
const add = (...c: Case[]): void => void cases.push(...c);

// ---- workspaces / workspaceStatuses
add(
  ...both([
    { fn: "workspaces", case: "none", script: { replies: { "workspaces.list": wsList() } } },
    { fn: "workspaces", case: "one", script: { replies: { "workspaces.list": wsList(ws()) } } },
    { fn: "workspaces", case: "three in the order the host gave", script: { replies: { "workspaces.list": wsList(ws({ id: "ws-3", name: "gamma" }), ws({ id: "ws-1" }), ws({ id: "ws-2", name: "beta", phase: "napping" })) } } },
    { fn: "workspaces", case: "every optional field", script: { replies: { "workspaces.list": wsList(ws({ kind: "local", folder: "/root/alpha", home: "/root", gone: "the provider has no machine m-1", wakeRefused: "no capacity", diskGone: "its disk was deleted on 2026-09-01", place: "place-9", provider: "solari", pauseMode: "memory", worktree: { path: "/root/wt/alpha", branch: "feat/x", made: true }, parentThreadId: "t-lead", rootThreadId: "t-lead", parentWorkspaceId: "ws-0" })) } } },
    { fn: "workspaces", case: "refused by the host", script: { replies: { "workspaces.list": err("not signed in to this host", "auth") } } },
    { fn: "workspaces", case: "refused with no kind", script: { replies: { "workspaces.list": err("the runtime is busy") } } },
    { fn: "workspaces", case: "the host closes during the call", script: { closes: { "workspaces.list": 1006 } } },
    { fn: "workspaces", case: "the host is stopping", script: { closes: { "workspaces.list": 4001 } } },
    { fn: "workspaceStatuses", case: "none", script: { replies: { "status.list": statuses() } } },
    { fn: "workspaceStatuses", case: "one running", script: { replies: { "status.list": statuses(listing()) } } },
    { fn: "workspaceStatuses", case: "paused and unreachable in the host's order", script: { replies: { "status.list": statuses(listing({ id: "ws-2", name: "beta", machineState: "paused", reach: { state: "napping" } }), listing({ id: "ws-1", reach: { state: "unreachable", offline: true } })) } } },
    { fn: "workspaceStatuses", case: "an idle time and a reason", script: { replies: { "status.list": statuses(listing({ phase: "napping", machineState: "paused", reason: "idle 20 min", idleAt: 1790000000000, reach: { state: "napping" } })) } } },
    { fn: "workspaceStatuses", case: "refused by the host", script: { replies: { "status.list": err("status is not readable here", "auth") } } },
  ]),
);

// ---- threads, threadOf, threadsOf (pickThread rides these)
const T1 = "thread-7f3a9c21e4b5";
const T2 = "thread-7f4000000000";
const T3 = "thread-b0b0b0b0c1c1";
const rows3 = sessions(sess("s-1", T1, { prompt: "fix the build" }), sess("s-2", T2, { prompt: "write the tests" }), sess("s-3", T3, { prompt: "review it" }));
add(
  ...both([
    { fn: "threads", case: "no sessions, no workspace named", script: { replies: { "sessions.list": sessions() } } },
    { fn: "threads", case: "a workspace named", args: { workspaceId: "ws-1" }, script: { replies: { "sessions.list": rows3 } } },
    { fn: "threads", case: "an empty workspace id is still sent", args: { workspaceId: "" }, script: { replies: { "sessions.list": sessions() } } },
    { fn: "threads", case: "a row with no thread id is its own thread", script: { replies: { "sessions.list": sessions(sess("s-9", undefined, { prompt: "alone" })) } } },
    { fn: "threads", case: "two turns of one thread fold into one row", script: { replies: { "sessions.list": sessions(sess("s-1", T1, { prompt: "fix the build. then more", status: "completed", claudeSessionId: "c-1", startedAt: 1000, endedAt: 2000 }), sess("s-2", T1, { status: "running", claudeSessionId: "c-1", startedAt: 3000, cwd: "/root/alpha/pkg" })) } } },
    { fn: "threads", case: "threads in the order of their first turn", script: { replies: { "sessions.list": sessions(sess("s-1", "t-b"), sess("s-2", "t-a"), sess("s-3", "t-b"), sess("s-4", "t-c")) } } },
    { fn: "threads", case: "the title is the harness name over the opening", script: { replies: { "sessions.list": sessions(sess("s-1", "t-1", { prompt: "fix the build", harnessTitle: "Build fix\nsecond line" })) } } },
    { fn: "threads", case: "the title is the opening's first sentence", script: { replies: { "sessions.list": sessions(sess("s-1", "t-1", { prompt: "Fix the build.   Then ship it, and tell everybody about it." })) } } },
    { fn: "threads", case: "the title is cut at 47 characters and an ellipsis", script: { replies: { "sessions.list": sessions(sess("s-1", "t-1", { prompt: "a".repeat(30) + " " + "b".repeat(30) })) } } },
    { fn: "threads", case: "the title is the opening at exactly 48", script: { replies: { "sessions.list": sessions(sess("s-1", "t-1", { prompt: "a".repeat(47) + " " })) } } },
    { fn: "threads", case: "the title is the opening, 49 characters on one word", script: { replies: { "sessions.list": sessions(sess("s-1", "t-1", { prompt: "a".repeat(49) })) } } },
    { fn: "threads", case: "the title falls to the agent's session id", script: { replies: { "sessions.list": sessions(sess("s-1", "t-1", { claudeSessionId: "cs-77" })) } } },
    { fn: "threads", case: "the title falls to the row id", script: { replies: { "sessions.list": sessions(sess("s-1", "t-1")) } } },
    { fn: "threads", case: "ran is true for a session the agent started", script: { replies: { "sessions.list": sessions(sess("s-1", "t-1", { claudeSessionId: "cs-1" })) } } },
    { fn: "threads", case: "ran is false for a refused turn", script: { replies: { "sessions.list": sessions(sess("s-1", "t-1", { claudeSessionId: "cs-1", refusal: "sign-in" })) } } },
    { fn: "threads", case: "ran is true while a turn runs", script: { replies: { "sessions.list": sessions(sess("s-1", "t-1", { status: "running" })) } } },
    { fn: "threads", case: "started by an agent, with its parents", script: { replies: { "sessions.list": sessions(sess("s-1", "t-1", { startedBy: "agent", parentThreadId: "t-0", rootThreadId: "t-0", attempt: "a-1" })) } } },
    { fn: "threads", case: "the first turn's parents win over a later one", script: { replies: { "sessions.list": sessions(sess("s-1", "t-1", { parentThreadId: "t-0", rootThreadId: "t-0" }), sess("s-2", "t-1", { parentThreadId: "t-9", rootThreadId: "t-9" })) } } },
    { fn: "threads", case: "cost adds up across turns", script: { replies: { "sessions.list": sessions(sess("s-1", "t-1", { costUsd: 0.25 }), sess("s-2", "t-1", { costUsd: 0.5 })) } } },
    { fn: "threads", case: "refused by the host", script: { replies: { "sessions.list": err("sessions are not readable by this caller", "auth") } } },
  ]),
);
add({ fn: "threads", case: "the key order of a folded row with asking and a cost", script: { replies: { "sessions.list": sessions(sess("s-1", "t-1", { prompt: "go", cwd: "/root/alpha", asking: "may I run make?", costUsd: 0.5, lastLine: "waiting", failure: "x" })) } } });

const pickCases = (fn: "threadOf" | "threadsOf"): Case[] => {
  const arg = (r: string): Obj => (fn === "threadOf" ? { ref: r } : { refs: [r] });
  return [
    { fn, case: "an exact id", args: arg(T1), script: { replies: { "sessions.list": rows3 } } },
    { fn, case: "a prefix naming one", args: arg("thread-b0"), script: { replies: { "sessions.list": rows3 } } },
    { fn, case: "a prefix naming two", args: arg("thread-7f"), script: { replies: { "sessions.list": rows3 } } },
    { fn, case: "a prefix naming all three", args: arg("thread-"), script: { replies: { "sessions.list": rows3 } } },
    { fn, case: "the empty word names every thread", args: arg(""), script: { replies: { "sessions.list": rows3 } } },
    { fn, case: "the empty word with one thread names it", args: arg(""), script: { replies: { "sessions.list": sessions(sess("s-1", T1)) } } },
    { fn, case: "an id that is also the prefix of another wins whole", args: arg("t-1"), script: { replies: { "sessions.list": sessions(sess("s-1", "t-1"), sess("s-2", "t-12")) } } },
    { fn, case: "a prefix of a longer id and a shorter one", args: arg("t-12"), script: { replies: { "sessions.list": sessions(sess("s-1", "t-1"), sess("s-2", "t-12"), sess("s-3", "t-123")) } } },
    { fn, case: "no thread", args: arg("zzz"), script: { replies: { "sessions.list": rows3 } } },
    { fn, case: "no thread among none", args: arg("zzz"), script: { replies: { "sessions.list": sessions() } } },
    { fn, case: "a ref longer than the id", args: arg(T1 + "0"), script: { replies: { "sessions.list": rows3 } } },
    { fn, case: "the id differs in case", args: arg("THREAD-7F3A"), script: { replies: { "sessions.list": rows3 } } },
    { fn, case: "a ref with a space", args: arg("thread-7f3a "), script: { replies: { "sessions.list": rows3 } } },
    { fn, case: "a row with no thread id is named by its row id", args: arg("s-9"), script: { replies: { "sessions.list": sessions(sess("s-9", undefined)) } } },
    { fn, case: "a prefix of a row id", args: arg("s-"), script: { replies: { "sessions.list": sessions(sess("s-9", undefined), sess("s-8", undefined)) } } },
    { fn, case: "two turns of one thread are one hit", args: arg("t-"), script: { replies: { "sessions.list": sessions(sess("s-1", "t-1"), sess("s-2", "t-1")) } } },
    { fn, case: "refused by the host", args: arg(T1), script: { replies: { "sessions.list": err("not allowed", "auth") } } },
  ];
};
add(...both(pickCases("threadOf")));
add(
  ...both([
    ...pickCases("threadsOf").filter(c => /^(an exact id|a prefix naming two|no thread|refused by the host)/.test(c.case)),
    { fn: "threadsOf", case: "none named", args: { refs: [] }, script: { replies: { "sessions.list": rows3 } } },
    { fn: "threadsOf", case: "none named, none held", args: { refs: [] }, script: { replies: { "sessions.list": sessions() } } },
    { fn: "threadsOf", case: "two in the order named", args: { refs: [T3, T1] }, script: { replies: { "sessions.list": rows3 } } },
    { fn: "threadsOf", case: "the same twice is two rows", args: { refs: [T1, "thread-7f3"] }, script: { replies: { "sessions.list": rows3 } } },
    { fn: "threadsOf", case: "the first bad ref stops the rest", args: { refs: ["thread-b0", "nope", "thread-7f"] }, script: { replies: { "sessions.list": rows3 } } },
    { fn: "threadsOf", case: "one listing for many refs", args: { refs: [T1, T2, T3] }, script: { replies: { "sessions.list": rows3 } } },
  ]),
);

// ---- workspaceOf
const OTHER_VERSION: Obj[] = [{ id: "ws-1" }, null as never, "ws-1" as never, 7 as never, { id: "ws-1", name: "alpha" }];
add(
  ...both([
    { fn: "workspaceOf", case: "by id", args: { ref: "ws-1" }, script: { replies: { "workspaces.resolve": resolved(ws()) } } },
    { fn: "workspaceOf", case: "keys come back in the schema's order", args: { ref: "ws-1" }, script: { replies: { "workspaces.resolve": JSON.stringify({ id: 1, ok: true, workspace: { project: PROJECT, phase: "running", createdAt: "2026-09-27T01:02:03.004Z", name: "alpha", golden: "snap-g1", machineId: "m-1", id: "ws-1", kind: "cloud" } }) } } },
    { fn: "workspaceOf", case: "an unknown key is dropped", args: { ref: "ws-1" }, script: { replies: { "workspaces.resolve": JSON.stringify({ id: 1, ok: true, workspace: { ...ws(), extra: 1 } }) } } },
    { fn: "workspaceOf", case: "every optional field", args: { ref: "alpha" }, script: { replies: { "workspaces.resolve": resolved(ws({ kind: "local", folder: "/root/alpha", home: "/root", wakeRefused: "", diskGone: "gone", place: "place-9", provider: "solari", pauseMode: "disk", worktree: { path: "/root/wt", branch: "b", made: true, gone: true } })) } } },
    { fn: "workspaceOf", case: "an empty ref is sent as typed", args: { ref: "" }, script: { replies: { "workspaces.resolve": resolved(ws()) } } },
    { fn: "workspaceOf", case: "a ref with spaces and unicode", args: { ref: "alpha \"one\" \u{1F9EA}" }, script: { replies: { "workspaces.resolve": resolved(ws({ name: "alpha \"one\" \u{1F9EA}" })) } } },
    { fn: "workspaceOf", case: "no such workspace", args: { ref: "nope" }, script: { replies: { "workspaces.resolve": err("no workspace nope; wsp workspaces lists them", "not-found") } } },
    { fn: "workspaceOf", case: "a name two workspaces carry", args: { ref: "alpha" }, script: { replies: { "workspaces.resolve": err("2 workspaces are named alpha: ws-1, ws-2", "usage") } } },
    { fn: "workspaceOf", case: "a workspace this caller may not drive", args: { ref: "ws-9" }, script: { replies: { "workspaces.resolve": err("ws-9 is not one this thread may drive", "auth") } } },
    { fn: "workspaceOf", case: "an answer with no workspace", args: { ref: "ws-1" }, script: { replies: { "workspaces.resolve": frame({}) } } },
    ...OTHER_VERSION.map((w, i): Case => ({ fn: "workspaceOf", case: `a workspace this build cannot read ${i + 1}`, args: { ref: "ws-1" }, script: { replies: { "workspaces.resolve": JSON.stringify({ id: 1, ok: true, workspace: w }) } } })),
  ]),
);

// ---- threadRows and branchHere
const HEAD = (b: string): string => `ref: refs/heads/${b}\n`;
const local = (o: Obj = {}): Obj => ws({ id: "ws-1", kind: "local", folder: "<cwd>/repo", ...o });
const row = (id: string, o: Obj = {}): Obj => sess(`s-${id}`, `t-${id}`, { prompt: `thread ${id}`, project: { id: "proj-1", name: "alpha" }, computerName: "attic", ...o });
const rowsReplies = (w: Obj[], s: Obj[]): Obj => ({ "workspaces.list": wsList(...w), "sessions.list": sessions(...s) });
add(
  ...both([
    { fn: "threadRows", case: "nothing", script: { replies: rowsReplies([], []) } },
    { fn: "threadRows", case: "a thread on a local folder takes the branch git has checked out", script: { replies: rowsReplies([local()], [row("1", { cwd: "<cwd>/repo" })]) }, files: { "repo/.git/HEAD": HEAD("feature/x") } },
    { fn: "threadRows", case: "the checked out branch beats the worktree record", script: { replies: rowsReplies([local({ worktree: { path: "<cwd>/repo", branch: "recorded", made: true } })], [row("1", { cwd: "<cwd>/repo" })]) }, files: { "repo/.git/HEAD": HEAD("switched") } },
    { fn: "threadRows", case: "a folder below the repo root finds the repo", script: { replies: rowsReplies([local()], [row("1", { cwd: "<cwd>/repo/pkg/api" })]) }, files: { "repo/.git/HEAD": HEAD("deep"), "repo/pkg/api/x.txt": "" } },
    { fn: "threadRows", case: "a detached head falls to the worktree record", script: { replies: rowsReplies([local({ worktree: { path: "<cwd>/repo", branch: "recorded", made: true } })], [row("1", { cwd: "<cwd>/repo" })]) }, files: { "repo/.git/HEAD": "0123456789abcdef0123456789abcdef01234567\n" } },
    { fn: "threadRows", case: "a detached head with no worktree record has no branch", script: { replies: rowsReplies([local()], [row("1", { cwd: "<cwd>/repo" })]) }, files: { "repo/.git/HEAD": "0123456789abcdef0123456789abcdef01234567\n" } },
    { fn: "threadRows", case: "a git folder with no head file falls to the record", script: { replies: rowsReplies([local({ worktree: { path: "<cwd>/repo", branch: "recorded", made: false } })], [row("1", { cwd: "<cwd>/repo" })]) }, files: { "repo/.git/config": "" } },
    { fn: "threadRows", case: "a folder in no repo has no branch", script: { replies: rowsReplies([local()], [row("1", { cwd: "<cwd>/plain" })]) }, files: { "plain/x.txt": "" } },
    { fn: "threadRows", case: "a folder that is not there has no branch", script: { replies: rowsReplies([local()], [row("1", { cwd: "<cwd>/missing/deeper" })]) } },
    { fn: "threadRows", case: "a worktree's .git file points at its git dir", script: { replies: rowsReplies([local({ folder: "<cwd>/wt" })], [row("1", { cwd: "<cwd>/wt" })]) }, files: { "wt/.git": "gitdir: <cwd>/gits/wt\n", "gits/wt/HEAD": HEAD("wt-branch") } },
    { fn: "threadRows", case: "a worktree's .git file with a relative path", script: { replies: rowsReplies([local({ folder: "<cwd>/wt" })], [row("1", { cwd: "<cwd>/wt" })]) }, files: { "wt/.git": "gitdir:   ../gits/rel  \n", "gits/rel/HEAD": HEAD("rel-branch") } },
    { fn: "threadRows", case: "a .git file naming no git dir", script: { replies: rowsReplies([local({ folder: "<cwd>/wt", worktree: { path: "<cwd>/wt", branch: "recorded", made: true } })], [row("1", { cwd: "<cwd>/wt" })]) }, files: { "wt/.git": "nonsense\n" } },
    { fn: "threadRows", case: "the ref line is read from any line of HEAD, trimmed", script: { replies: rowsReplies([local()], [row("1", { cwd: "<cwd>/repo" })]) }, files: { "repo/.git/HEAD": "# note\nref: refs/heads/spaced   \n" } },
    { fn: "threadRows", case: "a HEAD with CRLF line ends", script: { replies: rowsReplies([local()], [row("1", { cwd: "<cwd>/repo" })]) }, files: { "repo/.git/HEAD": "ref: refs/heads/crlf\r\n" } },
    { fn: "threadRows", case: "a ref outside heads is no branch", script: { replies: rowsReplies([local()], [row("1", { cwd: "<cwd>/repo" })]) }, files: { "repo/.git/HEAD": "ref: refs/remotes/origin/main\n" } },
    { fn: "threadRows", case: "a branch with slashes", script: { replies: rowsReplies([local()], [row("1", { cwd: "<cwd>/repo" })]) }, files: { "repo/.git/HEAD": HEAD("user/feat/deep-name") } },
    { fn: "threadRows", case: "the thread's own folder wins over the workspace folder", script: { replies: rowsReplies([local({ folder: "<cwd>/other" })], [row("1", { cwd: "<cwd>/repo" })]) }, files: { "repo/.git/HEAD": HEAD("own"), "other/.git/HEAD": HEAD("theirs") } },
    { fn: "threadRows", case: "no turn folder falls to the workspace folder", script: { replies: rowsReplies([local({ folder: "<cwd>/other" })], [row("1")]) }, files: { "other/.git/HEAD": HEAD("theirs") } },
    { fn: "threadRows", case: "no turn folder and no workspace folder falls to the project path", script: { replies: rowsReplies([ws({ id: "ws-1", kind: "local", project: { ...PROJECT, path: "<cwd>/proj" } })], [row("1")]) }, files: { "proj/.git/HEAD": HEAD("proj-branch") } },
    { fn: "threadRows", case: "two threads on one folder", script: { replies: rowsReplies([local()], [row("1", { cwd: "<cwd>/repo" }), row("2", { cwd: "<cwd>/repo" })]) }, files: { "repo/.git/HEAD": HEAD("shared") } },
    { fn: "threadRows", case: "a cloud workspace reads no git here and takes the record", script: { replies: rowsReplies([ws({ id: "ws-1", worktree: { path: "/root/wt", branch: "feat/x", made: true } })], [row("1", { cwd: "<cwd>/repo" })]) }, files: { "repo/.git/HEAD": HEAD("not-this-one") } },
    { fn: "threadRows", case: "a cloud workspace with no record has no branch", script: { replies: rowsReplies([ws()], [row("1", { cwd: "/root/alpha" })]) } },
    { fn: "threadRows", case: "a workspace of no kind is a cloud's", script: { replies: rowsReplies([ws({ kind: undefined, worktree: { path: "/w", branch: "rec", made: true } })], [row("1", { cwd: "<cwd>/repo" })]) }, files: { "repo/.git/HEAD": HEAD("not-this-one") } },
    { fn: "threadRows", case: "a place workspace never reads a folder here, though one stands there", script: { replies: rowsReplies([ws({ kind: "place", worktree: { path: "/w", branch: "rec", made: true } })], [row("1", { cwd: "<cwd>/repo" })]) }, files: { "repo/.git/HEAD": HEAD("not-this-one") } },
    { fn: "threadRows", case: "a place workspace takes the record", script: { replies: rowsReplies([ws({ kind: "place", worktree: { path: "/w", branch: "rec", made: true } })], [row("1", { cwd: "/root/alpha" })]) } },
    { fn: "threadRows", case: "a gone worktree gives no branch", script: { replies: rowsReplies([ws({ worktree: { path: "/root/wt", branch: "feat/x", made: true, gone: true } })], [row("1", { cwd: "/root/other" })]) } },
    { fn: "threadRows", case: "a worktree record with no branch", script: { replies: rowsReplies([ws({ worktree: { path: "/root/wt", made: false } })], [row("1", { cwd: "/root/other" })]) } },
    { fn: "threadRows", case: "a turn inside a gone worktree lists the workspace folder", script: { replies: rowsReplies([ws({ folder: "/root/alpha", worktree: { path: "/root/wt/feat", branch: "feat", made: true, gone: true } })], [row("1", { cwd: "/root/wt/feat/pkg" })]) } },
    { fn: "threadRows", case: "a turn outside a gone worktree keeps its folder", script: { replies: rowsReplies([ws({ folder: "/root/alpha", worktree: { path: "/root/wt/feat", branch: "feat", made: true, gone: true } })], [row("1", { cwd: "/root/wt/feature-two" })]) } },
    { fn: "threadRows", case: "a turn inside a gone worktree with no workspace folder lists the project path", script: { replies: rowsReplies([ws({ worktree: { path: "/root/wt/feat", branch: "feat", made: true, gone: true } })], [row("1", { cwd: "/root/wt/feat" })]) } },
    { fn: "threadRows", case: "a live worktree keeps the turn's folder", script: { replies: rowsReplies([ws({ worktree: { path: "/root/wt/feat", branch: "feat", made: true } })], [row("1", { cwd: "/root/wt/feat/pkg" })]) } },
    { fn: "threadRows", case: "a thread whose workspace is not listed", script: { replies: rowsReplies([], [row("1", { cwd: "/root/gone" })]) } },
    { fn: "threadRows", case: "a thread whose workspace is not listed and no folder", script: { replies: rowsReplies([], [row("1")]) } },
    { fn: "threadRows", case: "the session's project and computer, not the workspace's", script: { replies: rowsReplies([ws()], [row("1", { cwd: "/root/alpha", project: { id: "proj-9", name: "elsewhere" }, computerName: "the Hetzner box" })]) } },
    { fn: "threadRows", case: "a row with no project or computer name", script: { replies: rowsReplies([ws()], [sess("s-1", "t-1", { cwd: "/root/alpha" })]) } },
    { fn: "threadRows", case: "rows in the fold's order", script: { replies: rowsReplies([ws()], [row("b"), row("a"), row("b", { id: "s-b2" }), row("c")]) } },
    { fn: "threadRows", case: "the workspace list is asked before the sessions", script: { replies: rowsReplies([ws()], [row("1")]) } },
    { fn: "threadRows", case: "within a project by name", args: { within: "alpha" }, script: { replies: rowsReplies([ws(), ws({ id: "ws-2", name: "beta", project: { ...PROJECT, id: "proj-2", name: "beta" } })], [row("1"), sess("s-2", "t-2", { workspaceId: "ws-2", project: { id: "proj-2", name: "beta" } })]) } },
    { fn: "threadRows", case: "within a project by id", args: { within: "proj-2" }, script: { replies: rowsReplies([ws(), ws({ id: "ws-2", name: "beta", project: { ...PROJECT, id: "proj-2", name: "beta" } })], [row("1"), sess("s-2", "t-2", { workspaceId: "ws-2", project: { id: "proj-2", name: "beta" } })]) } },
    { fn: "threadRows", case: "within a project whose threads sit on a workspace the caller cannot list", args: { within: "alpha" }, script: { replies: rowsReplies([], [row("1")]) } },
    { fn: "threadRows", case: "within a project no session names, which the projects list holds", args: { within: "quiet" }, script: { replies: { ...rowsReplies([ws()], [row("1")]), "projects.list": frame({ projects: [{ ...PROJECT_ROW, id: "proj-q", name: "quiet" }] }) } } },
    { fn: "threadRows", case: "within a project the projects list holds by id", args: { within: "proj-q" }, script: { replies: { ...rowsReplies([ws()], [row("1")]), "projects.list": frame({ projects: [{ ...PROJECT_ROW, id: "proj-q", name: "quiet" }] }) } } },
    { fn: "threadRows", case: "within a project nothing names", args: { within: "nope" }, script: { replies: { ...rowsReplies([ws()], [row("1")]), "projects.list": frame({ projects: [PROJECT_ROW] }) } } },
    { fn: "threadRows", case: "within the empty word", args: { within: "" }, script: { replies: { ...rowsReplies([ws()], [row("1")]), "projects.list": frame({ projects: [PROJECT_ROW] }) } } },
    { fn: "threadRows", case: "within a project, the projects list refused, the workspaces name it", args: { within: "held" }, script: { replies: { ...rowsReplies([ws({ project: { ...PROJECT, id: "proj-h", name: "held" } })], [row("1")]), "projects.list": err("projects are not listed by this build") } } },
    { fn: "threadRows", case: "within a project nothing names, the projects list refused", args: { within: "nope" }, script: { replies: { ...rowsReplies([ws()], [row("1")]), "projects.list": err("projects are not listed by this build") } } },
    { fn: "threadRows", case: "within a project that is found in the sessions asks no projects", args: { within: "alpha" }, script: { replies: rowsReplies([ws()], [row("1")]) } },
    { fn: "threadRows", case: "the workspace list refused", script: { replies: { "workspaces.list": err("not signed in", "auth") } } },
    { fn: "threadRows", case: "the session list refused", script: { replies: { "workspaces.list": wsList(ws()), "sessions.list": err("sessions are not readable", "auth") } } },
  ]),
);

// ---- stateLine, rebuiltLine (pure)
const PHASES = ["running", "pausing", "napping", "waking", "gone", "queued", "released"];
const stateCases: Case[] = [];
for (const phase of PHASES) stateCases.push({ fn: "stateLine", case: `phase ${phase}`, args: { workspace: ws({ phase }) } });
for (const mode of ["disk", "memory"]) for (const phase of ["pausing", "napping", "running"]) stateCases.push({ fn: "stateLine", case: `phase ${phase} on a ${mode} pause`, args: { workspace: ws({ phase, pauseMode: mode }) } });
add(
  ...both(stateCases),
  ...both([
    { fn: "stateLine", case: "a name with spaces and quotes", args: { workspace: ws({ name: "alpha \"one\"  \u{1F9EA}", phase: "napping" }) } },
    { fn: "stateLine", case: "an empty name", args: { workspace: ws({ name: "", phase: "running" }) } },
    { fn: "rebuiltLine", case: "a running machine", args: { workspace: ws({ machineId: "m-2" }) } },
    { fn: "rebuiltLine", case: "the pause mode is not read", args: { workspace: ws({ machineId: "m-2", phase: "napping", pauseMode: "memory" }) } },
    { fn: "rebuiltLine", case: "a waking machine", args: { workspace: ws({ machineId: "m-3", phase: "waking" }) } },
    { fn: "rebuiltLine", case: "an empty machine id", args: { workspace: ws({ machineId: "" }) } },
  ]),
);

// ---- wokeLine: the listing's word, else the phase
const wokeCase = (name: string, workspace: Obj, listed: Obj[] | undefined): Case => ({ fn: "wokeLine", case: name, args: { workspace }, script: { replies: { "status.list": listed === undefined ? err("status is not readable", "auth") : statuses(...listed) } } });
add(
  ...both([
    wokeCase("the listing reads running", ws(), [listing()]),
    wokeCase("the listing reads the machine up and not answering", ws(), [listing({ reach: { state: "unreachable" } })]),
    wokeCase("the listing reads a zombie", ws(), [listing({ reach: { state: "zombie" } })]),
    wokeCase("the listing reads no daemon", ws(), [listing({ reach: { state: "no-daemon" } })]),
    wokeCase("the listing reads the machine paused under a running phase", ws(), [listing({ machineState: "paused" })]),
    wokeCase("the listing reads the machine starting", ws(), [listing({ machineState: "starting" })]),
    wokeCase("the listing reads the machine gone", ws(), [listing({ machineState: "gone" })]),
    wokeCase("the listing's phase is what it is read by, not the record's", ws({ phase: "running" }), [listing({ phase: "napping", machineState: "paused", reach: { state: "napping" } })]),
    wokeCase("the workspace is not in the listing", ws(), [listing({ id: "ws-2" })]),
    wokeCase("the listing is empty", ws({ phase: "napping" }), []),
    wokeCase("the listing is empty on a waking phase", ws({ phase: "waking" }), []),
    wokeCase("the second of two", ws({ id: "ws-2", name: "beta" }), [listing(), listing({ id: "ws-2", name: "beta", machineState: "paused" })]),
    wokeCase("a pause that keeps memory, read as paused", ws({ pauseMode: "memory", phase: "running" }), [listing({ machineState: "paused", reach: { state: "napping" } })]),
    wokeCase("a disk pause, read as stopped (the listing says asleep)", ws({ id: "ws-kid", name: "kid", pauseMode: "disk", phase: "napping" }), [listing({ id: "ws-kid", name: "kid", phase: "napping", machineState: "paused", reach: { state: "napping" } })]),
    wokeCase("the status list refused", ws(), undefined),
  ]),
);

// ---- nap, resize
const napped = (o: Obj = {}): string => resolved(ws({ phase: "napping", ...o }));
add(
  ...both([
    { fn: "nap", case: "a running workspace", args: { ref: "alpha" }, script: { replies: { "workspaces.resolve": resolved(ws()), "workspaces.nap": napped() } } },
    { fn: "nap", case: "the nap is asked by id, not the word typed", args: { ref: "alpha" }, script: { replies: { "workspaces.resolve": resolved(ws({ id: "ws-77" })), "workspaces.nap": napped({ id: "ws-77" }) } } },
    { fn: "nap", case: "no such workspace asks no nap", args: { ref: "nope" }, script: { replies: { "workspaces.resolve": err("no workspace nope; wsp workspaces lists them", "not-found") } } },
    { fn: "nap", case: "the nap refused", args: { ref: "alpha" }, script: { replies: { "workspaces.resolve": resolved(ws()), "workspaces.nap": err("the provider refused the pause", "provider") } } },
    { fn: "nap", case: "a record this build cannot read asks no nap", args: { ref: "alpha" }, script: { replies: { "workspaces.resolve": JSON.stringify({ id: 1, ok: true, workspace: {} }) } } },
    { fn: "nap", case: "already napping", args: { ref: "alpha" }, script: { replies: { "workspaces.resolve": napped(), "workspaces.nap": napped() } } },
    { fn: "nap", case: "the answer is not parsed, as the host wrote it", args: { ref: "alpha" }, script: { replies: { "workspaces.resolve": resolved(ws()), "workspaces.nap": napped({ pauseMode: "memory", provider: "solari" }) } } },
    { fn: "resize", case: "a thread's machine", args: { ref: T1, size: { cpu: 4, memMb: 8192 } }, script: { replies: { "sessions.list": rows3, "workspaces.resize": resolved(ws({ phase: "running" })) } } },
    { fn: "resize", case: "the thread is named by a prefix", args: { ref: "thread-b0", size: { cpu: 1, memMb: 1024 } }, script: { replies: { "sessions.list": rows3, "workspaces.resize": resolved(ws()) } } },
    { fn: "resize", case: "the thread's workspace id, not the thread's, is sent", args: { ref: "t-1", size: { cpu: 2, memMb: 2048 } }, script: { replies: { "sessions.list": sessions(sess("s-1", "t-1", { workspaceId: "ws-42" })), "workspaces.resize": resolved(ws({ id: "ws-42" })) } } },
    { fn: "resize", case: "no such thread asks no resize", args: { ref: "zzz", size: { cpu: 2, memMb: 2048 } }, script: { replies: { "sessions.list": rows3 } } },
    { fn: "resize", case: "a prefix naming two asks no resize", args: { ref: "thread-7f", size: { cpu: 2, memMb: 2048 } }, script: { replies: { "sessions.list": rows3 } } },
    { fn: "resize", case: "the size is not on offer", args: { ref: T1, size: { cpu: 3, memMb: 100 } }, script: { replies: { "sessions.list": rows3, "workspaces.resize": err("3 vCPU and 100 MB is not a size this provider offers", "usage") } } },
    { fn: "resize", case: "a turn is running", args: { ref: T1, size: { cpu: 8, memMb: 16384 } }, script: { replies: { "sessions.list": rows3, "workspaces.resize": err("a turn is running on it; stop it first", "usage") } } },
  ]),
);

// ---- rebuild
const rebuildCase = (name: string, source: Obj, listed: Obj[] | undefined, rebuilt?: Obj): Case => ({
  fn: "rebuild",
  case: name,
  args: { ref: "alpha" },
  script: { replies: { "workspaces.resolve": resolved(source), ...(listed === undefined ? {} : { "status.list": statuses(...listed) }), "workspaces.rebuild": resolved(rebuilt ?? ws({ machineId: "m-2" })) } },
});
add(
  ...both([
    rebuildCase("a gone phase", ws({ phase: "gone", gone: "the provider has no machine m-1" }), []),
    rebuildCase("a gone phase with a listing", ws({ phase: "gone" }), [listing({ phase: "gone", machineState: "gone", reach: { state: "gone" } })]),
    rebuildCase("a running phase whose machine the listing reads gone", ws(), [listing({ machineState: "gone", reach: { state: "gone" } })]),
    rebuildCase("a zombie", ws(), [listing({ reach: { state: "zombie" } })], ws({ phase: "waking" })),
    rebuildCase("a wake the provider refused, with no reason", ws({ phase: "napping", wakeRefused: "" }), []),
    rebuildCase("a wake the provider refused, with a reason", ws({ phase: "napping", wakeRefused: "no capacity" }), []),
    rebuildCase("a wake refused on a running workspace", ws({ wakeRefused: "x" }), [listing()]),
    rebuildCase("a released workspace", ws({ phase: "released" }), []),
    rebuildCase("running and reachable, refused", ws(), [listing()]),
    rebuildCase("running with no listing, refused", ws(), []),
    rebuildCase("unreachable is not gone, refused", ws(), [listing({ reach: { state: "unreachable" } })]),
    rebuildCase("no daemon is not gone, refused", ws(), [listing({ reach: { state: "no-daemon" } })]),
    rebuildCase("napping, refused", ws({ phase: "napping" }), [listing({ phase: "napping", machineState: "paused", reach: { state: "napping" } })]),
    rebuildCase("napping on a memory pause, refused", ws({ phase: "napping", pauseMode: "memory" }), []),
    rebuildCase("pausing, refused", ws({ phase: "pausing" }), []),
    rebuildCase("waking, refused", ws({ phase: "waking" }), []),
    rebuildCase("queued, refused", ws({ phase: "queued" }), []),
    rebuildCase("paused machine under a running phase, refused", ws(), [listing({ machineState: "paused" })]),
    rebuildCase("the listing of another workspace is not read", ws(), [listing({ id: "ws-2", reach: { state: "zombie" } })]),
    rebuildCase("a gone workspace whose rebuild came back running", ws({ phase: "gone" }), [], ws({ machineId: "m-9", phase: "running" })),
    { fn: "rebuild", case: "no such workspace asks no status", args: { ref: "nope" }, script: { replies: { "workspaces.resolve": err("no workspace nope; wsp workspaces lists them", "not-found") } } },
    { fn: "rebuild", case: "the status list refused", args: { ref: "alpha" }, script: { replies: { "workspaces.resolve": resolved(ws({ phase: "gone" })), "status.list": err("status is not readable", "auth") } } },
    { fn: "rebuild", case: "the rebuild refused", args: { ref: "alpha" }, script: { replies: { "workspaces.resolve": resolved(ws({ phase: "gone" })), "status.list": statuses(), "workspaces.rebuild": err("the provider refused to make a machine", "provider") } } },
  ]),
);

// ---- imageView, imageLines
const imageViews: [string, Obj][] = [
  ["none", view(null)],
  ["a record with no copies", view(IMAGE)],
  ["a record with a vault and one copy", view({ ...IMAGE, vault: VAULT, usedBytes: 5637144576 }, [COPY])],
  ["a copy built from an older record", view({ ...IMAGE, vault: VAULT }, [{ ...COPY, hash: "b".repeat(64) }])],
  ["a copy with no hash", view({ ...IMAGE, vault: VAULT }, [{ place: "attic", version: 1, snapshotId: "snap-c2", builtAt: "2026-09-19T00:00:00.000Z" }])],
  ["no vault and no mark, so no standing", view(IMAGE, [COPY])],
  ["no secrets, so a standing without a vault", view({ ...IMAGE, noSecrets: true }, [COPY, { ...COPY, place: "attic", hash: "b".repeat(64) }])],
  ["a copy with no size", view({ ...IMAGE, vault: VAULT }, [{ ...COPY, sizeBytes: undefined }])],
  ["copies in the host's order", view({ ...IMAGE, vault: VAULT }, [{ ...COPY, place: "zeta" }, { ...COPY, place: "alpha", version: 1 }, { ...COPY, place: "mid", version: 12 }])],
  ["a copy of zero bytes", view({ ...IMAGE, vault: VAULT }, [{ ...COPY, sizeBytes: 0 }])],
  ["a copy of a gigabyte less one byte and a gigabyte", view({ ...IMAGE, vault: VAULT }, [{ ...COPY, sizeBytes: 1073741823 }, { ...COPY, place: "b", sizeBytes: 1073741824 }])],
  ["a copy of 1023 and 1024 bytes", view({ ...IMAGE, vault: VAULT }, [{ ...COPY, sizeBytes: 1023 }, { ...COPY, place: "b", sizeBytes: 1024 }])],
  ["a record of the place it stands at, with a template", view({ ...IMAGE, vault: VAULT, place: "place-9" }, [{ ...COPY, templateId: "tpl-1" }])],
];
add(
  ...both(imageViews.map(([name, v]): Case => ({ fn: "imageLines", case: name, args: { view: v } }))),
  ...both(imageViews.slice(0, 4).map(([name, v]): Case => ({ fn: "imageView", case: name, script: { replies: { "image.get": frame({ view: v }) } } }))),
  ...both([
    { fn: "imageView", case: "an unknown key is dropped", script: { replies: { "image.get": frame({ view: { ...view({ ...IMAGE, extra: 1 }, [{ ...COPY, unknown: 1 }]), more: true } }) } } },
    { fn: "imageView", case: "keys come back in the schema's order", script: { replies: { "image.get": frame({ view: { projects: [], copies: [{ builtAt: "2026-09-20T00:00:00.000Z", snapshotId: "snap-c1", version: 2, place: "solari" }], image: { sealedFrom: "zingzy's Mac", sealedAt: "2026-09-20T00:00:00.000Z", logins: [], recipeHash: "r-1", hash: HASH, version: 2, name: "default" } } }) } } },
    { fn: "imageView", case: "the frame sent is image.get with no params", script: { replies: { "image.get": frame({ view: view(null) }) } } },
    { fn: "imageView", case: "refused by the host", script: { replies: { "image.get": err("the image is not readable here", "auth") } } },
  ]),
);

// ---- buildImageAt
const placed = { "places.list": frame({ places: [place] }) };
const boatPlace = { id: "box", kind: "provider", name: "box", default: false };
const IMAGE_VIEW = frame({ view: view({ ...IMAGE, vault: VAULT }, [COPY]) });
add(
  ...both([
    { fn: "buildImageAt", case: "built, with the stages streamed", args: { word: "attic" }, script: { replies: { ...placed, "image.build": built(), "image.get": IMAGE_VIEW }, pushed: { "image.build": [stage("creating"), stage("deploying-daemon"), stage("sealed")] } } },
    { fn: "buildImageAt", case: "a place by its id", args: { word: "place-9" }, script: { replies: { ...placed, "image.build": built(), "image.get": IMAGE_VIEW } } },
    { fn: "buildImageAt", case: "a word that names no place is sent as typed", args: { word: "solari" }, script: { replies: { ...placed, "image.build": built(COPY, false), "image.get": IMAGE_VIEW }, pushed: { "image.build": [stage("creating", "solari")] } } },
    { fn: "buildImageAt", case: "forced", args: { word: "attic", force: true }, script: { replies: { ...placed, "image.build": built(), "image.get": IMAGE_VIEW } } },
    { fn: "buildImageAt", case: "force false is not sent", args: { word: "attic", force: false }, script: { replies: { ...placed, "image.build": built(), "image.get": IMAGE_VIEW } } },
    { fn: "buildImageAt", case: "already built", args: { word: "attic" }, script: { replies: { ...placed, "image.build": built(COPY, false), "image.get": IMAGE_VIEW } } },
    { fn: "buildImageAt", case: "a failed stage with its detail", args: { word: "attic" }, script: { replies: { ...placed, "image.build": err("the build stopped", "provider") }, pushed: { "image.build": [stage("creating"), stage("failed", "place-9", { detail: "the provider refused" })] } } },
    { fn: "buildImageAt", case: "a failed stage with no detail", args: { word: "attic" }, script: { replies: { ...placed, "image.build": err("the build stopped", "provider") }, pushed: { "image.build": [stage("failed")] } } },
    { fn: "buildImageAt", case: "a stage with a detail", args: { word: "attic" }, script: { replies: { ...placed, "image.build": built(), "image.get": IMAGE_VIEW }, pushed: { "image.build": [stage("uploading-files", "place-9", { detail: "3 of 9 files" })] } } },
    { fn: "buildImageAt", case: "every stage in order", args: { word: "attic" }, script: { replies: { ...placed, "image.build": built(), "image.get": IMAGE_VIEW }, pushed: { "image.build": ["creating", "deploying-daemon", "applying-setup", "uploading-files", "installing-harness", "installing-tools", "installing-mcp", "ready", "snapshotting", "promoting", "smoke-forking", "sealed"].map(s => stage(s)) } } },
    { fn: "buildImageAt", case: "another place's stages and other frames are not streamed", args: { word: "attic" }, script: { replies: { ...placed, "image.build": built(), "image.get": IMAGE_VIEW }, pushed: { "image.build": [stage("creating", "place-2"), JSON.stringify({ type: "thread.updated", place: "place-9" }), stage("promoting")] } } },
    { fn: "buildImageAt", case: "a provider named in any case takes its table name", args: { word: "BOX" }, script: { replies: { "places.list": frame({ places: [boatPlace] }), "image.build": built(), "image.get": IMAGE_VIEW }, pushed: { "image.build": [stage("creating", "box")] } } },
    { fn: "buildImageAt", case: "a stage frame heard after the build returned is not streamed", args: { word: "attic" }, script: { replies: { ...placed, "image.build": built(), "image.get": IMAGE_VIEW }, after: { "image.get": [stage("promoting")] } } },
    { fn: "buildImageAt", case: "the place list names nobody and the build is refused", args: { word: "nowhere" }, script: { replies: { "places.list": frame({ places: [] }), "image.build": err("no computer or cloud named nowhere", "usage") } } },
    { fn: "buildImageAt", case: "the place list refused", args: { word: "attic" }, script: { replies: { "places.list": err("not allowed", "auth") } } },
    { fn: "buildImageAt", case: "no image stands after the build", args: { word: "attic" }, script: { replies: { ...placed, "image.build": built(), "image.get": frame({ view: view(null) }) } } },
    { fn: "buildImageAt", case: "no image after a build of boat-add's shape, an empty build record", args: { word: "boat" }, script: { replies: { "places.list": frame({ places: [{ ...boatPlace, rateUsdPerHour: 0.018, takesForks: true }] }), "image.build": frame({ build: {} }), "image.get": frame({ view: view(null) }) }, pushed: { "image.build": [stage("creating", "box")] } } },
    { fn: "buildImageAt", case: "the build refused after stages were streamed", args: { word: "attic" }, script: { replies: { ...placed, "image.build": err("the provider has no capacity", "provider") }, pushed: { "image.build": [stage("creating"), stage("deploying-daemon")] } } },
    { fn: "buildImageAt", case: "the image read refused after the build", args: { word: "attic" }, script: { replies: { ...placed, "image.build": built(), "image.get": err("the image is not readable", "auth") } } },
    { fn: "buildImageAt", case: "the host closes during the build", args: { word: "attic" }, script: { replies: placed, pushed: { "image.build": [stage("creating")] }, closes: { "image.build": 1006 } } },
    { fn: "buildImageAt", case: "the host stops during the build", args: { word: "attic" }, script: { replies: placed, closes: { "image.build": 4001 } } },
  ]),
);

// ---- the constants and sshWorkspaceOf
const sshList = (...names: [string, string][]): string => wsList(...names.map(([id, name]) => ws({ id, name })));
add(
  ...both([
    { fn: "const_host_side_vault", case: "the sentence" },
    { fn: "const_ssh_pipes_here_line", case: "the sentence" },
    { fn: "sshWorkspaceOf", case: "a name, not an alias, goes to the resolve", args: { ref: "alpha" }, script: { replies: { "workspaces.resolve": resolved(ws()) } } },
    { fn: "sshWorkspaceOf", case: "a name that is not an alias asks no list", args: { ref: "ws-1" }, script: { replies: { "workspaces.resolve": resolved(ws()) } } },
    { fn: "sshWorkspaceOf", case: "an alias of one workspace", args: { ref: "wsp-cart-rounding" }, script: { replies: { "workspaces.list": sshList(["ws-1", "Cart rounding"], ["ws-2", "beta"]) } } },
    { fn: "sshWorkspaceOf", case: "an alias answers the host's list entry whole", args: { ref: "wsp-alpha" }, script: { replies: { "workspaces.list": wsList(ws({ folder: "/root/alpha", phase: "napping", pauseMode: "disk" })) } } },
    { fn: "sshWorkspaceOf", case: "an alias of two workspaces", args: { ref: "wsp-a-b" }, script: { replies: { "workspaces.list": sshList(["ws-1", "A b"], ["ws-2", "a-b"]) } } },
    { fn: "sshWorkspaceOf", case: "an alias of three workspaces", args: { ref: "wsp-x" }, script: { replies: { "workspaces.list": sshList(["ws-1", "X"], ["ws-2", "x"], ["ws-3", "!x!"]) } } },
    { fn: "sshWorkspaceOf", case: "an alias of none falls to the resolve", args: { ref: "wsp-none" }, script: { replies: { "workspaces.list": sshList(["ws-1", "alpha"]), "workspaces.resolve": resolved(ws({ id: "ws-9", name: "wsp-none" })) } } },
    { fn: "sshWorkspaceOf", case: "an alias of none and no such workspace", args: { ref: "wsp-none" }, script: { replies: { "workspaces.list": sshList(["ws-1", "alpha"]), "workspaces.resolve": err("no workspace wsp-none; wsp workspaces lists them", "not-found") } } },
    { fn: "sshWorkspaceOf", case: "the prefix alone", args: { ref: "wsp-" }, script: { replies: { "workspaces.list": sshList(["ws-1", "alpha"]), "workspaces.resolve": err("no workspace wsp-; wsp workspaces lists them", "not-found") } } },
    { fn: "sshWorkspaceOf", case: "the prefix in capitals is a name", args: { ref: "WSP-alpha" }, script: { replies: { "workspaces.resolve": resolved(ws()) } } },
    { fn: "sshWorkspaceOf", case: "a name with no letters or digits is work", args: { ref: "wsp-work" }, script: { replies: { "workspaces.list": sshList(["ws-1", "\u{1F9EA}\u{1F9EA}"]) } } },
    { fn: "sshWorkspaceOf", case: "a long name is cut at 40", args: { ref: "wsp-" + "abcdefghij".repeat(4) }, script: { replies: { "workspaces.list": sshList(["ws-1", "abcdefghij".repeat(5)]) } } },
    { fn: "sshWorkspaceOf", case: "a long name cut at a dash drops the dash", args: { ref: "wsp-" + "a".repeat(39) }, script: { replies: { "workspaces.list": sshList(["ws-1", "a".repeat(39) + "-bcd"]) } } },
    { fn: "sshWorkspaceOf", case: "an empty list falls to the resolve", args: { ref: "wsp-alpha" }, script: { replies: { "workspaces.list": wsList(), "workspaces.resolve": err("no workspace wsp-alpha; wsp workspaces lists them", "not-found") } } },
    { fn: "sshWorkspaceOf", case: "the list refused", args: { ref: "wsp-alpha" }, script: { replies: { "workspaces.list": err("not allowed", "auth") } } },
    { fn: "sshWorkspaceOf", case: "the resolve answers a record this build cannot read", args: { ref: "alpha" }, script: { replies: { "workspaces.resolve": frame({ workspace: { id: "ws-1" } }) } } },
  ]),
);

const runs: Record<string, Run> = {
  workspaces: ({ mod, client }) => (mod["workspaces"] as any).workspaces(client),
  workspaceStatuses: ({ mod, client }) => (mod["workspaces"] as any).workspaceStatuses(client),
  threads: ({ mod, client, args }) => (mod["workspaces"] as any).threads(client, args["workspaceId"]),
  threadOf: ({ mod, client, args }) => (mod["workspaces"] as any).threadOf(client, args["ref"]),
  threadsOf: ({ mod, client, args }) => (mod["workspaces"] as any).threadsOf(client, args["refs"]),
  workspaceOf: ({ mod, client, args }) => (mod["workspaces"] as any).workspaceOf(client, args["ref"]),
  threadRows: async ({ mod, client, args, cwd }) => JSON.parse(JSON.stringify(await (mod["workspaces"] as any).threadRows(client, args["within"])).split(cwd).join("<cwd>")),
  stateLine: ({ mod, args }) => (mod["workspaces"] as any).stateLine(args["workspace"]),
  rebuiltLine: ({ mod, args }) => (mod["workspaces"] as any).rebuiltLine(args["workspace"]),
  wokeLine: ({ mod, client, args }) => (mod["workspaces"] as any).wokeLine(client, args["workspace"]),
  nap: ({ mod, client, args }) => (mod["workspaces"] as any).nap(client, args["ref"]),
  resize: ({ mod, client, args }) => (mod["workspaces"] as any).resize(client, args["ref"], args["size"]),
  rebuild: ({ mod, client, args }) => (mod["workspaces"] as any).rebuild(client, args["ref"]),
  imageView: ({ mod, client }) => (mod["workspaces"] as any).imageView(client),
  imageLines: ({ mod, args }) => (mod["workspaces"] as any).imageLines(args["view"]),
  buildImageAt: ({ mod, client, args, sinks }) => {
    const io = { log: (l: string) => sinks.stdout.push(l), error: (l: string) => sinks.stderr.push(l), stream: (t: string) => (sinks.streamed += t) };
    const out = (mod["client"] as any).formatter(io, false);
    return (mod["workspaces"] as any).buildImageAt(client, out, args["word"], args["force"]);
  },
  const_host_side_vault: ({ mod }) => Promise.resolve((mod["workspaces"] as any).HOST_SIDE_VAULT),
  const_ssh_pipes_here_line: ({ mod }) => Promise.resolve((mod["workspaces"] as any).SSH_PIPES_HERE_LINE),
  sshWorkspaceOf: ({ mod, client, args }) => (mod["workspaces"] as any).sshWorkspaceOf(client, args["ref"]),
};

it("ws_reads", async () => {
  await record("ws_reads", cases, runs);
});
