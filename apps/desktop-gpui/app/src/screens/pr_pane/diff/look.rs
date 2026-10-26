// SPDX-License-Identifier: AGPL-3.0-only
//! The code surface's colors, as `DIFF_SURFACE_THEME_UNSAFE_CSS` and `StyledDiffCodeView.tsx` map pierre's surfaces
//! onto the app's tokens, and the element that draws gpui-kit's `Diff` with gpui-kit's theme tokens set to them.

use gpui_kit::component::diff::DiffLook;
use gpui_kit::component::{ActiveTheme as _, Theme};
use gpui_kit::{px, App, Hsla, IntoElement, Rgba};

use crate::ui::theme::Scoped;
use crate::ui::ActivePalette as _;

/// `color-mix(in srgb, a p%, b)`.
pub(super) fn mix(a: Hsla, b: Hsla, p: f32) -> Hsla {
    let (a, b): (Rgba, Rgba) = (a.to_rgb(), b.to_rgb());
    let m = |x: f32, y: f32| x * p + y * (1. - p);
    Rgba { r: m(a.r, b.r), g: m(a.g, b.g), b: m(a.b, b.b), a: m(a.a, b.a) }.into()
}

/// `color-mix(in lab, a p%, b)`, as Chromium mixes: both to CIE Lab under D50, mixed, back to sRGB and clamped.
pub(super) fn mix_lab(a: Hsla, b: Hsla, p: f32) -> Hsla {
    let (a, b) = (lab(a.to_rgb()), lab(b.to_rgb()));
    let m = |i: usize| a[i] * p + b[i] * (1. - p);
    from_lab([m(0), m(1), m(2)])
}

const SRGB_TO_XYZ: [[f32; 3]; 3] =
    [[0.412_390_8, 0.357_584_33, 0.180_480_8], [0.212_639, 0.715_168_7, 0.072_192_32], [0.019_330_818, 0.119_194_78, 0.950_532_14]];
const XYZ_TO_SRGB: [[f32; 3]; 3] =
    [[3.240_97, -1.537_383_2, -0.498_610_76], [-0.969_243_65, 1.875_967_5, 0.041_555_06], [0.055_630_08, -0.203_976_96, 1.056_971_5]];
/// Bradford from D65 to D50, and back.
const D65_TO_D50: [[f32; 3]; 3] = [
    [1.047_929_8, 0.022_946_793, -0.050_192_23],
    [0.029_627_815, 0.990_434_5, -0.017_073_825],
    [-0.009_243_058, 0.015_055_145, 0.751_874_3],
];
const D50_TO_D65: [[f32; 3]; 3] =
    [[0.955_473_4, -0.023_098_537, 0.063_259_31], [-0.028_369_706, 1.009_995_5, 0.021_041_399], [0.012_314_002, -0.020_507_697, 1.330_366]];
const D50: [f32; 3] = [0.964_295_7, 1., 0.825_104_6];
const EPSILON: f32 = 216. / 24389.;
const KAPPA: f32 = 24389. / 27.;

fn times(m: &[[f32; 3]; 3], v: [f32; 3]) -> [f32; 3] {
    m.map(|r| r[0] * v[0] + r[1] * v[1] + r[2] * v[2])
}

fn lab(c: Rgba) -> [f32; 3] {
    let linear = |v: f32| if v <= 0.040_45 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) };
    let xyz = times(&D65_TO_D50, times(&SRGB_TO_XYZ, [linear(c.r), linear(c.g), linear(c.b)]));
    let f = |i: usize| {
        let v = xyz[i] / D50[i];
        if v > EPSILON {
            v.cbrt()
        } else {
            (KAPPA * v + 16.) / 116.
        }
    };
    let (x, y, z) = (f(0), f(1), f(2));
    [116. * y - 16., 500. * (x - y), 200. * (y - z)]
}

fn from_lab([l, a, b]: [f32; 3]) -> Hsla {
    let fy = (l + 16.) / 116.;
    let (fx, fz) = (a / 500. + fy, fy - b / 200.);
    let cube = |f: f32| if f.powi(3) > EPSILON { f.powi(3) } else { (116. * f - 16.) / KAPPA };
    let y = if l > KAPPA * EPSILON { fy.powi(3) } else { l / KAPPA };
    let xyz = [cube(fx) * D50[0], y * D50[1], cube(fz) * D50[2]];
    let gamma = |v: f32| if v <= 0.003_130_8 { 12.92 * v } else { 1.055 * v.powf(1. / 2.4) - 0.055 }.clamp(0., 1.);
    let [r, g, b] = times(&XYZ_TO_SRGB, times(&D50_TO_D65, xyz)).map(gamma);
    Rgba { r, g, b, a: 1. }.into()
}

/// The colors the code surface reads, resolved for the active theme.
#[derive(Clone, Copy)]
pub(super) struct DiffColors {
    /// `--code-background`, which the code view sets to the page's background.
    pub background: Hsla,
    pub foreground: Hsla,
    /// `--code-foreground`, the page's ink, which a changed line's changed characters are drawn in.
    pub ink: Hsla,
    /// Pierre's `--diffs-addition-base`, `--diffs-deletion-base` and `--diffs-modified-base`: the bars, the file
    /// chevron and the selected line's edge.
    pub added: Hsla,
    pub deleted: Hsla,
    pub modified: Hsla,
    /// `--diffs-fg-number`.
    pub number: Hsla,
    /// The file head's hover edge: `color-mix(in srgb, var(--code-foreground) 24%, transparent)`.
    pub edge: Hsla,
    /// The selected line's tint.
    pub selected: Hsla,
    /// The "unmodified lines" bar's words and rules.
    pub gap_ink: Hsla,
    pub gap_rule: Hsla,
}

