// SPDX-License-Identifier: AGPL-3.0-only
import { spawn, spawnSync } from "node:child_process";
import { existsSync, linkSync, mkdirSync, readFileSync, symlinkSync, truncateSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { describe, expect, it } from "vitest";
import { LOGIN_READ_MAX_BYTES, PLACE_LOGIN_PAST_CAP, placeDaemonPaths, placeLineStaysLine, placeLoginUnreadLine, wspBinIn, type PlaceFile } from "@wsp/protocol";
import { LOGIN_KEPT_LINE, profileSourceLine } from "../src/login-files.js";
import { addFoundScript, addUndoScript, addFound, DAEMON_GONE_LINE, joinedAddWrites, joinedPlace, loginFilesStep, sshDaemonPlace } from "../src/doctor.js";
import { brokenJoinLine, joinCutByLeaveLine, placeHeldRefusal, placeLeaveLineAs, placeNoKeyForSudoLine, placeUndoNeedsSudoLine, placeUndoNoKeyLine, placeUndoOtherKeyLine } from "../src/places.js";
import { writeStub } from "../../protocol/test/stub-script.js";
import { fakeRunner, sweepPlace, tmp } from "./places-fixture.js";

/** A box an add joined, as far as a login there sees it: the bundle's wsp, which answers with the words it was
 * typed with, and the login files every add and update writes, run for real under bash. */
function joinedBox(own: Record<string, string> = {}): string {
  const home = tmp("leave-line-box");
  for (const [name, text] of Object.entries(own)) writeFileSync(join(home, name), text);
  const at = placeDaemonPaths(home);
  const cli = wspBinIn(at.dir);
  mkdirSync(dirname(cli), { recursive: true });
  writeFileSync(cli, 'console.log("bundle wsp:", process.argv.slice(2).join(" "));\n');
  const ran = spawnSync("bash", ["-c", loginFilesStep(sshDaemonPlace({ home, path: "/usr/bin" })).join("\n")], { encoding: "utf8" });
  expect(ran.status, ran.stderr).toBe(0);
  return home;
}

/** A folder holding node alone, so a wsp installed beside node on this computer is never the one found. */
function nodeOnly(): string {
  const dir = tmp("leave-line-node");
  symlinkSync(process.execPath, `${dir}/node`);
  return dir;
}

/** A line run after what a bash login on the box reads, and nothing of this computer's own /etc/profile, which may
 * put a wsp of its own first. Root's login there reads ~/.profile, measured on hetzner. */
const login = (home: string, path: string, line: string) => spawnSync("/bin/bash", ["-c", `. "$HOME/.profile"\n${line}`], { encoding: "utf8", env: { HOME: home, PATH: path } });

/** A line typed at the box's own shell as root: sudo -i hands root's login shell the rest of the line, so both are
 * that login with a PATH that knows only node. */
function typedOn(home: string, line: string): { status: number | null; out: string } {
  const asRoot = line.replace(/^sudo -i /, "");
  const ran = login(home, nodeOnly(), asRoot);
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
    expect(login(home, own, 'printf %s "$PATH"').stdout.split(":").filter(dir => dir.startsWith(home))).toHaveLength(1);
    expect(login(home, own, "wsp").stdout).toBe("own wsp\n");
  });
});

