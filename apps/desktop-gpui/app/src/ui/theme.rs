// SPDX-License-Identifier: AGPL-3.0-only
//! wsp's themes: every theme the web app ships, each one gpui-kit theme file in `themes/` whose `colors` feed
//! gpui-kit's own tokens and whose `wsp` block holds the tokens gpui-kit has no name for. Switching one restyles every
//! window at once.

use std::rc::Rc;

use gpui_kit::component::{Theme, ThemeConfig, ThemeMode, ThemeRegistry};
use gpui_kit::{
    point, px, AnyElement, App, Bounds, BoxShadow, Element, ElementId, Global, GlobalElementId, Hsla, InspectorElementId, IntoElement,
    LayoutId, Pixels, Rgba, SharedString, Window,
};
use serde::Deserialize;

/// Which side of the app a theme draws: the side a window's own appearance follows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Light,
    Dark,
}

/// One theme as the web app lists it in `apps/web/src/themes/index.ts`.
pub struct ThemeEntry {
    pub id: &'static str,
    pub word: &'static str,
    pub side: Side,
    json: &'static str,
}

macro_rules! theme {
    ($id:literal, $word:literal, $side:ident) => {
        ThemeEntry { id: $id, word: $word, side: Side::$side, json: include_str!(concat!("themes/", $id, ".json")) }
    };
}

/// In the web app's order: the light side, then the dark.
pub const THEMES: [ThemeEntry; 8] = [
    theme!("paper", "Paper", Light),
    theme!("linen", "Linen", Light),
    theme!("frost", "Frost", Light),
    theme!("graphite", "Graphite", Dark),
    theme!("denim", "Denim", Dark),
    theme!("tungsten", "Tungsten", Dark),
    theme!("moss", "Moss", Dark),
    theme!("pitch", "Pitch", Dark),
];

/// Each side's default, as the web app's preferences default to them.
pub const LIGHT: &str = "paper";
pub const DARK: &str = "graphite";

/// One shadow as a theme file writes it, CSS's `box-shadow` in fields.
#[derive(Clone, Debug, Deserialize)]
struct ShadowSpec {
    #[serde(default)]
    inset: bool,
    x: f32,
    y: f32,
    #[serde(default)]
    blur: f32,
    #[serde(default)]
    spread: f32,
    color: String,
}

fn shadows(specs: &[ShadowSpec]) -> Vec<BoxShadow> {
    specs
        .iter()
        .map(|s| BoxShadow {
            color: color(&s.color),
            offset: point(px(s.x), px(s.y)),
            // CSS's blur radius is twice the Gaussian's sigma, which is what GPUI's shadow takes.
            blur_radius: px(s.blur / 2.),
            spread_radius: px(s.spread),
            inset: s.inset,
        })
        .collect()
}

fn color(hex: &str) -> Hsla {
    Rgba::try_from(hex).unwrap_or_else(|e| panic!("{hex} is not a #rrggbbaa color: {e}")).into()
}

