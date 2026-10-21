// SPDX-License-Identifier: AGPL-3.0-only
// The window key listeners that resolve a shortcut against the rules and the
// focus context, then run the command. Mounted once, inside the sidebar
// provider so the sidebar toggle is reachable.
import { useEffect, useRef } from "react";
import { isCommandPaletteOpen } from "../commandPaletteBus.js";
import { isTypingTarget } from "../keyOwners.js";
import { useSidebar } from "../components/ui/sidebar.js";
import { resolveShortcutCommand } from "../keybindings.js";
import type { ResolvedKeybindingsConfig } from "../keybindingTypes.js";
import { desktopBridge } from "../lib/desktopShell.js";
import { isPanelTabsFocused } from "../lib/panelFocus.js";
import { isPreviewFocused } from "../lib/previewFocus.js";
import { isTerminalFocused } from "../lib/terminalFocus.js";
import { threadFindOpen, threadOnScreen } from "../components/chat/find/store.js";
import { useSelectedWorkspaceId, useStore } from "../protocol/store.js";
import { runShellCommand, type ShellCommandTarget } from "./shellCommands.js";
import { useKeybindings } from "./useKeybindings.js";

/** Where focus is, read on every chord: the rules' when clauses name these. */
const focusContext = () => ({
  terminalFocus: isTerminalFocused(),
  previewFocus: isPreviewFocused(),
  panelTabsFocus: isPanelTabsFocused(),
  threadOpen: threadOnScreen() && !useStore.getState().settingsOpen,
  threadFindOpen: threadFindOpen() && !useStore.getState().settingsOpen,
});

export function KeybindingDispatcher({ keybindings: given }: { keybindings?: ResolvedKeybindingsConfig }) {
  const live = useKeybindings();
  const keybindings = given ?? live;
  const { toggleSidebar } = useSidebar();
  const workspaceId = useSelectedWorkspaceId();
  const target = useRef<ShellCommandTarget>({ workspaceId, toggleSidebar });
  target.current = { workspaceId, toggleSidebar };
  // Which body a chord is read in, held per render rather than looked up per keydown; the listeners are bound once.

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent): void => {
      if (event.defaultPrevented || event.isComposing) return;
      // Escape leaves the settings page, the one centre view with no row of its own to pick; a palette over it takes
      // the key first.
      if (event.key === "Escape" && useStore.getState().settingsOpen && !isCommandPaletteOpen()) {
        event.preventDefault();
        useStore.getState().closeSettings();
        return;
      }
      const command = resolveShortcutCommand(event, keybindings, { context: focusContext() });
      if (command === null) return;
      // An unchorded key inside an input is the user's text, whatever a rule says; a function key types nothing.
      const chorded = event.metaKey || event.ctrlKey || /^F\d+$/.test(event.key);
      if (!chorded && event.target instanceof Element && isTypingTarget(event.target)) return;
      event.preventDefault();
      event.stopPropagation();
      runShellCommand(command, target.current);
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [keybindings]);

  useEffect(() => {
    const bridge = desktopBridge();
    const setTerminalFocus = bridge?.setTerminalFocus;
    const onShellChord = bridge?.onShellChord;
    if (setTerminalFocus === undefined || onShellChord === undefined) return;
    // A focusout fires before the focus lands, so what has it is read on the turn after the move.
    let reported: boolean | null = null;
    let pending: ReturnType<typeof setTimeout> | null = null;
    const report = (): void => {
      pending = null;
      const focused = isTerminalFocused();
      if (focused === reported) return;
      reported = focused;
      setTerminalFocus(focused);
    };
    const scheduleReport = (): void => {
      if (pending === null) pending = setTimeout(report, 0);
    };
    report();
    window.addEventListener("focusin", scheduleReport);
    window.addEventListener("focusout", scheduleReport);
    const stopChords = onShellChord(chord => {
      const command = resolveShortcutCommand(chord, keybindings, { context: focusContext() });
      if (command !== null) runShellCommand(command, target.current);
    });
    return () => {
      if (pending !== null) clearTimeout(pending);
      window.removeEventListener("focusin", scheduleReport);
      window.removeEventListener("focusout", scheduleReport);
      stopChords();
    };
  }, [keybindings]);

  return null;
}
