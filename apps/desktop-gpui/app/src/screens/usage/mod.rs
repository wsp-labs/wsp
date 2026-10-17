// SPDX-License-Identifier: AGPL-3.0-only
//! Settings > Usage, after `apps/web/src/settings/usage.tsx`: what the turns used over a range, and each account with
//! what its agent last said of its plan's windows. It reads through the host client alone, again each hold while it
//! stands open, again on coming back to the window once a hold has passed, and when asked.

pub mod chart;
mod limits;
pub mod model;
mod parts;
mod used;
pub mod words;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use gpui_kit::component::notification::Notification;
use gpui_kit::component::{h_flex, v_flex, ActiveTheme, Icon, Sizable as _, WindowExt as _};
use gpui_kit::{
    div, prelude::*, px, relative, Animation, AnimationExt as _, App, Context, Entity, FontWeight, SharedString, Subscription, Task,
    Transformation, Window,
};
use tokio::runtime::Runtime;
use wsp_host_client::protocol::{self, AccountsAnswer, AccountsAnswerAccountsItem, UsedAnswer};
use wsp_host_client::{Env, Failure, HostClient};

use crate::ui::{ActivePalette, Glyph, Segmented};
use model::{Range, Split};

/// How long a read stands: a second visit, or coming back to the window, inside it draws the one answer.
pub const HOLD: Duration = Duration::from_secs(60);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tab {
    Used,
    Limits,
}

/// One standing read: its last answer, or the refusal where none came, and when it was answered.
pub struct Read<T> {
    pub value: Option<T>,
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
    fn answer(&mut self, asked: u64, answer: Result<T, Failure>) -> bool {
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

    /// Forgets the answer and lets no answer already asked for land.
    fn forget(&mut self) {
        self.asked += 1;
        self.reading = false;
        self.value = None;
        self.refused = None;
    }
}

pub fn now_ms() -> f64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0., |d| d.as_millis() as f64)
}

/// The page for a screen that mounts it, Settings' list among them.
pub fn page(host: Arc<HostClient>, runtime: Arc<Runtime>, window: &mut Window, cx: &mut App) -> Entity<UsagePage> {
    cx.new(|cx| UsagePage::new(host, runtime, window, cx))
}

pub struct UsagePage {
    host: Arc<HostClient>,
    runtime: Arc<Runtime>,
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
    /// The account a reset is being spent on, whose button waits meanwhile.
    pub resetting: Option<String>,
    hold: Duration,
    _every: Task<()>,
    _back: Option<Subscription>,
}

impl UsagePage {
    pub fn new(host: Arc<HostClient>, runtime: Arc<Runtime>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self::with_hold(host, runtime, HOLD, window, cx)
    }

