// SPDX-License-Identifier: AGPL-3.0-only
// The session fixture's extra threads, made only when BENCH_SESSION=<n> reaches fixture.sh: `session-1` to
// `session-<n>`, each a medium thread shaped like `real` (screenshots, pastes, tool output, subagents, code, tables)
// with its own seed and length, so a run that opens them one after another, as a person moves down the sidebar,
// meets a different thread each time. The default bench never makes them. fixture.mjs writes them into the host's
// state before it starts, as the host records the stub's turns, since a busy Mac's host could not keep up with the
// snapshot each turn takes: sessionFolder makes the commits their cards and checkpoints name, sessionState the rest.
import { spawnSync } from "node:child_process";
import { randomUUID } from "node:crypto";
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { turnsOf } from "./content.mjs";
import { fileText, shot } from "./real.mjs";
import { encode } from "../parity/png.mjs";

const NAME = /^session-(\d+)$/;

/** The plan of `session-<i>`, in the shape `real` takes; undefined for any other name. 12 to 40 turns. */
export function sessionPlan(name) {
  const i = Number(NAME.exec(name)?.[1] ?? 0);
  if (i < 1) return undefined;
  return { shape: "real", turns: 12 + ((i * 13) % 29), calls: 3, tasks: 2, changes: 1, pastes: 0.06, shots: 0.1, writeUps: 0.03, tables: 0.08, code: 0.3, seed: 100 + i };
}

/** The names BENCH_SESSION asks for: none when it is unset or empty. */
export function sessionNames(asked) {
  const n = Number(asked ?? 0);
  if (!Number.isInteger(n) || n < 0 || n > 200) throw new Error(`BENCH_SESSION=${asked} is not a count from 0 to 200`);
  return Array.from({ length: n }, (_, i) => `session-${i + 1}`);
}

/** The commits the threads' cards and checkpoints name, made in the repository at `folder` as the daemon makes its
 * snapshots and checkpoints, but from an index and a work tree of their own under `scratch`, so the checkout is left
 * alone. Each thread changes files on one turn. The work tree holds what real's turns wrote, then what each earlier
 * thread wrote, as the fixture once played them in that order. By name: the turn that changes files, what its card
 * records, and the two checkpoints (for the turns before it, and for it and the turns after). */
export function sessionFolder(folder, names, scratch) {
  const tree = join(scratch, "tree");
  mkdirSync(tree, { recursive: true });
  const env = { PATH: process.env.PATH, HOME: scratch, GIT_INDEX_FILE: join(scratch, "index") };
  const git = (...args) => {
    const ran = spawnSync("git", ["-c", "user.name=wsp", "-c", "user.email=wsp@localhost", `--git-dir=${join(folder, ".git")}`, `--work-tree=${tree}`, ...args], { cwd: tree, env, encoding: "utf8" });
    if (ran.status !== 0) throw new Error(`git ${args.join(" ")} failed: ${ran.stderr}`);
    return ran.stdout.trim();
  };
  const write = file => {
    mkdirSync(dirname(join(tree, file.path)), { recursive: true });
    writeFileSync(join(tree, file.path), fileText(file));
  };
  const head = git("rev-parse", "HEAD");
  git("read-tree", head);
  const message = `wsp snapshot\n\nreflog ${git("reflog", "--format=%H", "HEAD").split("\n").filter(Boolean).length}\nhead ${git("symbolic-ref", "-q", "HEAD")}`;
  const taken = () => {
    git("add", "-A");
    return git("write-tree");
  };
  const commit = (of, said) => git("commit-tree", "-p", head, "-m", said, of);
  for (const turn of turnsOf("real")) turn.files.forEach(write);
  const made = {};
  for (const name of names) {
    const turns = turnsOf(name);
    const at = turns.findIndex(t => (t.files ?? []).length > 0);
    const before = taken();
    turns[at].files.forEach(write);
    const left = taken();
    const [from, to] = [commit(before, message), commit(left, message)];
    // As git.turn lists them: the paths the turn changed, each against HEAD, so a file no commit holds is added whole.
    const touched = git("diff-tree", "-r", "-z", "--no-renames", "--name-only", from, to).split("\0").filter(Boolean);
    const kinds = new Map(fields(git("diff-tree", "-r", "-z", "--no-renames", "--name-status", head, to, "--", ...touched), 2).map(([kind, path]) => [path, kind]));
    const files = fields(git("diff-tree", "-r", "-z", "--no-renames", "--numstat", head, to, "--", ...touched).replace(/\t/g, "\0"), 3)
      .map(([additions, deletions, path]) => ({ path, kind: { A: "added", D: "deleted" }[kinds.get(path)] ?? "modified", additions: Number(additions), deletions: Number(deletions) }))
      .sort((a, b) => (a.path < b.path ? -1 : 1));
    // A turn that changed nothing records no card, as the host's read of it records none.
    made[name] = { at, ...(touched.length > 0 ? { changes: { from, to, files, moved: [] } } : {}), checkpoints: [commit(before, "wsp checkpoint"), commit(left, "wsp checkpoint")] };
  }
  return made;
}

