// SPDX-License-Identifier: AGPL-3.0-only
//! The page as Settings draws it: a view of its own over a list of its entries that lays out only those on screen, so
//! a scroll step draws the list alone, where it stands, and makes only the rows it shows.

use std::rc::Rc;

use gpui_kit::{div, list, prelude::*, px, App, Context, Entity, ListAlignment, ListState, StyleRefinement, Window};

use super::{Entry, Page};
use crate::screens::settings::frame;
use crate::ui::{narrow, scrollbar};

/// A row of the list before the list has laid it out: a figure row's height and its rule.
const ROW: f32 = 41.;
/// How far past each edge of the page the list lays out what is coming into view.
const OVERDRAW: f32 = 100.;

/// The page, drawn cached where Settings stands it.
pub struct ActivityPage {
    page: Option<Page>,
    list: Entity<Entries>,
}

impl ActivityPage {
    pub fn new(cx: &mut App) -> Self {
        let state = ListState::new(0, ListAlignment::Top, px(OVERDRAW)).scrolls_in_place().with_uniform_item_height(px(ROW));
        ActivityPage { page: None, list: cx.new(|_| Entries { state, entries: Rc::default(), narrow: false }) }
    }

    /// The list the page scrolls.
    pub fn list(&self, cx: &App) -> ListState {
        self.list.read(cx).state.clone()
    }

    /// What Settings draws the page from, laid on as Settings draws.
    pub fn set(&mut self, page: Page) {
        self.page = Some(page);
    }
}

impl Render for ActivityPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        crate::frames::count("ActivityPage");
        let (entries, menu) = self.page.clone().map(|page| page.entries(window, cx)).unwrap_or_default();
        let narrow = narrow::width(window).sm;
        self.list.update(cx, |list, _| list.set(entries, narrow));
        div().size_full().relative().child(self.list.clone().cached(StyleRefinement::default().size_full())).children(menu)
    }
}

/// The page's scrolling list of its entries.
struct Entries {
    state: ListState,
    entries: Rc<Vec<Entry>>,
    narrow: bool,
}

impl Entries {
    fn set(&mut self, entries: Vec<Entry>, narrow: bool) {
        let (was, count) = (self.state.item_count(), entries.len());
        if count > was {
            self.state.splice(was..was, count - was);
        } else if count < was {
            self.state.splice(count..was, 0);
        }
        // Any entry may have changed its height; each is laid out again as it shows, the offset kept.
        self.state.remeasure_items(0..count);
        self.entries = Rc::new(entries);
        self.narrow = narrow;
    }
}

impl Render for Entries {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        crate::frames::count("ActivityList");
        let (entries, narrow) = (self.entries.clone(), self.narrow);
        let last = entries.len().saturating_sub(1);
        let page = list(self.state.clone(), move |ix, window, cx| {
            frame::locked_entry(entries[ix](window, cx), ix == 0, ix == last, narrow).into_any_element()
        })
        .size_full();
        scrollbar::with_bar("settings-page", &self.state, page, window, cx).size_full()
    }
}