    /// A page that reads again every `hold`, which tests shorten.
    pub fn with_hold(host: Arc<HostClient>, runtime: Arc<Runtime>, hold: Duration, window: &mut Window, cx: &mut Context<Self>) -> Self {
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
            runtime,
            tab: Tab::Used,
            range: Range::Week,
            split: Split::Agent,
            accounts: Read::default(),
            used: Read::default(),
            models: Read::default(),
            projects: Vec::new(),
            said: None,
            resetting: None,
            hold,
            _every: every,
            _back: Some(back),
        };
        page.read_projects(cx);
        page.read_again(cx);
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
        self.read_accounts(cx);
        self.read_used(cx);
    }

    pub fn pick_tab(&mut self, tab: Tab, cx: &mut Context<Self>) {
        self.tab = tab;
        cx.notify();
    }

    pub fn pick_range(&mut self, range: Range, cx: &mut Context<Self>) {
        if self.range != range {
            self.range = range;
            self.read_used(cx);
        }
    }

    pub fn pick_split(&mut self, split: Split, cx: &mut Context<Self>) {
        if self.split != split {
            self.split = split;
            self.read_used(cx);
        }
    }

    fn read_projects(&mut self, cx: &mut Context<Self>) {
        let host = self.host.clone();
        let asking = self.runtime.spawn(async move { host.projects().await });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(projects)) = asking.await {
                _ = this.update(cx, |this, cx| {
                    this.projects = projects.into_iter().map(|p| p.id).collect();
                    cx.notify();
                });
            }
        })
        .detach();
    }

    fn read_accounts(&mut self, cx: &mut Context<Self>) {
        let asked = self.accounts.ask();
        let host = self.host.clone();
        // Each read asks the agents that can say for their limits now: a turn is otherwise the only thing that reads them.
        let asking = self.runtime.spawn(async move { host.call(protocol::UsageAccountsRequest { fresh: Some(true) }).await });
        cx.spawn(async move |this, cx| {
            let answer = asking.await.unwrap_or_else(|e| Err(Failure::new(e.to_string())));
            _ = this.update(cx, |this, cx| {
                if this.accounts.answer(asked, answer) {
                    cx.notify();
                }
            });
        })
        .detach();
        cx.notify();
    }

    fn read_used(&mut self, cx: &mut Context<Self>) {
        let (range, split) = (self.range, self.split);
        let asked = self.used.ask();
        let host = self.host.clone();
        let asking = self.runtime.spawn(async move { host.call(split.request(range)).await.map(|u| u.used) });
        cx.spawn(async move |this, cx| {
            let answer = asking.await.unwrap_or_else(|e| Err(Failure::new(e.to_string())));
            _ = this.update(cx, |this, cx| {
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
            let host = self.host.clone();
            let asking = self.runtime.spawn(async move { host.call(Split::Model.request(range)).await.map(|u| u.used) });
            cx.spawn(async move |this, cx| {
                let answer = asking.await.unwrap_or_else(|e| Err(Failure::new(e.to_string())));
                _ = this.update(cx, |this, cx| {
                    if this.models.answer(asked, answer) {
                        cx.notify();
                    }
                });
            })
            .detach();
        } else {
            self.models.forget();
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
        self.resetting = Some(row.key.clone());
        cx.notify();
        let host = self.host.clone();
        let key = row.key.clone();
        let asking =
            self.runtime.spawn(async move { host.call(protocol::UsageResetRequest { account: key, credit_id: None, on: None }).await });
        let window = window.window_handle();
        cx.spawn(async move |this, cx| {
            let answer = asking.await.unwrap_or_else(|e| Err(Failure::new(e.to_string())));
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
                this.resetting = None;
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
            for account in &mut accounts.accounts {
                if account.key == row.key {
                    *account = row.clone();
                }
            }
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
                    .when(reading, |el| el.opacity(0.64))
                    .when(!reading, |el| {
                        let page = page.clone();
                        el.cursor_pointer()
                            .hover(|s| s.bg(p.accent))
                            .on_click(move |_, _, cx| page.update(cx, |page, cx| page.read_again(cx)))
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

/// `--screen usage`: the page alone in a window, under a top bar holding the crumbs the shell's header will hold and the
/// page's tabs, connected to the host serving `state`.
pub struct UsageWindow {
    page: Option<Entity<UsagePage>>,
    note: Option<SharedString>,
    _connect: Task<()>,
}

impl UsageWindow {
    pub fn new(state: PathBuf, env: Env, runtime: Arc<Runtime>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let connecting = runtime.spawn(async move { HostClient::connect(&state, &env).await });
        let handle = window.window_handle();
        let connect = cx.spawn(async move |this, cx| {
            let connected = connecting.await.unwrap_or_else(|e| Err(Failure::new(e.to_string())));
            _ = handle.update(cx, |_, window, cx| {
                _ = this.update(cx, |this, cx| {
                    match connected {
                        Ok(host) => {
                            let host = Arc::new(host);
                            this.page = Some(page(host, runtime.clone(), window, cx));
                        }
                        Err(failure) => {
                            eprintln!("desktop-gpui: {}", failure.message);
                            this.note = Some(failure.message.into());
                        }
                    }
                    cx.notify();
                });
            });
        });
        UsageWindow { page: None, note: Some(words::CONNECTING.into()), _connect: connect }
    }

    pub fn page(&self) -> Option<&Entity<UsagePage>> {
        self.page.as_ref()
    }
}

impl Render for UsageWindow {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = cx.palette().clone();
        let tabs = self.page.clone().map(|page| page.update(cx, |page, cx| page.tabs(cx).into_any_element()));
        frame(tabs, self.page.clone().map(|page| page.into_any_element()), self.note.clone(), cx).bg(p.background)
    }
}

/// The top bar and the page's column as Settings lays them: 52 px of bar, then a column at most 1080 px wide with 32 px
/// sides, 56 px over it and 48 under.
pub fn frame(
    tabs: Option<gpui_kit::AnyElement>,
    page: Option<gpui_kit::AnyElement>,
    note: Option<SharedString>,
    cx: &App,
) -> gpui_kit::Div {
    let p = cx.palette();
    let crumbs = h_flex()
        .min_w_0()
        .gap_2()
        .text_sm()
        .line_height(px(20.))
        .child(div().text_color(p.muted_foreground).child(words::SETTINGS))
        .child(div().text_color(p.muted_foreground.opacity(0.5)).child("/"))
        .child(div().font_weight(FontWeight::MEDIUM).text_color(p.foreground).child(words::TITLE));
    let bar = h_flex().flex_none().h(px(52.)).pl(px(20.)).pr(px(21.)).gap_4().child(crumbs).child(div().ml_auto().children(tabs));
    let column = v_flex()
        .w_full()
        .max_w(px(1080.))
        .mx_auto()
        .px(px(32.))
        .pt(px(56.))
        .pb(px(48.))
        .children(page)
        .children(note.map(|note| div().text_sm().line_height(px(20.)).text_color(p.muted_foreground).child(note)));
    v_flex()
        .size_full()
        .font_family(cx.theme().font_family.clone())
        .text_color(p.foreground)
        .child(bar)
        .child(div().id("usage-scroll").flex_1().min_h_0().overflow_y_scroll().child(column))
}
