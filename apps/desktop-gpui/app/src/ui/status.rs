// SPDX-License-Identifier: AGPL-3.0-only
//! State as an icon in the slot every row keeps: the thread's status mark, a step's mark, the plain spinner, and the
//! crab, the one thing that moves at rest.
//!
//! The crab is the Whimsy Loaders pixel crab by Sasha (https://www.whimsically.app/loaders), transcribed from
//! `apps/web/src/components/status/Crab.tsx`; THIRD_PARTY_NOTICES, item 10, carries its credit.

use std::sync::OnceLock;
use std::time::{Duration, Instant};

use gpui_kit::component::{h_flex, Icon, Sizable as _};
use gpui_kit::{
    canvas, div, fill, linear_color_stop, linear_gradient, point, prelude::*, px, relative, size as gsize, Animation, AnimationExt as _,
    App, Bounds, FontWeight, Global, Hsla, Pixels, SharedString, Transformation, Window,
};

use super::card::{row_frame, CardItem, Corners};
use super::dialog::tip;
use super::icons::Glyph;
use super::theme::ActivePalette;
use super::{size, text};

/// Whether the pieces that move hold their first frame, as the web's crab does under reduced motion; captures set it.
#[derive(Clone, Copy, Default)]
pub struct Still(pub bool);

impl Global for Still {}

pub(super) fn still(cx: &App) -> bool {
    cx.reduce_motion() || cx.try_global::<Still>().is_some_and(|s| s.0)
}

/// The plain spinner: lucide's loader-circle at 14 px in the muted ink, one turn a second.
pub fn spinner(cx: &App) -> impl IntoElement {
    let icon = Icon::new(Glyph::LoaderCircle).with_size(px(14.)).text_color(cx.palette().muted_foreground);
    if still(cx) {
        return icon.into_any_element();
    }
    icon.with_animation("spinner", Animation::new(Duration::from_secs(1)).repeat(), |icon, delta| {
        icon.transform(Transformation::rotate(gpui_kit::percentage(delta)))
    })
    .into_any_element()
}

/// `Skeleton`: a bar in the hairline tier while what fills it is on its way, a highlight sweeping it every 2 s.
#[derive(IntoElement)]
pub struct Skeleton {
    width: Pixels,
    height: Pixels,
}

impl Skeleton {
    pub fn new(width: Pixels, height: Pixels) -> Self {
        Skeleton { width, height }
    }
}

impl RenderOnce for Skeleton {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.palette();
        let glow = p.skeleton_highlight;
        // GPUI's gradient takes two stops, so the 40-50-60% band is two halves; the web's 120 degree tilt is dropped.
        let half = |from: Hsla, to: Hsla| {
            div().absolute().top_0().bottom_0().w(relative(0.1)).bg(linear_gradient(
                90.,
                linear_color_stop(from, 0.),
                linear_color_stop(to, 1.),
            ))
        };
        let sweep = div()
            .absolute()
            .top_0()
            .bottom_0()
            .w_full()
            .child(half(glow.opacity(0.), glow).left(relative(0.4)))
            .child(half(glow, glow.opacity(0.)).left(relative(0.5)));
        div()
            .debug_selector(|| "skeleton".into())
            .relative()
            .flex_none()
            .overflow_hidden()
            .w(self.width)
            .h(self.height)
            .rounded(px(6.))
            .bg(p.border)
            .when(!still(cx), |el| {
                // The band crosses in the first 60% of the 2 s and is parked off the bar for the rest, as `skeleton` does.
                el.child(sweep.with_animation("skeleton", Animation::new(Duration::from_secs(2)).repeat(), |el, t| {
                    el.left(relative(-1. + 2. * (t / 0.6).min(1.)))
                }))
            })
    }
}

/// `RowSkeleton`: one row's room while the card's rows are on their way, a title bar over a line bar on its own card
/// surface; a card's `body`.
#[derive(IntoElement)]
pub struct RowSkeleton {
    id: SharedString,
}

impl RowSkeleton {
    pub fn new(id: impl Into<SharedString>) -> Self {
        RowSkeleton { id: id.into() }
    }
}

impl RenderOnce for RowSkeleton {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.palette();
        div()
            .id(self.id)
            .debug_selector(|| "row-skeleton".into())
            .flex()
            .flex_col()
            .justify_center()
            .gap_2()
            .py(size::ROW_PAD_Y)
            .px(size::CARD_INSET)
            .min_h(size::ROW_FLOOR)
            .rounded(size::CARD_RADIUS)
            .border_1()
            .border_color(p.border.opacity(0.6))
            .bg(p.card.opacity(0.4))
            .child(Skeleton::new(px(160.), px(14.)))
            .child(Skeleton::new(px(256.), px(12.)))
    }
}

