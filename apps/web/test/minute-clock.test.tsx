// SPDX-License-Identifier: AGPL-3.0-only
// The minute clock a reader holds only while it needs minutes: a reader that is
// off is drawn once however long passes before React checks it again.
import { cleanup, render } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { useMinuteClockWhen } from "../src/components/status/useMinuteClock.js";

afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

describe("useMinuteClockWhen", () => {
  it("draws a reader that is off once, though more than a second passes between its draw and React's check after it", () => {
    vi.useFakeTimers({ now: Date.parse("2026-10-12T08:00:00.000Z"), toFake: ["Date"] });
    let draws = 0;
    function Reader() {
      useMinuteClockWhen(false);
      draws++;
      // A slow runner: the subscription's check comes after the clock has moved on.
      vi.setSystemTime(Date.now() + 2_000);
      return null;
    }
    render(<Reader />);
    expect(draws).toBe(1);
  });
});
