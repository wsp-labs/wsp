// ws_life: workspaces-help.ts:311-598 (renaming a workspace, waking, committing, fixing, merging, starting and
// reviewing, the napping a dead launch owes, stopping, forgetting, settling, restoring, renaming a thread).
// Recorded by lib.ts against the TS at 65f6b89c7.
import { it } from "vitest";
import { frame, record, recorded, refused, type Case, type Obj, type Run } from "./lib.ts";

const ORDER = ["id", "name", "machineId", "phase", "kind", "golden", "createdAt", "project", "folder", "home", "claudeSessionId", "gone", "theme", "glyph", "daemonNote", "daemonRefusedAt", "vaultedAt", "vaultRefused", "wakeRefused", "diskGone", "agents", "parentThreadId", "rootThreadId", "parentWorkspaceId", "place", "provider", "pauseMode", "worktree", "from", "review"];
const inOrder = (o: Obj): Obj => Object.fromEntries(Object.entries(o).filter(([, v]) => v !== undefined).sort(([a], [b]) => ORDER.indexOf(a) - ORDER.indexOf(b)));
const PROJECT = { id: "proj-1", name: "alpha", path: "/root/alpha", computer: "place-9" };
const ws = (o: Obj = {}): Obj => inOrder({ id: "ws_1", name: "alpha", machineId: "m-1", phase: "running", kind: "cloud", golden: "snap-g1", createdAt: "2026-09-27T01:02:03.004Z", project: PROJECT, ...o });
const tv = (o: Obj = {}): Obj => ({ id: "thread-7f3a9c21e4b5", threadId: "thread-7f3a9c21e4b5", workspaceId: "ws_1", harness: "claude", startedBy: "person", status: "completed", title: "fix the build", sessionId: "s-1", turns: 1, ran: true, ...o });
const sess = (id: string, thread: string | undefined, o: Obj = {}): Obj => ({ id, workspaceId: "ws_1", harness: "claude", status: "completed", ...(thread !== undefined ? { threadId: thread } : {}), ...o });
const sessions = (...s: Obj[]): string => frame({ sessions: s });
const resolved = (w: Obj): string => frame({ workspace: w });
const err = (text: string, kind?: string): string => refused(text, kind);
const both = (cs: Case[]): Case[] => [...cs, ...cs.filter((c, i) => i === 0 || /refused by the host|^gone, with the provider/.test(c.case)).map(c => ({ ...c, case: `${c.case} (cloud)`, cloud: true }))];

const cases: Case[] = [];
const add = (...c: Case[]): void => void cases.push(...c);

// ---- renameWorkspace, renamedWorkspaceLine
add(
  ...both([
    { fn: "renameWorkspace", case: "renamed", args: { ref: "alpha", name: "beta" }, script: { replies: { "workspaces.resolve": resolved(ws()), "workspaces.rename": resolved(ws({ name: "beta" })) } } },
    { fn: "renameWorkspace", case: "the rename is sent by id with the name as typed", args: { ref: "alpha", name: "  Beta  \u{1F9EA} " }, script: { replies: { "workspaces.resolve": resolved(ws({ id: "ws_77" })), "workspaces.rename": resolved(ws({ id: "ws_77", name: "  Beta  \u{1F9EA} " })) } } },
    { fn: "renameWorkspace", case: "the old name is the resolved one, not the word typed", args: { ref: "ws_1", name: "beta" }, script: { replies: { "workspaces.resolve": resolved(ws({ name: "the old name" })), "workspaces.rename": resolved(ws({ name: "beta" })) } } },
    { fn: "renameWorkspace", case: "a blank name is the runtime's refusal", args: { ref: "alpha", name: "" }, script: { replies: { "workspaces.resolve": resolved(ws()), "workspaces.rename": err("a workspace's name cannot be blank", "usage") } } },
    { fn: "renameWorkspace", case: "a name another workspace carries", args: { ref: "alpha", name: "beta" }, script: { replies: { "workspaces.resolve": resolved(ws()), "workspaces.rename": err("beta is already the name of another workspace", "usage") } } },
    { fn: "renameWorkspace", case: "no such workspace asks no rename", args: { ref: "nope", name: "beta" }, script: { replies: { "workspaces.resolve": err("no workspace nope; wsp workspaces lists them", "not-found") } } },
    { fn: "renameWorkspace", case: "a record this build cannot read asks no rename", args: { ref: "alpha", name: "beta" }, script: { replies: { "workspaces.resolve": frame({ workspace: { id: "ws_1" } }) } } },
    { fn: "renameWorkspace", case: "the rename refused by the host", args: { ref: "alpha", name: "beta" }, script: { replies: { "workspaces.resolve": resolved(ws()), "workspaces.rename": err("not allowed", "auth") } } },
    { fn: "renamedWorkspaceLine", case: "a rename", args: { renamed: { was: "alpha", workspace: ws({ name: "beta" }) } } },
    { fn: "renamedWorkspaceLine", case: "names with spaces and quotes", args: { renamed: { was: "alpha \"one\"", workspace: ws({ id: "ws_9", name: "b  \u{1F9EA}" }) } } },
    { fn: "renamedWorkspaceLine", case: "an empty old name", args: { renamed: { was: "", workspace: ws({ name: "beta" }) } } },
  ]),
);

// ---- awake
const WAKE_FRAME_ONLY = (phase: string, o: Obj = {}): Obj => ({ "workspaces.wake": resolved(ws({ phase, ...o })) });
const awakeCases: Case[] = [];
for (const phase of ["running", "napping", "pausing", "waking", "queued", "released"]) {
  for (const action of ["send", "merge in"]) {
    awakeCases.push({ fn: "awake", case: `${phase} for ${action}, the wake answers running`, args: { workspace: ws({ phase }), action }, script: { replies: WAKE_FRAME_ONLY("running") } });
  }
}
for (const phase of ["napping", "pausing", "waking", "running"]) {
  awakeCases.push({ fn: "awake", case: `${phase}, the wake leaves it napping`, args: { workspace: ws({ phase }), action: "send" }, script: { replies: WAKE_FRAME_ONLY("napping") } });
  awakeCases.push({ fn: "awake", case: `${phase}, the wake leaves it waking`, args: { workspace: ws({ phase }), action: "send" }, script: { replies: WAKE_FRAME_ONLY("waking") } });
  awakeCases.push({ fn: "awake", case: `${phase} with its disk gone, the wake answers running`, args: { workspace: ws({ phase }), action: "send" }, script: { replies: WAKE_FRAME_ONLY("running", { diskGone: "the provider deleted its disk on 2026-09-01" }) } });
}
add(
  ...both([
    ...awakeCases,
    { fn: "awake", case: "napping with its disk gone, the wake leaves it napping", args: { workspace: ws({ phase: "napping" }), action: "send" }, script: { replies: WAKE_FRAME_ONLY("napping", { diskGone: "the disk is gone" }) } },
    { fn: "awake", case: "napping with an empty disk line", args: { workspace: ws({ phase: "napping" }), action: "send" }, script: { replies: WAKE_FRAME_ONLY("running", { diskGone: "" }) } },
    { fn: "awake", case: "the wake answers the record the host wrote", args: { workspace: ws({ phase: "napping" }), action: "send" }, script: { replies: WAKE_FRAME_ONLY("running", { id: "ws_other", name: "other", machineId: "m-9", pauseMode: "memory", provider: "solari" }) } },
    { fn: "awake", case: "the wake is asked by the workspace id", args: { workspace: ws({ id: "ws_42", phase: "napping" }), action: "send" }, script: { replies: WAKE_FRAME_ONLY("running", { id: "ws_42" }) } },
    { fn: "awake", case: "gone, with the provider's words", args: { workspace: ws({ phase: "gone", gone: "the provider has no machine m-1" }), action: "send" } },
    { fn: "awake", case: "gone, with no words", args: { workspace: ws({ phase: "gone" }), action: "send" } },
    { fn: "awake", case: "gone, with empty words", args: { workspace: ws({ phase: "gone", gone: "" }), action: "merge in" } },
    { fn: "awake", case: "gone, a name with spaces and an empty action", args: { workspace: ws({ phase: "gone", name: "alpha \"one\" \u{1F9EA}", gone: "x" }), action: "" } },
    { fn: "awake", case: "napping, the wake refused by the host", args: { workspace: ws({ phase: "napping" }), action: "send" }, script: { replies: { "workspaces.wake": err("the provider has no capacity", "provider") } } },
    { fn: "awake", case: "running, the wake refused by the host", args: { workspace: ws({ phase: "running" }), action: "send" }, script: { replies: { "workspaces.wake": err("the machine is gone with its disk", "usage") } } },
    { fn: "awake", case: "napping, the host closes during the wake", args: { workspace: ws({ phase: "napping" }), action: "send" }, script: { closes: { "workspaces.wake": 1006 } } },
    { fn: "awake", case: "napping, the host stops during the wake", args: { workspace: ws({ phase: "napping" }), action: "send" }, script: { closes: { "workspaces.wake": 4001 } } },
    { fn: "awake", case: "a memory-pause workspace says the same words", args: { workspace: ws({ phase: "napping", pauseMode: "memory" }), action: "send" }, script: { replies: WAKE_FRAME_ONLY("running") } },
  ]),
);

