// SPDX-License-Identifier: AGPL-3.0-only
import { spawn } from "node:child_process";
import { lstatSync, mkdirSync, readFileSync, symlinkSync, truncateSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { describe, expect, it } from "vitest";
import { MCP_ID_PREFIX, PLACE_FOUND_END, placeDaemonPaths, placeProvisionPaths } from "@wsp/protocol";
import { MCP_SERVERS_JSON } from "@wsp/catalog";
import { serverDigest } from "@wsp/engine";
import { PLACE_SERVERS_PAST_CAP, PLACE_SERVERS_UNREAD, placeFound, placeServersUnreadLine, SERVERS_READ_MAX_BYTES } from "../src/place-report.js";
import { fakeRunner, fifoWriter, made, shWithSha256sum, stopGroup, sweepPlace, tmp } from "./places-fixture.js";

/** Claude Code's own file on the box: one server wsp merged in and one of the person's own. */
const CLAUDE_TEXT = `${JSON.stringify({ mcpServers: { context7: { command: "npx", args: ["-y", "context7"] }, mine: { command: "/usr/local/bin/mine" } } }, null, 2)}\n`;

/** A home whose list names the one Claude server wsp merged, with the digest of the entry it left. */
function homeWithServer(name: string): string {
  const home = tmp(name);
  const at = placeProvisionPaths(home);
  mkdirSync(at.dir, { recursive: true });
  const digest = serverDigest(MCP_SERVERS_JSON.entryOf(CLAUDE_TEXT, "context7"))!;
  writeFileSync(at.landed, `${MCP_ID_PREFIX}claude/context7\t${digest}\t${digest}\n`);
  return home;
}

const leave = (home: string) => sweepPlace({ home, manager: undefined, run: fakeRunner().run, sh: shWithSha256sum() });

describe("the leave at the box reads an agent's own config file as a plain file only, and no further than a cap", () => {
  it("takes wsp's server out of a plain file and keeps the person's", async () => {
    const home = homeWithServer("leave-servers-plain");
    const claude = join(home, ".claude.json");
    writeFileSync(claude, CLAUDE_TEXT);
    const swept = await leave(home);
    expect(swept.removed).toContain(`context7 (out of ${claude})`);
    expect(Object.keys((JSON.parse(readFileSync(claude, "utf8")) as { mcpServers: object }).mcpServers)).toEqual(["mine"]);
  });

  it("keeps a fifo at the agent's config path unread and says so, without waiting for a writer", async () => {
    const home = homeWithServer("leave-servers-fifo");
    const claude = join(home, ".claude.json");
    await made(claude);
    // A late writer stands in for nobody ever writing: a read that waits on the fifo gets its bytes three seconds on
    // and takes the server, where one that never waits has said what it kept long before.
    const late = fifoWriter(claude, CLAUDE_TEXT, 3000);
    try {
      const swept = await leave(home);
      expect(swept.kept).toContain(placeServersUnreadLine(claude, PLACE_SERVERS_UNREAD));
      expect(swept.removed.filter(said => said.includes(".claude.json"))).toEqual([]);
      expect(lstatSync(claude).isFIFO()).toBe(true);
    } finally {
      late.stop();
    }
  });

  it("keeps a link to a device that never ends unread", async () => {
    const home = homeWithServer("leave-servers-zero");
    const claude = join(home, ".claude.json");
    symlinkSync("/dev/zero", claude);
    const swept = await leave(home);
    expect(swept.kept).toContain(placeServersUnreadLine(claude, PLACE_SERVERS_UNREAD));
    expect(lstatSync(claude).isSymbolicLink()).toBe(true);
  });

  it("opens no link at the agent's config path, whatever it points at", async () => {
    // A writer's open of a fifo waits until something opens its read end, so its return says the leave opened it.
    const far = join(tmp("leave-servers-far"), "fifo");
    await made(far);
    const home = homeWithServer("leave-servers-pointed");
    const claude = join(home, ".claude.json");
    symlinkSync(far, claude);
    const writer = fifoWriter(far, CLAUDE_TEXT);
    try {
      const swept = await leave(home);
      await new Promise(resolve => setTimeout(resolve, 200));
      expect(writer.opened()).toBe("");
      expect(swept.kept).toContain(placeServersUnreadLine(claude, PLACE_SERVERS_UNREAD));
      expect(swept.removed.filter(said => said.includes(".claude.json"))).toEqual([]);
    } finally {
      writer.stop();
    }
  });

  it("keeps a file past the cap unread", async () => {
    const home = homeWithServer("leave-servers-big");
    const claude = join(home, ".claude.json");
    writeFileSync(claude, CLAUDE_TEXT);
    truncateSync(claude, SERVERS_READ_MAX_BYTES + 1);
    const swept = await leave(home);
    expect(swept.kept).toContain(placeServersUnreadLine(claude, PLACE_SERVERS_PAST_CAP));
  });
});

/** Puts a plain file and a fifo at one path by turns for as long as it runs, each a second name renamed over the
 * other, so the path is never empty and flips between the two as fast as a rename goes. */
const SWAP = "const [p, f, q] = process.argv.slice(1); const fs = require('node:fs'); for (;;) { fs.linkSync(f, p + '.f'); fs.renameSync(p + '.f', p); fs.linkSync(q, p + '.q'); fs.renameSync(p + '.q', p); }";

describe("the leave's read of the add's record", () => {
  it("never waits on a fifo swapped in at the record's path between a look and the read", async () => {
    const home = tmp("leave-found-race");
    const path = placeDaemonPaths(home).placeFound;
    mkdirSync(dirname(path), { recursive: true });
    const record = join(dirname(path), "record");
    const fifo = join(dirname(path), "fifo");
    writeFileSync(record, `/x\0${PLACE_FOUND_END}\0`);
    await made(fifo);
    const swapper = spawn(process.execPath, ["-e", SWAP, path, record, fifo], { stdio: "ignore", detached: true });
    try {
      // A read that looks at the path and then opens it by name waits for good on the fifo that came in between, so
      // this loop ending is the proof; the answers it gives are the record's or nothing.
      const answers = new Set<string>();
      for (const end = Date.now() + 2000; Date.now() < end; ) answers.add(JSON.stringify([...(placeFound(home) ?? [])]));
      expect([...answers].filter(said => said !== "[]" && said !== '["/x"]')).toEqual([]);
    } finally {
      stopGroup(swapper);
    }
  });
});
