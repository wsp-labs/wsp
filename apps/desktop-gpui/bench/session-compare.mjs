// SPDX-License-Identifier: AGPL-3.0-only
// node session-compare.mjs <before.json> <after.json>: two fixture hosts started with BENCH_SESSION, still running (stop.sh
// deletes what this reads) and with no thread opened yet (opening one moves its read mark), compared thread by thread:
// the count of each kind of event, every event once ids, times, commit ids and temp folders are set aside, the folder
// trees each changed-files card and checkpoint names, the sidebar row, read mark and kept screenshots, and the files the
// turns left in the folder. Made to show that the session threads written into the state are the ones the host
// recorded when the stub played every turn; exits 1 on any difference.
import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { DatabaseSync } from "node:sqlite";
import { turnsOf } from "./content.mjs";

if (process.argv.length !== 4) {
  console.error("usage: node session-compare.mjs <before.json> <after.json>");
  process.exit(2);
}
const sides = process.argv.slice(2).map(path => {
  const fixture = JSON.parse(readFileSync(path, "utf8"));
  const home = dirname(dirname(fixture.state));
  const db = new DatabaseSync(join(dirname(fixture.state), "state.db"), { readOnly: true });
  const workspace = JSON.parse(db.prepare("select json from events where thread = ? limit 1").get(fixture.threads["session-1"]).json).workspaceId;
  const doc = collection => JSON.parse(db.prepare("select json from collections where collection = ? and id = ?").get(collection, workspace)?.json ?? "{}");
  const trees = commits => spawnSync("git", ["rev-parse", ...commits.map(c => `${c}^{tree}`)], { cwd: join(home, "bench"), env: { PATH: process.env.PATH }, encoding: "utf8" }).stdout.trim().split("\n");
  const git = (args, input) => spawnSync("git", args, { cwd: join(home, "bench"), env: { PATH: process.env.PATH }, input, encoding: "utf8" }).stdout;
  const untracked = git(["ls-files", "-z", "--others", "--exclude-standard"]).split("\0").filter(Boolean).sort();
  const hashes = git(["hash-object", "--stdin-paths"], untracked.join("\n")).trim().split("\n");
  return { fixture, home, db, sessions: doc("sessions"), kept: doc("attachment-keys"), trees, left: untracked.map((path, i) => `${path} ${hashes[i]}`).join("\n") };
});

const UUID = /[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}/g;
const sorted = v => (Array.isArray(v) ? v.map(sorted) : v && typeof v === "object" ? Object.fromEntries(Object.keys(v).sort().map(k => [k, sorted(v[k])])) : v);
const kindOf = e => (e.type === "session.delta" ? `delta.${e.kind}${e.parentToolUseId ? ".sub" : ""}` : e.type === "session.subagent" ? `subagent.${e.state}` : `${e.type}${e.attachments ? ".images" : ""}`);
/** Each event as a line with its ids numbered in the order they first appear, and no time, place, commit or folder. */
function lines(events, home) {
  const ids = new Map();
  return events.map(e => {
    const { at: _at, pos: _pos, ...rest } = e;
    if (rest.result) delete rest.result.durationMs;
    const said = JSON.stringify(rest).split(home).join("home").replace(UUID, id => (ids.has(id) ? ids.get(id) : ids.set(id, `id${ids.size}`).get(id)));
    return JSON.stringify(sorted(JSON.parse(said))).replace(/\b[0-9a-f]{40}\b/g, "commit").replace(/ws_[0-9a-f]{8}/g, "ws");
  });
}
/** By turn, in the order the turns ran: the host writes a turn's checkpoint and card after its end, sometimes after the next turn has begun. */
const byTurn = (events, said) => {
  const turns = new Map();
  events.forEach((e, i) => (turns.get(e.turnId) ?? turns.set(e.turnId, []).get(e.turnId)).push(said[i]));
  return [...turns.values()].flatMap(turn => turn.sort());
};
const row = (side, id) => {
  const { readAt, clouds: _clouds, ...entry } = side.sessions.threads[id];
  const { id: _id, workspaceId: _ws, threadId: _thread, claudeSessionId: _sid, turnId: _turn, startedAt: _start, endedAt, cwd: _cwd, ...rest } = side.sessions.sessions.find(r => r.threadId === id);
  return JSON.stringify(sorted({ ...rest, ...entry, read: readAt >= endedAt }));
};

