// SPDX-License-Identifier: AGPL-3.0-only
//! The window widths today's app lays itself out again at, Tailwind's `sm` and `md`: a media query reads the window's
//! viewport in CSS px, so these read the window's viewport in logical px. Every piece and page asks here.

use gpui_kit::{px, Pixels, Window};

/// `sm`, 40rem: under it `max-sm:` holds.
pub const SM: Pixels = px(640.);
/// `md`, 48rem: under it `max-md:` holds.
pub const MD: Pixels = px(768.);

/// Which of today's app's breakpoints the window stands under.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Width {
    /// Under 640 px: `max-sm:`.
    pub sm: bool,
    /// Under 768 px: `max-md:`, and Activity's `narrow:`.
    pub md: bool,
}

impl Width {
    pub fn at(width: Pixels) -> Width {
        Width { sm: width < SM, md: width < MD }
    }
}

/// The window's width as today's app's breakpoints read it.
pub fn width(window: &Window) -> Width {
    Width::at(window.viewport_size().width)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_breakpoint_holds_below_its_width_and_not_at_it() {
        assert_eq!(Width::at(px(600.)), Width { sm: true, md: true });
        assert_eq!(Width::at(px(640.)), Width { sm: false, md: true });
        assert_eq!(Width::at(px(760.)), Width { sm: false, md: true });
        assert_eq!(Width::at(px(768.)), Width { sm: false, md: false });
        assert_eq!(Width::at(px(1280.)), Width::default());
    }
}
