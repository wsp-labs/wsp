// SPDX-License-Identifier: AGPL-3.0-only
//! The View menu's zoom (`desktop:zoom.ts`, Electron's zoomIn, zoomOut and resetZoom roles): half a Chromium zoom level
//! a step, each level 1.2 times the last, for the run and every window, on GPUI's rem size. A focused terminal keeps
//! the chords for its own text size, as the shell stands aside for it there.

use gpui_kit::{actions, App, Global, KeyBinding};

actions!(app, [ActualSize, ZoomIn, ZoomOut]);

/// Electron's menu roles move `webContents.zoomLevel` by this much.
pub const STEP: f32 = 0.5;
/// Chromium's zoom factor for a level is this to the power of the level.
const RATIO: f32 = 1.2;
/// The steps whose factors stay inside Chromium's 25% to 500%.
pub const MIN_LEVEL: f32 = -7.5;
pub const MAX_LEVEL: f32 = 8.5;

/// The zoom level the run is on, 0 being actual size.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Zoom(pub f32);

impl Global for Zoom {}

impl Zoom {
    pub fn factor(self) -> f32 {
        RATIO.powf(self.0)
    }

    pub fn zoom_in(self) -> Zoom {
        Zoom((self.0 + STEP).min(MAX_LEVEL))
    }

    pub fn zoom_out(self) -> Zoom {
        Zoom((self.0 - STEP).max(MIN_LEVEL))
    }
}

/// The level the run is on now.
pub fn level(cx: &App) -> Zoom {
    cx.try_global::<Zoom>().copied().unwrap_or_default()
}

/// The menu's accelerators, `CmdOrCtrl+0`, `CmdOrCtrl+Plus` (and the `=` it shares a key with) and `CmdOrCtrl+-`,
/// on every window but a focused terminal, whose own zoom bindings then answer.
pub fn bindings(mac: bool) -> Vec<KeyBinding> {
    let m = if mac { "cmd" } else { "ctrl" };
    let at = Some("Root && !Terminal");
    vec![
        KeyBinding::new(&format!("{m}-0"), ActualSize, at),
        KeyBinding::new(&format!("{m}-+"), ZoomIn, at),
        KeyBinding::new(&format!("{m}-="), ZoomIn, at),
        KeyBinding::new(&format!("{m}-shift-="), ZoomIn, at),
        KeyBinding::new(&format!("{m}--"), ZoomOut, at),
    ]
}

/// Binds the chords and answers the three actions; the window frame sets each window's rem from [`level`] as it draws.
pub fn init(mac: bool, cx: &mut App) {
    cx.bind_keys(bindings(mac));
    cx.on_action(|_: &ActualSize, cx| set(Zoom::default(), cx));
    cx.on_action(|_: &ZoomIn, cx| set(level(cx).zoom_in(), cx));
    cx.on_action(|_: &ZoomOut, cx| set(level(cx).zoom_out(), cx));
}

fn set(zoom: Zoom, cx: &mut App) {
    cx.set_global(zoom);
    cx.refresh_windows();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_step_is_half_a_chromium_level() {
        let once = Zoom::default().zoom_in();
        assert_eq!(once, Zoom(0.5));
        assert!((once.factor() - 1.2f32.sqrt()).abs() < 1e-6, "{}", once.factor());
        assert!((Zoom(1.).factor() - 1.2).abs() < 1e-6);
        assert_eq!(once.zoom_out(), Zoom::default());
        assert_eq!(Zoom::default().factor(), 1.);
    }

    #[test]
    fn zoom_stops_inside_chromiums_limits() {
        let mut zoom = Zoom::default();
        for _ in 0..40 {
            zoom = zoom.zoom_in();
        }
        assert_eq!(zoom, Zoom(MAX_LEVEL));
        assert!(zoom.factor() <= 5. && zoom.zoom_in().factor() <= 5.);
        for _ in 0..80 {
            zoom = zoom.zoom_out();
        }
        assert_eq!(zoom, Zoom(MIN_LEVEL));
        assert!(zoom.factor() >= 0.25, "{}", zoom.factor());
        assert!(Zoom(MIN_LEVEL - STEP).factor() < 0.25 && Zoom(MAX_LEVEL + STEP).factor() > 5.);
    }

    #[test]
    fn the_chords_read_and_stand_aside_for_a_terminal() {
        for mac in [true, false] {
            let keys = bindings(mac);
            assert_eq!(keys.len(), 5);
            for key in &keys {
                assert_eq!(key.predicate().map(|p| p.to_string()).as_deref(), Some("Root && !Terminal"));
            }
        }
    }
}
