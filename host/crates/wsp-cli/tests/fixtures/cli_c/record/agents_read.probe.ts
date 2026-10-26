import { it } from "vitest";
import { frame, record, recorded, refused, type Case, type Obj, type Run } from "./lib.ts";

// Unit agents_read: agents-help.ts:1-298 at 65f6b89c7 (the target a line names, the report and the rows of its four
// lists, the plugin and server writes, sign-in's line). Every report or row the TS hands back parsed is written in the
// key order of the schema, so the Rust value serialises the same way.

const HERE = { placeId: "here" };
const lab = (s: string): string => JSON.stringify(s).replace(/[^\x20-\x7e]/g, "?");
const ordered = (keys: string[], o: Obj): Obj => Object.fromEntries(keys.filter(k => o[k] !== undefined).map(k => [k, o[k]]));
const AGENT_KEYS = ["id", "name", "installed", "version", "latest", "pinned", "road", "installsOnFirstRun", "versionUnread", "path", "via", "signIn", "signInRoad", "wspTools", "signInDetail", "update"];
const SKILL_KEYS = ["name", "description", "paths", "scope", "project", "plugin"];
const MCP_KEYS = ["agent", "name", "scope", "file", "launch", "transport", "envNames", "auth", "enabled", "inRecipe", "tools", "project", "signInLine", "off"];
const PLUGIN_KEYS = ["agent", "id", "name", "marketplace", "version", "scope", "project", "on", "missing", "path", "setIn", "description", "source", "brings"];
const NONE = { skills: [], commands: [], subagents: [], hooks: [], servers: [], lsp: [], apps: [] };

const agent = (o: Obj = {}): Obj => ordered(AGENT_KEYS, { id: "claude", name: "Claude Code", installed: true, version: "2.1.0", road: "wsp", path: "/usr/local/bin/claude", signIn: "signed-in", signInRoad: "device", wspTools: true, ...o });
const skill = (o: Obj = {}): Obj => ordered(SKILL_KEYS, { name: "unslop", paths: [{ path: "~/.claude/skills/unslop" }], scope: "user", ...o });
const server = (o: Obj = {}): Obj => ordered(MCP_KEYS, { agent: "claude", name: "linear", scope: "user", file: "~/.claude.json", transport: { kind: "http", host: "mcp.linear.app" }, envNames: [], auth: "open", enabled: true, ...o });
const plugin = (o: Obj = {}): Obj => ordered(PLUGIN_KEYS, { agent: "claude", id: "vercel@claude-plugins-official", name: "vercel", marketplace: "claude-plugins-official", scope: "user", on: true, brings: NONE, ...o });
const PROJECT = { id: "proj-1", name: "wsp", path: "~/p" };
const report = (o: Obj = {}): Obj => ordered(["target", "home", "user", "readAt", "stale", "pauseMode", "agents", "skills", "servers", "plugins", "refused", "projects", "reach"], { target: HERE, home: "/root", user: "root", readAt: "2026-09-27T10:00:00.000Z", agents: [], skills: [], servers: [], refused: [], ...o });

const cases: Case[] = [];
const add = (c: Case): void => void cases.push(c);

// ---- constants and one-line words
add({ fn: "constAimedBothLine", case: "the line" });
add({ fn: "constProjectUnnamedLine", case: "the line" });
add({ fn: "constToolsProjectBareLine", case: "the line" });
add({ fn: "constPluginKindWords", case: "kinds, one and many" });
add({ fn: "constServerToolColumns", case: "the two columns" });
for (const tool of ["servers", "plugins", "", "a b\n🧪"]) add({ fn: "aimedUsage", case: `tool ${lab(tool)}`, args: { tool } });
for (const value of ["wsp", "", "a b", "\"q\" 🧪"]) add({ fn: "projectOffComputerLine", case: `value ${lab(value)}`, args: { value } });
for (const [id, on, agentId] of [["vercel@m", true, "claude"], ["vercel@m", false, "claude"], ["x", true, "codex"], ["x", false, "not-in-catalog"], ["", true, ""]] as [string, boolean, string][]) {
  add({ fn: "pluginTurnedWords", case: `${id || "empty"} ${on ? "on" : "off"} for ${agentId || "empty"}`, args: { id, on, agent: agentId } });
}
for (const [on, agentId] of [[true, "claude"], [false, "claude"], [true, "codex"], [false, "not-in-catalog"]] as [boolean, string][]) {
  add({ fn: "pluginTurnedLine", case: `${on ? "on" : "off"} for ${agentId}`, args: { row: plugin({ agent: agentId, on }) } });
}
for (const variable of ["LINEAR_TOKEN", "", "a b\n🧪"]) add({ fn: "unsetVariableLine", case: `variable ${lab(variable)}`, args: { variable } });
for (const pair of ["Authorization", "=VAR", "Name=", "", "a b\n🧪"]) add({ fn: "notHeaderLine", case: `pair ${lab(pair)}`, args: { pair } });
for (const word of ["user", "home", "", "a b\n🧪"]) add({ fn: "projectScopeLine", case: `word ${lab(word)}`, args: { word } });
for (const [a, file] of [["Claude Code", "~/.claude.json"], ["Codex", ""], ["", "~/x"], ["a\n🧪", "~/é"]]) add({ fn: "toolsAddedLine", case: `agent ${lab(a)} file ${lab(file)}`, args: { agent: a, file } });
for (const [what, where, answer] of [
  ["Claude Code", "attic", { signedIn: true }],
  ["Claude Code", "attic", { signedIn: true, detail: "OAuth credentials" }],
  ["Claude Code", "attic", { signedIn: true, detail: "" }],
  ["Codex", "this computer", { signedIn: false }],
  ["Codex", "this computer", { signedIn: false, said: "Not logged in" }],
  ["Codex", "this computer", { signedIn: false, said: "" }],
  ["linear", "attic", { signedIn: true, detail: "a (b)", said: "ignored" }],
  ["", "", { signedIn: false, said: "x\ny" }],
] as [string, string, Obj][]) add({ fn: "signedInLine", case: `${what || "empty"} ${answer["signedIn"] ? "in" : "out"} ${Object.keys(answer).join("+")}`, args: { what, where, answer } });

