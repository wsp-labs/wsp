// tsexpand.mjs <rust files...>: replaces each `// @ts <ts path under protocol/src> <name>` line with that export's TS
// source as `// ` comment lines, headed by its file:line at the commit. Idempotent: an expanded block is kept.
import fs from "node:fs";
const ROOT = `${process.env.WSP_REPO}/packages/protocol/src`;
const calls = { ...JSON.parse(fs.readFileSync(`${process.env.WSP_WORK}/calls-views.json`, "utf8")), ...JSON.parse(fs.readFileSync(`${process.env.WSP_WORK}/calls-top.json`, "utf8")) };
for (const file of process.argv.slice(2)) {
  const lines = fs.readFileSync(file, "utf8").split("\n");
  const out = [];
  for (const l of lines) {
    const m = l.match(/^(\s*)\/\/ @ts (\S+) (\S+)\s*$/);
    if (!m) { out.push(l); continue; }
    const [, indent, ts, name] = m;
    const d = calls[ts]?.[name];
    if (!d) throw new Error(`${file}: no ${name} in ${ts}`);
    const src = fs.readFileSync(`${ROOT}/${ts}`, "utf8").split("\n").slice(d.line - 1, d.endLine);
    out.push(`${indent}// ${ts}:${d.line} at a3b870a6e`);
    for (const s of src) out.push(`${indent}//${s === "" ? "" : " "}${s}`.replace(/\s+$/, ""));
  }
  fs.writeFileSync(file, out.join("\n"));
}
