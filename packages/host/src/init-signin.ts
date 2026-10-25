// SPDX-License-Identifier: AGPL-3.0-only
// The signing-in stage of wsp init: each login chosen as "sign in during the
// build" runs in this terminal over a pty on the builder, and its exit code
// says whether it landed; a login whose files were copied is recorded as
// copied, and one left to first use is recorded as that with nothing run for
// it. The summary before the seal offers a retry (with the no-browser variant
// when the table has one) or a skip for a sign-in that failed or that the cap
// ended. What each login came to is written next to the import result so the
// golden's notes carry it.
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import type { Writable } from "node:stream";
import { styleText } from "node:util";
import type { ManifestEntry } from "@wsp/collect";
import { LOGIN_STATE_WORDS, type LoginChoice, type LoginState } from "@wsp/protocol";
import type { GoldenBuilderView, Runtime } from "@wsp/runtime";
import { S_BAR, log, note } from "@clack/prompts";
import { connectDaemonSocket, type ConnectOptions, type DaemonSocket } from "./doctor.js";
import { GUTTER, ellipsize, table, widthOf } from "./init-layout.js";
import { agentName } from "./init-recipe.js";
import { readKey } from "./init-select.js";
import { relayPty, type PtyLink, type RelayTerminal } from "./signin-relay.js";
import { hasLogin, signInFor, type SignIn } from "./signin-table.js";

export interface LoginOutcome {
  id: string;
  label: string;
  state: LoginState;
  /** The command that ran last, when one did. */
  command?: string;
  /** How the last command's pty ended; -1 when it never did. */
  exit?: number;
  note?: string;
  /** What a copied login's files went to the machine without, named by the account. */
  left?: string;
}

/** What the sign-in stage shares with the host's callback relay: whether one
 * page the machine asks to open may open here without a click (the person
 * pressed o in this flow; one o, one open), the page it asked for that o
 * should open instead of the printed link, and where a host line goes while
 * a pty is shown. */
export interface SignInFlow {
  armed: boolean;
  /** The page o opened here; the relay declines to open that one again. */
  openedUrl?: string;
  /** A page the machine asked to open that returns through a forwarded port, arrived while nothing was armed. */
  callbackUrl?: string;
  show?: (line: string) => void;
  /** Each page the machine asks to open while a stage holds this flow, with the callback port when it named one:
   * the hand-off stage prints it for the person, the terminal stage waits for o instead and leaves this unset. */
  onPage?: (url: string, port?: number) => void;
  /** What the relay's line says for a page it did not open; the terminal stage's o is the default. */
  openWords?: string;
}

/** Where a code from a sign-in's page reaches the tool waiting for it on the machine. The hand-off opens one writer
 * per login while its command runs, and a submit from the app types the code into that login's own pty, the way the
 * person would at the machine's terminal. Nothing is kept here: a code passes through and is gone. */
export class SignInCodes {
  private readonly writers = new Map<string, (code: string) => Promise<void>>();

  /** The login takes a code from now until the returned close, which only closes the writer it opened. */
  open(tool: string, write: (code: string) => Promise<void>): () => void {
    this.writers.set(tool, write);
    return () => {
      if (this.writers.get(tool) === write) this.writers.delete(tool);
    };
  }

  /** Types the code into that login's pty; refused when no sign-in for the tool is waiting for one. */
  async submit(tool: string, code: string): Promise<void> {
    const write = this.writers.get(tool);
    if (write === undefined) throw new Error(`no sign-in for ${tool} is waiting for a code from you`);
    await write(code);
  }
}

export interface BuilderLink {
  link: PtyLink;
  close(): void;
}

/** The whole wait a sign-in running during the build gets, whatever the tool itself would wait for: the build never
 * waits on a person, and a browser sign-in somebody is actually at lands well inside this. A row the cap ends is
 * deferred to the workspace, not failed. */
export const SIGN_IN_CAP_MS = 2 * 60_000;
const dim = (s: string): string => styleText("dim", s);