// ---- projectAsked, toolsProject
const pa = (name: string, value: unknown, thread?: string, on?: string): void => void add({ fn: "projectAsked", case: name, args: { ...(value !== undefined ? { value } : {}), ...(thread !== undefined ? { thread } : {}), ...(on !== undefined ? { on } : {}), usage: "wsp skills [--project]" } });
pa("no value", undefined);
pa("false", false);
pa("false with a computer", false, undefined, "attic");
pa("true is the thread's own project", true, "t-1");
pa("true with nothing named", true);
pa("true with a computer is refused", true, undefined, "attic");
pa("true with a thread and a computer is refused", true, "t-1", "attic");
pa("the empty string reads as true", "");
pa("the empty string with a computer is refused", "", undefined, "attic");
pa("a name with a computer", "wsp", undefined, "attic");
pa("a name with a computer and a thread", "wsp", "t-1", "attic");
pa("a name with no computer is refused", "wsp");
pa("a name with a thread and no computer is refused", "wsp", "t-1");
pa("a name with an emoji and quotes off a computer", "a \"b\" 🧪");
const tp = (name: string, value: string | undefined, thread?: string, on?: string): void => void add({ fn: "toolsProject", case: name, args: { ...(value !== undefined ? { value } : {}), ...(thread !== undefined ? { thread } : {}), ...(on !== undefined ? { on } : {}), usage: "wsp servers tools" } });
tp("no value", undefined);
tp("no value with a computer", undefined, undefined, "attic");
tp("the empty string is refused", "");
tp("the empty string with a computer is refused the same", "", undefined, "attic");
tp("a name with a computer", "wsp", undefined, "attic");
tp("a name with no computer is refused", "wsp");
tp("a name with a thread and a computer", "wsp", "t-1", "attic");

// ---- agentsTarget
const thr = recorded("agents", "a thread");
const comp = recorded("agents", "a computer");
const U = "wsp agents [thread]";
const at = (name: string, args: Obj, script: NonNullable<Case["script"]>, more: Partial<Case> = {}): void => void add({ fn: "agentsTarget", case: name, args: { usage: U, line: "wsp agents", ...args }, script, ...more });
at("neither names a thread nor a computer", {}, {});
at("neither, with a project asked, still names this computer", { project: "wsp" }, {});
at("both are refused before any frame", { thread: "t-1", on: "attic" }, { replies: { ...thr, ...comp } });
at("a thread by its id prefix", { thread: "t-1" }, { replies: thr });
at("a thread by its whole id", { thread: "t-1 one" }, { replies: thr });
at("a thread also asked with a project name is the workspace alone", { thread: "t-1", project: "wsp" }, { replies: thr });
at("a thread that names nothing", { thread: "zzz" }, { replies: { ...thr, "projects.list": frame({ projects: [] }) } });
at("a thread that names nothing, the cloud on", { thread: "zzz" }, { replies: { ...thr, "projects.list": frame({ projects: [] }), "workspaces.list": frame({ workspaces: [] }) } }, { cloud: true });
at("a word that is a project's name", { thread: "wsp" }, { replies: { "sessions.list": frame({ sessions: [] }), "projects.list": frame({ projects: [{ id: "proj-1", name: "wsp", path: "/w", computer: "place-9" }]  }) } });
at("a word that is a project's name, the cloud on", { thread: "wsp" }, { replies: { "sessions.list": frame({ sessions: [] }), "projects.list": frame({ projects: [{ id: "proj-1", name: "wsp", path: "/w", computer: "place-9" }] }) } }, { cloud: true });
at("a word that is a machine's name, the cloud on", { thread: "attic-work" }, { replies: { "sessions.list": frame({ sessions: [] }), "projects.list": frame({ projects: [] }), "workspaces.list": frame({ workspaces: [{ id: "ws-9", name: "attic-work", machineId: "m-1", phase: "running", kind: "cloud", golden: "default", createdAt: "2026-09-27T00:00:00.000Z", project: { id: "p-1", name: "wsp", path: "/root/wsp", computer: "here" }, home: "/root" }] }) } }, { cloud: true });
at("a word that is a machine's name, the cloud off", { thread: "attic-work" }, { replies: { "sessions.list": frame({ sessions: [] }), "projects.list": frame({ projects: [] }), "workspaces.list": frame({ workspaces: [{ id: "ws-9", name: "attic-work", machineId: "m-1", phase: "running", kind: "cloud", golden: "default", createdAt: "2026-09-27T00:00:00.000Z", project: { id: "p-1", name: "wsp", path: "/root/wsp", computer: "here" }, home: "/root" }] }) } });
at("a thread whose workspace does not read is another version", { thread: "t-1" }, { replies: { ...thr, "workspaces.get": frame({ workspace: { id: "ws-1" } }) } });
at("a computer by its name", { on: "attic" }, { replies: comp });
at("a computer with a project", { on: "attic", project: "wsp" }, { replies: comp });
at("a computer with an empty project name is sent as given", { on: "attic", project: "" }, { replies: comp });
at("this computer by the name it carries", { on: "this mac" }, { replies: comp });
at("a computer by its id", { on: "place-9" }, { replies: comp });
at("a computer that is none", { on: "nowhere" }, { replies: comp });
at("the computers are not readable", { on: "attic" }, { replies: { "places.list": refused("places are not readable") } });
at("an empty computer name", { on: "" }, { replies: comp });