/** git's -z output cut into records of `n` fields. */
const fields = (out, n) => {
  const all = out.split("\0").filter(Boolean);
  return Array.from({ length: all.length / n }, (_, i) => all.slice(i * n, i * n + n));
};

/** The host's state for the threads in `names`, as it records them once the stub has played every turn through
 * `wsp run` and `wsp send`, in a project `bench` at `folder` with the folder's workspace: the records to write into
 * the state file, the screenshots each message kept as blobs, and the checkpoint refs to write into the folder's
 * repository. `made` is sessionFolder's; the turns end two seconds apart, the last before `now`. */
export async function sessionState({ names, home, folder, made, now = Date.now() }) {
  const built = pkg => import(pathToFileURL(join(dirname(fileURLToPath(import.meta.url)), `../../../packages/${pkg}/dist/index.js`)).href);
  const [{ attachmentKey, claudeMemoryDir, claudeProjectKey, HERE_PLACE_ID, listedLastLine, subagentAsked }, { LOCAL_MACHINE_ID, localShape }] = await Promise.all([built("protocol"), built("engine")]);
  const projectId = `pr_${randomUUID().slice(0, 8)}`;
  const workspaceId = `ws_${randomUUID().slice(0, 8)}`;
  const memoryKey = claudeProjectKey(folder);
  const project = { id: projectId, name: "bench", computer: HERE_PLACE_ID, source: { kind: "folder", path: folder }, path: folder, remote: "", defaultBranch: "main", memoryKey, memoryDir: claudeMemoryDir(`${home}/.claude`, memoryKey), git: { top: folder } };
  const created = new Date(now - 2_000 * names.reduce((n, name) => n + turnsOf(name).length, 0) - 60_000).toISOString();
  const workspace = { id: workspaceId, name: "bench", kind: "local", machineId: LOCAL_MACHINE_ID, phase: "running", golden: "", createdAt: created, project: projectId, spec: {}, size: localShape(), firstLife: false, idleWindowMs: null };
  const sessions = { workspaceId, sessions: [], threads: {} };
  const events = [];
  const keys = {};
  const blobs = [];
  const refs = [];
  let clock = Date.parse(created) + 60_000;
  const facts = { model: "claude-sonnet-5", cwd: folder, permissionMode: "bypassPermissions", effort: "high", agent: "claude", tools: [], harness: { slashCommands: [], permissionMode: "bypassPermissions" } };
  for (const name of names) {
    const [threadId, sessionId] = [randomUUID(), randomUUID()];
    const thread = made[name];
    const turns = turnsOf(name);
    let row;
    let readAt;
    turns.forEach((turn, t) => {
      const turnId = randomUUID();
      const requestId = randomUUID();
      const scope = { workspaceId, sessionId, turnId, threadId };
      let at = (clock += 2_000) - 1_000;
      const startedAt = at;
      const record = e => events.push({ type: e.type, ...scope, ...e, at: at++ });
      const attachments = (turn.images ?? []).map((image, i) => {
        const bytes = encode(shot(image), 2);
        const key = attachmentKey(threadId, requestId, i);
        blobs.push({ collection: "attachments", id: key, bytes });
        (keys[threadId] ??= []).push({ key, mediaType: "image/png" });
        return { mediaType: "image/png", bytes: bytes.length, name: image.name };
      });
      record({ type: "session.start", prompt: turn.prompt, requestId, ...(t === 0 ? { opensThread: true } : {}), startedBy: "cli", ...(attachments.length > 0 ? { attachments } : {}), ...facts });
      let line = 0;
      const delta = (kind, e) => record({ type: "session.delta", line: ++line, kind, ...e });
      if (turn.thinking !== undefined) delta("thinking", { text: turn.thinking });
      turn.tools.forEach((call, i) => {
        const id = `${name}-${t}-${i}`;
        delta("tool_use", { text: JSON.stringify(call.input), toolName: call.name, toolUseId: `toolu_${id}` });
        if (call.children !== undefined) {
          const task = { task: `agent_${id}`, parentToolUseId: `toolu_${id}` };
          record({ type: "session.subagent", line: 1, ...task, state: "running", title: call.input.description, asked: subagentAsked(call.input.prompt) });
          record({ type: "session.subagent", line: 2, ...task, state: "running", model: facts.model });
          call.children.forEach((child, j) => {
            const under = { parentToolUseId: `toolu_${id}` };
            if (child.text !== undefined) return delta("text", { text: child.text, messageId: `msg_${id}-${j}`, ...under });
            delta("tool_use", { text: JSON.stringify(child.input), toolName: child.name, toolUseId: `toolu_${id}-${j}`, ...under });
            delta("tool_result", { text: child.output, toolUseId: `toolu_${id}-${j}`, isError: child.error === true, ...under });
          });
          const n = call.children.length;
          record({ type: "session.subagent", line: 3, ...task, state: "done", usage: { tokens: { input: n, output: n, cached: 0, cacheWrite: 0, reasoning: 0 } } });
        }
        delta("tool_result", { text: call.output, toolUseId: `toolu_${id}`, isError: call.error === true });
      });
      delta("text", { text: turn.reply, messageId: `msg_${name}-${t}-reply` });
      record({ type: "session.done", result: { status: "completed", durationMs: 10, tokens: { input: 1, output: 1, context: 2 }, text: turn.reply, costUsd: 0, model: facts.model } });
      const endedAt = at;
      record({ type: "session.end", exitCode: 0, sawResult: true });
      const ref = `refs/wsp/checkpoints/${workspaceId}/${threadId}/${turnId}`;
      refs.push([ref, thread.checkpoints[t < thread.at ? 0 : 1]]);
      record({ type: "session.checkpoint", ref, based: true, anchor: `a_${name}-${t}-reply` });
      if (t === thread.at && thread.changes !== undefined) {
        record({ type: "session.changes", ...thread.changes });
        readAt = at + 100;
      }
      row = { id: sessionId, workspaceId, harness: "claude", status: "completed", startedBy: "cli", threadId, prompt: turns[0].prompt, startedAt, claudeSessionId: sessionId, model: facts.model, effort: facts.effort, permissionMode: facts.permissionMode, cwd: folder, costUsd: 0, lastLine: listedLastLine(turn.reply), endedAt, turnId };
    });
    sessions.sessions.push(row);
    sessions.threads[threadId] = { harness: "claude", permissionMode: facts.permissionMode, worked: true, clouds: [], readAt };
  }
  const records = {
    projects: { [projectId]: { ...project, createdAt: created } },
    workspaces: { [workspaceId]: workspace },
    "workspace-names": { [workspaceId]: { workspaceId, name: "bench", project: projectId } },
    sessions: { [workspaceId]: sessions },
    transcripts: { [workspaceId]: { workspaceId, events } },
    "attachment-keys": { [workspaceId]: { threads: keys } },
  };
  const threads = Object.fromEntries(names.map((name, i) => [name, sessions.sessions[i].threadId]));
  return { records, blobs, refs, threads };
}
