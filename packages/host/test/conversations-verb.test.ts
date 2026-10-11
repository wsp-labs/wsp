// SPDX-License-Identifier: AGPL-3.0-only
// The lines `wsp conversations` prints: none in the folder only where every agent's store answered, and a store that
// was not read speaking for itself rather than under a line that says there are none.
import { describe, expect, it } from "vitest";
import { CONVERSATIONS_AGAIN_FIX, conversationsFailedLine, type ConversationsHeld } from "@wsp/protocol";
import { conversationsLines } from "../src/verbs/conversations.js";

const failed: ConversationsHeld = { agent: "claude", said: conversationsFailedLine("Claude Code", "at most 64 entries, got 74"), fix: CONVERSATIONS_AGAIN_FIX, why: "at most 64 entries, got 74" };

describe("wsp conversations", () => {
  it("says there are none in the folder where every store answered with none", () => {
    expect(conversationsLines({ rows: [], held: [] })).toEqual(["no Claude Code or Codex conversations in this folder"]);
  });

  it("leaves the held line to speak where no row came back and a store was not read", () => {
    expect(conversationsLines({ rows: [], held: [failed] })).toEqual(["Claude Code's conversations could not be read: at most 64 entries, got 74. Ask again in a moment."]);
  });

  it("prints the rows that were read above the held line", () => {
    const lines = conversationsLines({ rows: [{ agent: "codex", id: "01a12813-cd12-7a12-9b13-e76892906ff0", title: "lab terminal thread", cwd: "/work/acme/lab", lastAt: 0, origin: "terminal", live: false }], held: [failed] });
    expect(lines.at(-1)).toBe(`${failed.said}. ${failed.fix}`);
    expect(lines.some(l => l.includes("lab terminal thread"))).toBe(true);
    expect(lines.some(l => l.startsWith("no "))).toBe(false);
  });
});
