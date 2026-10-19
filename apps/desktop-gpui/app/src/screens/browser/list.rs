// SPDX-License-Identifier: AGPL-3.0-only
//! The servers list a browser tab shows with no page (`PreviewEmptyState.tsx` and its cards): the recents over the
//! listening ports, each row on the window glyph (`BrowserMockup.tsx`), filtered by what is typed in the address bar.

use std::rc::Rc;
use std::sync::Arc;

use gpui_kit::component::{h_flex, v_flex, ActiveTheme as _};
use gpui_kit::{
    div, prelude::*, px, relative, AnyElement, App, Div, ElementId, FontFeatures, Hsla, MouseButton, Pixels, SharedString, Window,
};

use super::bars::empty_sentence;
use super::words::{PREVIEW_EMPTY, PREVIEW_NO_MATCH};
use crate::ui::focus::ring_inset;
use crate::ui::{motion, size, ActivePalette, Button, Glyph};

/// `max-w-xl`: the list's column.
pub const COLUMN_WIDTH: Pixels = px(576.);

pub type OnUrl = Rc<dyn Fn(String, &mut Window, &mut App)>;

/// A place opened before: its url, the label the row shows for it (host, path, query and hash), the page's title
/// where one was read, and how long ago it was visited ("just now", "2m ago").
#[derive(Clone, Debug, PartialEq)]
pub struct RecentRow {
    pub url: SharedString,
    pub label: SharedString,
    pub title: Option<SharedString>,
    pub visited: SharedString,
}

/// A port listening on the thread's computer: its name (the process holding it, or "Listening" where the daemon read
/// none), its address ("localhost:3000") and the url a press opens.
#[derive(Clone, Debug, PartialEq)]
pub struct ServerRow {
    pub name: SharedString,
    pub address: SharedString,
    pub url: SharedString,
}

/// The servers list, given the rows the address bar's text leaves: the recents (the first eight), then the servers.
/// With both empty it says the empty sentence, or the no-match line when `filtered`, the text having hidden every row
/// there was. `behind` says why no server can show, in the empty sentence's place and over any list.
#[derive(IntoElement)]
pub struct ServerList {
    pub recents: Vec<RecentRow>,
    pub servers: Vec<ServerRow>,
    pub behind: Option<SharedString>,
    pub filtered: bool,
    pub on_open: OnUrl,
    pub on_remove: OnUrl,
}

fn tabular() -> FontFeatures {
    FontFeatures(Arc::new(vec![("tnum".into(), 1)]))
}

/// `BrowserMockup`: a 28 px window, three 3 px lights and two 2 px lines on the card.
pub fn window_glyph(cx: &App) -> Div {
    let p = cx.palette();
    let light = |ink: Hsla| div().flex_none().size(px(3.)).rounded_full().bg(ink.opacity(0.8));
    let line = |width, ink: Hsla| div().h(px(2.)).w(width).rounded_full().bg(ink);
    v_flex()
        .debug_selector(|| "browser-window-glyph".into())
        .flex_none()
        .size(px(28.))
        .gap(px(2.))
        .overflow_hidden()
        .rounded(px(5.))
        .border_1()
        .border_color(p.border.opacity(0.6))
        .bg(p.card)
        .shadow(p.control_shadow.clone())
        .p_1()
        .child(h_flex().gap(px(2.)).child(light(p.destructive)).child(light(p.warning)).child(light(p.success)))
        .child(
            v_flex()
                .mt(px(1.))
                .flex_1()
                .gap(px(2.))
                .child(line(relative(1.), p.muted_foreground.opacity(0.3)))
                .child(line(relative(0.6), p.muted_foreground.opacity(0.2))),
        )
}

fn group(heading: &'static str, rows: Vec<AnyElement>, cx: &App) -> impl IntoElement {
    let p = cx.palette();
    v_flex()
        .gap_2()
        .child(div().px_1().text_size(px(12.)).line_height(px(16.)).text_color(p.muted_foreground).child(heading))
        .child(v_flex().mx(px(-4.)).gap(px(2.)).children(rows))
}

fn line(text: SharedString, selector: &'static str, cx: &App) -> impl IntoElement {
    div()
        .debug_selector(move || selector.into())
        .px_1()
        .text_size(px(13.))
        .line_height(px(20.))
        .text_color(cx.palette().muted_foreground)
        .child(text)
}

