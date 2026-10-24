// SPDX-License-Identifier: AGPL-3.0-only
// node --test apps/desktop-gpui/parity/wave-2/gpui-shots.test.mjs: the parts of gpui-shots.mjs that need no app.
import { test } from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { STATES, baselineArgs, untilStill, caveatOf, deadline, geometry, inner, launchEnv, order, pairCompare, parseAE, parseArgs, picked, scaleClip, share, surfaceWords, xTools } from "./gpui-shots.mjs";

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

test("says where the GPUI side made the Electron state's data a second time", () => {
  assert.equal(caveatOf({}, false), undefined);
  assert.equal(caveatOf({ caveat: "read the pair" }, false), "read the pair");
  assert.match(caveatOf({}, true), /one more of what that data makes/);
  assert.match(caveatOf({ caveat: "read the pair" }, true), /^read the pair; the GPUI side ran/);
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

test("drives the Processes pane and Activity states as the Electron side reaches them", async () => {
  const said = [];
  const run = async id => {
    said.length = 0;
    for (const step of STATES.find(s => s.id === id).steps) await step.do((...argv) => said.push(argv.join(" ")), {});
    return [...said];
  };
  assert.deepEqual(await run("W2-PAL-023"), ["key --clearmodifiers ctrl+alt+b", "key --clearmodifiers p"]);
  assert.equal(STATES.find(s => s.id === "W2-PAL-025").open, "data");
  assert.ok((await run("W2-PAL-025")).some(c => c.includes("http.server 0")), "a server in the thread's terminal first");
  assert.equal((await run("W2-PAL-028")).at(-1), "getactivewindow mousemove --window %1 2162 50 click 1");
  for (const id of ["W2-PAL-027", "W2-PAL-028", "W2-PAL-038"]) assert.equal(STATES.find(s => s.id === id).still, true, `${id} waits out Activity's loading`);
  assert.equal((await run("W2-PAL-038")).at(-1), "getactivewindow windowsize 1200 1600", "600 by 800 at the GPUI side's scale");
  assert.ok((await run("W2-PAL-027")).includes("type --delay 40 Activity"));
});

test("waits for the clip to hold still, past a loading sweep", async () => {
  const frames = ["sweep-1", "sweep-2", "sweep-3", "rows", "rows"];
  let at = 0;
  const paused = [];
  const still = await untilStill(() => frames[Math.min(at++, frames.length - 1)], (a, b) => a === b, 60_000, async ms => paused.push(ms));
  assert.equal(still, true);
  assert.equal(at, 5, "grabbed until two grabs matched");
  assert.equal(await untilStill(() => `sweep-${at++}`, (a, b) => a === b, 30, async () => new Promise(done => setTimeout(done, 20))), false, "a sweep that never stops is never still");
});

test("a whole-window pair carries the surface check and a clip does not", { skip: spawnSync("convert", ["-version"]).status !== 0 && "no ImageMagick" }, () => {
  const dir = mkdtempSync(join(tmpdir(), "gpui-shots-test-"));
  try {
    const tools = xTools({ PATH: "/usr/bin:/bin" });
    const draw = (file, header) => tools.magick("convert", "-size", "1280x800", "xc:#f4f4f5", "-fill", header, "-draw", "rectangle 256,0 739,51", `PNG24:${join(dir, file)}`);
    draw("e.png", "#f4f4f5");
    draw("g.png", "#f9f9f9");
    const opts = { out: dir, scratch: dir, stem: "W" };
    const whole = pairCompare(tools, join(dir, "e.png"), join(dir, "g.png"), { ...opts, whole: true });
    assert.equal(whole.surface.ok, false);
    assert.equal(surfaceWords(whole.surface), "DIFFERS thread header at 730,8: Electron #f4f4f5, GPUI #f9f9f9");
    assert.equal(whole.inside, undefined);
    assert.ok(whole.shares["10%"] === 0 && whole.shares["0%"] > 2, "the fuzz scores a one-shade header as identical at 10%, which is why the surface check exists");
    const clipped = pairCompare(tools, join(dir, "e.png"), join(dir, "g.png"), opts);
    assert.equal(clipped.surface, undefined);
    assert.ok(clipped.inside !== undefined);
    assert.equal(pairCompare(tools, join(dir, "e.png"), join(dir, "e.png"), { ...opts, whole: true, regions: { panel: null } }).surface.ok, true);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});
