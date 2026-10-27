// SPDX-License-Identifier: AGPL-3.0-only

import { z } from "zod";
import { WorkspaceView } from "./workspace.js";
import { GoldenVersion, SealedImage, SealedImageCopy } from "./golden-image.js";

// --- snapshot lineage (golden manifest as the rollback UI reads it) -----------

/** Every sealed version of one golden and the head new forks use. head is null
 * while the golden has never been sealed. The manifest is the truth here, never
 * a backend snapshot listing (list() is best-effort). */
export const SnapshotLineage = z.object({
  name: z.string(),
  head: z.number().nullable(),
  versions: z.array(GoldenVersion),
});
export type SnapshotLineage = z.infer<typeof SnapshotLineage>;

/** What Settings > Image and `wsp image` draw: the record and every copy at every place. */
export const SealedImageView = z.object({
  image: SealedImage.nullable(),
  copies: z.array(SealedImageCopy),
});
export type SealedImageView = z.infer<typeof SealedImageView>;

/** One part of the listing: how many snapshots and what they hold. */
export const SnapshotGroup = z.object({ count: z.number(), bytes: z.number() });
export type SnapshotGroup = z.infer<typeof SnapshotGroup>;

/** Every snapshot on the account as the provider bills it: a snapshot is a full disk image, the free GB are shared
 * by all of them, and the rest costs usdPerGbMonth from billedFrom. Sizes come from the provider's snapshot
 * listing, never from a machine's requested disk. The three groups split that sum by who made each snapshot, since
 * the account is shared and the bill is not. */
export const SnapshotStorage = z.object({
  count: z.number(),
  totalBytes: z.number(),
  freeGb: z.number(),
  usdPerGbMonth: z.number(),
  billedFrom: z.string(),
  monthlyUsd: z.number(),
  /** On a provider that bills named snapshots by count: how many there are and how many are free. monthlyUsd is
   * what the ones past those cost. Its own unnamed history is free and not read, so the counts above are these too. */
  named: z.object({ count: z.number(), free: z.number() }).optional(),
  /** This host's and in use: a golden version or a live workspace names it, or this host took it
   * inside the grace a seal needs before the manifest records it. */
  kept: SnapshotGroup,
  /** This host's mark on the name and nothing names the id: what wsp doctor offers to delete. */
  orphans: SnapshotGroup,
  /** No mark of this host: another host's or a person's own, never touched. */
  others: SnapshotGroup,
});
export type SnapshotStorage = z.infer<typeof SnapshotStorage>;

/** Rollback only moves head. Workspaces already forked keep their machines and
 * image; the field says so on the wire so no client reads it as a fleet change. */
export const SnapshotRollbackResult = z.object({
  lineage: SnapshotLineage,
  existingWorkspaces: z.literal("untouched"),
});
export type SnapshotRollbackResult = z.infer<typeof SnapshotRollbackResult>;

/** The create's own answer: the workspace, and what the runtime had to do to make room for it (a builder kept
 * after a save, stopped at the machine cap), so the person who asked reads why. Absent when nothing was stopped. */
export const WorkspaceCreateResult = z.object({ workspace: WorkspaceView, notice: z.string().optional() });
export type WorkspaceCreateResult = z.infer<typeof WorkspaceCreateResult>;
