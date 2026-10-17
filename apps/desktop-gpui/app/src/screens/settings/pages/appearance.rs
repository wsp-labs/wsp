// SPDX-License-Identifier: AGPL-3.0-only
//! Appearance (`appearance.tsx`, `ThemePicker.tsx`, `FontPicker.tsx`): the side, then the themes of the side drawn,
//! each as the window it makes; whether the glass shows what is behind the window, where a window draws glass at all;
//! and the app and code faces and the sizes they are read at, under a sample.

mod picture;

use std::cell::Cell;
use std::rc::Rc;

use gpui_kit::component::text::TextView;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::{canvas, div, prelude::*, px, relative, App, Entity, FontWeight, Pixels, SharedString, Window, WindowAppearance};
use serde_json::{json, Value};
use wsp_host_client::protocol::Preferences;

use super::general::{choice_select, field_switch, here_name, once_named, record, reset_to};
use crate::screens::settings::page::{CardData, Ctx, Item};
use crate::screens::settings::Acts;
use crate::ui::theme::{self, Side, ThemeEntry, THEMES};
use crate::ui::{size, ActivePalette, Select};
use picture::{palette_of, window_picture};

/// `SETTINGS_WORDS`, `FONT_WORDS`, `THEME_SECTION_WORDS`, `GLASS_WORDS`, `THEME_WORDS` and `TRANSPARENCY_WORDS` in
/// `format.ts`, and each theme's line in `apps/web/src/themes/`.
mod words {
    pub const MODE: &str = "Mode";
    pub const THEME: &str = "Theme";
    pub const THEME_LEDE: &str = "Point at a theme to see this window in it.";
    pub const GLASS: &str = "Glass";
    pub const GLASS_LEDE: &str = "How wsp's window shows what is behind it.";
    pub const TRANSPARENCY: &str = "Transparency";
    pub const TRANSPARENCY_NOTE: &str = "Glass shows what is behind the window. Off, every surface is solid.";
    pub const TYPE: &str = "Type";
    pub const TYPE_LEDE: &str = "The faces and sizes the conversation and its code are read in.";
    pub const APP_FONT: &str = "App font";
    pub const APP_FONT_NOTE: &str = "The sidebar, replies and every page.";
    pub const CODE_FONT: &str = "Code font";
    pub const CODE_FONT_NOTE: &str = "Code in replies, the changes and files.";
    pub const DEFAULT: &str = "Default";
    pub const TEXT_SIZE: &str = "Reading size";
    pub const TEXT_SIZE_NOTE: &str = "Replies, your own messages and the box you write in.";
    pub const CODE_SIZE: &str = "Code size";
    pub const CODE_SIZE_NOTE: &str = "Code in replies, tool output, the changes and files.";
    pub const SAMPLE: &str = "The total rounded each line on its own, so three lines at 0.335 came to 1.00 or 1.01. It now rounds once:\n\n```ts\nexport const total = (lines: Line[]) => round(lines.reduce((sum, l) => sum + l.price, 0));\n```";

    pub fn mode_lede(here: &str) -> String {
        super::once_named(here, |h| format!("Light, dark, or whichever side {h} is on."))
    }
    pub fn mode(mode: &str) -> &'static str {
        match mode {
            "light" => "Light",
            "dark" => "Dark",
            _ => "System",
        }
    }
    pub fn themes_of(side: &str) -> String {
        format!("{side} themes")
    }
    pub fn px(size: i64) -> String {
        format!("{size} px")
    }
    pub fn line(theme: &str) -> &'static str {
        match theme {
            "paper" => "Zinc on near white, with the wsp blue.",
            "linen" => "Unbleached light with warm greys and an ink-black key.",
            "frost" => "Cool blue-grey light with a steel blue.",
            "graphite" => "Neutral black with a black sidebar and the wsp blue.",
            "denim" => "Indigo-black with a periwinkle blue.",
            "tungsten" => "Warm soot with ivory ink and an ivory key.",
            "moss" => "Forest grey with sage ink and a sage key.",
            _ => "True black for an OLED screen, one grey edge and a white key.",
        }
    }
}

