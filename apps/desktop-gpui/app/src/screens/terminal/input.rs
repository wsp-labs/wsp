// SPDX-License-Identifier: AGPL-3.0-only
//! A GPUI keystroke as libghostty's key encoder takes it (`terminal/ghostty/keyCodes.ts`, `surface/input.ts`): the
//! key by name, the modifiers, the text it types and its unshifted character.

use gpui_kit::{Keystroke, Modifiers};
use libghostty_vt::key::{Action, Key, Mods};

use super::engine::KeyInput;

/// The encoder's key for GPUI's key name, which is already the unshifted, layout-mapped key.
pub fn key_of(name: &str) -> Key {
    if let Some(c) = single(name) {
        if c.is_ascii_lowercase() || c.is_ascii_uppercase() {
            return letter(c.to_ascii_lowercase());
        }
        if let Some(d) = c.to_digit(10) {
            return [
                Key::Digit0,
                Key::Digit1,
                Key::Digit2,
                Key::Digit3,
                Key::Digit4,
                Key::Digit5,
                Key::Digit6,
                Key::Digit7,
                Key::Digit8,
                Key::Digit9,
            ][d as usize];
        }
        return match unshift(c) {
            '-' => Key::Minus,
            '=' => Key::Equal,
            '[' => Key::BracketLeft,
            ']' => Key::BracketRight,
            '\\' => Key::Backslash,
            ';' => Key::Semicolon,
            '\'' => Key::Quote,
            ',' => Key::Comma,
            '.' => Key::Period,
            '/' => Key::Slash,
            '`' => Key::Backquote,
            ' ' => Key::Space,
            _ => Key::Unidentified,
        };
    }
    match name {
        "enter" => Key::Enter,
        "backspace" => Key::Backspace,
        "tab" => Key::Tab,
        "escape" => Key::Escape,
        "space" => Key::Space,
        "left" => Key::ArrowLeft,
        "right" => Key::ArrowRight,
        "up" => Key::ArrowUp,
        "down" => Key::ArrowDown,
        "home" => Key::Home,
        "end" => Key::End,
        "pageup" => Key::PageUp,
        "pagedown" => Key::PageDown,
        "insert" => Key::Insert,
        "delete" => Key::Delete,
        "shift" => Key::ShiftLeft,
        "control" => Key::ControlLeft,
        "alt" => Key::AltLeft,
        "platform" => Key::MetaLeft,
        _ => function(name).unwrap_or(Key::Unidentified),
    }
}

fn single(s: &str) -> Option<char> {
    let mut chars = s.chars();
    let c = chars.next()?;
    chars.next().is_none().then_some(c)
}

fn letter(c: char) -> Key {
    const LETTERS: [Key; 26] = [
        Key::A,
        Key::B,
        Key::C,
        Key::D,
        Key::E,
        Key::F,
        Key::G,
        Key::H,
        Key::I,
        Key::J,
        Key::K,
        Key::L,
        Key::M,
        Key::N,
        Key::O,
        Key::P,
        Key::Q,
        Key::R,
        Key::S,
        Key::T,
        Key::U,
        Key::V,
        Key::W,
        Key::X,
        Key::Y,
        Key::Z,
    ];
    LETTERS[(c as u8 - b'a') as usize]
}

fn function(name: &str) -> Option<Key> {
    const F: [Key; 24] = [
        Key::F1,
        Key::F2,
        Key::F3,
        Key::F4,
        Key::F5,
        Key::F6,
        Key::F7,
        Key::F8,
        Key::F9,
        Key::F10,
        Key::F11,
        Key::F12,
        Key::F13,
        Key::F14,
        Key::F15,
        Key::F16,
        Key::F17,
        Key::F18,
        Key::F19,
        Key::F20,
        Key::F21,
        Key::F22,
        Key::F23,
        Key::F24,
    ];
    let n: usize = name.strip_prefix('f')?.parse().ok()?;
    F.get(n.checked_sub(1)?).copied()
}

/// The US layout's unshifted key for a shifted symbol, which today's pane falls back to without a layout map.
pub fn unshift(c: char) -> char {
    match c {
        '!' => '1',
        '@' => '2',
        '#' => '3',
        '$' => '4',
        '%' => '5',
        '^' => '6',
        '&' => '7',
        '*' => '8',
        '(' => '9',
        ')' => '0',
        '~' => '`',
        '_' => '-',
        '+' => '=',
        '{' => '[',
        '}' => ']',
        '|' => '\\',
        ':' => ';',
        '"' => '\'',
        '<' => ',',
        '>' => '.',
        '?' => '/',
        c => c.to_ascii_lowercase(),
    }
}

