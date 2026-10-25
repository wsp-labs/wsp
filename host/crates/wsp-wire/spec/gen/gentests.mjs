// gentests.mjs <fixtures dir> <crate dir>: one Rust test per exported zod schema, reading the fixtures zod answered.
import fs from "node:fs";
import path from "node:path";

const [FX, CRATE] = process.argv.slice(2);
const snake = s => s.replace(/([a-z0-9])([A-Z])/g, "$1_$2").replace(/([A-Z]+)([A-Z][a-z])/g, "$1_$2").toLowerCase();
const testsDir = path.join(CRATE, "tests");
fs.mkdirSync(path.join(testsDir, "shapes"), { recursive: true });
fs.mkdirSync(path.join(testsDir, "fixtures", "shapes"), { recursive: true });
const mods = [];
let total = 0;
for (const file of fs.readdirSync(FX).filter(f => f.endsWith(".json")).sort()) {
  const entries = JSON.parse(fs.readFileSync(path.join(FX, file), "utf8"));
  fs.copyFileSync(path.join(FX, file), path.join(testsDir, "fixtures", "shapes", file));
  const stem = file.replace(/\.json$/, "");
  const [kind, ...rest] = stem.split("_");
  const rustMod = `wsp_wire::${kind}::${rest.join("_")}`;
  const types = [...new Set(entries.map(e => e.type))];
  const tests = types.map(t => {
    const tag = entries.find(e => e.type === t && e.tag)?.tag;
    const call = tag
      ? `cases_tagged::<${t}>(FIXTURES, "${t}", "${tag.key}", ${t}::${snake(tag.key).toUpperCase()});`
      : `cases::<${t}>(FIXTURES, "${t}");`;
    const n = entries.filter(e => e.type === t).length;
    return `/// ${n} case${n === 1 ? "" : "s"}: what zod's parse of ${t} wrote, or the first issue it raised.\n#[test]\nfn ${snake(t)}_reads_and_writes_as_zod_does() {\n    ${call}\n}`;
  });
  total += types.length;
  const body = `// SPDX-License-Identifier: AGPL-3.0-only
//! The shapes of ${kind === "views" ? `views/${rest.join("-")}` : rest.join("-")}.ts against what zod answered for the same inputs. Generated.

use super::common::{cases, cases_tagged};
use ${rustMod}::*;

const FIXTURES: &str = include_str!("../fixtures/shapes/${file}");

${tests.join("\n\n")}
`;
  fs.writeFileSync(path.join(testsDir, "shapes", `${stem}.rs`), body);
  mods.push(stem);
}
fs.writeFileSync(path.join(testsDir, "shapes.rs"), `// SPDX-License-Identifier: AGPL-3.0-only
//! Every exported zod shape of the protocol's views/ and top-level files, read and written as zod does: the exact JSON
//! text zod's parse wrote for each sample, and the code (and, for a check's rule, the path) of the first issue it raised
//! for each sample that breaks a rule. The fixtures come from running zod on the TS schemas at a3b870a6e.

#[path = "shapes/common.rs"]
mod common;

${mods.map(m => `#[path = "shapes/${m}.rs"]\nmod ${m};`).join("\n")}
`);
console.log(`${mods.length} modules, ${total} tests`);
