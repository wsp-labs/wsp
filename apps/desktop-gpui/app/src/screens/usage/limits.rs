// SPDX-License-Identifier: AGPL-3.0-only
//! The Limits tab: each agent's subscription accounts as a pool, one line per window of what is left with the verdict
//! under it and a segment per account; then the accounts no window speaks for. The accounts and the tokens are two
//! ledgers and are never summed.

use gpui_kit::component::{h_flex, v_flex, ActiveTheme};
use gpui_kit::{deferred, div, prelude::*, px, relative, AnyElement, App, Context, FontWeight, Hsla, SharedString, Window};

use super::model::{self, Account, AccountLine, Pool, PoolWindow, Tone};
use super::parts::{agent_glyph, identity, quiet, rule, sized, surface, tip_skin, BLOCK_GAP, CARD_PAD};
use super::{words, UsagePage};
use crate::ui::{motion, ActivePalette, Button, Card, CardItem, DigitRoll, Skeleton};

/// The window's figure column at its `14rem`.
const WINDOW_COLUMN: f32 = 224.;
const SEGMENT_RADIUS: f32 = 5.;
/// The segment's hover opens this far above it (`sideOffset={8}`).
const TIP_OFFSET: f32 = 8.;

pub fn render(page: &UsagePage, now: f64, window: &mut Window, cx: &mut Context<UsagePage>) -> AnyElement {
    let entity = cx.entity();
    let accounts = page.accounts.value.as_ref().map(|a| &a.accounts[..]);
    let el = match (accounts, &page.accounts.refused) {
        (None, Some(refused)) => Card::new("usage-limits")
            .locked(true)
            .item(CardItem::any(quiet(words::limits_refused(refused), cx).px(CARD_PAD).py_5()))
            .into_any_element(),
        (None, None) => skeleton(cx).into_any_element(),
        (Some([]), _) => {
            Card::new("usage-limits").locked(true).item(CardItem::any(quiet(words::NO_ACCOUNTS, cx).px(CARD_PAD).py_5())).into_any_element()
        }
        (Some(accounts), _) => {
            let limits = model::limits(accounts);
            let no_limit = limits.no_limit_word();
            let quiet_card = limits.has_quiet_card().then(|| {
                let rows = limits.quiet.iter().map(|row| CardItem::any(account_line(row, cx))).collect::<Vec<_>>();
                let card = Card::new("usage-limits").locked(true).head(words::NO_PLAN_LIMIT).items(rows);
                match no_limit {
                    Some(word) => card.under(quiet(word, cx).px(CARD_PAD).debug_selector(|| "no-limit".into())),
                    None => card,
                }
            });
            v_flex()
                .gap(BLOCK_GAP)
                .children(limits.pools.iter().map(|pool| pool_card(pool, now, &page.resetting, &entity, window, cx)))
                .children(quiet_card)
                .into_any_element()
        }
    };
    div().debug_selector(|| "usage-limits-tab".into()).child(el).into_any_element()
}

fn small(cx: &App) -> gpui_kit::Div {
    sized(div(), 12.5).text_color(cx.palette().muted_foreground)
}

fn pool_card(pool: &Pool, now: f64, resetting: &[String], page: &gpui_kit::Entity<UsagePage>, window: &mut Window, cx: &mut App) -> Card {
    let p = cx.palette().clone();
    let tz = chrono::Local;
    let mark = agent_glyph(pool.agent, px(16.), p.foreground, cx);
    let head =
        h_flex().flex_wrap().justify_between().gap_x_6().gap_y_2().child(identity(mark, pool.title(), Some(pool.line()), None, cx)).child(
            sized(h_flex().flex_wrap().gap_x_5(), 12.5).text_color(p.muted_foreground).children(pool.burn_word()).child(pool.checked(now)),
        );
    let resets: Vec<_> =
        pool.accounts.iter().filter_map(|account| reset_line(account, now, resetting.contains(&account.key), page, cx)).collect();
    let line = rule(cx);
    let agent = pool.agent.to_string();
    let windows = pool
        .windows(now, &tz)
        .into_iter()
        .map(|w| pool_window(&agent, w, now, window, cx).border_t_1().border_color(line))
        .collect::<Vec<_>>();
    let windows = v_flex().mt_2().children(windows);
    let body =
        v_flex().debug_selector(move || format!("usage-pool-{agent}")).pt_5().px(CARD_PAD).child(head).children(resets).child(windows);
    Card::new(SharedString::from(format!("usage-pool-{}", pool.agent))).locked(true).head(pool.head()).item(CardItem::any(body))
}

