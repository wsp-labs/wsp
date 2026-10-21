// SPDX-License-Identifier: AGPL-3.0-only
//! `DigitRoll`, after `components/ui/digit-roll.tsx`: a figure that rests as plain text and rolls its digits only
//! while a new value lands. For one roll each digit is a column of 0 to 9 clipped to one line and moved to its new
//! figure; a timer ends the roll, not the columns' transition, so a roll can never stay drawn. Digits are keyed from
//! the right, so a figure that gains a digit keeps its ones in place. Under reduced motion it stands still. The figure
//! is a view of its own in the box its text fills, and each frame of a roll draws again only that view, in place.

use std::time::Instant;

use gpui_kit::{div, prelude::*, px, transparent_black, App, Context, ElementId, Pixels, SharedString, StyleRefinement, Window};

use super::motion;

#[derive(IntoElement)]
pub struct DigitRoll {
    id: ElementId,
    value: SharedString,
    roll_in: bool,
    line_height: Pixels,
}

impl DigitRoll {
    /// A figure on lines `line_height` tall, which its columns are clipped to.
    pub fn new(id: impl Into<ElementId>, value: impl Into<SharedString>, line_height: Pixels) -> Self {
        DigitRoll { id: id.into(), value: value.into(), roll_in: false, line_height }
    }

    /// `rollIn`: the first value rolls up from zeros.
    pub fn roll_in(mut self) -> Self {
        self.roll_in = true;
        self
    }
}

/// A roll's start: the figure it leaves, whether its columns stood there for a frame for their move to start from,
/// and when their move began.
struct Roll {
    from: Vec<char>,
    placed: bool,
    began: Option<Instant>,
}

/// The figure as a view of its own, so each frame of its roll draws again only the figure.
pub struct Rolling {
    id: ElementId,
    shown: SharedString,
    line_height: Pixels,
    roll: Option<Roll>,
    /// Counts the rolls' starts and changes, so only the newest timer ends the roll.
    armed: u64,
    timed: u64,
}

fn zeros(value: &str) -> Vec<char> {
    value.chars().map(|c| if c.is_ascii_digit() { '0' } else { c }).collect()
}

impl RenderOnce for DigitRoll {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let still = motion::reduced(cx);
        let (value, roll_in, line_height) = (self.value.clone(), self.roll_in, self.line_height);
        let rolling = window.use_keyed_state(self.id.clone(), cx, |_, _| Rolling {
            id: self.id.clone(),
            shown: value.clone(),
            line_height,
            roll: (roll_in && !still).then(|| Roll { from: zeros(&value), placed: false, began: None }),
            armed: 1,
            timed: 0,
        });
        let changed = rolling.update(cx, |r, _| {
            let changed = r.shown != value || r.line_height != line_height;
            r.line_height = line_height;
            if r.shown != value {
                let left = std::mem::replace(&mut r.shown, value.clone());
                r.roll = Some(Roll { from: left.chars().collect(), placed: false, began: None });
                r.armed += 1;
            }
            if still {
                r.roll = None;
            }
            changed
        });
        // The box the figure's text fills in the style it stands in, a column of each digit as wide as that digit.
        let style = window.text_style();
        let size = style.font_size.to_pixels(window.rem_size());
        let width = window.text_system().shape_line(value.clone(), size, &[style.to_run(value.len())], None).width;
        let style = StyleRefinement::default().flex_none().w(width).h(line_height);
        // A new figure draws whole, as its last frame drawn cached in the same box would stand for it.
        if changed {
            return div().flex_none().w(width).h(line_height).child(rolling).into_any_element();
        }
        rolling.cached(style).into_any_element()
    }
}

impl Render for Rolling {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        crate::frames::count("DigitRoll");
        if self.roll.is_some() && self.armed != self.timed {
            let armed = self.armed;
            self.timed = armed;
            cx.spawn(async move |this, cx| {
                cx.background_executor().timer(motion::ROLL_END).await;
                _ = this.update(cx, |r, cx| {
                    if r.armed == armed {
                        r.roll = None;
                        cx.notify_in_place();
                    }
                });
            })
            .detach();
        }