impl DiffColors {
    pub fn of(cx: &App) -> Self {
        let p = cx.palette();
        let theme = cx.theme();
        let dark = theme.mode.is_dark();
        let status = &theme.highlight_theme.style.status;
        let added = status.success(cx);
        let deleted = status.error(cx);
        let modified = status.info(cx);
        let background = p.background;
        let foreground = theme.highlight_theme.style.editor_foreground.unwrap_or(p.foreground);
        let selected =
            if dark { mix(background, mix(background, modified, 0.7), 0.8) } else { mix(background, mix(background, modified, 0.5), 0.88) };
        DiffColors {
            background,
            foreground,
            ink: p.foreground,
            added,
            deleted,
            modified,
            number: mix_lab(foreground, background, 0.65),
            edge: p.foreground.opacity(0.24),
            selected,
            gap_ink: mix(p.foreground, background, 0.52),
            gap_rule: mix(background, p.foreground, 0.92),
        }
    }
}

/// Today's row fills, as pierre's stylesheet and `DIFF_SURFACE_THEME_UNSAFE_CSS` resolve them (light / dark):
/// a changed line is `--code-background` at 88% / 80% in lab over the app's success or destructive at 50% / 70% over
/// it, its number cell 91% / 85% over 35% / 60%; a selected line takes the modified color the same way
/// (`StyledDiffCodeView.tsx`). Pierre's own `--diffs-bg-*` resolve on its host, where the overrides do not reach:
/// the deleted bar's stripe is the deletion color at 12% / 20%, the buffer black or white at 8%, the context (a note's
/// fill) black at 1.5% or white at 7.5%, and its number lane that over the background at 90% / 45%.
pub(super) fn diff_look(cx: &App) -> DiffLook {
    let p = cx.palette();
    let c = DiffColors::of(cx);
    let dark = cx.theme().mode.is_dark();
    let bg = c.background;
    let pick = |light: f32, dark_: f32| if dark { dark_ } else { light };
    let line = |base: Hsla| mix_lab(bg, mix(bg, base, pick(0.5, 0.7)), pick(0.88, 0.8));
    let number = |base: Hsla| mix_lab(bg, mix(bg, base, pick(0.35, 0.6)), pick(0.91, 0.85));
    let mixer = if dark { gpui_kit::white() } else { gpui_kit::black() };
    let context = mix_lab(bg, mixer, pick(0.985, 0.925));
    DiffLook {
        gap: px(2.),
        added_line: line(p.success),
        added_number: number(p.success),
        deleted_line: line(p.destructive),
        deleted_number: number(p.destructive),
        selected_line: line(c.modified),
        selected_number: number(c.modified),
        selected_ink: mix_lab(c.modified, mixer, pick(0.65, 0.75)),
        deleted_stripe: mix_lab(bg, c.deleted, pick(0.88, 0.8)),
        buffer_number: mix_lab(context, bg, pick(0.9, 0.45)),
        buffer_stripe: mix_lab(bg, mixer, 0.92),
        note: context,
        note_number: mix_lab(context, bg, pick(0.9, 0.45)),
    }
}

/// Pierre's code row, `--diffs-line-height`.
pub(super) const ROW_HEIGHT: f32 = 20.;

/// The share of pierre's added or deleted color behind a changed line's changed words, over the row's tint:
/// `--diffs-bg-addition-emphasis` and its deletion twin, `light-dark(rgb(from base / .15), rgb(from base / .2))`.
pub(super) fn emphasis(cx: &App) -> f32 {
    if cx.theme().mode.is_dark() {
        0.2
    } else {
        0.15
    }
}

/// gpui-kit's theme as the code surface draws: its tokens set to [`DiffColors`].
pub(super) fn code_theme(cx: &App) -> Theme {
    let c = DiffColors::of(cx);
    let mut theme = cx.theme().clone();
    theme.background = c.background;
    theme.foreground = c.foreground;
    theme.muted = c.background;
    theme.muted_foreground = c.number;
    // The edge bar and a changed line's number in pierre's color.
    theme.success = c.added;
    theme.danger = c.deleted;
    theme.border = c.background.opacity(0.);
    theme.info = c.modified;
    theme.selection = c.selected;
    theme.list_hover = mix(c.background, cx.palette().foreground, 0.94);
    // Pierre's 13 px code; each view sets pierre's 20 px rows, ROW_HEIGHT.
    theme.mono_font_size = px(13.);
    theme
}

/// Lays out, prepaints and paints `child` with `theme` as gpui-kit's global theme, so the `Diff` inside, its rows
/// rendered lazily during prepaint among them, reads the code surface's tokens.
pub(super) fn with_theme(theme: Theme, child: impl IntoElement) -> Scoped {
    Scoped::new(None, Some(theme), child)
}
