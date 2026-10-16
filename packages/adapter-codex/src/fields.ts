// SPDX-License-Identifier: AGPL-3.0-only
// Readers for the loose fields of the JSON lines the app server sends.

export function rec(value: unknown): Record<string, unknown> | undefined {
  return typeof value === "object" && value !== null && !Array.isArray(value) ? (value as Record<string, unknown>) : undefined;
}

export function str(value: unknown): string | undefined {
  return typeof value === "string" ? value : undefined;
}

export const count = (value: unknown): number | undefined => (typeof value === "number" && Number.isFinite(value) ? value : undefined);
