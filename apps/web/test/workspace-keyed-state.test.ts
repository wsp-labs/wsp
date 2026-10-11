// SPDX-License-Identifier: AGPL-3.0-only
// Every thread of a project folder shares one workspace record, so state the app keeps under a workspace id is shared
// by every thread there: a draft, a file, a refusal or a running turn kept that way shows in the sibling threads. A map
// indexed by a workspace id, a Map or Set read at one, and a name that says it is kept by workspace, is either per
// project or per machine by what it holds, named below with why, or it goes by the thread's key. A new one fails here
// until it says which.
import { readdirSync, readFileSync, statSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { type Allowed, type Found, holdTo } from "../../../packages/protocol/test/allowed.js";

const SRC = resolve(dirname(fileURLToPath(import.meta.url)), "..", "src");

function sources(dir: string): string[] {
  return readdirSync(dir).flatMap(name => {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) return sources(path);
    return /\.tsx?$/.test(name) && !/\.test\.tsx?$/.test(name) ? [path] : [];
  });
}

/** The host's own records of a workspace, which are per workspace by what they are: any reader may index them, and a
 * reader of one thread filters the rows to that thread. */
const RECORDS = new Set(["sessions", "statuses", "harnessesByWorkspace", "costs", "spending"]);

/** A map indexed at a workspace id, by its name, or a name that says it is kept by workspace; once per file. */
const INDEXED = /([A-Za-z_$][\w$]*)\??\.?\[(?:(?:[a-z]\w*\.)?(?:workspaceId|wsId)|workspace\.id)\]/g;
const NAMED = /\b[A-Za-z_]+ByWorkspace(?:Id)?\b/g;
/** A Map or Set read or written at a workspace id, by its name. */
const MAPPED = /([A-Za-z_$][\w$]*)\??\.(?:get|set|has|delete)\(\s*(?:(?:[a-z]\w*\.)?(?:workspaceId|wsId)|workspace\.id)\b/g;

function scan(): Found[] {
  const found: Found[] = [];
  for (const file of sources(SRC)) {
    const text = readFileSync(file, "utf8");
    const names = new Set([...text.matchAll(INDEXED)].map(m => `${m[1]}[workspaceId]`));
    for (const m of text.matchAll(NAMED)) names.add(m[0]);
    for (const m of text.matchAll(MAPPED)) names.add(`${m[1]}.get(workspaceId)`);
    for (const name of names) if (!RECORDS.has(name.replace("[workspaceId]", ""))) found.push({ text: name, where: relative(SRC, file) });
  }
  return found;
}

/** Each counted in files. */
const ALLOWED: readonly Allowed[] = [
  { text: "access[workspaceId]", count: 1, why: "the access default the next new thread in the project opens at; a thread that ran keeps its own" },
  { text: "broughtBack[workspaceId]", count: 1, why: "a cloud workspace brought home, which is the whole record" },
  { text: "byWorkspace[workspaceId]", count: 1, why: "the side question, which closes when the composer leaves the thread it was asked from" },
  {
    text: "byWorkspaceId[workspaceId]",
    count: 6,
    why: "composer picks (stamped per thread through pickedOn), the context meter (rewritten on a switch), browser tabs and guests (the pane that shows them is the thread's), the files root, and the bottom terminal drawer, which its own ticket moves to the thread",
  },
  { text: "current[workspaceId]", count: 1, why: "the last machine line said for a workspace's machine" },
  { text: "drafts[workspaceId]", count: 4, why: "a fork's draft, one for the folder's new thread page the fork opens, and a project home's draft under its home key" },
  { text: "kept[workspaceId]", count: 1, why: "reads the right pane an earlier build kept per workspace, once, onto the thread open last" },
  { text: "launches[workspaceId]", count: 1, why: "sends in flight before their thread exists, each held by its own request id" },
  { text: "memory[workspaceId]", count: 1, why: "the machine's memory reading" },
  { text: "out[workspaceId]", count: 1, why: "rows grouped by the workspace they came from" },
  { text: "pendingByWorkspaceId", count: 2, why: "a file the Changes pane is asked to reveal, a pane of the folder's checkout" },
  { text: "pendingByWorkspaceId[workspaceId]", count: 1, why: "a file the Changes pane is asked to reveal, a pane of the folder's checkout" },
  { text: "pickedOn[workspaceId]", count: 3, why: "which thread each composer pick was stamped on, the thread key inside" },
  { text: "picks[workspaceId]", count: 1, why: "reads preferences an old build kept in this browser, once" },
  { text: "reviews[workspaceId]", count: 1, why: "the pull request review draft, one per branch" },
  { text: "scopeByWorkspaceId", count: 2, why: "the Changes pane's scope over the folder's checkout" },
  { text: "scopeByWorkspaceId[workspaceId]", count: 1, why: "the Changes pane's scope over the folder's checkout" },
  { text: "shown[workspaceId]", count: 1, why: "which thread's composer is on screen for the workspace, so words from a pane reach that thread" },
  { text: "TabsByWorkspace", count: 1, why: "browser tabs, which move to the thread with the right pane" },
  { text: "tabs[workspaceId]", count: 1, why: "browser guests, which move to the thread with the right pane" },
  { text: "unclaimed[workspaceId]", count: 1, why: "a draft an earlier build kept per workspace, handed to the first thread opened there" },
  { text: "viewed[workspaceId]", count: 1, why: "viewed marks on the folder's changed files" },
  { text: "zoom[workspaceId]", count: 1, why: "the terminal's text size for the project" },
  { text: "byId.get(workspaceId)", count: 1, why: "projects by id, read for a switcher card" },
  { text: "forks.get(workspaceId)", count: 1, why: "the workspaces a thread forked out, which draw under that thread" },
  { text: "held.get(workspaceId)", count: 1, why: "how many surfaces read a folder's daemon, which keeps its link dialled" },
  { text: "hellos.get(workspaceId)", count: 1, why: "the daemon's hello for a workspace's machine" },
  { text: "ids.get(workspaceId)", count: 1, why: "the workspaces whose checkout this socket already asked for" },
  { text: "kept.get(workspaceId)", count: 1, why: "the workspaces that still exist, whose panes are kept" },
  { text: "pending.get(workspaceId)", count: 3, why: "a new-thread request for the workspace's view, which its latest view takes" },
  {
    text: "registry.get(workspaceId)",
    count: 5,
    why: "what a workspace's machine serves: browser models, file listings, readings, processes, and the terminals, which move to the thread with the bottom drawer",
  },
  { text: "rowsFrom.get(workspaceId)", count: 1, why: "the newest read of a workspace's rows, so an older reply does not replace it" },
  { text: "seen.get(workspaceId)", count: 1, why: "the newest transcript position seen for a workspace" },
  { text: "sessionsSaid.get(workspaceId)", count: 1, why: "a refused row list said once per workspace" },
  { text: "watching.get(workspaceId)", count: 1, why: "how many readers hold a workspace's machine readings" },
  { text: "wires.get(workspaceId)", count: 1, why: "the daemon wire of a workspace's machine" },
];

describe("state kept under a workspace id", () => {
  it("is per project or per machine and says why, or goes by the thread's key", () => {
    const { refused, over, stale } = holdTo(scan(), ALLOWED);
    expect(refused, "new state kept by workspace: key it by the thread (threadKey) or name it in ALLOWED with why it is per project").toEqual([]);
    expect(over, "more state kept by workspace than ALLOWED names").toEqual([]);
    expect(stale, "ALLOWED names state no longer kept by workspace: lower its count").toEqual([]);
  });

  it("catches a map indexed at a workspace id, a Map or Set read at one, and a name kept by workspace", () => {
    const line = "const draft = s.drafts[e.workspaceId] ?? none; const seen = useStore(st => st.seenByWorkspaceId);";
    expect([...line.matchAll(INDEXED)].map(m => m[1])).toEqual(["drafts"]);
    expect([...line.matchAll(NAMED)].map(m => m[0])).toEqual(["seenByWorkspaceId"]);
    const mapped = "registry.set(workspaceId, link); held?.delete(e.workspaceId); open.has(threadId);";
    expect([...mapped.matchAll(MAPPED)].map(m => m[1])).toEqual(["registry", "held"]);
  });
});