/// A thread's state in its row's status slot. The glyph stands alone; its word belongs on the hover.
#[derive(Clone, PartialEq)]
pub enum Status {
    NeedsYou,
    /// Working, with the time it has run: `0s`, `59m`, `1h 5m`.
    Working(SharedString),
    Failed,
    Done,
    Waiting,
    /// Read, with its age: `14m`, `3h`, `2d`.
    Read(SharedString),
}

/// The status mark in a one-line row: 12 px, the time before the glyph or the crab, weight 500 where it is toned.
#[derive(IntoElement)]
pub struct StatusMark {
    status: Status,
    tooltip: Option<SharedString>,
}

impl StatusMark {
    pub fn new(status: Status) -> Self {
        StatusMark { status, tooltip: None }
    }

    /// The state's word on the hover: "Needs you", "Working for 1h 5m".
    pub fn tooltip(mut self, text: impl Into<SharedString>) -> Self {
        self.tooltip = Some(text.into());
        self
    }
}

impl RenderOnce for StatusMark {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.palette();
        let (glyph, time, ink, toned) = match &self.status {
            Status::NeedsYou => (Some(Glyph::MessageCircleQuestionMark), None, p.status_input, true),
            Status::Working(elapsed) => (None, Some(elapsed.clone()), p.status_working, true),
            Status::Failed => (Some(Glyph::CircleAlert), None, p.status_failed, true),
            Status::Done => (Some(Glyph::CircleCheck), None, p.status_done, true),
            Status::Waiting => (Some(Glyph::Hourglass), None, p.muted_foreground, false),
            Status::Read(age) => (None, Some(age.clone()), p.muted_foreground, false),
        };
        let working = matches!(self.status, Status::Working(_));
        let mark = h_flex()
            .debug_selector(|| "status-mark".into())
            .flex_none()
            .min_w(px(16.))
            .justify_end()
            .gap_1()
            .text_size(text::XS.0)
            .line_height(text::XS.1)
            .font_weight(if toned { FontWeight::MEDIUM } else { FontWeight::NORMAL })
            .text_color(ink)
            .children(time)
            .children(glyph.map(|g| Icon::new(g).with_size(px(12.)).text_color(ink)))
            .when(working, |el| el.child(Crab::new(ink)));
        match self.tooltip {
            Some(text) => mark.id(SharedString::from(format!("status-mark-{text}"))).tooltip(tip(text)).into_any_element(),
            None => mark.into_any_element(),
        }
    }
}

/// A step's state in a list of steps.
#[derive(Clone, Copy, PartialEq)]
pub enum Step {
    NotStarted,
    /// A setup step at work: the plain spinner, never the crab.
    Running,
    Skipped,
    Done,
    Failed,
}

/// The step's mark in its 16 px box.
pub fn step_mark(step: Step, cx: &App) -> impl IntoElement {
    let p = cx.palette();
    let glyph = |g: Glyph, ink: Hsla| Icon::new(g).with_size(px(14.)).text_color(ink).into_any_element();
    let mark = match step {
        Step::NotStarted => glyph(Glyph::Circle, p.muted_foreground.opacity(0.6)),
        Step::Running => spinner(cx).into_any_element(),
        Step::Skipped => glyph(Glyph::CircleMinus, p.muted_foreground),
        Step::Done => glyph(Glyph::CircleCheck, p.status_done),
        Step::Failed => glyph(Glyph::CircleAlert, p.status_failed),
    };
    div().debug_selector(|| "step-mark".into()).flex().flex_none().items_center().justify_center().size(px(16.)).child(mark)
}

/// One step in a list of steps: its mark, then its title, muted until it starts.
#[derive(IntoElement)]
pub struct StepRow {
    id: SharedString,
    step: Step,
    title: SharedString,
    tooltip: Option<SharedString>,
    corners: Corners,
}

impl StepRow {
    pub fn new(id: impl Into<SharedString>, step: Step, title: impl Into<SharedString>) -> Self {
        StepRow { id: id.into(), step, title: title.into(), tooltip: None, corners: Corners::default() }
    }

    /// The step's state in words on its mark's hover: "Skipped: Claude Code is not installed there".
    pub fn tooltip(mut self, text: impl Into<SharedString>) -> Self {
        self.tooltip = Some(text.into());
        self
    }
}

impl From<StepRow> for CardItem {
    fn from(row: StepRow) -> Self {
        // A step's rule stands above its 48 px floor, not inside it, as `StepRow.tsx` draws it.
        CardItem::placed(move |corners| {
            let rule = corners.rule;
            div()
                .when_some(rule, |el, rule| el.border_t_1().border_color(rule))
                .child(StepRow { corners: Corners { rule: None, ..corners }, ..row })
                .into_any_element()
        })
    }
}

