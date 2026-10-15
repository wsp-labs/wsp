// SPDX-License-Identifier: AGPL-3.0-only
// The look a wake takes at a box's daemon unit, run through a real bash with
// systemctl, journalctl and sleep stubbed on the PATH: the stubs answer what
// systemd 255 answered on a box for a daemon that keeps exiting (2026-10-16).
import { spawnSync } from "node:child_process";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { DAEMON_UNIT } from "@wsp/protocol";
import { afterEach, describe, expect, it } from "vitest";
import { DAEMON_EXITS_CMD, daemonExitsOf } from "../src/box-backend.js";
import { writeStub } from "../../protocol/test/stub-script.js";

const dirs: string[] = [];
afterEach(() => {
  for (const dir of dirs.splice(0)) rmSync(dir, { recursive: true, force: true });
});

/** What the look prints on a unit whose two restart counts, five seconds apart, are `counts`, and what it asked the
 * journal. */
function look(o: { counts: [string, string]; active?: string; sub?: string; result?: string; status?: string; journal?: string }): { stdout: string; journal: string } {
  const dir = mkdtempSync(join(tmpdir(), "wsp-daemon-exits-"));
  dirs.push(dir);
  writeFileSync(join(dir, "counts"), `${o.counts.join("\n")}\n`);
  writeFileSync(join(dir, "journal"), o.journal ?? "");
  writeStub(
    join(dir, "systemctl"),
    [
      "#!/bin/sh",
      `case "$*" in`,
      `  "show -p NRestarts --value ${DAEMON_UNIT}") head -n 1 "${dir}/counts"; sed -i.was 1d "${dir}/counts" ;;`,
      `  "show -p ActiveState --value ${DAEMON_UNIT}") echo ${o.active ?? "active"} ;;`,
      `  "show -p SubState --value ${DAEMON_UNIT}") echo ${o.sub ?? "auto-restart"} ;;`,
      `  "show -p Result --value ${DAEMON_UNIT}") echo ${o.result ?? "exit-code"} ;;`,
      `  "show -p ExecMainStatus --value ${DAEMON_UNIT}") echo ${o.status ?? "2"} ;;`,
      `  *) echo "unexpected: $*" >&2; exit 1 ;;`,
      "esac",
      "",
    ].join("\n"),
  );
  writeStub(join(dir, "journalctl"), `#!/bin/sh\necho "$*" > "${dir}/asked"\ncat "${dir}/journal"\n`);
  writeStub(join(dir, "sleep"), "#!/bin/sh\nexit 0\n");
  const run = spawnSync("bash", ["-c", DAEMON_EXITS_CMD], { encoding: "utf8", env: { PATH: `${dir}:/usr/bin:/bin`, HOME: dir } });
  expect(run.stderr).toBe("");
  const asked = spawnSync("cat", [join(dir, "asked")], { encoding: "utf8" }).stdout.trim();
  return { stdout: run.stdout, journal: asked };
}

/** Three runs of a daemon from before --doors, as journalctl -o cat printed them: no blank lines, and the window of
 * the last lines starting inside a run. */
const LOOPED = ["For more information, try '--help'.", "error: unexpected argument '--doors' found", "Usage: wsp-daemon [OPTIONS]", "For more information, try '--help'.", "error: unexpected argument '--doors' found", "Usage: wsp-daemon [OPTIONS]", "For more information, try '--help'.", ""].join("\n");

describe("the look at a box's daemon unit", () => {
  it("reads a unit restarting under Restart=always as a daemon that exits, in the daemon's own words, each once and in their order", () => {
    const { stdout, journal } = look({ counts: ["4", "6"], journal: LOOPED });
    expect(daemonExitsOf(stdout)).toBe("error: unexpected argument '--doors' found; Usage: wsp-daemon [OPTIONS]; For more information, try '--help'.");
    // The daemon's own output, not systemd's lines about the unit.
    expect(journal).toBe(`_SYSTEMD_UNIT=${DAEMON_UNIT} -n 20 --no-pager -o cat`);
  });

  it("reads a unit a start limit left failed as a daemon that exits, however still its count", () => {
    expect(daemonExitsOf(look({ counts: ["26", "26"], active: "failed", journal: LOOPED }).stdout)).toBe("error: unexpected argument '--doors' found; Usage: wsp-daemon [OPTIONS]; For more information, try '--help'.");
  });

  it("cuts each line short, so the error before today's daemon's usage line, which runs to 767 characters, still shows", () => {
    const usage = `usage: wsp-daemon ${"[--host <addr>] ".repeat(48)}`.trim();
    const said = daemonExitsOf(look({ counts: ["20", "22"], journal: `error: unexpected argument '--no-such-flag' found\n${usage}\n`.repeat(4) }).stdout);
    expect(said).toBe(`error: unexpected argument '--no-such-flag' found; ${usage.slice(0, 120)}…`);
  });

  it("reads a daemon that lives three seconds before it exits, which restarts once a window, once its fifth restart lands", () => {
    expect(daemonExitsOf(look({ counts: ["4", "5"], journal: "panicked at slow start\n" }).stdout)).toBe("panicked at slow start");
  });

  it("says the exit status of a daemon that exits printing nothing", () => {
    expect(daemonExitsOf(look({ counts: ["0", "5"], status: "101" }).stdout)).toBe("exit status 101");
  });

  it("says the signal that killed a daemon that printed nothing, as the unit reads it while it waits to restart", () => {
    expect(daemonExitsOf(look({ counts: ["4", "6"], result: "signal", status: "9" }).stdout)).toBe("killed by signal 9");
    expect(daemonExitsOf(look({ counts: ["4", "6"], result: "oom-kill", status: "9" }).stdout)).toBe("killed for want of memory");
  });

  it("names no status once the next copy runs, since the unit then reads success and 0 whatever ended the last one", () => {
    expect(daemonExitsOf(look({ counts: ["4", "6"], sub: "running", result: "success", status: "0" }).stdout)).toBe("it printed nothing");
  });

  it.each([
    { how: "a daemon that runs", counts: ["0", "0"] as [string, string] },
    { how: "a daemon that exited once on purpose", counts: ["3", "4"] as [string, string] },
    { how: "a daemon that exited five times and now runs", counts: ["7", "7"] as [string, string] },
    { how: "a unit not started yet", counts: ["0", "0"] as [string, string], active: "inactive" },
    { how: "a unit the box does not have", counts: ["", ""] as [string, string], active: "inactive" },
  ])("reads nothing off $how", ({ counts, active }) => {
    const { stdout, journal } = look({ counts, ...(active !== undefined ? { active } : {}), journal: LOOPED });
    expect(stdout).toBe("");
    expect(daemonExitsOf(stdout)).toBeUndefined();
    expect(journal).toBe("");
  });
});
