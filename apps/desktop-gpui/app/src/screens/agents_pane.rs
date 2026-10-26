// SPDX-License-Identifier: AGPL-3.0-only
//! The Agents pane of the right panel, after `components/agents/AgentsSurface.tsx` and `AgentsPanel.tsx`: the tabs
//! across the top, then Settings > Agents' own cards for the computer the thread runs on, this one where the panel has
//! no thread or the thread runs here. It reads through the agents store Settings keeps for the host and window, so a
//! report read in either is not asked again in the other. A row of the other tabs opens its page in place of the list,
//! with a back and the page's name in the tabs' place; an agent's row opens that agent's page in Settings.

use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::{
    div, point, prelude::*, px, AnyElement, App, Context, Entity, FocusHandle, FontWeight, KeyDownEvent, ScrollHandle, Subscription,
    WeakEntity, Window,
};
use serde_json::{json, Value};
use wsp_host_client::protocol::{PlaceView, Preferences};

use crate::screens::palette::overlays;
use crate::screens::settings::model::{At, Data, GroupId};
use crate::screens::settings::page::Level;
use crate::screens::settings::pages::activity::Pick;
use crate::screens::settings::pages::agents::{self, kinds, model::w, store, Rows, Store, Tab};
use crate::screens::settings::pages::computers::RowMenu;
use crate::screens::settings::words::HERE;
use crate::screens::settings::{Acts, Ctx, Nav};
use crate::screens::{navigate, Host, Opened, Pane, Place, Route, Screen};
use crate::sidebar::Sidebar;
use crate::ui::{scroll_area, slash, ActivePalette, Button, Glyph, Segmented};

pub struct AgentsPane {
    opened: Opened,
    acts: Acts,
    store: Entity<Store>,
    activity: Entity<Pick>,
    menu: Entity<RowMenu>,
    sidebar: Option<Entity<Sidebar>>,
    tab: Tab,
    /// The page standing in place of the tab's list, and its back, which takes the focus as the page opens.
    level: Option<Level>,
    back: FocusHandle,
    focus_back: bool,
    scroll: ScrollHandle,
    sidebar_read: SidebarRead,
    _observed: Vec<Subscription>,
}

/// What the pane last read off the sidebar, which a sidebar change must move before the pane draws again.
#[derive(Default, PartialEq)]
struct SidebarRead(Option<String>, Option<Value>, Option<std::rc::Rc<Value>>, bool);

impl AgentsPane {
    pub fn store(&self) -> &Entity<Store> {
        &self.store
    }

    pub fn tab(&self) -> Tab {
        self.tab
    }

    pub fn level(&self) -> Option<&Level> {
        self.level.as_ref()
    }

    pub fn back(&self) -> &FocusHandle {
        &self.back
    }

    fn nav(&mut self, nav: Nav, cx: &mut Context<Self>) {
        match nav {
            Nav::Go(at) => {
                if let Some(place) = self.place(cx) {
                    self.store.update(cx, |s, cx| s.pick_place(place.id, cx));
                }
                navigate(Route::Screen(format!("settings:{}", at.id()).into()), cx);
            }
            Nav::Level(level) => {
                self.focus_back = level.is_some();
                self.level = level;
                self.scroll.set_offset(point(px(0.), px(0.)));
                cx.notify();
            }
        }
    }

    fn step_back(&mut self, cx: &mut Context<Self>) {
        let up = self.level.take().and_then(|l| l.up.map(|up| *up));
        self.nav(Nav::Level(up), cx);
    }

    /// Escape steps back a page, except from a field, or on an add page from any control but the back.
    fn escape(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let Some(level) = &self.level else { return };
        if event.keystroke.key != "escape" || window.context_stack().iter().any(|c| c.contains("Input")) {
            return;
        }
        if level.key == "add" && !self.back.is_focused(window) && window.focused(cx).is_some() {
            return;
        }
        cx.stop_propagation();
        self.step_back(cx);
    }