        let figure = div().id(self.id.clone()).flex_none().whitespace_nowrap();
        let Some((from, placed, began)) = self.roll.as_ref().map(|r| (r.from.clone(), r.placed, r.began)) else {
            return figure.child(self.shown.clone());
        };
        let now = cx.background_executor().now();
        // The columns stand on the figure they leave for a frame, then move from the frame after.
        let progress = match (placed, began) {
            (false, _) => None,
            (true, None) => Some(0.),
            (true, Some(began)) => Some((now.saturating_duration_since(began).as_secs_f32() / motion::ROLL.as_secs_f32()).min(1.)),
        };
        if let Some(roll) = self.roll.as_mut() {
            if roll.placed {
                roll.began.get_or_insert(now);
            }
            roll.placed = true;
        }
        if progress.is_none_or(|k| k < 1.) {
            window.request_animation_frame_in_place();
        }
        let eased = progress.map(|k| motion::settle_curve().sample(k));
        let chars: Vec<char> = self.shown.chars().collect();
        let offset = from.len() as isize - chars.len() as isize;
        let lh = self.line_height;
        let cells = chars.iter().enumerate().map(|(i, &c)| {
            let Some(digit) = c.to_digit(10) else { return div().child(c.to_string()).into_any_element() };
            let was = usize::try_from(i as isize + offset).ok().and_then(|at| from.get(at)).and_then(|w| w.to_digit(10));
            let was = was.unwrap_or(digit) as f32;
            let at = eased.map_or(was, |e| was + (digit as f32 - was) * e);
            div()
                .debug_selector(|| "digit-column".into())
                .relative()
                .overflow_hidden()
                .h(lh)
                .child(div().text_color(transparent_black()).child(c.to_string()))
                .child(
                    div()
                        .debug_selector(|| "digit-strip".into())
                        .absolute()
                        .left_0()
                        .right_0()
                        .top(px(-at * f32::from(lh)))
                        .flex()
                        .flex_col()
                        .items_center()
                        .children((0..10).map(|d| div().h(lh).line_height(lh).child(d.to_string()))),
                )
                .into_any_element()
        });
        figure.flex().children(cells.collect::<Vec<_>>())
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use gpui_kit::component::Root;
    use gpui_kit::{size, Context, Entity, TestAppContext, VisualTestContext};

    use super::*;
    use crate::ui::{self, Side};

    struct Figure(SharedString);

    impl Render for Figure {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().p(px(20.)).text_size(px(26.)).line_height(px(32.)).child(DigitRoll::new("figure", self.0.clone(), px(32.)).roll_in())
        }
    }

    fn open<'a>(cx: &'a mut TestAppContext, value: &str, still: bool) -> (Entity<Figure>, &'a mut VisualTestContext) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            ui::init(Side::Light, cx);
            cx.set_global(ui::Still(still));
        });
        let figure = cx.new(|_| Figure(value.to_owned().into()));
        let (_, vcx) = cx.add_window_view(|window, cx| Root::new(figure.clone(), window, cx));
        vcx.simulate_resize(size(px(320.), px(120.)));
        vcx.run_until_parked();
        (figure, vcx)
    }

    fn draw(cx: &mut VisualTestContext, after: u64) {
        cx.executor().advance_clock(Duration::from_millis(after));
        cx.update(|window, cx| window.simulate_next_frame(cx));
        cx.run_until_parked();
    }

    /// The figure the one column shows, in lines its strip has moved up, while the figure rolls.
    fn column(cx: &mut VisualTestContext) -> Option<f32> {
        let clip = cx.debug_bounds("digit-column")?;
        let strip = cx.debug_bounds("digit-strip")?;
        Some(f32::from(clip.top() - strip.top()) / 32.)
    }

    #[gpui_kit::test]
    fn a_figure_rolls_in_from_zero_and_rests_as_text_once_the_timer_ends(cx: &mut TestAppContext) {
        let (_, cx) = open(cx, "7", false);
        assert_eq!(column(cx), Some(0.), "the first value stands on its zero for a frame");
        draw(cx, 0);
        draw(cx, 250);
        let halfway = column(cx).expect("still rolling at 250 ms");
        assert!(halfway > 0. && halfway < 7., "on its way from 0 to 7: {halfway}");
        draw(cx, 300);
        assert_eq!(column(cx), Some(7.), "the column has landed by 550 ms");
        draw(cx, 50);
        assert_eq!(column(cx), None, "the timer ends the roll at 600 ms and the figure is text again");
    }

    #[gpui_kit::test]
    fn a_new_value_rolls_from_the_old_one(cx: &mut TestAppContext) {
        let (figure, cx) = open(cx, "7", false);
        draw(cx, 0);
        draw(cx, 700);
        assert_eq!(column(cx), None);
        cx.update(|_, cx| {
            figure.update(cx, |f, cx| {
                f.0 = "3".into();
                cx.notify();
            })
        });
        cx.run_until_parked();
        assert_eq!(column(cx), Some(7.), "the column starts on the figure it leaves");
        draw(cx, 0);
        draw(cx, 250);
        let halfway = column(cx).expect("rolling");
        assert!(halfway > 3. && halfway < 7., "on its way down to 3: {halfway}");
        draw(cx, 400);
        assert_eq!(column(cx), None);
    }

    #[gpui_kit::test]
    fn under_reduced_motion_nothing_rolls(cx: &mut TestAppContext) {
        let (figure, cx) = open(cx, "7", true);
        assert_eq!(column(cx), None, "no roll in");
        cx.update(|_, cx| {
            figure.update(cx, |f, cx| {
                f.0 = "3".into();
                cx.notify();
            })
        });
        cx.run_until_parked();
        assert_eq!(column(cx), None, "no roll to a new value");
    }
}