const MODES: [&str; 3] = ["system", "light", "dark"];
/// `TEXT_SIZES` and `CODE_SIZES`.
const TEXT_SIZES: [i64; 5] = [13, 14, 15, 16, 17];
const CODE_SIZES: [i64; 5] = [11, 12, 13, 14, 15];
/// `isSolidWindow`: the desktop app draws glass on macOS alone.
const SOLID: bool = !cfg!(target_os = "macos");
/// The page column's width before the first frame measures it: 760 less its 32 px sides.
const COLUMN: Pixels = px(696.);
/// `ring-2 ring-offset-2`: the picked ring stands 2 px off the card's edge.
const RING: Pixels = px(4.);

/// What the window draws its theme from: the side's pick, and each side's theme.
#[derive(Clone)]
struct Picks {
    mode: String,
    light: String,
    dark: String,
}

impl Picks {
    fn of(p: &Value) -> Picks {
        let read = |field: &str, or: &str| p[field].as_str().unwrap_or(or).to_owned();
        Picks { mode: read("theme", "system"), light: read("lightTheme", theme::LIGHT), dark: read("darkTheme", theme::DARK) }
    }

    /// `shownSide`: System reads the computer's side.
    fn side(&self, system_dark: bool) -> Side {
        match self.mode.as_str() {
            "dark" => Side::Dark,
            "light" => Side::Light,
            _ if system_dark => Side::Dark,
            _ => Side::Light,
        }
    }

    /// The theme the window draws for these picks: the side's pick, else the side's default.
    fn shown(&self, system_dark: bool) -> &'static ThemeEntry {
        let side = self.side(system_dark);
        theme_for(side, if side == Side::Dark { &self.dark } else { &self.light })
    }
}

/// `themeFor`: the pick when it is registered on that side, else the side's default.
fn theme_for(side: Side, id: &str) -> &'static ThemeEntry {
    let fallback = if side == Side::Dark { theme::DARK } else { theme::LIGHT };
    THEMES.iter().find(|t| t.id == id && t.side == side).or_else(|| theme::theme_by_id(fallback)).unwrap_or(&THEMES[0])
}

fn system_dark(window: &Window) -> bool {
    matches!(window.appearance(), WindowAppearance::Dark | WindowAppearance::VibrantDark)
}

/// `APPEARANCE_DEFAULTS`: the side, each side's theme, Transparency, both faces and both sizes.
pub fn defaults() -> Value {
    json!({ "theme": "system", "lightTheme": theme::LIGHT, "darkTheme": theme::DARK, "appFont": "", "codeFont": "",
        "textSize": null, "codeSize": null, "transparency": true })
}

/// `appearanceOffDefaults`.
pub fn off_defaults(p: &Preferences) -> bool {
    p.theme.to_string() != "system"
        || p.light_theme != theme::LIGHT
        || p.dark_theme != theme::DARK
        || !p.app_font.is_empty()
        || !p.code_font.is_empty()
        || p.text_size.is_some()
        || p.code_size.is_some()
        || (!p.transparency && !SOLID)
}

/// The column's width as the last frame drew it, which the tiles divide.
fn column_width(window: &mut Window, cx: &mut App) -> Rc<Cell<Pixels>> {
    window.use_keyed_state("appearance-width", cx, |_, _| Rc::new(Cell::new(COLUMN))).read(cx).clone()
}

/// A probe filling its parent that keeps the width in step and draws again once it moved.
fn measure(width: Rc<Cell<Pixels>>) -> impl IntoElement {
    canvas(
        move |bounds, window, _| {
            if (bounds.size.width - width.get()).abs() > px(0.5) {
                width.set(bounds.size.width);
                window.refresh();
            }
        },
        |_, _, _, _| {},
    )
    .absolute()
    .inset_0()
}

/// `CARD_SURFACE`'s look on a tile: its edge a step up under the pointer while not picked, and the ring once picked.
fn surface(group: SharedString, chosen: bool, cx: &App) -> gpui_kit::Div {
    let p = cx.palette();
    let hover = p.foreground.opacity(0.2);
    div()
        .absolute()
        .inset_0()
        .overflow_hidden()
        .rounded(size::CARD_RADIUS)
        .border_1()
        .border_color(p.border.opacity(0.6))
        .bg(p.card.opacity(0.4))
        .when(!chosen, |el| el.group_hover(group, move |s| s.border_color(hover)))
}

