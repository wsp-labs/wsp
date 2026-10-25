// SPDX-License-Identifier: AGPL-3.0-only
import { appendFileSync, mkdirSync } from "node:fs";
import { join } from "node:path";
import { requestSecrets, SCANNED_SECRETS, SECRET_REQUEST_FIELDS } from "@wsp/protocol";

/** The folder every door of this process writes its tapes into; unset, nothing is written. */
export const TAPE_DIR_ENV = "WSP_TAPE_DIR";
/** What the frames on a tape were recorded from, written on its first line: a test's name, or a day of the host. */
export const TAPE_SOURCE_ENV = "WSP_TAPE_SOURCE";
/** The host's own token on a tape, which a replay sends as the token of the host it replays against. Any other secret
 * is `<redacted-N>`, numbered by first sight on its tape: one the tape's recorded answers handed out (a paired device's
 * token) a replay swaps for the one its own host hands out, and any other stays as written, so a frame that carried a
 * wrong token is refused there too. */
export const HOST_TOKEN_MARK = "<host-token>";
/** The shortest host token marked wherever it appears; a host mints 32 hex characters. */
const MINTED_TOKEN_MIN = 16;
/** The shortest secret also blanked in free text: a shorter one is a case's stand-in, which prose holds as a word. */
const FREE_TEXT_MIN = 4;
/** Fields whose value is a secret wherever in a frame they sit: a paired device's bearer nests in place.prove's
 * answer as `device.deviceToken`. */
const DEEP_SECRET_FIELDS = new Set(["token", "turnToken", "ticket", "deviceToken", "passphrase", "sudoPassword"]);
/** A command line's flags whose next word is a secret, as `wsp join <url> --code <code>` takes one. */
const SECRET_FLAG = /^--(?:code|token|key|password|passphrase|secret)$/;
/** Text that carries a secret whatever field holds it: a flag's value in a command written out, a pairing or join code
 * with the fingerprint after it (standing alone, so never two groups of a uuid), a bearer in a URL's query, NAME=value for a secret's name, a Bearer header, and the
 * prefixes providers mint keys with. The second group, where there is one, is the secret. */
const SECRET_SHAPES: readonly RegExp[] = [
  /(--(?:code|token|key|password|passphrase|secret)(?:=|\s+))([^\s"'\\]+)/g,
  /()(?<![0-9A-Za-z-])[0-9A-Z]{4}-[0-9A-Z]{4}(?:\.SHA256:[A-Za-z0-9+/=]+)?(?![0-9A-Za-z-])/g,
  /([?&](?:[a-z_]*token|key|secret|code|sig|auth)=)([^&\s"'\\]+)/gi,
  /\b([A-Z][A-Z0-9_]*(?:TOKEN|SECRET|PASSWORD|API_KEY|_KEY)=)([^\s"'\\]+)/g,
  /(\bBearer\s+)([^\s"'\\]+)/g,
  /()\b(?:sk-ant-|sk-|ghp_|gho_|ghu_|ghs_|github_pat_|xox[abpr]-|AKIA)[A-Za-z0-9_-]{8,}/g,
];

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
  const secrets = new Map<string, string>();
  const mark = (v: string): string => {
    if (v === hostToken) return HOST_TOKEN_MARK;
    let m = secrets.get(v);
    if (m === undefined) secrets.set(v, (m = `<redacted-${secrets.size + 1}>`));
    return m;
  };
  // Around the marks already written, since a host token can be the text of a mark ("host-token" is one). In free
  // text only a token as long as a minted one: a case's stand-in token is a word ("secret") that prose holds too.
  const marked = (text: string): string =>
    hostToken === undefined || hostToken.trim().length < MINTED_TOKEN_MIN
      ? text
      : text
          .split(HOST_TOKEN_MARK)
          .map(piece => piece.split(hostToken).join(HOST_TOKEN_MARK))
          .join(HOST_TOKEN_MARK);
  // Every secret seen so far, blanked wherever it appears again: a token sent in a request comes back in the answers
  // that echo it. Longest first, so one secret holding another is blanked whole.
  const scrubbed = (text: string): string =>
    [...secrets]
      .filter(([v]) => v.trim().length >= FREE_TEXT_MIN)
      .sort(([a], [b]) => b.length - a.length)
      .reduce((t, [v, m]) => t.split(JSON.stringify(v).slice(1, -1)).join(m), text);
  // A tape is a witness: a frame it cannot write, one nested past what JSON.stringify walks or a full disk, costs
  // the tape that line and never changes what the host answers.
  const write = (line: Record<string, unknown>): void => {
    try {
      appendFileSync(path, `${scrubbed(marked(JSON.stringify(line)))}\n`);
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
      try {
        write({ [dir]: blanked(parsed, mark, 0) });
      } catch (e) {
        write({ [dir]: `<not written: ${e instanceof Error ? e.name : "error"}>` });
      }
    };
  return { in: side("in"), out: side("out") };
}

/** The deepest a frame is walked; past it a value is written as it is and the free-text pass still blanks what it knows. */
const DEPTH_MAX = 64;

/** The frame with every secret marked: a secret field's value one level into a record as `requestSecrets` reads it,
 * the fields that are a secret anywhere, a flag's next word, and every secret-shaped run of text. */
function blanked(frame: unknown, mark: (v: string) => string, depth: number): unknown {
  if (depth > DEPTH_MAX) return frame;
  if (typeof frame === "string") return shaped(frame, mark);
  if (Array.isArray(frame)) {
    return frame.map((v, i) => (typeof v === "string" && i > 0 && typeof frame[i - 1] === "string" && SECRET_FLAG.test(frame[i - 1] as string) ? mark(v) : blanked(v, mark, depth + 1)));
  }
  if (typeof frame !== "object" || frame === null) return frame;
  const out: Record<string, unknown> = {};
  for (const [k, v] of Object.entries(frame as Record<string, unknown>)) {
    const field = depth === 0 && SECRET_REQUEST_FIELDS.includes(k);
    if (typeof v === "string" && (field || DEEP_SECRET_FIELDS.has(k))) out[k] = mark(v);
    else if (field && typeof v === "object" && v !== null && !Array.isArray(v)) out[k] = Object.fromEntries(Object.entries(v).map(([n, x]) => [n, typeof x === "string" ? mark(x) : x]));
    else out[k] = blanked(v, mark, depth + 1);
  }
  return out;
}

/** A run of text with each secret-shaped piece of it marked. */
function shaped(text: string, mark: (v: string) => string): string {
  return SECRET_SHAPES.reduce((t, shape) => t.replace(shape, (whole: string, before: string, secret?: string) => (typeof secret === "string" ? `${before}${mark(secret)}` : mark(whole))), text);
}

function parseOr(text: string): unknown {
  try {
    return JSON.parse(text);
  } catch {
    return text;
  }
}
