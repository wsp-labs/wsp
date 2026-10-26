// SPDX-License-Identifier: AGPL-3.0-only
// Records what ws's own Receiver does with a corpus of byte sequences, for tests/ws_corpus.rs to replay through
// src/serve/wsframe.rs. Remake the corpus when ws changes:
//
//   node gen.mjs <ws's lib folder, e.g. node_modules/ws/lib> > corpus.json
//
// The corpus is deterministic: a seeded generator, no clock. Each case is a list of chunks (hex) and what a server
// side Receiver (the options serve.ts's WebSocketServer gives it) emitted for them, in order, up to its first error or
// close: {t:"text"|"binary"|"ping"|"pong", hex}, {t:"close", code, hex} (the reason's bytes) or
// {t:"error", code, sentence}.
import { createRequire } from "node:module";
import { readFileSync } from "node:fs";
import { join } from "node:path";

const lib = process.argv[2];
if (!lib) throw new Error("usage: node gen.mjs <ws lib folder>");
const require = createRequire(join(lib, "x.js"));
const Receiver = require(join(lib, "receiver.js"));
const { kStatusCode } = require(join(lib, "constants.js"));
const version = JSON.parse(readFileSync(join(lib, "..", "package.json"), "utf8")).version;

const MAX_PAYLOAD = 100 * 1024 * 1024;
const OPTIONS = {
  isServer: true,
  allowSynchronousEvents: true,
  maxBufferedChunks: 256 * 1024,
  maxFragments: 16 * 1024,
  maxPayload: MAX_PAYLOAD,
  skipUTF8Validation: false,
};

let seed = 0x9e3779b9;
function rand() {
  seed = (seed + 0x6d2b79f5) | 0;
  let t = Math.imul(seed ^ (seed >>> 15), 1 | seed);
  t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
  return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
}
const int = n => Math.floor(rand() * n);
const pick = list => list[int(list.length)];
const bytes = n => Buffer.from(Array.from({ length: n }, () => int(256)));

/** One frame, masked unless `mask` is false; `len` overrides the declared length (a truncated frame). */
function frame({ opcode, fin = true, rsv = 0, payload = Buffer.alloc(0), mask = true, len }) {
  const declared = len ?? payload.length;
  const head = [(fin ? 0x80 : 0) | (rsv << 4) | opcode];
  const m = mask ? 0x80 : 0;
  if (declared < 126) head.push(m | declared);
  else if (declared < 65536) head.push(m | 126, declared >> 8, declared & 255);
  else {
    const b = Buffer.alloc(8);
    b.writeBigUInt64BE(BigInt(declared));
    head.push(m | 127, ...b);
  }
  const parts = [Buffer.from(head)];
  if (mask) {
    const key = bytes(4);
    parts.push(key, Buffer.from(payload.map((x, i) => x ^ key[i & 3])));
  } else parts.push(payload);
  return Buffer.concat(parts);
}

/** A frame whose 64-bit length is written out as given (a BigInt), with no payload. */
function frame64(first, length, mask = true) {
  const b = Buffer.alloc(8);
  b.writeBigUInt64BE(length);
  return Buffer.concat([Buffer.from([first, (mask ? 0x80 : 0) | 127]), b, mask ? bytes(4) : Buffer.alloc(0)]);
}

const cases = [];
function add(name, ...chunks) {
  cases.push({ name, chunks: chunks.map(c => c.toString("hex")) });
}
/** The same bytes cut into chunks at every boundary, and once in one piece. */
function addCut(name, buf) {
  add(`${name} whole`, buf);
  for (let i = 1; i < buf.length; i++) add(`${name} cut ${i}`, buf.subarray(0, i), buf.subarray(i));
}

const UTF8 = [
  "a",
  "héllo",
  "€",
  "日本語",
  "😀",
  "a😀b€c",
  "\u0000\u007f",
  "퟿￿",
  "\u{10ffff}",
];
const BAD_UTF8 = [
  [0xff],
  [0xc0, 0x80],
  [0xc1, 0xbf],
  [0xe0, 0x80, 0x80],
  [0xed, 0xa0, 0x80],
  [0xed, 0xbf, 0xbf],
  [0xf0, 0x80, 0x80, 0x80],
  [0xf4, 0x90, 0x80, 0x80],
  [0xf5, 0x80, 0x80, 0x80],
  [0xe2, 0x82],
  [0x80],
  [0x61, 0xf0, 0x9f, 0x98],
  [0xc2],
];