macro_rules! palette {
    ($($field:ident),* $(,)?) => {
        #[derive(Deserialize)]
        #[serde(rename_all = "kebab-case")]
        struct PaletteSpec {
            $($field: String,)*
            dialog_shadow: Vec<ShadowSpec>,
            popover_shadow: Vec<ShadowSpec>,
            control_shadow: Vec<ShadowSpec>,
            tooltip_shadow: Vec<ShadowSpec>,
            composer_shadow: Vec<ShadowSpec>,
            thumb_shadow: Vec<ShadowSpec>,
            dropdown_shadow: Vec<ShadowSpec>,
            terminal_ansi: Vec<String>,
        }

        /// wsp's tokens, named as `apps/web/src/index.css` and the theme sheets name them, resolved to sRGB.
        #[derive(Clone, Debug)]
        pub struct Palette {
            $(pub $field: Hsla,)*
            pub dialog_shadow: Vec<BoxShadow>,
            pub popover_shadow: Vec<BoxShadow>,
            pub control_shadow: Vec<BoxShadow>,
            pub tooltip_shadow: Vec<BoxShadow>,
            pub composer_shadow: Vec<BoxShadow>,
            /// `shadow-sm/5`, a switch's thumb.
            pub thumb_shadow: Vec<BoxShadow>,
            /// `dropdown-glass`'s shadow, and in dark `popover-shadow` over it: a select's list.
            pub dropdown_shadow: Vec<BoxShadow>,
            /// `--terminal-ansi-0` to `-15`: the terminal's first sixteen colours when its config names none.
            pub terminal_ansi: Vec<Hsla>,
        }

        impl From<&PaletteSpec> for Palette {
            fn from(spec: &PaletteSpec) -> Self {
                Palette {
                    $($field: color(&spec.$field),)*
                    dialog_shadow: shadows(&spec.dialog_shadow),
                    popover_shadow: shadows(&spec.popover_shadow),
                    control_shadow: shadows(&spec.control_shadow),
                    tooltip_shadow: shadows(&spec.tooltip_shadow),
                    composer_shadow: shadows(&spec.composer_shadow),
                    thumb_shadow: shadows(&spec.thumb_shadow),
                    dropdown_shadow: shadows(&spec.dropdown_shadow),
                    terminal_ansi: spec.terminal_ansi.iter().map(|c| color(c)).collect(),
                }
            }
        }
    };
}

palette!(
    background,
    foreground,
    card,
    card_hover,
    muted,
    muted_foreground,
    accent,
    primary,
    primary_foreground,
    primary_hover,
    slate_accent,
    scrollbar_thumb,
    scrollbar_thumb_hover,
    border,
    input,
    input_fill,
    input_fill_hover,
    keycap_top,
    chip_top,
    skeleton_highlight,
    popover,
    popover_edge,
    ring,
    success,
    warning,
    warning_foreground,
    yellow_foreground,
    error,
    error_foreground,
    destructive,
    on_destructive,
    info_foreground,
    placeholder,
    sidebar,
    sidebar_foreground,
    sidebar_muted_foreground,
    sidebar_row_hover,
    sidebar_row_selected,
    sidebar_row_edge,
    sidebar_border,
    sidebar_whisper,
    top_row_meta,
    sidebar_prose,
    sidebar_icon_color,
    status_input,
    status_working,
    status_failed,
    status_done,
    composer_outline,
    composer_highlight,
    composer_menu_outline,
    button_light,
    destructive_ink,
    sidebar_canvas,
    sidebar_card,
    sidebar_hover,
    sidebar_hover_ink,
    sidebar_fill,
    sidebar_quiet,
    sidebar_input,
    sidebar_control_surface,
    sidebar_row_active,
    sidebar_rail,
    sidebar_row_rest,
    pr_merged,
    hero_field,
    hue_red,
    hue_orange,
    hue_amber,
    hue_green,
    hue_teal,
    hue_blue,
    hue_violet,
    hue_pink,
    terminal_background,
    terminal_foreground,
    terminal_cursor,
    terminal_selection,
);

#[derive(Deserialize)]
struct FileSpec {
    themes: Vec<ConfigSpec>,
}

#[derive(Deserialize)]
struct ConfigSpec {
    wsp: PaletteSpec,
}

impl ThemeEntry {
    /// gpui-kit's name for the theme, which its registry keys on.
    pub fn name(&self) -> SharedString {
        SharedString::new_static(self.word)
    }

    pub fn palette(&self) -> Palette {
        let file: FileSpec = serde_json::from_str(self.json).unwrap_or_else(|e| panic!("themes/{}.json: {e}", self.id));
        Palette::from(&file.themes[0].wsp)
    }
}

pub fn theme_by_id(id: &str) -> Option<&'static ThemeEntry> {
    THEMES.iter().find(|t| t.id == id)
}

/// The theme every window draws in, and its palette.
pub struct WspTheme {
    pub entry: &'static ThemeEntry,
    pub palette: Palette,
}

impl Global for WspTheme {}

pub trait ActivePalette {
    fn palette(&self) -> &Palette;
}

impl ActivePalette for App {
    fn palette(&self) -> &Palette {
        &self.global::<WspTheme>().palette
    }
}

