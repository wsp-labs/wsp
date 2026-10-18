// SPDX-License-Identifier: AGPL-3.0-only
// For gpui.sh, against a host bench/fixture.sh started:
//   node prep.mjs data <fixture out.json> <id>      runs the state's data() and prints how our app opens on it, as JSON
//   node prep.mjs theme <fixture out.json> <theme>  sets the host's theme preference to paper's side or graphite's
//   node prep.mjs undo <fixture out.json> <id> <thread>  runs the state's cleanup()
import { spawnSync } from "node:child_process";
import { dirname, join } from "node:path";
import { hostOf, states } from "./host.mjs";

/** How our app reaches each state: the pane it opens with --screen, the capture state the panes read from
 * WSP_GPUI_SHOT (pr_pane::shot()), keys typed once it draws, its window in css px, seconds to let its reads land
 * before the shot (2 unless named), and what no flag of ours reaches. */
const OURS = {
  "W2-PRP-019": { keys: ["ctrl+alt+b"], needs: "nothing opens the right panel's launcher at start; ctrl+alt+b is pressed" },
  "W2-PRP-021": { screen: "changes", shot: "uncommitted,stacked" },
  "W2-PRP-023": { screen: "changes", shot: "uncommitted,stacked" },
  "W2-PRP-024": { screen: "changes", shot: "uncommitted,stacked" },
  "W2-PRP-025": { screen: "changes", shot: "uncommitted,stacked" },
  "W2-PRP-026": { screen: "changes", shot: "uncommitted,stacked" },
  "W2-PRP-027": { screen: "changes", shot: "uncommitted,split" },
  "W2-PRP-029": { screen: "changes", shot: "uncommitted,stacked", viewed: "src/keep.ts" },
  "W2-PRP-030": { screen: "changes", shot: "uncommitted,stacked,commit" },
  "W2-PRP-031": { screen: "changes", shot: "uncommitted,stacked,discard:src/added.ts" },
  "W2-PRP-032": { screen: "changes", shot: "uncommitted,stacked,edit:src/keep.ts" },
  "W2-PRP-033": { screen: "changes", shot: "uncommitted,stacked", size: [600, 800] },
  "W2-PRP-037": { screen: "files", shot: "search:NOTES" },
  "W2-PRP-039": { screen: "files:file:src/keep.ts", shot: "line:2" },
  "W2-PRP-040": { screen: "files:file:docs/NOTES.md" },
  "W2-PRP-041": { screen: "files:file:logo.bin" },
  "W2-PRP-042": { screen: "files:file:big.txt" },
  "W2-PRP-044": { screen: "pull-request", shot: "tab:conversation", wait: 12 },
  "W2-PRP-045": { screen: "pull-request", shot: "file:src/pane.rs", wait: 12 },
  "W2-PRP-046": { screen: "pull-request", shot: "review", wait: 12 },
  "W2-PRP-047-merged": { screen: "pull-request", shot: "tab:conversation", wait: 12 },
  "W2-PRP-047-closed": { screen: "pull-request", shot: "tab:conversation", wait: 12 },
  "W2-PRP-901": { screen: "changes", shot: "uncommitted,stacked", wait: 8 },
  "W2-PRP-043": { screen: "files", shot: "finder:kee", needs: "the finder opens from the Files pane, so the panel shows Files under it where today's shows the thread's panel" },
};

const [verb, outJson, arg, threadArg] = process.argv.slice(2);
if (outJson === undefined || arg === undefined || !["data", "theme", "undo"].includes(verb)) {
  console.error("usage: node prep.mjs data <fixture out.json> <id> | theme <fixture out.json> paper|graphite | undo <fixture out.json> <id> <thread>");
  process.exit(2);
}
const host = hostOf(outJson);
if (verb === "undo") {
  await (await states()).find(s => s.id === arg)?.cleanup?.(host, threadArg);
  process.exit(0);
}
if (verb === "theme") {
  if (!["paper", "graphite"].includes(arg)) throw new Error(`no theme ${arg}`);
  await host.pref("theme", arg === "paper" ? "light" : "dark");
  process.exit(0);
}
const state = (await states()).find(s => s.id === arg);
const ours = OURS[arg];
if (state === undefined || ours === undefined) throw new Error(`no state ${arg}`);
const data = state.data === undefined ? undefined : await state.data(host);
const thread = typeof data === "string" ? data : host.fixture.threads["short-1"];
if (ours.viewed !== undefined) {
  const folder = join(dirname(dirname(host.fixture.state)), "bench");
  const blob = spawnSync("git", ["hash-object", ours.viewed], { cwd: folder, env: { HOME: host.home, PATH: "/usr/bin:/bin" }, encoding: "utf8" }).stdout.trim();
  await host.request("workspaces.viewed", { workspaceId: await host.workspaceOf(thread), path: ours.viewed, blob });
}
console.log(JSON.stringify({ id: arg, thread, screen: ours.screen ?? null, shot: ours.shot ?? null, keys: ours.keys ?? [], size: ours.size ?? [1280, 800], wait: ours.wait ?? 2, needs: ours.needs ?? null }));
process.exit(0);