// ---- agentsReport
const ar = (name: string, args: Obj, script: NonNullable<Case["script"]>, more: Partial<Case> = {}): void => void add({ fn: "agentsReport", case: name, args: { usage: U, line: "wsp agents", ...args }, script, ...more });
const read = (r: Obj): string => frame({ report: r });
const FULL = report({
  stale: "napping",
  pauseMode: "memory",
  agents: [agent(), agent({ id: "codex", name: "Codex", installed: false, version: undefined, road: "none", path: undefined, signIn: "none", signInRoad: "code", wspTools: false })],
  skills: [skill({ description: "cut the tells", paths: [{ path: "~/.claude/skills/unslop", agent: "claude", linkTo: "~/.agents/skills/unslop" }, { path: "~/.agents/skills/unslop" }] })],
  servers: [server({ inRecipe: false }), server({ agent: "codex", name: "wsp", scope: "project", file: "~/p/.codex/config.toml", transport: { kind: "stdio", line: "wsp mcp --scoped" }, envNames: ["WSP_HOST_URL"], enabled: false, project: PROJECT })],
  plugins: [plugin({ version: "0.50.0", path: "~/.claude/plugins/cache/x", source: "anthropics/claude-plugins-official", brings: { ...NONE, skills: ["vercel:nextjs"], hooks: ["SessionStart"] } })],
  refused: ["codex: config.toml did not parse\nat line 3"],
  projects: [PROJECT],
  reach: "here",
});
ar("this computer, every part", {}, { replies: { "agents.read": read(FULL) } });
ar("a report with only what is required", {}, { replies: { "agents.read": read(report()) } });
ar("a thread", { thread: "t-1" }, { replies: { ...thr, "agents.read": read(report({ target: { workspaceId: "ws-1" } })) } });
ar("a computer", { on: "attic" }, { replies: { ...comp, "agents.read": read(report({ target: { placeId: "place-9" } })) } });
ar("a napping workspace's kept report", { thread: "t-1" }, { replies: { ...thr, "agents.read": read(report({ target: { workspaceId: "ws-1" }, stale: "napping", pauseMode: "disk" })) } });
ar("both a thread and a computer are refused", { thread: "t-1", on: "attic" }, { replies: { ...thr, ...comp } });
ar("the host refuses the read", {}, { replies: { "agents.read": refused("this computer is not readable", "usage") } });
ar("the read is refused for a napping machine", { thread: "t-1" }, { replies: { ...thr, "agents.read": refused("parser is napping and was not read while it ran; wake it to read what stands there", "usage") } });
ar("a project word is refused in the thread's place", { thread: "wsp" }, { replies: { "sessions.list": frame({ sessions: [] }), "projects.list": frame({ projects: [{ id: "proj-1", name: "wsp", path: "/w", computer: "place-9" }] }) } });
ar("keys this build does not know are dropped", {}, { replies: { "agents.read": frame({ report: { ...report(), unknownTop: { anything: 1 } } }) } });

// ---- reportFacts
const rf = (name: string, r: Obj): void => void add({ fn: "reportFacts", case: name, args: { report: r } });
rf("only what is required", report());
rf("every optional fact", report({ stale: "napping", pauseMode: "memory", projects: [PROJECT], reach: "relay", refused: ["a", "b"] }));
rf("stale without a pause mode", report({ stale: "napping" }));
rf("a pause mode without stale", report({ pauseMode: "disk" }));
rf("projects that are none", report({ projects: [] }));
rf("a thread's target", report({ target: { workspaceId: "ws-1" } }));
rf("a computer's target with a project", report({ target: { placeId: "place-9", project: "proj-1" } }));
rf("rows are left out", report({ agents: [agent()], skills: [skill()], servers: [server()], plugins: [plugin()] }));

// ---- agentRowLines
const rowsCase = (fn: string, name: string, r: Obj): void => void add({ fn, case: name, args: { report: r } });
const A = (name: string, agents: Obj[], extra: Obj = {}): void => rowsCase("agentRowLines", name, report({ agents, ...extra }));
A("no agents leaves the head alone", []);
A("one signed in agent", [agent()]);
A("the four rows of the recorded report", [agent({ latest: "2.2.0", pinned: "2.1.0", update: { command: "claude update ", to: "2.2.0" }, signInDetail: "OAuth credentials" }), agent({ id: "codex", name: "Codex ", installed: false, version: undefined, road: "none", path: undefined, signIn: "none", signInRoad: "code", wspTools: false }), agent({ id: "gemini", name: "Gemini", version: "1.0", road: "own", path: "/opt/🧪/gemini", signIn: "unknown", signInRoad: "terminal", wspTools: false }), agent({ id: "amp", name: "Amp", version: undefined, road: "shim", path: undefined, signIn: "vault-key", signInRoad: "key" })]);
A("not installed reads not found whatever the sign-in", [agent({ installed: false, signIn: "signed-in" })]);
A("not installed with an unknown sign-in", [agent({ installed: false, signIn: "unknown" })]);
A("an installed agent with an unknown sign-in", [agent({ signIn: "unknown" })]);
A("an installed agent that is not signed in", [agent({ signIn: "none" })]);
A("an installed agent on a vault key", [agent({ signIn: "vault-key" })]);
A("a version wins over the first-run word", [agent({ installsOnFirstRun: true })]);
A("no version, installs on first run", [agent({ version: undefined, installsOnFirstRun: true })]);
A("no version, version unreadable", [agent({ version: undefined, versionUnread: true })]);
A("no version and both words, first run wins", [agent({ version: undefined, installsOnFirstRun: true, versionUnread: true })]);
A("no version at all", [agent({ version: undefined })]);
A("an empty version is a version", [agent({ version: "" })]);
A("no latest", [agent({ latest: undefined })]);
A("a latest", [agent({ latest: "2.2.0" })]);
A("an update with its command", [agent({ update: { to: "2.2.0", command: "claude update" } })]);
A("no update", [agent({ update: undefined })]);
A("no path", [agent({ path: undefined })]);
A("wsp tools off", [agent({ wspTools: false })]);
A("a name wider than the head", [agent({ name: "A very long agent name indeed", id: "x" })]);
A("an emoji in a name counts two cells", [agent({ name: "🧪 Lab", id: "x" }), agent({ name: "Plain", id: "y" })]);
A("a path with accents", [agent({ path: "/opt/é/claude" }), agent({ id: "y", name: "Y", path: undefined })]);
A("an agent id the catalog does not know prints its own name", [agent({ id: "not-in-catalog", name: "Not In Catalog" })]);
A("a stale report that is napping", [agent()], { stale: "napping" });
A("a stale report that naps in memory", [agent()], { stale: "napping", pauseMode: "memory" });
A("a stale report that naps on disk", [agent()], { stale: "napping", pauseMode: "disk" });
A("a pause mode without stale says nothing", [agent()], { pauseMode: "memory" });
A("a refused reader", [agent()], { refused: ["codex: config.toml did not parse"] });
A("two refused readers in order", [agent()], { refused: ["b: second", "a: first"] });
A("a refused line is made a cell", [agent()], { refused: ["tab\there\nnew line\u0007bell"] });
A("stale then refused, in that order", [], { stale: "napping", pauseMode: "memory", refused: ["x"] });

