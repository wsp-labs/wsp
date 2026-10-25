// SPDX-License-Identifier: AGPL-3.0-only
//! The controls a slate holds, after `apps/web/src/slate/pieces/button.tsx`, `input.tsx`, `select.tsx`,
//! `choices.tsx`, `toggle.tsx`, `press.ts` and `outcome.tsx`. A press goes to the host's `slates.event` and draws busy
//! until it is answered, its outcome or refusal said under it for two seconds. A pick is written with `slates.state`
//! and its change raised once the host answered. Typing is sent 300 ms after the last keystroke and at once on blur,
//! Enter or a submit; a focused field keeps its text against a write from elsewhere and offers the choice on blur.

use std::time::{Duration, Instant};

use gpui_kit::component::input::{InputEvent, InputState, Textarea, TextareaState};
use gpui_kit::component::{h_flex, v_flex, ActiveTheme as _, Icon, Sizable as _};
use gpui_kit::TestSupportExt as _;
use gpui_kit::{
    div, prelude::*, px, AnyElement, App, Context, Entity, EntityInputHandler as _, FocusHandle, Focusable as _, FontWeight, KeyDownEvent,
    SharedString, Subscription, Task, TextRun, WeakEntity, Window,
};
use serde_json::{json, Value};
use wsp_host_client::protocol;

use super::layout::{is_toolbar, place_of, two_way, Place};
use super::{look, PieceCx};
use crate::screens::slate::consent::link_domain;
use crate::screens::slate::model::Loud;
use crate::screens::slate::{call, refusal, truthy, Load, Outcome, Raise, Slate, SlateScreen};
use crate::ui::card::keycap;
use crate::ui::{motion, size, text, tip, ActivePalette, Button, Field, Glyph, Segmented, Select, SelectItem, SelectState, Switch};

/// A second press within this long is folded into the first (`COALESCE_MS`).
pub(super) const COALESCE: Duration = Duration::from_millis(500);
/// Typing waits this long for the next keystroke (`DEBOUNCE_MS`).
const DEBOUNCE: Duration = Duration::from_millis(300);
/// The 160 px slot a one-line field takes beside its label (`SLOT_FIELD`).
const SLOT: f32 = 160.;

pub mod words {
    pub const SEND: &str = "Send";
    pub const PICK: &str = "Pick one";
    pub const ALL: &str = "All";
    pub const RIGHT: &str = "Right";
    pub const WRONG: &str = "Wrong";
    pub const CHANGED: &str = "The agent changed this while you were typing";
    pub const KEEP: &str = "Keep mine";
    pub const THEIRS: &str = "Take theirs";
}

pub fn draw(p: &PieceCx, window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    Some(match p.piece.kind.as_str() {
        "button" => button(p, window, cx),
        "input" => input(p, window, cx),
        "select" => select(p, window, cx),
        "choices" => choices(p, window, cx),
        "decision" => decision(p, window, cx),
        "toggle" => toggle(p, window, cx),
        _ => return None,
    })
}

/// `slateHeldText`: a non-empty text holds a control, anything else lets it go.
pub fn held_by(v: Option<&Value>) -> Option<String> {
    v.and_then(Value::as_str).filter(|s| !s.trim().is_empty()).map(str::to_owned)
}

/// The steps of a handler, one or a list.
fn steps_of(steps: Option<&Value>) -> Vec<&Value> {
    match steps {
        Some(Value::Array(a)) => a.iter().collect(),
        Some(one @ Value::Object(_)) => vec![one],
        _ => Vec::new(),
    }
}

/// `pressTitle`: what each step of a press does, said on its hover before it is pressed.
pub fn press_title(steps: &[&Value], slate: &Slate) -> Option<String> {
    let s = |step: &Value, k: &str| step.get(k).and_then(Value::as_str).unwrap_or_default().to_owned();
    let lines: Vec<String> = steps
        .iter()
        .filter_map(|step| {
            Some(match step.get("do")?.as_str()? {
                "send" | "steer" | "queue" | "fill" => {
                    let with: Vec<&str> = step
                        .get("with")
                        .and_then(Value::as_array)
                        .map(|w| w.iter().filter_map(Value::as_str).collect())
                        .unwrap_or_default();
                    if with.is_empty() {
                        s(step, "text")
                    } else {
                        format!("{} (with {})", s(step, "text"), with.join(", "))
                    }
                }
                "start" => format!("Runs {}", s(step, "run")),
                "cancel" => format!("Stops {}", s(step, "run")),
                "set" | "toggle" => format!("Changes {}", s(step, "path")),
                "open" => {
                    let target = step.get("target");
                    let href = target
                        .and_then(|v| crate::screens::slate::model::filled(v, &slate.resolved))
                        .and_then(|v| v.as_str().map(str::to_owned));
                    match href.as_deref().and_then(link_domain) {
                        Some(domain) => format!("Opens {domain}"),
                        None => "Opens a link".to_owned(),
                    }
                }
                "copy" => "Copies".to_owned(),
                "pane" => format!("Opens {}", s(step, "kind")),
                _ => return None,
            })
        })
        .collect();
    (!lines.is_empty()).then(|| lines.join("\n"))
}

/// `hoverTitle`: a note stands in for the step list, but the domain a link would open is always said.
fn hover_title(note: Option<String>, steps: &[&Value], slate: &Slate) -> Option<String> {
    let Some(note) = note else { return press_title(steps, slate) };
    let opens: Vec<&Value> = steps.iter().copied().filter(|s| s.get("do").and_then(Value::as_str) == Some("open")).collect();
    match press_title(&opens, slate) {
        Some(o) => Some(format!("{note}\n{o}")),
        None => Some(note),
    }
}

