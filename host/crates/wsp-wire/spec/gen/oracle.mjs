// oracle.mjs <out.json>: what the TS functions of views/ (and the top-level ones wsp-wire takes) answer for chosen
// inputs, edge cases first. The Rust rule tests read these as their expectations; nothing here is written by hand.
import M from "./bundle.mjs";
import fs from "node:fs";

const V = n => M[`views/${n}`];
const T = n => M[n];
const START = 1_760_000_000_000;
const row = (pid, ppid, comm, cmdline, over = {}) => ({ pid, ppid, user: "ada", state: "S", comm, cmdline, cpu: 1, rss: 1000, startedAt: START + pid, ...over });
const out = {};
const add = (fn, args, want) => (out[fn] ??= []).push({ args, want: want === undefined ? null : want });

// --- activity-names, activity
const names = [
  ["cargo", "cargo test -p wsp-wire"], ["pnpm", "pnpm dev"], ["pnpm", "pnpm --filter web dev"], ["pnpm", "pnpm dlx create-vite@5.1 app"],
  ["npx", "npx -y @modelcontextprotocol/server-github@0.6.2"], ["npx", "npx --package foo@1 bar"], ["uv", "/opt/homebrew/bin/uv tool uvx mcp-search-console"],
  ["uv", "uv tool run --from git+https://x/y.git y"], ["uvx", "uvx --python 3.12 ruff check"], ["npm", "npm exec -- tsc"], ["npm", "npm run build"],
  ["npm", "npm x https://example.com/pkg.tgz"], ["yarn", "yarn"], ["go", "go test ./..."], ["bunx", "bunx v1.2.3"],
  ["node", "node /lab/node_modules/.bin/vite"], ["node", "node /usr/local/lib/node_modules/@openai/codex/bin/codex.js app"], ["node", "node dist/index.js serve --port 3"],
  ["node", "node --inspect /a/b/cli.mjs"], ["node", "node"], ["python3", "python3 -m http.server"], ["python3", "/usr/bin/python3 /srv/feed.py"],
  ["python3.13", "/home/dev/.cache/uv/archive-v0/djbB/bin/python /home/dev/.cache/uv/archive-v0/djbB/bin/mcp-search-console"],
  ["2.1.296", "claude -p --input-format stream-json"], ["", "/usr/bin/vim notes.md"], ["", ""], ["bash", "bash runs/53af26c40316.sh"],
  ["deno", "deno run -A main.ts"], ["ruby", "ruby bin/rails server"], ["bun", "bun lib/index.ts"], ["node", "node /x/node_modules/.bin/index.js"],
  ["cargo", "cargo"], ["pnpm", "pnpm exec @scope/tool@2"], ["npm", "npm exec -c 'echo hi'"],
];
for (const [comm, cmdline] of names) {
  const p = row(1, 0, comm, cmdline);
  add("proc_name", [p], V("activity-names").procName(p));
  add("proc_bins", [p], V("activity").procBins(p));
}
const agents = [{ id: "claude", bin: "claude" }, { id: "cursor", bin: "cursor-agent" }, { id: "codex", bin: "codex" }];
for (const [comm, cmdline] of [["2.1.296", "claude -p"], ["node", "/usr/local/bin/cursor-agent --print"], ["node", "node /opt/homebrew/bin/codex exec"], ["zsh", "-zsh"], ["claude", ""]]) {
  const p = row(5, 1, comm, cmdline);
  add("agent_by_bin", [p, agents], V("activity-names").agentByBin(p, agents));
}
for (const [pid, facts] of [[60, { daemon: 60, host: 50 }], [50, { daemon: 60, host: 50 }], [60, { daemon: 60, agent: "Claude Code" }], [7, {}]]) {
  const p = row(pid, 1, "node", "node /x/vite.js");
  add("activity_name", [p, facts], V("activity-names").activityName(p, facts));
}
for (const c of ["bash runs/53af26c40316.sh", "bash /tmp/lab/runs/3d518f162d41.sh", "bash /tmp/runs/3d518f162d4.sh", "bash /tmp/runs/3D518F162D41.sh", "tail -n +1 -f /tmp/lab/runs/3d518f162d41.in", "tail -n +1 -f /tmp/notes.in", "bash /tmp/runs/3d518f162d41.sh --x", " bash /a/3d518f162d41.sh", "bash 3d518f162d41.sh"]) add("is_turn_plumbing", [c], V("activity").isTurnPlumbing(c));
for (const [at, cg] of [[undefined, "/wsp-threads/a"], ["/wsp-threads/a", "/wsp-threads/a"], ["/wsp-threads/a/b", "/wsp-threads/a"], ["/wsp-threads/ab", "/wsp-threads/a"], ["", ""]]) add("in_cgroup", [at ?? null, cg], T("thread-cgroup").inCgroup(at, cg));
const cgs = [{ cgroup: "/a", memory: 5 }, { cgroup: "/a", memory: 9 }, { cgroup: "/b", memory: 7 }];
for (const [list, cg] of [[cgs, "/a"], [cgs, "/c"], [undefined, "/a"], [cgs, undefined]]) add("cgroup_memory", [list ?? null, cg ?? null], T("thread-cgroup").cgroupMemory(list, cg));
for (const name of ["lab-box", "ü box"]) {
  for (const f of ["boxEndLine", "notJoinedActivityFix", "boxBehindActivityLine", "notReadLine"]) add(f, [name], V("activity")[f](name));
}
for (const pid of [0, 42, -1]) for (const f of ["endGoneSaid", "endGoneLine", "wspOwnLine", "notThreadsLine", "notStartedLine"]) add(f, [pid], V("activity")[f](pid));
add("endReusedLine", [200, row(200, 1, "vite", "vite"), START - 60_000], V("activity").endReusedLine(200, row(200, 1, "vite", "vite"), START - 60_000));
add("endReusedLine", [7, row(7, 1, "x", "x", { startedAt: 0 }), 1], V("activity").endReusedLine(7, row(7, 1, "x", "x", { startedAt: 0 }), 1));
for (const verb of ["settle", "delete"]) for (const theirs of [[], [400, 12, 400]]) add("processesLeftFix", [verb, [...new Set(theirs)]], V("activity").processesLeftFix(verb, new Set(theirs)));

