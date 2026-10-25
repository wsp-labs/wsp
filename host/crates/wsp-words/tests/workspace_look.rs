// SPDX-License-Identifier: AGPL-3.0-only
//! Ported from packages/protocol/test/workspace-theme.test.ts:40-207 (a dot's colour, the harmonies, grain, scheme
//! and ink, the presets), including :114 (the opacity cap), whose one fmtThemeVars line calls
//! wsp_words::words::places::fmt_theme_vars. Skipped: :210, :222 and :231 (fmtThemeVars, fmtPercent and fmtRgb; they
//! are in tests/places.rs).

use serde_json::json;
use std::collections::HashSet;
use wsp_wire::types::js_number::JsNumber;
use wsp_wire::types::workspace_look::{
    ThemeDot, ThemeHarmony, ThemeMode, WorkspaceTheme, INK_FLOOR, THEME_GRAIN_STEPS, THEME_HARMONIES, THEME_MAX_DOTS,
    WORD_FLOOR,
};
use wsp_words::words::places::fmt_theme_vars;
use wsp_words::workspace_look::*;

fn dot(angle: f64, radius: f64) -> ThemeDot {
    serde_json::from_value(json!({ "angle": angle, "radius": radius })).unwrap()
}

fn harmony(name: &str) -> ThemeHarmony {
    serde_json::from_value(json!(name)).unwrap()
}

fn preset_dots(preset: &ThemePreset) -> Vec<ThemeDot> {
    preset.dots.iter().map(|&(angle, radius)| dot(angle, radius)).collect()
}

fn js_round(x: f64) -> f64 {
    (x + 0.5).floor()
}

fn close(a: f64, b: f64, digits: i32) -> bool {
    (a - b).abs() < 10f64.powi(-digits) / 2.0
}

// The TS test's `theme`: the default theme with a complementary pair on (200, 0.5).
fn theme() -> WorkspaceTheme {
    WorkspaceTheme { dots: harmony_dots(&dot(200.0, 0.5), ThemeHarmony::Complementary), harmony: ThemeHarmony::Complementary, ..default_theme() }
}

fn over(top: &Rgb, alpha: f64, under: &Rgb) -> Rgb {
    [0, 1, 2].map(|i| js_round(top[i] * alpha + under[i] * (1.0 - alpha)))
}

mod a_dots_colour {
    use super::*;

    #[test]
    fn takes_its_hue_from_the_angle_and_its_lightness_from_the_distance_black_at_the_ce() {
        assert_eq!(dot_colour(&dot(0.0, 0.0)), [0.0, 0.0, 0.0]);
        assert_eq!(dot_colour(&dot(120.0, 1.0)), [255.0, 255.0, 255.0]);
        let [h, s, l] = rgb_to_hsl(&dot_colour(&dot(200.0, 0.5)));
        assert_eq!(js_round(h), 200.0);
        assert!(close(s, 0.95, 1), "{s}");
        assert!(close(l, 0.5, 1), "{l}");
        assert_eq!(dot_colour(&dot(0.0, 0.5)), [249.0, 6.0, 6.0]);
    }
}

mod the_harmonies {
    use super::*;

    #[test]
    fn place_one_two_or_three_dots_at_the_first_one_s_radius_at_fixed_angles_from_it_wr() {
        assert_eq!(THEME_HARMONIES.iter().map(|h| harmony_size(harmony(h))).collect::<Vec<_>>(), vec![1, 2, 2, 3, 3, 3]);
        assert_eq!(harmonies_of(2), vec![ThemeHarmony::Complementary, ThemeHarmony::Analogous]);
        assert_eq!(harmonies_of(4), Vec::<ThemeHarmony>::new());
        assert_eq!(
            harmony_dots(&dot(350.0, 0.4), ThemeHarmony::Complementary),
            vec![dot(350.0, 0.4), dot(170.0, 0.4)]
        );
        assert_eq!(
            harmony_dots(&dot(300.0, 0.4), ThemeHarmony::Triadic),
            vec![dot(300.0, 0.4), dot(60.0, 0.4), dot(180.0, 0.4)]
        );
    }

    #[test]
    fn moving_the_first_dot_carries_the_others_with_it_adding_and_removing_a_dot_picks_() {
        let theme = theme();
        let moved = move_first_dot(&theme, &dot(10.0, 0.8));
        assert_eq!(moved.dots, vec![dot(10.0, 0.8), dot(190.0, 0.8)]);
        assert_eq!(moved.harmony, ThemeHarmony::Complementary);
        let three = resize_dots(&theme, true).unwrap();
        assert_eq!(three.harmony, ThemeHarmony::SplitComplementary);
        assert_eq!(three.dots.len(), 3);
        assert_eq!(resize_dots(&three, true), None);
        assert_eq!(three.dots.len(), THEME_MAX_DOTS as usize);
        let one = resize_dots(&theme, false).unwrap();
        assert_eq!(one.harmony, ThemeHarmony::Single);
        assert_eq!(one.dots, vec![dot(200.0, 0.5)]);
        assert_eq!(resize_dots(&one, false), None);
    }

    #[test]
    fn cycling_walks_the_harmonies_of_the_same_size_and_wraps_re_placing_the_dots_each_() {
        let theme = theme();
        let next = cycle_harmony(&theme);
        assert_eq!(next.harmony, ThemeHarmony::Analogous);
        assert_eq!(next.dots[1], dot(150.0, 0.5));
        assert_eq!(cycle_harmony(&next).harmony, ThemeHarmony::Complementary);
        let one = resize_dots(&theme, false).unwrap();
        assert_eq!(cycle_harmony(&one), one);
    }
}

mod grain_scheme_and_ink {
    use super::*;

    #[test]
    fn the_grain_snaps_to_the_slider_s_steps_inside_0_and_1() {
        let steps = THEME_GRAIN_STEPS as f64;
        assert_eq!(snap_grain(0.49), js_round(0.49 * steps) / steps);
        assert_eq!(snap_grain(-1.0), 0.0);
        assert_eq!(snap_grain(7.0), 1.0);
    }