// ---- skillRowLines
const S = (name: string, skills: Obj[], extra: Obj = {}): void => rowsCase("skillRowLines", name, report({ skills, ...extra }));
S("no skills", []);
S("a user skill", [skill()]);
S("the recorded three", [skill({ description: "cut the tells", paths: [{ path: "~/.claude/skills/unslop", agent: "claude", linkTo: "~/.agents/skills/unslop" }, { path: "~/.agents/skills/unslop" }] }), skill({ name: "tab\there\nnew line", paths: [{ path: "~/p/.claude/skills/x", off: true }], scope: "project", project: { id: "proj-1", name: "wsp \u0007bell", path: "~/p" } }), skill({ name: "plug", paths: [{ path: "~/.claude/plugins/p/skills/plug" }], scope: "plugin" })]);
S("a project skill names its project", [skill({ scope: "project", project: PROJECT })]);
S("a skill with a project but a user scope reads project", [skill({ scope: "user", project: PROJECT })]);
S("a skill with a project and a plugin scope reads project", [skill({ scope: "plugin", project: PROJECT })]);
S("a plugin skill", [skill({ scope: "plugin", plugin: "vercel@m" })]);
S("two folders joined by a comma", [skill({ paths: [{ path: "~/a" }, { path: "~/b" }] })]);
S("a link shows its target", [skill({ paths: [{ path: "~/a", linkTo: "~/b" }] })]);
S("a link and a plain folder", [skill({ paths: [{ path: "~/a", linkTo: "~/b" }, { path: "~/c" }] })]);
S("an empty link target is still a link", [skill({ paths: [{ path: "~/a", linkTo: "" }] })]);
S("a path with control characters is a cell", [skill({ paths: [{ path: "~/a\tb\nc\u0007" }] })]);
S("a name with a newline", [skill({ name: "one\ntwo" })]);
S("a project name with control characters", [skill({ scope: "project", project: { id: "p", name: "a\tb\u0007c", path: "~/p" } })]);
S("a wide name against the head", [skill({ name: "a-very-long-skill-name-indeed" }), skill({ name: "short" })]);
S("an emoji name counts two cells", [skill({ name: "🧪" }), skill({ name: "abc" })]);
S("a napping report", [skill()], { stale: "napping", pauseMode: "memory" });
S("no skills and a refusal", [], { refused: ["claude: skills unreadable"] });
S("a description is not printed", [skill({ description: "never shown" })]);

// ---- serverRowLines
const V = (name: string, servers: Obj[], extra: Obj = {}): void => rowsCase("serverRowLines", name, report({ servers, ...extra }));
V("no servers", []);
V("an http server", [server()]);
V("a stdio server", [server({ transport: { kind: "stdio", line: "node k.js" } })]);
V("the recorded four", [server({ auth: "unknown", inRecipe: false }), server({ agent: "codex", name: "wsp", scope: "project", file: "~/p/.codex/config.toml", transport: { kind: "stdio", line: "wsp mcp --scoped" }, envNames: ["WSP_HOST_URL", "WSP_HOST_TOKEN"], enabled: false, project: PROJECT }), server({ name: "kept", scope: "local", file: "~/.claude-cfg/.claude.json", transport: { kind: "stdio", line: "node k.js" }, project: PROJECT }), server({ agent: "not-in-catalog", name: "odd", scope: "home", file: "~/.x", transport: { kind: "stdio", line: "odd é" }, tools: [{ name: "t" }] })]);
V("a launch row", [server({ name: "wsp", scope: "user", file: undefined, launch: true, transport: { kind: "stdio", line: "wsp mcp" } })]);
V("a launch row beats disabled", [server({ file: undefined, launch: true, enabled: false })]);
V("a launch row beats off", [server({ file: undefined, launch: true, off: "needs a sign-in" })]);
V("a launch row beats not in recipe", [server({ file: undefined, launch: true, inRecipe: false })]);
V("disabled", [server({ enabled: false })]);
V("disabled and not in recipe", [server({ enabled: false, inRecipe: false })]);
V("off while enabled", [server({ off: "linear needs your sign-in" })]);
V("off and not in recipe", [server({ off: "linear needs your sign-in", inRecipe: false })]);
V("disabled beats off and prints no off line", [server({ enabled: false, off: "linear needs your sign-in" })]);
V("not in recipe with an auth", [server({ auth: "needs-sign-in", inRecipe: false })]);
V("in recipe says nothing", [server({ inRecipe: true })]);
for (const auth of ["open", "env-key", "connected", "signed-in", "needs-sign-in", "failed", "unknown"]) V(`auth ${auth}`, [server({ auth })]);
V("project scope names its project", [server({ scope: "project", project: PROJECT })]);
V("local scope names its project as local", [server({ scope: "local", project: PROJECT })]);
V("a user scope with a project reads project", [server({ scope: "user", project: PROJECT })]);
V("a home scope", [server({ scope: "home" })]);
V("a project name with control characters", [server({ scope: "project", project: { id: "p", name: "a\tb\u0007c", path: "~/p" } })]);
V("the off sentence once for two rows", [server({ off: "needs a key" }), server({ name: "b", off: "needs a key" })]);
V("two off sentences in row order", [server({ off: "second" }), server({ name: "b", off: "first" }), server({ name: "c", off: "second" })]);
V("an off sentence made a cell", [server({ off: "tab\there\nnew line" })]);
V("off lines come before the stale and refused lines", [server({ off: "needs a key" })], { stale: "napping", pauseMode: "disk", refused: ["x"] });
V("an agent the catalog does not know", [server({ agent: "not-in-catalog" })]);
V("an agent the catalog knows by name", [server({ agent: "codex" })]);
V("an empty off sentence is an off server with an empty line", [server({ off: "" })]);
V("a wide name against the head", [server({ name: "a-very-long-server-name-indeed" }), server({ name: "b" })]);
V("an emoji in a name counts two cells", [server({ name: "🧪" }), server({ name: "abc" })]);
const cloudOff = (name: string): string => `${name} runs only on a computer where you signed in to it; a cloud machine holds no sign-in or key of yours.`;
// agents-reader.test.ts: "wsp servers and the servers tool print these rows: off, the sentence once per server, and no sign-in command"
V("a cloud machine's report: servers that need the person's sign-in are off", [server({ name: "signs", auth: "unknown", off: cloudOff("signs") }), server({ name: "open", auth: "unknown" }), server({ name: "cf", auth: "env-key", off: cloudOff("cf") }), server({ agent: "codex", name: "linear", off: cloudOff("linear") }), server({ name: "keyed", off: cloudOff("keyed") }), server({ name: "aws-mcp", off: cloudOff("aws-mcp") }), server({ name: "homed", auth: "open" })], { target: { workspaceId: "w_cloud" }, readAt: "2026-10-14T00:00:00.000Z" });
V("no servers and a refusal", [], { refused: ["codex: config.toml did not parse"] });

