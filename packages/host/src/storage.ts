// SPDX-License-Identifier: AGPL-3.0-only
// Snapshot storage in words: the line wsp prints at start, wsp init's offer to
// delete the golden versions retention no longer needs, and the doctor's split
// of the account listing into this host's, this host's orphans and the rest.
import type { Readable, Writable } from "node:stream";
import { billedByCount, plural, type SnapshotStoragePricing } from "@wsp/engine";
import type { GoldenVersion, InitSaidLine, SnapshotStorage } from "@wsp/protocol";
import type { AccountOrphans, OrphansDeleted, PlacedRetention, RetentionPlan, Runtime } from "@wsp/runtime";
import { isCancel, log } from "@clack/prompts";
import { confirmPrompt } from "./init-layout.js";

/** The provider lists and bills snapshots in decimal GB (a 3839352227-byte snapshot is 3.84 GB to it), so this line keeps its unit rather than the binary one fmtBytes prints. */
const gb = (bytes: number): string => `${(bytes / 1e9).toFixed(1)} GB`;
const perMonth = (usd: number): string => `about $${usd.toFixed(2)}/month`;

/** v1, v2 and v3. */
function versionList(versions: readonly GoldenVersion[]): string {
  const names = versions.map(v => `v${v.version}`);
  return names.length <= 1 ? names.join("") : `${names.slice(0, -1).join(", ")} and ${names[names.length - 1]}`;
}

/** The account listing split by who made each snapshot: what this host keeps, what this host left with nothing
 * recording it, and what carries no mark of this host. Left out when this host keeps every snapshot, where it
 * would only repeat the count. */
export function describeOwners(s: SnapshotStorage): string {
  if (s.kept.count === s.count) return "";
  const parts = [
    [s.kept, "kept here"],
    [s.orphans, "this host's with nothing recording them"],
    [s.others, "not this host's"],
  ] as const;
  return parts.filter(([g]) => g.count > 0).map(([g, words]) => `${g.count} ${words}, ${gb(g.bytes)}`).join("; ");
}

/** What a host wired to no provider says where the storage line would have been: it asks no listing, because
 * there is no account to ask and no key to ask with. The state file is named because the key is read beside it. */
export const noProviderStorageLine = (statePath?: string): string =>
  `storage: nothing listed; this host is wired to no provider${statePath === undefined ? "" : ` (no key beside ${statePath})`}`;

export function describeStorage(s: SnapshotStorage, placeName?: string): string {
  const named = s.named;
  const past = named === undefined ? 0 : Math.max(0, named.count - named.free);
  const cost = named !== undefined
    ? past > 0 ? `${perMonth(s.monthlyUsd)} for the ${past} past the ${named.free} free` : `inside the ${named.free} free, nothing to pay`
    : s.monthlyUsd > 0 ? `${perMonth(s.monthlyUsd)} above the free ${s.freeGb} GB from ${s.billedFrom}` : `inside the free ${s.freeGb} GB, nothing to pay from ${s.billedFrom}`;
  const owners = describeOwners(s);
  return `storage${placeName === undefined ? "" : ` at ${placeName}`}: ${named !== undefined ? plural(named.count, "named snapshot") : plural(s.count, "snapshot")}, ${gb(s.totalBytes)}; ${cost}${owners === "" ? "" : `. ${owners}`}`;
}

/** A row as a person reads it: the name it carries and the provider id under it, since the id is what a delete takes. */
const rowLine = (r: { id: string; name?: string }): string => (r.name === undefined ? r.id : `${r.name} (${r.id})`);

/** Every orphan named, then every row left alone. A snapshot carries no provider metadata, so this host owns a row
 * only by the mark in its name: anything without that mark is named here and never deleted, whoever made it. */
export function describeOrphans(plan: AccountOrphans): string[] {
  const lines: string[] = [];
  for (const t of plan.templates) lines.push(`  orphan template ${rowLine(t)}`);
  for (const r of plan.snapshots) lines.push(`  orphan snapshot ${rowLine(r)}, ${gb(r.sizeBytes)}`);
  for (const t of plan.others.templates) lines.push(`  left alone, no mark of this host: template ${rowLine(t)}`);
  for (const r of plan.others.snapshots) lines.push(`  left alone, no mark of this host: snapshot ${rowLine(r)}, ${gb(r.sizeBytes)}`);
  return lines;
}

/** What the orphans cost to keep, and how they go. A template is a pointer at a snapshot and holds no billed bytes
 * of its own, so an offer with no snapshot in it says that rather than blaming the free GB for saving nothing. The
 * state file is named because it is what decided: a run under a different --state reads the usual file's goldens as
 * recorded by nothing, and this delete cannot be undone. */
export function describeOrphanOffer(plan: AccountOrphans, yes: boolean, pricing: SnapshotStoragePricing, statePath?: string): string {
  const what = [
    plan.snapshots.length > 0 ? plural(plan.snapshots.length, "orphan snapshot") : "",
    plan.templates.length > 0 ? plural(plan.templates.length, "orphan template") : "",
  ].filter(w => w !== "").join(" and ");
  const cost = billedByCount(pricing) && plan.snapshots.length > 0
    ? plan.savesUsdPerMonth > 0 ? `, saving ${perMonth(plan.savesUsdPerMonth)}` : `, inside the ${pricing.freeNamed ?? 0} kept free, so nothing saved yet`
    : plan.freedBytes === 0
      ? ", holding no snapshot bytes"
      : plan.savesUsdPerMonth > 0
        ? `, ${gb(plan.freedBytes)}, saving ${perMonth(plan.savesUsdPerMonth)}`
        : `, ${gb(plan.freedBytes)}, inside the free GB, so nothing saved yet`;
  const decided = statePath === undefined ? "" : `; nothing in ${statePath} records them`;
  return yes ? `deleting ${what}${cost}${decided}` : `${what}${cost}${decided}; wsp doctor --yes deletes them`;
}

