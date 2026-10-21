// SPDX-License-Identifier: AGPL-3.0-only
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { App } from "../src/App";
import { INSTALL } from "../src/links";

describe("the landing page", () => {
  it("copies the whole install command, not the short one it shows, and says so", async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, "clipboard", { value: { writeText }, configurable: true });
    render(<App />);
    const [button] = screen.getAllByRole("button", { name: `Copy ${INSTALL}` });
    fireEvent.click(button!);
    await waitFor(() => expect(writeText).toHaveBeenCalledWith(INSTALL));
    await waitFor(() => expect(screen.getAllByRole("button", { name: "Copied" }).length).toBe(1));
  });

  it("switches the Slate example when a tab is pressed", () => {
    render(<App />);
    const tab = screen.getByRole("tab", { name: "Deploy setup" });
    fireEvent.click(tab);
    expect(tab.getAttribute("aria-selected")).toBe("true");
    expect(screen.getByText(/never sees/)).toBeTruthy();
  });

  it("carries no em-dash anywhere a visitor reads", () => {
    const { container } = render(<App />);
    expect(container.textContent).not.toContain("—");
  });
});
