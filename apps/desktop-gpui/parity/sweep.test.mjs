// SPDX-License-Identifier: AGPL-3.0-only
// node --test apps/desktop-gpui/parity/sweep.test.mjs: the sweep's table against today's source, and its launches,
// with no app.
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "node:test";
import assert from "node:assert/strict";
import { encode } from "./png.mjs";
import { readShot } from "./surface.mjs";
import { BOXES, KEPT, PAIRS, SOURCES, TABLE, holds, madeThreads, standingOf, addressForms, bareFailures, centreScreens, inkBox, isBare, kindsOf, launchArgs, paneKinds, parseArgs, refusal, shotRow, stepWords } from "./sweep.mjs";
import sweepStates from "./wave-sweep/states.mjs";
import { LONG_BRIEF } from "./wave-sweep/drives.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const repo = join(here, "../../..");

test("every kind today's source names has a row, and every row a kind", () => {
  const kinds = kindsOf();
  assert.deepEqual(kinds.filter(k => !TABLE.some(r => r.kind === k)), [], "kinds with no row");
  assert.deepEqual(TABLE.map(r => r.kind).filter(k => !kinds.includes(k)), [], "rows for kinds today's source no longer has");
  assert.ok(kinds.includes("pane:agents") && kinds.includes("centre:ProjectHome") && kinds.includes("link:project"), kinds.join(" "));
});

test("a kind added to today's source without a row is caught", () => {
  const read = path => {
    const source = readFileSync(path, "utf8");
    return path === SOURCES.panes ? source.replace('| "slate"', '| "slate" | "logs"') : source;
  };
  const kinds = kindsOf(read);
  assert.ok(kinds.includes("pane:logs"));
  assert.deepEqual(kinds.filter(k => !TABLE.some(r => r.kind === k)), ["pane:logs"]);
});

test("reads the kinds from the shapes today's files write them in", () => {
  assert.deepEqual(paneKinds('export type RightPanelKind = "preview" | "terminal";'), ["preview", "terminal"]);
  assert.deepEqual(addressForms('const PREFIX = "#w/";\nconst LINK_ID = /x/;\nexport const LINK_KINDS = ["thread", "settings"] as const;'), { pieces: ["PREFIX"], links: ["thread", "settings"] });
  assert.deepEqual(centreScreens('function WorkspaceCenter() {\n  if (a) return <FirstRun />;\n  return <p data-k="centre-refused" />;\n}\nfunction Other() { return <Shell />; }\n'), ["FirstRun", "centre-refused"]);
  assert.throws(() => paneKinds("nothing"), /no RightPanelKind/);
});

test("each empty state still lives where the table says", () => {
  for (const { kind, file, holds } of KEPT) assert.ok(readFileSync(join(repo, file), "utf8").includes(holds), `${kind}: ${file} no longer holds ${holds}`);
});

test("every row is driven, skipped with a reason, missing with a reason, or the same as a row that is", () => {
  const ids = sweepStates.map(s => s.id);
  for (const row of [...TABLE, ...PAIRS]) {
    const shot = shotRow(row.kind);
    if (typeof shot.electron === "string") assert.ok(ids.includes(shot.electron), `${row.kind}: wave-sweep has no ${shot.electron}`);
    else assert.ok(shot.electron.skip.length >= 12, `${row.kind} says why today's app is not driven`);
    const g = shot.gpui;
    assert.ok(g.args !== undefined || g.plain === true || (g.missing ?? g.skip ?? "").length >= 8, `${row.kind}: a launch, or why not`);
  }
  const used = new Set([...TABLE, ...PAIRS].map(r => shotRow(r.kind).electron).filter(e => typeof e === "string"));
  assert.deepEqual(ids.filter(id => !used.has(id)), [], "wave-sweep states no row shoots");
  assert.throws(() => shotRow("pane:nothing"), /no row for pane:nothing/);
});

test("builds each GPUI launch from the fixture", () => {
  const ctx = { state: "/fx/state/state.json", threads: { panes: "T-panes", "short-1": "T-short" }, workspaces: { bench: "ws_1" }, projects: { quiet: "pr_q" } };
  const launch = kind => launchArgs(shotRow(kind).gpui, ctx);
  assert.deepEqual(launch("pane:agents"), ["--state", "/fx/state/state.json", "--open", "T-panes", "--pane", "agents"]);
  assert.deepEqual(launch("pane:diff"), ["--state", "/fx/state/state.json", "--open", "T-panes", "--pane", "changes"]);
  assert.deepEqual(launch("pane:machine").slice(-1), ["computer"]);
  assert.deepEqual(launch("hash:NEW"), ["--state", "/fx/state/state.json", "--screen", "composer:ws_1"]);
  assert.deepEqual(launch("centre:WorkspaceThread"), ["--state", "/fx/state/state.json", "--open", "T-short"]);
  assert.deepEqual(launch("centre:ProjectHome"), ["--state", "/fx/state/state.json"], "no thread selected is a launch that opens nothing");
  assert.deepEqual(launch("empty:EmptyThread"), ["--state", "/fx/state/state.json", "wsp://project/pr_q"]);
  assert.equal(shotRow("first:Onboarding").gpui.empty, true);
  assert.deepEqual(launchArgs(shotRow("pair:long-message").gpui, { ...ctx, threads: { "long-message": "T-long" } }), ["--state", "/fx/state/state.json", "--open", "T-long"]);
  assert.equal(shotRow("pair:agents").kind, "pane:agents");
  assert.equal(shotRow("pair:starting").fixture, "starting");
  assert.equal(shotRow("pair:started").fixture, "stood");
  assert.deepEqual(launchArgs(shotRow("pair:starting").gpui, { ...ctx, threads: { lead: "T-lead" } }).slice(-2), ["--open", "T-lead"]);
  assert.throws(() => launchArgs({ args: ["--open", "{thread:gone}"] }, ctx), /names no thread gone/);
  assert.deepEqual(stepWords({ click: [498, 722] }, "7"), [["mousemove", "--window", "7", "996", "1444", "click", "1"]]);
  assert.deepEqual(stepWords({ key: "ctrl+j" }, "7"), [["key", "--clearmodifiers", "ctrl+j"]]);
});