// --- golden-image
const G = V("golden-image");
for (const [bin, p] of [[undefined, "/usr/bin"], ["/home/a/.npm/bin", "/usr/bin:/bin"], ["", "/x"]]) add("on_npm_bin", [bin ?? null, p], G.onNpmBin(bin, p));
for (const id of ["agents/claude", "agents/mcp/github", "agents/", "tools/brew/jq", "agents"]) add("agent_of_row", [id], G.agentOfRow({ id }) ?? null);
for (const id of ["tools/brew/jq", "tools/brew/hashicorp/tap/terraform", "tools/npm", "tools/npm/", "x"]) add("package_of", [id], G.packageOf({ id }));
for (const [m, p] of [["brew", "jq"], ["npm", "@scope/x"], ["", ""]]) add("tool_row_id", [m, p], G.toolRowId(m, p));
const digest = { ticks: [
  { id: "tools/brew/zz", pin: { tag: "1.0", sha256: "aa" } }, { id: "tools/brew/Ab", pin: { tag: "2.0", latest: true }, road: "brew" },
  { id: "tools/npm/ab", pin: { tag: "3" } }, { id: "tools/npm/b", road: "npm" }, { id: "agents/claude", pin: { tag: "4", arch: "aarch64" } },
  { id: "tools/brew/é", pin: { tag: "5" } }, { id: "tools/brew/e", pin: { tag: "6" } }, { id: "tools/brew/_x", pin: { tag: "7" } }, { id: "tools/brew/10", pin: { tag: "8" } }, { id: "tools/brew/9", pin: { tag: "9" } },
], files: [] };
const versions = [{ version: 1, snapshotId: "s1", baseTemplate: "b", setupSha: "x", createdAt: "t", smoke: { cmd: "true", exitCode: 0 } }, { version: 2, snapshotId: "s2", templateId: "tpl", baseTemplate: "b", setupSha: "x", createdAt: "t", smoke: { cmd: "true", exitCode: 0 } }];
for (const m of [undefined, { head: 2, versions }, { head: 3, versions }, { head: 1, versions: [] }]) add("golden_head", [m ?? null], G.goldenHead(m) ?? null);
for (const v of versions) add("golden_image", [v], G.goldenImage(v));

