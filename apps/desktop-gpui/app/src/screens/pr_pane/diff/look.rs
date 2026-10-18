// SPDX-License-Identifier: AGPL-3.0-only
//! The code surface's colors, as `DIFF_SURFACE_THEME_UNSAFE_CSS` and `StyledDiffCodeView.tsx` map pierre's surfaces
//! onto the app's tokens, and the element that draws gpui-kit's `Diff` with gpui-kit's theme tokens set to them.

use gpui_kit::component::{ActiveTheme as _, Theme};
use gpui_kit::{
    px, AnyElement, App, Bounds, Element, ElementId, GlobalElementId, Hsla, InspectorElementId, IntoElement, LayoutId, Pixels, Rgba, Window,
};

use crate::ui::ActivePalette as _;

/// `color-mix(in srgb, a p%, b)`.
pub(super) fn mix(a: Hsla, b: Hsla, p: f32) -> Hsla {
    let (a, b): (Rgba, Rgba) = (a.to_rgb(), b.to_rgb());
    let m = |x: f32, y: f32| x * p + y * (1. - p);
    Rgba { r: m(a.r, b.r), g: m(a.g, b.g), b: m(a.b, b.b), a: m(a.a, b.a) }.into()
}

/// The colors the code surface reads, resolved for the active theme.
#[derive(Clone, Copy)]
pub(super) struct DiffColors {
    /// `--code-background`, which the code view sets to the page's background.
    pub background: Hsla,
    pub foreground: Hsla,
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
            added,
            deleted,
            modified,
            number: mix(p.foreground, background, 0.65),
            edge: p.foreground.opacity(0.24),
            selected,
            gap_ink: mix(p.foreground, background, 0.52),
            gap_rule: mix(background, p.foreground, 0.92),
        }
    }
}

/// The share of pierre's added or deleted color in a changed row's tint, read off today's rows in Paper and Graphite.
const TINT: f32 = 0.065;

/// gpui-kit's theme as the code surface draws: its tokens set to [`DiffColors`].
pub(super) fn code_theme(cx: &App) -> Theme {
    let c = DiffColors::of(cx);
    let mut theme = cx.theme().clone();
    theme.background = c.background;
    theme.foreground = c.foreground;
    theme.muted = c.background;
    theme.muted_foreground = c.number;
    // gpui-kit tints a changed row with its bar's color at 12%; this color gives pierre's tint at that share.
    theme.success = mix(c.added, c.background, TINT / 0.12);
    theme.danger = mix(c.deleted, c.background, TINT / 0.12);
    theme.border = c.background.opacity(0.);
    theme.info = c.modified;
    theme.selection = c.selected;
    theme.list_hover = mix(c.background, cx.palette().foreground, 0.94);
    // Pierre's 13 px code on 20 px rows: gpui-kit's rows are 1.6 times the code size.
    theme.mono_font_size = px(12.5);
    theme
}

/// Lays out, prepaints and paints `child` with `theme` as gpui-kit's global theme, so the `Diff` inside, its rows
/// rendered lazily during prepaint among them, reads the code surface's tokens.
pub(super) fn with_theme(theme: Theme, child: impl IntoElement) -> Themed {
    Themed { theme, child: child.into_any_element() }
}

pub(super) struct Themed {
    theme: Theme,
    child: AnyElement,
}

impl Themed {
    fn within<R>(&mut self, cx: &mut App, f: impl FnOnce(&mut AnyElement, &mut App) -> R) -> R {
        std::mem::swap(Theme::global_mut(cx), &mut self.theme);
        let out = f(&mut self.child, cx);
        std::mem::swap(Theme::global_mut(cx), &mut self.theme);
        out
    }
}

impl IntoElement for Themed {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl Element for Themed {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        (self.within(cx, |child, cx| child.request_layout(window, cx)), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.within(cx, |child, cx| child.prepaint(window, cx));
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.within(cx, |child, cx| child.paint(window, cx));
    }
}