// ---- noDraftLine and the fix constant
add(
  ...both([
    { fn: "noDraftLine", case: "a note", args: { note: "No agent here drafts commit messages; write it yourself." } },
    { fn: "noDraftLine", case: "no note", args: {} },
    { fn: "noDraftLine", case: "an empty note", args: { note: "" } },
    { fn: "const_no_draft_fix", case: "the sentence" },
  ]),
);

// ---- committed
const commitReply = frame({ oid: "0123456789abcdef0123", subject: "Fix the thing", filesChanged: 1, insertions: 3, deletions: 1 });
const draft = (message: string | null, note?: string): string => frame({ message, ...(note !== undefined ? { note } : {}) });
const say = ["fix: thing"];
void say;
add(
  ...both([
    { fn: "committed", case: "a message given, every change", args: { workspaceId: "ws_1", message: "Fix the thing", files: [] }, script: { replies: { "workspaces.commit": commitReply } } },
    { fn: "committed", case: "a message given with the files named", args: { workspaceId: "ws_1", message: "Fix the thing", files: ["a.ts", "src/b c.ts"] }, script: { replies: { "workspaces.commit": commitReply } } },
    { fn: "committed", case: "a message given, a thread named", args: { workspaceId: "ws_1", message: "Fix the thing", files: [], threadId: "thread-7f3a9c21e4b5" }, script: { replies: { "workspaces.commit": commitReply } } },
    { fn: "committed", case: "a message, the files and a thread, in the order sent", args: { workspaceId: "ws_1", message: "m", files: ["x"], threadId: "t-1" }, script: { replies: { "workspaces.commit": commitReply } } },
    { fn: "committed", case: "an empty message is a message, no draft is asked", args: { workspaceId: "ws_1", message: "", files: [] }, script: { replies: { "workspaces.commit": commitReply } } },
    { fn: "committed", case: "a multi-line message is sent whole", args: { workspaceId: "ws_1", message: "Fix\n\nBecause it broke.\n", files: [] }, script: { replies: { "workspaces.commit": commitReply } } },
    { fn: "committed", case: "no message, the draft is said and used", args: { workspaceId: "ws_1", files: [] }, script: { replies: { "workspaces.commitDraft": draft("Fix the  thing \u{1F9EA}\n\nBecause it broke."), "workspaces.commit": commitReply } } },
    { fn: "committed", case: "no message, the files and a thread go to the draft too", args: { workspaceId: "ws_1", files: ["a.ts"], threadId: "t-1" }, script: { replies: { "workspaces.commitDraft": draft("Drafted"), "workspaces.commit": commitReply } } },
    { fn: "committed", case: "no message, an empty draft is still a draft", args: { workspaceId: "ws_1", files: [] }, script: { replies: { "workspaces.commitDraft": draft(""), "workspaces.commit": commitReply } } },
    { fn: "committed", case: "no message, a draft with a note, the note is not said", args: { workspaceId: "ws_1", files: [] }, script: { replies: { "workspaces.commitDraft": draft("Drafted", "drafted by claude"), "workspaces.commit": commitReply } } },
    { fn: "committed", case: "no message, no draft, with the note", args: { workspaceId: "ws_1", files: [] }, script: { replies: { "workspaces.commitDraft": draft(null, "No agent here drafts commit messages; write it yourself.") } } },
    { fn: "committed", case: "no message, no draft, no note", args: { workspaceId: "ws_1", files: [] }, script: { replies: { "workspaces.commitDraft": draft(null) } } },
    { fn: "committed", case: "no message, no draft, an empty note", args: { workspaceId: "ws_1", files: [] }, script: { replies: { "workspaces.commitDraft": draft(null, "") } } },
    { fn: "committed", case: "the draft refused by the host", args: { workspaceId: "ws_1", files: [] }, script: { replies: { "workspaces.commitDraft": err("the agent is working", "usage") } } },
    { fn: "committed", case: "the commit refused by a hook", args: { workspaceId: "ws_1", message: "m", files: [] }, script: { replies: { "workspaces.commit": err("a hook said no: lint failed", "usage") } } },
    { fn: "committed", case: "the commit refused after a draft was said", args: { workspaceId: "ws_1", files: [] }, script: { replies: { "workspaces.commitDraft": draft("Drafted"), "workspaces.commit": err("nothing to commit", "usage") } } },
    { fn: "committed", case: "the reply's unknown keys are dropped", args: { workspaceId: "ws_1", message: "m", files: [] }, script: { replies: { "workspaces.commit": JSON.stringify({ id: 1, ok: true, deletions: 1, extra: 1, insertions: 3, filesChanged: 1, subject: "m", oid: "abc" }) } } },
    { fn: "committed", case: "the host closes during the commit", args: { workspaceId: "ws_1", message: "m", files: [] }, script: { closes: { "workspaces.commit": 1006 } } },
  ]),
);

// ---- askedToFix, fixLine
const fixed = (o: Obj): string => frame(o);
add(
  ...both([
    { fn: "askedToFix", case: "a failed check", args: { workspaceId: "ws_1", check: "ci / test", threadId: "t-1" }, script: { replies: { "workspaces.fix": fixed({ outcome: "steered", threadId: "t-1", check: "ci / test", base: "main", agent: "codex" }) } } },
    { fn: "askedToFix", case: "the conflicts, no check", args: { workspaceId: "ws_1", threadId: "t-1" }, script: { replies: { "workspaces.fix": fixed({ outcome: "started", threadId: "t-2", base: "main", agent: "claude" }) } } },
    { fn: "askedToFix", case: "a child to merge", args: { workspaceId: "ws_1", child: { workspaceId: "ws_4", threadId: "t-4" }, threadId: "t-1" }, script: { replies: { "workspaces.fix": fixed({ outcome: "started", threadId: "t-1", child: "ws_4", base: "tree/lead", agent: "claude" }) } } },
    { fn: "askedToFix", case: "a check and a child, in the order sent", args: { workspaceId: "ws_1", check: "lint", child: { workspaceId: "ws_4", threadId: "t-4" }, threadId: "t-1" }, script: { replies: { "workspaces.fix": fixed({ outcome: "queued", threadId: "t-1", check: "lint", child: "ws_4", base: "main", agent: "claude" }) } } },
    { fn: "askedToFix", case: "an empty check is sent", args: { workspaceId: "ws_1", check: "", threadId: "t-1" }, script: { replies: { "workspaces.fix": fixed({ outcome: "queued", threadId: "t-1", base: "main", agent: "claude" }) } } },
    { fn: "askedToFix", case: "held by its computer's threads", args: { workspaceId: "ws_1", check: "ci", threadId: "t-6" }, script: { replies: { "workspaces.fix": fixed({ outcome: "held", threadId: "t-6", check: "ci", base: "main", agent: "claude", capped: { placeId: "p_hetzner", place: "hetzner", running: 2, atOnce: 1 } }) } } },
    { fn: "askedToFix", case: "updated clean", args: { workspaceId: "ws_1", threadId: "t-1" }, script: { replies: { "workspaces.fix": fixed({ outcome: "updated", base: "main" }) } } },
    { fn: "askedToFix", case: "an unknown key is dropped", args: { workspaceId: "ws_1", threadId: "t-1" }, script: { replies: { "workspaces.fix": fixed({ extra: 1, agent: "claude", base: "main", threadId: "t-1", outcome: "started" }) } } },
    { fn: "askedToFix", case: "a sent answer naming no agent is another version", args: { workspaceId: "ws_1", threadId: "t-1" }, script: { replies: { "workspaces.fix": fixed({ outcome: "started", threadId: "t-5", base: "main" }) } } },
    { fn: "askedToFix", case: "an outcome this build does not know is another version", args: { workspaceId: "ws_1", threadId: "t-1" }, script: { replies: { "workspaces.fix": fixed({ outcome: "paused", threadId: "t-5", base: "main", agent: "claude" }) } } },
    { fn: "askedToFix", case: "an answer with no outcome is another version", args: { workspaceId: "ws_1", threadId: "t-1" }, script: { replies: { "workspaces.fix": fixed({ base: "main" }) } } },
    { fn: "askedToFix", case: "refused by the host", args: { workspaceId: "ws_1", check: "lint", threadId: "t-1" }, script: { replies: { "workspaces.fix": err("lint has not failed on #4", "usage") } } },
  ]),
);
const CAP = { placeId: "p_hetzner", place: "hetzner", running: 2, atOnce: 1 };
const fixCases: [string, Obj, string | undefined][] = [
  ["updated", { outcome: "updated", base: "main" }, undefined],
  ["updated with a child named, which is ignored", { outcome: "updated", base: "main" }, "beta"],
  ["steered on a check", { outcome: "steered", threadId: "t-1", check: "ci / test", base: "main", agent: "codex" }, undefined],
  ["started on the conflicts", { outcome: "started", threadId: "t-1", base: "main", agent: "claude" }, undefined],
  ["queued on the conflicts", { outcome: "queued", threadId: "t-1", base: "develop", agent: "claude" }, undefined],
  ["a child, named by its label", { outcome: "started", threadId: "t-1", child: "ws_4", base: "tree/lead", agent: "claude" }, "Fix it"],
  ["a child with no label, named by its workspace id", { outcome: "started", threadId: "t-1", child: "ws_4", base: "tree/lead", agent: "claude" }, undefined],
  ["a child and a check, the child wins", { outcome: "started", threadId: "t-1", child: "ws_4", check: "lint", base: "main", agent: "claude" }, "Fix it"],
  ["an agent the catalog does not name", { outcome: "started", threadId: "t-1", base: "main", agent: "someone-else" }, undefined],
  ["held on a check, the wait under it", { outcome: "held", threadId: "t-1", check: "ci", base: "main", agent: "claude", capped: CAP }, undefined],
  ["held on the conflicts, a slot wait", { outcome: "held", threadId: "t-1", base: "main", agent: "codex", capped: { ...CAP, why: "slot" } }, undefined],
  ["held on a child, a minute wait", { outcome: "held", threadId: "t-1", child: "ws_4", base: "main", agent: "claude", capped: { ...CAP, why: "minute" } }, "Fix it"],
  ["held with no wait stated", { outcome: "held", threadId: "t-1", check: "ci", base: "main", agent: "claude" }, undefined],
  ["started with a wait stated, which is not said", { outcome: "started", threadId: "t-1", check: "ci", base: "main", agent: "claude", capped: CAP }, undefined],
  ["names with spaces and an emoji", { outcome: "steered", threadId: "t-1", check: "ci  / test \u{1F9EA}", base: "main  \u{1F9EA}", agent: "claude" }, undefined],
];
add(...both(fixCases.map(([name, asked, child]): Case => ({ fn: "fixLine", case: name, args: { thread: "thread 7f3a9c21", asked, ...(child !== undefined ? { child } : {}) } }))));