fn recent_row(row: &RecentRow, on_open: OnUrl, on_remove: OnUrl, window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.palette().clone();
    let key = format!("browser-recent-{}", row.url);
    let id = ElementId::Name(key.clone().into());
    let row_over = motion::hovered(id.clone(), window, cx);
    let open_id = ElementId::Name(format!("{key}-open").into());
    let open_over = motion::hovered(open_id.clone(), window, cx);
    let x_id = ElementId::Name(format!("{key}-remove").into());
    let x_over = motion::hovered(x_id.clone(), window, cx);
    let shown = *row_over.read(cx);
    // The X stands over the press target, which loses its hover under it as the web's sibling button does.
    let over = *open_over.read(cx) && !*x_over.read(cx);
    let mono = cx.theme().mono_font_family.clone();
    let title = match &row.title {
        Some(title) => div().text_size(px(14.)).line_height(px(20.)).child(title.clone()),
        None => div().font_family(mono.clone()).text_size(px(13.)).line_height(px(19.5)).child(row.label.clone()),
    };
    let under = h_flex()
        .min_w_0()
        .text_size(px(12.))
        .line_height(px(16.))
        .text_color(p.muted_foreground)
        .when_some(row.title.as_ref(), |el, _| el.child(div().min_w_0().truncate().font_family(mono).child(row.label.clone())))
        .child(div().flex_none().when(row.title.is_some(), |el| el.ml_2()).child(row.visited.clone()));
    let fill = motion::ease_ink(open_id.clone(), "fill", if over { p.accent } else { p.accent.opacity(0.) }, motion::COLORS, window, cx);
    let url = row.url.clone();
    let body = h_flex()
        .id(open_id)
        .debug_selector({
            let key = key.clone();
            move || key
        })
        .w_full()
        .gap_3()
        .rounded(size::BUTTON_RADIUS)
        .p_2()
        .pr(px(40.))
        .bg(fill)
        .cursor_pointer()
        .tab_index(0)
        .focus_visible({
            let ring = p.ring;
            move |s| s.shadow(vec![ring_inset(ring)])
        })
        .on_hover(motion::track_hover(open_over))
        .on_click({
            let url = url.clone();
            move |_, window, cx| on_open(url.to_string(), window, cx)
        })
        .child(window_glyph(cx))
        .child(v_flex().flex_1().min_w_0().text_color(p.foreground).child(title.truncate()).child(under));
    let remove = Button::icon(SharedString::from(format!("{key}-x")), Glyph::X).on_click({
        let url = url.clone();
        move |_, window, cx| on_remove(url.to_string(), window, cx)
    });
    div()
        .id(id)
        .relative()
        .flex()
        .w_full()
        .items_center()
        .on_hover(motion::track_hover(row_over))
        .child(body)
        .child(
            div()
                .id(x_id)
                .debug_selector({
                    let key = key.clone();
                    move || format!("{key}-remove")
                })
                .absolute()
                .right(px(8.))
                .opacity(if shown { 1. } else { 0. })
                .on_hover(motion::track_hover(x_over))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(remove),
        )
        .into_any_element()
}

fn server_row(row: &ServerRow, on_open: OnUrl, window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.palette().clone();
    let key = format!("browser-server-{}", row.address);
    let id = ElementId::Name(key.clone().into());
    let over = motion::hovered(id.clone(), window, cx);
    let fill =
        motion::ease_ink(id.clone(), "fill", if *over.read(cx) { p.accent } else { p.accent.opacity(0.) }, motion::COLORS, window, cx);
    let url = row.url.clone();
    h_flex()
        .id(id)
        .debug_selector(move || key)
        .w_full()
        .gap_3()
        .rounded(size::BUTTON_RADIUS)
        .p_2()
        .bg(fill)
        .cursor_pointer()
        .tab_index(0)
        .focus_visible({
            let ring = p.ring;
            move |s| s.shadow(vec![ring_inset(ring)])
        })
        .on_hover(motion::track_hover(over))
        .on_click(move |_, window, cx| on_open(url.to_string(), window, cx))
        .child(window_glyph(cx))
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .child(div().truncate().text_size(px(14.)).line_height(px(20.)).text_color(p.foreground).child(row.name.clone()))
                .child(
                    div()
                        .truncate()
                        .font_family(cx.theme().mono_font_family.clone())
                        .font_features(tabular())
                        .text_size(px(12.))
                        .line_height(px(16.))
                        .text_color(p.muted_foreground)
                        .child(row.address.clone()),
                ),
        )
        .into_any_element()
}

