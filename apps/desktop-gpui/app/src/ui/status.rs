// SPDX-License-Identifier: AGPL-3.0-only
//! State as an icon in the slot every row keeps: the thread's status mark, a step's mark, the plain spinner, and the
//! crab, the one thing that moves at rest.
//!
//! The crab is the Whimsy Loaders pixel crab by Sasha (https://www.whimsically.app/loaders), transcribed from
//! `apps/web/src/components/status/Crab.tsx`; THIRD_PARTY_NOTICES, item 10, carries its credit.

use std::collections::HashMap;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use gpui_kit::component::{h_flex, Icon, Sizable as _};
use gpui_kit::{
    canvas, div, fill, linear_color_stop, linear_gradient, point, prelude::*, px, relative, size as gsize, AnyElement, App, Bounds,
    ClickEvent, Context, ElementId, EntityId, FontWeight, Global, HighlightStyle, Hsla, Pixels, SharedString, StyledText, Task,
    Transformation, Window,
};

use super::card::{CardItem, Corners};
use super::dialog::tip;
use super::focus::ring_inset;
use super::icons::Glyph;
use super::motion;
use super::theme::ActivePalette;
use super::{size, text};

/// Whether the pieces that move hold their first frame, as the web's crab does under reduced motion; captures set it.
#[derive(Clone, Copy, Default)]
pub struct Still(pub bool);

impl Global for Still {}

pub(crate) fn still(cx: &App) -> bool {
    cx.reduce_motion() || cx.try_global::<Still>().is_some_and(|s| s.0)
}

/// The most often a spinner or a skeleton asks for a frame: each frame draws again the whole view it stands in, and
/// every view above that one.
const LOADING_FRAME: Duration = Duration::from_millis(33);

/// When the first spinner or skeleton was drawn, so they all turn and sweep in step and ask for the same frames.
fn loading_started() -> Instant {
    static STARTED: OnceLock<Instant> = OnceLock::new();
    *STARTED.get_or_init(Instant::now)
}

/// The frame each view already waits for.
#[derive(Default)]
struct Wakes(HashMap<EntityId, Instant>);

impl Global for Wakes {}

/// Draws the view being drawn again after `wait`, unless a frame at or before then is already on its way to it.
fn wake_after(wait: Duration, window: &Window, cx: &mut App) {
    let view = window.current_view();
    let due = cx.background_executor().now() + wait;
    let wakes = cx.default_global::<Wakes>();
    if wakes.0.get(&view).is_some_and(|at| *at <= due) {
        return;
    }
    wakes.0.insert(view, due);
    cx.spawn(async move |cx| {
        cx.background_executor().timer(wait).await;
        cx.update(|cx| {
            let wakes = cx.default_global::<Wakes>();
            if wakes.0.get(&view) == Some(&due) {
                wakes.0.remove(&view);
            }
            cx.notify(view);
        });
    })
    .detach();
}

/// The plain spinner: lucide's loader-circle at 14 px in the muted ink, one turn a second.
pub fn spinner(_: &App) -> Spinner {
    Spinner(px(14.))
}

#[derive(IntoElement)]
pub struct Spinner(Pixels);

impl Spinner {
    /// Another size: 12 px for a refreshing mark, 16 for a picture on its way.
    pub fn size(mut self, size: Pixels) -> Self {
        self.0 = size;
        self
    }
}

impl RenderOnce for Spinner {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        turning(Icon::new(Glyph::LoaderCircle).with_size(self.0).text_color(cx.palette().muted_foreground), window, cx)
    }
}