// ---- mergedIn
const mergeInReply = (o: Obj = {}): string => frame({ lead: "alpha", child: "beta", branch: "child/one", merged: true, commits: 3, conflicts: [], ...o });
const LEAD = { workspace: ws(), threadId: "t-lead" };
const KID = { workspaceId: "ws_4", threadId: "t-4" };
add(
  ...both([
    { fn: "mergedIn", case: "a running lead", args: { lead: LEAD, child: KID }, script: { replies: { "workspaces.wake": resolved(ws()), "workspaces.mergeIn": mergeInReply() } } },
    { fn: "mergedIn", case: "a napping lead is woken first, and the merge goes to the woken record's id", args: { lead: { workspace: ws({ phase: "napping" }), threadId: "t-lead" }, child: KID }, script: { replies: { "workspaces.wake": resolved(ws({ id: "ws_9" })), "workspaces.mergeIn": mergeInReply() } } },
    { fn: "mergedIn", case: "a pausing lead is woken", args: { lead: { workspace: ws({ phase: "pausing" }), threadId: "t-lead" }, child: KID }, script: { replies: { "workspaces.wake": resolved(ws()), "workspaces.mergeIn": mergeInReply({ commits: 1 }) } } },
    { fn: "mergedIn", case: "a lead with its disk gone says the line and merges", args: { lead: { workspace: ws({ phase: "napping" }), threadId: "t-lead" }, child: KID }, script: { replies: { "workspaces.wake": resolved(ws({ diskGone: "its disk was deleted" })), "workspaces.mergeIn": mergeInReply() } } },
    { fn: "mergedIn", case: "a released lead is woken, as any action but send is", args: { lead: { workspace: ws({ phase: "released" }), threadId: "t-lead" }, child: KID }, script: { replies: { "workspaces.wake": resolved(ws()), "workspaces.mergeIn": mergeInReply() } } },
    { fn: "mergedIn", case: "a gone lead is refused before any frame", args: { lead: { workspace: ws({ phase: "gone", gone: "the provider has no machine m-1" }), threadId: "t-lead" }, child: KID } },
    { fn: "mergedIn", case: "conflicts", args: { lead: LEAD, child: KID }, script: { replies: { "workspaces.wake": resolved(ws()), "workspaces.mergeIn": mergeInReply({ merged: false, commits: 0, conflicts: ["lead.txt", "src/a b .ts"] }) } } },
    { fn: "mergedIn", case: "a sub-thread's commits not pushed", args: { lead: LEAD, child: KID }, script: { replies: { "workspaces.wake": resolved(ws()), "workspaces.mergeIn": mergeInReply({ unpushed: 2 }) } } },
    { fn: "mergedIn", case: "nothing to take", args: { lead: LEAD, child: KID }, script: { replies: { "workspaces.wake": resolved(ws()), "workspaces.mergeIn": mergeInReply({ commits: 0 }) } } },
    { fn: "mergedIn", case: "the merge refused by the host", args: { lead: LEAD, child: KID }, script: { replies: { "workspaces.wake": resolved(ws()), "workspaces.mergeIn": err("the lead is working", "usage") } } },
    { fn: "mergedIn", case: "the wake refused by the host asks no merge", args: { lead: { workspace: ws({ phase: "napping" }), threadId: "t-lead" }, child: KID }, script: { replies: { "workspaces.wake": err("the provider has no capacity", "provider") } } },
  ]),
);

