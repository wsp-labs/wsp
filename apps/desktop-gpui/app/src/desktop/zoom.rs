// SPDX-License-Identifier: AGPL-3.0-only
//! The View menu's zoom (`desktop:zoom.ts`, Electron's zoomIn, zoomOut and resetZoom roles): half a Chromium zoom level
//! a step, each level 1.2 times the last, for every window as Chromium's page zoom scales the whole page, and kept
//! across launches as Chromium keeps a page's level in its profile. A focused terminal keeps the chords for its own
//! text size, as the shell stands aside for it there.

use gpui_kit::{actions, App, Global, KeyBinding};
use serde_json::Value;

use crate::local_store::LocalStore;
use crate::screens::app_shell::menu::Os;

actions!(app, [ActualSize, ZoomIn, ZoomOut]);

/// Electron's menu roles move `webContents.zoomLevel` by this much.
pub const STEP: f32 = 0.5;
/// Chromium's zoom factor for a level is this to the power of the level.
const RATIO: f32 = 1.2;
/// The steps whose factors stay inside Chromium's 25% to 500%.
pub const MIN_LEVEL: f32 = -7.5;
pub const MAX_LEVEL: f32 = 8.5;
/// The app store's key for the level the last run left.
pub const KEY: &str = "wsp:zoom-level";

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
/// on every window but a focused terminal, whose own zoom bindings then answer. Electron sets no menu on Linux, so
/// there the chords are a focused terminal's alone.
pub fn bindings(os: Os) -> Vec<KeyBinding> {
    let m = match os {
        Os::Mac => "cmd",
        Os::Windows => "ctrl",
        Os::Linux => return Vec::new(),
    };
    let at = Some("Root && !Terminal");
    vec![
        KeyBinding::new(&format!("{m}-0"), ActualSize, at),
        KeyBinding::new(&format!("{m}-+"), ZoomIn, at),
        KeyBinding::new(&format!("{m}-="), ZoomIn, at),
        KeyBinding::new(&format!("{m}-shift-="), ZoomIn, at),
        KeyBinding::new(&format!("{m}--"), ZoomOut, at),
    ]
}

/// The level the app store kept, inside the limits; none when it kept nothing readable.
fn stored(cx: &App) -> Option<Zoom> {
    let level = cx.try_global::<LocalStore>()?.get_as::<f32>(KEY)?;
    Some(Zoom(level.clamp(MIN_LEVEL, MAX_LEVEL)))
}

/// Binds the chords, zooms every window to the level the store kept and answers the three actions, each kept in the
/// store. Runs before the first window opens, so it opens zoomed.
pub fn init(os: Os, cx: &mut App) {
    cx.bind_keys(bindings(os));
    apply(stored(cx).unwrap_or_default(), cx);
    cx.on_action(|_: &ActualSize, cx| set(Zoom::default(), cx));
    cx.on_action(|_: &ZoomIn, cx| set(level(cx).zoom_in(), cx));
    cx.on_action(|_: &ZoomOut, cx| set(level(cx).zoom_out(), cx));
}

fn apply(zoom: Zoom, cx: &mut App) {
    cx.set_global(zoom);
    cx.set_window_zoom(zoom.factor());
}

fn set(zoom: Zoom, cx: &mut App) {
    apply(zoom, cx);
    if cx.has_global::<LocalStore>() {
        if let Err(e) = cx.global_mut::<LocalStore>().set(KEY, Value::from(zoom.0)) {
            super::log::error(&format!("zoom not kept: {e}"));
        }
    }
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

    fn zoomed_in_twice(cx: &mut gpui_kit::TestAppContext, path: &std::path::Path) {
        cx.update(|cx| {
            crate::local_store::init(path, cx);
            init(Os::Linux, cx);
        });
        assert_eq!(cx.update(|cx| (level(cx), cx.window_zoom())), (Zoom(0.), 1.), "an empty store opens at actual size");
        cx.update(|cx| {
            cx.dispatch_action(&ZoomIn);
            cx.dispatch_action(&ZoomIn);
        });
        cx.run_until_parked();
    }

    #[gpui_kit::test]
    fn the_level_is_kept_and_the_next_launch_opens_at_it(cx: &mut gpui_kit::TestAppContext, next: &mut gpui_kit::TestAppContext) {
        let path = crate::local_store::scratch("zoom");
        zoomed_in_twice(cx, &path);
        assert_eq!(LocalStore::open(&path).get_as::<f32>(KEY), Some(1.));
        next.update(|cx| {
            crate::local_store::init(&path, cx);
            init(Os::Linux, cx);
        });
        let (zoom, factor) = next.update(|cx| (level(cx), cx.window_zoom()));
        assert_eq!(zoom, Zoom(1.));
        assert!((factor - 1.2).abs() < 1e-6, "{factor}");
        next.update(|cx| cx.dispatch_action(&ActualSize));
        next.run_until_parked();
        assert_eq!(LocalStore::open(&path).get_as::<f32>(KEY), Some(0.));
    }

    #[test]
    fn a_kept_level_past_the_limits_or_of_another_shape_reads_inside_them() {
        let path = crate::local_store::scratch("zoom-limits");
        let read = |value: Value| {
            let mut store = LocalStore::open(&path);
            store.set(KEY, value).unwrap();
            let cx = gpui_kit::TestAppContext::single();
            cx.update(|cx| {
                cx.set_global(LocalStore::open(&path));
                stored(cx)
            })
        };
        assert_eq!(read(Value::from(40.)), Some(Zoom(MAX_LEVEL)));
        assert_eq!(read(Value::from(-40.)), Some(Zoom(MIN_LEVEL)));
        assert_eq!(read(Value::from("big")), None);
    }

    #[test]
    fn the_chords_read_and_stand_aside_for_a_terminal() {
        assert!(bindings(Os::Linux).is_empty(), "Electron sets no menu on Linux, so the window takes no zoom chord");
        for os in [Os::Mac, Os::Windows] {
            let keys = bindings(os);
            assert_eq!(keys.len(), 5);
            let on_mod = |k: &KeyBinding| {
                let m = k.keystrokes()[0].inner().modifiers;
                if os == Os::Mac {
                    m.platform && !m.control
                } else {
                    m.control && !m.platform
                }
            };
            assert!(keys.iter().all(on_mod), "{os:?} zooms on Cmd on the Mac and Ctrl elsewhere");
            for key in &keys {
                assert_eq!(key.predicate().map(|p| p.to_string()).as_deref(), Some("Root && !Terminal"));
            }
        }
    }
}
