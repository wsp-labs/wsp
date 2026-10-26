// SPDX-License-Identifier: AGPL-3.0-only
// Writes record/frame-seal.js: the web app's seal of a slate frame's page (apps/web/src/slate/pieces/frame-seal.ts) as
// one plain script that sets window.wspSeal, which the GPUI app runs in its frame's web view, so both apps seal a page
// with the same code. A change to the seal fails the Rust test until the record is written again.
//
//   node frame-seal.mjs           write the record again
//   node frame-seal.mjs --check   exit 1 when the record is not what the seal bundles to now
import { readFileSync, renameSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";

const RECORD = fileURLToPath(new URL("./record/frame-seal.js", import.meta.url));
const SEAL = fileURLToPath(new URL("../../web/src/slate/pieces/frame-seal.ts", import.meta.url));
const WEB = fileURLToPath(new URL("../../web/package.json", import.meta.url));
const esbuild = createRequire(createRequire(WEB).resolve("vite/package.json"))("esbuild");

const POLICY = fileURLToPath(new URL("../../../packages/protocol/src/slate/frame-policy.ts", import.meta.url));
/** The seal names the policy through the protocol's slate entry; the bundle takes the policy's own module alone. */
const policyOnly = { name: "policy-only", setup: b => b.onResolve({ filter: /^@wsp\/protocol\/slate$/ }, () => ({ path: POLICY })) };
const ROOT = fileURLToPath(new URL("../../../", import.meta.url));
const built = await esbuild.build({ absWorkingDir: ROOT, entryPoints: [SEAL], plugins: [policyOnly], bundle: true, format: "iife", globalName: "wspSeal", platform: "browser", target: "es2020", write: false, legalComments: "none", logLevel: "silent" });
const text = `// SPDX-License-Identifier: AGPL-3.0-only\n// Written by frame-seal.mjs from apps/web/src/slate/pieces/frame-seal.ts; never edited by hand.\n${built.outputFiles[0].text}`;

if (process.argv.includes("--check")) {
  let held = "";
  try {
    held = readFileSync(RECORD, "utf8");
  } catch {}
  if (held !== text) {
    console.error(`${RECORD} is behind apps/web's seal; run node apps/desktop-gpui/app/frame-seal.mjs and commit it`);
    process.exit(1);
  }
} else {
  writeFileSync(`${RECORD}.tmp`, text);
  renameSync(`${RECORD}.tmp`, RECORD);
}