// ---- startedFrom, reviewStarted, reviewPosted
const startReply = (o: Obj = {}): string => frame({ workspace: ws({ from: { kind: "issue", repo: "o/r", number: 7, url: "https://github.com/o/r/issues/7", title: "It breaks" } }), threadId: "thread-7f3a9c21e4b5", sessionId: "s-1", ...o });
const URL = "https://github.com/o/r/issues/7";
add(
  ...both([
    { fn: "startedFrom", case: "a link alone", args: { o: { url: URL } }, script: { replies: { "workspaces.start": startReply() } } },
    { fn: "startedFrom", case: "every pick, in the order of the line", args: { o: { url: URL, project: "alpha", agent: "codex", model: "gpt-5", effort: "high", access: "full" } }, script: { replies: { "workspaces.start": startReply() } } },
    { fn: "startedFrom", case: "an absent pick is left out of the frame", args: { o: { url: URL, agent: "claude", effort: "low" } }, script: { replies: { "workspaces.start": startReply() } } },
    { fn: "startedFrom", case: "access ask", args: { o: { url: URL, access: "ask" } }, script: { replies: { "workspaces.start": startReply() } } },
    { fn: "startedFrom", case: "access auto-edit", args: { o: { url: URL, access: "auto-edit" } }, script: { replies: { "workspaces.start": startReply() } } },
    { fn: "startedFrom", case: "access plan", args: { o: { url: URL, access: "plan" } }, script: { replies: { "workspaces.start": startReply() } } },
    { fn: "startedFrom", case: "an access that is no word is refused before any frame", args: { o: { url: URL, access: "yolo" } } },
    { fn: "startedFrom", case: "an empty access is no word", args: { o: { url: URL, access: "" } } },
    { fn: "startedFrom", case: "an access in capitals is no word", args: { o: { url: URL, access: "FULL" } } },
    { fn: "startedFrom", case: "empty picks are sent", args: { o: { url: URL, project: "", agent: "" } }, script: { replies: { "workspaces.start": startReply() } } },
    { fn: "startedFrom", case: "an answer with no origin", args: { o: { url: URL } }, script: { replies: { "workspaces.start": startReply({ workspace: ws() }) } } },
    { fn: "startedFrom", case: "only the three fields of the answer are kept", args: { o: { url: URL } }, script: { replies: { "workspaces.start": startReply({ extra: true }) } } },
    { fn: "startedFrom", case: "refused by the host", args: { o: { url: URL } }, script: { replies: { "workspaces.start": err("no project here is a checkout of o/r; add one with wsp add https://github.com/o/r", "usage") } } },
    { fn: "reviewStarted", case: "a link", args: { o: { url: "https://github.com/o/r/pull/7" } }, script: { replies: { "workspaces.review": startReply() } } },
    { fn: "reviewStarted", case: "a workspace's own pull request", args: { o: { workspaceId: "ws_1" } }, script: { replies: { "workspaces.review": startReply() } } },
    { fn: "reviewStarted", case: "every pick, in the order of the line", args: { o: { url: "https://github.com/o/r/pull/7", agent: "codex", model: "gpt-5", effort: "high" } }, script: { replies: { "workspaces.review": startReply() } } },
    { fn: "reviewStarted", case: "a workspace and the picks", args: { o: { workspaceId: "ws_1", agent: "claude" } }, script: { replies: { "workspaces.review": startReply() } } },
    { fn: "reviewStarted", case: "nothing named", args: { o: {} }, script: { replies: { "workspaces.review": err("a review needs a pull request's link, or a thread with a pull request", "usage") } } },
    { fn: "reviewStarted", case: "an agent with no read-only access", args: { o: { url: "https://github.com/o/r/pull/7", agent: "cursor" } }, script: { replies: { "workspaces.review": err("Cursor has no read-only access wsp can give a reviewer; review with Claude Code or Codex", "usage") } } },
    { fn: "reviewPosted", case: "no edits, posted as drafted", args: { workspaceId: "ws_1", edits: {}, threadId: "t-1" }, script: { replies: { "workspaces.reviewPost": frame({ url: "https://github.com/o/r/pull/7#pullrequestreview-9", number: 7, comments: 2, folded: 1 }) } } },
    { fn: "reviewPosted", case: "a verdict edited first", args: { workspaceId: "ws_1", edits: { verdict: "approve" }, threadId: "t-1" }, script: { replies: { "workspaces.reviewDraft": frame({ review: { note: "x", at: 1 } }), "workspaces.reviewPost": frame({ url: "https://github.com/o/r/pull/12#pullrequestreview-3", number: 12, comments: 1, folded: 0 }) } } },
    { fn: "reviewPosted", case: "a summary edited first", args: { workspaceId: "ws_1", edits: { summary: "Looks fine." }, threadId: "t-1" }, script: { replies: { "workspaces.reviewDraft": frame({ review: { note: "x", at: 1 } }), "workspaces.reviewPost": frame({ url: "u", number: 1, comments: 0, folded: 0 }) } } },
    { fn: "reviewPosted", case: "an empty summary is an edit", args: { workspaceId: "ws_1", edits: { summary: "" }, threadId: "t-1" }, script: { replies: { "workspaces.reviewDraft": frame({ review: { note: "x", at: 1 } }), "workspaces.reviewPost": frame({ url: "u", number: 1, comments: 0, folded: 0 }) } } },
    { fn: "reviewPosted", case: "the verdict and the summary, in the order given", args: { workspaceId: "ws_1", edits: { verdict: "request_changes", summary: "Two things." }, threadId: "t-1" }, script: { replies: { "workspaces.reviewDraft": frame({ review: { note: "x", at: 1 } }), "workspaces.reviewPost": frame({ url: "u", number: 3, comments: 4, folded: 5 }) } } },
    { fn: "reviewPosted", case: "no review yet", args: { workspaceId: "ws_1", edits: {}, threadId: "t-1" }, script: { replies: { "workspaces.reviewPost": err("alpha has no review to post yet", "usage") } } },
    { fn: "reviewPosted", case: "the draft edit refused asks no post", args: { workspaceId: "ws_1", edits: { verdict: "approve" }, threadId: "t-1" }, script: { replies: { "workspaces.reviewDraft": err("the pull request moved on since the review", "usage") } } },
    { fn: "reviewPosted", case: "an unknown key of the answer is dropped", args: { workspaceId: "ws_1", edits: {}, threadId: "t-1" }, script: { replies: { "workspaces.reviewPost": JSON.stringify({ id: 1, ok: true, folded: 1, x: 1, comments: 2, number: 7, url: "u" }) } } },
  ]),
);

// ---- startedLines, reviewLines, threadLabel
const FROM_ISSUE = { kind: "issue", repo: "o/r", number: 7, url: "https://github.com/o/r/issues/7", title: "It breaks" };
const startedViews: [string, Obj][] = [
  ["an issue", { workspace: ws({ from: FROM_ISSUE }), threadId: "thread-7f3a9c21e4b5", sessionId: "s-1" }],
  ["a pull request", { workspace: ws({ from: { ...FROM_ISSUE, kind: "pull_request", number: 12 } }), threadId: "thread-7f3a9c21e4b5", sessionId: "s-1" }],
  ["a review", { workspace: ws({ from: { ...FROM_ISSUE, kind: "review", number: 12 } }), threadId: "thread-7f3a9c21e4b5", sessionId: "s-1" }],
  ["no origin", { workspace: ws(), threadId: "thread-7f3a9c21e4b5", sessionId: "s-1" }],
  ["a short thread id", { workspace: ws({ from: FROM_ISSUE }), threadId: "t-1", sessionId: "s-1" }],
  ["a name with spaces and an emoji", { workspace: ws({ name: "alpha  \u{1F9EA}", from: FROM_ISSUE }), threadId: "thread-7f3a9c21e4b5", sessionId: "s-1" }],
];
for (const fn of ["startedLines", "reviewLines"]) {
  add(...both(startedViews.flatMap(([name, started]): Case[] => [{ fn, case: name, args: { started } }, { fn, case: `${name}, the caller's own thread line`, args: { started, thread: "echo" } }])));
}
add(
  ...both([
    { fn: "threadLabel", case: "a thread id", args: { thread: tv() } },
    { fn: "threadLabel", case: "a row with no thread id is named by its row id", args: { thread: tv({ threadId: undefined, id: "row-0123456789" }) } },
    { fn: "threadLabel", case: "a short id", args: { thread: tv({ id: "t-1", threadId: "t-1" }) } },
    { fn: "threadLabel", case: "an id of exactly 8", args: { thread: tv({ id: "12345678", threadId: "12345678" }) } },
    { fn: "threadLabel", case: "an id of 9", args: { thread: tv({ id: "123456789", threadId: "123456789" }) } },
    { fn: "threadLabel", case: "the thread id wins over the row id", args: { thread: tv({ id: "aaaaaaaaaaaa", threadId: "bbbbbbbbbbbb" }) } },
  ]),
);

// ---- threadCwd, folderOf, withShared, sharedOf, VERDICTS
const WT = { path: "/root/wt/feat", branch: "feat", made: true };
const cwdCases: [string, Obj][] = [
  ["no workspace at all", { cwd: "/root/a", noWorkspace: true }],
  ["no worktree", { cwd: "/root/a" }],
  ["no turn folder", { worktree: WT }],
  ["no turn folder under a gone worktree", { worktree: { ...WT, gone: true } }],
  ["a live worktree and a folder inside it", { cwd: "/root/wt/feat/pkg", worktree: WT }],
  ["a gone worktree and a folder inside it", { cwd: "/root/wt/feat/pkg", worktree: { ...WT, gone: true } }],
  ["a gone worktree and its own folder", { cwd: "/root/wt/feat", worktree: { ...WT, gone: true } }],
  ["a gone worktree and a sibling that shares the prefix", { cwd: "/root/wt/feature", worktree: { ...WT, gone: true } }],
  ["a gone worktree and a folder elsewhere", { cwd: "/root/alpha", worktree: { ...WT, gone: true } }],
  ["a gone worktree whose path ends in a slash", { cwd: "/root/wt/feat/pkg", worktree: { ...WT, path: "/root/wt/feat/", gone: true } }],
  ["a gone worktree whose path ends in a slash, the sibling", { cwd: "/root/wt/feature", worktree: { ...WT, path: "/root/wt/feat/", gone: true } }],
  ["an empty turn folder under a gone worktree", { cwd: "", worktree: { ...WT, gone: true } }],
];
add(
  ...both(cwdCases.map(([name, a]): Case => ({ fn: "threadCwd", case: name, args: a }))),
  ...both([
    { fn: "folderOf", case: "the workspace folder, under the home", args: { workspace: ws({ folder: "/root/alpha/pkg" }) }, env: { HOME: "/root" } },
    { fn: "folderOf", case: "the home itself", args: { workspace: ws({ folder: "/root" }) }, env: { HOME: "/root" } },
    { fn: "folderOf", case: "a sibling of the home sharing its prefix", args: { workspace: ws({ folder: "/rooty/alpha" }) }, env: { HOME: "/root" } },
    { fn: "folderOf", case: "no folder, the project path", args: { workspace: ws() }, env: { HOME: "/root" } },
    { fn: "folderOf", case: "outside the home", args: { workspace: ws({ folder: "/srv/alpha" }) }, env: { HOME: "/home/zingzy" } },
    { fn: "folderOf", case: "a home with a trailing slash", args: { workspace: ws({ folder: "/home/zingzy/alpha" }) }, env: { HOME: "/home/zingzy/" } },
    { fn: "folderOf", case: "an empty folder is a folder", args: { workspace: ws({ folder: "" }) }, env: { HOME: "/root" } },
    { fn: "withShared", case: "none share it", args: { line: "committed abc", shared: [] } },
    { fn: "withShared", case: "one shares it", args: { line: "committed abc", shared: ["thread-7f3a9c21e4b5"] } },
    { fn: "withShared", case: "two share it", args: { line: "committed abc", shared: ["thread-7f3a9c21e4b5", "thread-b0b0b0b0c1c1"] } },
    { fn: "withShared", case: "three share it, a short id and a long one", args: { line: "committed abc", shared: ["t-1", "thread-7f3a9c21e4b5", "12345678"] } },
    { fn: "withShared", case: "a multi-line line", args: { line: "a\nb", shared: ["t-1"] } },
    { fn: "sharedOf", case: "none", args: { shared: [] } },
    { fn: "sharedOf", case: "one", args: { shared: ["thread-7f3a9c21e4b5"] } },
    { fn: "sharedOf", case: "two, whole and in order", args: { shared: ["thread-b0b0b0b0c1c1", "thread-7f3a9c21e4b5"] } },
    { fn: "const_verdicts", case: "the table" },
  ]),
);

