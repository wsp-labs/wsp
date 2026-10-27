// SPDX-License-Identifier: AGPL-3.0-only
// What a sign-in on a machine reads off the tool and hands it: the code its page asks for, the display a callback
// road runs with, how often the status is asked, and the note on a sign-in the person stopped.
import { stripVTControlCharacters } from "node:util";
import type { SignInFinish } from "@wsp/protocol";
import { stripOsc8, urlsIn } from "./signin-relay.js";

/** Past this much of the wait the person is on the page or gone, so the checks slow to SLOW_POLL_MS: sixteen
 * minutes at five seconds is about two hundred guest ptys for one login. */
const FAST_FOR_MS = 60_000;
const SLOW_POLL_MS = 20_000;
/** The note on a sign-in the person stopped before it was through. */
export const STOPPED_NOTE = "stopped before the sign-in was through";

/** The code the page will ask for, in the shape the catalog row says this flow prints it: read outside the URLs the
 * tool printed, so a page's own query string is never mistaken for a code. A row with no shape shows no code. */
export function codeIn(text: string, shape: RegExp | undefined): string | undefined {
  if (shape === undefined) return undefined;
  const plain = stripVTControlCharacters(stripOsc8(text));
  const words = urlsIn(plain).reduce((s, url) => s.replaceAll(url, " "), plain);
  return shape.exec(words)?.[0];
}

/** A callback sign-in's pty runs with a DISPLAY, which the daemon otherwise drops so a tool takes its paste-code
 * road instead: gcloud, gemini and railway read an empty DISPLAY as "no browser here". Nothing ever connects to it;
 * the shim as BROWSER is what carries the page to this computer, and the forwarded port carries the redirect back.
 * Written here rather than imported: the daemon is a binary the host cannot import, so the host mirrors the guest's
 * constants (see doctor.ts). */
export const CALLBACK_DISPLAY = ":0";

/** The environment a login's pty runs with, by the road its row finishes on; nothing added on the other roads. */
export function signInEnv(finish: SignInFinish): Record<string, string> | undefined {
  return finish === "callback" ? { DISPLAY: CALLBACK_DISPLAY } : undefined;
}

/** How long until the status is next asked: pollMs through the first minute, then the slower of it and SLOW_POLL_MS. */
export function cadence(elapsedMs: number, pollMs: number): number {
  return elapsedMs < FAST_FOR_MS ? pollMs : Math.max(pollMs, SLOW_POLL_MS);
}
