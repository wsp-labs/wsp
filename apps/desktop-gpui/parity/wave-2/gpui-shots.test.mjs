// SPDX-License-Identifier: AGPL-3.0-only
// node --test apps/desktop-gpui/parity/wave-2/gpui-shots.test.mjs: the parts of gpui-shots.mjs that need no app.
import { test } from "node:test";
import assert from "node:assert/strict";
import { join } from "node:path";
import { STATES, baselineArgs, deadline, geometry, inner, launchEnv, order, parseAE, parseArgs, picked, scaleClip, share } from "./gpui-shots.mjs";

const base = ["--fixture", "/f/fx.json", "--app", "/a/desktop-gpui", "--electron-app", "/x/wsp", "--electron", "/e", "--out", "/o"];

test("reads its flags", () => {
  const a = parseArgs(base);
  assert.equal(a.app, "/a/desktop-gpui");
  assert.equal(a.electronApp, "/x/wsp");
  assert.deepEqual(a.themes, ["paper", "graphite"]);
  assert.equal(a.display, ":96");
  assert.equal(a.only, undefined);
  const b = parseArgs([...base, "--theme", "graphite", "--only", "W2-PAL-01*", "--display", ":95"]);
  assert.deepEqual(b.themes, ["graphite"]);
  assert.equal(b.only, "W2-PAL-01*");
  assert.equal(b.display, ":95");
});

test("refuses what it cannot run", () => {
  assert.throws(() => parseArgs(base.slice(2)), /--fixture is missing/);
  assert.throws(() => parseArgs([...base.slice(0, 4), ...base.slice(6)]), /--electron-app is missing/);
  assert.throws(() => parseArgs([...base, "--theme", "ink"]), /--theme takes paper or graphite/);
  assert.throws(() => parseArgs([...base, "--scale", "2"]), /not a flag/);
  assert.throws(() => parseArgs([...base, "--only"]), /--only takes a value/);
  assert.throws(() => parseArgs([...base, "--only", "--theme"]), /--only takes a value/);
  assert.throws(() => parseArgs([...base, "--display", "96"]), /--display takes/);
});

test("picks states by glob", () => {
  const ids = STATES.map(s => s.id);
  assert.deepEqual(picked(ids, undefined), ids);
  assert.deepEqual(picked(ids, "W2-UNO-*"), ["W2-UNO-016", "W2-UNO-017"]);
  assert.deepEqual(picked(ids, "W2-PAL-01?, W2-PAL-036"), ["W2-PAL-016", "W2-PAL-017", "W2-PAL-018", "W2-PAL-019", "W2-PAL-036"]);
  assert.deepEqual(picked(ids, "W2-DLG-*"), []);
});

test("shoots each state's GPUI side right after its Electron side", () => {
  assert.deepEqual(
    order(["W2-PAL-017", "W2-PAL-034"], ["paper", "graphite"]).map(s => `${s.side} ${s.id} ${s.theme}`),
    [
      "electron W2-PAL-017 paper",
      "gpui W2-PAL-017 paper",
      "electron W2-PAL-017 graphite",
      "gpui W2-PAL-017 graphite",
      "electron W2-PAL-034 paper",
      "gpui W2-PAL-034 paper",
      "electron W2-PAL-034 graphite",
      "gpui W2-PAL-034 graphite",
    ],
  );
});

test("hands baseline.mjs one state in one theme at the GPUI side's scale", () => {
  const argv = baselineArgs(parseArgs(base), "W2-UNO-016", "graphite");
  assert.match(argv[0], /parity\/baseline\.mjs$/);
  assert.deepEqual(argv.slice(1), ["--wave", "2", "--fixture", "/f/fx.json", "--app", "/x/wsp", "--out", "/e", "--only", "W2-UNO-016", "--theme", "graphite", "--scale", "2"]);
});

