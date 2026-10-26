// SPDX-License-Identifier: AGPL-3.0-only
import type { GoldenMissingTool, GoldenStage, GoldenStageEvent, LoginState, MachineState, SealedImage, SealedImageCopy } from "../index.js";
import { fmtThreads, lowerFirst, plural } from "./base.js";
import { fmtBytes, fmtDuration } from "./units.js";
import { CLOUD_SETUP_WORDS, GOLDEN_STAGE_WORDS, initStoppedLine, SAVING_IMAGE_LINE } from "./init.js";
// --- the image and its copies, as `wsp image` and Settings > Image read them ---

/** What to do where there is no image: wsp doctor on a cloud computer builds the default one, the only road that makes
 * an image. Where a next version is asked for, the image stays as it is and NO_NEW_VERSION says so. */
export const NO_IMAGE_FIX = "Run wsp doctor with the name of your cloud computer; it builds the default image there.";
/** What a build says where a next version of the image is asked for: this build makes none. */
export const NO_NEW_VERSION = "The image stays as it is, and this build makes no new version.";
/** What a record with no vault says: its copies ask for every sign-in again, and this build makes no new version to hold them. */
export const IMAGE_NO_VAULT = "no sign-ins held; this build makes no new version to hold them";
/** What a record sealed at a provider says: it holds no sign-in by design, so there is nothing to cut again for. */
export const IMAGE_NO_SECRETS = "no sign-ins on it";
/** Whether a copy of this record asks for every sign-in again: it was sealed holding none, and not at a provider,
 * whose seal holds none by design. The refusal that wants force and the card that offers to copy it anyway read
 * this one rule. */
export const copyAsksSignIns = (image: Pick<SealedImage, "vault" | "noSecrets">): boolean => image.vault === undefined && image.noSecrets !== true;
/** Why a fork at a provider is refused: the image was sealed before a provider's seal refused every sign-in, so its
 * disk may still hold them. imageRebuildFix is what to do. */
export const imageHeldSignInsLine = (where: string): string => `Your image on ${where} was sealed with sign-ins on it`;
export const imageRebuildFix = (place: string): string => `The image stays as it is, and this build makes no new version, so a fork on ${place} stays refused.`;
/** What to do about each refusal a copy build meets before anything boots: said after it on a terminal, and in the
 * fix ink beside the Image card's press in the app. Only the no-sign-ins one names a flag: the card sends force on
 * such a record before the host could refuse it, so that fix is read on a terminal alone. */
export const COPY_BUILD_FIX = {
  noCopy: "Start its tasks on a cloud or a computer that holds your image.",
  noImage: NO_IMAGE_FIX,
  noRecipe: `${NO_NEW_VERSION} This one cannot be copied.`,
  noVault: `${NO_NEW_VERSION} Run it again with --force to copy this one anyway.`,
} as const;
/** What a host that has sealed nothing says. Named apart from the attachment lines beside it: those are pictures. */
export const NO_SEALED_IMAGE = `no image yet. ${NO_IMAGE_FIX}`;
/** The same where a road needed the image to fork and there is none. */
export const NO_IMAGE_YET = NO_SEALED_IMAGE;
/** Whether a copy stands on the record as it is now: built at the record's hash, which is the whole of it. The
 * version is the place's own manifest number and says nothing about which record the copy came from, so a second
 * place's v1 built from the record's v2 is current. A copy with no hash was built before hashes were recorded and
 * no record matches it. The one rule: the refusal that will not rebuild a current copy reads it too. */
export function copyIsCurrent(image: Pick<SealedImage, "hash">, copy: Pick<SealedImageCopy, "hash">): boolean {
  return copy.hash === image.hash;
}

/** The states a login ends in with something of it on the machine, which is what the vault then carries: signed in
 * there, copied from this computer, or there with no status command to prove it. The rest left nothing behind. */
const LOGIN_ON_MACHINE: readonly LoginState[] = ["signed-in", "copied", "not-verified"];