impl RenderOnce for ServerList {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let ServerList { recents, servers, behind, filtered, on_open, on_remove } = self;
        if recents.is_empty() && servers.is_empty() && !filtered {
            return empty_sentence(behind.unwrap_or_else(|| PREVIEW_EMPTY.into()), None).into_any_element();
        }
        let none = recents.is_empty() && servers.is_empty();
        let recents: Vec<AnyElement> =
            recents.iter().take(super::recents::SHOWN_MAX).map(|r| recent_row(r, on_open.clone(), on_remove.clone(), window, cx)).collect();
        let servers: Vec<AnyElement> = servers.iter().map(|s| server_row(s, on_open.clone(), window, cx)).collect();
        div()
            .id("browser-list")
            .debug_selector(|| "browser-list".into())
            .flex()
            .size_full()
            .min_h_0()
            .overflow_y_scroll()
            .px_5()
            .py_8()
            .child(
                v_flex()
                    .debug_selector(|| "browser-list-column".into())
                    .mx_auto()
                    .w_full()
                    .max_w(COLUMN_WIDTH)
                    .gap_6()
                    .children(behind.map(|b| line(b, "browser-list-behind", cx)))
                    .when(none, |el| el.child(line(PREVIEW_NO_MATCH.into(), "browser-list-no-match", cx)))
                    .when(!recents.is_empty(), |el| el.child(group("Recently used", recents, cx)))
                    .when(!servers.is_empty(), |el| el.child(group("Local servers", servers, cx))),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use gpui_kit::component::Root;
    use gpui_kit::{div, px, size, Context, Modifiers, TestAppContext, VisualTestContext, Window};

    use super::*;
    use crate::ui::{self, Side};

    fn recent(url: &str, label: &str, title: Option<&str>) -> RecentRow {
        RecentRow {
            url: url.to_owned().into(),
            label: label.to_owned().into(),
            title: title.map(|t| t.to_owned().into()),
            visited: "2m ago".into(),
        }
    }

    fn server(name: &str, port: u16) -> ServerRow {
        ServerRow {
            name: name.to_owned().into(),
            address: format!("localhost:{port}").into(),
            url: format!("http://localhost:{port}/").into(),
        }
    }

    type Log = Rc<RefCell<Vec<String>>>;

    struct List {
        recents: Vec<RecentRow>,
        servers: Vec<ServerRow>,
        filtered: bool,
        behind: Option<SharedString>,
        opened: Log,
        removed: Log,
    }

    impl Render for List {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let (opened, removed) = (self.opened.clone(), self.removed.clone());
            div().size_full().flex().flex_col().child(ServerList {
                recents: self.recents.clone(),
                servers: self.servers.clone(),
                behind: self.behind.clone(),
                filtered: self.filtered,
                on_open: Rc::new(move |url, _, _| opened.borrow_mut().push(url)),
                on_remove: Rc::new(move |url, _, _| removed.borrow_mut().push(url)),
            })
        }
    }

    fn open(cx: &mut TestAppContext, width: f32, list: List) -> &mut VisualTestContext {
        cx.update(|cx| {
            gpui_kit::init(cx);
            ui::init(Side::Light, cx);
            cx.set_global(ui::Still(true));
        });
        let view = cx.new(|_| list);
        let (_, cx) = cx.add_window_view(|window, cx| Root::new(view, window, cx));
        cx.simulate_resize(size(px(width), px(600.)));
        cx.run_until_parked();
        cx
    }

    fn list(recents: Vec<RecentRow>, servers: Vec<ServerRow>, filtered: bool) -> (List, Log, Log) {
        let (opened, removed): (Log, Log) = Default::default();
        (List { recents, servers, filtered, behind: None, opened: opened.clone(), removed: removed.clone() }, opened, removed)
    }

    fn both() -> (Vec<RecentRow>, Vec<ServerRow>) {
        (
            vec![recent("http://localhost:47803/settings", "localhost:47803/settings", Some("Parity settings"))],
            vec![server("node", 3000), server("Listening", 5173)],
        )
    }

    #[gpui_kit::test]
    fn with_nothing_opened_and_nothing_listening_it_says_the_empty_sentence(cx: &mut TestAppContext) {
        let (l, _, _) = list(vec![], vec![], false);
        let cx = open(cx, 480., l);
        assert!(cx.debug_bounds("browser-empty").is_some(), "the empty sentence stands in the list's place");
        assert!(cx.debug_bounds("browser-list").is_none());
    }

    #[gpui_kit::test]
    fn a_filter_that_hides_every_row_reads_the_no_match_line(cx: &mut TestAppContext) {
        let (l, _, _) = list(vec![], vec![], true);
        let cx = open(cx, 700., l);
        let (line, column) = (
            cx.debug_bounds("browser-list-no-match").expect("No address or server matches."),
            cx.debug_bounds("browser-list-column").unwrap(),
        );
        assert_eq!((line.top(), line.size.height), (column.top(), px(20.)), "one 13 px line at the column's top");
        assert!(cx.debug_bounds("browser-empty").is_none(), "the list stands, with only its line");
    }

