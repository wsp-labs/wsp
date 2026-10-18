// SPDX-License-Identifier: AGPL-3.0-only
//! How a terminal looks: the owner's Ghostty config as the host read it (`host.terminalConfig`), over the app theme's
//! terminal colours where the config is silent (`ghosttyConfig.ts`, `terminalThemeFromApp`).

use std::cell::RefCell;
use std::future::Future;

use gpui_kit::{px, App, Font, FontFallbacks, FontStyle, FontWeight, Hsla, Rgba, SharedString};
use libghostty_vt::terminal::CursorStyle;
use serde_json::Value;
use wsp_host_client::{protocol, Failure};

use super::engine::{Colors, Rgb};
use crate::screens::Host;
use crate::ui::ActivePalette;

/// `--font-size-terminal`, the app's own base size.
pub const BASE_SIZE: f32 = 14.;
/// `terminalFontSize`: what the surface draws, whatever the zoom.
pub const MIN_SIZE: f32 = 6.;
pub const MAX_SIZE: f32 = 32.;
/// `DEFAULT_TERMINAL_TEXT_FACES` in `terminal/ghostty/fontChain.ts`.
pub const DEFAULT_FACES: [&str; 6] = ["SF Mono", "SFMono-Regular", "Menlo", "Consolas", "DejaVu Sans Mono", "Liberation Mono"];
/// `DEFAULT_TERMINAL_PADDING`, each side.
const PADDING: f32 = 4.;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Padding {
    pub left: f32,
    pub right: f32,
    pub top: f32,
    pub bottom: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Look {
    pub font_family: SharedString,
    /// The faces a glyph the first lacks is looked for in, in order.
    pub fallbacks: Vec<SharedString>,
    pub font_size: f32,
    pub colors: Colors,
    /// Laid over a selected cell's background.
    pub selection: Hsla,
    pub padding: Padding,
    /// The pane's ground alpha; cells with their own background stay opaque.
    pub opacity: f32,
    pub cursor_style: Option<CursorStyle>,
    pub cursor_blink: bool,
    pub scrollbar_thumb: Hsla,
    pub scrollbar_thumb_hover: Hsla,
}

/// What besides the config picks the face and the size.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Choices {
    /// The family the person typed in the font card (`wsp:terminal-font`), which beats the file's.
    pub family: Option<String>,
    /// The family the host detected as the person's own terminal's.
    pub detected: Option<String>,
    /// The size preference reads the file's `font-size` rather than the app's.
    pub size_from_file: bool,
    /// The workspace's zoom, in pixels.
    pub zoom: f32,
    /// The window draws glass; without it the pane is opaque whatever the config says.
    pub glass: bool,
}

/// The pixel size the surface draws for `base + zoom`.
pub fn font_size(base: f32, zoom: f32) -> f32 {
    (base + zoom).round().clamp(MIN_SIZE, MAX_SIZE)
}

fn rgb_of(c: Hsla) -> Rgb {
    let c = c.to_rgb();
    let b = |v: f32| (v.clamp(0., 1.) * 255.).round() as u8;
    Rgb(b(c.r), b(c.g), b(c.b))
}

pub fn hsla_of(c: Rgb) -> Hsla {
    Rgba { r: c.0 as f32 / 255., g: c.1 as f32 / 255., b: c.2 as f32 / 255., a: 1. }.into()
}

fn rgb_field(v: &Value) -> Option<Rgb> {
    let n = |k: &str| v.get(k)?.as_u64().map(|n| n.min(255) as u8);
    Some(Rgb(n("r")?, n("g")?, n("b")?))
}

thread_local! {
    static INSTALLED: RefCell<Option<Vec<String>>> = const { RefCell::new(None) };
}

fn installed(cx: &App) -> Vec<String> {
    INSTALLED.with(|i| i.borrow_mut().get_or_insert_with(|| cx.text_system().all_font_names()).clone())
}

/// `isMonospaceFamily`: the advances of a few glyphs agree, so a proportional face the person names is refused.
fn monospace(family: &str, cx: &App) -> bool {
    let font = Font {
        family: family.to_owned().into(),
        features: Default::default(),
        fallbacks: None,
        weight: FontWeight::NORMAL,
        style: FontStyle::Normal,
    };
    let id = cx.text_system().resolve_font(&font);
    let widths: Vec<f32> = ['i', 'M', 'W', '0', '@', '#', '.', ' ']
        .iter()
        .filter_map(|&c| cx.text_system().advance(id, px(32.), c).ok().map(|s| f32::from(s.width)))
        .collect();
    widths.first().is_some_and(|w0| widths.iter().all(|w| (w - w0).abs() <= 0.01))
}

impl Look {
    /// The config over the theme, with the app's own size and face choices and the workspace's `zoom`.
    pub fn new(config: Option<&protocol::TerminalConfig>, zoom: Option<f32>, cx: &App) -> Look {
        Look::with(config, &Choices { zoom: zoom.unwrap_or(0.), glass: true, ..Choices::default() }, cx)
    }

