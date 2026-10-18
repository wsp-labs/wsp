// SPDX-License-Identifier: AGPL-3.0-only
//! Settings > Usage, after `apps/web/src/settings/usage.tsx`: what the turns used over a range, and each account with
//! what its agent last said of its plan's windows. It reads through the host client alone, again each hold while it
//! stands open, again on coming back to the window once a hold has passed, and when asked.

mod held;
mod limits;
pub mod model;
mod parts;
mod used;
pub mod words;

use std::rc::Rc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use gpui_kit::component::{h_flex, v_flex, ActiveTheme};
use gpui_kit::{
    div, prelude::*, px, relative, App, Context, Entity, FocusHandle, FontWeight, ScrollHandle, Subscription, Task, Window, WindowId,
};
use serde_json::json;
use wsp_host_client::ops::settings::{Places, PreferencesReply};
use wsp_host_client::protocol::{
    self, AccountsAnswer, AccountsAnswerAccountsItem, PlaceView, PlacesListRequest, Preferences, PreferencesGetRequest, UsedAnswer,
};
use wsp_host_client::Failure;

use crate::screens::dialogs::layer;
use crate::screens::settings::page::DialogBuild;
use crate::screens::settings::{Acts, DIALOG};
use crate::screens::Host;
use crate::ui::{scroll_area, ActivePalette, AlertDialog, Button, Glyph, Notice, Segmented};
use model::{Range, Split};

/// How often the page reads again while it stands open, and the age past which coming back to the window reads again.
pub const HOLD: Duration = Duration::from_secs(60);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tab {
    Used,
    Limits,
}

/// One standing read: its last answer, or the refusal where none came, and when it was answered.
pub struct Read<T> {
    pub value: Option<Rc<T>>,
    pub refused: Option<String>,
    pub at: Option<f64>,
    pub reading: bool,
    asked: u64,
}

impl<T> Default for Read<T> {
    fn default() -> Self {
        Read { value: None, refused: None, at: None, reading: false, asked: 0 }
    }
}

impl<T> Read<T> {
    fn ask(&mut self) -> u64 {
        self.asked += 1;
        self.reading = true;
        self.asked
    }

    /// Takes an answer unless a later ask is already out; a refusal keeps the last answer and its age standing.
    fn answer(&mut self, asked: u64, answer: Result<Rc<T>, Failure>) -> bool {
        if asked != self.asked {
            return false;
        }
        self.reading = false;
        match answer {
            Ok(value) => {
                self.value = Some(value);
                self.refused = None;
                self.at = Some(now_ms());
            }
            Err(failure) => self.refused = Some(failure.message),
        }
        true
    }

    /// Takes what the window kept, with its age.
    fn kept(&mut self, value: Rc<T>, at: f64) {
        self.value = Some(value);
        self.refused = None;
        self.at = Some(at);
    }

    /// Forgets the answer and lets no answer already asked for land.
    fn forget(&mut self) {
        self.asked += 1;
        self.reading = false;
        self.value = None;
        self.refused = None;
    }
}

const ACCOUNTS: &str = "usage:accounts";
/// The tab picked last in the window, which every visit opens on while the window stands.
const TAB: &str = "usage:tab";

fn used_key(range: Range, split: Split) -> String {
    format!("usage:used:{range:?}:{split:?}")
}

pub fn now_ms() -> f64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0., |d| d.as_millis() as f64)
}

/// The page, as Settings mounts it under `settings:usage`.
pub fn page(host: Host, window: &mut Window, cx: &mut App) -> Entity<UsagePage> {
    cx.new(|cx| UsagePage::new(host, window, cx))
}

