// SPDX-License-Identifier: AGPL-3.0-only
// Bundles extract.ts, and glance-of.ts where this revision has the glance warnings, into <out folder>.
import { existsSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const repo = resolve(here, "../../../../..");
const out = resolve(process.argv[2]);
const { build } = createRequire(createRequire(join(repo, "package.json")).resolve("tsup"))("esbuild");
const entries = ["extract", ...(existsSync(join(repo, "packages/protocol/src/slate/glance.ts")) ? ["glance-of"] : [])];
for (const e of entries) await build({ entryPoints: [join(here, `${e}.ts`)], outfile: join(out, `${e}.cjs`), bundle: true, platform: "node", target: "node22", format: "cjs", logLevel: "warning" });
