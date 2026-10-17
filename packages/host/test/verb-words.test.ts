// SPDX-License-Identifier: AGPL-3.0-only
// Every sentence wsp prints that names `wsp <word>` names a word the command
// line answers to, or a word read as prose about wsp itself ("wsp starts",
// "this wsp"); after a command that has subcommands, the next word is one of
// them or prose too. A word in neither list fails here, so a removed or invented
// verb (a builder's line once told the person to run wsp sweep) never reaches them.
import { readdirSync, readFileSync, statSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { COMMAND_LINES } from "../src/cli.js";
import { ALL_VERBS } from "../src/verbs.js";

const REPO = fileURLToPath(new URL("../../../", import.meta.url));
const ROOTS = [
  "packages/host/src", "packages/protocol/src", "packages/runtime/src", "packages/engine/src", "packages/catalog/src", "packages/collect/src", "packages/keys/src",
  "packages/own-file/src", "packages/adapter-claude/src", "packages/adapter-codex/src", "packages/adapter-cursor/src", "packages/adapter-opencode/src", "apps/web/src",
  "apps/desktop/src",
];

/** Words after "wsp", or after a command, that read as prose about wsp the program, a wsp setup or a part of it. */
const PROSE: readonly string[] = [
  "adds", "again", "already", "and", "answers", "app", "as", "asked", "asks", "at", "blue", "browses", "builds", "bundle", "but", "can", "cannot",
  "carries", "cleared", "clones", "comes", "command", "could", "counted", "cut", "daemon", "deleted", "deploys", "device", "did", "docs", "does",
  "draws", "ends", "fetches", "files", "for", "forgot", "forked", "forks", "forwards", "found", "from", "guard", "hands", "has", "here", "holds",
  "home", "in", "installed", "installs", "instead", "is", "it", "joins", "just", "keeps", "kept", "knows", "landed", "leaves", "left", "lists",
  "logs", "machine", "made", "makes", "merged", "moves", "needs", "never", "no", "of", "off", "older", "on", "only", "opens", "or", "place",
  "process", "put", "ran", "read", "reads", "refused", "restarts", "running", "runs", "saves", "says", "section", "sends", "server", "serves",
  "serving", "set", "shows", "skill", "skipped", "started", "starts", "stood", "stopped", "stops", "takes", "than", "that", "the", "there", "to",
  "too", "tools", "tries", "turns", "under", "until", "updates", "uses", "verb", "waits", "was", "will", "with", "within", "workspaces", "would",
  "writes", "wrote", "yet",
];

function sources(dir: string): string[] {
  return readdirSync(dir).flatMap(name => {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) return name === "node_modules" || name === "dist" ? [] : sources(path);
    return /\.tsx?$/.test(name) && !/\.(test|d)\.tsx?$/.test(name) ? [path] : [];
  });
}

/** Each `wsp <word> [<word>]` on a line of code, with where it stands; a comment is no sentence anyone reads. */
function named(): { word: string; next: string | undefined; at: string }[] {
  return ROOTS.flatMap(root => sources(join(REPO, root))).flatMap(path =>
    readFileSync(path, "utf8")
      .split("\n")
      .flatMap((line, i) => {
        const text = line.trim();
        if (text.startsWith("//") || text.startsWith("*") || text.startsWith("/*")) return [];
        const code = line.replace(/\s\/\/ .*$/, "");
        return [...code.matchAll(/\bwsp ([a-z][a-z-]*)(?: ([a-z][a-z-]*))?/g)].map(m => ({ word: m[1]!, next: m[2], at: `${path.slice(REPO.length)}:${i + 1}` }));
      }),
  );
}

describe("the verbs wsp's sentences name", () => {
  it("every wsp <word> a sentence names is a word the command line answers to or one read as prose about wsp, and so is the word after a command with subcommands", () => {
    const lines = [...COMMAND_LINES.map(c => c.words), ...ALL_VERBS.map(v => v.name)].map(words => words.split(" "));
    const commands = new Set(lines.map(words => words[0]!).concat("help"));
    const subs = new Map<string, Set<string>>();
    for (const [word, sub] of lines) if (sub !== undefined) subs.set(word!, (subs.get(word!) ?? new Set()).add(sub));
    const seen = named();
    expect(seen.length).toBeGreaterThan(500);
    const unknown = seen.filter(n => !commands.has(n.word) && !PROSE.includes(n.word));
    expect(unknown.map(n => `wsp ${n.word} at ${n.at}`)).toEqual([]);
    const unknownSub = seen.filter(n => n.next !== undefined && subs.has(n.word) && !subs.get(n.word)!.has(n.next) && !PROSE.includes(n.next));
    expect(unknownSub.map(n => `wsp ${n.word} ${n.next} at ${n.at}`)).toEqual([]);
    // A word on both lists would let a verb's name pass as prose once the verb is gone.
    expect(PROSE.filter(word => commands.has(word))).toEqual([]);
  });
});
