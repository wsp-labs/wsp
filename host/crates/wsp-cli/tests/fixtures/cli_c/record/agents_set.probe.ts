// Unit `agents_set`: agents-help.ts:300-573 (skills.sh and skills lines, agent and project defaults, an agent's setup).
import { it } from "vitest";
import { frame, record, recorded, refused, type Case, type Obj, type Run } from "./lib.ts";

const ask = (tool: string, name: string, op: string): string => recorded(tool, name)[op]!;
const BASE = (JSON.parse(ask("agents_set", "model, effort and access", "preferences.set")) as { preferences: Obj }).preferences;
/** A `preferences` reply: the recorded one with the fields named replaced. */
const prefs = (over: Obj): string => frame({ preferences: { ...BASE, ...over } });
const PROJECT = (JSON.parse(ask("projects_set", "set", "projects.resolve")) as { project: Obj }).project;
const PLACES = ask("skills_show", "on a project", "places.list");
const SESSIONS = ask("skills_show", "in a workspace", "sessions.list");
const WORKSPACE = ask("skills_show", "in a workspace", "workspaces.get");
/** A script that finds the thread `t-landing`, the places and the replies given. */
const thread = (more: Record<string, string> = {}): Record<string, string> => ({ "sessions.list": SESSIONS, "workspaces.get": WORKSPACE, ...more });
const box = (more: Record<string, string> = {}): Record<string, string> => ({ "places.list": PLACES, ...more });
const USAGE = "usage: wsp skills show <skill>";
const LINE = "wsp skills show";

const ascii = (s: string): string => s.replace(/[^\x00-\x7f]/gu, c => `U+${c.codePointAt(0)!.toString(16).toUpperCase()}`);
const cases: Case[] = [];
const add = (c: Case): void => void cases.push(c);

// ---------------------------------------------------------------------------------------------------------- searchSkillsSh
const hits = ask("skills_search", "hits", "skills.search");
add({ fn: "searchSkillsSh", case: "hits with a limit, an extra key dropped", args: { q: "review code", limit: 2 }, script: { replies: { "skills.search": hits } } });
add({ fn: "searchSkillsSh", case: "no limit sends no limit", args: { q: "review code" }, script: { replies: { "skills.search": hits } } });
add({ fn: "searchSkillsSh", case: "limit zero is sent", args: { q: "x", limit: 0 }, script: { replies: { "skills.search": ask("skills_search", "none", "skills.search") } } });
add({ fn: "searchSkillsSh", case: "none", args: { q: "nothing" }, script: { replies: { "skills.search": ask("skills_search", "none", "skills.search") } } });
add({ fn: "searchSkillsSh", case: "an empty query is sent as it is", args: { q: "" }, script: { replies: { "skills.search": ask("skills_search", "refused", "skills.search") } } });
add({ fn: "searchSkillsSh", case: "refused by the host", args: { q: "x", limit: 5 }, script: { replies: { "skills.search": ask("skills_search", "refused", "skills.search") } } });
add({ fn: "searchSkillsSh", case: "the op is not in the record", args: { q: "x" } });
add({ fn: "searchSkillsSh", case: "a query with spaces and a unicode name", args: { q: "  pdf forms 🧪 ", limit: 20 }, script: { replies: { "skills.search": frame({ skills: [{ id: "a/b/pdf", source: "a/b", skillId: "pdf", name: "pdf 🧪", installs: 200826 }] }) } } });

// ---------------------------------------------------------------------------------------------------------- skillHitLines
const hit = (name: string, installs: number, id = "o/r/s"): Obj => ({ id, source: "o/r", skillId: "s", name, installs });
add({ fn: "skillHitLines", case: "none", args: { hits: [] } });
add({ fn: "skillHitLines", case: "one", args: { hits: [hit("pdf", 200826, "anthropics/skills/pdf")] } });
add({ fn: "skillHitLines", case: "columns pad to the widest cell", args: { hits: [hit("pdf", 200826, "anthropics/skills/pdf"), hit("a-much-longer-skill-name", 7, "o/r/a-much-longer-skill-name"), hit("c", 0, "a/b/c")] } });
add({ fn: "skillHitLines", case: "installs zero and a large count", args: { hits: [hit("z", 0), hit("big", 1234567890)] } });
add({ fn: "skillHitLines", case: "a tab and a newline in a name become spaces", args: { hits: [hit("code-review \ttabbed\nsplit 🧪", 123456, "zingzy/skills/code-review")] } });
add({ fn: "skillHitLines", case: "control characters and escapes are dropped (skills-sh.test.ts what a skills verb prints)", args: { hits: [{ id: "a/b/c\x1b[2J", source: "a/b", skillId: "c", name: "c\x1b]0;x\x07", installs: 1 }] } });
add({ fn: "skillHitLines", case: "a wide character counts as two cells of width", args: { hits: [hit("🧪🧪", 1), hit("ab", 22)] } });
add({ fn: "skillHitLines", case: "an empty name", args: { hits: [hit("", 3, "o/r/s"), hit("pdf", 4)] } });
add({ fn: "skillHitLines", case: "the same with the cloud on", cloud: true, args: { hits: [hit("pdf", 200826, "anthropics/skills/pdf")] } });

