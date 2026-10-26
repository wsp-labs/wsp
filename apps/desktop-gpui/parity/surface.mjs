// SPDX-License-Identifier: AGPL-3.0-only
// The surface check beside the fuzz score: for a whole-window pair, the flat ground of each region (sidebar, its header,
// thread pane, its header, right panel, its header, composer) is sampled in both shots and any difference fails. Text
// keeps its fuzz in the pixel compare; a surface gets none.
//   node surface.mjs <electron.png> <gpui.png> [--regions <json>] [--scale <n>]
// Exits 1 on any difference, printing each region and both colours; 2 on a shot it cannot read.
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { decode } from "./png.mjs";

export const USAGE = "usage: node surface.mjs <electron.png> <gpui.png> [--regions <json>] [--scale <n>]";
/** The window both shots are of, in CSS px; the scale is read off the shot's width against it. */
export const WINDOW = { width: 1280, height: 800 };
/** The side of the square patch averaged at each point, in CSS px: the web paints a noise tile over the sidebar and the
 * thread column (apps/web/src/index.css:855), about one level a pixel, which a single pixel would read as a difference. */
export const PATCH = 9;
/** How far apart two patch means may be, per channel, after rounding. */
export const TOLERANCE = 0;
/** A point of flat ground in each region at 1280x800 with a thread open and the panel's launcher showing, in CSS px. */
export const REGIONS = {
  "sidebar header": { x: 200, y: 26 },
  sidebar: { x: 200, y: 785 },
  "thread header": { x: 730, y: 8 },
  "thread pane": { x: 262, y: 600 },
  "panel header": { x: 1000, y: 6 },
  panel: { x: 752, y: 785 },
  composer: { x: 498, y: 738 },
};

/** The default regions with a state's own: a point moves a region, null drops it (a panel that is shut has no ground). */
export function regionsFor(overrides = {}) {
  const out = { ...REGIONS };
  for (const [name, point] of Object.entries(overrides)) {
    if (point === null) delete out[name];
    else if (Number.isFinite(point?.x) && Number.isFinite(point?.y)) out[name] = { x: point.x, y: point.y };
    else throw new Error(`the region ${name} takes { x, y } in CSS px or null, not ${JSON.stringify(point)}`);
  }
  return out;
}

/** A shot decoded, or a sentence naming the file and what to save it as instead. */
export function readShot(png, name = "the shot") {
  try {
    return decode(png);
  } catch (e) {
    throw new Error(`${name}: ${e.message}. Save a shot as 8-bit RGB (ImageMagick: convert <in> PNG24:<out>), since ImageMagick writes a flat image as greyscale or a palette`);
  }
}

/** The rounded mean colour of the `size` CSS px square centred on `point`, at `scale` device pixels a CSS px. */
export function patchMean(image, scale, point, size = PATCH) {
  const half = (size * scale) / 2;
  const [x0, y0] = [Math.round(point.x * scale - half), Math.round(point.y * scale - half)];
  const [x1, y1] = [x0 + Math.round(size * scale), y0 + Math.round(size * scale)];
  if (x0 < 0 || y0 < 0 || x1 > image.width || y1 > image.height) throw new Error(`a ${size} px patch at ${point.x},${point.y} falls off a ${image.width}x${image.height} shot at scale ${scale}`);
  const sum = [0, 0, 0];
  for (let y = y0; y < y1; y++)
    for (let x = x0; x < x1; x++) {
      const at = (y * image.width + x) * image.channels;
      for (let c = 0; c < 3; c++) sum[c] += image.pixels[at + c];
    }
  const n = (x1 - x0) * (y1 - y0);
  return sum.map(s => Math.round(s / n));
}

export const hex = rgb => `#${rgb.map(v => v.toString(16).padStart(2, "0")).join("")}`;

/** Each region's ground in both shots, and whether they match: `{ ok, scale, rows: [{ region, point, electron, gpui, same }] }`. */
export function compareSurfaces(electron, gpui, { regions = REGIONS, scale } = {}) {
  if (electron.width !== gpui.width || electron.height !== gpui.height) throw new Error(`the shots differ in size: ${electron.width}x${electron.height} against ${gpui.width}x${gpui.height}`);
  const s = scale ?? electron.width / WINDOW.width;
  const rows = Object.entries(regions).map(([region, point]) => {
    const [a, b] = [patchMean(electron, s, point), patchMean(gpui, s, point)];
    return { region, point, electron: hex(a), gpui: hex(b), same: a.every((v, c) => Math.abs(v - b[c]) <= TOLERANCE) };
  });
  return { ok: rows.every(r => r.same), scale: s, rows };
}

/** The check on two PNG files' bytes. */
export const surfaceCheck = (electronPng, gpuiPng, options = {}) =>
  compareSurfaces(readShot(electronPng, options.names?.[0] ?? "the Electron shot"), readShot(gpuiPng, options.names?.[1] ?? "the GPUI shot"), options);

/** One line per region, the differing ones first. */
export const describe = result =>
  [...result.rows]
    .sort((a, b) => Number(a.same) - Number(b.same))
    .map(r => `${r.same ? "same" : "DIFFERS"} ${r.region} at ${r.point.x},${r.point.y}: Electron ${r.electron}, GPUI ${r.gpui}`);

export function parseArgs(argv) {
  const files = [];
  let regions = {};
  let scale;
  for (let i = 0; i < argv.length; i++) {
    if (argv[i] === "--regions") {
      if (argv[i + 1] === undefined) throw new Error(`${USAGE}; --regions takes a value`);
      regions = JSON.parse(argv[++i]);
    } else if (argv[i] === "--scale") {
      scale = Number(argv[++i]);
      if (!(scale > 0)) throw new Error(`--scale takes a number above 0, not ${argv[i]}`);
    } else if (argv[i].startsWith("--")) throw new Error(`${USAGE}; ${argv[i]} is not a flag it takes`);
    else files.push(argv[i]);
  }
  if (files.length !== 2) throw new Error(`${USAGE}; it takes two shots, not ${files.length}`);
  return { electron: resolve(files[0]), gpui: resolve(files[1]), regions: regionsFor(regions), scale };
}

function main() {
  let args;
  let result;
  try {
    args = parseArgs(process.argv.slice(2));
    result = surfaceCheck(readFileSync(args.electron), readFileSync(args.gpui), { regions: args.regions, scale: args.scale, names: [args.electron, args.gpui] });
  } catch (e) {
    console.error(`surface: ${e.message}`);
    process.exit(2);
  }
  for (const line of describe(result)) console.log(`surface: ${line}`);
  console.log(`surface: ${result.ok ? "every region's ground matches" : `${result.rows.filter(r => !r.same).length} of ${result.rows.length} regions differ`}`);
  process.exit(result.ok ? 0 : 1);
}

if (process.argv[1] !== undefined && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) main();
