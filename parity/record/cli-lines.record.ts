// SPDX-License-Identifier: AGPL-3.0-only
// One command line tape for every wsp line the skill, the tool server's instructions and the tools' descriptions
// show a reader, read as the parity check reads them, placeholders and all: the host's refusal of a placeholder is
// an answer the Rust host gives too.
import { it } from "vitest";
import { TAPE_SOURCE_ENV, tapeOf } from "@wsp/runtime";
import { instructions, wspSkill } from "../../packages/host/src/skill.js";
import { VERBS, hasTool } from "../../packages/host/src/verbs.js";
import { wspCommands } from "../../packages/host/test/wsp-lines.js";

const unquoted = (word: string): string => (/^(["']).*\1$/.test(word) ? word.slice(1, -1) : word);

it("tapes every wsp line a reader is shown", () => {
  const lines = wspCommands([wspSkill(), instructions(), ...VERBS.filter(hasTool).map(v => v.tool.description)].join("\n\n"));
  for (const words of lines) {
    const argv = words.slice(1).map(unquoted);
    tapeOf({ ...process.env, [TAPE_SOURCE_ENV]: `skill > wsp ${argv.join(" ")}` }, "cli")?.in({ argv });
  }
});
