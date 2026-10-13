// SPDX-License-Identifier: AGPL-3.0-only
// The menu bar takes a registered chord before the page or a browser tab's
// page sees it, so a row that closes the window on mod+w closes the window
// from inside any pane. Mod+w is the page's close-tab key; the window closes on
// mod+shift+w.
import type { MenuItemConstructorOptions } from "electron";
import { describe, expect, it } from "vitest";
import { shellMenu } from "../src/menu.js";

/** The rows Electron fills a role's menu with when the template gives it none, per platform, read off
 * lib/browser/api/menu-item-roles.ts at v43.5.1 and, for macOS, off Menu.buildFromTemplate on this Electron. */
const ROLE_ROWS: Record<string, (mac: boolean) => MenuItemConstructorOptions[]> = {
  filemenu: mac => [mac ? { role: "close" } : { role: "quit" }],
  windowmenu: mac => [{ role: "minimize" }, { role: "zoom" }, ...(mac ? [{ role: "front" as const }] : [{ role: "close" as const }])],
};

const ROLE_ACCELERATORS: Record<string, string> = { close: "CommandOrControl+W", quit: "CommandOrControl+Q", minimize: "CommandOrControl+M" };

/** Every row the built menu holds, with the accelerator it ends up carrying. */
function rowsOf(items: readonly MenuItemConstructorOptions[], mac: boolean): { role?: string; accelerator?: string }[] {
  return items.flatMap(item => {
    const role = item.role?.toLowerCase();
    const sub = Array.isArray(item.submenu) ? item.submenu : role !== undefined && item.submenu === undefined ? ROLE_ROWS[role]?.(mac) : undefined;
    const accelerator = item.accelerator ?? (role === undefined ? undefined : ROLE_ACCELERATORS[role]);
    const self = sub === undefined ? [{ ...(role === undefined ? {} : { role }), ...(accelerator === undefined ? {} : { accelerator: String(accelerator) }) }] : [];
    return [...self, ...(sub === undefined ? [] : rowsOf(sub, mac))];
  });
}

const ROWS = { appName: "wsp", quit: { label: "Quit wsp", accelerator: "CmdOrCtrl+Q" }, hosts: { label: "Hosts", submenu: [] } };
const plain = (accelerator: string | undefined): string | undefined => accelerator?.replace("CommandOrControl", "CmdOrCtrl");

describe("the shell's menu bar", () => {
  for (const platform of ["darwin", "win32"] as const) {
    it(`on ${platform} binds no row to mod+w and closes the window on mod+shift+w`, () => {
      const rows = rowsOf(shellMenu(platform, ROWS), platform === "darwin");
      expect(rows.filter(row => plain(row.accelerator) === "CmdOrCtrl+W")).toEqual([]);
      const closes = rows.filter(row => row.role === "close");
      expect(closes).toEqual([{ role: "close", accelerator: "CmdOrCtrl+Shift+W" }]);
    });

    it(`on ${platform} is the same before there is a host list, as at launch under onboarding and the starting window`, () => {
      const { hosts: _hosts, ...launch } = ROWS;
      const menu = shellMenu(platform, launch);
      expect(menu.map(item => item.label)).not.toContain("Hosts");
      const rows = rowsOf(menu, platform === "darwin");
      expect(rows.filter(row => plain(row.accelerator) === "CmdOrCtrl+W")).toEqual([]);
      expect(rows.filter(row => row.role === "close")).toEqual([{ role: "close", accelerator: "CmdOrCtrl+Shift+W" }]);
    });
  }
});