const names = Object.keys(sides[0].fixture.threads).filter(name => name.startsWith("session-")).sort((a, b) => a.localeCompare(b, "en", { numeric: true }));
const kinds = new Set();
const counts = names.map(name =>
  sides.map(side => {
    const n = {};
    for (const r of side.db.prepare("select json from events where thread = ? order by pos").all(side.fixture.threads[name])) {
      const k = kindOf(JSON.parse(r.json));
      kinds.add(k);
      n[k] = (n[k] ?? 0) + 1;
    }
    return n;
  }),
);
const columns = [...kinds].sort();
console.log(`thread\t${columns.join("\t")}\t(before / after where they differ)`);
let differ = 0;
const tally = { events: 0, eventsDiffer: 0, outOfPlace: 0, cards: 0, checkpoints: 0, rows: 0, images: 0 };
names.forEach((name, i) => {
  const [a, b] = counts[i];
  console.log(`${name}\t${columns.map(k => ((a[k] ?? 0) === (b[k] ?? 0) ? String(a[k] ?? 0) : (differ++, `${a[k] ?? 0} / ${b[k] ?? 0}`))).join("\t")}`);
  const [ea, eb] = sides.map(side => side.db.prepare("select json from events where thread = ? order by pos").all(side.fixture.threads[name]).map(r => JSON.parse(r.json)));
  const [la, lb] = [lines(ea, sides[0].home), lines(eb, sides[1].home)];
  const [ta, tb] = [byTurn(ea, la), byTurn(eb, lb)];
  tally.events += ta.length;
  for (let k = 0; k < Math.max(ta.length, tb.length); k++) if (ta[k] !== tb[k]) tally.eventsDiffer++;
  for (let k = 0; k < la.length; k++) if (la[k] !== lb[k]) tally.outOfPlace++;
  const [ca, cb] = [ea, eb].map(events => events.find(e => e.type === "session.changes"));
  if (ca && cb && sides[0].trees([ca.from, ca.to]).join() === sides[1].trees([cb.from, cb.to]).join()) tally.cards++;
  const [ka, kb] = [ea, eb].map((events, s) => sides[s].trees(events.filter(e => e.type === "session.checkpoint").map(e => e.ref)));
  if (ka.length === turnsOf(name).length && ka.join() === kb.join()) tally.checkpoints++;
  const [ida, idb] = sides.map(side => side.fixture.threads[name]);
  if (row(sides[0], ida) === row(sides[1], idb)) tally.rows++;
  const sizes = (side, id) => side.db.prepare("select length(bytes) n from blobs where collection = 'attachments' and id like ?").all(`${id}.%`).map(r => r.n).sort().join(",");
  if (sizes(sides[0], ida) === sizes(sides[1], idb) && (sides[0].kept.threads[ida] ?? []).length === (sides[1].kept.threads[idb] ?? []).length) tally.images++;
});
const n = names.length;
console.log(`events: ${tally.events}, ${tally.eventsDiffer} differ turn by turn; ${tally.outOfPlace} stand elsewhere in the thread's order`);
console.log(`cards whose folder trees match: ${tally.cards} of ${n}; threads whose checkpoints' trees match: ${tally.checkpoints} of ${n}; rows, read marks and thread records: ${tally.rows} of ${n}; kept screenshots: ${tally.images} of ${n}`);
console.log(`the folder as the turns left it: ${sides[0].left === sides[1].left ? "the same" : "differs"}, ${sides[0].left.split("\n").length} and ${sides[1].left.split("\n").length} files`);
const same = sides[0].left === sides[1].left && differ === 0 && tally.eventsDiffer === 0 && tally.cards === n && tally.checkpoints === n && tally.rows === n && tally.images === n;
console.log(same ? "the session threads match" : "the session threads differ");
process.exit(same ? 0 : 1);
