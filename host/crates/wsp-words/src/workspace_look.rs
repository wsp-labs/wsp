// SPDX-License-Identifier: AGPL-3.0-only
//! The maths that turns a workspace theme's dots into colours, the harmonies and presets a picker offers, and the
//! side and ink a theme draws under. Port of the functions and tables of packages/protocol/src/workspace-look.ts at
//! a3b870a6e; its zod shapes and flat constants are wsp_wire::types::workspace_look. See spec/workspace_look.md.

use crate::js::js_round;
use wsp_wire::types::js_number::JsNumber;
use wsp_wire::types::workspace_look::{
    ThemeDot, ThemeHarmony, ThemeMode, WorkspaceTheme, INK_FLOOR, THEME_GRAIN_STEPS, THEME_MIN_OPACITY, WORD_FLOOR,
};

// The glyphs offered, in the order the picker shows them; each id names one icon of the app's set.
// workspace-look.ts:15
// export const WORKSPACE_GLYPHS = [
//   "terminal",
//   "code",
//   "bug",
//   "wrench",
//   "hammer",
//   "rocket",
//   "flask",
//   "database",
//   "cloud",
//   "globe",
//   "compass",
//   "map",
//   "book",
//   "feather",
//   "palette",
//   "leaf",
//   "star",
//   "bolt",
//   "key",
//   "shield",
//   "box",
//   "folder",
//   "chip",
//   "lamp",
// ] as const;
// in wsp_wire::types::workspace_look: WORKSPACE_GLYPHS

// workspace-look.ts:41
// export const WorkspaceGlyph = z.enum(WORKSPACE_GLYPHS);
// in wsp_wire::types::workspace_look: WorkspaceGlyph

// workspace-look.ts:42
// export type WorkspaceGlyph = z.infer<typeof WorkspaceGlyph>;

// One colour dot on the picker's wheel: its hue is the angle in degrees, its lightness the distance from the
// centre, 0 at the centre and 1 at the rim.
// workspace-look.ts:46
// export const ThemeDot = z.object({ angle: z.number().min(0).max(360), radius: z.number().min(0).max(1) });
// in wsp_wire::types::workspace_look: ThemeDot

// workspace-look.ts:47
// export type ThemeDot = z.infer<typeof ThemeDot>;

// How the dots after the first sit against it, by the angles they keep from it around the wheel. The first dot is
// the one a person drags; the others follow at its radius. A harmony with no angles is one dot alone.
// workspace-look.ts:51
// export const THEME_HARMONIES = ["single", "complementary", "analogous", "splitComplementary", "triadic", "analogousThree"] as const;
// in wsp_wire::types::workspace_look: THEME_HARMONIES

// workspace-look.ts:52
// export const ThemeHarmony = z.enum(THEME_HARMONIES);
// in wsp_wire::types::workspace_look: ThemeHarmony

// workspace-look.ts:53
// export type ThemeHarmony = z.infer<typeof ThemeHarmony>;

// workspace-look.ts:55
// private in the TS: port it as a private helper
// const HARMONY_ANGLES: Record<ThemeHarmony, ReadonlyArray<number>> = {
//   single: [],
//   complementary: [180],
//   analogous: [310],
//   splitComplementary: [150, 210],
//   triadic: [120, 240],
//   analogousThree: [50, 310],
// };

/// The dots a harmony places, the first included.
// workspace-look.ts:65
// export const harmonySize = (harmony: ThemeHarmony): number => HARMONY_ANGLES[harmony].length + 1;
pub fn harmony_size(harmony: ThemeHarmony) -> usize {
    harmony_angles(harmony).len() + 1
}

/// The harmonies that place this many dots, in the order the picker cycles them.
// workspace-look.ts:68
// export function harmoniesOf(count: number): ThemeHarmony[] {
//   return THEME_HARMONIES.filter(harmony => harmonySize(harmony) === count);
// }
pub fn harmonies_of(count: usize) -> Vec<ThemeHarmony> {
    ALL_HARMONIES.iter().copied().filter(|harmony| harmony_size(*harmony) == count).collect()
}

