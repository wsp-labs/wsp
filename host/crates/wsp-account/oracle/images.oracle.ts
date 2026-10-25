// SPDX-License-Identifier: AGPL-3.0-only
// What keptProjectIcon (runtime/src/projects/images.ts at 65f6b89c7) answers for hand-made PNGs, and how Node decodes
// the base64 a window sends: tests/fixtures/images.json. Run from the repository root:
//   pnpm exec vitest run --config host/crates/wsp-account/oracle/vitest.config.ts --minWorkers=1 --maxWorkers=1
import { createHash } from "node:crypto";
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { crc32, deflateSync } from "node:zlib";
import { it } from "vitest";
import { keptProjectIcon } from "../../../../packages/runtime/src/projects/images.ts";

const chunk = (type: string, data: Buffer): Buffer => {
  const body = Buffer.concat([Buffer.from(type, "latin1"), data]);
  const out = Buffer.alloc(12 + data.length);
  out.writeUInt32BE(data.length, 0);
  body.copy(out, 4);
  out.writeUInt32BE(crc32(body), 8 + data.length);
  return out;
};
const SIG = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);
interface Made { size?: number; width?: number; depth?: number; colour?: number; compression?: number; filter?: number; interlace?: number; before?: Buffer[]; pixels?: Buffer; raw?: Buffer; seed?: number; after?: Buffer[]; noEnd?: boolean }
function png(o: Made = {}): Buffer {
  const size = o.size ?? 128;
  const colour = o.colour ?? 6;
  const channels = colour === 6 ? 4 : colour === 2 ? 3 : 1;
  const head = Buffer.alloc(13);
  head.writeUInt32BE(o.width ?? size, 0);
  head.writeUInt32BE(size, 4);
  head.set([o.depth ?? 8, colour, o.compression ?? 0, o.filter ?? 0, o.interlace ?? 0], 8);
  const rows = Buffer.alloc(size * (1 + size * channels));
  for (let i = 0; i < rows.length; i++) if (i % (1 + size * channels) !== 0) rows[i] = (i * 7 + (o.seed ?? 0)) & 255;
  const idat = o.raw ?? deflateSync(o.pixels ?? rows);
  return Buffer.concat([SIG, chunk("IHDR", head), ...(o.before ?? []), chunk("IDAT", idat), ...(o.noEnd ? [] : [chunk("IEND", Buffer.alloc(0))]), ...(o.after ?? [])]);
}
const answer = (file: Buffer): unknown => {
  try {
    const kept = keptProjectIcon(file);
    return { kept: kept.toString("hex"), sha256: createHash("sha256").update(kept).digest("hex") };
  } catch (e) {
    return { said: (e as Error).message, fix: (e as { fix?: string }).fix, kind: (e as { kind?: string }).kind };
  }
};