/// An account's banked resets, and Use reset where one is banked.
fn reset_line(account: &Account, now: f64, busy: bool, page: &gpui_kit::Entity<UsagePage>, cx: &App) -> Option<gpui_kit::Div> {
    let line = model::reset_line(account, now)?;
    let p = cx.palette();
    let row = account.clone();
    let page = page.clone();
    let key = account.key.clone();
    Some(
        sized(h_flex().flex_wrap().justify_between().gap_x_6().gap_y_2().pt_3(), 12.5)
            .debug_selector(move || format!("usage-resets-{key}"))
            .text_color(p.muted_foreground)
            .child(
                h_flex()
                    .items_baseline()
                    .gap_2()
                    .child(words::RESETS)
                    .child(div().text_color(p.foreground.opacity(0.8)).child(line.banked)),
            )
            .when(line.can_use, |el| {
                let key = account.key.clone();
                el.child(
                    div().debug_selector(move || format!("use-reset-{key}")).child(
                        Button::outline(
                            SharedString::from(format!("use-reset-{}", account.key)),
                            if busy { words::USING_RESET } else { words::USE_RESET },
                        )
                        .xs()
                        .held(busy)
                        .on_click(move |_, _, cx| page.update(cx, |page, cx| page.ask_reset(row.clone(), cx))),
                    ),
                )
            }),
    )
}

