// SPDX-License-Identifier: AGPL-3.0-only
// git's smart HTTP push on the wire: the receive-pack advertisement, the pkt-lines a push sends before its pack, and the
// report-status answer, so the road can judge each command and merge its refusals into the upstream's report.
// gitprotocol-pack: a push may open with `shallow <oid>` lines, its first command carries the capabilities, push
// options follow the first flush, and a side-band answer carries the report on band 1 and `remote:` text on band 2.
import type { RefCommand } from "./branch.js";

export const FLUSH = Buffer.from("0000");
/** The most a push's lines before its pack may hold; a ref list a push names is far smaller. */
const HEAD_CAP = 8 * 1024 * 1024;
const SIDE_BAND_64K_DATA = 65515;
const SIDE_BAND_DATA = 995;

export function pkt(payload: string | Buffer): Buffer {
  const body = typeof payload === "string" ? Buffer.from(payload) : payload;
  return Buffer.concat([Buffer.from((body.length + 4).toString(16).padStart(4, "0")), body]);
}

/** Reads pkt-lines off a byte stream, then hands back whatever follows them untouched. */
export class PktReader {
  private buf = Buffer.alloc(0);
  private read = 0;
  private done = false;
  private readonly source: AsyncIterator<Buffer>;

  constructor(source: AsyncIterable<Buffer>) {
    this.source = source[Symbol.asyncIterator]();
  }

  private async fill(n: number): Promise<boolean> {
    while (this.buf.length < n && !this.done) {
      const next = await this.source.next();
      if (next.done) this.done = true;
      else this.buf = Buffer.concat([this.buf, next.value]);
    }
    return this.buf.length >= n;
  }

  /** The next line's payload, null at a flush, undefined when the stream ends. */
  async line(): Promise<Buffer | null | undefined> {
    if (!(await this.fill(4))) return undefined;
    const size = Number.parseInt(this.buf.subarray(0, 4).toString("latin1"), 16);
    if (Number.isNaN(size) || (size > 0 && size < 4)) throw new Error("not a pkt-line");
    this.read += Math.max(size, 4);
    if (this.read > HEAD_CAP) throw new Error("pkt-lines past the cap");
    if (size === 0) {
      this.buf = this.buf.subarray(4);
      return null;
    }
    if (!(await this.fill(size))) throw new Error("pkt-line cut short");
    const payload = this.buf.subarray(4, size);
    this.buf = this.buf.subarray(size);
    return payload;
  }

  /** Everything after the lines read, the bytes already buffered first. */
  async *rest(): AsyncGenerator<Buffer> {
    if (this.buf.length > 0) yield this.buf;
    this.buf = Buffer.alloc(0);
    while (!this.done) {
      const next = await this.source.next();
      if (next.done) this.done = true;
      else yield next.value;
    }
  }
}

/** One command as it came, so a forwarded command keeps its bytes. */
export interface PushCommand extends RefCommand {
  line: Buffer;
}

export interface PushHead {
  shallow: Buffer[];
  commands: PushCommand[];
  caps: string;
  options: Buffer[];
  /** The push sent a certificate, which hides its commands inside it. */
  certified: boolean;
}

/** The lines of a receive-pack request before its pack. A probe (`0000` alone) reads as no commands. */
export async function readPushHead(reader: PktReader): Promise<PushHead> {
  const head: PushHead = { shallow: [], commands: [], caps: "", options: [], certified: false };
  for (;;) {
    const line = await reader.line();
    if (line === undefined || line === null) break;
    const text = line.toString("latin1");
    if (text.startsWith("shallow ")) {
      head.shallow.push(line);
      continue;
    }
    if (text.startsWith("push-cert")) {
      head.certified = true;
      return head;
    }
    const nul = line.indexOf(0);
    const bytes = nul === -1 ? line : line.subarray(0, nul);
    const [old, next, ref] = bytes.toString("utf8").replace(/\n$/, "").split(" ");
    if (!old || !next || !ref) throw new Error("not a push command");
    if (head.commands.length === 0 && nul !== -1) head.caps = line.subarray(nul + 1).toString("latin1").replace(/\n$/, "");
    head.commands.push({ old, new: next, ref, line: bytes.at(-1) === 0x0a ? bytes.subarray(0, -1) : bytes });
  }
  if (head.commands.length > 0 && capList(head.caps).includes("push-options")) {
    for (let line = await reader.line(); line; line = await reader.line()) head.options.push(line);
  }
  return head;
}

export const capList = (caps: string): string[] => caps.split(" ").filter(Boolean);

/** The lines to send upstream: the shallow lines, the kept commands (the first carrying the push's capabilities), the options. */
export function encodePushHead(head: PushHead, kept: PushCommand[]): Buffer {
  const parts = head.shallow.map((line) => pkt(line));
  kept.forEach((command, i) => parts.push(pkt(i === 0 ? Buffer.concat([command.line, Buffer.from([0]), Buffer.from(head.caps, "latin1")]) : command.line)));
  parts.push(FLUSH);
  if (capList(head.caps).includes("push-options")) parts.push(...head.options.map((line) => pkt(line)), FLUSH);
  return Buffer.concat(parts);
}

