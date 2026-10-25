// SPDX-License-Identifier: AGPL-3.0-only
//! A page as data, after `rows.tsx`'s `SettingsCardData`: each card's head, lede and items, every item with the words
//! the search reads, so one function per page serves the page, the search page and the sidebar's trail.

use std::rc::Rc;

use gpui_kit::{AnyElement, App, Entity, IntoElement, SharedString, Window};

use super::model::{At, Data};
use super::pages::activity::Pick;
use super::pages::agents::Store;
use super::pages::computers::RowMenu;
use super::Acts;
use crate::ui::{Card, CardItem, Line, Row};

/// What every page is drawn from: the host's records, the page Settings stands on, and the acts a row raises.
pub struct Ctx<'a> {
    pub data: &'a Data,
    pub at: &'a At,
    /// The page standing in place of the open group's list, where one does.
    pub level: Option<&'a Level>,
    pub acts: Acts,
    /// Activity's view and computer, which its tabs in the top bar and its page share.
    pub activity: Entity<Pick>,
    /// The Agents pages' tab, computer and reports, which a computer's page picks before it opens them.
    pub agents: Entity<Store>,
    /// The Computers rows' one menu, held by the screen so what it last asked for can be read.
    pub computers_menu: Entity<RowMenu>,
}

impl<'a> Ctx<'a> {
    pub fn new(
        data: &'a Data,
        at: &'a At,
        level: Option<&'a Level>,
        acts: Acts,
        activity: Entity<Pick>,
        agents: Entity<Store>,
        computers_menu: Entity<RowMenu>,
    ) -> Self {
        Ctx { data, at, level, acts, activity, agents, computers_menu }
    }
}

/// A page under a group's list (`AgentsLevel`): its key and its name for the crumb, and where the crumb's back goes.
#[derive(Clone, Debug, PartialEq)]
pub struct Level {
    pub key: SharedString,
    pub name: SharedString,
    pub up: Option<Box<Level>>,
}

/// One item of a card: its id, the words a search finds it by (its title or label first), and how it draws.
pub struct Item {
    pub id: SharedString,
    pub words: Vec<SharedString>,
    pub el: CardItem,
}

impl Item {
    /// A row, found by its title and its sentence.
    pub fn row(id: impl Into<SharedString>, title: impl Into<SharedString>, note: Option<&str>, build: impl FnOnce(Row) -> Row) -> Item {
        let (id, title) = (id.into(), title.into());
        let mut words = vec![title.clone()];
        let mut row = Row::new(id.clone(), title);
        if let Some(note) = note {
            words.push(SharedString::from(note.to_owned()));
            row = row.note(note.to_owned());
        }
        Item { id, words, el: build(row).into() }
    }

    /// A line, found by its label and its hover sentence.
    pub fn line(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        hover: Option<&str>,
        build: impl FnOnce(Line) -> Line,
    ) -> Item {
        let (id, label) = (id.into(), label.into());
        let mut words = vec![label.clone()];
        words.extend(hover.map(|h| SharedString::from(h.to_owned())));
        Item { id: id.clone(), words, el: build(Line::new(id, label)).into() }
    }

    /// The card's one empty line: what is not there yet, in the quiet note.
    pub fn empty(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Item {
        Item::line(id, label, None, Line::empty)
    }

    /// Anything else a card holds, found by the words given.
    pub fn any(id: impl Into<SharedString>, words: Vec<SharedString>, el: impl Into<CardItem>) -> Item {
        Item { id: id.into(), words, el: el.into() }
    }

    pub fn matches(&self, query: &str) -> bool {
        let q = fold(query);
        !q.is_empty() && self.words.iter().any(|w| fold(w).contains(&q))
    }
}

/// `normalizeSearchText`: case and runs of space aside.
pub fn fold(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

/// What a press on a card's head does.
pub type Open = Rc<dyn Fn(&mut Window, &mut App)>;

/// One card of a page.
pub struct CardData {
    pub id: SharedString,
    pub head: Option<SharedString>,
    pub lede: Option<SharedString>,
    pub items: Vec<Item>,
    /// A control too large for a row, under the head and over the surface.
    pub body: Option<AnyElement>,
    /// The act the card invites, under it.
    pub under: Vec<AnyElement>,
    /// The rows a search finds in place of the items, for a card whose body draws them its own way.
    pub search: Option<Vec<Item>>,
    /// A head that opens a page: each group's name on the results page.
    pub open: Option<Open>,
}

impl CardData {
    pub fn new(id: impl Into<SharedString>) -> Self {
        CardData { id: id.into(), head: None, lede: None, items: Vec::new(), body: None, under: Vec::new(), search: None, open: None }
    }

    pub fn head(mut self, head: impl Into<SharedString>) -> Self {
        self.head = Some(head.into());
        self
    }

    pub fn lede(mut self, lede: impl Into<SharedString>) -> Self {
        self.lede = Some(lede.into());
        self
    }

    pub fn item(mut self, item: Item) -> Self {
        self.items.push(item);
        self
    }

    pub fn items(mut self, items: impl IntoIterator<Item = Item>) -> Self {
        self.items.extend(items);
        self
    }

    pub fn body(mut self, body: impl IntoElement) -> Self {
        self.body = Some(body.into_any_element());
        self
    }

    pub fn under(mut self, el: impl IntoElement) -> Self {
        self.under.push(el.into_any_element());
        self
    }

    pub fn on_head_click(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.open = Some(Rc::new(f));
        self
    }

    pub fn search(mut self, items: Vec<Item>) -> Self {
        self.search = Some(items);
        self
    }

    /// The items a search for `query` finds on this card.
    pub fn found(self, query: &str) -> Vec<Item> {
        self.search.unwrap_or(self.items).into_iter().filter(|item| item.matches(query)).collect()
    }

    /// The card drawn: the head, the lede, the body, the surface of items and the act under it.
    pub fn render(self) -> Card {
        let mut card = Card::new(self.id);
        if let Some(head) = self.head {
            card = card.head(head);
        }
        if let Some(lede) = self.lede {
            card = card.lede(lede);
        }
        if let Some(body) = self.body {
            card = card.body(body);
        }
        if let Some(open) = self.open {
            card = card.on_head_click(move |_, window, cx| open(window, cx));
        }
        for under in self.under {
            card = card.under(under);
        }
        card.items(self.items.into_iter().map(|item| item.el))
    }
}

pub(crate) type Build = Rc<dyn Fn(&Ctx, &mut Window, &mut App) -> AnyElement>;

/// A dialog a page stands on the window's dialogs layer, built again each frame from the screen's records.
#[derive(Clone)]
pub struct DialogBuild {
    pub build: Build,
    /// A press on the scrim closes it.
    pub dismissible: bool,
}

impl DialogBuild {
    /// The web's `Dialog`, a sheet that edits a value: a press on the scrim closes it.
    pub fn dialog(build: impl Fn(&Ctx, &mut Window, &mut App) -> AnyElement + 'static) -> Self {
        DialogBuild { build: Rc::new(build), dismissible: true }
    }

    /// The web's `AlertDialog`, the one confirmation before something that does not come back: only its own buttons
    /// and Escape close it.
    pub fn alert(build: impl Fn(&Ctx, &mut Window, &mut App) -> AnyElement + 'static) -> Self {
        DialogBuild { build: Rc::new(build), dismissible: false }
    }
}