// ---- pluginRowLines, broughtWords
const P = (name: string, plugins: Obj[] | undefined, extra: Obj = {}): void => rowsCase("pluginRowLines", name, report({ ...(plugins !== undefined ? { plugins } : {}), ...extra }));
P("no plugins key reads as none", undefined);
P("an empty list", []);
P("one plugin on", [plugin()]);
P("one plugin off", [plugin({ on: false })]);
P("the recorded four", [plugin({ version: "0.50.0", path: "~/x", source: "anthropics/claude-plugins-official", brings: { skills: ["vercel:nextjs"], commands: ["vercel:deploy", "vercel:env"], subagents: ["vercel:ai-architect"], hooks: ["SessionStart"], servers: ["plugin:vercel:vercel"], lsp: [], apps: [] } }), plugin({ id: "posthog\tlab@lab-local", name: "posthog\tlab", marketplace: "lab-local", scope: "local", project: PROJECT, on: false, missing: "folder", setIn: "~/p/.claude/settings.local.json", brings: { ...NONE, lsp: ["rust-analyzer"] } }), plugin({ agent: "codex", id: "chrome@openai-bundled", name: "chrome", marketplace: "openai-bundled", missing: "marketplace", source: "/x/.codex/.tmp/bundled-marketplaces/openai-bundled" }), plugin({ agent: "not-in-catalog", id: "odd@m", name: "odd", marketplace: "m", on: false, brings: { ...NONE, hooks: ["preToolUse"], apps: ["Gmail "] } })]);
P("missing and on", [plugin({ missing: "folder", on: true })]);
P("missing and off", [plugin({ missing: "marketplace", on: false })]);
P("missing uninstalled", [plugin({ missing: "uninstalled" })]);
P("a project's plugin names its project", [plugin({ scope: "project", project: PROJECT })]);
P("a local plugin names its project as local", [plugin({ scope: "local", project: PROJECT })]);
P("a user plugin with a project reads project", [plugin({ scope: "user", project: PROJECT })]);
P("no version", [plugin({ version: undefined })]);
P("a version", [plugin({ version: "1.2.3" })]);
P("an id with control characters is a cell", [plugin({ id: "a\tb\nc@m" })]);
P("a wide id against the head", [plugin({ id: "a-very-long-plugin-id@some-marketplace" }), plugin({ id: "b@m" })]);
P("an emoji id counts two cells", [plugin({ id: "🧪@m" }), plugin({ id: "abc@m" })]);
P("a napping report", [plugin()], { stale: "napping", pauseMode: "memory" });
P("no plugins and a refusal", [], { refused: ["codex: plugins unreadable"] });
const bw = (name: string, brings: Obj): void => void add({ fn: "broughtWords", case: name, args: { brings } });
bw("nothing", NONE);
bw("one skill", { ...NONE, skills: ["a"] });
bw("two skills", { ...NONE, skills: ["a", "b"] });
bw("one of every kind", { skills: ["a"], commands: ["a"], subagents: ["a"], hooks: ["a"], servers: ["a"], lsp: ["a"], apps: ["a"] });
bw("two of every kind", { skills: ["a", "b"], commands: ["a", "b"], subagents: ["a", "b"], hooks: ["a", "b"], servers: ["a", "b"], lsp: ["a", "b"], apps: ["a", "b"] });
bw("only the language server", { ...NONE, lsp: ["rust-analyzer"] });
bw("only apps", { ...NONE, apps: ["Gmail ", "Drive"] });
bw("kinds in the table's order, not the object's", { apps: ["a"], lsp: ["a"], servers: ["a"], hooks: ["a"], subagents: ["a"], commands: ["a"], skills: ["a"] });
bw("an empty-string member still counts", { ...NONE, hooks: [""] });
bw("a dozen", { ...NONE, commands: Array.from({ length: 12 }, (_, i) => `c${i}`) });

// ---- toolLines
const tl = (name: string, nameOf: string, answer: Obj): void => void add({ fn: "toolLines", case: name, args: { name: nameOf, answer } });
const ANSWER_KEYS = ["auth", "tools", "holder", "refused", "readAt"];
const ans = (o: Obj): Obj => ordered(ANSWER_KEYS, { readAt: "2026-09-27T00:00:00.000Z", ...o });
tl("tools with a first line only of a long description", "linear", ans({ auth: "signed-in", tools: [{ name: "search_issues", description: "Finds issues\nacross teams", params: [{ name: "query", type: "string", required: true }] }, { name: "whoami" }] }));
tl("a held sign-in", "linear", ans({ auth: "unknown", holder: "codex" }));
tl("a held sign-in with tools", "linear", ans({ auth: "unknown", holder: "claude", tools: [{ name: "t" }] }));
tl("a holder the catalog does not know", "linear", ans({ auth: "unknown", holder: "not-in-catalog" }));
tl("refused by the server", "linear", ans({ auth: "failed", refused: "Did not answer in 20 s." }));
tl("refused wins over tools", "linear", ans({ auth: "failed", refused: "late", tools: [{ name: "t" }] }));
tl("refused with a holder", "linear", ans({ auth: "needs-sign-in", holder: "codex", refused: "no" }));
tl("not yet asked", "linear", ans({ auth: "unknown" }));
tl("no tools", "linear", ans({ auth: "open", tools: [] }));
tl("a tool with no description reads a dash", "x", ans({ auth: "open", tools: [{ name: "a" }, { name: "b", description: "has one" }] }));
tl("an empty description is a description", "x", ans({ auth: "open", tools: [{ name: "a", description: "" }] }));
tl("a description of a newline then text reads empty", "x", ans({ auth: "open", tools: [{ name: "a", description: "\nsecond" }] }));
tl("a description with a carriage return", "x", ans({ auth: "open", tools: [{ name: "a", description: "one\r\ntwo" }] }));
tl("a description with a tab and control characters", "x", ans({ auth: "open", tools: [{ name: "a", description: "tab\there\u0007bell" }] }));
tl("a wide tool name against the head", "x", ans({ auth: "open", tools: [{ name: "a_very_long_tool_name", description: "d" }, { name: "b", description: "e" }] }));
tl("an emoji tool name counts two cells", "x", ans({ auth: "open", tools: [{ name: "🧪", description: "d" }, { name: "abc", description: "e" }] }));
tl("a server name with an emoji and a colon", "a: b 🧪", ans({ auth: "connected", tools: [{ name: "t" }] }));
for (const auth of ["open", "env-key", "connected", "signed-in", "needs-sign-in", "failed", "unknown"]) tl(`auth ${auth}`, "s", ans({ auth, tools: [{ name: "t", description: "d" }] }));

