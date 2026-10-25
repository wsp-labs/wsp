// SPDX-License-Identifier: AGPL-3.0-only
//! The project and computer switchers over the list (`sidebar/NounSwitcher.tsx`, `ProjectSwitcher.tsx`,
//! `ComputerSwitcher.tsx`): the row that names the pick, and its popup with a search field, the rows with the pick's
//! check and a cog to its settings, drag to reorder the projects, and the footer's Add.

use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{h_flex, v_flex, ActiveTheme as _, Icon, Sizable as _};
use gpui_kit::{
    anchored, canvas, deferred, div, prelude::*, px, AnyElement, Context, Entity, FocusHandle, KeyDownEvent, MouseDownEvent, SharedString,
    Subscription, Window,
};
use serde_json::{json, Value};
use wsp_host_client::protocol::PreferencesSetRequest;

use super::chrome::{ONE_LINE, ROW_RADIUS};
use super::{looks, Asked, Sidebar};
use crate::ui::{ActivePalette, Glyph, Palette};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Noun {
    Projects,
    Computers,
}

impl Noun {
    pub(crate) fn ix(self) -> usize {
        match self {
            Noun::Projects => 0,
            Noun::Computers => 1,
        }
    }

    fn words(self) -> (&'static str, &'static str, &'static str) {
        match self {
            Noun::Projects => ("All projects", "Search projects", "Add a project"),
            Noun::Computers => ("All computers", "Search computers", "Add a computer"),
        }
    }
}

/// An open switcher: its search field, the row the keys stand on, and the project being dragged.
pub struct OpenSwitcher {
    pub noun: Noun,
    query: Entity<InputState>,
    highlight: usize,
    focus: FocusHandle,
    _events: Subscription,
}

/// One row of a switcher's list: its id (none for All), its name and its quiet word on the right.
#[derive(Clone)]
struct Choice {
    id: Option<String>,
    name: String,
    meta: Option<String>,
}

/// A project dragged in the popup.
#[derive(Clone)]
struct DraggedProject(String);

