// SPDX-License-Identifier: AGPL-3.0-only
//! `PermissionPromptRow`: a consent prompt or a question, its options and the pick sent, or its one record line.

use gpui_kit::component::{h_flex, v_flex, ActiveTheme, Icon, Sizable as _};
use gpui_kit::{div, prelude::*, px, AnyElement, App, Context, SharedString, Window};
use wsp_host_client::protocol::SessionsAnswerRequest;

use super::super::rows::{icon_slot, SM};
use super::{rail_follows, FOOT, META, SM_TIGHT, XS};
use crate::screens::composer::dock::{answer_words, sentence};
use crate::thread_view::ThreadView;
use crate::transcript::{Asked, Glyph as RowGlyph, PromptOption, Row, RowKind};
use crate::ui::{Button, Glyph, Palette};

/// The one id a set of ticked boxes travels as (`PICK_JOIN` in packages/protocol/src/words/tools.ts).
pub const PICK_JOIN: &str = "|";
/// What the host said about a pick it would not take, said under the prompt while it stays open.
#[derive(Clone, Debug, PartialEq)]
pub struct Refusal {
    pub said: String,
    pub fix: Option<String>,
}

fn record_glyph(g: RowGlyph) -> Glyph {
    match g {
        RowGlyph::Terminal => Glyph::Terminal,
        RowGlyph::SquarePen | RowGlyph::FilePlus => Glyph::SquarePen,
        RowGlyph::Globe => Glyph::Globe,
        _ => Glyph::Wrench,
    }
}

pub fn answer_request(session: &str, ask: &str, option: &str) -> SessionsAnswerRequest {
    SessionsAnswerRequest { session_id: session.to_owned(), ask_id: ask.to_owned(), option_id: option.to_owned(), reason: None }
}

