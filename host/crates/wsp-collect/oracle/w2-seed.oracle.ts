// SPDX-License-Identifier: AGPL-3.0-only
// Fixtures for spec/seed.md: seed.ts on the edge inputs of the wave-2 audit (group a).
// Cases: {name, laptop, args: [folder, {claudeStateHome}], out: the SeedPlan, calls}. `calls` holds every host call in order.
import { it } from "vitest";
import { seedMenu } from "../../../../packages/collect/src/seed.js";
import { type Laptop, host, settle, write } from "./w2-host.js";

type Case = Record<string, unknown>;
const git = (folder: string, ...args: string[]): string => ["git", "-C", folder, ...args].join(" ");
const LS = ["ls-files", "-z", "-o", "-i", "--exclude-standard", "--directory"];
const IN = (p: string): string[] => ["ls-files", "-z", "-o", "-i", "--exclude-standard", "--", p];
const du = (...paths: string[]): string => `du -sk ${paths.join(" ")}`;
const T = "\t";

const seed = async (name: string, folder: string, laptop: Laptop, state = "/Users/dev/.claude"): Promise<Case> => {
  const h = host(laptop);
  const args = [folder, { claudeStateHome: state }];
  const r = await settle(() => seedMenu(h, folder, { claudeStateHome: state }));
  return { name, laptop, args, ...r, calls: h.calls };
};