    pub fn with(config: Option<&protocol::TerminalConfig>, choices: &Choices, cx: &App) -> Look {
        let file = config.and_then(|c| serde_json::to_value(c).ok()).unwrap_or(Value::Null);
        let p = cx.palette();
        let mut colors = Colors {
            foreground: rgb_of(p.terminal_foreground),
            background: rgb_of(p.terminal_background),
            cursor: Some(rgb_of(p.terminal_cursor)),
            palette: p.terminal_ansi.iter().enumerate().map(|(i, c)| (i as u8, rgb_of(*c))).collect(),
        };
        if let Some(c) = file.get("foreground").and_then(rgb_field) {
            colors.foreground = c;
        }
        if let Some(c) = file.get("background").and_then(rgb_field) {
            colors.background = c;
        }
        if let Some(c) = file.get("cursorColor").and_then(rgb_field) {
            colors.cursor = Some(c);
        }
        if let Some(slots) = file.get("palette").and_then(Value::as_array) {
            for (i, slot) in slots.iter().enumerate().take(256) {
                if let Some(c) = rgb_field(slot) {
                    colors.palette.retain(|(j, _)| *j as usize != i);
                    colors.palette.push((i as u8, c));
                }
            }
        }
        let selection = file.get("selectionBackground").and_then(rgb_field).map(hsla_of).unwrap_or(p.terminal_selection);

        let base = if choices.size_from_file { file.get("fontSize").and_then(Value::as_f64).map(|s| s as f32) } else { None };
        let font_size = font_size(base.unwrap_or(BASE_SIZE), choices.zoom);

        let have = installed(cx);
        let typed = choices.family.as_deref().map(str::trim).filter(|f| !f.is_empty());
        let mut chain: Vec<String> = Vec::new();
        match typed {
            Some(f) => chain.push(f.to_owned()),
            None => {
                if let Some(families) = file.get("fontFamily").and_then(Value::as_array) {
                    chain.extend(families.iter().filter_map(|f| f.as_str()).map(str::to_owned));
                }
                if chain.is_empty() {
                    chain.extend(choices.detected.clone());
                }
            }
        }
        chain.retain(|f| have.iter().any(|h| h == f) && monospace(f, cx));
        let mac = cfg!(target_os = "macos");
        let default = crate::ui::theme::first_installed(&DEFAULT_FACES, &have, mac)
            .unwrap_or_else(|| gpui_kit::component::ActiveTheme::theme(cx).mono_font_family.clone());
        let mut faces: Vec<SharedString> = chain.into_iter().map(SharedString::from).collect();
        faces.push(default);
        for face in DEFAULT_FACES {
            if have.iter().any(|h| h == face) && !faces.iter().any(|f| f == face) {
                faces.push(face.into());
            }
        }
        let font_family = faces.remove(0);

        let pad =
            |axis: &str, side: &str| file.get(axis).and_then(|a| a.get(side)).and_then(Value::as_f64).map(|v| v as f32).unwrap_or(PADDING);
        let opacity = if choices.glass {
            file.get("backgroundOpacity").and_then(Value::as_f64).map(|o| o.clamp(0., 1.) as f32).unwrap_or(1.)
        } else {
            1.
        };
        let cursor_style = match file.get("cursorStyle").and_then(Value::as_str) {
            Some("bar") => Some(CursorStyle::Bar),
            Some("block") => Some(CursorStyle::Block),
            Some("underline") => Some(CursorStyle::Underline),
            Some("block_hollow") => Some(CursorStyle::BlockHollow),
            _ => None,
        };
        Look {
            font_family,
            fallbacks: faces,
            font_size,
            colors,
            selection,
            padding: Padding {
                left: pad("windowPaddingX", "left"),
                right: pad("windowPaddingX", "right"),
                top: pad("windowPaddingY", "top"),
                bottom: pad("windowPaddingY", "bottom"),
            },
            opacity,
            cursor_style,
            cursor_blink: file.get("cursorStyleBlink").and_then(Value::as_bool) != Some(false),
            scrollbar_thumb: p.scrollbar_thumb,
            scrollbar_thumb_hover: p.scrollbar_thumb_hover,
        }
    }

    /// The same look with the theme's colours read again, as a theme flip changes colours only.
    pub fn recolored(&self, config: Option<&protocol::TerminalConfig>, cx: &App) -> Look {
        let fresh = Look::with(config, &Choices::default(), cx);
        Look {
            colors: fresh.colors,
            selection: fresh.selection,
            scrollbar_thumb: fresh.scrollbar_thumb,
            scrollbar_thumb_hover: fresh.scrollbar_thumb_hover,
            ..self.clone()
        }
    }

    pub fn font(&self, bold: bool, italic: bool) -> Font {
        Font {
            family: self.font_family.clone(),
            features: Default::default(),
            fallbacks: (!self.fallbacks.is_empty())
                .then(|| FontFallbacks::from_fonts(self.fallbacks.iter().map(|f| f.to_string()).collect())),
            weight: if bold { FontWeight::BOLD } else { FontWeight::NORMAL },
            style: if italic { FontStyle::Italic } else { FontStyle::Normal },
        }
    }
}

/// `host.terminalConfig` for the side the app shows, which picks the side of a `light:X,dark:Y` theme.
pub fn read_config(host: &Host, dark: bool) -> impl Future<Output = Result<protocol::TerminalConfig, Failure>> + 'static {
    host.run(move |client| async move {
        let ask: protocol::HostTerminalConfigRequest =
            serde_json::from_value(serde_json::json!({ "scheme": if dark { "dark" } else { "light" } }))
                .map_err(|e| Failure::new(e.to_string()))?;
        client.call(ask).await.map(|reply| reply.config)
    })
}