// workspace-look.ts:72
// in wsp_wire::types::workspace_look: THEME_MAX_DOTS

// Which side the theme renders under: the app's own, or one side pinned for this workspace alone.
// workspace-look.ts:75
// export const ThemeMode = z.enum(["auto", "light", "dark"]);
// in wsp_wire::types::workspace_look: ThemeMode

// workspace-look.ts:76
// export type ThemeMode = z.infer<typeof ThemeMode>;

// workspace-look.ts:79
// in wsp_wire::types::workspace_look: THEME_MIN_OPACITY

// workspace-look.ts:81
// in wsp_wire::types::workspace_look: THEME_GRAIN_STEPS

// workspace-look.ts:83
// export const WorkspaceTheme = z.object({
//   dots: z.array(ThemeDot).min(1).max(THEME_MAX_DOTS),
//   harmony: ThemeHarmony,
//   /** How much grain lies over the gradient, 0 for none. */
//   grain: z.number().min(0).max(1),
//   /** How much of the colour shows over the sidebar's own surface. */
//   opacity: z.number().min(THEME_MIN_OPACITY).max(1),
//   mode: ThemeMode,
// });
// in wsp_wire::types::workspace_look: WorkspaceTheme

// workspace-look.ts:92
// export type WorkspaceTheme = z.infer<typeof WorkspaceTheme>;

// What one call changes: a key left out keeps that fact as it is, and null clears it back to none.
// workspace-look.ts:95
// export const WorkspaceLook = z.object({
//   theme: WorkspaceTheme.nullable().optional(),
//   glyph: WorkspaceGlyph.nullable().optional(),
// });
// in wsp_wire::types::workspace_look: WorkspaceLook

// workspace-look.ts:99
// export type WorkspaceLook = z.infer<typeof WorkspaceLook>;

// Which of the two facts a picker or a menu entry is for.
// workspace-look.ts:102
// export type LookPart = keyof WorkspaceLook;
// in wsp_wire::types::workspace_look: LookPart

// The two in the order every menu shows them.
// workspace-look.ts:105
// export const LOOK_PARTS = ["glyph", "theme"] as const satisfies ReadonlyArray<LookPart>;
// in wsp_wire::types::workspace_look: LOOK_PARTS

/// The dots a harmony places around the first, at its radius; the first stays where it is.
// workspace-look.ts:108
// export function harmonyDots(first: ThemeDot, harmony: ThemeHarmony): ThemeDot[] {
//   return [first, ...HARMONY_ANGLES[harmony].map(offset => ({ angle: (first.angle + offset) % 360, radius: first.radius }))];
// }
pub fn harmony_dots(first: &ThemeDot, harmony: ThemeHarmony) -> Vec<ThemeDot> {
    let mut dots = vec![first.clone()];
    dots.extend(
        harmony_angles(harmony).iter().map(|offset| ThemeDot { angle: JsNumber((first.angle.0 + offset) % 360.0), radius: first.radius }),
    );
    dots
}

/// The theme with its first dot moved: the others follow the harmony.
// workspace-look.ts:113
// export function moveFirstDot(theme: WorkspaceTheme, first: ThemeDot): WorkspaceTheme {
//   return { ...theme, dots: harmonyDots(first, theme.harmony) };
// }
pub fn move_first_dot(theme: &WorkspaceTheme, first: &ThemeDot) -> WorkspaceTheme {
    WorkspaceTheme { dots: harmony_dots(first, theme.harmony), ..theme.clone() }
}

