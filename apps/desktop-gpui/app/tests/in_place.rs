// SPDX-License-Identifier: AGPL-3.0-only
//! A view notified in place draws again where it stands: nothing above it renders, the views beside it keep their
//! output and still take clicks, and a view not drawn cached draws again with the nearest cached view around it.

use desktop_gpui::ui::{self, ActivePalette as _, Palette, Side};
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

/// Paints its ground in the active palette, with `inner` drawn cached inside it.
struct Ground {
    n: usize,
    inner: Option<Entity<Ground>>,
    renders: usize,
}

impl Render for Ground {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.renders += 1;
        let n = self.n;
        div()
            .size_full()
            .p(px(4.))
            .bg(cx.palette().background)
            .debug_selector(move || format!("ground:{n}"))
            .children(self.inner.clone().map(|inner| inner.cached(full())))
    }
}

struct Panel {
    top: Entity<Ground>,
    scope: Palette,
}

impl Render for Panel {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(ui::theme::scoped(self.scope.clone(), self.top.clone().cached(full())))
    }
}

/// A view drawn in place two or three cached views under a palette's scope keeps that palette: each view's record
/// holds the same scope, and entering it again inside itself undid it, so every second level drew in the page's.
#[gpui_kit::test]
fn a_view_drawn_in_place_deep_under_a_scope_keeps_its_palette(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Dark, cx);
    });
    let third = cx.new(|_| Ground { n: 3, inner: None, renders: 0 });
    let second = cx.new(|_| Ground { n: 2, inner: Some(third.clone()), renders: 0 });
    let first = cx.new(|_| Ground { n: 1, inner: Some(second.clone()), renders: 0 });
    let grounds = [first, second, third];
    let mut scope = cx.update(|cx| cx.palette().clone());
    scope.background = gpui_kit::hsla(0.6, 0.5, 0.3, 1.);
    let panel = cx.new(|_| Panel { top: grounds[0].clone(), scope });
    let (_, cx) = cx.add_window_view(|window, cx| Root::new(panel, window, cx).cache_view());
    cx.simulate_resize(size(px(400.), px(300.)));
    cx.run_until_parked();
    let quads = |cx: &mut VisualTestContext| -> Vec<String> {
        cx.update(|window, _| window.painted_quads().into_iter().map(|q| format!("{:?} {:?}", q.bounds, q.background)).collect())
    };
    for depth in [2, 3] {
        let view = grounds[depth - 1].clone();
        let before: Vec<usize> = grounds.iter().map(|g| g.read_with(cx, |g, _| g.renders)).collect();
        view.update(cx, |_, cx| cx.notify_in_place());
        cx.run_until_parked();
        let drawn: Vec<usize> = grounds.iter().zip(&before).map(|(g, b)| g.read_with(cx, |g, _| g.renders) - b).collect();
        assert_eq!(drawn[depth - 1], 1, "depth {depth}: drawn again: {drawn:?}");
        assert!(drawn[..depth - 1].iter().all(|&n| n == 0), "depth {depth}: nothing above it drew: {drawn:?}");
        let in_place = quads(cx);
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
        assert_eq!(in_place, quads(cx), "depth {depth}: drawn in place, then whole, the grounds painted differently");
    }
}

/// A count no view draws by itself, read by the views below.
struct Count(usize);

struct Reader {
    name: &'static str,
    count: Entity<Count>,
    inner: Vec<Entity<Reader>>,
    renders: usize,
}

impl Render for Reader {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.renders += 1;
        let (name, count) = (self.name, self.count.read(cx).0);
        div()
            .flex()
            .size_full()
            .child(div().debug_selector(move || format!("{name}:{count}")).w(px(10.)).h(px(4.)))
            .children(self.inner.iter().map(|inner| div().w(px(100.)).h(px(40.)).child(inner.clone().cached(full()))))
    }
}