/** How many of this image's logins ended with something on the machine for the vault to hold. */
export function sealedLoginsHeld(image: Pick<SealedImage, "logins">): number {
  return image.logins.filter(l => LOGIN_ON_MACHINE.includes(l.state)).length;
}

/** The record in one line: the version, its hash, how many sign-ins it holds and what its disk came to. */
export function sealedImageLine(image: SealedImage): string {
  const held = image.noSecrets === true ? IMAGE_NO_SECRETS : image.vault === undefined ? IMAGE_NO_VAULT : `${plural(sealedLoginsHeld(image), "sign-in")} held, ${plural(image.vault.paths, "path")}`;
  const size = image.usedBytes === undefined ? [] : [fmtBytes(image.usedBytes)];
  return [`${image.name} v${image.version}`, image.hash, held, ...size, `sealed on ${image.sealedFrom}`].join("  ");
}

/** What a place's row says while a copy of the image is built there: the stage in the seal's own words, so the row
 * and the init sheet name one stage one way. */
export const copyBuildingLine = (stage: Exclude<GoldenStage, "failed">): string => `copying your image: ${lowerFirst(GOLDEN_STAGE_WORDS[stage])}`;

/** What the row says after a build there stopped: the seal's own headline, and the reason the way the sheet reads it.
 * It stands until the next build there starts: wsp image build, the next version cut, or the computer connecting
 * again. */
export const copyStoppedLine = (reason?: string): string => {
  const head = lowerFirst(CLOUD_SETUP_WORDS.build.failed);
  const said = reason === undefined ? "" : initStoppedLine(reason);
  return said === "" ? head : `${head}: ${said}`;
};

/** What one golden.stage frame says of the copy at its place: the stage while a build runs, the reason once one
 * stopped, and nothing once one sealed, when what stands is the copies' to say. The host's row, the app's store and
 * its image card all read a frame through this one rule. */
export type CopyBuild = { line: string; stopped: boolean };
export const copyBuildOf = (frame: Pick<GoldenStageEvent, "stage" | "detail">): CopyBuild | undefined => {
  if (frame.stage === "sealed") return undefined;
  return frame.stage === "failed" ? { line: copyStoppedLine(frame.detail), stopped: true } : { line: copyBuildingLine(frame.stage), stopped: false };
};

/** The two words a copy's standing is said in, either of which fits the slot the longer one needs. */
export const COPY_CURRENT = "current";
export const COPY_STALE = "stale";

/** How a copy stands against the record, as the one word a person reads: current when it was built from the record
 * as it is now, stale when it was built from an older one. Nothing at all where the record holds no vault and was not
 * sealed at a provider: such a record was read back off its own copies rather than written at a seal, so it has
 * nothing to judge them by. The one home for the word and for that gate; `wsp image` and Settings > Image both read it. */
export function copyStanding(image: Pick<SealedImage, "hash" | "vault" | "noSecrets">, copy: Pick<SealedImageCopy, "hash">): string | undefined {
  if (image.vault === undefined && image.noSecrets !== true) return undefined;
  return copyIsCurrent(image, copy) ? COPY_CURRENT : COPY_STALE;
}

/** One place's copy in one line: the place, the version it holds, its size where the provider reports one, and
 * whether it stands on the record as it is now. */
export function sealedCopyLine(image: SealedImage, copy: SealedImageCopy): string {
  const size = copy.sizeBytes === undefined ? [] : [fmtBytes(copy.sizeBytes)];
  const standing = copyStanding(image, copy);
  return [copy.place, `v${copy.version}`, ...size, ...(standing === undefined ? [] : [standing])].join("  ");
}

/** What a build at a place came to, as the line a person reads after it: the copy that place now holds, and, when
 * the place already stood on the record, that nothing was built. */
export function sealedBuiltLine(image: SealedImage, built: { copy: SealedImageCopy; built: boolean }): string {
  const line = sealedCopyLine(image, built.copy);
  return built.built ? line : `${line}  already built from this image; nothing was built`;
}