pub struct UsagePage {
    host: Host,
    /// The window whose kept answers the page draws.
    window: WindowId,
    pub tab: Tab,
    pub range: Range,
    pub split: Split,
    pub accounts: Read<AccountsAnswer>,
    pub used: Read<UsedAnswer>,
    /// The model split of the same range, which nests each agent's models under it on the split by agent.
    pub models: Read<UsedAnswer>,
    /// The ids of the host's projects, so use filed under a project it no longer lists reads as no project's.
    pub projects: Vec<String>,
    /// The host's computers and the person's preferences, for the glyph each computer and project wears.
    pub places: Vec<PlaceView>,
    pub preferences: Option<Preferences>,
    /// Settings, which draws the reset's dialog over itself; a page standing alone asks no reset.
    pub acts: Option<Acts>,
    /// The account whose reset dialog stands open.
    asking: Option<String>,
    /// What the host last said of a reset spent here, the words of the notice it raised.
    pub said: Option<String>,
    /// The accounts a reset is being spent on, whose buttons wait meanwhile.
    pub resetting: Vec<String>,
    /// The refresh button's place in the tab order.
    pub refresh: FocusHandle,
    hold: Duration,
    _every: Task<()>,
    _back: Option<Subscription>,
}

impl UsagePage {
    pub fn new(host: Host, window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self::with_hold(host, HOLD, window, cx)
    }

    /// A page that reads again every `hold`, which tests shorten.
    pub fn with_hold(host: Host, hold: Duration, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let every = cx.spawn(async move |this, cx| loop {
            cx.background_executor().timer(hold).await;
            if this.update(cx, |this, cx| this.read_again(cx)).is_err() {
                break;
            }
        });
        let back = cx.observe_window_activation(window, |this, window, cx| {
            let stale = this.read_at().is_none_or(|at| now_ms() - at > this.hold.as_millis() as f64);
            if window.is_window_active() && stale {
                this.read_again(cx);
            }
        });
        let window_id = window.window_handle().window_id();
        let tab = held::get::<Tab>(window_id, TAB, Duration::MAX, cx).map_or(Tab::Used, |(tab, _, _)| *tab);
        let mut page = UsagePage {
            host,
            window: window_id,
            tab,
            range: Range::Week,
            split: Split::Agent,
            accounts: Read::default(),
            used: Read::default(),
            models: Read::default(),
            projects: Vec::new(),
            places: Vec::new(),
            preferences: None,
            acts: None,
            asking: None,
            said: None,
            resetting: Vec::new(),
            refresh: cx.focus_handle().tab_stop(true),
            hold,
            _every: every,
            _back: Some(back),
        };
        page.read_projects(cx);
        page.read_looks(cx);
        page.read_accounts(false, cx);
        page.read_used(false, cx);
        page
    }

    /// When both reads last answered, the older of the two.
    pub fn read_at(&self) -> Option<f64> {
        Some(self.accounts.at?.min(self.used.at?))
    }

    pub fn reading(&self) -> bool {
        self.accounts.reading || self.used.reading
    }

    /// Reads both tabs again at once, as the refresh asks.
    pub fn read_again(&mut self, cx: &mut Context<Self>) {
        self.read_accounts(true, cx);
        self.read_used(true, cx);
    }

    pub fn pick_tab(&mut self, tab: Tab, cx: &mut Context<Self>) {
        self.tab = tab;
        held::put(self.window, TAB.to_owned(), Rc::new(tab), now_ms(), cx);
        cx.notify();
    }

    pub fn pick_range(&mut self, range: Range, cx: &mut Context<Self>) {
        if self.range != range {
            self.range = range;
            self.read_used(false, cx);
        }
    }

    pub fn pick_split(&mut self, split: Split, cx: &mut Context<Self>) {
        if self.split != split {
            self.split = split;
            self.read_used(false, cx);
        }
    }

    fn read_projects(&mut self, cx: &mut Context<Self>) {
        let asking = self.host.run(|host| async move { host.projects().await });
        cx.spawn(async move |this, cx| {
            if let Ok(projects) = asking.await {
                _ = this.update(cx, |this, cx| {
                    this.projects = projects.into_iter().map(|p| p.id).collect();
                    cx.notify();
                });
            }
        })
        .detach();
    }

