// SPDX-License-Identifier: AGPL-3.0-only
//! A subagent's page (`ChatView.tsx` with its `subagent`, `SubagentBar.tsx`, `ThreadBreadcrumb.tsx`): the lead's
//! transcript cut to that subagent's lines, the bar in the composer's place, and crumbs that end in the lead and the
//! subagent. No context ring, Git or Open split stands in its title row.

use gpui_kit::component::{h_flex, ActiveTheme as _};
use gpui_kit::{div, prelude::*, px, App, Context, Entity, FontWeight, SharedString, Subscription, Window};

use super::cards::SUBAGENT;
use super::header::{self, Opener};
use super::rows::Clock;
use crate::screens::{navigate, Host, Opened, Parts, Place, Route, Screen};
use crate::sidebar::ThreadItem;
use crate::thread_view::{ThreadView, ThreadViewEvent};
use crate::transcript::{Page, ThreadAt};
use crate::ui::{catalog, tip, ActivePalette, Button, Glyph};

/// `SUBAGENT_BAR_WORDS`.
const BACK: &str = "Back to lead";
const WORKING_FOR: &str = "Working for";

/// The lead and the call a page's argument names: `<workspace>/<thread>/<call>`.
pub fn parse(arg: &str) -> Option<(ThreadAt, String)> {
    let mut parts = arg.splitn(3, '/');
    let (workspace, thread, call) = (parts.next()?, parts.next()?, parts.next()?);
    (!workspace.is_empty() && !thread.is_empty() && !call.is_empty())
        .then(|| (ThreadAt { workspace: workspace.to_owned(), thread: thread.to_owned() }, call.to_owned()))
}

/// What the bar says of the subagent's state (`endedLine`): how long it worked, why it failed, or when it was stopped;
/// None while it runs, when the bar counts its time.
pub fn ended_line(page: &Page) -> Option<String> {
    let ran = page.started.zip(page.ended).map(|(from, to)| crate::words::fmt_duration(to - from));
    match page.state.as_str() {
        "running" => None,
        "failed" => Some(page.failure.as_ref().map_or_else(|| "Failed".to_owned(), |why| format!("Failed: {why}"))),
        "stopped" => Some(ran.map_or_else(|| "Stopped".to_owned(), |ran| format!("Stopped after {ran}"))),
        _ => Some(ran.map_or_else(|| "Done".to_owned(), |ran| format!("Worked for {ran}"))),
    }
}

pub struct SubagentScreen {
    view: Entity<ThreadView>,
    /// The page draws the kept thread screen's view where that follows the lead, rather than read and follow the lead
    /// a second time; else a view of its own.
    shared: bool,
    host: Host,
    lead: Option<ThreadAt>,
    call: Option<String>,
    /// The running subagent's time, ticking once a second, by the start it counts from.
    clock: Option<(f64, Entity<Clock>)>,
    _watch: Subscription,
}

impl SubagentScreen {
    pub fn view(&self) -> &Entity<ThreadView> {
        &self.view
    }

    fn open(&mut self, opened: Opened, cx: &mut Context<Self>) {
        let Some((lead, call)) = opened.arg.as_deref().and_then(parse) else { return };
        if self.lead.as_ref() == Some(&lead) && self.call.as_deref() == Some(call.as_str()) {
            return;
        }
        let kept = super::kept_view(&lead.thread, cx);
        let shared = kept.is_some();
        let view = kept.unwrap_or_else(|| if self.shared { cx.new(ThreadView::new) } else { self.view.clone() });
        if view != self.view {
            self.leave(cx);
            self._watch = cx.observe(&view, |this, _, cx| this.watch(cx));
            self.view = view;
        }
        self.shared = shared;
        let page = Some(call.clone());
        if shared {
            self.view.update(cx, |v, cx| v.set_page(page, cx));
        } else {
            let item = ThreadItem {
                workspace_id: lead.workspace.clone(),
                thread_id: lead.thread.clone(),
                project: String::new(),
                title: String::new(),
            };
            let host = self.host.clone();
            self.view.update(cx, |v, cx| {
                v.follow(&host, &item, false, cx);
                v.set_page(page, cx);
            });
        }
        self.lead = Some(lead);
        self.call = Some(call);
    }