    #[test]
    fn auto_keeps_the_app_s_side_and_lets_the_opacity_cap_give_so_a_dark_app_stays_dark() {
        let theme = theme();
        assert_eq!(theme_scheme(&theme, true), Side::Dark);
        assert_eq!(theme_scheme(&theme, false), Side::Light);
        // A pale colour laid whole on a dark app: the side stays dark and the cap drops instead of the words.
        let pale = WorkspaceTheme { dots: vec![dot(55.0, 0.72)], harmony: ThemeHarmony::Single, opacity: JsNumber(1.0), ..theme.clone() };
        assert_eq!(theme_scheme(&pale, true), Side::Dark);
        assert!(opacity_cap(&pale, true) < 0.5);
        assert_eq!(theme_scheme(&pale, false), Side::Light);
        // A deep colour laid whole on a light app: the same the other way round.
        let deep = WorkspaceTheme { dots: vec![dot(262.0, 0.22)], harmony: ThemeHarmony::Single, opacity: JsNumber(1.0), ..theme.clone() };
        assert_eq!(theme_scheme(&deep, false), Side::Light);
        assert!(opacity_cap(&deep, false) < 0.5);
        assert_eq!(theme_scheme(&deep, true), Side::Dark);
        // Every preset at the default opacity draws under the app's own side on both sides.
        for preset in THEME_PRESETS.iter() {
            let picked = WorkspaceTheme { dots: preset_dots(preset), ..theme.clone() };
            assert_eq!(
                (preset.id, theme_scheme(&picked, true), theme_scheme(&picked, false)),
                (preset.id, Side::Dark, Side::Light)
            );
        }
        assert_eq!(theme_scheme(&WorkspaceTheme { mode: ThemeMode::Dark, ..pale.clone() }, false), Side::Dark);
        assert_eq!(theme_scheme(&WorkspaceTheme { mode: ThemeMode::Light, ..deep.clone() }, true), Side::Light);
    }

    #[test]
    fn the_opacity_stops_where_the_side_s_own_words_would_drop_under_aa_on_the_colour_l() {
        let theme = theme();
        let side = |dark: bool| side_ink(if dark { Side::Dark } else { Side::Light });
        // Moss green pinned dark on a light app, the state that read as light ink on saturated green: the dark side's
        // foreground would fall to 2.4 to 1 at 100%, so the paint stops short of it.
        let pinned_dark = WorkspaceTheme {
            dots: vec![dot(150.0, 0.52)],
            harmony: ThemeHarmony::Single,
            opacity: JsNumber(1.0),
            mode: ThemeMode::Dark,
            ..theme.clone()
        };
        let cap = opacity_cap(&pinned_dark, false);
        assert!(cap < 1.0, "{cap}");
        assert!(cap >= 0.1, "{cap}");
        assert_eq!(effective_opacity(&pinned_dark, false), cap);
        let dark = side(true);
        let first = dot_colour(&pinned_dark.dots[0]);
        assert!(contrast_ratio(&dark.foreground, &over(&first, cap, &dark.ground)) >= WORD_FLOOR);
        assert!(contrast_ratio(&dark.foreground, &over(&first, cap + 0.05, &dark.ground)) < WORD_FLOOR);
        let gradient = fmt_theme_vars(&pinned_dark, false).space_gradient;
        let stop = format!("/ {}%)", js_round(cap * 100.0));
        assert!(gradient.contains(&stop), "{gradient}");
        // A quiet wash at the default opacity is never touched.
        assert_eq!(effective_opacity(&theme, false), theme.opacity.0);
        assert_eq!(effective_opacity(&theme, true), theme.opacity.0);
        // Every preset keeps most of its colour on both sides; the ones whose wash crowds the dark words at the default
        // 50 percent (sea, fern, sand on a dark app) paint a step under it rather than moving the side.
        for preset in THEME_PRESETS.iter() {
            for app_dark in [false, true] {
                let picked = WorkspaceTheme { dots: preset_dots(preset), ..theme.clone() };
                assert!(opacity_cap(&picked, app_dark) >= 0.4, "{} appDark {app_dark}", preset.id);
            }
        }
        // Every dot counts, not the first alone: a pale second colour pulls the cap under what the first alone allows.
        let two_tone = WorkspaceTheme {
            dots: vec![dot(225.0, 0.42), dot(55.0, 0.72)],
            harmony: ThemeHarmony::Complementary,
            opacity: JsNumber(1.0),
            ..theme.clone()
        };
        let first_alone = WorkspaceTheme { dots: vec![two_tone.dots[0].clone()], ..two_tone.clone() };
        assert!(opacity_cap(&two_tone, true) < opacity_cap(&first_alone, true));
    }

    #[test]
    fn the_ink_keeps_the_first_dot_s_hue_and_lands_darker_on_the_light_side_than_on_the() {
        let theme = theme();
        for radius in [0.15, 0.5, 0.9] {
            // The sides are pinned here: Auto would move a pale or deep pick to the side whose words read on it.
            let pick = WorkspaceTheme { dots: harmony_dots(&dot(200.0, radius), ThemeHarmony::Complementary), ..theme.clone() };
            let [light_h, _, light_l] = rgb_to_hsl(&theme_ink(&WorkspaceTheme { mode: ThemeMode::Light, ..pick.clone() }, false));
            let [dark_h, _, dark_l] = rgb_to_hsl(&theme_ink(&WorkspaceTheme { mode: ThemeMode::Dark, ..pick.clone() }, true));
            assert_eq!(js_round(light_h), 200.0);
            assert_eq!(js_round(dark_h), 200.0);
            assert!(light_l < 0.5, "{light_l}");
            assert!(dark_l > 0.55, "{dark_l}");
            assert!(dark_l > light_l, "{dark_l} {light_l}");
        }
        // A pinned side takes that side's ink even when the app draws the other.
        assert_eq!(theme_ink(&WorkspaceTheme { mode: ThemeMode::Dark, ..theme.clone() }, false), theme_ink(&theme, true));
    }

    #[test]
    fn the_ink_clears_the_graphics_floor_on_the_ground_always_and_on_the_first_colour_l() {
        let theme = theme();
        let light: Rgb = [250.0, 250.0, 250.0];
        let dark: Rgb = [17.0, 17.0, 17.0];
        struct Pick {
            dots: Vec<ThemeDot>,
            opacity: f64,
            both: bool,
        }
        let sweep: Vec<ThemeDot> = [0.1, 0.3, 0.5, 0.7, 0.9]
            .iter()
            .flat_map(|&radius| [0.0, 60.0, 120.0, 180.0, 240.0, 300.0].map(|angle| dot(angle, radius)))
            .collect();
        let mut picks: Vec<Pick> = Vec::new();
        picks.extend(THEME_PRESETS.iter().map(|preset| Pick { dots: preset_dots(preset), opacity: 0.5, both: true }));
        picks.extend(sweep.iter().map(|d| Pick { dots: vec![d.clone()], opacity: 0.5, both: true }));
        // At full opacity a near-black pick on the light side leaves no lightness of its hue that reads on both.
        picks.extend(sweep.iter().map(|d| Pick { dots: vec![d.clone()], opacity: 1.0, both: false }));
        for app_dark in [false, true] {
            for pick in &picks {
                let chosen = WorkspaceTheme { dots: pick.dots.clone(), opacity: JsNumber(pick.opacity), mode: ThemeMode::Auto, ..theme.clone() };
                let ink = theme_ink(&chosen, app_dark);
                // The ground is the side the theme draws under, which Auto may move off the app's, at the opacity it paints at.
                let under = match theme_scheme(&chosen, app_dark) {
                    Side::Light => light,
                    Side::Dark => dark,
                };
                let washed = over(&dot_colour(&pick.dots[0]), effective_opacity(&chosen, app_dark), &under);
                let on_ground = contrast_ratio(&ink, &under) >= INK_FLOOR as f64;
                let on_washed = if pick.both { contrast_ratio(&ink, &washed) >= INK_FLOOR as f64 } else { true };
                assert!(
                    on_ground && on_washed,
                    "{:?} opacity {} appDark {app_dark}: onGround {on_ground} onWashed {on_washed}",
                    pick.dots,
                    pick.opacity
                );
            }
        }
    }
}

