// SPDX-License-Identifier: AGPL-3.0-only
//! A subagent's page (`ChatView.tsx` with its `subagent`, `SubagentBar.tsx`, `ThreadBreadcrumb.tsx`): the lead's
//! transcript cut to that subagent's lines, the bar in the composer's place, and crumbs that end in the lead and the
//! subagent. No context ring, Git or Open split stands in its title row.

use gpui_kit::component::{h_flex, ActiveTheme as _, Icon, Sizable as _};
use gpui_kit::{div, prelude::*, px, App, Context, Entity, FontWeight, SharedString, Subscription, Window};

use super::cards::SUBAGENT;
use crate::screens::{navigate, Host, Opened, Parts, Place, Route, Screen};
use crate::sidebar::ThreadItem;
use crate::thread_view::ThreadView;
use crate::transcript::{Page, ThreadAt};
use crate::ui::{catalog, tip, ActivePalette, Button, Glyph};

/// `SUBAGENT_BAR_WORDS`.
const BACK: &str = "Back to lead";

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
    host: Host,
    lead: Option<ThreadAt>,
    call: Option<String>,
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
        let item = ThreadItem {
            workspace_id: lead.workspace.clone(),
            thread_id: lead.thread.clone(),
            project: String::new(),
            title: String::new(),
        };
        let host = self.host.clone();
        let page = call.clone();
        self.view.update(cx, |v, cx| {
            v.follow(&host, &item, false, cx);
            v.set_page(Some(page), cx);
        });
        self.lead = Some(lead);
        self.call = Some(call);
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

    fn back(&self, cx: &mut App) {
        if let Some(lead) = &self.lead {
            navigate(Route::Thread { workspace_id: lead.workspace.clone(), thread_id: lead.thread.clone() }, cx);
        }
    }
}

impl Screen for SubagentScreen {
    const NAME: &'static str = SUBAGENT;
    const PLACE: Place = Place::Main;

    fn new(host: Host, opened: Opened, _: &mut Window, cx: &mut Context<Self>) -> Self {
        let view = cx.new(ThreadView::new);
        let watch = cx.observe(&view, |this, _, cx| this.watch(cx));
        let mut screen = SubagentScreen { view, host, lead: None, call: None, _watch: watch };
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
        div()
            .relative()
            .size_full()
            .child(self.view.clone())
            .children(page.map(|page| div().absolute().bottom_0().left_0().right_0().child(bar(&page, cx))))
    }
}

/// `SubagentBar`: in the composer's place, hugging what it says in the composer's column: the agent's mark and the
/// model, the state, then Back to lead.
fn bar(page: &Page, cx: &mut Context<SubagentScreen>) -> impl IntoElement {
    let p = cx.palette().clone();
    let harness = page.harness.clone().unwrap_or_default();
    let model = page.model.clone().unwrap_or_default();
    let agent = catalog::name(&harness);
    let state = match ended_line(page) {
        Some(ended) => ended,
        None => {
            let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0., |d| d.as_millis() as f64);
            format!("Working for {}", crate::words::fmt_duration(page.started.map_or(0., |s| now - s)))
        }
    };
    let words = format!("{agent}, {model}");
    let mark = h_flex()
        .id("subagent-model")
        .flex_none()
        .gap(px(6.))
        .text_size(px(13.))
        .text_color(p.foreground)
        .tooltip(tip(words))
        .child(catalog::mark(&harness, &agent, px(16.), p.muted_foreground, cx))
        .child(model);
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
                    div()
                        .id("subagent-state")
                        .min_w_0()
                        .truncate()
                        .text_size(px(13.))
                        .text_color(p.muted_foreground)
                        .tooltip(tip(state.clone()))
                        .child(state),
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
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = cx.palette().clone();
        let screen = self.screen.read(cx);
        let transcript = screen.view.read(cx).transcript();
        let lead = transcript.listed().and_then(|l| l.title.clone()).unwrap_or_default();
        let title = screen.call.as_deref().and_then(|call| transcript.page_of(call)).map(|p| p.title).unwrap_or_default();
        let back = self.screen.clone();
        h_flex()
            .id("subagent-crumbs")
            .debug_selector(|| "crumbs".into())
            .min_w_0()
            .gap_2()
            .text_size(px(14.))
            .line_height(px(20.))
            .child(
                div()
                    .id("subagent-crumb-lead")
                    .min_w_0()
                    .truncate()
                    .text_color(p.muted_foreground)
                    .cursor_pointer()
                    .hover(|s| s.text_color(p.foreground))
                    .child(SharedString::from(lead))
                    .on_click(move |_, _, cx| back.update(cx, |screen, cx| screen.back(cx))),
            )
            .child(div().flex_none().text_color(p.muted_foreground.opacity(0.5)).child("/"))
            .child(
                h_flex()
                    .min_w_0()
                    .gap_2()
                    .child(Icon::new(Glyph::Bot).with_size(px(16.)).text_color(p.muted_foreground).flex_none())
                    .child(
                        div()
                            .debug_selector(|| "crumbs-title".into())
                            .truncate()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(p.foreground)
                            .font_family(cx.theme().font_family.clone())
                            .child(SharedString::from(title)),
                    ),
            )
    }
}
