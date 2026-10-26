// SPDX-License-Identifier: AGPL-3.0-only
// One command line tape for every wsp line the parity and skill suites read: the skill, the tool server's
// instructions and the tools' descriptions, the README with its renames table's old lines, and the lines the parity
// suite plants to test the reader. Each is taped as the parity check reads it, placeholders and all: the host's refusal
// of a placeholder, a stale flag or a renamed verb is an answer the Rust host gives too.
import { readFileSync } from "node:fs";
import { it } from "vitest";
import { TAPE_SOURCE_ENV, tapeOf } from "@wsp/runtime";
import { instructions, wspSkill } from "../../packages/host/src/skill.js";
import { VERBS, hasTool } from "../../packages/host/src/verbs.js";
import { wspCommands } from "../../packages/host/test/wsp-lines.js";

const unquoted = (word: string): string => (/^(["']).*\1$/.test(word) ? word.slice(1, -1) : word);

function tape(from: string, lines: string[][]): void {
  for (const words of lines) {
    const argv = words.slice(1).map(unquoted);
    tapeOf({ ...process.env, [TAPE_SOURCE_ENV]: `${from} > wsp ${argv.join(" ")}` }, "cli")?.in({ argv });
  }
}

it("tapes every wsp line a reader is shown", () => {
  tape("skill", wspCommands([wspSkill(), instructions(), ...VERBS.filter(hasTool).map(v => v.tool.description)].join("\n\n")));
});

it("tapes every wsp line in the README, and the old lines its renames table keeps", () => {
  const readme = readFileSync(new URL("../../README.md", import.meta.url), "utf8");
  const renames = /<!-- renames:start -->\n([\s\S]*?)<!-- renames:end -->/.exec(readme)?.[1] ?? "";
  const was = [...renames.matchAll(/^\| (.+?) \| .+ \|$/gm)].map(m => m[1]!).filter(cell => !/^-+$/.test(cell) && cell !== "was");
  tape("readme", wspCommands(was.reduce((text, cell) => text.replace(`| ${cell} |`, "|"), readme)));
  tape("readme renames", wspCommands(was.join("\n")));
});

it("tapes the lines the parity suite plants for its reader", () => {
  // A copy of the planted block in parity.test.ts: a stale flag, an exec with a redirect and a misspelt verb.
  tape("parity planted", wspCommands("```\nwsp recipe --tick used --pick node=on\nwsp exec dev -- sh -c 'ls | wc -l' > /tmp/out 2>&1 &\n```\nThen `wsp run dev --cwd` and `wsp forkk dev`."));
});