// ---------------------------------------------------------------------------------------------------------- skillShown
const shown = (extra: Obj): Obj => ({ skill: "review", usage: USAGE, line: LINE, ...extra });
const skillsShGet = ask("skills_show", "skills.sh", "skills.get");
const preview = (text: string, size: number): string => frame({ preview: { text, size } });
add({ fn: "skillShown", case: "an id off skills.sh asks skills.get", args: shown({ skill: "o/r/s" }), script: { replies: { "skills.get": skillsShGet } } });
add({ fn: "skillShown", case: "an id off skills.sh with no project", args: shown({ skill: "anthropics/skills/pdf", project: { project: false } }), script: { replies: { "skills.get": preview("# pdf\n", 6) } } });
add({ fn: "skillShown", case: "a skills.sh id with an empty middle is still one", args: shown({ skill: "a//c" }), script: { replies: { "skills.get": preview("x", 1) } } });
add({ fn: "skillShown", case: "two parts name a skill on the target", args: shown({ skill: "o/r" }), script: { replies: { "skills.preview": preview("x", 1) } } });
add({ fn: "skillShown", case: "four parts name a skill on the target", args: shown({ skill: "a/b/c/d" }), script: { replies: { "skills.preview": preview("x", 1) } } });
add({ fn: "skillShown", case: "an empty name asks the target", args: shown({ skill: "" }), script: { replies: { "skills.preview": preview("", 0) } } });
for (const [label, extra] of [
  ["a thread", { thread: "t-landing" }],
  ["a computer", { on: "attic" }],
  ["a project of the thread", { project: { project: true } }],
  ["a project by name", { on: "attic", project: { project: true, name: "api" } }],
] as const) add({ fn: "skillShown", case: `an id off skills.sh with ${label} is refused before any frame`, args: shown({ skill: "o/r/s", ...extra }), script: { replies: skillsShGet === "" ? {} : { "skills.get": skillsShGet } } });
add({ fn: "skillShown", case: "here: skills.preview of the name, no project key", args: shown({ skill: "review" }), script: { replies: { "skills.preview": ask("skills_show", "here", "skills.preview") } } });
add({ fn: "skillShown", case: "on a computer's project by name", args: shown({ on: "attic", project: { project: true, name: "api" } }), script: { replies: box({ "skills.preview": preview("x", 65536) }) } });
add({ fn: "skillShown", case: "on a computer with the project asked but unnamed", args: shown({ on: "attic", project: { project: true } }), script: { replies: box({ "skills.preview": preview("x", 3) }) } });
add({ fn: "skillShown", case: "on a computer, no project", args: shown({ on: "attic" }), script: { replies: box({ "skills.preview": preview("x", 3) }) } });
add({ fn: "skillShown", case: "on a computer by its id", args: shown({ on: "place-9" }), script: { replies: box({ "skills.preview": preview("x", 3) }) } });
add({ fn: "skillShown", case: "on a provider by its name in other case", args: shown({ on: "SOLARI" }), script: { replies: box({ "skills.preview": preview("x", 3) }) } });
add({ fn: "skillShown", case: "a project name given with no --on is only read by the target of a thread", args: shown({ thread: "t-landing", project: { project: true, name: "api" } }), script: { replies: thread({ "skills.preview": preview("x", 65537) }) } });
add({ fn: "skillShown", case: "in a workspace by thread, its own project", args: shown({ thread: "t-landing", project: { project: true } }), script: { replies: thread({ "skills.preview": preview("x", 65537) }) } });
add({ fn: "skillShown", case: "in a workspace by thread, not a project's", args: shown({ thread: "t-landing" }), script: { replies: thread({ "skills.preview": preview("x", 1) }) } });
add({ fn: "skillShown", case: "both a thread and a computer", args: shown({ thread: "t-landing", on: "attic" }), script: { replies: thread() } });
add({ fn: "skillShown", case: "no such computer", args: shown({ on: "nowhere" }), script: { replies: box() } });
add({ fn: "skillShown", case: "another version answers the thread", args: shown({ thread: "t-landing" }), script: { replies: { "sessions.list": SESSIONS, "workspaces.get": frame({}) } } });
add({ fn: "skillShown", case: "the host refuses the name", args: shown({ skill: "gone" }), script: { replies: { "skills.preview": ask("skills_show", "refused", "skills.preview") } } });
add({ fn: "skillShown", case: "a thread naming nothing, the cloud off", args: shown({ thread: "zzz" }), script: { replies: { "sessions.list": frame({ sessions: [] }) } } });
add({ fn: "skillShown", case: "a thread naming nothing, the cloud on", cloud: true, args: shown({ thread: "zzz" }), script: { replies: { "sessions.list": frame({ sessions: [] }) } } });
add({ fn: "skillShown", case: "the host refuses an id off skills.sh", args: shown({ skill: "o/r/gone" }), script: { replies: { "skills.get": refused("skills.sh has no skill o/r/gone.", "not-found") } } });
add({ fn: "skillShown", case: "the same with the cloud on", cloud: true, args: shown({ skill: "review" }), script: { replies: { "skills.preview": ask("skills_show", "here", "skills.preview") } } });

// ---------------------------------------------------------------------------------------------------------- shownText
const text = (t: string, size: number): Obj => ({ p: { text: t, size } });
add({ fn: "shownText", case: "a short text, whole", args: text("# pdf\n\nUse it.\n", 15) });
add({ fn: "shownText", case: "size 0", args: text("", 0) });
add({ fn: "shownText", case: "size 1", args: text("x", 1) });
add({ fn: "shownText", case: "size just under the cap", args: text("x", 65535) });
add({ fn: "shownText", case: "size at the cap, no cut line", args: text("x", 65536) });
add({ fn: "shownText", case: "size one over the cap, 65 KB", args: text("x", 65537) });
add({ fn: "shownText", case: "size 66559, rounds up to 65 KB", args: text("x", 66559) });
add({ fn: "shownText", case: "size 66560 is exactly 65 KB", args: text("x", 66560) });
add({ fn: "shownText", case: "size 66561 rounds up to 66 KB", args: text("x", 66561) });
add({ fn: "shownText", case: "size 70000 rounds up to 69 KB", args: text("# S\n\u001b[31mred\u0085\tend\r", 70000) });
add({ fn: "shownText", case: "a huge size", args: text("x", 10_000_000) });
add({ fn: "shownText", case: "an empty text over the cap", args: text("", 65537) });
add({ fn: "shownText", case: "control characters and escapes are dropped, tab and newline kept (skills-sh.test.ts)", args: text("# pdf\n\tUse it.\x1b]52;c;cm0gLXJmIH4=\x07\x1b]0;title\x07\x1b]8;;https://evil.test\x1b\\click\x1b]8;;\x1b\\\r\x9b2J\x00end", 120) });
add({ fn: "shownText", case: "the same hostile text over the cap", args: text("a\x1b[2Jb\x00c\x7fd\x9fe\x0bf\x0cg", 100000) });
add({ fn: "shownText", case: "emoji and accents are kept", args: text("é 🧪 日本語\n", 20) });
add({ fn: "shownText", case: "the same with the cloud on", cloud: true, args: text("x", 65537) });

// ---------------------------------------------------------------------------------------------------------- skillAdded
const adds = (extra: Obj): Obj => ({ skill: "o/r/s", usage: "usage: wsp skills add <skill>", line: "wsp skills add", project: { project: false }, ...extra });
add({ fn: "skillAdded", case: "copies for two agents", args: adds({ skill: "zingzy/skills/code\u0085review", agents: ["codex", "zed-x"] }), script: { replies: { "skills.add": ask("skills_add", "copies", "skills.add") } } });
add({ fn: "skillAdded", case: "alone, no agents key", args: adds({ skill: "plain" }), script: { replies: { "skills.add": ask("skills_add", "alone", "skills.add") } } });
add({ fn: "skillAdded", case: "an empty agents list sends no agents key", args: adds({ skill: "plain", agents: [] }), script: { replies: { "skills.add": ask("skills_add", "alone", "skills.add") } } });
add({ fn: "skillAdded", case: "one agent", args: adds({ agents: ["claude"] }), script: { replies: { "skills.add": frame({ added: { path: "~/.agents/skills/s", agents: [{ agent: "claude", path: "~/.claude/skills/s" }] } }) } } });
add({ fn: "skillAdded", case: "a project on a computer", args: adds({ on: "place-solari", project: { project: true, name: "api" }, agents: [] }), script: { replies: { "places.list": PLACES, "skills.add": ask("skills_add", "a project on a computer", "skills.add") } } });
add({ fn: "skillAdded", case: "a project on a computer, unnamed", args: adds({ on: "attic", project: { project: true } }), script: { replies: box({ "skills.add": ask("skills_add", "alone", "skills.add") }) } });
add({ fn: "skillAdded", case: "the thread's own project", args: adds({ thread: "t-landing", project: { project: true }, agents: ["codex"] }), script: { replies: thread({ "skills.add": ask("skills_add", "copies", "skills.add") }) } });
add({ fn: "skillAdded", case: "the host refuses, the machine napping", args: adds({ thread: "t-landing" }), script: { replies: thread({ "skills.add": ask("skills_add", "refused", "skills.add") }) } });
add({ fn: "skillAdded", case: "both a thread and a computer", args: adds({ thread: "t-landing", on: "attic" }), script: { replies: thread() } });
add({ fn: "skillAdded", case: "no such computer", args: adds({ on: "nowhere" }), script: { replies: box() } });
add({ fn: "skillAdded", case: "an id off skills.sh asks the target like a name", args: adds({ skill: "anthropics/skills/pdf", on: "attic" }), script: { replies: box({ "skills.add": ask("skills_add", "alone", "skills.add") }) } });
add({ fn: "skillAdded", case: "the cloud on changes nothing", cloud: true, args: adds({ skill: "plain" }), script: { replies: { "skills.add": ask("skills_add", "alone", "skills.add") } } });