/// The theme with one dot more or fewer: the next harmony of that size, with its dots placed. Null where the count
/// cannot move that way.
// workspace-look.ts:119
// export function resizeDots(theme: WorkspaceTheme, step: 1 | -1): WorkspaceTheme | null {
//   const harmony = harmoniesOf(theme.dots.length + step)[0];
//   return harmony === undefined ? null : { ...theme, harmony, dots: harmonyDots(theme.dots[0]!, harmony) };
// }
/// `grow` true is the TS step 1, false is -1; None is the TS null.
pub fn resize_dots(theme: &WorkspaceTheme, grow: bool) -> Option<WorkspaceTheme> {
    let next = if grow { theme.dots.len() + 1 } else { theme.dots.len().checked_sub(1)? };
    let harmony = *harmonies_of(next).first()?;
    Some(WorkspaceTheme { harmony, dots: harmony_dots(&theme.dots[0], harmony), ..theme.clone() })
}

/// The theme under the next harmony that places as many dots, wrapping.
// workspace-look.ts:125
// export function cycleHarmony(theme: WorkspaceTheme): WorkspaceTheme {
//   const choices = harmoniesOf(theme.dots.length);
//   const harmony = choices[(choices.indexOf(theme.harmony) + 1) % choices.length] ?? theme.harmony;
//   return { ...theme, harmony, dots: harmonyDots(theme.dots[0]!, harmony) };
// }
pub fn cycle_harmony(theme: &WorkspaceTheme) -> WorkspaceTheme {
    let choices = harmonies_of(theme.dots.len());
    let harmony = if choices.is_empty() {
        theme.harmony
    } else {
        let position = choices.iter().position(|harmony| *harmony == theme.harmony);
        choices[position.map_or(0, |found| found + 1) % choices.len()]
    };
    WorkspaceTheme { harmony, dots: harmony_dots(&theme.dots[0], harmony), ..theme.clone() }
}

/// A grain value on the slider's steps.
// workspace-look.ts:132
// export const snapGrain = (grain: number): number => Math.round(Math.min(1, Math.max(0, grain)) * THEME_GRAIN_STEPS) / THEME_GRAIN_STEPS;
pub fn snap_grain(grain: f64) -> f64 {
    let steps = THEME_GRAIN_STEPS as f64;
    js_round(grain.clamp(0.0, 1.0) * steps) / steps
}

// workspace-look.ts:134
// export type Rgb = readonly [number, number, number];
/// A colour's three channels, 0 to 255; what hsl_to_rgb and the wash answer is whole.
pub type Rgb = [f64; 3];

/// The colour a dot stands for: hue by angle, lightness by distance from the centre (black at the centre, white at
/// the rim), saturation near full and a little fuller toward the rim.
// workspace-look.ts:138
// export function dotColour(dot: ThemeDot): Rgb {
//   return hslToRgb(dot.angle / 360, 0.9 + 0.1 * dot.radius, dot.radius);
// }
pub fn dot_colour(dot: &ThemeDot) -> Rgb {
    hsl_to_rgb(dot.angle.0 / 360.0, 0.9 + 0.1 * dot.radius.0, dot.radius.0)
}

// workspace-look.ts:142
// export function hslToRgb(h: number, s: number, l: number): Rgb {
//   if (s === 0) {
//     const grey = Math.round(l * 255);
//     return [grey, grey, grey];
//   }
//   const q = l < 0.5 ? l * (1 + s) : l + s - l * s;
//   const p = 2 * l - q;
//   const channel = (t: number): number => {
//     const u = ((t % 1) + 1) % 1;
//     if (u < 1 / 6) return p + (q - p) * 6 * u;
//     if (u < 1 / 2) return q;
//     if (u < 2 / 3) return p + (q - p) * (2 / 3 - u) * 6;
//     return p;
//   };
//   return [Math.round(channel(h + 1 / 3) * 255), Math.round(channel(h) * 255), Math.round(channel(h - 1 / 3) * 255)];
// }
pub fn hsl_to_rgb(h: f64, s: f64, l: f64) -> Rgb {
    if s == 0.0 {
        let grey = js_round(l * 255.0);
        return [grey, grey, grey];
    }
    let q = if l < 0.5 { l * (1.0 + s) } else { l + s - l * s };
    let p = 2.0 * l - q;
    let channel = |t: f64| -> f64 {
        let u = ((t % 1.0) + 1.0) % 1.0;
        if u < 1.0 / 6.0 {
            p + (q - p) * 6.0 * u
        } else if u < 1.0 / 2.0 {
            q
        } else if u < 2.0 / 3.0 {
            p + (q - p) * (2.0 / 3.0 - u) * 6.0
        } else {
            p
        }
    };
    [js_round(channel(h + 1.0 / 3.0) * 255.0), js_round(channel(h) * 255.0), js_round(channel(h - 1.0 / 3.0) * 255.0)]
}

