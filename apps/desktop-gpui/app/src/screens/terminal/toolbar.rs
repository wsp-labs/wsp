// SPDX-License-Identifier: AGPL-3.0-only
//! The drawer's toolbar (`ThreadTerminalDrawer.tsx`): split, split vertical, new, font and close, as a floating strip
//! over a lone shell or as the tab column's 22 px head, each with today's "<title> (<chord>)" tooltip.

use std::rc::Rc;

use gpui_kit::component::{h_flex, Icon, Sizable as _};
use gpui_kit::{div, prelude::*, px, AnyElement, App, Keystroke, SharedString, Window};

use crate::screens::app_shell::keys::{keystroke, DEFAULTS};
use crate::ui::context_menu::keys_label;
use crate::ui::{ActivePalette, Glyph};

/// `MAX_TERMINALS_PER_GROUP`'s refusal (`actions/format.ts`).
pub const SPLIT_LIMIT: &str = "Max 4 per group";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Act {
    Split,
    SplitVertical,
    New,
    Font,
    Close,
}

impl Act {
    pub const ALL: [Act; 5] = [Act::Split, Act::SplitVertical, Act::New, Act::Font, Act::Close];

    pub fn title(self) -> &'static str {
        match self {
            Act::Split => "Split terminal horizontally",
            Act::SplitVertical => "Split terminal vertically",
            Act::New => "New terminal",
            Act::Font => "Terminal font",
            Act::Close => "Close terminal",
        }
    }

    fn glyph(self) -> Glyph {
        match self {
            Act::Split => Glyph::SquareSplitHorizontal,
            Act::SplitVertical => Glyph::SquareSplitVertical,
            Act::New => Glyph::Plus,
            Act::Font => Glyph::Type,
            Act::Close => Glyph::Trash,
        }
    }

    fn command(self) -> Option<&'static str> {
        match self {
            Act::Split => Some("terminal.split"),
            Act::New => Some("terminal.new"),
            _ => None,
        }
    }

    fn id(self) -> &'static str {
        match self {
            Act::Split => "terminal-split",
            Act::SplitVertical => "terminal-split-vertical",
            Act::New => "terminal-new",
            Act::Font => "terminal-font",
            Act::Close => "terminal-close",
        }
    }
}

/// The chord a command is bound to in a focused terminal.
pub fn chord_of(command: &str, mac: bool) -> Option<SharedString> {
    let rule = DEFAULTS.iter().find(|r| r.command == command && r.when == Some("terminalFocus"))?;
    let typed = Keystroke::parse(&keystroke(rule.chord, mac)?).ok()?;
    Some(keys_label(&typed, mac).into())
}

/// The tooltip's words: the refusal or the chord in brackets after the title, or the title alone.
pub fn label(act: Act, refused: bool, mac: bool) -> SharedString {
    let suffix = if refused && matches!(act, Act::Split | Act::SplitVertical) {
        Some(SharedString::new_static(SPLIT_LIMIT))
    } else {
        act.command().and_then(|c| chord_of(c, mac))
    };
    match suffix {
        Some(suffix) => format!("{} ({suffix})", act.title()).into(),
        None => act.title().into(),
    }
}

pub type OnAct = Rc<dyn Fn(Act, &mut Window, &mut App)>;

fn button(act: Act, refused: bool, header: bool, first: bool, on: &OnAct, cx: &App) -> AnyElement {
    let p = cx.palette();
    let held = refused && matches!(act, Act::Split | Act::SplitVertical);
    let tip = label(act, refused, cfg!(target_os = "macos"));
    let on = on.clone();
    div()
        .id(act.id())
        .debug_selector(move || act.id().into())
        .flex()
        .items_center()
        .justify_center()
        .map(|el| if header { el.h_full().px_1() } else { el.p_1() })
        .when(header && !first, |el| el.border_l_1().border_color(p.border.opacity(0.7)))
        .text_color(p.foreground.opacity(0.9))
        .tooltip(crate::ui::tip(tip))
        .map(|el| {
            if held {
                el.opacity(0.45).cursor_not_allowed()
            } else {
                let hover = if header { p.accent.opacity(0.7) } else { p.accent };
                el.cursor_pointer().hover(move |s| s.bg(hover)).on_click(move |_, window, cx| on(act, window, cx))
            }
        })
        .child(Icon::new(act.glyph()).with_size(px(13.)))
        .into_any_element()
}

/// The strip over a lone shell: 8 px from the top and right, radius 8, its buttons 21 px with 16 px dividers.
pub fn floating(can_split: bool, on: OnAct, cx: &App) -> AnyElement {
    let p = cx.palette();
    let mut strip = h_flex()
        .id("terminal-toolbar")
        .debug_selector(|| "terminal-toolbar".into())
        .items_center()
        .overflow_hidden()
        .rounded(px(8.))
        .border_1()
        .border_color(p.border.opacity(0.8))
        .bg(p.background)
        .shadow_xs();
    for (ix, act) in Act::ALL.into_iter().enumerate() {
        if ix > 0 {
            strip = strip.child(div().h(px(16.)).w(px(1.)).bg(p.border.opacity(0.8)));
        }
        strip = strip.child(button(act, !can_split, false, ix == 0, &on, cx));
    }
    strip.into_any_element()
}

/// The tab column's head: 22 px, the same buttons edge to edge, a line under it.
pub fn header(can_split: bool, on: OnAct, cx: &App) -> AnyElement {
    let p = cx.palette();
    h_flex()
        .id("terminal-toolbar")
        .debug_selector(|| "terminal-toolbar".into())
        .flex_none()
        .h(px(22.))
        .items_stretch()
        .justify_end()
        .border_b_1()
        .border_color(p.border)
        .children(Act::ALL.into_iter().enumerate().map(|(ix, act)| button(act, !can_split, true, ix == 0, &on, cx)))
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tooltips_read_as_todays() {
        assert_eq!(label(Act::Split, false, false), "Split terminal horizontally (Ctrl+D)");
        assert_eq!(label(Act::Split, false, true), "Split terminal horizontally (⌘D)");
        assert_eq!(label(Act::New, false, false), "New terminal (Ctrl+N)");
        assert_eq!(label(Act::SplitVertical, false, false), "Split terminal vertically");
        assert_eq!(label(Act::SplitVertical, true, false), "Split terminal vertically (Max 4 per group)");
        assert_eq!(label(Act::Close, true, false), "Close terminal");
        assert_eq!(chord_of("terminal.zoomIn", false).as_deref(), Some("Ctrl+="));
        assert_eq!(chord_of("terminal.zoomIn", true).as_deref(), Some("⌘="));
    }
}
