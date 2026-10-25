// SPDX-License-Identifier: AGPL-3.0-only
// Writes record/type-sample.json: the code line of Appearance's type sample as the web's code block colours it, shiki
// with the pierre theme of each side (lib/diffRendering.ts DIFF_THEME_NAMES), which
// src/screens/settings/pages/appearance/sample.rs draws token for token. It reads shiki and the themes off the web
// package's @pierre/diffs, and the sample off format.ts, so a new sample or a theme bump fails the Rust test until the
// record is written again.
//
//   node type-sample.mjs           write the record again
//   node type-sample.mjs --check   exit 1 when the record is not what shiki writes now
import { readFileSync, realpathSync, renameSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { fileURLToPath, pathToFileURL } from "node:url";

const RECORD = fileURLToPath(new URL("./record/type-sample.json", import.meta.url));
const diffs = createRequire(realpathSync(fileURLToPath(new URL("../../web/node_modules/@pierre/diffs/package.json", import.meta.url))));
const load = name => import(pathToFileURL(diffs.resolve(name)).href);
const { createHighlighter } = await load("shiki");
const light = (await load("@pierre/theme/pierre-light")).default;
const dark = (await load("@pierre/theme/pierre-dark")).default;

const format = readFileSync(fileURLToPath(new URL("../../web/src/settings/format.ts", import.meta.url)), "utf8");
const fence = /```(\w+)\\n(.*?)\\n```/.exec(format);
if (fence === null) throw new Error("format.ts holds no fenced sample");
const [, language, escaped] = fence;
const code = JSON.parse(`"${escaped}"`);

const highlighter = await createHighlighter({ themes: [light, dark], langs: [language] });
const side = theme => {
  const { tokens, fg } = highlighter.codeToTokens(code, { lang: language, theme });
  return { fg: fg.toLowerCase(), tokens: tokens.flat().map(t => [t.content, (t.color ?? fg).toLowerCase()]) };
};
// One token to a line.
const record = `${JSON.stringify({ language, code, light: side("pierre-light"), dark: side("pierre-dark") }, null, 1).replace(/\[\n\s+("(?:[^"\\]|\\.)*"),\n\s+("#[0-9a-f]+")\n\s+\]/g, "[$1, $2]")}\n`;

if (process.argv.includes("--check")) {
  let held = "";
  try {
    held = readFileSync(RECORD, "utf8");
  } catch {}
  if (held !== record) {
    console.error("record/type-sample.json is not what shiki writes now: run node type-sample.mjs");
    process.exit(1);
  }
} else {
  writeFileSync(`${RECORD}.tmp`, record);
  renameSync(`${RECORD}.tmp`, RECORD);
}
