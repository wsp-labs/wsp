// SPDX-License-Identifier: AGPL-3.0-only
//! A row's time that steps on its own: a resting row's age or a turn's running time. Each is its own view, drawn
//! cached in the box its row gives it, and wakes only when its text changes, which draws it again where it stands; the
//! sidebar draws nothing for a tick.

use std::time::{Duration, Instant};

use gpui_kit::component::h_flex;
use gpui_kit::{
    prelude::*, px, AnyElement, App, Context, ElementId, Entity, Font, FontWeight, Hsla, Pixels, SharedString, StyleRefinement, Task,
    TextRun, Window,
};

use super::model::{resting_age, running_time};
use crate::ui::Crab;

/// What a time counts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Count {
    /// How long ago a row came to rest: "now", "5m", "3h", "2d".
    Age,
    /// How long a turn has run: "42s", "17m", "1h 5m".
    Running,
}

impl Count {
    fn say(self, ms: f64) -> String {
        match self {
            Count::Age => resting_age(0., ms),
            Count::Running => running_time(ms),
        }
    }

    /// How long until the text `ms` in says next changes.
    fn wait(self, ms: f64) -> Duration {
        let (first, unit) = match self {
            Count::Age if ms < 60_000. => (60_000., 60_000.),
            Count::Age if ms < 3_600_000. => (0., 60_000.),
            Count::Age if ms < 86_400_000. => (0., 3_600_000.),
            Count::Age => (0., 86_400_000.),
            Count::Running if ms < 60_000. => (1_000., 1_000.),
            Count::Running if ms < 86_400_000. => (0., 60_000.),
            Count::Running => (0., 3_600_000.),
        };
        let left = if ms < first { first - ms } else { unit - ms.rem_euclid(unit) };
        Duration::from_millis(left.ceil() as u64 + 1)
    }
}

/// Where a time stands in the box its row gives it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Room {
    /// As wide as its text, which the row measures: a text of another width moves the row, so it draws the row again.
    Fitted,
    /// The rest of its line, the time at its start.
    Start,
    /// The rest of its line, the time at its end.
    End,
}

pub struct Age {
    count: Count,
    room: Room,
    /// How far the count had gone when the view was made, and the executor's clock then, which a test's clock moves.
    began: f64,
    at: Instant,
    text: SharedString,
    /// The font the text was last drawn in, which a step measures its next text by.
    font: Option<(Font, Pixels)>,
    _tick: Task<()>,
}

impl Age {
    fn new(count: Count, began: f64, room: Room, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let tick = cx.spawn_in(window, async move |this, cx| loop {
            let Ok(wait) = this.update(cx, |this, cx| this.count.wait(this.elapsed(cx))) else { return };
            cx.background_executor().timer(wait).await;
            if this.update_in(cx, |this, window, cx| this.step(window, cx)).is_err() {
                return;
            }
        });
        let at = cx.background_executor().now();
        Age { count, room, began, at, text: count.say(began).into(), font: None, _tick: tick }
    }

    fn elapsed(&self, cx: &App) -> f64 {
        self.began + cx.background_executor().now().saturating_duration_since(self.at).as_secs_f64() * 1000.
    }

    pub fn text(&self) -> &SharedString {
        &self.text
    }

    fn step(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text: SharedString = self.count.say(self.elapsed(cx)).into();
        if text == self.text {
            return;
        }
        let same_box = match (&self.font, self.room) {
            (_, Room::Start | Room::End) => true,
            (Some((font, size)), Room::Fitted) => width(&text, font, *size, window) == width(&self.text, font, *size, window),
            (None, Room::Fitted) => false,
        };
        self.text = text;
        if same_box {
            cx.notify_in_place();
        } else {
            cx.notify();
        }
    }
}

impl Render for Age {
    fn render(&mut self, window: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        crate::frames::count("SidebarAge");
        let style = window.text_style();
        self.font = Some((style.font(), style.font_size.to_pixels(window.rem_size())));
        h_flex()
            .size_full()
            .when(self.room != Room::Start, |el| el.justify_end())
            .child(gpui_kit::div().flex_none().debug_selector(|| "sidebar-age".into()).child(self.text.clone()))
    }
}

