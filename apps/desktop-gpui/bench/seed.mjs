// SPDX-License-Identifier: AGPL-3.0-only
// A host's state shaped like a person's long-used one, written before the host first starts so it costs no `wsp run`:
// projects, many workspaces and many settled threads, each thread one row of the sidebar carrying what a real row
// carries (its opening words up to the host's OPENING_KEPT, its title, its last line, model, effort, access, times,
// read and settled marks) and a short transcript, a third of them with the agent's own subagents, which every listing
// stamps on the row. fixture.mjs writes it when BENCH_SEED=<threads> reaches fixture.sh, so a clean launch reads a
// host like the dev host (1,581 threads); the default bench never makes it.

/** `OPENING_KEPT` in packages/protocol: the most of a thread's opening message its row keeps. */
const OPENING_KEPT = 600;
const WORDS = "the list keeps every row it has measured so nothing jumps when a long tool output lands and the anchor holds through the resize while older pages fold under the rows already drawn and the reader stays on the line they were reading host sidebar thread workspace settle fold pin snooze".split(" ");
const NAMES = ["spoo", "wsp", "landing", "billing", "docs", "infra", "mobile", "cli", "api", "web", "desk", "agents", "slate", "notes"];
const MODELS = ["claude-opus-5", "claude-sonnet-5", "gpt-5.5-codex"];

function rng(seed) {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

const between = (r, lo, hi) => lo + Math.floor(r() * (hi - lo + 1));
const words = (r, n) => Array.from({ length: n }, () => WORDS[Math.floor(r() * WORDS.length)]).join(" ");
const text = (r, chars) => {
  let s = "";
  while (s.length < chars) s += `${words(r, between(r, 6, 18))}. `;
  return s.slice(0, chars).trim();
};
const uuid = r => [8, 4, 4, 4, 12].map(n => Array.from({ length: n }, () => Math.floor(r() * 16).toString(16)).join("")).join("-");

/** Opening words as a row keeps them: a third a line, a third a paragraph, a third a brief cut at OPENING_KEPT. */
function opening(r) {
  const roll = r();
  return text(r, roll < 0.33 ? between(r, 30, 140) : roll < 0.66 ? between(r, 140, 450) : OPENING_KEPT);
}

/** The state's projects, workspaces, sessions and transcripts for `threads` settled threads over `workspaces`
 * workspaces, under the folder `dir`; `now` is the clock the times count back from. */
export function seededState({ threads, workspaces, dir, now = Date.now(), seed = 7 }) {
  const r = rng(seed);
  const day = 86_400_000;
  const projects = {};
  for (const name of NAMES) {
    const path = `${dir}/wsp-work/${name}`;
    projects[`pr_${name}`] = { id: `pr_${name}`, name, computer: "here", source: { kind: "folder", path }, path, remote: "", defaultBranch: "main", memoryKey: path, memoryDir: path, createdAt: new Date(now - 90 * day).toISOString() };
  }
  const spaces = {};
  for (let i = 0; i < workspaces; i++) {
    const project = NAMES[i % NAMES.length];
    const id = `ws_seed${String(i).padStart(4, "0")}`;
    const folder = `${dir}/wsp-work/${project}`;
    spaces[id] = { id, name: `${project}-${words(r, 2).replace(/ /g, "-")}-${i}`, machineId: "local", phase: "running", kind: "local", golden: "", createdAt: new Date(now - between(r, 1, 90) * day).toISOString(), size: { cpu: 8, memMb: 16384 }, home: dir, folder, spec: {}, project: `pr_${project}`, claudeSessionId: uuid(r) };
  }
  const ids = Object.keys(spaces);
  const sessions = {};
  const transcripts = {};
  for (let t = 0; t < threads; t++) {
    const workspaceId = ids[Math.floor(r() * ids.length)];
    const startedAt = now - between(r, 1, 80 * 24) * 3_600_000;
    const endedAt = startedAt + between(r, 20, 3600) * 1000;
    const [threadId, sessionId, turnId] = [uuid(r), uuid(r), uuid(r)];
    const prompt = opening(r);
    const lastLine = text(r, between(r, 60, 240));
    const model = MODELS[Math.floor(r() * MODELS.length)];
    const harness = model.startsWith("gpt") ? "codex" : "claude";
    const row = {
      id: sessionId,
      workspaceId,
      turnId,
      threadId,
      harness,
      status: r() < 0.94 ? "completed" : "failed",
      startedBy: r() < 0.7 ? "person" : "agent",
      claudeSessionId: sessionId,
      prompt,
      ...(r() < 0.8 ? { harnessTitle: words(r, between(r, 3, 8)), titleSource: "harness" } : {}),
      startedAt,
      endedAt,
      cwd: spaces[workspaceId].folder,
      costUsd: Math.round(r() * 400) / 100,
      lastLine,
      model,
      effort: "high",
      permissionMode: "bypassPermissions",
      contextWindow: r() < 0.5 ? "1m" : "200k",
      ...(r() < 0.3 ? { fast: true } : {}),
    };
    const entry = (sessions[workspaceId] ??= { workspaceId, sessions: [], threads: {} });
    entry.sessions.push(row);
    entry.threads[threadId] = { harness, readAt: endedAt + 60_000, ...(r() < 0.85 ? { settledAt: endedAt + day } : {}) };
    const scope = { workspaceId, sessionId, threadId, turnId };
    const events = [{ ...scope, type: "session.start", at: startedAt, prompt, model, cwd: row.cwd }];
    const children = r() < 0.35 ? between(r, 2, 24) : 0;
    for (let c = 0; c < children; c++) {
      const task = `agent_${t}_${c}`;
      const call = `toolu_${t}_${c}`;
      const at = startedAt + c * 1000;
      events.push({ ...scope, type: "session.subagent", at, task, state: "running", parentToolUseId: call, title: words(r, between(r, 3, 7)), model, asked: text(r, between(r, 60, 200)) });
      events.push({ ...scope, type: "session.subagent", at: at + 500, task, state: "done", parentToolUseId: call, summary: text(r, between(r, 40, 200)) });
    }
    events.push({ ...scope, type: "session.delta", at: endedAt - 1000, kind: "text", text: lastLine });
    events.push({ ...scope, type: "session.done", at: endedAt, result: { status: row.status === "failed" ? "failed" : "completed", durationMs: endedAt - startedAt, costUsd: row.costUsd, text: lastLine } });
    events.push({ ...scope, type: "session.end", at: endedAt, exitCode: 0, sawResult: true });
    (transcripts[workspaceId] ??= { workspaceId, events: [] }).events.push(...events);
  }
  return { projects, workspaces: spaces, sessions, transcripts };
}