/// An icon turning with the spinners, one turn a second.
pub fn turning(icon: Icon, window: &mut Window, cx: &mut App) -> Icon {
    if still(cx) {
        return icon;
    }
    wake_after(LOADING_FRAME, window, cx);
    icon.transform(Transformation::rotate(gpui_kit::percentage(loading_started().elapsed().as_secs_f32().fract())))
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
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        // The band crosses in the first 60% of each 2 s and is parked off the bar for the rest, as `skeleton` does; parked,
        // it asks for one frame, at the next crossing.
        let left = (!still(cx)).then(|| {
            let t = (loading_started().elapsed().as_secs_f32() / 2.).fract();
            let wait = if t < 0.6 { LOADING_FRAME } else { Duration::from_secs_f32((1. - t) * 2.) };
            wake_after(wait, window, cx);
            -1. + 2. * (t / 0.6).min(1.)
        });
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
        let sweep = |left: f32| {
            div()
                .absolute()
                .top_0()
                .bottom_0()
                .w_full()
                .left(relative(left))
                .child(half(glow.opacity(0.), glow).left(relative(0.4)))
                .child(half(glow, glow.opacity(0.)).left(relative(0.5)))
        };
        div()
            .debug_selector(|| "skeleton".into())
            .relative()
            .flex_none()
            .overflow_hidden()
            .w(self.width)
            .h(self.height)
            .rounded(px(6.))
            .bg(p.border)
            .children(left.map(sweep))
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

/// A thread's state in its row's status slot: every kind of `apps/web/src/components/status/kinds/`, in its registry's
/// order, and the Finished fold's own. The glyph stands alone; its word belongs on the hover.
#[derive(Clone, PartialEq)]
pub enum Status {
    NeedsYou,
    /// Held for a slot.
    Waiting,
    /// The crab is its glyph.
    Starting,
    /// Working, with the time it has run: `0s`, `59m`, `1h 5m`.
    Working(SharedString),
    /// At its plan's limit.
    Limited,
    /// Waiting for a limit's reset to go on.
    Resuming,
    Failed,
    Released,
    Expired,
    Stopped,
    NotPushed,
    /// Ended and not yet seen.
    Done,
    /// Read, with its age: `14m`, `3h`, `2d`.
    Read(SharedString),
    /// A sub-thread in a Finished fold.
    Finished,
}

/// `MarkState`: a machine's or a step's state, as `StateMark` draws it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MarkState {
    /// Setting up: the plain spinner, never the crab, since a machine at work is not an agent.
    Working,
    Done,
    NeedsYou,
    Failed,
    Pending,
    Offline,
    /// Draws nothing.
    Waiting,
    /// Draws nothing, as a resting thread does.
    Ready,
}

impl MarkState {
    /// `MARK_WORDS`: the one word a state is read as, for a hover that says nothing more.
    pub fn word(self) -> &'static str {
        match self {
            MarkState::Working => "Setting up",
            MarkState::Done => "Done",
            MarkState::NeedsYou => "Needs you",
            MarkState::Failed => "Failed",
            MarkState::Pending => "Pending",
            MarkState::Offline => "Offline",
            MarkState::Waiting => "Not started",
            MarkState::Ready => "Ready",
        }
    }
}

#[derive(Clone, PartialEq)]
enum Marked {
    Thread(Status),
    State(SharedString, MarkState),
}

/// The status mark: a thread's in a one-line row, 12 px, the time before the glyph or the crab, weight 500 where it is
/// toned; or a machine's or a step's (`StateMark`), one 14 px icon in its ink with the sentence on its hover.
#[derive(IntoElement)]
pub struct StatusMark {
    marked: Marked,
    tooltip: Option<SharedString>,
    crab: bool,
    rest: Option<Hsla>,
    glyph_ink: Option<Hsla>,
    key: Option<ElementId>,
    text: (Pixels, Pixels),
}

impl StatusMark {
    pub fn new(status: Status) -> Self {
        StatusMark { marked: Marked::Thread(status), tooltip: None, crab: true, rest: None, glyph_ink: None, key: None, text: text::XS }
    }

    /// `StateMark`: a machine's or a step's state, its word on the hover until `tooltip` gives the sentence.
    pub fn state(id: impl Into<SharedString>, state: MarkState) -> Self {
        StatusMark {
            marked: Marked::State(id.into(), state),
            tooltip: None,
            crab: true,
            rest: None,
            glyph_ink: None,
            key: None,
            text: text::XS,
        }
    }

    /// Whether a working thread's crab stands in the slot; a tile draws it at its title's end instead.
    pub fn crab(mut self, crab: bool) -> Self {
        self.crab = crab;
        self
    }

    /// The ink of a kind with no tone, which takes its row's own; the muted ink where none is given.
    pub fn rest_ink(mut self, ink: Hsla) -> Self {
        self.rest = Some(ink);
        self
    }