// --- harness, harness-picks
const H = V("harness");
const HP = T("harness-picks");
const catalog = {
  harness: "claude", label: "Claude Code", version: "2.1.296", steers: true, source: "harness", models: [{ value: "opus", label: "Opus", isDefault: true, efforts: ["high", "low"], defaultEffort: "low" }, { value: "sonnet", label: "Sonnet", contextWindows: ["1m"] }, { value: "haiku", label: "Haiku", fast: true }],
  legacyModels: [{ value: "opus-old", label: "Opus old" }], hiddenModels: [{ value: "secret", label: "Secret" }],
  efforts: [{ value: "low", label: "Low" }, { value: "medium", label: "Medium" }, { value: "high", label: "High", isDefault: true, short: "Hi" }],
  permissionModes: [{ value: "default", label: "Ask" }, { value: "bypassPermissions", label: "Full", isDefault: true }],
  contextWindows: [{ value: "200k", label: "200k" }, { value: "1m", label: "1M" }], renames: false, images: true, mcpServers: true,
};
const parsedCatalog = H.HarnessCatalog.parse(catalog);
add("catalog", [], parsedCatalog);
for (const m of [null, "opus", "sonnet", "nope"]) {
  const model = HP.modelOf(parsedCatalog, m ?? undefined);
  add("model_of", [m], model);
  add("efforts_for", [m], HP.effortsFor(parsedCatalog, model));
  add("context_windows_for", [m], HP.contextWindowsFor(parsedCatalog, model));
}
add("every_model", [], HP.everyModel(parsedCatalog));
for (const list of ["efforts", "permissionModes", "models", "contextWindows"]) add("marked_default", [list], HP.markedDefault(parsedCatalog[list]) ?? null);
for (const v of [undefined, "high", "nope"]) add("listed_pick", [v ?? null], HP.listedPick(parsedCatalog.efforts, v) ?? null);
for (const [m, w] of [["opus[1m]", undefined], ["opus[1m]", "200k"], ["opus", undefined], ["opus[1m]x", undefined], ["[1m]", undefined]]) add("model_picks", [m, w ?? null], HP.modelPicks(m, w));
const rows = [
  { id: "a", workspaceId: "w", harness: "claude", status: "completed", model: "opus[1m]", effort: "high" },
  { id: "b", workspaceId: "w", harness: "claude", status: "completed", permissionMode: "default" },
  { id: "c", workspaceId: "w", harness: "claude", status: "completed", model: "sonnet", contextWindow: "200k" },
];
for (const rs of [rows, [], [rows[1]], [rows[2], rows[0]]]) add("recorded_picks", [rs], HP.recordedPicks(rs));
for (const [cat, ran, named] of [
  [undefined, { model: "opus", effort: "high", contextWindow: "1m" }, {}],
  [undefined, { model: "opus" }, { effort: "low" }],
  [parsedCatalog, { model: "opus", effort: "high" }, {}],
  [parsedCatalog, { model: "opus", effort: "medium" }, {}],
  [parsedCatalog, { model: "sonnet", contextWindow: "1m" }, { effort: "high" }],
  [parsedCatalog, { model: "gone", effort: "high" }, {}],
  [parsedCatalog, { model: "opus", effort: "high" }, { model: "sonnet" }],
  [parsedCatalog, { model: "sonnet", contextWindow: "1m" }, { contextWindow: "200k" }],
]) add("kept_picks", [cat === undefined ? null : "catalog", ran, named], HP.keptPicks(cat, ran, named));
const startCases = [[{}, true, undefined], [{}, false, undefined], [{ model: "sonnet" }, true, undefined], [{ model: "secret" }, true, undefined], [{ model: "nope" }, true, undefined], [{ effort: "max" }, true, undefined], [{ permissionMode: "plan" }, true, undefined], [{ fast: true }, true, undefined], [{ fast: true, model: "haiku" }, true, undefined], [{ fast: true }, false, "haiku"], [{ fast: false, model: "haiku" }, true, undefined], [{ effort: "high", model: "opus" }, true, undefined]];
for (const [picks, opens, runsOn] of startCases) {
  let want;
  try { want = { ok: H.startPicks(parsedCatalog, picks, opens, runsOn) }; } catch (e) { want = { err: { message: e.message, kind: e.kind } }; }
  add("start_picks", ["catalog", picks, opens, runsOn ?? null], want);
  let none;
  try { none = { ok: H.startPicks(undefined, picks, opens, runsOn) }; } catch (e) { none = { err: { message: e.message, kind: e.kind } }; }
  add("start_picks", [null, picks, opens, runsOn ?? null], none);
}
for (const [servers, takes] of [[undefined, undefined], [{}, undefined], [{ gh: { command: "gh", args: [] } }, undefined], [{ gh: { command: "gh", args: [] } }, true]]) add("mcp_servers_blocked", [servers ?? null, takes === true], H.mcpServersBlocked(servers, takes, "cursor"));
add("no_mcp_servers_line", ["cursor"], H.noMcpServersLine("cursor"));
for (const c of [null, "catalog", "table"]) {
  const cat = c === null ? null : c === "table" ? { ...parsedCatalog, source: "table" } : parsedCatalog;
  add("keeps_rename", [c], H.keepsRename(cat));
  add("reads_images", [c], H.readsImages(cat));
  add("takes_mcp_servers", [c], H.takesMcpServers(cat));
}
add("no_fast_line", ["Opus"], H.noFastLine("Opus"));

