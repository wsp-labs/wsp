// SPDX-License-Identifier: AGPL-3.0-only
//! The Limits tab: each agent's subscription accounts as a pool, one line per window of what is left with the verdict
//! under it and a segment per account; then the accounts no window speaks for. The accounts and the tokens are two
//! ledgers and are never summed.

use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{h_flex, v_flex, ActiveTheme};
use gpui_kit::{div, prelude::*, px, relative, AnyElement, App, Context, FontWeight, Hsla, SharedString, Window};

use super::model::{self, Account, AccountLine, Pool, PoolWindow, Tone};
use super::parts::{agent_glyph, bone, card, identity, quiet, rule, sized, surface, BLOCK_GAP, CARD_PAD};
use super::{words, UsagePage};
use crate::ui::{ActivePalette, Button};

/// The window's figure column at its `14rem`.
const WINDOW_COLUMN: f32 = 224.;
const SEGMENT_RADIUS: f32 = 5.;

pub fn render(page: &UsagePage, now: f64, _: &mut Window, cx: &mut Context<UsagePage>) -> AnyElement {
    let entity = cx.entity();
    let accounts = page.accounts.value.as_ref().map(|a| &a.accounts[..]);
    let el = match (accounts, &page.accounts.refused) {
        (None, Some(refused)) => {
            card("usage-limits", None, surface(cx).child(quiet(words::limits_refused(refused), cx).px(CARD_PAD).py_5()), cx)
        }
        (None, None) => skeleton(cx),
        (Some([]), _) => card("usage-limits", None, surface(cx).child(quiet(words::NO_ACCOUNTS, cx).px(CARD_PAD).py_5()), cx),
        (Some(accounts), _) => {
            let limits = model::limits(accounts);
            let no_limit = limits.no_limit_word();
            let quiet_card = limits.has_quiet_card().then(|| {
                let line = rule(cx);
                let rows = limits
                    .quiet
                    .iter()
                    .enumerate()
                    .map(|(ix, row)| account_line(row, cx).when(ix > 0, |el| el.border_t_1().border_color(line)));
                let body = v_flex()
                    .gap_4()
                    .when(!limits.quiet.is_empty(), |el| el.child(surface(cx).children(rows)))
                    .children(no_limit.map(|word| quiet(word, cx).px(CARD_PAD).debug_selector(|| "no-limit".into())));
                card("usage-no-plan", Some(words::NO_PLAN_LIMIT.into()), body, cx)
            });
            v_flex()
                .gap(BLOCK_GAP)
                .children(limits.pools.iter().map(|pool| pool_card(pool, now, &page.resetting, &entity, cx)))
                .children(quiet_card)
        }
    };
    el.debug_selector(|| "usage-limits-tab".into()).into_any_element()
}

fn small(cx: &App) -> gpui_kit::Div {
    sized(div(), 12.5).text_color(cx.palette().muted_foreground)
}

fn pool_card(pool: &Pool, now: f64, resetting: &[String], page: &gpui_kit::Entity<UsagePage>, cx: &App) -> gpui_kit::Div {
    let p = cx.palette();
    let tz = chrono::Local;
    let mark = agent_glyph(pool.agent, px(16.), p.foreground, cx);
    let head =
        h_flex().flex_wrap().justify_between().gap_x_6().gap_y_2().child(identity(mark, pool.title(), Some(pool.line()), None, cx)).child(
            sized(h_flex().flex_wrap().gap_x_5(), 12.5).text_color(p.muted_foreground).children(pool.burn_word()).child(pool.checked(now)),
        );
    let resets = pool.accounts.iter().filter_map(|account| reset_line(account, now, resetting.contains(&account.key), page, cx));
    let line = rule(cx);
    let windows = v_flex()
        .mt_2()
        .children(pool.windows(now, &tz).into_iter().map(|window| pool_window(window, now, cx).border_t_1().border_color(line)));
    let agent = pool.agent.to_string();
    let body = surface(cx).child(
        v_flex().debug_selector(move || format!("usage-pool-{agent}")).pt_5().px(CARD_PAD).child(head).children(resets).child(windows),
    );
    card("usage-pool", Some(pool.head().into()), body, cx)
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
                        .on_click(move |_, window, cx| page.update(cx, |page, cx| page.ask_reset(row.clone(), window, cx))),
                    ),
                )
            }),
    )
}