fn readers(cx: &mut TestAppContext) -> (Entity<Count>, [Entity<Reader>; 3], &mut VisualTestContext) {
    cx.update(gpui_kit::init);
    let count = cx.new(|_| Count(0));
    let reader = |name, inner| Reader { name, count: count.clone(), inner, renders: 0 };
    let left = cx.new(|_| reader("left", Vec::new()));
    let right = cx.new(|_| reader("right", Vec::new()));
    let parent = cx.new(|_| reader("parent", vec![left.clone(), right.clone()]));
    let top = parent.clone();
    let (_, cx) = cx.add_window_view(|window, cx| Root::new(top, window, cx).cache_view());
    cx.simulate_resize(size(px(400.), px(300.)));
    cx.run_until_parked();
    (count, [parent, left, right], cx)
}

fn bump(count: &Entity<Count>, readers: &[Entity<Reader>; 3], n: usize, cx: &mut VisualTestContext) -> Vec<usize> {
    let before: Vec<usize> = readers.iter().map(|r| r.read_with(cx, |r, _| r.renders)).collect();
    count.update(cx, |count, cx| {
        count.0 = n;
        cx.notify();
    });
    cx.run_until_parked();
    readers.iter().zip(before).map(|(r, b)| r.read_with(cx, |r, _| r.renders) - b).collect()
}

/// Every cached view that read an entity draws again when it changes: a parent and both its children, though the
/// parent read it first in the frame and the left child before the right.
#[gpui_kit::test]
fn every_cached_view_that_read_an_entity_draws_again_on_its_change(cx: &mut TestAppContext) {
    let (count, readers, cx) = readers(cx);
    for n in 1..=3 {
        assert_eq!(bump(&count, &readers, n, cx), [1, 1, 1], "change {n}: parent, left, right");
        for name in ["parent", "left", "right"] {
            assert!(cx.debug_bounds(leaked(format!("{name}:{n}"))).is_some(), "change {n}: {name} shows it");
        }
    }
}

/// Reads the count outside every cached view, around a view drawn as if uncached on all but in-place frames.
struct Top {
    count: Entity<Count>,
    holder: Entity<Holder>,
    renders: usize,
}

impl Render for Top {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.renders += 1;
        let count = self.count.read(cx).0;
        div()
            .size_full()
            .child(div().debug_selector(move || format!("top:{count}")).w(px(10.)).h(px(4.)))
            .child(gpui_kit::AnyView::from(self.holder.clone()).cached_for_in_place(full()))
    }
}

struct Holder {
    inner: Entity<Reader>,
    renders: usize,
}

impl Render for Holder {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.renders += 1;
        div().size_full().child(self.inner.clone().cached(full()))
    }
}

/// An entity read by content drawn outside every cached view, which every frame draws again, draws no whole frame:
/// the view drawn as if uncached between in-place frames keeps its output, and only the cached view that read it draws.
#[gpui_kit::test]
fn an_entity_read_outside_every_cached_view_draws_no_whole_frame(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let count = cx.new(|_| Count(0));
    let inner = cx.new(|_| Reader { name: "inner", count: count.clone(), inner: Vec::new(), renders: 0 });
    let holder = cx.new(|_| Holder { inner: inner.clone(), renders: 0 });
    let top_count = count.clone();
    let top_holder = holder.clone();
    let (top, cx) = cx.add_window_view(|_, _| Top { count: top_count, holder: top_holder, renders: 0 });
    cx.simulate_resize(size(px(400.), px(300.)));
    cx.run_until_parked();
    let renders = |cx: &mut VisualTestContext| cx.read(|cx| [top.read(cx).renders, holder.read(cx).renders, inner.read(cx).renders]);
    let before = renders(cx);
    count.update(cx, |count, cx| {
        count.0 = 1;
        cx.notify();
    });
    cx.run_until_parked();
    let after = renders(cx);
    assert_eq!([after[0] - before[0], after[1] - before[1], after[2] - before[2]], [1, 0, 1], "top, holder, inner");
    assert!(cx.debug_bounds("top:1").is_some() && cx.debug_bounds("inner:1").is_some());
}

/// Reads the count only while painting, once `reads` is set, and keeps what it read.
struct Painter {
    count: Entity<Count>,
    reads: bool,
    seen: std::rc::Rc<std::cell::Cell<usize>>,
}

impl Render for Painter {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let (count, reads, seen) = (self.count.clone(), self.reads, self.seen.clone());
        gpui_kit::canvas(
            |_, _, _| (),
            move |_, _, _, cx| {
                if reads {
                    seen.set(count.read(cx).0)
                }
            },
        )
        .size_full()
    }
}

