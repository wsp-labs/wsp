// SPDX-License-Identifier: AGPL-3.0-only
//! A view notified in place draws again where it stands: nothing above it renders, the views beside it keep their
//! output and still take clicks, and a view not drawn cached draws again with the nearest cached view around it.

use gpui_kit::component::Root;
use gpui_kit::{div, point, prelude::*, px, size, AppContext, Context, Entity, Modifiers, StyleRefinement, TestAppContext};
use gpui_kit::{VisualTestContext, Window};

fn full() -> StyleRefinement {
    StyleRefinement::default().size_full()
}

fn leaked(selector: String) -> &'static str {
    Box::leak(selector.into_boxed_str())
}

#[derive(Default)]
struct Leaf {
    n: usize,
    rows: usize,
    renders: usize,
}

impl Render for Leaf {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.renders += 1;
        let n = self.n;
        div()
            .size_full()
            .child(div().debug_selector(move || format!("leaf:{n}")).w(px(10.)).h(px(4.)))
            .children((0..self.rows).map(|row| div().id(row).debug_selector(move || format!("row:{row}")).w(px(10.)).h(px(2.))))
    }
}

#[derive(Default)]
struct Button {
    clicks: usize,
    renders: usize,
}

impl Render for Button {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.renders += 1;
        let clicks = self.clicks;
        div()
            .id("button")
            .debug_selector(move || format!("button:{clicks}"))
            .size_full()
            .on_click(cx.listener(|this, _, _, _| this.clicks += 1))
    }
}

/// Drawn plainly inside the middle view, not cached.
#[derive(Default)]
struct Plain {
    n: usize,
    renders: usize,
}

impl Render for Plain {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.renders += 1;
        let n = self.n;
        div().debug_selector(move || format!("plain:{n}")).w(px(10.)).h(px(10.))
    }
}

struct Middle {
    leaf: Entity<Leaf>,
    button: Entity<Button>,
    plain: Entity<Plain>,
    renders: usize,
}

impl Render for Middle {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.renders += 1;
        div()
            .flex()
            .flex_col()
            .size_full()
            .child(div().flex_none().w(px(100.)).h(px(40.)).child(self.leaf.clone().cached(full())))
            .child(div().flex_none().w(px(100.)).h(px(40.)).child(self.button.clone().cached(full())))
            .child(self.plain.clone())
    }
}

struct Outer {
    middle: Entity<Middle>,
    renders: usize,
}

impl Render for Outer {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.renders += 1;
        div().size_full().child(self.middle.clone().cached(full()))
    }
}

struct Views {
    outer: Entity<Outer>,
    middle: Entity<Middle>,
    leaf: Entity<Leaf>,
    button: Entity<Button>,
    plain: Entity<Plain>,
}

impl Views {
    fn renders(&self, cx: &mut VisualTestContext) -> [usize; 5] {
        cx.read(|cx| {
            [
                self.outer.read(cx).renders,
                self.middle.read(cx).renders,
                self.leaf.read(cx).renders,
                self.button.read(cx).renders,
                self.plain.read(cx).renders,
            ]
        })
    }

    /// Renders of each view while `f` runs and the frames it asks for are drawn.
    fn while_drawing(&self, cx: &mut VisualTestContext, f: impl FnOnce(&mut VisualTestContext)) -> [usize; 5] {
        let before = self.renders(cx);
        f(cx);
        cx.run_until_parked();
        let after = self.renders(cx);
        std::array::from_fn(|ix| after[ix] - before[ix])
    }
}

fn opened(cx: &mut TestAppContext) -> (Views, &mut VisualTestContext) {
    cx.update(gpui_kit::init);
    let leaf = cx.new(|_| Leaf::default());
    let button = cx.new(|_| Button::default());
    let plain = cx.new(|_| Plain::default());
    let middle = cx.new(|_| Middle { leaf: leaf.clone(), button: button.clone(), plain: plain.clone(), renders: 0 });
    let outer = cx.new(|_| Outer { middle: middle.clone(), renders: 0 });
    let root = outer.clone();
    let (_, cx) = cx.add_window_view(|window, cx| Root::new(root, window, cx).cache_view());
    cx.simulate_resize(size(px(400.), px(300.)));
    cx.run_until_parked();
    (Views { outer, middle, leaf, button, plain }, cx)
}

