// SPDX-License-Identifier: AGPL-3.0-only
import type { MenuItemConstructorOptions } from "electron";

/** The rows the shell draws itself: Quit, which asks a question, and Hosts, drawn from the host list once there is
 * one. */
export interface MenuRows {
  readonly appName: string;
  readonly quit: MenuItemConstructorOptions;
  readonly hosts?: MenuItemConstructorOptions;
}

/** Mod+w is the page's own key, closing the focused pane's tab; a row registered on it would take the press first,
 * from every pane, so the window closes on mod+shift+w. Electron's own File and Window menus close it on mod+w. */
const CLOSE_WINDOW: MenuItemConstructorOptions = { role: "close", accelerator: "CmdOrCtrl+Shift+W" };

/** The shell's own menu bar: the platform's rows by their roles, with Quit, Hosts and Close Window drawn by hand. */
export function shellMenu(platform: NodeJS.Platform, rows: MenuRows): MenuItemConstructorOptions[] {
  const mac = platform === "darwin";
  return [
    ...(mac
      ? [
          { label: rows.appName, submenu: [{ role: "about" as const }, { type: "separator" as const }, { role: "services" as const }, { type: "separator" as const }, { role: "hide" as const }, { role: "hideOthers" as const }, { role: "unhide" as const }, { type: "separator" as const }, rows.quit] },
          { role: "fileMenu" as const, submenu: [CLOSE_WINDOW] },
        ]
      : [{ label: "File", submenu: [CLOSE_WINDOW, { type: "separator" as const }, rows.quit] }]),
    { role: "editMenu" },
    { role: "viewMenu" },
    ...(rows.hosts === undefined ? [] : [rows.hosts]),
    mac ? { role: "windowMenu" } : { role: "windowMenu", submenu: [{ role: "minimize" }, { role: "zoom" }] },
  ];
}
