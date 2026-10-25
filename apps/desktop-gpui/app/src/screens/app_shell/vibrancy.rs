// SPDX-License-Identifier: AGPL-3.0-only
//! The Mac window's own glass, as the Electron window sets it (`apps/desktop/src/window.ts`): macOS's sidebar material
//! behind the whole window, following the window's active state, while the page draws glass. GPUI's own Blurred
//! appearance draws a different material with its tint and saturation taken out, so the view is added here instead.

use gpui_kit::component::Root;
use gpui_kit::{AnyView, Context, Styled as _, Window, WindowBackgroundAppearance};

/// The root of a window the shell fills: gpui-kit's Root paints the theme's ground over the whole window, which would
/// hide the glass, so it paints none and the shell lays the window's ground itself while it draws no glass. It draws the
/// shell cached, so a frame that only draws live pieces again in place leaves the shell alone.
pub fn clear_root(view: impl Into<AnyView>, window: &mut Window, cx: &mut Context<Root>) -> Root {
    Root::new(view, window, cx).cache_view().bg(gpui_kit::transparent_black())
}

/// The window's own background: clear while macOS's material stands behind it, opaque otherwise.
pub fn background_appearance(material: bool) -> WindowBackgroundAppearance {
    if material {
        WindowBackgroundAppearance::Transparent
    } else {
        WindowBackgroundAppearance::Opaque
    }
}

/// The material view behind the window's content, while one is there, and whether the frame last asked for it.
#[derive(Default)]
pub struct Vibrancy {
    asked: bool,
    #[cfg(target_os = "macos")]
    view: Option<mac::Id>,
}

impl Vibrancy {
    /// Puts the material behind the window, or takes it away and draws the window opaque again. A material that
    /// cannot be added leaves the window opaque, and is not tried again until the frame asks anew.
    pub fn set(&mut self, on: bool, window: &mut Window) {
        if on == self.asked {
            return;
        }
        self.asked = on;
        #[cfg(target_os = "macos")]
        {
            use raw_window_handle::{HasWindowHandle, RawWindowHandle};
            if let Some(view) = self.view.take() {
                // SAFETY: the view was added by `mac::attach` and is still retained by its superview.
                unsafe { mac::detach(view) };
            }
            if on {
                if let Ok(handle) = HasWindowHandle::window_handle(window) {
                    if let RawWindowHandle::AppKit(handle) = handle.as_raw() {
                        // SAFETY: the handle's view is GPUI's own view, live inside the window's content view, on the main thread.
                        self.view = unsafe { mac::attach(handle.ns_view.as_ptr()) };
                    }
                }
            }
            window.set_background_appearance(background_appearance(self.view.is_some()));
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = window;
        }
    }
}

/// Whether the computer asks for no transparency (Reduce transparency in Accessibility), which the web app reads too.
pub fn reduce_transparency() -> bool {
    #[cfg(target_os = "macos")]
    {
        // SAFETY: a read of a shared AppKit object's flag on the main thread.
        unsafe { mac::reduce_transparency() }
    }
    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

#[cfg(target_os = "macos")]
mod mac {
    use std::ffi::c_void;

    use cocoa::appkit::{
        NSView, NSViewHeightSizable, NSViewWidthSizable, NSVisualEffectBlendingMode, NSVisualEffectMaterial, NSVisualEffectState,
        NSVisualEffectView, NSWindowOrderingMode,
    };
    use cocoa::base::{id, nil, BOOL, NO};
    use objc::{class, msg_send, sel, sel_impl};

    pub type Id = id;

    /// Adds a sidebar material view under everything in `ns_view`'s window, the way GPUI adds its own blur view.
    pub unsafe fn attach(ns_view: *mut c_void) -> Option<id> {
        unsafe {
            let ns_view = ns_view as id;
            let window: id = msg_send![ns_view, window];
            if window == nil {
                return None;
            }
            let content: id = msg_send![window, contentView];
            let mut view: id = msg_send![class!(NSVisualEffectView), alloc];
            view = NSView::initWithFrame_(view, NSView::bounds(content));
            NSVisualEffectView::setMaterial_(view, NSVisualEffectMaterial::Sidebar);
            NSVisualEffectView::setBlendingMode_(view, NSVisualEffectBlendingMode::BehindWindow);
            NSVisualEffectView::setState_(view, NSVisualEffectState::FollowsWindowActiveState);
            view.setAutoresizingMask_(NSViewWidthSizable | NSViewHeightSizable);
            let _: () = msg_send![content, addSubview: view positioned: NSWindowOrderingMode::NSWindowBelow relativeTo: nil];
            let _: () = msg_send![view, release];
            Some(view)
        }
    }

    pub unsafe fn detach(view: id) {
        unsafe { NSView::removeFromSuperview(view) }
    }

    pub unsafe fn reduce_transparency() -> bool {
        unsafe {
            let workspace: id = msg_send![class!(NSWorkspace), sharedWorkspace];
            let reduce: BOOL = msg_send![workspace, accessibilityDisplayShouldReduceTransparency];
            reduce != NO
        }
    }
}

#[cfg(test)]
mod tests {
    use gpui_kit::WindowBackgroundAppearance;

    use super::background_appearance;

    #[test]
    fn the_window_is_clear_only_over_the_material() {
        assert_eq!(background_appearance(true), WindowBackgroundAppearance::Transparent);
        assert_eq!(background_appearance(false), WindowBackgroundAppearance::Opaque);
    }
}
