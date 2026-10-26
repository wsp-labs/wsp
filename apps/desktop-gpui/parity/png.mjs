// SPDX-License-Identifier: AGPL-3.0-only
// The parity driver's PNG work, kept apart so a test can load it without launching anything: a whole-window capture
// is cut to a state's clip here rather than by Page.captureScreenshot's clip, which moves the page's pointer to the
// window's centre before it paints and runs the capture through a viewport override.
import { crc32, deflateSync, inflateSync } from "node:zlib";

const SIGNATURE = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);
/** Bytes per pixel of the two colour types a capture comes in: 2 is RGB, 6 is RGBA, both 8 bits a channel. */
const CHANNELS = { 2: 3, 6: 4 };

function* chunksOf(png) {
  if (!png.subarray(0, 8).equals(SIGNATURE)) throw new Error("not a PNG");
  for (let at = 8; at < png.length; ) {
    const length = png.readUInt32BE(at);
    yield { type: png.toString("latin1", at + 4, at + 8), data: png.subarray(at + 8, at + 8 + length), whole: png.subarray(at, at + 12 + length) };
    at += 12 + length;
  }
}

const chunk = (type, data) => {
  const body = Buffer.concat([Buffer.from(type, "latin1"), data]);
  const out = Buffer.alloc(body.length + 8);
  out.writeUInt32BE(data.length, 0);
  body.copy(out, 4);
  out.writeUInt32BE(crc32(body), body.length + 4);
  return out;
};

/** The display scale as pixels per metre, so a 2x image reads as 144 dpi. */
const physOf = scale => {
  const ppm = Math.round((scale * 72) / 0.0254);
  const data = Buffer.alloc(9);
  data.writeUInt32BE(ppm, 0);
  data.writeUInt32BE(ppm, 4);
  data[8] = 1;
  return chunk("pHYs", data);
};

const paeth = (a, b, c) => {
  const p = a + b - c;
  const pa = Math.abs(p - a);
  const pb = Math.abs(p - b);
  const pc = Math.abs(p - c);
  return pa <= pb && pa <= pc ? a : pb <= pc ? b : c;
};

/** An 8-bit RGB or RGBA PNG with no interlace as { width, height, channels, pixels }, its rows unfiltered. */
export function decode(png) {
  let head;
  const idat = [];
  for (const { type, data } of chunksOf(png)) {
    if (type === "IHDR") head = { width: data.readUInt32BE(0), height: data.readUInt32BE(4), depth: data[8], colour: data[9], interlace: data[12] };
    else if (type === "IDAT") idat.push(data);
  }
  if (head === undefined) throw new Error("a PNG with no IHDR");
  const channels = CHANNELS[head.colour];
  if (head.depth !== 8 || channels === undefined || head.interlace !== 0) throw new Error(`a PNG of depth ${head.depth}, colour type ${head.colour}, interlace ${head.interlace}; only 8-bit RGB or RGBA without interlace is read`);
  const { width, height } = head;
  const stride = width * channels;
  const raw = inflateSync(Buffer.concat(idat));
  const pixels = Buffer.alloc(stride * height);
  for (let y = 0; y < height; y++) {
    const filter = raw[y * (stride + 1)];
    const line = raw.subarray(y * (stride + 1) + 1, (y + 1) * (stride + 1));
    const row = y * stride;
    const up = row - stride;
    for (let i = 0; i < stride; i++) {
      const a = i >= channels ? pixels[row + i - channels] : 0;
      const b = y > 0 ? pixels[up + i] : 0;
      const c = y > 0 && i >= channels ? pixels[up + i - channels] : 0;
      const v = line[i];
      pixels[row + i] = filter === 0 ? v : filter === 1 ? v + a : filter === 2 ? v + b : filter === 3 ? v + ((a + b) >> 1) : filter === 4 ? v + paeth(a, b, c) : -1;
    }
    if (filter > 4) throw new Error(`row ${y} has filter ${filter}`);
  }
  return { width, height, channels, pixels };
}

/** The image as a PNG: each row with the filter whose bytes, read as signed, sum smallest (libpng's choice), deflated
 * at level 9, with the scale in a pHYs chunk. */
export function encode({ width, height, channels, pixels }, scale) {
  const stride = width * channels;
  const out = Buffer.alloc((stride + 1) * height);
  const tries = Array.from({ length: 5 }, () => Buffer.alloc(stride));
  for (let y = 0; y < height; y++) {
    const row = y * stride;
    const up = row - stride;
    const sums = [0, 0, 0, 0, 0];
    const [none, sub, upward, average, nearest] = tries;
    const signed = v => (v < 128 ? v : 256 - v);
    for (let i = 0; i < stride; i++) {
      const x = pixels[row + i];
      const a = i >= channels ? pixels[row + i - channels] : 0;
      const b = y > 0 ? pixels[up + i] : 0;
      const c = y > 0 && i >= channels ? pixels[up + i - channels] : 0;
      none[i] = x;
      sums[0] += signed(x);
      sums[1] += signed((sub[i] = (x - a) & 0xff));
      sums[2] += signed((upward[i] = (x - b) & 0xff));
      sums[3] += signed((average[i] = (x - ((a + b) >> 1)) & 0xff));
      sums[4] += signed((nearest[i] = (x - paeth(a, b, c)) & 0xff));
    }
    const best = sums.indexOf(Math.min(...sums));
    out[y * (stride + 1)] = best;
    tries[best].copy(out, y * (stride + 1) + 1);
  }
  const head = Buffer.alloc(13);
  head.writeUInt32BE(width, 0);
  head.writeUInt32BE(height, 4);
  head.set([8, channels === 4 ? 6 : 2, 0, 0, 0], 8);
  return Buffer.concat([SIGNATURE, chunk("IHDR", head), physOf(scale), chunk("IDAT", deflateSync(out, { level: 9 })), chunk("IEND", Buffer.alloc(0))]);
}

/** The PNG cut to `rect` (CSS px) at `scale` device pixels per CSS pixel, clamped to the image; and its size. */
export function crop(png, rect, scale) {
  const image = decode(png);
  const x = Math.max(0, Math.round(rect.x * scale));
  const y = Math.max(0, Math.round(rect.y * scale));
  const width = Math.min(image.width, Math.round((rect.x + rect.width) * scale)) - x;
  const height = Math.min(image.height, Math.round((rect.y + rect.height) * scale)) - y;
  if (width <= 0 || height <= 0) throw new Error(`the clip ${JSON.stringify(rect)} holds nothing of a ${image.width}x${image.height} image at scale ${scale}`);
  const stride = image.width * image.channels;
  const pixels = Buffer.alloc(width * height * image.channels);
  for (let r = 0; r < height; r++) image.pixels.copy(pixels, r * width * image.channels, (y + r) * stride + x * image.channels, (y + r) * stride + (x + width) * image.channels);
  return { png: encode({ width, height, channels: image.channels, pixels }, scale), width, height };
}

/** The PNG as it came, with the scale in a pHYs chunk after IHDR in place of any it had; and its size. */
export function withPhys(png, scale) {
  const parts = [SIGNATURE];
  for (const { type, whole } of chunksOf(png)) {
    if (type !== "pHYs") parts.push(whole);
    if (type === "IHDR") parts.push(physOf(scale));
  }
  return { png: Buffer.concat(parts), width: png.readUInt32BE(16), height: png.readUInt32BE(20) };
}