// ---------------------------------------------------------------------------------------------------------- addedLine
const added = (path: string, agents: Obj[]): Obj => ({ path, agents });
add({ fn: "addedLine", case: "alone", args: { skill: "o/r/plain", a: added("~/.agents/skills/plain", []) } });
add({ fn: "addedLine", case: "one agent copy", args: { skill: "o/r/s", a: added("~/.agents/skills/s", [{ agent: "claude", path: "~/.claude/skills/s" }]) } });
add({ fn: "addedLine", case: "two copies, comma joined, in the order answered", args: { skill: "zingzy/skills/code-review", a: added("~/.agents/skills/code-review", [{ agent: "codex", path: "~/.codex/skills/code-review" }, { agent: "claude", path: "~/.claude/skills/code-review" }]) } });
add({ fn: "addedLine", case: "an agent the catalog does not know is named by its id", args: { skill: "o/r/s", a: added("~/p", [{ agent: "zed-x", path: "~/.zed\n/skills" }]) } });
add({ fn: "addedLine", case: "a name with no slash", args: { skill: "plain", a: added("~/p", []) } });
add({ fn: "addedLine", case: "a trailing slash leaves the name empty", args: { skill: "o/r/", a: added("~/p", []) } });
add({ fn: "addedLine", case: "an empty skill", args: { skill: "", a: added("~/p", []) } });
add({ fn: "addedLine", case: "the name is made printable", args: { skill: "o/r/co\tde\nreview\x85", a: added("~/p", []) } });
add({ fn: "addedLine", case: "folders print without escapes or line breaks (skills-sh.test.ts)", args: { skill: "a/b/pdf", a: added("~/.agents/skills/pdf\x1b]0;x\x07", [{ agent: "claude", path: "~/.claude/skills/pdf\x9b2J\nfake" }]) } });
add({ fn: "addedLine", case: "an empty path", args: { skill: "o/r/s", a: added("", [{ agent: "codex", path: "" }]) } });

// ---------------------------------------------------------------------------------------------------------- skillChanged
const change = (op: string, extra: Obj): Obj => ({ op, name: "review", project: { project: false }, usage: "usage: wsp skills " + (op === "skills.remove" ? "remove" : "disable") + " <skill>", line: "wsp skills " + (op === "skills.remove" ? "remove" : "disable"), ...extra });
add({ fn: "skillChanged", case: "remove, the thread's own project", args: change("skills.remove", { name: "review\tx", thread: "t-landing", project: { project: true } }), script: { replies: thread({ "skills.remove": ask("skills_remove", "removed", "skills.remove") }) } });
add({ fn: "skillChanged", case: "remove here, no project, no on key", args: change("skills.remove", {}), script: { replies: { "skills.remove": ask("skills_remove", "removed", "skills.remove") } } });
add({ fn: "skillChanged", case: "remove ignores a turn word", args: change("skills.remove", { turn: false }), script: { replies: { "skills.remove": ask("skills_remove", "removed", "skills.remove") } } });
add({ fn: "skillChanged", case: "remove reads removed, not paths", args: change("skills.remove", {}), script: { replies: { "skills.remove": frame({ removed: ["~/a"], paths: ["~/b"] }) } } });
add({ fn: "skillChanged", case: "remove of nothing", args: change("skills.remove", {}), script: { replies: { "skills.remove": frame({ removed: [] }) } } });
add({ fn: "skillChanged", case: "remove refused", args: change("skills.remove", { name: "wsp" }), script: { replies: { "skills.remove": ask("skills_remove", "refused", "skills.remove") } } });
add({ fn: "skillChanged", case: "remove with both aimed", args: change("skills.remove", { thread: "t-landing", on: "attic" }), script: { replies: thread() } });
add({ fn: "skillChanged", case: "toggle off on a computer", args: change("skills.toggle", { on: "attic", turn: false }), script: { replies: box({ "skills.toggle": ask("skills_disable", "off", "skills.toggle") }) } });
add({ fn: "skillChanged", case: "toggle on sends on true", args: change("skills.toggle", { turn: true }), script: { replies: { "skills.toggle": ask("skills_enable", "on", "skills.toggle") } } });
add({ fn: "skillChanged", case: "toggle with no turn sends no on key", args: change("skills.toggle", {}), script: { replies: { "skills.toggle": ask("skills_enable", "on", "skills.toggle") } } });
add({ fn: "skillChanged", case: "toggle reads paths, not removed", args: change("skills.toggle", { turn: true }), script: { replies: { "skills.toggle": frame({ removed: ["~/a"], paths: ["~/b", "~/c"] }) } } });
add({ fn: "skillChanged", case: "toggle a project's by name on a computer", args: change("skills.toggle", { on: "place-9", turn: false, project: { project: true, name: "api" } }), script: { replies: box({ "skills.toggle": frame({ paths: ["/root/api/.agents/skills/s/SKILL.md.off"] }) }) } });
add({ fn: "skillChanged", case: "toggle refused", args: change("skills.toggle", { name: "gone", turn: false }), script: { replies: { "skills.toggle": ask("skills_disable", "refused", "skills.toggle") } } });
add({ fn: "skillChanged", case: "toggle in the thread's workspace, a project's", args: change("skills.toggle", { thread: "t-landing", turn: false, project: { project: true } }), script: { replies: thread({ "skills.toggle": ask("skills_disable", "off", "skills.toggle") }) } });
add({ fn: "skillChanged", case: "no such computer", args: change("skills.toggle", { on: "nowhere", turn: true }), script: { replies: box() } });
add({ fn: "skillChanged", case: "the cloud on changes nothing", cloud: true, args: change("skills.remove", {}), script: { replies: { "skills.remove": ask("skills_remove", "removed", "skills.remove") } } });

// ---------------------------------------------------------------------------------------------------------- removedLine
add({ fn: "removedLine", case: "one folder", args: { name: "review", removed: ["~/.agents/skills/review"] } });
add({ fn: "removedLine", case: "two folders, comma joined", args: { name: "review", removed: ["~/.agents/skills/review", "~/.claude/skills/review"] } });
add({ fn: "removedLine", case: "no folder", args: { name: "review", removed: [] } });
add({ fn: "removedLine", case: "folders print without escapes or line breaks (skills-sh.test.ts)", args: { name: "pdf\x1b[2J", removed: ["~/.claude/skills/pdf\x1b]52;c;eA==\x07", "~/.agents/skills/pdf\r\n"] } });
add({ fn: "removedLine", case: "a tab in the name and a newline in a folder", args: { name: "re\tview", removed: ["a\nb", "c\td"] } });