fn ring(chosen: bool, cx: &App) -> Option<gpui_kit::Div> {
    let primary = cx.palette().primary;
    chosen.then(|| {
        div()
            .absolute()
            .top(-RING)
            .left(-RING)
            .right(-RING)
            .bottom(-RING)
            .rounded(size::CARD_RADIUS + RING)
            .border_2()
            .border_color(primary)
    })
}

/// `ModeTile`: the side's window in the theme it draws; System's halves are each side's.
fn mode_tile(mode: &'static str, picks: &Picks, width: Pixels, acts: &Acts, cx: &App) -> impl IntoElement {
    let p = cx.palette();
    let chosen = picks.mode == mode;
    let (light, dark) = (theme_for(Side::Light, &picks.light), theme_for(Side::Dark, &picks.dark));
    let group = SharedString::from(format!("mode-{mode}"));
    let inner = width - px(2.);
    let height = width * 0.625;
    let shown = if mode == "dark" { dark } else { light };
    let mut face = surface(group.clone(), chosen, cx).child(window_picture(palette_of(shown.id), inner));
    if mode == "system" {
        face = face.child(
            div()
                .absolute()
                .top_0()
                .bottom_0()
                .right_0()
                .w(inner / 2.)
                .overflow_hidden()
                .child(div().absolute().top_0().left(-inner / 2.).child(window_picture(palette_of(dark.id), inner))),
        );
    }
    let acts = acts.clone();
    let (ink, hover_ink) = (if chosen { p.foreground } else { p.muted_foreground }, p.foreground);
    v_flex()
        .id(group.clone())
        .group(group.clone())
        .debug_selector(move || format!("theme-mode-{mode}"))
        .w(width)
        .gap_2()
        .cursor_pointer()
        .on_click(move |_, _, cx| acts.set_preferences(json!({ "theme": mode }), cx))
        .child(div().relative().w(width).h(height).child(face).children(ring(chosen, cx)))
        .child(
            div()
                .text_size(px(13.))
                .line_height(px(16.))
                .text_color(ink)
                .group_hover(group, move |s| s.text_color(hover_ink))
                .child(words::mode(mode)),
        )
}

/// `ThemeCard`: the theme's window in its own tokens over its name and what it is like. A pointer over it shows the
/// real window in it, and leaving puts the pick back.
fn theme_card(entry: &'static ThemeEntry, picks: &Picks, width: Pixels, acts: &Acts, cx: &App) -> impl IntoElement {
    let p = cx.palette();
    let field = if entry.side == Side::Dark { "darkTheme" } else { "lightTheme" };
    let chosen = (if entry.side == Side::Dark { &picks.dark } else { &picks.light }) == entry.id;
    let group = SharedString::from(format!("theme-{}", entry.id));
    let inner = width - px(2.);
    let words_block = v_flex()
        .gap_1()
        .px_3()
        .py(px(10.))
        .child(div().text_size(px(13.)).line_height(px(16.)).font_weight(FontWeight::MEDIUM).text_color(p.foreground).child(entry.word))
        .child(div().min_h(px(32.)).text_size(px(12.)).line_height(px(16.)).text_color(p.muted_foreground).child(words::line(entry.id)));
    let face = v_flex()
        .overflow_hidden()
        .rounded(size::CARD_RADIUS)
        .border_1()
        .border_color(p.border.opacity(0.6))
        .bg(p.card.opacity(0.4))
        .when(!chosen, |el| {
            let hover = p.foreground.opacity(0.2);
            el.group_hover(group.clone(), move |s| s.border_color(hover))
        })
        .child(
            div()
                .relative()
                .w(inner)
                .h(inner * 0.625)
                .overflow_hidden()
                .border_b_1()
                .border_color(p.border)
                .child(window_picture(palette_of(entry.id), inner)),
        )
        .child(words_block);
    let (pick_acts, restore) = (acts.clone(), picks.clone());
    v_flex()
        .id(group.clone())
        .group(group)
        .debug_selector(move || format!("theme-option-{}", entry.id))
        .relative()
        .w(width)
        .cursor_pointer()
        .on_hover(move |hovered, window, cx| {
            let id = if *hovered { entry.id } else { restore.shown(system_dark(window)).id };
            if theme::current(cx).id != id {
                theme::set_theme(id, cx);
                window.refresh();
            }
        })
        .on_click(move |_, _, cx| pick_acts.set_preferences(json!({ field: entry.id }), cx))
        .child(face)
        .children(ring(chosen, cx))
}

