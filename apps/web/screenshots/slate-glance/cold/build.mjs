// SPDX-License-Identifier: AGPL-3.0-only
// Builds a cold writer's kit from the repo this file sits in: node build.mjs <out folder>. The folder gets `slate`,
// one bundled file with no repo behind it, and README.md.
import { execFileSync } from "node:child_process";
import { chmodSync, existsSync, mkdirSync, readFileSync, rmSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const repo = resolve(here, "../../../../..");
const out = resolve(process.argv[2]);
// esbuild as tsup has it: no package here depends on it by name.
const { build } = createRequire(createRequire(join(repo, "package.json")).resolve("tsup"))("esbuild");

/** @wsp/<package>[/<sub>] to its source, as the vite and vitest configs alias it. */
const workspaceSource = {
  name: "wsp-source",
  setup(b) {
    b.onResolve({ filter: /^@wsp\// }, args => {
      const [, pkg, ...sub] = args.path.split("/");
      const base = join(repo, "packages", pkg, "src");
      const tries = sub.length === 0 ? [join(base, "index.ts")] : [join(base, ...sub, "index.ts"), `${join(base, ...sub)}.ts`];
      const found = tries.find(existsSync);
      return found === undefined ? undefined : { path: found };
    });
    // The host reads its version off its package.json at load, which a kit has no copy of.
    b.onLoad({ filter: /packages\/host\/src\/version\.ts$/ }, args => ({
      contents: readFileSync(args.path, "utf8").replace(/JSON\.parse\(readFileSync\(new URL\("\.\.\/package\.json", import\.meta\.url\), "utf8"\)\)/, readFileSync(join(repo, "packages/host/package.json"), "utf8").trim()),
      loader: "ts",
    }));
  },
};

mkdirSync(out, { recursive: true });
const common = { bundle: true, platform: "node", target: "node22", format: "cjs", mainFields: ["module", "main"], plugins: [workspaceSource], logLevel: "warning", define: { __WSP_CLOUD__: "true", __WSP_POSTHOG_KEY__: '""', __WSP_POSTHOG_HOST__: '""', "import.meta.url": "__importMetaUrl" }, inject: [join(here, "meta-url.js")], loader: { ".md": "text" } };
await build({ ...common, entryPoints: [join(here, "kit.ts")], outfile: join(out, "slate"), minify: true, banner: { js: "#!/usr/bin/env node" } });
chmodSync(join(out, "slate"), 0o755);
const readme = join(out, ".readme.cjs");
await build({ ...common, entryPoints: [join(here, "readme.ts")], outfile: readme });
execFileSync("sh", ["-c", `node "${readme}" > "${join(out, "README.md")}"`]);
rmSync(readme);
