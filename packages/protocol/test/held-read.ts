// SPDX-License-Identifier: AGPL-3.0-only
import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { heldPlaceScript, placeDaemonPaths } from "../src/index.js";

/** What the held read answers on a box holding `file` as its place file, or none where it is undefined: the real
 * script, run under bash in a home of its own, so a fake box answers as a box does. `rc` is what the login's
 * ~/.bashrc prints, which a bash built with SSH_SOURCE_BASHRC reads before the command wherever SSH_CLIENT is set,
 * as sshd sets it for every session. */
export function heldReadOf(file?: string, rc?: string): string {
  const home = mkdtempSync(join(tmpdir(), "wsp-held-"));
  try {
    if (rc !== undefined) writeFileSync(join(home, ".bashrc"), `printf '%s\\n' '${rc}'\n`);
    if (file !== undefined) {
      mkdirSync(placeDaemonPaths(home).wsp, { recursive: true });
      writeFileSync(placeDaemonPaths(home).placeFile, file);
    }
    return execFileSync("/bin/bash", ["-c", heldPlaceScript(home)], { encoding: "utf8", env: { HOME: home, PATH: "/usr/bin:/bin", SSH_CLIENT: "192.168.1.20 50000 22" } });
  } finally {
    rmSync(home, { recursive: true, force: true });
  }
}
