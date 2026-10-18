// SPDX-License-Identifier: AGPL-3.0-only
//! What the palette and its pages share with every list laid over the window, after `components/ui/command.tsx`,
//! `autocomplete.tsx`, `kbd.tsx` and `CommandPaletteContent.tsx`: the blurred scrim, the popup, the search row, a
//! group's label, a row, and the footer's key hints.

use gpui_kit::component::{h_flex, v_flex, ActiveTheme as _, Icon, Sizable as _};
use gpui_kit::{div, prelude::*, px, AnyElement, App, Div, ElementId, FontWeight, Pixels, SharedString, Stateful, Window};

use crate::ui::card::footer_keycap;
use crate::ui::dialog::backdrop;
use crate::ui::{text, ActivePalette, Glyph};

/// `max-w-xl`.
pub const POPUP_WIDTH: Pixels = px(576.);
/// `max-h-105`.
pub const POPUP_MAX: Pixels = px(420.);
/// `rounded-xl`, and the footer's `rounded-b-[13px]` inside its 1 px edge.
pub const POPUP_RADIUS: Pixels = px(14.);
/// `--command-shell-inset` and `--command-content-inset`.
pub const SHELL_INSET: Pixels = px(8.);
pub const CONTENT_INSET: Pixels = px(16.);
/// The `sm` breakpoint, under which the popup sits 4vh down and the footer stacks.
pub const NARROW: Pixels = px(640.);
/// A row's `sm:min-h-7` and a group's `[[role=group]+&]:mt-1.5`.
pub const ROW_FLOOR: Pixels = px(28.);
pub const GROUP_GAP: Pixels = px(6.);
/// `rounded-sm` and `rounded-md` under `--radius: 0.625rem`.
pub const RADIUS_SM: Pixels = px(6.);
pub const RADIUS_MD: Pixels = px(8.);

/// `dialog-backdrop`: the page at 60% (64% in a dark theme) over a 4 px blur, over the whole window. It takes every
/// press so nothing under it reacts; `on_press` is the layer's own close.
pub fn scrim(id: impl Into<ElementId>, cx: &App) -> Stateful<Div> {
    backdrop(cx).id(id).absolute().top_0().left_0().size_full().occlude()
}

/// `CommandDialogViewport`'s `py-[max(--spacing(4),4vh)] sm:py-[10vh]`: how far down the popup sits.
pub fn popup_top(window: &Window) -> Pixels {
    let viewport = window.viewport_size();
    if viewport.width < NARROW {
        (viewport.height * 0.04).max(px(16.))
    } else {
        viewport.height * 0.1
    }
}

/// The popup's surface, `DIALOG_POPUP_CLASS` with the palette's `max-w-xl` and `overflow-hidden p-0`: the popover
/// fill inside the 10% edge, the dialog's shadow, and its children clipped to the corners.
pub fn popup(cx: &App) -> Div {
    let p = cx.palette();
    v_flex()
        .relative()
        .w_full()
        .max_w(POPUP_WIDTH)
        .max_h(POPUP_MAX)
        .min_h_0()
        .rounded(POPUP_RADIUS)
        .border_1()
        .border_color(p.popover_edge)
        .bg(p.popover)
        .shadow(p.dialog_shadow.clone())
        .overflow_hidden()
        .text_color(p.foreground)
}

/// `CommandInput`: the search glyph (or a page's own lead, such as Back) and the field, 34 px inside the 1 px its
/// clear edge takes, `py-1.5` around it.
pub fn search_row(lead: Option<AnyElement>, field: impl IntoElement, cx: &App) -> Div {
    let p = cx.palette();
    let lead = lead.unwrap_or_else(|| Icon::new(Glyph::Search).with_size(px(16.)).text_color(p.muted_foreground).into_any_element());
    div().px(SHELL_INSET).py(px(6.)).child(
        h_flex()
            .relative()
            .h(px(36.))
            .p(px(1.))
            .child(h_flex().absolute().top_0().bottom_0().left(px(10.)).items_center().opacity(0.8).child(lead))
            .child(div().flex_1().min_w_0().h_full().pl(SHELL_INSET + px(24.)).child(field)),
    )
}