it("writes the images fixture", () => {
  const good = png();
  const crcBroken = Buffer.from(good);
  crcBroken[40] = crcBroken[40]! ^ 0xff;
  const twoIdat = (() => {
    const rows = Buffer.alloc(128 * 513);
    for (let i = 0; i < rows.length; i++) if (i % 513 !== 0) rows[i] = (i * 3) & 255;
    const z = deflateSync(rows);
    const half = Math.floor(z.length / 2);
    const head = Buffer.alloc(13);
    head.writeUInt32BE(128, 0);
    head.writeUInt32BE(128, 4);
    head.set([8, 6, 0, 0, 0], 8);
    return Buffer.concat([SIG, chunk("IHDR", head), chunk("IDAT", z.subarray(0, half)), chunk("tIME", Buffer.alloc(7)), chunk("IDAT", z.subarray(half)), chunk("IEND", Buffer.alloc(0))]);
  })();
  const sized = (n: number): Buffer => {
    // A tEXt chunk padding the file to exactly n bytes.
    const base = png().length + 12;
    return png({ before: [chunk("tEXt", Buffer.alloc(n - base, 65))] });
  };
  const cases: [string, Buffer][] = [
    ["good rgba", good],
    ["good rgb", png({ colour: 2 })],
    ["text and phys dropped", png({ before: [chunk("tEXt", Buffer.from("Comment\0taken at the office, 12 Acme Road", "latin1")), chunk("pHYs", Buffer.alloc(9))] })],
    ["two idat chunks around another", twoIdat],
    ["bytes after iend", png({ after: [Buffer.from("trailing junk after the end")] })],
    ["a chunk after iend", png({ after: [chunk("tEXt", Buffer.from("late"))] })],
    ["exactly 81920 bytes", sized(81920)],
    ["81921 bytes", sized(81921)],
    ["over 80 KB", png({ before: [chunk("tEXt", Buffer.alloc(81 * 1024, 65))] })],
    ["not a png", Buffer.from("GIF89a, then something")],
    ["seven bytes", SIG.subarray(0, 7)],
    ["signature alone", SIG],
    ["ends inside a chunk head", Buffer.concat([SIG, Buffer.alloc(5)])],
    ["ends inside a chunk body", good.subarray(0, 40)],
    ["checksum", crcBroken],
    ["checksum before a missing header", Buffer.concat([SIG, (() => { const c = chunk("tEXt", Buffer.from("x")); c[c.length - 1] ^= 1; return c; })()])],
    ["no header first", Buffer.concat([SIG, chunk("tEXt", Buffer.from("x")), good.subarray(8)])],
    ["header of 12 bytes", Buffer.concat([SIG, chunk("IHDR", Buffer.alloc(12)), chunk("IEND", Buffer.alloc(0))])],
    ["no end chunk", png({ noEnd: true })],
    ["64 px", png({ size: 64 })],
    ["256 px", png({ size: 256 })],
    ["128 by 64", png({ size: 64, width: 128 })],
    ["16 bits", png({ depth: 16 })],
    ["16 bits and 64 px", png({ depth: 16, size: 64 })],
    ["palette", png({ colour: 3 })],
    ["grey with alpha", png({ colour: 4 })],
    ["grey", png({ colour: 0 })],
    ["compression 1", png({ compression: 1 })],
    ["filter 1", png({ filter: 1 })],
    ["interlaced", png({ interlace: 1 })],
    ["animated", png({ before: [chunk("acTL", Buffer.alloc(8))] })],
    ["pixels for a smaller image", png({ pixels: Buffer.alloc(64 * 513) })],
    ["pixels one past", png({ pixels: Buffer.alloc(65_665) })],
    ["pixels two past", png({ pixels: Buffer.alloc(65_666) })],
    ["pixels for a bigger image", png({ pixels: Buffer.alloc(129 * 513) })],
    ["a deflate bomb", png({ pixels: Buffer.alloc(64 * 1024 * 1024) })],
    ["pixels that are not deflate", png({ raw: Buffer.from("not deflate") })],
    ["a cut stream", png({ raw: deflateSync(Buffer.alloc(65_664)).subarray(0, 20) })],
    ["no idat at all", Buffer.concat([good.subarray(0, 33), chunk("IEND", Buffer.alloc(0))])],
    ["rgb pixels under an rgba header", png({ pixels: Buffer.alloc(49_280) })],
    ["rgba pixels under an rgb header", png({ colour: 2, pixels: Buffer.alloc(65_664) })],
    ["a filter png lacks", png({ pixels: Buffer.alloc(65_664, 9) })],
    ["filter 4 on every row", png({ pixels: (() => { const b = Buffer.alloc(65_664); for (let r = 0; r < 128; r++) b[r * 513] = 4; return b; })() })],
    ["filter 5 on row 77", png({ pixels: (() => { const b = Buffer.alloc(65_664); b[77 * 513] = 5; return b; })() })],
  ];
  const decoded: [string, string][] = [
    "aGVsbG8gd29ybGQ=",
    "aGVsbG8g\nd29y\r\nbGQ=",
    "aGVs bG8-_w==",
    "aGVsbG8gd29ybGQ=trailing",
    "aGVsbG8gd29ybGQ",
    "a",
    "",
    "!!!aGk=",
  ].map(s => [s, Buffer.from(s, "base64").toString("hex")]);
  const out = fileURLToPath(new URL("../tests/fixtures/", import.meta.url));
  mkdirSync(out, { recursive: true });
  writeFileSync(join(out, "images.json"), `${JSON.stringify({ cases: cases.map(([name, file]) => ({ name, file: file.toString("hex"), ...(answer(file) as object) })), base64: decoded.map(([text, hex]) => ({ text, hex })) }, null, 1)}\n`);
});