test("gives each launch a HOME of its own under the run's folder, and LANG", () => {
  const steps = ["electron", "gpui"].flatMap(side => ["W2-PAL-016", "W2-PAL-035"].flatMap(id => ["paper", "graphite"].map(theme => ({ side, id, theme }))));
  const envs = steps.map(s => launchEnv("/t/run", s, ":96"));
  assert.equal(new Set(envs.map(e => e.HOME)).size, steps.length);
  for (const e of envs) {
    assert.ok(e.HOME.startsWith(join("/t/run", "homes")), e.HOME);
    assert.equal(e.LANG, "en_US.UTF-8");
    assert.equal(e.DISPLAY, ":96");
  }
  assert.equal(launchEnv("/t/run", { side: "gpui", id: "W2-PAL-016", theme: "paper" }, ":96").GPUI_X11_SCALE_FACTOR, "2");
  assert.equal(launchEnv("/t/run", { side: "electron", id: "W2-PAL-016", theme: "paper" }, ":96").GPUI_X11_SCALE_FACTOR, undefined);
});

test("scales a CSS clip to device pixels and clamps it to the window", () => {
  assert.deepEqual(scaleClip({ x: 336, y: 64, width: 608, height: 452 }), { x: 672, y: 128, width: 1216, height: 904 });
  assert.deepEqual(scaleClip({ x: 724, y: 0, width: 556, height: 800 }), { x: 1448, y: 0, width: 1112, height: 1600 });
  assert.deepEqual(scaleClip({ x: 1200, y: 780, width: 200, height: 100 }), { x: 2400, y: 1560, width: 160, height: 40 });
  assert.deepEqual(scaleClip({ x: -10, y: 0, width: 20, height: 20 }, 1), { x: 0, y: 0, width: 20, height: 20 });
  assert.deepEqual(scaleClip(null), { x: 0, y: 0, width: 2560, height: 1600 });
  assert.equal(geometry(scaleClip({ x: 16, y: 299, width: 1248, height: 202 })), "2496x404+32+598");
});

test("cuts the page around the element and its own edge off a capture", () => {
  assert.deepEqual(inner(1216, 904), { x: 40, y: 40, width: 1136, height: 824 });
  assert.deepEqual(inner(100, 100, 1), { x: 20, y: 20, width: 60, height: 60 });
  assert.throws(() => inner(80, 900), /nothing inside/);
});

test("counts differing pixels as a share", () => {
  assert.equal(share(0, 1216 * 904), 0);
  assert.equal(share(1216 * 904, 1216 * 904), 100);
  assert.equal(share(1, 3), 33.33);
  assert.equal(share(27_489, 1_099_264), 2.5);
  assert.throws(() => share(1, 0), /a share of 0 pixels/);
  assert.equal(parseAE("0"), 0);
  assert.equal(parseAE("1.09926e+06\n"), 1_099_260);
  assert.equal(parseAE("4521 (0.0123)"), 4521);
  assert.throws(() => parseAE("compare: image widths or heights differ"), /no pixel count/);
});

test("a shot past its deadline stops its own app and no later shot's", async () => {
  const stopped = [];
  const stop = shot => stopped.push(shot.app);
  let release;
  const first = deadline(20, async shot => {
    shot.app = "first";
    await new Promise(done => (release = done));
    if (!shot.aborted) stopped.push("first, from its own tail");
  }, stop);
  await assert.rejects(first, /ran past 0.02 s/);
  const second = await deadline(1000, async shot => ((shot.app = "second"), "shot"), stop);
  assert.equal(second, "shot");
  release();
  await new Promise(done => setTimeout(done, 10));
  assert.deepEqual(stopped, ["first"]);
});

test("a shot that throws stops its own app", async () => {
  const stopped = [];
  await assert.rejects(deadline(1000, async shot => ((shot.app = "only"), Promise.reject(new Error("no window"))), shot => stopped.push(shot.app)), /no window/);
  assert.deepEqual(stopped, ["only"]);
});