// workspace-look.ts:159
// export function rgbToHsl([r, g, b]: Rgb): readonly [number, number, number] {
//   const [red, green, blue] = [r / 255, g / 255, b / 255];
//   const max = Math.max(red, green, blue);
//   const min = Math.min(red, green, blue);
//   const l = (max + min) / 2;
//   if (max === min) return [0, 0, l];
//   const d = max - min;
//   const s = l > 0.5 ? d / (2 - max - min) : d / (max + min);
//   const h = max === red ? (green - blue) / d + (green < blue ? 6 : 0) : max === green ? (blue - red) / d + 2 : (red - green) / d + 4;
//   return [(h / 6) * 360, s, l];
// }
/// Hue 0 to 1, saturation and lightness 0 to 1.
pub fn rgb_to_hsl(rgb: &Rgb) -> [f64; 3] {
    let red = rgb[0] / 255.0;
    let green = rgb[1] / 255.0;
    let blue = rgb[2] / 255.0;
    let max = red.max(green).max(blue);
    let min = red.min(green).min(blue);
    let l = (max + min) / 2.0;
    if max == min {
        return [0.0, 0.0, l];
    }
    let d = max - min;
    let s = if l > 0.5 { d / (2.0 - max - min) } else { d / (max + min) };
    let h = if max == red {
        (green - blue) / d + if green < blue { 6.0 } else { 0.0 }
    } else if max == green {
        (blue - red) / d + 2.0
    } else {
        (red - green) / d + 4.0
    };
    [(h / 6.0) * 360.0, s, l]
}

// workspace-look.ts:172
// in wsp_wire::types::workspace_look: WORD_FLOOR

// workspace-look.ts:173
// in wsp_wire::types::workspace_look: INK_FLOOR

/// What each side's sidebar is made of, as the theme is read against it: the ground its surface token paints
/// (zinc-50, and the near-black card) and the foreground its words take (zinc-800, and the near-white). The theme
/// lays its colours over the ground at their opacity, and the words have to keep reading on the result. Mirrors of
/// the app's tokens rather than reads of them, because a theme pinned to the side the app is not drawing has no
/// element to read that side's tokens from.
// workspace-look.ts:180
// export const SIDE_INK: Record<"light" | "dark", { readonly ground: Rgb; readonly foreground: Rgb }> = {
//   light: { ground: [250, 250, 250], foreground: [39, 39, 42] },
//   dark: { ground: [17, 17, 17], foreground: [241, 243, 247] },
// };
/// `SIDE_INK[side]`: light ground [250,250,250] foreground [39,39,42]; dark ground [17,17,17] foreground [241,243,247].
pub fn side_ink(side: Side) -> SideInk {
    match side {
        Side::Light => SideInk { ground: [250.0, 250.0, 250.0], foreground: [39.0, 39.0, 42.0] },
        Side::Dark => SideInk { ground: [17.0, 17.0, 17.0], foreground: [241.0, 243.0, 247.0] },
    }
}

/// The ground and the foreground a side draws.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SideInk {
    pub ground: Rgb,
    pub foreground: Rgb,
}

