// SPDX-License-Identifier: AGPL-3.0-only
// node --test apps/desktop-gpui/parity/baseline.test.mjs: the parts of the parity driver that need no app, and that
// each wave's states.mjs shoots exactly the states its spec gives a baseline of their own.
import { spawnSync } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { test } from "node:test";
import assert from "node:assert/strict";
import { crc32, deflateSync } from "node:zlib";
import { crop, decode } from "./png.mjs";

const here = dirname(fileURLToPath(import.meta.url));
// Only a wave whose spec names each state's baseline file (wave 1's `png` words) can be held to its states.mjs.
const waves = readdirSync(here).filter(name => /^wave-\d+$/.test(name) && existsSync(join(here, name, "spec.md")) && readFileSync(join(here, name, "spec.md"), "utf8").includes("png `"));

/** Every state item of a wave's spec files: its id and the baseline it is shot in, its own, another's, or none. */
function specStates(wave) {
  const states = [];
  for (const file of ["spec.md", "more.md"]) {
    const path = join(here, wave, file);
    if (!existsSync(path)) continue;
    let list = "";
    for (const line of readFileSync(path, "utf8").split("\n")) {
      if (line.startsWith("### ")) list = line.slice(4);
      const item = /^- (W\d+-[A-Z]+-\d+) (.*)$/.exec(line);
      if (item === null || list !== "States") continue;
      const png = /png `(W\d+-[A-Z]+-\d+)-<theme>\.png`/.exec(item[2])?.[1];
      states.push({ id: item[1], png: png ?? null });
    }
  }
  return states;
}

for (const wave of waves) {
  test(`${wave}: states.mjs shoots each state with a baseline of its own, and no other`, async () => {
    const listed = specStates(wave);
    assert.ok(listed.length > 0, `${wave}'s spec lists no states`);
    const own = listed.filter(s => s.png === s.id).map(s => s.id);
    for (const s of listed.filter(s => s.png !== null && s.png !== s.id)) assert.ok(own.includes(s.png), `${s.id} shares ${s.png}, which shoots no baseline of its own`);
    const { default: states } = await import(pathToFileURL(join(here, wave, "states.mjs")).href);
    const shot = states.map(s => s.id);
    assert.deepEqual(shot.filter(id => !own.includes(id)), [], "states.mjs shoots states the spec does not give a baseline");
    assert.deepEqual(own.filter(id => !shot.includes(id)), [], "the spec gives these a baseline and states.mjs never shoots them");
  });
}

const run = args => spawnSync(process.execPath, [join(here, "baseline.mjs"), ...args], { encoding: "utf8", env: { PATH: process.env.PATH } });

test("a run with a flag missing is refused with the usage line", () => {
  const got = run(["--wave", "1"]);
  assert.equal(got.status, 2);
  assert.match(got.stderr, /usage: node baseline\.mjs --wave <n>.*--out is missing/);
});

test("a theme other than paper or graphite is refused", () => {
  const got = run(["--wave", "1", "--out", tmpdir(), "--app", "/nowhere/wsp", "--fixture", "/nowhere.json", "--theme", "blue"]);
  assert.equal(got.status, 2);
  assert.match(got.stderr, /--theme takes paper or graphite, not blue/);
});

test("a fixture folder fixture.sh did not mark is refused before anything launches", () => {
  const home = mkdtempSync(join(tmpdir(), "parity-test-"));
  try {
    const fixture = join(home, "fx.json");
    writeFileSync(fixture, JSON.stringify({ state: join(home, "state", "state.json"), pid: 1, threads: {} }));
    const got = run(["--wave", "1", "--out", join(home, "out"), "--app", "/nowhere/wsp", "--fixture", fixture]);
    assert.equal(got.status, 2);
    assert.match(got.stderr, /has no \.wsp-bench-fixture mark/);
    assert.equal(existsSync(join(home, "out")), false);
  } finally {
    rmSync(home, { recursive: true, force: true });
  }
});

/** A PNG written plainly, every row unfiltered, so the crop is checked against a writer that is not its own. */
function plainPng(width, height, channels, pixel) {
  const rows = [];
  for (let y = 0; y < height; y++) rows.push(Buffer.from([0, ...Array.from({ length: width }, (_, x) => pixel(x, y).slice(0, channels)).flat()]));
  const chunk = (type, data) => {
    const body = Buffer.concat([Buffer.from(type, "latin1"), data]);
    const out = Buffer.alloc(body.length + 8);
    out.writeUInt32BE(data.length, 0);
    body.copy(out, 4);
    out.writeUInt32BE(crc32(body), body.length + 4);
    return out;
  };
  const head = Buffer.alloc(13);
  head.writeUInt32BE(width, 0);
  head.writeUInt32BE(height, 4);
  head.set([8, channels === 4 ? 6 : 2, 0, 0, 0], 8);
  return Buffer.concat([Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]), chunk("IHDR", head), chunk("IDAT", deflateSync(Buffer.concat(rows))), chunk("IEND", Buffer.alloc(0))]);
}

// Gradients, a hard edge and noise, so the encoder picks different filters on different rows.
const pixelAt = (x, y) => [(x * 37 + y * 11) & 0xff, x < 9 ? 20 : 230, (x * y * 2654435761) >>> 24, 128 + ((x - y) & 0x3f)];

for (const channels of [3, 4]) {
  test(`a clip is cut from a whole ${channels === 4 ? "RGBA" : "RGB"} capture at the real scale, pixel for pixel`, () => {
    const png = plainPng(20, 14, channels, pixelAt);
    for (const [rect, scale] of [
      [{ x: 3, y: 2, width: 9, height: 7 }, 1],
      [{ x: 2, y: 1, width: 5, height: 4 }, 2],
      [{ x: 15, y: 10, width: 30, height: 30 }, 1],
    ]) {
      const cut = crop(png, rect, scale);
      const back = decode(cut.png);
      const [x0, y0] = [rect.x * scale, rect.y * scale];
      const [w, h] = [Math.min(20, (rect.x + rect.width) * scale) - x0, Math.min(14, (rect.y + rect.height) * scale) - y0];
      assert.deepEqual([back.width, back.height, cut.width, cut.height], [w, h, w, h]);
      for (let y = 0; y < h; y++)
        for (let x = 0; x < w; x++) {
          const at = (y * w + x) * channels;
          assert.deepEqual([...back.pixels.subarray(at, at + channels)], pixelAt(x0 + x, y0 + y).slice(0, channels), `pixel ${x},${y} of the clip ${JSON.stringify(rect)} at ${scale}x`);
        }
      const phys = cut.png.indexOf("pHYs", 0, "latin1");
      assert.equal(cut.png.readUInt32BE(phys + 4), Math.round((scale * 72) / 0.0254));
    }
  });
}
