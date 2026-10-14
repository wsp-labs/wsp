// SPDX-License-Identifier: AGPL-3.0-only
import { spawnSync } from "node:child_process";
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname } from "node:path";
import { describe, expect, it } from "vitest";
import { placeDaemonPaths, wspBinIn, type PlaceFile } from "@wsp/protocol";
import { loginFilesStep, sshDaemonPlace } from "../src/doctor.js";
import { brokenJoinLine, joinCutByLeaveLine, placeHeldRefusal, placeLeaveLineAs, placeNoKeyForSudoLine, placeUndoNeedsSudoLine, placeUndoNoKeyLine, placeUndoOtherKeyLine } from "../src/places.js";
import { writeStub } from "../../protocol/test/stub-script.js";
import { tmp } from "./places-fixture.js";

/** A box an add joined, as far as a login there sees it: the bundle's wsp, which answers with the words it was
 * typed with, and the login files every add and update writes, run for real under bash. */
function joinedBox(): string {
  const home = tmp("leave-line-box");
  const at = placeDaemonPaths(home);
  const cli = wspBinIn(at.dir);
  mkdirSync(dirname(cli), { recursive: true });
  writeFileSync(cli, 'console.log("bundle wsp:", process.argv.slice(2).join(" "));\n');
  const ran = spawnSync("bash", ["-c", loginFilesStep(sshDaemonPlace({ home, path: "/usr/bin" })).join("\n")], { encoding: "utf8" });
  expect(ran.status, ran.stderr).toBe(0);
  return home;
}

/** A line typed at the box's own shell as root: sudo -i hands root's login shell the rest of the line, so both are
 * that login shell with a PATH that knows only node and the system. */
function typedOn(home: string, line: string): { status: number | null; out: string } {
  const asRoot = line.replace(/^sudo -i /, "");
  const ran = spawnSync("bash", ["-lc", asRoot], { encoding: "utf8", env: { HOME: home, PATH: `${dirname(process.execPath)}:/usr/bin:/bin` } });
  return { status: ran.status, out: `${ran.stdout}${ran.stderr}` };
}

const held: PlaceFile = { placeId: "p_x", name: "hetzner", hostName: "studio", hostUrls: ["http://127.0.0.1:23780"], hostPublicKey: "c3R1ZGlv", keyPath: "/root/.wsp/place-key.pem", joinedAt: "2026-10-12T17:27:11Z" };

describe("every line that names wsp leave names one a login on a joined box can type", () => {
  const lines: { said: string; command: string }[] = [
    { said: placeHeldRefusal("root@hetzner", held, undefined, undefined, placeLeaveLineAs(true)), command: "wsp leave" },
    { said: placeHeldRefusal("maya@box", held, undefined, undefined, placeLeaveLineAs(false)), command: "sudo -i wsp leave" },
    { said: placeHeldRefusal("root@hetzner", held, "SHA256:x", [], placeLeaveLineAs(true)), command: "wsp leave" },
    { said: brokenJoinLine("/root/.wsp/place.json"), command: "wsp leave" },
    { said: joinCutByLeaveLine, command: "wsp leave" },
    { said: placeNoKeyForSudoLine("maya@box", "box"), command: "sudo -i wsp leave" },
    { said: placeUndoNeedsSudoLine("maya@box"), command: "sudo -i wsp leave" },
    { said: placeUndoNoKeyLine("root@box"), command: "wsp leave" },
    { said: placeUndoNoKeyLine("maya@box"), command: "sudo -i wsp leave" },
    { said: placeUndoOtherKeyLine("root@box:2222"), command: "wsp leave" },
    { said: placeUndoOtherKeyLine("maya@box:2222"), command: "sudo -i wsp leave" },
  ];

  it.each(lines)("$said", ({ said, command }) => {
    expect(said.match(/(sudo -i )?wsp leave/)?.[0]).toBe(command);
    const typed = typedOn(joinedBox(), `${command} --help`);
    expect(typed.out).toBe("bundle wsp: leave --help\n");
    expect(typed.status).toBe(0);
  });

  it("finds the bundle's wsp behind a wsp of the person's own, and puts it on PATH once however often the files are written", () => {
    const home = joinedBox();
    const files = loginFilesStep(sshDaemonPlace({ home, path: "/usr/bin" })).join("\n");
    expect(spawnSync("bash", ["-c", files], { encoding: "utf8" }).status).toBe(0);
    const own = tmp("leave-line-own");
    writeStub(`${own}/wsp`, "#!/bin/sh\necho own wsp\n");
    const path = spawnSync("bash", ["-lc", 'printf %s "$PATH"'], { encoding: "utf8", env: { HOME: home, PATH: `${own}:/usr/bin:/bin` } }).stdout;
    expect(path.split(":").filter(dir => dir.startsWith(home))).toHaveLength(1);
    expect(spawnSync("bash", ["-lc", "wsp"], { encoding: "utf8", env: { HOME: home, PATH: `${own}:/usr/bin:/bin` } }).stdout).toBe("own wsp\n");
  });
});