// --- shell-quote
const SQ = T("shell-quote");
for (const s of ["", "a", "it's", "''", "a b", "$HOME", "é", "\n"]) add("shell_quote", [s], SQ.shellQuote(s));
for (const ws of [[], [""], ["a", "b c"], ["--flag=1", "x@y:z", "~/x", "a*"], ["it's"]]) add("shell_line", [ws], SQ.shellLine(ws));
for (const [cwd, c] of [[undefined, "ls"], ["/a b", "ls"], ["", "ls"]]) add("in_folder", [cwd ?? null, c], SQ.inFolder(cwd, c));

// --- place
const P = V("place");
for (const s of ["MacBook Pro", "Mac mini", "iMac", "Mac Studio", "Mac Pro", "macbookair", "Mac\tmini", "Studio", "", "MacBook"]) add("mac_kind_of", [s], P.macKindOf(s) ?? null);
for (const id of ["signins/claude", "github", "signins/", "gh", "tools/x"]) add("sign_in_of_row", [id], P.signInOfRow(id) ?? null);
add("sign_in_row_id", ["codex"], P.signInRowId("codex"));
const prow = (id, kind) => ({ id, label: id, outcome: "installed", ...(kind ? { kind } : {}) });
for (const rs of [[], [prow("a", "server"), prow("b"), prow("c", "file"), prow("d", "server")]]) add("provision_counts", [rs], P.provisionCounts(rs));
const job = { addId: "a1", address: "box", startedAt: "t", state: "running", steps: [{ step: "connect", state: "done", ms: 3 }, { step: "check", state: "running" }] };
for (const e of [
  { step: "check", state: "done", ms: 5 }, { step: "root", state: "running", note: "n" }, { step: "join", state: "done", placeId: "p1" }, { step: "join", state: "done" },
  { step: "check", state: "failed", note: "no ssh" }, { step: "check", state: "failed" }, { step: "connect", state: "running", note: "again", placeId: "zz" },
]) {
  add("with_place_stage", [job, { type: "place.stage", addId: "a1", ...e }], P.withPlaceStage(job, e));
  add("with_place_stage", [{ ...job, said: "kept" }, { type: "place.stage", addId: "a1", ...e }], P.withPlaceStage({ ...job, said: "kept" }, e));
}
for (const holds of [{ forks: [], projects: [], unsaved: [] }, { forks: [{ name: "f", threads: 1 }], projects: [], unsaved: [], away: true }, { forks: [], projects: [], unsaved: [], away: true }, { forks: [], projects: [{ name: "p", threads: 0 }], unsaved: [], away: true }]) add("place_forgets_only", [holds], P.placeForgetsOnly(holds));