/// `Outcome`: the one line under a control after a press, a refusal in the refusal ink.
fn outcome(said: Option<&Outcome>, cx: &App) -> Option<AnyElement> {
    let p = cx.palette();
    let (text, ink) = match said? {
        Outcome::Said(s) => (s.clone(), p.muted_foreground),
        Outcome::Refused(r) => (r.clone(), p.error_foreground),
    };
    Some(
        div()
            .debug_selector(|| "slate-outcome".into())
            .text_size(px(12.))
            .line_height(px(16.))
            .text_color(ink)
            .child(text)
            .into_any_element(),
    )
}

/// Raises `event` on `piece` unless its press is out or was raised under 500 ms ago (`usePress`).
fn press(screen: &WeakEntity<SlateScreen>, piece: &str, event: &'static str, last: &Entity<Option<Instant>>, cx: &mut App) {
    let now = Instant::now();
    if last.read(cx).is_some_and(|at| now.duration_since(at) < COALESCE) {
        return;
    }
    let piece = piece.to_owned();
    let Some(screen) = screen.upgrade() else { return };
    if screen.read(cx).slate().is_some_and(|sl| sl.busy.contains(&piece)) {
        return;
    }
    last.update(cx, |l, _| *l = Some(now));
    screen.update(cx, |s, cx| s.raise(piece, event, Raise::default(), cx));
}

/// Where a pick goes: the piece's two-way path, its change raised once the host took the value where it handles one.
#[derive(Clone)]
struct Picker {
    screen: WeakEntity<SlateScreen>,
    piece: String,
    path: Option<String>,
    change: bool,
}

impl Picker {
    fn of(p: &PieceCx, path: Option<String>) -> Self {
        Picker { screen: p.screen.clone(), piece: p.piece.id.clone(), path, change: p.piece.on.contains_key("change") }
    }

    fn pick(&self, value: Value, cx: &mut App) {
        let (Some(path), piece, change) = (self.path.clone(), self.piece.clone(), self.change) else { return };
        _ = self.screen.update(cx, |s, cx| {
            s.write_then(
                path,
                value,
                move |s, cx| {
                    if change {
                        s.raise(piece, "change", Raise::default(), cx);
                    }
                },
                cx,
            )
        });
    }
}

/// The value a two-way prop shows: the slate's at its path, else the prop as the host answered it.
fn current<'a>(p: &'a PieceCx, path: Option<&str>) -> Option<&'a Value> {
    match path {
        Some(path) => p.slate.value(path),
        None => p.prop("value"),
    }
    .filter(|v| !v.is_null())
}

/// A held control's sentence on the hover of a box around it, since a held control takes no pointer.
fn held_hover(id: SharedString, held: Option<String>, control: impl IntoElement) -> AnyElement {
    match held {
        None => control.into_any_element(),
        Some(why) => div()
            .id(id)
            .debug_selector(|| "slate-held".into())
            .flex_none()
            .tooltip(tip(why))
            .child(control)
            .test_support()
            .into_any_element(),
    }
}

fn button(p: &PieceCx, window: &mut Window, cx: &mut App) -> AnyElement {
    let label = p.str("label").unwrap_or_default();
    let held = held_by(p.prop("held"));
    let id = p.el("button");
    let holder = p.slate.doc().and_then(|d| d.parent(p.id())).map(|h| h.kind.as_str());
    let small = matches!(holder, Some("row" | "empty")) && place_of(p.slate, p.id()) == Place::Inside;
    let control = match p.prop("variant").and_then(Value::as_str) {
        Some("primary") if p.slate.is_loud(Loud::Primary, p.id()) => Button::primary(id.clone(), label),
        Some("quiet") => Button::ghost(id.clone(), label),
        Some("danger") => Button::danger(id.clone(), label),
        _ => Button::outline(id.clone(), label),
    };
    let control = if small { control.xs() } else { control };
    let busy = p.busy();
    let last = window.use_keyed_state(p.el("last"), cx, |_, _| None::<Instant>);
    let (screen, piece) = (p.screen.clone(), p.piece.id.clone());
    let steps = steps_of(p.piece.on.get("press"));
    let title = held.is_none().then(|| hover_title(p.str("note"), &steps, p.slate)).flatten();
    let control = control.held(held.is_some()).busy(busy).on_click(move |_, _, cx| press(&screen, &piece, "press", &last, cx));
    let control = match title {
        Some(t) => control.tooltip(t),
        None => control,
    };
    // A held button says why under itself, where the person looks for it, not only on hover.
    let why = held.clone().map(|w| {
        div()
            .debug_selector(|| "slate-held-why".into())
            .text_size(px(12.))
            .line_height(px(16.))
            .text_color(cx.palette().muted_foreground)
            .child(w)
    });
    v_flex()
        .min_w_0()
        .items_start()
        .gap(px(4.))
        .child(held_hover(p.el("held"), held, control))
        .children(why)
        .children(outcome(p.outcome(), cx))
        .into_any_element()
}

/// One field's text, one line or several.
#[derive(Clone)]
enum Text {
    Line(Entity<InputState>),
    Area(Entity<TextareaState>),
}

impl Text {
    fn value(&self, cx: &App) -> String {
        match self {
            Text::Line(s) => s.read(cx).value().to_string(),
            Text::Area(s) => s.read(cx).value().to_string(),
        }
    }

    fn set(&self, text: &str, window: &mut Window, cx: &mut App) {
        let text = text.to_owned();
        match self {
            Text::Line(s) => s.update(cx, |s, cx| s.set_value(text, window, cx)),
            Text::Area(s) => s.update(cx, |s, cx| s.set_value(text, window, cx)),
        }
    }

