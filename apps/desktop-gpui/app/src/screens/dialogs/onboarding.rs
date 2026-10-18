// SPDX-License-Identifier: AGPL-3.0-only
//! The onboarding window (`desktop:onboarding.html`): the welcome, the mark alone with the accent falling on it, then
//! the agents screen, one row per catalog agent with the found ones ticked, and the one press that gives the ticked
//! agents the wsp tools and opens the app. It follows the computer's light or dark side, as no preference exists yet.

use std::collections::HashSet;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use gpui_kit::base::Easing;
use gpui_kit::component::{h_flex, v_flex, ActiveTheme, Root};
use gpui_kit::{
    div, img, prelude::*, px, relative, Animation, AnimationExt as _, AnyElement, App, Context, FocusHandle, FontWeight, Hsla, Image,
    ImageFormat, KeyDownEvent, ObjectFit, Rgba, SharedString, Subscription, Task, TextAlign, Window, WindowAppearance,
};

use super::startup::{self, Agent};
use crate::screens::Host;
use crate::ui::status::still;
use crate::ui::{self, catalog, checkbox_plain, tip, ActivePalette, Button, Palette, Refusal};

pub const HEADING: &str = "Your agents drive wsp";
pub const GET_STARTED: &str = "Get started";
pub const OPEN: &str = "Open wsp";
pub const CHECK_AGAIN: &str = "Check again";
pub const SOME: &str = "Agents you install later get the tools from Settings.";
pub const NONE_TICKED: &str = "Tick at least one agent. wsp works through the agents you give it.";
pub const UNREAD_FIX: &str = "Press Check again to read them again.";
pub const TOOLS_FIX: &str = "Take the tick off that agent, or fix its config and press again.";
pub const SHELL_FIX: &str = "Press Open wsp again.";

/// " on X" once the computer's name has landed, else nothing, so no sentence ends on a word standing in for it.
fn on(here: Option<&str>) -> String {
    here.map(|h| format!(" on {h}")).unwrap_or_default()
}

pub fn sentence(here: Option<&str>) -> String {
    format!("Each ticked agent gets the wsp tools and skill, so it can open threads and tasks{}.", on(here))
}

pub fn none_found(here: Option<&str>) -> String {
    format!("No agent found{}. Threads start once you install one. Settings then gives it the tools.", on(here))
}

pub fn unread(here: Option<&str>) -> String {
    format!("The agents{} could not be read.", on(here))
}

pub fn shell_refused(here: Option<&str>) -> String {
    format!("wsp could not open{}.", on(here))
}