test("names a pane or screen the app refuses", () => {
  assert.equal(refusal(64, "libEGL warning: x\ndesktop-gpui: no right panel pane is named agents\n"), 'no screen: desktop-gpui refused --pane agents (exit 64, "no right panel pane is named agents")');
  assert.match(refusal(64, "desktop-gpui: no screen is named starting\n"), /refused --screen starting/);
  assert.match(refusal(1, "thread 'main' panicked\n"), /ended with 1 before its window showed: thread 'main' panicked/);
});

/** A 1280x800 window at scale 1: a flat ground with blocks of ink where `blocks` says, in CSS px. */
function shotOf(blocks, ground = [250, 250, 250]) {
  const [width, height] = [1280, 800];
  const pixels = Buffer.alloc(width * height * 3);
  for (let i = 0; i < width * height; i++) pixels.set(ground, i * 3);
  for (const b of blocks) for (let y = b.y; y < b.y + b.height; y++) for (let x = b.x; x < b.x + b.width; x++) pixels.set([40, 40, 40], (y * width + x) * 3);
  return readShot(encode({ width, height, channels: 3, pixels }, 1));
}

test("a panel with one short line where today's draws a pane fails as bare", () => {
  const nothingOpen = shotOf([{ x: 950, y: 410, width: 120, height: 14 }, { x: 900, y: 432, width: 220, height: 12 }]);
  const agents = shotOf([{ x: 760, y: 70, width: 500, height: 30 }, { x: 760, y: 120, width: 480, height: 200 }]);
  assert.deepEqual(inkBox(nothingOpen, 1, BOXES.panel), { x: 900, y: 410, width: 220, height: 34 });
  assert.equal(isBare(inkBox(nothingOpen, 1, BOXES.panel)), true);
  assert.equal(isBare(inkBox(agents, 1, BOXES.panel)), false);
  assert.deepEqual(bareFailures(agents, nothingOpen, ["panel"], 1), ["bare panel: GPUI draws only 220x34 px of words at 900,410 where today's draws 500x250 px of screen"]);
  assert.deepEqual(bareFailures(nothingOpen, nothingOpen, ["panel"], 1), [], "both empty is not a failure");
  assert.deepEqual(bareFailures(agents, agents, ["panel"], 1), []);
  const pick = shotOf([{ x: 272, y: 72, width: 82, height: 14 }]);
  const home = shotOf([{ x: 300, y: 300, width: 400, height: 200 }]);
  assert.match(bareFailures(home, pick, ["centre"], 1)[0], /^bare centre: GPUI draws only 82x14 px/, "Pick a thread alone is a bare centre");
});

test("reads its flags", () => {
  const base = ["--fixture", "/f.json", "--app", "/a", "--electron-app", "/e", "--out", "/o"];
  assert.equal(parseArgs(base).display, ":97");
  assert.deepEqual(parseArgs([...base, "--theme", "paper"]).themes, ["paper"]);
  assert.equal(parseArgs([...base, "--electron-from", "/prev"]).electronFrom, "/prev");
  assert.equal(parseArgs([...base, "--starting", "/s.json"]).starting, "/s.json");
  assert.equal(parseArgs([...base, "--stood", "/t.json"]).stood, "/t.json");
  assert.throws(() => parseArgs(base.slice(2)), /--fixture is missing/);
  assert.throws(() => parseArgs([...base, "--scale", "2"]), /not a flag/);
  assert.ok(existsSync(SOURCES.panes) && existsSync(SOURCES.address) && existsSync(SOURCES.centre));
});

test("a row stands on reached and surfaces alone, and no field reads as visual parity", () => {
  const equal = { ok: true, rows: [] };
  const differ = { ok: false, rows: [{ region: "composer", same: false }] };
  assert.deepEqual(standingOf({ surface: equal }), { reached: true, notReached: [], surfaces: "equal", status: "reached, surfaces equal" });
  assert.equal(holds(standingOf({ surface: equal })), true);
  assert.equal(holds(standingOf({ surface: differ })), false, "one region off is a row that does not hold, whatever its fuzz");
  assert.equal(standingOf({ surface: differ }).status, "reached, surfaces differ");
  const unshot = standingOf({});
  assert.equal(unshot.surfaces, "not compared");
  assert.equal(holds(unshot), false, "a GPUI shot with no Electron shot to hold it against does not hold");
  const bare = standingOf({ notReached: ["bare centre: GPUI draws only 80x11 px of words"], surface: equal });
  assert.equal(bare.status, "not reached, surfaces equal");
  assert.equal(holds(bare), false);
  for (const s of [standingOf({ surface: equal }), unshot, bare]) assert.ok(!("verdict" in s) && !Object.values(s).includes("pass"));
});

test("finds the long message's thread in the ledger baseline.mjs keeps", () => {
  const home = mkdtempSync(join(tmpdir(), "sweep-test-"));
  try {
    assert.deepEqual(madeThreads(home), {});
    writeFileSync(join(home, "parity-made.json"), JSON.stringify({ [`thread:${LONG_BRIEF}`]: "T-long", second: true }));
    assert.deepEqual(madeThreads(home), { "long-message": "T-long" });
    assert.ok(LONG_BRIEF.length > 600, "long enough that the transcript folds it");
  } finally {
    rmSync(home, { recursive: true, force: true });
  }
});