// ---- mergedPr, updateLine
add(
  ...both([
    { fn: "mergedPr", case: "no method, not on checks", args: { workspaceId: "ws_1", whenChecksPass: false, threadId: "t-1" }, script: { replies: { "workspaces.merge": frame({ number: 4, method: "squash", merged: true, autoArmed: false }) } } },
    { fn: "mergedPr", case: "a method", args: { workspaceId: "ws_1", method: "rebase", whenChecksPass: false, threadId: "t-1" }, script: { replies: { "workspaces.merge": frame({ number: 4, method: "rebase", merged: true, autoArmed: false }) } } },
    { fn: "mergedPr", case: "squash", args: { workspaceId: "ws_1", method: "squash", whenChecksPass: false, threadId: "t-1" }, script: { replies: { "workspaces.merge": frame({ number: 4, method: "squash", merged: true, autoArmed: false }) } } },
    { fn: "mergedPr", case: "merge", args: { workspaceId: "ws_1", method: "merge", whenChecksPass: false, threadId: "t-1" }, script: { replies: { "workspaces.merge": frame({ number: 4, method: "merge", merged: true, autoArmed: false }) } } },
    { fn: "mergedPr", case: "when checks pass, armed", args: { workspaceId: "ws_1", whenChecksPass: true, threadId: "t-1" }, script: { replies: { "workspaces.merge": frame({ number: 12, method: "merge", merged: false, autoArmed: true }) } } },
    { fn: "mergedPr", case: "a method and when checks pass, in the order sent", args: { workspaceId: "ws_1", method: "squash", whenChecksPass: true, threadId: "t-1" }, script: { replies: { "workspaces.merge": frame({ number: 12, method: "squash", merged: false, autoArmed: true }) } } },
    { fn: "mergedPr", case: "an unknown key of the answer is dropped", args: { workspaceId: "ws_1", whenChecksPass: false, threadId: "t-1" }, script: { replies: { "workspaces.merge": JSON.stringify({ id: 1, ok: true, autoArmed: false, merged: true, method: "merge", number: 1, extra: 2 }) } } },
    { fn: "mergedPr", case: "refused by the repository", args: { workspaceId: "ws_1", method: "rebase", whenChecksPass: false, threadId: "t-1" }, script: { replies: { "workspaces.merge": err("the repository does not allow rebase merges", "usage") } } },
    { fn: "updateLine", case: "merged, several commits", args: { workspace: "alpha", done: { base: "main", merged: true, commits: 3, conflicts: [] } } },
    { fn: "updateLine", case: "merged, one commit", args: { workspace: "alpha", done: { base: "main", merged: true, commits: 1, conflicts: [] } } },
    { fn: "updateLine", case: "already current", args: { workspace: "alpha", done: { base: "main", merged: true, commits: 0, conflicts: [] } } },
    { fn: "updateLine", case: "conflicts, two files", args: { workspace: "alpha", done: { base: "main", merged: false, commits: 0, conflicts: ["a.ts", "b .ts"] } } },
    { fn: "updateLine", case: "conflicts, one file", args: { workspace: "alpha", done: { base: "main", merged: false, commits: 0, conflicts: ["a.ts"] } } },
    { fn: "updateLine", case: "not merged and no files named", args: { workspace: "alpha", done: { base: "main", merged: false, commits: 0, conflicts: [] } } },
    { fn: "updateLine", case: "merged, with files listed which are not said", args: { workspace: "alpha", done: { base: "main", merged: true, commits: 2, conflicts: ["a.ts"] } } },
    { fn: "updateLine", case: "names with spaces and an emoji", args: { workspace: "alpha \u{1F9EA}", done: { base: "main  \u{1F9EA}", merged: true, commits: 2, conflicts: [] } } },
  ]),
);

// ---- napAfterDeadLaunch (the intents of verbs-turns.test.ts:743-807, on a scripted host)
const WOKEN = { workspace: ws(), woke: true };
const statusRow = (o: Obj = {}): Obj => ws({ phase: "running", machineState: "running", size: { cpu: 2, memMb: 4096 }, rateUsdPerHour: 0.018, reach: { state: "reachable" }, ...o });
const dead = (o: Obj = {}): Obj => sess("s_1", "t_1", { status: "failed", ...o });
const other = (o: Obj = {}): Obj => sess("s_2", "t_other", { status: "running", ...o });
const nap = (name: string, args: Obj, replies: Record<string, string> | undefined): Case => ({ fn: "napAfterDeadLaunch", case: name, args: { woken: WOKEN, ...args }, ...(replies !== undefined ? { script: { replies } } : {}) });
const NAPPED = { "workspaces.nap": resolved(ws({ phase: "napping" })) };
add(
  ...both([
    nap("a wake this call did not make owes nothing", { woken: { workspace: ws(), woke: false } }, undefined),
    nap("a turn the agent refused ran, so nothing is owed", { turn: { threadId: "t_1", refusal: "sign-in" } }, undefined),
    nap("naps it where nothing else is running there", {}, { "sessions.list": sessions(dead()), ...NAPPED }),
    nap("naps it for a turn whose thread is the only one", { turn: { threadId: "t_1" } }, { "sessions.list": sessions(dead()), ...NAPPED }),
    nap("naps it where no thread is listed at all", { turn: { threadId: "t_1" } }, { "sessions.list": sessions(), ...NAPPED }),
    nap("naps it where no turn is given and no thread is listed", {}, { "sessions.list": sessions(), ...NAPPED }),
    nap("naps it where the turn's thread is not listed and others have ended", { turn: { threadId: "t_gone" } }, { "sessions.list": sessions(dead(), sess("s_3", "t_3", { status: "completed" })), ...NAPPED }),
    nap("a thread whose turn did work is not a dead launch", { turn: { threadId: "t_1" } }, { "sessions.list": sessions(dead({ claudeSessionId: "cs-1" })) }),
    nap("a thread of the turn that is still running did work", { turn: { threadId: "t_1" } }, { "sessions.list": sessions(dead({ status: "running" })) }),
    nap("a thread that never reached the agent is a dead launch even beside a refused one", { turn: { threadId: "t_1" } }, { "sessions.list": sessions(dead(), sess("s_4", "t_4", { status: "failed", claudeSessionId: "cs-2", refusal: "sign-in" })), ...NAPPED }),
    nap("leaves it up where another thread is working, with no nap time to say", { turn: { threadId: "t_1" } }, { "sessions.list": sessions(dead(), other()), "status.list": frame({ statuses: [statusRow()] }) }),
    nap("leaves it up with a nap time already past, which reads as none left", { turn: { threadId: "t_1" } }, { "sessions.list": sessions(dead(), other()), "status.list": frame({ statuses: [statusRow({ idleAt: 1000 })] }) }),
    nap("leaves it up where the workspace has no status row", { turn: { threadId: "t_1" } }, { "sessions.list": sessions(dead(), other()), "status.list": frame({ statuses: [statusRow({ id: "ws_2" })] }) }),
    nap("leaves it up where the status list is empty", {}, { "sessions.list": sessions(dead(), other()), "status.list": frame({ statuses: [] }) }),
    nap("leaves it up where the status list is refused", {}, { "sessions.list": sessions(dead(), other()), "status.list": err("status is not readable", "auth") }),
    nap("another thread working counts with no turn given", {}, { "sessions.list": sessions(other()), "status.list": frame({ statuses: [] }) }),
    nap("two others working", { turn: { threadId: "t_1" } }, { "sessions.list": sessions(dead(), other(), sess("s_5", "t_5", { status: "running" })), "status.list": frame({ statuses: [statusRow()] }) }),
    nap("a turn's thread listed twice is one thread, and its other turn runs", { turn: { threadId: "t_1" } }, { "sessions.list": sessions(dead(), sess("s_6", "t_1", { status: "running" })) }),
    nap("answers the same where the host cannot be reached at all", {}, {}),
    nap("the thread read refused", {}, { "sessions.list": err("not allowed", "auth") }),
    nap("the nap refused", {}, { "sessions.list": sessions(dead()), "workspaces.nap": err("the provider refused the pause", "provider") }),
    { fn: "napAfterDeadLaunch", case: "the host closes during the thread read", args: { woken: WOKEN }, script: { closes: { "sessions.list": 1006 } } },
    { fn: "napAfterDeadLaunch", case: "the workspace is named in the thread read", args: { woken: { workspace: ws({ id: "ws_77", name: "gamma" }), woke: true } }, script: { replies: { "sessions.list": sessions(dead({ workspaceId: "ws_77" })), "workspaces.nap": resolved(ws({ id: "ws_77", phase: "napping" })) } } },
  ]),
);