// Every opcode, FIN, RSV and mask bit with the lengths either side of each width.
for (let opcode = 0; opcode < 16; opcode++) {
  for (const fin of [true, false]) {
    for (const rsv of [0, 1, 2, 3, 4, 7]) {
      for (const mask of [true, false]) {
        for (const n of [0, 1, 2, 125, 126, 127]) {
          if (rsv !== 0 && (opcode > 2 || n > 2)) continue;
          add(`opcode ${opcode} fin ${fin} rsv ${rsv} mask ${mask} len ${n}`, frame({ opcode, fin, rsv, mask, payload: bytes(n) }));
        }
      }
    }
  }
}
for (const n of [65536]) add(`text len ${n}`, frame({ opcode: 1, payload: Buffer.alloc(n, 0x61) }));
for (const n of [65536]) add(`binary len ${n}`, frame({ opcode: 2, payload: bytes(n) }));

// Close frames: the code's edges, the reason's UTF-8, the payload's length.
const CODES = [0, 1, 999, 1000, 1001, 1002, 1003, 1004, 1005, 1006, 1007, 1008, 1009, 1010, 1011, 1012, 1013, 1014, 1015, 1016, 2999, 3000, 3999, 4000, 4401, 4999, 5000, 65535];
for (const code of CODES) {
  const c = Buffer.alloc(2);
  c.writeUInt16BE(code);
  add(`close ${code}`, frame({ opcode: 8, payload: c }));
  add(`close ${code} reason`, frame({ opcode: 8, payload: Buffer.concat([c, Buffer.from("bye")]) }));
  add(`close ${code} reason 123`, frame({ opcode: 8, payload: Buffer.concat([c, Buffer.alloc(123, 0x7a)]) }));
  add(`close ${code} bad reason`, frame({ opcode: 8, payload: Buffer.concat([c, Buffer.from([0xff, 0xfe])]) }));
}
for (const n of [0, 1, 2, 3, 124, 125, 126]) add(`close payload ${n}`, frame({ opcode: 8, payload: Buffer.concat([Buffer.from([0x03, 0xe8]), Buffer.alloc(Math.max(0, n - 2), 0x61)]).subarray(0, n) }));
for (const bad of BAD_UTF8) {
  add(`close reason ${bad.map(b => b.toString(16)).join("")}`, frame({ opcode: 8, payload: Buffer.concat([Buffer.from([0x03, 0xe8]), Buffer.from(bad)]) }));
}
add("close then text", Buffer.concat([frame({ opcode: 8, payload: Buffer.from([0x03, 0xe8]) }), frame({ opcode: 1, payload: Buffer.from("late") })]));
add("close empty then text", Buffer.concat([frame({ opcode: 8 }), frame({ opcode: 1, payload: Buffer.from("late") })]));
add("close then close", Buffer.concat([frame({ opcode: 8 }), frame({ opcode: 8 })]));

// Text: valid strings and invalid bytes, whole, cut into chunks at every byte, and cut into fragments at every byte.
for (const s of UTF8) {
  const buf = Buffer.from(s);
  addCut(`text ${JSON.stringify(s)}`, frame({ opcode: 1, payload: buf }));
  for (let i = 0; i <= buf.length; i++) {
    add(`text ${JSON.stringify(s)} fragments at ${i}`, frame({ opcode: 1, fin: false, payload: buf.subarray(0, i) }), frame({ opcode: 0, payload: buf.subarray(i) }));
    add(`text ${JSON.stringify(s)} three fragments at ${i}`, frame({ opcode: 1, fin: false, payload: buf.subarray(0, i) }), frame({ opcode: 0, fin: false, payload: buf.subarray(i, i + 1) }), frame({ opcode: 0, payload: buf.subarray(i + 1) }));
  }
}
for (const bad of BAD_UTF8) {
  const buf = Buffer.from(bad);
  const name = `bad text ${bad.map(b => b.toString(16)).join("")}`;
  addCut(name, frame({ opcode: 1, payload: buf }));
  for (let i = 0; i <= buf.length; i++) {
    add(`${name} fragments at ${i}`, frame({ opcode: 1, fin: false, payload: buf.subarray(0, i) }), frame({ opcode: 0, payload: buf.subarray(i) }));
  }
  add(`${name} binary`, frame({ opcode: 2, payload: buf }));
}

