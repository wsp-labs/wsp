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

use gpui_kit::component::notification::Notification;
use gpui_kit::component::{h_flex, v_flex, ActiveTheme, Icon, Sizable as _, WindowExt as _};
use gpui_kit::{
    div, point, prelude::*, px, relative, Animation, AnimationExt as _, App, BoxShadow, Context, Entity, FocusHandle, FontWeight,
    KeyDownEvent, Subscription, Task, Transformation, Window, WindowId,
};
use wsp_host_client::protocol::{self, AccountsAnswer, AccountsAnswerAccountsItem, UsedAnswer};
use wsp_host_client::Failure;

use crate::screens::{Host, Opened, Parts, Place, Screen};
use crate::ui::{ActivePalette, Glyph, Segmented};
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

fn used_key(range: Range, split: Split) -> String {
    format!("usage:used:{range:?}:{split:?}")
}

pub fn now_ms() -> f64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0., |d| d.as_millis() as f64)
}

/// The page for a screen that mounts it, Settings' list among them.
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
        let mut page = UsagePage {
            host,
            window: window.window_handle().window_id(),
            tab: Tab::Used,
            range: Range::Week,
            split: Split::Agent,
            accounts: Read::default(),
            used: Read::default(),
            models: Read::default(),
            projects: Vec::new(),
            said: None,
            resetting: Vec::new(),
            refresh: cx.focus_handle().tab_stop(true),
            hold,
            _every: every,
            _back: Some(back),
        };
        page.read_projects(cx);
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

    /// Spends one banked reset of `row` after the question the command line asks, says what the host answered and
    /// puts the row it sends back in place.
    fn ask_reset(&mut self, row: AccountsAnswerAccountsItem, window: &mut Window, cx: &mut Context<Self>) {
        let Some(question) = model::reset_line(&row, now_ms()).map(|line| line.question) else { return };
        let page = cx.entity();
        window.open_dialog(cx, move |dialog, _, _| {
            let (page, row) = (page.clone(), row.clone());
            dialog
                .title(words::USE_RESET)
                .child(question.clone())
                .button_props(
                    gpui_kit::component::dialog::DialogButtonProps::default()
                        .ok_text(words::USE_RESET)
                        .cancel_text(words::CANCEL)
                        .show_cancel(true),
                )
                .on_ok(move |_, window, cx| {
                    page.update(cx, |page, cx| page.use_reset(row.clone(), window, cx));
                    true
                })
        });
    }

    pub fn use_reset(&mut self, row: AccountsAnswerAccountsItem, window: &mut Window, cx: &mut Context<Self>) {
        if self.resetting.contains(&row.key) {
            return;
        }
        self.resetting.push(row.key.clone());
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
                this.said = Some(said.clone());
                cx.notify();
            });
            let notice = if spent { Notification::info(said) } else { Notification::error(said) };
            _ = window.update(cx, |_, window, cx| window.push_notification(notice, cx));
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
        let reading = self.reading();
        let checked = self.read_at().map(|at| words::checked(&words::read_when((now_ms() - at).max(0.))));
        let picked = match self.tab {
            Tab::Used => 0,
            Tab::Limits => 1,
        };
        let page = cx.entity();
        let ring =
            vec![BoxShadow { color: p.ring, offset: point(px(0.), px(0.)), blur_radius: px(0.), spread_radius: px(2.), inset: false }];
        let spin = Icon::new(Glyph::RotateCw).with_size(px(14.)).text_color(p.muted_foreground);
        h_flex()
            .gap_3()
            .children(checked.map(|c| {
                div().debug_selector(|| "usage-read-at".into()).text_xs().line_height(px(16.)).text_color(p.muted_foreground).child(c)
            }))
            .child(
                div()
                    .id("usage-refresh")
                    .debug_selector(|| "usage-refresh".into())
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .size(px(24.))
                    .rounded(px(8.))
                    .track_focus(&self.refresh)
                    .focus_visible(|s| s.shadow(ring))
                    .when(reading, |el| el.opacity(0.64))
                    .when(!reading, |el| {
                        let (page, keyed) = (page.clone(), page.clone());
                        el.cursor_pointer()
                            .hover(|s| s.bg(p.accent))
                            .on_click(move |_, _, cx| page.update(cx, |page, cx| page.read_again(cx)))
                            .on_key_down(move |event: &KeyDownEvent, _, cx| {
                                if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                                    cx.stop_propagation();
                                    keyed.update(cx, |page, cx| page.read_again(cx));
                                }
                            })
                    })
                    .tooltip(|window, cx| gpui_kit::component::tooltip::Tooltip::new(words::READ_AGAIN).build(window, cx))
                    .map(|el| {
                        if reading {
                            el.child(spin.with_animation(
                                "usage-refresh-spin",
                                Animation::new(Duration::from_secs(1)).repeat(),
                                |icon, delta| icon.transform(Transformation::rotate(gpui_kit::percentage(delta))),
                            ))
                        } else {
                            el.child(spin)
                        }
                    }),
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