    /// The computers and the preferences, read once a visit as the projects are.
    fn read_looks(&mut self, cx: &mut Context<Self>) {
        let asking = self.host.run(|host| async move {
            let places = serde_json::from_value::<PlacesListRequest>(json!({})).map_err(|e| Failure::new(e.to_string()))?;
            let preferences = serde_json::from_value::<PreferencesGetRequest>(json!({})).map_err(|e| Failure::new(e.to_string()))?;
            let places: Places = host.call(places).await?;
            let preferences: PreferencesReply = host.call(preferences).await?;
            Ok::<_, Failure>((places.places, preferences.preferences))
        });
        cx.spawn(async move |this, cx| {
            if let Ok((places, preferences)) = asking.await {
                _ = this.update(cx, |this, cx| {
                    this.places = places;
                    this.preferences = Some(preferences);
                    cx.notify();
                });
            }
        })
        .detach();
    }

    /// The accounts the window kept while they are inside the hold, else a read; `force` reads regardless.
    fn read_accounts(&mut self, force: bool, cx: &mut Context<Self>) {
        let (window, hold) = (self.window, self.hold);
        if let Some((value, at, fresh)) = held::get::<AccountsAnswer>(window, ACCOUNTS, hold, cx) {
            self.accounts.kept(value, at);
            if fresh && !force {
                return cx.notify();
            }
        }
        let asked = self.accounts.ask();
        // Each read asks the agents that can say for their limits now: a turn is otherwise the only thing that reads them.
        let asking = self.host.run(|host| async move { host.call(protocol::UsageAccountsRequest { fresh: Some(true) }).await });
        cx.spawn(async move |this, cx| {
            let answer = asking.await.map(Rc::new);
            _ = this.update(cx, |this, cx| {
                if let Ok(value) = &answer {
                    held::put(window, ACCOUNTS.to_owned(), value.clone(), now_ms(), cx);
                }
                if this.accounts.answer(asked, answer) {
                    cx.notify();
                }
            });
        })
        .detach();
        cx.notify();
    }

    /// The range's split as the window kept it while inside the hold, else a read; `force` reads regardless. Until an
    /// answer lands the last range's figures stand.
    fn read_used(&mut self, force: bool, cx: &mut Context<Self>) {
        let (range, split, window, hold) = (self.range, self.split, self.window, self.hold);
        let fresh = |key: &str, read: &mut Read<UsedAnswer>, cx: &mut Context<Self>| match held::get::<UsedAnswer>(window, key, hold, cx) {
            Some((value, at, fresh)) => {
                read.kept(value, at);
                fresh && !force
            }
            None => false,
        };
        let used_fresh = fresh(&used_key(range, split), &mut self.used, cx);
        let models_fresh = split != Split::Agent || fresh(&used_key(range, Split::Model), &mut self.models, cx);
        if split != Split::Agent {
            self.models.forget();
        }
        if used_fresh && models_fresh {
            return cx.notify();
        }
        let asked = self.used.ask();
        let asking = self.host.run(move |host| async move { host.call(split.request(range)).await.map(|u| u.used) });
        cx.spawn(async move |this, cx| {
            let answer = asking.await.map(Rc::new);
            _ = this.update(cx, |this, cx| {
                if let Ok(value) = &answer {
                    held::put(window, used_key(range, split), value.clone(), now_ms(), cx);
                }
                // A refusal stands in for another range's figures rather than under them.
                let other = this.used.value.as_ref().is_some_and(|u| Range::from(u.range) != range || Split::from(u.split) != split);
                if answer.is_err() && other && asked == this.used.asked {
                    this.used.value = None;
                }
                if this.used.answer(asked, answer) {
                    cx.notify();
                }
            });
        })
        .detach();
        if split == Split::Agent {
            let asked = self.models.ask();
            let asking = self.host.run(move |host| async move { host.call(Split::Model.request(range)).await.map(|u| u.used) });
            cx.spawn(async move |this, cx| {
                let answer = asking.await.map(Rc::new);
                _ = this.update(cx, |this, cx| {
                    if let Ok(value) = &answer {
                        held::put(window, used_key(range, Split::Model), value.clone(), now_ms(), cx);
                    }
                    if this.models.answer(asked, answer) {
                        cx.notify();
                    }
                });
            })
            .detach();
        }
        cx.notify();
    }

