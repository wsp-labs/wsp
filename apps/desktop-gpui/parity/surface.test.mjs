// SPDX-License-Identifier: AGPL-3.0-only
// node --test apps/desktop-gpui/parity/surface.test.mjs: the surface check on drawn windows, no app.
import { spawnSync } from "node:child_process";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "node:test";
import assert from "node:assert/strict";
import { crc32, deflateSync } from "node:zlib";
import { encode } from "./png.mjs";
import { REGIONS, compareSurfaces, describe, parseArgs, patchMean, readShot, regionsFor, surfaceCheck } from "./surface.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const GROUND = { sidebar: [244, 244, 245], header: [244, 244, 245], thread: [250, 250, 250], panel: [250, 250, 250], composer: [255, 255, 255] };

/** A 1280x800 window at scale 1 laid out as both apps draw it, each part in its colour; `noise` adds -1, 0 or +1 a pixel. */
function windowOf(colours = {}, { noise = false, channels = 3 } = {}) {
  const c = { ...GROUND, ...colours };
  const [width, height] = [1280, 800];
  const pixels = Buffer.alloc(width * height * channels);
  for (let y = 0; y < height; y++)
    for (let x = 0; x < width; x++) {
      const part =
        x >= 276 && x <= 720 && y >= 700 && y <= 760 ? c.composer
        : x < 256 ? (y < 52 ? c.sidebarHeader ?? c.header : c.sidebar)
        : x < 740 ? (y < 52 ? c.threadHeader ?? c.header : c.thread)
        : y < 52 ? c.panelHeader ?? c.header
        : c.panel;
      const jitter = noise ? (((x * 2654435761) ^ (y * 40503)) >>> 0) % 3 - 1 : 0;
      const at = (y * width + x) * channels;
      for (let k = 0; k < 3; k++) pixels[at + k] = Math.min(255, Math.max(0, part[k] + jitter));
      if (channels === 4) pixels[at + 3] = 255;
    }
  return encode({ width, height, channels, pixels }, 1);
}

test("equal grounds pass, region by region", () => {
  const got = surfaceCheck(windowOf(), windowOf());
  assert.equal(got.ok, true);
  assert.equal(got.scale, 1);
  assert.deepEqual(got.rows.map(r => r.region), Object.keys(REGIONS));
  assert.equal(got.rows.find(r => r.region === "composer").gpui, "#ffffff");
});

test("the web's grain alone passes", () => {
  assert.equal(surfaceCheck(windowOf({}, { noise: true }), windowOf()).ok, true);
  assert.equal(surfaceCheck(windowOf({}, { noise: true, channels: 4 }), windowOf()).ok, true, "an RGBA capture reads as its RGB");
});

test("a header one shade off fails, naming the header and both colours", () => {
  const got = surfaceCheck(windowOf(), windowOf({ threadHeader: [249, 250, 250] }));
  assert.equal(got.ok, false);
  const off = got.rows.filter(r => !r.same);
  assert.deepEqual(off.map(r => r.region), ["thread header"]);
  assert.deepEqual([off[0].electron, off[0].gpui], ["#f4f4f5", "#f9fafa"]);
  assert.equal(describe(got)[0], "DIFFERS thread header at 730,8: Electron #f4f4f5, GPUI #f9fafa");
  const shade = surfaceCheck(windowOf(), windowOf({ sidebarHeader: [244, 244, 246] }));
  assert.deepEqual(shade.rows.filter(r => !r.same).map(r => r.region), ["sidebar header"], "one level in one channel is a difference");
});

test("a shot that is not 8-bit RGB is refused with what to save it as", () => {
  const chunk = (type, data) => {
    const body = Buffer.concat([Buffer.from(type, "latin1"), data]);
    const out = Buffer.alloc(body.length + 8);
    out.writeUInt32BE(data.length, 0);
    body.copy(out, 4);
    out.writeUInt32BE(crc32(body), body.length + 4);
    return out;
  };
  const head = Buffer.alloc(13);
  head.writeUInt32BE(4, 0);
  head.writeUInt32BE(4, 4);
  head.set([8, 0, 0, 0, 0], 8);
  const grey = Buffer.concat([Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]), chunk("IHDR", head), chunk("IDAT", deflateSync(Buffer.alloc(20))), chunk("IEND", Buffer.alloc(0))]);
  assert.throws(() => readShot(grey, "gpui.png"), /^Error: gpui\.png: a PNG of depth 8, colour type 0.*PNG24:/);
  assert.throws(() => surfaceCheck(windowOf(), grey), /the GPUI shot: .*colour type 0/);
});

test("a state moves or drops regions; a patch off the shot is an error", () => {
  const r = regionsFor({ composer: { x: 498, y: 420 }, panel: null });
  assert.deepEqual(r.composer, { x: 498, y: 420 });
  assert.equal("panel" in r, false);
  assert.deepEqual(r.sidebar, REGIONS.sidebar);
  assert.throws(() => regionsFor({ sidebar: { x: 1 } }), /takes \{ x, y \}/);
  const shot = readShot(windowOf());
  assert.deepEqual(patchMean(shot, 1, { x: 200, y: 26 }), [244, 244, 245]);
  assert.throws(() => patchMean(shot, 1, { x: 2, y: 400 }), /falls off/);
  assert.throws(() => compareSurfaces(shot, { ...shot, width: 640 }), /differ in size/);
});

test("the command line exits 1 on a difference and prints the region", () => {
  const dir = mkdtempSync(join(tmpdir(), "surface-test-"));
  try {
    writeFileSync(join(dir, "e.png"), windowOf());
    writeFileSync(join(dir, "g.png"), windowOf({ panelHeader: [240, 240, 240] }));
    const run = (...argv) => spawnSync(process.execPath, [join(here, "surface.mjs"), ...argv], { encoding: "utf8", env: { PATH: "/usr/bin:/bin" } });
    const off = run(join(dir, "e.png"), join(dir, "g.png"));
    assert.equal(off.status, 1, off.stderr);
    assert.match(off.stdout, /DIFFERS panel header at 1000,6: Electron #f4f4f5, GPUI #f0f0f0/);
    const dropped = run(join(dir, "e.png"), join(dir, "g.png"), "--regions", JSON.stringify({ "panel header": null }));
    assert.equal(dropped.status, 0, dropped.stdout);
    assert.equal(run(join(dir, "e.png")).status, 2);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
  assert.throws(() => parseArgs(["a.png", "b.png", "--fuzz", "3"]), /not a flag/);
});