// --- session, session-events, sub-row, workspace
const S = V("session");
const sv = (over) => ({ id: "s", workspaceId: "w", harness: "claude", status: "completed", ...over });
add("thread_key_of", [sv({ threadId: "t" })], S.threadKeyOf(sv({ threadId: "t" })));
add("thread_key_of", [sv({})], S.threadKeyOf(sv({})));
for (const turns of [[], [sv({ status: "running" })], [sv({ claudeSessionId: "c" })], [sv({ claudeSessionId: "c", refusal: "sign-in" })], [sv({}), sv({ claudeSessionId: "c" })]]) {
  let ok = true;
  for (const t of turns) if (!S.SessionView.safeParse(t).success) ok = false;
  if (ok) add("thread_ran", [turns], S.threadRan(turns));
}
const SE = V("session-events");
for (const env of [{}, { WSP_HOST_URL: " http://x ", WSP_HOST_TOKEN: "t" }, { WSP_HOST_URL: "u", WSP_HOST_TOKEN: "  " }, { WSP_HOST_URL: "﻿u ", WSP_HOST_TOKEN: "t", WSP_HOST_KEY: " k " }, { WSP_HOST_URL: "u", WSP_HOST_TOKEN: "t", WSP_HOST_KEY: "" }]) {
  const named = Object.fromEntries(Object.entries(env).map(([k, v]) => [{ WSP_HOST_URL: T("env").HOST_URL_ENV, WSP_HOST_TOKEN: T("env").HOST_TOKEN_ENV, WSP_HOST_KEY: T("env").HOST_KEY_ENV }[k], v]));
  add("host_from_env", [named], SE.hostFromEnv(named) ?? null);
}
for (const v of [undefined, "", " t ", "t"]) { const env = v === undefined ? {} : { [T("env").TURN_TOKEN_ENV]: v }; add("turn_token_of", [env], SE.turnTokenOf(env) ?? null); }
for (const t of ["session.start", "thread.waited", "session.slate", "workspace.created", "session", ""]) add("is_session_event", [t], SE.isSessionEvent({ type: t }));
add("agent_starting_line", ["Claude Code", true], SE.agentStartingLine("Claude Code", true));
add("agent_starting_line", ["Codex", false], SE.agentStartingLine("Codex", false));
for (const d of ["/opt/wsp", "", "/a/"]) { add("wsp_package_in", [d], SE.wspPackageIn(d)); add("wsp_bin_in", [d], SE.wspBinIn(d)); }
const SR = V("sub-row");
for (const s of [undefined, { startedAt: 1, log: [] }, { startedAt: 1, log: [], readyAt: 2 }, { startedAt: 1, log: [], failed: { step: "machine", said: "x" } }]) add("sub_spawning", [s ?? null], SR.subSpawning(s));
const res = (over) => ({ branch: "b", pushedSha: null, commits: 0, unpushed: 0, changed: 0, readAt: "t", ...over });
for (const r of [res({}), res({ unpushed: 1 }), res({ changed: 2 }), res({ readFailed: "x" }), res({ readFailed: "" })]) add("sub_all_pushed", [r], SR.subAllPushed(r));
for (const [rel, st] of [[undefined, 5], [10, undefined], [10, 5], [10, 10], [10, 11], [0, -1]]) add("sub_turn_gone", [rel ?? null, st ?? null], SR.subTurnGone(rel === undefined ? undefined : { releasedAt: rel }, st));
const W = V("workspace");
for (const [held, patch] of [[undefined, {}], [{ maxMachines: 1, maxDepth: 1 }, { maxDepth: 4 }]]) add("agents_from", [held ?? null, patch], W.agentsFrom(held, patch));