mod the_presets {
    use super::*;

    #[test]
    fn keep_every_dot_thirty_degrees_off_the_green_that_means_running_and_the_red_that_() {
        let apart = |a: f64, b: f64| -> f64 {
            let d = (a - b).abs() % 360.0;
            d.min(360.0 - d)
        };
        for preset in THEME_PRESETS.iter() {
            // The TS table is built by harmonyDots from each preset's first dot, so the dots checked are those.
            let first = preset_dots(preset)[0].clone();
            assert_eq!(preset_dots(preset), harmony_dots(&first, preset.harmony), "{}", preset.id);
            for &(angle, _) in preset.dots {
                let off_green = apart(angle, 160.0) >= 30.0;
                let off_red = apart(angle, 0.0) >= 30.0;
                assert!(off_green && off_red, "{} angle {angle}: offGreen {off_green} offRed {off_red}", preset.id);
            }
        }
    }

    #[test]
    fn are_distinct_each_a_harmony_with_its_dots_placed_and_applying_one_moves_the_dots() {
        let theme = theme();
        assert!(THEME_PRESETS.len() >= 6);
        assert_eq!(THEME_PRESETS.iter().map(|p| p.id).collect::<HashSet<_>>().len(), THEME_PRESETS.len());
        for chosen in THEME_PRESETS.iter() {
            let dots = preset_dots(chosen);
            assert_eq!(dots, harmony_dots(&dots[0], chosen.harmony), "{}", chosen.id);
            assert!(chosen.dots.len() <= THEME_MAX_DOTS as usize);
        }
        let custom = WorkspaceTheme { grain: JsNumber(0.5), opacity: JsNumber(0.3), mode: ThemeMode::Dark, ..theme.clone() };
        let applied = apply_preset(&custom, &THEME_PRESETS[2]);
        assert_eq!(applied.grain, JsNumber(0.5));
        assert_eq!(applied.opacity, JsNumber(0.3));
        assert_eq!(applied.mode, ThemeMode::Dark);
        assert_eq!(applied.harmony, THEME_PRESETS[2].harmony);
        assert!(is_preset(&applied, &THEME_PRESETS[2]));
        assert!(!is_preset(&applied, &THEME_PRESETS[0]));
        assert!(!is_preset(&move_first_dot(&applied, &dot(5.0, 0.5)), &THEME_PRESETS[2]));
        assert!(is_preset(&default_theme(), &THEME_PRESETS[0]));
    }
}

// Cases the spec pins; every expected value is what the TS function returned under node at a3b870a6e.
mod pinned {
    use wsp_words::workspace_look::*;

    use serde_json::{json, Value};
    use wsp_wire::types::workspace_look::{ThemeDot, ThemeHarmony, WorkspaceTheme};

    fn js<T: serde::Serialize>(v: &T) -> String {
        wsp_words::exit::json_line(&serde_json::to_value(v).unwrap(), None)
    }

    fn th(v: Value) -> WorkspaceTheme {
        serde_json::from_value(v).unwrap()
    }

    fn dot(v: Value) -> ThemeDot {
        serde_json::from_value(v).unwrap()
    }

    fn side(s: Side) -> String {
        serde_json::to_value(s).unwrap().as_str().unwrap().to_string()
    }