/// Names in a sentence: one, two joined by and, more with commas before the and.
pub fn list(names: &[String]) -> String {
    match names {
        [] => String::new(),
        [one] => one.clone(),
        [a, b] => format!("{a} and {b}"),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

/// The same, capped for a refusal: three names whole, past that the first two and how many more.
pub fn capped(names: &[String]) -> String {
    if names.len() <= 3 {
        list(names)
    } else {
        format!("{}, {} and {} more", names[0], names[1], names.len() - 2)
    }
}

pub fn tools_refused(names: &[String]) -> String {
    match names {
        [one] => format!("{one}'s config would not take the wsp tools."),
        _ => format!("{} would not take the wsp tools.", capped(names)),
    }
}

pub fn state_word(agent: &Agent) -> &'static str {
    if agent.installs {
        "installs on first run"
    } else if agent.found {
        "installed"
    } else {
        "not installed"
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Welcome,
    Agents,
}

/// A refusal in its two halves, what happened and what to do, with the host's own words for the hover.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refused {
    pub happened: String,
    pub fix: String,
    pub said: Option<String>,
}

/// What Open wsp does once the tools are in.
pub type Finish = Rc<dyn Fn(&mut Window, &mut App)>;

/// What the screen reads and writes: the host, and what opening the app does.
#[derive(Clone)]
pub struct Road {
    pub host: Host,
    pub finish: Finish,
}

pub struct Onboarding {
    road: Option<Road>,
    step: Step,
    here: Option<String>,
    /// Found agents first, in the catalog's order, then the rest; none until the scan answers.
    agents: Option<Vec<Agent>>,
    ticked: HashSet<String>,
    /// The last scan refused: a computer whose agents could not be read is not one with none.
    unread: bool,
    refused: Option<Refused>,
    busy: bool,
    art: Option<(&'static str, Art)>,
    focus: FocusHandle,
    _scan: Option<Task<()>>,
    _press: Option<Task<()>>,
    _appearance: Option<Subscription>,
}

impl Onboarding {
    /// The screen on the host, reading the computer's name and its agents while the mark draws.
    pub fn new(road: Road, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let appearance = cx.observe_window_appearance(window, |_, window, cx| follow_side(window, cx));
        follow_side(window, cx);
        let mut this = Onboarding::blank(Some(road), window, cx);
        this._appearance = Some(appearance);
        this.scan(true, window, cx);
        this
    }

    /// A state staged from fixture answers, with no host behind it.
    pub fn staged(
        step: Step,
        here: Option<&str>,
        scan: Result<Vec<Agent>, String>,
        refused: Option<Refused>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut this = Onboarding::blank(None, window, cx);
        this.step = step;
        this.here = here.map(str::to_owned);
        this.take(scan);
        if refused.is_some() {
            this.refused = refused;
        }
        this
    }

    fn blank(road: Option<Road>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        Onboarding {
            road,
            step: Step::Welcome,
            here: None,
            agents: None,
            ticked: HashSet::new(),
            unread: false,
            refused: None,
            busy: false,
            art: None,
            focus,
            _scan: None,
            _press: None,
            _appearance: None,
        }
    }

    pub fn step(&self) -> Step {
        self.step
    }

    pub fn here(&self) -> Option<&str> {
        self.here.as_deref()
    }

    pub fn agents(&self) -> &[Agent] {
        self.agents.as_deref().unwrap_or_default()
    }

    pub fn ticked(&self) -> Vec<String> {
        self.agents().iter().filter(|a| self.ticked.contains(&a.id)).map(|a| a.id.clone()).collect()
    }

    pub fn refused(&self) -> Option<&Refused> {
        self.refused.as_ref()
    }

    pub fn busy(&self) -> bool {
        self.busy
    }

    fn found(&self) -> usize {
        self.agents().iter().filter(|a| a.found).count()
    }

    /// The keycap's word.
    pub fn button_word(&self) -> &'static str {
        if self.unread {
            CHECK_AGAIN
        } else {
            OPEN
        }
    }

    /// The line under the card: what the ticks mean, or why the keycap is held.
    pub fn line(&self) -> String {
        if self.unread || self.agents.is_none() {
            String::new()
        } else if self.found() == 0 {
            none_found(self.here())
        } else if self.ticked.is_empty() {
            NONE_TICKED.to_owned()
        } else {
            SOME.to_owned()
        }
    }

    /// The keycap is held while the scan has not answered, while a press runs, and while agents are here and none is
    /// ticked.
    pub fn held(&self) -> bool {
        self.busy || (self.agents.is_none() && !self.unread) || (!self.unread && self.found() > 0 && self.ticked.is_empty())
    }

    fn take(&mut self, scan: Result<Vec<Agent>, String>) {
        match scan {
            Ok(agents) => {
                let (found, rest): (Vec<Agent>, Vec<Agent>) = agents.into_iter().partition(|a| a.found);
                self.ticked = found.iter().map(|a| a.id.clone()).collect();
                self.agents = Some(found.into_iter().chain(rest).collect());
                self.unread = false;
                self.refused = None;
            }
            Err(said) => {
                self.agents = Some(Vec::new());
                self.ticked.clear();
                self.unread = true;
                self.refused = Some(Refused { happened: unread(self.here()), fix: UNREAD_FIX.to_owned(), said: Some(said) });
            }
        }
    }

    /// `scan`: the agents here, after the computer's name where it is still to be read.
    fn scan(&mut self, name: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(road) = self.road.clone() else { return };
        let naming = name.then(|| cx.background_spawn(async { startup::computer_name() }));
        let read = road.host.run(|c| async move { startup::agents(&c).await });
        self._scan = Some(cx.spawn_in(window, async move |this, cx| {
            if let Some(naming) = naming {
                let here = naming.await;
                _ = this.update(cx, |this, cx| {
                    this.here = here;
                    cx.notify();
                });
            }
            let scan = read.await.map_err(|f| f.message);
            _ = this.update(cx, |this, cx| {
                this.busy = false;
                this.take(scan);
                cx.notify();
            });
        }));
    }

    pub fn toggle(&mut self, id: &str, cx: &mut Context<Self>) {
        if self.busy || !self.agents().iter().any(|a| a.id == id && a.found) {
            return;
        }
        if !self.ticked.remove(id) {
            self.ticked.insert(id.to_owned());
        }
        cx.notify();
    }

    /// Get started, Open wsp or Check again, whichever the screen shows; Enter presses it.
    pub fn press(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.step {
            Step::Welcome => {
                self.step = Step::Agents;
                cx.notify();
            }
            Step::Agents => self.open_wsp(window, cx),
        }
    }

    /// `openWsp`: the tools into every ticked agent, then the app. A config that refused says which and stays here, so
    /// its tick can come off and the press go again.
    fn open_wsp(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.held() {
            return;
        }
        let Some(road) = self.road.clone() else { return };
        self.busy = true;
        cx.notify();
        if self.unread {
            self.scan(false, window, cx);
            return;
        }
        self.refused = None;
        let ids = self.ticked();
        let names: Vec<(String, String)> = self.agents().iter().map(|a| (a.id.clone(), a.name.clone())).collect();
        let installs = (!ids.is_empty()).then(|| road.host.run(|c| async move { Ok(startup::add_tools(&c, ids).await) }));
        self._press = Some(cx.spawn_in(window, async move |this, cx| {
            let installed = match installs {
                Some(installs) => installs.await,
                None => Ok(Vec::new()),
            };
            _ = this.update_in(cx, |this, window, cx| {
                this.busy = false;
                cx.notify();
                match installed {
                    Ok(refused) if refused.is_empty() => (road.finish.clone())(window, cx),
                    Ok(refused) => {
                        let named: Vec<String> = refused
                            .iter()
                            .map(|(id, _)| names.iter().find(|(i, _)| i == id).map_or_else(|| id.clone(), |(_, n)| n.clone()))
                            .collect();
                        let said = refused.iter().map(|(id, e)| format!("{id}: {e}")).collect::<Vec<_>>().join("\n");
                        this.refused = Some(Refused { happened: tools_refused(&named), fix: TOOLS_FIX.to_owned(), said: Some(said) });
                    }
                    Err(f) => {
                        this.refused =
                            Some(Refused { happened: shell_refused(this.here()), fix: SHELL_FIX.to_owned(), said: Some(f.message) });
                    }
                }
            });
        }));
    }
}

/// The welcome's art in the theme's inks, drawn once per theme.
struct Art {
    tilde: Arc<Image>,
    glow: Arc<Image>,
    halo: Arc<Image>,
}

fn hex(color: Hsla) -> String {
    let c = Rgba::from(color);
    let b = |v: f32| (v.clamp(0., 1.) * 255.).round() as u8;
    format!("#{:02x}{:02x}{:02x}", b(c.r), b(c.g), b(c.b))
}

fn svg(text: String) -> Arc<Image> {
    Arc::new(Image::from_bytes(ImageFormat::Svg, text.into_bytes()))
}

impl Art {
    fn new(p: &Palette, dark: bool) -> Self {
        let primary = hex(p.primary);
        // The mark lit from above: a thin light along its top edge and a thin shade along its bottom, each cut from the
        // stroke's own outline moved 0.3 units, as the page's filter draws it.
        let (lit, lit_alpha) = (hex(p.primary_foreground), if dark { 1. } else { 0.75 });
        let shade = hex(if dark { p.background } else { p.foreground });
        let tilde = format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="320" height="200" viewBox="0 3 16 10" fill="none" stroke="{ink}" stroke-linecap="round"><defs><filter id="edges" filterUnits="userSpaceOnUse" x="-1" y="2" width="18" height="12"><feOffset in="SourceAlpha" dy="0.3" result="down"/><feComposite in="SourceAlpha" in2="down" operator="out" result="top"/><feFlood flood-color="{lit}" flood-opacity="{lit_alpha}" result="light"/><feComposite in="light" in2="top" operator="in" result="rim"/><feOffset in="SourceAlpha" dy="-0.3" result="up"/><feComposite in="SourceAlpha" in2="up" operator="out" result="bottom"/><feFlood flood-color="{shade}" flood-opacity="0.4" result="ink"/><feComposite in="ink" in2="bottom" operator="in" result="under"/><feMerge><feMergeNode in="SourceGraphic"/><feMergeNode in="rim"/><feMergeNode in="under"/></feMerge></filter></defs><g filter="url(#edges)" stroke-width="2.6"><path d="M1.6 8A3.4 3.4 0 0 1 8 8A3.4 3.4 0 0 0 14.4 8"/></g></svg>"##,
            ink = hex(p.foreground),
        );
        // `radial-gradient(60% 75% at 50% 0%, primary 22%, transparent)`, small and stretched to the window.
        let glow = format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="200" viewBox="0 0 200 200" preserveAspectRatio="none"><defs><radialGradient id="g" gradientUnits="userSpaceOnUse" cx="100" cy="0" r="100" gradientTransform="translate(100 0) scale(1.2 1.5) translate(-100 0)"><stop offset="0" stop-color="{primary}" stop-opacity="0.22"/><stop offset="1" stop-color="{primary}" stop-opacity="0"/></radialGradient></defs><rect width="200" height="200" fill="url(#g)"/></svg>"##
        );
        // The halo: an ellipse the mark's box and 40 px by 20 px past it, in the primary at 28%, blurred 56 px; drawn a
        // quarter size with three deviations of room around it.
        let halo = format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="174" height="154" viewBox="0 0 174 154"><defs><filter id="b" x="-1" y="-1" width="3" height="3"><feGaussianBlur stdDeviation="14"/></filter></defs><ellipse cx="87" cy="77" rx="45" ry="35" fill="{primary}" fill-opacity="0.28" filter="url(#b)"/></svg>"##
        );
        Art { tilde: svg(tilde), glow: svg(glow), halo: svg(halo) }
    }
}