// ---- withLine
const withLineCases: [string, Obj][] = [
  ["a refusal with a kind keeps the kind", { error: { message: "no thread zzz", kind: "not-found" }, line: "tell the person" }],
  ["a usage refusal", { error: { message: "bad flag", kind: "usage" }, line: "second line" }],
  ["a plain error has no kind", { error: { message: "it failed" }, line: "second line" }],
  ["no line returns the error as it was", { error: { message: "it failed", kind: "auth" } }],
  ["an empty line is a line", { error: { message: "it failed", kind: "provider" }, line: "" }],
  ["the fix is not carried", { error: { message: "bad flag", kind: "usage", fix: "Name one." }, line: "more" }],
  ["a problem list is not carried", { error: { message: "refused", kind: "usage", errors: [{ path: "a", message: "b" }] }, line: "more" }],
  ["a multi-line line", { error: { message: "first", kind: "usage" }, line: "a\nb" }],
  ["a message that is already two lines", { error: { message: "first\nsecond", kind: "usage" }, line: "third" }],
];
add(...both(withLineCases.map(([name, a]): Case => ({ fn: "withLine", case: name, args: a }))));

// ---- stop, stopLine
const T1 = "thread-7f3a9c21e4b5";
const rows = sessions(sess("s-1", T1, { prompt: "fix the build" }), sess("s-2", T1, { status: "running" }), sess("s-3", "thread-b0b0b0b0c1c1"));
const interrupted = (o: Obj): string => frame(o);
add(
  ...both([
    { fn: "stop", case: "stopped with the tree", args: { ref: T1 }, script: { replies: { "sessions.list": rows, "sessions.interrupt": interrupted({ outcome: "accepted", under: ["thread-1111aaaa2222", "thread-3333bbbb4444"] }) } } },
    { fn: "stop", case: "the latest turn's session id is the one stopped", args: { ref: "thread-7f" }, script: { replies: { "sessions.list": rows, "sessions.interrupt": interrupted({ outcome: "accepted" }) } } },
    { fn: "stop", case: "an empty under is dropped", args: { ref: T1 }, script: { replies: { "sessions.list": rows, "sessions.interrupt": interrupted({ outcome: "not-running", under: [] }) } } },
    { fn: "stop", case: "stopped with a process left running", args: { ref: T1 }, script: { replies: { "sessions.list": rows, "sessions.interrupt": interrupted({ outcome: "accepted", left: "a process this thread started on hetzner still runs after Stop (4242)" }) } } },
    { fn: "stop", case: "not found by the host", args: { ref: T1 }, script: { replies: { "sessions.list": rows, "sessions.interrupt": interrupted({ outcome: "not-found" }) } } },
    { fn: "stop", case: "refused with its reason", args: { ref: T1 }, script: { replies: { "sessions.list": rows, "sessions.interrupt": interrupted({ outcome: "refused", error: "Claude Code would not stop it" }) } } },
    { fn: "stop", case: "unsupported", args: { ref: T1 }, script: { replies: { "sessions.list": rows, "sessions.interrupt": interrupted({ outcome: "unsupported", error: "Stop is not available" }) } } },
    { fn: "stop", case: "one subagent stopped", args: { ref: T1, subagent: "task-9" }, script: { replies: { "sessions.list": rows, "sessions.interrupt": interrupted({ outcome: "accepted" }) } } },
    { fn: "stop", case: "one subagent, an empty name", args: { ref: T1, subagent: "" }, script: { replies: { "sessions.list": rows, "sessions.interrupt": interrupted({ outcome: "accepted" }) } } },
    { fn: "stop", case: "one subagent already over", args: { ref: T1, subagent: "task-9" }, script: { replies: { "sessions.list": rows, "sessions.interrupt": interrupted({ outcome: "not-running" }) } } },
    { fn: "stop", case: "every field of the answer, in the order kept", args: { ref: T1, subagent: "task-9" }, script: { replies: { "sessions.list": rows, "sessions.interrupt": interrupted({ left: "L", error: "E", under: ["t-1"], outcome: "refused" }) } } },
    { fn: "stop", case: "an unknown key of the answer is dropped", args: { ref: T1 }, script: { replies: { "sessions.list": rows, "sessions.interrupt": interrupted({ outcome: "accepted", extra: 1 }) } } },
    { fn: "stop", case: "no such thread asks no stop", args: { ref: "zzz" }, script: { replies: { "sessions.list": rows } } },
    { fn: "stop", case: "a prefix naming two asks no stop", args: { ref: "thread-" }, script: { replies: { "sessions.list": rows } } },
    { fn: "stop", case: "the stop refused by the host", args: { ref: T1 }, script: { replies: { "sessions.list": rows, "sessions.interrupt": err("not allowed", "auth") } } },
    { fn: "stop", case: "the host closes during the stop", args: { ref: T1 }, script: { replies: { "sessions.list": rows }, closes: { "sessions.interrupt": 1006 } } },
  ]),
);
const stopLines: [string, Obj][] = [
  ["accepted", { threadId: T1, outcome: "accepted" }],
  ["not running", { threadId: T1, outcome: "not-running" }],
  ["not found", { threadId: T1, outcome: "not-found" }],
  ["refused", { threadId: T1, outcome: "refused" }],
  ["unsupported", { threadId: T1, outcome: "unsupported" }],
  ["a subagent named", { threadId: T1, task: "task-9", outcome: "accepted" }],
  ["an error says it in place of the outcome", { threadId: T1, outcome: "refused", error: "Claude Code would not stop it" }],
  ["an error on a subagent", { threadId: T1, task: "task-9", outcome: "unsupported", error: "Stop is not available for Codex subagents" }],
  ["an error beats a tree and a left", { threadId: T1, outcome: "accepted", under: ["t-1"], left: "L", error: "E" }],
  ["one thread under it", { threadId: T1, outcome: "accepted", under: ["thread-1111aaaa2222"] }],
  ["two threads under it", { threadId: T1, outcome: "accepted", under: ["thread-1111aaaa2222", "thread-3333bbbb4444"] }],
  ["an empty under says nothing", { threadId: T1, outcome: "accepted", under: [] }],
  ["a short id under it", { threadId: T1, outcome: "accepted", under: ["t-1"] }],
  ["a process left on a computer", { threadId: T1, outcome: "accepted", left: "a process this thread started on hetzner still runs after Stop (4242)" }],
  ["a tree and a left", { threadId: T1, outcome: "accepted", under: ["thread-1111aaaa2222"], left: "one process runs" }],
  ["not running with a left", { threadId: T1, outcome: "not-running", left: "L" }],
  ["a subagent with a tree and a left", { threadId: T1, task: "task-9", outcome: "accepted", under: ["t-1", "t-2", "t-3"], left: "L" }],
];
add(...both(stopLines.map(([name, stopped]): Case => ({ fn: "stopLine", case: name, args: { stopped } }))));