// ---------------------------------------------------------------------------------------------------------- defaultAgentSet
add({ fn: "defaultAgentSet", case: "set", args: { agent: "codex" }, script: { replies: { "preferences.set": ask("agents_default", "set", "preferences.set") } } });
add({ fn: "defaultAgentSet", case: "an agent the host runs no thread of", args: { agent: "nope" }, script: { replies: { "preferences.set": ask("agents_default", "an agent the host runs no thread of", "preferences.set") } } });
add({ fn: "defaultAgentSet", case: "an empty agent is sent as it is", args: { agent: "" }, script: { replies: { "preferences.set": ask("agents_default", "set", "preferences.set") } } });
add({ fn: "defaultAgentSet", case: "the op is not in the record", args: { agent: "claude" } });

// ---------------------------------------------------------------------------------------------------------- agentDefaultsSet
const noGet = (after: Obj): Record<string, string> => ({ "preferences.set": prefs({ agentDefaults: after }) });
const picker = (kept: Obj | undefined, after: Obj): Record<string, string> => ({ "preferences.get": prefs({ agentDefaults: kept === undefined ? {} : { claude: { models: kept } } }), "preferences.set": prefs({ agentDefaults: after }) });
const dset = (name: string, askOf: Obj, script: Record<string, string>, extra: Partial<Case> = {}, agent = "claude"): void => add({ fn: "agentDefaultsSet", case: name, args: { agent, ask: askOf }, script: { replies: script }, ...extra });
dset("model, effort and access", { model: "opus \u0085", effort: "high", access: "ask" }, { "preferences.set": ask("agents_set", "model, effort and access", "preferences.set") });
dset("the picker's lists merged over what stands", { hide: ["haiku"], show: ["sonnet"], order: ["opus", "haiku"], addModel: ["custom-1", "c0"], dropModel: ["c9"] }, { "preferences.get": ask("agents_set", "the picker's lists merged over what stands", "preferences.get"), "preferences.set": ask("agents_set", "the picker's lists merged over what stands", "preferences.set") });
dset("the picker's lists with none standing", { hide: ["gpt"] }, { "preferences.get": ask("agents_set", "the picker's lists with none standing", "preferences.get"), "preferences.set": ask("agents_set", "the picker's lists with none standing", "preferences.set") }, {}, "codex");
dset("put back", { reset: ["model", "models"] }, { "preferences.set": ask("agents_set", "put back", "preferences.set") });
dset("nothing to change", {}, {});
dset("empty lists and no reset are nothing to change", { hide: [], show: [], order: [], addModel: [], dropModel: [], reset: [] }, {});
dset("a model and empty lists reads no preferences", { model: "m", hide: [] }, noGet({ claude: { model: "m" } }));
dset("an agent's own spelling of access", { access: "bypassPermissions" }, {});
dset("an empty access word", { access: "" }, {});
dset("a word the agent has no mode for", { access: "plan" }, { "preferences.set": ask("agents_set", "a word the agent has no mode for", "preferences.set") });
for (const word of ["ask", "auto-edit", "full"]) dset(`access ${word}`, { access: word }, noGet({ claude: { access: word } }));
dset("a reset word outside the list", { reset: ["bogus"] }, {});
dset("a project's reset word is refused", { model: "m", reset: ["model", "after-worktree"] }, noGet({}));
dset("the first bad reset word is named", { reset: ["effort", "x", "y"] }, {});
dset("a reset word in other case is refused", { reset: ["Model"] }, {});
dset("a value wins over a reset of the same field", { model: "m2", reset: ["model"] }, noGet({ claude: { model: "m2" } }));
dset("reset effort and access", { reset: ["effort", "access"] }, noGet({}));
dset("reset all four", { reset: ["model", "effort", "access", "models"] }, noGet({}));
dset("reset models beats the lists and reads no preferences", { hide: ["a"], order: ["b"], reset: ["models"] }, noGet({}));
dset("reset models with a model set", { model: "m", reset: ["models"] }, noGet({ claude: { model: "m" } }));
dset("the same reset twice", { reset: ["effort", "effort"] }, noGet({}));
dset("a list with a model and a reset of another field", { show: ["x"], reset: ["model"] }, picker({ hide: ["x", "y"] }, { claude: { models: { hide: ["y"] } } }));
dset("show takes a model out of the kept hide", { show: ["sonnet"] }, picker({ hide: ["sonnet", "opus"] }, { claude: { models: { hide: ["opus"] } } }));
dset("show of the last hidden model leaves an empty picker", { show: ["sonnet"] }, picker({ hide: ["sonnet"] }, { claude: {} }));
dset("hide and show of the same model: hide is added after the filter", { hide: ["b"], show: ["b"] }, picker({ hide: ["a"] }, { claude: { models: { hide: ["a", "b"] } } }));
dset("hide is de-duplicated against what stands and itself", { hide: ["a", "a", "b"] }, picker({ hide: ["a"] }, { claude: { models: { hide: ["a", "b"] } } }));
dset("hide with the kept list keeping its order first", { hide: ["z"] }, picker({ hide: ["m", "a"] }, { claude: { models: { hide: ["m", "a", "z"] } } }));
dset("order replaces the kept order", { order: ["z"] }, picker({ order: ["x", "y"] }, { claude: { models: { order: ["z"] } } }));
dset("order is not de-duplicated", { order: ["a", "a", "b"] }, picker(undefined, { claude: { models: { order: ["a", "a", "b"] } } }));
dset("the kept order stays when only hide is given", { hide: ["h"] }, picker({ order: ["x", "y"], custom: ["c"] }, { claude: { models: { hide: ["h"], order: ["x", "y"], custom: ["c"] } } }));
dset("an empty order keeps the kept order", { order: [], hide: ["h"] }, picker({ order: ["x"] }, { claude: { models: { hide: ["h"], order: ["x"] } } }));
dset("addModel after what stands, de-duplicated", { addModel: ["a", "b", "b"] }, picker({ custom: ["a"] }, { claude: { models: { custom: ["a", "b"] } } }));
dset("dropModel takes from the kept custom list", { dropModel: ["a"] }, picker({ custom: ["a", "b"] }, { claude: { models: { custom: ["b"] } } }));
dset("dropModel of the only custom model sends an empty models object", { dropModel: ["a"] }, picker({ custom: ["a"] }, { claude: {} }));
dset("dropModel with nothing standing sends an empty models object", { dropModel: ["a"] }, picker(undefined, { claude: {} }));
dset("dropModel and addModel of the same model: add comes after the drop", { dropModel: ["a"], addModel: ["a"] }, picker({ custom: ["a", "b"] }, { claude: { models: { custom: ["b", "a"] } } }));
dset("every list at once keeps hide, order, custom in that order", { hide: ["h"], show: ["s"], order: ["o"], addModel: ["c"], dropModel: ["d"] }, picker({ hide: ["s"], order: ["old"], custom: ["d"] }, { claude: { models: { hide: ["h"], order: ["o"], custom: ["c"] } } }));
dset("the kept lists of another agent are not read", { hide: ["h"] }, { "preferences.get": prefs({ agentDefaults: { codex: { models: { hide: ["other"], custom: ["x"] } } } }), "preferences.set": prefs({ agentDefaults: { claude: { models: { hide: ["h"] } } } }) });
dset("an agent standing with no models", { hide: ["h"] }, { "preferences.get": prefs({ agentDefaults: { claude: { model: "m" } } }), "preferences.set": prefs({ agentDefaults: { claude: { model: "m", models: { hide: ["h"] } } } }) });
dset("a model and a list together: the patch keeps model first, models last", { model: "m", effort: "low", access: "full", hide: ["h"] }, { "preferences.get": prefs({}), "preferences.set": prefs({ agentDefaults: { claude: { model: "m", effort: "low", access: "full", models: { hide: ["h"] } } } }) });
dset("the answer has no entry for the agent", { model: "m" }, noGet({ codex: { model: "m" } }));
dset("the answer has the agent's whole defaults", { effort: "low" }, noGet({ claude: { model: "m", effort: "low", access: "plan", models: { hide: ["h"], order: ["o"], custom: ["c"] } } }));
dset("the host refuses the models", { hide: ["h"] }, { "preferences.get": prefs({}), "preferences.set": refused("no model named h", "usage") });
dset("reading the preferences is refused: nothing is set", { hide: ["h"] }, { "preferences.get": refused("preferences are not readable", "auth") });
dset("model names with spaces and unicode are sent as they are", { hide: ["gpt 5 🧪"], addModel: ["é"] }, picker(undefined, { claude: { models: { hide: ["gpt 5 🧪"], custom: ["é"] } } }));
dset("an empty model name is sent as it is", { model: "" }, noGet({ claude: { model: "" } }));
dset("effort only", { effort: "xhigh" }, noGet({ claude: { effort: "xhigh" } }));
dset("an agent not in the catalog", { model: "m" }, noGet({ "zed-x": { model: "m" } }), {}, "zed-x");
dset("the op is not in the record", { model: "m" }, {});
dset("the same with the cloud on", { hide: ["h"] }, picker({ hide: ["a"] }, { claude: { models: { hide: ["a", "h"] } } }), { cloud: true });