// The lightness the ink starts from on each side: dark enough to read on a light surface, light enough on a dark
// one. The ink keeps some of the dot's own lightness so a pale pick still reads paler than a deep one.
// workspace-look.ts:187
// private in the TS: port it as a private helper
// const INK_LIGHTNESS = { light: 0.38, dark: 0.66 } as const;

// workspace-look.ts:189
// private in the TS: port it as a private helper
// const luminance = ([r, g, b]: Rgb): number => {
//   const channel = (v: number): number => {
//     const u = v / 255;
//     return u <= 0.04045 ? u / 12.92 : ((u + 0.055) / 1.055) ** 2.4;
//   };
//   return 0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b);
// };

// workspace-look.ts:197
// export function contrastRatio(a: Rgb, b: Rgb): number {
//   const [la, lb] = [luminance(a), luminance(b)];
//   return (Math.max(la, lb) + 0.05) / (Math.min(la, lb) + 0.05);
// }
pub fn contrast_ratio(a: &Rgb, b: &Rgb) -> f64 {
    let la = luminance(a);
    let lb = luminance(b);
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

// workspace-look.ts:202
// private in the TS: port it as a private helper
// const over = (top: Rgb, alpha: number, under: Rgb): Rgb => [0, 1, 2].map(i => Math.round(top[i]! * alpha + under[i]! * (1 - alpha))) as unknown as Rgb;

// workspace-look.ts:204
// type Side = "light" | "dark";
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    Light,
    Dark,
}

// workspace-look.ts:205
// type Wash = Pick<WorkspaceTheme, "dots" | "opacity">;

// The least the side's words read at over any of the theme's colours laid on its ground at this alpha.
// workspace-look.ts:208
// private in the TS: port it as a private helper
// function wordsRead(theme: Wash, side: Side, alpha: number): number {
//   const { ground, foreground } = SIDE_INK[side];
//   return Math.min(...theme.dots.map(dot => contrastRatio(foreground, over(dotColour(dot), alpha, ground))));
// }

/// The side a theme draws under, given the app's: its own pin, else the app's side. The opacity cap is what gives
/// when the words would drown, so a dark app stays dark under a pale wash and a light one light under a deep one;
/// only where even the slider's least opacity leaves the app's side unreadable does Auto take the other side, and
/// only while that side reads better there.
// workspace-look.ts:217
// export function themeScheme(theme: Pick<WorkspaceTheme, "mode"> & Partial<Wash>, appDark: boolean): Side {
//   if (theme.mode !== "auto") return theme.mode;
//   const own: Side = appDark ? "dark" : "light";
//   if (theme.dots === undefined || theme.opacity === undefined) return own;
//   const other: Side = appDark ? "light" : "dark";
//   const wash: Wash = { dots: theme.dots, opacity: theme.opacity };
//   const ownReads = wordsRead(wash, own, THEME_MIN_OPACITY);
//   return ownReads < WORD_FLOOR && wordsRead(wash, other, THEME_MIN_OPACITY) > ownReads ? other : own;
// }
pub fn theme_scheme(theme: &WorkspaceTheme, app_dark: bool) -> Side {
    match theme.mode {
        ThemeMode::Light => return Side::Light,
        ThemeMode::Dark => return Side::Dark,
        ThemeMode::Auto => {}
    }
    let own = if app_dark { Side::Dark } else { Side::Light };
    let other = if app_dark { Side::Light } else { Side::Dark };
    let own_reads = words_read(theme, own, THEME_MIN_OPACITY);
    if own_reads < WORD_FLOOR && words_read(theme, other, THEME_MIN_OPACITY) > own_reads {
        other
    } else {
        own
    }
}