    fn focus(&self, cx: &App) -> FocusHandle {
        match self {
            Text::Line(s) => s.read(cx).focus_handle(cx),
            Text::Area(s) => s.read(cx).focus_handle(cx),
        }
    }

    /// Whether an input method is composing a word in it, whose Enter is the input method's.
    fn composing(&self, window: &mut Window, cx: &mut App) -> bool {
        match self {
            Text::Line(s) => s.update(cx, |s, cx| s.marked_text_range(window, cx).is_some()),
            Text::Area(s) => s.update(cx, |s, cx| s.marked_text_range(window, cx).is_some()),
        }
    }
}

/// What one input piece keeps between frames.
struct FieldState {
    text: Text,
    screen: WeakEntity<SlateScreen>,
    piece: String,
    path: Option<String>,
    numeric: bool,
    secret: bool,
    /// The host has a secret for the path; an emptied field then clears it.
    filled: bool,
    submits: bool,
    changes: bool,
    focused: bool,
    /// What the field holds, typed and not sent yet.
    pending: Option<Value>,
    timer: Option<Task<()>>,
    /// The values this field sent, so their echo is not read as someone else's write.
    sent: Vec<Value>,
    /// The host's value as this field last saw it while focused.
    seen: Option<Value>,
    /// A value written from elsewhere while the field was focused.
    theirs: Option<Value>,
    conflict: Option<(Value, Value)>,
    /// A secret's text was typed since it was last sent.
    touched: bool,
    refused: Option<String>,
    last: Option<Instant>,
    placeholder: String,
    /// The host took the secret's text, so the next frame empties the field.
    clear: bool,
    _subs: Vec<Subscription>,
}

impl FieldState {
    fn typed(&self, cx: &App) -> Value {
        let text = self.text.value(cx);
        if !self.numeric {
            return Value::String(text);
        }
        text.trim().parse::<f64>().ok().filter(|n| n.is_finite() && !text.trim().is_empty()).map_or(Value::Null, number)
    }

    fn on_event(&mut self, event: &InputEvent, window: &mut Window, cx: &mut Context<Self>) {
        match event {
            InputEvent::Change => self.changed(cx),
            InputEvent::Focus => {
                self.focused = true;
                self.seen = None;
            }
            InputEvent::Blur => self.blurred(cx),
            InputEvent::PressEnter { secondary, shift } => {
                if self.text.composing(window, cx) {
                    return;
                }
                if self.secret {
                    self.send_secret(cx);
                    return;
                }
                let area = matches!(self.text, Text::Area(_));
                if area && self.submits && !secondary && !shift {
                    if let Text::Area(s) = &self.text {
                        s.update(cx, |s, cx| s.insert("\n", window, cx));
                    }
                    return;
                }
                if self.submits && (!area || *secondary) {
                    self.submit(cx);
                }
            }
        }
    }

