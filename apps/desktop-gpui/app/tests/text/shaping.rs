// SPDX-License-Identifier: AGPL-3.0-only
//! What the text system shapes, apart from where it draws: figures under `ui::text::tabular()`, and where a wrapped line
//! may break, through both of GPUI's wrappers (the text element's and `LineWrapper`, which gpui-kit's inputs and inline
//! text use). Chromium's breaks come from Blink's table for ASCII pairs (`FillAscii` in
//! third_party/blink/renderer/platform/text/character_property_data_generator.cc).

use std::borrow::Cow;
use std::path::Path;

use desktop_gpui::ui::text::tabular;
use gpui_kit::component::ActiveTheme;
use gpui_kit::{font, px, rgb, App, Font, IndentAdjustment, LineFragment, SharedString, TextRun, Window};
/// The code line of the Appearance page's type sample (`sample` in apps/web/src/settings/format.ts).
const CODE: &str = "export const total = (lines: Line[]) => round(lines.reduce((sum, l) => sum + l.price, 0));";
const HYPHENATED: &str = "Delete a finished sub-thread's machine after";
const SIZE: f32 = 14.;

fn run(len: usize, font: Font) -> TextRun {
    TextRun { len, font, color: rgb(0).into(), background_color: None, underline: None, strikethrough: None }
}

fn width(text: &str, font: Font, window: &Window) -> f32 {
    f32::from(window.text_system().shape_line(SharedString::from(text.to_owned()), px(SIZE), &[run(text.len(), font)], None).width)
}

fn advances(text: &str, font: Font, window: &Window) -> Vec<f32> {
    let line = window.text_system().shape_line(SharedString::from(text.to_owned()), px(SIZE), &[run(text.len(), font)], None);
    let xs: Vec<f32> =
        line.runs.iter().flat_map(|r| r.glyphs.iter().map(|g| f32::from(g.position.x))).chain([f32::from(line.width)]).collect();
    xs.windows(2).map(|w| w[1] - w[0]).collect()
}

/// The byte index each wrapped line starts at, from the text element's wrapper and from `LineWrapper`.
fn breaks(text: &str, font: Font, wrap: f32, window: &Window, cx: &App) -> [Vec<usize>; 2] {
    let lines = window
        .text_system()
        .shape_text(SharedString::from(text.to_owned()), px(SIZE), &[run(text.len(), font.clone())], Some(px(wrap)), None)
        .unwrap();
    let layout = &lines[0].unwrapped_layout;
    let element = lines[0].wrap_boundaries().iter().map(|b| layout.runs[b.run_ix].glyphs[b.glyph_ix].index).collect();
    let mut wrapper = cx.text_system().line_wrapper(font, px(SIZE));
    let wrapper = wrapper.wrap_line(&[LineFragment::text(text)], px(wrap), IndentAdjustment::NoIndent).map(|b| b.ix).collect();
    [element, wrapper]
}

const PATHS: [&str; 2] = ["the text element", "LineWrapper"];

/// A face whose figures are proportional until `tnum` is on: SF on macOS. Linux's sans, Liberation Sans, has no `tnum`,
/// so Linux loads text/InterFigures-Regular.ttf, Inter's figures and `m` (cosmic-text loads no face without one) with
/// their `tnum`, cut from Inter 3.019 and renamed under its OFL (text/InterFigures-OFL.txt).
fn proportional(cx: &App) -> Result<SharedString, String> {
    if cfg!(target_os = "macos") {
        return Ok(".SystemUIFont".into());
    }
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/text/InterFigures-Regular.ttf");
    let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    cx.text_system().add_fonts(vec![Cow::Owned(bytes)]).map_err(|e| e.to_string())?;
    Ok("Inter Figures".into())
}

