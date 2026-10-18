#!/usr/bin/env node
// SPDX-License-Identifier: AGPL-3.0-only
// Writes the `highlight` block of every theme file in app/src/ui/themes from the pierre theme today's app colors code
// with (apps/web/src/lib/diffRendering.ts: "pierre-light" on the light side, "pierre-dark" on the dark), as installed
// under apps/web/node_modules through @pierre/diffs:
//   node scripts/pierre-themes.mjs           rewrite the blocks
//   node scripts/pierre-themes.mjs --check   exit 1 and name each file whose block is not what this writes
// Each tree-sitter name gpui-kit colors is given the color pierre's TextMate rules give one scope that names the same
// thing, resolved as TextMate resolves a scope: the longest matching selector wins, a later rule breaks a tie. The
// block also carries pierre's plain text color and its added, deleted and modified colors, which the diff's bars and
// file chevrons draw in. Only the block is touched; the rest of each file keeps its bytes.
import { existsSync, readFileSync, readdirSync, realpathSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const THEMES = join(here, "../app/src/ui/themes");
const WEB = join(here, "../../web");

/** gpui-kit's highlight names (highlighter/registry.rs, HIGHLIGHT_NAMES) and the TextMate scope each stands for. */
export const SCOPES = {
  attribute: "entity.other.attribute-name",
  boolean: "constant.language.boolean",
  comment: "comment",
  "comment.doc": "comment.block.documentation",
  constant: "variable.other.constant",
  "constant.builtin": "constant.language",
  constructor: "entity.name.type.class",
  embedded: "meta.embedded",
  emphasis: "markup.italic",
  "emphasis.strong": "markup.bold",
  enum: "entity.name.type.enum",
  function: "entity.name.function",
  keyword: "keyword",
  "keyword.storage": "storage.type",
  label: "entity.name.label",
  link_text: "string.other.link.title.markdown",
  link_uri: "markup.underline.link",
  namespace: "entity.name.namespace",
  number: "constant.numeric",
  operator: "keyword.operator.arithmetic",
  preproc: "meta.preprocessor",
  property: "variable.other.property",
  punctuation: "punctuation",
  "punctuation.bracket": "meta.brace",
  "punctuation.delimiter": "punctuation.separator",
  "punctuation.list_marker": "punctuation.definition.list.begin.markdown",
  "punctuation.special": "punctuation.definition.template-expression.begin",
  string: "string",
  "string.escape": "constant.character.escape",
  "string.regex": "string.regexp",
  "string.special": "constant.other.symbol",
  "string.special.symbol": "constant.other.symbol",
  tag: "entity.name.tag",
  "tag.doctype": "meta.tag.metadata.doctype",
  "text.code.span": "markup.inline.raw",
  "text.literal": "markup.raw",
  title: "markup.heading",
  type: "entity.name.type",
  variable: "variable",
  "variable.parameter": "variable.parameter",
  "variable.special": "variable.language",
  variant: "variable.other.enummember",
};

/** A pierre color as the theme files write colors: `#rrggbbaa`, lower case. */
export function hex(color) {
  const c = color.toLowerCase();
  if (/^#[0-9a-f]{6}$/.test(c)) return `${c}ff`;
  if (/^#[0-9a-f]{8}$/.test(c)) return c;
  if (/^#[0-9a-f]{3}$/.test(c)) return `#${[...c.slice(1)].map(d => d + d).join("")}ff`;
  throw new Error(`not a hex color: ${color}`);
}

/** The settings TextMate gives `scope` under `theme`'s rules, or null. Descendant selectors (with a space) are skipped:
 * a tree-sitter name has no parent scopes to match them against. */
export function resolve(theme, scope) {
  let best = null;
  for (const rule of theme.tokenColors) {
    const selectors = Array.isArray(rule.scope) ? rule.scope : String(rule.scope ?? "").split(",");
    for (const raw of selectors) {
      const sel = raw.trim();
      if (!sel || sel.includes(" ")) continue;
      if ((scope === sel || scope.startsWith(`${sel}.`)) && (!best || sel.length >= best.sel.length)) best = { sel, settings: rule.settings };
    }
  }
  return best?.settings ?? null;
}

/** The `highlight` block for one pierre theme. */
export function block(theme) {
  const syntax = {};
  for (const [name, scope] of Object.entries(SCOPES)) {
    const settings = resolve(theme, scope);
    if (!settings?.foreground) continue;
    const style = { color: hex(settings.foreground) };
    const font = String(settings.fontStyle ?? "");
    if (font.includes("italic")) style.font_style = "italic";
    if (font.includes("bold")) style.font_weight = 700;
    syntax[name] = style;
  }
  const c = theme.colors;
  return {
    "editor.foreground": hex(c["editor.foreground"]),
    success: hex(c["gitDecoration.addedResourceForeground"]),
    error: hex(c["gitDecoration.deletedResourceForeground"]),
    info: hex(c["gitDecoration.modifiedResourceForeground"]),
    syntax,
  };
}

/** `text` with its theme's `highlight` block replaced by `highlight`, every other byte kept. The block is written as
 * the theme object's last member. */
export function splice(text, highlight) {
  const tail = /\n {4}\}\n {2}\]\n\}\n?$/;
  const end = text.search(tail);
  if (end < 0) throw new Error("a theme file ends with its one theme object, then `]` and `}`");
  let head = text.slice(0, end);
  const at = head.lastIndexOf(',\n      "highlight": ');
  if (at >= 0) head = head.slice(0, at);
  const body = JSON.stringify(highlight, null, 2).replace(/\n/g, "\n      ");
  return `${head},\n      "highlight": ${body}${text.slice(end)}`;
}

/** The package folder `name` resolves to from `from`, as Node looks it up: the nearest `node_modules` above. */
function packageDir(from, name) {
  for (let dir = from; ; dir = dirname(dir)) {
    const candidate = join(dir, "node_modules", name);
    if (existsSync(join(candidate, "package.json"))) return realpathSync(candidate);
    if (dirname(dir) === dir) throw new Error(`${name} is not installed above ${from}; run pnpm install`);
  }
}

async function pierre(name) {
  const diffs = packageDir(WEB, "@pierre/diffs");
  const dist = join(packageDir(diffs, "@pierre/theme"), "dist", `${name}.mjs`);
  return (await import(pathToFileURL(dist).href)).default;
}

async function main() {
  const check = process.argv.includes("--check");
  const blocks = { light: block(await pierre("pierre-light")), dark: block(await pierre("pierre-dark")) };
  let stale = 0;
  for (const file of readdirSync(THEMES).filter(f => f.endsWith(".json")).sort()) {
    const path = join(THEMES, file);
    const text = readFileSync(path, "utf8");
    const mode = JSON.parse(text).themes[0].mode;
    const next = splice(text, blocks[mode === "dark" ? "dark" : "light"]);
    const before = JSON.parse(text).themes[0];
    const after = JSON.parse(next).themes[0];
    for (const key of Object.keys(before)) {
      if (key !== "highlight" && JSON.stringify(before[key]) !== JSON.stringify(after[key])) throw new Error(`${file}: ${key} moved`);
    }
    if (next === text) continue;
    stale += 1;
    if (check) console.error(`pierre-themes: ${file} is not what this script writes`);
    else writeFileSync(path, next);
  }
  if (check && stale > 0) process.exit(1);
  console.log(`pierre-themes: ${check ? "checked" : "wrote"} ${readdirSync(THEMES).filter(f => f.endsWith(".json")).length} theme files`);
}

if (import.meta.url === pathToFileURL(process.argv[1]).href) await main();
