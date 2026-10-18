// SPDX-License-Identifier: AGPL-3.0-only
//! wsp's shared pieces, ported from the Settings pages' grammar (`apps/web/src/settings/layout.ts` and the files it
//! names) and the locked design in `.claude/skills/wsp-design`. Every screen composes these; none draws its own card,
//! row or button.

pub mod act;
pub mod card;
pub mod catalog;
pub mod chart;
pub mod computer_glyph;
pub mod context_menu;
pub mod controls;
pub mod dialog;
pub mod focus;
pub mod grain;
pub mod grid;
pub mod icons;
pub mod menu;
pub mod motion;
pub mod segmented;
pub mod select;
pub mod sparkline;
pub mod status;
pub mod stepper;
pub mod theme;
pub mod toggle;

use gpui_kit::{px, App, Pixels};

pub use act::{act_button, Act, Ask, Asks, Run};
pub use card::{
    chips, framed, glyph, glyph_frame, is_paste, section_head, word, Card, CardItem, Chip, HeadRow, KeyCaps, Line, Row, Taken, WordClass,
    Words, RESET_WORD,
};
pub use chart::{Axis, Chart, ChartLine, Tick, CHART_HEIGHT};
pub use computer_glyph::ComputerIcon;
pub use controls::{Button, ButtonKind, Choice, Field, PickRow};
pub use dialog::{quiet, shortcut_tooltip, shortcut_tooltip_view, tip, AlertDialog, CopyRow, Dialog, Refusal};
pub use grain::grain;
pub use grid::{chevron, num, slash, Col, Grid, GridHead, GridName, GridRow, HeadCell, StateCell, VersionFact};
pub use icons::{Glyph, UiAssets, LOCKUP, MARK};
pub use menu::{icon_menu, menu_of, row_menu, MenuAct};
pub use segmented::Segmented;
pub use select::{Select, SelectEvent, SelectItem, SelectState};
pub use sparkline::Sparkline;
pub use status::{spinner, step_mark, Crab, MarkState, RowSkeleton, Skeleton, Status, StatusMark, Step, StepRow, Still};
pub use stepper::Stepper;
pub use theme::{ActivePalette, Palette, Side};
pub use toggle::Switch;

/// Every size below is `layout.ts`'s, or the Tailwind class a piece in its list writes, in pixels.
pub mod size {
    use super::*;

    /// `CARD_INSET`: the sides every card keeps its content at.
    pub const CARD_INSET: Pixels = px(20.);
    /// `ROW_FLOOR`, `min-h-15`.
    pub const ROW_FLOOR: Pixels = px(60.);
    /// `LINE_FLOOR`, `min-h-12`.
    pub const LINE_FLOOR: Pixels = px(48.);
    /// A row's and a line's `py-3`.
    pub const ROW_PAD_Y: Pixels = px(12.);
    /// `CARD_SURFACE`'s `rounded-[11px]`.
    pub const CARD_RADIUS: Pixels = px(11.);
    /// `GLYPH_FRAME`'s `size-8` and `rounded-md`.
    pub const GLYPH_FRAME: Pixels = px(32.);
    pub const GLYPH_FRAME_RADIUS: Pixels = px(8.);
    /// `GLYPH`'s `size-4`.
    pub const GLYPH: Pixels = px(16.);
    /// `SECTION_HEAD`'s `min-h-7`.
    pub const SECTION_HEAD_FLOOR: Pixels = px(28.);
    /// `SELECT_WIDTH`: `w-44`, `h-[30px]`, `rounded-[7px]`.
    pub const SELECT_WIDTH: Pixels = px(176.);
    pub const CONTROL_HEIGHT: Pixels = px(30.);
    pub const CONTROL_RADIUS: Pixels = px(7.);
    /// `PAGE_GAP`.
    pub const PAGE_GAP: Pixels = px(30.);
    /// `STEP_WIDTH`, and the dialog popup's `rounded-xl` from `--radius`.
    pub const DIALOG_WIDTH: Pixels = px(560.);
    pub const DIALOG_RADIUS: Pixels = px(14.);
    /// The inset a dialog's cards keep (`STEP_FRAME`'s `--settings-inset:16px`).
    pub const DIALOG_INSET: Pixels = px(16.);
    /// `--control-radius` on buttons, and the copy row's `rounded-md`: both 8 px.
    pub const BUTTON_RADIUS: Pixels = px(8.);
}

/// The type scale as the pieces use it: size and line height in pixels.
pub mod text {
    use super::*;
    use gpui_kit::{font, SharedString, TextRun, Window};

    pub const SETTING_TITLE: (Pixels, Pixels) = (px(14.), px(20.));
    pub const LIST_TITLE: (Pixels, Pixels) = (px(15.), px(24.));
    pub const NOTE: (Pixels, Pixels) = (px(13.), px(20.));
    /// A row's description, `text-[13px] leading-[1.45]`.
    pub const DESCRIPTION: (Pixels, Pixels) = (px(13.), px(18.85));
    /// `text-[13.5px]` and `text-[13px]` keep the 1.5 line height Tailwind's base gives the page.
    pub const SECTION_HEAD: (Pixels, Pixels) = (px(13.5), px(20.25));
    pub const FACT: (Pixels, Pixels) = (px(13.), px(19.5));
    pub const VALUE: (Pixels, Pixels) = (px(14.), px(20.));
    pub const BUTTON: Pixels = px(13.);
    pub const XS: (Pixels, Pixels) = (px(12.), px(16.));
    pub const DIALOG_TITLE: (Pixels, Pixels) = (px(15.), px(20.));

    /// A line's width to the fraction, as the web lays text out; a text box rounds it up, and a row of them drifts.
    pub fn width(text: &str, size: f32, family: SharedString, window: &Window) -> Pixels {
        let run = TextRun {
            len: text.len(),
            font: font(family),
            color: gpui_kit::black(),
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        window.text_system().shape_line(SharedString::from(text.to_owned()), px(size), &[run], None).width
    }
}

/// Installs wsp's themes and font stacks; call once after `gpui_kit::init`. Open the app with `UiAssets` for the glyphs.
pub fn init(side: Side, cx: &mut App) {
    theme::init(side, cx);
}
