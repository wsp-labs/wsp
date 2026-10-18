// SPDX-License-Identifier: AGPL-3.0-only
//! Held runs and the sheets that answer them, after `apps/web/src/slate/consent.tsx`, `approvals.tsx`, `mcp.tsx` and
//! `standing.tsx`: the rows under the header, the run, confirm, batch, tool and link sheets, the thread's approvals,
//! and the Forget secrets alert. Which sheet opens on its own follows `Consent()` in `SlateSurface.tsx`.

use std::collections::HashSet;

use gpui_kit::component::{h_flex, v_flex, ActiveTheme as _, Icon, Sizable as _};
use gpui_kit::{
    anchored, deferred, div, point, prelude::*, px, AnyElement, App, Context, FocusHandle, KeyDownEvent, MouseButton, SharedString, Window,
};
use serde_json::Value;

use super::model::Ask;
use super::pieces::{flags, layout};
use super::{Slate, SlateScreen};
use crate::ui::{text, ActivePalette, AlertDialog, Button, Card, CardItem, Glyph, Row, Switch};

/// The sheets' words, `CONSENT_WORDS`, `LINK_WORDS`, `MCP_WORDS`, `APPROVALS_WORDS` and `STANDING_WORDS`.
pub mod words {
    pub const TITLE: &str = "Run this command?";
    pub const WANTS: &str = "This slate wants to run";
    pub const REVIEW: &str = "Review";
    pub const NOT_NOW: &str = "Not now";
    pub const ONCE: &str = "Run once";
    pub const ALWAYS: &str = "Always in this thread";
    pub const RUN: &str = "Run";
    pub const WROTE: &str = "Written by the agent in this thread";
    pub fn more(n: usize) -> String {
        if n == 1 {
            "1 more command waits after this one".into()
        } else {
            format!("{n} more commands wait after this one")
        }
    }
    pub fn link_title(domain: &str) -> String {
        format!("Open {domain} from this slate?")
    }
    pub const LINK_ALWAYS: &str = "Always for this domain";
    pub const LINK_ONCE: &str = "Open once";
    pub const TOOL_RUN: &str = "Let this panel run";
    pub const TOOL_READ: &str = "Let this panel read";
    pub const ROW_RUN: &str = "Run";
    pub const ROW_READ: &str = "Read";
    pub const SHOW_CODE: &str = "Show code";
    pub const ALLOW: &str = "Allow";
    pub const THEN: &str = "Then its result goes on stdin to";
    pub fn file(name: &str) -> String {
        format!("It reads {name}, which holds")
    }
    pub const ALLOW_ALL: &str = "Allow all";
    pub fn allow(n: usize) -> String {
        format!("Allow {n}")
    }
    pub const STANDING_TITLE: &str = "Approvals in this thread";
    pub const STANDING_NONE: &str = "Nothing in this thread runs without asking.";
    pub const REVOKE: &str = "Revoke";
    pub const SENDS: &str = "Reactions may message the agent";
    pub const SENDS_LINE: &str = "send() in a reaction";
    pub const REFUSED: &str = "Refused. ";
    pub fn run(run: &str) -> String {
        format!("Run ${run}")
    }
    pub const CANCEL: &str = "Cancel";
}

/// The why a run held only for the person carries (`SLATE_HELD_APPROVAL`), which the sheet does not repeat.
const HELD_APPROVAL: &str = "needs your approval";
/// The approval key reaction sends are kept under (`SLATE_SEND_KEY`).
const SEND_KEY: &str = "send";

/// The Forget secrets alert's state: busy while the write is out, and the host's refusal.
#[derive(Clone, Debug, Default)]
pub struct Forget {
    pub busy: bool,
    pub refused: Option<String>,
}

/// The approvals dialog's state: the key whose Revoke is out, and the host's refusal of one.
#[derive(Clone, Debug, Default)]
pub struct Standing {
    pub busy: Option<String>,
    pub refused: Option<String>,
}

/// The open sheet's answer: busy while it is out, and the host's refusal, which keeps the sheet open.
#[derive(Clone, Debug, Default)]
pub struct Answer {
    pub busy: bool,
    pub refused: Option<String>,
}

/// The batch sheet's rows as it opened and as they arrived since, and which are switched on.
#[derive(Clone, Debug, Default)]
pub struct Batch {
    pub known: HashSet<String>,
    pub on: HashSet<String>,
}

#[derive(Default)]
pub struct Consent {
    /// The ask a press held or Review named, whose sheet is open.
    pub asking: Option<Ask>,
    /// Asks whose sheet this window showed, which do not open on their own again.
    pub seen: HashSet<String>,
    /// The address a press's link waits on.
    pub linking: Option<String>,
    pub forgetting: Option<Forget>,
    pub standing: Option<Standing>,
    pub answer: Answer,
    pub batch: Option<Batch>,
    /// Whether the tool sheet's Show code is open.
    pub code: bool,
    /// The sheet last drawn, so a new one starts clean and takes the keys.
    shown: Option<String>,
    focus: Option<FocusHandle>,
}

impl Consent {
    pub fn link(&mut self, href: String, cx: &mut Context<SlateScreen>) {
        self.linking = Some(href);
        cx.notify();
    }
}

/// What is open over the panel, one at a time.
#[derive(Clone, Debug, PartialEq)]
pub enum Open {
    Forget,
    Standing,
    Link(String),
    Batch(Vec<Ask>),
    One(Box<Ask>),
}