/** The receive-pack advertisement's capabilities as the road serves them: no thin packs, so a pack indexes alone, and no push certificates. */
export function servedCaps(caps: string): string {
  const kept = capList(caps).filter((cap) => cap !== "push-cert" && !cap.startsWith("push-cert="));
  if (!kept.includes("no-thin")) kept.push("no-thin");
  return kept.join(" ");
}

/**
 * Rewrites the capabilities on an advertisement's first ref line. Answers undefined until `head` holds that line, and
 * the bytes to send in its place once it does.
 */
export function rewriteAdvertisement(head: Buffer): Buffer | undefined {
  let at = 0;
  for (;;) {
    if (head.length < at + 4) return undefined;
    const size = Number.parseInt(head.subarray(at, at + 4).toString("latin1"), 16);
    if (Number.isNaN(size)) throw new Error("not a pkt-line");
    if (size === 0) {
      at += 4;
      continue;
    }
    if (head.length < at + size) return undefined;
    const payload = head.subarray(at + 4, at + size);
    const nul = payload.indexOf(0);
    if (payload.toString("latin1").startsWith("# service=") || nul === -1) {
      at += size;
      continue;
    }
    const caps = payload.subarray(nul + 1).toString("latin1");
    const newline = caps.endsWith("\n") ? "\n" : "";
    const line = Buffer.concat([payload.subarray(0, nul + 1), Buffer.from(servedCaps(caps.replace(/\n$/, "")) + newline, "latin1")]);
    return Buffer.concat([head.subarray(0, at), pkt(line), head.subarray(at + size)]);
  }
}

export type SideBand = "side-band-64k" | "side-band" | undefined;

export function sideBandOf(caps: string): SideBand {
  const list = capList(caps);
  if (list.includes("side-band-64k")) return "side-band-64k";
  if (list.includes("side-band")) return "side-band";
  return undefined;
}

export interface Refused {
  ref: string;
  said: string;
}

const ngLines = (refused: Refused[]): Buffer[] => refused.map((r) => pkt(`ng ${r.ref} ${r.said.replace(/\n/g, " ")}\n`));

function banded(band: number, data: Buffer, sideBand: SideBand): Buffer[] {
  const max = sideBand === "side-band-64k" ? SIDE_BAND_64K_DATA : SIDE_BAND_DATA;
  const out: Buffer[] = [];
  for (let at = 0; at < data.length; at += max) out.push(pkt(Buffer.concat([Buffer.from([band]), data.subarray(at, at + max)])));
  return out;
}

const sentences = (refused: Refused[]): string[] => [...new Set(refused.map((r) => r.said))];

/** The road's own answer to a push it forwarded none of. */
export function refusalReport(refused: Refused[], sideBand: SideBand): Buffer {
  const report = Buffer.concat([pkt("unpack ok\n"), ...ngLines(refused), FLUSH]);
  if (!sideBand) return report;
  return Buffer.concat([...sentences(refused).flatMap((s) => banded(2, Buffer.from(`${s}\n`), sideBand)), ...banded(1, report, sideBand), FLUSH]);
}

/** Splits a buffer of pkt-lines into payloads, null for each flush; a trailing partial line is dropped. */
function payloads(data: Buffer): (Buffer | null)[] {
  const out: (Buffer | null)[] = [];
  let at = 0;
  while (at + 4 <= data.length) {
    const size = Number.parseInt(data.subarray(at, at + 4).toString("latin1"), 16);
    if (Number.isNaN(size) || (size > 0 && size < 4) || at + Math.max(size, 4) > data.length) break;
    out.push(size === 0 ? null : data.subarray(at + 4, at + size));
    at += Math.max(size, 4);
  }
  return out;
}

/** The upstream's report-status with an `ng` line for each refused command and its sentence on band 2. */
export function mergeReport(upstream: Buffer, refused: Refused[], sideBand: SideBand): Buffer {
  if (refused.length === 0) return upstream;
  const withNg = (report: Buffer): Buffer => {
    const lines = payloads(report);
    const flush = lines.indexOf(null);
    const kept = (flush === -1 ? lines : lines.slice(0, flush)).filter((l): l is Buffer => l !== null).map((l) => pkt(l));
    return Buffer.concat([...kept, ...ngLines(refused), FLUSH]);
  };
  if (!sideBand) return withNg(upstream);
  const report: Buffer[] = [];
  const other: Buffer[] = [];
  for (const payload of payloads(upstream)) {
    if (payload === null) break;
    if (payload[0] === 1) report.push(payload.subarray(1));
    else other.push(pkt(payload));
  }
  const notes = sentences(refused).flatMap((s) => banded(2, Buffer.from(`${s}\n`), sideBand));
  return Buffer.concat([...notes, ...other, ...banded(1, withNg(Buffer.concat(report)), sideBand), FLUSH]);
}
