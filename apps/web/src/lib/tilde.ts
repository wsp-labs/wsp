// SPDX-License-Identifier: AGPL-3.0-only

/** A path under `home` written from `~`, as a person reads it; any other path, or no home known, stays whole. */
export const tilde = (path: string, home: string): string => (home !== "" && (path === home || path.startsWith(`${home}/`)) ? `~${path.slice(home.length)}` : path);