impl Open {
    fn key(&self) -> String {
        match self {
            Open::Forget => "forget".into(),
            Open::Standing => "standing".into(),
            Open::Link(h) => format!("link:{h}"),
            Open::Batch(_) => "batch".into(),
            Open::One(a) => format!("ask:{}", a.key),
        }
    }
}

/// Whether an ask can be answered beside others on one sheet: no confirm, and a command the host can pin.
fn batchable_one(ask: &Ask) -> bool {
    ask.confirm.is_none() && (ask.kind != "cmd" || ask.no_always != Some(true))
}

/// The asks one sheet can answer together, one per key.
pub fn batchable(asks: &[Ask]) -> Vec<Ask> {
    let mut keys = HashSet::new();
    asks.iter().filter(|a| batchable_one(a) && keys.insert(a.key.clone())).cloned().collect()
}

/// The sheet over the panel: the person's own Forget secrets or Approvals, a link waiting, then the one a press held or
/// Review named; else a batch for two or more where one is unseen; else the first unseen ask.
pub fn open(screen: &SlateScreen) -> Option<Open> {
    let c = &screen.consent;
    let slate = screen.slate()?;
    if c.forgetting.is_some() {
        return Some(Open::Forget);
    }
    if c.standing.is_some() {
        return Some(Open::Standing);
    }
    if let Some(href) = &c.linking {
        return Some(Open::Link(href.clone()));
    }
    let asks = &slate.record.asks;
    if c.asking.is_none() {
        let batch = batchable(asks);
        if batch.len() > 1 && batch.iter().any(|a| !c.seen.contains(&a.key)) {
            return Some(Open::Batch(batch));
        }
    }
    c.asking.clone().or_else(|| asks.iter().find(|a| !c.seen.contains(&a.key)).cloned()).map(|a| Open::One(Box::new(a)))
}

/// `slateLinkDomain`: an http or https address's host, lower case; None for anything else.
pub fn link_domain(href: &str) -> Option<String> {
    let rest = href.strip_prefix("https://").or_else(|| href.strip_prefix("http://"))?;
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let host = authority.rsplit('@').next().unwrap_or_default();
    let host = if let Some(v6) = host.strip_prefix('[') {
        format!("[{}]", v6.split(']').next().unwrap_or_default())
    } else {
        host.split(':').next().unwrap_or_default().to_owned()
    };
    (!host.is_empty()).then(|| host.to_lowercase())
}

/// `capitalised`.
fn capitalised(word: &str) -> String {
    let mut chars = word.chars();
    chars.next().map_or_else(String::new, |c| c.to_uppercase().chain(chars).collect())
}

/// `cadenceOf`: its timer, else the reactions that start it, else a press.
pub fn cadence(slate: &Slate, run: &str) -> String {
    if let Some(timer) = layout::timer_cadence(slate, run) {
        return timer;
    }
    let reactions = slate.doc().map(|d| d.reactions.as_slice()).unwrap_or_default();
    let when: Vec<String> = reactions
        .iter()
        .filter(|r| {
            let steps = r.get("do").and_then(Value::as_array).map(Vec::as_slice).unwrap_or_default();
            steps.iter().any(|s| {
                s.get("do").and_then(Value::as_str) == Some("start")
                    && s.get("run").and_then(Value::as_str).map(|r| r.trim_start_matches('$')) == Some(run)
            })
        })
        .filter_map(|r| {
            let on = r.get("on")?;
            if let Some(change) = on.get("change").and_then(Value::as_array) {
                let names: Vec<&str> = change.iter().filter_map(Value::as_str).collect();
                Some(format!("{} changes", names.join(" or ")))
            } else {
                on.get("done").and_then(Value::as_str).map(|d| format!("${} finishes", d.trim_start_matches('$')))
            }
        })
        .collect();
    if when.is_empty() {
        "Runs when you press it".into()
    } else {
        format!("Runs each time {}", when.join(" or "))
    }
}

/// One held run's row: its first line, the host's ask where it carries a sheet, and why it waits otherwise.
pub struct Held {
    pub run: String,
    pub line: String,
    pub ask: Option<Ask>,
    pub why: Option<String>,
}

/// The slate's held runs in document order (`heldRuns`).
pub fn held(slate: &Slate) -> Vec<Held> {
    let Some(doc) = slate.doc() else { return Vec::new() };
    doc.runs
        .iter()
        .filter_map(|(run, decl)| {
            let record = slate.record.run(run)?;
            if record.get("state").and_then(Value::as_str) != Some("held") {
                return None;
            }
            let ask = slate.record.asks.iter().find(|a| &a.run == run).cloned();
            let field = |k: &str| decl.get(k).and_then(Value::as_str).unwrap_or_default().to_owned();
            let line = match ask.as_ref().filter(|a| a.kind == "cmd").and_then(|a| a.cmd.clone()) {
                Some(cmd) => cmd,
                None => match decl.get("kind").and_then(Value::as_str) {
                    Some("tool") => format!("{}.{}", field("server"), field("tool")),
                    Some("resource") => format!("{}:{}", field("server"), field("uri")),
                    _ => field("cmd"),
                },
            };
            let line = line.split('\n').next().unwrap_or_default().to_owned();
            Some(Held { run: run.clone(), line, ask, why: record.get("why").and_then(Value::as_str).map(str::to_owned) })
        })
        .collect()
}

