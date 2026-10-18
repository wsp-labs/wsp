#!/usr/bin/env node
// SPDX-License-Identifier: AGPL-3.0-only
// Prints the color today's app (shiki and @pierre/theme as installed under apps/web, the oniguruma engine, set up as
// @pierre/diffs sets it up) gives each token of each sample file, for app/tests/pr_highlight.rs to compare against:
//   node scripts/shiki-tokens.mjs app/tests/pr_highlight > app/tests/pr_highlight/pierre.tsv
// Columns: file, side (light|dark), line:col (1-based line, 0-based byte column), color (#rrggbb or plain), text.
import { readFileSync, readdirSync, realpathSync, existsSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const WEB = join(dirname(fileURLToPath(import.meta.url)), "../../web");

function packageDir(from, name) {
  for (let dir = from; ; dir = dirname(dir)) {
    const candidate = join(dir, "node_modules", name);
    if (existsSync(join(candidate, "package.json"))) return realpathSync(candidate);
    if (dirname(dir) === dir) throw new Error(`${name} not installed above ${from}`);
  }
}
const load = (dir, rel) => import(pathToFileURL(join(dir, rel)).href);

const diffs = packageDir(WEB, "@pierre/diffs");
const shikiDir = packageDir(diffs, "shiki");
const themeDir = packageDir(packageDir(diffs, "@pierre/theming"), "@pierre/theme");
const shikiPkg = JSON.parse(readFileSync(join(shikiDir, "package.json"), "utf8"));
const themePkg = JSON.parse(readFileSync(join(themeDir, "package.json"), "utf8"));
console.error(`shiki ${shikiPkg.version}, @pierre/theme ${themePkg.version}`);

const shiki = await load(shikiDir, "dist/index.mjs");
const { getFiletypeFromFileName } = await load(diffs, "dist/utils/getFiletypeFromFileName.js");
const themes = {
  light: (await load(themeDir, "dist/pierre-light.mjs")).default,
  dark: (await load(themeDir, "dist/pierre-dark.mjs")).default,
};

const dir = process.argv[2];
const files = readdirSync(dir).filter(f => f.startsWith("sample.")).sort();
const langs = [...new Set(files.map(f => getFiletypeFromFileName(f)))];
const highlighter = await shiki.createHighlighter({
  themes: Object.values(themes),
  langs,
  engine: shiki.createOnigurumaEngine(import(pathToFileURL(join(shikiDir, "dist/wasm.mjs")).href)),
});

const out = [];
for (const file of files) {
  const code = readFileSync(join(dir, file), "utf8");
  if (!/^[\x00-\x7f]*$/.test(code)) throw new Error(`${file}: ASCII only, so columns are bytes`);
  const lang = getFiletypeFromFileName(file);
  for (const [side, theme] of Object.entries(themes)) {
    const fg = highlighter.getTheme(theme.name).fg.toLowerCase();
    const lines = highlighter.codeToTokensBase(code, { lang, theme: theme.name });
    lines.forEach((tokens, i) => {
      let col = 0;
      for (const token of tokens) {
        const text = token.content;
        const lead = text.length - text.trimStart().length;
        const body = text.trim();
        if (body) {
          const color = (token.color ?? fg).toLowerCase().slice(0, 7);
          out.push([file, side, `${i + 1}:${col + lead}`, color === fg.slice(0, 7) ? "plain" : color, body].join("\t"));
        }
        col += text.length;
      }
    });
  }
}
console.log(out.join("\n"));
