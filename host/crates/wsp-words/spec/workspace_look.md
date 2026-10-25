# workspace_look
Source: the functions and tables of `packages/protocol/src/workspace-look.ts` (304 lines) at a3b870a6e; its zod shapes and flat constants (`WorkspaceTheme`, `ThemeDot`, `ThemeHarmony`, `WORD_FLOOR`, `INK_FLOOR`, ...) are `wsp_wire::types::workspace_look`. Tests: `packages/protocol/test/workspace-theme.test.ts` (11 cases ported; 3 `fmtThemeVars` cases live in `tests/places.rs`), plus `pinned` in `tests/workspace_look.rs` (every value printed by the TS under node).

## What it does
The maths that turns a workspace theme's dots into colours: the harmonies a picker offers and the dots each places, the grain's steps, HSL and contrast, the side a theme draws under, the opacity cap that keeps the side's words readable, the ink colour the chrome takes, and the presets.

## Behaviour
1. `harmony_size`: offsets + 1 (single 1; complementary and analogous 2; the other three 3). `harmonies_of(n)` in THEME_HARMONIES' order. `harmony_dots(first, h)`: first, then `(angle + offset) % 360` at the same radius per offset (complementary 180, analogous 310, splitComplementary 150 and 210, triadic 120 and 240, analogousThree 50 and 310) (workspace-look.ts:55-112).
2. `resize_dots(theme, grow)`: the first harmony of size dots ± 1 with its dots from the first dot; None where none (:119-123). `cycle_harmony`: the next harmony of the same size, wrapping (:125-130). `move_first_dot`: the harmony's dots from the new first dot (:113-117).
3. `snap_grain(g)`: `js_round(clamp(g, 0, 1) * 16) / 16` (:132).
4. `hsl_to_rgb(h, s, l)`: s 0 gives grey `js_round(l*255)`; else the standard two-point formula with `((t % 1) + 1) % 1` and each channel `js_round(c * 255)` (:142-157). `dot_colour(dot)`: `hsl_to_rgb(angle/360, 0.9 + 0.1*radius, radius)` (:138-140). `rgb_to_hsl`: hue in degrees, saturation and lightness 0 to 1 (:159-170).
5. `contrast_ratio(a, b)`: WCAG relative luminance (sRGB channel `u <= 0.04045 ? u/12.92 : ((u+0.055)/1.055)^2.4`), `(max + 0.05)/(min + 0.05)` (:183-201).
6. `theme_scheme(theme, app_dark)`: a pinned mode is itself; auto takes the app's side unless the dots at minimum opacity read under WORD_FLOOR there and better on the other side (:217-228). `opacity_cap`: from 1 down in steps of 0.05 (each step `js_round((a - 0.05) * 100) / 100`), the first alpha where every dot washed over the side's ground keeps the side's foreground at WORD_FLOOR; else THEME_MIN_OPACITY (:230-237). `effective_opacity`: min(theme opacity, cap) (:239-241).
7. `theme_ink(theme, app_dark)`: from the first dot's hue at saturation + 0.2, lightness starting at `l*0.2 + (light 0.38 | dark 0.66)*0.8` and stepping 0.03 toward the side's foreground for 24 tries: the first ink at INK_FLOOR on both the ground and the washed first dot; else the best on the washed dot among those at INK_FLOOR on the ground; else the last tried (:248-270).
8. `THEME_PRESETS` (8, copied by script), `apply_preset`, `is_preset` (dots and harmony equal) and `default_theme()` (the first preset, grain 0, opacity 0.5, mode auto) (:276-304). `side_ink`: light ground 250,250,250 and foreground 39,39,42; dark 17,17,17 and 241,243,247.

## Edge cases
- `harmonies_of(0)` and `harmonies_of(4)` are empty; `resize_dots` past three dots or below one is None.
- `snap_grain(-1)` is 0, `snap_grain(2)` is 1.
- Colours are whole channels: `hsl_to_rgb` and the wash round each with `js_round` (rule 8.12), so they compare exactly; ratios are compared at ten places.

## Behaviour nobody wrote down
- Rule 8.12: every rounding here is `Math.round`.
- `DEFAULT_THEME` carries the preset's `id` key at runtime in the TS; a `WorkspaceTheme` has no such field and drops it.

## Tapes
`workspaces.look`, and every client that paints a theme.

## Decisions
1. `resize_dots` takes `grow: bool` for the TS step ±1; `side_ink` and `Side` replace the TS record.
2. Presets are `(angle, radius)` pairs so the table is a constant.

## After the port
- none.

## Rules pinned
| Rule | Test |
|---|---|
| 1, 2 | `the_harmonies::*`, `harmonies_place_dots_at_fixed_angles_and_cycle_within_a_size` |
| 3, 4, 5 | `a_dots_colour::*`, `grain_snaps_and_colours_round_to_whole_channels` |
| 6, 7 | `grain_scheme_and_ink::*`, `the_side_the_cap_and_the_ink_read_off_the_wash` |
| 8 | `the_presets::*`, `presets_apply_and_are_recognised` |
