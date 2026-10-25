// SPDX-License-Identifier: AGPL-3.0-only
// The sign-in stage with nobody at this terminal. Each login chosen as "sign in
// on the machine" runs on the builder the same way, but instead of showing the
// machine's terminal this prints the page the tool wants opened, the code the
// flow shows when it has one, and the command that opens that page on this
// computer; then it waits, asking the tool's own status on the machine, until
// the person finishes, the command ends, or the deadline passes. With --json
// each of those is one object on stdout for whoever is driving the run. A
// login whose row says it finishes by callback runs with a browser to reach,
// so its page returns through the forwarded port and no code comes back; one
// whose page hands a code back takes that code from the client and types it
// into the tool. A code is never read out of the output, kept or printed.
import type { Writable } from "node:stream";
import { stripVTControlCharacters, styleText } from "node:util";
import type { StatusCheck } from "@wsp/catalog";
import type { ManifestEntry } from "@wsp/collect";
import { fmtDuration, redirectsToMachine, shellQuote, type SignInFinish } from "@wsp/protocol";
import { S_BAR, log } from "@clack/prompts";
import { agentName } from "./init-recipe.js";
import { SIGN_IN_CAP_MS, settledOutcomes, stateLine, toolOf, type BuilderLink, type LoginOutcome, type SignInCodes, type SignInFlow } from "./init-signin.js";
import { openerCommand } from "./relay.js";
import { runQuiet, stripOsc8, urlsIn, watchPty, type WatchOutcome } from "./signin-relay.js";
import { hasLogin, questionsOf, signInFor } from "./signin-table.js";

const POLL_MS = 5_000;
/** Past this much of the wait the person is on the page or gone, so the checks slow to SLOW_POLL_MS: sixteen
 * minutes at five seconds is about two hundred guest ptys for one login. */
const FAST_FOR_MS = 60_000;
const SLOW_POLL_MS = 20_000;
const GRACE_MS = 2_000;
/** A status command that hangs is given up on, so one bad check never eats the person's deadline. */
const STATUS_MS = 60_000;
/** The tool's output kept for the code scan: the page and its code come first, and a tool that prints for the whole
 * cap must not be held in memory. */
const TEXT_CAP = 16 * 1024;
const dim = (s: string): string => styleText("dim", s);
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

/** Resolves after ms, or as soon as `until` settles, leaving no timer behind either way. */
function wait(ms: number, until: Promise<unknown>): Promise<void> {
  return new Promise(resolve => {
    const timer = setTimeout(resolve, ms);
    void until.then(
      () => {
        clearTimeout(timer);
        resolve();
      },
      () => {
        clearTimeout(timer);
        resolve();
      },
    );
  });
}