// Fragmentation orders: valid and not, with control frames between.
const piece = (opcode, fin, text) => frame({ opcode, fin, payload: Buffer.from(text) });
add("text 3 fragments", piece(1, false, "a"), piece(0, false, "b"), piece(0, true, "c"));
add("binary 3 fragments", piece(2, false, "a"), piece(0, false, "b"), piece(0, true, "c"));
add("ping between fragments", piece(1, false, "a"), piece(9, true, "p"), piece(0, true, "b"));
add("pong between fragments", piece(2, false, "a"), piece(10, true, "p"), piece(0, true, "b"));
add("close between fragments", piece(1, false, "a"), frame({ opcode: 8 }), piece(0, true, "b"));
add("empty fragments", piece(1, false, ""), piece(0, false, ""), piece(0, true, ""));
add("continuation first", piece(0, true, "a"));
add("continuation first not final", piece(0, false, "a"));
add("text inside text", piece(1, false, "a"), piece(1, true, "b"));
add("binary inside text", piece(1, false, "a"), piece(2, true, "b"));
add("text after complete", piece(1, true, "a"), piece(0, true, "b"));
add("fragmented ping", piece(9, false, "a"));
add("fragmented close", piece(8, false, ""));
add("two messages in one chunk", Buffer.concat([piece(1, true, "a"), piece(2, true, "b"), piece(9, true, "c")]));
add("message then fault", Buffer.concat([piece(1, true, "a"), frame({ opcode: 3 })]));
add("message then unmasked", Buffer.concat([piece(1, true, "a"), frame({ opcode: 1, mask: false, payload: Buffer.from("b") })]));
for (const n of [16384, 16385]) {
  add(`${n} fragments`, piece(1, false, ""), ...Array.from({ length: n - 1 }, () => piece(0, false, "")), piece(0, true, ""));
}

// Declared lengths past what arrives, and past the ceilings.
const MASKED_FRAME_FIRST = [0x81, 0x82, 0x01, 0x02];
for (const len of [1, 2, 125, 126, 127, 65535, 65536, 1000000, MAX_PAYLOAD - 1, MAX_PAYLOAD, MAX_PAYLOAD + 1, MAX_PAYLOAD * 2]) {
  add(`declared ${len} text, no payload`, frame({ opcode: 1, payload: Buffer.alloc(0), len }).subarray(0, len < 126 ? 2 + 4 : len < 65536 ? 4 + 4 : 10 + 4));
  add(`declared ${len} binary, no payload`, frame({ opcode: 2, payload: Buffer.alloc(0), len }).subarray(0, len < 126 ? 2 + 4 : len < 65536 ? 4 + 4 : 10 + 4));
  add(`declared ${len} text, short payload`, Buffer.concat([frame({ opcode: 1, payload: Buffer.alloc(0), len }), bytes(Math.min(len, 5))]));
  add(`declared ${len} fragment then fragment`, Buffer.concat([frame({ opcode: 1, fin: false, payload: Buffer.alloc(0), len }).subarray(0, 14)]));
}
for (const len of [126, 127]) add(`declared ${len} control`, frame({ opcode: 9, payload: Buffer.alloc(0), len }).subarray(0, 10 + 4));
for (const hi of [0n, 1n, 0x1fffffn, 0x200000n, 0x7fffffffn, 0xffffffffn]) {
  for (const lo of [0n, 1n, 0xffffffffn]) add(`length64 ${hi}:${lo}`, frame64(0x82, (hi << 32n) | lo));
}
add("length64 top bit", frame64(0x82, 0x8000000000000000n));
add("length64 all ones", frame64(0x82, 0xffffffffffffffffn));
add("length64 unmasked", frame64(0x82, 5n, false));
add("two fragments over the ceiling", Buffer.concat([frame({ opcode: 2, fin: false, len: MAX_PAYLOAD - 1 }).subarray(0, 14), Buffer.alloc(0)]));
add("fragments summing over the ceiling", Buffer.concat([frame({ opcode: 2, fin: false, payload: Buffer.alloc(0), len: 60 * 1024 * 1024 }).subarray(0, 14), Buffer.alloc(60 * 1024 * 1024 - 60 * 1024 * 1024)]));