/// One window: its figure column beside its meter, the figure over the meter under 640 px (`max-sm:grid-cols-1
/// max-sm:gap-y-3`).
fn pool_window(agent: &str, window: PoolWindow, now: f64, win: &mut Window, cx: &mut App) -> gpui_kit::Div {
    let narrow = crate::ui::narrow::width(win).sm;
    let p = cx.palette().clone();
    let kind = format!("{:?}", window.kind);
    let tz = chrono::Local;
    let verdict_ink = match window.verdict.tone {
        Tone::Warn => p.warning_foreground,
        Tone::Quiet => p.muted_foreground,
        Tone::Plain => p.foreground.opacity(0.8),
    };
    let figure = v_flex()
        .debug_selector(|| "usage-window-figure".into())
        .when(!narrow, |el| el.flex_none().w(px(WINDOW_COLUMN)))
        .min_w_0()
        .gap_1()
        .child(small(cx).line_height(px(20.)).child(window.word.clone()))
        .child(
            h_flex()
                .items_baseline()
                .gap_2()
                .child(
                    div()
                        .font_family(cx.theme().mono_font_family.clone())
                        .text_2xl()
                        .line_height(px(32.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(if window.stale { p.muted_foreground } else { p.foreground })
                        .child(DigitRoll::new(SharedString::from(format!("left-{agent}-{kind}")), window.left.clone(), px(32.)).roll_in()),
                )
                .child(small(cx).child(words::LEFT)),
        )
        .child(sized(div(), 12.5).line_height(px(20.)).truncate().text_color(verdict_ink).child(window.verdict.word.clone()));
    let segments = window
        .segments
        .iter()
        .enumerate()
        .map(|(ix, s)| {
            let fill: Hsla = if s.stale_at.is_some() {
                p.foreground.opacity(0.2)
            } else if s.used >= 90. {
                p.warning
            } else {
                p.foreground.opacity(0.55)
            };
            let label: SharedString = s.account.label.clone().into();
            let said: SharedString = words::segment_tip(s.used, s.stale_at, s.resets_at, now, &tz).into();
            let id = SharedString::from(format!("segment-{agent}-{kind}-{ix}"));
            let hovered = motion::hovered(id.clone(), win, cx);
            let tip = (*hovered.read(cx)).then(|| {
                let mono = cx.theme().mono_font_family.clone();
                let body = tip_skin(cx)
                    .debug_selector(|| "segment-tip".into())
                    .gap(px(2.))
                    .text_xs()
                    .line_height(px(16.))
                    .child(div().text_color(p.foreground).child(label))
                    .child(div().font_family(mono).text_color(p.muted_foreground).child(said));
                div()
                    .absolute()
                    .top(px(-TIP_OFFSET))
                    .left(relative(0.5))
                    .w_0()
                    .h_0()
                    .flex()
                    .justify_center()
                    .items_end()
                    .child(deferred(body))
            });
            div()
                .id(id)
                .debug_selector(|| "usage-segment".into())
                .relative()
                .flex_1()
                .h_full()
                .rounded(px(SEGMENT_RADIUS))
                .bg(p.foreground.opacity(0.09))
                .on_hover(motion::track_hover(hovered))
                .child(
                    div()
                        .absolute()
                        .top_0()
                        .bottom_0()
                        .left_0()
                        .rounded(px(SEGMENT_RADIUS))
                        .bg(fill)
                        .w(relative((s.used.clamp(0., 100.) / 100.) as f32)),
                )
                .children(s.elapsed.map(|elapsed| {
                    div()
                        .absolute()
                        .top(px(-4.))
                        .bottom(px(-4.))
                        .left(relative(elapsed as f32))
                        .w(px(2.))
                        .rounded_full()
                        .bg(p.foreground.opacity(0.8))
                }))
                .children(tip)
        })
        .collect::<Vec<_>>();
    let meter =
        h_flex().debug_selector(|| "usage-meter".into()).when(!narrow, |el| el.flex_1()).min_w_0().h(px(20.)).gap_1().children(segments);
    let line = if narrow { v_flex().gap_3() } else { h_flex().items_center().gap(px(40.)) };
    line.debug_selector(|| "usage-window".into()).py_4().child(figure).child(meter)
}

fn account_line(row: &AccountLine, cx: &App) -> gpui_kit::Div {
    let p = cx.palette();
    let mark = agent_glyph(&row.account.agent, px(16.), p.foreground, cx);
    let key = row.account.key.clone();
    h_flex()
        .debug_selector(move || format!("usage-account-{key}"))
        .flex_wrap()
        .justify_between()
        .gap_x_6()
        .gap_y_2()
        .py_5()
        .px(CARD_PAD)
        .child(identity(mark, row.account.label.clone(), Some(row.line.clone()), None, cx))
        .when(!row.state.is_empty(), |el| {
            el.child(
                div()
                    .text_size(px(13.))
                    .line_height(px(20.))
                    .text_color(if row.warn { p.warning_foreground } else { p.muted_foreground })
                    .child(row.state.clone()),
            )
        })
}

/// One pool's card with two windows, as most people have, while the accounts are on the way.
fn skeleton(cx: &App) -> gpui_kit::Div {
    let bar = |w: f32, h: f32| Skeleton::new(px(w), px(h));
    let row = || {
        h_flex()
            .gap_4()
            .child(div().w(px(88.)).child(bar(48., 12.)))
            .child(div().flex_1().child(Skeleton::new(px(0.), px(10.)).full()))
            .child(div().w(px(48.)).flex().justify_end().child(bar(32., 12.)))
            .child(div().w(px(160.)).child(bar(96., 12.)))
    };
    v_flex().debug_selector(|| "limits-loading".into()).gap_4().child(bar(64., 14.)).child(
        surface(cx)
            .gap_4()
            .p_4()
            .child(h_flex().gap_3().child(bar(32., 32.).radius(px(8.))).child(v_flex().gap_2().child(bar(160., 14.)).child(bar(112., 12.))))
            .child(row())
            .child(row()),
    )
}