fn click(cx: &mut VisualTestContext, selector: &'static str) {
    let at = cx.debug_bounds(selector).unwrap_or_else(|| panic!("{selector} is drawn")).center();
    cx.simulate_click(at, Modifiers::none());
}

#[gpui_kit::test]
fn a_view_notified_in_place_draws_alone_where_it_stands(cx: &mut TestAppContext) {
    let (views, cx) = opened(cx);
    let leaf = views.leaf.clone();
    let drawn = views.while_drawing(cx, |cx| {
        leaf.update(cx, |leaf, cx| {
            leaf.n = 1;
            cx.notify_in_place();
        })
    });
    assert_eq!(drawn, [0, 0, 1, 0, 0], "only the leaf renders: outer, middle, leaf, button, plain");
    assert!(cx.debug_bounds("leaf:1").is_some() && cx.debug_bounds("leaf:0").is_none(), "the leaf shows its change");
    assert_eq!(cx.debug_bounds("leaf:1").unwrap().origin, point(px(0.), px(0.)), "where it stood");
    click(cx, "button:0");
    cx.run_until_parked();
    assert_eq!(views.button.read_with(cx, |b, _| b.clicks), 1, "the view beside it still takes a click");
}

#[gpui_kit::test]
fn a_view_that_grows_in_place_leaves_the_views_after_it_whole(cx: &mut TestAppContext) {
    let (views, cx) = opened(cx);
    let leaf = views.leaf.clone();
    for (n, rows) in [(1, 6), (2, 1), (3, 9), (4, 9), (5, 2), (6, 7), (7, 7), (8, 3), (9, 9)] {
        let drawn = views.while_drawing(cx, |cx| {
            leaf.update(cx, |leaf, cx| {
                leaf.n = n;
                leaf.rows = rows;
                cx.notify_in_place();
            })
        });
        assert_eq!(drawn, [0, 0, 1, 0, 0], "step {n}");
        assert!(cx.debug_bounds(leaked(format!("row:{}", rows - 1))).is_some(), "step {n}: all {rows} rows drawn");
        assert!(cx.debug_bounds(leaked(format!("row:{rows}"))).is_none(), "step {n}: no row left from before");
        assert_eq!(cx.debug_bounds("plain:0").map(|b| b.origin.y), Some(px(80.)), "step {n}: the plain view stays put");
    }
    click(cx, "button:0");
    cx.run_until_parked();
    assert_eq!(views.button.read_with(cx, |b, _| b.clicks), 1, "the button still takes a click");
    // A plain notify of the middle view draws it whole again, from the output the in-place frames left.
    let middle = views.middle.clone();
    let drawn = views.while_drawing(cx, |cx| middle.update(cx, |_, cx| cx.notify()));
    assert_eq!(drawn[..2], [1, 1], "outer and middle render");
    assert!(cx.debug_bounds("leaf:9").is_some() && cx.debug_bounds("row:8").is_some() && cx.debug_bounds("button:1").is_some());
}

#[gpui_kit::test]
fn two_views_notified_in_place_in_one_frame_both_draw(cx: &mut TestAppContext) {
    let (views, cx) = opened(cx);
    let (leaf, button) = (views.leaf.clone(), views.button.clone());
    let drawn = views.while_drawing(cx, |cx| {
        leaf.update(cx, |leaf, cx| {
            leaf.n = 4;
            cx.notify_in_place();
        });
        button.update(cx, |button, cx| {
            button.clicks = 7;
            cx.notify_in_place();
        });
    });
    assert_eq!(drawn, [0, 0, 1, 1, 0]);
    assert!(cx.debug_bounds("leaf:4").is_some() && cx.debug_bounds("button:7").is_some());
}

#[gpui_kit::test]
fn a_view_not_drawn_cached_draws_with_the_cached_view_around_it(cx: &mut TestAppContext) {
    let (views, cx) = opened(cx);
    let plain = views.plain.clone();
    let drawn = views.while_drawing(cx, |cx| {
        plain.update(cx, |plain, cx| {
            plain.n = 5;
            cx.notify_in_place();
        })
    });
    assert_eq!(drawn[..2], [0, 1], "the middle view renders for it, and nothing above the middle view");
    assert_eq!(drawn[4], 1);
    assert!(cx.debug_bounds("plain:5").is_some());
}

