// SPDX-License-Identifier: AGPL-3.0-only
import { useEffect, useRef, useState } from "react";
import { fmtElapsed, fmtUptime } from "@wsp/protocol";
import { parseTimestampMs } from "../../sidebar/Sidebar.logic.js";

/** A turn's running time as a slot reads it: seconds in its first minute, then whole minutes, so it moves once a
 * minute: "42s", "17m", "1h 5m". */
export const fmtRunning = (ms: number): string => (ms < 60_000 ? fmtElapsed(ms) : fmtUptime(ms));

/** Every running clock on the page writes off one timer, which runs only while one is drawn. */
const writers = new Set<() => void>();
let timer: ReturnType<typeof setInterval> | undefined;

function tickWith(write: () => void): () => void {
  writers.add(write);
  timer ??= setInterval(() => {
    for (const each of writers) each();
  }, 1000);
  return () => {
    writers.delete(write);
    if (writers.size === 0 && timer !== undefined) {
      clearInterval(timer);
      timer = undefined;
    }
  };
}

/** How long the latest turn has run, written to its own node each second so the tick commits nothing to React, as
 * the transcript's working timer does. A turn with no start on its row yet, a send the runtime has not answered,
 * counts from the moment it was drawn. */
export function WorkingSince({ since, format = fmtElapsed }: { since: string | null; format?: (ms: number) => string }) {
  const node = useRef<HTMLSpanElement>(null);
  const [drawnAt] = useState(Date.now);
  const from = since === null ? drawnAt : parseTimestampMs(since);
  useEffect(() => {
    const write = () => {
      const text = format(Date.now() - from);
      if (node.current !== null && node.current.textContent !== text) node.current.textContent = text;
    };
    write();
    return tickWith(write);
  }, [from, format]);
  return (
    <span ref={node} aria-hidden className="tabular-nums">
      {format(Date.now() - from)}
    </span>
  );
}
