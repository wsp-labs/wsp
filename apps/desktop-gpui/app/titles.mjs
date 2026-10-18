// SPDX-License-Identifier: AGPL-3.0-only
// Writes record/titles.json: what the protocol's titleLine and openingTitle make of each text below, which
// src/screens/app_shell/title.rs is held to row for row. A change to either rule in the protocol fails the Rust test
// until the record is written again, and then fails it again until title.rs follows. It reads the protocol package's
// build.
//
//   node titles.mjs           write the record again
//   node titles.mjs --check   exit 1 when the record is not what the protocol writes now
import { readFileSync, renameSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const RECORD = fileURLToPath(new URL("./record/titles.json", import.meta.url));
const { openingTitle, titleLine } = await import(new URL("../../../packages/protocol/dist/index.js", import.meta.url).href);

const a = n => "a".repeat(n);
const exact = "Rename the thread by its opening words and stop.";
const TEXTS = [
  "Bump the lockfile. Then run the gate.",
  "Is the gate green? Say so.",
  "Ship it! Then tag it.",
  "Really?! Yes.",
  "Wait... what? ok",
  "Bump to 1.2.3 and run the gate",
  "Use e.g. the cache for this one",
  'He said "stop." Then went home.',
  "Fix it (now.) Then test.",
  "Open https://example.com/a.b/c.html. Then fix it",
  "Look at ./src/main.rs and package.json, then fix",
  "Fix it.Then go",
  "?",
  "Fix the bug\nThen ship.",
  "  Fix the\tbuild  \nsecond line",
  "\r\n  \n\tReply  with\texactly   the word pong.  \r\n",
  "Add a test that the paging stops at the first event. (short-3, turn 1)",
  "You are a builder for the wsp repo, which is at /Users/dev/wsp on this Mac: read the ticket, then run `pnpm test` and report.\n\nTicket: wsp-labs/wsp-map#408.",
  "Look at why the transcript jumps when a long tool output lands and fix it.\n\nAttached files:\n/tmp/a.png",
  exact,
  `${exact.slice(0, -1)} now.`,
  a(47),
  a(48),
  a(49),
  `${a(47)} b`,
  `${a(48)} b`,
  "one, two, three, four, five, six, seven, eight, nine",
  "alpha; beta; gamma; delta; epsilon; zeta; eta; theta",
  `${a(40)},,,,,,,, ${"b".repeat(10)}`,
  `${"x".repeat(60)} y`,
  `${a(46)}\u{1F600}b`,
  "\u{1F600}".repeat(24),
  "\u{1F600}".repeat(25),
  "修".repeat(60),
  "e\u0301".repeat(30),
  "תקן את הבאג. אחר כך",
  "```ts\nconst a = 1;\n```\nFix this",
  "Rename `foo.bar()` to `baz`. Then test.",
  "# Fix the sidebar scroll\n\nbody",
  "- item one\n- item two",
  "/compact",
  "Fix\u00A0the\u3000build\u2028now. Then",
  "\uFEFFFix the build. Then ship",
  "\uFEFF\nSecond line",
  "Fix it.\uFEFFThen go",
  "fix the \u0085 build",
  "Fix it.\u0085Then go",
  "\u0085\nSecond line",
  "zero\u200Bwidth",
  "",
  "   \n\t  ",
];

// Invisible characters are written as escapes, so the record reads in a diff.
const json = s => JSON.stringify(s).replace(/[\p{C}\p{Z}\p{M}]/gu, c => (c === " " ? c : c.split("").map(u => `\\u${u.charCodeAt(0).toString(16).toUpperCase().padStart(4, "0")}`).join("")));
const text = `[\n${TEXTS.map(t => ` [${[t, titleLine(t), openingTitle(t)].map(json).join(",")}]`).join(",\n")}\n]\n`;

if (process.argv.includes("--check")) {
  let held = "";
  try {
    held = readFileSync(RECORD, "utf8");
  } catch {}
  if (held !== text) {
    console.error(`${RECORD} is behind packages/protocol; run node apps/desktop-gpui/app/titles.mjs and commit it`);
    process.exit(1);
  }
} else {
  writeFileSync(`${RECORD}.tmp`, text);
  renameSync(`${RECORD}.tmp`, RECORD);
}
