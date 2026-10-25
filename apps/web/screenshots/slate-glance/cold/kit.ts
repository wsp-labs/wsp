// SPDX-License-Identifier: AGPL-3.0-only
// The two slate tools as one command for a writer with no host, for #2439's cold test: `slate catalog [name]` answers
// what slate_catalog does and `slate write <file>` what slate_write does, through the runtime's own slates and the
// host's own reply and failure shapes. The thread's slate is kept beside this file between calls, and every write is
// logged with its answer to .writes.jsonl there.
import { appendFileSync, existsSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { failureWords } from "../../../../../packages/engine/src/errors.js";
import { refusalFields, type Caller } from "../../../../../packages/protocol/src/index.js";
import { asText } from "../../../../../packages/host/src/verbs/io.js";
import { toolFailure } from "../../../../../packages/host/src/verbs/client.js";
import { problemListsOf } from "../../../../../packages/protocol/src/exit.js";
import { createSlates } from "../../../../../packages/runtime/src/slates.js";
import { SLATES } from "../../../../../packages/runtime/src/lazy-slates.js";
import { memoryStore } from "../../../../../packages/runtime/src/store.js";

const here = dirname(resolve(process.argv[1]!));
const statePath = join(here, ".slate-state.json");
const THREAD = "0b7e5c2a-4f1d-4c8e-9a3b-slatewriter1";
const WORKSPACE = "ws-slate-writer";
const caller: Caller = { origin: "here", by: { kind: "thread", threadId: THREAD, workspaceId: WORKSPACE, rootThreadId: THREAD } };

async function main(): Promise<number> {
  const [verb, ...args] = process.argv.slice(2);
  const store = memoryStore();
  if (existsSync(statePath)) for (const [id, r] of Object.entries(JSON.parse(readFileSync(statePath, "utf8")) as Record<string, unknown>)) await store.put(SLATES, id, r);
  const slates = createSlates({
    store,
    now: () => Date.now(),
    record: () => {},
    emit: () => {},
    thread: () => ({ workspaceId: WORKSPACE, rootThreadId: THREAD, sessionId: "s", folder: process.cwd(), hostFolder: process.cwd(), computer: "this computer" }),
    under: lead => [lead],
    threadOfToken: () => THREAD,
    sources: () => ({ threadId: THREAD, workspaceId: WORKSPACE, now: Date.now(), rows: () => [], results: () => [], account: async () => undefined, workspace: () => ({ computer: "this computer", folder: process.cwd() }) }),
    deliver: async () => ({ outcome: "queued" }),
    runEnv: () => ({}),
  });
  // A host loads its slates once at start; loaded first inside a write's own queue, the load waits on that write.
  await slates.get(THREAD);
  let request: Record<string, unknown>;
  let ask: () => Promise<Record<string, unknown>>;
  if (verb === "catalog" && args.length <= 1) {
    request = args[0] !== undefined ? { name: args[0] } : {};
    ask = () => slates.catalog({ turnToken: "t", ...request }, caller);
  } else if (verb === "write" && args.filter(a => a !== "--check").length === 1) {
    const file = args.find(a => a !== "--check")!;
    if (!/\.(slate|json)$/.test(file)) return say(`${file} is neither a .slate nor a .json file. Write the JSX-like form to a .slate file or the stored form to a .json file.`, 3);
    if (!existsSync(file)) return say(`there is no file at ${resolve(file)}. Name a .slate or .json file that is there.`, 3);
    const body = readFileSync(file, "utf8");
    request = { ...(file.endsWith(".slate") ? { text: body } : { document: JSON.parse(body) as Record<string, unknown> }), ...(args.includes("--check") ? { check: true } : {}) };
    ask = () => slates.write({ turnToken: "t", ...request }, caller) as unknown as Promise<Record<string, unknown>>;
  } else {
    return say("usage: ./slate catalog [<name>] | ./slate write <file.slate> [--check]", 3);
  }
  let answer: { text: string; isError: boolean; structured: unknown };
  try {
    const got = await ask();
    const result = asText((got as { text: string }).text, got as never);
    answer = { text: (result.content[0] as { text: string }).text, isError: false, structured: result.structuredContent };
  } catch (e) {
    // The host's reply frame for a failure, then the client's error off it, then the tool's failure, as the wire does.
    const { kind, fix } = e as { kind?: unknown; fix?: unknown };
    const frame = { error: failureWords(e), ...(typeof kind === "string" ? { kind } : {}), ...(typeof fix === "string" ? { fix } : {}), ...refusalFields(e) };
    const err = Object.assign(new Error(frame.error), typeof frame.kind === "string" ? { kind: frame.kind } : {}, problemListsOf(frame));
    const failed = toolFailure(err, verb === "write" ? "wsp slate write [<thread>] [<file>] [--check] [--set <path>=<json>]... [--press <piece>] [--row <n>] [--action <n>] [--if-version <n>]" : "wsp slate catalog [<name>]");
    answer = { text: (failed.content[0] as { text: string }).text, isError: true, structured: failed.structuredContent };
  }
  const kept: Record<string, unknown> = {};
  for (const id of await store.keys(SLATES)) kept[id] = await store.get(SLATES, id);
  writeFileSync(statePath, JSON.stringify(kept));
  const log = verb === "write" ? ".writes.jsonl" : ".catalog.jsonl";
  appendFileSync(join(here, log), `${JSON.stringify({ at: new Date().toISOString(), cwd: process.cwd(), args, request, isError: answer.isError, answer: answer.text, structured: answer.structured })}\n`);
  (answer.isError ? process.stderr : process.stdout).write(`${answer.text}\n`);
  return answer.isError ? 1 : 0;
}

function say(line: string, code: number): number {
  process.stderr.write(`${line}\n`);
  return code;
}

main().then(code => process.exit(code), (e: unknown) => { process.stderr.write(`${e instanceof Error ? e.stack : String(e)}\n`); process.exit(1); });