impl RenderOnce for StepRow {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.palette();
        let waiting = self.step == Step::NotStarted;
        row_frame(&self.id, size::LINE_FLOOR, self.corners).child(
            h_flex()
                .gap_3()
                .child(
                    div()
                        .id(SharedString::from(format!("{}-mark", self.id)))
                        .when_some(self.tooltip, |el, t| el.tooltip(tip(t)))
                        .child(step_mark(self.step, cx)),
                )
                .child(
                    div()
                        .debug_selector(|| "title".into())
                        .min_w_0()
                        .text_size(text::SETTING_TITLE.0)
                        .line_height(text::SETTING_TITLE.1)
                        .font_weight(if waiting { FontWeight::NORMAL } else { FontWeight::MEDIUM })
                        .text_color(if waiting { p.muted_foreground } else { p.foreground })
                        .child(self.title),
                ),
        )
    }
}

/// The crab's box, 16 by 14 css px, one crab pixel to one px.
const CRAB: (f32, f32) = (16., 14.);

const BODY: [(i8, i8); 41] = [
    (-4, 0),
    (-3, 0),
    (-2, 0),
    (-1, 0),
    (0, 0),
    (1, 0),
    (2, 0),
    (3, 0),
    (4, 0),
    (-4, -1),
    (-3, -1),
    (-2, -1),
    (-1, -1),
    (0, -1),
    (1, -1),
    (2, -1),
    (3, -1),
    (4, -1),
    (-4, 1),
    (-3, 1),
    (-2, 1),
    (-1, 1),
    (0, 1),
    (1, 1),
    (2, 1),
    (3, 1),
    (4, 1),
    (-3, -2),
    (-2, -2),
    (-1, -2),
    (0, -2),
    (1, -2),
    (2, -2),
    (3, -2),
    (-3, 2),
    (-2, 2),
    (-1, 2),
    (0, 2),
    (1, 2),
    (2, 2),
    (3, 2),
];
const LEFT_CLAW: [(i8, i8); 6] = [(-5, -2), (-6, -2), (-6, -1), (-5, -1), (-6, -3), (-7, -2)];
const RIGHT_CLAW: [(i8, i8); 6] = [(5, -2), (6, -2), (6, -1), (5, -1), (6, -3), (7, -2)];
const LEGS: [(f32, f32); 8] = [(-4., 0.), (-3., 0.5), (-2., 1.), (-1., 1.5), (1., 0.), (2., 0.5), (3., 1.), (4., 1.5)];

/// JavaScript's `Math.round`, which takes a half up, where Rust's `round` takes it away from zero.
fn js_round(v: f32) -> f32 {
    (v + 0.5).floor()
}

/// One frame of the crab at `t` ms, as rectangles in px from the box's corner with their alpha, painted in order so
/// that overlapping pixels stack as the canvas stacks them.
pub fn crab_frame(t: f64) -> Vec<(f32, f32, f32, f32, f32)> {
    let mut out = Vec::new();
    let (cx, cy) = (js_round(CRAB.0 / 2.), js_round(CRAB.1 / 2.));
    let c = |out: &mut Vec<_>, x: f32, y: f32, a: f32| out.push((js_round(cx + x), js_round(cy + y), 1., 1., a));
    BODY.iter().for_each(|&(x, y)| c(&mut out, x.into(), y.into(), 0.62));
    c(&mut out, -1., -3., 0.72);
    c(&mut out, 1., -3., 0.72);
    c(&mut out, -1., -2., 0.58);
    c(&mut out, 1., -2., 0.58);
    if (t / 2800.).floor() as i64 % 6 == 0 && t % 2800. < 160. {
        out.push((js_round(cx - 1.5), js_round(cy - 3.), 2., 1., 0.5));
    } else {
        c(&mut out, -1., -3., 0.92);
        c(&mut out, 1., -3., 0.92);
    }
    let bob = js_round((1.5 * (0.0018 * t).sin()) as f32);
    LEFT_CLAW.iter().for_each(|&(x, y)| c(&mut out, x.into(), y.into(), 0.65));
    c(&mut out, -7., -1. + bob, 0.55);
    c(&mut out, -7., -3. - bob, 0.45);
    RIGHT_CLAW.iter().for_each(|&(x, y)| c(&mut out, x.into(), y.into(), 0.65));
    c(&mut out, 7., -1. + bob, 0.55);
    c(&mut out, 7., -3. - bob, 0.45);
    [(-5., 0.), (-4., -1.), (5., 0.), (4., -1.)].into_iter().for_each(|(x, y)| c(&mut out, x, y, 0.58));
    let u = 0.003 * t;
    for (bx, ph) in LEGS {
        let phase = u + f64::from(ph) * std::f64::consts::PI;
        let r = phase.sin().abs() as f32;
        let o = if bx < 0. { -1. } else { 1. };
        let nx = bx + o * js_round(1. + r);
        let ny = 2. + js_round((1.2 * phase.sin()) as f32);
        c(&mut out, bx, 2., 0.52);
        c(&mut out, nx, ny, 0.42 + 0.2 * r);
        if r > 0.5 {
            c(&mut out, nx + o, ny + 1., 0.28);
        }
    }
    out
}