// ---- forgetThread, threadForgotLine
add(
  ...both([
    { fn: "forgetThread", case: "a thread that never ran", args: { thread: tv({ ran: false }) }, script: { replies: { "sessions.forget": frame({}) } } },
    { fn: "forgetThread", case: "a check asked", args: { thread: tv({ ran: false }), check: true }, script: { replies: { "sessions.forget": frame({}) } } },
    { fn: "forgetThread", case: "a check not asked is not sent", args: { thread: tv({ ran: false }), check: false }, script: { replies: { "sessions.forget": frame({}) } } },
    { fn: "forgetThread", case: "the thread id, not the row id, is sent", args: { thread: tv({ id: "row-1", threadId: "thread-9999999999" }) }, script: { replies: { "sessions.forget": frame({}) } } },
    { fn: "forgetThread", case: "a row from before threads is refused before any frame", args: { thread: tv({ id: "row-1", threadId: undefined }) } },
    { fn: "forgetThread", case: "a row from before threads, a check asked", args: { thread: tv({ id: "row-1", threadId: undefined }), check: true } },
    { fn: "forgetThread", case: "refused once a turn ran", args: { thread: tv() }, script: { replies: { "sessions.forget": err("thread thread-7f3a9c21e4b5 ran a turn, so forgetting it would lose work; delete it instead", "usage") } } },
    { fn: "forgetThread", case: "the host closes during the forget", args: { thread: tv() }, script: { closes: { "sessions.forget": 1006 } } },
    { fn: "threadForgotLine", case: "a thread", args: { thread: tv() } },
    { fn: "threadForgotLine", case: "a row id with spaces", args: { thread: tv({ id: "row 1  \u{1F9EA}" }) } },
  ]),
);

// ---- settleThreads, restoreThreads, settledLines, restoredLines
const settleReply = (o: Obj = {}): string => frame({ settled: [{ threadId: T1, title: "fix the  build" }], left: [], ...o });
const TA = tv();
const TB = tv({ id: "thread-b0b0b0b0c1c1", threadId: "thread-b0b0b0b0c1c1" });
const TOLD = tv({ id: "row-1", threadId: undefined });
add(
  ...both([
    { fn: "settleThreads", case: "one thread", args: { threads: [TA], finished: false }, script: { replies: { "sessions.settle": settleReply() } } },
    { fn: "settleThreads", case: "two threads, in the order given", args: { threads: [TB, TA], finished: false }, script: { replies: { "sessions.settle": settleReply({ settled: [{ threadId: T1, title: "a" }, { threadId: "thread-b0b0b0b0c1c1", title: "b" }] }) } } },
    { fn: "settleThreads", case: "no threads named", args: { threads: [], finished: false }, script: { replies: { "sessions.settle": settleReply({ settled: [] }) } } },
    { fn: "settleThreads", case: "finished only", args: { threads: [TA], finished: true }, script: { replies: { "sessions.settle": settleReply({ settled: [] }) } } },
    { fn: "settleThreads", case: "the row id is sent, not the thread id", args: { threads: [TOLD], finished: false }, script: { replies: { "sessions.settle": settleReply() } } },
    { fn: "settleThreads", case: "processes stop", args: { threads: [TA], finished: false, processes: "stop" }, script: { replies: { "sessions.settle": settleReply() } } },
    { fn: "settleThreads", case: "processes keep", args: { threads: [TA], finished: false, processes: "keep" }, script: { replies: { "sessions.settle": settleReply() } } },
    { fn: "settleThreads", case: "processes ask", args: { threads: [TA], finished: true, processes: "ask" }, script: { replies: { "sessions.settle": settleReply() } } },
    { fn: "settleThreads", case: "one settled and one left", args: { threads: [TA, TB], finished: false }, script: { replies: { "sessions.settle": settleReply({ left: [{ threadId: "thread-b0b0b0b0c1c1", why: "still working: stop it first" }] }) } } },
    { fn: "settleThreads", case: "ended processes are kept", args: { threads: [TA], finished: false, processes: "stop" }, script: { replies: recordedSettle() } },
    { fn: "settleThreads", case: "an unknown key of the answer is dropped", args: { threads: [TA], finished: false }, script: { replies: { "sessions.settle": JSON.stringify({ id: 1, ok: true, extra: 1, left: [], settled: [{ title: "t", threadId: T1, more: 1 }] }) } } },
    { fn: "settleThreads", case: "refused while processes run", args: { threads: [TA], finished: false }, script: { replies: { "sessions.settle": err("a thread to settle still runs 2 processes: settle it again with processes stop to end them first, or processes keep to leave them running (--processes on the command line).", "usage") } } },
    { fn: "settleThreads", case: "refused for a thread under another lead", args: { threads: [TA], finished: false }, script: { replies: { "sessions.settle": err("thread thread-7 is not under this thread", "usage") } } },
    { fn: "restoreThreads", case: "one thread", args: { threads: [TA] }, script: { replies: { "sessions.restore": frame({ restored: [{ threadId: T1, title: "fix the  build" }] }) } } },
    { fn: "restoreThreads", case: "two threads, in the order given", args: { threads: [TB, TA] }, script: { replies: { "sessions.restore": frame({ restored: [{ threadId: "thread-b0b0b0b0c1c1", title: "b" }, { threadId: T1, title: "a" }] }) } } },
    { fn: "restoreThreads", case: "none named", args: { threads: [] }, script: { replies: { "sessions.restore": frame({ restored: [] }) } } },
    { fn: "restoreThreads", case: "the row id is sent", args: { threads: [TOLD] }, script: { replies: { "sessions.restore": frame({ restored: [] }) } } },
    { fn: "restoreThreads", case: "refused by the host", args: { threads: [TA] }, script: { replies: { "sessions.restore": err("thread thread-7 is not under this thread", "usage") } } },
  ]),
);
function recordedSettle(): Record<string, string> {
  const all = recorded("thread_settle", "processes stopped first");
  return { "sessions.settle": all["sessions.settle"]! };
}
const settledViews: [string, Obj][] = [
  ["nothing", { settled: [], left: [] }],
  ["one settled", { settled: [{ threadId: T1, title: "fix the build" }], left: [] }],
  ["two settled, in order", { settled: [{ threadId: T1, title: "a" }, { threadId: "thread-b0b0b0b0c1c1", title: "b" }], left: [] }],
  ["one left", { settled: [], left: [{ threadId: T1, why: "already settled" }] }],
  ["settled then left", { settled: [{ threadId: T1, title: "fix" }], left: [{ threadId: "thread-b0b0b0b0c1c1", why: "still working: stop it first" }] }],
  ["a title with a quote and an accent", { settled: [{ threadId: T1, title: "naïve \"résumé\"" }], left: [] }],
  ["an empty title", { settled: [{ threadId: T1, title: "" }], left: [] }],
  ["a short thread id", { settled: [{ threadId: "t-1", title: "x" }], left: [{ threadId: "t-2", why: "y" }] }],
  ["ended processes are not said", { settled: [{ threadId: T1, title: "x" }], left: [], ended: [{ pid: 1, ended: [], forced: [], left: [] }] }],
];
add(
  ...both(settledViews.map(([name, answer]): Case => ({ fn: "settledLines", case: name, args: { answer } }))),
  ...both([
    { fn: "restoredLines", case: "nothing", args: { answer: { restored: [] } } },
    { fn: "restoredLines", case: "one", args: { answer: { restored: [{ threadId: T1, title: "fix the build" }] } } },
    { fn: "restoredLines", case: "two, in order", args: { answer: { restored: [{ threadId: "thread-b0b0b0b0c1c1", title: "b" }, { threadId: T1, title: "a" }] } } },
    { fn: "restoredLines", case: "a short id and an empty title", args: { answer: { restored: [{ threadId: "t-1", title: "" }] } } },
  ]),
);

