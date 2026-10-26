// SPDX-License-Identifier: AGPL-3.0-only
// The new and changed slate pieces, each in a slate the size an agent writes: stat rows of two, three and four tiles
// with long labels and windows, a decision, a progress, a summary-first output, a facts line, choices and a table.
import type { SlateJson } from "@wsp/protocol/slate";

const LONG_LABELS = ["Requests served from the edge cache", "Median time to first byte at the origin", "Builds that passed on the first try", "Spend on cloud machines"];
const LONG_WINDOWS = ["over the last 24 hours across every region", "this week against the week before it", "since the release branch was cut", "this calendar month, tax included"];
const stats = (n: number, long: boolean): string =>
  `<slate><column><stats id="stats">${Array.from({ length: n }, (_, i) => {
    const label = long ? LONG_LABELS[i]! : ["Passed", "Spent", "Open", "Built"][i]!;
    const window = long ? LONG_WINDOWS[i]! : ["this run", "this thread", "right now", "today"][i]!;
    return i === 0 ? `<stat label="${label}" value={209} max={214} window="${window}" tone="good" />` : i === 1 ? `<stat label="${label}" value={4.82} format="usd" window="${window}" />` : i === 2 ? `<stat label="${label}" value={3} window="${window}" tone="warning" />` : `<stat label="${label}" value={1268000} format="bytes" window="${window}" />`;
  }).join("")}</stats></column></slate>`;

const VITEST = [
  " RUN  v2.1.9 /home/maya/acme",
  "",
  " ❯ |node| src/checkout.test.ts (3 tests | 1 failed | 1 skipped) 9ms",
  "   × checkout > rounds a refund 7ms",
  "     → expected 0.30000000000000004 to be 0.3 // Object.is equality",
  "",
  " FAIL |node|  src/checkout.test.ts > checkout > rounds a refund",
  "AssertionError: expected 0.30000000000000004 to be 0.3 // Object.is equality",
  "",
  " Test Files  1 failed (1)",
  "      Tests  1 failed | 1 passed | 1 skipped (3)",
  "   Duration  336ms (transform 15ms, setup 20ms, collect 6ms, tests 9ms, environment 0ms, prepare 59ms)",
].join("\n");

export const GLANCE_CASES: Readonly<Record<string, { text: string; values?: Record<string, SlateJson> }>> = {
  "glance-stats-2": { text: stats(2, true) },
  "glance-stats-3": { text: stats(3, true) },
  "glance-stats-4": { text: stats(4, true) },
  "glance-stats-3-short": { text: stats(3, false) },
  "glance-decision": {
    text: `<slate><value name="ship" start="" /><column>
  <decision id="decision" label="Ship the refund fix tonight?" value={$ship} pick="now" onChange={send("Ship it?", $ship)}>
    <option value="now" label="Ship now" note="Checks are green" /><option value="morning" label="Wait for morning" note="Support is awake then" /><option value="no" label="Hold it" />
  </decision>
</column></slate>`,
  },
  "glance-progress": {
    text: `<slate><value name="screens" start={[{ n: "Settings", d: true }, { n: "Sidebar", d: true }, { n: "Thread view", d: true }, { n: "Usage page", d: false, w: "Chart tooltips left" }, { n: "Computers page", d: false, w: "Waits on the box row" }, { n: "Slate", d: true }]} /><column>
  <progress id="progress" label="screens in the app" items={$screens} name={item.n} done={item.d} note={item.w} />
</column></slate>`,
  },
  "glance-output": {
    text: `<slate><run name="tests" cmd="pnpm test" /><column><output id="output" label="Unit tests" run={$tests} /></column></slate>`,
    values: { tests: { state: "failed", exit: 1, out: VITEST, runs: 1 } },
  },
  "glance-facts": {
    text: `<slate><column><facts id="facts"><fact label="Branch" value="fix/refund-rounding" mono /><fact label="Commit" value="3f2a9c1" mono /><fact label="Author" value="Maya" /><fact label="Opened" value="2 hours ago" /></facts></column></slate>`,
  },
  "glance-facts-row": {
    text: `<slate><column><row id="head" wrap><status tone="warning">Investigating</status><facts id="facts" layout="line"><fact label="Started" value="14:10" emphasis="strong" /><fact label="Scope" value="Checkout API" /><fact label="Impact" value="Customers receive 502 during checkout" tone="bad" /></facts></row></column></slate>`,
  },
  "glance-facts-row-long": {
    text: `<slate><column><row id="head" wrap><status tone="warning">Investigating</status><facts id="facts" layout="line"><fact label="Started" value="14:10" emphasis="strong" /><fact label="Scope" value="Checkout API" /><fact label="Impact" value="Customers on the mobile app receive 502 during checkout after every retry" tone="bad" /></facts></row></column></slate>`,
  },
  "glance-choices": {
    text: `<slate><value name="pick" start="" /><column><choices id="choices" label="Which queue runs the jobs?" value={$pick} options={["Graphile Worker", "BullMQ", "A cron table"]} /></column></slate>`,
  },
  "glance-table": {
    text: `<slate><value name="rows" start={[{ t: "proration across a DST change", r: "ap-south-1", f: "24/24" }, { t: "refund applies partial tax", r: "eu-west-1", f: "1/3" }]} /><column><table id="table" items={$rows}><col title="Test" value={item.t} /><col title="Region" value={item.r} mono={true} /><col title="Failed" value={item.f} /></table></column></slate>`,
  },
};