struct Pair {
    painter: gpui_kit::AnyView,
    leaf: Entity<Leaf>,
}

impl Render for Pair {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .child(div().w(px(100.)).h(px(40.)).child(self.painter.clone().cached(full())))
            .child(div().w(px(100.)).h(px(40.)).child(self.leaf.clone().cached(full())))
    }
}

struct Wrap {
    pair: Entity<Pair>,
}

impl Render for Wrap {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(self.pair.clone().cached(full()))
    }
}

/// What a view drawn again in place read while painting stays read by the view around it: reused whole on a later
/// frame, that view still holds it, so the window still hears the entity change.
#[gpui_kit::test]
fn an_entity_first_read_in_an_in_place_paint_is_still_heard_after_its_parent_is_reused(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let count = cx.new(|_| Count(5));
    let seen = std::rc::Rc::new(std::cell::Cell::new(0));
    let painter = cx.new(|_| Painter { count: count.clone(), reads: false, seen: seen.clone() });
    let leaf = cx.new(|_| Leaf::default());
    let pair = cx.new(|_| Pair { painter: painter.clone().into(), leaf: leaf.clone() });
    let wrap = cx.new(|_| Wrap { pair });
    let (_, cx) = cx.add_window_view(|window, cx| Root::new(wrap, window, cx).cache_view());
    cx.simulate_resize(size(px(400.), px(300.)));
    cx.run_until_parked();
    painter.update(cx, |painter, cx| {
        painter.reads = true;
        cx.notify_in_place();
    });
    cx.run_until_parked();
    assert_eq!(seen.get(), 5, "drawn in place, it read the count while painting");
    // A frame that draws only the leaf beside it reuses the pair whole.
    leaf.update(cx, |leaf, cx| {
        leaf.n = 1;
        cx.notify_in_place();
    });
    cx.run_until_parked();
    count.update(cx, |count, cx| {
        count.0 = 7;
        cx.notify();
    });
    cx.run_until_parked();
    assert_eq!(seen.get(), 7, "the count's change drew the painter again");
}

/// A field's text, drawn by no view of its own: its field's view reads it, and so does the window's input handler
/// after every frame.
struct Typed(usize);

impl gpui_kit::EntityInputHandler for Typed {
    fn text_for_range(
        &mut self,
        _: std::ops::Range<usize>,
        _: &mut Option<std::ops::Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        None
    }

    fn selected_text_range(&mut self, _: bool, _: &mut Window, _: &mut Context<Self>) -> Option<gpui_kit::UTF16Selection> {
        None
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<std::ops::Range<usize>> {
        None
    }

    fn unmark_text(&mut self, _: &mut Window, _: &mut Context<Self>) {}

    fn replace_text_in_range(&mut self, _: Option<std::ops::Range<usize>>, _: &str, _: &mut Window, _: &mut Context<Self>) {}

    fn replace_and_mark_text_in_range(
        &mut self,
        _: Option<std::ops::Range<usize>>,
        _: &str,
        _: Option<std::ops::Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) {
    }

    fn bounds_for_range(
        &mut self,
        _: std::ops::Range<usize>,
        _: gpui_kit::Bounds<gpui_kit::Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<gpui_kit::Bounds<gpui_kit::Pixels>> {
        None
    }

    fn character_index_for_point(&mut self, _: gpui_kit::Point<gpui_kit::Pixels>, _: &mut Window, _: &mut Context<Self>) -> Option<usize> {
        None
    }
}

struct TypedField {
    typed: Entity<Typed>,
    focus: gpui_kit::FocusHandle,
    renders: usize,
}

impl Render for TypedField {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.renders += 1;
        let n = self.typed.read(cx).0;
        let (typed, focus) = (self.typed.clone(), self.focus.clone());
        div().size_full().track_focus(&self.focus).child(div().debug_selector(move || format!("typed:{n}")).w(px(10.)).h(px(4.))).child(
            gpui_kit::canvas(
                |_, _, _| (),
                move |bounds, _, window, cx| window.handle_input(&focus, gpui_kit::ElementInputHandler::new(bounds, typed), cx),
            )
            .size_full(),
        )
    }
}

/// Draws `inner` cached and reads nothing itself.
struct Around {
    inner: gpui_kit::AnyView,
    renders: usize,
}

impl Render for Around {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.renders += 1;
        div().size_full().child(self.inner.clone().cached(full()))
    }
}

