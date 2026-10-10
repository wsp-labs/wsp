// SPDX-License-Identifier: AGPL-3.0-only
// The search over a thread's parts: every match in thread order, each named by its entry, its part and its place
// among that part's matches, which is how the page finds the same match among the ranges it draws.
import { MATCH_CAP, spansIn, type Needle } from "./match";
import type { FindDoc } from "./text";

export interface FindMatch {
  readonly entryId: string;
  readonly part: number;
  /** Its place among the matches of its part, from the part's start. */
  readonly ordinal: number;
}

export interface FindResult {
  readonly matches: ReadonlyArray<FindMatch>;
  /** More than MATCH_CAP matched, and counting stopped there. */
  readonly capped: boolean;
}

const lowered = new WeakMap<ReadonlyArray<string>, string[]>();
const lowerOf = (segments: ReadonlyArray<string>): string[] => {
  let held = lowered.get(segments);
  if (held === undefined) {
    held = segments.map(s => s.toLowerCase());
    lowered.set(segments, held);
  }
  return held;
};

export function searchDocs(docs: ReadonlyArray<FindDoc>, needle: Needle, tools: boolean): FindResult {
  const matches: FindMatch[] = [];
  const lower = needle.kind === "literal" && !needle.matchCase;
  for (const doc of docs) {
    for (const part of doc.parts) {
      if (part.tool && !tools) continue;
      let ordinal = 0;
      const lows = lower ? lowerOf(part.segments) : undefined;
      for (let i = 0; i < part.segments.length; i++) {
        for (const _span of spansIn(part.segments[i]!, needle, MATCH_CAP + 1 - matches.length, lows?.[i])) {
          matches.push({ entryId: doc.entryId, part: part.part, ordinal: ordinal++ });
        }
        if (matches.length > MATCH_CAP) return { matches: matches.slice(0, MATCH_CAP), capped: true };
      }
    }
  }
  return { matches, capped: false };
}