#[gpui_kit::test]
fn a_notify_in_place_tells_no_observer(cx: &mut TestAppContext) {
    let (views, cx) = opened(cx);
    let heard = std::rc::Rc::new(std::cell::Cell::new(0));
    let counter = heard.clone();
    let _observer = cx.update(|_, cx| cx.observe(&views.leaf, move |_, _| counter.set(counter.get() + 1)));
    views.leaf.update(cx, |_, cx| cx.notify_in_place());
    cx.run_until_parked();
    assert_eq!(heard.get(), 0, "drawing again in place changes nothing an observer reads");
    views.leaf.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    assert_eq!(heard.get(), 1, "a plain notify still tells it");
}

struct Hider {
    leaf: Entity<Leaf>,
    hidden: bool,
}

impl Render for Hider {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(div().w(px(100.)).h(px(40.)).when(self.hidden, |el| el.invisible()).child(self.leaf.clone().cached(full())))
    }
}

/// A hidden parent prepaints a cached view without painting it; drawn again after, it paints whole, not from a stale
/// record of a paint that never happened.
#[gpui_kit::test]
fn a_cached_view_hidden_for_a_frame_paints_again(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let leaf = cx.new(|_| Leaf { n: 6, ..Leaf::default() });
    let hider = cx.new(|_| Hider { leaf: leaf.clone(), hidden: false });
    let root = hider.clone();
    let (_, cx) = cx.add_window_view(|window, cx| Root::new(root, window, cx).cache_view());
    cx.simulate_resize(size(px(400.), px(300.)));
    cx.run_until_parked();
    for hidden in [true, false, true, false] {
        hider.update(cx, |hider, cx| {
            hider.hidden = hidden;
            cx.notify();
        });
        cx.run_until_parked();
        assert_eq!(cx.debug_bounds("leaf:6").is_some(), !hidden, "hidden: {hidden}");
    }
}

/// Draws `middle` cached or whole as `cached` says, after `lines` lines of text that stand elsewhere: they move where
/// its output falls among the frame's, never where it is drawn.
struct Switcher {
    middle: Entity<Middle>,
    cached: bool,
    lines: usize,
}

impl Render for Switcher {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let middle =
            if self.cached { self.middle.clone().cached(full()).into_any_element() } else { self.middle.clone().into_any_element() };
        div()
            .size_full()
            .relative()
            .child(
                div()
                    .absolute()
                    .left(px(220.))
                    .children((0..self.lines).map(|line| div().id(line).h(px(10.)).child(format!("line {line}")))),
            )
            .child(div().absolute().top_0().left_0().w(px(200.)).h(px(200.)).child(middle))
    }
}

/// A cached view inside a view drawn cached, then whole: while its parent's output was reused it was not placed, so
/// drawn again it draws whole and never by a record of a frame before.
#[gpui_kit::test]
fn a_cached_view_whose_parent_turns_from_cached_to_whole_draws_whole(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let leaf = cx.new(|_| Leaf { n: 1, ..Leaf::default() });
    let button = cx.new(|_| Button::default());
    let plain = cx.new(|_| Plain::default());
    let middle = cx.new(|_| Middle { leaf: leaf.clone(), button, plain, renders: 0 });
    let switcher = cx.new(|_| Switcher { middle, cached: false, lines: 1 });
    let root = switcher.clone();
    let (_, cx) = cx.add_window_view(|window, cx| Root::new(root, window, cx));
    cx.simulate_resize(size(px(400.), px(600.)));
    cx.run_until_parked();
    for (cached, lines) in [(true, 3), (true, 5), (true, 2), (false, 6), (true, 1), (false, 4)] {
        switcher.update(cx, |s, cx| {
            (s.cached, s.lines) = (cached, lines);
            cx.notify();
        });
        cx.run_until_parked();
        assert_eq!(cx.debug_bounds("leaf:1").map(|b| b.origin), Some(point(px(0.), px(0.))), "cached {cached}, {lines} lines");
    }
}
