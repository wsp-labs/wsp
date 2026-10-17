// SPDX-License-Identifier: AGPL-3.0-only
//! The app menu as the Electron app sets it (`apps/desktop/src/menu.ts`): on macOS the app menu, File, Edit, View,
//! Hosts and Window; on Windows File, Edit, View, Hosts and Window; on Linux none, as the Electron window there has no
//! menu bar. View keeps full screen; Electron's reload, developer tools and page zoom are a web page's.

use gpui_kit::component::input;
use gpui_kit::{actions, App, KeyBinding, Menu, MenuItem, OsAction, SystemMenuType};

use crate::TITLE;

actions!(app, [About, ShowHosts, Quit, CloseWindow, Hide, HideOthers, ShowAll, Minimize, Zoom, ToggleFullScreen]);

/// Where the menu bar has which menus; Electron sets none on Linux.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Os {
    Mac,
    Windows,
    Linux,
}

impl Os {
    pub fn current() -> Os {
        if cfg!(target_os = "macos") {
            Os::Mac
        } else if cfg!(target_os = "windows") {
            Os::Windows
        } else {
            Os::Linux
        }
    }
}

/// `HOST_WORDS.hosts` in `packages/protocol/src/words/places.ts`.
pub const HOSTS: &str = "Hosts";
/// The routes the About row and the Hosts row open; their screens draw the sheet and the list.
pub const ABOUT_SCREEN: &str = "about";
pub const HOSTS_SCREEN: &str = "hosts";

/// `QUIT_WORD` in `apps/desktop/src/quit.ts`.
pub const QUIT: &str = "Quit wsp";

/// The menus for `os`, each item with its action.
pub fn menus(os: Os) -> Vec<Menu> {
    let menu = |name: &str, items: Vec<MenuItem>| Menu { name: name.to_owned().into(), items, disabled: false };
    let quit = MenuItem::action(QUIT, Quit);
    let close = MenuItem::action("Close Window", CloseWindow);
    let edit = menu(
        "Edit",
        vec![
            MenuItem::os_action("Undo", input::Undo, OsAction::Undo),
            MenuItem::os_action("Redo", input::Redo, OsAction::Redo),
            MenuItem::separator(),
            MenuItem::os_action("Cut", input::Cut, OsAction::Cut),
            MenuItem::os_action("Copy", input::Copy, OsAction::Copy),
            MenuItem::os_action("Paste", input::Paste, OsAction::Paste),
            MenuItem::os_action("Select All", input::SelectAll, OsAction::SelectAll),
        ],
    );
    let view = menu("View", vec![MenuItem::action("Toggle Full Screen", ToggleFullScreen)]);
    let hosts = menu(HOSTS, vec![MenuItem::action("Show Hosts", ShowHosts)]);
    match os {
        Os::Mac => vec![
            menu(
                TITLE,
                vec![
                    MenuItem::action(format!("About {TITLE}"), About),
                    MenuItem::separator(),
                    MenuItem::os_submenu("Services", SystemMenuType::Services),
                    MenuItem::separator(),
                    MenuItem::action(format!("Hide {TITLE}"), Hide),
                    MenuItem::action("Hide Others", HideOthers),
                    MenuItem::action("Show All", ShowAll),
                    MenuItem::separator(),
                    quit,
                ],
            ),
            menu("File", vec![close]),
            edit,
            view,
            hosts,
            menu("Window", vec![MenuItem::action("Minimize", Minimize), MenuItem::action("Zoom", Zoom)]),
        ],
        Os::Windows => vec![
            menu("File", vec![close, MenuItem::separator(), quit]),
            edit,
            view,
            hosts,
            menu("Window", vec![MenuItem::action("Minimize", Minimize), MenuItem::action("Zoom", Zoom)]),
        ],
        Os::Linux => Vec::new(),
    }
}