    #[test]
    fn harmonies_place_dots_at_fixed_angles_and_cycle_within_a_size() {
        assert_eq!(harmony_size(ThemeHarmony::Single) as f64, 1.0); // harmonySize("single")
        assert_eq!(harmony_size(ThemeHarmony::Complementary) as f64, 2.0); // harmonySize("complementary")
        assert_eq!(harmony_size(ThemeHarmony::Analogous) as f64, 2.0); // harmonySize("analogous")
        assert_eq!(harmony_size(ThemeHarmony::SplitComplementary) as f64, 3.0); // harmonySize("splitComplementary")
        assert_eq!(harmony_size(ThemeHarmony::Triadic) as f64, 3.0); // harmonySize("triadic")
        assert_eq!(harmony_size(ThemeHarmony::AnalogousThree) as f64, 3.0); // harmonySize("analogousThree")
        assert_eq!(js(&harmonies_of(0)), "[]"); // JSON.stringify(harmoniesOf(0))
        assert_eq!(js(&harmonies_of(1)), "[\"single\"]"); // JSON.stringify(harmoniesOf(1))
        assert_eq!(js(&harmonies_of(2)), "[\"complementary\",\"analogous\"]"); // JSON.stringify(harmoniesOf(2))
        assert_eq!(js(&harmonies_of(3)), "[\"splitComplementary\",\"triadic\",\"analogousThree\"]"); // JSON.stringify(harmoniesOf(3))
        assert_eq!(js(&harmonies_of(4)), "[]"); // JSON.stringify(harmoniesOf(4))
        assert_eq!(js(&harmony_dots(&dot(json!({"angle":300,"radius":0.5})), ThemeHarmony::Single)), "[{\"angle\":300,\"radius\":0.5}]"); // JSON.stringify(harmonyDots({angle:300,radius:0.5}, "single"))
        assert_eq!(js(&harmony_dots(&dot(json!({"angle":300,"radius":0.5})), ThemeHarmony::Complementary)), "[{\"angle\":300,\"radius\":0.5},{\"angle\":120,\"radius\":0.5}]"); // JSON.stringify(harmonyDots({angle:300,radius:0.5}, "complementary"))
        assert_eq!(js(&harmony_dots(&dot(json!({"angle":300,"radius":0.5})), ThemeHarmony::Analogous)), "[{\"angle\":300,\"radius\":0.5},{\"angle\":250,\"radius\":0.5}]"); // JSON.stringify(harmonyDots({angle:300,radius:0.5}, "analogous"))
        assert_eq!(js(&harmony_dots(&dot(json!({"angle":300,"radius":0.5})), ThemeHarmony::SplitComplementary)), "[{\"angle\":300,\"radius\":0.5},{\"angle\":90,\"radius\":0.5},{\"angle\":150,\"radius\":0.5}]"); // JSON.stringify(harmonyDots({angle:300,radius:0.5}, "splitComplementary"))
        assert_eq!(js(&harmony_dots(&dot(json!({"angle":300,"radius":0.5})), ThemeHarmony::Triadic)), "[{\"angle\":300,\"radius\":0.5},{\"angle\":60,\"radius\":0.5},{\"angle\":180,\"radius\":0.5}]"); // JSON.stringify(harmonyDots({angle:300,radius:0.5}, "triadic"))
        assert_eq!(js(&harmony_dots(&dot(json!({"angle":300,"radius":0.5})), ThemeHarmony::AnalogousThree)), "[{\"angle\":300,\"radius\":0.5},{\"angle\":350,\"radius\":0.5},{\"angle\":250,\"radius\":0.5}]"); // JSON.stringify(harmonyDots({angle:300,radius:0.5}, "analogousThree"))
        assert_eq!(js(&move_first_dot(&th(json!({"dots":[{"angle":205,"radius":0.56},{"angle":325,"radius":0.56},{"angle":85,"radius":0.56}],"harmony":"triadic","grain":0,"opacity":0.8,"mode":"dark"})), &dot(json!({"angle":10,"radius":0.2})))), "{\"dots\":[{\"angle\":10,\"radius\":0.2},{\"angle\":130,\"radius\":0.2},{\"angle\":250,\"radius\":0.2}],\"harmony\":\"triadic\",\"grain\":0,\"opacity\":0.8,\"mode\":\"dark\"}"); // JSON.stringify(moveFirstDot({"dots":[{"angle":205,"radius":0.56},{"angle":325,"radius":0.56},{"angle":85,"radius":0.56}],"harmony":"triadic","grain":0,"opacity":0.8,"mode":"dark"}, {angle:10,radius:0.2}))
        assert_eq!(js(&resize_dots(&th(json!({"dots":[{"angle":55,"radius":0.72}],"harmony":"single","grain":0.3,"opacity":1,"mode":"auto"})), true)), "{\"dots\":[{\"angle\":55,\"radius\":0.72},{\"angle\":235,\"radius\":0.72}],\"harmony\":\"complementary\",\"grain\":0.3,\"opacity\":1,\"mode\":\"auto\"}"); // JSON.stringify(resizeDots({"dots":[{"angle":55,"radius":0.72}],"harmony":"single","grain":0.3,"opacity":1,"mode":"auto"}, 1))
        assert_eq!(js(&resize_dots(&th(json!({"dots":[{"angle":55,"radius":0.72}],"harmony":"single","grain":0.3,"opacity":1,"mode":"auto"})), false)), "null"); // JSON.stringify(resizeDots({"dots":[{"angle":55,"radius":0.72}],"harmony":"single","grain":0.3,"opacity":1,"mode":"auto"}, -1))
        assert_eq!(js(&resize_dots(&th(json!({"dots":[{"angle":262,"radius":0.62},{"angle":212,"radius":0.62}],"harmony":"analogous","grain":0,"opacity":0.5,"mode":"auto"})), true)), "{\"dots\":[{\"angle\":262,\"radius\":0.62},{\"angle\":52,\"radius\":0.62},{\"angle\":112,\"radius\":0.62}],\"harmony\":\"splitComplementary\",\"grain\":0,\"opacity\":0.5,\"mode\":\"auto\"}"); // JSON.stringify(resizeDots({"dots":[{"angle":262,"radius":0.62},{"angle":212,"radius":0.62}],"harmony":"analogous","grain":0,"opacity":0.5,"mode":"auto"}, 1))
        assert_eq!(js(&resize_dots(&th(json!({"dots":[{"angle":262,"radius":0.62},{"angle":212,"radius":0.62}],"harmony":"analogous","grain":0,"opacity":0.5,"mode":"auto"})), false)), "{\"dots\":[{\"angle\":262,\"radius\":0.62}],\"harmony\":\"single\",\"grain\":0,\"opacity\":0.5,\"mode\":\"auto\"}"); // JSON.stringify(resizeDots({"dots":[{"angle":262,"radius":0.62},{"angle":212,"radius":0.62}],"harmony":"analogous","grain":0,"opacity":0.5,"mode":"auto"}, -1))
        assert_eq!(js(&resize_dots(&th(json!({"dots":[{"angle":205,"radius":0.56},{"angle":325,"radius":0.56},{"angle":85,"radius":0.56}],"harmony":"triadic","grain":0,"opacity":0.8,"mode":"dark"})), true)), "null"); // JSON.stringify(resizeDots({"dots":[{"angle":205,"radius":0.56},{"angle":325,"radius":0.56},{"angle":85,"radius":0.56}],"harmony":"triadic","grain":0,"opacity":0.8,"mode":"dark"}, 1))
        assert_eq!(js(&resize_dots(&th(json!({"dots":[{"angle":205,"radius":0.56},{"angle":325,"radius":0.56},{"angle":85,"radius":0.56}],"harmony":"triadic","grain":0,"opacity":0.8,"mode":"dark"})), false)), "{\"dots\":[{\"angle\":205,\"radius\":0.56},{\"angle\":25,\"radius\":0.56}],\"harmony\":\"complementary\",\"grain\":0,\"opacity\":0.8,\"mode\":\"dark\"}"); // JSON.stringify(resizeDots({"dots":[{"angle":205,"radius":0.56},{"angle":325,"radius":0.56},{"angle":85,"radius":0.56}],"harmony":"triadic","grain":0,"opacity":0.8,"mode":"dark"}, -1))
        assert_eq!(js(&cycle_harmony(&th(json!({"dots":[{"angle":55,"radius":0.72}],"harmony":"single","grain":0.3,"opacity":1,"mode":"auto"})))), "{\"dots\":[{\"angle\":55,\"radius\":0.72}],\"harmony\":\"single\",\"grain\":0.3,\"opacity\":1,\"mode\":\"auto\"}"); // JSON.stringify(cycleHarmony({"dots":[{"angle":55,"radius":0.72}],"harmony":"single","grain":0.3,"opacity":1,"mode":"auto"}))
        assert_eq!(js(&cycle_harmony(&th(json!({"dots":[{"angle":262,"radius":0.62},{"angle":212,"radius":0.62}],"harmony":"analogous","grain":0,"opacity":0.5,"mode":"auto"})))), "{\"dots\":[{\"angle\":262,\"radius\":0.62},{\"angle\":82,\"radius\":0.62}],\"harmony\":\"complementary\",\"grain\":0,\"opacity\":0.5,\"mode\":\"auto\"}"); // JSON.stringify(cycleHarmony({"dots":[{"angle":262,"radius":0.62},{"angle":212,"radius":0.62}],"harmony":"analogous","grain":0,"opacity":0.5,"mode":"auto"}))
        assert_eq!(js(&cycle_harmony(&th(json!({"dots":[{"angle":205,"radius":0.56},{"angle":325,"radius":0.56},{"angle":85,"radius":0.56}],"harmony":"triadic","grain":0,"opacity":0.8,"mode":"dark"})))), "{\"dots\":[{\"angle\":205,\"radius\":0.56},{\"angle\":255,\"radius\":0.56},{\"angle\":155,\"radius\":0.56}],\"harmony\":\"analogousThree\",\"grain\":0,\"opacity\":0.8,\"mode\":\"dark\"}"); // JSON.stringify(cycleHarmony({"dots":[{"angle":205,"radius":0.56},{"angle":325,"radius":0.56},{"angle":85,"radius":0.56}],"harmony":"triadic","grain":0,"opacity":0.8,"mode":"dark"}))
        assert_eq!(js(&cycle_harmony(&th(json!({"dots":[{"angle":205,"radius":0.56},{"angle":325,"radius":0.56},{"angle":85,"radius":0.56}],"harmony":"analogousThree","grain":0,"opacity":0.8,"mode":"dark"})))), "{\"dots\":[{\"angle\":205,\"radius\":0.56},{\"angle\":355,\"radius\":0.56},{\"angle\":55,\"radius\":0.56}],\"harmony\":\"splitComplementary\",\"grain\":0,\"opacity\":0.8,\"mode\":\"dark\"}"); // JSON.stringify(cycleHarmony({"dots":[{"angle":205,"radius":0.56},{"angle":325,"radius":0.56},{"angle":85,"radius":0.56}],"harmony":"analogousThree","grain":0,"opacity":0.8,"mode":"dark"}))
    }