/// `AutocompleteGroupLabel` with the palette's `ps-[9px]`: `px-2 py-1.5`, 12 px on a 16 px line, muted.
pub fn group_label(label: impl Into<SharedString>, cx: &App) -> Div {
    let p = cx.palette();
    div()
        .pl(px(9.))
        .pr(px(8.))
        .py(px(6.))
        .text_size(text::XS.0)
        .line_height(text::XS.1)
        .font_weight(FontWeight::NORMAL)
        .text_color(p.muted_foreground)
        .child(label.into())
}

/// One result row, `CommandPaletteResultRow` and its held twin: a glyph, the title with what the row says of itself
/// under it, then what trails it, the chord and a page's chevron.
pub struct Row {
    pub id: ElementId,
    pub icon: Option<AnyElement>,
    pub title: SharedString,
    pub description: Option<AnyElement>,
    pub trailing: Option<AnyElement>,
    pub shortcut: Option<SharedString>,
    pub page: bool,
    pub active: bool,
    pub held: bool,
}

impl Row {
    pub fn new(id: impl Into<ElementId>, title: impl Into<SharedString>) -> Row {
        Row {
            id: id.into(),
            icon: None,
            title: title.into(),
            description: None,
            trailing: None,
            shortcut: None,
            page: false,
            active: false,
            held: false,
        }
    }

    pub fn icon(mut self, icon: impl IntoElement) -> Row {
        self.icon = Some(icon.into_any_element());
        self
    }

    pub fn description(mut self, description: impl IntoElement) -> Row {
        self.description = Some(description.into_any_element());
        self
    }

    pub fn trailing(mut self, trailing: impl IntoElement) -> Row {
        self.trailing = Some(trailing.into_any_element());
        self
    }

    pub fn shortcut(mut self, shortcut: impl Into<SharedString>) -> Row {
        self.shortcut = Some(shortcut.into());
        self
    }

    pub fn page(mut self, page: bool) -> Row {
        self.page = page;
        self
    }

    pub fn active(mut self, active: bool) -> Row {
        self.active = active;
        self
    }

    pub fn held(mut self, held: bool) -> Row {
        self.held = held;
        self
    }

    /// The row as drawn; a held row takes no press and shows no chord, as the web's disabled row does.
    pub fn render(self, cx: &App) -> Stateful<Div> {
        let p = cx.palette();
        let title = div()
            .min_w_0()
            .truncate()
            .text_size(text::SETTING_TITLE.0)
            .line_height(text::SETTING_TITLE.1)
            .text_color(p.foreground)
            .child(self.title);
        let words = match self.description {
            Some(description) => v_flex()
                .flex_1()
                .min_w_0()
                .child(title)
                .child(div().min_w_0().text_size(text::XS.0).line_height(text::XS.1).text_color(p.muted_foreground).child(description)),
            None => h_flex().flex_1().min_w_0().items_center().child(title),
        };
        let held = self.held;
        h_flex()
            .id(self.id)
            .min_h(ROW_FLOOR)
            .gap_2()
            .px_2()
            .py(px(6.))
            .rounded(if held { RADIUS_SM } else { RADIUS_MD })
            .text_size(text::SETTING_TITLE.0)
            .line_height(text::SETTING_TITLE.1)
            .when(held, |row| row.opacity(0.64))
            .when(!held, |row| row.cursor_pointer())
            .when(self.active && !held, |row| row.bg(p.accent))
            .children(self.icon)
            .child(words)
            .children(self.trailing)
            .when(!held, |row| {
                row.children(self.shortcut.map(|keys| {
                    div()
                        .flex_none()
                        .ml_auto()
                        .font_family(cx.theme().mono_font_family.clone())
                        .text_size(text::XS.0)
                        .line_height(text::XS.1)
                        .text_color(p.muted_foreground)
                        .child(keys)
                }))
                .when(self.page, |row| {
                    row.child(
                        div()
                            .flex_none()
                            .ml_auto()
                            .mr(px(-2.))
                            .child(Icon::new(Glyph::ChevronRight).with_size(px(16.)).text_color(p.muted_foreground.opacity(0.7))),
                    )
                })
            })
    }
}

