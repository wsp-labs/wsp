// SPDX-License-Identifier: AGPL-3.0-only
// What a tile tells on its hover card to its right, as T3 Code's sidebar
// details tooltip: the full title, where the thread runs and in which folder, its branch, the
// agent's model, the pull request with its state as a word and the files
// changed, then whatever holds the thread. The tile itself keeps two rows and
// one mark: an open pull request's icon.
import { agentName } from "@wsp/catalog";
import { PULL_REQUEST_WORDS, capWaitLine, cloudSaid, diskExpired, diskExpiredLine, diskMayGoLine, fmtRate, fmtRateDollars, fmtMemGb, forkedFromLine, sizeNameOf, subNotPushedReason, type PullRequestState, type SubRow, type ThreadCapWait } from "@wsp/protocol";
import { PR_WORDS } from "../pull-request/words.js";
import { APP_LOCALE } from "../lib/timestampFormat.js";
import { CLOCK } from "../settings/image.js";
import type { TilePlace } from "./ThreadTile.js";

export interface TileCardInput {
  readonly title: string;
  readonly place: TilePlace;
  /** A sub-thread's machine, said where the computer's name would be. */
  readonly machine?: string | undefined;
  /** Where a thread in the Needs you inbox hangs, in words, for a thread under a tree. */
  readonly startedBy?: string | undefined;
  /** The title of the thread this one was forked from. */
  readonly forkedFrom?: string | undefined;
  /** The folder the workspace works in: its copy, or the project's own. */
  readonly folder?: string | undefined;
  /** Empty where the workspace is on no branch or none is known. */
  readonly branch: string;
  readonly harness: string | null;
  /** The model by the label its catalog gives it, where the thread names one. */
  readonly model: string | null;
  readonly pr?: { readonly number: number; readonly state: PullRequestState } | undefined;
  /** What the checkout holds uncommitted, in its own words, where the host read it. */
  readonly changed?: string | undefined;
  /** What holds the thread, each a sentence of its own: the question it waits on, why the pull request is not read. */
  readonly notes: ReadonlyArray<string | null>;
}

export type TileCardLine = { readonly kind: "started-by" | "forked" | "project" | "computer" | "folder" | "branch" | "agent" | "pr" | "changed" | "note"; readonly text: string };

export function tileCardLines(o: TileCardInput): { title: string; lines: TileCardLine[] } {
  const line = (kind: TileCardLine["kind"], text: string | null | undefined): TileCardLine[] => (text === null || text === undefined || text === "" ? [] : [{ kind, text }]);
  return {
    title: o.title,
    lines: [
      ...line("started-by", o.startedBy),
      ...line("forked", o.forkedFrom === undefined ? undefined : forkedFromLine(o.forkedFrom)),
      ...line("project", o.place.project),
      ...line("computer", o.machine ?? o.place.computer),
      ...line("folder", o.folder),
      ...line("branch", o.branch),
      ...line("agent", o.model ?? (o.harness === null ? null : agentName(o.harness))),
      ...line("pr", o.pr === undefined ? null : `${PR_WORDS.row(o.pr.number)}, ${PULL_REQUEST_WORDS[o.pr.state]}`),
      ...line("changed", o.changed),
      ...o.notes.flatMap(note => line("note", note)),
    ],
  };
}

/** Whether a tile's second row ends in the pull request's icon: only while it is open. */
export const tilePrIcon = (pr: { readonly state: PullRequestState } | undefined): boolean => pr?.state === "open";

/** A stretch to come in whole minutes, past an hour in hours and minutes. */
export const minutesWord = (ms: number): string => {
  const minutes = Math.max(1, Math.ceil(ms / 60_000));
  return minutes < 60 ? `${minutes} min` : `${Math.floor(minutes / 60)} h${minutes % 60 === 0 ? "" : ` ${minutes % 60} min`}`;
};

const KEPT_DAY = new Intl.DateTimeFormat(APP_LOCALE, { month: "short", day: "numeric" });