/// The window follows the computer's side, in that side's default theme.
fn follow_side(window: &mut Window, cx: &mut App) {
    let dark = matches!(window.appearance(), WindowAppearance::Dark | WindowAppearance::VibrantDark);
    let id = if dark { ui::theme::DARK } else { ui::theme::LIGHT };
    if ui::theme::current(cx).id != id {
        ui::theme::set_theme(id, cx);
        window.refresh();
    }
}

/// `rise`: in from 12 px below, fading in, after `delay`, in the screen's one curve.
fn rise(id: &'static str, child: impl IntoElement, delay: f32, duration: f32, cx: &App) -> AnyElement {
    let wrap = div().relative().w_full().flex().flex_col().items_center().child(child);
    if still(cx) {
        return wrap.into_any_element();
    }
    let curve = Easing::cubic_bezier(0.16, 1., 0.3, 1.).expect("a valid curve");
    let total = delay + duration;
    wrap.with_animation(id, Animation::new(Duration::from_secs_f32(total)), move |el, t| {
        let t = curve.sample(((t * total - delay) / duration).clamp(0., 1.));
        el.top(px(12. * (1. - t))).opacity(t)
    })
    .into_any_element()
}

impl Onboarding {
    fn row(&self, at: usize, agent: &Agent, cx: &mut Context<Self>) -> AnyElement {
        let p = cx.palette().clone();
        let checked = self.ticked.contains(&agent.id);
        let disabled = !agent.found;
        let mark_ink = if checked { p.foreground } else { p.muted_foreground };
        let mark = match catalog::mono(&agent.id, mark_ink) {
            Some(image) => img(image).size(px(16.)).into_any_element(),
            None => div()
                .text_size(px(12.))
                .line_height(px(12.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(mark_ink)
                .child(agent.name.chars().take(1).collect::<String>())
                .into_any_element(),
        };
        let id = agent.id.clone();
        h_flex()
            .id(SharedString::from(format!("agent-{}", agent.id)))
            .debug_selector(|| "agent-row".into())
            .flex_none()
            .h(px(48.))
            .gap(px(12.))
            .pl(px(16.))
            .pr(px(10.))
            .when(at > 0, |el| el.border_t_1().border_color(p.border))
            .when(!disabled, |el| {
                el.cursor_pointer().hover(|s| s.bg(p.accent.opacity(0.4))).on_click(cx.listener(move |this, _, _, cx| this.toggle(&id, cx)))
            })
            .child(checkbox_plain(checked, disabled, cx))
            .child(div().flex().flex_none().items_center().justify_center().size(px(18.)).child(mark))
            .child(
                div()
                    .min_w_0()
                    .max_w(relative(0.6))
                    .truncate()
                    .text_size(px(15.))
                    .line_height(px(22.5))
                    .text_color(if disabled { p.muted_foreground } else { p.foreground })
                    .child(agent.name.clone()),
            )
            .child(
                h_flex().ml_auto().min_w(px(96.)).justify_end().child(
                    div()
                        .debug_selector(|| "agent-state".into())
                        .truncate()
                        .font_family(cx.theme().mono_font_family.clone())
                        .text_size(px(12.))
                        .line_height(px(18.))
                        .text_color(p.muted_foreground)
                        .child(state_word(agent)),
                ),
            )
            .into_any_element()
    }

    fn keycap(&self, id: &'static str, word: &'static str, held: bool, cx: &mut Context<Self>) -> impl IntoElement {
        let me = cx.weak_entity();
        Button::primary(id, format!("{word}  \u{2192}")).keycap().held(held).on_click(move |_, window, cx| {
            _ = me.update(cx, |this, cx| this.press(window, cx));
        })
    }

    fn welcome(&self, art: &Art, cx: &mut Context<Self>) -> AnyElement {
        let halo = img(art.halo.clone()).absolute().left(px(-188.)).top(px(-208.)).w(px(696.)).h(px(616.));
        let halo = if still(cx) {
            halo.into_any_element()
        } else {
            let curve = Easing::cubic_bezier(0.16, 1., 0.3, 1.).expect("a valid curve");
            halo.with_animation("halo", Animation::new(Duration::from_millis(1400)), move |el, t| {
                el.opacity(curve.sample(((t * 1.4 - 0.2) / 1.2).clamp(0., 1.)))
            })
            .into_any_element()
        };
        let hero = div()
            .relative()
            .debug_selector(|| "tilde".into())
            .w(px(320.))
            .h(px(200.))
            .child(halo)
            .child(img(art.tilde.clone()).absolute().inset_0().size_full());
        column()
            .child(margin(px(48.)))
            .child(div().flex_none().child(hero))
            .child(rise("welcome-foot", div().pt(px(56.)).child(self.keycap("start", GET_STARTED, false, cx)), 1., 0.5, cx))
            .child(margin(px(40.)))
            .into_any_element()
    }

    fn agents_screen(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = cx.palette().clone();
        let head = v_flex()
            .w_full()
            .items_center()
            .mb(px(64.))
            .child(
                div()
                    .debug_selector(|| "heading".into())
                    .text_size(px(34.))
                    .line_height(px(39.1))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(p.foreground)
                    .child(HEADING),
            )
            .child(
                div()
                    .debug_selector(|| "sentence".into())
                    .mt(px(16.))
                    .max_w(px(440.))
                    .text_align(TextAlign::Center)
                    .text_size(px(15.))
                    .line_height(px(22.5))
                    .text_color(p.muted_foreground)
                    .child(sentence(self.here())),
            );
        let rows: Vec<AnyElement> = self.agents().iter().enumerate().map(|(at, a)| self.row(at, a, cx)).collect();
        let card = div()
            .id("rows")
            .debug_selector(|| "rows".into())
            .w_full()
            .flex_shrink(1.)
            .min_h(px(50.))
            .overflow_y_scroll()
            .rounded(px(10.))
            .border_1()
            .border_color(p.border)
            .bg(p.card)
            .when(rows.is_empty(), |el| el.invisible())
            .child(v_flex().children(rows));
        let hint = div()
            .debug_selector(|| "hint".into())
            .w_full()
            .flex_none()
            .mt(px(12.))
            .min_h(px(19.5))
            .text_align(TextAlign::Center)
            .text_size(px(13.))
            .line_height(px(19.5))
            .text_color(p.muted_foreground)
            .child(self.line());
        let refusal = self.refused.as_ref().map(|r| {
            div()
                .id("refusal-slot")
                .absolute()
                .top(px(18.))
                .w(px(560.))
                .max_w_full()
                .text_align(TextAlign::Center)
                .when_some(r.said.clone(), |el, said| el.tooltip(tip(said)))
                .child(Refusal::new().mono().said(r.happened.clone()).fix(r.fix.clone()))
        });
        let foot = div().relative().w_full().flex().flex_col().items_center().pt(px(56.)).children(refusal).child(self.keycap(
            "open",
            self.button_word(),
            self.held(),
            cx,
        ));
        column()
            .h_full()
            .child(margin(px(48.)))
            .child(rise("agents-head", head, 0., 0.34, cx))
            .child(card)
            .child(rise("agents-hint", hint, 0.06, 0.34, cx))
            .child(rise("agents-foot", foot, 0.1, 0.34, cx))
            .child(margin(px(40.)))
            .into_any_element()
    }
}

/// `.setup`: the screen's 560 px column, centred.
fn column() -> gpui_kit::Div {
    v_flex().mx_auto().min_h_full().w(px(560.)).max_w_full().items_center()
}

/// `.margin`: the two bands sharing what the screen leaves, so it stands centred and each keeps its least.
fn margin(least: gpui_kit::Pixels) -> impl IntoElement {
    div().w_full().flex_grow(1.).flex_shrink(1.).flex_basis(px(0.)).min_h(least)
}

impl Render for Onboarding {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = cx.palette().clone();
        let theme = ui::theme::current(cx).id;
        let dark = cx.theme().is_dark();
        if self.art.as_ref().is_none_or(|(id, _)| *id != theme) {
            self.art = Some((theme, Art::new(&p, dark)));
        }
        let Some((_, art)) = self.art.take() else { unreachable!() };
        let glow = img(art.glow.clone()).absolute().inset_0().size_full().object_fit(ObjectFit::Fill);
        let glow = match (self.step, still(cx)) {
            (Step::Welcome, _) => Some(glow.into_any_element()),
            (Step::Agents, true) => None,
            (Step::Agents, false) => Some(
                glow.with_animation("glow-out", Animation::new(Duration::from_millis(400)), |el, t| {
                    el.opacity(1. - Easing::cubic_bezier(0.16, 1., 0.3, 1.).map_or(t, |c| c.sample(t)))
                })
                .into_any_element(),
            ),
        };
        let screen = match self.step {
            Step::Welcome => self.welcome(&art, cx),
            Step::Agents => self.agents_screen(cx),
        };
        self.art = Some((theme, art));
        div()
            .id("onboarding")
            .debug_selector(|| "onboarding".into())
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, e: &KeyDownEvent, window, cx| {
                // A button the keyboard is on answers its own Enter.
                if e.keystroke.key == "enter" && this.focus.is_focused(window) {
                    this.press(window, cx);
                }
            }))
            .relative()
            .size_full()
            .bg(p.background)
            .text_color(p.foreground)
            .text_size(px(14.))
            .line_height(px(21.))
            .children(glow)
            .child(div().id("onboarding-page").absolute().inset_0().overflow_y_scroll().child(screen))
    }
}

/// Opens the onboarding window on the host, at the app's window size; Open wsp opens the app with `app` before this
/// window closes, so the count of windows never reaches zero.
pub fn open(host: Host, app: Rc<dyn Fn(&mut App)>, cx: &mut App) {
    let options = startup::window_options(cx);
    let opened = cx.open_window(options, |window, cx| {
        let finish: Finish = Rc::new(move |window, cx| {
            app(cx);
            window.remove_window();
        });
        let view = cx.new(|cx| Onboarding::new(Road { host, finish }, window, cx));
        cx.new(|cx| Root::new(view, window, cx))
    });
    if opened.is_ok() {
        cx.activate(true);
    }
}