/// Whether a piece under the root reads a refreshing run the root does not say itself.
pub fn refreshing_under_root(slate: &Slate) -> bool {
    let Some(root) = slate.doc().and_then(|d| d.piece(&d.root)) else { return false };
    !flags(&root.kind).says_refreshing && layout::refreshing_under(slate, &root.id)
}

impl SlateScreen {
    /// A press's link: an allowed domain, or one that is not http, opens at once; any other waits on the prompt.
    pub fn open_link(&mut self, href: String, cx: &mut Context<Self>) {
        let allowed = |domain: &str| {
            self.record().and_then(|r| r.approvals.get(&format!("domain:{domain}"))).and_then(|a| a.get("state")).and_then(Value::as_str)
                == Some("allowed")
        };
        match link_domain(&href) {
            Some(domain) if !allowed(&domain) => self.consent.link(href, cx),
            _ => cx.open_url(&href),
        }
    }

    /// Review on a held row: that run's sheet.
    pub fn review(&mut self, run: &str, cx: &mut Context<Self>) {
        self.consent.asking = self.record().and_then(|r| r.asks.iter().find(|a| a.run == run)).cloned();
        cx.notify();
    }

    /// Not now on a held row: refused, and its sheet does not open on its own after.
    pub fn refuse_held(&mut self, key: String, cx: &mut Context<Self>) {
        self.consent.seen.insert(key.clone());
        self.approve(key, "refuse", cx).detach();
        cx.notify();
    }

    /// Closes what is open, answered or not; a closed sheet does not open on its own again.
    pub fn close_sheet(&mut self, cx: &mut Context<Self>) {
        if let Some(open) = open(self) {
            self.close(open, cx);
        }
    }

    fn close(&mut self, open: Open, cx: &mut Context<Self>) {
        let c = &mut self.consent;
        match open {
            Open::Forget => c.forgetting = None,
            Open::Standing => c.standing = None,
            Open::Link(_) => c.linking = None,
            Open::Batch(asks) => {
                c.seen.extend(asks.into_iter().map(|a| a.key));
                c.batch = None;
                self.reload(cx);
            }
            Open::One(ask) => {
                c.seen.insert(ask.key);
                c.asking = None;
            }
        }
        cx.notify();
    }

    /// The open sheet's answer: each ask it answers by its own key, one after another, then closed; a refusal keeps it
    /// open under the host's words.
    pub fn decide(&mut self, scope: &'static str, cx: &mut Context<Self>) {
        let Some(opened) = open(self) else { return };
        let keys: Vec<String> = match &opened {
            Open::One(a) => vec![a.key.clone()],
            Open::Batch(asks) => {
                let on = self.consent.batch.as_ref().map(|b| b.on.clone()).unwrap_or_default();
                asks.iter().filter(|a| on.contains(&a.key)).map(|a| a.key.clone()).collect()
            }
            _ => return,
        };
        if keys.is_empty() {
            return;
        }
        self.consent.answer = Answer { busy: true, refused: None };
        cx.notify();
        cx.spawn(async move |this, cx| {
            let mut answer = Ok(());
            for key in keys {
                let Ok(asking) = this.update(cx, |s, cx| s.approve(key, scope, cx)) else { return };
                if let Err(refused) = asking.await {
                    answer = Err(refused);
                    break;
                }
            }
            _ = this.update(cx, |s, cx| match answer {
                Ok(()) => s.close(opened, cx),
                Err(refused) => {
                    s.consent.answer = Answer { busy: false, refused: Some(refused) };
                    cx.notify();
                }
            });
        })
        .detach();
    }

    pub fn link_once(&mut self, cx: &mut Context<Self>) {
        if let Some(href) = self.consent.linking.take() {
            cx.open_url(&href);
        }
        cx.notify();
    }