// --- preferences
const PR = V("preferences");
for (const f of ["editorOpensHereLine", "sshIncludeLine", "sshCopyHereLine", "sshBehindLine"]) add(f, ["lab-box"], PR[f]("lab-box"));
add("serverIconsLeftLine", ["/x/icons", "EBUSY"], PR.serverIconsLeftLine("/x/icons", "EBUSY"));
for (const v of [undefined, "1", "0", "true", " 1", ""]) { const env = v === undefined ? {} : { [T("env").LABS_ENV]: v, [T("env").CLOUD_ENV]: v }; add("labs_from_env", [env], PR.labsFromEnv(env)); add("cloud_from_env", [env], PR.cloudFromEnv(env)); }
add("default_preferences_text", [], JSON.stringify(PR.DEFAULT_PREFERENCES));
for (const stored of [undefined, null, {}, { sidebarMode: "spaces", labs: true, projectIcon: { p: { hash: "a".repeat(64) } } }, { sidebarMode: "nope" }, "corrupt", [], { terminalZoom: { a: 2, b: 1 }, sidebarWidth: 300, editor: "zed" }, { unknownKey: 1, theme: "dark" }]) {
  let parsedOk = true;
  try { PR.preferencesFrom(stored); } catch { parsedOk = false; }
  if (parsedOk) add("preferences_from_text", [stored === undefined ? "absent" : stored], JSON.stringify(PR.preferencesFrom(stored)));
}
const base = PR.applyPreferencesPatch(PR.DEFAULT_PREFERENCES, { terminalZoom: { a: 1, b: 2 }, access: { a: "plan" }, sidebarWidth: 280, target: { workspace: "w" }, textSize: 14 });
for (const patch of [{}, { terminalZoom: { a: null } }, { terminalZoom: { a: null, c: 3 } }, { terminalZoom: { b: 5, a: 9 } }, { sidebarWidth: null }, { target: null }, { textSize: null, codeSize: 12 }, { agentDefaults: { claude: { model: "opus" } } }, { recipeLook: { r: { icon: "x" } } }, { threadsShut: { t: true } }, { threadsShut: { t: null } }, { sendWith: "mod-enter", planAlerts: false }]) {
  const ok = PR.PreferencesPatch.safeParse(patch);
  if (!ok.success) { add("patch_refused", [patch], ok.error.issues[0].code); continue; }
  add("apply_preferences_patch_text", [JSON.parse(JSON.stringify(base)), patch], JSON.stringify(PR.applyPreferencesPatch(base, ok.data)));
}
const twice = PR.applyPreferencesPatch(PR.applyPreferencesPatch(base, { terminalZoom: { a: null } }), { terminalZoom: { a: 4 } });
add("zoom_dropped_then_set_text", [], JSON.stringify(twice.terminalZoom));
const TD = T("thread-defaults");
for (const [kept, patch] of [[undefined, { model: "opus" }], [{ model: "opus", effort: "high" }, { model: null }], [{ model: "opus" }, { model: null }], [{ effort: "high" }, { model: "opus" }], [undefined, {}], [{ model: "opus" }, { effort: undefined }]]) add("patched_fields", [kept ?? null, patch], TD.patchedFields(kept, patch) ?? null);
const GP = T("general-prefs");
add("general_defaults", [], GP.GENERAL_DEFAULTS);
add("patched_general", [GP.GENERAL_DEFAULTS, { onQuit: "stop", askDelete: false }], GP.patchedGeneral(GP.GENERAL_DEFAULTS, { onQuit: "stop", askDelete: false }));


// --- placement, whole: every map in insertion order, and each pid's kind, need, End and reach, read in the procs' order
const A = V("activity");
const placeCase = (name, input) => {
  const listening = new Set(input.listening ?? []);
  const app = input.app ? new Map(input.app) : undefined;
  const placed = A.placeProcs({ procs: input.procs, daemon: input.daemon, host: input.host, threads: input.threads, agentBins: input.agentBins, listens: pid => listening.has(pid), ...(app ? { app } : {}) });
  const pids = placed.procs.map(p => p.pid);
  add("place_procs", [name, input], {
    procs: pids,
    whose: [...placed.whose].map(([k, w]) => [k, typeof w === "object" ? { thread: w.thread } : w]),
    agent: [...placed.agent], foldsInto: [...placed.foldsInto], rootOf: [...placed.rootOf],
    kinds: pids.map(pid => [pid, placed.kindOf(pid)]),
    needed: pids.map(pid => [pid, placed.needed(pid)]), endable: pids.map(pid => [pid, placed.endable(pid)]), reachOf: pids.map(pid => [pid, placed.reachOf(pid) ?? null]),
    kidsOf: pids.map(pid => [pid, placed.kidsOf(pid).map(k => k.pid)]),
  });
};
const LABT = { id: "lab-thread-1", roots: [100] };
const basic = [row(1, 0, "launchd", "/sbin/launchd"), row(100, 50, "bash", "bash runs/53af26c40316.sh", { pgid: 100 }), row(101, 100, "bash", "bash", { pgid: 100 }), row(102, 101, "2.1.296", "claude -p --input-format stream-json", { pgid: 100 }),
  row(200, 102, "node", "node /lab/node_modules/.bin/vite", { cpu: 9 }), row(201, 200, "esbuild", "esbuild --service"), row(202, 102, "node", "node /x/mcp/server.js"), row(203, 202, "zsh", "zsh -c ls"),
  row(150, 1, "tail", "tail -n +1 -f /tmp/runs/53af26c40316.in", { pgid: 100 }), row(160, 1, "node", "node zombie.js", { state: "Z", owner: { by: "fork", thread: "lab-thread-1" } }),
  row(400, 1, "node", "node detached.js", { owner: { by: "fork", thread: "lab-thread-1" } }), row(50, 1, "node", "node wsp host"), row(60, 1, "wsp-daemon", "wsp-daemon serve"), row(70, 1, "zsh", "-zsh"),
  row(80, 80, "init", "init"), row(90, 91, "a", "loop a"), row(91, 90, "b", "loop b"), row(95, 1, "vim", "vim x", { owner: { by: "coalition" } }), row(96, 1, "python3", "python3 /state/slates/lab-thread-1/feed.py"), row(97, 1, "python3", "python3 /state/slates/other/feed.py"),
  row(98, 1, "node", "node /x/vitest run", { pty: "pty_9" }), row(99, 1, "cargo", "cargo build --release")];