fn pool_window(window: PoolWindow, now: f64, cx: &App) -> gpui_kit::Div {
    let p = cx.palette();
    let tz = chrono::Local;
    let verdict_ink = match window.verdict.tone {
        Tone::Warn => p.warning_foreground,
        Tone::Quiet => p.muted_foreground,
        Tone::Plain => p.foreground.opacity(0.8),
    };
    let figure = v_flex()
        .flex_none()
        .w(px(WINDOW_COLUMN))
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
                        .child(window.left.clone()),
                )
                .child(small(cx).child(words::LEFT)),
        )
        .child(sized(div(), 12.5).line_height(px(20.)).truncate().text_color(verdict_ink).child(window.verdict.word.clone()));
    let kind = format!("{:?}", window.kind);
    let meter = h_flex().flex_1().min_w_0().h(px(20.)).gap_1().children(window.segments.iter().enumerate().map(|(ix, s)| {
        let fill: Hsla = if s.stale_at.is_some() {
            p.foreground.opacity(0.2)
        } else if s.used >= 90. {
            p.warning
        } else {
            p.foreground.opacity(0.55)
        };
        let label: SharedString = s.account.label.clone().into();
        let said: SharedString = words::segment_tip(s.used, s.stale_at, s.resets_at, now, &tz).into();
        let (border, popover, shadow, fg, muted, mono) =
            (p.border, p.popover, p.tooltip_shadow.clone(), p.foreground, p.muted_foreground, cx.theme().mono_font_family.clone());
        div()
            .id(SharedString::from(format!("segment-{kind}-{ix}")))
            .debug_selector(|| "usage-segment".into())
            .relative()
            .flex_1()
            .h_full()
            .rounded(px(SEGMENT_RADIUS))
            .bg(p.foreground.opacity(0.09))
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
            .tooltip(move |window, cx| {
                let (label, said, mono) = (label.clone(), said.clone(), mono.clone());
                Tooltip::element(move |_, _| {
                    v_flex()
                        .gap(px(2.))
                        .text_xs()
                        .line_height(px(16.))
                        .child(div().text_color(fg).child(label.clone()))
                        .child(div().font_family(mono.clone()).text_color(muted).child(said.clone()))
                })
                .m_0()
                .rounded(px(8.))
                .py_1()
                .px_2()
                .border_color(border)
                .bg(popover)
                .shadow(shadow.clone())
                .build(window, cx)
            })
    }));
    h_flex().debug_selector(|| "usage-window".into()).items_center().gap(px(40.)).py_4().child(figure).child(meter)
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
    let row = || {
        h_flex()
            .gap_4()
            .child(div().w(px(88.)).child(bone(px(48.), px(12.), cx)))
            .child(div().flex_1().h(px(10.)).rounded(px(6.)).bg(cx.palette().border))
            .child(div().w(px(48.)).flex().justify_end().child(bone(px(32.), px(12.), cx)))
            .child(div().w(px(160.)).child(bone(px(96.), px(12.), cx)))
    };
    v_flex().debug_selector(|| "limits-loading".into()).gap_4().child(bone(px(64.), px(14.), cx)).child(
        surface(cx)
            .gap_4()
            .p_4()
            .child(
                h_flex()
                    .gap_3()
                    .child(div().size(px(32.)).rounded(px(8.)).bg(cx.palette().border))
                    .child(v_flex().gap_2().child(bone(px(160.), px(14.), cx)).child(bone(px(112.), px(12.), cx))),
            )
            .child(row())
            .child(row()),
    )
}
