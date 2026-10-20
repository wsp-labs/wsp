// SPDX-License-Identifier: AGPL-3.0-only
//! What the frame's surfaces paint on the Mac, where the window stands on macOS's material: `.desktop-mac` in
//! `apps/web/src/index.css`. A dark theme lays its material ground over the glass at a share per region; a light theme
//! keeps its solid ground; the person's solid setting, or the computer's Reduce transparency, takes the glass away.

use gpui_kit::{hsla, rgb, Hsla, Rgba};

use crate::ui::Palette;

/// A dark theme's material, from its sheet in `apps/web/src/themes`: the ground its regions lay over the glass, and
/// the ink its sidebar takes there.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Material {
    pub ground: Hsla,
    pub ink: Hsla,
}

/// `--material-ground` and `--glass-ink` of each dark theme; graphite's ink is `oklch(87% 0 none)`, #d4d4d4.
pub fn material(theme: &str) -> Option<Material> {
    let (ground, ink) = match theme {
        "graphite" => (0x060606, 0xd4d4d4),
        "denim" => (0x0f1117, 0xd2d7e2),
        "tungsten" => (0x0d0c0b, 0xdcd5ca),
        "moss" => (0x121614, 0xd6dbd5),
        "pitch" => (0x000000, 0xd0d0d0),
        _ => return None,
    };
    Some(Material { ground: rgb(ground).into(), ink: rgb(ink).into() })
}

/// `.desktop-mac.dark`'s shares of the ground: the centre column, the right panel, the sidebar, and the panel's lines
/// and cards in white.
pub const CENTRE: f32 = 0.67;
pub const PANEL: f32 = 0.74;
pub const SIDEBAR: f32 = 0.40;
pub const LINE: f32 = 0.10;
pub const CARD: f32 = 0.02;
/// `.desktop-mac.dark:not(.solid)`'s share of the glass ink in the page's muted ink, over the ground.
pub const MUTED: f32 = 0.85;

/// Where the window is drawn and how.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Look {
    pub mac: bool,
    /// The page draws glass: the person's transparency on and the computer's Reduce transparency off.
    pub glass: bool,
}

/// What each region paints.
#[derive(Clone, Debug, PartialEq)]
pub struct Fills {
    pub window: Option<Hsla>,
    pub sidebar: Hsla,
    pub centre: Hsla,
    pub panel: Hsla,
    /// The page's ground, cards, tints, edges and fields inside the panel and on the settings page.
    pub panel_ground: Hsla,
    pub panel_card: Hsla,
    pub panel_muted: Hsla,
    pub panel_line: Hsla,
    pub panel_input: Hsla,
    /// The noise the web app lays on its surfaces, which every Mac dark rule's background shorthand clears.
    pub grain: bool,
    /// The sidebar's quiet ink and the page's muted one, where the glass moves them.
    pub sidebar_ink: Option<Hsla>,
    pub muted_foreground: Option<Hsla>,
    /// The window takes macOS's material behind it.
    pub vibrancy: bool,
}

fn share(color: Hsla, share: f32) -> Hsla {
    Hsla { a: color.a * share, ..color }
}

pub fn fills(look: Look, theme: &str, p: &Palette) -> Fills {
    let plain = Fills {
        window: Some(p.background),
        sidebar: p.sidebar,
        centre: p.background,
        panel: p.background,
        panel_ground: p.background,
        panel_card: p.card,
        panel_muted: p.muted,
        panel_line: p.border,
        panel_input: p.input,
        grain: true,
        sidebar_ink: None,
        muted_foreground: None,
        vibrancy: false,
    };
    if !look.mac {
        return plain;
    }
    let Some(material) = material(theme) else {
        // A light theme keeps its solid ground on the Mac; only its window's own canvas is clear over the glass.
        return Fills { window: (!look.glass).then_some(p.background), vibrancy: look.glass, ..plain };
    };
    let white = hsla(0., 0., 1., 1.);
    let shares = if look.glass { (CENTRE, PANEL, SIDEBAR) } else { (1., 1., 1.) };
    Fills {
        window: (!look.glass).then_some(p.background),
        sidebar: share(material.ground, shares.2),
        centre: share(material.ground, shares.0),
        panel: share(material.ground, shares.1),
        panel_ground: gpui_kit::transparent_black(),
        panel_card: share(white, CARD),
        panel_muted: share(white, CARD + 0.01),
        panel_line: share(white, LINE),
        panel_input: share(white, LINE + 0.06),
        grain: false,
        sidebar_ink: look.glass.then_some(material.ink),
        muted_foreground: look.glass.then(|| oklab_mix(material.ink, material.ground, MUTED)),
        vibrancy: look.glass,
    }
}

