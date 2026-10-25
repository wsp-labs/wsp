// SPDX-License-Identifier: AGPL-3.0-only
// A cold writer's README: the slate paragraph of the wsp server's instructions and the two slate tools' descriptions,
// as this revision has them.
import { THREAD_SLATE_WORDS } from "../../../../../packages/host/src/skill.js";
import { SLATE_VERBS } from "../../../../../packages/host/src/verbs/slate.js";

const description = (name: string): string => SLATE_VERBS.find(v => v.name === name)!.tool!.description;
process.stdout.write(
  [
    "# The slate",
    "",
    "`./slate catalog` stands for slate_catalog and `./slate write` for slate_write.",
    "",
    "## From the wsp server's instructions",
    "",
    THREAD_SLATE_WORDS,
    "",
    "## slate_catalog",
    "",
    description("slate catalog"),
    "",
    "## slate_write",
    "",
    description("slate write"),
    "",
  ].join("\n"),
);