/// `SizePicker`: Default, which leaves every surface its own size, then each size the table offers.
fn size_select(ctx: &Ctx, p: &Value, field: &'static str, sizes: &[i64], window: &mut Window, cx: &mut App) -> Select<String> {
    let mut choices = vec![(String::new(), SharedString::new_static(words::DEFAULT))];
    choices.extend(sizes.iter().map(|s| (s.to_string(), SharedString::from(words::px(*s)))));
    let value = p[field].as_i64().map(|s| s.to_string()).unwrap_or_default();
    let acts = ctx.acts.clone();
    choice_select(SharedString::from(format!("appearance-{field}")), choices, &value, window, cx, move |next, cx| {
        let size = next.parse::<i64>().map_or(Value::Null, Value::from);
        acts.set_preferences(json!({ field: size }), cx)
    })
}

/// `FontPicker`: the families installed on this computer, read once, with Default first; a family picked that is not
/// installed stays listed.
fn font_select(ctx: &Ctx, p: &Value, field: &'static str, families: &[String], window: &mut Window, cx: &mut App) -> Select<String> {
    let value = p[field].as_str().unwrap_or_default();
    let mut choices = vec![(String::new(), SharedString::new_static(words::DEFAULT))];
    if !value.is_empty() && !families.iter().any(|f| f == value) {
        choices.push((value.to_owned(), SharedString::from(value.to_owned())));
    }
    choices.extend(families.iter().map(|f| (f.clone(), SharedString::from(f.clone()))));
    let acts = ctx.acts.clone();
    choice_select(SharedString::from(format!("appearance-{field}")), choices, value, window, cx, move |next, cx| {
        acts.set_preferences(json!({ field: next }), cx)
    })
}

/// `fontFamilies`: every installed family once, sorted as a person reads them; a dotted name is the system's own.
fn families_of(names: Vec<String>) -> Vec<String> {
    let mut families: Vec<String> =
        names.into_iter().map(|n| n.trim().to_owned()).filter(|n| !n.is_empty() && !n.starts_with('.')).collect();
    families.sort();
    families.dedup();
    families.sort_by_key(|f| f.to_lowercase());
    families
}

/// `TypeSample`: a reply with a line of code, in the face and size picked.
fn type_sample(p: &Value, cx: &App) -> impl IntoElement {
    let palette = cx.palette();
    let face = p["appFont"].as_str().filter(|f| !f.is_empty()).map(|f| SharedString::from(f.to_owned()));
    let size = p["textSize"].as_i64();
    div()
        .debug_selector(|| "type-sample".into())
        .rounded(size::CARD_RADIUS)
        .border_1()
        .border_color(palette.border.opacity(0.6))
        .bg(palette.card.opacity(0.4))
        .px(size::CARD_INSET)
        .py_3()
        .when_some(face, |el, face| el.font_family(face))
        // `text-[length:var(--font-size-chat,0.875rem)] leading-relaxed text-foreground/80`.
        .text_size(px(size.unwrap_or(14) as f32))
        .line_height(relative(1.625))
        .text_color(palette.foreground.opacity(0.8))
        .child(TextView::markdown("type-sample", words::SAMPLE))
}