/// How far the opacity may go before the side's words drop under the floor on the colour laid over the ground: the
/// largest step of the slider's grid that keeps them reading, never under the slider's least. The picker's slider
/// ends here, and the paint stops here whatever the record holds.
// workspace-look.ts:230
// export function opacityCap(theme: Pick<WorkspaceTheme, "dots" | "opacity" | "mode">, appDark: boolean): number {
//   const side = themeScheme(theme, appDark);
//   for (let alpha = 1; alpha > THEME_MIN_OPACITY; alpha = Math.round((alpha - 0.05) * 100) / 100) {
//     if (wordsRead(theme, side, alpha) >= WORD_FLOOR) return alpha;
//   }
//   return THEME_MIN_OPACITY;
// }
pub fn opacity_cap(theme: &WorkspaceTheme, app_dark: bool) -> f64 {
    let side = theme_scheme(theme, app_dark);
    let mut alpha = 1.0_f64;
    while alpha > THEME_MIN_OPACITY {
        if words_read(theme, side, alpha) >= WORD_FLOOR {
            return alpha;
        }
        alpha = js_round((alpha - 0.05) * 100.0) / 100.0;
    }
    THEME_MIN_OPACITY
}

/// The opacity the theme paints at: the record's, held under the cap.
// workspace-look.ts:239
// export function effectiveOpacity(theme: Pick<WorkspaceTheme, "dots" | "opacity" | "mode">, appDark: boolean): number {
//   return Math.min(theme.opacity, opacityCap(theme, appDark));
// }
pub fn effective_opacity(theme: &WorkspaceTheme, app_dark: bool) -> f64 {
    theme.opacity.0.min(opacity_cap(theme, app_dark))
}

/// The one colour a theme lends the chrome: the first dot, saturated a step and pulled toward the lightness that
/// reads on the side it draws under, then stepped further from the surface until it clears the floor both on the
/// ground and on the first colour laid over it. A pick so deep at full opacity that no lightness of its hue clears
/// both takes the step that clears the ground and comes nearest on the wash. The active space icon and the header's
/// glyph take it; nothing else does.
// workspace-look.ts:248
// export function themeInk(theme: Pick<WorkspaceTheme, "dots" | "mode" | "opacity">, appDark: boolean): Rgb {
//   const scheme = themeScheme(theme, appDark);
//   const first = dotColour(theme.dots[0]!);
//   const [h, s, l] = rgbToHsl(first);
//   const { ground } = SIDE_INK[scheme];
//   const washed = over(first, effectiveOpacity(theme, appDark), ground);
//   const step = scheme === "dark" ? 0.03 : -0.03;
//   const at = (lightness: number): Rgb => hslToRgb(h / 360, Math.min(1, s + 0.2), lightness);
//   let lightness = l * 0.2 + INK_LIGHTNESS[scheme] * 0.8;
//   let best: { ink: Rgb; onWashed: number } | null = null;
//   for (let i = 0; i < 24; i++) {
//     const ink = at(lightness);
//     const onWashed = contrastRatio(ink, washed);
//     if (contrastRatio(ink, ground) >= INK_FLOOR) {
//       if (onWashed >= INK_FLOOR) return ink;
//       if (best === null || onWashed > best.onWashed) best = { ink, onWashed };
//     }
//     lightness = Math.min(1, Math.max(0, lightness + step));
//   }
//   return best?.ink ?? at(lightness);
// }
pub fn theme_ink(theme: &WorkspaceTheme, app_dark: bool) -> Rgb {
    let scheme = theme_scheme(theme, app_dark);
    let first = dot_colour(&theme.dots[0]);
    let [h, s, l] = rgb_to_hsl(&first);
    let ground = side_ink(scheme).ground;
    let washed = over(&first, effective_opacity(theme, app_dark), &ground);
    let step = if scheme == Side::Dark { 0.03 } else { -0.03 };
    let ink_lightness = if scheme == Side::Dark { 0.66 } else { 0.38 };
    let at = |lightness: f64| hsl_to_rgb(h / 360.0, (s + 0.2).min(1.0), lightness);
    let mut lightness = l * 0.2 + ink_lightness * 0.8;
    let mut best: Option<(Rgb, f64)> = None;
    for _ in 0..24 {
        let ink = at(lightness);
        let on_washed = contrast_ratio(&ink, &washed);
        if contrast_ratio(&ink, &ground) >= INK_FLOOR as f64 {
            if on_washed >= INK_FLOOR as f64 {
                return ink;
            }
            if best.is_none_or(|(_, found)| on_washed > found) {
                best = Some((ink, on_washed));
            }
        }
        lightness = (lightness + step).clamp(0.0, 1.0);
    }
    best.map_or_else(|| at(lightness), |(ink, _)| ink)
}

