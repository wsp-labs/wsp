// SPDX-License-Identifier: AGPL-3.0-only
// node --test apps/desktop-gpui/bench/real-host.test.mjs: the fixture host's display name, without a host.
import { spawnSync } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { test } from "node:test";
import assert from "node:assert/strict";
import { LABEL_ENV, MAC_ONLY, relabel } from "./real-host.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const dist = join(here, "../../../packages/host/dist");

test("the built host still holds the line the label replaces", { skip: !existsSync(dist) && "packages/host is not built" }, () => {
  const holding = readdirSync(dist).filter(f => f.endsWith(".js") && readFileSync(join(dist, f), "utf8").includes('"ComputerName"'));
  assert.equal(holding.length, 1, `one file reads the Mac's ComputerName, found ${holding.join(", ")}`);
  assert.ok(readFileSync(join(dist, holding[0]), "utf8").includes(MAC_ONLY), `${holding[0]} no longer holds ${MAC_ONLY}, so the fixture's computer would keep its hostname`);
});

test("relabel answers the label where a Mac would read scutil", () => {
  const source = `let labelHere = null;\nfunction placeLabelHere() {\n  labelHere = void 0;\n  ${MAC_ONLY}\n  return "scutil";\n}`;
  const fn = new Function(`${relabel(source, "zingzy's MacBook Pro")}; return placeLabelHere();`);
  assert.equal(fn(), "zingzy's MacBook Pro");
  assert.equal(relabel("unrelated", "x"), "unrelated");
});

test("loaded with NODE_OPTIONS it relabels the host's module and leaves children nothing", () => {
  const dir = mkdtempSync(join(tmpdir(), "real-host-test-"));
  try {
    writeFileSync(join(dir, "package.json"), '{ "type": "module" }');
    writeFileSync(join(dir, "this-computer.js"), `import { platform } from "node:os";\nlet labelHere = null;\nexport function placeLabelHere() {\n  labelHere = void 0;\n  ${MAC_ONLY}\n  return "mac";\n}\n`);
    writeFileSync(join(dir, "main.js"), `import { placeLabelHere } from "./this-computer.js";\nconsole.log(JSON.stringify({ label: placeLabelHere() ?? null, options: process.env.NODE_OPTIONS ?? null, given: process.env.${LABEL_ENV} ?? null }));\n`);
    const run = env => JSON.parse(spawnSync(process.execPath, [join(dir, "main.js")], { encoding: "utf8", env: { PATH: "/usr/bin:/bin", ...env } }).stdout);
    const options = `--import=${pathToFileURL(join(here, "real-host.mjs")).href}`;
    assert.deepEqual(run({ NODE_OPTIONS: options, [LABEL_ENV]: "zingzy's MacBook Pro" }), { label: "zingzy's MacBook Pro", options: null, given: null });
    assert.deepEqual(run({ NODE_OPTIONS: options }), { label: null, options, given: null }, "no label asked, nothing changes");
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});