    /// The ink its glyph takes over its kind's, where the list it stands in inks every glyph alike, as the palette's
    /// rows do (`[&_svg:not([class*='text-'])]:text-muted-foreground` in `autocomplete.tsx`).
    pub fn glyph_ink(mut self, ink: Hsla) -> Self {
        self.glyph_ink = Some(ink);
        self
    }

    /// The size and line height of its time, where the slot takes its row's, as a tile's first row does.
    pub fn text(mut self, size: Pixels, line_height: Pixels) -> Self {
        self.text = (size, line_height);
        self
    }

    /// The key of the crab the mark draws, where a list draws several.
    pub fn key(mut self, key: impl Into<ElementId>) -> Self {
        self.key = Some(key.into());
        self
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
        let status = match self.marked {
            Marked::Thread(status) => status,
            Marked::State(id, state) => {
                let glyph = |g: Glyph, ink: Hsla| Icon::new(g).with_size(px(14.)).text_color(ink).into_any_element();
                let inner = match state {
                    MarkState::Waiting | MarkState::Ready => return div().into_any_element(),
                    MarkState::Working => spinner(cx).into_any_element(),
                    MarkState::Done => glyph(Glyph::CircleCheck, p.status_done),
                    MarkState::NeedsYou => glyph(Glyph::MessageCircleQuestionMark, p.status_input),
                    MarkState::Failed => glyph(Glyph::CircleAlert, p.status_failed),
                    MarkState::Pending => glyph(Glyph::CircleDashed, p.muted_foreground),
                    MarkState::Offline => glyph(Glyph::Unplug, p.muted_foreground),
                };
                let label = self.tooltip.unwrap_or_else(|| state.word().into());
                return div()
                    .id(id)
                    .debug_selector(|| "state-mark".into())
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .tooltip(tip(label))
                    .child(inner)
                    .into_any_element();
            }
        };
        let rest = self.rest.unwrap_or(p.muted_foreground);
        let (glyph, time, ink, toned) = match &status {
            Status::NeedsYou => (Some(Glyph::MessageCircleQuestionMark), None, p.status_input, true),
            Status::Waiting => (Some(Glyph::Hourglass), None, rest, false),
            Status::Starting => (None, None, p.status_working, true),
            Status::Working(elapsed) => (None, Some(elapsed.clone()), p.status_working, true),
            Status::Limited => (Some(Glyph::Gauge), None, p.warning_foreground, true),
            Status::Resuming => (Some(Glyph::Hourglass), None, rest, false),
            Status::Failed => (Some(Glyph::CircleAlert), None, p.status_failed, true),
            Status::Released => (Some(Glyph::CloudOff), None, rest, false),
            Status::Expired => (Some(Glyph::CalendarX2), None, rest, false),
            Status::Stopped => (Some(Glyph::CircleStop), None, rest, false),
            Status::NotPushed => (Some(Glyph::Upload), None, p.status_input, true),
            Status::Done => (Some(Glyph::CircleCheck), None, p.status_done, true),
            Status::Read(age) => (None, Some(age.clone()), rest, false),
            Status::Finished => (Some(Glyph::CircleCheck), None, rest, false),
        };
        let crab = match status {
            Status::Starting => true,
            Status::Working(_) => self.crab,
            _ => false,
        };
        let key = self.key;
        let mark = h_flex()
            .debug_selector(|| "status-mark".into())
            .flex_none()
            .min_w(px(16.))
            .justify_end()
            .gap_1()
            .text_size(self.text.0)
            .line_height(self.text.1)
            .font_weight(if toned { FontWeight::MEDIUM } else { FontWeight::NORMAL })
            .text_color(ink)
            .children(time)
            .children(glyph.map(|g| Icon::new(g).with_size(px(12.)).text_color(self.glyph_ink.unwrap_or(ink))))
            .when(crab, |el| {
                let crab = Crab::new(ink);
                el.child(match key {
                    Some(key) => crab.key(key),
                    None => crab,
                })
            });
        match self.tooltip {
            Some(text) => mark.id(SharedString::from(format!("status-mark-{text}"))).tooltip(tip(text)).into_any_element(),
            None => mark.into_any_element(),
        }
    }
}

/// A step's state in a list of steps (`StepState` in `settings/add/setup.ts`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// The muted empty circle, the name muted.
    NotStarted,
    /// At work: the plain spinner, or the crab on an agent's own step.
    Running,
    /// Set aside: the muted minus.
    Skipped,
    Done,
    Failed,
    NeedsYou,
    /// Passed with a warning: the triangle.
    Warned,
}