/// A focused field's text read by its cached view and by the input handler after the frame draws its view alone:
/// what the window reads after drawing is not content drawn outside every cached view.
#[gpui_kit::test]
fn a_change_a_focused_fields_input_handler_reads_draws_its_view_alone(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let typed = cx.new(|_| Typed(0));
    let focus = cx.update(|cx| cx.focus_handle());
    let field = cx.new(|_| TypedField { typed: typed.clone(), focus: focus.clone(), renders: 0 });
    let holder = cx.new(|_| Around { inner: field.clone().into(), renders: 0 });
    let root = holder.clone();
    let (_, cx) = cx.add_window_view(|window, cx| Root::new(root, window, cx).cache_view());
    cx.simulate_resize(size(px(400.), px(300.)));
    cx.update(|window, cx| {
        window.activate_window();
        focus.focus(window, cx);
    });
    cx.run_until_parked();
    for n in 1..=3 {
        let before = cx.read(|cx| (holder.read(cx).renders, field.read(cx).renders));
        typed.update(cx, |typed, cx| {
            typed.0 = n;
            cx.notify();
        });
        cx.run_until_parked();
        let after = cx.read(|cx| (holder.read(cx).renders, field.read(cx).renders));
        assert_eq!((after.0 - before.0, after.1 - before.1), (0, 1), "change {n}: holder, field");
        assert!(cx.debug_bounds(leaked(format!("typed:{n}"))).is_some(), "change {n}: the field shows it");
    }
}

/// Reads the count while `reads` is set.
struct Maybe {
    count: Entity<Count>,
    reads: bool,
    renders: usize,
}

impl Render for Maybe {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.renders += 1;
        let n = if self.reads { self.count.read(cx).0 } else { 0 };
        div().debug_selector(move || format!("maybe:{n}")).w(px(10.)).h(px(4.))
    }
}

/// A view drawn again in place that no longer reads an entity leaves no view around it holding it: its change then
/// draws nothing, where a record that only grew drew the frame whole.
#[gpui_kit::test]
fn an_entity_a_view_stopped_reading_in_place_draws_nothing(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let count = cx.new(|_| Count(0));
    let maybe = cx.new(|_| Maybe { count: count.clone(), reads: true, renders: 0 });
    let parent = cx.new(|_| Around { inner: maybe.clone().into(), renders: 0 });
    let holder = cx.new(|_| Around { inner: parent.clone().into(), renders: 0 });
    let root = holder.clone();
    let (_, cx) = cx.add_window_view(|window, cx| Root::new(root, window, cx).cache_view());
    cx.simulate_resize(size(px(400.), px(300.)));
    cx.run_until_parked();
    maybe.update(cx, |maybe, cx| {
        maybe.reads = false;
        cx.notify_in_place();
    });
    cx.run_until_parked();
    let renders = |cx: &mut VisualTestContext| cx.read(|cx| [holder.read(cx).renders, parent.read(cx).renders, maybe.read(cx).renders]);
    let before = renders(cx);
    count.update(cx, |count, cx| {
        count.0 = 1;
        cx.notify();
    });
    cx.run_until_parked();
    let after = renders(cx);
    assert_eq!([after[0] - before[0], after[1] - before[1], after[2] - before[2]], [0, 0, 0], "holder, parent, maybe");
}

/// Reads the count only in a deferred element's paint, as a popover does.
struct Popover {
    count: Entity<Count>,
    seen: std::rc::Rc<std::cell::Cell<usize>>,
}

impl Render for Popover {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let (count, seen) = (self.count.clone(), self.seen.clone());
        div()
            .size_full()
            .child(gpui_kit::deferred(gpui_kit::canvas(|_, _, _| (), move |_, _, _, cx| seen.set(count.read(cx).0)).size_full()))
    }
}

