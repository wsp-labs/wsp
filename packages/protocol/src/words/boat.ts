// SPDX-License-Identifier: AGPL-3.0-only
// Boat's own lines: what a key holds as GET /limits answers it, and what a seal
// there ends on. Boat answers /limits under two spellings, ascii.dev's box
// fields and boat.dev's sandbox fields, and every reading here takes both.
import type { LoginState } from "../init-job.js";
import { fmtCpus, fmtMemGb, fmtRateDollars } from "./units.js";

/** What a key holds, off one /limits answer. A field the answer leaves out is left out here, and its piece of the
 * line with it, so nothing is said that the account did not say. */
export interface BoatPlan {
  trial: boolean;
  /** The plan's monthly price in dollars; absent on a trial. */
  dollars?: number;
  /** The trial's machine time in hours. */
  hours?: number;
  machines?: number;
  /** Starts a minute, an hour and a day, or unlimited where the account has no start limits. */
  starts?: { minute?: number; hour?: number; day?: number } | "unlimited";
}

const record = (value: unknown): Record<string, unknown> | undefined => (typeof value === "object" && value !== null && !Array.isArray(value) ? (value as Record<string, unknown>) : undefined);
const count = (value: unknown): number | undefined => (typeof value === "number" && Number.isFinite(value) && value >= 0 ? value : undefined);
const first = (...values: unknown[]): number | undefined => values.map(count).find(v => v !== undefined);

/** The plan off Boat's /limits answer, or nothing where the answer is not one. */
export function boatPlanOf(answer: unknown): BoatPlan | undefined {
  const a = record(answer);
  if (a === undefined) return undefined;
  const key = typeof a["boxPlanKey"] === "string" ? a["boxPlanKey"] : typeof a["sandboxPlanKey"] === "string" ? a["sandboxPlanKey"] : undefined;
  const trial = a["accessTier"] === "trial" || key === "trial";
  const starts = record(a["starts"]);
  const caps = record(a["startLimits"]);
  const windowLimit = (name: string): unknown => record(starts?.[name])?.["limit"];
  const unlimited = a["unlimited"] === true || starts?.["unlimited"] === true;
  const read = { minute: first(windowLimit("minute"), caps?.["perMinute"]), hour: first(windowLimit("hour"), caps?.["perHour"]), day: first(windowLimit("day"), caps?.["perDay"]) };
  const keyDollars = key === undefined ? undefined : /^\w+_(\d+)$/.exec(key)?.[1];
  const dollars = trial ? undefined : first(a["boxPlanDollars"], a["sandboxPlanDollars"], keyDollars === undefined ? undefined : Number(keyDollars));
  const capSeconds = count(a["trialComputeCapSeconds"]);
  const hours = trial ? (capSeconds !== undefined ? capSeconds / 3600 : count(a["creditBalanceHours"])) : undefined;
  const machines = first(a["maxActiveBoxes"], a["maxActiveSandboxes"]);
  const startsRead = Object.values(read).some(v => v !== undefined);
  return {
    trial,
    ...(dollars !== undefined ? { dollars } : {}),
    ...(hours !== undefined ? { hours } : {}),
    ...(machines !== undefined ? { machines } : {}),
    ...(unlimited ? { starts: "unlimited" as const } : startsRead ? { starts: Object.fromEntries(Object.entries(read).filter(([, v]) => v !== undefined)) } : {}),
  };
}

/** A count of hours as the plan line says it: whole where it is whole, else to one decimal. */
const hoursWord = (hours: number): string => `${Number.isInteger(hours) ? hours : hours.toFixed(1)} h`;

function startsWords(starts: BoatPlan["starts"]): string[] {
  if (starts === undefined) return [];
  if (starts === "unlimited") return ["no start limits"];
  const said = [
    starts.minute === undefined ? undefined : `${starts.minute} starts a minute`,
    starts.hour === undefined ? undefined : `${starts.hour} ${starts.minute === undefined ? "starts " : ""}an hour`,
    starts.day === undefined ? undefined : `${starts.day} ${starts.minute === undefined && starts.hour === undefined ? "starts " : ""}a day`,
  ];
  return said.filter((s): s is string => s !== undefined);
}

/** The size a new machine gets on Boat until the person picks another, as the plan line names it. */
export interface BoatSizeWords {
  type: string;
  cpu: number;
  memMb: number;
  rateUsdPerHour: number;
}

/** What wsp add boat prints once the key is saved, off the account's own /limits answer: the plan, the machines it
 * runs at once and its starts, then the size a machine gets. A trial says its hours and the sizes it is sold. */
export function boatAddedLine(plan: BoatPlan | undefined, size: BoatSizeWords): string {
  if (plan === undefined) return `Boat added; ${sizeWords(size)}.`;
  const machines = plan.machines === undefined ? [] : [`${plan.machines} machine${plan.machines === 1 ? "" : "s"} at once`];
  if (plan.trial) {
    const held = [...(plan.hours === undefined ? [] : [hoursWord(plan.hours)]), ...machines, ...startsWords(plan.starts)];
    return `Boat added: trial${held.length === 0 ? "" : ` (${held.join(", ")})`}; small and default sizes.`;
  }
  const held = [plan.dollars === undefined ? "a paid plan" : `$${plan.dollars} plan`, ...machines, ...startsWords(plan.starts)];
  return `Boat added: ${held.join(", ")}; ${sizeWords(size)}.`;
}

const sizeWords = (size: BoatSizeWords): string => `${size.type} ${fmtCpus(size.cpu)} ${fmtMemGb(size.memMb)} at ${fmtRateDollars(size.rateUsdPerHour)}/h until you pick another size`;

/** What wsp init says of the first machine's flags on a build on Boat, which forks no first machine. */
export const boatForksNoFirstLine = (flags: readonly string[]): string => `Boat forks no first machine, so ${flags.join(" and ")} ${flags.length === 1 ? "does" : "do"} nothing here.`;

/** What a seal on Boat ends on in place of a first machine: nothing is forked there, a lead's sub-thread is. */
export const BOAT_READY_LINE = "Boat is ready for sub-threads: a thread starts one with run and on boat.";

/** The warning a seal on Boat ends with when its image holds no GitHub sign-in: a sub-thread there clones and pushes
 * with it. */
export const BOAT_NO_GITHUB_LINE =
  "This image holds no GitHub sign-in, so a sub-thread on Boat can neither clone a private repository nor push its branch. Run wsp init --on boat again with the GitHub CLI login set to copy.";

/** The GitHub CLI login row's label: the detector writes it and a seal records the login under it. */
export const GITHUB_LOGIN_NAME = "GitHub CLI login";

/** The sign-in states that leave a login on the image a sub-thread can use. */
const GITHUB_HELD: readonly LoginState[] = ["signed-in", "copied"];

/** The lines a seal on Boat ends on, off the sign-ins it recorded by name. */
export function boatSealLines(logins: readonly { name: string; state?: LoginState | undefined }[]): string[] {
  const github = logins.some(l => l.name === GITHUB_LOGIN_NAME && l.state !== undefined && GITHUB_HELD.includes(l.state));
  return [BOAT_READY_LINE, ...(github ? [] : [BOAT_NO_GITHUB_LINE])];
}