/// When the first crab was drawn, so every crab walks in step.
fn started() -> Instant {
    static STARTED: OnceLock<Instant> = OnceLock::new();
    *STARTED.get_or_init(Instant::now)
}

/// The crab, in the ink it is given: an agent working. Never a loading spinner.
#[derive(IntoElement)]
pub struct Crab {
    ink: Hsla,
}

impl Crab {
    pub fn new(ink: Hsla) -> Self {
        Crab { ink }
    }
}

impl RenderOnce for Crab {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        // Held still, the crab draws its frame at t = 0, as the web's does under reduced motion.
        let (ink, still) = (self.ink, still(cx));
        let pixels = canvas(
            |_, _, _| (),
            move |bounds: Bounds<Pixels>, _, window, _| {
                let t = if still { 0. } else { started().elapsed().as_secs_f64() * 1000. };
                for (x, y, w, h, a) in crab_frame(t) {
                    let origin = bounds.origin + point(px(x), px(y));
                    window.paint_quad(fill(Bounds::new(origin, gsize(px(w), px(h))), ink.opacity(a)));
                }
                if !still {
                    window.request_animation_frame();
                }
            },
        )
        .size_full();
        div().debug_selector(|| "crab".into()).flex_none().w(px(CRAB.0)).h(px(CRAB.1)).child(pixels)
    }
}

#[cfg(test)]
mod tests {
    use super::crab_frame;

    /// Composited alpha per pixel, two decimals, as the web's canvas stacks them: `.claude/skills/wsp-design`'s
    /// `crab.js` run at the same `t`.
    fn grid(t: f64) -> Vec<String> {
        let mut alpha = [[0f32; 16]; 14];
        for (x, y, w, h, a) in crab_frame(t) {
            for row in alpha.iter_mut().skip(y as usize).take(h as usize) {
                for cell in row.iter_mut().skip(x as usize).take(w as usize) {
                    *cell = a + *cell * (1. - a);
                }
            }
        }
        let cell = |a: f32| if a == 0. { " . ".to_owned() } else { format!("{a:.2}")[1..].to_owned() };
        alpha.iter().map(|row| row.iter().map(|&a| cell(a)).collect::<Vec<_>>().join(" ")).collect()
    }

    #[test]
    fn the_still_frame_is_the_web_crab_at_t_0_mid_blink() {
        let want = [
            " .  .45 .65  .   .   .   .  .86 .50 .72  .   .   .   .  .65 .45",
            " .  .65 .65 .65  .  .62 .62 .84 .62 .84 .62 .62  .  .65 .65 .65",
            " .  .55 .65 .65 .84 .62 .62 .62 .62 .62 .62 .62 .84 .65 .65 .55",
            " .   .   .  .58 .62 .62 .62 .62 .62 .62 .62 .62 .62 .58  .   . ",
            " .   .   .   .  .62 .86 .62 .62 .62 .62 .62 .62 .62  .  .62  . ",
            " .   .   .  .42 .65 .89 .82 .82 .62 .82 .89 .82 .72  .   .  .28",
            " .   .   .  .62  .   .   .   .   .   .   .   .  .62  .   .   . ",
            " .   .  .28  .   .   .   .   .   .   .   .   .   .  .28  .   . ",
        ];
        assert_eq!(grid(0.)[4..12], want);
    }

    #[test]
    fn a_walking_frame_is_the_web_crab_at_the_same_time() {
        let want = [
            " .  .45  .   .   .   .   .   .   .   .   .   .   .   .   .  .45",
            " .   .  .65  .   .   .   .  .98  .  .98  .   .   .   .  .65  . ",
            " .  .65 .65 .65  .  .62 .62 .84 .62 .84 .62 .62  .  .65 .65 .65",
            " .   .  .65 .65 .84 .62 .62 .62 .62 .62 .62 .62 .84 .65 .65  . ",
            " .  .55  .  .58 .62 .62 .62 .62 .62 .62 .62 .62 .62 .58  .  .55",
            " .   .  .53 .59 .62 .62 .62 .62 .62 .62 .62 .82 .84  .   .   . ",
            " .  .28 .28  .  .52 .82 .82 .82 .62 .82 .82 .82 .65 .28  .   . ",
            " .   .   .   .  .53 .59  .   .   .   .   .   .   .  .53 .59  . ",
            " .   .   .  .28 .28  .   .   .   .   .   .   .   .   .  .28 .28",
        ];
        assert_eq!(grid(1234.)[3..12], want);
    }
}