impl Palette {
    /// `[data-app-sidebar]` in `index.css`: the page's roles drawn in the sidebar's own tokens.
    pub fn sidebar_scope(&self) -> Palette {
        Palette {
            background: self.sidebar_canvas,
            foreground: self.sidebar_foreground,
            card: self.sidebar_card,
            accent: self.sidebar_hover,
            muted: self.sidebar_fill,
            muted_foreground: self.sidebar_quiet,
            input: self.sidebar_input,
            ..self.clone()
        }
    }
}

/// Runs `f` with `palette` as the active one: a view's render that builds a scope's elements reads the scope's tokens.
pub fn within<C: std::ops::DerefMut<Target = App>, R>(palette: Palette, cx: &mut C, f: impl FnOnce(&mut C) -> R) -> R {
    let page = std::mem::replace(&mut (**cx).global_mut::<WspTheme>().palette, palette);
    let out = f(cx);
    (**cx).global_mut::<WspTheme>().palette = page;
    out
}

/// Lays out, prepaints and paints `child` with `palette` as the active one, so a shared piece drawn inside reads the
/// scope's tokens, as a CSS scope's custom properties reach every element under it.
pub fn scoped(palette: Palette, child: impl IntoElement) -> Scoped {
    Scoped { palette, kit: None, child: child.into_any_element() }
}

/// [`scoped`] in the whole theme `id` names, gpui-kit's own tokens too (a field's caret, selection and ring): a window
/// that stands in one theme whatever the app's, which it leaves as it was.
pub fn scoped_theme(id: &str, child: impl IntoElement, cx: &App) -> Scoped {
    let entry = theme_by_id(id).unwrap_or_else(|| panic!("wsp has no theme {id}"));
    let mut kit = Theme::global(cx).clone();
    take(&mut kit, &config_of(entry, cx));
    Scoped { palette: entry.palette(), kit: Some(kit), child: child.into_any_element() }
}

pub struct Scoped {
    palette: Palette,
    kit: Option<Theme>,
    child: AnyElement,
}

impl Scoped {
    fn within<R>(&mut self, cx: &mut App, f: impl FnOnce(&mut AnyElement, &mut App) -> R) -> R {
        std::mem::swap(&mut cx.global_mut::<WspTheme>().palette, &mut self.palette);
        if let Some(kit) = self.kit.as_mut() {
            std::mem::swap(Theme::global_mut(cx), kit);
        }
        let out = f(&mut self.child, cx);
        if let Some(kit) = self.kit.as_mut() {
            std::mem::swap(Theme::global_mut(cx), kit);
        }
        std::mem::swap(&mut cx.global_mut::<WspTheme>().palette, &mut self.palette);
        out
    }
}