#[cfg(test)]
thread_local! {
    /// Every answer the cards sent in this test thread, so a test can read the ids a click sent.
    pub static SENT: std::cell::RefCell<Vec<SessionsAnswerRequest>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Sends a pick and keeps any refusal by ask id; the prompt closes on the event the host records, never on the reply.
pub fn answer(view: &mut ThreadView, session: &str, ask: &str, option: &str, cx: &mut Context<ThreadView>) {
    // A row drawn before the prompt closed, or before the listing let go of a question this thread waited behind,
    // can still take a press: an ask no longer open answers nothing.
    let open = view.rows().iter().any(|r| match &r.kind {
        RowKind::Permission { ask: a, open, .. } => a == ask && *open,
        RowKind::Question { ask: a, outcome, .. } => a == ask && outcome.is_none(),
        _ => false,
    });
    if !open {
        return;
    }
    view.refusals.remove(ask);
    let request = answer_request(session, ask, option);
    #[cfg(test)]
    SENT.with(|sent| sent.borrow_mut().push(request.clone()));
    let ask = ask.to_owned();
    view.call(request, cx, move |view, answered, cx| {
        let refused = match answered {
            Ok(result) => {
                let outcome = serde_json::to_value(result.outcome).ok().and_then(|v| v.as_str().map(str::to_owned)).unwrap_or_default();
                answer_words(&outcome).map(|said| Refusal { said: sentence(said), fix: None })
            }
            Err(failure) => Some(Refusal { said: sentence(&failure.message), fix: None }),
        };
        match refused {
            Some(refused) => view.refusals.insert(ask, refused),
            None => view.refusals.remove(&ask),
        };
        cx.notify();
    });
}

/// The refusal the view holds for a prompt, while it holds one.
pub fn refusal_of(view: &ThreadView, ask: &str) -> Option<Refusal> {
    view.refusals.get(ask).cloned()
}

/// `PermissionPromptRow`: a consent prompt or a question, open with its options, or closed as its one record line.
pub(super) fn prompt(
    view: &mut ThreadView,
    ix: usize,
    row: &Row,
    p: &Palette,
    window: &mut Window,
    cx: &mut Context<ThreadView>,
) -> AnyElement {
    // The thread's own open prompt is answered in the composer's dock; its row keeps the waiting record alone.
    let waits = view.transcript().docked_key() == Some(row.key.as_str());
    let (asker, body) = match &row.kind {
        RowKind::Permission { glyph, lead, code, .. } if waits => {
            (None, record(record_glyph(*glyph), lead, code.as_deref(), None, p, cx).into_any_element())
        }
        RowKind::Question { .. } if waits => {
            (None, record(Glyph::MessageCircleQuestionMark, &row.text, None, None, p, cx).into_any_element())
        }
        RowKind::Permission { ask, session, asker, glyph, lead, code, fields, file, options, outcome, open } => {
            let body = if *open {
                let refused = view.refusals.get(ask).cloned();
                consent(&row.key, ask, session, lead, code.as_deref(), fields, file.as_deref(), options, refused, p, window, cx)
            } else {
                record(record_glyph(*glyph), lead, code.as_deref(), outcome.as_deref(), p, cx).into_any_element()
            };
            (asker.clone(), body)
        }
        RowKind::Question { ask, session, asker, questions, outcome } => {
            let body = if outcome.is_none() {
                let refused = view.refusals.get(ask).cloned();
                question(&row.key, ask, session, questions, refused, p, window, cx)
            } else {
                record(Glyph::MessageCircleQuestionMark, &row.text, None, outcome.as_deref(), p, cx).into_any_element()
            };
            (asker.clone(), body)
        }
        _ => return div().into_any_element(),
    };
    let shown = div().min_w_0().pt(px(4.)).pb(px(8.)).when(asker.is_none(), |el| el.px(px(4.))).child(
        v_flex()
            .min_w_0()
            .gap(px(4.))
            .children(asker.clone().map(|a| div().text_size(META.0).line_height(META.1).text_color(p.muted_foreground).child(a)))
            .child(body),
    );
    if asker.is_none() {
        return shown.into_any_element();
    }
    // A subagent's prompt stands in its fold's body: the fold's gutter, then the rail from the chevron's centre.
    let pad = if rail_follows(view, ix) { px(0.) } else { FOOT };
    div()
        .px(px(4.))
        .pb(pad)
        .child(div().ml(px(6.)).border_l_1().border_color(p.border.opacity(0.6)).pl(px(11.)).pt(px(4.)).child(shown))
        .into_any_element()
}

/// `RecordRow`: the glyph in its slot, what was asked on one line, and what was picked after it.
fn record(g: Glyph, says: &str, code: Option<&str>, outcome: Option<&str>, p: &Palette, cx: &App) -> impl IntoElement {
    let ink = if outcome.is_some() { p.foreground.opacity(0.8) } else { p.muted_foreground };
    h_flex()
        .debug_selector(|| "record".into())
        .min_h(px(24.))
        .w_full()
        .min_w_0()
        .gap(px(6.))
        .px(px(2.))
        .py(px(2.))
        .text_size(SM.0)
        .line_height(SM.1)
        .child(icon_slot(g, p.muted_foreground))
        .child(div().flex_none().text_color(ink).child(says.to_owned()))
        .children(code.map(|code| {
            div()
                .flex_1()
                .min_w_0()
                .overflow_hidden()
                .whitespace_nowrap()
                .font_family(cx.theme().mono_font_family.clone())
                .text_size(XS.0)
                .line_height(XS.1)
                .text_color(ink)
                .child(code.to_owned())
        }))
        .children(outcome.map(|o| div().flex_none().min_w_0().truncate().text_color(p.muted_foreground).child(o.to_owned())))
}

/// An open consent: its lead, the fields the lead does not say, the file folded away, the options and any refusal.
#[allow(clippy::too_many_arguments)]
fn consent(
    key: &str,
    ask: &str,
    session: &str,
    lead: &str,
    code: Option<&str>,
    fields: &[(String, String)],
    file: Option<&str>,
    options: &[PromptOption],
    refused: Option<Refusal>,
    p: &Palette,
    window: &mut Window,
    cx: &mut Context<ThreadView>,
) -> AnyElement {
    let mono = cx.theme().mono_font_family.clone();
    let ink = p.foreground.opacity(0.8);
    let says = match code {
        None => div()
            .debug_selector(|| "lead".into())
            .text_size(SM.0)
            .line_height(SM.1)
            .text_color(ink)
            .child(lead.to_owned())
            .into_any_element(),
        Some(code) => h_flex()
            .debug_selector(|| "lead".into())
            .min_w_0()
            .items_start()
            .gap(px(6.))
            .child(div().flex_none().text_size(SM.0).line_height(SM.1).text_color(ink).child(lead.to_owned()))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .py(px(4.))
                    .font_family(mono.clone())
                    .text_size(XS.0)
                    .line_height(XS.1)
                    .text_color(ink)
                    .child(code.to_owned()),
            )
            .into_any_element(),
    };
    let rest = (!fields.is_empty()).then(|| {
        let text = fields.iter().map(|(k, v)| format!("{k}: {v}")).collect::<Vec<_>>().join("\n");
        div().font_family(mono.clone()).text_size(XS.0).line_height(XS.1).text_color(p.muted_foreground).child(text)
    });
    let shown_file = window.use_keyed_state(SharedString::from(format!("show-file-{key}")), cx, |_, _| false);
    let file_open = *shown_file.read(cx);
    let file = file.map(|text| {
        let toggle = shown_file.clone();
        let glyph = if file_open { Glyph::ChevronDown } else { Glyph::ChevronRight };
        v_flex()
            .child(
                h_flex()
                    .id(SharedString::from(format!("show-file-{key}")))
                    .debug_selector(|| "show-file".into())
                    .gap(px(4.))
                    .cursor_pointer()
                    .text_size(XS.0)
                    .line_height(XS.1)
                    .text_color(p.muted_foreground)
                    .hover(|s| s.text_color(p.foreground))
                    .child(Icon::new(glyph).with_size(px(12.)))
                    .child("show the file")
                    .on_click(move |_, _, cx| {
                        toggle.update(cx, |open, cx| {
                            *open = !*open;
                            cx.notify();
                        })
                    }),
            )
            .when(file_open, |el| {
                el.child(
                    div()
                        .id(SharedString::from(format!("file-{key}")))
                        .max_h(px(320.))
                        .overflow_y_scroll()
                        .pt(px(4.))
                        .font_family(mono.clone())
                        .text_size(XS.0)
                        .line_height(XS.1)
                        .text_color(p.muted_foreground)
                        .child(text.to_owned()),
                )
            })
    });
    let buttons = options.iter().map(|option| {
        let (session, ask, id) = (session.to_owned(), ask.to_owned(), option.id.clone());
        let view = cx.entity().downgrade();
        Button::outline(SharedString::from(format!("option-{ask}-{}", option.id)), option.label.clone()).xs().on_click(move |_, _, cx| {
            _ = view.update(cx, |view, cx| answer(view, &session, &ask, &id, cx));
        })
    });
    v_flex()
        .min_w_0()
        .gap(px(4.))
        .child(says)
        .children(rest)
        .children(file)
        .child(h_flex().debug_selector(|| "options".into()).flex_wrap().gap(px(6.)).pt(px(2.)).children(buttons))
        .children(refused.map(|r| refusal(r, p)))
        .into_any_element()
}