impl Render for DraggedProject {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

/// `movedBefore`: the order with `moved` taken out and put just before `target`.
fn moved_before(order: &[String], moved: &str, target: &str) -> Vec<String> {
    let mut out: Vec<String> = order.iter().filter(|id| *id != moved).cloned().collect();
    let at = out.iter().position(|id| id == target).unwrap_or(out.len());
    out.insert(at, moved.to_owned());
    out
}

/// `normalizeSearchText`: lower case, the way the web compares a query.
fn norm(text: &str) -> String {
    text.trim().to_lowercase()
}

impl Sidebar {
    /// The projects in the order the person dragged them (`threadTree.ts` inProjectOrder): those `projectOrder`
    /// names first, then the rest in the host's order.
    pub(crate) fn ordered_projects(&self) -> Vec<(String, String)> {
        let order: Vec<&str> = self
            .around
            .preferences
            .as_ref()
            .and_then(|p| p.get("projectOrder")?.as_array())
            .map(|a| a.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default();
        let mut out: Vec<(String, String)> =
            order.iter().filter_map(|id| self.around.projects.iter().find(|(p, _)| p == id).cloned()).collect();
        out.extend(self.around.projects.iter().filter(|(p, _)| !order.contains(&p.as_str())).cloned());
        out
    }

    /// A project's computer as the switcher names it beside it, none for this computer (`projectComputerWord`).
    fn project_computer(&self, project: &str) -> Option<String> {
        let at = self.around.project_computers.get(project)?;
        if at == "here" {
            return None;
        }
        let place = self.around.places.iter().find(|p| p.get("id").and_then(Value::as_str) == Some(at.as_str()));
        Some(place.map(looks::place_name).unwrap_or_else(|| at.clone()))
    }

    fn choices(&self, noun: Noun, query: &str) -> Vec<Choice> {
        let all = Choice { id: None, name: noun.words().0.to_owned(), meta: None };
        let q = norm(query);
        let rest: Vec<Choice> = match noun {
            Noun::Projects => self
                .ordered_projects()
                .into_iter()
                .map(|(id, name)| Choice { meta: self.project_computer(&id), id: Some(id), name })
                .collect(),
            Noun::Computers => self
                .around
                .places
                .iter()
                .filter_map(|p| Some(Choice { id: Some(p.get("id")?.as_str()?.to_owned()), name: looks::place_name(p), meta: None }))
                .collect(),
        };
        std::iter::once(all).chain(rest.into_iter().filter(|c| norm(&c.name).contains(&q))).collect()
    }

    fn picked(&self, noun: Noun) -> Option<String> {
        match noun {
            Noun::Projects => self.prefs.project.clone(),
            Noun::Computers => self.computer.clone(),
        }
    }

    pub(crate) fn toggle_switcher(&mut self, noun: Noun, window: &mut Window, cx: &mut Context<Self>) {
        if self.switcher.as_ref().is_some_and(|s| s.noun == noun) {
            return self.close_switcher(window, cx);
        }
        let query = cx.new(|cx| InputState::new(window, cx).placeholder(noun.words().1));
        let events = cx.subscribe(&query, |this, _, e: &InputEvent, cx| {
            if let (InputEvent::Change, Some(open)) = (e, this.switcher.as_mut()) {
                open.highlight = 0;
                cx.notify();
            }
        });
        query.update(cx, |q, cx| q.focus(window, cx));
        self.switcher = Some(OpenSwitcher { noun, query, highlight: 0, focus: cx.focus_handle(), _events: events });
        cx.notify();
    }

    fn close_switcher(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.switcher.take().is_some() {
            window.focus(&self.focus, cx);
            cx.notify();
        }
    }

    fn pick(&mut self, noun: Noun, id: Option<String>, cx: &mut Context<Self>) {
        self.switcher = None;
        self.hand_keys_back(cx);
        match noun {
            Noun::Projects => {
                self.prefs.project = id;
                self.rebuild(cx);
            }
            Noun::Computers => {
                self.computer = id;
                self.refold(cx);
            }
        }
    }

    fn add(&mut self, noun: Noun, cx: &mut Context<Self>) {
        self.switcher = None;
        self.hand_keys_back(cx);
        cx.emit(if noun == Noun::Projects { Asked::AddProject } else { Asked::AddComputer });
        cx.notify();
    }

    fn switcher_key(&mut self, e: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let Some(open) = self.switcher.as_ref() else { return };
        let (noun, query) = (open.noun, open.query.read(cx).value().to_string());
        let choices = self.choices(noun, &query);
        let n = choices.len() + 1;
        let open = self.switcher.as_mut().unwrap();
        match e.keystroke.key.as_str() {
            "down" => open.highlight = (open.highlight + 1) % n,
            "up" => open.highlight = (open.highlight + n - 1) % n,
            "enter" => {
                let at = open.highlight;
                cx.stop_propagation();
                return match choices.get(at) {
                    Some(c) => self.pick(noun, c.id.clone(), cx),
                    None => self.add(noun, cx),
                };
            }
            "escape" => {
                cx.stop_propagation();
                return self.close_switcher(window, cx);
            }
            _ => return,
        }
        cx.stop_propagation();
        cx.notify();
    }

    /// Writes the order the person dragged the projects into, at once here and then on the host.
    fn reorder(&mut self, moved: &str, target: &str, cx: &mut Context<Self>) {
        let order: Vec<String> = self.ordered_projects().into_iter().map(|(id, _)| id).collect();
        let next = moved_before(&order, moved, target);
        if let Some(p) = self.around.preferences.as_mut().map(std::rc::Rc::make_mut).and_then(Value::as_object_mut) {
            p.insert("projectOrder".to_owned(), json!(next));
        }
        self.write_op::<PreferencesSetRequest>(json!({ "patch": { "projectOrder": next } }), cx);
        cx.notify();
    }

    /// The switcher's row over the list: the pick's glyph and name, its computer, the chevron; with its popup under it
    /// while open, drawn in the page's palette as the web portals it.
    pub(crate) fn switcher_row(&mut self, noun: Noun, page: &Palette, cx: &mut Context<Self>) -> AnyElement {
        let p = cx.palette().clone();
        let picked = self.picked(noun);
        let prefs = self.around.preferences.clone().unwrap_or_default();
        let open = self.switcher.as_ref().is_some_and(|s| s.noun == noun);
        let (name, glyph, meta) = match (noun, &picked) {
            (Noun::Projects, Some(id)) => (
                self.around.projects.iter().find(|(p, _)| p == id).map_or_else(|| noun.words().0.to_owned(), |(_, n)| n.clone()),
                looks::project_glyph(Some(id), &prefs, &self.around.images, px(16.), p.sidebar_icon_color, true, &p),
                self.project_computer(id),
            ),
            (Noun::Computers, Some(id)) => {
                let place = self.around.places.iter().find(|pl| pl.get("id").and_then(Value::as_str) == Some(id.as_str()));
                (
                    place.map(looks::place_name).unwrap_or_else(|| id.clone()),
                    place.map_or_else(
                        || Icon::new(Glyph::Monitor).with_size(px(16.)).text_color(p.sidebar_icon_color).into_any_element(),
                        |pl| looks::computer_glyph(pl, &prefs, px(16.), p.sidebar_icon_color),
                    ),
                    None,
                )
            }
            (Noun::Projects, None) => (
                noun.words().0.to_owned(),
                Icon::new(Glyph::Folder).with_size(px(16.)).text_color(p.sidebar_icon_color).into_any_element(),
                None,
            ),
            (Noun::Computers, None) => (
                noun.words().0.to_owned(),
                Icon::new(Glyph::Monitor).with_size(px(16.)).text_color(p.sidebar_icon_color).into_any_element(),
                None,
            ),
        };
        let id = if noun == Noun::Projects { "projects" } else { "computers" };
        let mono = cx.theme().mono_font_family.clone();
        let (hover, active, ink) = (p.sidebar_row_hover, p.sidebar_row_active, p.sidebar_foreground);
        let heads = self.switcher_heads.clone();
        let row = h_flex()
            .id(id)
            .debug_selector(move || id.into())
            .relative()
            .flex_none()
            .h(ONE_LINE)
            .gap(px(10.))
            .px(px(8.))
            .rounded(ROW_RADIUS)
            .text_size(px(14.))
            .line_height(px(20.))
            .text_color(p.sidebar_row_rest)
            .cursor_pointer()
            .hover(move |s| s.bg(hover).text_color(ink))
            .active(move |s| s.bg(active))
            .on_click(cx.listener(move |this, _, window, cx| this.toggle_switcher(noun, window, cx)))
            .child(
                canvas(
                    move |bounds, _, _| {
                        let mut all = heads.get();
                        all[noun.ix()] = bounds;
                        heads.set(all);
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .size_full(),
            )
            .child(glyph)
            .child(div().flex_1().min_w_0().truncate().child(name))
            .children(meta.map(|m| div().flex_none().font_family(mono.clone()).text_size(px(11.)).text_color(p.top_row_meta).child(m)))
            .when(noun == Noun::Projects && picked.is_some(), |el| el.child(div().flex_none().w(px(20.))))
            .child(Icon::new(if open { Glyph::ChevronUp } else { Glyph::ChevronDown }).with_size(px(16.)).text_color(p.sidebar_icon_color));
        let popup = open.then(|| self.switcher_popup(noun, page, cx));
        div().relative().child(row).children(popup).into_any_element()
    }

    fn switcher_popup(&mut self, noun: Noun, p: &Palette, cx: &mut Context<Self>) -> AnyElement {
        let open = self.switcher.as_ref().unwrap();
        let (query, highlight, focus) = (open.query.clone(), open.highlight, open.focus.clone());
        let typed = query.read(cx).value().to_string();
        let choices = self.choices(noun, &typed);
        let picked = self.picked(noun);
        let prefs = self.around.preferences.clone().unwrap_or_default();
        let drags = noun == Noun::Projects && typed.is_empty();
        let mono = cx.theme().mono_font_family.clone();
        let rows: Vec<AnyElement> = choices
            .iter()
            .enumerate()
            .map(|(ix, c)| {
                let lit = ix == highlight;
                let glyph = match (noun, &c.id) {
                    (_, None) => Icon::new(if noun == Noun::Projects { Glyph::Folder } else { Glyph::Monitor })
                        .with_size(px(16.))
                        .text_color(p.muted_foreground)
                        .into_any_element(),
                    (Noun::Projects, Some(id)) => {
                        looks::project_glyph(Some(id), &prefs, &self.around.images, px(16.), p.foreground, true, p)
                    }
                    (Noun::Computers, Some(id)) => {
                        let place = self.around.places.iter().find(|pl| pl.get("id").and_then(Value::as_str) == Some(id.as_str()));
                        place.map_or_else(|| div().into_any_element(), |pl| looks::computer_glyph(pl, &prefs, px(16.), p.muted_foreground))
                    }
                };
                let (id, settings) = (c.id.clone(), c.id.clone());
                let cog = settings.map(|sid| {
                    let asked = if noun == Noun::Projects { Asked::ProjectSettings(sid) } else { Asked::ComputerSettings(sid) };
                    div()
                        .id(SharedString::from(format!("cog-{ix}")))
                        .flex()
                        .flex_none()
                        .items_center()
                        .justify_center()
                        .size(px(28.))
                        .mr(px(-4.))
                        .rounded(px(8.))
                        .text_color(p.muted_foreground)
                        .hover(|s| s.bg(p.background.opacity(0.6)).text_color(p.foreground))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            cx.stop_propagation();
                            this.switcher = None;
                            this.hand_keys_back(cx);
                            cx.emit(asked.clone());
                            cx.notify();
                        }))
                        .child(Icon::new(Glyph::Settings).with_size(px(16.)))
                });
                let mut row = h_flex()
                    .id(SharedString::from(format!("choice-{ix}")))
                    .h(ONE_LINE)
                    .w_full()
                    .gap(px(10.))
                    .px(px(8.))
                    .rounded(ROW_RADIUS)
                    .text_size(px(14.))
                    .line_height(px(20.))
                    .text_color(p.foreground)
                    .cursor_pointer()
                    .when(lit, |el| el.bg(p.accent))
                    .on_mouse_move(cx.listener(move |this, _, _, cx| {
                        if let Some(open) = this.switcher.as_mut().filter(|o| o.highlight != ix) {
                            open.highlight = ix;
                            cx.notify();
                        }
                    }))
                    .on_click(cx.listener({
                        let id = id.clone();
                        move |this, _, _, cx| this.pick(noun, id.clone(), cx)
                    }))
                    .child(glyph)
                    .child(div().flex_1().min_w_0().truncate().child(c.name.clone()))
                    .children(c.meta.clone().map(|m| div().flex_none().font_family(mono.clone()).text_size(px(11.)).child(m)))
                    .when(picked == c.id, |el| el.child(Icon::new(Glyph::Check).with_size(px(16.))))
                    .children(cog);
                if let (true, Some(pid)) = (drags, id) {
                    let target = pid.clone();
                    row = row
                        .on_drag(DraggedProject(pid), |d, _, _, cx| cx.new(|_| d.clone()))
                        .on_drop(cx.listener(move |this, d: &DraggedProject, _, cx| this.reorder(&d.0, &target, cx)));
                }
                row.into_any_element()
            })
            .collect();
        let add_ix = choices.len();
        let footer = h_flex()
            .id("switcher-add")
            .h(ONE_LINE)
            .w_full()
            .gap(px(10.))
            .px(px(8.))
            .rounded(ROW_RADIUS)
            .text_size(px(14.))
            .text_color(p.foreground)
            .cursor_pointer()
            .when(highlight == add_ix, |el| el.bg(p.accent))
            .on_mouse_move(cx.listener(move |this, _, _, cx| {
                if let Some(open) = this.switcher.as_mut().filter(|o| o.highlight != add_ix) {
                    open.highlight = add_ix;
                    cx.notify();
                }
            }))
            .on_click(cx.listener(move |this, _, _, cx| this.add(noun, cx)))
            .child(Icon::new(Glyph::Plus).with_size(px(16.)).text_color(p.muted_foreground))
            .child(noun.words().2);
        let panel = v_flex()
            .id("switcher-popup")
            .debug_selector(|| "switcher-popup".into())
            .track_focus(&focus)
            .occlude()
            .w(px(239.))
            .mt(px(4.))
            .rounded(px(12.))
            .border_1()
            .border_color(p.foreground.opacity(0.1))
            .bg(p.popover)
            .shadow(p.popover_shadow.clone())
            .text_color(p.foreground)
            .on_key_down(cx.listener(|this, e: &KeyDownEvent, window, cx| this.switcher_key(e, window, cx)))
            .on_mouse_down_out(cx.listener(move |this, e: &MouseDownEvent, window, cx| {
                // A press on the head is the head's own, whose click closes the popup.
                if !this.switcher_heads.get()[noun.ix()].contains(&e.position) {
                    this.close_switcher(window, cx);
                }
            }))
            .child(
                h_flex()
                    .h(px(32.))
                    .gap(px(8.))
                    .px(px(12.))
                    .border_b_1()
                    .border_color(p.border)
                    .child(Icon::new(Glyph::Search).with_size(px(16.)).text_color(p.muted_foreground))
                    .child(div().flex_1().child(Input::new(&query).appearance(false).text_size(px(13.)).xsmall())),
            )
            .child(v_flex().p(px(4.)).children(rows))
            .child(div().border_t_1().border_color(p.border).p(px(4.)).child(footer));
        deferred(anchored().child(panel)).with_priority(1).into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::moved_before;

    #[gpui_kit::test]
    fn a_pick_and_a_snooze_hand_the_keys_back_to_the_list(cx: &mut gpui_kit::TestAppContext) {
        use super::{Noun, Sidebar};
        use gpui_kit::AppContext as _;
        cx.update(|cx| {
            gpui_kit::init(cx);
            crate::ui::init(crate::ui::Side::Light, cx);
        });
        let sidebar = cx.new(Sidebar::new);
        let (_, cx) = cx.add_window_view(|window, cx| gpui_kit::component::Root::new(sidebar.clone(), window, cx));
        let has_keys = |cx: &mut gpui_kit::VisualTestContext| cx.update(|window, cx| sidebar.read(cx).focus.is_focused(window));

        cx.update(|window, cx| sidebar.update(cx, |s, cx| s.toggle_switcher(Noun::Projects, window, cx)));
        cx.run_until_parked();
        assert!(!has_keys(cx), "the popup's search takes the keys");
        sidebar.update(cx, |s, cx| s.pick(Noun::Projects, None, cx));
        cx.run_until_parked();
        assert!(has_keys(cx), "a pick hands them back");

        cx.update(|window, cx| sidebar.update(cx, |s, cx| super::super::dialogs::snooze(s, "t", window, cx)));
        cx.run_until_parked();
        assert!(!has_keys(cx), "the dialog takes the keys as it opens");
        sidebar.update(cx, |s, cx| s.snooze_until("t".to_owned(), 0, cx));
        cx.run_until_parked();
        assert!(has_keys(cx), "a snooze hands them back");
    }

    #[test]
    fn a_drop_puts_the_project_just_before_its_target() {
        let order = ["a", "b", "c", "d"].map(String::from);
        assert_eq!(moved_before(&order, "d", "b"), ["a", "d", "b", "c"]);
        assert_eq!(moved_before(&order, "a", "c"), ["b", "a", "c", "d"]);
    }
}