// ---------------------------------------------------------------------------------------------------------- agentDefaultsLine
const dl = (name: string, agent: string, d: Obj, extra: Partial<Case> = {}): void => add({ fn: "agentDefaultsLine", case: name, args: { agent, d }, ...extra });
dl("its own defaults", "claude", {});
dl("model only", "claude", { model: "opus" });
dl("model, effort, access", "claude", { model: "opus", effort: "high", access: "ask" });
dl("access only", "codex", { access: "plan" });
dl("hide only", "claude", { models: { hide: ["haiku"] } });
dl("hide, order, custom in that order", "claude", { models: { custom: ["c0", "custom-1"], order: ["opus", "haiku"], hide: ["x 🧪", "haiku"] } });
dl("every field", "cursor", { model: "m", effort: "e", access: "full", models: { hide: ["a", "b"], order: ["c"], custom: ["d", "e", "f"] } });
dl("empty lists say nothing", "claude", { models: { hide: [], order: [], custom: [] } });
dl("an empty models object", "claude", { models: {} });
dl("an empty list among full ones", "claude", { model: "m", models: { hide: [], order: ["o"] } });
dl("an agent the catalog does not know", "zed-x", { effort: "low" });
dl("opencode", "opencode", {});
dl("model names are not cleaned", "claude", { model: "a\tb\u0085", models: { hide: ["x y"] } });
dl("the same with the cloud on", "claude", { model: "opus" }, { cloud: true });

// ---------------------------------------------------------------------------------------------------------- envNameOf
for (const name of ["FOO", "foo_bar", "_X", "A1", "a", "_", "Z9_z"]) add({ fn: "envNameOf", case: `${name} is a name`, args: { name, named: "--unset-env" } });
for (const name of ["", "1X", "1 BAD", "a b", "A-B", "A=B", "$A", "É", "A.B", " A", "A\n"]) add({ fn: "envNameOf", case: `${ascii(JSON.stringify(name))} is refused`, args: { name, named: "--env" } });
add({ fn: "envNameOf", case: "the refusal opens with the line's own name", args: { name: "9", named: "wsp agents setup --unset-env" } });
add({ fn: "envNameOf", case: "a name that decides how an agent starts is a name here", args: { name: "LD_PRELOAD", named: "--env" } });