impl IntoElement for Scoped {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl Element for Scoped {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        (self.within(cx, |child, cx| child.request_layout(window, cx)), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.within(cx, |child, cx| child.prepaint(window, cx));
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.within(cx, |child, cx| child.paint(window, cx));
    }
}

/// The system font, which GPUI names `.SystemUIFont`: SF Pro on macOS.
const SYSTEM: &str = ".SystemUIFont";

/// `--font-sans-system` in `apps/web/src/index.css`. Chromium skips `-apple-system`, which it does not know, and draws
/// `BlinkMacSystemFont` on macOS and `system-ui` everywhere in the system font.
pub const SANS: [&str; 6] = ["-apple-system", "BlinkMacSystemFont", "Segoe UI", "Inter", "Liberation Sans", "system-ui"];
/// `--font-mono-system`. Chromium has no `ui-monospace` either, so on macOS it draws SF Mono only where a family of that
/// name is installed.
pub const MONO: [&str; 7] = ["ui-monospace", "SF Mono", "SFMono-Regular", "Menlo", "Consolas", "DejaVu Sans Mono", "Liberation Mono"];

/// The first face of a CSS stack the machine has, as Chromium picks it; `None` leaves gpui-kit's own default.
pub fn first_installed(stack: &[&str], installed: &[String], mac: bool) -> Option<SharedString> {
    stack.iter().find_map(|&face| match face {
        "BlinkMacSystemFont" if mac => Some(SharedString::new_static(SYSTEM)),
        "system-ui" => Some(SharedString::new_static(SYSTEM)),
        _ => installed.iter().any(|f| f == face).then(|| SharedString::from(face.to_owned())),
    })
}

/// Registers every theme with gpui-kit, takes the web app's font stacks and draws the side's default. Call after
/// `gpui_kit::init`.
pub fn init(side: Side, cx: &mut App) {
    let registry = ThemeRegistry::global_mut(cx);
    for entry in &THEMES {
        registry.load_themes_from_str(entry.json).unwrap_or_else(|e| panic!("themes/{}.json: {e}", entry.id));
    }
    let installed = cx.text_system().all_font_names();
    let mac = cfg!(target_os = "macos");
    let (sans, mono) = (first_installed(&SANS, &installed, mac), first_installed(&MONO, &installed, mac));
    Theme::update(cx, |theme| {
        if let Some(sans) = sans {
            theme.font_family = sans;
        }
        if let Some(mono) = mono {
            theme.mono_font_family = mono;
        }
    });
    set_theme(if side == Side::Dark { DARK } else { LIGHT }, cx);
}

/// Draws every window in the theme `id` names.
pub fn set_theme(id: &str, cx: &mut App) {
    let entry = theme_by_id(id).unwrap_or_else(|| panic!("wsp has no theme {id}"));
    let config = config_of(entry, cx);
    cx.set_global(WspTheme { entry, palette: entry.palette() });
    Theme::update(cx, |theme| take(theme, &config));
}

fn config_of(entry: &ThemeEntry, cx: &App) -> Rc<ThemeConfig> {
    let config: Rc<ThemeConfig> =
        ThemeRegistry::global(cx).themes().get(&entry.name()).cloned().unwrap_or_else(|| panic!("{} is not registered", entry.word));
    debug_assert_eq!(config.mode, if entry.side == Side::Dark { ThemeMode::Dark } else { ThemeMode::Light });
    config
}

fn take(theme: &mut Theme, config: &Rc<ThemeConfig>) {
    theme.apply_config(config);
    // wsp's focused field tints its edge in the ring color and draws no ring outside it.
    theme.focus_ring = false;
}

pub fn current(cx: &App) -> &'static ThemeEntry {
    cx.global::<WspTheme>().entry
}

#[cfg(test)]
mod tests {
    use super::{first_installed, Side, MONO, SANS, THEMES};
    use gpui_kit::component::{ThemeMode, ThemeSet};

    #[test]
    fn every_theme_parses_on_its_side_and_feeds_gpui_kit_the_same_colors() {
        for entry in &THEMES {
            let palette = entry.palette();
            let set: ThemeSet = serde_json::from_str(entry.json).unwrap();
            let config = &set.themes[0];
            assert_eq!(config.name.as_ref(), entry.word, "{}", entry.id);
            assert_eq!(config.mode, if entry.side == Side::Dark { ThemeMode::Dark } else { ThemeMode::Light }, "{}", entry.id);
            let kit = |value: &Option<gpui_kit::SharedString>| super::color(value.as_deref().expect("a color"));
            let colors = &config.colors;
            for (name, kit, ours) in [
                ("background", kit(&colors.background), palette.background),
                ("foreground", kit(&colors.foreground), palette.foreground),
                ("border", kit(&colors.border), palette.border),
                ("input", kit(&colors.input), palette.input),
                ("muted-foreground", kit(&colors.muted_foreground), palette.muted_foreground),
                ("accent", kit(&colors.accent), palette.accent),
                ("primary", kit(&colors.primary), palette.primary),
                ("popover", kit(&colors.popover), palette.popover),
                ("ring", kit(&colors.ring), palette.ring),
                ("sidebar", kit(&colors.sidebar), palette.sidebar),
            ] {
                assert_eq!(kit, ours, "{}: gpui-kit's {name} and the wsp block's differ", entry.id);
            }
        }
    }

