// SPDX-License-Identifier: AGPL-3.0-only
// The close chords, as the shell reads a press before any page does. Mod+w
// closes the focused pane's tab, which the app's page answers from its own
// keybindings; mod+shift+w closes the window. A browser tab's page is a
// webContents of its own whose keys never reach the app's page, so the shell
// takes both chords there and hands mod+w on.
import { holdsMod, type ShellKeyInput } from "./zoom.js";

export type CloseChord = "tab" | "window";

export function closeChordOf(input: ShellKeyInput, platform: NodeJS.Platform): CloseChord | undefined {
  if (input.type !== "keyDown" || input.alt || input.key.toLowerCase() !== "w" || !holdsMod(input, platform)) return undefined;
  return input.shift ? "window" : "tab";
}