describe("a box whose root login reads a bash login file before ~/.profile", () => {
  const OWN = "export EDITOR=vim\n";

  /** What `sudo -i` runs: root's own shell as a login, which reads the first of ~/.bash_profile, ~/.bash_login and
   * ~/.profile it finds, with a PATH that knows only node. */
  const loggedIn = (home: string, line: string) => spawnSync("/bin/bash", ["-l", "-c", line], { encoding: "utf8", env: { HOME: home, PATH: nodeOnly() } });

  it.each([".bash_profile", ".bash_login"])("finds the bundle's wsp through %s, and every road off the box leaves that file as it was", async file => {
    const home = joinedBox({ [file]: OWN });
    const typed = loggedIn(home, `${placeLeaveLineAs(true)} --help`);
    expect(`${typed.stdout}${typed.stderr}`).toBe("bundle wsp: leave --help\n");
    // An update writes the files again: one line of wsp's in their file however often.
    const files = loginFilesStep(sshDaemonPlace({ home, path: "/usr/bin" })).join("\n");
    expect(spawnSync("bash", ["-c", files], { encoding: "utf8" }).status).toBe(0);
    expect(readFileSync(join(home, file), "utf8").split("\n").filter(row => row.includes(placeDaemonPaths(home).profileFile))).toHaveLength(1);
    await sweepPlace({ home, manager: undefined, run: fakeRunner().run });
    expect(readFileSync(join(home, file), "utf8")).toBe(OWN);
  });

  it.each([".bash_profile", ".bash_login"])("takes wsp's line back out of %s when a failed add is undone, and makes no such file where there was none", file => {
    const home = tmp("leave-line-undo");
    writeFileSync(join(home, file), OWN);
    const place = joinedPlace({ home, path: "/usr/bin" }, { hostUrls: [], codeFile: `${placeDaemonPaths(home).wsp}/join-code`, name: "box" });
    const writes = joinedAddWrites(place, "/nonexistent/wsp-place.service").filter(w => w.path.startsWith(`${home}/`));
    const bash = (script: string) => spawnSync("/bin/bash", ["-c", script], { encoding: "utf8" });
    const found = addFound(bash(addFoundScript(place, writes, "true")).stdout, writes.length)!;
    expect(bash(loginFilesStep(place).join("\n")).status).toBe(0);
    expect(readFileSync(join(home, file), "utf8")).not.toBe(OWN);
    for (const other of [".bash_profile", ".bash_login"].filter(name => name !== file)) expect(existsSync(join(home, other))).toBe(false);
    expect(bash(addUndoScript(place, writes, found, "true", true)).stdout).toContain(DAEMON_GONE_LINE);
    expect(readFileSync(join(home, file), "utf8")).toBe(OWN);
    expect(existsSync(join(home, ".profile"))).toBe(false);
  });

  it("writes through no login file that is a link or has a second name, and names each one it left", () => {
    const home = tmp("leave-line-linked");
    const outside = tmp("leave-line-outside");
    const theirs = "root:x:0:0:root:/root:/bin/bash\n";
    writeFileSync(join(outside, "passwd"), theirs);
    writeFileSync(join(outside, "profile"), OWN);
    writeFileSync(join(outside, "login"), OWN);
    symlinkSync(join(outside, "passwd"), join(home, ".bash_profile"));
    symlinkSync(join(outside, "profile"), join(home, ".profile"));
    linkSync(join(outside, "login"), join(home, ".bash_login"));
    const ran = spawnSync("/bin/bash", ["-c", loginFilesStep(sshDaemonPlace({ home, path: "/usr/bin" })).join("\n")], { encoding: "utf8" });
    expect(ran.status, ran.stderr).toBe(0);
    for (const name of ["passwd", "profile", "login"]) expect(readFileSync(join(outside, name), "utf8")).toBe(name === "passwd" ? theirs : OWN);
    expect(ran.stdout.split("\n").filter(line => line.startsWith(`${LOGIN_KEPT_LINE} `))).toEqual([".profile", ".bash_profile", ".bash_login"].map(name => `${LOGIN_KEPT_LINE} ${join(home, name)}`));
    // A dangling link is a link too: nothing is made at the far end of it.
    const dangling = tmp("leave-line-dangling");
    symlinkSync(join(outside, "nowhere"), join(dangling, ".profile"));
    expect(spawnSync("/bin/bash", ["-c", loginFilesStep(sshDaemonPlace({ home: dangling, path: "/usr/bin" })).join("\n")]).status).toBe(0);
    expect(existsSync(join(outside, "nowhere"))).toBe(false);
    // A fifo is no plain file: an append to it, or a grep of it, waits for the other end for as long as the run lasts.
    const fifo = tmp("leave-line-fifo");
    expect(spawnSync("mkfifo", [join(fifo, ".bash_login")]).status).toBe(0);
    const step = spawnSync("/bin/bash", ["-c", loginFilesStep(sshDaemonPlace({ home: fifo, path: "/usr/bin" })).join("\n")], { encoding: "utf8", timeout: 10_000 });
    expect(step.status, step.error?.message).toBe(0);
    expect(step.stdout).toContain(`${LOGIN_KEPT_LINE} ${join(fifo, ".bash_login")}\n`);
    const place = joinedPlace({ home: fifo, path: "/usr/bin" }, { hostUrls: [], codeFile: `${placeDaemonPaths(fifo).wsp}/join-code`, name: "box" });
    const found = spawnSync("/bin/bash", ["-c", addFoundScript(place, joinedAddWrites(place, "/nonexistent/wsp-place.service"), "true")], { encoding: "utf8", timeout: 10_000 });
    expect(found.status, found.error?.message).toBe(0);
  });

  it("writes the take-out's copy beside no login file, so nothing put at that name takes the update's or the undo's write or holds it", () => {
    const outside = tmp("leave-line-beside-outside");
    writeFileSync(join(outside, "passwd"), "theirs\n");
    const run = (script: string) => spawnSync("/bin/bash", ["-c", script], { encoding: "utf8", timeout: 10_000 });
    // An update over a box already holding wsp's line in each login file takes it out before it writes it again.
    const home = tmp("leave-line-beside");
    const line = `${profileSourceLine(placeDaemonPaths(home).profileFile)}\n`;
    for (const name of [".profile", ".bash_profile", ".bash_login"]) writeFileSync(join(home, name), `${OWN}${line}`);
    symlinkSync(join(outside, "passwd"), join(home, ".profile.wsp-out"));
    symlinkSync(join(outside, "passwd"), join(home, ".bash_profile.wsp-out"));
    expect(spawnSync("mkfifo", [join(home, ".bash_login.wsp-out")]).status).toBe(0);
    const update = run(loginFilesStep(sshDaemonPlace({ home, path: "/usr/bin" })).join("\n"));
    expect(readFileSync(join(outside, "passwd"), "utf8")).toBe("theirs\n");
    expect(update.status, update.error?.message).toBe(0);
    for (const name of [".profile", ".bash_profile", ".bash_login"]) expect(readFileSync(join(home, name), "utf8")).toBe(`${OWN}${line}`);
    // The undo of a failed add takes the line out the same way.
    const undone = tmp("leave-line-beside-undo");
    writeFileSync(join(undone, ".bash_profile"), OWN);
    const place = joinedPlace({ home: undone, path: "/usr/bin" }, { hostUrls: [], codeFile: `${placeDaemonPaths(undone).wsp}/join-code`, name: "box" });
    const writes = joinedAddWrites(place, "/nonexistent/wsp-place.service").filter(w => w.path.startsWith(`${undone}/`));
    const found = addFound(run(addFoundScript(place, writes, "true")).stdout, writes.length)!;
    expect(run(loginFilesStep(place).join("\n")).status).toBe(0);
    symlinkSync(join(outside, "passwd"), join(undone, ".bash_profile.wsp-out"));
    expect(spawnSync("mkfifo", [join(undone, ".profile.wsp-out")]).status).toBe(0);
    const undo = run(addUndoScript(place, writes, found, "true", true));
    expect(readFileSync(join(outside, "passwd"), "utf8")).toBe("theirs\n");
    expect(undo.status, undo.error?.message).toBe(0);
    expect(readFileSync(join(undone, ".bash_profile"), "utf8")).toBe(OWN);
  });

  it("leaves its line in a login file the leave at the box finds linked or past the cap, and says why", async () => {
    const home = tmp("leave-line-sweep-linked");
    const line = profileSourceLine(placeDaemonPaths(home).profileFile);
    mkdirSync(join(home, "dotfiles"));
    writeFileSync(join(home, "dotfiles", "bash_profile"), `${OWN}${line}\n`);
    symlinkSync(join(home, "dotfiles", "bash_profile"), join(home, ".bash_profile"));
    writeFileSync(join(home, ".bash_login"), "");
    truncateSync(join(home, ".bash_login"), LOGIN_READ_MAX_BYTES + 1);
    const swept = await sweepPlace({ home, manager: undefined, run: fakeRunner().run });
    expect(readFileSync(join(home, "dotfiles", "bash_profile"), "utf8")).toBe(`${OWN}${line}\n`);
    expect(swept.kept).toContain(placeLoginUnreadLine(join(home, ".bash_profile")));
    expect(swept.kept).toContain(placeLineStaysLine("wsp's line", join(home, ".bash_login"), PLACE_LOGIN_PAST_CAP));
    // Neither a fifo nor a link to a device that never ends is read, and both stat as 0 bytes: the leave reads a plain
    // file only, and no further than the cap.
    const odd = tmp("leave-line-sweep-odd");
    expect(spawnSync("mkfifo", [join(odd, ".bash_login")]).status).toBe(0);
    symlinkSync("/dev/zero", join(odd, ".bash_profile"));
    const left = await sweepPlace({ home: odd, manager: undefined, run: fakeRunner().run });
    expect(left.removed.filter(said => said.includes(odd))).toEqual([]);
    expect(left.kept).toContain(placeLoginUnreadLine(join(odd, ".bash_profile")));
    expect(left.kept).toContain(placeLoginUnreadLine(join(odd, ".bash_login")));
  });

  it("opens no link at a login file in the leave at the box, whatever it points at", async () => {
    // A writer's open of a fifo waits until something opens its read end, so its return says the leave opened it.
    const far = join(tmp("leave-line-sweep-far"), "fifo");
    expect(spawnSync("mkfifo", [far]).status).toBe(0);
    const pointed = tmp("leave-line-sweep-pointed");
    symlinkSync(far, join(pointed, ".bash_profile"));
    const writer = spawn("/bin/sh", ["-c", 'exec 3>"$1"; echo opened', "sh", far], { stdio: ["ignore", "pipe", "ignore"] });
    try {
      let opened = "";
      writer.stdout.on("data", chunk => (opened += String(chunk)));
      expect((await sweepPlace({ home: pointed, manager: undefined, run: fakeRunner().run })).kept).toContain(placeLoginUnreadLine(join(pointed, ".bash_profile")));
      await new Promise(resolve => setTimeout(resolve, 200));
      expect(opened).toBe("");
    } finally {
      writer.kill();
    }
  });
});
