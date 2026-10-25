// Records what the TS command line does for every recorded case of the tools in cli-b's slice: the frames each verb
// sends, its stdout and stderr lines and its exit code, as text and as --json, against a host answering each op with
// the case's frame. Read-only over /root/wsp-vm/wt-2441-cut (65f6b89c7).
import { it, vi } from "vitest";
import { mkdtempSync, mkdirSync, readFileSync, writeFileSync, readdirSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const R = "/root/wsp-vm/wt-2441-cut";
const ANSWERS = "/root/wsp-vm/wt-2449-cli-b/daemon/crates/wsp-mcp/tests/answers";
const OUT = process.env.OUT ?? "/root/wsp-vm/wt-2449-cli-b/host/crates/wsp-cli/tests/fixtures/cli_b";

type Args = Record<string, unknown>;
type Spec = { verb: string; pos?: string[]; yes?: boolean; custom?: (a: Args, files: Record<string, string>) => string[] };

const S = (verb: string, pos: string[] = [], yes = false): Spec => ({ verb, pos, yes });
const UNIT: Record<string, string[]> = {
  activity: ["activity", "activity_end", "conversations"],
  slate: ["slate_catalog", "slate_write", "slate_state", "slate_read"],
  projects: ["projects", "projects_remove", "projects_set", "projects_seed", "threads", "threads_wait"],
  computers: ["computers", "computers_set", "recipes", "recipes_show", "recipes_save", "recipes_remove", "usage"],
  agents_skills: ["agents", "skills", "skills_search", "skills_show", "skills_add", "skills_remove", "skills_disable", "skills_enable"],
  agents_servers: ["servers", "servers_tools", "servers_add", "servers_remove", "servers_disable", "servers_enable", "plugins", "plugins_disable", "plugins_enable", "agents_addtools", "agents_default", "agents_set", "agents_setup"],
  workspaces_git: ["rename", "fork", "commit", "discard", "fix", "merge_in", "merge"],
  workspaces_life: ["start", "review", "review_post", "update", "pause", "resize", "wake", "rebuild", "image", "image_build", "forget", "delete"],
};

const SPECS: Record<string, Spec> = {
  activity: S("activity", ["thread"]),
  activity_end: S("activity end", ["pid"]),
  conversations: S("conversations", ["project"]),
  slate_catalog: S("slate catalog", ["name"]),
  slate_write: {
    verb: "slate write",
    custom: (a, files) => {
      const argv: string[] = [];
      if (typeof a["thread"] === "string") argv.push(a["thread"]);
      if (typeof a["text"] === "string") {
        files["slate.slate"] = a["text"];
        argv.push("slate.slate");
      } else if (a["document"] !== undefined) {
        files["slate.json"] = JSON.stringify(a["document"]);
        argv.push("slate.json");
      }
      if (a["check"] === true) argv.push("--check");
      for (const [k, v] of Object.entries((a["values"] as Args | undefined) ?? {})) argv.push(`--set=${k}=${JSON.stringify(v)}`);
      if (typeof a["press"] === "string") argv.push(`--press=${a["press"]}`);
      if (a["row"] !== undefined) argv.push(`--row=${a["row"]}`);
      if (a["action"] !== undefined) argv.push(`--action=${a["action"]}`);
      if (a["if_version"] !== undefined) argv.push(`--if-version=${a["if_version"]}`);
      return argv;
    },
  },
  slate_state: {
    verb: "slate state",
    custom: a => {
      const argv: string[] = [];
      if (typeof a["thread"] === "string") argv.push(a["thread"]);
      for (const [k, v] of Object.entries((a["values"] as Args | undefined) ?? {})) argv.push(`${k}=${JSON.stringify(v)}`);
      for (const s of (a["start"] as string[] | undefined) ?? []) argv.push(`--start=${s}`);
      if (a["if_version"] !== undefined) argv.push(`--if-version=${a["if_version"]}`);
      return argv;
    },
  },
  slate_read: {
    verb: "slate read",
    custom: a => {
      const argv: string[] = [];
      if (typeof a["thread"] === "string") argv.push(a["thread"]);
      for (const s of (a["values"] as string[] | undefined) ?? []) argv.push(`--values=${s}`);
      if (a["text"] === false) argv.push("--no-text");
      if (a["sketch"] === false) argv.push("--no-sketch");
      if (a["document"] === true) argv.push("--document");
      return argv;
    },
  },
  projects: S("projects"),
  projects_remove: S("projects remove", ["project"], true),
  projects_set: S("projects set", ["project"]),
  projects_seed: S("projects seed", ["project"]),
  threads: S("threads", ["project"]),
  threads_wait: S("threads wait", ["threads"]),
  computers: S("computers"),
  computers_set: S("computers set", ["computer"]),
  recipes: S("recipes"),
  recipes_show: S("recipes show", ["name"]),
  recipes_save: S("recipes save", ["name"]),
  recipes_remove: S("recipes remove", ["name"], true),
  usage: S("usage"),
  agents: S("agents", ["thread"]),
  skills: S("skills", ["thread"]),
  skills_search: S("skills search", ["query"]),
  skills_show: S("skills show", ["skill", "thread"]),
  skills_add: S("skills add", ["skill", "thread"]),
  skills_remove: S("skills remove", ["name", "thread"], true),
  skills_disable: S("skills disable", ["name", "thread"]),
  skills_enable: S("skills enable", ["name", "thread"]),
  servers: S("servers", ["thread"]),
  servers_tools: S("servers tools", ["name", "thread"]),
  servers_add: S("servers add", ["name", "thread"]),
  servers_remove: S("servers remove", ["name", "thread"], true),
  servers_disable: S("servers disable", ["name", "thread"]),
  servers_enable: S("servers enable", ["name", "thread"]),
  plugins: S("plugins", ["thread"]),
  plugins_disable: S("plugins disable", ["plugin", "thread"]),
  plugins_enable: S("plugins enable", ["plugin", "thread"]),
  agents_addtools: S("agents addtools", ["agent"]),
  agents_default: S("agents default", ["agent"]),
  agents_set: S("agents set", ["agent"]),
  agents_setup: S("agents setup", ["agent"]),
  rename: S("rename", ["workspace", "name"]),
  fork: S("fork", ["workspace"]),
  commit: S("commit", ["thread"]),
  discard: S("discard", ["thread", "path"], true),
  fix: S("fix", ["thread"]),
  merge_in: S("merge in", ["lead", "child"]),
  merge: S("merge", ["thread"]),
  start: S("start", ["link"]),
  review: S("review", ["target"]),
  review_post: S("review post", ["thread"]),
  update: S("update", ["thread"]),
  pause: S("pause", ["workspace"]),
  resize: S("resize", ["thread"]),
  wake: S("wake", ["workspace"]),
  rebuild: S("rebuild", ["workspace"]),
  image: S("image"),
  image_build: S("image build", ["place"]),
  forget: S("forget", ["workspace"], true),
  delete: S("delete", ["thread", "workspace"]),
};

/** The tool's input names as the command line spells them, where they differ from the input with _ as -. */
const RENAMED: Record<string, string> = { files: "file", trackers: "tracker", task: "send", args: "arg", started: "started" };

function argvOf(tool: string, a: Args, files: Record<string, string>): string[] {
  const spec = SPECS[tool]!;
  const words = spec.verb.split(" ");
  if (spec.custom !== undefined) return [...words, ...spec.custom(a, files)];
  const argv = [...words];
  const pos = spec.pos ?? [];
  for (const p of pos) {
    const v = a[p];
    if (Array.isArray(v)) argv.push(...v.map(String));
    else if (v !== undefined) argv.push(String(v));
  }
  for (const [k, v] of Object.entries(a)) {
    if (pos.includes(k) || v === undefined) continue;
    if (k === "confirm") {
      if (v === true) argv.push("--yes");
      continue;
    }
    if (tool === "agents_setup" && k === "enabled") {
      argv.push(v === true ? "--enable" : "--disable");
      continue;
    }
    if (tool === "agents_set" && k === "order") {
      argv.push(`--order=${(v as string[]).join(",")}`);
      continue;
    }
    if (tool === "review_post" && k === "verdict") {
      argv.push(`--verdict=${String(v).replace(/_/g, "-")}`);
      continue;
    }
    const name = RENAMED[k] ?? k.replace(/_/g, "-");
    if (v === true) argv.push(`--${name}`);
    else if (v === false) continue;
    else if (Array.isArray(v)) for (const x of v) argv.push(`--${name}=${String(x)}`);
    else argv.push(`--${name}=${String(v)}`);
  }
  if (spec.yes === true && !argv.includes("--yes")) argv.push("--yes");
  return argv;
}

const NOT_IN_RECORD = "is not in this record";

interface Run {
  frames: string[];
  stdout: string[];
  stderr: string[];
  streamed: string;
  exit: number;
}

function answeringHost(replies: Record<string, string>, frames: string[], pushed: Record<string, string[]>) {
  const listeners = new Set<(f: Record<string, unknown>) => void>();
  const seen = new Map<string, number>();
  let next = 2;
  let subscribed: Promise<void> | undefined;
  const request = async (op: string, params: Record<string, unknown> = {}): Promise<Record<string, unknown>> => {
    const id = next++;
    const nth = (seen.get(op) ?? 0) + 1;
    seen.set(op, nth);
    let text = JSON.stringify({ ...params, id, op });
    text = text.replace(/"requestId":"[^"]*"/, '"requestId":"<request id>"');
    frames.push(text);
    const answer = replies[`${op} #${nth}`] ?? replies[op] ?? (op === "events.subscribe" ? JSON.stringify({ id: 1, ok: true }) : undefined);
    const frame = JSON.parse(answer ?? JSON.stringify({ ok: false, error: `${op} ${NOT_IN_RECORD}` })) as Record<string, unknown>;
    for (const f of pushed[op] ?? []) for (const fn of [...listeners]) fn(JSON.parse(f));
    const { problemListsOf } = await import(`${R}/packages/protocol/src/index.ts`);
    if (frame["ok"] !== true) throw Object.assign(new Error(typeof frame["error"] === "string" ? frame["error"] : `${op} failed`), typeof frame["kind"] === "string" ? { kind: frame["kind"] } : {}, problemListsOf(frame));
    return frame;
  };
  return {
    request,
    events: () => (subscribed ??= request("events.subscribe").then(() => undefined)),
    onFrame: (fn: (f: Record<string, unknown>) => void) => {
      listeners.add(fn);
      return () => listeners.delete(fn);
    },
    closed: new Promise<number>(() => {}),
    closeWords: () => "the host closed the connection",
    close: () => {},
    terminate: () => {},
  };
}

let plainVerbs: Promise<typeof import("/root/wsp-vm/wt-2441-cut/packages/host/src/verbs.ts")> | undefined;
let cloudVerbs: Promise<typeof import("/root/wsp-vm/wt-2441-cut/packages/host/src/verbs.ts")> | undefined;
async function verbsIn(cloud: boolean) {
  plainVerbs ??= import(`${R}/packages/host/src/verbs.ts`);
  const plain = await plainVerbs;
  if (!cloud) return plain;
  cloudVerbs ??= (async () => {
    vi.resetModules();
    vi.stubEnv("WSP_CLOUD", "1");
    try {
      return await import(`${R}/packages/host/src/verbs.ts`);
    } finally {
      vi.unstubAllEnvs();
    }
  })();
  return cloudVerbs;
}

async function runLine(argv: string[], replies: Record<string, string>, o: { cloud: boolean; env: Record<string, string>; pushed: Record<string, string[]>; files: Record<string, string>; tty?: Tty }): Promise<Run & { cwd: string; asked?: string[] }> {
  const V = await verbsIn(o.cloud);
  const cwd = mkdtempSync(join(tmpdir(), "cli-b-"));
  for (const [name, text] of Object.entries(o.files)) writeFileSync(join(cwd, name), text);
  const run: Run = { frames: [], stdout: [], stderr: [], streamed: "", exit: -1 };
  const verb = V.findVerb(argv);
  if (verb === undefined) throw new Error(`no verb for ${argv.join(" ")}`);
  const asked: string[] = [];
  const secrets = [...(o.tty?.secrets ?? [])];
  const answers = [...(o.tty?.answers ?? [])];
  const io = {
    ...(o.tty !== undefined ? { isTTY: true } : {}),
    log: (l: string) => run.stdout.push(l),
    error: (l: string) => run.stderr.push(l),
    stream: (t: string) => (run.streamed += t),
    ask: (q: string) => (o.tty !== undefined && answers.length > 0 ? (asked.push(q), Promise.resolve(answers.shift()!)) : Promise.reject(new Error(`unexpected prompt: ${q}`))),
    askSecret: (q: string) => (o.tty !== undefined && secrets.length > 0 ? (asked.push(q), Promise.resolve(secrets.shift()!)) : Promise.reject(new Error(`unexpected prompt: ${q}`))),
  };
  const host = answeringHost(replies, run.frames, o.pushed);
  run.exit = await V.runVerb(verb, argv, io as never, () => "/state/state.json", { env: { ...o.env }, cwd, dial: (async () => host) as never });
  const scrub = (s: string) => s.split(cwd).join("<cwd>");
  return { frames: run.frames.map(scrub), stdout: run.stdout.map(scrub), stderr: run.stderr.map(scrub), streamed: scrub(run.streamed), exit: run.exit, cwd, ...(o.tty !== undefined ? { asked } : {}) };
}


type Tty = { secrets: string[]; answers: string[] };
type Hand = { unit: string; case: string; argv: string[]; replies?: Record<string, string>; cloud?: true; env?: Record<string, string>; files?: Record<string, string>; tty?: Tty };
const H = (unit: string, c: string, argv: string, extra: Partial<Hand> = {}): Hand => ({ unit, case: c, argv: argv.split(" ").filter(w => w !== ""), ...extra });
const recorded = (tool: string, name: string): Record<string, string> => (JSON.parse(readFileSync(join(ANSWERS, `${tool}.json`), "utf8")).cases as { case: string; replies: Record<string, string> }[]).find(c => c.case === name)!.replies;
const PLACES_TWO = JSON.stringify({ id: 1, ok: true, places: [{ id: "here", kind: "computer", name: "this mac", default: true }, { id: "place-9", kind: "computer", name: "attic", default: false }] });
const ENDED = JSON.stringify({ id: 1, ok: true, pid: 16, ended: [], forced: [], left: [] });
const HA = (unit: string, c: string, argv: string[], extra: Partial<Hand> = {}): Hand => ({ unit, case: c, argv, ...extra });
const HAND: Hand[] = [
  HA("workspaces_life", "wake with no status listed", ["wake", "alpha"], { cloud: true, replies: { ...recorded("wake", "woken"), "status.list": JSON.stringify({ id: 1, ok: true, statuses: [] }) } }),
  HA("workspaces_life", "rebuild a gone machine", ["rebuild", "alpha"], { cloud: true, replies: recorded("rebuild", "rebuilt") }),
  HA("agents_servers", "agents key at a terminal, the token typed", ["agents", "key", "claude"], { tty: { secrets: ["sk-ant-oat01-SECRET"], answers: [] }, replies: { "agents.key": JSON.stringify({ id: 1, ok: true }) } }),
  HA("agents_servers", "agents key at a terminal, an API key", ["agents", "key", "codex", "--api-key"], { tty: { secrets: ["sk-proj-SECRET"], answers: [] }, replies: { "agents.key": JSON.stringify({ id: 1, ok: true }) } }),
  HA("agents_servers", "agents setup at a terminal, two variables typed", ["agents", "setup", "claude", "--env", "FOO", "--env", "BAR_TOKEN"], { tty: { secrets: ["foo-SECRET", "bar-SECRET"], answers: [] }, replies: recorded("agents_setup", "set here") }),
  HA("agents_servers", "servers add with a variable read off the environment", ["servers", "add", "lin", "--agent", "claude", "--command", "npx lin --token ${LIN_TOKEN}", "--env", "LIN_TOKEN"], { env: { LIN_TOKEN: "lin-SECRET" }, replies: { "servers.add": JSON.stringify({ id: 1, ok: true, file: "~/.claude.json" }) } }),
  HA("agents_servers", "servers add an address with a header", ["servers", "add", "lin", "--agent", "codex", "--url", "https://mcp.example/${LIN_HOST}", "--header", "Authorization=LIN_AUTH", "--env", "LIN_HOST"], { env: { LIN_AUTH: "Bearer auth-SECRET", LIN_HOST: "h1" }, replies: { "servers.add": JSON.stringify({ id: 1, ok: true, file: "~/.codex/config.toml" }) } }),
  HA("workspaces_life", "delete at a terminal answered no", ["delete", "t-2"], { tty: { secrets: [], answers: ["no"] }, replies: recorded("delete", "thread deleted with its worktree") }),
  HA("workspaces_life", "delete at a terminal answered yes", ["delete", "t-2"], { tty: { secrets: [], answers: ["yes"] }, replies: recorded("delete", "thread deleted with its worktree") }),
  HA("projects", "remove at a terminal answered yes", ["projects", "remove", "alpha"], { tty: { secrets: [], answers: ["yes"] }, replies: recorded("projects_remove", "removed") }),
  HA("computers", "usage reset by key, said yes", ["usage", "reset", "codex:acct_7f3a", "--credit", "cr-1", "--on", "attic", "--yes"], { replies: { "usage.accounts": recorded("usage", "an account read and one with no reading, and a day of use by agent")["usage.accounts"]!, "usage.reset": JSON.stringify({ id: 1, ok: true, outcome: "reset", said: "used a reset on dév@example.com; its windows start again now" }) } }),
  HA("computers", "usage reset by label in another case, off a terminal", ["usage", "reset", "DÉV@example.com"], { replies: { "usage.accounts": recorded("usage", "an account read and one with no reading, and a day of use by agent")["usage.accounts"]! } }),
  HA("computers", "usage reset of an account nobody holds", ["usage", "reset", "nobody", "--yes"], { replies: { "usage.accounts": recorded("usage", "an account read and one with no reading, and a day of use by agent")["usage.accounts"]! } }),
  HA("computers", "recipes remove off a terminal after the check", ["recipes", "remove", "laptop-one"], { replies: recorded("recipes_remove", "removed") }),
  HA("projects", "remove off a terminal asks after the host's check", ["projects", "remove", "alpha"], { replies: recorded("projects_remove", "removed") }),
  HA("projects", "remove with unsaved work, off a terminal", ["projects", "remove", "alpha"], { replies: { ...recorded("projects_remove", "removed"), "projects.remove #1": JSON.stringify({ id: 1, ok: true, unsaved: "/root/alpha on attic" }) } }),
  HA("projects", "seed with an empty menu", ["projects", "seed", "lab"], { replies: { "projects.seed": JSON.stringify({ id: 1, ok: true, project: "lab", folder: "/work/acme/lab", rows: [], picked: { paths: [], bytes: 0, missing: [] }, saved: false }) } }),
  HA("projects", "projects with rows, threads, computers and defaults", ["projects"], { replies: { ...recorded("projects", "rows"), ...recorded("threads", "rows"), "places.list": PLACES_TWO } }),
  HA("projects", "projects with rows when the places read is refused", ["projects"], { replies: { ...recorded("projects", "rows"), ...recorded("threads", "rows") } }),
  HA("projects", "threads as a tree", ["threads", "--tree"], { replies: recorded("threads", "a lead's child in a box folder") }),
  HA("slate", "state values JSON reads as V8 does", ["slate", "state", "$big=12345678901234567890", "$huge=1e400", "$neg=-0", "$exp=0.1e1", "$lead=01", "$pad= 1 ", "$str=\"q\"", "$deep={\"b\":1,\"a\":[2]}"], { replies: { "slates.state": JSON.stringify({ id: 1, ok: true, version: 2, text: "s" }) } }),
  HA("slate", "write JSON with a duplicate key and a big number", ["slate", "write", "dup.json"], { files: { "dup.json": "{\"schema\":1,\"a\":1,\"a\":2,\"n\":12345678901234567890,\"z\":0.1e1}" }, replies: { "slates.write": JSON.stringify({ id: 1, ok: true, version: 1, text: "s" }) } }),
  HA("slate", "write JSON with a trailing comma", ["slate", "write", "comma.json"], { files: { "comma.json": "{\"a\":1,}" } }),
  HA("slate", "write an empty JSON file", ["slate", "write", "empty.json"], { files: { "empty.json": "" } }),
  HA("activity", "end a hex pid and a hex start", ["activity", "end", "0x10", "--started", "0x1F"], { replies: { "activity.end": ENDED } }),
  HA("activity", "end an exponent pid", ["activity", "end", "1e3", "--started", "1760000000000"], { replies: { "activity.end": ENDED } }),
  HA("activity", "end a padded pid", ["activity", "end", " 7 ", "--started", "5"], { replies: { "activity.end": ENDED } }),
  HA("activity", "end a blank start reads as 0", ["activity", "end", "7", "--started", " "], { replies: { "activity.end": ENDED } }),
  HA("activity", "end an infinite start", ["activity", "end", "7", "--started", "Infinity"]),
  HA("activity", "end a negative pid", ["activity", "end", "--started", "1", "--", "-4"]),
  HA("slate", "a hex row", ["slate", "write", "--press", "p", "--row", "0x2"], { replies: { "slates.write": JSON.stringify({ id: 1, ok: true, version: 1, text: "s" }) } }),
  HA("slate", "a blank if-version reads as 0", ["slate", "write", "a.slate", "--if-version="], { files: { "a.slate": "<slate/>" }, replies: { "slates.write": JSON.stringify({ id: 1, ok: true, version: 1, text: "s" }) } }),
  HA("slate", "catalog of one entry under a token", ["slate", "catalog", "runs"], { env: { WSP_TURN: "tok-9" }, replies: { "slates.catalog": JSON.stringify({ id: 1, ok: true, text: "runs: ..." }) } }),
  HA("agents_skills", "search a hex limit", ["skills", "search", "lint", "--limit", "0x10"], { replies: { "skills.search": JSON.stringify({ id: 1, ok: true, skills: [] }) } }),
  HA("agents_skills", "search two words as one query", ["skills", "search", "rust", "lint"], { replies: { "skills.search": JSON.stringify({ id: 1, ok: true, skills: [] }) } }),
  HA("agents_skills", "search with no words", ["skills", "search"], { replies: { "skills.search": JSON.stringify({ id: 1, ok: false, error: "skills search takes words to search for" }) } }),
  H("activity", "two threads", "activity a b"),
  H("activity", "end with no pid", "activity end --started 1"),
  H("activity", "end with a word for a pid", "activity end x --started 1"),
  H("activity", "end with pid 0", "activity end 0 --started 1"),
  H("activity", "end with a fraction for a pid", "activity end 1.5 --started 1"),
  H("activity", "end with two pids", "activity end 5 6 --started 1"),
  H("activity", "end without --started", "activity end 5"),
  H("activity", "end with an empty --started", "activity end 5 --started="),
  H("activity", "end with a word for --started", "activity end 5 --started abc"),
  H("activity", "end with a fractional --started", "activity end 5 --started 1.5", { replies: { "activity.end": JSON.stringify({ id: 1, ok: true, pid: 5, ended: [], forced: [], left: [] }) } }),
  H("activity", "wide over nothing", "activity --wide", { replies: { "activity.read": JSON.stringify({ id: 1, ok: true, activity: { computer: "lab", at: 1, threads: [], wsp: [], unowned: [] } }) } }),
  H("activity", "conversations of two projects", "conversations a b"),
  H("activity", "conversations with none", "conversations", { replies: { "conversations.list": JSON.stringify({ id: 1, ok: true, rows: [], held: [] }) } }),
  H("slate", "catalog of two names", "slate catalog a b"),
  H("slate", "write with nothing", "slate write"),
  H("slate", "write with three words", "slate write a b c"),
  H("slate", "write a thread and no file", "slate write t1"),
  H("slate", "write a file of another kind", "slate write notes.txt"),
  H("slate", "write a file that is not there", "slate write missing.slate"),
  H("slate", "write JSON that does not parse", "slate write bad.json", { files: { "bad.json": "{nope" } }),
  H("slate", "write JSON that is no object", "slate write list.json", { files: { "list.json": "[1,2]" } }),
  H("slate", "write JSON null", "slate write null.json", { files: { "null.json": "null" } }),
  H("slate", "row without press", "slate write --row 1 --check"),
  H("slate", "a negative row", "slate write --press p --row=-1"),
  H("slate", "a fractional action", "slate write --press p --action 1.5"),
  H("slate", "set without =", "slate write --set nope"),
  H("slate", "set with = first", "slate write --set =1"),
  H("slate", "if-version a word", "slate write a.slate --if-version abc", { files: { "a.slate": "<slate/>" } }),
  H("slate", "if-version negative", "slate write a.slate --if-version=-1", { files: { "a.slate": "<slate/>" } }),
  H("slate", "write a .slate by its own name, values as text and JSON", "slate write a.slate --set $a=2 --set $b=two --set c={\"x\":1} --if-version 0", { files: { "a.slate": "<slate>hi</slate>" }, replies: { "slates.write": JSON.stringify({ id: 1, ok: true, version: 3, text: "sketch", warnings: [] }) } }),
  H("slate", "write a .json document", "slate write doc.json --check", { files: { "doc.json": "{\"schema\":1,\"root\":\"r\",\"pieces\":{}}" }, replies: { "slates.write": JSON.stringify({ id: 1, ok: true, version: 3, text: "sketch", problems: [] }) } }),
  H("slate", "write a press rehearsal with no file", "slate write --press go --row 0 --action 2", { replies: { "slates.write": JSON.stringify({ id: 1, ok: true, version: 1, text: "s" }) } }),
  H("slate", "write under a turn token", "slate write a.slate", { files: { "a.slate": "<slate/>" }, env: { WSP_TURN: "tok-1" }, replies: { "slates.write": JSON.stringify({ id: 1, ok: true, version: 1, text: "s" }) } }),
  H("slate", "state with nothing", "slate state"),
  H("slate", "state with two loose words", "slate state a b $x=1"),
  H("slate", "state with a start only", "slate state --start r1 --start r2", { replies: { "slates.state": JSON.stringify({ id: 1, ok: true, version: 2, text: "s", notStarted: [] }) } }),
  H("slate", "state values as JSON and text", "slate state $a=1 $b=x $c=[1] i=", { replies: { "slates.state": JSON.stringify({ id: 1, ok: true, version: 2, text: "s" }) } }),
  H("slate", "state if-version a word", "slate state $a=1 --if-version x"),
  H("slate", "read of two threads", "slate read a b"),
  H("slate", "read every switch", "slate read --values $a --values * --no-text --no-sketch --document", { replies: { "slates.read": JSON.stringify({ id: 1, ok: true, version: 2, text: "s", values: {} }) } }),
  H("projects", "projects with a word", "projects x"),
  H("projects", "projects with none", "projects", { replies: { "projects.list": JSON.stringify({ id: 1, ok: true, projects: [] }) } }),
  H("projects", "remove with none", "projects remove"),
  H("projects", "remove with two", "projects remove a b"),
  H("projects", "set with none", "projects set"),
  H("projects", "set with two", "projects set a b"),
  H("projects", "seed with none", "projects seed"),
  H("projects", "seed with two", "projects seed a b"),
  H("projects", "threads with two", "threads a b"),
  H("projects", "wait with none", "threads wait"),
  H("projects", "wait with a word for a timeout", "threads wait t --timeout soon"),
  H("computers", "computers with a word", "computers x"),
  H("computers", "set with none", "computers set"),
  H("computers", "set with two", "computers set a b"),
  H("computers", "set threads 0", "computers set a --threads 0"),
  H("computers", "set threads a word", "computers set a --threads many"),
  H("computers", "set a negative spend", "computers set a --spend=-1"),
  H("computers", "set nap a word", "computers set a --nap soon"),
  H("computers", "set turn limit a word", "computers set a --turn-limit long"),
  H("computers", "set delete-after a word", "computers set a --delete-after later"),
  H("computers", "set a reset off the list", "computers set a --reset colour"),
  H("computers", "set max-depth 0", "computers set a --max-depth 0"),
  H("computers", "recipes with a word", "recipes x"),
  H("computers", "recipes show with none", "recipes show"),
  H("computers", "recipes save without --from", "recipes save lab"),
  H("computers", "recipes remove with none", "recipes remove"),
  H("computers", "recipes remove off a terminal", "recipes remove lab", { replies: { "recipes.remove": JSON.stringify({ id: 1, ok: true }) } }),
  H("computers", "usage with a word", "usage x"),
  H("computers", "usage a range off the list", "usage --range year"),
  H("computers", "usage a split off the list", "usage --by colour"),
  H("computers", "usage reset with none", "usage reset"),
  H("computers", "usage reset with two", "usage reset a b"),
  H("agents_skills", "agents of two threads", "agents a b"),
  H("agents_skills", "skills of two threads", "skills a b"),
  H("agents_skills", "search limit 0", "skills search lint --limit 0"),
  H("agents_skills", "search limit 51", "skills search lint --limit 51"),
  H("agents_skills", "search limit a fraction", "skills search lint --limit 1.5"),
  H("agents_skills", "show with none", "skills show"),
  H("agents_skills", "show with three", "skills show a b c"),
  H("agents_skills", "add with none", "skills add"),
  H("agents_skills", "remove with none", "skills remove"),
  H("agents_skills", "disable with none", "skills disable"),
  H("agents_skills", "enable with three", "skills enable a b c"),
  H("agents_servers", "servers of two threads", "servers a b"),
  H("agents_servers", "signin with none", "servers signin"),
  H("agents_servers", "signin without --agent", "servers signin linear"),
  H("agents_servers", "tools without --agent", "servers tools linear"),
  H("agents_servers", "add without --agent", "servers add linear"),
  H("agents_servers", "remove without --agent", "servers remove linear"),
  H("agents_servers", "disable without --agent", "servers disable linear"),
  H("agents_servers", "enable with three", "servers enable a b c --agent claude"),
  H("agents_servers", "plugins of two threads", "plugins a b"),
  H("agents_servers", "plugin disable without --agent", "plugins disable p@m"),
  H("agents_servers", "agents signin with none", "agents signin"),
  H("agents_servers", "agents key with none", "agents key"),
  H("agents_servers", "agents key off a terminal", "agents key claude"),
  H("agents_servers", "addtools with none", "agents addtools"),
  H("agents_servers", "default with two", "agents default a b"),
  H("agents_servers", "set with none", "agents set"),
  H("agents_servers", "setup with none", "agents setup"),
  H("agents_servers", "setup on and off", "agents setup claude --enable --disable"),
  H("agents_servers", "setup a variable off a terminal", "agents setup claude --env FOO"),
  H("agents_servers", "setup a variable wsp never sets", "agents setup claude --env PATH"),
  H("workspaces_git", "fork with none", "fork", { cloud: true }),
  H("workspaces_git", "fork a pick without --send", "fork w --model m", { cloud: true }),
  H("workspaces_git", "rename with one", "rename w", { cloud: true }),
  H("workspaces_git", "commit with none", "commit"),
  H("workspaces_git", "discard without a path", "discard t"),
  H("workspaces_git", "fix a check and a child", "fix t --check ci --child c"),
  H("workspaces_git", "merge in with one", "merge in a"),
  H("workspaces_git", "merge a method off the list", "merge t --method octopus"),
  H("workspaces_life", "start with none", "start"),
  H("workspaces_life", "review with two", "review a b"),
  H("workspaces_life", "post a verdict off the list", "review post t --verdict meh"),
  H("workspaces_life", "update with none", "update"),
  H("workspaces_life", "pause with none", "pause", { cloud: true }),
  H("workspaces_life", "resize without --size", "resize t"),
  H("workspaces_life", "resize a word that names no size", "resize t --size big"),
  H("workspaces_life", "wake with none", "wake", { cloud: true }),
  H("workspaces_life", "rebuild with none", "rebuild", { cloud: true }),
  H("workspaces_life", "image with a word", "image x"),
  H("workspaces_life", "image build with none", "image build"),
  H("workspaces_life", "forget with none", "forget", { cloud: true }),
  H("workspaces_life", "delete with none", "delete"),
  H("workspaces_life", "delete with none, cloud on", "delete", { cloud: true }),
  H("workspaces_life", "delete processes off the list", "delete t --processes maybe"),
  H("workspaces_life", "delete a thread nobody holds", "delete nope", { replies: { "sessions.list": JSON.stringify({ id: 1, ok: true, sessions: [] }), "workspaces.list": JSON.stringify({ id: 1, ok: true, workspaces: [] }) } }),
];
const HELP_OF: Record<string, string[]> = Object.fromEntries(Object.entries(UNIT).map(([u, tools]) => [u, tools.map(t => SPECS[t]!.verb)]));
HELP_OF["computers"]!.push("usage reset");
HELP_OF["agents_servers"]!.push("servers signin", "agents signin", "agents key");

const strip = (r: Run & { cwd?: string; asked?: string[] }) => ({ frames: r.frames, stdout: r.stdout, stderr: r.stderr, ...(r.streamed !== "" ? { streamed: r.streamed } : {}), exit: r.exit, ...(r.asked !== undefined ? { asked: r.asked } : {}) });

it("records cli-b's lines", async () => {
  mkdirSync(OUT, { recursive: true });
  const only = process.env.UNITS?.split(",");
  for (const [unit, tools] of Object.entries(UNIT)) {
    if (only !== undefined && !only.includes(unit)) continue;
    const cases: unknown[] = [];
    for (const tool of tools) {
      const recorded = JSON.parse(readFileSync(join(ANSWERS, `${tool}.json`), "utf8")) as { cases: Record<string, unknown>[] };
      for (const c of recorded.cases) {
        if (c["guest"] !== undefined) continue;
        const files: Record<string, string> = {};
        const argv = argvOf(tool, c["arguments"] as Args, files);
        const o = { cloud: c["cloud"] === true, env: (c["env"] as Record<string, string>) ?? {}, pushed: (c["pushed"] as Record<string, string[]>) ?? {}, files };
        const replies = c["replies"] as Record<string, string>;
        const text = await runLine(argv, replies, o);
        const json = await runLine([...argv, "--json"], replies, o);
        const resultOf = (line: string) => (JSON.parse(line) as { result?: unknown }).result;
        cases.push({
          tool,
          case: c["case"],
          argv,
          ...(o.cloud ? { cloud: true } : {}),
          ...(Object.keys(o.env).length > 0 ? { env: o.env } : {}),
          ...(Object.keys(files).length > 0 ? { files } : {}),
          replies,
          ...(Object.keys(o.pushed).length > 0 ? { pushed: o.pushed } : {}),
          tool_arguments: c["arguments"],
          tool_asked: c["asked"],
          tool_result: resultOf(c["line"] as string),
          text: strip(text),
          json: strip(json),
        });
      }
    }
    for (const h of HAND.filter(h => h.unit === unit)) {
      const o = { cloud: h.cloud === true, env: h.env ?? {}, pushed: {}, files: h.files ?? {}, ...(h.tty !== undefined ? { tty: h.tty } : {}) };
      const replies = h.replies ?? {};
      cases.push({ hand: true, case: h.case, argv: h.argv, ...(h.tty !== undefined ? { tty: h.tty } : {}), ...(o.cloud ? { cloud: true } : {}), ...(h.env ? { env: h.env } : {}), ...(h.files ? { files: h.files } : {}), replies, text: strip(await runLine(h.argv, replies, o)), json: strip(await runLine([...h.argv, "--json"], replies, o)) });
    }
    const pages: Record<string, unknown> = {};
    for (const cloud of [false, true]) for (const verb of HELP_OF[unit]!) {
      const r = await runLine([...verb.split(" "), "--help"], {}, { cloud, env: {}, pushed: {}, files: {} }).catch(e => ({ error: String(e) }));
      pages[`${cloud ? "cloud" : "local"} ${verb}`] = "stdout" in r ? { stdout: r.stdout, exit: r.exit } : r;
    }
    writeFileSync(join(OUT, `${unit}.pages.json`), `${JSON.stringify(pages, null, 1)}\n`);
    writeFileSync(join(OUT, `${unit}.json`), `${JSON.stringify({ source: "TS command line at 65f6b89c7 against daemon/crates/wsp-mcp/tests/answers; recorded by the cli-b recorder", cases }, null, 1)}\n`);
    console.log(unit, cases.length);
  }
});