/// What a deferred element read stays heard while its owner's output is reused, and its change draws the owner again.
#[gpui_kit::test]
fn an_entity_a_deferred_element_read_is_heard_while_its_owner_is_reused(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let count = cx.new(|_| Count(5));
    let seen = std::rc::Rc::new(std::cell::Cell::new(0));
    let popover = cx.new(|_| Popover { count: count.clone(), seen: seen.clone() });
    let leaf = cx.new(|_| Leaf::default());
    let pair = cx.new(|_| Pair { painter: popover.clone().into(), leaf: leaf.clone() });
    let wrap = cx.new(|_| Wrap { pair });
    let (_, cx) = cx.add_window_view(|window, cx| Root::new(wrap, window, cx).cache_view());
    cx.simulate_resize(size(px(400.), px(300.)));
    cx.run_until_parked();
    assert_eq!(seen.get(), 5, "the deferred element painted the count");
    leaf.update(cx, |leaf, cx| {
        leaf.n = 1;
        cx.notify_in_place();
    });
    cx.run_until_parked();
    count.update(cx, |count, cx| {
        count.0 = 7;
        cx.notify();
    });
    cx.run_until_parked();
    assert_eq!(seen.get(), 7, "the count's change drew the popover again");
}

/// Shows its count, drawn cached inside a deferred element.
#[derive(Default)]
struct Ticker {
    n: usize,
}

impl Render for Ticker {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let n = self.n;
        div().debug_selector(move || format!("ticker:{n}")).w(px(10.)).h(px(4.))
    }
}

/// Draws `ticker` cached inside a deferred element, as the sidebar's hover card draws its time and crab.
struct Card {
    ticker: Entity<Ticker>,
}

impl Render for Card {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(gpui_kit::deferred(div().w(px(50.)).h(px(20.)).child(self.ticker.clone().cached(full()))))
    }
}

/// A cached view inside a deferred element keeps drawing its steps after a frame reuses the element's owner: the window
/// still hears it, and a step draws its owner again.
#[gpui_kit::test]
fn a_cached_view_in_a_deferred_element_steps_after_its_owner_is_reused(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let ticker = cx.new(|_| Ticker::default());
    let card = cx.new(|_| Card { ticker: ticker.clone() });
    let leaf = cx.new(|_| Leaf::default());
    let pair = cx.new(|_| Pair { painter: card.into(), leaf: leaf.clone() });
    let wrap = cx.new(|_| Wrap { pair });
    let (_, cx) = cx.add_window_view(|window, cx| Root::new(wrap, window, cx).cache_view());
    cx.simulate_resize(size(px(400.), px(300.)));
    cx.run_until_parked();
    assert!(cx.debug_bounds("ticker:0").is_some(), "the ticker is drawn");
    leaf.update(cx, |leaf, cx| {
        leaf.n = 1;
        cx.notify_in_place();
    });
    cx.run_until_parked();
    for n in 1..=3 {
        ticker.update(cx, |ticker, cx| {
            ticker.n = n;
            cx.notify_in_place();
        });
        cx.run_until_parked();
        assert!(cx.debug_bounds(leaked(format!("ticker:{n}"))).is_some(), "step {n} drew the ticker");
    }
}

/// Labels itself with the count, drawn by no cached view.
struct Labelled {
    count: Entity<Count>,
}

impl Render for Labelled {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let n = self.count.read(cx).0;
        div().size_full().child(div().id("count").role(gpui_kit::Role::Image).aria_label(format!("count {n}")).w(px(10.)).h(px(4.)))
    }
}

/// A change only content outside every cached view read sends a screen reader the tree with it, as a whole frame did.
#[gpui_kit::test]
fn a_change_read_outside_every_cached_view_sends_the_tree_again(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let count = cx.new(|_| Count(0));
    let counted = count.clone();
    let (_, cx) = cx.add_window_view(|_, _| Labelled { count: counted });
    cx.simulate_resize(size(px(400.), px(300.)));
    cx.update(|window, _| window.simulate_a11y_active(true));
    cx.run_until_parked();
    let labels = |cx: &mut VisualTestContext| -> String { cx.update(|window, _| window.debug_a11y_tree_json()).unwrap_or_default() };
    assert!(labels(cx).contains("count 0"), "the first tree: {}", labels(cx));
    count.update(cx, |count, cx| {
        count.0 = 1;
        cx.notify();
    });
    cx.run_until_parked();
    let tree = labels(cx);
    assert!(tree.contains("count 1") && !tree.contains("count 0"), "the tree after the change: {tree}");
}