// ---- pluginChanged
const pc = (name: string, ask: Obj, script: NonNullable<Case["script"]>, args: Obj = {}, more: Partial<Case> = {}): void => void add({ fn: "pluginChanged", case: name, args: { ask, usage: "wsp plugins enable", line: "wsp plugins enable", ...args }, script, ...more });
const ASK = { agent: "claude", plugin: "vercel@claude-plugins-official", on: true };
const PROW = plugin({ version: "0.50.0", path: "~/.claude/plugins/cache/x", brings: { ...NONE, skills: ["vercel:nextjs"] } });
pc("on, here", ASK, { replies: { "plugins.toggle": frame({ plugin: PROW }) } });
pc("off, here", { ...ASK, on: false }, { replies: { "plugins.toggle": frame({ plugin: plugin({ on: false }) }) } });
pc("another agent, off", { agent: "codex", plugin: "p@m", on: false }, { replies: { "plugins.toggle": frame({ plugin: plugin({ agent: "codex", on: false }) }) } });
pc("on a thread's computer", ASK, { replies: { ...thr, "plugins.toggle": frame({ plugin: PROW }) } }, { thread: "t-1" });
pc("on a named computer", ASK, { replies: { ...comp, "plugins.toggle": frame({ plugin: PROW }) } }, { on: "attic" });
pc("a thread and a computer are refused", ASK, { replies: { ...thr, ...comp } }, { thread: "t-1", on: "attic" });
pc("the host refuses", ASK, { replies: { "plugins.toggle": refused("a missing plugin cannot be turned on", "usage") } });
pc("the reply's extra keys are dropped", ASK, { replies: { "plugins.toggle": frame({ plugin: { ...PROW, later: 1 } }) } });
pc("the socket goes", ASK, { closes: { "plugins.toggle": 1006 } });

// ---- serverToolsOf
const st = (name: string, args: Obj, script: NonNullable<Case["script"]>, more: Partial<Case> = {}): void => void add({ fn: "serverToolsOf", case: name, args: { name: "linear", agent: "claude", refresh: false, usage: "wsp servers tools", line: "wsp servers tools", ...args }, script, ...more });
const TOOLS = ans({ auth: "signed-in", tools: [{ name: "search_issues", description: "Finds issues\nacross teams", params: [{ name: "query", type: "string", required: true }, { name: "limit", required: false, description: "how many" }] }, { name: "whoami" }] });
st("here", {}, { replies: { "servers.tools": frame({ answer: TOOLS }) } });
st("refresh is sent only when true", { refresh: true }, { replies: { "servers.tools": frame({ answer: TOOLS }) } });
st("held", {}, { replies: { "servers.tools": frame({ answer: ans({ auth: "unknown", holder: "codex" }) }) } });
st("refused by the server", {}, { replies: { "servers.tools": frame({ answer: ans({ auth: "failed", refused: "Did not answer in 20 s." }) }) } });
st("on a thread", { thread: "t-1" }, { replies: { ...thr, "servers.tools": frame({ answer: TOOLS }) } });
st("on a computer with a project", { on: "attic", project: "wsp" }, { replies: { ...comp, "servers.tools": frame({ answer: TOOLS }) } });
st("on a computer with no project", { on: "attic" }, { replies: { ...comp, "servers.tools": frame({ answer: TOOLS }) } });
st("a project with no computer is not sent", { project: "wsp" }, { replies: { "servers.tools": frame({ answer: TOOLS }) } });
st("a thread and a computer are refused", { thread: "t-1", on: "attic" }, { replies: { ...thr, ...comp } });
st("an empty server name is sent", { name: "" }, { replies: { "servers.tools": frame({ answer: TOOLS }) } });
st("the host refuses", {}, { replies: { "servers.tools": refused("linear is not a server there", "usage") } });
st("extra keys in the answer are dropped", {}, { replies: { "servers.tools": frame({ answer: { ...TOOLS, extra: "dropped" } }) } });

// ---- serverChanged
const sc = (name: string, op: string, body: Obj, script: NonNullable<Case["script"]>, args: Obj = {}, more: Partial<Case> = {}): void => void add({ fn: "serverChanged", case: name, args: { op, body, usage: "wsp servers add", line: "wsp servers add", ...args }, script, ...more });
const FILE = frame({ file: "~/.claude.json" });
sc("add here", "servers.add", { agent: "claude", name: "linear", url: "https://mcp.linear.app/mcp" }, { replies: { "servers.add": FILE } });
sc("remove here", "servers.remove", { agent: "claude", name: "linear" }, { replies: { "servers.remove": FILE } });
sc("toggle here", "servers.toggle", { agent: "claude", name: "linear", on: false }, { replies: { "servers.toggle": FILE } });
sc("the body's key order is kept after the target", "servers.add", { name: "k", agent: "codex", command: "node", args: ["k.js"], env: { A: "1" } }, { replies: { "servers.add": FILE } });
sc("on a thread with a project", "servers.add", { agent: "claude", name: "k", project: true }, { replies: { ...thr, "servers.add": FILE } }, { thread: "t-1" });
sc("on a computer with a project name", "servers.add", { agent: "claude", name: "k", project: true }, { replies: { ...comp, "servers.add": FILE } }, { on: "attic", project: "wsp" });
sc("a thread and a computer are refused", "servers.add", { agent: "claude", name: "k" }, { replies: { ...thr, ...comp } }, { thread: "t-1", on: "attic" });
sc("the answer is cut to its file", "servers.add", { agent: "claude", name: "k" }, { replies: { "servers.add": frame({ file: "~/x.toml", extra: 1 }) } });
sc("the host refuses", "servers.remove", { agent: "claude", name: "k" }, { replies: { "servers.remove": refused("no server k in ~/.claude.json", "usage") } });
sc("the socket goes", "servers.toggle", { agent: "claude", name: "k", on: true }, { closes: { "servers.toggle": 4001 } });