/// The width a line of `text` takes in `font` at `size`, as a text element lays it out.
fn width(text: &str, font: &Font, size: Pixels, window: &Window) -> Pixels {
    let run = TextRun {
        len: text.len(),
        font: font.clone(),
        color: Hsla::default(),
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    window.text_system().shape_line(SharedString::from(text.to_owned()), size, &[run], None).width.ceil()
}

/// The time `key` names in the row being drawn, `began` ms into its count: made once, then kept while the row is drawn.
pub fn age(key: String, count: Count, began: f64, room: Room, window: &mut Window, cx: &mut App) -> Entity<Age> {
    window.use_keyed_state(ElementId::from(SharedString::from(key)), cx, |window, cx| Age::new(count, began, room, window, cx))
}

/// A fitted time in its box: as wide as its text in the row's font at `size` and `weight`, `line` high.
pub fn fitted(age: &Entity<Age>, size: Pixels, line: Pixels, weight: FontWeight, window: &Window, cx: &App) -> AnyElement {
    let font = Font { weight, features: crate::ui::text::tabular(), ..window.text_style().font() };
    let w = width(age.read(cx).text(), &font, size, window);
    age.clone().cached(StyleRefinement::default().flex_none().w(w).h(line)).into_any_element()
}

/// A time over the rest of its line, as high as the line.
pub fn rest(age: &Entity<Age>) -> gpui_kit::Div {
    gpui_kit::div()
        .flex_1()
        .min_w_0()
        .self_stretch()
        .font_features(crate::ui::text::tabular())
        .child(age.clone().cached(StyleRefinement::default().size_full()))
}

/// The status mark of a kind that says a time, as `StatusMark` draws it, with the time live: Working's in its tone,
/// at weight 500, a resting age in `ink`. `crab` keys the crab after a working time, where the row draws it there.
pub fn timed_mark(
    time: &Entity<Age>,
    working: bool,
    ink: Hsla,
    crab: Option<SharedString>,
    text: (Pixels, Pixels),
    window: &Window,
    cx: &App,
) -> AnyElement {
    let weight = if working { FontWeight::MEDIUM } else { FontWeight::NORMAL };
    h_flex()
        .debug_selector(|| "status-mark".into())
        .flex_none()
        .min_w(px(16.))
        .justify_end()
        .gap_1()
        .text_size(text.0)
        .line_height(text.1)
        .font_weight(weight)
        // `tabular-nums`, as the web's slot draws it, so most steps keep the time's width and draw it alone.
        .font_features(crate::ui::text::tabular())
        .text_color(ink)
        .child(fitted(time, text.0, text.1, weight, window, cx))
        .children(crab.map(|key| Crab::new(ink).key(key)))
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use gpui_kit::component::{h_flex, v_flex};
    use gpui_kit::{div, prelude::*, px, size, Context, TestAppContext, Window};

    use super::{age, timed_mark, Count, Room};
    use crate::ui::{ActivePalette, Side, Status, StatusMark};

    /// Each time drawn twice, by `StatusMark` with its text fixed and live in its box.
    struct Both;

    const TIMES: [(Count, f64, bool, &str); 4] = [
        (Count::Age, 0., false, "now"),
        (Count::Age, 120_000., false, "2m"),
        (Count::Running, 7_000., true, "7s"),
        (Count::Running, 3_900_000., true, "1h 5m"),
    ];

    impl Render for Both {
        fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let ink = cx.palette().muted_foreground;
            v_flex().children(TIMES.iter().enumerate().map(|(i, &(count, began, working, text))| {
                let fixed = if working { Status::Working(text.into()) } else { Status::Read(text.into()) };
                let time = age(format!("time-{i}"), count, began, Room::Fitted, window, cx);
                h_flex()
                    .child(
                        div()
                            .debug_selector(move || format!("fixed-{i}"))
                            .flex_none()
                            .child(StatusMark::new(fixed).crab(false).text(px(11.), px(14.))),
                    )
                    .child(div().debug_selector(move || format!("live-{i}")).flex_none().child(timed_mark(
                        &time,
                        working,
                        ink,
                        None,
                        (px(11.), px(14.)),
                        window,
                        cx,
                    )))
            }))
        }
    }

    /// Catches: a live time whose box is not as wide as its text, which moves the row's other words off today's place.
    #[gpui_kit::test]
    fn a_live_time_takes_the_room_the_fixed_mark_took(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        cx.update(|cx| crate::ui::init(Side::Light, cx));
        let (_, cx) = cx.add_window_view(|_, _| Both);
        cx.simulate_resize(size(px(400.), px(200.)));
        cx.run_until_parked();
        for (i, &(_, _, _, text)) in TIMES.iter().enumerate() {
            let fixed = cx.debug_bounds(Box::leak(format!("fixed-{i}").into_boxed_str())).expect("the fixed mark");
            let live = cx.debug_bounds(Box::leak(format!("live-{i}").into_boxed_str())).expect("the live mark");
            assert_eq!(live.size, fixed.size, "{text} takes the fixed mark's room");
        }
    }

    #[test]
    fn a_time_wakes_when_its_text_next_changes() {
        let next = |count: Count, ms: f64| count.wait(ms).as_millis() as f64;
        assert_eq!(next(Count::Age, 30_000.), 30_001., "now until a minute");
        assert_eq!(next(Count::Age, -5_000.), 65_001., "a moment ahead of the clock reads now until a minute past it");
        assert_eq!(next(Count::Age, 5. * 60_000. + 10_000.), 50_001., "5m until 6m");
        assert_eq!(next(Count::Age, 3. * 3_600_000. + 60_000.), 3_540_001., "3h until 4h");
        assert_eq!(next(Count::Running, 42_400.), 601., "42s until 43s");
        assert_eq!(next(Count::Running, -300.), 1_301., "0s until 1s");
        assert_eq!(next(Count::Running, 65. * 60_000. + 1_000.), 59_001., "1h 5m until 1h 6m");
        for (count, ms) in [(Count::Age, 59_999.), (Count::Age, 3_599_999.), (Count::Running, 59_999.), (Count::Running, 999.)] {
            assert_ne!(count.say(ms), count.say(ms + next(count, ms)), "{count:?} at {ms} moves when it wakes");
        }
    }
}