/** A held sub-thread's cloud and the day it may delete the napping machine's disk from; none while the machine runs. */
export const subKept = (sub: SubRow, diskKeptUntil: number | undefined): { on: string; day: string } | undefined =>
  sub.held === undefined || diskKeptUntil === undefined ? undefined : { on: cloudSaid(sub.on), day: KEPT_DAY.format(diskKeptUntil) };

/** The card's line for a thread whose napping machine is past the keep of its disk; null on every other. */
export function expiredNote(t: { diskKeptUntil?: number; status: string; asking?: string | null }, now: number): string | null {
  return t.diskKeptUntil !== undefined && diskExpired(t, now) ? diskExpiredLine(KEPT_DAY.format(t.diskKeptUntil)) : null;
}

/** What a sub-thread's card says of its machine on the computer's line: its size and what it costs while it runs, when
 * it is deleted once done, that its work holds it, that it was deleted, or that its cloud left the host. */
export function subMachineLine(sub: SubRow, working: boolean, accruedUsd: number | null, here: string, now: number, diskKeptUntil?: number): string {
  const on = cloudSaid(sub.on);
  const size = sizeNameOf(sub.size);
  if (sub.cloudGone === true) return `${on} was removed from ${here}`;
  if (sub.state === "released") return `${on}, its machine was deleted ${CLOCK.format(sub.releasedAt ?? now)}; a message starts a new one`;
  const kept = subKept(sub, diskKeptUntil);
  if (!working && kept !== undefined) return `${on}, ${size}, napping; ${diskMayGoLine(kept.on, kept.day)}`;
  if (!working && sub.held !== undefined) return `${on}, ${size}, napping, kept until its work is pushed`;
  if (!working && sub.deleteAt !== undefined) {
    const goes = `its machine is deleted in ${minutesWord(sub.deleteAt - now)}`;
    return sub.state === "running" ? `${on}, ${size}, running (${on} would not stop it); ${goes}` : `${on}, ${size}, napping; ${goes}`;
  }
  const rate = sub.rateUsdPerHour === undefined ? [] : [fmtRate(sub.rateUsdPerHour)];
  const sofar = accruedUsd === null || sub.state === "waiting" ? [] : [`${fmtRateDollars(accruedUsd)} so far`];
  return [on, size, `${sub.size.cpu} vCPU`, fmtMemGb(sub.size.memMb), ...rate, ...sofar].join(", ");
}

/** The reason under a sub-thread's status word on its card, where its machine says more than the thread does. */
export function subReason(sub: SubRow, capped: ThreadCapWait | undefined, working: boolean, diskKeptUntil?: number): string | undefined {
  const on = cloudSaid(sub.on);
  if (working && capped !== undefined) return `${capWaitLine(capped).charAt(0).toUpperCase()}${capWaitLine(capped).slice(1)}`;
  if (working && sub.state === "starting") return `Starting a machine on ${on} (${sizeNameOf(sub.size)}, ${sub.size.cpu} vCPU, ${fmtMemGb(sub.size.memMb)})`;
  if (!working && sub.held !== undefined) return subNotPushedReason(sub.held, sub.branch, subKept(sub, diskKeptUntil));
  return undefined;
}

/** The machine pane's two facts for a sub-machine: what it costs while it stands, and when it goes. */
export function subMachineFacts(sub: SubRow, working: boolean, accruedUsd: number | null, now: number, diskKeptUntil?: number): { cost?: string; goes?: string } {
  if (sub.state === "released") return {};
  const rate = sub.rateUsdPerHour === undefined ? [] : [fmtRate(sub.rateUsdPerHour)];
  const sofar = accruedUsd === null ? [] : [`${fmtRateDollars(accruedUsd)} so far`];
  const cost = [...rate, ...sofar].join(", ");
  const kept = subKept(sub, diskKeptUntil);
  const goes = working ? undefined : kept !== undefined ? diskMayGoLine(kept.on, kept.day) : sub.held !== undefined ? "Kept until its work is pushed" : sub.deleteAt === undefined ? undefined : `Deleted in ${minutesWord(sub.deleteAt - now)}`;
  return { ...(cost === "" ? {} : { cost }), ...(goes === undefined ? {} : { goes }) };
}