impl Step {
    /// The one word the mark's hover says (`StateMark`'s `MARK_WORDS`, and `StepRow.tsx`'s own labels).
    fn word(self, agent: bool) -> &'static str {
        match self {
            Step::NotStarted => "Not started",
            Step::Running if agent => "Working",
            Step::Running => MarkState::Working.word(),
            Step::Skipped => "Skipped",
            Step::Done => MarkState::Done.word(),
            Step::Failed => MarkState::Failed.word(),
            Step::NeedsYou => MarkState::NeedsYou.word(),
            Step::Warned => "Passed with a warning",
        }
    }
}

/// The step's mark in its 16 px box, as `StepRow.tsx` draws it: `StateMark`'s glyphs and inks, the muted circle, minus
/// and triangle of its own states.
pub fn step_mark(step: Step, cx: &App) -> impl IntoElement {
    let p = cx.palette();
    let glyph = |g: Glyph, ink: Hsla| Icon::new(g).with_size(px(14.)).text_color(ink).into_any_element();
    let mark = match step {
        Step::NotStarted => glyph(Glyph::Circle, p.muted_foreground.opacity(0.6)),
        Step::Running => spinner(cx).into_any_element(),
        Step::Skipped => glyph(Glyph::CircleMinus, p.muted_foreground),
        Step::Done => glyph(Glyph::CircleCheck, p.status_done),
        Step::Failed => glyph(Glyph::CircleAlert, p.status_failed),
        Step::NeedsYou => glyph(Glyph::MessageCircleQuestionMark, p.status_input),
        Step::Warned => glyph(Glyph::TriangleAlert, p.warning),
    };
    div().debug_selector(|| "step-mark".into()).flex().flex_none().items_center().justify_center().size(px(16.)).child(mark)
}

/// What happened and what to do about it, one paragraph in two inks (`data-k="step-refusal"`): the refusal's ink, then
/// the fix in the foreground. No room is kept for it, as `RefusalSlot` keeps.
pub fn step_refusal(said: impl Into<SharedString>, fix: Option<SharedString>, cx: &App) -> impl IntoElement {
    let p = cx.palette();
    let said: SharedString = said.into();
    let fix = fix.map(|f| format!(" {f}"));
    let whole: SharedString = format!("{said}{}", fix.as_deref().unwrap_or_default()).into();
    let mut highlights = vec![(0..said.len(), HighlightStyle { color: Some(p.error_foreground), ..Default::default() })];
    if let Some(fix) = &fix {
        highlights.push((said.len()..said.len() + fix.len(), HighlightStyle { color: Some(p.foreground), ..Default::default() }));
    }
    div()
        .debug_selector(|| "step-refusal".into())
        .text_size(text::FACT.0)
        .line_height(px(18.))
        .child(StyledText::new(whole).with_highlights(highlights))
}

type Toggle = std::rc::Rc<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

/// What a row's time says: nothing, how long it took, or the time since it started, ticking.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Time {
    None,
    Took(u64),
    Since(f64),
}

/// The words' edge in a step row: the 16 px mark and its 12 px gap.
const MARK_EDGE: Pixels = px(28.);

/// One step in a list of steps (`StepRow.tsx`): its mark, the title (muted until it starts) with its quiet note, the
/// time in the mono at the right and the chevron of a row that opens; under it what happened (`said` in the refusal's
/// ink, its fix in the foreground), what the caller puts there (a log, a sign-in) and the acts. An item under a step
/// steps in by the mark's width.
#[derive(IntoElement)]
pub struct StepRow {
    id: SharedString,
    step: Step,
    title: SharedString,
    tooltip: Option<SharedString>,
    note: Option<SharedString>,
    time: Time,
    said: Option<SharedString>,
    fix: Option<SharedString>,
    nested: bool,
    agent: bool,
    toggle: Option<(bool, Option<Toggle>)>,
    under: Vec<AnyElement>,
    acts: Vec<AnyElement>,
    corners: Corners,
}