    const SHEETS: [(&str, &str); 8] = [
        ("paper", include_str!("../../../../web/src/themes/paper.css")),
        ("linen", include_str!("../../../../web/src/themes/linen.css")),
        ("frost", include_str!("../../../../web/src/themes/frost.css")),
        ("graphite", include_str!("../../../../web/src/themes/graphite.css")),
        ("denim", include_str!("../../../../web/src/themes/denim.css")),
        ("tungsten", include_str!("../../../../web/src/themes/tungsten.css")),
        ("moss", include_str!("../../../../web/src/themes/moss.css")),
        ("pitch", include_str!("../../../../web/src/themes/pitch.css")),
    ];

    /// Every token a web sheet writes as a plain hex must stand in the wsp block as that hex.
    #[test]
    fn every_hex_a_web_sheet_writes_is_the_wsp_blocks() {
        for (id, css) in SHEETS {
            let entry = THEMES.iter().find(|t| t.id == id).unwrap();
            let file: serde_json::Value = serde_json::from_str(entry.json).unwrap();
            let wsp = &file["themes"][0]["wsp"];
            for line in css.lines() {
                let Some((name, value)) = line.trim().strip_prefix("--").and_then(|l| l.split_once(':')) else { continue };
                let value = value.trim().trim_end_matches(';').to_ascii_lowercase();
                if let (Some(ours), true) = (wsp[name].as_str(), value.len() == 7 && value.starts_with('#')) {
                    assert_eq!(ours, format!("{value}ff"), "{id}: --{name}");
                }
            }
        }
    }

    /// `--app-scrollbar-thumb` and its hover: grays on the light side, white at 8% and 12% on the dark, as `index.css`
    /// writes them.
    #[test]
    fn the_scrollbar_thumb_is_the_webs_on_each_side() {
        let css = include_str!("../../../../web/src/index.css");
        assert!(css.contains("--app-scrollbar-thumb: rgb(217 217 217);"), "the web's light thumb moved");
        assert!(css.contains("--app-scrollbar-thumb: rgb(255 255 255 / 8%);"), "the web's dark thumb moved");
        assert!(css.contains("--app-scrollbar-thumb-hover: rgb(191 191 191);"), "the web's light hover moved");
        assert!(css.contains("--app-scrollbar-thumb-hover: rgb(255 255 255 / 12%);"), "the web's dark hover moved");
        for entry in &THEMES {
            let dark = entry.side == Side::Dark;
            let (rest, hover) = if dark { ("#ffffff14", "#ffffff1f") } else { ("#d9d9d9ff", "#bfbfbfff") };
            assert_eq!(entry.palette().scrollbar_thumb, super::color(rest), "{}", entry.id);
            assert_eq!(entry.palette().scrollbar_thumb_hover, super::color(hover), "{} hovered", entry.id);
        }
    }

    fn installed(names: &[&str]) -> Vec<String> {
        names.iter().map(|n| n.to_string()).collect()
    }

    #[test]
    fn linux_takes_the_first_installed_face_of_each_stack() {
        let linux = installed(&["DejaVu Sans", "DejaVu Sans Mono", "Liberation Mono", "Liberation Sans"]);
        assert_eq!(first_installed(&SANS, &linux, false).as_deref(), Some("Liberation Sans"));
        assert_eq!(first_installed(&MONO, &linux, false).as_deref(), Some("DejaVu Sans Mono"));
        let inter = installed(&["Inter", "Liberation Sans"]);
        assert_eq!(first_installed(&SANS, &inter, false).as_deref(), Some("Inter"));
    }

    #[test]
    fn the_mac_takes_its_system_font_and_sf_mono_where_it_has_it() {
        let mac = installed(&["Helvetica", "Inter", "Menlo"]);
        assert_eq!(first_installed(&SANS, &mac, true).as_deref(), Some(".SystemUIFont"));
        assert_eq!(first_installed(&MONO, &mac, true).as_deref(), Some("Menlo"));
        let sf = installed(&["Menlo", "SF Mono"]);
        assert_eq!(first_installed(&MONO, &sf, true).as_deref(), Some("SF Mono"));
    }
}
