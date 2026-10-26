// SPDX-License-Identifier: AGPL-3.0-only
// Writes cases.json: what node's own object semantics give a thread's record on each road that writes it, for the
// Rust tests that keep the record's keys in the order the TS inserts them.
//
//   node gen.mjs > cases.json
//
// Each road is the TS expression, copied:
//   mark    machines/boot.ts:932-937    { ...base, ...stamps }, then delete every stamp left undefined
//   create  threads/sessions.ts:831     the record a thread's first turn writes
//   launch  threads/sessions.ts:832-838 clouds assigned in place, limitResume deleted, filesIn replaced
//   access  threads/sessions.ts:1044    { ...record, permissionMode }
//   end     threads/transcripts.ts:607  { ...record, ended }
//   worked  threads/transcripts.ts:623  { ...record, worked: true }
//   rewind  threads/sessions.ts:1379-82 held ?? { workspaceId, harness }, cutOwed deleted, rewound set
// A stamp or input of null stands for the TS's undefined.

const base = { workspaceId: "w1", harness: "claude" };
const cases = [];
const bytes = record => JSON.stringify(record);
const add = (road, name, before, input, after) => cases.push({ road, name, before, input, after: bytes(after) });
const undef = value => (value === null ? undefined : value);

const mark = (before, stamps) => {
  const held = { ...before };
  const real = Object.fromEntries(Object.entries(stamps).map(([k, v]) => [k, undef(v)]));
  const next = { ...held, ...real };
  for (const key of Object.keys(real)) if (real[key] === undefined) delete next[key];
  return next;
};
const worked = { ...base, worked: true };
const ended = { ...base, worked: true, ended: "completed" };
const marks = [
  ["readAt", base, { readAt: 1700 }],
  ["readAt after worked", worked, { readAt: 1700 }],
  ["settledAt and readAt", ended, { readAt: 1800, settledAt: 1800 }],
  ["settle names the thread", ended, { readAt: 1800, settledAt: 1800, settleNamedAt: 1800 }],
  ["settleNamedAt alone", ended, { settleNamedAt: 1800 }],
  ["pinnedAt", ended, { pinnedAt: 7 }],
  ["order", ended, { order: 2.5 }],
  ["foldedAt", ended, { foldedAt: 1900 }],
  ["snoozedUntil and readAt", ended, { snoozedUntil: 9000, readAt: 1900 }],
  ["section", ended, { section: { name: "done", whileState: "idle" } }],
  ["limitResume", ended, { limitResume: { at: 5000, turnId: "turn-1" } }],
  ["every pick of one mark", ended, { pinnedAt: 7, order: 2, foldedAt: 1900, snoozedUntil: 9000, readAt: 1900, section: { name: "working", whileState: "idle" } }],
  ["a later key already held", { ...base, snoozedUntil: 3 }, { settledAt: 4 }],
  ["a later key held, two stamps", { ...base, snoozedUntil: 3, worked: false }, { readAt: 5, settledAt: 5 }],
  ["a held key keeps its place", { ...base, readAt: 1, settledAt: 2, snoozedUntil: 3, worked: true }, { settledAt: 9 }],
  ["a taken key leaves the others in place", { ...base, readAt: 1, settledAt: 2, snoozedUntil: 3, worked: true }, { readAt: 10, settledAt: null }],
  ["restore", { ...base, readAt: 1, settledAt: 2, snoozedUntil: 3, worked: true }, { readAt: 11, settledAt: null }],
  ["snooze ended", { ...base, snoozedUntil: 3, worked: true }, { snoozedUntil: 4000 }],
  ["resume disarmed", { ...base, limitResume: { at: 1, turnId: "t" }, worked: true }, { limitResume: null }],
  ["a stamp on a thread with no record yet", base, { settledAt: 4, readAt: 4 }],
  ["a tree then a stamp", { ...base, parentThreadId: "p", rootThreadId: "r", permissionMode: "plan", worked: true }, { foldedAt: 3 }],
];
for (const [name, before, stamps] of marks) add("mark", name, before, stamps, mark(before, stamps));

const forked = { threadId: "t0", turnId: "turn-0", title: "Earlier" };
const create = i => ({
  workspaceId: i.workspaceId,
  harness: i.harness,
  ...i.tree,
  ...(i.permissionMode !== undefined ? { permissionMode: i.permissionMode } : {}),
  ...(i.replaces !== undefined ? { replaces: i.replaces } : {}),
  ...(i.forkedFrom !== undefined ? { forkedFrom: i.forkedFrom } : {}),
  worked: i.worked,
});
const creates = [
  ["a thread under a lead, at plan, restarting one", { ...base, tree: { parentThreadId: "p", rootThreadId: "r" }, permissionMode: "plan", replaces: "t9", worked: false }],
  ["a fork", { ...base, tree: { parentThreadId: "p", rootThreadId: "r" }, forkedFrom: forked, worked: false }],
  ["every pick", { ...base, tree: { parentThreadId: "p", rootThreadId: "r" }, permissionMode: "acceptEdits", replaces: "t9", forkedFrom: forked, worked: true }],
  ["a root at a mode", { ...base, tree: {}, permissionMode: "plan", worked: false }],
  ["a root, nothing picked", { ...base, tree: {}, worked: false }],
  ["a later turn's tree", { ...base, tree: { rootThreadId: "r" }, permissionMode: "default", worked: true }],
];
for (const [name, input] of creates) add("create", name, null, input, create(input));