impl StepRow {
    pub fn new(id: impl Into<SharedString>, step: Step, title: impl Into<SharedString>) -> Self {
        StepRow {
            id: id.into(),
            step,
            title: title.into(),
            tooltip: None,
            note: None,
            time: Time::None,
            said: None,
            fix: None,
            nested: false,
            agent: false,
            toggle: None,
            under: Vec::new(),
            acts: Vec::new(),
            corners: Corners::default(),
        }
    }

    /// The step's state in words on its mark's hover: "Skipped: Claude Code is not installed there".
    pub fn tooltip(mut self, text: impl Into<SharedString>) -> Self {
        self.tooltip = Some(text.into());
        self
    }

    /// The quiet note under the title: what the step is doing, or what it put there.
    pub fn note(mut self, note: impl Into<SharedString>) -> Self {
        self.note = Some(note.into());
        self
    }

    /// How long an ended step took, in ms: "0.4s", "1m 5s".
    pub fn took(mut self, ms: u64) -> Self {
        self.time = Time::Took(ms);
        self
    }

    /// When a running step began, in ms since the epoch: its time climbs from there once a second.
    pub fn since(mut self, epoch_ms: f64) -> Self {
        self.time = Time::Since(epoch_ms);
        self
    }

    /// What happened, in the refusal's ink under the row, and what to do about it in the foreground.
    pub fn said(mut self, said: impl Into<SharedString>, fix: Option<SharedString>) -> Self {
        self.said = Some(said.into()).filter(|s| !s.is_empty());
        self.fix = fix;
        self
    }

    /// An item under a step (one sign-in, one skill that did not land): it steps in by the mark's width.
    pub fn nested(mut self, nested: bool) -> Self {
        self.nested = nested;
        self
    }

    /// The steps are an agent's own (`TasksBar`): a step at work is the crab, not the spinner.
    pub fn agent(mut self, agent: bool) -> Self {
        self.agent = agent;
        self
    }

    /// A row in a list where rows open on their output: whether it is open, and the press that turns it; none on a row
    /// with nothing to open, which keeps the chevron's room so every time stands on one edge.
    pub fn toggle(mut self, open: bool, on_toggle: Option<impl Fn(&ClickEvent, &mut Window, &mut App) + 'static>) -> Self {
        self.toggle = Some((open, on_toggle.map(|f| std::rc::Rc::new(f) as Toggle)));
        self
    }

    /// What stands under the row before its acts: a log, a sign-in, a refusal.
    pub fn under(mut self, el: impl IntoElement) -> Self {
        self.under.push(el.into_any_element());
        self
    }

    /// An act under the row, in order: Retry, Skip, Copy the keys, Sign in.
    pub fn act(mut self, act: impl IntoElement) -> Self {
        self.acts.push(act.into_any_element());
        self
    }
}

impl From<StepRow> for CardItem {
    fn from(row: StepRow) -> Self {
        // A step's rule stands above its 48 px floor, not inside it, as `StepRow.tsx` draws it.
        CardItem::placed(move |corners| StepRow { corners, ..row }.into_any_element())
    }
}