    /// Always for this domain: allowed for the thread, then opened.
    pub fn link_always(&mut self, cx: &mut Context<Self>) {
        let Some(href) = self.consent.linking.clone() else { return };
        let Some(domain) = link_domain(&href) else { return };
        self.consent.answer = Answer { busy: true, refused: None };
        let asking = self.approve(format!("domain:{domain}"), "thread", cx);
        cx.spawn(async move |this, cx| {
            let answer = asking.await;
            _ = this.update(cx, |s, cx| {
                match answer {
                    Ok(()) => {
                        s.consent.linking = None;
                        cx.open_url(&href);
                    }
                    Err(refused) => s.consent.answer = Answer { busy: false, refused: Some(refused) },
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    pub fn forget(&mut self, cx: &mut Context<Self>) {
        self.consent.forgetting = Some(Forget { busy: true, refused: None });
        let asking = self.forget_secrets(cx);
        cx.spawn(async move |this, cx| {
            let answer = asking.await;
            _ = this.update(cx, |s, cx| {
                s.consent.forgetting = answer.err().map(|refused| Forget { busy: false, refused: Some(refused) });
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    pub fn revoke_standing(&mut self, key: String, cx: &mut Context<Self>) {
        self.consent.standing = Some(Standing { busy: Some(key.clone()), refused: None });
        let asking = self.revoke(key, cx);
        cx.spawn(async move |this, cx| {
            let answer = asking.await;
            _ = this.update(cx, |s, cx| {
                if let Some(standing) = &mut s.consent.standing {
                    standing.busy = None;
                    standing.refused = answer.err();
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn toggle_batch(&mut self, key: String, on: bool, cx: &mut Context<Self>) {
        if let Some(batch) = &mut self.consent.batch {
            if on {
                batch.on.insert(key);
            } else {
                batch.on.remove(&key);
            }
        }
        cx.notify();
    }
}

/// The held runs' rows under the header.
pub fn held_rows(screen: &SlateScreen, _window: &mut Window, cx: &mut Context<SlateScreen>) -> Vec<AnyElement> {
    let Some(slate) = screen.slate() else { return Vec::new() };
    let rows = held(slate);
    if rows.is_empty() {
        return Vec::new();
    }
    let p = cx.palette().clone();
    let mono = cx.theme().mono_font_family.clone();
    let me = cx.entity().downgrade();
    let rows = rows.into_iter().map(|h| {
        let run = h.run.clone();
        let tail: Vec<AnyElement> = match h.ask {
            None => vec![div()
                .flex_none()
                .text_size(px(12.))
                .line_height(px(16.))
                .text_color(p.muted_foreground)
                .child(h.why.unwrap_or_default())
                .into_any_element()],
            Some(ask) => {
                let (me_review, me_refuse, run_review) = (me.clone(), me.clone(), run.clone());
                let (review, refuse) = (format!("slate-review-{run}"), format!("slate-refuse-{run}"));
                vec![
                    div()
                        .debug_selector({
                            let review = review.clone();
                            move || review.clone()
                        })
                        .child(
                            Button::outline(SharedString::from(review), words::REVIEW)
                                .xs()
                                .on_click(move |_, _, cx| _ = me_review.update(cx, |s, cx| s.review(&run_review, cx))),
                        )
                        .into_any_element(),
                    div()
                        .debug_selector({
                            let refuse = refuse.clone();
                            move || refuse.clone()
                        })
                        .child(
                            Button::ghost(SharedString::from(refuse), words::NOT_NOW)
                                .xs()
                                .on_click(move |_, _, cx| _ = me_refuse.update(cx, |s, cx| s.refuse_held(ask.key.clone(), cx))),
                        )
                        .into_any_element(),
                ]
            }
        };
        h_flex()
            .h(px(28.))
            .min_w_0()
            .gap(px(8.))
            .text_size(px(13.))
            .debug_selector(move || format!("slate-held-{run}"))
            .child(div().flex_none().text_color(p.muted_foreground).child(words::WANTS))
            .child(div().flex_1().min_w_0().truncate().font_family(mono.clone()).text_size(px(12.)).text_color(p.foreground).child(h.line))
            .children(tail)
            .into_any_element()
    });
    vec![v_flex().flex_none().gap(px(4.)).px(px(12.)).pb(px(4.)).children(rows).into_any_element()]
}

/// The sheets and dialogs over the panel.
pub fn overlays(screen: &mut SlateScreen, window: &mut Window, cx: &mut Context<SlateScreen>) -> Vec<AnyElement> {
    let Some(opened) = open(screen) else {
        screen.consent.shown = None;
        screen.consent.batch = None;
        return Vec::new();
    };
    let key = opened.key();
    if screen.consent.shown.as_deref() != Some(key.as_str()) {
        screen.consent.shown = Some(key);
        screen.consent.answer = Answer::default();
        screen.consent.code = false;
        if !matches!(opened, Open::Batch(_)) {
            screen.consent.batch = None;
        }
        let focus = screen.consent.focus.get_or_insert_with(|| cx.focus_handle()).clone();
        window.focus(&focus, cx);
    }
    if let Open::Batch(asks) = &opened {
        // The rows on the sheet as it opened start on; one that arrives while it is open starts off.
        let fresh = screen.consent.batch.is_none();
        let batch = screen.consent.batch.get_or_insert_with(Batch::default);
        for ask in asks {
            if batch.known.insert(ask.key.clone()) && fresh {
                batch.on.insert(ask.key.clone());
            }
        }
    }
    let screen = &*screen;
    let (body, scrim_closes) = match &opened {
        Open::Forget => (forget_alert(screen, cx), false),
        Open::Standing => (standing_dialog(screen, cx), true),
        Open::Link(href) => (link_prompt(screen, href, cx), true),
        Open::Batch(asks) => (batch_sheet(screen, asks, window, cx), true),
        Open::One(ask) if ask.kind == "cmd" => (command_sheet(screen, ask, window, cx), ask.confirm.is_none()),
        Open::One(ask) => (tool_sheet(screen, ask, window, cx), ask.confirm.is_none()),
    };
    vec![layer(screen, body, scrim_closes, window, cx)]
}

/// The scrim the size of the window and the sheet on it: it takes the keys as it opens, so Escape closes it; a plain
/// sheet closes on the scrim, an alert asks for its answer.
fn layer(screen: &SlateScreen, body: AnyElement, scrim_closes: bool, window: &mut Window, cx: &mut Context<SlateScreen>) -> AnyElement {
    let focus = screen.consent.focus.clone().unwrap_or_else(|| cx.focus_handle());
    let size = window.viewport_size();
    let scrim = cx.palette().background.opacity(if cx.theme().is_dark() { 0.64 } else { 0.6 });
    let layer = div()
        .id("slate-sheet")
        .debug_selector(|| "slate-sheet".into())
        .track_focus(&focus)
        .on_key_down(cx.listener(|this, e: &KeyDownEvent, _, cx| {
            if e.keystroke.key == "escape" {
                this.close_sheet(cx);
            }
        }))
        .occlude()
        .w(size.width)
        .h(size.height)
        .p(px(16.))
        .bg(scrim)
        .backdrop_blur(px(4.))
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .when(scrim_closes, |el| el.on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| this.close_sheet(cx))))
        .child(div().on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation()).child(body));
    deferred(anchored().position(point(px(0.), px(0.))).child(layer)).with_priority(3).into_any_element()
}

fn act(
    cx: &Context<SlateScreen>,
    f: impl Fn(&mut SlateScreen, &mut Context<SlateScreen>) + 'static,
) -> impl Fn(&gpui_kit::ClickEvent, &mut Window, &mut App) + 'static {
    let me = cx.entity().downgrade();
    move |_, _, cx| _ = me.update(cx, |s, cx| f(s, cx))
}

fn line(text: impl Into<SharedString>, ink: gpui_kit::Hsla) -> gpui_kit::Div {
    div().min_w_0().text_color(ink).child(text.into())
}

/// The block a command, its pipe or a file shows in: mono on the accent fill, `lines` tall at most, then it scrolls.
fn code(id: String, body: String, lines: f32, cx: &App) -> AnyElement {
    let p = cx.palette();
    div()
        .id(SharedString::from(id))
        .min_w_0()
        .max_h(px(lines * 16. + 16.))
        .overflow_y_scroll()
        .rounded(px(8.))
        .bg(p.accent)
        .px(px(10.))
        .py(px(8.))
        .font_family(cx.theme().mono_font_family.clone())
        .text_size(px(12.))
        .line_height(px(16.))
        .text_color(p.foreground)
        .child(body)
        .into_any_element()
}

/// A value the host sent as dots: a secret, shown muted.
fn dots(value: &str) -> bool {
    value.starts_with('•')
}

fn shown(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// Names and values side by side in mono, the names' column as wide as the widest (`grid-cols-[auto_minmax(0,1fr)]`).
fn grid(rows: Vec<(String, String)>, window: &Window, cx: &App) -> Option<AnyElement> {
    if rows.is_empty() {
        return None;
    }
    let p = cx.palette();
    let mono = cx.theme().mono_font_family.clone();
    let widest = rows.iter().map(|(n, _)| text::width(n, 12., mono.clone(), window)).fold(px(0.), |a, b| if b > a { b } else { a });
    Some(
        v_flex()
            .gap(px(4.))
            .font_family(mono)
            .text_size(px(12.))
            .line_height(px(16.))
            .children(rows.into_iter().map(|(name, value)| {
                let ink = if dots(&value) { p.muted_foreground } else { p.foreground };
                h_flex()
                    .items_start()
                    .gap(px(16.))
                    .child(div().flex_none().w(widest).text_color(p.muted_foreground).child(name))
                    .child(div().flex_1().min_w_0().text_color(ink).child(value))
            }))
            .into_any_element(),
    )
}

/// A run's pipe: the literal command its raw result goes to.
fn then_block(id: &str, then: Option<&String>, cx: &App) -> Option<AnyElement> {
    let then = then?;
    Some(
        v_flex()
            .min_w_0()
            .gap(px(4.))
            .child(line(words::THEN, cx.palette().muted_foreground))
            .child(code(format!("{id}-then"), then.clone(), 6., cx))
            .into_any_element(),
    )
}

/// Each of the slate's files a run reads, by name with its whole text.
fn file_blocks(id: &str, ask: &Ask, cx: &App) -> Vec<AnyElement> {
    ask.files
        .iter()
        .map(|(name, body)| {
            v_flex()
                .min_w_0()
                .gap(px(4.))
                .child(line(words::file(name), cx.palette().muted_foreground))
                .child(code(format!("{id}-file-{name}"), shown(body), 12., cx))
                .into_any_element()
        })
        .collect()
}

fn why_line(ask: &Ask, cx: &App) -> Option<AnyElement> {
    let why = ask.why.as_deref().filter(|w| *w != HELD_APPROVAL && !w.is_empty())?;
    Some(line(format!("{}.", capitalised(why)), cx.palette().muted_foreground).into_any_element())
}

/// `CommandBody`: the text, its pipe, its files, how often it runs, each value it is handed, and where and how long.
fn command_body(ask: &Ask, cadence: String, lines: f32, window: &Window, cx: &App) -> Vec<AnyElement> {
    let p = cx.palette();
    let id = format!("slate-consent-{}", ask.key);
    let mut out = vec![code(format!("{id}-cmd"), ask.cmd.clone().unwrap_or_default(), lines, cx)];
    out.extend(then_block(&id, ask.then.as_ref(), cx));
    out.extend(file_blocks(&id, ask, cx));
    out.push(line(cadence, p.foreground).into_any_element());
    let mut rows: Vec<(String, String)> = ask.env.iter().map(|(n, v)| (n.clone(), shown(v))).collect();
    let args = ask.args.as_array().map(Vec::as_slice).unwrap_or_default();
    rows.extend(args.iter().enumerate().map(|(at, a)| (format!("${}", at + 1), shown(a))));
    rows.extend(ask.stdin.clone().map(|s| ("stdin".to_owned(), s)));
    out.extend(grid(rows, window, cx));
    let timeout = ask.timeout_s.map(super::pieces::look::num_text).unwrap_or_default();
    out.push(
        line(
            format!("on {}, in {}, {timeout} s at most", ask.computer.clone().unwrap_or_default(), ask.folder.clone().unwrap_or_default()),
            p.foreground,
        )
        .into_any_element(),
    );
    out.extend(why_line(ask, cx));
    out
}

fn refusal(answer: &Answer, cx: &App) -> Option<AnyElement> {
    answer
        .refused
        .clone()
        .map(|r| line(r, cx.palette().error_foreground).debug_selector(|| "slate-sheet-refused".into()).into_any_element())
}

fn sheet_body(id: String, children: Vec<AnyElement>) -> AnyElement {
    v_flex()
        .min_w_0()
        .gap(px(12.))
        .text_size(px(13.))
        .line_height(px(20.))
        .debug_selector(move || id.clone())
        .children(children)
        .into_any_element()
}

/// "Run this command?", or the confirm alert where the run names `confirm`.
fn command_sheet(screen: &SlateScreen, ask: &Ask, window: &Window, cx: &mut Context<SlateScreen>) -> AnyElement {
    let slate = screen.slate().expect("a drawn slate");
    let c = &screen.consent;
    let busy = c.answer.busy;
    let mut body = command_body(ask, cadence(slate, &ask.run), 12., window, cx);
    body.push(line(format!("{}.", words::WROTE), cx.palette().muted_foreground).into_any_element());
    let more = slate.record.asks.iter().filter(|a| a.key != ask.key && !c.seen.contains(&a.key)).count();
    if more > 0 {
        body.push(line(words::more(more), cx.palette().muted_foreground).into_any_element());
    }
    body.extend(refusal(&c.answer, cx));
    let body = sheet_body(format!("slate-consent-{}", ask.run), body);
    let not_now = Button::outline("slate-sheet-refuse", words::NOT_NOW).held(busy).on_click(act(cx, |s, cx| s.decide("refuse", cx)));
    let dialog = AlertDialog::new(words::TITLE);
    match &ask.confirm {
        Some(confirm) => dialog
            .sentence(confirm.clone())
            .child(body)
            .act(not_now.neutral_ring())
            .act(Button::destructive("slate-sheet-once", words::RUN).held(busy).on_click(act(cx, |s, cx| s.decide("once", cx))))
            .into_any_element(),
        None => {
            let mut dialog = dialog.child(body).act(not_now);
            if ask.no_always != Some(true) {
                dialog = dialog
                    .act(Button::outline("slate-sheet-always", words::ALWAYS).held(busy).on_click(act(cx, |s, cx| s.decide("thread", cx))));
            }
            dialog
                .act(Button::primary("slate-sheet-once", words::ONCE).held(busy).on_click(act(cx, |s, cx| s.decide("once", cx))))
                .into_any_element()
        }
    }
}

/// The tool and its server as a title says them: "Let this panel run execute on Cloudflare".
fn tool_title(ask: &Ask, lead: &str) -> String {
    format!("{lead} {} on {}", ask.tool.clone().unwrap_or_default(), capitalised(ask.server.as_deref().unwrap_or_default()))
}

/// What the run does, where and how often, and its code folded behind Show code.
fn tool_body(screen: &SlateScreen, ask: &Ask, cadence: String, window: &Window, cx: &mut Context<SlateScreen>) -> Vec<AnyElement> {
    let p = cx.palette().clone();
    let id = format!("slate-consent-{}", ask.key);
    let mut out: Vec<AnyElement> = ask.description.clone().map(|d| line(d, p.foreground).into_any_element()).into_iter().collect();
    out.push(line(format!("On {}. {cadence}.", ask.computer.clone().unwrap_or_default()), p.muted_foreground).into_any_element());
    let open = screen.consent.code;
    let fold = h_flex()
        .id(SharedString::from(format!("{id}-code-fold")))
        .gap(px(4.))
        .cursor_pointer()
        .text_size(px(12.))
        .line_height(px(16.))
        .text_color(p.muted_foreground)
        .hover(|s| s.text_color(p.foreground))
        .on_click(act(cx, |s, cx| {
            s.consent.code = !s.consent.code;
            cx.notify();
        }))
        .child(Icon::new(if open { Glyph::ChevronDown } else { Glyph::ChevronRight }).with_size(px(12.)))
        .child(words::SHOW_CODE);
    out.push(fold.into_any_element());
    if open {
        let (server, tool) = (ask.server.clone().unwrap_or_default(), ask.tool.clone().unwrap_or_default());
        let named = if ask.resource == Some(true) { format!("{server}:{tool}") } else { format!("{server}.{tool}") };
        let mut panel = vec![div()
            .min_w_0()
            .font_family(cx.theme().mono_font_family.clone())
            .text_size(px(12.))
            .line_height(px(16.))
            .text_color(p.foreground)
            .child(named)
            .into_any_element()];
        let args: Vec<(String, String)> =
            ask.args.as_object().map(|o| o.iter().map(|(n, v)| (n.clone(), shown(v))).collect()).unwrap_or_default();
        panel.extend(grid(args, window, cx));
        panel.extend(then_block(&id, ask.then.as_ref(), cx));
        panel.extend(file_blocks(&id, ask, cx));
        out.push(
            v_flex().min_w_0().gap(px(8.)).pt(px(8.)).debug_selector(|| "slate-consent-code".into()).children(panel).into_any_element(),
        );
    }
    out
}

/// "Let this panel run <tool> on <Server>?", or "read" for a resource; the confirm alert where it names `confirm`.
fn tool_sheet(screen: &SlateScreen, ask: &Ask, window: &Window, cx: &mut Context<SlateScreen>) -> AnyElement {
    let slate = screen.slate().expect("a drawn slate");
    let busy = screen.consent.answer.busy;
    let lead = if ask.resource == Some(true) { words::TOOL_READ } else { words::TOOL_RUN };
    let mut body = tool_body(screen, ask, cadence(slate, &ask.run), window, cx);
    body.extend(refusal(&screen.consent.answer, cx));
    let body = sheet_body(format!("slate-consent-{}", ask.run), body);
    let dialog = AlertDialog::new(format!("{}?", tool_title(ask, lead)));
    let not_now = Button::outline("slate-sheet-refuse", words::NOT_NOW).held(busy).on_click(act(cx, |s, cx| s.decide("refuse", cx)));
    match &ask.confirm {
        Some(confirm) => dialog
            .sentence(confirm.clone())
            .child(body)
            .act(not_now.neutral_ring())
            .act(Button::destructive("slate-sheet-once", words::RUN).held(busy).on_click(act(cx, |s, cx| s.decide("once", cx))))
            .into_any_element(),
        None => dialog
            .child(body)
            .act(not_now)
            .act(Button::primary("slate-sheet-allow", words::ALLOW).held(busy).on_click(act(cx, |s, cx| s.decide("thread", cx))))
            .into_any_element(),
    }
}

/// "Run these N commands?": a switch per row, Not now, Allow all or Allow N, and Run once for the rows switched on.
fn batch_sheet(screen: &SlateScreen, asks: &[Ask], window: &Window, cx: &mut Context<SlateScreen>) -> AnyElement {
    let slate = screen.slate().expect("a drawn slate");
    let c = &screen.consent;
    let on = c.batch.as_ref().map(|b| b.on.clone()).unwrap_or_default();
    let chosen = asks.iter().filter(|a| on.contains(&a.key)).count();
    let held = c.answer.busy || chosen == 0;
    let title = if asks.iter().all(|a| a.kind == "cmd") {
        format!("Run these {} commands?", asks.len())
    } else {
        format!("Let this slate run these {}?", asks.len())
    };
    let me = cx.entity().downgrade();
    let mut rows: Vec<AnyElement> = Vec::new();
    for ask in asks {
        let picked = on.contains(&ask.key);
        let body = if ask.kind == "cmd" {
            command_body(ask, cadence(slate, &ask.run), 4., window, cx)
        } else {
            let lead = if ask.resource == Some(true) { words::ROW_READ } else { words::ROW_RUN };
            let mut body = vec![line(tool_title(ask, lead), cx.palette().foreground).into_any_element()];
            body.extend(tool_body(screen, ask, cadence(slate, &ask.run), window, cx));
            body
        };
        let (me, key) = (me.clone(), ask.key.clone());
        let switch = Switch::new(SharedString::from(format!("slate-batch-{}", ask.key)), picked).on_change(move |next, _, cx| {
            let key = key.clone();
            _ = me.update(cx, |s, cx| s.toggle_batch(key, *next, cx));
        });
        let key = ask.key.clone();
        rows.push(
            h_flex()
                .items_start()
                .min_w_0()
                .gap(px(12.))
                .when(!picked, |el| el.opacity(0.6))
                .debug_selector(move || format!("slate-approval-{key}"))
                .child(div().mt(px(6.)).child(switch))
                .child(v_flex().flex_1().min_w_0().gap(px(8.)).children(body))
                .into_any_element(),
        );
    }
    rows.push(line(format!("{}.", words::WROTE), cx.palette().muted_foreground).into_any_element());
    rows.extend(refusal(&c.answer, cx));
    let body =
        v_flex().min_w_0().gap(px(20.)).text_size(px(13.)).line_height(px(20.)).debug_selector(|| "slate-approvals".into()).children(rows);
    let allow = if chosen == asks.len() { words::ALLOW_ALL.to_owned() } else { words::allow(chosen) };
    AlertDialog::new(title)
        .child(body)
        .act(Button::outline("slate-sheet-refuse", words::NOT_NOW).held(held).on_click(act(cx, |s, cx| s.decide("refuse", cx))))
        .act(Button::outline("slate-sheet-always", allow).held(held).on_click(act(cx, |s, cx| s.decide("thread", cx))))
        .act(Button::primary("slate-sheet-once", words::ONCE).held(held).on_click(act(cx, |s, cx| s.decide("once", cx))))
        .into_any_element()
}

/// "Open <domain> from this slate?": the whole address, Not now, Always for this domain and Open once.
fn link_prompt(screen: &SlateScreen, href: &str, cx: &mut Context<SlateScreen>) -> AnyElement {
    let p = cx.palette().clone();
    let busy = screen.consent.answer.busy;
    let domain = link_domain(href).unwrap_or_default();
    let body = v_flex()
        .min_w_0()
        .gap(px(8.))
        .child(
            div()
                .min_w_0()
                .font_family(cx.theme().mono_font_family.clone())
                .text_size(px(12.))
                .line_height(px(16.))
                .text_color(p.muted_foreground)
                .debug_selector(|| "slate-link-href".into())
                .child(href.to_owned()),
        )
        .text_size(px(13.))
        .line_height(px(20.))
        .children(refusal(&screen.consent.answer, cx));
    AlertDialog::new(words::link_title(&domain))
        .child(body)
        .act(Button::outline("slate-link-refuse", words::NOT_NOW).held(busy).on_click(act(cx, |s, cx| s.close_sheet(cx))))
        .act(Button::outline("slate-link-always", words::LINK_ALWAYS).held(busy).on_click(act(cx, |s, cx| s.link_always(cx))))
        .act(Button::primary("slate-link-once", words::LINK_ONCE).held(busy).on_click(act(cx, |s, cx| s.link_once(cx))))
        .into_any_element()
}

/// "Approvals in this thread": a row per approval with Revoke, or the sentence that nothing runs without asking.
fn standing_dialog(screen: &SlateScreen, cx: &mut Context<SlateScreen>) -> AnyElement {
    let p = cx.palette().clone();
    let standing = screen.consent.standing.clone().unwrap_or_default();
    let approvals = screen.record().map(|r| r.approvals.clone()).unwrap_or_default();
    let me = cx.entity().downgrade();
    let body = if approvals.is_empty() {
        line(words::STANDING_NONE, p.muted_foreground).text_size(px(14.)).line_height(px(20.)).into_any_element()
    } else {
        let rows = approvals.iter().map(|(key, approval)| {
            let field = |k: &str| approval.get(k).and_then(Value::as_str).map(str::to_owned);
            let (title, note) = if key == SEND_KEY {
                (words::SENDS.to_owned(), words::SENDS_LINE.to_owned())
            } else {
                let refused = if field("state").as_deref() == Some("refused") { words::REFUSED } else { "" };
                let named = match (field("run"), field("cmd")) {
                    (Some(run), _) => words::run(&run),
                    (None, Some(_)) => key.clone(),
                    _ => String::new(),
                };
                (field("cmd").unwrap_or_else(|| key.clone()), format!("{refused}{named}"))
            };
            let (me, k) = (me.clone(), key.clone());
            let revoke = Button::outline(SharedString::from(format!("slate-revoke-{key}")), words::REVOKE)
                .xs()
                .held(standing.busy.is_some())
                .on_click(move |_, _, cx| _ = me.update(cx, |s, cx| s.revoke_standing(k.clone(), cx)));
            CardItem::from(Row::new(SharedString::from(format!("approval-{key}")), title).note(note).mono().clip().control(revoke))
        });
        Card::new("slate-approvals").inset(crate::ui::size::CARD_INSET).items(rows).into_any_element()
    };
    let refused = standing.refused.map(|r| line(r, p.error_foreground).pt(px(12.)).text_size(px(13.)).line_height(px(20.)));
    AlertDialog::new(words::STANDING_TITLE)
        .child(v_flex().min_w_0().debug_selector(|| "slate-standing".into()).child(body).children(refused))
        .into_any_element()
}

/// "Forget this slate's secrets?": Cancel, the destructive Forget secrets, the refusal inside.
fn forget_alert(screen: &SlateScreen, cx: &mut Context<SlateScreen>) -> AnyElement {
    let forget = screen.consent.forgetting.clone().unwrap_or_default();
    let mut dialog = AlertDialog::new(super::words::FORGET_TITLE).sentence(super::words::FORGET_BODY);
    if let Some(refused) = forget.refused {
        dialog = dialog.child(
            line(refused, cx.palette().error_foreground)
                .text_size(px(13.))
                .line_height(px(20.))
                .debug_selector(|| "slate-forget-refused".into()),
        );
    }
    dialog
        .act(Button::outline("slate-forget-cancel", words::CANCEL).neutral_ring().on_click(act(cx, |s, cx| s.close_sheet(cx))))
        .act(Button::destructive("slate-forget", super::words::FORGET).held(forget.busy).on_click(act(cx, |s, cx| s.forget(cx))))
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_link_names_its_domain_and_only_http_has_one() {
        assert_eq!(link_domain("https://User@Example.COM:8080/a?b").as_deref(), Some("example.com"));
        assert_eq!(link_domain("http://[::1]:3000/").as_deref(), Some("[::1]"));
        assert_eq!(link_domain("mailto:a@b.c"), None);
        assert_eq!(link_domain("ftp://example.com/file"), None);
    }

    #[test]
    fn a_command_with_confirm_or_no_always_asks_alone() {
        let ask = |key: &str, confirm: Option<&str>, no_always: Option<bool>| {
            let mut a: Ask = serde_json::from_value(serde_json::json!({"key": key, "run": key, "kind": "cmd"})).unwrap();
            a.confirm = confirm.map(str::to_owned);
            a.no_always = no_always;
            a
        };
        let asks = vec![
            ask("a", None, None),
            ask("b", Some("sure?"), None),
            ask("c", None, Some(true)),
            ask("a", None, None),
            ask("d", None, None),
        ];
        assert_eq!(batchable(&asks).iter().map(|a| a.key.as_str()).collect::<Vec<_>>(), ["a", "d"]);
    }
}