pub fn mods_of(m: &Modifiers) -> Mods {
    let mut mods = Mods::empty();
    mods.set(Mods::SHIFT, m.shift);
    mods.set(Mods::CTRL, m.control);
    mods.set(Mods::ALT, m.alt);
    mods.set(Mods::SUPER, m.platform);
    mods
}

/// The encoder's input for a key going down, held (`repeat`), or up.
pub fn key_input(k: &Keystroke, action: Action) -> KeyInput {
    let name = k.key.as_str();
    let typed = k.key_char.as_deref().filter(|t| t.chars().count() == 1);
    let utf8 = match name {
        "backspace" => Some("\x7f".to_owned()),
        "space" => Some(" ".to_owned()),
        _ => typed
            .map(str::to_owned)
            .or_else(|| single(name).filter(|c| !c.is_control()).map(|c| c.to_string()).filter(|_| k.modifiers.control || k.modifiers.alt)),
    };
    let only_shift = k.modifiers.shift && !k.modifiers.control && !k.modifiers.alt && !k.modifiers.platform;
    let consumed = if only_shift && typed.is_some() { Mods::SHIFT } else { Mods::empty() };
    let unshifted = match name {
        "space" => ' ',
        _ => single(name).map(unshift).unwrap_or('\0'),
    };
    KeyInput { action, key: key_of(name), mods: mods_of(&k.modifiers), consumed, utf8, unshifted }
}

/// A key the platform turns into text the shell should get as typed: a printable character with no Control, Alt or
/// Command held. Such a key reaches the terminal through its input handler, which also commits an IME's text.
pub fn types_text(k: &Keystroke) -> bool {
    let m = &k.modifiers;
    !m.control
        && !m.alt
        && !m.platform
        && !m.function
        && k.key_char.as_deref().is_some_and(|t| !t.is_empty() && !t.chars().any(char::is_control))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::screens::terminal::engine::Engine;

    fn bytes(chord: &str, engine: &mut Engine) -> Vec<u8> {
        let k = Keystroke::parse(chord).unwrap();
        engine.encode_key(&key_input(&k, Action::Press))
    }

    #[test]
    fn named_and_control_keys_encode_as_a_terminal_sends_them() {
        let mut e = Engine::new(10, 2);
        assert_eq!(bytes("enter", &mut e), b"\r");
        assert_eq!(bytes("backspace", &mut e), b"\x7f");
        assert_eq!(bytes("escape", &mut e), b"\x1b");
        assert_eq!(bytes("tab", &mut e), b"\t");
        assert_eq!(bytes("ctrl-c", &mut e), b"\x03");
        assert_eq!(bytes("ctrl-d", &mut e), b"\x04");
        assert_eq!(bytes("ctrl-[", &mut e), b"\x1b[91;5u", "Ghostty sends Ctrl+[ as CSI u, after fixterms");
        assert_eq!(bytes("alt-b", &mut e), b"\x1bb");
        assert_eq!(bytes("f5", &mut e), b"\x1b[15~");
        assert_eq!(bytes("pageup", &mut e), b"\x1b[5~");
    }

    /// Option on a Mac types a character (Option+b is `∫`), which GPUI reports as the key's text; today's pane sends
    /// such a chord as Alt and the unshifted key, so readline's word moves work.
    #[test]
    fn option_or_alt_with_b_and_f_move_by_words_as_today() {
        let mut e = Engine::new(10, 2);
        let typed = |key: &str, text: &str| Keystroke {
            modifiers: Modifiers { alt: true, ..Modifiers::default() },
            key: key.into(),
            key_char: Some(text.into()),
        };
        let (b, f) = if cfg!(target_os = "macos") { (typed("b", "∫"), typed("f", "ƒ")) } else { (typed("b", "b"), typed("f", "f")) };
        assert_eq!(e.encode_key(&key_input(&b, Action::Press)), b"\x1bb");
        assert_eq!(e.encode_key(&key_input(&f, Action::Press)), b"\x1bf");
    }

    #[test]
    fn cursor_keys_follow_decckm() {
        let mut e = Engine::new(10, 2);
        assert_eq!(bytes("up", &mut e), b"\x1b[A");
        e.write(b"\x1b[?1h");
        assert_eq!(bytes("up", &mut e), b"\x1bOA");
    }

    #[test]
    fn plain_text_goes_through_the_input_handler_and_chords_do_not() {
        assert!(types_text(&Keystroke::parse("a->a").unwrap()));
        assert!(!types_text(&Keystroke::parse("ctrl-a").unwrap()));
        assert!(!types_text(&Keystroke::parse("enter").unwrap()));
        assert_eq!(unshift('!'), '1');
        assert_eq!(key_of("?"), Key::Slash);
    }
}