placeCase("a thread from its root, its pump, a zombie, wsp's own, a parent loop, the slate folder and the kinds by line", { procs: basic, daemon: 60, host: 50, threads: [LABT], agentBins: ["claude"], listening: [200] });
placeCase("the same with no thread at all", { procs: basic, daemon: 60, host: 50, threads: [], agentBins: ["claude"], listening: [] });
placeCase("a box thread named by its cgroup alone, its heads the roots", { procs: [row(500, 1, "bash", "bash /r/aaaaaaaaaaaa.sh", { cgroup: "/wsp-threads/t" }), row(501, 500, "claude", "claude -p", { cgroup: "/wsp-threads/t" }), row(502, 1, "node", "node srv.js", { cgroup: "/wsp-threads/t/x" }), row(503, 1, "node", "node other.js", { cgroup: "/wsp-threads/tt" }), row(60, 1, "wsp-daemon", "wsp-daemon serve")], daemon: 60, threads: [{ id: "t", roots: [], cgroup: "/wsp-threads/t" }], agentBins: ["claude"] });
placeCase("the app's own parts and a tab a thread's pane holds", { procs: [row(10, 1, "Electron", "/A/wsp"), row(11, 10, "Electron Helper (Renderer)", "x --type=renderer"), row(12, 10, "Electron Helper (Renderer)", "x --type=renderer"), row(13, 10, "Electron Helper (GPU)", "x --type=gpu"), row(60, 1, "wsp-daemon", "wsp-daemon serve")], daemon: 60, host: 99, threads: [{ id: "t1", roots: [], tabs: [11, 13] }, { id: "t2", roots: [], tabs: [11] }], agentBins: [], app: [[11, "tab"], [12, "tab"], [13, "app"], [10, "app"]] });
placeCase("a recorded agent pid wins over a name, and a named child of it joins the stretch", { procs: [row(700, 1, "bash", "bash /r/bbbbbbbbbbbb.sh"), row(701, 700, "node", "node /usr/local/bin/codex app"), row(702, 701, "codex", "/v/codex app-server"), row(703, 702, "node", "node tool.js")], daemon: 60, threads: [{ id: "c", roots: [700], agents: [701], bin: "codex" }], agentBins: ["codex"] });
placeCase("a root the daemon stamped as an agent stands for the agent until it shows", { procs: [row(800, 1, "bash", "bash /r/cccccccccccc.sh", { kind: "agent", owner: { by: "root", thread: "s" } }), row(801, 800, "sleep", "sleep 5")], daemon: 60, threads: [{ id: "s", roots: [] }], agentBins: ["claude"] });
placeCase("a mark by env alone reaches only through a stretch, a cgroup or a parent", { procs: [row(900, 1, "node", "node a.js", { owner: { by: "env", thread: "m" } }), row(901, 900, "node", "node b.js", { owner: { by: "env", thread: "m" } }), row(902, 1, "node", "node c.js", { owner: { by: "group", thread: "m" } }), row(903, 902, "node", "node d.js", { owner: { by: "env", thread: "m" } })], daemon: 60, threads: [{ id: "m", roots: [] }], agentBins: [] });