    #[test]
    fn grain_snaps_and_colours_round_to_whole_channels() {
        assert_eq!(snap_grain(-1_f64), 0.0); // snapGrain(-1)
        assert_eq!(snap_grain(0_f64), 0.0); // snapGrain(0)
        assert_eq!(snap_grain(0.03_f64), 0.0); // snapGrain(0.03)
        assert_eq!(snap_grain(0.0313_f64), 0.0625); // snapGrain(0.0313)
        assert_eq!(snap_grain(0.5_f64), 0.5); // snapGrain(0.5)
        assert_eq!(snap_grain(0.97_f64), 1.0); // snapGrain(0.97)
        assert_eq!(snap_grain(2_f64), 1.0); // snapGrain(2)
        assert_eq!(js(&dot_colour(&dot(json!({"angle":0,"radius":0})))), "[0,0,0]"); // JSON.stringify(dotColour({"angle":0,"radius":0}))
        assert_eq!(js(&dot_colour(&dot(json!({"angle":0,"radius":0.5})))), "[249,6,6]"); // JSON.stringify(dotColour({"angle":0,"radius":0.5}))
        assert_eq!(js(&dot_colour(&dot(json!({"angle":120,"radius":1})))), "[255,255,255]"); // JSON.stringify(dotColour({"angle":120,"radius":1}))
        assert_eq!(js(&dot_colour(&dot(json!({"angle":262,"radius":0.62})))), "[133,65,251]"); // JSON.stringify(dotColour({"angle":262,"radius":0.62}))
        assert_eq!(js(&dot_colour(&dot(json!({"angle":359,"radius":0.3})))), "[148,5,8]"); // JSON.stringify(dotColour({"angle":359,"radius":0.3}))
        assert_eq!(js(&hsl_to_rgb(0_f64, 0_f64, 0.5_f64)), "[128,128,128]"); // JSON.stringify(hslToRgb(0, 0, 0.5))
        assert_eq!(js(&hsl_to_rgb(0.5_f64, 1_f64, 0.5_f64)), "[0,255,255]"); // JSON.stringify(hslToRgb(0.5, 1, 0.5))
        assert_eq!(js(&hsl_to_rgb(1_f64, 0.4_f64, 0.2_f64)), "[71,31,31]"); // JSON.stringify(hslToRgb(1, 0.4, 0.2))
        assert_eq!(js(&hsl_to_rgb(-0.25_f64, 0.5_f64, 0.7_f64)), "[178,140,217]"); // JSON.stringify(hslToRgb(-0.25, 0.5, 0.7))
        assert_eq!(js(&rgb_to_hsl(&[0.0, 0.0, 0.0])), "[0,0,0]"); // JSON.stringify(rgbToHsl([0,0,0]))
        assert_eq!(js(&rgb_to_hsl(&[255.0, 0.0, 0.0])), "[0,1,0.5]"); // JSON.stringify(rgbToHsl([255,0,0]))
        assert_eq!(js(&rgb_to_hsl(&[0.0, 255.0, 0.0])), "[120,1,0.5]"); // JSON.stringify(rgbToHsl([0,255,0]))
        assert_eq!(js(&rgb_to_hsl(&[0.0, 0.0, 255.0])), "[240,1,0.5]"); // JSON.stringify(rgbToHsl([0,0,255]))
        assert_eq!(js(&rgb_to_hsl(&[10.0, 200.0, 30.0])), "[126.3157894736842,0.9047619047619049,0.4117647058823529]"); // JSON.stringify(rgbToHsl([10,200,30]))
        assert_eq!(js(&rgb_to_hsl(&[128.0, 128.0, 128.0])), "[0,0,0.5019607843137255]"); // JSON.stringify(rgbToHsl([128,128,128]))
        assert_eq!(wsp_words::js::to_fixed(contrast_ratio(&[0.0, 0.0, 0.0], &[255.0, 255.0, 255.0]), 10), "21.0000000000"); // contrastRatio([0,0,0], [255,255,255]).toFixed(10)
        assert_eq!(wsp_words::js::to_fixed(contrast_ratio(&[39.0, 39.0, 42.0], &[250.0, 250.0, 250.0]), 10), "14.2700563010"); // contrastRatio([39,39,42], [250,250,250]).toFixed(10)
        assert_eq!(wsp_words::js::to_fixed(contrast_ratio(&[10.0, 10.0, 10.0], &[10.0, 10.0, 10.0]), 10), "1.0000000000"); // contrastRatio([10,10,10], [10,10,10]).toFixed(10)
    }