/** What the delete did, by row, so a refusal names the row that stayed. */
export function describeDeleted(done: OrphansDeleted): string {
  const gone = [
    done.snapshots.length > 0 ? plural(done.snapshots.length, "snapshot") : "",
    done.templates.length > 0 ? plural(done.templates.length, "template") : "",
  ].filter(w => w !== "").join(" and ");
  const kept = done.failed.map(f => `${rowLine(f)} stayed (${f.message})`);
  const deleted = gone === "" ? "nothing deleted" : `deleted ${gone}`;
  return kept.length === 0 ? deleted : `${deleted}; ${kept.join("; ")}`;
}

/** The versions a plan names, as the image's own or as the copy of it at another place. */
type Whose = Partial<Pick<PlacedRetention, "copy" | "placeName">>;
const whose = (plan: Whose, versions: string): string => (plan.copy === true ? `${versions} of the copy of your image at ${plan.placeName}` : `image ${versions}`);
const sentence = (words: string): string => `${words[0]!.toUpperCase()}${words.slice(1)}`;

/** The one line the offer asks with: what goes, what it holds, what it saves, what stays. */
export function describeRetention(plan: RetentionPlan & Whose): string {
  const { pricing } = plan;
  const byCount = billedByCount(pricing);
  const held = byCount ? plural(plan.namedFreed, "named snapshot") : gb(plan.freedBytes);
  const saving = plan.savesUsdPerMonth > 0
    ? `saving ${perMonth(plan.savesUsdPerMonth)}${byCount ? "" : ` from ${pricing.billedFrom}`}`
    : byCount ? `inside the ${pricing.freeNamed ?? 0} kept free, so nothing saved yet` : `inside the free ${pricing.freeGb} GB, so nothing saved yet`;
  const parent = plan.keep[1];
  const assumed = plan.parentAssumed && parent !== undefined ? ` v${parent.version} is taken as the parent by version order: v${plan.keep[0]!.version} was sealed before parents were recorded.` : "";
  const abandoned = plan.abandoned.length > 0 ? ` ${versionList(plan.abandoned)} ${plan.abandoned.length === 1 ? "is an abandoned branch" : "are abandoned branches"}: v${plan.keep[0]!.version} was not built through ${plan.abandoned.length === 1 ? "it" : "them"}.` : "";
  return `Delete ${whose(plan, versionList(plan.drop))}, ${held}, ${saving}? ${versionList(plan.keep)} stay.${assumed}${abandoned}`;
}

export function describeGuarded(g: RetentionPlan["guarded"][number], plan: Whose = {}): string {
  const who = g.workspaces.length === 1 ? `workspace ${g.workspaces[0]} was` : `workspaces ${g.workspaces.join(", ")} were`;
  return `${sentence(whose(plan, `v${g.version.version}`))} stays: ${who} forked from it.`;
}

export interface RetentionOfferOptions {
  rt: Runtime;
  interactive: boolean;
  /** Why nothing is asked, in runInit's words, when the offer is taken as yes. */
  takenAs: string;
  input: Readable;
  output: Writable;
  /** Each line said is sent here too, so a host's init job hands it to the terminal beside it. */
  json?: (record: Record<string, unknown>) => void;
}

function say(o: RetentionOfferOptions, level: InitSaidLine["level"], line: string): void {
  log[level](line, { output: o.output });
  o.json?.({ event: "cleanup", level, line });
}

/** The older ancestors and the abandoned branches are offered for deletion at every place holding versions of the
 * image, each kept on a No, deleted on a Yes or when nobody is asked. A listing a provider refuses keeps every version
 * at that place and says so. */
export async function retentionOffer(o: RetentionOfferOptions): Promise<void> {
  let at: string[];
  try {
    at = await o.rt.golden.retentionPlaces();
  } catch (e) {
    say(o, "warn", `Snapshot storage was not read (${e instanceof Error ? e.message : String(e)}); every image version is kept.`);
    return;
  }
  for (const place of at) await offerAt(o, place);
}

async function offerAt(o: RetentionOfferOptions, place: string): Promise<void> {
  let plan: PlacedRetention | undefined;
  try {
    plan = await o.rt.golden.retention(undefined, place);
  } catch (e) {
    say(o, "warn", `Snapshot storage was not read (${e instanceof Error ? e.message : String(e)}); every image version is kept.`);
    return;
  }
  if (plan === undefined) return;
  for (const g of plan.guarded) say(o, "info", describeGuarded(g, plan));
  if (plan.drop.length === 0) return;
  const line = describeRetention(plan);
  if (o.interactive) {
    const go = await confirmPrompt({ message: line, hint: "No keeps every version.", initialValue: true, input: o.input, output: o.output });
    if (isCancel(go) || !go) {
      say(o, "step", plan.copy ? `Every version of the copy of your image at ${plan.placeName} is kept.` : "Every image version is kept.");
      return;
    }
  } else {
    say(o, "step", `${line} Taken as yes (${o.takenAs}).`);
  }
  const { dropped, failed, storage } = await o.rt.golden.prune(undefined, place);
  if (dropped.length > 0) say(o, "success", `Deleted ${whose(plan, versionList(dropped))}.`);
  for (const f of failed) say(o, "warn", `${sentence(whose(plan, `v${f.version}`))} was not deleted (${f.message}); it stays in the lineage.`);
  if (storage !== undefined) say(o, "info", describeStorage(storage, plan.placeName));
}