/** What forgetting a workspace takes off this computer, the one sentence every client's confirmation shows. */
export function forgetNotice(threads: number): string {
  return `Its record and ${fmtThreads(threads)} leave this computer; the computer it ran on is already gone.`;
}

/** How much of a checksum a line shows: enough to tell two apart, short enough that a reason line with both fits its cut. */
export const SUM_SHOWN = 12;
export const shortSum = (sha256: string): string => sha256.slice(0, SUM_SHOWN);

/** The install step's failure when a download the recipe pinned does not hash to the recorded sum: what came down,
 * at which tag, and both sums, so a moved asset reads as a moved asset and never as a broken download. */
export function pinMismatchLine(what: string, tag: string, recorded: string, served: string): string {
  return `${what} at ${tag} does not match the checksum recorded on its first install: recorded ${recorded}, served ${served}`;
}

/** What a row marked latest does on every place: the words `wsp recipe`, `wsp image` and the seal's stage all use. */
export const INSTALLS_LATEST = "installs latest";

/** The app's line for a workspace still forked from an older golden version, offered the way the helper update is:
 * a state in words, never a badge. The move is the person's; nothing replaces a machine they are working on. */
export function behindGoldenLine(on: number, head: number): string {
  return `on image v${on}, v${head} available`;
}

/** The states a version row can be in, each as the muted mono word the row's marks column shows: state is text there,
 * never a badge, and a missing tool's outcome indexes this table as it is. The keys are the wire's; the words are
 * the person's, so a row says what the version is rather than the name the code holds it under. The volatile key's
 * word says what such a version has behind it, a snapshot at the provider and no durable template, since that is
 * the whole of what it is and what the provider can drop. */
export const LINEAGE_MARKS = { now: "now", head: "newest", fork: "this one", failed: "failed", skipped: "skipped", volatile: "snapshot only" } as const;
export type LineageMark = keyof typeof LINEAGE_MARKS;

/** What is said for each folder git named no branch for: nothing in a branch slot or the diff pane's git mark, which
 * show the branch or nothing until it is known, whatever kept it unknown; and the one line an empty diff pane says in
 * place of a diff, only for a folder outside any repository, since an empty pane with no reason reads as broken while
 * a refused read shows the cause git gave. */
export const REPO_STATE_WORDS = {
  unknown: { pane: "" },
  none: { pane: "This folder is not inside a git repository, so there is nothing to diff." },
  refused: { pane: "" },
} as const;
export type RepoStateWord = keyof typeof REPO_STATE_WORDS;

/** A missing tool's row as the lineage shows it. A record sealed before the name and outcome were recorded still
 * carries its id, so it reads by that and as failed rather than as a blank row. */
export function missingToolRow(t: { id: string; name?: string; outcome?: GoldenMissingTool["outcome"]; note: string }): { name: string; note: string; mark: LineageMark } {
  return { name: t.name === undefined || t.name === "" ? t.id : t.name, note: t.note, mark: t.outcome ?? "failed" };
}

/** What the provider answered one call with: the status, its message, the request id its reply carried when it
 * carried one (measured 2026-09-07: Solari's replies carry none), and the UTC time the reply landed. */
export interface ProviderAnswer {
  status: number;
  message: string;
  requestId?: string;
  at: string;
}

/** What the builder read at the provider after the last attempt: a state, or unread when the GET itself failed. */
export type BuilderReading = MachineState | "unread";

/** The provider's answer as a report to the provider needs it: status, message, and the request id; without one,
 * that the reply carried none and when it landed, never an empty id. */
export function providerAnswerLine(a: ProviderAnswer): string {
  return `${a.status} ${a.message} (${a.requestId !== undefined ? `request ${a.requestId}` : `no request id from the provider, at ${a.at}`})`;
}

/** What df read of a machine's root disk, or why it read nothing. */
export type DiskUse = { kind: "use"; usedBytes: number; sizeBytes: number } | { kind: "unknown"; reason: string };

/** The snapshotting stage's line after one refused attempt with another to come: which attempt, what the provider
 * said, what the builder reads at the provider, and when the next attempt is. */