    #[test]
    fn the_side_the_cap_and_the_ink_read_off_the_wash() {
        assert_eq!(side(theme_scheme(&th(json!({"dots":[{"angle":55,"radius":0.72}],"harmony":"single","grain":0.3,"opacity":1,"mode":"auto"})), true)), "dark"); // themeScheme({"dots":[{"angle":55,"radius":0.72}],"harmony":"single","grain":0.3,"opacity":1,"mode":"auto"}, true)
        assert_eq!(wsp_words::js::number_string(opacity_cap(&th(json!({"dots":[{"angle":55,"radius":0.72}],"harmony":"single","grain":0.3,"opacity":1,"mode":"auto"})), true)), "0.4"); // String(opacityCap({"dots":[{"angle":55,"radius":0.72}],"harmony":"single","grain":0.3,"opacity":1,"mode":"auto"}, true))
        assert_eq!(wsp_words::js::number_string(effective_opacity(&th(json!({"dots":[{"angle":55,"radius":0.72}],"harmony":"single","grain":0.3,"opacity":1,"mode":"auto"})), true)), "0.4"); // String(effectiveOpacity({"dots":[{"angle":55,"radius":0.72}],"harmony":"single","grain":0.3,"opacity":1,"mode":"auto"}, true))
        assert_eq!(js(&theme_ink(&th(json!({"dots":[{"angle":55,"radius":0.72}],"harmony":"single","grain":0.3,"opacity":1,"mode":"auto"})), true)), "[255,241,88]"); // JSON.stringify(themeInk({"dots":[{"angle":55,"radius":0.72}],"harmony":"single","grain":0.3,"opacity":1,"mode":"auto"}, true))
        assert_eq!(side(theme_scheme(&th(json!({"dots":[{"angle":55,"radius":0.72}],"harmony":"single","grain":0.3,"opacity":1,"mode":"auto"})), false)), "light"); // themeScheme({"dots":[{"angle":55,"radius":0.72}],"harmony":"single","grain":0.3,"opacity":1,"mode":"auto"}, false)
        assert_eq!(wsp_words::js::number_string(opacity_cap(&th(json!({"dots":[{"angle":55,"radius":0.72}],"harmony":"single","grain":0.3,"opacity":1,"mode":"auto"})), false)), "1"); // String(opacityCap({"dots":[{"angle":55,"radius":0.72}],"harmony":"single","grain":0.3,"opacity":1,"mode":"auto"}, false))
        assert_eq!(wsp_words::js::number_string(effective_opacity(&th(json!({"dots":[{"angle":55,"radius":0.72}],"harmony":"single","grain":0.3,"opacity":1,"mode":"auto"})), false)), "1"); // String(effectiveOpacity({"dots":[{"angle":55,"radius":0.72}],"harmony":"single","grain":0.3,"opacity":1,"mode":"auto"}, false))
        assert_eq!(js(&theme_ink(&th(json!({"dots":[{"angle":55,"radius":0.72}],"harmony":"single","grain":0.3,"opacity":1,"mode":"auto"})), false)), "[137,125,0]"); // JSON.stringify(themeInk({"dots":[{"angle":55,"radius":0.72}],"harmony":"single","grain":0.3,"opacity":1,"mode":"auto"}, false))
        assert_eq!(side(theme_scheme(&th(json!({"dots":[{"angle":262,"radius":0.62},{"angle":212,"radius":0.62}],"harmony":"analogous","grain":0,"opacity":0.5,"mode":"auto"})), true)), "dark"); // themeScheme({"dots":[{"angle":262,"radius":0.62},{"angle":212,"radius":0.62}],"harmony":"analogous","grain":0,"opacity":0.5,"mode":"auto"}, true)
        assert_eq!(wsp_words::js::number_string(opacity_cap(&th(json!({"dots":[{"angle":262,"radius":0.62},{"angle":212,"radius":0.62}],"harmony":"analogous","grain":0,"opacity":0.5,"mode":"auto"})), true)), "0.7"); // String(opacityCap({"dots":[{"angle":262,"radius":0.62},{"angle":212,"radius":0.62}],"harmony":"analogous","grain":0,"opacity":0.5,"mode":"auto"}, true))
        assert_eq!(wsp_words::js::number_string(effective_opacity(&th(json!({"dots":[{"angle":262,"radius":0.62},{"angle":212,"radius":0.62}],"harmony":"analogous","grain":0,"opacity":0.5,"mode":"auto"})), true)), "0.5"); // String(effectiveOpacity({"dots":[{"angle":262,"radius":0.62},{"angle":212,"radius":0.62}],"harmony":"analogous","grain":0,"opacity":0.5,"mode":"auto"}, true))
        assert_eq!(js(&theme_ink(&th(json!({"dots":[{"angle":262,"radius":0.62},{"angle":212,"radius":0.62}],"harmony":"analogous","grain":0,"opacity":0.5,"mode":"auto"})), true)), "[162,108,255]"); // JSON.stringify(themeInk({"dots":[{"angle":262,"radius":0.62},{"angle":212,"radius":0.62}],"harmony":"analogous","grain":0,"opacity":0.5,"mode":"auto"}, true))
        assert_eq!(side(theme_scheme(&th(json!({"dots":[{"angle":262,"radius":0.62},{"angle":212,"radius":0.62}],"harmony":"analogous","grain":0,"opacity":0.5,"mode":"auto"})), false)), "light"); // themeScheme({"dots":[{"angle":262,"radius":0.62},{"angle":212,"radius":0.62}],"harmony":"analogous","grain":0,"opacity":0.5,"mode":"auto"}, false)
        assert_eq!(wsp_words::js::number_string(opacity_cap(&th(json!({"dots":[{"angle":262,"radius":0.62},{"angle":212,"radius":0.62}],"harmony":"analogous","grain":0,"opacity":0.5,"mode":"auto"})), false)), "0.7"); // String(opacityCap({"dots":[{"angle":262,"radius":0.62},{"angle":212,"radius":0.62}],"harmony":"analogous","grain":0,"opacity":0.5,"mode":"auto"}, false))
        assert_eq!(wsp_words::js::number_string(effective_opacity(&th(json!({"dots":[{"angle":262,"radius":0.62},{"angle":212,"radius":0.62}],"harmony":"analogous","grain":0,"opacity":0.5,"mode":"auto"})), false)), "0.5"); // String(effectiveOpacity({"dots":[{"angle":262,"radius":0.62},{"angle":212,"radius":0.62}],"harmony":"analogous","grain":0,"opacity":0.5,"mode":"auto"}, false))
        assert_eq!(js(&theme_ink(&th(json!({"dots":[{"angle":262,"radius":0.62},{"angle":212,"radius":0.62}],"harmony":"analogous","grain":0,"opacity":0.5,"mode":"auto"})), false)), "[80,0,218]"); // JSON.stringify(themeInk({"dots":[{"angle":262,"radius":0.62},{"angle":212,"radius":0.62}],"harmony":"analogous","grain":0,"opacity":0.5,"mode":"auto"}, false))
        assert_eq!(side(theme_scheme(&th(json!({"dots":[{"angle":205,"radius":0.56},{"angle":325,"radius":0.56},{"angle":85,"radius":0.56}],"harmony":"triadic","grain":0,"opacity":0.8,"mode":"dark"})), true)), "dark"); // themeScheme({"dots":[{"angle":205,"radius":0.56},{"angle":325,"radius":0.56},{"angle":85,"radius":0.56}],"harmony":"triadic","grain":0,"opacity":0.8,"mode":"dark"}, true)
        assert_eq!(wsp_words::js::number_string(opacity_cap(&th(json!({"dots":[{"angle":205,"radius":0.56},{"angle":325,"radius":0.56},{"angle":85,"radius":0.56}],"harmony":"triadic","grain":0,"opacity":0.8,"mode":"dark"})), true)), "0.45"); // String(opacityCap({"dots":[{"angle":205,"radius":0.56},{"angle":325,"radius":0.56},{"angle":85,"radius":0.56}],"harmony":"triadic","grain":0,"opacity":0.8,"mode":"dark"}, true))
        assert_eq!(wsp_words::js::number_string(effective_opacity(&th(json!({"dots":[{"angle":205,"radius":0.56},{"angle":325,"radius":0.56},{"angle":85,"radius":0.56}],"harmony":"triadic","grain":0,"opacity":0.8,"mode":"dark"})), true)), "0.45"); // String(effectiveOpacity({"dots":[{"angle":205,"radius":0.56},{"angle":325,"radius":0.56},{"angle":85,"radius":0.56}],"harmony":"triadic","grain":0,"opacity":0.8,"mode":"dark"}, true))
        assert_eq!(js(&theme_ink(&th(json!({"dots":[{"angle":205,"radius":0.56},{"angle":325,"radius":0.56},{"angle":85,"radius":0.56}],"harmony":"triadic","grain":0,"opacity":0.8,"mode":"dark"})), true)), "[71,179,255]"); // JSON.stringify(themeInk({"dots":[{"angle":205,"radius":0.56},{"angle":325,"radius":0.56},{"angle":85,"radius":0.56}],"harmony":"triadic","grain":0,"opacity":0.8,"mode":"dark"}, true))
        assert_eq!(side(theme_scheme(&th(json!({"dots":[{"angle":205,"radius":0.56},{"angle":325,"radius":0.56},{"angle":85,"radius":0.56}],"harmony":"triadic","grain":0,"opacity":0.8,"mode":"dark"})), false)), "dark"); // themeScheme({"dots":[{"angle":205,"radius":0.56},{"angle":325,"radius":0.56},{"angle":85,"radius":0.56}],"harmony":"triadic","grain":0,"opacity":0.8,"mode":"dark"}, false)
        assert_eq!(wsp_words::js::number_string(opacity_cap(&th(json!({"dots":[{"angle":205,"radius":0.56},{"angle":325,"radius":0.56},{"angle":85,"radius":0.56}],"harmony":"triadic","grain":0,"opacity":0.8,"mode":"dark"})), false)), "0.45"); // String(opacityCap({"dots":[{"angle":205,"radius":0.56},{"angle":325,"radius":0.56},{"angle":85,"radius":0.56}],"harmony":"triadic","grain":0,"opacity":0.8,"mode":"dark"}, false))
        assert_eq!(wsp_words::js::number_string(effective_opacity(&th(json!({"dots":[{"angle":205,"radius":0.56},{"angle":325,"radius":0.56},{"angle":85,"radius":0.56}],"harmony":"triadic","grain":0,"opacity":0.8,"mode":"dark"})), false)), "0.45"); // String(effectiveOpacity({"dots":[{"angle":205,"radius":0.56},{"angle":325,"radius":0.56},{"angle":85,"radius":0.56}],"harmony":"triadic","grain":0,"opacity":0.8,"mode":"dark"}, false))
        assert_eq!(js(&theme_ink(&th(json!({"dots":[{"angle":205,"radius":0.56},{"angle":325,"radius":0.56},{"angle":85,"radius":0.56}],"harmony":"triadic","grain":0,"opacity":0.8,"mode":"dark"})), false)), "[71,179,255]"); // JSON.stringify(themeInk({"dots":[{"angle":205,"radius":0.56},{"angle":325,"radius":0.56},{"angle":85,"radius":0.56}],"harmony":"triadic","grain":0,"opacity":0.8,"mode":"dark"}, false))
        assert_eq!(side(theme_scheme(&th(json!({"dots":[{"angle":60,"radius":0.95}],"harmony":"single","grain":0,"opacity":1,"mode":"auto"})), true)), "dark"); // themeScheme({"dots":[{"angle":60,"radius":0.95}],"harmony":"single","grain":0,"opacity":1,"mode":"auto"}, true)
        assert_eq!(wsp_words::js::number_string(opacity_cap(&th(json!({"dots":[{"angle":60,"radius":0.95}],"harmony":"single","grain":0,"opacity":1,"mode":"auto"})), true)), "0.4"); // String(opacityCap({"dots":[{"angle":60,"radius":0.95}],"harmony":"single","grain":0,"opacity":1,"mode":"auto"}, true))
        assert_eq!(wsp_words::js::number_string(effective_opacity(&th(json!({"dots":[{"angle":60,"radius":0.95}],"harmony":"single","grain":0,"opacity":1,"mode":"auto"})), true)), "0.4"); // String(effectiveOpacity({"dots":[{"angle":60,"radius":0.95}],"harmony":"single","grain":0,"opacity":1,"mode":"auto"}, true))
        assert_eq!(js(&theme_ink(&th(json!({"dots":[{"angle":60,"radius":0.95}],"harmony":"single","grain":0,"opacity":1,"mode":"auto"})), true)), "[255,255,111]"); // JSON.stringify(themeInk({"dots":[{"angle":60,"radius":0.95}],"harmony":"single","grain":0,"opacity":1,"mode":"auto"}, true))
        assert_eq!(side(theme_scheme(&th(json!({"dots":[{"angle":60,"radius":0.95}],"harmony":"single","grain":0,"opacity":1,"mode":"auto"})), false)), "light"); // themeScheme({"dots":[{"angle":60,"radius":0.95}],"harmony":"single","grain":0,"opacity":1,"mode":"auto"}, false)
        assert_eq!(wsp_words::js::number_string(opacity_cap(&th(json!({"dots":[{"angle":60,"radius":0.95}],"harmony":"single","grain":0,"opacity":1,"mode":"auto"})), false)), "1"); // String(opacityCap({"dots":[{"angle":60,"radius":0.95}],"harmony":"single","grain":0,"opacity":1,"mode":"auto"}, false))
        assert_eq!(wsp_words::js::number_string(effective_opacity(&th(json!({"dots":[{"angle":60,"radius":0.95}],"harmony":"single","grain":0,"opacity":1,"mode":"auto"})), false)), "1"); // String(effectiveOpacity({"dots":[{"angle":60,"radius":0.95}],"harmony":"single","grain":0,"opacity":1,"mode":"auto"}, false))
        assert_eq!(js(&theme_ink(&th(json!({"dots":[{"angle":60,"radius":0.95}],"harmony":"single","grain":0,"opacity":1,"mode":"auto"})), false)), "[145,145,0]"); // JSON.stringify(themeInk({"dots":[{"angle":60,"radius":0.95}],"harmony":"single","grain":0,"opacity":1,"mode":"auto"}, false))
        assert_eq!(side(theme_scheme(&th(json!({"dots":[{"angle":200,"radius":0.1}],"harmony":"single","grain":0,"opacity":0.6,"mode":"light"})), true)), "light"); // themeScheme({"dots":[{"angle":200,"radius":0.1}],"harmony":"single","grain":0,"opacity":0.6,"mode":"light"}, true)
        assert_eq!(wsp_words::js::number_string(opacity_cap(&th(json!({"dots":[{"angle":200,"radius":0.1}],"harmony":"single","grain":0,"opacity":0.6,"mode":"light"})), true)), "0.45"); // String(opacityCap({"dots":[{"angle":200,"radius":0.1}],"harmony":"single","grain":0,"opacity":0.6,"mode":"light"}, true))
        assert_eq!(wsp_words::js::number_string(effective_opacity(&th(json!({"dots":[{"angle":200,"radius":0.1}],"harmony":"single","grain":0,"opacity":0.6,"mode":"light"})), true)), "0.45"); // String(effectiveOpacity({"dots":[{"angle":200,"radius":0.1}],"harmony":"single","grain":0,"opacity":0.6,"mode":"light"}, true))
        assert_eq!(js(&theme_ink(&th(json!({"dots":[{"angle":200,"radius":0.1}],"harmony":"single","grain":0,"opacity":0.6,"mode":"light"})), true)), "[0,69,104]"); // JSON.stringify(themeInk({"dots":[{"angle":200,"radius":0.1}],"harmony":"single","grain":0,"opacity":0.6,"mode":"light"}, true))
        assert_eq!(side(theme_scheme(&th(json!({"dots":[{"angle":200,"radius":0.1}],"harmony":"single","grain":0,"opacity":0.6,"mode":"light"})), false)), "light"); // themeScheme({"dots":[{"angle":200,"radius":0.1}],"harmony":"single","grain":0,"opacity":0.6,"mode":"light"}, false)
        assert_eq!(wsp_words::js::number_string(opacity_cap(&th(json!({"dots":[{"angle":200,"radius":0.1}],"harmony":"single","grain":0,"opacity":0.6,"mode":"light"})), false)), "0.45"); // String(opacityCap({"dots":[{"angle":200,"radius":0.1}],"harmony":"single","grain":0,"opacity":0.6,"mode":"light"}, false))
        assert_eq!(wsp_words::js::number_string(effective_opacity(&th(json!({"dots":[{"angle":200,"radius":0.1}],"harmony":"single","grain":0,"opacity":0.6,"mode":"light"})), false)), "0.45"); // String(effectiveOpacity({"dots":[{"angle":200,"radius":0.1}],"harmony":"single","grain":0,"opacity":0.6,"mode":"light"}, false))
        assert_eq!(js(&theme_ink(&th(json!({"dots":[{"angle":200,"radius":0.1}],"harmony":"single","grain":0,"opacity":0.6,"mode":"light"})), false)), "[0,69,104]"); // JSON.stringify(themeInk({"dots":[{"angle":200,"radius":0.1}],"harmony":"single","grain":0,"opacity":0.6,"mode":"light"}, false))
    }