// ---- serverValues, serverCommand, serverScope
const sv = (name: string, env: Obj, names: string[], headers: string[]): void => void add({ fn: "serverValues", case: name, args: { env, names, headers, usage: "wsp servers add" } });
sv("nothing asked", { A: "1" }, [], []);
sv("one variable", { LINEAR_TOKEN: "secret" }, ["LINEAR_TOKEN"], []);
sv("two variables in the order named", { A: "1", B: "2" }, ["B", "A"], []);
sv("a variable the environment lacks", { A: "1" }, ["A", "B"], []);
sv("the first lacking variable is the one refused", {}, ["X", "Y"], []);
sv("an empty value is a value", { A: "" }, ["A"], []);
sv("a name repeated keeps its first place", { A: "1", B: "2" }, ["A", "B", "A"], []);
sv("a name that is a whole number goes first in the object", { A: "1", "1": "x" }, ["A", "1"], []);
sv("two whole-number names go first in numeric order", { A: "1", "10": "x", "2": "y" }, ["A", "10", "2"], []);
sv("one header", { TOKEN: "t" }, [], ["Authorization=TOKEN"]);
sv("two headers", { A: "1", B: "2" }, [], ["X-A=A", "X-B=B"]);
sv("a header and a variable", { A: "1", B: "2" }, ["A"], ["X-B=B"]);
sv("a header whose variable is lacking", {}, [], ["X=Y"]);
sv("a header with no equals sign", { A: "1" }, [], ["Authorization"]);
sv("a header with nothing before the equals sign", { A: "1" }, [], ["=A"]);
sv("a header with nothing after the equals sign", { A: "1" }, [], ["X="]);
sv("a header that is a lone equals sign", { A: "1" }, [], ["="]);
sv("a header with two equals signs reads the rest as the variable", { "B=C": "v", B: "w" }, [], ["A=B=C"]);
sv("a header with two equals signs and no such variable", { B: "w" }, [], ["A=B=C"]);
sv("the variables are read before the headers", {}, ["V"], ["not a header"]);
sv("a bad header before a lacking variable", {}, [], ["bad", "X=Y"]);
sv("the headers stop at the first bad one", { A: "1" }, [], ["X=A", "bad", "Y=A"]);
sv("a header repeated keeps its first place and last value", { A: "1", B: "2" }, [], ["X=A", "Y=A", "X=B"]);
sv("an environment with an empty name", { "": "v" }, [""], []);
sv("a value with a newline and an emoji", { A: "line\n🧪" }, ["A"], []);
const cmd = (name: string, line?: string): void => void add({ fn: "serverCommand", case: name, args: { ...(line !== undefined ? { line } : {}), usage: "wsp servers add" } });
cmd("no line", undefined);
cmd("one word", "node");
cmd("a program and arguments", "node k.js --flag");
cmd("extra spaces", "  node   k.js  ");
cmd("a single-quoted word", "node 'a b' c");
cmd("a double-quoted word with an escape", "node \"a \\\"b\\\" c\" d");
cmd("a backslash outside quotes", "node a\\ b");
cmd("an unclosed single quote", "node 'a b");
cmd("an unclosed double quote", "node \"a b");
cmd("an empty line", "");
cmd("a line of spaces", "   ");
cmd("an empty quoted word", "node ''");
cmd("a tab separates nothing", "node\tk.js");
cmd("an emoji argument", "node 🧪");
const scope = (name: string, word: string | undefined, project?: Obj): void => void add({ fn: "serverScope", case: name, args: { ...(word !== undefined ? { word } : {}), ...(project !== undefined ? { project } : {}), usage: "wsp servers add" } });
scope("no word, no project", undefined);
scope("no word, project asked reads project", undefined, { project: true });
scope("no word, project asked with a name", undefined, { project: true, name: "wsp" });
scope("no word, project not asked", undefined, { project: false });
for (const w of ["user", "home", "local", "project"]) scope(`${w}`, w);
scope("an unknown word", "plugin");
scope("a word in capitals", "User");
scope("an empty word", "");
scope("a word with quotes and an emoji", "a\"b🧪");
for (const w of ["project", "local"]) scope(`${w} with --project`, w, { project: true });
for (const w of ["user", "home"]) scope(`${w} with --project is refused`, w, { project: true });
scope("an unknown word with --project is refused as a scope first", "plugin", { project: true });
scope("a word with --project false", "user", { project: false });

// ---- addTools
const addTools = (name: string, agentWord: string, script: NonNullable<Case["script"]>): void => void add({ fn: "addTools", case: name, args: { agent: agentWord }, script });
addTools("added", "claude", { replies: { "agents.addTools": frame({ file: "~/.claude.json" }) } });
addTools("the answer is cut to its file", "codex", { replies: { "agents.addTools": frame({ file: "~/.codex/config.toml", extra: 1 }) } });
addTools("an empty agent is sent", "", { replies: { "agents.addTools": frame({ file: "~/x" }) } });
addTools("an agent the catalog does not know", "nope", { replies: { "agents.addTools": refused("nope is not an agent wsp knows", "usage") } });
addTools("refused", "claude", { replies: { "agents.addTools": refused("the config is not writable") } });
addTools("the socket goes", "claude", { closes: { "agents.addTools": 1006 } });

// ---- drawRows
const IS_WATCH = { watch: true };
const draw = (name: string, args: Obj, script: NonNullable<Case["script"]> = {}, more: Partial<Case> = {}): void =>
  void add({ fn: "drawRows", case: name, args: { words: "wsp agents", frames: [{ value: { agents: [1] }, rows: ["AGENT", "claude"] }], flags: {}, ...args }, script, ...more });
draw("one frame, written once with its lines joined", {});
draw("under --json the value is the line and the rows are not printed", { flags: { json: true } });
draw("no rows prints an empty line", { frames: [{ value: { a: 1 }, rows: [] }] });
draw("one row", { frames: [{ value: {}, rows: ["only"] }] });
draw("rows with an emoji and a newline are joined as given", { frames: [{ value: { x: "é" }, rows: ["a 🧪", "b\nc"] }] });
draw("the frame is handed the socket and sends its read", { frameOp: "agents.read" }, { replies: { "agents.read": frame({}) } });
draw("the frame's refusal rides out with nothing printed", { frameOp: "agents.read" }, { replies: { "agents.read": refused("not readable", "usage") } });
draw("--watch off is not a watch", { flags: { watch: false } });
draw("--watch with --json is refused before any dial", { flags: { watch: true, json: true } });
draw("--watch with no terminal to redraw on is refused before any dial", { flags: { watch: true } });
draw("--watch with --json and a terminal is refused for --json first", { flags: { watch: true, json: true }, redraw: true });
draw("--watch with a different line's words", { words: "wsp servers", flags: { watch: true } });
draw("--watch draws one frame and a stop ends it, one dial", { flags: IS_WATCH, redraw: true, stopAfter: 1 });
draw("--watch draws every frame over one dial", { flags: IS_WATCH, redraw: true, stopAfter: 2, frames: [{ value: {}, rows: ["one", "two"] }, { value: {}, rows: ["three"] }] }, {}, { within: 4000 });
draw("--watch with no rows draws nothing but the cursor", { flags: IS_WATCH, redraw: true, stopAfter: 1, frames: [{ value: {}, rows: [] }] });
draw("--watch whose frame is refused restores the cursor and refuses", { flags: IS_WATCH, redraw: true, stopAfter: 1, frameOp: "agents.read" }, { replies: { "agents.read": refused("not readable", "usage") } });

