#!/usr/bin/env node
// SPDX-License-Identifier: AGPL-3.0-only
// Writes app/src/screens/pr_pane/files/pierre_icons.rs from the glyphs today's Files tree draws: @pierre/trees' "complete"
// set (components/files/FileBrowserPanel.tsx: `icons: { set: "complete", colored: false }`), as installed under
// apps/web/node_modules:
//   node scripts/pierre-file-icons.mjs           rewrite the file
//   node scripts/pierre-file-icons.mjs --check   exit 1 if the file is not what this writes
// The file holds pierre's file-name and extension tables (the complete set's extension overrides laid over the
// extensions) and each glyph as a whole SVG; rows.rs picks a file's glyph from them as pierre's resolver does.
import { existsSync, readFileSync, realpathSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const OUT = join(here, "../app/src/screens/pr_pane/files/pierre_icons.rs");
const WEB = join(here, "../../web");

function packageDir(from, name) {
  for (let dir = from; ; dir = dirname(dir)) {
    const candidate = join(dir, "node_modules", name);
    if (existsSync(join(candidate, "package.json"))) return realpathSync(candidate);
    if (dirname(dir) === dir) throw new Error(`${name} is not installed above ${from}; run pnpm install`);
  }
}

/** The object literal `const <name> = { ... };` in `source`, read as data. */
function table(source, name) {
  const at = source.indexOf(`const ${name} = {`);
  if (at < 0) throw new Error(`no ${name} in builtInIcons.js`);
  const end = source.indexOf("\n};", at);
  return Function(`"use strict"; return (${source.slice(at + `const ${name} = `.length, end + 2)});`)();
}

const rust = s => JSON.stringify(s);

async function main() {
  const trees = packageDir(WEB, "@pierre/trees");
  const dist = join(trees, "dist", "builtInIcons.js");
  const source = readFileSync(dist, "utf8");
  const { getBuiltInSpriteSheet } = await import(pathToFileURL(dist).href);
  const names = table(source, "BUILT_IN_FILE_NAME_TOKENS");
  const extensions = { ...table(source, "BUILT_IN_FILE_EXTENSION_TOKENS"), ...table(source, "COMPLETE_EXTENSION_OVERRIDES") };
  const glyphs = {};
  for (const [, id, viewBox, body] of getBuiltInSpriteSheet("complete").matchAll(/<symbol id="([^"]+)" viewBox="([^"]+)">([\s\S]*?)<\/symbol>/g)) {
    const svg = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="${viewBox}">${body.replace(/\s*\n\s*/g, "")}</svg>`;
    if (id === "file-tree-icon-chevron") glyphs.chevron = svg;
    const token = id.match(/^file-tree-builtin-(.+)$/)?.[1];
    if (token) glyphs[token] = svg;
  }
  for (const token of new Set([...Object.values(names), ...Object.values(extensions), "default"])) {
    if (!glyphs[token]) throw new Error(`no glyph for ${token}`);
  }
  const version = JSON.parse(readFileSync(join(trees, "package.json"), "utf8")).version;
  const pairs = obj => Object.entries(obj).map(([k, v]) => `    (${rust(k)}, ${rust(v)}),`).join("\n");
  const text = `//! \`@pierre/trees\` ${version}'s "complete" file glyphs, as today's Files tree draws them. Written by
//! \`scripts/pierre-file-icons.mjs\`; run it again rather than editing this file.

/// \`BUILT_IN_FILE_NAME_TOKENS\`: a file's whole name, without case, to its glyph.
pub const NAMES: &[(&str, &str)] = &[
${pairs(names)}
];

/// \`BUILT_IN_FILE_EXTENSION_TOKENS\` with \`COMPLETE_EXTENSION_OVERRIDES\` over it.
pub const EXTENSIONS: &[(&str, &str)] = &[
${pairs(extensions)}
];

/// Each glyph as a whole SVG, the folder chevron (pointing down) as \`chevron\`.
pub const GLYPHS: &[(&str, &str)] = &[
${pairs(glyphs)}
];
`;
  const check = process.argv.includes("--check");
  const same = existsSync(OUT) && readFileSync(OUT, "utf8") === text;
  if (check) {
    if (!same) {
      console.error("pierre-file-icons: pierre_icons.rs is not what this script writes");
      process.exit(1);
    }
  } else if (!same) writeFileSync(OUT, text);
  console.log(`pierre-file-icons: ${check ? "checked" : "wrote"} ${Object.keys(glyphs).length} glyphs`);
}

if (import.meta.url === pathToFileURL(process.argv[1]).href) await main();