function minutes(ms: number): string {
  return `${Math.round(ms / 60_000)} min`;
}

/** The row's note and what its copy left behind, as one detail. */
function detailOf(o: LoginOutcome): string {
  return [o.note, o.left].filter(x => x !== undefined).join("; ");
}

/** The row's state and detail; the detail cut to `width` cells for the row as a whole. */
export function stateLine(o: LoginOutcome, width = Infinity): string {
  const word = LOGIN_STATE_WORDS[o.state];
  const colored = o.state === "signed-in" ? styleText("green", word) : o.state === "not-signed-in" ? styleText("yellow", word) : dim(word);
  const detail = ellipsize(detailOf(o), width - o.label.length - word.length - 5);
  return `${o.label}: ${colored}${detail !== "" ? dim(` (${detail})`) : ""}`;
}

/** The outcome an answer reaches with nothing run for it: a copy's files are on the machine already, so its status is
 * the catalog's to check from the app and not this terminal's, and a login left to first use never runs here at all.
 * Nothing for the answers a stage signs in or another step takes. One entry per answer, so a fifth cannot be offered
 * before it says whether the build runs it. */
const SETTLED_AT: Record<LoginChoice, LoginState | undefined> = { copy: "copied", later: "deferred", machine: undefined, key: undefined, skip: undefined, token: undefined };

/** Where this row lands with nothing run for it, or nothing when the stage is to sign it in. */
const settledAt = (e: ManifestEntry): LoginState | undefined => (e.choice === undefined ? undefined : SETTLED_AT[e.choice]);

/** The outcomes a sign-in stage starts from, with every answer the build runs nothing for already settled and sent
 * to json as a row of the build. What is left is the rows to sign in on the machine, each beside its outcome. */
export function settledOutcomes(logins: readonly ManifestEntry[], left: ReadonlyMap<string, string> | undefined, output: Writable, json?: (record: Record<string, unknown>) => void): { outcomes: LoginOutcome[]; machine: [ManifestEntry, LoginOutcome][] } {
  const outcomes = logins.map((e): LoginOutcome => {
    const had = left?.get(e.id);
    return { id: e.id, label: e.label, state: "skipped", ...(had !== undefined ? { left: had } : {}) };
  });
  const rows = logins.map((e, i): [ManifestEntry, LoginOutcome] => [e, outcomes[i]!]);
  for (const [e, r] of rows) {
    const settled = settledAt(e);
    if (settled === undefined) continue;
    r.state = settled;
    log.message(stateLine(r), { output, symbol: dim(S_BAR) });
    json?.({ event: "sign-in-result", tool: agentName(e), label: r.label, state: r.state });
  }
  return { outcomes, machine: rows.filter(([e]) => settledAt(e) === undefined) };
}

/** A detail cut below this says nothing, so the column is left out rather than stubbed. */
const DETAIL_ROOM = 12;

/** Label, state, detail per login; the detail is cut so the note frame (6 columns) never wraps a row. A state word
 * long enough to take the whole line leaves the detail out instead of pushing the row past the frame, which is what
 * wrapped it; the line each row printed as it settled carries that detail whole. */
function summaryRows(outcomes: readonly LoginOutcome[], width: number): string[] {
  const labelW = Math.max(...outcomes.map(r => r.label.length));
  const stateW = Math.max(...outcomes.map(r => LOGIN_STATE_WORDS[r.state].length));
  const room = width - 6 - labelW - stateW - 2 * GUTTER.length;
  return table(outcomes.map(r => [r.label, LOGIN_STATE_WORDS[r.state], ellipsize(detailOf(r), room < DETAIL_ROOM ? 0 : room)]));
}

/** The first word of a command past its NAME=value prefixes: the tool the shell could not find. */
export function toolOf(command: string): string {
  return command.split(" ").find(w => !/^[A-Za-z_][A-Za-z0-9_]*=/.test(w)) ?? command;
}