fn tabular_figures_have_equal_advances(window: &mut Window, cx: &mut App) -> Result<(), String> {
    let face = proportional(cx)?;
    let spread = |a: &[f32]| a.iter().cloned().fold(f32::MIN, f32::max) - a.iter().cloned().fold(f32::MAX, f32::min);
    let plain = advances("0123456789", font(face.clone()), window);
    if spread(&plain) < 0.1 {
        return Err(format!("{face}'s figures are one width without tnum ({plain:?}), so this proves nothing"));
    }
    let mut tnum = font(face.clone());
    tnum.features = tabular();
    let figures = advances("0123456789", tnum, window);
    if spread(&figures) > 0.01 {
        return Err(format!("0 to 9 under ui::text::tabular() in {face} advance {figures:?}"));
    }
    Ok(())
}

fn a_hyphenated_word_wraps_after_its_hyphen(window: &mut Window, cx: &mut App) -> Result<(), String> {
    let sans = font(cx.theme().font_family.clone());
    let at = HYPHENATED.find("thread").unwrap();
    let wrap = width(&HYPHENATED[..at], sans.clone(), window) + 1.;
    for (path, starts) in PATHS.iter().zip(breaks(HYPHENATED, sans, wrap, window, cx)) {
        if !starts.contains(&at) {
            let lines: Vec<&str> = starts.iter().map(|&i| &HYPHENATED[i..]).collect();
            return Err(format!("at {wrap:.1} px {path} starts lines at {lines:?}, not after `sub-`"));
        }
    }
    Ok(())
}

fn the_appearance_code_line_breaks_where_chromium_does(window: &mut Window, cx: &mut App) -> Result<(), String> {
    let mono = font(cx.theme().mono_font_family.clone());
    let longest = CODE.split(' ').map(|w| width(w, mono.clone(), window)).fold(0., f32::max);
    let full = width(CODE, mono.clone(), window);
    let cut = |i: usize| format!("`{}` | `{}`", &CODE[i.saturating_sub(8)..i], &CODE[i..(i + 8).min(CODE.len())]);
    let (mut wrapped, mut inside, mut at_space) = (0, None, None);
    // Every word fits from `longest` up, and there Blink's table allows a break in this line only after a space.
    for wrap in (longest.ceil() as u32 + 1)..(full.floor() as u32) {
        for (path, starts) in PATHS.iter().zip(breaks(CODE, mono.clone(), wrap as f32, window, cx)) {
            wrapped += starts.len();
            if let Some(&i) = starts.iter().find(|&&i| !CODE[..i].ends_with(' ') && !CODE[i..].starts_with(' ')) {
                inside.get_or_insert_with(|| format!("at {wrap} px {path} breaks {} inside a word, where Blink allows no break", cut(i)));
            }
            // LineWrapper, which gpui-kit's editor uses, still breaks before a space that crosses the edge.
            if let (Some(&i), "the text element") = (starts.iter().find(|&&i| CODE[i..].starts_with(' ')), *path) {
                at_space.get_or_insert_with(|| {
                    format!("at {wrap} px {path} breaks {} before a space, which should hang at the end of the line", cut(i))
                });
            }
        }
    }
    if wrapped == 0 {
        return Err(format!("the code line never wrapped between {longest} and {full} px"));
    }
    let off: Vec<String> = inside.into_iter().chain(at_space).collect();
    if off.is_empty() {
        Ok(())
    } else {
        Err(off.join("; "))
    }
}

/// Each check's name and its failure, if any.
pub fn checks(window: &mut Window, cx: &mut App) -> Vec<(&'static str, Result<(), String>)> {
    vec![
        ("tabular_figures_have_equal_advances", tabular_figures_have_equal_advances(window, cx)),
        ("a_hyphenated_word_wraps_after_its_hyphen", a_hyphenated_word_wraps_after_its_hyphen(window, cx)),
        ("the_appearance_code_line_breaks_where_chromium_does", the_appearance_code_line_breaks_where_chromium_does(window, cx)),
    ]
}