/// The page as the shell mounts it in the main column, with "Settings / Usage" as its crumbs and the tabs as its
/// actions. Settings mounts `UsagePage` in its own frame under `settings:usage` once it lands.
pub struct UsageScreen {
    page: Entity<UsagePage>,
}

impl Screen for UsageScreen {
    const NAME: &'static str = "usage";
    const PLACE: Place = Place::Main;

    fn new(host: Host, _: Opened, window: &mut Window, cx: &mut Context<Self>) -> Self {
        UsageScreen { page: page(host, window, cx) }
    }

    fn parts(this: &Entity<Self>, _: &mut Window, cx: &mut App) -> Parts {
        let page = this.read(cx).page.clone();
        Parts {
            crumbs: Some(cx.new(|_| UsageCrumbs).into()),
            actions: Some(cx.new(|cx| UsageActions::new(page, cx)).into()),
            ..Default::default()
        }
    }
}

impl UsageScreen {
    pub fn page(&self) -> &Entity<UsagePage> {
        &self.page
    }
}

impl Render for UsageScreen {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        column(Some(self.page.clone().into_any_element()), cx)
    }
}

struct UsageCrumbs;

impl Render for UsageCrumbs {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        crumbs(cx)
    }
}

/// The page's tabs in the header, drawn again whenever the page changes.
struct UsageActions {
    page: Entity<UsagePage>,
    _page: Subscription,
}

impl UsageActions {
    fn new(page: Entity<UsagePage>, cx: &mut Context<Self>) -> Self {
        let observed = cx.observe(&page, |_, _, cx| cx.notify());
        UsageActions { page, _page: observed }
    }
}

impl Render for UsageActions {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.page.update(cx, |page, cx| page.tabs(cx).into_any_element())
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
fn column(page: Option<gpui_kit::AnyElement>, cx: &App) -> gpui_kit::Div {
    let p = cx.palette();
    let column = v_flex().w_full().max_w(px(1080.)).mx_auto().px(px(32.)).pt(px(56.)).pb(px(48.)).children(page);
    v_flex()
        .size_full()
        .font_family(cx.theme().font_family.clone())
        .text_color(p.foreground)
        .child(div().id("usage-scroll").flex_1().min_h_0().overflow_y_scroll().child(column))
}

/// The shell's 52 px header row with the crumbs and the tabs over the page's column, for captures of the page alone.
pub fn frame(tabs: Option<gpui_kit::AnyElement>, page: Option<gpui_kit::AnyElement>, cx: &App) -> gpui_kit::Div {
    let bar = h_flex().flex_none().h(px(52.)).pl(px(20.)).pr(px(21.)).gap_4().child(crumbs(cx)).child(div().ml_auto().children(tabs));
    v_flex().size_full().child(bar).child(column(page, cx))
}
