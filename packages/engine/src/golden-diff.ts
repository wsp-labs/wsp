// SPDX-License-Identifier: AGPL-3.0-only
// A recipe row's id read in two parts: its rung and its name.

/** A row's rung is the first segment of its id, its default label the last. */
export const rungOf = (id: string): string => id.slice(0, id.indexOf("/"));
export const nameOf = (id: string): string => id.slice(id.lastIndexOf("/") + 1);