it("seed fixtures", async () => {
  const cases: Case[] = [];
  const F = "/p";
  const ex = (exec: Record<string, string>, files: Laptop["files"] = {}): Laptop => ({ exec, files });

  cases.push(await seed("an opened folder: nested, duplicated, a file of the name, a near name; junk and never inner paths", F, ex({
    [git(F, ...LS)]: ["a/.claude/", "foo.claude/", ".claude/", ".claude/", "x/", "sub/node_modules/", "plain.txt", ".claude"].join("\0"),
    [git(F, ...IN("a/.claude"))]: ["a/.claude/settings.local.json", "a/.claude/.DS_Store", "a/.claude/.credentials.json", "a/.claude/other.txt", "a/.claude/.cc-writes/y"].join("\0"),
    [git(F, ...IN(".claude"))]: [".claude/settings.local.json", ".claude/CLAUDE.local.md"].join("\0"),
    [du("/p/a/.claude", "/p/foo.claude", "/p/.claude", "/p/.claude", "/p/x", "/p/sub/node_modules", "/p/plain.txt", "/p/.claude", "/p/a/.claude/settings.local.json", "/p/a/.claude/.DS_Store", "/p/a/.claude/.credentials.json", "/p/.claude/settings.local.json", "/p/.claude/CLAUDE.local.md")]: [
      "100", "7", "50", "9", "300", "1", "10", "20", "30", "5", "6",
    ].map((kb, i) => `${kb}${T}/p/${["a/.claude", "foo.claude", ".claude", "x", "sub/node_modules", "plain.txt", "a/.claude/settings.local.json", "a/.claude/.DS_Store", "a/.claude/.credentials.json", ".claude/settings.local.json", ".claude/CLAUDE.local.md"][i]}`).join("\n"),
  })));

  cases.push(await seed("the inside listing fails: the folder keeps its whole size", F, ex({
    [git(F, ...LS)]: [".claude/", "z/"].join("\0"),
    [du("/p/.claude", "/p/z")]: `50${T}/p/.claude\n4${T}/p/z`,
  })));

  const F2 = "/p/";
  cases.push(await seed("du lines: Number() parsing, the slash-only path trim, the last line for a path wins", F2, ex({
    [git(F2, ...LS)]: ["a", "b", "c", "d", "e", "f", "g", "h/", "i", "j", "k", "l", ".DS_Store", "m"].join("\0"),
    [du("/p/a", "/p/b", "/p/c", "/p/d", "/p/e", "/p/f", "/p/g", "/p/h", "/p/i", "/p/j", "/p/k", "/p/l", "/p/.DS_Store", "/p/m")]: [
      `0x10${T}/p/a`, `${T}/p/b`, ` 4${T}/p/c`, `4.3${T}/p/d`, `-4${T}/p/e`, `7${T}/p/f`, `9${T}/p/f`, `3${T}/p/g\r`, `2${T}/p/h/`, "5 /p/i", `1e1${T}/p/j`, `8${T} /p/k`, `6${T}/p/l `, "   ", `4${T}/p/.DS_Store`, `12${T}/p/m${T}`,
    ].join("\n"),
  })));

  const F3 = "/p//";
  cases.push(await seed("a folder with several trailing slashes: git as given, du prefix trimmed, source as given", F3, ex({ [git(F3, ...LS)]: "a\0", [du("/p/a")]: `3${T}/p/a` })));
  cases.push(await seed("only a junk entry: du still runs, no rows", F, ex({ [git(F, ...LS)]: ".DS_Store\0", [du("/p/.DS_Store")]: `4${T}/p/.DS_Store` })));
  cases.push(await seed("no du answer: every size is 0", F, ex({ [git(F, ...LS)]: "a\0b/\0" })));
  cases.push(await seed("empty and doubled NULs in the listing are dropped", F, ex({ [git(F, ...LS)]: "\0\0a\0\0", [du("/p/a")]: `1${T}/p/a` })));

  const o = (over: Record<string, string>): Laptop => ex(over);
  cases.push(await seed("origin url with a blank first line is no remote", F, o({ [git(F, "remote", "get-url", "origin")]: "\nhttps://x\n" })));
  cases.push(await seed("origin url of only unicode blanks is no remote", F, o({ [git(F, "remote", "get-url", "origin")]: "  ﻿ \n" })));
  cases.push(await seed("origin url is the first line, trimmed", F, o({ [git(F, "remote", "get-url", "origin")]: "  git@github.com:o/r.git  \nsecond\n" })));
  cases.push(await seed("HEAD with a blank first line is an empty branch", F, o({ [git(F, "rev-parse", "--abbrev-ref", "HEAD")]: "\n\nmain" })));
  cases.push(await seed("HEAD with CRLF and padding", F, o({ [git(F, "rev-parse", "--abbrev-ref", "HEAD")]: "  feat/x \r\nmore" })));
  cases.push(await seed("upstream with a blank first line is no upstream", F, o({
    [git(F, "rev-parse", "--abbrev-ref", "@{upstream}")]: "\norigin/feat\n",
    [git(F, "symbolic-ref", "refs/remotes/origin/HEAD")]: "refs/remotes/origin/main\n", [git(F, "merge-base", "HEAD", "origin/main")]: "dd\n", [git(F, "rev-list", "--count", "dd..HEAD")]: "4\n",
  })));
  cases.push(await seed("merge base with a blank first line is no merge base", F, o({
    [git(F, "rev-parse", "--abbrev-ref", "@{upstream}")]: "origin/feat\n", [git(F, "merge-base", "HEAD", "origin/feat")]: "\nbb\n", [git(F, "rev-list", "--count", "bb..HEAD")]: "2",
  })));
  cases.push(await seed("merge base is the first line, trimmed", F, o({
    [git(F, "rev-parse", "--abbrev-ref", "@{upstream}")]: "origin/feat\n", [git(F, "merge-base", "HEAD", "origin/feat")]: "  bb  \nxx\n", [git(F, "rev-list", "--count", "bb..HEAD")]: "2",
  })));
  for (const [what, ref] of [["ends at origin/", "refs/remotes/origin/\n"], ["is under heads", "refs/heads/main\n"], ["names another remote", "refs/remotes/upstream/main\n"], ["repeats the prefix", "refs/remotes/origin/refs/remotes/origin/x\n"], ["is origin/main", "refs/remotes/origin/main\n"]] as const) {
    const branch = ref.trim().replace(/^refs\/remotes\/origin\//, "");
    cases.push(await seed(`the symbolic ref ${what}`, F, o({ [git(F, "symbolic-ref", "refs/remotes/origin/HEAD")]: ref, [git(F, "merge-base", "HEAD", `origin/${branch}`)]: "ee\n", [git(F, "rev-list", "--count", "ee..HEAD")]: "2\n" })));
  }
  cases.push(await seed("an upstream whose merge base is missing does not fall back to the default branch", F, o({
    [git(F, "rev-parse", "--abbrev-ref", "@{upstream}")]: "origin/feat\n",
    [git(F, "symbolic-ref", "refs/remotes/origin/HEAD")]: "refs/remotes/origin/main\n", [git(F, "merge-base", "HEAD", "origin/main")]: "dd\n", [git(F, "rev-list", "--count", "dd..HEAD")]: "4\n",
  })));
  cases.push(await seed("an upstream with a merge base and no count", F, o({ [git(F, "rev-parse", "--abbrev-ref", "@{upstream}")]: "origin/feat\n", [git(F, "merge-base", "HEAD", "origin/feat")]: "bb\n" })));
  for (const count of ["3.0", "0x3", "1e1", "+2", "-1", "Infinity", "", " 5 ", "2 3", "007", "1_0", "4\n9", "0", "00"]) {
    cases.push(await seed(`rev-list count ${JSON.stringify(count)}`, F, o({
      [git(F, "rev-parse", "--abbrev-ref", "@{upstream}")]: "origin/feat\n", [git(F, "merge-base", "HEAD", "origin/feat")]: "bb", [git(F, "rev-list", "--count", "bb..HEAD")]: count,
    })));
  }
  cases.push(await seed("status lines that are blank under a JS trim are not counted", F, o({ [git(F, "status", "--porcelain")]: "﻿\n M a\n \n\t\n?? b\r\n \n" })));

  const FM = "/Users/dev/app/";
  const mem = "/Users/dev/.claude/projects/-Users-dev-app-/memory";
  cases.push(await seed("memory: the folder's own size, files at any depth, a trailing slash on the folder and the state home", FM, { files: { [`${mem}/`]: 5000, [`${mem}/a.md`]: 100, [`${mem}/sub/b.md`]: 50 } }, "/Users/dev/.claude/"));
  cases.push(await seed("memory: no folder key, the size is the sum", FM, { files: { [`${mem}/a.md`]: 100, [`${mem}/b.md`]: 50 } }, "/Users/dev/.claude/"));
  cases.push(await seed("memory: an empty folder is still a row", FM, { files: { [`${mem}/`]: 0 } }, "/Users/dev/.claude/"));
  cases.push(await seed("memory: a file of that name is no row", FM, { files: { [mem]: 12 } }, "/Users/dev/.claude/"));
  cases.push(await seed("memory: none", FM, {}, "/Users/dev/.claude/"));
  const astral = "/p/😀 é/x";
  cases.push(await seed("memory: the key replaces each UTF-16 unit, an astral character is two dashes", astral, { files: { "/Users/dev/.claude/projects/-p------x/memory/a.md": 1 } }));

  const names = [".env", ".env.local", "env", "Env.txt", "a b", "a-b", "a_b", "a.b", "ab", "a/b", "10", "9", "é", "e", "f", "Z", "z", "ä", "B", "b", "A", "a", "résumé", "resume", "Résumé", "_x", "~y", "😀", "Ａ", "item2", "item10", "ß", "ss", ".claude", "docs/", "Docs/"];
  cases.push(await seed("rows sort by localeCompare", F, ex({ [git(F, ...LS)]: names.join("\0") })));
  cases.push(await seed("equal and near paths keep listing order", F, ex({ [git(F, ...LS)]: ["x", "X", "x"].join("\0") })));

  write("seed", { seed_menu: cases });
}, 60000);