    /// Asks the one question the command line asks before spending one banked reset of `row`, in Settings' alert
    /// dialog, which stays open and holds Escape until the host has answered.
    fn ask_reset(&mut self, row: AccountsAnswerAccountsItem, cx: &mut Context<Self>) {
        let Some(acts) = self.acts.clone() else { return };
        let Some(question) = model::reset_line(&row, now_ms()).map(|line| line.question) else { return };
        self.asking = Some(row.key.clone());
        let page = cx.weak_entity();
        let spending = {
            let (page, key) = (page.clone(), row.key.clone());
            move |cx: &App| page.upgrade().is_some_and(|page| page.read(cx).resetting.contains(&key))
        };
        let dialog = DialogBuild::alert(move |ctx, _, cx| {
            let busy = spending(cx);
            let cancel = {
                let acts = ctx.acts.clone();
                Button::outline("reset-cancel", words::CANCEL).neutral_ring().on_click(move |_, _, cx| {
                    if !busy {
                        acts.close_dialog(cx);
                    }
                })
            };
            let (page, row) = (page.clone(), row.clone());
            let confirm = Button::primary("reset-confirm", if busy { words::USING_RESET } else { words::USE_RESET }).held(busy).on_click(
                move |_, window, cx| {
                    _ = page.update(cx, |page, cx| page.use_reset(row.clone(), window, cx));
                },
            );
            AlertDialog::new(words::USE_RESET).layered().sentence(question.clone()).act(cancel).act(confirm).into_any_element()
        });
        acts.open_dialog(dialog, cx);
    }

    pub fn use_reset(&mut self, row: AccountsAnswerAccountsItem, window: &mut Window, cx: &mut Context<Self>) {
        if self.resetting.contains(&row.key) {
            return;
        }
        self.resetting.push(row.key.clone());
        if self.asking.as_ref() == Some(&row.key) {
            layer::hold(DIALOG, true, cx);
        }
        cx.notify();
        let key = row.key.clone();
        let spending = row.key.clone();
        let asking =
            self.host.run(|host| async move { host.call(protocol::UsageResetRequest { account: key, credit_id: None, on: None }).await });
        let window = window.window_handle();
        cx.spawn(async move |this, cx| {
            let answer = asking.await;
            let (said, spent) = match &answer {
                Ok(answer) => (answer.said.clone(), answer.outcome == protocol::ResetAnswerOutcome::Reset),
                Err(failure) => (failure.message.clone(), false),
            };
            _ = this.update(cx, |this, cx| {
                if let Ok(answer) = answer {
                    if let Some(account) = answer.account.and_then(|a| model::account_row(&a)) {
                        this.put_account(account, cx);
                    }
                }
                this.resetting.retain(|key| *key != spending);
                if this.asking.as_ref() == Some(&spending) {
                    this.asking = None;
                    layer::hold(DIALOG, false, cx);
                    if let Some(acts) = &this.acts {
                        acts.close_dialog(cx);
                    }
                }
                this.said = Some(said.clone());
                cx.notify();
            });
            let notice = if spent { Notice::note(said) } else { Notice::error(said) };
            _ = window.update(cx, |_, window, cx| crate::ui::notify_in(window, cx, notice));
        })
        .detach();
    }

    /// An account's new row in place of its old one.
    pub fn put_account(&mut self, row: AccountsAnswerAccountsItem, cx: &mut Context<Self>) {
        if let Some(accounts) = &mut self.accounts.value {
            for account in &mut Rc::make_mut(accounts).accounts {
                if account.key == row.key {
                    *account = row.clone();
                }
            }
            held::keep(self.window, ACCOUNTS, accounts.clone(), cx);
        }
        cx.notify();
    }