// ---------------------------------------------------------------------------------------------------------- agentSetupSet
const SETUP_USAGE = "usage: wsp agents setup <agent>";
const setupRow = (extra: Obj = {}): string => frame({ agent: { id: "claude", name: "Claude Code", installed: true, road: "wsp", signIn: "signed-in", signInRoad: "device", wspTools: true, setup: { on: true, envNames: [] }, ...extra } });
const sset = (name: string, askOf: Obj, script: Record<string, string>, values: Record<string, string> = {}, extra: Partial<Case> = {}, agent = "claude"): void => add({ fn: "agentSetupSet", case: name, args: { agent, ask: askOf, values, usage: SETUP_USAGE }, script: { replies: script }, ...extra });
sset("set here", { enabled: false, program: "/opt/\u{1F9EA}/claude", config: "/Users/x/claude wsp", args: ["--debug", "a \u0085 b"], unsetEnv: ["OLD"] }, { "agents.setup": ask("agents_setup", "set here", "agents.setup") });
sset("on a computer", { on: "attic", enabled: true, reset: ["program", "args"] }, { "places.list": ask("agents_setup", "on a computer", "places.list"), "agents.setup": ask("agents_setup", "on a computer", "agents.setup") });
sset("no such computer", { on: "nowhere", enabled: true }, { "places.list": ask("agents_setup", "no such computer", "places.list") });
sset("a config folder that is not absolute", { config: 'claude "wsp"' }, {});
sset("a tilde config folder is not absolute", { config: "~/claude" }, {});
sset("a dot config folder is not absolute", { config: "./claude" }, {});
sset("an empty config folder is refused", { config: "" }, {});
sset("an absolute config folder is sent as it is, never resolved", { config: "/nonexistent/../place//x/" }, { "agents.setup": setupRow() });
sset("a name a shell would not read", { unsetEnv: ["1 BAD"] }, {});
sset("the first bad unset name is named", { unsetEnv: ["OK", "BAD ONE", "2"] }, {});
sset("nothing to change", {}, {});
sset("empty args and empty lists are nothing to change", { args: [], unsetEnv: [], reset: [] }, {});
sset("refused by the host", { config: "/Users/x" }, { "agents.setup": ask("agents_setup", "refused by the host", "agents.setup") });
sset("a reset word outside the list", { reset: ["model"] }, {});
sset("a project's reset word is refused", { reset: ["after-worktree"] }, {});
sset("the first bad reset word is named", { program: "p", reset: ["config", "env", "args"] }, {});
sset("a bad reset word is refused before the computer is looked up", { on: "attic", reset: ["nope"] }, { "places.list": PLACES });
sset("nothing to change still looks the computer up first", { on: "attic" }, { "places.list": PLACES });
sset("a bad config still looks the computer up first", { on: "attic", config: "rel" }, { "places.list": PLACES });
sset("a bad unset name still looks the computer up first", { on: "attic", unsetEnv: ["1x"] }, { "places.list": PLACES });
sset("a value on a computer", { on: "place-9", program: "/p" }, { "places.list": PLACES, "agents.setup": setupRow({ id: "codex", name: "Codex" }) });
sset("values ride env", { program: "/p" }, { "agents.setup": setupRow() }, { FOO: "bar", EMPTY: "" });
sset("values alone are a change", {}, { "agents.setup": setupRow({ setup: { on: true, envNames: ["FOO"] } }) }, { FOO: "bar" });
sset("values and unset names, values first", { unsetEnv: ["OLD", "OLD2"] }, { "agents.setup": setupRow() }, { FOO: "bar", BAZ: "q" });
sset("an unset name that is also a value is null in the value's place", { unsetEnv: ["FOO"] }, { "agents.setup": setupRow() }, { FOO: "bar", BAR: "b" });
sset("an unset name twice is one key", { unsetEnv: ["A", "A"] }, { "agents.setup": setupRow() });
sset("a value's name is not checked here", { program: "p" }, { "agents.setup": setupRow() }, { "1 BAD": "x" });
sset("enabled true alone", { enabled: true }, { "agents.setup": setupRow() });
sset("enabled false alone", { enabled: false }, { "agents.setup": setupRow({ setup: { on: false, envNames: [] } }) });
sset("an empty program is sent as it is", { program: "" }, { "agents.setup": setupRow() });
sset("reset program", { reset: ["program"] }, { "agents.setup": setupRow() });
sset("reset config", { reset: ["config"] }, { "agents.setup": setupRow() });
sset("reset all three, in the patch's own order", { reset: ["args", "config", "program"] }, { "agents.setup": setupRow() });
sset("a value wins over a reset", { program: "/p", config: "/c", args: ["-x"], reset: ["program", "config", "args"] }, { "agents.setup": setupRow() });
sset("empty args with a reset put args back", { args: [], reset: ["args"] }, { "agents.setup": setupRow() });
sset("args without a reset", { args: ["-a", "-b c"] }, { "agents.setup": setupRow({ setup: { on: true, args: ["-a", "-b c"], envNames: [] } }) });
sset("the row comes back with its setup", { program: "/opt/claude", args: ["--debug"] }, { "agents.setup": setupRow({ setup: { on: true, program: "/opt/claude", args: ["--debug"], envNames: [] } }) });
sset("the row has no setup", { enabled: true }, { "agents.setup": frame({ agent: { id: "codex", name: "Codex", installed: false, road: "none", signIn: "none", signInRoad: "code", wspTools: false } }) });
sset("the host takes the setup of an agent not in the catalog", { enabled: true }, { "agents.setup": setupRow({ id: "zed-x", name: "zed-x" }) }, {}, {}, "zed-x");
sset("the op is not in the record", { enabled: true }, {});
sset("the same with the cloud on", { enabled: true }, { "agents.setup": setupRow() }, {}, { cloud: true });

// ---------------------------------------------------------------------------------------------------------- agentSetupLines
const row = (name: string, setup: Obj | undefined): Obj => ({ id: "claude", name, installed: true, road: "wsp", signIn: "signed-in", signInRoad: "device", wspTools: true, ...(setup !== undefined ? { setup } : {}) });
const sl = (label: string, r: Obj, configMoved: boolean, extra: Partial<Case> = {}): void => add({ fn: "agentSetupLines", case: label, args: { row: r, configMoved }, ...extra });
sl("no setup reads as on", row("Claude Code", undefined), false);
sl("on with nothing set", row("Claude Code", { on: true, envNames: [] }), false);
sl("off", row("Codex", { on: false, envNames: [] }), false);
sl("every fact", row("Claude Code", { on: true, program: "/opt/claude", configDir: "/Users/x/claude wsp", args: ["--debug", "a b"], envNames: ["FOO", "BAR"] }), false);
sl("a moved config folder asks for a sign-in", row("Claude Code", { on: true, configDir: "/Users/x/c", envNames: [] }), true);
sl("a moved config folder with no config dir says nothing", row("Claude Code", { on: true, envNames: [] }), true);
sl("config dir not moved says nothing more", row("Claude Code", { on: true, configDir: "/c", envNames: [] }), false);
sl("empty args print an empty list", row("Claude Code", { on: true, args: [], envNames: [] }), false);
sl("one variable", row("Claude Code", { on: true, envNames: ["A"] }), false);
sl("control characters in program, folder and args are cleaned", row("Claude Code", { on: true, program: "/p\x1b[2J\nx", configDir: "/c\t\x9bd", args: ["a\nb", "c\x07"], envNames: [] }), true);
sl("the row's own name is used, not the catalog's", row("my claude", { on: true, configDir: "/c", envNames: [] }), true);
sl("the same with the cloud on", row("Claude Code", { on: true, program: "/p", envNames: [] }), false, { cloud: true });