const m = (mod: Obj, name: string): any => (mod["agents"] as any)[name];
const runs: Record<string, Run> = {
  constAimedBothLine: ({ mod }) => m(mod, "aimedBothLine"),
  constProjectUnnamedLine: ({ mod }) => m(mod, "projectUnnamedLine"),
  constToolsProjectBareLine: ({ mod }) => m(mod, "toolsProjectBareLine"),
  constPluginKindWords: ({ mod }) => m(mod, "PLUGIN_KIND_WORDS"),
  constServerToolColumns: ({ mod }) => m(mod, "SERVER_TOOL_COLUMNS"),
  aimedUsage: ({ mod, args }) => m(mod, "aimedUsage")(args["tool"]),
  projectOffComputerLine: ({ mod, args }) => m(mod, "projectOffComputerLine")(args["value"]),
  pluginTurnedWords: ({ mod, args }) => m(mod, "pluginTurnedWords")(args["id"], args["on"], args["agent"]),
  pluginTurnedLine: ({ mod, args }) => m(mod, "pluginTurnedLine")(args["row"]),
  unsetVariableLine: ({ mod, args }) => m(mod, "unsetVariableLine")(args["variable"]),
  notHeaderLine: ({ mod, args }) => m(mod, "notHeaderLine")(args["pair"]),
  projectScopeLine: ({ mod, args }) => m(mod, "projectScopeLine")(args["word"]),
  toolsAddedLine: ({ mod, args }) => m(mod, "toolsAddedLine")(args["agent"], args["file"]),
  signedInLine: ({ mod, args }) => m(mod, "signedInLine")(args["what"], args["where"], args["answer"]),
  projectAsked: ({ mod, args }) => m(mod, "projectAsked")(args["value"], args["thread"], args["on"], args["usage"]),
  toolsProject: async ({ mod, args }) => (await m(mod, "toolsProject")(args["value"], args["thread"], args["on"], args["usage"])) ?? null,
  agentsTarget: ({ mod, client, args }) => m(mod, "agentsTarget")(client, args["thread"], args["on"], args["usage"], args["line"], args["project"]),
  agentsReport: ({ mod, client, args }) => m(mod, "agentsReport")(client, args["thread"], args["on"], args["usage"], args["line"]),
  reportFacts: ({ mod, proto, args }) => m(mod, "reportFacts")((proto["AgentsReport"] as any).parse(args["report"])),
  agentRowLines: ({ mod, proto, args }) => m(mod, "agentRowLines")((proto["AgentsReport"] as any).parse(args["report"])),
  skillRowLines: ({ mod, proto, args }) => m(mod, "skillRowLines")((proto["AgentsReport"] as any).parse(args["report"])),
  serverRowLines: ({ mod, proto, args }) => m(mod, "serverRowLines")((proto["AgentsReport"] as any).parse(args["report"])),
  pluginRowLines: ({ mod, proto, args }) => m(mod, "pluginRowLines")((proto["AgentsReport"] as any).parse(args["report"])),
  broughtWords: ({ mod, args }) => m(mod, "broughtWords")(args["brings"]),
  toolLines: ({ mod, proto, args }) => m(mod, "toolLines")(args["name"], (proto["ServerToolsAnswer"] as any).parse(args["answer"])),
  pluginChanged: ({ mod, client, args }) => m(mod, "pluginChanged")(client, args["ask"], args["thread"], args["on"], args["usage"], args["line"]),
  serverToolsOf: ({ mod, client, args }) => m(mod, "serverToolsOf")(client, args["name"], args["agent"], args["thread"], args["on"], args["refresh"], args["usage"], args["line"], args["project"]),
  serverChanged: ({ mod, client, args }) => m(mod, "serverChanged")(client, args["op"], args["body"], args["thread"], args["on"], args["usage"], args["line"], args["project"]),
  serverValues: ({ mod, args }) => m(mod, "serverValues")(args["env"], args["names"], args["headers"], args["usage"]),
  serverCommand: ({ mod, args }) => m(mod, "serverCommand")(args["line"], args["usage"]),
  serverScope: ({ mod, args }) => m(mod, "serverScope")(args["word"], args["usage"], args["project"]),
  addTools: ({ mod, client, args }) => m(mod, "addTools")(client, args["agent"]),
  drawRows: async ({ mod, client, args, sinks }) => {
    const io: Obj = { log: (l: string) => sinks.stdout.push(l), error: (l: string) => sinks.stderr.push(l), stream: (t: string) => (sinks.streamed += t) };
    const listeners: (() => void)[] = [];
    if (args["redraw"] === true) {
      io["redraw"] = {
        columns: () => 80,
        write: (t: string) => {
          sinks.streamed += t;
        },
      };
    }
    const flags = args["flags"] as Obj;
    const ctx: Obj = {
      flags,
      io,
      out: (mod["client"] as any).formatter(io, flags["json"] === true),
      client: async () => {
        sinks.said.push("dial");
        return client;
      },
    };
    if (args["redraw"] === true) {
      ctx["signals"] = {
        on: (sig: string, fn: () => void) => {
          sinks.said.push(`on ${sig}`);
          listeners.push(fn);
        },
        off: (sig: string) => void sinks.said.push(`off ${sig}`),
      };
    }
    const frames = args["frames"] as { value: object; rows: string[] }[];
    let n = 0;
    const frame = async (c: Obj): Promise<{ value: object; rows: string[] }> => {
      sinks.said.push(`frame ${n + 1}`);
      if (args["frameOp"] !== undefined) await (c as any).request(args["frameOp"]);
      const given = frames[Math.min(n++, frames.length - 1)]!;
      if (n === args["stopAfter"]) for (const l of [...listeners]) l();
      return given;
    };
    return m(mod, "drawRows")(ctx, args["words"], frame);
  },
};

it("agents_read", async () => {
  await record("agents_read", cases, runs);
});
