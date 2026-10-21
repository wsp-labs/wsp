// SPDX-License-Identifier: AGPL-3.0-only
// What a rewind says before the click and when the host says no: one home for
// the words, so the dialog, the refusal and the tests read the same sentence.
import { describe, expect, it } from "vitest";
import { rewindBesideLine, rewindChildrenLine, rewindNoAnchorLine, rewindNote } from "../src/index.js";

describe("the words a rewind says", () => {
  it("names what goes before the click, and that undo brings the files back and never the conversation", () => {
    expect(rewindNote({ turns: 3, files: true, cutsConversation: true, agent: "Claude Code" })).toBe(
      "The 3 turns after this reply leave the conversation, and the files go back to how they stood at this reply. If another thread has worked in this folder since this reply, the files stay as they are. Undo rewind puts them back until the next turn in this folder ends; the conversation does not come back.",
    );
    expect(rewindNote({ turns: 1, files: false, cutsConversation: true, agent: "Codex" })).toBe("The turn after this reply leaves the conversation; the files stay as they are.");
    // An agent that keeps its own history: only the files move, and the note says why the turns stay.
    expect(rewindNote({ turns: 2, files: true, cutsConversation: false, agent: "Cursor" })).toBe(
      "Cursor keeps its own history, so the conversation stays and the files go back to how they stood at this reply. If another thread has worked in this folder since this reply, the files stay as they are. Undo rewind puts them back until the next turn in this folder ends.",
    );
  });

  it("refuses in one sentence each, naming what to do", () => {
    expect(rewindChildrenLine(["fix the port list", "write the migration"])).toBe(
      "stop the threads under this one first (fix the port list, write the migration); a rewind never stops them for you",
    );
    expect(rewindBesideLine("move the pricing table")).toBe(
      "another thread in this folder is working (move the pricing table), and its files would go back too; let its turn end or stop it first",
    );
    expect(rewindNoAnchorLine("Claude Code")).toBe("Claude Code left no point in that reply to cut its conversation at, so only the files can go back");
  });
});