// ---------------------------------------------------------------------------------------------------------- projectDefaultsSet
const DEFAULTS = { "proj-1": { agent: { value: "codex", from: "project" }, model: { value: "gpt", from: "default" }, effort: { value: "low", from: "catalog" }, access: { value: "full", mode: "bypass", from: "project" } } };
const pscript = (echo: Obj | undefined, defaults: Obj = DEFAULTS): Record<string, string> => ({ "projects.resolve": frame({ project: PROJECT }), "preferences.set": prefs({ projectDefaults: echo === undefined ? {} : { "proj-1": echo } }), "projects.defaults": frame({ defaults }) });
const pset = (name: string, askOf: Obj, script: Record<string, string>, extra: Partial<Case> = {}, ref = "wsp"): void => add({ fn: "projectDefaultsSet", case: name, args: { ref, ask: askOf }, script: { replies: script }, ...extra });
pset("set", { agent: "codex", access: "full" }, { "projects.resolve": ask("projects_set", "set", "projects.resolve"), "preferences.set": ask("projects_set", "set", "preferences.set"), "projects.defaults": ask("projects_set", "set", "projects.defaults") });
pset("put back", { reset: ["agent", "access"] }, { "projects.resolve": ask("projects_set", "put back", "projects.resolve"), "preferences.set": ask("projects_set", "put back", "preferences.set"), "projects.defaults": ask("projects_set", "put back", "projects.defaults") });
pset("no defaults answered", { effort: "low" }, { "projects.resolve": ask("projects_set", "no defaults answered", "projects.resolve"), "preferences.set": ask("projects_set", "no defaults answered", "preferences.set"), "projects.defaults": ask("projects_set", "no defaults answered", "projects.defaults") });
pset("defaults answered for another project only", { effort: "low" }, pscript({ effort: "low" }, { "proj-2": DEFAULTS["proj-1"] }));
pset("nothing to change", {}, {});
pset("empty trackers and no reset are nothing to change", { trackers: [], reset: [] }, {});
pset("a blank after-worktree command", { afterWorktree: " " }, {});
pset("an empty after-worktree command is blank", { afterWorktree: "" }, {});
pset("a tab and a newline are blank", { afterWorktree: "\t\n " }, {});
pset("an after-worktree command is sent untrimmed", { afterWorktree: "  make setup  " }, pscript({ afterWorktree: "make setup" }));
pset("after-worktree beside a model, the patch order is fixed", { afterWorktree: "pnpm i", model: "m" }, pscript({ model: "m", afterWorktree: "pnpm i" }));
pset("a blank command is refused before a bad tracker", { afterWorktree: " ", trackers: ["lab"] }, {});
pset("a bad tracker is refused before a bad reset word", { trackers: ["lab"], reset: ["nope"] }, {});
pset("a bad reset word is refused before an access word", { access: "bad", reset: ["nope"] }, {});
pset("a bad access word is refused before any frame", { access: "bypass" }, {});
pset("an access word is refused before the project is resolved even when it is the only field", { access: "" }, {});
pset("nothing to change is refused before the project is resolved", { reset: [] }, {});
for (const word of ["lab", "acme/", "/map", "a/b/c", " acme/lab", "acme/lab ", "acme /lab", "", "acme\t/lab", "acme\n/lab"]) pset(`tracker ${ascii(JSON.stringify(word))} names no repo`, { trackers: ["acme/lab", word] }, {});
for (const word of ["acme/lab", "a/b", "my-org/my.repo", "é/é", "a b/c".replace(" ", "_")]) pset(`tracker ${ascii(word)} is a repo`, { trackers: [word] }, pscript({ trackers: [word] }));
pset("the first bad tracker is named", { trackers: ["ok/ok", "bad1", "bad2"] }, {});
pset("trackers are de-duplicated in first-seen order", { trackers: ["acme/lab", "acme/map", "acme/lab"] }, pscript({ trackers: ["acme/lab", "acme/map"] }));
pset("trackers echo with the whole list", { trackers: ["acme/lab", "acme/map"] }, pscript({ trackers: ["acme/lab", "acme/map"] }), {}, "h-1");
pset("an empty trackers list with a reset of trackers puts it back", { trackers: [], reset: ["trackers"] }, pscript(undefined));
pset("an echo with an empty trackers list is returned", { effort: "low" }, pscript({ effort: "low", trackers: [] }));
pset("reset trackers and after-worktree", { reset: ["trackers", "after-worktree"] }, pscript(undefined));
pset("reset after-worktree alone", { reset: ["after-worktree"] }, pscript(undefined));
pset("reset every field, in the patch's order", { reset: ["trackers", "after-worktree", "access", "effort", "model", "agent"] }, pscript(undefined));
pset("a value wins over a reset", { agent: "a", model: "m", effort: "e", access: "plan", afterWorktree: "x", trackers: ["o/n"], reset: ["agent", "model", "effort", "access", "after-worktree", "trackers"] }, pscript({ agent: "a", model: "m", effort: "e", access: "plan", afterWorktree: "x", trackers: ["o/n"] }));
pset("a reset word that is a field's key spelling", { reset: ["afterWorktree"] }, {});
pset("a singular tracker reset word", { reset: ["tracker"] }, {});
pset("an agent's reset word is refused", { reset: ["models"] }, {});
pset("every field set", { agent: "claude", model: "m", effort: "e", access: "ask", afterWorktree: "make", trackers: ["a/b"] }, pscript({ agent: "claude", model: "m", effort: "e", access: "ask", afterWorktree: "make", trackers: ["a/b"] }));
pset("access auto-edit", { access: "auto-edit" }, pscript({ access: "auto-edit" }));
pset("access plan goes to the host", { access: "plan" }, { "projects.resolve": frame({ project: PROJECT }), "preferences.set": refused("Claude Code takes no plan access; it takes ask, auto-edit, full", "usage") });
pset("only another project's overrides echo", { model: "m" }, { "projects.resolve": frame({ project: PROJECT }), "preferences.set": prefs({ projectDefaults: { "proj-2": { trackers: ["x/y"], afterWorktree: "z" } } }), "projects.defaults": frame({ defaults: DEFAULTS }) });
pset("no such project", { model: "m" }, { "projects.resolve": ask("projects_set", "no such project", "projects.resolve") }, {}, "nope");
pset("the host refuses the overrides: the defaults are not read", { model: "m" }, { "projects.resolve": frame({ project: PROJECT }), "preferences.set": refused("no harness named x", "usage") });
pset("the defaults read is refused", { model: "m" }, { "projects.resolve": frame({ project: PROJECT }), "preferences.set": prefs({}), "projects.defaults": refused("defaults are not readable", "provider") });
pset("a project name with control characters in the missing-defaults error", { model: "m" }, { "projects.resolve": frame({ project: { ...PROJECT, name: "my\tproj\u0085" } }), "preferences.set": prefs({}), "projects.defaults": frame({ defaults: {} }) });
pset("the defaults carry only some of the picks", { model: "m" }, pscript({ model: "m" }, { "proj-1": { agent: { value: "claude", from: "catalog" } } }));
pset("the same with the cloud on", { trackers: ["acme/lab"] }, pscript({ trackers: ["acme/lab"] }), { cloud: true });
pset("the op is not in the record", { model: "m" }, { "projects.resolve": frame({ project: PROJECT }) });

// ---------------------------------------------------------------------------------------------------------- defaultsCell, threadDefaultsLines
const td = (agent: string, from: string, more: Obj = {}): Obj => ({ agent: { value: agent, from }, ...more });
const dc = (label: string, d: Obj | undefined, extra: Partial<Case> = {}): void => add({ fn: "defaultsCell", case: label, ...(d !== undefined ? { args: { d } } : {}), ...extra });
dc("nothing read", undefined);
dc("an agent only", td("claude", "catalog"));
dc("every pick, none from the project", td("claude", "catalog", { model: { value: "opus", from: "default" }, effort: { value: "high", from: "named" }, access: { value: "full", mode: "bypass", from: "catalog" } }));
dc("every pick from the project", td("codex", "project", { model: { value: "gpt", from: "project" }, effort: { value: "low", from: "project" }, access: { value: "ask", mode: "ask", from: "project" } }));
dc("effort is not in the cell", td("claude", "catalog", { effort: { value: "high", from: "project" } }));
dc("the model absent, the access from the project", td("claude", "default", { access: { value: "plan", mode: "plan", from: "project" } }));
dc("the agent from the project alone", td("codex", "project"));
dc("a value with spaces and controls is not cleaned", td("a b", "catalog", { model: { value: "m\tx", from: "project" } }));
dc("the same with the cloud on", td("claude", "project"), { cloud: true });
const tl = (label: string, d: Obj, extra: Partial<Case> = {}): void => add({ fn: "threadDefaultsLines", case: label, args: { d }, ...extra });
tl("an agent only", td("claude", "catalog"));
tl("every pick", td("codex", "project", { model: { value: "gpt \u0085", from: "default" }, effort: { value: "low", from: "catalog" }, access: { value: "full", mode: "bypass", from: "named" } }));
for (const from of ["named", "project", "default", "catalog"]) tl(`the agent from ${from}`, td("claude", from));
tl("an agent the catalog does not know is named by its id", td("zed-x", "default"));
tl("a model alone beside the agent", td("claude", "named", { model: { value: "m", from: "project" } }));
tl("an effort alone beside the agent", td("claude", "named", { effort: { value: "e", from: "project" } }));
tl("an access alone beside the agent: the value, not the mode", td("claude", "named", { access: { value: "auto-edit", mode: "acceptEdits", from: "project" } }));
tl("values are not cleaned", td("claude", "named", { model: { value: "a\tb\nc", from: "default" } }));
tl("the same with the cloud on", td("claude", "named"), { cloud: true });