    /// The page's two tabs for the top bar's right end, after the read's age and the refresh.
    pub fn tabs(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = cx.palette().clone();
        let checked = self.read_at().map(|at| words::checked(&words::read_when((now_ms() - at).max(0.))));
        let picked = match self.tab {
            Tab::Used => 0,
            Tab::Limits => 1,
        };
        let (page, again) = (cx.entity(), cx.entity());
        h_flex()
            .gap_3()
            .children(checked.map(|c| {
                div().debug_selector(|| "usage-read-at".into()).text_xs().line_height(px(16.)).text_color(p.muted_foreground).child(c)
            }))
            .child(
                div().debug_selector(|| "usage-refresh".into()).flex().child(
                    Button::icon_xs("usage-refresh", Glyph::RotateCw)
                        .busy(self.reading())
                        .track_focus(&self.refresh)
                        .tooltip(words::READ_AGAIN)
                        .on_click(move |_, _, cx| again.update(cx, |page, cx| page.read_again(cx))),
                ),
            )
            .child(
                Segmented::new("usage-tabs", picked)
                    .height(px(36.))
                    .segment_size((px(14.), px(20.)), px(14.))
                    .glyph_segment(Glyph::ChartLine, words::TAB_USED)
                    .glyph_segment(Glyph::Gauge, words::TAB_LIMITS)
                    .on_pick(move |ix, _, cx| page.update(cx, |page, cx| page.pick_tab(if ix == 0 { Tab::Used } else { Tab::Limits }, cx))),
            )
    }
}

impl Render for UsagePage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let now = now_ms();
        let tab = match self.tab {
            Tab::Used => used::render(self, now, window, cx),
            Tab::Limits => limits::render(self, now, window, cx),
        };
        // Text with no leading of its own takes the page's 1.5, as preflight gives the web's body.
        div().line_height(relative(1.5)).child(tab)
    }
}

/// `SettingsCrumbs` on a group page: "Settings", the slash, and the page.
fn crumbs(cx: &App) -> gpui_kit::Div {
    let p = cx.palette();
    h_flex()
        .min_w_0()
        .gap_2()
        .text_sm()
        .line_height(px(20.))
        .child(div().text_color(p.muted_foreground).child(words::SETTINGS))
        .child(div().text_color(p.muted_foreground.opacity(0.5)).child("/"))
        .child(div().font_weight(FontWeight::MEDIUM).text_color(p.foreground).child(words::TITLE))
}

/// The page's column as Settings lays it: at most 1080 px wide with 32 px sides, 56 px over it and 48 under.
fn column(page: Option<gpui_kit::AnyElement>, window: &mut Window, cx: &mut App) -> gpui_kit::Div {
    let column = v_flex().w_full().max_w(px(1080.)).mx_auto().px(px(32.)).pt(px(56.)).pb(px(48.)).children(page);
    let scroll = window.use_keyed_state("usage-scroll-handle", cx, |_, _| ScrollHandle::new()).read(cx).clone();
    let scroll = scroll_area("usage-scroll", &scroll, column, window, cx).flex_1().min_h_0();
    v_flex().size_full().font_family(cx.theme().font_family.clone()).text_color(cx.palette().foreground).child(scroll)
}

/// The shell's 52 px header row with the crumbs and the tabs over the page's column, for captures of the page alone.
pub fn frame(tabs: Option<gpui_kit::AnyElement>, page: Option<gpui_kit::AnyElement>, window: &mut Window, cx: &mut App) -> gpui_kit::Div {
    let bar = h_flex().flex_none().h(px(52.)).pl(px(20.)).pr(px(21.)).gap_4().child(crumbs(cx)).child(div().ml_auto().children(tabs));
    v_flex().size_full().child(bar).child(column(page, window, cx))
}