    fn find_sidebar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let waiting = overlays::find_sidebar(self, window, cx, |this, sidebar, _, cx| {
            this._observed.push(cx.observe(&sidebar, |this, _, cx| {
                let read = this.sidebar_read(cx);
                if read != this.sidebar_read {
                    this.sidebar_read = read;
                    cx.notify();
                }
            }));
            this.sidebar = Some(sidebar);
            this.sidebar_read = this.sidebar_read(cx);
            cx.notify();
        });
        self._observed.extend(waiting);
    }

    /// What the pane reads off the sidebar: the computer's id and row, the preferences, and whether a provider is set.
    fn sidebar_read(&self, cx: &App) -> SidebarRead {
        let Some(s) = self.sidebar.as_ref().map(|s| s.read(cx)) else { return SidebarRead::default() };
        let id = match &self.opened.workspace_id {
            None => Some(HERE.to_owned()),
            Some(ws) => s.place_of(ws),
        };
        let row = id.as_deref().and_then(|id| s.place(id).cloned());
        SidebarRead(id, row, s.around().preferences.clone(), self.providers(cx))
    }

    /// The computer the rows are on: this one with no thread, else the one the thread's workspace runs on; none while
    /// the sidebar has not read that workspace.
    pub fn place(&self, cx: &App) -> Option<PlaceView> {
        let sidebar = self.sidebar.as_ref().map(|s| s.read(cx));
        let id = match &self.opened.workspace_id {
            None => HERE.to_owned(),
            Some(ws) => sidebar?.place_of(ws)?,
        };
        let row = sidebar.and_then(|s| s.place(&id).cloned());
        Some(row.and_then(|r| serde_json::from_value(r).ok()).unwrap_or_else(|| stand_in(&id)))
    }

    fn preferences(&self, cx: &App) -> Option<Preferences> {
        let s = self.sidebar.as_ref()?.read(cx);
        s.around().preferences.as_deref().and_then(|p| serde_json::from_value(p.clone()).ok())
    }

    fn providers(&self, cx: &App) -> bool {
        self.sidebar.as_ref().is_some_and(|s| s.read(cx).around().places.iter().any(|p| p["kind"] == "provider"))
    }

    fn top(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(level) = &self.level else { return self.tabs(cx).into_any_element() };
        let tab = w::TABS[Tab::ALL.iter().position(|t| *t == self.tab).unwrap_or_default()].1;
        let back_to = level.up.as_ref().map_or(tab.into(), |up| up.name.clone());
        let me = cx.entity().downgrade();
        let back =
            Button::ghost("agents-back", back_to).xs().glyph(Glyph::ChevronLeft).track_focus(&self.back).on_click(move |_, _, cx| {
                _ = me.update(cx, |this, cx| this.step_back(cx));
            });
        h_flex()
            .h_8()
            .min_w_0()
            .gap_2()
            .text_size(px(13.))
            .child(div().debug_selector(|| "agents-back".into()).flex_none().ml(px(-8.)).child(back))
            .child(slash(cx))
            .child(
                div()
                    .debug_selector(|| "agents-page".into())
                    .min_w_0()
                    .truncate()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(cx.palette().foreground)
                    .child(level.name.clone()),
            )
            .into_any_element()
    }

    fn tabs(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let picked = Tab::ALL.iter().position(|t| *t == self.tab);
        let control =
            Segmented::new("agents-pane-tabs", picked).fill().segment_size((px(13.), px(13.)), px(10.)).glyph_size(px(14.), px(6.));
        let control = w::TABS
            .iter()
            .zip([Glyph::Bot, Glyph::Server, Glyph::ScrollText, Glyph::Puzzle])
            .fold(control, |control, ((_, label), glyph)| control.glyph_segment(glyph, *label));
        let me = cx.entity().downgrade();
        control.on_pick(move |ix, _, cx| {
            _ = me.update(cx, |this, cx| {
                this.tab = Tab::ALL[ix];
                this.nav(Nav::Level(None), cx);
            });
        })
    }

    fn body(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let Some(place) = self.place(cx) else { return Vec::new() };
        let store = Store::of(&self.store, &self.acts, cx);
        let ask_broker = place.id == HERE && self.providers(cx);
        store.update(cx, |s, cx| {
            s.read(&place.id, cx);
            if ask_broker {
                s.ask_broker(cx);
            }
        });
        let mut rows = Rows::of(&place, store.read(cx));
        // A task's panel names only what its report holds; the recipe's misses are its computer's page's.
        rows.misses.clear();
        match self.tab {
            Tab::Agents => agents::agents_cards(&store, &rows, window, cx),
            tab => {
                let data = Data { preferences: self.preferences(cx), ..Data::default() };
                let at = At::Group(GroupId::Agents);
                let ctx =
                    Ctx::new(&data, &at, self.level.as_ref(), self.acts.clone(), self.activity.clone(), store.clone(), self.menu.clone());
                vec![kinds::tab(&ctx, &store, &rows, tab, window, cx)]
            }
        }
    }
}

/// A computer's row until the sidebar has read the places list.
fn stand_in(id: &str) -> PlaceView {
    let row: Value = json!({ "id": id, "kind": "computer", "name": "", "default": id == HERE });
    serde_json::from_value(row).expect("a place row of the protocol's shape")
}

impl Screen for AgentsPane {
    const NAME: &'static str = "agents";
    const PLACE: Place = Place::Panel(Pane::Agents);

    fn new(host: Host, opened: Opened, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let me = cx.weak_entity();
        let acts = Acts::new(WeakEntity::new_invalid(), Some(host.client().clone()), host.runtime().clone())
            .in_pane(move |nav, cx| _ = me.update(cx, |this, cx| this.nav(nav, cx)));
        let made = acts.clone();
        let store = store::kept(host.client(), window, cx, move |cx| cx.new(|_| Store::new(made)));
        store.update(cx, |s, _| s.show());
        let observed = vec![cx.observe(&store, |_, _, cx| cx.notify())];
        let mut pane = AgentsPane {
            opened,
            acts,
            store,
            activity: cx.new(|_| Pick::default()),
            menu: cx.new(|_| RowMenu::default()),
            sidebar: None,
            tab: Tab::Agents,
            level: None,
            back: cx.focus_handle(),
            focus_back: false,
            scroll: ScrollHandle::new(),
            sidebar_read: SidebarRead::default(),
            _observed: observed,
        };
        pane.find_sidebar(window, cx);
        pane
    }
}

impl Render for AgentsPane {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let top = self.top(cx);
        if std::mem::take(&mut self.focus_back) {
            self.back.focus(window, cx);
        }
        let body = v_flex().debug_selector(|| "agents-pane-body".into()).gap(px(30.)).px_4().pb_6().children(self.body(window, cx));
        v_flex()
            .debug_selector(|| "agents-pane".into())
            .size_full()
            .min_h_0()
            .on_key_down(cx.listener(Self::escape))
            .child(div().debug_selector(|| "agents-pane-top".into()).flex().flex_none().items_center().p_4().child(top))
            .child(scroll_area("agents-pane-scroll", &self.scroll, body, window, cx).flex_1().min_h_0())
    }
}