const launch = (before, input) => {
  const record = before ?? create(input.create);
  const thread = Object.assign(record, input.clouds !== undefined ? { clouds: input.clouds } : {});
  if (thread.limitResume !== undefined) delete thread.limitResume;
  if (input.filesFolder !== undefined && !(thread.filesIn ?? []).includes(input.filesFolder)) return { ...thread, filesIn: [...(thread.filesIn ?? []), input.filesFolder] };
  return thread;
};
const tree = { parentThreadId: "p", rootThreadId: "r" };
const launches = [
  ["a first turn with clouds and a files folder", null, { create: { ...base, tree, permissionMode: "plan", worked: false }, clouds: ["c1"], filesFolder: "/f" }],
  ["a first turn with a files folder", null, { create: { ...base, tree: {}, worked: false }, filesFolder: "/f" }],
  ["clouds after worked", { ...base, worked: true }, { clouds: ["c1", "c2"] }],
  ["clouds after ended", { ...base, ended: "failed" }, { clouds: ["c1"] }],
  ["clouds held keep their place", { ...base, clouds: ["a"], worked: true }, { clouds: ["b"] }],
  ["a newer turn leaves nothing to resume", { ...base, limitResume: { at: 5, turnId: "t" }, worked: true }, { clouds: ["c"], filesFolder: "/f" }],
  ["a second folder joins the first", { ...base, filesIn: ["/a"], worked: true }, { filesFolder: "/b" }],
  ["a folder already kept", { ...base, filesIn: ["/a"], worked: true }, { filesFolder: "/a" }],
  ["files folder after worked and ended", ended, { filesFolder: "/f" }],
];
for (const [name, before, input] of launches) add("launch", name, before, input, launch(before === null ? null : { ...before }, input));

const accesses = [
  ["a record that worked", worked],
  ["a record with a mode held", { ...base, permissionMode: "default", worked: true }],
  ["a record with a tree", { ...base, parentThreadId: "p", rootThreadId: "r", worked: true }],
  ["a thread with no record yet", base],
];
for (const [name, before] of accesses) add("access", name, before, { permissionMode: "plan" }, { ...before, permissionMode: "plan" });

const endings = [
  ["a record that worked", worked],
  ["a record that ended before", { ...base, worked: true, ended: "completed" }],
  ["a record with marks", { ...base, readAt: 1, settledAt: 2, worked: true }],
  ["a record that holds clouds", { ...base, worked: true, clouds: ["c1"] }],
];
for (const [name, before] of endings) add("end", name, before, { ended: "failed" }, { ...before, ended: "failed" });

const workeds = [
  ["a record that had not worked", { ...base, worked: false, ended: "completed" }],
  ["marks before it", { ...base, readAt: 1, worked: false }],
];
for (const [name, before] of workeds) add("worked", name, before, {}, { ...before, worked: true });

const rewind = (held, input) => {
  const record = held ?? { workspaceId: "w1", harness: "claude" };
  if (input.cutOwed) delete record.cutOwed;
  if (input.before !== undefined) record.rewound = { before: input.before, at: input.at };
  return record;
};
const rewinds = [
  ["a rewind that pays an owed cut", { ...base, cutOwed: "x", worked: true }, { cutOwed: true, before: "abc", at: 77 }],
  ["a rewind on a record that worked", worked, { cutOwed: false, before: "abc", at: 77 }],
  ["a rewind on a thread with no record yet", null, { cutOwed: true, before: "abc", at: 77 }],
  ["a second rewind keeps its place", { ...base, rewound: { before: "old", at: 1 }, worked: true }, { cutOwed: false, before: "abc", at: 77 }],
  ["a rewind with marks held", { ...base, readAt: 1, settledAt: 2, snoozedUntil: 3, worked: true, ended: "completed" }, { cutOwed: false, before: "abc", at: 77 }],
];
for (const [name, before, input] of rewinds) add("rewind", name, before, input, rewind(before === null ? null : { ...before }, input));

process.stdout.write("[\n" + cases.map(c => "  " + JSON.stringify(c)).join(",\n") + "\n]\n");
