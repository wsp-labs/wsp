// SPDX-License-Identifier: AGPL-3.0-only
// node scripts/bundle-size.mjs <app>: what the app takes on disk, counted as du
// counts it, against the classic app it replaces as the main download. Prints
// the size and exits 1 when the app is not smaller, so a release never makes
// the download people get bigger than the one they had.
import { lstatSync, readdirSync } from "node:fs";
import { join } from "node:path";
import { pathToFileURL } from "node:url";

/** The classic app 0.3.8 installed on a Mac, du -sm of its wsp.app: the size the GPUI app has to come in under. */
export const CLASSIC_APP_MB = 515;

/** The blocks a file or folder holds on disk, every file under it counted once and no link followed, in MB. */
export function sizeOnDiskMb(path) {
  let bytes = 0;
  const walk = at => {
    const stat = lstatSync(at);
    bytes += stat.blocks * 512;
    if (stat.isDirectory()) for (const name of readdirSync(at)) walk(join(at, name));
  };
  walk(path);
  return bytes / (1024 * 1024);
}

/** The line the release prints for an app of `mb`, and whether it passes. */
export function sizeVerdict(mb, limit = CLASSIC_APP_MB) {
  const said = `${mb.toFixed(1)} MB on disk, against the classic app's ${limit} MB`;
  return mb < limit ? { ok: true, line: `ok    ${said}` } : { ok: false, line: `refused: ${said}; the main download may not grow` };
}

if (process.argv[1] !== undefined && import.meta.url === pathToFileURL(process.argv[1]).href) {
  try {
    const app = process.argv[2];
    if (app === undefined) throw new Error("usage: node bundle-size.mjs <app>");
    const verdict = sizeVerdict(sizeOnDiskMb(app));
    console.log(verdict.line);
    if (!verdict.ok) process.exitCode = 1;
  } catch (e) {
    console.error(e instanceof Error ? e.message : String(e));
    process.exitCode = 1;
  }
}