pub fn cards(ctx: &Ctx, window: &mut Window, cx: &mut App) -> Vec<CardData> {
    let Some(p) = record(ctx) else { return Vec::new() };
    let picks = Picks::of(&p);
    let side = picks.side(system_dark(window));
    let width = column_width(window, cx);
    let column = width.get();
    let mode_lede = words::mode_lede(&here_name(ctx.data));

    let tile = (column - px(24.)) / 3.;
    let modes = div()
        .relative()
        .debug_selector(|| "mode-picker".into())
        .child(measure(width))
        .child(h_flex().gap_3().children(MODES.map(|mode| mode_tile(mode, &picks, tile, &ctx.acts, cx))));

    // `repeat(auto-fill, minmax(13rem, 1fr))` with a 12 px gap.
    let cols = (((column + px(12.)) / px(220.)).floor() as usize).max(1);
    let card = (column - px(12.) * (cols - 1) as f32) / cols as f32;
    let themes = h_flex()
        .debug_selector(|| "theme-picker".into())
        .flex_wrap()
        .items_start()
        .gap_3()
        .children(THEMES.iter().filter(|t| t.side == side).map(|t| theme_card(t, &picks, card, &ctx.acts, cx)));
    let side_word = if side == Side::Dark { words::mode("dark") } else { words::mode("light") };

    let mut cards = vec![
        CardData::new("mode")
            .head(words::MODE)
            .lede(mode_lede.clone())
            .search(vec![Item::row("mode", words::MODE, Some(&mode_lede), |row| row)])
            .body(modes),
        CardData::new("theme")
            .head(words::themes_of(side_word))
            .lede(words::THEME_LEDE)
            .search(vec![Item::row("theme", words::THEME, Some(words::THEME_LEDE), |row| row)])
            .body(themes),
    ];
    if !SOLID {
        cards.push(CardData::new("glass").head(words::GLASS).lede(words::GLASS_LEDE).item(Item::row(
            "transparency",
            words::TRANSPARENCY,
            Some(words::TRANSPARENCY_NOTE),
            |row| reset_to(row.control(field_switch(ctx, &p, "transparency", "transparency")), ctx, &p, "transparency", json!(true)),
        )));
    }

    let families: Entity<Vec<String>> =
        window.use_keyed_state("appearance-families", cx, |_, cx| families_of(cx.text_system().all_font_names()));
    let families = families.read(cx).clone();
    let app_font = font_select(ctx, &p, "appFont", &families, window, cx);
    let text_size = size_select(ctx, &p, "textSize", &TEXT_SIZES, window, cx);
    let code_font = font_select(ctx, &p, "codeFont", &families, window, cx);
    let code_size = size_select(ctx, &p, "codeSize", &CODE_SIZES, window, cx);
    cards.push(CardData::new("fonts").head(words::TYPE).lede(words::TYPE_LEDE).body(type_sample(&p, cx)).items([
        Item::row("app-font", words::APP_FONT, Some(words::APP_FONT_NOTE), |row| {
            reset_to(row.control(app_font), ctx, &p, "appFont", json!(""))
        }),
        Item::row("text-size", words::TEXT_SIZE, Some(words::TEXT_SIZE_NOTE), |row| {
            reset_to(row.control(text_size), ctx, &p, "textSize", Value::Null)
        }),
        Item::row("code-font", words::CODE_FONT, Some(words::CODE_FONT_NOTE), |row| {
            reset_to(row.control(code_font), ctx, &p, "codeFont", json!(""))
        }),
        Item::row("code-size", words::CODE_SIZE, Some(words::CODE_SIZE_NOTE), |row| {
            reset_to(row.control(code_size), ctx, &p, "codeSize", Value::Null)
        }),
    ]));
    cards
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pick_off_its_side_draws_the_sides_default() {
        assert_eq!(theme_for(Side::Dark, "paper").id, "graphite");
        assert_eq!(theme_for(Side::Light, "frost").id, "frost");
        let picks = Picks { mode: "system".into(), light: "linen".into(), dark: "moss".into() };
        assert_eq!(picks.shown(true).id, "moss");
        assert_eq!(picks.shown(false).id, "linen");
        assert_eq!(words::mode_lede("box"), "Light, dark, or whichever side box is on.");
    }

    #[test]
    fn families_read_once_each_in_reading_order() {
        let names = vec!["zed mono".into(), ".SF NS".into(), "Arial".into(), "arial".into(), "Arial".into(), " ".into()];
        assert_eq!(families_of(names), ["Arial", "arial", "zed mono"]);
    }
}
