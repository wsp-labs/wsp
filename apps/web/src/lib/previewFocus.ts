// Adapted from pingdotgg/t3code apps/web/src/lib/previewFocus.ts at 57a66608 (MIT).
/**
 * Returns true when the user's keyboard focus is somewhere inside the right
 * panel, or on a browser tab's guest page. The guests stand in a layer of
 * their own beside the shell, over their tab's slot, so a focused guest is
 * outside the panel's element; a guest the person can focus is a shown one.
 *
 * Used by the global keybinding handler to gate panel commands to only
 * fire while the panel owns focus.
 */
export function isPreviewFocused(): boolean {
  const activeElement = document.activeElement;
  if (!(activeElement instanceof HTMLElement)) return false;
  if (!activeElement.isConnected) return false;
  return activeElement.closest("[data-preview-panel-mode], [data-browser-guest]") !== null;
}
