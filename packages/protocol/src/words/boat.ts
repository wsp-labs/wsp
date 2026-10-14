// SPDX-License-Identifier: AGPL-3.0-only
// Boat's own lines: what a key holds as GET /limits answers it, and what a seal
// there ends on. The answer is read by the engine's limitsOf; these are words.
import { fmtCpus, fmtMemGb, fmtRateDollars } from "./units.js";

/** What a key holds, as the engine reads it off one /limits answer (`limitsOf`): the plan, its machine cap, its
 * three windows of starts (absent where the account has none) and the balance. A field the answer left out is left
 * out of the line, so nothing is said that the account did not say. */
export interface BoatLimitsRead {
  trial: boolean;
  planUsd?: number;
  machinesAtOnce?: number;
  starts?: Record<"minute" | "hour" | "day", { limit: number }>;
  creditBalanceHours?: number;
}

/** A count of hours as the plan line says it: whole where it is whole, else to one decimal. */
const hoursWord = (hours: number): string => `${Number.isInteger(hours) ? hours : hours.toFixed(1)} h`;

const startsWords = (starts: BoatLimitsRead["starts"]): string[] =>
  starts === undefined ? [] : [`${starts.minute.limit} starts a minute`, `${starts.hour.limit} an hour`, `${starts.day.limit} a day`];

/** The size a new machine gets on Boat until the person picks another, as the plan line names it. */
export interface BoatSizeWords {
  type: string;
  cpu: number;
  memMb: number;
  rateUsdPerHour: number;
}

/** What wsp add boat prints once the key is saved, off the account's own /limits answer: the plan, the machines it
 * runs at once and its starts, then the size a machine gets. A trial says its hours and the sizes it is sold. */
export function boatAddedLine(limits: BoatLimitsRead | undefined, size: BoatSizeWords): string {
  if (limits === undefined) return `Boat added; ${sizeWords(size)}.`;
  const machines = limits.machinesAtOnce === undefined ? [] : [`${limits.machinesAtOnce} machine${limits.machinesAtOnce === 1 ? "" : "s"} at once`];
  if (limits.trial) {
    const held = [...(limits.creditBalanceHours === undefined ? [] : [hoursWord(limits.creditBalanceHours)]), ...machines, ...startsWords(limits.starts)];
    return `Boat added: trial${held.length === 0 ? "" : ` (${held.join(", ")})`}; small and default sizes.`;
  }
  const held = [limits.planUsd === undefined ? "a paid plan" : `$${limits.planUsd} plan`, ...machines, ...startsWords(limits.starts)];
  return `Boat added: ${held.join(", ")}; ${sizeWords(size)}.`;
}

const sizeWords = (size: BoatSizeWords): string => `${size.type} ${fmtCpus(size.cpu)} ${fmtMemGb(size.memMb)} at ${fmtRateDollars(size.rateUsdPerHour)}/h until you pick another size`;

/** What wsp init says of the first machine's flags on a build on Boat, which forks no first machine. */
export const boatForksNoFirstLine = (flags: readonly string[]): string => `Boat forks no first machine, so ${flags.join(" and ")} ${flags.length === 1 ? "does" : "do"} nothing here.`;

/** What a seal on Boat ends on in place of a first machine: nothing is forked there, a lead's sub-thread is. */
export const BOAT_READY_LINE = "Boat is ready for sub-threads: a thread starts one with run and on boat.";

/** The GitHub CLI login row's label: the detector writes it and a seal records the login under it. */
export const GITHUB_LOGIN_NAME = "GitHub CLI login";
