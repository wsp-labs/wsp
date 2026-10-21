// SPDX-License-Identifier: AGPL-3.0-only
// node scripts/test-times.mjs <ci run id>...
//
// Writes scripts/test-times.json, the milliseconds each test file's tests took in the "Tests" jobs of the ci runs
// named, read off the line vitest's default reporter prints per file, keyed by vitest project and path from the root
// as vitest.shard.ts reads them. A later run's time for a file replaces an earlier one's, so name a nightly first and
// the land runs after it; a file no longer in the tree is dropped.
import { execFileSync } from "node:child_process";
import { existsSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const runs = process.argv.slice(2);
if (runs.length === 0 || runs.some(run => !/^\d+$/.test(run))) {
  process.stderr.write("usage: node scripts/test-times.mjs <ci run id>...\n");
  process.exit(3);
}

const root = execFileSync("git", ["rev-parse", "--show-toplevel"], { encoding: "utf8" }).trim();
const gh = (...words) => execFileSync("gh", ["api", "--allow-escape-sequences", ...words], { encoding: "utf8", maxBuffer: 256 << 20 });
// " ✓ |node| packages/host/test/parity.test.ts (30 tests) 1234ms", and a skipped file with no time.
const LINE = /^\S+ +[✓❯×↓] \|([^|]+)\| (\S+) \([^)]*\)(?: (\d+)ms)?$/;

const times = {};
for (const run of runs) {
  const jobs = JSON.parse(gh(`repos/{owner}/{repo}/actions/runs/${run}/jobs?per_page=100`)).jobs.filter(job => /^Tests \d+\/\d+$/.test(job.name));
  for (const job of jobs) {
    for (const raw of gh(`repos/{owner}/{repo}/actions/jobs/${job.id}/logs`).split("\n")) {
      const hit = LINE.exec(raw.replace(/\x1b\[[0-9;]*m/g, "").trimEnd());
      if (!hit) continue;
      const [, project, path, ms] = hit;
      // An app's own project names its files from the app's folder.
      const file = project.startsWith("@wsp/") ? `apps/${project.slice(5)}/${path}` : path;
      times[`${project}:${file}`] = Number(ms ?? 0);
    }
  }
  process.stderr.write(`run ${run}: ${jobs.length} test jobs\n`);
}

const kept = Object.keys(times).filter(key => existsSync(join(root, key.slice(key.indexOf(":") + 1)))).sort();
writeFileSync(join(root, "scripts/test-times.json"), `{\n${kept.map(key => `  ${JSON.stringify(key)}: ${times[key]}`).join(",\n")}\n}\n`);
process.stdout.write(`scripts/test-times.json: ${kept.length} files, ${Object.keys(times).length - kept.length} gone from the tree\n`);