impl Fills {
    /// `.desktop-mac.dark :is([data-preview-panel-mode="inline"], [data-settings-page])`: the page's palette with the
    /// region's own tokens, so a pane's ground is the panel's fill whatever the look.
    pub fn panel_scope(&self, p: &Palette) -> Palette {
        Palette {
            background: self.panel_ground,
            card: self.panel_card,
            muted: self.panel_muted,
            border: self.panel_line,
            input: self.panel_input,
            ..p.clone()
        }
    }
}

/// CSS `color-mix(in oklab, a share, b)`.
pub fn oklab_mix(a: Hsla, b: Hsla, share: f32) -> Hsla {
    let (a, b) = (oklab(a.into()), oklab(b.into()));
    let mixed = [0, 1, 2].map(|i| a[i] * share + b[i] * (1. - share));
    srgb(mixed).into()
}

fn linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn gamma(c: f32) -> f32 {
    if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1. / 2.4) - 0.055
    }
}

fn oklab(c: Rgba) -> [f32; 3] {
    let (r, g, b) = (linear(c.r), linear(c.g), linear(c.b));
    let l = (0.412_221_47 * r + 0.536_332_55 * g + 0.051_445_995 * b).cbrt();
    let m = (0.211_903_5 * r + 0.680_699_5 * g + 0.107_396_96 * b).cbrt();
    let s = (0.088_302_46 * r + 0.281_718_85 * g + 0.629_978_7 * b).cbrt();
    [
        0.210_454_26 * l + 0.793_617_8 * m - 0.004_072_047 * s,
        1.977_998_5 * l - 2.428_592_2 * m + 0.450_593_7 * s,
        0.025_904_037 * l + 0.782_771_77 * m - 0.808_675_77 * s,
    ]
}