/// `PromptRefusal`: 13 px on a 20 px line in the error ink, the fix after it in the foreground.
pub(crate) fn refusal(refused: Refusal, p: &Palette) -> impl IntoElement {
    let fix = refused.fix.map(|fix| format!(" {fix}")).unwrap_or_default();
    let whole = format!("{}{fix}", refused.said);
    let body = gpui_kit::HighlightStyle { color: Some(p.foreground), ..Default::default() };
    let text = gpui_kit::StyledText::new(whole).with_highlights([(refused.said.len()..refused.said.len() + fix.len(), body)]);
    div().debug_selector(|| "refusal".into()).text_size(px(13.)).line_height(px(20.)).text_color(p.error_foreground).child(text)
}

/// `QuestionPrompt`: one single-answer question answers on the press; any other takes ticks and an Answer button.
#[allow(clippy::too_many_arguments)]
fn question(
    key: &str,
    ask: &str,
    session: &str,
    questions: &[Asked],
    refused: Option<Refusal>,
    p: &Palette,
    window: &mut Window,
    cx: &mut Context<ThreadView>,
) -> AnyElement {
    let one_shot = questions.len() == 1 && !questions[0].multiple;
    let ticks = window.use_keyed_state(SharedString::from(format!("ticks-{key}")), cx, |_, _| Vec::<String>::new());
    let ticked = ticks.read(cx).clone();
    let answered = questions.iter().all(|q| q.options.iter().any(|o| ticked.contains(&o.id)));
    let view = cx.entity().downgrade();
    let ink = p.foreground.opacity(0.8);
    let items = questions.iter().enumerate().map(|(qx, q)| {
        let options = q.options.iter().map(|option| {
            let description = option
                .description
                .clone()
                .filter(|d| !d.is_empty())
                .map(|d| div().text_size(META.0).line_height(META.1).text_color(p.muted_foreground).child(d));
            if one_shot {
                let (view, session, ask, id) = (view.clone(), session.to_owned(), ask.to_owned(), option.id.clone());
                let button = Button::outline(SharedString::from(format!("answer-{ask}-{}", option.id)), option.label.clone())
                    .xs()
                    .on_click(move |_, _, cx| {
                        _ = view.update(cx, |view, cx| answer(view, &session, &ask, &id, cx));
                    });
                return v_flex().min_w_0().items_start().gap(px(2.)).child(button).children(description).into_any_element();
            }
            let (ticks, id, multiple) = (ticks.clone(), option.id.clone(), q.multiple);
            let siblings: Vec<String> = q.options.iter().map(|o| o.id.clone()).collect();
            h_flex()
                .id(SharedString::from(format!("tick-{key}-{qx}-{}", option.id)))
                .debug_selector(|| "tick".into())
                .min_w_0()
                .items_start()
                .gap(px(8.))
                .py(px(2.))
                .cursor_pointer()
                .child(div().mt(px(2.)).child(crate::ui::controls::neutral_checkbox(
                    SharedString::from(format!("tick-box-{key}-{qx}-{}", option.id)),
                    ticked.contains(&option.id),
                    window,
                    cx,
                )))
                .child(
                    v_flex()
                        .min_w_0()
                        .gap(px(2.))
                        .child(div().text_size(SM_TIGHT.0).line_height(SM_TIGHT.1).text_color(ink).child(option.label.clone()))
                        .children(description),
                )
                .on_click(move |_, _, cx| {
                    ticks.update(cx, |held, cx| {
                        if let Some(at) = held.iter().position(|o| *o == id) {
                            held.remove(at);
                        } else {
                            if !multiple {
                                held.retain(|o| !siblings.contains(o));
                            }
                            held.push(id.clone());
                        }
                        cx.notify();
                    })
                })
                .into_any_element()
        });
        v_flex()
            .min_w_0()
            .gap(px(6.))
            .children(
                q.header
                    .clone()
                    .filter(|h| !h.is_empty())
                    .map(|h| div().text_size(META.0).line_height(META.1).text_color(p.muted_foreground).child(h)),
            )
            .child(div().text_size(SM.0).line_height(SM.1).text_color(ink).child(q.question.clone()))
            .child(v_flex().min_w_0().gap(px(8.)).pt(px(2.)).children(options))
    });
    let submit = (!one_shot).then(|| {
        let (view, session, ask, picked) = (view.clone(), session.to_owned(), ask.to_owned(), ticked.join(PICK_JOIN));
        h_flex().child(Button::outline(SharedString::from(format!("answer-{ask}")), "Answer").xs().held(!answered).on_click(
            move |_, _, cx| {
                _ = view.update(cx, |view, cx| answer(view, &session, &ask, &picked, cx));
            },
        ))
    });
    v_flex()
        .min_w_0()
        .gap(px(4.))
        .child(v_flex().debug_selector(|| "question".into()).min_w_0().gap(px(12.)).children(items).children(submit))
        .children(refused.map(|r| refusal(r, p)))
        .into_any_element()
}