    fn changed(&mut self, cx: &mut Context<Self>) {
        if self.secret {
            self.touched = true;
            return;
        }
        let Some(path) = self.path.clone() else { return };
        let typed = self.typed(cx);
        self.pending = Some(typed.clone());
        // Kept in `mine` at once, unredrawn, so a thread switch inside the debounce still keeps and sends it.
        _ = self.screen.update(cx, |s, _| {
            if let Load::Drawn(slate) = &mut s.load {
                slate.queue(path, typed);
            }
        });
        self.timer = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(DEBOUNCE).await;
            _ = this.update(cx, |this, cx| this.flush(None, cx));
        }));
    }

    /// Sends what the field holds at once, then raises `then` on the piece; raises it now where nothing waits.
    fn flush(&mut self, then: Option<&'static str>, cx: &mut Context<Self>) {
        self.timer = None;
        let piece = self.piece.clone();
        let raise = move |s: &mut SlateScreen, cx: &mut Context<SlateScreen>| {
            if let Some(event) = then {
                s.raise(piece, event, Raise::default(), cx);
            }
        };
        match (self.pending.take(), self.path.clone()) {
            (Some(value), Some(path)) => {
                self.sent.push(value.clone());
                let over = self.sent.len().saturating_sub(20);
                self.sent.drain(..over);
                _ = self.screen.update(cx, |s, cx| s.write_then(path, value, raise, cx));
            }
            _ => _ = self.screen.update(cx, raise),
        }
    }

    fn blurred(&mut self, cx: &mut Context<Self>) {
        self.focused = false;
        if self.secret {
            self.send_secret(cx);
            return;
        }
        if let Some(theirs) = self.theirs.take() {
            let mine = self.typed(cx);
            if let Some(path) = self.path.clone() {
                let kept = mine.clone();
                _ = self.screen.update(cx, |s, cx| s.keep_drawn(path, kept, cx));
            }
            self.conflict = Some((mine, theirs));
        }
        let then = self.changes.then_some("change");
        if self.path.is_some() || then.is_some() {
            self.flush(then, cx);
        }
        cx.notify();
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        let now = Instant::now();
        if self.last.is_some_and(|at| now.duration_since(at) < COALESCE) {
            return;
        }
        let piece = self.piece.clone();
        if !self.screen.update(cx, |s, cx| s.hold(&piece, cx)).unwrap_or(false) {
            return;
        }
        self.last = Some(now);
        self.flush(Some("submit"), cx);
    }

    /// A secret's text, sent once and never kept: once the host has it the field is empty again and draws its dots.
    fn send_secret(&mut self, cx: &mut Context<Self>) {
        let text = self.text.value(cx);
        let Some(path) = self.path.clone() else { return };
        if !self.touched || (text.is_empty() && !self.filled) {
            return;
        }
        self.touched = false;
        let Ok(sending) = self.screen.update(cx, |s, cx| s.secret(path, text.clone(), cx)) else { return };
        cx.spawn(async move |this, cx| {
            let answer = sending.await;
            _ = this.update(cx, |this, cx| {
                match answer {
                    Ok(()) => {
                        this.refused = None;
                        this.clear = this.text.value(cx) == text;
                    }
                    Err(why) => {
                        this.touched = true;
                        this.refused = Some(why);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
}

impl SlateScreen {
    /// Draws a piece busy from its submit's Enter, through the write before it, until `raise` has the host's answer;
    /// false where it already is.
    pub fn hold(&mut self, piece: &str, cx: &mut Context<Self>) -> bool {
        let crate::screens::slate::Load::Drawn(slate) = &mut self.load else { return false };
        let held = slate.busy.insert(piece.to_owned());
        cx.notify();
        held
    }

    /// Writes a secret's text with `slates.state` without keeping it, and reads the record again for its handle.
    pub fn secret(&mut self, path: String, text: String, cx: &mut Context<Self>) -> Task<Result<(), String>> {
        let Some(thread) = self.opened.thread_id.clone() else { return Task::ready(Err("No thread".into())) };
        let asking = self.host().run(move |host| async move {
            let mut values = serde_json::Map::new();
            values.insert(path, Value::String(text));
            call::<protocol::SlatesStateRequest>(&host, json!({ "threadId": thread, "values": values })).await
        });
        cx.spawn(async move |this, cx| {
            let answer = asking.await.map(|_| ()).map_err(|f| refusal(&f));
            _ = this.update(cx, |this, cx| this.reload(cx));
            answer
        })
    }
}

/// A typed number as JSON writes `Number(text)`: a whole one without a fraction, so the host's echo of it reads as this
/// field's own write.
fn number(n: f64) -> Value {
    if n.fract() == 0.0 && n.abs() <= 9_007_199_254_740_991.0 {
        json!(n as i64)
    } else {
        json!(n)
    }
}

fn as_text(v: Option<&Value>) -> String {
    match v {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(s)) => s.clone(),
        Some(other) => look::str(Some(other)).unwrap_or_default(),
    }
}

/// `fieldFor`: a placeholder the 160 px slot cannot show whole puts the field on its own line: about 17 mono
/// characters, 20 sans.
pub fn own_line(placeholder: Option<&str>, mono: bool) -> bool {
    placeholder.map_or(0, |p| p.chars().count()) > if mono { 17 } else { 20 }
}

/// What Enter does in the field, on the label's line: the submit label after its key.
fn submit_hint(label: String, with_mod: bool, busy: bool, cx: &App) -> AnyElement {
    let key = match (with_mod, cfg!(target_os = "macos")) {
        (false, _) => "↵",
        (true, true) => "⌘ ↵",
        (true, false) => "Ctrl ↵",
    };
    h_flex()
        .debug_selector(|| "slate-submit-hint".into())
        .flex_none()
        .gap(px(4.))
        .text_size(px(12.))
        .line_height(px(16.))
        .text_color(cx.palette().muted_foreground)
        .when(busy, |el| el.opacity(0.64))
        .child(keycap(key.into(), cx))
        .child(label)
        .into_any_element()
}

fn input(p: &PieceCx, window: &mut Window, cx: &mut App) -> AnyElement {
    let pal = cx.palette().clone();
    let path = two_way(p.piece.prop("value"));
    let secret =
        path.as_deref().and_then(|path| path.strip_prefix('$')).is_some_and(|name| p.slate.doc().is_some_and(|d| d.is_secret(name)));
    let lines = if secret { 1 } else { p.num("lines").map_or(1, |n| n.floor().clamp(1., 20.) as usize) };
    let label = p.str("label").unwrap_or_default();
    let held = held_by(p.prop("held"));
    let placeholder = p.str("placeholder");
    let mono = p.bool("mono") && !secret;
    let submits = p.piece.on.contains_key("submit");
    let handle = path.as_deref().and_then(|path| p.slate.record.get_own(path)).cloned();
    let filled = secret && handle.as_ref().and_then(|h| h.get("set")).and_then(Value::as_bool) == Some(true);
    let dots = filled.then(|| {
        let len = handle.as_ref().and_then(|h| h.get("len")).and_then(Value::as_f64).unwrap_or(8.);
        "•".repeat(len.clamp(4., 24.) as usize)
    });
    let shown_placeholder = dots.clone().or(placeholder.clone()).unwrap_or_default();
    let (screen, piece, init_path) = (p.screen.clone(), p.piece.id.clone(), path.clone());
    let password = secret || p.prop("kind").and_then(Value::as_str) == Some("password");
    let state = window.use_keyed_state(p.el("field"), cx, |window, cx| {
        let text = if lines > 1 {
            let area = cx.new(|cx| TextareaState::new(window, cx).auto_grow(3, 20).submit_on_enter(submits));
            Text::Area(area)
        } else {
            Text::Line(cx.new(|cx| InputState::new(window, cx).masked(password)))
        };
        let sub = match &text {
            Text::Line(s) => cx.subscribe_in(s, window, |this: &mut FieldState, _, e, window, cx| this.on_event(e, window, cx)),
            Text::Area(s) => cx.subscribe_in(s, window, |this: &mut FieldState, _, e, window, cx| this.on_event(e, window, cx)),
        };
        FieldState {
            text,
            screen,
            piece,
            path: init_path,
            numeric: false,
            secret,
            filled,
            submits,
            changes: false,
            focused: false,
            pending: None,
            timer: None,
            sent: Vec::new(),
            seen: None,
            theirs: None,
            conflict: None,
            touched: false,
            refused: None,
            last: None,
            placeholder: String::new(),
            clear: false,
            _subs: vec![sub],
        }
    });
    let host = path.as_deref().and_then(|path| p.slate.record.get_own(path)).cloned();
    let stored = if secret { None } else { current(p, path.as_deref()).cloned() };
    let (numeric, changes) = (p.prop("kind").and_then(Value::as_str) == Some("number"), p.piece.on.contains_key("change"));
    let text = state.update(cx, |f, cx| {
        f.numeric = numeric;
        f.changes = changes;
        f.submits = submits;
        f.filled = filled;
        if f.focused {
            if f.seen.is_none() {
                f.seen = Some(host.clone().unwrap_or(Value::Null));
            } else if f.seen.as_ref() != Some(&host.clone().unwrap_or(Value::Null)) {
                let theirs = host.clone().unwrap_or(Value::Null);
                f.seen = Some(theirs.clone());
                if !f.sent.contains(&theirs) && theirs != f.typed(cx) {
                    f.theirs = Some(theirs);
                }
            }
        }
        f.text.clone()
    });
    if state.read(cx).clear {
        state.update(cx, |f, _| f.clear = false);
        text.set("", window, cx);
    }
    let focused = text.focus(cx).is_focused(window);
    // The frame that loses the focus draws before the blur reaches the field, which holds the typed text until then.
    if !focused && !secret && !state.read(cx).focused && state.read(cx).pending.is_none() {
        let want = as_text(stored.as_ref());
        if text.value(cx) != want {
            text.set(&want, window, cx);
        }
    }
    if state.read(cx).placeholder != shown_placeholder {
        state.update(cx, |f, _| f.placeholder = shown_placeholder.clone());
        match &text {
            Text::Line(s) => s.update(cx, |s, cx| s.set_placeholder(shown_placeholder.clone(), window, cx)),
            Text::Area(s) => s.update(cx, |s, cx| s.set_placeholder(shown_placeholder.clone(), window, cx)),
        }
    }
    let line = lines == 1 && own_line(placeholder.as_deref(), mono);
    let field = match &text {
        Text::Line(s) => {
            let f = Field::new(s).held(held.is_some()).width(if line {
                gpui_kit::relative(1.)
            } else {
                gpui_kit::DefiniteLength::from(px(SLOT))
            });
            let f = if mono { f.mono() } else { f };
            let id = p.el("line");
            div()
                .id(id.clone())
                .debug_selector(|| "slate-field".into())
                .when(line, |el| el.w_full())
                .when(!line, |el| el.max_w_full())
                .flex_none()
                .map(|el| match held.clone() {
                    Some(why) => el.tooltip(tip(why)),
                    None => el,
                })
                .child(f)
                .into_any_element()
        }
        Text::Area(s) => {
            let dim = held.is_some();
            let held_look = crate::ui::controls::Dim::new(pal.background, pal.input_fill);
            let (edge, fill) = if dim { (held_look.over(pal.input), held_look.fill()) } else { (pal.input, pal.input_fill) };
            let area = Textarea::new(s)
                .disabled(dim)
                .w_full()
                .rounded(size::BUTTON_RADIUS)
                .border_1()
                .border_color(edge)
                .bg(fill)
                .px(px(11.))
                .py(px(5.))
                .text_size(text::VALUE.0)
                .line_height(px(20.))
                .text_color(pal.foreground)
                .when(mono, |el| el.font_family(cx.theme().mono_font_family.clone()));
            div()
                .id(p.el("area"))
                .debug_selector(|| "slate-field".into())
                .w_full()
                .map(|el| match held.clone() {
                    Some(why) => el.tooltip(tip(why)),
                    None => el,
                })
                .child(area)
                .into_any_element()
        }
    };
    let hint = (submits && held.is_none() && !secret)
        .then(|| submit_hint(p.str("submit").unwrap_or_else(|| words::SEND.into()), lines > 1, p.busy(), cx));
    let head = h_flex()
        .flex_1()
        .min_w_0()
        .items_baseline()
        .gap(px(8.))
        .child(div().flex_1().min_w_0().text_size(px(14.)).line_height(px(20.)).text_color(pal.foreground).child(label))
        .children(hint);
    let row = h_flex()
        .debug_selector(|| "slate-input-line".into())
        .min_w_0()
        .flex_wrap()
        .justify_between()
        .gap_x(px(16.))
        .gap_y(px(6.))
        .child(head);
    let row =
        if lines == 1 { row.child(field).into_any_element() } else { v_flex().gap(px(6.)).child(row).child(field).into_any_element() };
    let f = state.read(cx);
    let conflict = f.conflict.clone().filter(|_| path.is_some()).map(|(mine, theirs)| {
        let (keep, take, path_a, path_b, screen_a, screen_b) = (
            state.clone(),
            state.clone(),
            path.clone().unwrap_or_default(),
            path.clone().unwrap_or_default(),
            p.screen.clone(),
            p.screen.clone(),
        );
        h_flex()
            .debug_selector(|| "slate-conflict".into())
            .flex_wrap()
            .gap(px(8.))
            .text_size(px(12.))
            .line_height(px(16.))
            .text_color(pal.muted_foreground)
            .child(words::CHANGED)
            .child(Button::outline(p.el("keep"), words::KEEP).xs().on_click(move |_, _, cx| {
                keep.update(cx, |f, cx| {
                    f.conflict = None;
                    cx.notify();
                });
                _ = screen_a.update(cx, |s, cx| s.write(path_a.clone(), mine.clone(), cx));
            }))
            .child(Button::ghost(p.el("theirs"), words::THEIRS).xs().on_click(move |_, window, cx| {
                let field = take.update(cx, |f, cx| {
                    f.conflict = None;
                    cx.notify();
                    f.text.clone()
                });
                field.set(&as_text(Some(&theirs)), window, cx);
                _ = screen_b.update(cx, |s, cx| s.write(path_b.clone(), theirs.clone(), cx));
            }))
    });
    let refused = f.refused.clone().map(Outcome::Refused);
    v_flex().min_w_0().gap(px(6.)).child(row).children(conflict).children(outcome(refused.as_ref().or(p.outcome()), cx)).into_any_element()
}

/// The options of a select: a list of words, or of `{ value, label }`.
fn options_of(list: &[Value]) -> Vec<(String, String)> {
    list.iter()
        .filter_map(|o| match o {
            Value::String(_) | Value::Number(_) => {
                let v = look::str(Some(o))?;
                Some((v.clone(), v))
            }
            Value::Object(r) => {
                let v = look::str(r.get("value"))?;
                Some((v.clone(), look::str(r.get("label")).unwrap_or(v)))
            }
            _ => None,
        })
        .collect()
}

/// A piece's list prop with the items whose own `when` holds.
fn present(p: &PieceCx, name: &str) -> Vec<Value> {
    match p.prop(name) {
        Some(Value::Array(list)) => p.slate.present(p.piece.prop(name), list),
        _ => Vec::new(),
    }
}

/// How wide a bare select's trigger stands to hold its word: `w-auto` over the trigger's `min-w-36`.
fn trigger_width(words: &str, window: &Window) -> gpui_kit::Pixels {
    let font = window.text_style().font();
    let run = TextRun { len: words.len(), font, color: gpui_kit::black(), background_color: None, underline: None, strikethrough: None };
    let shaped = window.text_system().shape_line(words.to_owned().into(), text::FACT.0, &[run], None);
    (shaped.width + px(9. * 2. + 8. + 12. - 4. + 2.)).max(px(144.))
}

fn select(p: &PieceCx, window: &mut Window, cx: &mut App) -> AnyElement {
    let pal = cx.palette().clone();
    let path = two_way(p.piece.prop("value"));
    let options = options_of(&present(p, "options"));
    let value = look::str(current(p, path.as_deref()));
    let label = p.str("label").unwrap_or_default();
    let held = held_by(p.prop("held"));
    let bare = p.slate.doc().and_then(|d| d.parent_id(p.id())).is_some_and(|parent| is_toolbar(p.slate, parent));
    let items = || options.iter().map(|(v, l)| SelectItem::new(v.clone(), l.clone())).collect::<Vec<_>>();
    let state = window.use_keyed_state(p.el("select"), cx, |_, _| SelectState::new(items(), value.as_ref()));
    let had: Vec<(String, String)> = options.clone();
    let kept = window.use_keyed_state(p.el("select-items"), cx, |_, _| had.clone());
    if *kept.read(cx) != options {
        kept.update(cx, |k, _| *k = options.clone());
        state.update(cx, |s, cx| s.set_items(items(), cx));
    }
    if state.read(cx).value() != value.as_ref() {
        state.update(cx, |s, cx| s.set_value(value.as_ref(), cx));
    }
    let placeholder = p.str("placeholder").unwrap_or_else(|| words::PICK.into());
    let shown = value
        .as_ref()
        .and_then(|v| options.iter().find(|(o, _)| o == v).map(|(_, l)| l.clone()).or(Some(v.clone())))
        .unwrap_or(placeholder.clone());
    let picker = Picker::of(p, path.clone());
    let control = Select::new(&state)
        .held(held.is_some() || path.is_none())
        .placeholder(placeholder)
        .on_change(move |v: &String, _, cx| picker.pick(json!(v), cx));
    let control = if bare { control.width(trigger_width(&shown, window)) } else { control };
    h_flex()
        .debug_selector(|| "slate-select".into())
        .min_w_0()
        .flex_wrap()
        .justify_between()
        .gap_x(px(16.))
        .gap_y(px(6.))
        .when(!bare, |el| el.child(div().min_w_0().text_size(px(14.)).line_height(px(20.)).text_color(pal.foreground).child(label)))
        .child(held_hover(p.el("held"), held, control))
        .into_any_element()
}

/// One option of a choices piece.
struct Choice {
    value: Value,
    label: String,
    note: Option<String>,
}

fn choices_of(list: &[Value]) -> Vec<Choice> {
    list.iter()
        .map(|o| match o {
            Value::Object(r) if r.contains_key("value") => Choice {
                value: r.get("value").cloned().unwrap_or(Value::Null),
                label: look::str(r.get("label")).or_else(|| look::str(r.get("value"))).unwrap_or_default(),
                note: look::str(r.get("note")),
            },
            other => Choice { value: other.clone(), label: look::str(Some(other)).unwrap_or_default(), note: None },
        })
        .collect()
}

fn same(a: Option<&Value>, b: Option<&Value>) -> bool {
    matches!(a, Some(a) if !a.is_null() && Some(a) == b)
}

/// The row a key picks from the group's stop: the arrows the next or last round the ends, Enter and Space the stop.
fn choice_key(key: &str, stop: usize, n: usize) -> Option<usize> {
    match key {
        "down" | "right" => Some((stop + 1) % n),
        "up" | "left" => Some((stop + n - 1) % n),
        "enter" | "space" => Some(stop),
        _ => None,
    }
}

/// `decision.tsx`: the question as a setting's title, the answers as outline keycaps and the agent's pick named under
/// them with its reason; once answered, the answer with a check and a quiet Change, which clears it.
fn decision(p: &PieceCx, _window: &mut Window, cx: &mut App) -> AnyElement {
    let pal = cx.palette().clone();
    let path = two_way(p.piece.prop("value"));
    let picked = current(p, path.as_deref()).cloned().filter(|v| !v.is_null() && v.as_str() != Some(""));
    let open = held_by(p.prop("held")).is_none() && path.is_some() && !p.busy();
    let list: Vec<Choice> = choices_of(&present(p, "options")).into_iter().take(4).collect();
    let mine = list.iter().find(|c| same(Some(&c.value), p.prop("pick")));
    let answered = list.iter().find(|c| same(Some(&c.value), picked.as_ref()));
    let mut others = list.iter().filter(|c| !mine.is_some_and(|m| std::ptr::eq(m, *c)) && c.note.is_some()).peekable();
    let title = div()
        .text_size(px(14.))
        .line_height(px(20.))
        .font_weight(if answered.is_none() { FontWeight::MEDIUM } else { FontWeight::NORMAL })
        .text_color(if answered.is_none() { pal.foreground } else { pal.muted_foreground })
        .children(p.str("label"));
    let body = match answered {
        None => v_flex()
            .min_w_0()
            .gap(px(10.))
            .child(h_flex().min_w_0().flex_wrap().gap(px(8.)).children(list.iter().enumerate().map(|(ix, c)| {
                let (picker, value) = (Picker::of(p, path.clone()), c.value.clone());
                Button::outline(p.el(&format!("answer-{ix}")), c.label.clone())
                    .held(!open)
                    .on_click(move |_, _, cx| picker.pick(value.clone(), cx))
            })))
            .when(mine.is_some() || others.peek().is_some(), |el| {
                // The other answers' notes stand under the pick's, quieter, so each answer's cost reads without a hover.
                el.child(
                    v_flex()
                        .min_w_0()
                        .gap(px(4.))
                        .text_size(px(12.))
                        .line_height(px(16.))
                        .children(mine.map(|c| {
                            let why = c.note.as_ref().map(|n| format!(". {n}")).unwrap_or_default();
                            div().text_color(pal.muted_foreground).child(format!("My pick: {}{why}", c.label))
                        }))
                        .children(others.map(|c| {
                            div()
                                .debug_selector(|| "slate-answer-note".into())
                                .text_color(pal.muted_foreground.opacity(0.7))
                                .child(format!("{}: {}", c.label, c.note.clone().unwrap_or_default()))
                        })),
                )
            })
            .into_any_element(),
        Some(c) => {
            let picker = Picker::of(p, path.clone());
            h_flex()
                .min_w_0()
                .items_center()
                .gap(px(8.))
                .child(Icon::new(Glyph::Check).with_size(px(16.)).flex_none().text_color(pal.success))
                .child(div().flex_1().min_w_0().text_size(px(14.)).line_height(px(20.)).text_color(pal.foreground).child(c.label.clone()))
                .child(
                    Button::ghost(p.el("change"), "Change")
                        .xs()
                        .held(!open)
                        .on_click(move |_, _, cx| picker.pick(Value::String(String::new()), cx)),
                )
                .into_any_element()
        }
    };
    v_flex()
        .min_w_0()
        .gap(px(10.))
        .debug_selector(move || format!("slate-decision-{}", if answered.is_some() { "answered" } else { "open" }))
        .child(title)
        .child(body)
        .children(outcome(p.outcome(), cx))
        .into_any_element()
}

fn choices(p: &PieceCx, window: &mut Window, cx: &mut App) -> AnyElement {
    let pal = cx.palette().clone();
    let path = two_way(p.piece.prop("value"));
    let picked = current(p, path.as_deref()).cloned();
    let answer = p.prop("answer").cloned();
    let held = held_by(p.prop("held"));
    let label = p.str("label").unwrap_or_default();
    let known = picked.is_some() && answer.is_some();
    let place = place_of(p.slate, p.id());
    let inset = if place == Place::Inside { px(0.) } else { super::layout::INSET };
    let list = choices_of(&present(p, "options"));
    let open = held.is_none() && path.is_some();
    let focus = window.use_keyed_state(p.el("focus"), cx, |_, cx| cx.focus_handle().tab_stop(true)).read(cx).clone();
    let ringed = focus.is_focused(window) && window.last_input_was_keyboard();
    let at = list.iter().position(|c| same(Some(&c.value), picked.as_ref()));
    // One stop, the picked row's or else the first, as a native radio group keeps.
    let stop = at.unwrap_or(0);
    let rule = pal.border.opacity(0.5);
    let rows: Vec<AnyElement> = list
        .iter()
        .enumerate()
        .map(|(ix, choice)| {
            let chosen = at == Some(ix);
            let right = known && same(Some(&choice.value), answer.as_ref());
            let wrong = known && chosen && !right;
            let word = if right {
                Some((words::RIGHT, pal.success))
            } else if wrong {
                Some((words::WRONG, pal.error_foreground))
            } else {
                None
            };
            let mark = word.map(|(w, ink)| {
                div().flex_none().text_size(px(13.)).line_height(px(20.)).font_weight(FontWeight::MEDIUM).text_color(ink).child(w)
            });
            let radio = if chosen {
                Icon::new(Glyph::CircleDot).with_size(px(16.)).flex_none().text_color(pal.foreground)
            } else {
                Icon::new(Glyph::Circle).with_size(px(16.)).flex_none().text_color(pal.muted_foreground.opacity(0.7))
            };
            let state = if right {
                "right"
            } else if wrong {
                "wrong"
            } else if chosen {
                "picked"
            } else {
                "open"
            };
            let ring = ix == stop && ringed;
            let id = p.el(&format!("choice-{ix}"));
            let over = motion::hovered(id.clone(), window, cx);
            let fill = pal.accent.opacity(if open && *over.read(cx) { 0.6 } else { 0. });
            let fill = motion::ease_ink(id.clone(), "fill", fill, motion::COLORS, window, cx);
            let (value, picker, focus) = (choice.value.clone(), Picker::of(p, path.clone()), focus.clone());
            h_flex()
                .id(id)
                .debug_selector(move || format!("slate-choice-{state}{}", if ring { "-ring" } else { "" }))
                .min_h(px(48.))
                .w_full()
                .min_w_0()
                .items_center()
                .gap(px(12.))
                .py(px(12.))
                .px(inset)
                .when(ix > 0, |el| el.border_t_1().border_color(rule))
                .bg(fill)
                .when(ring, |el| el.shadow(vec![crate::ui::focus::ring_inset(pal.ring)]))
                .when_some(held.clone(), |el, why| el.tooltip(tip(why)))
                .when(open, |el| {
                    el.cursor_pointer().on_hover(motion::track_hover(over)).on_click(move |_, window, cx| {
                        window.focus(&focus, cx);
                        picker.pick(value.clone(), cx)
                    })
                })
                .child(div().flex_none().debug_selector(|| "slate-choice-radio".into()).child(radio))
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .child(
                            div()
                                .text_size(px(14.))
                                .line_height(px(20.))
                                .text_color(if at.is_some() && !chosen { pal.muted_foreground } else { pal.foreground })
                                .child(choice.label.clone()),
                        )
                        .children(
                            choice
                                .note
                                .clone()
                                .map(|n| div().text_size(px(13.)).line_height(px(20.)).text_color(pal.muted_foreground).child(n)),
                        ),
                )
                .children(mark)
                .into_any_element()
        })
        .collect();
    let values: Vec<Value> = list.iter().map(|c| c.value.clone()).collect();
    let picker = Picker::of(p, path.clone());
    let options = v_flex()
        .id(p.el("choices"))
        .debug_selector(|| "slate-choices-card".into())
        .min_w_0()
        .when(open && !values.is_empty(), |el| {
            el.track_focus(&focus).on_key_down(move |e: &KeyDownEvent, _, cx| {
                let Some(to) = choice_key(&e.keystroke.key, stop, values.len()) else { return };
                cx.stop_propagation();
                picker.pick(values[to].clone(), cx);
            })
        })
        .when(place == Place::Page, |el| {
            el.overflow_hidden().rounded(size::CARD_RADIUS).border_1().border_color(pal.border.opacity(0.6)).bg(pal.card.opacity(0.4))
        })
        .children(rows);
    // The question is a setting's title over a card of its own, so questions one after another stand apart.
    v_flex()
        .debug_selector(|| "slate-choices".into())
        .min_w_0()
        .gap(px(10.))
        .when(!label.is_empty(), |el| {
            el.child(
                div()
                    .debug_selector(|| "slate-choices-title".into())
                    .text_size(px(14.))
                    .line_height(px(20.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(pal.foreground)
                    .child(label.clone()),
            )
        })
        .child(options)
        .into_any_element()
}

/// A toggle in the toolbar beside a select, which filters the list under it.
fn filters(slate: &Slate, id: &str) -> bool {
    let Some(doc) = slate.doc() else { return false };
    let Some(parent) = doc.parent_id(id) else { return false };
    is_toolbar(slate, parent)
        && doc.piece(parent).is_some_and(|row| row.children.iter().any(|c| doc.piece(c).is_some_and(|c| c.kind == "select")))
}

fn toggle(p: &PieceCx, _window: &mut Window, cx: &mut App) -> AnyElement {
    let pal = cx.palette().clone();
    let path = two_way(p.piece.prop("value"));
    let on = truthy(current(p, path.as_deref()));
    let label = p.str("label").unwrap_or_default();
    let note = p.str("note");
    let held = held_by(p.prop("held"));
    let still = held.is_some() || path.is_none();
    let picker = Picker::of(p, path.clone());
    let set = move |next: bool, cx: &mut App| picker.pick(json!(next), cx);
    if filters(p.slate, p.id()) {
        let set = set.clone();
        let control = div().debug_selector(|| "slate-toggle-segments".into()).flex_none().child(
            Segmented::new(p.el("segments"), Some(on as usize))
                .keycap()
                .held(still)
                .segment(words::ALL)
                .segment(label)
                .on_pick(move |ix, _, cx| set(ix == 1, cx)),
        );
        return held_hover(p.el("held"), held, control);
    }
    let words_set = set.clone();
    h_flex()
        .debug_selector(|| "slate-toggle".into())
        .min_w_0()
        .justify_between()
        .gap(px(16.))
        .child(
            v_flex()
                .id(p.el("label"))
                .flex_1()
                .min_w_0()
                .gap(px(2.))
                .when(!still, |el| el.cursor_pointer().on_click(move |_, _, cx| words_set(!on, cx)))
                .child(
                    div().text_size(px(14.)).line_height(px(20.)).font_weight(FontWeight::MEDIUM).text_color(pal.foreground).child(label),
                )
                .children(note.map(|n| div().text_size(px(13.)).line_height(px(18.85)).text_color(pal.muted_foreground).child(n))),
        )
        .child(held_hover(p.el("held"), held, Switch::new(p.el("switch"), on).held(still).on_change(move |next, _, cx| set(*next, cx))))
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{held_by, own_line};

    #[test]
    fn only_words_hold_a_control() {
        assert_eq!(held_by(Some(&json!("Waiting on the build"))), Some("Waiting on the build".into()));
        assert_eq!(held_by(Some(&json!("  "))), None);
        assert_eq!(held_by(Some(&json!(false))), None);
        assert_eq!(held_by(None), None);
    }

    #[test]
    fn a_long_placeholder_puts_the_field_on_its_own_line() {
        assert!(!own_line(Some("Ada"), false));
        assert!(own_line(Some("a placeholder of 21 c"), false));
        assert!(!own_line(Some("a placeholder of 20"), false));
        assert!(own_line(Some("eighteen chars mon"), true));
    }
}