/// Preset themes, in the row's order: each is a set of dots with its harmony, and takes the person's grain, opacity
/// and mode as they are.
// workspace-look.ts:272
// export interface ThemePreset {
//   readonly id: string;
//   readonly dots: ReadonlyArray<ThemeDot>;
//   readonly harmony: ThemeHarmony;
// }
/// A preset: its id, its harmony and its dots as (angle, radius).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ThemePreset {
    pub id: &'static str,
    pub harmony: ThemeHarmony,
    pub dots: &'static [(f64, f64)],
}

// workspace-look.ts:278
// private in the TS: port it as a private helper
// const preset = (id: string, angle: number, radius: number, harmony: ThemeHarmony): ThemePreset => ({ id, harmony, dots: harmonyDots({ angle, radius }, harmony) });

/// Every dot of every preset, the harmony's included, keeps thirty degrees off the green that means running and the
/// red that means danger: the wash is the owner's exemption for a free pick, the presets are ours.
// workspace-look.ts:282
// export const THEME_PRESETS: ReadonlyArray<ThemePreset> = [
//   preset("dusk", 262, 0.62, "analogous"),
//   preset("sea", 205, 0.56, "triadic"),
//   preset("fern", 95, 0.5, "analogous"),
//   preset("clay", 35, 0.6, "complementary"),
//   preset("orchid", 320, 0.62, "analogous"),
//   preset("sand", 55, 0.72, "single"),
//   preset("slate", 225, 0.42, "complementary"),
//   preset("plum", 290, 0.46, "complementary"),
// ];
/// Copied by script from the TS table, in its order.
pub const THEME_PRESETS: [ThemePreset; 8] = [
    ThemePreset { id: "dusk", harmony: ThemeHarmony::Analogous, dots: &[(262.0, 0.62), (212.0, 0.62)] },
    ThemePreset { id: "sea", harmony: ThemeHarmony::Triadic, dots: &[(205.0, 0.56), (325.0, 0.56), (85.0, 0.56)] },
    ThemePreset { id: "fern", harmony: ThemeHarmony::Analogous, dots: &[(95.0, 0.5), (45.0, 0.5)] },
    ThemePreset { id: "clay", harmony: ThemeHarmony::Complementary, dots: &[(35.0, 0.6), (215.0, 0.6)] },
    ThemePreset { id: "orchid", harmony: ThemeHarmony::Analogous, dots: &[(320.0, 0.62), (270.0, 0.62)] },
    ThemePreset { id: "sand", harmony: ThemeHarmony::Single, dots: &[(55.0, 0.72)] },
    ThemePreset { id: "slate", harmony: ThemeHarmony::Complementary, dots: &[(225.0, 0.42), (45.0, 0.42)] },
    ThemePreset { id: "plum", harmony: ThemeHarmony::Complementary, dots: &[(290.0, 0.46), (110.0, 0.46)] },
];