/// The menu's own chords, Electron's accelerators: `CmdOrCtrl+Q`, `CmdOrCtrl+Shift+W`, and on macOS the app menu's. Linux
/// has no menu and so none of them, as a chord there would be taken from the page and the terminal.
pub fn bindings(os: Os) -> Vec<KeyBinding> {
    if os == Os::Linux {
        return Vec::new();
    }
    let m = if os == Os::Mac { "cmd" } else { "ctrl" };
    let mut keys = vec![KeyBinding::new(&format!("{m}-q"), Quit, None), KeyBinding::new(&format!("{m}-shift-w"), CloseWindow, None)];
    if os == Os::Mac {
        keys.extend([
            KeyBinding::new("cmd-h", Hide, None),
            KeyBinding::new("cmd-alt-h", HideOthers, None),
            KeyBinding::new("cmd-m", Minimize, None),
            KeyBinding::new("cmd-ctrl-f", ToggleFullScreen, None),
        ]);
    }
    keys
}

/// Binds the menu's chords, then sets the menus, which read their shortcuts off the keymap as it stands.
pub fn init(cx: &mut App) {
    let os = Os::current();
    cx.bind_keys(bindings(os));
    cx.on_action(|_: &Quit, cx| cx.quit());
    cx.on_action(|_: &About, cx| super::navigate(super::Route::Screen(ABOUT_SCREEN.into()), cx));
    cx.on_action(|_: &ShowHosts, cx| super::navigate(super::Route::Screen(HOSTS_SCREEN.into()), cx));
    cx.on_action(|_: &Hide, cx| cx.hide());
    cx.on_action(|_: &HideOthers, cx| cx.hide_other_apps());
    cx.on_action(|_: &ShowAll, cx| cx.unhide_other_apps());
    cx.on_action(|_: &CloseWindow, cx| on_active_window(cx, |window, _| window.remove_window()));
    cx.on_action(|_: &Minimize, cx| on_active_window(cx, |window, _| window.minimize_window()));
    cx.on_action(|_: &Zoom, cx| on_active_window(cx, |window, _| window.zoom_window()));
    cx.on_action(|_: &ToggleFullScreen, cx| on_active_window(cx, |window, _| window.toggle_fullscreen()));
    cx.set_menus(menus(os));
}

fn on_active_window(cx: &mut App, f: impl FnOnce(&mut gpui_kit::Window, &mut App)) {
    if let Some(window) = cx.active_window() {
        _ = window.update(cx, |_, window, cx| f(window, cx));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(os: Os) -> Vec<String> {
        menus(os).iter().map(|m| m.name.to_string()).collect()
    }

    fn rows(menu: &Menu) -> Vec<String> {
        menu.items
            .iter()
            .map(|item| match item {
                MenuItem::Action { name, .. } => name.to_string(),
                MenuItem::Separator => "-".to_owned(),
                MenuItem::SystemMenu(_) => "Services".to_owned(),
                MenuItem::Submenu(menu) => menu.name.to_string(),
            })
            .collect()
    }

    #[test]
    fn each_platform_has_electrons_menus() {
        assert_eq!(names(Os::Mac), [TITLE, "File", "Edit", "View", HOSTS, "Window"]);
        assert_eq!(names(Os::Windows), ["File", "Edit", "View", HOSTS, "Window"]);
        assert!(names(Os::Linux).is_empty());
        let app = &menus(Os::Mac)[0];
        assert_eq!(rows(app), ["About wsp", "-", "Services", "-", "Hide wsp", "Hide Others", "Show All", "-", QUIT]);
        assert_eq!(rows(&menus(Os::Windows)[0]), ["Close Window", "-", QUIT]);
    }

    #[test]
    fn linux_binds_no_menu_chords() {
        assert!(bindings(Os::Linux).is_empty());
        assert_eq!(bindings(Os::Windows).len(), 2);
        assert_eq!(bindings(Os::Mac).len(), 6);
    }
}