impl RenderOnce for StepRow {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.palette().clone();
        let corners = self.corners;
        let inset = corners.inset;
        let lead = inset + if self.nested { MARK_EDGE } else { px(0.) };
        let quiet = self.step == Step::NotStarted;
        let crab = self.agent && self.step == Step::Running;
        let tip_words = self.tooltip.clone().unwrap_or_else(|| self.step.word(self.agent).into());
        let mark = div().id(SharedString::from(format!("{}-mark", self.id))).tooltip(tip(tip_words)).child(if crab {
            div()
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .size(px(16.))
                .child(Crab::new(p.status_working).key(SharedString::from(format!("{}-crab", self.id))))
                .into_any_element()
        } else {
            step_mark(self.step, cx).into_any_element()
        });
        let time = match self.time {
            Time::None => None,
            Time::Took(ms) => Some(div().child(fmt_duration(ms as f64)).into_any_element()),
            Time::Since(at) => {
                let ticking =
                    window.use_keyed_state(SharedString::from(format!("{}-time", self.id)), cx, |_, _| Ticking { since: at, next: None });
                ticking.update(cx, |t, _| t.since = at);
                Some(ticking.into_any_element())
            }
        };
        let chevron = self.toggle.as_ref().map(|(open, on)| match on {
            None => div().flex_none().size(px(14.)).into_any_element(),
            Some(_) => {
                let icon = Icon::new(Glyph::ChevronRight).with_size(px(14.)).flex_none().text_color(p.muted_foreground);
                if *open { icon.transform(Transformation::rotate(gpui_kit::percentage(0.25))) } else { icon }.into_any_element()
            }
        });
        let words = gpui_kit::component::v_flex()
            .flex_1()
            .min_w_0()
            .child(
                div()
                    .debug_selector(|| "title".into())
                    .min_w_0()
                    .text_size(text::SETTING_TITLE.0)
                    .line_height(text::SETTING_TITLE.1)
                    .font_weight(if quiet { FontWeight::NORMAL } else { FontWeight::MEDIUM })
                    .text_color(if quiet { p.muted_foreground } else { p.foreground })
                    .child(self.title),
            )
            .children(self.note.map(|note| {
                div()
                    .debug_selector(|| "step-note".into())
                    .text_size(text::NOTE.0)
                    .line_height(text::NOTE.1)
                    .text_color(p.muted_foreground)
                    .child(note)
            }));
        let right = (time.is_some() || chevron.is_some()).then(|| {
            h_flex()
                .flex_none()
                .justify_end()
                .gap_2()
                .child(
                    div()
                        .debug_selector(|| "step-time".into())
                        .font_family(gpui_kit::component::ActiveTheme::theme(cx).mono_font_family.clone())
                        .text_size(text::FACT.0)
                        .line_height(text::FACT.1)
                        .text_color(p.muted_foreground)
                        .children(time),
                )
                .children(chevron)
        });
        let line =
            h_flex().gap_x_3().py(size::ROW_PAD_Y).pl(lead).pr(inset).min_h(size::LINE_FLOOR).child(mark).child(words).children(right);
        let opened = !self.under.is_empty() || !self.acts.is_empty() || self.said.is_some();
        let r = corners.radius - px(1.);
        let line = match self.toggle.and_then(|(_, on)| on) {
            None => line.into_any_element(),
            Some(on) => {
                let id = SharedString::from(format!("{}-toggle", self.id));
                let over = motion::hovered(id.clone(), window, cx);
                let ground = if *over.read(cx) { p.accent.opacity(0.6) } else { p.accent.opacity(0.) };
                let ground = motion::ease_ink(id.clone(), "ground", ground, motion::COLORS, window, cx);
                line.id(id)
                    .debug_selector(|| "step-toggle".into())
                    .cursor_pointer()
                    .bg(ground)
                    .when(corners.top, |el| el.rounded_tl(r).rounded_tr(r))
                    .when(corners.bottom && !opened, |el| el.rounded_bl(r).rounded_br(r))
                    .tab_index(0)
                    .focus_visible(move |s| s.shadow(vec![ring_inset(p.ring)]))
                    .on_hover(motion::track_hover(over))
                    .on_click(move |e, window, cx| on(e, window, cx))
                    .into_any_element()
            }
        };
        let part = opened.then(|| {
            gpui_kit::component::v_flex()
                .debug_selector(|| "step-opened".into())
                .gap_3()
                .pb_4()
                .pl(lead + MARK_EDGE)
                .pr(inset)
                .children(self.said.map(|said| step_refusal(said, self.fix, cx)))
                .children(self.under)
                .when(!self.acts.is_empty(), |el| el.child(h_flex().flex_wrap().gap_2().children(self.acts)))
        });
        let wrap = div().id(self.id).debug_selector(|| "row".into()).flex().flex_col();
        let wrap = match corners.rule {
            Some(rule) => wrap.border_t_1().border_color(rule),
            None => wrap,
        };
        wrap.child(line).children(part)
    }
}

/// A running step's time, written again each second in an entity of its own so nothing else draws for the clock.
struct Ticking {
    since: f64,
    next: Option<Task<()>>,
}

impl Render for Ticking {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !still(cx) && self.next.is_none() {
            self.next = Some(cx.spawn(async move |this, cx| {
                cx.background_executor().timer(Duration::from_secs(1)).await;
                let _ = this.update(cx, |t, cx| {
                    t.next = None;
                    cx.notify();
                });
            }));
        }
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64() * 1000.).unwrap_or(0.);
        div().child(fmt_elapsed(now - self.since))
    }
}

