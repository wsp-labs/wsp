// SPDX-License-Identifier: AGPL-3.0-only
// Fixtures for spec/skills.md: skills.ts on the edge inputs of the wave-2 audit (group d). `agents` args are catalog agent ids;
// plugin roots are not reachable from a fixture (a plugin read holds a shelf), so skill_roots always gets no plugins.
// Real-folder cases carry {"tree": {"<rel>": "<text>" | {"link": "<target>"} | {"dir": true}}}: paths and link targets written
// "<dir>/<rel>" stand for the folder the test makes.
import { mkdirSync, mkdtempSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { it } from "vitest";
import { CATALOG_AGENTS } from "../../../../packages/catalog/src/index.js";
import { detectSkills, skillFrontmatter, skillMdFrontmatter, skillRoots, type SkillRootAt } from "../../../../packages/collect/src/detect/skills.js";
import { nodeHost } from "../../../../packages/collect/src/live-host.js";
import { type Laptop, host, plain, settle, write } from "./w2-host.js";

type Case = Record<string, unknown>;
type Project = { id: string; name: string; path: string };
const pure = (name: string, f: (...a: never[]) => unknown, ...args: unknown[]): Case => ({ name, args, out: plain((f as (...a: unknown[]) => unknown)(...args)) });
const agentsOf = (ids: string[] | null) => (ids === null ? undefined : ids.map(id => CATALOG_AGENTS.find(a => a.id === id)!));
const keys = (calls: string[]): string[] => calls.filter(c => c.startsWith("run sh -c ")).map(c => c.slice(4).replace(/ \(20000 ms\)$/, ""));

const REC = (root: string, dir: string, link = "", off = "", head = ""): string => `\x1e${root}\x1f${dir}\x1f${link}\x1f${off}\x1f${head}`;
const rt = (dir: string, scope: SkillRootAt["scope"] = "user", o: Partial<SkillRootAt> = {}): SkillRootAt => ({ dir, scope, ...o });
const R = "/Users/dev/.a/skills";

/** detect_skills over a canned answer: the key the fake host matches is the one the TS itself runs. */
async function canned(name: string, roots: SkillRootAt[], said: string | undefined): Promise<Case> {
  const probe = host({});
  await detectSkills(probe, roots);
  const exec: Record<string, string> = said === undefined ? {} : Object.fromEntries(keys(probe.calls).map(k => [k, said]));
  const laptop: Laptop = { exec };
  const h = host(laptop);
  const r = await settle(() => detectSkills(h, roots));
  return { name, laptop, args: [roots], ...r, calls: h.calls };
}

/** skill_roots with each project's link check answered by `answers[i]` (nothing where undefined). */
async function roots(name: string, agents: string[] | null, projects: Project[], answers: (string | undefined)[], stores?: Record<string, string>): Promise<Case> {
  const base: Laptop = stores ? { stores } : {};
  const probe = host(base);
  await skillRoots(probe, { agents: agentsOf(agents), projects });
  const exec: Record<string, string> = {};
  keys(probe.calls).forEach((k, i) => { if (answers[i] !== undefined) exec[k] = answers[i]!; });
  const laptop: Laptop = { ...base, exec };
  const h = host(laptop);
  const r = await settle(() => skillRoots(h, { agents: agentsOf(agents), projects }));
  return { name, laptop, args: [agents, projects, []], ...r, calls: h.calls };
}

type Tree = Record<string, string | { link: string } | { dir: true }>;
/** detect_skills over a real folder made from `tree`. */
async function real(name: string, tree: Tree, rootsIn: SkillRootAt[]): Promise<Case> {
  const base = mkdtempSync(join(tmpdir(), "w2-skills-"));
  const put = (v: string): string => v.split("<dir>").join(base);
  for (const [rel, v] of Object.entries(tree)) {
    const at = put(rel.startsWith("<dir>/") ? rel : `<dir>/${rel}`);
    if (typeof v === "string") { mkdirSync(dirname(at), { recursive: true }); writeFileSync(at, v); } else if ("dir" in v) mkdirSync(at, { recursive: true });
  }
  for (const [rel, v] of Object.entries(tree)) {
    if (typeof v === "string" || !("link" in v)) continue;
    const at = put(rel.startsWith("<dir>/") ? rel : `<dir>/${rel}`);
    mkdirSync(dirname(at), { recursive: true });
    symlinkSync(put(v.link), at);
  }
  const live = nodeHost();
  const r = await detectSkills(live, JSON.parse(put(JSON.stringify(rootsIn))));
  const back = JSON.parse(JSON.stringify(plain(r)).split(base).join("<dir>"));
  return { name, tree, args: [rootsIn], out: back };
}

const md = (body: string): string => `---\n${body}\n---\n`;

it("skills fixtures", async () => {
  const fixture: Record<string, Case[]> = {};

  fixture["skill_frontmatter"] = [
    ["no space after the colon", "name:pdf\ndescription:x"],
    ["CRLF lines", "name: pdf\r\ndescription: d\r\n"],
    ["CRLF, last line without a CR", "name: pdf\r\ndescription: d"],
    ["a lone CR inside a value", "name: a\rb"],
    ["U+2028 inside a value", "name: a b"],
    ["blanks after the colon", "name:   \ndescription: d"],
    ["a tab after the colon", "name:\tpdf"],
    ["an indented key", " name: x\ndescription: y"],
    ["a key in capitals", "Name: x"],
    ["an empty name then a name", "name:\nname: x"],
    ["an empty name then an indented line", "name:\n  foo\n  bar"],
    ["an empty quoted value", 'name: ""\ndescription: d'],
    ["an empty single-quoted name then a name", "name: ''\nname: x"],
    ["text after a closing double quote", 'name: "a" b'],
    ["text after a closing single quote", "name: 'a' b 'c'"],
    ["a quote inside single quotes", "name: 'a'b'"],
    ["an unterminated double quote", 'name: "unterminated'],
    ["a backslash n in double quotes", 'name: "a\\nb"'],
    ["a double quote ending in a backslash", 'name: "a\\"'],
    ["escapes in double quotes", 'name: "a\\\\b\\"c\\td"'],
    ["a plain value with a comment", "name: a #c"],
    ["a plain value with a tab then a comment", "name: a\t#c"],
    ["a comment only, then a name", "name: #c\nname: z"],
    ["a hash with no space before it", "description: http://x/#frag"],
    ["inner spaces", "description: a   b"],
    ["NBSP around a value", "name:  x "],
    ["a BOM before a value", "name: ﻿x"],
    ["a folded block", "description: >\n  a\n  b\nname: n"],
    ["a literal block keep", "description: |+\n  a\n  b"],
    ["a literal block strip", "description: |-\n  a\n  b"],
    ["a blank line ends a block", "description: |\n  a\n\n  b"],
    ["a comment after the block indicator", "description: > # c\n  a\n  b"],
    ["a numeric block indicator", "description: >2\n  a"],
    ["a block with no continuation", "description: >\nname: n"],
    ["a nested key as a continuation", "description: x\n  nested: y\n  - item"],
    ["a quoted continuation is not unquoted", 'description: "a"\n  "b"'],
    ["a comment line as a continuation", "description: a\n  # c\n  b"],
    ["a tab-indented continuation", "description: a\n\tb"],
    ["a space-only line ends a continuation", "description: a\n   \n  b"],
    ["description before name", "description: d\nname: n"],
    ["names counting: empty and quoted keys count", 'name: a\n"name": b\n\'name\': c\nname:\n'],
    ["names counting: a continuation line is not counted", "name: a\n  name: b\nname: c"],
    ["description repeated: the first", "description: a\ndescription: b"],
    ["description empty then a value", "description:\ndescription: b"],
    ["quoted keys", "\"description\": d\n'name': n"],
    ["space or tab before the colon", "name  : n\ndescription\t: d"],
    ["other keys", "title: t\nauthor: a\nname-x: y"],
    ["CRLF inside a block", "description: >\r\n  a\r\n  b\r\n"],
    ["empty text", ""],
    ["only a newline", "\n"],
    ["two blocks", "description: >\n  a\nname: >\n  b"],
  ].map(([n, t]) => pure(n!, skillFrontmatter, t));

  const SPLIT = "a".repeat(4078) + "\u{1F600}";
  fixture["skill_md_frontmatter"] = [
    ["the opening line with a trailing space", "--- \nname: x\n---\n"],
    ["a BOM before the opening line", "﻿---\nname: x\n---\n"],
    ["a blank first line", "\n---\nname: x\n---\n"],
    ["unclosed", "---\nname: x\ndescription: y"],
    ["a closing line with a trailing space is no close", "---\nname: x\n--- \ndescription: y\n---\n"],
    ["three dots are no close", "---\nname: x\n...\ndescription: y\n---\n"],
    ["CRLF", "---\r\nname: x\r\ndescription: y\r\n---\r\nname: z\r\n"],
    ["a lone CR inside a value", "---\nname: a\rb\n---\n"],
    ["only the opening line", "---"],
    ["an empty front", "---\n---\n"],
    ["a second document is ignored", "---\nname: a\n---\nname: b\n"],
    ["head exactly at the cut", `---\ndescription: ${"a".repeat(4079)}bcd\n---\n`],
    ["the closing line is past the cut", `---\ndescription: ${"a".repeat(5000)}\n---\n`],
  ].map(([n, t]) => pure(n!, skillMdFrontmatter, t)).concat([{
    name: "an emoji split by the cut leaves a lone high surrogate; out is the description's UTF-16 length and last unit",
    args: [`---\ndescription: ${SPLIT}\n---\n`],
    out: (() => { const d = skillMdFrontmatter(`---\ndescription: ${SPLIT}\n---\n`).description!; return { units: d.length, lastUnit: d.charCodeAt(d.length - 1) }; })(),
  }]);

  const P = (id: string, path: string, name = id): Project => ({ id, name, path });
  const LINE = (dir: string, link: string, to?: string): string => `${dir}\t${link}${to === undefined ? "" : `\t${to}`}\n`;
  const END = "\x1eEND\n";
  const p1 = "/Users/dev/p";
  const sr: Case[] = (fixture["skill_roots"] = []);
  sr.push(await roots("every catalog agent's user folders", null, [], []));
  sr.push(await roots("a shared folder takes the agent whose own it is", ["opencode", "claude"], [], []));
  sr.push(await roots("a store moves only the agent's own state folder", ["opencode", "claude"], [], [], { claude: "/Users/dev/store" }));
  sr.push(await roots("a project whose links could not be told (no answer)", ["claude"], [P("p", p1)], [undefined]));
  sr.push(await roots("a project with nothing linked out", ["claude", "codex"], [P("p", p1)], [END]));
  sr.push(await roots("the home folder as a project, check fails: user folders become skipped", ["claude", "codex"], [P("h", "/Users/dev", "home")], [undefined]));
  sr.push(await roots("the home folder as a project, nothing linked", ["claude", "codex"], [P("h", "/Users/dev", "home")], [END]));
  sr.push(await roots("the home folder as a project, one user folder linked out", ["claude", "codex"], [P("h", "/Users/dev", "home")], [LINE("/Users/dev/.claude/skills", "/Users/dev/.claude/skills", "/etc/x") + END]));
  sr.push(await roots("an agent with no project folders still runs the check", ["hermes"], [P("p", p1)], [END]));
  sr.push(await roots("no agents, a project still runs the check", [], [P("p", p1)], [undefined]));
  sr.push(await roots("two projects on one path make one root and two runs", ["claude"], [P("p1", p1, "one"), P("p2", p1, "two")], [LINE(`${p1}/.claude/skills`, `${p1}/.claude`, "/e") + END, LINE(`${p1}/.claude/skills`, `${p1}/.claude`, "/e") + END]));
  sr.push(await roots("two projects keep project order", ["claude"], [P("p1", "/Users/dev/p1"), P("p2", "/Users/dev/p2")], [END, END]));
  sr.push(await roots("a project path with a trailing slash", ["claude", "gemini"], [P("p", "/Users/dev/p/")], [END]));
  sr.push(await roots("a project path with dot dot", ["claude"], [P("p", "/Users/dev/p/../q")], [END]));
  sr.push(await roots("link lines: the later wins, a fourth field is ignored, a missing target is empty", ["claude", "gemini"], [P("p", p1)], [LINE(`${p1}/.claude/skills`, "/l1", "/t1") + LINE(`${p1}/.claude/skills`, `${p1}/.claude/skills`, "/t2") + LINE(`${p1}/.gemini/skills`, `${p1}/.gemini`) + "\x1eEND"]));
  sr.push(await roots("a link line under home, and an end mark followed by spaces", ["claude"], [P("p", p1)], [LINE(`${p1}/.claude/skills`, `${p1}/.claude`, "/Users/dev/x") + "\x1eEND \n"]));
  sr.push(await roots("a link line for a folder that was not asked", ["claude"], [P("p", p1), P("q", "/Users/dev/q")], [LINE("/Users/dev/q/.claude/skills", "/l", "/t") + END, END]));
  sr.push(await roots("an empty answer is no answer", ["claude"], [P("p", p1)], [""]));
  sr.push(await roots("an answer with text after the end mark is no answer", ["claude"], [P("p", p1)], [LINE(`${p1}/.claude/skills`, `${p1}/.claude`, "/t") + END + "x"]));

  const ds: Case[] = (fixture["detect_skills"] = []);
  const A = rt(R, "user", { agent: "a" });
  ds.push(await canned("a head holding U+001F is cut at it", [A], REC(R, `${R}/s`, "", "", "name: n\x1fdescription: after\nname: b\n") + END));
  ds.push(await canned("a description before U+001F is kept", [A], REC(R, `${R}/s`, "", "", "description: before\x1fafter\n") + END));
  ds.push(await canned("a record of four pieces is skipped", [rt(R)], `\x1e${R}\x1f${R}/s\x1f\x1f` + END));
  ds.push(await canned("five pieces with an empty head", [rt(R)], REC(R, `${R}/s`) + END));
  ds.push(await canned("six pieces: the fifth is the head", [rt(R)], `${REC(R, `${R}/s`, "", "", "description: d")}\x1fextra\x1fmore` + END));
  ds.push(await canned("text before the first record is dropped", [rt(R)], "noise\n" + REC(R, `${R}/s`, "", "", "description: d\n") + END));
  ds.push(await canned("only the end mark", [rt(R)], END));
  ds.push(await canned("an empty answer is cut short, not unreadable", [rt(R)], ""));
  ds.push(await canned("no answer", [rt(R)], undefined));
  ds.push(await canned("blanks after the end mark", [rt(R)], REC(R, `${R}/s`) + "\x1eEND \n\n  "));
  ds.push(await canned("text after the end mark", [rt(R)], REC(R, `${R}/s`) + "\x1eEND\nx"));
  ds.push(await canned("NBSP after the end mark", [rt(R)], REC(R, `${R}/s`) + "\x1eEND "));
  const mixed = [rt(R, "project", { skipped: "skills: S1" }), rt("/b/s"), rt("/c/s", "project", { skipped: "skills: S2" })];
  ds.push(await canned("refusal order: cut short, then the skipped lines", mixed, "\x1eEN"));
  ds.push(await canned("refusal order: unreadable, then the skipped lines", mixed, undefined));
  ds.push(await canned("refusal order: whole, the skipped lines only", mixed, END));
  ds.push(await canned("only skipped roots run nothing", [rt(R, "project", { skipped: "skills: S1" })], "NEVER"));
  ds.push(await canned("a duplicate root dir: the last root wins", [rt("/r", "user", { agent: "x" }), rt("/r", "project", { project: { id: "p", name: "P", path: "/Users/dev/p" } })], REC("/r", "/r/s") + END));
  ds.push(await canned("a root with a trailing slash accepts only dirs under two slashes", [rt("/r/")], REC("/r/", "/r//s") + REC("/r/", "/r/s") + END));
  ds.push(await canned("a dir equal to its root", [rt("/r")], REC("/r", "/r") + END));
  ds.push(await canned("dot segments", [rt("/r")], REC("/r", "/r/.h/x") + REC("/r", "/r/a/.b") + REC("/r", "/r/a./b") + REC("/r", "/r/.") + REC("/r", "/r/..x") + REC("/r", "/r/ok") + END));
  ds.push(await canned("a skill inside a skill is not a skill, whatever the order", [rt("/r")], REC("/r", "/r/cat", "", "", "description: cat") + REC("/r", "/r/cat/s", "", "", "description: s") + REC("/r", "/r/cat2/s", "", "", "description: s2") + END));
  ds.push(await canned("the inner record first", [rt("/r")], REC("/r", "/r/cat/s", "", "", "description: s") + REC("/r", "/r/cat", "", "", "description: cat") + END));
  ds.push(await canned("the ancestor is looked up under the same root only", [rt("/r"), rt("/r/cat")], REC("/r", "/r/cat", "", "", "description: cat") + REC("/r", "/r/cat/s") + REC("/r/cat", "/r/cat/s") + END));
  ds.push(await canned("three folders deep", [rt("/r")], REC("/r", "/r/a/b/c", "", "", "description: deep") + END));
  ds.push(await canned("the first record builds the row: a later description is not merged", [rt("/r1", "user", { agent: "x" }), rt("/r2", "user", { agent: "y" })], REC("/r1", "/r1/pdf") + REC("/r2", "/r2/pdf", "", "", "description: later") + END));
  ds.push(await canned("the first path keeps its agent", [rt("/r", "user", { agent: "x" }), rt("/r/cat", "user", { agent: "y" })], REC("/r", "/r/cat/s") + REC("/r/cat", "/r/cat/s") + END));
  ds.push(await canned("link targets resolve against the folder's parent", [rt("/r", "user", { agent: "x" })], REC("/r", "/r/a", "../x") + REC("/r", "/r/b", "/abs/t") + REC("/r", "/r/c", "sub/") + REC("/r", "/r/d", "../../../../up") + REC("/r", "/r/e", ".") + REC("/r", "/r/f", "/Users/dev/y/./z/") + REC("/r", "/r/g", "~/x") + END));
  ds.push(await canned("link targets under home", [rt("/Users/dev/s", "user", { agent: "x" })], REC("/Users/dev/s", "/Users/dev/s/a", "../dot/a") + REC("/Users/dev/s", "/Users/dev/s/b", "/Users/dev") + END));
  ds.push(await canned("off records and plugin roots", [rt("/pl", "plugin", { agent: "claude", plugin: { id: "p@m", prefix: "pre", on: false } }), rt("/pl2", "plugin", { agent: "claude", plugin: { id: "q@m", prefix: "pre", on: true } })], REC("/pl", "/pl/x", "", "1", "description: d1") + REC("/pl2", "/pl2/x", "", "", "description: d2") + REC("/pl", "/pl/y", "", "", "") + END));
  ds.push(await canned("project rows and a user row of one name", [rt("/p1/s", "project", { agent: "x", project: { id: "p1", name: "Ann", path: "/Users/dev/code/p1" } }), rt("/p2/s", "project", { project: { id: "p2", name: "Ann", path: "/Users/dev/code/p2" } }), rt("/u", "user")], REC("/p1/s", "/p1/s/a") + REC("/p2/s", "/p2/s/a") + REC("/u", "/u/a") + END));
  const NAMES = ["a-b", "ab", "a_b", "a b", "A", "a", "B", "é", "e", "f", "10", "9", "Z", "z", "a1", "aB", "Ab", "ä", "ñ", "n", "ß", "ss", "中", "a.b", "-x", "x-"];
  ds.push(await canned("rows sort by localeCompare of the name", [rt("/r")], NAMES.map(n => REC("/r", `/r/${n}`)).join("") + END));
  ds.push(await canned("one name in a project, a user folder and a plugin", [rt("/r", "user"), rt("/p", "project", { project: { id: "p", name: "P", path: "/p" } }), rt("/pl", "plugin", { plugin: { id: "i@m", prefix: "x", on: true } })], REC("/r", "/r/x") + REC("/p", "/p/x") + REC("/pl", "/pl/x") + END));
  ds.push(await canned("two plugin rows of one name keep insertion order", [rt("/pl", "plugin", { plugin: { id: "b@m", prefix: "x", on: true } }), rt("/pl2", "plugin", { plugin: { id: "a@m", prefix: "x", on: true } })], REC("/pl", "/pl/s") + REC("/pl2", "/pl2/s") + END));
  ds.push(await canned("two projects of one name keep insertion order", [rt("/p1", "project", { project: { id: "p1", name: "same", path: "/p1" } }), rt("/p2", "project", { project: { id: "p2", name: "same", path: "/p2" } })], REC("/p2", "/p2/s") + REC("/p1", "/p1/s") + END));
  ds.push(await canned("a record under an unknown root", [rt("/r")], REC("/nope", "/nope/s") + REC("/r/", "/r/s") + END));

  const K = (n: number) => "a".repeat(n);
  const front = (body: string) => `---\n${body}\n---\n`;
  const walk: Tree = {
    "r/s": "", "r/s/SKILL.md": front("name: s\ndescription: plain"), "r/cat/t/SKILL.md": front("description: deep"),
    "r/link": { link: "<dir>/r/s" }, "r/rel": { link: "s" },
    "big/bomfile/SKILL.md": "﻿---\ndescription: bom\n---\n",
    "big/crlf/SKILL.md": "---\r\ndescription: win\r\nname: w\r\n---\r\n",
    "big/dashes/SKILL.md": "---\ndescription: d\n---   \nname: after\n---\n",
    "big/tabs/SKILL.md": "---\ndescription:\tt\n---\n",
    "big/nofront/SKILL.md": "# title\ndescription: x\n",
    "big/off/SKILL.md.off": front("description: offd"),
    "big/offboth/SKILL.md": front("description: both"), "big/offboth/SKILL.md.off": "x",
  };
  delete walk["r/s"];
  const rs = (extra: SkillRootAt[] = []): SkillRootAt[] => [rt("<dir>/r", "user", { agent: "x" }), rt("<dir>/big"), ...extra];
  ds.push(await real("a real tree: links, categories, odd front matter, .off", walk, rs()));
  ds.push(await real("a root with a trailing slash reads nothing", { "r/s/SKILL.md": front("name: s") }, [rt("<dir>/r/", "user", { agent: "y" })]));
  ds.push(await real("the same root with and without the slash: only the plain one reads", { "r/s/SKILL.md": front("name: s") }, [rt("<dir>/r", "user", { agent: "x" }), rt("<dir>/r/", "user", { agent: "y" })]));
  ds.push(await real("a description of 3000 two-byte letters is cut at 4092 bytes of its line", { "big/m/SKILL.md": front(`description: ${"é".repeat(3000)}`) }, [rt("<dir>/big")]));
  ds.push(await real("a description that fits the byte cap exactly", { "big/m2/SKILL.md": front(`description: ${"é".repeat(2039)}x`) }, [rt("<dir>/big")]));
  ds.push(await real("a description of 4079 one-byte letters then more", { "big/m3/SKILL.md": front(`description: ${K(5000)}`) }, [rt("<dir>/big")]));
  ds.push(await real("a skill inside a skill, a dot folder and a folder with no SKILL.md", { "r/outer/SKILL.md": front("description: outer"), "r/outer/examples/SKILL.md": front("description: inner"), "r/.hidden/x/SKILL.md": front("description: h"), "r/empty": { dir: true } }, [rt("<dir>/r", "user", { agent: "x" })]));

  write("skills", fixture);
});