// Random sequences: random frames, random headers, random chunking.
for (let n = 0; n < 1500; n++) {
  const frames = [];
  const count = 1 + int(6);
  for (let i = 0; i < count; i++) {
    const opcode = rand() < 0.8 ? pick([0, 1, 2, 8, 9, 10]) : int(16);
    const fin = rand() < 0.7;
    const rsv = rand() < 0.05 ? 1 + int(7) : 0;
    const mask = rand() < 0.95;
    const size = pick([0, 0, 1, 2, 3, 5, 10, 125, 126, 200]);
    let payload = bytes(size);
    if (opcode === 1 || (opcode === 0 && rand() < 0.5)) payload = Buffer.from(pick(UTF8).repeat(1 + int(3))).subarray(0, rand() < 0.2 ? int(8) : undefined);
    if (opcode === 8 && payload.length >= 2 && rand() < 0.6) payload.writeUInt16BE(pick(CODES));
    const len = rand() < 0.05 ? payload.length + 1 + int(3) : undefined;
    frames.push(frame({ opcode, fin, rsv, mask, payload, len }));
  }
  const all = Buffer.concat(frames);
  const chunks = [];
  let at = 0;
  while (at < all.length) {
    const take = rand() < 0.5 ? 1 + int(4) : 1 + int(all.length);
    chunks.push(all.subarray(at, at + take));
    at += take;
  }
  add(`random ${n}`, ...chunks);
}

// Well formed sequences, so most of them end with messages and not with a fault.
for (let n = 0; n < 800; n++) {
  const frames = [];
  const messages = 1 + int(4);
  for (let m = 0; m < messages; m++) {
    const text = rand() < 0.6;
    const full = text ? Buffer.from(pick(UTF8).repeat(1 + int(4))) : bytes(int(40));
    const parts = 1 + int(4);
    let at = 0;
    for (let p = 0; p < parts; p++) {
      const end = p === parts - 1 ? full.length : Math.min(full.length, at + int(full.length - at + 1));
      frames.push(frame({ opcode: p === 0 ? (text ? 1 : 2) : 0, fin: p === parts - 1, payload: full.subarray(at, end) }));
      at = end;
      if (rand() < 0.2) frames.push(frame({ opcode: pick([9, 10]), payload: bytes(int(6)) }));
    }
  }
  if (rand() < 0.4) frames.push(frame({ opcode: 8, payload: rand() < 0.5 ? Buffer.alloc(0) : Buffer.concat([Buffer.from([0x03, 0xe9]), Buffer.from("going")]) }));
  const all = Buffer.concat(frames);
  const chunks = [];
  let at = 0;
  while (at < all.length) {
    const take = 1 + int(rand() < 0.5 ? 3 : all.length);
    chunks.push(all.subarray(at, at + take));
    at += take;
  }
  add(`sequence ${n}`, ...chunks);
}

/** What ws's Receiver says to these chunks: events in order up to its first error or close. */
async function record(chunks) {
  const events = [];
  const receiver = new Receiver(OPTIONS);
  let stopped = false;
  const data = (t, hex) => events.push({ t, hex });
  receiver.on("message", (buf, isBinary) => data(isBinary ? "binary" : "text", Buffer.from(buf).toString("hex")));
  receiver.on("ping", buf => data("ping", Buffer.from(buf).toString("hex")));
  receiver.on("pong", buf => data("pong", Buffer.from(buf).toString("hex")));
  receiver.on("conclude", (code, buf) => {
    stopped = true;
    events.push({ t: "close", code, hex: Buffer.from(buf).toString("hex") });
  });
  receiver.on("error", err => {
    stopped = true;
    events.push({ t: "error", code: err[kStatusCode], sentence: err.message });
  });
  for (const chunk of chunks) {
    if (stopped) break;
    receiver.write(Buffer.from(chunk, "hex"));
    await new Promise(resolve => setImmediate(resolve));
  }
  return events;
}

for (const c of cases) c.events = await record(c.chunks);
process.stdout.write(JSON.stringify({ ws: version, options: OPTIONS, cases }) + "\n");