/// `fmtDuration`'s short style: ms under a second, tenths under ten, whole seconds under a minute, then minutes and
/// seconds.
pub fn fmt_duration(ms: f64) -> String {
    if ms < 60_000. || !ms.is_finite() {
        if ms < 1_000. {
            return format!("{}ms", ms.round().max(1.) as i64);
        }
        if ms < 10_000. {
            let tenths = (ms / 100.).round() / 10.;
            return if tenths >= 10. { "10s".into() } else { format!("{tenths:.1}s") };
        }
        return format!("{}s", (ms / 1_000.).round() as i64);
    }
    let total = (ms / 1_000.).round() as i64;
    let (minutes, seconds) = (total / 60, total % 60);
    if seconds == 0 {
        format!("{minutes}m")
    } else {
        format!("{minutes}m {seconds}s")
    }
}

/// `fmtElapsed`: whole seconds, then the short style's minutes and seconds.
pub fn fmt_elapsed(ms: f64) -> String {
    let seconds = if ms.is_finite() && ms > 0. { (ms / 1_000.).floor() } else { 0. };
    if seconds < 60. {
        format!("{seconds}s")
    } else {
        fmt_duration(seconds * 1_000.)
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

/// The ms from `t` to the crab's next pose: a claw, a leg, a toe or the eyes a pixel from where they are. Between two
/// poses only the feet's alpha glides, by at most 0.2.
pub fn next_pose(t: f64) -> f64 {
    let pose = |t: f64| crab_frame(t).into_iter().map(|(x, y, w, h, _)| (x, y, w, h)).collect::<Vec<_>>();
    let now = pose(t);
    let mut ahead = POSE_STEP_MS;
    while ahead < 1000. && pose(t + ahead) == now {
        ahead += POSE_STEP_MS;
    }
    ahead
}

/// When the first crab was drawn, so every crab walks in step.
fn started() -> Instant {
    static STARTED: OnceLock<Instant> = OnceLock::new();
    *STARTED.get_or_init(Instant::now)
}

/// The most frames a second a walking crab draws. The web's draws on every animation frame, but there a frame is one
/// canvas copy; here each one draws the crab's view and the views above it again, so it draws only when a pixel of
/// the crab moves (`next_pose`), never more often than this.
pub const CRAB_FPS: u32 = 30;
/// How finely `next_pose` looks ahead for the crab's next move.
const POSE_STEP_MS: f64 = 4.;
/// `SCROLL_HOLD_MS` in `apps/web/src/lib/frames.ts`: crabs hold their frame this long after the last scroll.
pub const SCROLL_HOLD: Duration = Duration::from_millis(150);

/// When anything holding crabs last scrolled.
#[derive(Clone, Copy, Default)]
pub struct Scrolled(pub Option<Instant>);

impl Global for Scrolled {}

/// Holds every crab still for `SCROLL_HOLD` from now; a scrolling list calls it on each scroll.
pub fn hold_crabs(cx: &mut App) {
    cx.set_global(Scrolled(Some(Instant::now())));
}

fn held_for(cx: &App) -> Option<Duration> {
    let since = cx.try_global::<Scrolled>()?.0?.elapsed();
    SCROLL_HOLD.checked_sub(since).filter(|left| !left.is_zero())
}

/// One walking crab, its own entity. It asks for its next frame from its paint alone, and only when that paint lands
/// inside the window's visible area, so a crab scrolled out of view, clipped away or no longer drawn asks for none.
pub struct CrabView {
    ink: Hsla,
    /// The walk's time the crab shows, in ms.
    t: f64,
    next: Option<Task<()>>,
}

impl CrabView {
    pub fn new(ink: Hsla) -> Self {
        CrabView { ink, t: started().elapsed().as_secs_f64() * 1000., next: None }
    }

    /// Whether a next frame is asked for, which a test reads.
    pub fn walking(&self) -> bool {
        self.next.is_some()
    }

    fn ask_next(&mut self, cx: &mut Context<Self>) {
        if self.next.is_some() || still(cx) {
            return;
        }
        let t = started().elapsed().as_secs_f64() * 1000.;
        let wait = held_for(cx).unwrap_or_else(|| Duration::from_secs_f64(next_pose(t) / 1000.).max(Duration::from_secs(1) / CRAB_FPS));
        self.next = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(wait).await;
            _ = this.update(cx, |this, cx| {
                this.next = None;
                cx.notify();
            });
        }));
    }
}