export function snapshotAttemptLine(attempt: number, attempts: number, answer: ProviderAnswer, builderState: BuilderReading, retryMs: number): string {
  return `attempt ${attempt} of ${attempts} answered ${providerAnswerLine(answer)}; the builder reads ${builderState}, next attempt in ${fmtDuration(retryMs)}`;
}

/** The seal's failure line once the snapshot is given up on: how many attempts, the last answer, and whether the
 * provider still has the builder. A builder the provider answers 404 for is named gone at the provider's hand. */
export function snapshotFailedLine(attempts: number, answer: ProviderAnswer, builderState: BuilderReading, readError?: string): string {
  const head = `the snapshot failed ${plural(attempts, "time")}: the provider answered ${providerAnswerLine(answer)}`;
  if (builderState === "gone") return `${head} and no longer has the builder (404)`;
  if (builderState === "unread") return `${head} and could not be read about the builder (${readError ?? "no reason given"})`;
  return `${head} while the builder read ${builderState}`;
}

/** The promoting stage's line for one read of the saved image: what the provider says of it and whether the seal asks
 * again. Ready is the last line the stage writes; the template's id is the provider's and reaches no screen. */
export function templateStatusLine(status: string): string {
  return status === "ready" ? "the image is saved" : `${SAVING_IMAGE_LINE}, the provider says ${status}; asking again`;
}

/** The seal's failure line when the provider marks the template failed: forks would have nothing to boot from. */
export function templateFailedLine(templateId: string, reason: string | undefined): string {
  return `the provider failed the template ${templateId}: ${reason ?? "no reason given"}`;
}

/** The seal's failure line when the template never read ready inside the wait. */
export function templateWaitedLine(templateId: string, status: string, waitedMs: number): string {
  return `the template ${templateId} still reads ${status} after ${fmtDuration(waitedMs)}`;
}

/** The doctor's line per version it made durable: the image and version, the template it promoted, and when other
 * templates already carry the name (another host's, or a run that recorded nothing), how many; none when the count
 * is zero or the listing was not given. */
export function templateRecordedLine(image: string, version: number, templateId: string, sharing: number | undefined): string {
  const head = `image ${image} v${version}: template ${templateId} promoted and recorded`;
  return sharing === undefined || sharing === 0 ? head : `${head}; ${sharing} other ${sharing === 1 ? "template carries" : "templates carry"} its name`;
}

/** The doctor's line per version it could not make durable and why: a lost snapshot in the provider's own words is
 * left to the doctor's rebuild road below it. */
export function templateSkippedLine(image: string, version: number, reason: string): string {
  return `image ${image} v${version}: no template recorded, ${reason}`;
}

/** What a version's row says when the provider answers 404 for its snapshot: the vanish the templates exist to outlive. */
export const SNAPSHOT_GONE_REASON = "its snapshot is gone at the provider";

/** The doctor's line on a backend whose capabilities lack templates: nothing to promote, nothing wrong. */
export const NO_TEMPLATES_LINE = "this backend has no templates; image versions stay as snapshots";

/** The doctor's line on a backend that cannot list snapshots: nothing to split, nothing to clean. */
export const NO_SNAPSHOT_LISTING = "this backend lists no snapshots; nothing to split by owner";

/** The doctor's line for the machines its teardown check found on the account that this host did not make: each
 * named with the host that did, left alone and never a failure, since two computers on one account each stand
 * their own. */
export function otherHostsMachinesLine(machines: readonly { id: string; owner?: string }[]): string {
  const named = machines.map(m => `${m.id} (${m.owner === undefined ? "no owner" : `owner ${m.owner}`})`);
  return `left alone ${plural(machines.length, "machine")} this host did not make: ${named.join(", ")}`;
}

/** Why a seal was refused before it took the snapshot: the builder holds a file a sign-in leaves behind. The paths
 * are the guest's own, so whoever reads the line can go and look at them. */
export function credentialOnBuilderLine(paths: readonly string[]): string {
  return `the builder holds a sign-in file at ${paths.join(", ")}; sign-ins never sit in an image, so nothing was sealed`;
}
