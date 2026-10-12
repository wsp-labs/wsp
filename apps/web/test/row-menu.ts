// SPDX-License-Identifier: AGPL-3.0-only
// An Activity row's acts live in its menu: open it, read what it offers, run one.
import { act, fireEvent, waitFor } from "@testing-library/react";
import { expect } from "vitest";
import { useContextMenuStore } from "../src/actions/contextMenu.js";

/** Opens a row's menu and answers the ids it offers, leaving it open. */
export async function menuOf(row: Element): Promise<string[]> {
  fireEvent.click(row.querySelector("[data-act=menu]")!);
  await waitFor(() => expect(useContextMenuStore.getState().menu).not.toBeNull());
  return useContextMenuStore.getState().menu!.items.map(i => i.id);
}

/** Runs one act from a row's menu. */
export async function fromMenu(row: Element, id: string): Promise<void> {
  expect(await menuOf(row)).toContain(id);
  act(() => useContextMenuStore.getState().choose(id));
}

export const closeMenu = (): void => act(() => useContextMenuStore.getState().choose(null));