// --- activity_of, whole: thread order, totals, tree order and depth, ports, command fallback, cgroup memory
const actCase = (name, r) => add("activity_of", [name, { ...r, ports: r.ports ? Object.fromEntries(r.ports) : null }], JSON.parse(JSON.stringify(A.activityOf({ ...r, ports: r.ports ? new Map(r.ports) : undefined }))));
actCase("siblings busiest first, ties by pid, a missing parent a head, an empty command line the comm", { computer: "lab", at: START, procs: [row(100, 50, "bash", "bash runs/53af26c40316.sh"), row(101, 100, "claude", "claude -p"), row(110, 101, "a", "", { cpu: 2 }), row(111, 101, "b", "b", { cpu: 2 }), row(112, 101, "c", "c", { cpu: 5.5 }), row(113, 999, "d", "d", { owner: { by: "fork", thread: "t" } }), row(50, 1, "node", "node wsp host"), row(60, 1, "wsp-daemon", "wsp-daemon serve")], daemon: 60, host: 50, scopes: [{ threadId: "t", title: "T", workspaceId: "w", roots: [100] }, { threadId: "empty", title: "E", workspaceId: "w", roots: [] }], agentBins: ["claude"], ports: [[110, [5173, 24678]], [111, []]] });
actCase("a box thread's memory is its cgroup's where the daemon read one", { computer: "box", at: 1.5, procs: [row(500, 1, "bash", "bash /r/aaaaaaaaaaaa.sh", { cgroup: "/wsp-threads/t", rss: 10 }), row(501, 500, "claude", "claude -p", { cgroup: "/wsp-threads/t", rss: 20 }), row(60, 1, "wsp-daemon", "wsp-daemon serve")], daemon: 60, host: 0, scopes: [{ threadId: "t", title: "T", workspaceId: "w", roots: [], cgroup: "/wsp-threads/t" }, { threadId: "u", title: "U", workspaceId: "w", roots: [500] }], agentBins: ["claude"], cgroups: [{ cgroup: "/wsp-threads/x", memory: 1 }, { cgroup: "/wsp-threads/t", memory: 4096 }] });
const leftView = A.activityOf({ computer: "lab", at: START, procs: basic, daemon: 60, host: 50, scopes: [{ threadId: "lab-thread-1", title: "L", workspaceId: "w", roots: [100] }], agentBins: ["claude"] });
add("processes_left", [leftView, ["lab-thread-1"]], A.processesLeft(leftView, ["lab-thread-1"]));
add("processes_left", [leftView, ["nobody"]], A.processesLeft(leftView, ["nobody"]));
const ended = [{ pid: 1, ended: [leftView.threads[0].processes[0], leftView.threads[0].processes[1]], forced: [], left: [leftView.threads[0].processes[1].pid] }, { pid: 2, ended: [leftView.threads[0].processes[2]], forced: [7], left: [999, leftView.threads[0].processes[2].pid] }];
add("standing_after_stop", [ended], A.standingAfterStop(ended));
const heads = [{ pid: 5, ppid: 4 }, { pid: 4, ppid: 1 }, { pid: 6, ppid: 5 }, { pid: 7, ppid: 7 }, { pid: 8, ppid: 9 }];
add("end_heads", [heads], A.endHeads(heads));
const roots = [row(1, 0, "init", "i"), row(2, 1, "bash", "bash"), row(3, 1, "bash", "bash"), row(4, 2, "claude", "claude"), row(5, 3, "claude", "claude"), row(6, 4, "claude", "claude"), row(7, 6, "claude", "claude"), row(8, 7, "x", "x"), row(9, 7, "claude", "claude")];
for (const root of [1, 2, 3, 8, 42]) {
  const byPid = new Map(roots.map(p => [p.pid, p]));
  const kids = pid => roots.filter(p => p.ppid === pid && p.pid !== pid);
  add("agent_path", [roots, root, "claude"], A.agentPath(root, kids, p => p.comm === "claude", byPid) ?? null);
}

// Every typed input must be one its schema takes: the Rust side reads it through serde.
const typed = { with_place_stage: [V("place").PlaceAddJob, V("place").PlaceStageEvent], recipe_pins: [V("golden-image").RecipeDigest], golden_head: [V("golden-image").GoldenManifest], golden_image: [V("golden-image").GoldenVersion], provision_counts: [V("place").PlaceProvisionRow.array()], place_forgets_only: [V("place").PlaceHolds], thread_key_of: [S.SessionView], thread_ran: [S.SessionView.array()], recorded_picks: [S.SessionView.array()], sub_spawning: [V("sub-row").SubSpawn], sub_all_pushed: [V("sub-row").SubResult], processes_left: [A.ActivityView], standing_after_stop: [A.ActivityEnded.array()] };
for (const [fn, schemas] of Object.entries(typed)) for (const c of out[fn] ?? []) schemas.forEach((sch, i) => { if (c.args[i] === null) return; const r = sch.safeParse(c.args[i]); if (!r.success) throw new Error(`${fn} arg ${i}: ${JSON.stringify(r.error.issues[0])}`); });

fs.writeFileSync(process.argv[2], JSON.stringify(out, null, 1) + "\n");
console.log(Object.entries(out).map(([k, v]) => `${k}:${v.length}`).join(" "));