/// A row's glyph, `ITEM_ICON_CLASS`: 16 px in the muted icon ink.
pub fn row_icon(glyph: Glyph, cx: &App) -> Icon {
    Icon::new(glyph).with_size(px(16.)).text_color(cx.palette().muted_foreground)
}

/// What a key in the footer holds: a glyph, as the arrows are, or its name.
pub enum Cap {
    Glyph(Glyph),
    Word(SharedString),
}

/// A footer key: a glyph 12 px in the foreground's ink, or its name.
pub fn kbd(cap: Cap, cx: &App) -> Div {
    let inner = match cap {
        Cap::Glyph(glyph) => Icon::new(glyph).with_size(px(12.)).text_color(cx.palette().foreground).into_any_element(),
        Cap::Word(word) => word.into_any_element(),
    };
    footer_keycap(inner, cx)
}

/// One hint in the footer: its keys, then what they do.
pub struct Hint {
    pub caps: Vec<Cap>,
    pub label: SharedString,
}

impl Hint {
    pub fn new(caps: Vec<Cap>, label: impl Into<SharedString>) -> Hint {
        Hint { caps, label: label.into() }
    }

    pub fn word(word: &'static str, label: impl Into<SharedString>) -> Hint {
        Hint::new(vec![Cap::Word(word.into())], label)
    }
}

/// `CommandFooter`: the hints in 14 px medium muted words on the foreground at 2.5%, stacked under 640 px; `trailing`
/// stands at its right.
pub fn footer(hints: Vec<Hint>, trailing: Option<AnyElement>, window: &Window, cx: &App) -> Div {
    let p = cx.palette();
    let narrow = window.viewport_size().width < NARROW;
    let hints = h_flex().flex_wrap().items_center().gap_x_3().gap_y_2().children(hints.into_iter().map(|hint| {
        h_flex()
            .items_center()
            .gap(px(6.))
            .child(h_flex().gap(px(6.)).children(hint.caps.into_iter().map(|cap| kbd(cap, cx))))
            .child(hint.label)
    }));
    let row = if narrow { v_flex().items_start() } else { h_flex().items_center().justify_between() };
    row.relative()
        .flex_none()
        .gap_3()
        .px(CONTENT_INSET)
        .py(px(10.))
        .bg(p.foreground.opacity(0.025))
        .text_size(text::SETTING_TITLE.0)
        .line_height(text::SETTING_TITLE.1)
        .font_weight(FontWeight::MEDIUM)
        .text_color(p.muted_foreground)
        .child(hints)
        .children(trailing)
}

/// The palette footer's usual hints: the arrows Navigate, Enter `action`, Esc Close.
pub fn hints(action: &'static str) -> Vec<Hint> {
    vec![
        Hint::new(vec![Cap::Glyph(Glyph::ArrowUp), Cap::Glyph(Glyph::ArrowDown)], "Navigate"),
        Hint::word("Enter", action),
        Hint::word("Esc", "Close"),
    ]
}

/// `CommandPaletteResults`' empty line: 14 px muted, centred, `py-10`.
pub fn empty(line: impl Into<SharedString>, cx: &App) -> Div {
    div()
        .w_full()
        .py(px(40.))
        .text_center()
        .text_size(text::SETTING_TITLE.0)
        .line_height(text::SETTING_TITLE.1)
        .text_color(cx.palette().muted_foreground)
        .child(line.into())
}

/// The layer's column over the scrim: the popup centred, `popup_top` down, with `px-4` beside it.
pub fn column(window: &Window) -> Div {
    v_flex().absolute().top_0().left_0().size_full().items_center().px_4().pt(popup_top(window)).pb(popup_top(window))
}