// ---- rename, renameLine
const renameReply = (o: Obj): string => frame(o);
add(
  ...both([
    { fn: "rename", case: "renamed", args: { thread: tv(), title: "the name he typed" }, script: { replies: { "sessions.rename": renameReply({ outcome: "renamed" }) } } },
    { fn: "rename", case: "the session id of the thread's latest turn is sent", args: { thread: tv({ sessionId: "s-9" }), title: "t" }, script: { replies: { "sessions.rename": renameReply({ outcome: "renamed" }) } } },
    { fn: "rename", case: "unsupported", args: { thread: tv({ harness: "codex" }), title: "t" }, script: { replies: { "sessions.rename": renameReply({ outcome: "unsupported" }) } } },
    { fn: "rename", case: "no session", args: { thread: tv(), title: "nowhere" }, script: { replies: { "sessions.rename": renameReply({ outcome: "no-session" }) } } },
    { fn: "rename", case: "failed with the machine's line", args: { thread: tv(), title: "locked" }, script: { replies: { "sessions.rename": renameReply({ outcome: "failed", error: "database is locked" }) } } },
    { fn: "rename", case: "failed with no line", args: { thread: tv(), title: "x" }, script: { replies: { "sessions.rename": renameReply({ outcome: "failed" }) } } },
    { fn: "rename", case: "not found by the host", args: { thread: tv(), title: "x" }, script: { replies: { "sessions.rename": renameReply({ outcome: "not-found" }) } } },
    { fn: "rename", case: "an error beside an outcome that is not failed is still kept", args: { thread: tv(), title: "x" }, script: { replies: { "sessions.rename": renameReply({ outcome: "renamed", error: "odd" }) } } },
    { fn: "rename", case: "an empty title is sent", args: { thread: tv(), title: "" }, script: { replies: { "sessions.rename": renameReply({ outcome: "renamed" }) } } },
    { fn: "rename", case: "a title with a newline and an emoji is sent as given", args: { thread: tv(), title: "a\nb \u{1F9EA}" }, script: { replies: { "sessions.rename": renameReply({ outcome: "renamed" }) } } },
    { fn: "rename", case: "a row from before threads is named by its row id", args: { thread: tv({ id: "row-1", threadId: undefined }), title: "x" }, script: { replies: { "sessions.rename": renameReply({ outcome: "renamed" }) } } },
    { fn: "rename", case: "an unknown key of the answer is dropped", args: { thread: tv(), title: "x" }, script: { replies: { "sessions.rename": JSON.stringify({ id: 1, ok: true, extra: 1, outcome: "renamed" }) } } },
    { fn: "rename", case: "refused by the host", args: { thread: tv(), title: "x" }, script: { replies: { "sessions.rename": err("not allowed", "auth") } } },
    { fn: "rename", case: "the host closes during the rename", args: { thread: tv(), title: "x" }, script: { closes: { "sessions.rename": 1006 } } },
  ]),
);
const renameLines: Obj[] = [];
for (const harness of ["claude", "codex", "someone-else"]) {
  for (const outcome of ["renamed", "unsupported", "no-session", "failed", "not-found"]) renameLines.push({ threadId: T1, title: "the name", harness, outcome });
}
renameLines.push({ threadId: T1, title: "the name", harness: "claude", outcome: "failed", error: "EACCES: the store is read-only" });
renameLines.push({ threadId: T1, title: "the name", harness: "claude", outcome: "failed", error: "" });
renameLines.push({ threadId: T1, title: "the name", harness: "claude", outcome: "renamed", error: "ignored" });
renameLines.push({ threadId: T1, title: "the name", harness: "claude", outcome: "not-found", error: "ignored" });
renameLines.push({ threadId: "t-1", title: "", harness: "codex", outcome: "renamed" });
renameLines.push({ threadId: T1, title: "naïve \"résumé\"  \u{1F9EA}", harness: "claude", outcome: "renamed" });
add(...both(renameLines.map((renamed): Case => ({ fn: "renameLine", case: `${renamed["harness"]} ${renamed["outcome"]}${renamed["error"] !== undefined ? ` with error ${JSON.stringify(renamed["error"])}` : ""}${renamed["title"] === "" ? " and no title" : renamed["title"] !== "the name" ? " with a hard title" : ""}`, args: { renamed } }))));

// ---- the run table
const W = (mod: Obj): any => mod["workspaces"];
const nul = async (p: unknown): Promise<unknown> => (await p) ?? null;
const lineOf = (sinks: { said: string[] }) => (l: string): void => void sinks.said.push(l);
const errorFrom = (spec: Obj): Error => Object.assign(new Error(spec["message"] as string), ...["kind", "fix", "errors"].filter(k => spec[k] !== undefined).map(k => ({ [k]: spec[k] })));
const runs: Record<string, Run> = {
  renameWorkspace: ({ mod, client, args }) => W(mod).renameWorkspace(client, args["ref"], args["name"]),
  renamedWorkspaceLine: ({ mod, args }) => nul(W(mod).renamedWorkspaceLine(args["renamed"])),
  awake: ({ mod, client, args, sinks }) => W(mod).awake(client, args["workspace"], args["action"], lineOf(sinks)),
  noDraftLine: ({ mod, args }) => nul(W(mod).noDraftLine(args["note"])),
  const_no_draft_fix: ({ mod }) => Promise.resolve(W(mod).NO_DRAFT_FIX),
  committed: ({ mod, client, args, sinks }) => W(mod).committed(client, args["workspaceId"], args["message"], args["files"], lineOf(sinks), args["threadId"]),
  askedToFix: ({ mod, client, args }) => W(mod).askedToFix(client, args["workspaceId"], args["check"], args["child"], args["threadId"]),
  fixLine: ({ mod, args }) => nul(W(mod).fixLine(args["thread"], args["asked"], args["child"])),
  mergedIn: ({ mod, client, args, sinks }) => W(mod).mergedIn(client, args["lead"], args["child"], lineOf(sinks)),
  startedFrom: ({ mod, client, args }) => W(mod).startedFrom(client, args["o"]),
  reviewStarted: ({ mod, client, args }) => W(mod).reviewStarted(client, args["o"]),
  reviewPosted: ({ mod, client, args }) => W(mod).reviewPosted(client, args["workspaceId"], args["edits"], args["threadId"]),
  startedLines: ({ mod, args }) => nul(W(mod).startedLines(args["started"], args["thread"] === "echo" ? (id: string) => `opened ${id}` : undefined)),
  reviewLines: ({ mod, args }) => nul(W(mod).reviewLines(args["started"], args["thread"] === "echo" ? (id: string) => `opened ${id}` : undefined)),
  threadLabel: ({ mod, args }) => nul(W(mod).threadLabel(args["thread"])),
  threadCwd: ({ mod, args }) => nul(W(mod).threadCwd({ ...(args["cwd"] !== undefined ? { cwd: args["cwd"] } : {}) }, args["noWorkspace"] === true ? undefined : { ...(args["worktree"] !== undefined ? { worktree: args["worktree"] } : {}) })),
  folderOf: async ({ mod, args, c }) => {
    const was = process.env["HOME"];
    process.env["HOME"] = c.env?.["HOME"] ?? "";
    try {
      return W(mod).folderOf(args["workspace"]);
    } finally {
      if (was === undefined) delete process.env["HOME"];
      else process.env["HOME"] = was;
    }
  },
  withShared: ({ mod, args }) => nul(W(mod).withShared(args["line"], args["shared"])),
  sharedOf: ({ mod, args }) => nul(W(mod).sharedOf(args["shared"])),
  const_verdicts: ({ mod }) => Promise.resolve(W(mod).VERDICTS),
  mergedPr: ({ mod, client, args }) => W(mod).mergedPr(client, args["workspaceId"], args["method"], args["whenChecksPass"], args["threadId"]),
  updateLine: ({ mod, args }) => nul(W(mod).updateLine(args["workspace"], args["done"])),
  napAfterDeadLaunch: ({ mod, client, args }) => nul(W(mod).napAfterDeadLaunch(client, args["woken"], args["turn"] === undefined ? undefined : { threadId: args["turn"]["threadId"], ...(args["turn"]["refusal"] !== undefined ? { result: { refusal: args["turn"]["refusal"] } } : {}) })),
  withLine: ({ mod, args }) => nul(W(mod).withLine(errorFrom(args["error"]), args["line"])),
  stop: ({ mod, client, args }) => W(mod).stop(client, args["ref"], args["subagent"]),
  stopLine: ({ mod, args }) => nul(W(mod).stopLine(args["stopped"])),
  forgetThread: ({ mod, client, args }) => nul(W(mod).forgetThread(client, args["thread"], args["check"] === undefined ? {} : { check: args["check"] })),
  threadForgotLine: ({ mod, args }) => nul(W(mod).threadForgotLine(args["thread"])),
  settleThreads: ({ mod, client, args }) => W(mod).settleThreads(client, args["threads"], args["finished"], args["processes"]),
  restoreThreads: ({ mod, client, args }) => W(mod).restoreThreads(client, args["threads"]),
  settledLines: ({ mod, args }) => nul(W(mod).settledLines(args["answer"])),
  restoredLines: ({ mod, args }) => nul(W(mod).restoredLines(args["answer"])),
  rename: ({ mod, client, args }) => W(mod).rename(client, args["thread"], args["title"]),
  renameLine: ({ mod, args }) => nul(W(mod).renameLine(args["renamed"])),
};

it("ws_life", async () => {
  await record("ws_life", cases, runs);
});