    /// Gives the kept view back to the lead's own rows.
    fn leave(&self, cx: &mut App) {
        if self.shared {
            self.view.update(cx, |v, cx| v.set_page(None, cx));
        }
    }

    /// A call that neither the transcript nor the listing knows, once both are read, opens the lead instead.
    fn watch(&mut self, cx: &mut Context<Self>) {
        let (Some(lead), Some(call)) = (self.lead.clone(), self.call.clone()) else { return };
        let view = self.view.read(cx);
        if view.loaded() && view.transcript().listed().is_some() && view.transcript().page_of(&call).is_none() {
            navigate(Route::Thread { workspace_id: lead.workspace, thread_id: lead.thread }, cx);
        }
        cx.notify();
    }

    /// Back to lead: the lead selected and the caret in its composer (`requestComposerFocus`), once the frame has moved
    /// the kept thread screen onto it.
    fn back(&self, cx: &mut App) {
        let Some(lead) = self.lead.clone() else { return };
        navigate(Route::Thread { workspace_id: lead.workspace.clone(), thread_id: lead.thread.clone() }, cx);
        cx.defer(move |cx| {
            if let Some(view) = super::kept_view(&lead.thread, cx) {
                view.update(cx, |_, cx| cx.emit(ThreadViewEvent::FocusComposer));
            }
        });
    }

    /// The clock the bar counts a running subagent's time on, once it has drawn one.
    pub fn ticking(&self) -> Option<Entity<Clock>> {
        self.clock.as_ref().map(|(_, clock)| clock.clone())
    }

    /// The clock a running subagent's bar counts on, kept while its start stays the same.
    fn clock_from(&mut self, since: f64, cx: &mut Context<Self>) -> Entity<Clock> {
        match &self.clock {
            Some((at, clock)) if *at == since => clock.clone(),
            _ => {
                let now = self.view.read(cx).frozen();
                let clock = cx.new(|cx| Clock::new(since, now, cx));
                self.clock = Some((since, clock.clone()));
                clock
            }
        }
    }
}

impl Screen for SubagentScreen {
    const NAME: &'static str = SUBAGENT;
    const PLACE: Place = Place::Main;

    fn new(host: Host, opened: Opened, _: &mut Window, cx: &mut Context<Self>) -> Self {
        let view = cx.new(ThreadView::new);
        let watch = cx.observe(&view, |this, _, cx| this.watch(cx));
        cx.on_release(|this, cx| this.leave(cx)).detach();
        let mut screen = SubagentScreen { view, shared: false, host, lead: None, call: None, clock: None, _watch: watch };
        screen.open(opened, cx);
        screen
    }

    fn show(&mut self, opened: Opened, _: &mut Window, cx: &mut Context<Self>) {
        self.open(opened, cx);
    }

    fn parts(this: &Entity<Self>, _: &mut Window, cx: &mut App) -> Parts {
        let screen = this.clone();
        let crumbs = cx.new(|cx| Crumbs { _redraw: cx.observe(&screen, |_, _, cx| cx.notify()), screen });
        Parts { crumbs: Some(crumbs.into()), ..Parts::default() }
    }
}

impl Render for SubagentScreen {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let page = self.call.as_deref().and_then(|call| self.view.read(cx).transcript().page_of(call));
        let clock = page.as_ref().filter(|p| ended_line(p).is_none()).and_then(|p| p.started).map(|since| self.clock_from(since, cx));
        div()
            .relative()
            .size_full()
            .child(self.view.clone())
            .children(page.map(|page| div().absolute().bottom_0().left_0().right_0().child(bar(&page, clock, cx))))
    }
}

