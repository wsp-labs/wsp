// SPDX-License-Identifier: AGPL-3.0-only
import { describe, expect, it } from "vitest";
import { ARM_MS } from "../src/components/arm.js";
import { ESCALATE_MS, IDLE, offered, press, settle, termed } from "../src/components/procs/kill.js";

describe("kill flow", () => {
  it("first press arms, a second within two seconds sends, and then nothing is offered until the daemon answers", () => {
    const armed = press(IDLE, 1000);
    expect(armed).toEqual({ state: { step: "armed", at: 1000, send: "end" }, send: undefined });
    expect(offered(armed.state, 1500)).toEqual({ send: "end", confirm: true });
    const sent = press(armed.state, 1000 + ARM_MS - 1);
    expect(sent).toEqual({ state: { step: "sent" }, send: "end" });
    expect(offered(sent.state, 99_999)).toBeNull();
    expect(press(sent.state, 99_999)).toEqual({ state: { step: "sent" }, send: undefined });
  });

  it("an arm nobody confirms lapses after two seconds", () => {
    const armed = press(IDLE, 1000).state;
    expect(settle(armed, 1000 + ARM_MS - 1)).toBe(armed);
    expect(settle(armed, 1000 + ARM_MS)).toEqual(IDLE);
    // A press after the lapse arms again rather than sending.
    expect(press(armed, 1000 + ARM_MS)).toEqual({ state: { step: "armed", at: 1000 + ARM_MS, send: "end" }, send: undefined });
  });

  it("idle offers the kill, unconfirmed", () => {
    expect(offered(IDLE, 0)).toEqual({ send: "end", confirm: false });
    expect(settle(IDLE, 99_999)).toBe(IDLE);
  });

  it("after a daemon too old for the end took TERM, KILL is offered once five seconds pass, two presses again", () => {
    const old = termed(1000);
    expect(offered(old, 1000 + ESCALATE_MS - 1)).toBeNull();
    expect(offered(old, 1000 + ESCALATE_MS)).toEqual({ send: "force", confirm: false });
    const armed = press(old, 1000 + ESCALATE_MS);
    expect(armed.send).toBeUndefined();
    expect(press(armed.state, 1000 + ESCALATE_MS + 1)).toEqual({ state: { step: "sent" }, send: "force" });
    // An unconfirmed force arm lapses back to the offer, not to a fresh end.
    const lapsed = settle(armed.state, 1000 + ESCALATE_MS + ARM_MS);
    expect(offered(lapsed, 1000 + ESCALATE_MS + ARM_MS)).toEqual({ send: "force", confirm: false });
  });
});
