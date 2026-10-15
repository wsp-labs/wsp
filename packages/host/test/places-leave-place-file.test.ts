// SPDX-License-Identifier: AGPL-3.0-only
// The place file sits in a home a workspace writes, and wsp leave reads it first, as root on a box. So it is read as
// the leave reads a login file or an agent's config: no link followed, no wait on a fifo, a plain file only, a cap.
import { lstatSync, mkdirSync, symlinkSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { describe, expect, it } from "vitest";
import { placeFileText } from "@wsp/protocol";
import { brokenPlaceLeftLine } from "../src/places.js";
import { PLACE_READ_MAX_BYTES, placeFilePath, readPlaceFile } from "../src/place-report.js";
import { captured } from "./verbs-fixture.js";
import { fakeRunner, fifoWriter, leaveCommand, made, tmp } from "./places-fixture.js";

const PLACE_TEXT = placeFileText({ placeId: "p_box", name: "box", hostName: "studio", hostUrls: ["http://192.168.1.5:4640"], hostPublicKey: "c3R1ZGlv", keyPath: "/root/.wsp/place.key", joinedAt: "2026-10-15T00:00:00.000Z" });

/** A home with a place file's folder and nothing in it yet. */
function placeHome(name: string): { home: string; file: string } {
  const home = tmp(name);
  const file = placeFilePath(home);
  mkdirSync(dirname(file), { recursive: true });
  return { home, file };
}

const leave = (home: string) => {
  const io = captured();
  return leaveCommand(io, [], { home, run: fakeRunner().run, platform: "linux" }, { yes: true }).then(code => ({ code, lines: io.lines }));
};

describe("wsp leave reads the place file as a plain file only, and no further than a cap", () => {
  it("still reads a plain place file as the join it names", async () => {
    const { home, file } = placeHome("place-read-plain");
    writeFileSync(file, PLACE_TEXT);
    expect(readPlaceFile(file)?.name).toBe("box");
    const left = await leave(home);
    expect(left.lines[0]).toBe("box left the wsp at http://192.168.1.5:4640; removed:");
  });

  it("answers at once on a fifo at the place file, naming it as not read, and takes it", async () => {
    const { home, file } = placeHome("place-read-fifo");
    await made(file);
    // A late writer stands in for nobody ever writing: a read that waits on the fifo gets a whole place file three
    // seconds on and names the join, where one that never waits has said it could not read it long before.
    const late = fifoWriter(file, PLACE_TEXT, 3000);
    try {
      const left = await leave(home);
      expect(left.code).toBe(0);
      expect(left.lines[0]).toBe(brokenPlaceLeftLine(file));
      expect(late.opened()).toBe("");
      expect(() => lstatSync(file)).toThrow();
    } finally {
      late.stop();
    }
  });

  it("opens no link at the place file, whatever it points at", async () => {
    const far = join(tmp("place-read-far"), "fifo");
    await made(far);
    const { home, file } = placeHome("place-read-link");
    symlinkSync(far, file);
    // A writer's open of a fifo waits until something opens its read end, so its return says the leave opened it.
    const writer = fifoWriter(far, PLACE_TEXT);
    try {
      const left = await leave(home);
      await new Promise(resolve => setTimeout(resolve, 200));
      expect(writer.opened()).toBe("");
      expect(left.lines[0]).toBe(brokenPlaceLeftLine(file));
    } finally {
      writer.stop();
    }
  });

  it("reads a link to a whole place file as not read", () => {
    const real = join(tmp("place-read-real"), "place.json");
    writeFileSync(real, PLACE_TEXT);
    const { file } = placeHome("place-read-pointed");
    symlinkSync(real, file);
    expect(readPlaceFile(file)).toBeUndefined();
  });

  it("reads a file past the cap as not read", async () => {
    const { home, file } = placeHome("place-read-big");
    // Padded with spaces, which JSON reads past, so only the cap keeps it from parsing as the join it names.
    writeFileSync(file, PLACE_TEXT.padEnd(PLACE_READ_MAX_BYTES + 1));
    expect(readPlaceFile(file)).toBeUndefined();
    expect((await leave(home)).lines[0]).toBe(brokenPlaceLeftLine(file));
  });
});