/// The theme a workspace starts with when a person opens the picker on one that has none.
// workspace-look.ts:294
// export const DEFAULT_THEME: WorkspaceTheme = { ...THEME_PRESETS[0]!, dots: [...THEME_PRESETS[0]!.dots], grain: 0, opacity: 0.5, mode: "auto" };
/// The first preset's harmony and dots, grain 0, opacity 0.5, mode auto. (The TS object also carries the preset's
/// `id` key at runtime, which WorkspaceTheme has no field for.)
pub fn default_theme() -> WorkspaceTheme {
    let first = &THEME_PRESETS[0];
    WorkspaceTheme { dots: preset_dots(first), harmony: first.harmony, grain: JsNumber(0.0), opacity: JsNumber(0.5), mode: ThemeMode::Auto }
}

/// A preset applied over a theme: the dots and harmony move, the rest stays.
// workspace-look.ts:297
// export function applyPreset(theme: WorkspaceTheme, chosen: ThemePreset): WorkspaceTheme {
//   return { ...theme, harmony: chosen.harmony, dots: [...chosen.dots] };
// }
pub fn apply_preset(theme: &WorkspaceTheme, chosen: &ThemePreset) -> WorkspaceTheme {
    WorkspaceTheme { harmony: chosen.harmony, dots: preset_dots(chosen), ..theme.clone() }
}

/// Whether a theme's dots are the preset's, which is what marks the preset as the current one.
// workspace-look.ts:302
// export function isPreset(theme: Pick<WorkspaceTheme, "dots" | "harmony">, chosen: ThemePreset): boolean {
//   return theme.harmony === chosen.harmony && theme.dots.length === chosen.dots.length && theme.dots.every((dot, i) => Math.abs(dot.angle - chosen.dots[i]!.angle) < 0.5 && Math.abs(dot.radius - chosen.dots[i]!.radius) < 0.005);
// }
pub fn is_preset(theme: &WorkspaceTheme, chosen: &ThemePreset) -> bool {
    theme.harmony == chosen.harmony
        && theme.dots.len() == chosen.dots.len()
        && theme
            .dots
            .iter()
            .zip(chosen.dots.iter())
            .all(|(dot, &(angle, radius))| (dot.angle.0 - angle).abs() < 0.5 && (dot.radius.0 - radius).abs() < 0.005)
}
const ALL_HARMONIES: [ThemeHarmony; 6] = [
    ThemeHarmony::Single,
    ThemeHarmony::Complementary,
    ThemeHarmony::Analogous,
    ThemeHarmony::SplitComplementary,
    ThemeHarmony::Triadic,
    ThemeHarmony::AnalogousThree,
];

fn harmony_angles(harmony: ThemeHarmony) -> &'static [f64] {
    match harmony {
        ThemeHarmony::Single => &[],
        ThemeHarmony::Complementary => &[180.0],
        ThemeHarmony::Analogous => &[310.0],
        ThemeHarmony::SplitComplementary => &[150.0, 210.0],
        ThemeHarmony::Triadic => &[120.0, 240.0],
        ThemeHarmony::AnalogousThree => &[50.0, 310.0],
    }
}

fn preset_dots(preset: &ThemePreset) -> Vec<ThemeDot> {
    preset.dots.iter().map(|&(angle, radius)| ThemeDot { angle: JsNumber(angle), radius: JsNumber(radius) }).collect()
}

fn luminance(rgb: &Rgb) -> f64 {
    let channel = |v: f64| {
        let u = v / 255.0;
        if u <= 0.04045 {
            u / 12.92
        } else {
            ((u + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(rgb[0]) + 0.7152 * channel(rgb[1]) + 0.0722 * channel(rgb[2])
}

fn over(top: &Rgb, alpha: f64, under: &Rgb) -> Rgb {
    [0, 1, 2].map(|i| js_round(top[i] * alpha + under[i] * (1.0 - alpha)))
}

fn words_read(theme: &WorkspaceTheme, side: Side, alpha: f64) -> f64 {
    let ink = side_ink(side);
    theme.dots.iter().map(|dot| contrast_ratio(&ink.foreground, &over(&dot_colour(dot), alpha, &ink.ground))).fold(f64::INFINITY, f64::min)
}