impl Render for CrabView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if still(cx) {
            // Held still, the crab draws its frame at t = 0, as the web's does under reduced motion.
            self.t = 0.;
        } else if held_for(cx).is_none() {
            self.t = started().elapsed().as_secs_f64() * 1000.;
        }
        let (ink, t, me) = (self.ink, self.t, cx.weak_entity());
        let pixels = canvas(
            |_, _, _| (),
            move |bounds: Bounds<Pixels>, _, window, cx| {
                for (x, y, w, h, a) in crab_frame(t) {
                    let origin = bounds.origin + point(px(x), px(y));
                    window.paint_quad(fill(Bounds::new(origin, gsize(px(w), px(h))), ink.opacity(a)));
                }
                if window.content_mask().bounds.intersects(&bounds) {
                    _ = me.update(cx, |this, cx| this.ask_next(cx));
                }
            },
        )
        .size_full();
        div().debug_selector(|| "crab".into()).flex_none().w(px(CRAB.0)).h(px(CRAB.1)).child(pixels)
    }
}

/// The crab, in the ink it is given: an agent working. Never a loading spinner. Each one is a `CrabView` kept for as
/// long as the element is drawn, under the key it is given, or the place it was made at.
#[derive(IntoElement)]
pub struct Crab {
    ink: Hsla,
    key: ElementId,
}

impl Crab {
    #[track_caller]
    pub fn new(ink: Hsla) -> Self {
        Crab { ink, key: ElementId::CodeLocation(*std::panic::Location::caller()) }
    }

    /// The crab's own key, where one place draws several crabs, as rows of a list do.
    pub fn key(mut self, key: impl Into<ElementId>) -> Self {
        self.key = key.into();
        self
    }
}

impl RenderOnce for Crab {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let ink = self.ink;
        let view = window.use_keyed_state(self.key, cx, |_, _| CrabView::new(ink));
        view.update(cx, |v, _| v.ink = ink);
        view
    }
}

#[cfg(test)]
mod tests {
    use gpui_kit::component::{h_flex, Root};
    use gpui_kit::{prelude::*, px, size, Context, TestAppContext, Window};

    use super::{crab_frame, next_pose, MarkState, StatusMark};
    use crate::ui::{self, Side};

    #[test]
    fn the_crab_moves_a_pixel_about_nine_times_a_second() {
        let mut t = 0.;
        let mut poses = 0;
        while t < 10_000. {
            t += next_pose(t);
            poses += 1;
        }
        assert!((80..=100).contains(&poses), "{poses} poses in 10 s");
        let pose = |t: f64| crab_frame(t).into_iter().map(|(x, y, w, h, _)| (x, y, w, h)).collect::<Vec<_>>();
        assert_eq!(pose(1234.), pose(1234. + next_pose(1234.) - 4.), "nothing moves before the next pose");
        assert_ne!(pose(1234.), pose(1234. + next_pose(1234.)));
    }

    struct Lone(MarkState);

    impl Render for Lone {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            h_flex().p(px(24.)).child(StatusMark::state("mark", self.0))
        }
    }

    /// Every state `StateMark` draws is one 14 px icon; waiting and ready draw nothing.
    #[gpui_kit::test]
    fn a_state_mark_is_one_14_px_icon_and_ready_draws_nothing(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            ui::init(Side::Light, cx);
            cx.set_global(ui::Still(true));
        });
        let all = [
            MarkState::Working,
            MarkState::Done,
            MarkState::NeedsYou,
            MarkState::Failed,
            MarkState::Pending,
            MarkState::Offline,
            MarkState::Waiting,
            MarkState::Ready,
        ];
        for state in all {
            let view = cx.new(|_| Lone(state));
            let (_, vcx) = cx.add_window_view(|window, cx| Root::new(view, window, cx));
            vcx.simulate_resize(size(px(120.), px(80.)));
            vcx.run_until_parked();
            let drawn = vcx.debug_bounds("state-mark").map(|b| (b.size.width, b.size.height));
            let want = (!matches!(state, MarkState::Waiting | MarkState::Ready)).then_some((px(14.), px(14.)));
            assert_eq!(drawn, want, "{state:?}");
        }
    }

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
