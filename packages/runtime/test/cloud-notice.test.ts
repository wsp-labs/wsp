// SPDX-License-Identifier: AGPL-3.0-only
// A cloud added or removed after a thread started reaches that thread as one line ahead of its next message, once per
// change, since an agent keeps the tool words and the computers it read at its start. The place door is made to
// answer a Boat row after the lead's first turn, as the app's Add Boat makes the host do.
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { cloudAddedLine, cloudRemovedLine, type PlaceView } from "@wsp/protocol";
import { leadAndBox, NO_START } from "./box-fixture.js";

let root: string | undefined;
afterEach(() => {
  if (root !== undefined) rmSync(root, { recursive: true, force: true });
  root = undefined;
});

describe("a cloud added or removed after a thread started", () => {
  it("is one line ahead of that thread's next message, once, and never ahead of a thread that started after it", async () => {
    root = mkdtempSync(join(tmpdir(), "wsp-cloud-notice-"));
    const { rt, starts, folder, threadId, turn } = await leadAndBox(root);
    starts[0]!.answer("nothing to do");
    await turn.finished;
    const door = rt.places!;
    const rows = door.rows.bind(door);
    let boat = true;
    door.rows = async (): Promise<PlaceView[]> => [...(await rows()), ...(boat ? [{ id: "box", kind: "provider", name: "box", default: false } as PlaceView] : [])];
    const send = async (prompt: string, thread: string | undefined = threadId): Promise<string> => {
      await (await rt.sessions.start(folder.id, { ...(thread !== undefined ? { thread } : {}), prompt, harness: "claude" })).finished;
      return starts.at(-1)!.o.prompt;
    };
    // A send whose turn never launches told the agent nothing, so the next one still carries the line.
    await expect(send(`${NO_START}x`)).rejects.toThrow();
    expect(await send("can you spawn a cloud thread to run the test suite?")).toBe(`${cloudAddedLine("Boat")}\n\ncan you spawn a cloud thread to run the test suite?`);
    expect(await send("and again")).toBe("and again");
    expect(await send("a new thread", undefined)).toBe("a new thread");
    boat = false;
    expect(await send("is it still there?")).toBe(`${cloudRemovedLine("Boat")}\n\nis it still there?`);
    expect(await send("once more")).toBe("once more");
    expect(cloudAddedLine("Boat")).toBe("Boat was added since this thread started: run with on boat starts a sub-thread there.");
  });
});