/// `SubagentBar`: in the composer's place, hugging what it says in the composer's column: the agent's mark and the
/// model, the state, then Back to lead. A running subagent's time is the transcript's own ticking clock.
fn bar(page: &Page, clock: Option<Entity<Clock>>, cx: &mut Context<SubagentScreen>) -> impl IntoElement {
    let p = cx.palette().clone();
    let harness = page.harness.clone().unwrap_or_default();
    let agent = catalog::name(&harness);
    // `useSubagentModel`: a subagent that says no model is named by its agent.
    let model = page.model.clone().unwrap_or_else(|| agent.to_string());
    let ended = ended_line(page);
    let words = format!("{agent}, {model}");
    let mark = h_flex()
        .id("subagent-model")
        .flex_none()
        .gap(px(6.))
        .text_size(px(13.))
        .text_color(p.foreground)
        .tooltip(tip(words))
        .child(catalog::mark(&harness, &agent, px(16.), p.muted_foreground, cx))
        .child(div().debug_selector(|| "subagent-model-name".into()).child(model));
    let back =
        Button::ghost("subagent-back", BACK).glyph(Glyph::ArrowUpLeft).tooltip(BACK).on_click(cx.listener(|this, _, _, cx| this.back(cx)));
    h_flex().w_full().justify_center().px(px(20.)).pt(px(8.)).pb(px(20.)).child(
        crate::screens::composer::parts::glass_box(cx).w_auto().child(
            h_flex()
                .debug_selector(|| "subagent-bar".into())
                .min_h(px(48.))
                .gap(px(12.))
                .py(px(8.))
                .pl(px(20.))
                .pr(px(8.))
                .child(mark)
                .child(
                    h_flex()
                        .id("subagent-state")
                        .debug_selector(|| "subagent-state".into())
                        .min_w_0()
                        .truncate()
                        .gap(px(4.))
                        .text_size(px(13.))
                        .text_color(p.muted_foreground)
                        .when_some(ended.clone(), |el, ended| el.tooltip(tip(ended.clone())).child(ended))
                        .when(ended.is_none(), |el| el.child(WORKING_FOR).children(clock)),
                )
                .child(div().ml_auto().flex_none().child(back)),
        ),
    )
}

/// The page's crumbs: the lead as the way back, then the subagent's title.
struct Crumbs {
    screen: Entity<SubagentScreen>,
    _redraw: Subscription,
}

impl Render for Crumbs {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = cx.palette().clone();
        let screen = self.screen.read(cx);
        let transcript = screen.view.read(cx).transcript();
        let title = screen.call.as_deref().and_then(|call| transcript.page_of(call)).map(|p| p.title).unwrap_or_default();
        let lead = screen.lead.as_ref().map(|l| Opener {
            workspace_id: l.workspace.clone(),
            thread_id: Some(l.thread.clone()),
            title: transcript.listed().and_then(|l| l.title.clone()).unwrap_or_default(),
        });
        let project = screen.lead.as_ref().and_then(|l| header::project_of(&l.thread, cx));
        let project = project.map(|(id, name)| header::project_crumb(id.as_deref(), name, window, cx));
        h_flex()
            .id("subagent-crumbs")
            .debug_selector(|| "crumbs".into())
            .min_w_0()
            .items_center()
            .gap_2()
            .text_size(px(14.))
            .line_height(px(20.))
            .children(project)
            .children(lead.map(|lead| header::thread_link("crumbs-lead", lead, cx)))
            .child(crate::ui::slash(cx))
            .child(
                div()
                    .id("subagent-crumb-title")
                    .debug_selector(|| "crumbs-title".into())
                    .min_w_0()
                    .truncate()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(p.foreground)
                    .font_family(cx.theme().font_family.clone())
                    .child(SharedString::from(title)),
            )
    }
}