fn srgb([l, a, b]: [f32; 3]) -> Rgba {
    let l_ = (l + 0.396_337_78 * a + 0.215_803_76 * b).powi(3);
    let m_ = (l - 0.105_561_346 * a - 0.063_854_17 * b).powi(3);
    let s_ = (l - 0.089_484_18 * a - 1.291_485_5 * b).powi(3);
    let r = 4.076_741_7 * l_ - 3.307_711_6 * m_ + 0.230_969_94 * s_;
    let g = -1.268_438 * l_ + 2.609_757_4 * m_ - 0.341_319_38 * s_;
    let b = -0.004_196_086_3 * l_ - 0.703_418_6 * m_ + 1.707_614_7 * s_;
    Rgba { r: gamma(r).clamp(0., 1.), g: gamma(g).clamp(0., 1.), b: gamma(b).clamp(0., 1.), a: 1. }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn css(path: &str) -> String {
        std::fs::read_to_string(format!("{}/../../web/src/{path}", env!("CARGO_MANIFEST_DIR"))).unwrap()
    }

    /// The value a sheet gives `--name`, as written.
    fn token(sheet: &str, name: &str) -> String {
        let line = sheet.lines().find(|l| l.trim_start().starts_with(&format!("--{name}:"))).unwrap_or_else(|| panic!("no --{name}"));
        line.split_once(':').unwrap().1.trim().trim_end_matches(';').to_owned()
    }

    fn hex(color: Hsla) -> String {
        let c: Rgba = color.into();
        format!("#{:02x}{:02x}{:02x}", (c.r * 255.).round() as u8, (c.g * 255.).round() as u8, (c.b * 255.).round() as u8)
    }

    #[test]
    fn the_materials_are_the_theme_sheets() {
        for theme in ["graphite", "denim", "tungsten", "moss", "pitch"] {
            let sheet = css(&format!("themes/{theme}.css"));
            let m = material(theme).unwrap();
            assert_eq!(hex(m.ground), token(&sheet, "material-ground"), "{theme} ground");
            let ink = token(&sheet, "glass-ink");
            if ink.starts_with('#') {
                assert_eq!(hex(m.ink), ink, "{theme} ink");
            } else {
                assert_eq!(ink, "oklch(87% 0 none)", "{theme} ink");
                assert_eq!(hex(m.ink), "#d4d4d4");
            }
        }
        for theme in ["paper", "linen", "frost"] {
            assert!(material(theme).is_none(), "{theme} keeps its solid ground");
        }
    }

    #[test]
    fn the_shares_are_the_mac_sheets() {
        let index = css("index.css");
        let block = &index[index.find(".desktop-mac.dark {\n  --material-centre").expect("the Mac's dark shares")..];
        for (name, share) in [("centre", CENTRE), ("panel", PANEL), ("sidebar", SIDEBAR), ("line", LINE), ("card", CARD)] {
            let written: f32 = token(block, &format!("material-{name}")).trim_end_matches('%').parse().unwrap();
            assert_eq!(written / 100., share, "--material-{name}");
        }
        let muted = index
            .lines()
            .find_map(|l| l.trim().strip_prefix("--muted-foreground: color-mix(in oklab, var(--glass-ink) "))
            .expect("the Mac's muted ink");
        assert_eq!(muted.split('%').next().unwrap().parse::<f32>().unwrap() / 100., MUTED, "--muted-foreground");
    }

    #[test]
    fn each_look_fills_its_regions() {
        let graphite = crate::ui::theme::theme_by_id("graphite").unwrap().palette();
        let paper = crate::ui::theme::theme_by_id("paper").unwrap().palette();
        let ground: Hsla = rgb(0x060606).into();

        let off = fills(Look { mac: false, glass: true }, "graphite", &graphite);
        assert_eq!((off.sidebar, off.centre, off.grain, off.vibrancy), (graphite.sidebar, graphite.background, true, false));

        let glass = fills(Look { mac: true, glass: true }, "graphite", &graphite);
        assert_eq!(glass.window, None);
        assert_eq!((glass.sidebar.a, glass.centre.a, glass.panel.a), (0.40, 0.67, 0.74));
        assert_eq!(Hsla { a: 1., ..glass.centre }, ground);
        assert!(glass.vibrancy && !glass.grain);
        assert_eq!(glass.sidebar_ink.map(hex).as_deref(), Some("#d4d4d4"));
        // color-mix(in oklab, #d4d4d4 85%, #060606): Chromium computes oklab(0.757765 0 0), #b0b0b0.
        assert_eq!(glass.muted_foreground.map(hex).as_deref(), Some("#b0b0b0"));

        let solid = fills(Look { mac: true, glass: false }, "graphite", &graphite);
        assert_eq!((solid.sidebar, solid.centre, solid.panel), (ground, ground, ground));
        assert!(!solid.vibrancy && solid.sidebar_ink.is_none() && solid.muted_foreground.is_none());

        let light = fills(Look { mac: true, glass: true }, "paper", &paper);
        assert_eq!((light.sidebar, light.centre, light.window), (paper.sidebar, paper.background, None));
        assert!(light.vibrancy && light.grain);
    }

    /// A pane paints the page's `background` under its bar and its body; inside the panel on a dark theme's material
    /// that ground is clear, so the panel's own share is the one fill under its tab strip and its panes alike.
    #[test]
    fn the_panels_scope_clears_the_ground_only_on_a_dark_material() {
        let tokens = |p: &Palette| [p.background, p.card, p.muted, p.border, p.input];
        let white = |share: f32| hsla(0., 0., 1., share);
        for theme in ["graphite", "moss", "paper"] {
            let page = crate::ui::theme::theme_by_id(theme).unwrap().palette();
            for look in [
                Look { mac: false, glass: false },
                Look { mac: false, glass: true },
                Look { mac: true, glass: true },
                Look { mac: true, glass: false },
            ] {
                let scope = fills(look, theme, &page).panel_scope(&page);
                if look.mac && theme != "paper" {
                    assert_eq!(scope.background.a, 0., "{theme} {look:?}: a pane's ground is clear over the panel's share");
                    assert_eq!(tokens(&scope)[1..], [white(0.02), white(0.03), white(0.10), white(0.16)], "{theme} {look:?}");
                } else {
                    assert_eq!(tokens(&scope), tokens(&page), "{theme} {look:?}: the page's own tokens");
                }
                assert_eq!(scope.foreground, page.foreground, "{theme} {look:?}: the scope moves only the five tokens");
            }
        }
    }
}