    #[gpui_kit::test]
    fn the_column_is_576_wide_on_20_by_32_px(cx: &mut TestAppContext) {
        let (recents, servers) = both();
        let (l, _, _) = list(recents, servers, false);
        let cx = open(cx, 900., l);
        let (outer, column) = (cx.debug_bounds("browser-list").unwrap(), cx.debug_bounds("browser-list-column").unwrap());
        assert_eq!(column.size.width, COLUMN_WIDTH);
        assert_eq!(column.center().x, outer.center().x, "centred in the pane");
        assert_eq!(column.top() - outer.top(), px(32.));
        assert!(cx.debug_bounds("browser-list-no-match").is_none());
        cx.simulate_resize(size(px(400.), px(600.)));
        cx.run_until_parked();
        let (outer, column) = (cx.debug_bounds("browser-list").unwrap(), cx.debug_bounds("browser-list-column").unwrap());
        assert_eq!(column.left() - outer.left(), px(20.), "a narrow pane keeps 20 px each side");
        assert_eq!(column.size.width, px(360.));
    }

    #[gpui_kit::test]
    fn rows_stand_on_the_glyph_with_8_px_of_padding(cx: &mut TestAppContext) {
        let (recents, servers) = both();
        let (l, _, _) = list(recents, servers, false);
        let cx = open(cx, 700., l);
        let row = cx.debug_bounds("browser-server-localhost:3000").expect("the node row is drawn");
        let glyph = cx.debug_bounds("browser-window-glyph").expect("a glyph is drawn");
        assert_eq!(row.size.height, px(52.), "36 px of two lines on 8 px above and below");
        assert_eq!((glyph.size.width, glyph.size.height), (px(28.), px(28.)));
        let next = cx.debug_bounds("browser-server-localhost:5173").expect("the bare port's row is drawn");
        assert_eq!(next.top() - row.bottom(), px(2.), "rows 2 px apart");
        let column = cx.debug_bounds("browser-list-column").unwrap();
        assert_eq!(column.left() - row.left(), px(4.), "rows run 4 px past the column, as the headings stand 4 px in");
    }

    #[gpui_kit::test]
    fn the_list_shows_eight_recents_at_most(cx: &mut TestAppContext) {
        let recents: Vec<RecentRow> = (0..10).map(|i| recent(&format!("http://localhost:{}/", 4000 + i), "x", None)).collect();
        let (l, _, _) = list(recents, vec![], false);
        let cx = open(cx, 700., l);
        assert!(cx.debug_bounds("browser-recent-http://localhost:4007/").is_some());
        assert!(cx.debug_bounds("browser-recent-http://localhost:4008/").is_none(), "the ninth is not shown");
    }

    #[gpui_kit::test]
    fn a_row_opens_and_its_x_removes(cx: &mut TestAppContext) {
        let (recents, servers) = both();
        let (l, opened, removed) = list(recents, servers, false);
        let cx = open(cx, 700., l);
        let row = cx.debug_bounds("browser-recent-http://localhost:47803/settings").unwrap();
        cx.simulate_mouse_move(row.center(), None, Modifiers::none());
        cx.run_until_parked();
        let x = cx.debug_bounds("browser-recent-http://localhost:47803/settings-remove").expect("the X is drawn");
        assert_eq!((x.size.width, row.right() - x.right()), (px(28.), px(8.)), "a 28 px ghost, 8 px from the right");
        cx.simulate_click(x.center(), Modifiers::none());
        cx.run_until_parked();
        assert_eq!(
            (opened.borrow().len(), removed.borrow().clone()),
            (0, vec!["http://localhost:47803/settings".to_string()]),
            "the X removes and opens nothing"
        );
        cx.simulate_click(row.center(), Modifiers::none());
        let server = cx.debug_bounds("browser-server-localhost:3000").unwrap();
        cx.simulate_click(server.center(), Modifiers::none());
        cx.run_until_parked();
        assert_eq!(*opened.borrow(), vec!["http://localhost:47803/settings".to_string(), "http://localhost:3000/".to_string()]);
    }

    #[gpui_kit::test]
    fn behind_stands_over_the_list(cx: &mut TestAppContext) {
        let (recents, servers) = both();
        let (mut l, _, _) = list(recents, servers, false);
        l.behind = Some("spoo is behind: no port watch; wsp add spoo --update puts this wsp's daemon on it".into());
        let cx = open(cx, 700., l);
        let (behind, column) = (cx.debug_bounds("browser-list-behind").unwrap(), cx.debug_bounds("browser-list-column").unwrap());
        assert_eq!(behind.top(), column.top(), "the behind line is the list's first line");
    }
}
