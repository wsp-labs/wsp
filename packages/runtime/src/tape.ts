// SPDX-License-Identifier: AGPL-3.0-only
import { appendFileSync, mkdirSync } from "node:fs";
import { join } from "node:path";
import { redacted, requestSecrets, SCANNED_SECRETS, SECRET_REQUEST_FIELDS } from "@wsp/protocol";

/** The folder every door of this process writes its tapes into; unset, nothing is written. */
export const TAPE_DIR_ENV = "WSP_TAPE_DIR";
/** What the frames on a tape were recorded from, written on its first line: a test's name, or a day of the host. */
export const TAPE_SOURCE_ENV = "WSP_TAPE_SOURCE";
/** The host's own token on a tape, which a replay sends as the token of the host it replays against. Any other secret
 * is `<redacted>` and stays that on a replay, so a frame that carried a wrong token is refused there too. */
export const HOST_TOKEN_MARK = "<host-token>";
const SECRET_MARK = "<redacted>";

/** The ways into a host a tape records: its websocket, the tool server on stdio, and the command line's words. */
export type TapeDoor = "ws" | "mcp" | "cli";

/** One door's frames, in the order they crossed it, each line `{"in": frame}` or `{"out": frame}`. */
export interface Tape {
  in(frame: unknown): void;
  out(frame: unknown): void;
}

let opened = 0;

/** A tape for one connection through `door`, or none when the environment names no folder. Every secret a frame in
 * either direction carries is blanked on that line and every line after it, since a token sent in a request comes back
 * in the answers that echo it; a secret field is blanked whatever its length, which `redacted()` leaves alone under
 * four characters. */
export function tapeOf(env: Readonly<Record<string, string | undefined>>, door: TapeDoor, hostToken?: string): Tape | undefined {
  const dir = env[TAPE_DIR_ENV];
  if (dir === undefined || dir === "") return undefined;
  try {
    mkdirSync(dir, { recursive: true });
  } catch {
    return undefined;
  }
  const path = join(dir, `${door}-${process.pid}-${++opened}.tape`);
  const secrets = new Set<string>();
  // Around the marks already written, since a host token can be the text of a mark ("host-token" is one).
  const marked = (text: string): string =>
    hostToken === undefined || hostToken.trim().length < 4
      ? text
      : text
          .split(HOST_TOKEN_MARK)
          .map(piece => piece.split(hostToken).join(HOST_TOKEN_MARK))
          .join(HOST_TOKEN_MARK);
  // A tape is a witness: a frame it cannot write, one nested past what JSON.stringify walks or a full disk, costs
  // the tape that line and never changes what the host answers.
  const write = (line: Record<string, unknown>): void => {
    try {
      appendFileSync(path, `${redacted(marked(JSON.stringify(line)), [...secrets])}\n`);
    } catch (e) {
      try {
        appendFileSync(path, `${JSON.stringify({ [Object.keys(line)[0]!]: `<not written: ${e instanceof Error ? e.name : "error"}>` })}\n`);
      } catch {
        // The disk took neither line; the host goes on answering.
      }
    }
  };
  write({ meta: { door, source: env[TAPE_SOURCE_ENV] ?? "host" } });
  const side =
    (dir: "in" | "out") =>
    (frame: unknown): void => {
      const parsed = typeof frame === "string" ? parseOr(frame) : frame;
      const carried = requestSecrets(parsed);
      if (carried.length > SCANNED_SECRETS) return write({ [dir]: `<withheld: ${carried.length} secret values>` });
      for (const s of carried) if (s !== hostToken) secrets.add(s);
      write({ [dir]: blanked(parsed, hostToken) });
    };
  return { in: side("in"), out: side("out") };
}

/** The frame with each secret field's value marked, read one level into a record as `requestSecrets` reads it. */
function blanked(frame: unknown, hostToken: string | undefined): unknown {
  if (typeof frame !== "object" || frame === null || Array.isArray(frame)) return frame;
  const mark = (v: unknown): unknown => (typeof v !== "string" ? v : v === hostToken ? HOST_TOKEN_MARK : SECRET_MARK);
  const out: Record<string, unknown> = { ...(frame as Record<string, unknown>) };
  for (const field of SECRET_REQUEST_FIELDS) {
    const v = out[field];
    if (typeof v === "string") out[field] = mark(v);
    else if (typeof v === "object" && v !== null && !Array.isArray(v)) out[field] = Object.fromEntries(Object.entries(v).map(([k, x]) => [k, mark(x)]));
  }
  return out;
}

function parseOr(text: string): unknown {
  try {
    return JSON.parse(text);
  } catch {
    return text;
  }
}