// ---------------------------------------------------------------------------------------------------------- the words
const consts = [
  "noSkillHitsLine", "SKILL_HIT_COLUMNS", "AGENT_SET_RESETS", "PROJECT_SET_RESETS", "AGENT_SETUP_RESETS", "agentSetNothingLine", "NOTHING_TO_SET_FIX", "ENV_NAME_FIX", "agentSetupNothingLine", "SETUP_WORDS", "projectSetNothingLine", "TRACKER_SHAPE_FIX", "afterWorktreeBlankLine", "AFTER_WORKTREE_BLANK_FIX", "FROM_WORDS", "DEFAULTS_LABELS", "SKILL_CHANGE_WORDS", "AGENTS_ON_WORDS", "AGENTS_READ_WORDS",
];
for (const name of consts) add({ fn: "constText", case: name, args: { name } });
add({ fn: "constText", case: "the same words with the cloud on", cloud: true, args: { name: "AGENTS_READ_WORDS" } });
const sentences: [string, unknown[]][] = [
  ["previewCutLine", [65]], ["previewCutLine", [1]], ["previewCutLine", [0]],
  ["skillsShPlacelessLine", ["o/r/s"]],
  ["isInLine", ["pdf", "~/.agents/skills/pdf"]], ["isInAlsoLine", ["pdf", "~/a", "claude's ~/b, codex's ~/c"]], ["agentCopyWords", ["Claude Code", "~/.claude/skills/pdf"]],
  ["goneFromLine", ["pdf", "~/a, ~/b"]], ["turnedInLine", ["pdf", true, "~/a/SKILL.md"]], ["turnedInLine", ["pdf", false, "~/a/SKILL.md"]], ["turnedLine", ["pdf", true]], ["turnedLine", ["pdf", false]],
  ["PICKER_WORDS.hide", ["a b"]], ["PICKER_WORDS.order", ["a b"]], ["PICKER_WORDS.custom", ["a b"]],
  ["startsOnOwnLine", ["Claude Code"]], ["startsOnLine", ["Claude Code", "opus; high"]], ["defaultAgentLine", ["codex"]], ["defaultAgentLine", ["zed-x"]],
  ["envNameLine", ["--unset-env", '"1 BAD"']], ["setupOnLine", ["Claude Code", true]], ["setupOnLine", ["Claude Code", false]],
  ["trackerShapeLine", ["lab"]], ["trackerShapeLine", ['a"b\n']], ["trackersLine", [["acme/lab"]]], ["trackersLine", [["acme/lab", "acme/map"]]], ["trackersLine", [[]]],
  ["afterWorktreeLine", ["make setup"]], ["noDefaultsAnsweredLine", ["wsp"]], ["newThreadsHeadLine", ["wsp"]], ["defaultsValueLine", ["agent", "Codex", "this project's"]],
];
for (const [name, a] of sentences) add({ fn: "sentence", case: `${name} ${JSON.stringify(a)}`, args: { name, a } });
add({ fn: "sentence", case: "newThreadsHeadLine with the cloud on", cloud: true, args: { name: "newThreadsHeadLine", a: ["wsp"] } });

// ---------------------------------------------------------------------------------------------------------- the runs
const agents = (mod: Obj): any => mod["agents"];
const runs: Record<string, Run> = {
  searchSkillsSh: ({ mod, client, args }) => agents(mod).searchSkillsSh(client, args["q"], args["limit"]),
  skillHitLines: ({ mod, args }) => Promise.resolve(agents(mod).skillHitLines(args["hits"])),
  skillShown: ({ mod, client, args }) => agents(mod).skillShown(client, args["skill"], args["thread"], args["on"], args["project"] ?? { project: false }, args["usage"], args["line"]),
  shownText: ({ mod, args }) => Promise.resolve(agents(mod).shownText(args["p"])),
  skillAdded: ({ mod, client, args }) => agents(mod).skillAdded(client, args["skill"], args["thread"], args["on"], args["agents"], args["project"] ?? { project: false }, args["usage"], args["line"]),
  addedLine: ({ mod, args }) => Promise.resolve(agents(mod).addedLine(args["skill"], args["a"])),
  skillChanged: ({ mod, client, args }) => agents(mod).skillChanged(client, args["op"], args["name"], args["thread"], args["on"], args["project"] ?? { project: false }, args["turn"], args["usage"], args["line"]),
  removedLine: ({ mod, args }) => Promise.resolve(agents(mod).removedLine(args["name"], args["removed"])),
  defaultAgentSet: ({ mod, client, args }) => agents(mod).defaultAgentSet(client, args["agent"]),
  agentDefaultsSet: ({ mod, client, args }) => agents(mod).agentDefaultsSet(client, args["agent"], args["ask"]),
  agentDefaultsLine: ({ mod, args }) => Promise.resolve(agents(mod).agentDefaultsLine(args["agent"], args["d"])),
  envNameOf: ({ mod, args }) => Promise.resolve(agents(mod).envNameOf(args["name"], args["named"])),
  agentSetupSet: ({ mod, client, args }) => agents(mod).agentSetupSet(client, args["agent"], args["ask"], args["values"] ?? {}, args["usage"]),
  agentSetupLines: ({ mod, args }) => Promise.resolve(agents(mod).agentSetupLines(args["row"], args["configMoved"])),
  projectDefaultsSet: ({ mod, client, args }) => agents(mod).projectDefaultsSet(client, args["ref"], args["ask"]),
  defaultsCell: ({ mod, args }) => Promise.resolve(agents(mod).defaultsCell(args["d"])),
  threadDefaultsLines: ({ mod, args }) => Promise.resolve(agents(mod).threadDefaultsLines(args["d"])),
  constText: ({ mod, args }) => Promise.resolve(agents(mod)[args["name"] as string]),
  sentence: ({ mod, args }) => {
    const [head, key] = (args["name"] as string).split(".");
    const f = key === undefined ? agents(mod)[head!] : agents(mod)[head!][key];
    return Promise.resolve(f(...(args["a"] as unknown[])));
  },
};
it("agents_set", async () => {
  await record("agents_set", cases, runs);
});
