// SPDX-License-Identifier: AGPL-3.0-only
// Snapshot storage in words: the line wsp prints at start and the doctor's split of the account listing into this
// host's, this host's orphans and the rest.
import { billedByCount, plural, type SnapshotStoragePricing } from "@wsp/engine";
import { type SnapshotStorage } from "@wsp/protocol";
import type { AccountOrphans, OrphansDeleted } from "@wsp/runtime";

/** The provider lists and bills snapshots in decimal GB (a 3839352227-byte snapshot is 3.84 GB to it), so this line keeps its unit rather than the binary one fmtBytes prints. */
const gb = (bytes: number): string => `${(bytes / 1e9).toFixed(1)} GB`;
const perMonth = (usd: number): string => `about $${usd.toFixed(2)}/month`;

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
