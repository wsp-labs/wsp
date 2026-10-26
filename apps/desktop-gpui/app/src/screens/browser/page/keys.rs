// SPDX-License-Identifier: AGPL-3.0-only
//! Which keys a focused page gives the app: the app's chords, as Electron's menu accelerators take them before a page
//! does, and `mod+w`, which today's shell takes from a page to close its tab. Every other key, Escape too, is the page's.

use gpui_kit::Keystroke;

use crate::screens::app_shell::menu::{self, Os};

/// The chords the app keeps while a page holds the keys, as GPUI spells them.
pub fn app_chords(os: Os) -> Vec<Keystroke> {
    let close_tab = if os == Os::Mac { "cmd-w" } else { "ctrl-w" };
    menu::bindings(os)
        .iter()
        .flat_map(|binding| binding.keystrokes().iter().map(|k| k.inner().clone()))
        .chain(Keystroke::parse(close_tab).ok())
        .collect()
}

/// Whether a press with these modifiers on `key` (lower case, as GPUI names keys) is one of the app's chords.
pub fn is_app_chord(chords: &[Keystroke], key: &str, modifiers: &gpui_kit::Modifiers) -> bool {
    chords.iter().any(|chord| chord.key.eq_ignore_ascii_case(key) && chord.modifiers == *modifiers)
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui_kit::Modifiers;

    fn ctrl(shift: bool) -> Modifiers {
        Modifiers { control: true, shift, ..Modifiers::default() }
    }

    #[test]
    fn on_linux_the_page_gives_up_only_the_close_chords() {
        let chords = app_chords(Os::Linux);
        assert!(is_app_chord(&chords, "w", &ctrl(false)), "ctrl-w closes the tab");
        assert!(is_app_chord(&chords, "w", &ctrl(true)), "ctrl-shift-w closes the window");
        assert!(!is_app_chord(&chords, "k", &ctrl(false)), "the palette's chord is the page's");
        assert!(!is_app_chord(&chords, "escape", &Modifiers::default()), "Escape is the page's");
        assert!(!is_app_chord(&chords, "q", &ctrl(false)), "Linux has no menu, so no Quit chord");
    }

    #[test]
    fn on_the_mac_the_menus_chords_stay_the_apps() {
        let chords = app_chords(Os::Mac);
        let cmd = |shift, alt| Modifiers { platform: true, shift, alt, ..Modifiers::default() };
        assert!(is_app_chord(&chords, "q", &cmd(false, false)));
        assert!(is_app_chord(&chords, "w", &cmd(false, false)));
        assert!(is_app_chord(&chords, "w", &cmd(true, false)));
        assert!(is_app_chord(&chords, "m", &cmd(false, false)));
        assert!(is_app_chord(&chords, "h", &cmd(false, true)));
        assert!(!is_app_chord(&chords, "k", &cmd(false, false)));
        assert!(!is_app_chord(&chords, "c", &cmd(false, false)), "copy is the page's");
    }
}