    #[test]
    fn presets_apply_and_are_recognised() {
        assert_eq!(js(&apply_preset(&th(json!({"dots":[{"angle":205,"radius":0.56},{"angle":325,"radius":0.56},{"angle":85,"radius":0.56}],"harmony":"triadic","grain":0,"opacity":0.8,"mode":"dark"})), &THEME_PRESETS[0])), "{\"dots\":[{\"angle\":262,\"radius\":0.62},{\"angle\":212,\"radius\":0.62}],\"harmony\":\"analogous\",\"grain\":0,\"opacity\":0.8,\"mode\":\"dark\"}"); // JSON.stringify(applyPreset({"dots":[{"angle":205,"radius":0.56},{"angle":325,"radius":0.56},{"angle":85,"radius":0.56}],"harmony":"triadic","grain":0,"opacity":0.8,"mode":"dark"}, THEME_PRESETS[0]))
        assert_eq!(is_preset(&th(json!({"dots":[{"angle":262,"radius":0.62},{"angle":212,"radius":0.62}],"harmony":"analogous","grain":0,"opacity":0.5,"mode":"auto"})), &THEME_PRESETS[0]), true); // isPreset({"dots":[{"angle":262,"radius":0.62},{"angle":212,"radius":0.62}],"harmony":"analogous","grain":0,"opacity":0.5,"mode":"auto"}, THEME_PRESETS[0])
        assert_eq!(is_preset(&th(json!({"dots":[{"angle":262,"radius":0.62},{"angle":212,"radius":0.62}],"harmony":"analogous","grain":0,"opacity":0.5,"mode":"auto"})), &THEME_PRESETS[1]), false); // isPreset({"dots":[{"angle":262,"radius":0.62},{"angle":212,"radius":0.62}],"harmony":"analogous","grain":0,"opacity":0.5,"mode":"auto"}, THEME_PRESETS[1])
        assert_eq!(is_preset(&th(json!({"dots":[{"angle":262,"radius":0.62},{"angle":212,"radius":0.62}],"harmony":"complementary","grain":0,"opacity":0.5,"mode":"auto"})), &THEME_PRESETS[0]), false); // isPreset({"dots":[{"angle":262,"radius":0.62},{"angle":212,"radius":0.62}],"harmony":"complementary","grain":0,"opacity":0.5,"mode":"auto"}, THEME_PRESETS[0])
        assert_eq!(is_preset(&th(json!({"dots":[{"angle":205,"radius":0.56},{"angle":325,"radius":0.56},{"angle":85,"radius":0.56}],"harmony":"triadic","grain":0,"opacity":0.8,"mode":"dark"})), &THEME_PRESETS[1]), true); // isPreset({"dots":[{"angle":205,"radius":0.56},{"angle":325,"radius":0.56},{"angle":85,"radius":0.56}],"harmony":"triadic","grain":0,"opacity":0.8,"mode":"dark"}, THEME_PRESETS[1])
        assert_eq!(serde_json::to_value(default_theme()).unwrap(), json!({"harmony":"analogous","dots":[{"angle":262,"radius":0.62},{"angle":212,"radius":0.62}],"grain":0,"opacity":0.5,"mode":"auto"})); // JSON.stringify((({id, ...rest}) => rest)(DEFAULT_THEME))
    }
}
