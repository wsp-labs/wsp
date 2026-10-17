// SPDX-License-Identifier: AGPL-3.0-only
//! The transcript's cards as the web app draws them: prompts, plans, subagent folds, children, changed files, forks, files.

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use gpui_kit::component::{h_flex, v_flex, ActiveTheme, Icon, Sizable as _};
use gpui_kit::{
    anchored, deferred, div, img, linear_color_stop, linear_gradient, point, prelude::*, px, Anchor, AnyElement, App, BoxShadow,
    ClipboardItem, Context, FontWeight, Hsla, Image, ImageFormat, ImageSource, ObjectFit, Pixels, SharedString, StyledText, Window,
};
use wsp_host_client::protocol::{SessionsAnswerRequest, SessionsAttachmentRequest};

use super::rows::{icon_slot, SM};
use super::{find, markdown};
use crate::screens::composer::dock::{answer_words, sentence};
use crate::screens::{navigate, Pane, Route};
use crate::thread_view::ThreadView;
use crate::transcript::{
    self, Asked, ChangedFile, Child, ChildPart, FileTile, Glyph as RowGlyph, Listed, PromptOption, Row, RowKind, StartFailed, ThreadAt,
};
use crate::ui::{catalog, tip, ActivePalette, Button, Glyph, Palette, Status, StatusMark};

/// `text-sm` alone, and `text-sm leading-5`.
const SM_TIGHT: (Pixels, Pixels) = (px(14.), px(20.));
/// `text-xs`, `text-xs leading-4`.
const XS: (Pixels, Pixels) = (px(12.), px(16.));
/// `text-[11px] leading-4`.
const META: (Pixels, Pixels) = (px(11.), px(16.));
/// `text-[11px]` under the page's `line-height: 1.5`.
const META_LOOSE: (Pixels, Pixels) = (px(11.), px(16.5));
/// `pb-4`: every card row's foot.
const FOOT: Pixels = px(16.);
/// A plan folds behind Expand plan at `max-h-104`, its last `h-24` faded into the card.
const PLAN_FOLDED: Pixels = px(416.);
const PLAN_FADE: Pixels = px(96.);
/// The one id a set of ticked boxes travels as (`PICK_JOIN` in packages/protocol/src/words/tools.ts).
pub const PICK_JOIN: &str = "|";

/// One attachment of a person's message as the bubble holds it.
#[derive(Clone)]
pub enum Shot {
    /// Not an image: its tile draws off its record alone.
    File,
    Asking,
    /// The image, and its size in pixels where its header says it.
    Held(Arc<Image>, Option<(f32, f32)>),
    /// The host keeps none, or did not answer; asked again only once the view drops the row's kept state and draws it anew.
    Missing,
}

/// Takes out the attachments of the rows `keep` turns away; a row drew its images only once it held them all.
pub fn drop_images(images: &mut HashMap<String, Vec<Shot>>, released: &mut Vec<Arc<Image>>, keep: impl Fn(&str) -> bool) {
    for (_, shots) in images.extract_if(|key, _| !keep(key)) {
        if shots.iter().all(|s| !matches!(s, Shot::Asking | Shot::Missing)) {
            released.extend(shots.into_iter().filter_map(|s| match s {
                Shot::Held(image, _) => Some(image),
                _ => None,
            }));
        }
    }
}

/// What the host said about a pick it would not take, said under the prompt while it stays open.
#[derive(Clone, Debug, PartialEq)]
pub struct Refusal {
    pub said: String,
    pub fix: Option<String>,
}

/// The text a plan row's parsed document is built from: the collapsed preview while shut, the plan while open.
pub fn plan_text(row: &Row) -> String {
    match row.kind {
        RowKind::Plan { open, .. } => transcript::plan_shown(&row.text, open),
        _ => row.text.clone(),
    }
}

/// The row's foot, `pb-4`; none where the next row hangs on the same rail, which the card pads as a foot cannot see it.
pub fn foot(kind: &RowKind) -> Pixels {
    match kind {
        RowKind::Subagent { .. } | RowKind::Threads { .. } | RowKind::Rule => px(0.),
        RowKind::Permission { asker: Some(_), .. } | RowKind::Question { asker: Some(_), .. } => px(0.),
        _ => FOOT,
    }
}

/// Whether the row after `ix` is a prompt a subagent raised, which hangs on the same fold's rail.
fn rail_follows(view: &ThreadView, ix: usize) -> bool {
    matches!(
        view.rows().get(ix + 1).map(|r| &r.kind),
        Some(RowKind::Permission { asker: Some(_), .. } | RowKind::Question { asker: Some(_), .. })
    )
}

pub fn card(view: &mut ThreadView, ix: usize, row: &Row, window: &mut Window, cx: &mut Context<ThreadView>) -> AnyElement {
    let p = cx.palette().clone();
    match &row.kind {
        RowKind::Permission { .. } | RowKind::Question { .. } => prompt(view, ix, row, &p, window, cx),
        RowKind::Plan { title, open } => plan(view, row, title, *open, &p, window, cx),
        RowKind::Subagent { id, title, state, open, lines } => {
            let page = subagent_page(view, id);
            subagent(view, id, title, state.as_deref(), *open, lines, rail_follows(view, ix), page, &p, cx)
        }
        RowKind::Spawn { children } => spawn(view, &row.key, children, &p, cx),
        RowKind::ChangedFiles { label, files, others } => changed_files(&row.key, label, files, others, &p, window, cx),
        RowKind::Fork { open, .. } => fork(&row.text, open.clone(), &p, cx),
        RowKind::StartFailed { failed } => start_failed(view, &row.key, failed, &p, window, cx),
        RowKind::Threads { open, .. } => threads(&row.key, *open, view.listing(), &p, window, cx),
        RowKind::Rule => rule_line("machine-line", &row.text, None, p.muted_foreground),
        RowKind::Slate => slate_line(&row.key, &row.text, &p, window, cx),
        _ => div().into_any_element(),
    }
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
fn prompt(view: &mut ThreadView, ix: usize, row: &Row, p: &Palette, window: &mut Window, cx: &mut Context<ThreadView>) -> AnyElement {
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
    let fix = refused.fix.map(|fix| div().text_color(p.foreground).child(format!(" {fix}")));
    h_flex()
        .debug_selector(|| "refusal".into())
        .flex_wrap()
        .text_size(px(13.))
        .line_height(px(20.))
        .text_color(p.error_foreground)
        .child(refused.said)
        .children(fix)
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

/// The outline keycap at `icon-xs`, 24 px square: the plan's menu button.
fn outline_icon(id: SharedString, g: Glyph, p: &Palette) -> gpui_kit::Stateful<gpui_kit::Div> {
    let bevel =
        vec![BoxShadow { color: p.keycap_top, offset: point(px(0.), px(1.)), blur_radius: px(0.), spread_radius: px(0.), inset: true }];
    h_flex()
        .id(id)
        .debug_selector(|| "menu-button".into())
        .flex_none()
        .justify_center()
        .size(px(24.))
        .rounded(px(8.))
        .border_1()
        .border_color(p.input)
        .bg(p.input_fill)
        .cursor_pointer()
        .hover(|s| s.bg(p.input_fill_hover))
        .child(div().absolute().inset_0().rounded(px(7.)).shadow(bevel))
        .child(Icon::new(g).with_size(px(16.)).text_color(p.muted_foreground))
}

/// A ghost act at `xs`: 24 px high, a 12 px glyph, the muted word, and where `live` the foreground under the pointer.
fn ghost_xs(id: SharedString, g: Glyph, label: Option<&'static str>, live: bool, p: &Palette) -> gpui_kit::Stateful<gpui_kit::Div> {
    h_flex()
        .id(id)
        .flex_none()
        .justify_center()
        .h(px(24.))
        .when(label.is_none(), |el| el.w(px(24.)))
        .when(label.is_some(), |el| el.px(px(8.)))
        .gap(px(4.))
        .rounded(px(8.))
        .border_1()
        .border_color(gpui_kit::transparent_black())
        .text_size(XS.0)
        .line_height(XS.1)
        .text_color(p.muted_foreground)
        .when(live, |el| el.cursor_pointer().hover(|s| s.bg(p.accent).text_color(p.foreground)))
        .child(Icon::new(g).with_size(px(12.)).text_color(p.muted_foreground))
        .children(label)
}

/// `buildProposedPlanMarkdownFilename`: the title's words joined by hyphens, `plan.md` without one.
pub fn plan_filename(title: Option<&str>) -> String {
    let mut name = String::new();
    for c in title.unwrap_or("plan").to_lowercase().chars().filter(|c| !"`'\".,!?()[]{}".contains(*c)) {
        if c.is_ascii_alphanumeric() {
            name.push(c);
        } else if !name.ends_with('-') {
            name.push('-');
        }
    }
    let name = name.trim_matches('-');
    format!("{}.md", if name.is_empty() { "plan" } else { name })
}

/// Download as markdown: the window's own save dialog, opened on the Downloads folder under the plan's name as a
/// browser's download prompt is, then the plan written where the person says.
fn download(markdown: &str, cx: &mut App) {
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
    let folder = Some(home.join("Downloads")).filter(|d| d.is_dir()).unwrap_or(home);
    let asked = cx.prompt_for_new_path(&folder, Some(&plan_filename(plan_heading(markdown).as_deref())));
    let text = format!("{}\n", markdown.trim_end());
    cx.background_spawn(async move {
        let Ok(Ok(Some(path))) = asked.await else { return };
        if let Err(e) = std::fs::write(&path, text) {
            eprintln!("desktop-gpui: downloading the plan to {}: {e}", path.display());
        }
    })
    .detach();
}

/// `proposedPlanTitle`: the first heading's words, as the file is named, which the row's title falls back from.
fn plan_heading(markdown: &str) -> Option<String> {
    markdown.lines().find_map(|line| {
        let t = line.trim_start();
        let lead = line.len() - t.len();
        let hashes = t.chars().take_while(|c| *c == '#').count();
        let words = t[hashes..].trim();
        (lead <= 3 && (1..=6).contains(&hashes) && t[hashes..].starts_with(char::is_whitespace) && !words.is_empty())
            .then(|| words.to_owned())
    })
}

/// `ProposedPlanCard`: the card, its title and menu, the plan, and past 900 characters or 20 lines its fold.
#[allow(clippy::too_many_arguments)]
fn plan(
    view: &mut ThreadView,
    row: &Row,
    title: &str,
    open: bool,
    p: &Palette,
    window: &mut Window,
    cx: &mut Context<ThreadView>,
) -> AnyElement {
    let folds = transcript::plan_folds(&row.text);
    let body = view.body(row, cx);
    let cwd = view.transcript().cwd().map(str::to_owned);
    let menu = window.use_keyed_state(SharedString::from(format!("plan-menu-{}", row.key)), cx, |_, _| false);
    let menu_open = *menu.read(cx);
    let toggle = format!("plan:{}", row.key);
    let opener = menu.clone();
    let trigger = outline_icon(SharedString::from(format!("plan-actions-{}", row.key)), Glyph::Ellipsis, p).on_click(move |_, _, cx| {
        opener.update(cx, |open, cx| {
            *open = !*open;
            cx.notify();
        })
    });
    let popup = menu_open.then(|| {
        let item = |id: &'static str, label: &'static str, act: Box<dyn Fn(&mut App)>| {
            let menu = menu.clone();
            div()
                .id(id)
                .debug_selector(move || id.into())
                .flex()
                .items_center()
                .min_h(px(28.))
                .px(px(8.))
                .py(px(4.))
                .rounded(px(8.))
                .text_size(SM_TIGHT.0)
                .line_height(SM_TIGHT.1)
                .text_color(p.foreground)
                .cursor_pointer()
                .hover(|s| s.bg(p.accent))
                .child(label)
                .on_click(move |_, _, cx| {
                    act(cx);
                    menu.update(cx, |open, cx| {
                        *open = false;
                        cx.notify();
                    });
                })
        };
        let (copied, downloaded) = (format!("{}\n", row.text.trim_end()), row.text.clone());
        let closer = menu.clone();
        div().absolute().right_0().top(px(28.)).child(
            deferred(
                anchored().anchor(Anchor::TopRight).snap_to_window().child(
                    v_flex()
                        .id(SharedString::from(format!("plan-menu-{}", row.key)))
                        .debug_selector(|| "plan-menu".into())
                        .occlude()
                        .min_w(px(128.))
                        .p(px(4.))
                        .rounded(px(12.))
                        .border_1()
                        .border_color(p.foreground.opacity(0.1))
                        .bg(p.popover)
                        .shadow(p.popover_shadow.clone())
                        .child(item(
                            "plan-copy",
                            "Copy to clipboard",
                            Box::new(move |cx| cx.write_to_clipboard(ClipboardItem::new_string(copied.clone()))),
                        ))
                        .child(item("plan-download", "Download as markdown", Box::new(move |cx| download(&downloaded, cx))))
                        .on_mouse_down_out(move |_, _, cx| {
                            closer.update(cx, |open, cx| {
                                *open = false;
                                cx.notify();
                            })
                        }),
                ),
            )
            .with_priority(1),
        )
    });
    let folded = folds && !open;
    let (mid, fade) = (p.card.opacity(0.8), p.card.opacity(0.95));
    let shown = div()
        .debug_selector(|| "plan-body".into())
        .relative()
        .when(folded, |el| el.max_h(PLAN_FOLDED).overflow_hidden())
        .child(markdown::reply(&body, false, None, cwd.as_deref(), cx))
        .when(folded, |el| {
            el.child(
                v_flex()
                    .absolute()
                    .left_0()
                    .right_0()
                    .bottom_0()
                    .h(PLAN_FADE)
                    .child(div().h(PLAN_FADE / 2.).bg(linear_gradient(
                        180.,
                        linear_color_stop(p.card.opacity(0.), 0.),
                        linear_color_stop(mid, 1.),
                    )))
                    .child(div().h(PLAN_FADE / 2.).bg(linear_gradient(180., linear_color_stop(mid, 0.), linear_color_stop(fade, 1.)))),
            )
        });
    let expand = folds.then(|| {
        let toggle = toggle.clone();
        let label = if open { "Collapse plan" } else { "Expand plan" };
        h_flex().mt(px(16.)).justify_center().child(
            Button::outline(SharedString::from(format!("plan-fold-{}", row.key)), label)
                .on_click(cx.listener(move |this, _, _, cx| this.toggle(&toggle, cx))),
        )
    });
    div()
        .min_w_0()
        .px(px(4.))
        .py(px(2.))
        .child(
            v_flex()
                .debug_selector(|| "plan".into())
                .rounded(px(14.))
                .border_1()
                .border_color(p.border.opacity(0.8))
                .bg(p.card.opacity(0.7))
                .p(px(20.))
                .child(
                    h_flex()
                        .flex_wrap()
                        .justify_between()
                        .gap(px(12.))
                        .child(
                            div()
                                .debug_selector(|| "plan-title".into())
                                .min_w_0()
                                .truncate()
                                .text_size(SM_TIGHT.0)
                                .line_height(SM_TIGHT.1)
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(p.foreground)
                                .child(title.to_owned()),
                        )
                        .child(div().relative().child(trigger).children(popup)),
                )
                .child(div().mt(px(16.)).child(shown).children(expand)),
        )
        .into_any_element()
}

/// What `ink` at `share` over `ground` shows, as one solid color.
fn mix_over(ink: Hsla, share: f32, ground: Hsla) -> Hsla {
    ground.blend(ink.opacity(share))
}

/// `SubagentFoldRow`: the chevron, the title, its state and elapsed, the page arrow, and open, its lines on the rail.
#[allow(clippy::too_many_arguments)]
fn subagent(
    view: &ThreadView,
    id: &str,
    title: &str,
    state: Option<&str>,
    open: bool,
    lines: &[transcript::SubLine],
    rail_follows: bool,
    page: Option<String>,
    p: &Palette,
    cx: &mut Context<ThreadView>,
) -> AnyElement {
    let mono = cx.theme().mono_font_family.clone();
    let toggle = id.to_owned();
    // `Spaced`: the state word and the elapsed apart by a space and `ms-2`, never a dot.
    let parts: Vec<String> = match state {
        Some(state) => match state.split_once(' ') {
            Some((word, rest)) if word.chars().all(char::is_alphabetic) => vec![word.to_owned(), rest.to_owned()],
            _ => vec![state.to_owned()],
        },
        None => Vec::new(),
    };
    let slot = h_flex()
        .flex_none()
        .gap(px(8.))
        .font_family(mono.clone())
        .text_size(META.0)
        .line_height(META.1)
        .text_color(p.muted_foreground)
        .children(parts);
    let glyph = if open { Glyph::ChevronDown } else { Glyph::ChevronRight };
    let trigger = h_flex()
        .id(SharedString::from(format!("subagent-{id}")))
        .debug_selector(|| "subagent-trigger".into())
        .flex_1()
        .min_w_0()
        .gap(px(6.))
        .cursor_pointer()
        .child(Icon::new(glyph).with_size(px(12.)).text_color(p.muted_foreground))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_size(SM_TIGHT.0)
                .line_height(SM_TIGHT.1)
                .text_color(p.foreground.opacity(0.8))
                .child(title.to_owned()),
        )
        .child(slot)
        .on_click(cx.listener(move |this, _, _, cx| this.toggle(&toggle, cx)));
    // `icon-micro` ghost: Open its page, its word on the tooltip; drawn only where the lead's page has an address.
    let page = page.map(|route| {
        h_flex()
            .id(SharedString::from(format!("subagent-open-{id}")))
            .debug_selector(|| "subagent-page".into())
            .flex_none()
            .justify_center()
            .size(px(20.))
            .rounded(px(6.))
            .cursor_pointer()
            .hover(|s| s.bg(p.accent))
            .tooltip(tip(OPEN_PAGE))
            .child(Icon::new(Glyph::ArrowUpRight).with_size(px(12.)).text_color(p.muted_foreground))
            .on_click(move |_, _, cx| go(Route::Screen(route.clone().into()), cx))
    });
    let body = open.then(|| {
        v_flex()
            .debug_selector(|| "subagent-body".into())
            .min_w_0()
            .gap(px(4.))
            .ml(px(6.))
            .border_l_1()
            .border_color(p.border.opacity(0.6))
            .pt(px(4.))
            .pl(px(11.))
            .children(lines.iter().enumerate().map(|(i, line)| {
                // Plain text, so find's matches are painted here off the marks it hands each part.
                let marked = |text: &str, detail: bool| marked(view, id, &find::subagent_part(i, detail), text, cx);
                let code = |text: StyledText| {
                    div().font_family(mono.clone()).text_size(XS.0).line_height(XS.1).text_color(p.muted_foreground).child(text)
                };
                let label = (!line.text.is_empty()).then(|| {
                    if line.mono {
                        code(marked(&line.text, false))
                    } else {
                        div().text_size(SM.0).line_height(SM.1).text_color(p.foreground.opacity(0.8)).child(marked(&line.text, false))
                    }
                });
                v_flex().min_w_0().children(label).children(line.detail.as_deref().map(|d| code(marked(d, true))))
            }))
    });
    v_flex()
        .min_w_0()
        .px(px(4.))
        .pb(if rail_follows { px(0.) } else { FOOT })
        .child(h_flex().min_w_0().gap(px(6.)).child(trigger).children(page))
        .children(body)
        .into_any_element()
}

/// A row, its part and the spans find marked on it.
#[cfg(test)]
type Painted = (String, String, Vec<std::ops::Range<usize>>);

#[cfg(test)]
thread_local! {
    /// Each part of a row a card painted find's marks on in this test thread: its row, its part and the marked spans.
    pub static PAINTED: std::cell::RefCell<Vec<Painted>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// A part of a row drawn as plain text, with find's matches in it marked while the bar is open.
fn marked(view: &ThreadView, key: &str, part: &str, text: &str, cx: &App) -> StyledText {
    let marks = find::marks(view, key, part, text, cx);
    #[cfg(test)]
    if !marks.is_empty() {
        PAINTED.with(|p| p.borrow_mut().push((key.to_owned(), part.to_owned(), marks.iter().map(|(r, _)| r.clone()).collect())));
    }
    StyledText::new(text.to_owned()).with_highlights(marks)
}

/// `SpawnTiles`: one column, each child a `ThreadRow` or `SubagentRow` in the row's box.
fn spawn(view: &ThreadView, key: &str, children: &[Child], p: &Palette, cx: &App) -> AnyElement {
    let lead = lead_of(view);
    v_flex()
        .debug_selector(|| "spawn".into())
        .min_w_0()
        .children(children.iter().enumerate().map(|(at_ix, child)| spawn_row(key, at_ix, child, lead.as_ref(), p, cx)))
        .into_any_element()
}

/// The lead thread the view shows, where its workspace is known: what a subagent's page is opened under.
fn lead_of(view: &ThreadView) -> Option<ThreadAt> {
    let workspace =
        view.followed_workspace().map(str::to_owned).or_else(|| view.transcript().listed().and_then(|l| l.workspace.clone()))?;
    Some(ThreadAt { workspace, thread: view.thread()?.to_owned() })
}

/// The address of a subagent's page under `lead`, by the call that launched it (`subagent:<workspace>/<thread>/<call>`).
pub fn subagent_route(lead: &ThreadAt, call: &str) -> String {
    format!("{SUBAGENT}:{}/{}/{call}", lead.workspace, lead.thread)
}

fn subagent_page(view: &ThreadView, id: &str) -> Option<String> {
    Some(subagent_route(&lead_of(view)?, id.strip_prefix("subagent:")?))
}

const OPEN_PAGE: &str = "Open its page";
/// The name the subagent's page registers under.
pub const SUBAGENT: &str = "subagent";

#[cfg(test)]
thread_local! {
    /// Every move the cards asked the frame for in this test thread.
    pub static ROUTED: std::cell::RefCell<Vec<Route>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Moves the window to a thread or a page.
fn go(route: Route, cx: &mut App) {
    #[cfg(test)]
    ROUTED.with(|r| r.borrow_mut().push(route.clone()));
    navigate(route, cx);
}

fn open_thread(at: &ThreadAt, cx: &mut App) {
    go(Route::Thread { workspace_id: at.workspace.clone(), thread_id: at.thread.clone() }, cx);
}

/// The listing's kind as the status slot draws it.
fn listed_status(listed: &Listed) -> Status {
    crate::screens::sidebar::tile::status_of(listed.kind, listed.slot.clone())
}

/// One child in the row's box (`rowBox`, `ThreadRow`, `SubagentRow`): 36 px, or at least 48 with a second line; the
/// mark, the title over the computer and the note, then the status slot. A press opens the thread, or a subagent's page.
fn spawn_row(key: &str, ix: usize, child: &Child, lead: Option<&ThreadAt>, p: &Palette, cx: &App) -> AnyElement {
    let listed = child.listed.as_ref();
    let status = match (listed, child.settled, child.status.as_deref()) {
        (Some(listed), ..) => Some(listed_status(listed)),
        (None, true, _) | (None, _, None) => None,
        (None, false, Some("running")) => Some(Status::Working(SharedString::default())),
        (None, false, Some("done")) => Some(Status::Done),
        (None, false, Some("failed")) => Some(Status::Failed),
        _ => None,
    };
    let settled = listed.map_or(child.settled, |l| l.part == ChildPart::Settled);
    let note = listed.map_or(child.note.clone(), |l| l.note.clone());
    let mark = if child.subagent {
        Icon::new(Glyph::Bot).with_size(px(13.)).text_color(p.muted_foreground).into_any_element()
    } else {
        match listed.map(|l| l.harness.as_str()).filter(|h| !h.is_empty()) {
            Some(harness) => catalog::mark(harness, &catalog::name(harness), px(13.), p.muted_foreground, cx),
            None => div().into_any_element(),
        }
    };
    let press: Option<Route> = match (listed, &child.call, lead) {
        (Some(listed), ..) => Some(Route::Thread { workspace_id: listed.at.workspace.clone(), thread_id: listed.at.thread.clone() }),
        (None, Some(call), Some(lead)) if child.subagent => Some(Route::Screen(subagent_route(lead, call).into())),
        _ => None,
    };
    child_row(
        SharedString::from(format!("spawn-{key}-{ix}")),
        "spawn-row",
        mark,
        &child.title,
        settled,
        listed.and_then(|l| l.computer.clone()),
        note,
        status,
        press,
        p,
    )
}

/// The row's box every child takes, in the Threads section and at the calls that started it.
#[allow(clippy::too_many_arguments)]
fn child_row(
    id: SharedString,
    selector: &'static str,
    mark: AnyElement,
    title: &str,
    settled: bool,
    computer: Option<String>,
    note: Option<String>,
    status: Option<Status>,
    press: Option<Route>,
    p: &Palette,
) -> AnyElement {
    let two = note.is_some() || computer.is_some();
    let second = two.then(|| {
        h_flex()
            .min_w_0()
            .gap(px(12.))
            .text_size(px(11.))
            .line_height(px(14.))
            .text_color(p.muted_foreground)
            .children(computer.map(|c| div().flex_none().child(c)))
            .children(note.map(|note| div().min_w_0().truncate().child(note)))
    });
    h_flex()
        .id(id)
        .debug_selector(move || selector.into())
        .min_w_0()
        .gap(px(10.))
        .px(px(8.))
        .rounded(px(8.))
        .text_size(SM_TIGHT.0)
        .line_height(SM_TIGHT.1)
        .hover(|s| s.bg(p.accent))
        .map(|el| if two { el.min_h(px(48.)).py(px(6.)) } else { el.h(px(36.)) })
        .child(div().flex().flex_none().items_center().justify_center().size(px(13.)).child(mark))
        .child(
            v_flex()
                .flex_1()
                .min_w(px(112.))
                .child(
                    div().min_w_0().truncate().text_color(if settled { p.muted_foreground } else { p.foreground }).child(title.to_owned()),
                )
                .children(second),
        )
        .children(status.map(StatusMark::new))
        .when_some(press, |el, route| el.cursor_pointer().on_click(move |_, _, cx| go(route.clone(), cx)))
        .into_any_element()
}

/// `TimelineRuleLine`: one quiet line about the workspace, 24 px, 11 px muted, cut short and whole on hover, then any
/// act after it. The fork rule, the machine line and the failed start's words are this piece.
fn rule_line(id: &'static str, line: &str, act: Option<AnyElement>, ink: Hsla) -> AnyElement {
    h_flex()
        .debug_selector(move || id.into())
        .h(px(24.))
        .min_w_0()
        .gap(px(8.))
        .px(px(4.))
        .text_size(px(11.))
        .text_color(ink)
        .child(div().id(id).min_w_0().truncate().tooltip(tip(line.to_owned())).child(line.to_owned()))
        .children(act)
        .into_any_element()
}

#[cfg(test)]
thread_local! {
    /// Every pane and tab the cards asked the frame to open in this test thread, and the slate line's last ink.
    pub static PANES: std::cell::RefCell<Vec<(Pane, Option<String>)>> = const { std::cell::RefCell::new(Vec::new()) };
    pub static SLATE_INK: std::cell::Cell<Option<Hsla>> = const { std::cell::Cell::new(None) };
}

fn open_pane(pane: Pane, tab: Option<&str>, cx: &mut App) {
    #[cfg(test)]
    PANES.with(|p| p.borrow_mut().push((pane, tab.map(str::to_owned))));
    super::open_pane(pane, tab, cx);
}

/// `SlateUpdatedLine`: the rule line as a button, `mt-1` under the reply, its ink easing to the foreground under the
/// pointer over 150 ms; a press opens the Slate tab.
fn slate_line(key: &str, line: &str, p: &Palette, window: &mut Window, cx: &mut Context<ThreadView>) -> AnyElement {
    let id = SharedString::from(format!("slate-updated-{key}"));
    let hovered = crate::ui::motion::hovered(id.clone(), window, cx);
    let target = if *hovered.read(cx) { p.foreground } else { p.muted_foreground };
    let ink = crate::ui::motion::ease_ink(id.clone(), "ink", target, crate::ui::motion::COLORS, window, cx);
    #[cfg(test)]
    SLATE_INK.with(|i| i.set(Some(ink)));
    div()
        .id(id)
        .debug_selector(|| "slate-updated".into())
        .mt(px(4.))
        .rounded(px(8.))
        .cursor_pointer()
        .on_hover(crate::ui::motion::track_hover(hovered))
        .on_click(|_, _, cx| crate::screens::slate::open_tab(cx))
        .child(rule_line("slate-updated-line", line, None, ink))
        .into_any_element()
}

/// `ForkRuleRow`: "Forked from {title}", and Open while the thread it came from is listed.
fn fork(line: &str, open: Option<ThreadAt>, p: &Palette, _cx: &App) -> AnyElement {
    let act = open.map(|at| {
        let open = Button::outline("fork-open", "Open").xs().on_click(move |_, _, cx| open_thread(&at, cx));
        div().debug_selector(|| "fork-open".into()).flex_none().child(open).into_any_element()
    });
    rule_line("fork", line, act, p.muted_foreground)
}

/// `SpawnLine` for a start that failed: the words in the refusal ink with the chevron, open on the failed step with
/// what the host said and Try again, then the raw log one level further down.
fn start_failed(
    view: &ThreadView,
    key: &str,
    failed: &StartFailed,
    p: &Palette,
    window: &mut Window,
    cx: &mut Context<ThreadView>,
) -> AnyElement {
    let open_state = window.use_keyed_state(SharedString::from(format!("spawn-open-{key}")), cx, |_, _| true);
    let raw_state = window.use_keyed_state(SharedString::from(format!("spawn-raw-{key}")), cx, |_, _| false);
    let (open, raw) = (*open_state.read(cx), *raw_state.read(cx));
    let flip = |state: gpui_kit::Entity<bool>| {
        move |_: &gpui_kit::ClickEvent, _: &mut Window, cx: &mut App| {
            state.update(cx, |b, cx| {
                *b = !*b;
                cx.notify();
            })
        }
    };
    let ink = p.error_foreground;
    let mark = |g: Glyph, ink: Hsla| Icon::new(g).with_size(px(12.)).text_color(ink);
    let head = h_flex()
        .id(SharedString::from(format!("spawn-line-{key}")))
        .debug_selector(|| "spawn-line".into())
        .gap(px(6.))
        .text_color(ink)
        .cursor_pointer()
        .child(mark(Glyph::CircleAlert, ink))
        .child(format!("Could not start a machine on {}", failed.on))
        .child(mark(if open { Glyph::ChevronDown } else { Glyph::ChevronRight }, ink))
        .on_click(flip(open_state.clone()));
    let _ = view;
    let prompt = failed.prompt.clone();
    let steps = open.then(|| {
        let said = h_flex()
            .flex_wrap()
            .text_color(ink)
            .child(failed.said.clone())
            .children(failed.fix.clone().map(|fix| div().text_color(p.foreground).child(format!(" {fix}"))));
        let again = prompt.map(|prompt| {
            let again = Button::outline("spawn-try-again", "Try again").xs().on_click(cx.listener(move |view, _, _, cx| {
                view.send(prompt.clone(), cx);
                if let Some(thread) = view.thread().map(str::to_owned) {
                    cx.emit(crate::thread_view::ThreadViewEvent::SentElsewhere { thread, prompt: prompt.clone() });
                }
            }));
            div().debug_selector(|| "spawn-try-again".into()).child(again)
        });
        v_flex()
            .debug_selector(|| "spawn-steps".into())
            .mt(px(8.))
            .w_full()
            .max_w(px(576.))
            .gap(px(4.))
            .pl(px(24.))
            .child(
                v_flex()
                    .gap(px(4.))
                    .child(
                        h_flex()
                            .min_w_0()
                            .gap(px(8.))
                            .child(mark(Glyph::CircleAlert, ink))
                            .child(div().text_color(ink).child(failed.step.clone())),
                    )
                    .child(v_flex().items_start().gap(px(8.)).pl(px(20.)).child(said).children(again)),
            )
            .child(
                h_flex()
                    .id(SharedString::from(format!("spawn-details-{key}")))
                    .mt(px(4.))
                    .gap(px(6.))
                    .cursor_pointer()
                    .hover(|s| s.text_color(p.foreground))
                    .child(mark(if raw { Glyph::ChevronDown } else { Glyph::ChevronRight }, p.muted_foreground))
                    .child(if raw { "Hide details" } else { "Show details" })
                    .on_click(flip(raw_state.clone())),
            )
            .when(raw, |el| el.child(div().pl(px(20.)).font_family(cx.theme().mono_font_family.clone()).child(failed.log.join("\n"))))
    });
    v_flex()
        .debug_selector(|| "start-failed".into())
        .items_center()
        .text_size(META.0)
        .line_height(META.1)
        .text_color(p.muted_foreground)
        .child(head)
        .children(steps)
        .into_any_element()
}

/// How many rows an open fold of the Threads section draws at a time (`PAGE` in TreeRows.tsx).
const TREE_PAGE: usize = 20;
/// `TRANSCRIPT_LIST_CLASS`'s `ml-3.5`: a child list's line under the centre of the 13 px mark 8 px in.
const TREE_LIST: Pixels = px(14.);
/// `ELBOW`'s `rounded-bl-sm`.
const TREE_ELBOW: Pixels = px(6.);
/// How long Stop stands armed after its first press (`ARM_MS`).
const STOP_ARMED: std::time::Duration = std::time::Duration::from_millis(2_000);

#[cfg(test)]
thread_local! {
    /// Every act the Threads section and the strip handed the sidebar's runner or the thread in this test thread.
    pub static RAN: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Runs an act on the window's sidebar, whose runner says a refusal over its list.
fn on_sidebar(said: String, cx: &mut App, act: impl FnOnce(&mut crate::sidebar::Sidebar, &mut Context<crate::sidebar::Sidebar>)) {
    #[cfg(test)]
    RAN.with(|r| r.borrow_mut().push(said.clone()));
    let _ = said;
    if let Some(sidebar) = crate::screens::terminal::workspaces_sidebar(cx) {
        sidebar.update(cx, act);
    }
}

fn stop_child(session: &str, cx: &mut App) {
    let params = serde_json::json!({ "sessionId": session });
    on_sidebar(format!("stop {session}"), cx, |s, cx| s.write_op::<wsp_host_client::protocol::SessionsInterruptRequest>(params, cx));
}

fn settle_children(ids: Vec<String>, cx: &mut App) {
    on_sidebar(format!("settle {}", ids.join(" ")), cx, |s, cx| s.settle_ids(ids, crate::sidebar::Processes::Ask, cx));
}

fn restore_child(id: String, cx: &mut App) {
    on_sidebar(format!("restore {id}"), cx, |s, cx| s.restore_ids(vec![id], cx));
}

/// Shuts or opens the section under `lead`: at once on the view, and in the preferences through the sidebar, whose
/// next read agrees.
fn shut_threads(view: &gpui_kit::WeakEntity<ThreadView>, lead: &str, shut: bool, cx: &mut App) {
    let lead = lead.to_owned();
    _ = view.update(cx, |v, cx| {
        if let Some(listing) = v.listing().filter(|l| l.thread == lead).cloned() {
            v.set_listing(transcript::Listing { shut, ..listing }, cx);
        }
    });
    on_sidebar(format!("shut {lead} {shut}"), cx, |s, cx| s.shut_threads(&lead, shut, cx));
}

/// One count of the shut head as the status slot draws its kind: the glyph, then the figure, in the kind's ink and
/// weight, its words on the hover (`Glyphed` in TreeCounts.tsx).
fn tree_count(
    id: &'static str,
    glyph: Option<Glyph>,
    n: usize,
    words: String,
    ink: Hsla,
    toned: bool,
) -> gpui_kit::Stateful<gpui_kit::Div> {
    h_flex()
        .id(id)
        .debug_selector(move || format!("tree-count-{id}"))
        .flex_none()
        .gap(px(4.))
        .text_color(ink)
        .when(toned, |el| el.font_weight(FontWeight::MEDIUM))
        .tooltip(tip(words))
        .children(glyph.map(|g| Icon::new(g).with_size(px(12.)).text_color(ink)))
        .child(n.to_string())
}

/// `TreeCounts`: each live status's figure, working's crab after its number, and with nothing live how many finished.
fn tree_counts(counts: transcript::TreeCounts, p: &Palette) -> AnyElement {
    let many = |n: usize, one: &str| format!("{n} {one}{}", if n == 1 { "" } else { "s" });
    let live = counts.needs_you + counts.failed + counts.working + counts.waiting > 0;
    let threads = counts.working - counts.working_subagents;
    let working = [
        (threads > 0).then(|| many(threads, "thread")),
        (counts.working_subagents > 0).then(|| many(counts.working_subagents, "subagent")),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" and ");
    h_flex()
        .debug_selector(|| "tree-counts".into())
        .flex_none()
        .gap(px(12.))
        .when(counts.needs_you > 0, |el| {
            el.child(tree_count(
                "needs-you",
                Some(Glyph::MessageCircleQuestionMark),
                counts.needs_you,
                format!("{} needs you", counts.needs_you),
                p.status_input,
                true,
            ))
        })
        .when(counts.failed > 0, |el| {
            el.child(tree_count(
                "failed",
                Some(Glyph::CircleAlert),
                counts.failed,
                format!("{} failed", counts.failed),
                p.status_failed,
                true,
            ))
        })
        .when(counts.working > 0, |el| {
            el.child(
                tree_count("working", None, counts.working, format!("{working} working"), p.status_working, true)
                    .child(crate::ui::Crab::new(p.status_working).key("tree-count-crab")),
            )
        })
        .when(counts.waiting > 0, |el| {
            el.child(tree_count(
                "waiting",
                Some(Glyph::Hourglass),
                counts.waiting,
                format!("{} waiting", counts.waiting),
                p.muted_foreground,
                false,
            ))
        })
        .when(!live && counts.finished > 0, |el| {
            el.child(
                div()
                    .debug_selector(|| "tree-count-finished".into())
                    .text_color(p.muted_foreground)
                    .child(format!("{} finished", counts.finished)),
            )
        })
        .into_any_element()
}

/// The section's head (`Head` in TreeRows.tsx): the tree glyph and the word as the toggle, the tree's counts while shut,
/// Settle N finished while open, and the chevron.
fn tree_head(listing: &transcript::Listing, open: bool, p: &Palette, cx: &mut Context<ThreadView>) -> AnyElement {
    let me = cx.weak_entity();
    let lead = listing.thread.clone();
    let toggle = {
        let (me, lead) = (me.clone(), lead.clone());
        move |_: &gpui_kit::ClickEvent, _: &mut Window, cx: &mut App| shut_threads(&me, &lead, open, cx)
    };
    let (finished, threads) = listing.finished();
    let settle = (open && !finished.is_empty()).then(|| {
        div().debug_selector(|| "threads-settle".into()).flex_none().child(
            Button::outline("threads-settle", format!("Settle {threads} finished"))
                .xs()
                .glyph(Glyph::Archive)
                .on_click(move |_, _, cx| settle_children(finished.clone(), cx)),
        )
    });
    let fg = p.foreground;
    h_flex()
        .debug_selector(|| "threads-head".into())
        .h(px(32.))
        .w_full()
        .min_w_0()
        .gap(px(10.))
        .px(px(8.))
        .text_size(XS.0)
        .line_height(XS.1)
        .text_color(p.muted_foreground)
        .child(
            h_flex()
                .id("threads-toggle")
                .debug_selector(|| "threads-toggle".into())
                .flex_1()
                .min_w_0()
                .h_full()
                .gap(px(10.))
                .rounded(px(8.))
                .cursor_pointer()
                .hover(move |s| s.text_color(fg))
                .child(
                    div()
                        .flex()
                        .flex_none()
                        .items_center()
                        .justify_center()
                        .size(px(13.))
                        .child(Icon::new(Glyph::ListTree).with_size(px(12.))),
                )
                .child(div().flex_1().min_w_0().truncate().child("Threads"))
                .when(!open, |el| el.child(tree_counts(listing.counts(), p)))
                .on_click(toggle.clone()),
        )
        .children(settle)
        .child(
            div()
                .id("threads-chevron")
                .debug_selector(|| "threads-chevron".into())
                .flex()
                .flex_none()
                .items_center()
                .cursor_pointer()
                .hover(move |s| s.text_color(fg))
                .child(Icon::new(if open { Glyph::ChevronDown } else { Glyph::ChevronRight }).with_size(px(14.)))
                .on_click(toggle),
        )
        .into_any_element()
}

/// A fold among the children in a row's box (`FoldRow`): the part's glyph, the word, then the count and the chevron in
/// the status slot's grammar; "N more" draws neither.
fn tree_fold(
    id: SharedString,
    glyph: Glyph,
    word: String,
    count: Option<(usize, bool)>,
    p: &Palette,
    flip: impl Fn(&mut App) + 'static,
) -> AnyElement {
    let selector = id.clone();
    let (accent, fg) = (p.accent, p.foreground);
    h_flex()
        .id(id)
        .debug_selector(move || selector.to_string())
        .h(px(36.))
        .w_full()
        .min_w_0()
        .gap(px(10.))
        .px(px(8.))
        .rounded(px(8.))
        .text_size(SM_TIGHT.0)
        .line_height(SM_TIGHT.1)
        .text_color(p.muted_foreground)
        .cursor_pointer()
        .hover(move |s| s.bg(accent).text_color(fg))
        .child(div().flex().flex_none().items_center().justify_center().size(px(13.)).child(Icon::new(glyph).with_size(px(12.))))
        .child(div().flex_1().min_w_0().truncate().child(word))
        .children(count.map(|(n, open)| {
            h_flex()
                .flex_none()
                .min_w(px(16.))
                .justify_end()
                .gap(px(4.))
                .text_size(XS.0)
                .line_height(XS.1)
                .text_color(p.muted_foreground)
                .child(n.to_string())
                .child(Icon::new(if open { Glyph::ChevronDown } else { Glyph::ChevronRight }).with_size(px(14.)))
        }))
        .on_click(move |_, _, cx| flip(cx))
        .into_any_element()
}

/// A drawn entry of a level, and whether it is a row with a second line, where a rail's elbow meets it lower.
struct TreeEntry {
    el: AnyElement,
    two: bool,
}

/// A list of entries on the rail (`TRANSCRIPT_LIST_CLASS` and `TRANSCRIPT_ITEM_CLASS`): 14 px in, each item 8 px past
/// its line, the elbow from its top into its row's middle, the line carried on to the next item's top.
fn tree_rail(entries: Vec<TreeEntry>, p: &Palette) -> AnyElement {
    let n = entries.len();
    let rail = p.sidebar_rail;
    v_flex()
        .debug_selector(|| "tree-list".into())
        .ml(TREE_LIST)
        .min_w_0()
        .children(entries.into_iter().enumerate().map(|(k, e)| {
            let mark = if e.two { 24. } else { 18. };
            div()
                .relative()
                .min_w_0()
                .pl(px(8.))
                .when(k + 1 < n, |el| el.child(div().absolute().left_0().top(px(mark - 5.)).bottom_0().w(px(1.)).bg(rail)))
                .child(
                    div()
                        .absolute()
                        .left_0()
                        .top_0()
                        .w(px(8.))
                        .h(px(mark + 1.))
                        .border_l_1()
                        .border_b_1()
                        .border_color(rail)
                        .rounded_bl(TREE_ELBOW),
                )
                .child(e.el)
        }))
        .into_any_element()
}

/// The Threads section under the transcript (`TreeRows`): its head, then open, the tree's levels; shut, the most
/// pressing live child at any depth fading out under it, a peek that opens the section on a press.
fn threads(
    key: &str,
    open: bool,
    listing: Option<&transcript::Listing>,
    p: &Palette,
    window: &mut Window,
    cx: &mut Context<ThreadView>,
) -> AnyElement {
    let Some(listing) = listing else { return div().into_any_element() };
    let head = tree_head(listing, open, p, cx);
    let body: Vec<AnyElement> = if open {
        tree_level(&listing.children, 1, true, &format!("{key}-{}", listing.thread), p, window, cx).into_iter().map(|e| e.el).collect()
    } else {
        let me = cx.weak_entity();
        let lead = listing.thread.clone();
        listing
            .most_pressing()
            .map(|node| {
                let row = tree_row(node, true, p, window, cx);
                let ground = p.background;
                div()
                    .id("threads-peek")
                    .debug_selector(|| "threads-peek".into())
                    .relative()
                    .cursor_pointer()
                    .on_click(move |_, _, cx| shut_threads(&me, &lead, false, cx))
                    .child(row)
                    // `mask-b-from-0%`, drawn as the transcript's ground over the row, since this base has no masks.
                    .child(div().absolute().inset_0().bg(linear_gradient(
                        180.,
                        linear_color_stop(ground.opacity(0.), 0.),
                        linear_color_stop(ground, 1.),
                    )))
                    .into_any_element()
            })
            .into_iter()
            .collect()
    };
    v_flex().debug_selector(|| "threads".into()).mt(px(8.)).min_w_0().child(head).children(body).into_any_element()
}

/// One level of the tree (`Level`): its live children, then its Finished fold; the top level adds Settled. From the
/// second level on a child's own children stand at its x, in the same list.
fn tree_level(
    nodes: &[Listed],
    depth: usize,
    top: bool,
    scope: &str,
    p: &Palette,
    window: &mut Window,
    cx: &mut Context<ThreadView>,
) -> Vec<TreeEntry> {
    let mut out = Vec::new();
    for node in nodes.iter().filter(|n| n.part == ChildPart::Live) {
        out.extend(tree_item(node, depth, p, window, cx));
    }
    let folds = [(ChildPart::Finished, "Finished", Glyph::CircleCheck, true), (ChildPart::Settled, "Settled", Glyph::Archive, top)];
    for (part, word, glyph, stands) in folds {
        let held: Vec<&Listed> = nodes.iter().filter(|n| n.part == part).collect();
        if !stands || held.is_empty() {
            continue;
        }
        let id = format!("{scope}-{word}");
        let shown = window.use_keyed_state(SharedString::from(format!("tree-open-{id}")), cx, |_, _| false);
        let page = window.use_keyed_state(SharedString::from(format!("tree-page-{id}")), cx, |_, _| TREE_PAGE);
        let is_open = *shown.read(cx);
        let flip = move |cx: &mut App| {
            shown.update(cx, |o, cx| {
                *o = !*o;
                cx.notify();
            })
        };
        out.push(TreeEntry {
            el: tree_fold(SharedString::from(format!("threads-fold-{word}")), glyph, word.to_owned(), Some((held.len(), is_open)), p, flip),
            two: false,
        });
        if !is_open {
            continue;
        }
        let shown_n = *page.read(cx);
        for node in held.iter().take(shown_n) {
            out.extend(tree_item(node, depth, p, window, cx));
        }
        if held.len() > shown_n {
            let rest = held.len() - shown_n;
            let more = move |cx: &mut App| {
                page.update(cx, |n, cx| {
                    *n += TREE_PAGE;
                    cx.notify();
                })
            };
            out.push(TreeEntry {
                el: tree_fold(
                    SharedString::from(format!("threads-more-{word}")),
                    Glyph::ChevronsDown,
                    format!("{rest} more"),
                    None,
                    p,
                    more,
                ),
                two: false,
            });
        }
    }
    out
}

/// One child's row with its own children under it (`Item`): at the top level they hang on a rail under it, deeper
/// they follow it in the list it stands in.
fn tree_item(node: &Listed, depth: usize, p: &Palette, window: &mut Window, cx: &mut Context<ThreadView>) -> Vec<TreeEntry> {
    let row = tree_row(node, false, p, window, cx);
    let two = node.note.is_some() || node.computer.is_some();
    let scope = format!("{}-{}", node.at.thread, node.subagent.as_deref().unwrap_or(""));
    let kids =
        if node.part == ChildPart::Settled { Vec::new() } else { tree_level(&node.children, depth + 1, false, &scope, p, window, cx) };
    if depth == 1 {
        let list = (!kids.is_empty()).then(|| tree_rail(kids, p));
        return vec![TreeEntry { el: v_flex().min_w_0().child(row).children(list).into_any_element(), two }];
    }
    let mut out = vec![TreeEntry { el: row, two }];
    out.extend(kids);
    out
}

/// A child's row (`ThreadRow`, `SubagentRow`): the mark, the title over the computer and the note, then the status slot,
/// which yields on hover to its state act and More; a press opens the thread, or a subagent's page. `inert` draws it
/// for the shut section's peek, which takes no act of its own.
fn tree_row(node: &Listed, inert: bool, p: &Palette, window: &mut Window, cx: &mut Context<ThreadView>) -> AnyElement {
    let id = match &node.subagent {
        Some(call) => format!("tree-sub-{}-{call}", node.at.thread),
        None => format!("tree-row-{}", node.at.thread),
    };
    let group = SharedString::from(format!("{id}-group"));
    let settled = node.part == ChildPart::Settled;
    let two = node.note.is_some() || node.computer.is_some();
    let mark = match &node.subagent {
        Some(_) => Icon::new(Glyph::Bot).with_size(px(13.)).text_color(p.muted_foreground).into_any_element(),
        None => catalog::mark(&node.harness, &catalog::name(&node.harness), px(13.), p.muted_foreground, cx),
    };
    let second = two.then(|| {
        h_flex()
            .min_w_0()
            .gap(px(12.))
            .text_size(px(11.))
            .line_height(px(14.))
            .text_color(p.muted_foreground)
            .children(node.computer.clone().map(|c| div().flex_none().child(c)))
            .children(node.note.clone().map(|note| div().min_w_0().truncate().child(note)))
    });
    let status = StatusMark::new(listed_status(node)).key(SharedString::from(format!("{id}-crab")));
    let acts = if inert { Vec::new() } else { tree_acts(node, &id, p, window, cx) };
    let menu = if inert { Vec::new() } else { child_menu(node) };
    let slot = if acts.is_empty() {
        div().flex_none().child(status).into_any_element()
    } else {
        // `ACTS_ROOM`: as wide as the acts it holds with 4 px between, so the title's width never moves as they show.
        let room = [0., 24., 52., 80.][acts.len().min(3)];
        h_flex()
            .relative()
            .flex_none()
            .justify_end()
            .min_w(px(room))
            .child(div().flex().group_hover(group.clone(), |s| s.invisible()).child(status))
            .child(
                h_flex()
                    .debug_selector(|| "child-acts".into())
                    // A press on an act is not a press on the row.
                    .on_mouse_down(gpui_kit::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_mouse_up(gpui_kit::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .right_0()
                    .gap(px(4.))
                    .opacity(0.)
                    .group_hover(group.clone(), |s| s.opacity(1.))
                    .children(acts),
            )
            .into_any_element()
    };
    let press: Option<Route> = match &node.subagent {
        _ if inert => None,
        None => Some(Route::Thread { workspace_id: node.at.workspace.clone(), thread_id: node.at.thread.clone() }),
        Some(call) => Some(Route::Screen(subagent_route(&node.at, call).into())),
    };
    let accent = p.accent;
    let selector = id.clone();
    let armed = window.use_keyed_state(SharedString::from(format!("{id}-armed")), cx, |_, _| None::<std::time::Instant>);
    h_flex()
        .id(SharedString::from(id))
        .group(group)
        .debug_selector(move || selector.clone())
        .min_w_0()
        .gap(px(10.))
        .px(px(8.))
        .rounded(px(8.))
        .text_size(SM_TIGHT.0)
        .line_height(SM_TIGHT.1)
        .when(!inert, |el| el.hover(move |s| s.bg(accent)))
        .map(|el| if two { el.min_h(px(48.)).py(px(6.)) } else { el.h(px(36.)) })
        .child(div().flex().flex_none().items_center().justify_center().size(px(13.)).child(mark))
        .child(
            v_flex()
                .flex_1()
                .min_w(px(112.))
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .text_color(if settled { p.muted_foreground } else { p.foreground })
                        .child(node.title.clone()),
                )
                .children(second),
        )
        .child(slot)
        // The pointer leaving the row disarms its Stop.
        .on_hover(move |over, _, cx| {
            if !*over {
                armed.update(cx, |a, cx| {
                    if a.take().is_some() {
                        cx.notify();
                    }
                });
            }
        })
        .when_some(press, |el, route| el.cursor_pointer().on_click(move |_, _, cx| go(route.clone(), cx)))
        .when(!menu.is_empty(), |el| {
            let (view, menu) = (cx.weak_entity(), Rc::new(menu));
            el.on_mouse_down(gpui_kit::MouseButton::Right, move |e, window, cx| {
                open_child_menu(&view, menu.clone(), e.position, window, cx)
            })
        })
        .into_any_element()
}

/// The acts a thread's row yields its slot to (`Slot` in ThreadRows.tsx): the state act that can run now, Stop in two
/// presses, Settle or Restore, then More with every act the row takes; a subagent's row takes none here.
fn tree_acts(node: &Listed, id: &str, p: &Palette, window: &mut Window, cx: &mut Context<ThreadView>) -> Vec<AnyElement> {
    use crate::screens::sidebar::menu::{act_words as words, settle_refusal};
    if node.subagent.is_some() {
        return Vec::new();
    }
    let settled = node.part == ChildPart::Settled;
    let stops = !settled && (node.running || node.working);
    let settles = !settled && !node.running;
    let held = settle_refusal(node.working);
    let armed = window.use_keyed_state(SharedString::from(format!("{id}-armed")), cx, |_, _| None::<std::time::Instant>);
    let is_armed = armed.read(cx).is_some_and(|at| at.elapsed() < STOP_ARMED);
    let mut out: Vec<AnyElement> = Vec::new();
    let (session, thread) = (node.session.clone(), node.at.thread.clone());
    if stops {
        let label = if node.under { words::STOP_TREE } else { words::STOP };
        let (session, armed) = (session.clone(), armed.clone());
        let press = move |_: &gpui_kit::ClickEvent, _: &mut Window, cx: &mut App| {
            cx.stop_propagation();
            let was = armed.read(cx).is_some_and(|at| at.elapsed() < STOP_ARMED);
            if was {
                stop_child(&session, cx);
            }
            let at = (!was).then(std::time::Instant::now);
            armed.update(cx, |a, cx| {
                *a = at;
                cx.notify();
            });
            if let Some(at) = at {
                let armed = armed.downgrade();
                cx.spawn(async move |cx| {
                    cx.background_executor().timer(STOP_ARMED).await;
                    // Only the arm this press made lapses; a later arm keeps its own two seconds.
                    _ = armed.update(cx, |a, cx| {
                        if *a == Some(at) {
                            *a = None;
                            cx.notify();
                        }
                    });
                })
                .detach();
            }
        };
        let act = if is_armed {
            h_flex()
                .id(SharedString::from(format!("{id}-stop")))
                .debug_selector({
                    let id = format!("{id}-act-stop");
                    move || id.clone()
                })
                .h(px(24.))
                .px(px(6.))
                .rounded(px(8.))
                .text_size(XS.0)
                .line_height(XS.1)
                .font_weight(FontWeight::MEDIUM)
                .text_color(p.error_foreground)
                .cursor_pointer()
                .tooltip(tip(words::STOP_AGAIN))
                .child(words::STOP_ARMED)
                .on_click(press)
                .into_any_element()
        } else {
            div()
                .debug_selector({
                    let id = format!("{id}-act-stop");
                    move || id.clone()
                })
                .child(Button::icon_xs(SharedString::from(format!("{id}-stop")), Glyph::Square).tooltip(label).on_click(press))
                .into_any_element()
        };
        out.push(act);
    } else if settles && held.is_none() {
        out.push(
            div()
                .debug_selector({
                    let id = format!("{id}-act-settle");
                    move || id.clone()
                })
                .child(Button::icon_xs(SharedString::from(format!("{id}-settle")), Glyph::Archive).tooltip(words::SETTLE).on_click(
                    move |_, _, cx| {
                        cx.stop_propagation();
                        settle_children(vec![thread.clone()], cx)
                    },
                ))
                .into_any_element(),
        );
    } else if settled {
        out.push(
            div()
                .debug_selector({
                    let id = format!("{id}-act-restore");
                    move || id.clone()
                })
                .child(
                    Button::icon_xs(SharedString::from(format!("{id}-restore")), Glyph::ArchiveRestore).tooltip(words::RESTORE).on_click(
                        move |_, _, cx| {
                            cx.stop_propagation();
                            restore_child(thread.clone(), cx)
                        },
                    ),
                )
                .into_any_element(),
        );
    }
    if is_armed {
        return out;
    }
    use gpui_kit::component::menu::DropdownMenu as _;
    let acts = Rc::new(child_menu(node));
    let trigger = crate::ui::menu::Trigger(
        div()
            .id(SharedString::from(format!("{id}-more-menu")))
            .debug_selector({
                let id = format!("{id}-act-more");
                move || id.clone()
            })
            .flex()
            .child(Button::icon_xs(SharedString::from(format!("{id}-more")), Glyph::Ellipsis).tooltip("More")),
    );
    out.push(
        trigger
            .dropdown_menu(move |menu, _, cx| {
                let p = cx.palette();
                crate::ui::menu_of(menu.surface(crate::ui::menu::surface(p)), &acts, p.error_foreground, p.muted_foreground)
            })
            .into_any_element(),
    );
    out
}

/// Every act a thread's row takes, in the order its More and its right-click list them (`childActs`); a subagent's
/// row takes none here.
fn child_menu(node: &Listed) -> Vec<crate::ui::MenuAct> {
    use crate::screens::sidebar::menu::{act_words as words, settle_refusal};
    if node.subagent.is_some() {
        return Vec::new();
    }
    let settled = node.part == ChildPart::Settled;
    let (stops, settles) = (!settled && (node.running || node.working), !settled && !node.running);
    let act = |group: u8, glyph: Glyph, title: &str, why: Option<String>, run: Rc<dyn Fn(&mut App)>| crate::ui::MenuAct {
        group,
        glyph: Some(glyph),
        title: SharedString::from(title.to_owned()),
        destructive: false,
        held: why.is_some(),
        why: why.map(SharedString::from),
        keys: None,
        run: Rc::new(move |_, cx| run(cx)),
    };
    let send_held = Some("This client cannot send a message".to_owned());
    let mut menu = vec![act(0, Glyph::MessageSquare, "Send a message", send_held, Rc::new(|_| {}))];
    let (session, thread) = (node.session.clone(), node.at.thread.clone());
    if stops {
        menu.push(act(1, Glyph::Square, words::STOP, None, Rc::new(move |cx| stop_child(&session, cx))));
    }
    if settles {
        let ids = vec![thread.clone()];
        menu.push(act(1, Glyph::Archive, words::SETTLE, settle_refusal(node.working), Rc::new(move |cx| settle_children(ids.clone(), cx))));
    }
    if settled {
        menu.push(act(1, Glyph::ArchiveRestore, words::RESTORE, None, Rc::new(move |cx| restore_child(thread.clone(), cx))));
    }
    menu
}

/// Opens a row's acts at the pointer, as its right-click does (`openContextMenu`).
fn open_child_menu(
    view: &gpui_kit::WeakEntity<ThreadView>,
    acts: Rc<Vec<crate::ui::MenuAct>>,
    at: gpui_kit::Point<Pixels>,
    window: &mut Window,
    cx: &mut App,
) {
    let items = acts
        .iter()
        .map(|a| crate::ui::context_menu::MenuItem {
            label: a.title.clone(),
            group: if a.group == 0 { "talk" } else { "state" },
            refusal: a.why.clone(),
            keys: None,
            destructive: a.destructive,
        })
        .collect();
    _ = view.update(cx, |v, cx| {
        v.row_menu.open(at, items, window, cx, move |_, pick, window, cx| {
            if let Some(act) = pick.and_then(|ix| acts.get(ix)) {
                (act.run)(window, cx);
            }
        })
    });
}

/// `TRANSCRIPT_LOADING`: the transcript on its way, centred over the composer, as a thread not read yet and a fork its
/// agent is still making both stand (ChatView's `!hydrated || (empty && forkWaits)`).
pub fn loading(composer_inset: Pixels, cx: &App) -> impl IntoElement {
    div()
        .debug_selector(|| "transcript-loading".into())
        .absolute()
        .inset_0()
        .flex()
        .items_center()
        .justify_center()
        .pb(composer_inset)
        .text_size(SM_TIGHT.0)
        .line_height(SM_TIGHT.1)
        .text_color(cx.palette().muted_foreground)
        .child("Loading transcript")
}

/// A create this window asked for with no message, landed: `SetupCard` standing as the thread's first content until
/// its first message, every step done, centred in the room over the composer (`SetupRoom`).
#[derive(Clone, Debug, PartialEq)]
pub struct Setup {
    pub name: String,
    /// Each step's words, as the create's log named it (`creationSteps`).
    pub steps: Vec<String>,
}

pub fn setup_card(setup: &Setup, composer_inset: Pixels) -> impl IntoElement {
    let steps = if setup.steps.is_empty() { vec!["Asking wsp to start it".to_owned()] } else { setup.steps.clone() };
    let card = crate::ui::Card::new("setting-up").head(format!("{} is set up", setup.name)).items(
        steps
            .into_iter()
            .enumerate()
            .map(|(i, name)| crate::ui::StepRow::new(SharedString::from(format!("step-{i}")), crate::ui::Step::Done, name)),
    );
    v_flex()
        .debug_selector(|| "setting-up-room".into())
        .absolute()
        .top_0()
        .left_0()
        .right_0()
        .bottom(composer_inset)
        .gap(px(16.))
        .px(px(20.))
        .pt(px(16.))
        .child(
            div().flex().flex_1().items_center().justify_center().pb(px(24.)).child(div().w_full().max_w(px(768.)).min_w_0().child(card)),
        )
}

/// `SubStrip`: the one row over a sub-thread's composer while its machine holds or blocks it, in the composer's own
/// box: the state's glyph in its frame, the state as the title with the why under it, and its acts.
pub fn strip(view: &gpui_kit::Entity<ThreadView>, strip: &transcript::Strip, cx: &App) -> AnyElement {
    let p = cx.palette();
    let glyph = match strip.key {
        "waiting-minute" | "waiting-machines" => Glyph::Hourglass,
        "oom" => Glyph::MemoryStick,
        "released" => Glyph::CloudOff,
        "not-pushed" => Glyph::Upload,
        "forgotten" => Glyph::Unplug,
        _ => Glyph::Timer,
    };
    let words = h_flex().min_w_0().gap(px(12.)).child(crate::ui::glyph_frame(glyph, cx)).child(
        v_flex()
            .min_w_0()
            .gap(px(2.))
            .child(
                div()
                    .debug_selector(|| "sub-strip-title".into())
                    .text_size(px(15.))
                    .line_height(px(24.))
                    .text_color(p.foreground)
                    .child(strip.title.clone()),
            )
            .child(
                div().text_size(px(13.)).line_height(px(20.)).text_color(p.muted_foreground).whitespace_normal().child(strip.note.clone()),
            ),
    );
    let acts: Vec<AnyElement> = strip
        .acts
        .iter()
        .enumerate()
        .map(|(i, act)| {
            let id = format!("sub-strip-act-{i}");
            let button = Button::outline(SharedString::from(id.clone()), act.word()).xs();
            let button = match act {
                // The cloud's settings and the copy home have no screen in this app: held, with why on the hover.
                transcript::StripAct::Raise => button.held(true).tooltip("This client cannot open settings"),
                transcript::StripAct::CopyHome(_) => button.held(true).tooltip("This client cannot export projects"),
                transcript::StripAct::PushAgain(branch) => {
                    let (view, prompt) = (view.downgrade(), transcript::push_again_prompt(branch));
                    button.on_click(move |_, _, cx| {
                        #[cfg(test)]
                        RAN.with(|r| r.borrow_mut().push(format!("send {prompt}")));
                        _ = view.update(cx, |v, cx| v.send(prompt.clone(), cx));
                    })
                }
            };
            div().debug_selector(move || id.clone()).child(button).into_any_element()
        })
        .collect();
    let two = acts.len() > 1;
    let row = match acts.len() {
        0 => h_flex().min_h(px(60.)).px(px(20.)).py(px(12.)).child(words),
        _ if two => v_flex()
            .min_h(px(60.))
            .px(px(20.))
            .py(px(12.))
            .gap(px(12.))
            .child(words)
            .child(h_flex().flex_wrap().gap(px(8.)).pl(px(44.)).children(acts)),
        _ => h_flex()
            .min_h(px(60.))
            .px(px(20.))
            .py(px(12.))
            .justify_between()
            .gap(px(20.))
            .child(words)
            .child(h_flex().flex_none().children(acts)),
    };
    div()
        .debug_selector(move || format!("sub-strip-{}", strip.key))
        .w_full()
        .px(px(20.))
        .child(div().mx_auto().w_full().max_w(px(768.)).min_w_0().child(crate::screens::composer::parts::glass_box(cx).child(row)))
        .into_any_element()
}

/// One node of the changed files' tree (`buildTurnDiffTree`): a folder, its lone-folder chain joined into its name.
#[derive(Debug, PartialEq)]
pub enum Node {
    Folder { name: String, path: String, added: u64, removed: u64, children: Vec<Node> },
    File { name: String, path: String, added: u64, removed: u64 },
}

#[derive(Default)]
struct Building {
    folders: BTreeMap<String, Building>,
    files: Vec<(String, String, u64, u64)>,
    added: u64,
    removed: u64,
}

/// `localeCompare` with `numeric: true, sensitivity: "base"`, near enough for paths: case apart, digit runs by value.
fn by_name(a: &str, b: &str) -> std::cmp::Ordering {
    let key = |s: &str| {
        let mut parts: Vec<(u8, String, u64)> = Vec::new();
        let mut chars = s.chars().peekable();
        while let Some(c) = chars.next() {
            if c.is_ascii_digit() {
                let mut n = c.to_digit(10).unwrap_or(0) as u64;
                while let Some(d) = chars.peek().and_then(|d| d.to_digit(10)) {
                    n = n.saturating_mul(10).saturating_add(d as u64);
                    chars.next();
                }
                parts.push((0, String::new(), n));
            } else {
                parts.push((1, c.to_lowercase().collect(), 0));
            }
        }
        parts
    };
    key(a).cmp(&key(b))
}

pub fn tree(files: &[ChangedFile]) -> Vec<Node> {
    let mut root = Building::default();
    for file in files {
        let segments: Vec<&str> = file.path.split(['/', '\\']).filter(|s| !s.is_empty()).collect();
        let Some((name, folders)) = segments.split_last() else { continue };
        let mut at = &mut root;
        at.added += file.added;
        at.removed += file.removed;
        for folder in folders {
            at = at.folders.entry((*folder).to_owned()).or_default();
            at.added += file.added;
            at.removed += file.removed;
        }
        at.files.push(((*name).to_owned(), segments.join("/"), file.added, file.removed));
    }
    nodes(root, "")
}

fn nodes(building: Building, path: &str) -> Vec<Node> {
    let mut folders: Vec<(String, Building)> = building.folders.into_iter().collect();
    folders.sort_by(|a, b| by_name(&a.0, &b.0));
    let mut out: Vec<Node> = folders
        .into_iter()
        .map(|(name, sub)| {
            let here = if path.is_empty() { name.clone() } else { format!("{path}/{name}") };
            let (added, removed) = (sub.added, sub.removed);
            let mut node = Node::Folder { name, path: here.clone(), added, removed, children: nodes(sub, &here) };
            while let Node::Folder { name, children, .. } = &mut node {
                let [Node::Folder { .. }] = children.as_slice() else { break };
                let Some(Node::Folder { name: lone, path: deeper, added, removed, children: under }) = children.pop() else { break };
                node = Node::Folder { name: format!("{name}/{lone}"), path: deeper, added, removed, children: under };
            }
            node
        })
        .collect();
    let mut files = building.files;
    files.sort_by(|a, b| by_name(&a.0, &b.0));
    out.extend(files.into_iter().map(|(name, path, added, removed)| Node::File { name, path, added, removed }));
    out
}

/// `formatCompactDiffCount`: whole under a thousand, then `1.2k`, `12k`, `1.2m`.
pub fn compact(n: u64) -> String {
    let short = |v: f64, unit: &str| {
        if v < 10. {
            let s = format!("{v:.1}");
            format!("{}{unit}", s.strip_suffix(".0").unwrap_or(&s))
        } else {
            format!("{}{unit}", v.round() as u64)
        }
    };
    match n {
        n if n < 1_000 => n.to_string(),
        n if n < 1_000_000 => short(n as f64 / 1e3, "k"),
        n if n < 1_000_000_000 => short(n as f64 / 1e6, "m"),
        n => short(n as f64 / 1e9, "b"),
    }
}

/// `DiffStatLabel` inline: `+n` in the success ink and `-n` in the error ink, mono, 4 px apart.
fn stat(added: u64, removed: u64, p: &Palette, cx: &App) -> impl IntoElement {
    h_flex()
        .debug_selector(|| "stat".into())
        .gap(px(4.))
        .font_family(cx.theme().mono_font_family.clone())
        .child(div().text_color(p.success).child(format!("+{}", compact(added))))
        .child(div().text_color(p.error_foreground).child(format!("-{}", compact(removed))))
}

/// `DiffStatLabel`'s aligned layout, a tree row's: each figure right-aligned in a `4ch` column, the two `gap-2` apart.
fn aligned_stat(added: u64, removed: u64, p: &Palette, cx: &App) -> impl IntoElement {
    let mono = cx.theme().mono_font_family.clone();
    let font = cx.text_system().resolve_font(&gpui_kit::font(mono.clone()));
    let ch = cx.text_system().ch_width(font, META_LOOSE.0).unwrap_or(META_LOOSE.0 * 0.6);
    let column = |figure: String, ink: Hsla| div().flex_none().w(ch * 4.).flex().justify_end().text_color(ink).child(figure);
    h_flex()
        .debug_selector(|| "stat-aligned".into())
        .gap(px(8.))
        .font_family(mono)
        .child(column(format!("+{}", compact(added)), p.success))
        .child(column(format!("-{}", compact(removed)), p.error_foreground))
}

/// Which folders a card's trees stand open in: every folder at the card's toggle, less the ones pressed since.
#[derive(Default)]
struct Folders {
    all: bool,
    pressed: HashMap<String, bool>,
}

/// `ChangedFilesCard`: the head with the count, the stat and its acts, then the tree with every folder shut.
fn changed_files(
    key: &str,
    label: &str,
    files: &[ChangedFile],
    others: &[ChangedFile],
    p: &Palette,
    window: &mut Window,
    cx: &mut Context<ThreadView>,
) -> AnyElement {
    let dark = cx.theme().is_dark();
    let folders = window.use_keyed_state(SharedString::from(format!("folders-{key}")), cx, |_, _| Folders::default());
    let all = folders.read(cx).all;
    let (added, removed) = files.iter().fold((0, 0), |(a, r), f| (a + f.added, r + f.removed));
    let nested = files.iter().chain(others).any(|f| f.path.contains(['/', '\\']));
    let toggle_all = nested.then(|| {
        let (g, word) = if all { (Glyph::ChevronsDownUp, "Collapse all folders") } else { (Glyph::ChevronsUpDown, "Expand all folders") };
        let folders = folders.clone();
        ghost_xs(SharedString::from(format!("folders-all-{key}")), g, None, true, p)
            .debug_selector(|| "expand-all".into())
            .tooltip(move |window, cx| gpui_kit::component::tooltip::Tooltip::new(word).build(window, cx))
            .on_click(move |_, _, cx| {
                folders.update(cx, |f, cx| {
                    f.all = !f.all;
                    f.pressed.clear();
                    cx.notify();
                })
            })
    });
    // The Changes pane opens on the turn once a screen draws it; until then the press goes nowhere, as `open_pane` says.
    let open_diff = ghost_xs(SharedString::from(format!("open-diff-{key}")), Glyph::FileDiff, Some("Open diff"), true, p)
        .debug_selector(|| "open-diff".into())
        .on_click(|_, _, cx| open_pane(Pane::Changes, None, cx));
    let head_fill = if dark { mix_over(p.input, 0.2, p.background) } else { p.muted };
    let head = h_flex()
        .debug_selector(|| "changed-head".into())
        .justify_between()
        .gap(px(8.))
        .rounded_t(px(10.))
        .bg(head_fill)
        .px(px(12.))
        .py(px(8.))
        .child(
            h_flex()
                .min_w_0()
                .flex_wrap()
                .gap_x(px(12.))
                .text_size(XS.0)
                .line_height(XS.1)
                .font_weight(FontWeight::MEDIUM)
                .text_color(p.foreground)
                .child(label.to_owned())
                .when(added > 0 || removed > 0, |el| el.child(stat(added, removed, p, cx))),
        )
        .child(h_flex().flex_none().gap(px(4.)).children(toggle_all).child(open_diff));
    let tree_of = |files: &[ChangedFile], cx: &mut Context<ThreadView>| {
        let nodes = tree(files);
        let lead = nodes.iter().any(|n| matches!(n, Node::Folder { .. }));
        let mut rows = Vec::new();
        let state = folders.read(cx);
        tree_rows(key, &nodes, 0, lead, state, &folders, p, cx, &mut rows);
        div().p(px(8.)).child(v_flex().gap(px(2.)).children(rows))
    };
    let own = (!files.is_empty()).then(|| tree_of(files, cx));
    let also = (!others.is_empty()).then(|| {
        v_flex()
            .pt(px(16.))
            .child(div().px(px(12.)).text_size(XS.0).line_height(XS.1).text_color(p.muted_foreground).child("Also changed in this folder"))
            .child(tree_of(others, cx))
    });
    v_flex()
        .debug_selector(|| "changed-files".into())
        .rounded(px(10.))
        .bg(if dark { p.input.opacity(0.2) } else { p.muted })
        .child(head)
        .children(own)
        .children(also)
        .into_any_element()
}

#[allow(clippy::too_many_arguments)]
fn tree_rows(
    key: &str,
    nodes: &[Node],
    depth: usize,
    lead: bool,
    state: &Folders,
    folders: &gpui_kit::Entity<Folders>,
    p: &Palette,
    cx: &App,
    out: &mut Vec<AnyElement>,
) {
    let mono = cx.theme().mono_font_family.clone();
    let line = |id: String, pad: f32| {
        h_flex()
            .id(SharedString::from(id))
            .w_full()
            .gap(px(6.))
            .rounded(px(14.))
            .py(px(4.))
            .pr(px(12.))
            .pl(px(pad))
            .font_family(mono.clone())
            .text_size(META_LOOSE.0)
            .line_height(META_LOOSE.1)
            .cursor_pointer()
            .hover(|s| s.bg(p.accent.opacity(0.6)))
    };
    let pad = 8. + depth as f32 * 14.;
    for node in nodes {
        match node {
            Node::Folder { name, path, added, removed, children } => {
                let open = state.pressed.get(path).copied().unwrap_or(state.all);
                let (pressing, pressed) = (folders.clone(), path.clone());
                out.push(
                    line(format!("folder-{key}-{path}"), pad)
                        .debug_selector(|| "folder".into())
                        .child(
                            Icon::new(if open { Glyph::ChevronDown } else { Glyph::ChevronRight })
                                .with_size(px(14.))
                                .text_color(p.muted_foreground.opacity(0.7)),
                        )
                        .child(
                            Icon::new(if open { Glyph::Folder } else { Glyph::FolderClosed })
                                .with_size(px(14.))
                                .text_color(p.muted_foreground.opacity(0.75)),
                        )
                        .child(div().min_w_0().truncate().text_color(p.muted_foreground).child(name.clone()))
                        .when(*added > 0 || *removed > 0, |el| {
                            el.child(div().ml_auto().flex_none().child(aligned_stat(*added, *removed, p, cx)))
                        })
                        .on_click(move |_, _, cx| {
                            pressing.update(cx, |f, cx| {
                                let now = f.pressed.get(&pressed).copied().unwrap_or(f.all);
                                f.pressed.insert(pressed.clone(), !now);
                                cx.notify();
                            })
                        })
                        .into_any_element(),
                );
                if open {
                    tree_rows(key, children, depth + 1, lead, state, folders, p, cx, out);
                }
            }
            Node::File { name, path, added, removed } => out.push(
                line(format!("file-{key}-{path}"), pad)
                    .debug_selector(|| "changed-file".into())
                    .when(lead || depth > 0, |el| el.child(div().flex_none().size(px(14.))))
                    .child(Icon::new(Glyph::File).with_size(px(14.)).text_color(p.muted_foreground.opacity(0.7)))
                    .child(div().min_w_0().truncate().text_color(p.muted_foreground).child(name.clone()))
                    .child(div().ml_auto().flex_none().child(aligned_stat(*added, *removed, p, cx)))
                    .on_click(|_, _, cx| open_pane(Pane::Changes, None, cx))
                    .into_any_element(),
            ),
        }
    }
}

/// `attachmentLine`: what an image the bubble holds no bytes of reads as, the line the command line prints.
pub fn attachment_line(tile: &FileTile) -> String {
    let size = crate::ui::fmt_bytes(tile.bytes.unwrap_or(0));
    match tile.mime.as_deref().and_then(image_word) {
        Some(word) => format!("[image {size} {word}]"),
        None => format!("[file {size} {}]", tile.name),
    }
}

fn image_word(mime: &str) -> Option<&'static str> {
    match mime {
        "image/png" => Some("png"),
        "image/jpeg" => Some("jpeg"),
        "image/gif" => Some("gif"),
        "image/webp" => Some("webp"),
        _ => None,
    }
}

fn image_format(mime: &str) -> Option<ImageFormat> {
    match mime {
        "image/png" => Some(ImageFormat::Png),
        "image/jpeg" => Some(ImageFormat::Jpeg),
        "image/gif" => Some(ImageFormat::Gif),
        "image/webp" => Some(ImageFormat::Webp),
        _ => None,
    }
}

/// Standard base64 with or without padding; None on anything else.
pub fn unbase64(text: &str) -> Option<Vec<u8>> {
    let value = |c: u8| match c {
        b'A'..=b'Z' => Some(c - b'A'),
        b'a'..=b'z' => Some(c - b'a' + 26),
        b'0'..=b'9' => Some(c - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    };
    let clean: Vec<u8> = text.bytes().filter(|c| !c.is_ascii_whitespace()).collect();
    let body = clean.strip_suffix(b"==").or_else(|| clean.strip_suffix(b"=")).unwrap_or(&clean);
    let mut out = Vec::with_capacity(body.len() * 3 / 4);
    for chunk in body.chunks(4) {
        let v: Vec<u8> = chunk.iter().map(|c| value(*c)).collect::<Option<_>>()?;
        if v.len() == 1 {
            return None;
        }
        let n = v.iter().enumerate().fold(0u32, |n, (i, x)| n | (*x as u32) << (18 - 6 * i));
        out.extend_from_slice(&n.to_be_bytes()[1..v.len()]);
    }
    Some(out)
}

/// An image's size in pixels off its header, for the dialog to open at: PNG, GIF, JPEG and WebP.
pub fn image_size(b: &[u8]) -> Option<(f32, f32)> {
    let be16 = |i: usize| Some(u16::from_be_bytes(b.get(i..i + 2)?.try_into().ok()?) as f32);
    let le16 = |i: usize| Some(u16::from_le_bytes(b.get(i..i + 2)?.try_into().ok()?) as f32);
    let le24 = |i: usize| Some(*b.get(i)? as f32 + (*b.get(i + 1)? as f32) * 256. + (*b.get(i + 2)? as f32) * 65_536.);
    if b.starts_with(b"\x89PNG") {
        let w = u32::from_be_bytes(b.get(16..20)?.try_into().ok()?);
        let h = u32::from_be_bytes(b.get(20..24)?.try_into().ok()?);
        return Some((w as f32, h as f32));
    }
    if b.starts_with(b"GIF8") {
        return Some((le16(6)?, le16(8)?));
    }
    if b.starts_with(b"RIFF") && b.get(8..12) == Some(b"WEBP") {
        return match b.get(12..16)? {
            b"VP8X" => Some((le24(24)? + 1., le24(27)? + 1.)),
            b"VP8L" => {
                let bits = u32::from_le_bytes(b.get(21..25)?.try_into().ok()?);
                Some(((bits & 0x3fff) as f32 + 1., ((bits >> 14) & 0x3fff) as f32 + 1.))
            }
            _ => Some(((le16(26)? as u32 & 0x3fff) as f32, (le16(28)? as u32 & 0x3fff) as f32)),
        };
    }
    if b.starts_with(&[0xff, 0xd8]) {
        let mut i = 2;
        while i + 9 < b.len() {
            if b[i] != 0xff {
                i += 1;
                continue;
            }
            let marker = b[i + 1];
            if matches!(marker, 0xc0..=0xc3 | 0xc5..=0xc7 | 0xc9..=0xcb | 0xcd..=0xcf) {
                return Some((be16(i + 7)?, be16(i + 5)?));
            }
            i += 2 + be16(i + 2)? as usize;
        }
    }
    None
}

/// Asks the host for each image of the row's message it keeps, once per time the row is drawn anew.
fn recall(view: &mut ThreadView, row: &Row, files: &[FileTile], cx: &mut Context<ThreadView>) {
    if view.images.contains_key(&row.key) || !files.iter().any(|f| f.image) {
        return;
    }
    let (workspace, thread) = (view.followed_workspace().map(str::to_owned), view.followed().map(str::to_owned));
    let shots = files
        .iter()
        .map(|f| match (f.image, &f.id, &workspace, &thread) {
            (false, ..) => Shot::File,
            (true, Some(_), Some(_), Some(_)) => Shot::Asking,
            _ => Shot::Missing,
        })
        .collect();
    view.images.insert(row.key.clone(), shots);
    let (Some(workspace), Some(thread)) = (workspace, thread) else { return };
    for (slot, tile) in files.iter().enumerate() {
        let (true, Some(request)) = (tile.image, tile.id.clone()) else { continue };
        let ask = SessionsAttachmentRequest {
            workspace_id: workspace.clone(),
            thread_id: thread.clone(),
            request_id: request,
            index: tile.index as u64,
        };
        let key = row.key.clone();
        view.call(ask, cx, move |view, kept, cx| {
            let shot = kept
                .ok()
                .and_then(|kept| {
                    let format = image_format(&kept.attachment.media_type)?;
                    let bytes = unbase64(&kept.attachment.bytes)?;
                    let size = image_size(&bytes);
                    Some(Shot::Held(Arc::new(Image::from_bytes(format, bytes)), size))
                })
                .unwrap_or(Shot::Missing);
            if let Some(slot) = view.images.get_mut(&key).and_then(|shots| shots.get_mut(slot)) {
                *slot = shot;
            }
            cx.notify();
        });
    }
}

/// `ChatFileRow`: images as thumbnails once all are held, else one mono line each; every other file as its tile.
pub fn files(view: &mut ThreadView, row: &Row, files: &[FileTile], p: &Palette, cx: &mut Context<ThreadView>) -> Option<AnyElement> {
    if files.is_empty() {
        return None;
    }
    recall(view, row, files, cx);
    let shots = view.images.get(&row.key).cloned().unwrap_or_default();
    let held = files.iter().enumerate().all(|(i, f)| !f.image || matches!(shots.get(i), Some(Shot::Held(..))));
    let mono = cx.theme().mono_font_family.clone();
    let mut tiles: Vec<AnyElement> = Vec::new();
    for (i, tile) in files.iter().enumerate() {
        match (tile.image, shots.get(i)) {
            (true, Some(Shot::Held(image, size))) if held => tiles.push(thumb(&row.key, i, &tile.name, image.clone(), *size, p)),
            (true, _) => {}
            (false, _) => tiles.push(file_tile(&tile.name, tile.bytes.unwrap_or(0), p, &mono)),
        }
    }
    let lines: Vec<String> = if held { Vec::new() } else { files.iter().filter(|f| f.image).map(attachment_line).collect() };
    Some(
        v_flex()
            .when(!tiles.is_empty(), |el| {
                el.child(h_flex().debug_selector(|| "tiles".into()).flex_wrap().gap(px(6.)).mb(px(8.)).children(tiles))
            })
            .when(!lines.is_empty(), |el| {
                el.child(
                    v_flex()
                        .debug_selector(|| "file-lines".into())
                        .gap(px(2.))
                        .mb(px(4.))
                        .font_family(mono.clone())
                        .text_size(META.0)
                        .line_height(META.1)
                        .text_color(p.muted_foreground)
                        .children(lines),
                )
            })
            .into_any_element(),
    )
}

/// `ChatImageThumb`: 56 px square, 10 px round, a 1 px edge at 60%, the image covering it; a press opens it whole.
fn thumb(key: &str, at_ix: usize, name: &str, image: Arc<Image>, size: Option<(f32, f32)>, p: &Palette) -> AnyElement {
    let name = SharedString::from(name.to_owned());
    let shown = image.clone();
    div()
        .id(SharedString::from(format!("thumb-{key}-{at_ix}")))
        .debug_selector(|| "thumb".into())
        .flex_none()
        .size(px(56.))
        .overflow_hidden()
        .rounded(px(10.))
        .border_1()
        .border_color(p.border.opacity(0.6))
        .bg(p.card.opacity(0.5))
        .cursor_pointer()
        .child(img(ImageSource::Image(image)).size_full().object_fit(ObjectFit::Cover))
        .on_click(move |_, window, cx| open_image(name.clone(), shown.clone(), size, window, cx))
        .into_any_element()
}

/// The image at full size in a dialog, `w-auto max-w-[90vw] p-2`, the image `max-h-[80vh] max-w-[86vw]`, 8 px round.
fn open_image(name: SharedString, image: Arc<Image>, size: Option<(f32, f32)>, window: &mut Window, cx: &mut App) {
    use crate::screens::dialogs::layer::{self, Closes};
    let view = cx.new(|_| ImagePreview { name, image, size });
    layer::open(IMAGE_PREVIEW, view.into(), Closes::Outside, None, window, cx);
}

/// The dialogs layer's name for a message's image opened whole.
pub const IMAGE_PREVIEW: &str = "image-preview";

/// What the image dialog draws on the layer, which gives it its backdrop, its place, Escape and the press outside.
struct ImagePreview {
    name: SharedString,
    image: Arc<Image>,
    size: Option<(f32, f32)>,
}

impl Render for ImagePreview {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = cx.palette().clone();
        let view = window.viewport_size();
        let (max_w, max_h) = (view.width * 0.86, view.height * 0.8);
        let (w, h) = match self.size {
            Some((w, h)) => {
                let scale = (max_w / px(w)).min(max_h / px(h)).min(1.);
                (px(w) * scale, px(h) * scale)
            }
            None => (max_w, max_h),
        };
        div().p(px(8.)).rounded(px(14.)).border_1().border_color(p.popover_edge).bg(p.popover).shadow(p.dialog_shadow.clone()).child(
            img(ImageSource::Image(self.image.clone()))
                .debug_selector(|| "image-dialog".into())
                .id(self.name.clone())
                .w(w)
                .h(h)
                .rounded(px(8.))
                .object_fit(ObjectFit::Contain),
        )
    }
}

/// `ChatFileTile`: as tall as a thumbnail, at most 192 px wide, the file glyph and its name over its weight.
fn file_tile(name: &str, size: u64, p: &Palette, mono: &SharedString) -> AnyElement {
    h_flex()
        .debug_selector(|| "file-tile".into())
        .flex_none()
        .h(px(56.))
        .max_w(px(192.))
        .gap(px(8.))
        .rounded(px(10.))
        .border_1()
        .border_color(p.border.opacity(0.6))
        .bg(p.card.opacity(0.5))
        .pl(px(10.))
        .pr(px(12.))
        .child(Icon::new(Glyph::FileText).with_size(px(16.)).text_color(p.muted_foreground))
        .child(
            v_flex()
                .min_w_0()
                .child(div().truncate().text_size(px(13.)).line_height(px(20.)).text_color(p.foreground).child(name.to_owned()))
                .child(
                    div()
                        .font_family(mono.clone())
                        .text_size(META.0)
                        .line_height(META.1)
                        .text_color(p.muted_foreground)
                        .child(crate::ui::fmt_bytes(size)),
                ),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use gpui_kit::component::Root;
    use gpui_kit::{size, Bounds, Entity, Modifiers, TestAppContext, VisualTestContext};
    use serde_json::json;
    use wsp_host_client::protocol::SessionEvent;

    use super::*;
    use crate::screens::sidebar::model::Kind;
    use crate::transcript::Listing;

    fn base(kind: &str, pos: u64, extra: serde_json::Value) -> SessionEvent {
        let mut v = json!({"type": kind, "workspaceId": "w", "sessionId": "s", "threadId": "t", "turnId": "u1", "pos": pos});
        v.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        serde_json::from_value(v).unwrap()
    }

    fn window(events: Vec<SessionEvent>, cx: &mut TestAppContext) -> (Entity<ThreadView>, &mut VisualTestContext) {
        window_listed(events, None, cx)
    }

    /// The window with what the listing says already standing when the transcript is read, as the lead's page has an
    /// address once the view follows its thread.
    fn window_listed(
        events: Vec<SessionEvent>,
        listing: Option<Listing>,
        cx: &mut TestAppContext,
    ) -> (Entity<ThreadView>, &mut VisualTestContext) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            crate::ui::init(crate::ui::Side::Light, cx);
        });
        let view = cx.new(ThreadView::new);
        let layered = cx.new(|_| Layered(view.clone()));
        let (_, cx) = cx.add_window_view(|window, cx| Root::new(layered, window, cx));
        cx.simulate_resize(size(px(900.), px(1000.)));
        view.update(cx, |v, cx| {
            v.show("t", cx);
            if let Some(listing) = listing {
                v.set_listing(listing, cx);
            }
            v.load(events, cx);
            v.scroll_to_top(cx);
        });
        cx.run_until_parked();
        (view, cx)
    }

    /// The view under the window's dialogs layer, as the shell draws the layer last.
    struct Layered(Entity<ThreadView>);

    impl Render for Layered {
        fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            div().size_full().child(self.0.clone()).children(crate::screens::dialogs::layer::draw(window, cx))
        }
    }

    fn bounds(cx: &mut VisualTestContext, selector: &'static str) -> Bounds<Pixels> {
        cx.debug_bounds(selector).unwrap_or_else(|| panic!("{selector} was not drawn"))
    }

    /// The frame of the first row of a kind, which every card stands in.
    fn frame(view: &Entity<ThreadView>, cx: &mut VisualTestContext, kind: fn(&RowKind) -> bool) -> Bounds<Pixels> {
        let ix = view.read_with(cx, |v, _| v.rows().iter().position(|r| kind(&r.kind)).expect("a row of that kind"));
        let selector: &'static str = Box::leak(format!("row-{ix}").into_boxed_str());
        bounds(cx, selector)
    }

    fn sent() -> Vec<(String, String, String)> {
        SENT.with(|s| s.borrow().iter().map(|r| (r.session_id.clone(), r.ask_id.clone(), r.option_id.clone())).collect())
    }

    /// A permission to run `cargo test --all`: the thread's own, or where `subagent` names its call, a subagent's.
    fn permission_of(subagent: Option<&str>) -> Vec<SessionEvent> {
        let options = json!([{"id": "y", "label": "Allow", "effect": "allow"}, {"id": "n", "label": "Deny", "effect": "deny"}]);
        let input = "{\"command\":\"cargo test --all\",\"timeout\":5}";
        let mut asked = json!({"askId": "k", "toolName": "Bash", "toolUseId": "a", "input": input, "options": options});
        if let Some(call) = subagent {
            asked["parentToolUseId"] = json!(call);
        }
        vec![
            base("session.start", 1, json!({"prompt": "go"})),
            base("session.delta", 2, json!({"kind": "tool_use", "toolName": "Bash", "toolUseId": "a", "text": input})),
            base("session.permission", 3, asked),
        ]
    }

    fn permission() -> Vec<SessionEvent> {
        permission_of(None)
    }

    #[gpui_kit::test]
    fn the_threads_own_open_permission_is_the_waiting_record_and_offers_nothing(cx: &mut TestAppContext) {
        let (view, cx) = window(permission(), cx);
        let row = frame(&view, cx, |k| matches!(k, RowKind::Permission { .. }));
        let record = bounds(cx, "record");
        assert_eq!((record.left() - row.left(), record.top() - row.top(), record.size.height), (px(4.), px(4.), px(28.)));
        assert!(cx.debug_bounds("options").is_none(), "the composer's dock answers it, not the row");
        assert!(cx.debug_bounds("lead").is_none());
    }

    #[gpui_kit::test]
    fn an_open_permission_draws_as_the_web_and_a_pick_sends_its_ids(cx: &mut TestAppContext) {
        SENT.with(|s| s.borrow_mut().clear());
        // A subagent's prompt keeps its card in its fold; the thread's own docks.
        let (view, cx) = window(permission_of(Some("task-1")), cx);
        let row = frame(&view, cx, |k| matches!(k, RowKind::Permission { asker: Some(_), .. }));
        let lead = bounds(cx, "lead");
        // The fold's gutter and rail (`px-1`, `ml-1.5`, the 1 px rule, `pl-[11px]`), `pt-1` twice and the asker's 16 px
        // line 4 px over the lead: the command's `py-1` on a 16 px line stands taller than the 22.75 px words.
        assert_eq!((lead.left() - row.left(), lead.top() - row.top(), lead.size.height), (px(22.), px(28.), px(24.)));
        // `pt-0.5` over the xs buttons, 4 px under the fields' line.
        let options = bounds(cx, "options");
        assert_eq!(options.size.height, px(26.));
        assert_eq!(options.top() - lead.bottom(), px(4. + 16. + 4.));
        cx.simulate_click(options.origin + gpui_kit::point(px(12.), px(14.)), Modifiers::none());
        cx.run_until_parked();
        assert_eq!(sent(), [("s".to_owned(), "k".to_owned(), "y".to_owned())]);
        // With no host the call refuses at once, and the refusal stands under the options in the error ink's line.
        let refused = view.read_with(cx, |v, _| v.refusals.get("k").cloned());
        assert_eq!(refused.map(|r| r.said).as_deref(), Some("The thread view follows no host."));
        let line = bounds(cx, "refusal");
        assert_eq!((line.top() - options.bottom(), line.size.height), (px(4.), px(20.)));
    }

    #[gpui_kit::test]
    fn a_closed_permission_is_one_record_line(cx: &mut TestAppContext) {
        let mut events = permission();
        events.push(base("session.permission.closed", 4, json!({"askId": "k", "outcome": "allowed", "optionId": "y"})));
        let (view, cx) = window(events, cx);
        let row = frame(&view, cx, |k| matches!(k, RowKind::Permission { .. }));
        let record = bounds(cx, "record");
        // `px-0.5 py-0.5` round the 24 px glyph slot, inside `pt-1 pb-2 px-1`, then the row's `pb-4`.
        assert_eq!((record.left() - row.left(), record.top() - row.top(), record.size.height), (px(4.), px(4.), px(28.)));
        // The last row also keeps the list's end space and the composer's inset under its foot.
        assert_eq!(row.bottom() - record.bottom(), px(8. + 16.) + super::super::rows::END_SPACE + super::super::rows::COMPOSER_INSET);
        assert!(cx.debug_bounds("options").is_none(), "a closed prompt offers nothing");
    }

    /// A subagent's question, which keeps its card; the thread's own docks in the composer.
    fn question(multiple: bool) -> Vec<SessionEvent> {
        let input = json!({"questions": [{"question": "Which colour?", "header": "Colour", "multiSelect": multiple, "options": [
            {"label": "Red", "description": "warm"}, {"label": "Blue"}
        ]}]});
        let options = json!([{"id": "q0:o0", "label": "Red", "effect": "answer"}, {"id": "q0:o1", "label": "Blue", "effect": "answer"}]);
        let asked = json!({"askId": "w", "toolName": "AskUserQuestion", "parentToolUseId": "task-1", "input": input.to_string(), "options": options});
        vec![base("session.start", 1, json!({"prompt": "go"})), base("session.permission", 2, asked)]
    }

    #[gpui_kit::test]
    fn the_threads_own_question_is_the_waiting_record(cx: &mut TestAppContext) {
        let mut events = question(false);
        if let Some(SessionEvent::SessionPermission { parent_tool_use_id, .. }) = events.last_mut() {
            *parent_tool_use_id = None;
        }
        let (_, cx) = window(events, cx);
        assert!(cx.debug_bounds("question").is_none(), "the composer's dock asks it");
        assert!(cx.debug_bounds("record").is_some());
    }

    #[gpui_kit::test]
    fn a_question_with_several_answers_ticks_boxes_and_sends_them_as_one(cx: &mut TestAppContext) {
        SENT.with(|s| s.borrow_mut().clear());
        let (_, cx) = window(question(true), cx);
        let tick = bounds(cx, "checkbox");
        assert_eq!(tick.size, size(px(16.), px(16.)));
        let asked = bounds(cx, "question");
        let answer = asked.bottom_left() + gpui_kit::point(px(12.), px(-12.));
        // Held until every question has a tick: a press sends nothing.
        cx.simulate_click(answer, Modifiers::none());
        cx.run_until_parked();
        assert!(sent().is_empty(), "{:?}", sent());
        // The first option's line, under the header (16), the question (22.75), the gaps (6, 6) and `pt-0.5`.
        cx.simulate_click(asked.origin + gpui_kit::point(px(8.), px(16. + 6. + 22.75 + 6. + 2. + 10.)), Modifiers::none());
        cx.run_until_parked();
        // An option's line: `py-0.5`, the box `mt-0.5`.
        let tick = bounds(cx, "checkbox");
        let line = cx.debug_bounds("tick").unwrap();
        assert_eq!(tick.top() - line.top(), px(4.));
        // `tone="neutral"`: the tick fills with the foreground, not the accent.
        let foreground = cx.update(|_, cx| cx.palette().foreground);
        assert_eq!(crate::ui::controls::TICKED_FILL.with(|f| f.get()), Some(foreground), "the ticked box took the accent fill");
        cx.simulate_click(answer, Modifiers::none());
        cx.run_until_parked();
        assert_eq!(sent(), [("s".to_owned(), "w".to_owned(), "q0:o0".to_owned())]);
    }

    #[gpui_kit::test]
    fn a_question_with_one_answer_answers_on_the_press(cx: &mut TestAppContext) {
        SENT.with(|s| s.borrow_mut().clear());
        let (_, cx) = window(question(false), cx);
        assert!(cx.debug_bounds("checkbox").is_none(), "one answer takes no boxes");
        let asked = bounds(cx, "question");
        // The header (16) and the question (22.75), 6 px apart, then `pt-0.5`: the first answer's button.
        cx.simulate_click(asked.origin + gpui_kit::point(px(12.), px(16. + 6. + 22.75 + 6. + 2. + 12.)), Modifiers::none());
        cx.run_until_parked();
        assert_eq!(sent(), [("s".to_owned(), "w".to_owned(), "q0:o0".to_owned())]);
    }

    fn plan(text: &str) -> Vec<SessionEvent> {
        vec![
            base("session.start", 1, json!({"prompt": "plan it"})),
            base("session.plan", 2, json!({"text": text})),
            base("session.delta", 3, json!({"kind": "text", "text": "Here it is."})),
            base("session.done", 4, json!({"result": {"status": "completed", "durationMs": 2_000}})),
            base("session.end", 5, json!({"exitCode": 0, "sawResult": true})),
        ]
    }

    #[gpui_kit::test]
    fn a_long_plan_folds_behind_expand_plan_and_its_menu_copies_it(cx: &mut TestAppContext) {
        let long: String = std::iter::once("# Fix the build\n\n".to_owned()).chain((0..30).map(|n| format!("- step {n}\n"))).collect();
        let (view, cx) = window(plan(&long), cx);
        let card = bounds(cx, "plan");
        let title = bounds(cx, "plan-title");
        // `border p-5`; the title's 20 px line; the menu's `icon-xs` keycap at the right.
        // The head is as tall as the button, the title centred in it.
        assert_eq!((title.left() - card.left(), title.top() - card.top(), title.size.height), (px(21.), px(23.), px(20.)));
        let menu = bounds(cx, "menu-button");
        assert_eq!((menu.size, card.right() - menu.right()), (size(px(24.), px(24.)), px(21.)));
        // Folded: `mt-4`, the preview at most `max-h-104`, `mt-4`, the 32 px Expand plan, the card's padding and border.
        let folded = card.size.height;
        let body = bounds(cx, "plan-body");
        assert!(body.size.height <= px(416.), "{body:?}");
        assert_eq!(folded, px(1. + 20. + 24. + 16.) + body.size.height + px(16. + 32. + 20. + 1.));
        let expand = card.bottom() - px(21. + 16.);
        cx.simulate_click(gpui_kit::point(card.center().x, expand), Modifiers::none());
        cx.run_until_parked();
        assert!(view.read_with(cx, |v, _| v.rows().iter().any(|r| matches!(r.kind, RowKind::Plan { open: true, .. }))));
        assert!(bounds(cx, "plan").size.height > folded + px(100.), "the plan opens past its fold");
        cx.simulate_click(menu.center(), Modifiers::none());
        cx.run_until_parked();
        let popup = bounds(cx, "plan-menu");
        let copy = bounds(cx, "plan-copy");
        // `min-w-32 p-1`, its rows `min-h-7`, its right edge under the button's.
        assert!(popup.size.width >= px(128.));
        assert_eq!(
            (copy.size.height, copy.top() - popup.top(), popup.right(), popup.top() - menu.bottom()),
            (px(28.), px(5.), menu.right(), px(4.))
        );
        cx.simulate_click(copy.center(), Modifiers::none());
        cx.run_until_parked();
        assert_eq!(cx.read_from_clipboard().and_then(|c| c.text()), Some(format!("{}\n", long.trim_end())));
        assert!(cx.debug_bounds("plan-menu").is_none(), "a pick shuts the menu");
    }

    #[gpui_kit::test]
    fn a_path_in_a_plan_draws_as_a_file_chip(cx: &mut TestAppContext) {
        let mut events = plan("# Fix the build\n\n- Change `src/main.rs` so it holds.\n");
        events[0] = base("session.start", 1, json!({"prompt": "plan it", "cwd": "/w"}));
        let (_, cx) = window(events, cx);
        assert!(cx.debug_bounds("file-chip").is_some(), "the plan drew src/main.rs as code, not as today's file chip");
    }

    #[test]
    fn a_refused_pick_reads_the_protocols_answer_words() {
        let words = ["gone", "unsupported", "not-found", "no-option"].map(answer_words);
        assert_eq!(words[0], Some("the prompt closed before the answer reached it"));
        assert!(words.iter().all(|w| w.is_some_and(|w| !w.contains('"'))), "{words:?}");
        assert_eq!(answer_words("answered"), None, "an answered pick is no refusal");
        assert_eq!(sentence(words[2].unwrap()), "This host holds no turn of that thread.");
    }

    #[gpui_kit::test]
    fn a_plan_downloads_where_the_save_dialog_says(cx: &mut TestAppContext) {
        assert_eq!(plan_filename(Some("Fix the build's (CI) step!")), "fix-the-builds-ci-step.md");
        assert_eq!(plan_filename(None), "plan.md");
        let text = "# Fix the build\n\n- step one\n";
        let (_, cx) = window(plan(text), cx);
        let at = bounds(cx, "menu-button").center();
        cx.simulate_click(at, Modifiers::none());
        cx.run_until_parked();
        let at = bounds(cx, "plan-download").center();
        cx.simulate_click(at, Modifiers::none());
        cx.run_until_parked();
        assert!(cx.did_prompt_for_new_path(), "Download as markdown wrote without asking where");
        let folder = tempfile::tempdir().unwrap();
        let chosen = folder.path().join("mine.md");
        let to = chosen.clone();
        cx.simulate_new_path_selection(move |_| Some(to));
        cx.run_until_parked();
        assert_eq!(std::fs::read_to_string(&chosen).unwrap(), format!("{}\n", text.trim_end()));
    }

    fn subagent() -> Vec<SessionEvent> {
        let task = "{\"description\":\"Survey the code\",\"prompt\":\"look\"}";
        vec![
            base("session.start", 1, json!({"prompt": "go", "at": 1_000.0})),
            base("session.delta", 2, json!({"kind": "tool_use", "toolName": "Task", "toolUseId": "t", "text": task, "at": 2_000.0})),
            base("session.delta", 3, json!({"kind": "text", "text": "Reading.", "parentToolUseId": "t"})),
            base(
                "session.delta",
                4,
                json!({"kind": "tool_use", "toolName": "Read", "toolUseId": "c", "parentToolUseId": "t", "text": "{\"file_path\":\"src/b.rs\"}"}),
            ),
            base("session.delta", 5, json!({"kind": "tool_result", "toolUseId": "c", "parentToolUseId": "t", "text": "fn b() {}"})),
            base("session.delta", 6, json!({"kind": "tool_result", "toolUseId": "t", "text": "Found it.", "at": 67_000.0})),
        ]
    }

    #[gpui_kit::test]
    fn a_subagent_fold_opens_its_lines_on_the_rail(cx: &mut TestAppContext) {
        let lead = Listing { thread: "t".into(), workspace: Some("w".into()), ..Listing::default() };
        let (view, cx) = window_listed(subagent(), Some(lead), cx);
        let row = frame(&view, cx, |k| matches!(k, RowKind::Subagent { .. }));
        let trigger = bounds(cx, "subagent-trigger");
        let page = bounds(cx, "subagent-page");
        // `px-1`; the title's 20 px line; the `icon-micro` arrow 6 px after it; shut, the row's `pb-4`.
        assert_eq!((trigger.left() - row.left(), trigger.size.height, row.size.height), (px(4.), px(20.), px(36.)));
        assert_eq!((page.size, page.left() - trigger.right()), (size(px(20.), px(20.)), px(6.)));
        cx.simulate_click(trigger.center(), Modifiers::none());
        cx.run_until_parked();
        let body = bounds(cx, "subagent-body");
        // The rail hangs from the chevron's centre (`ml-1.5`) and the lines start under the title (`pl-[11px]`, `pt-1`).
        assert_eq!((body.left() - trigger.left(), body.top()), (px(6.), trigger.bottom()));
        // Prose, the call over its answer with no gap between the two, prose: `gap-1` only between the lines.
        let lines = px(4. + 22.75 + 4. + 16. + 16. + 4. + 22.75);
        assert!((body.size.height - lines).abs() < px(1.), "the fold's lines are {:?}, the web's {lines:?}", body.size.height);
    }

    #[gpui_kit::test]
    fn the_children_a_call_started_stand_in_the_rows_box(cx: &mut TestAppContext) {
        let events = vec![
            base("session.start", 1, json!({"prompt": "go"})),
            base(
                "session.delta",
                2,
                json!({"kind": "tool_use", "toolName": "Task", "toolUseId": "t", "text": "{\"description\":\"Survey\"}"}),
            ),
            base("session.subagent", 3, json!({"task": "k1", "state": "running", "parentToolUseId": "t", "title": "Survey it"})),
            base("session.delta", 4, json!({"kind": "tool_result", "toolUseId": "t", "text": "Found it."})),
            base("session.subagent", 5, json!({"task": "k1", "state": "failed", "parentToolUseId": "t", "summary": "out of room"})),
            base(
                "session.delta",
                6,
                json!({"kind": "tool_use", "toolName": "mcp__wsp__run", "toolUseId": "r", "text": "{\"title\":\"Fix docs\"}"}),
            ),
            base("session.delta", 7, json!({"kind": "tool_result", "toolUseId": "r", "text": "{\"threadId\":\"th1\"}"})),
        ];
        let listing = Listing::holding(&events);
        let (view, cx) = window_listed(events, Some(listing), cx);
        let row = frame(&view, cx, |k| matches!(k, RowKind::Spawn { .. }));
        let column = bounds(cx, "spawn");
        // A row with a note under its title is at least 48 px, one without is 36; no gap, no gutter.
        assert_eq!((column.left(), column.top(), column.size.height), (row.left(), row.top(), px(48. + 36.)));
    }

    fn changes() -> Vec<SessionEvent> {
        vec![
            base("session.start", 1, json!({"prompt": "fix it"})),
            base("session.delta", 2, json!({"kind": "text", "text": "Done here."})),
            base("session.done", 3, json!({"result": {"status": "completed", "durationMs": 2_000}})),
            base("session.end", 4, json!({"exitCode": 0, "sawResult": true})),
            base(
                "session.changes",
                5,
                json!({"from": "a", "to": "b", "moved": [], "files": [
                    {"path": "src/ui/a.rs", "kind": "modified", "additions": 3, "deletions": 1},
                    {"path": "src/ui/c.rs", "kind": "modified", "additions": 1200, "deletions": 0},
                    {"path": "b.rs", "kind": "added", "additions": 9, "deletions": 0}
                ]}),
            ),
        ]
    }

    #[gpui_kit::test]
    fn a_turns_changed_files_open_shut_and_expand_all(cx: &mut TestAppContext) {
        let (_, cx) = window(changes(), cx);
        let card = bounds(cx, "changed-files");
        let head = bounds(cx, "changed-head");
        // `px-3 py-2` round the 24 px acts; the tree `p-2`, each row `py-1` on 11 px at the page's 1.5, 2 px apart.
        assert_eq!(head.size.height, px(40.));
        let row = px(4. + 16.5 + 4.);
        assert_eq!(card.size.height, head.size.height + px(8.) + row + px(2.) + row + px(8.));
        let folder = bounds(cx, "folder");
        assert_eq!(folder.left() - card.left(), px(8.));
        cx.simulate_click(folder.center(), Modifiers::none());
        cx.run_until_parked();
        assert_eq!(bounds(cx, "changed-files").size.height, card.size.height + (px(2.) + row) * 2.);
        cx.simulate_click(folder.center(), Modifiers::none());
        cx.run_until_parked();
        let all = bounds(cx, "expand-all");
        cx.simulate_click(all.center(), Modifiers::none());
        cx.run_until_parked();
        assert_eq!(bounds(cx, "changed-files").size.height, card.size.height + (px(2.) + row) * 2.);
        let open_diff = bounds(cx, "open-diff");
        assert_eq!((open_diff.size.height, head.right() - open_diff.right()), (px(24.), px(12.)));
    }

    #[test]
    fn the_tree_joins_lone_folders_and_counts_compactly() {
        let file = |path: &str, added| ChangedFile { path: path.into(), added, removed: 1 };
        let nodes = tree(&[file("a/b/c.rs", 2), file("a/b/d10.rs", 3), file("a/b/d9.rs", 1), file("z.rs", 1)]);
        let Node::Folder { name, added, children, .. } = &nodes[0] else { panic!("{nodes:?}") };
        assert_eq!((name.as_str(), *added), ("a/b", 6));
        let names: Vec<&str> = children.iter().map(|n| if let Node::File { name, .. } = n { name.as_str() } else { "" }).collect();
        assert_eq!(names, ["c.rs", "d9.rs", "d10.rs"]);
        assert!(matches!(&nodes[1], Node::File { name, .. } if name == "z.rs"));
        assert_eq!(
            (compact(999), compact(1_200), compact(12_345), compact(1_000_000)),
            ("999".into(), "1.2k".into(), "12k".into(), "1m".into())
        );
    }

    fn attachments() -> Vec<SessionEvent> {
        vec![base(
            "session.start",
            1,
            json!({"prompt": "look", "requestId": "q1", "attachments": [
                {"mediaType": "image/png", "bytes": 1300, "name": "shot.png"},
                {"mediaType": "text/plain", "bytes": 30, "name": "notes.txt"}
            ]}),
        )]
    }

    /// A 2 x 1 PNG, whole.
    const PNG: &str = "iVBORw0KGgoAAAANSUhEUgAAAAIAAAABCAIAAAB7QOjdAAAAEElEQVR4nGP4z8DAwMDAAAAJ/QH/ZoWm8QAAAABJRU5ErkJggg==";

    #[gpui_kit::test]
    fn a_messages_files_draw_as_tiles_and_its_unheld_image_as_its_line(cx: &mut TestAppContext) {
        let (view, cx) = window(attachments(), cx);
        let tile = bounds(cx, "file-tile");
        assert_eq!(tile.size.height, px(56.));
        assert!(tile.size.width <= px(192.));
        // With no host to ask, the image is not held: its line, 2 px under the tiles' `mb-2`.
        let lines = bounds(cx, "file-lines");
        assert_eq!((lines.size.height, lines.top() - tile.bottom()), (px(16.), px(8.)));
        assert!(cx.debug_bounds("thumb").is_none());
        let key = view.read_with(cx, |v, _| v.rows()[0].key.clone());
        let bytes = unbase64(PNG).unwrap();
        assert_eq!(image_size(&bytes), Some((2., 1.)));
        view.update(cx, |v, cx| {
            let held = Shot::Held(Arc::new(Image::from_bytes(ImageFormat::Png, bytes)), Some((2., 1.)));
            v.images.insert(key, vec![held, Shot::File]);
            cx.notify();
        });
        cx.run_until_parked();
        let thumb = bounds(cx, "thumb");
        let tile = bounds(cx, "file-tile");
        assert_eq!((thumb.size, tile.left() - thumb.right()), (size(px(56.), px(56.)), px(6.)));
        assert!(cx.debug_bounds("file-lines").is_none(), "a held image needs no line");
        // The layer times its fade out on the wall clock.
        let shut = |cx: &mut VisualTestContext| {
            std::thread::sleep(std::time::Duration::from_millis(250));
            cx.update(|window, cx| window.simulate_next_frame(cx));
            cx.run_until_parked();
            !cx.update(|_, cx| crate::screens::dialogs::layer::is_open(super::IMAGE_PREVIEW, cx))
                && cx.debug_bounds("image-dialog").is_none()
        };
        cx.simulate_click(thumb.center(), Modifiers::none());
        cx.run_until_parked();
        assert_eq!(
            cx.update(|_, cx| crate::screens::dialogs::layer::top(cx)),
            Some(super::IMAGE_PREVIEW),
            "the image opened off the layer"
        );
        assert!(cx.debug_bounds("image-dialog").is_some() && cx.debug_bounds("dialogs-layer").is_some(), "a press opens the image whole");
        for _ in 0..3 {
            cx.executor().advance_clock(std::time::Duration::from_millis(16));
            cx.update(|window, cx| window.simulate_next_frame(cx));
            cx.run_until_parked();
        }
        cx.simulate_keystrokes("escape");
        assert!(shut(cx), "Escape left the image open");
        cx.simulate_click(thumb.center(), Modifiers::none());
        cx.run_until_parked();
        cx.update(|window, cx| window.simulate_next_frame(cx));
        cx.simulate_click(gpui_kit::point(px(4.), px(4.)), Modifiers::none());
        assert!(shut(cx), "a press outside left the image open");
    }

    #[gpui_kit::test]
    fn an_image_lets_go_of_its_pixels_once_its_row_leaves_the_kept_rows(cx: &mut TestAppContext) {
        let mut events = attachments();
        events.push(base("session.done", 2, json!({"result": {"status": "completed"}})));
        events.push(base("session.end", 3, json!({"exitCode": 0, "sawResult": true})));
        for n in 0..60u64 {
            let (turn, pos) = (format!("t{n}"), 10 + 4 * n);
            events.push(base("session.start", pos, json!({"prompt": format!("Question {n}"), "turnId": turn})));
            events.push(base("session.delta", pos + 1, json!({"kind": "text", "text": format!("Answer {n}."), "turnId": turn})));
            events.push(base("session.done", pos + 2, json!({"result": {"status": "completed"}, "turnId": turn})));
            events.push(base("session.end", pos + 3, json!({"exitCode": 0, "sawResult": true, "turnId": turn})));
        }
        let (view, cx) = window(events, cx);
        let key = view.read_with(cx, |v, _| v.rows()[0].key.clone());
        let image = Arc::new(Image::from_bytes(ImageFormat::Png, unbase64(PNG).unwrap()));
        view.update(cx, |v, cx| {
            v.images.insert(key.clone(), vec![Shot::Held(image.clone(), Some((2., 1.))), Shot::File]);
            cx.notify();
        });
        cx.run_until_parked();
        assert!(cx.debug_bounds("thumb").is_some(), "the image was not drawn");
        assert!(cx.update(|_, cx| image.is_asset_cached(cx)), "the drawn image was not decoded");
        view.update(cx, |v, cx| v.scroll_to_end(cx));
        for _ in 0..3 {
            cx.update(|window, cx| window.simulate_next_frame(cx));
            cx.run_until_parked();
        }
        assert!(view.read_with(cx, |v, _| !v.images.contains_key(&key)), "the row's attachments outlived its kept rows");
        assert!(!cx.update(|_, cx| image.is_asset_cached(cx)), "the image's decoded pixels outlived its row");
    }

    #[test]
    fn an_unheld_attachment_reads_as_the_command_line_prints_it() {
        let tile = |mime: &str, bytes| FileTile {
            name: "a.txt".into(),
            bytes: Some(bytes),
            image: mime.starts_with("image/"),
            id: None,
            index: 0,
            mime: Some(mime.into()),
        };
        assert_eq!(attachment_line(&tile("image/png", 1_258_291)), "[image 1 MB png]");
        assert_eq!(attachment_line(&tile("image/jpeg", 900)), "[image 900 B jpeg]");
        assert_eq!(attachment_line(&tile("text/plain", 2_048)), "[file 2 KB a.txt]");
        assert_eq!(unbase64("aGk="), Some(b"hi".to_vec()));
        assert_eq!(unbase64("aGk"), Some(b"hi".to_vec()));
        assert_eq!(unbase64("a$=="), None);
    }

    #[gpui_kit::test]
    fn a_fork_says_where_it_came_from_on_one_line(cx: &mut TestAppContext) {
        let mut events = vec![
            base("session.start", 1, json!({"prompt": "fix it", "copiedFrom": "th-source"})),
            base("session.delta", 2, json!({"kind": "text", "text": "Done.", "copiedFrom": "th-source"})),
            base("session.done", 3, json!({"result": {"status": "completed"}, "copiedFrom": "th-source"})),
            base("session.end", 4, json!({"exitCode": 0, "sawResult": true, "copiedFrom": "th-source"})),
        ];
        events.push(base("session.start", 5, json!({"prompt": "and now", "turnId": "u2"})));
        let (view, cx) = window(events, cx);
        assert!(cx.debug_bounds("fork").is_none(), "the rule stands before the listing names its thread");
        let at = ThreadAt { workspace: "w0".into(), thread: "th-source".into() };
        let source = transcript::ForkSource { title: "Fix the build".into(), at: Some(at) };
        listed(&view, Listing { source: Some(source), ..Listing::default() }, cx);
        let row = frame(&view, cx, |k| matches!(k, RowKind::Fork { .. }));
        let line = bounds(cx, "fork");
        assert_eq!((line.size.height, row.size.height), (px(24.), px(24. + 16.)));
        assert!(view.read_with(cx, |v, _| v.rows().iter().any(|r| r.text == "Forked from Fix the build")));
        ROUTED.with(|r| r.borrow_mut().clear());
        let at = bounds(cx, "fork-open").center();
        cx.simulate_click(at, Modifiers::none());
        cx.run_until_parked();
        assert_eq!(routed(), [Route::Thread { workspace_id: "w0".into(), thread_id: "th-source".into() }]);
    }

    fn listed(view: &Entity<ThreadView>, listing: Listing, cx: &mut VisualTestContext) {
        view.update(cx, |v, cx| v.set_listing(Listing { thread: "t".into(), ..listing }, cx));
        cx.run_until_parked();
    }

    fn routed() -> Vec<Route> {
        ROUTED.with(|r| r.borrow().clone())
    }

    fn child(thread: &str, kind: Kind, part: ChildPart) -> Listed {
        let at = ThreadAt { workspace: "w2".into(), thread: thread.into() };
        Listed { harness: "codex".into(), ..Listed::thread(at, &format!("Thread {thread}"), kind, part) }
    }

    fn spawned() -> Vec<SessionEvent> {
        vec![
            base("session.start", 1, json!({"prompt": "go"})),
            base(
                "session.delta",
                2,
                json!({"kind": "tool_use", "toolName": "Task", "toolUseId": "t", "text": "{\"description\":\"Survey\"}"}),
            ),
            base("session.subagent", 3, json!({"task": "k1", "state": "running", "parentToolUseId": "t", "title": "Survey it"})),
            base(
                "session.delta",
                4,
                json!({"kind": "tool_use", "toolName": "mcp__wsp__run", "toolUseId": "r", "text": "{\"title\":\"Fix docs\"}"}),
            ),
            base("session.delta", 5, json!({"kind": "tool_result", "toolUseId": "r", "text": "{\"threadId\":\"th1\"}"})),
        ]
    }

    #[gpui_kit::test]
    fn a_spawn_row_takes_its_state_off_the_listing_and_opens_its_child(cx: &mut TestAppContext) {
        let (view, cx) = window(spawned(), cx);
        let child_row = |view: &Entity<ThreadView>, cx: &mut VisualTestContext| {
            view.read_with(cx, |v, _| {
                v.rows().iter().find_map(|r| match &r.kind {
                    RowKind::Spawn { children } => children.iter().find(|c| !c.subagent).cloned(),
                    _ => None,
                })
            })
        };
        assert!(child_row(&view, cx).is_none(), "a child the listing does not hold stands as its call");
        let subagents = Listing::holding(&spawned()).subagents;
        listed(
            &view,
            Listing {
                workspace: Some("w".into()),
                children: vec![child("th1", Kind::NeedsYou, ChildPart::Live)],
                subagents,
                ..Listing::default()
            },
            cx,
        );
        assert_eq!(child_row(&view, cx).and_then(|c| c.listed).map(|l| l.kind), Some(Kind::NeedsYou));
        let column = bounds(cx, "spawn");
        ROUTED.with(|r| r.borrow_mut().clear());
        // The subagent's row first, then the thread's, 36 px each.
        cx.simulate_click(column.origin + gpui_kit::point(px(60.), px(18.)), Modifiers::none());
        cx.run_until_parked();
        cx.simulate_click(column.origin + gpui_kit::point(px(60.), px(36. + 18.)), Modifiers::none());
        cx.run_until_parked();
        assert_eq!(
            routed(),
            [Route::Screen("subagent:w/t/t".into()), Route::Thread { workspace_id: "w2".into(), thread_id: "th1".into() }]
        );
    }

    #[gpui_kit::test]
    fn open_its_page_moves_to_the_subagents_page(cx: &mut TestAppContext) {
        let (no_lead, cx) = window(subagent(), cx);
        assert!(cx.debug_bounds("subagent-page").is_none(), "Open its page stands before the lead's page has an address");
        drop(no_lead);
        let lead = Listing { thread: "t".into(), workspace: Some("w".into()), ..Listing::default() };
        let view = cx.new(ThreadView::new);
        cx.update(|window, cx| window.replace_root(cx, |window, cx| Root::new(view.clone(), window, cx)));
        view.update(cx, |v, cx| {
            v.show("t", cx);
            v.set_listing(lead, cx);
            v.load(subagent(), cx);
        });
        cx.run_until_parked();
        ROUTED.with(|r| r.borrow_mut().clear());
        let at = bounds(cx, "subagent-page").center();
        cx.simulate_click(at, Modifiers::none());
        cx.run_until_parked();
        assert_eq!(routed(), [Route::Screen("subagent:w/t/t".into())]);
        assert!(
            view.read_with(cx, |v, _| v.rows().iter().any(|r| matches!(r.kind, RowKind::Subagent { open: false, .. }))),
            "the press opened the fold too"
        );
    }

    fn sub_row(sub: serde_json::Value) -> serde_json::Value {
        json!({"id": "s1", "workspaceId": "w", "harness": "claude", "status": "completed", "threadId": "t", "startedAt": 1.0, "sub": sub})
    }

    #[gpui_kit::test]
    fn a_failed_start_says_why_under_its_message_with_try_again(cx: &mut TestAppContext) {
        let (view, cx) = window(vec![base("session.start", 1, json!({"prompt": "go"}))], cx);
        assert!(cx.debug_bounds("start-failed").is_none());
        let spawn = json!({"startedAt": 1.0, "log": ["asked boat"], "asked": "go", "failed": {"step": "machine", "said": "the cloud refused the key", "fix": "Add it again."}});
        let listing = Listing::of(&[sub_row(json!({"state": "released", "on": "box", "spawn": spawn}))], "t", 10.0);
        let failed = listing.start_failed.clone().expect("the listing kept no failed start");
        assert_eq!((failed.on.as_str(), failed.step.as_str()), ("Boat", "Asking Boat for a machine"));
        assert_eq!(listing.strip, None, "a restart that failed is said by its line alone");
        listed(&view, listing, cx);
        let person = frame(&view, cx, |k| matches!(k, RowKind::Person { .. }));
        let line = bounds(cx, "start-failed");
        assert!(line.top() >= person.bottom(), "the line stands under the message it was for");
        assert!(cx.debug_bounds("spawn-steps").is_some(), "a failed start opens on its step");
        let at = bounds(cx, "spawn-line").center();
        cx.simulate_click(at, Modifiers::none());
        cx.run_until_parked();
        assert!(cx.debug_bounds("spawn-steps").is_none(), "the chevron did not shut the steps");
    }

    #[gpui_kit::test]
    fn the_threads_section_lists_the_threads_this_one_opened(cx: &mut TestAppContext) {
        let (view, cx) = window(plan("Plan."), cx);
        let row = |id: &str, parent: &str, status: &str| json!({"id": id, "workspaceId": "w2", "harness": "codex", "status": status, "threadId": id, "parentThreadId": parent, "startedAt": 5.0, "harnessTitle": format!("Thread {id}")});
        let rows = [
            json!({"id": "s0", "workspaceId": "w", "harness": "claude", "status": "completed", "threadId": "t", "startedAt": 1.0}),
            row("th1", "t", "running"),
            row("th2", "t", "completed"),
            row("th3", "elsewhere", "running"),
        ];
        let listing = Listing::of(&rows, "t", 10.0);
        let ids: Vec<&str> = listing.children.iter().map(|c| c.at.thread.as_str()).collect();
        assert_eq!(ids, ["th1", "th2"]);
        listed(&view, listing, cx);
        let head = bounds(cx, "threads-head");
        assert_eq!(head.size.height, px(32.));
        let live = bounds(cx, "tree-row-th1");
        assert_eq!((live.top() - head.bottom(), live.size.height), (px(0.), px(36.)));
        ROUTED.with(|r| r.borrow_mut().clear());
        cx.simulate_click(live.center(), Modifiers::none());
        cx.run_until_parked();
        assert_eq!(routed(), [Route::Thread { workspace_id: "w2".into(), thread_id: "th1".into() }]);
        cx.simulate_click(head.center(), Modifiers::none());
        cx.run_until_parked();
        assert!(cx.debug_bounds("threads-fold-Finished").is_none(), "the head did not shut the section");
        assert!(cx.debug_bounds("threads-peek").is_some(), "the shut section shows no peek");
    }

    const HOUR: f64 = 3_600_000.;
    const NOW: f64 = 1_800_000_000_000.;

    /// A lead `t` with a working child `th1` whose own child `th1a` asks, a finished child `th2`, a child `th3` quiet
    /// for three hours, a child `th4` on another computer, and a running subagent of the lead's agent.
    fn tree_rows() -> Vec<serde_json::Value> {
        let row = |id: &str, parent: &str, status: &str, started: f64| {
            json!({"id": format!("s-{id}"), "workspaceId": format!("w-{id}"), "harness": "codex", "status": status, "threadId": id,
                "parentThreadId": parent, "startedAt": started, "harnessTitle": format!("Thread {id}"), "computerName": "here"})
        };
        let quiet = |mut r: serde_json::Value, ended: f64| {
            r["endedAt"] = json!(ended);
            r["readAt"] = json!(ended);
            r
        };
        let mut asking = row("th1a", "th1", "running", NOW - 60_000.);
        asking["asking"] = json!("Which branch?");
        let mut away = quiet(row("th4", "t", "completed", NOW - 2. * 60_000.), NOW - 60_000.);
        away["computerName"] = json!("studio");
        away["lastLine"] = json!("All done.");
        vec![
            json!({"id": "s-t", "workspaceId": "w", "harness": "claude", "status": "running", "threadId": "t", "startedAt": NOW - HOUR,
                "computerName": "here", "subagents": [{"id": "k1", "title": "Survey it", "state": "running", "parentToolUseId": "call-1", "startedAt": NOW - 30_000.}]}),
            row("th1", "t", "running", NOW - 10. * 60_000.),
            asking,
            quiet(row("th2", "t", "completed", NOW - 20. * 60_000.), NOW - 5. * 60_000.),
            quiet(row("th3", "t", "completed", NOW - 4. * HOUR), NOW - 3. * HOUR),
            away,
        ]
    }

    fn tree_listing(settle: Option<f64>, preferences: serde_json::Value) -> Listing {
        let rows: Vec<crate::screens::sidebar::model::Row> =
            tree_rows().into_iter().filter_map(crate::screens::sidebar::model::Row::owned).collect();
        Listing::listed(&rows, "t", NOW, settle, &preferences, "here")
    }

    #[test]
    fn the_tree_nests_children_with_subagents_among_them_in_their_parts() {
        let listing = tree_listing(Some(2. * HOUR), json!({}));
        let top: Vec<(&str, ChildPart)> =
            listing.children.iter().map(|c| (c.subagent.as_deref().unwrap_or(c.at.thread.as_str()), c.part)).collect();
        // Live by the most pressing rank under each, then finished by newest end, then settled.
        assert_eq!(
            top,
            [
                ("th1", ChildPart::Live),
                ("call-1", ChildPart::Live),
                ("th4", ChildPart::Finished),
                ("th2", ChildPart::Finished),
                ("th3", ChildPart::Settled)
            ]
        );
        let th1 = &listing.children[0];
        assert_eq!(
            th1.children.iter().map(|c| c.at.thread.as_str()).collect::<Vec<_>>(),
            ["th1a"],
            "the grandchild stands under its parent"
        );
        assert_eq!(th1.children[0].note.as_deref(), Some("Which branch?"));
        assert_eq!(th1.children[0].kind, Kind::NeedsYou);
        let away = &listing.children[2];
        assert_eq!(
            (away.computer.as_deref(), away.note.as_deref()),
            (Some("studio"), Some("All done.")),
            "the lead's computer is left out, another named"
        );
        assert_eq!(listing.children[1].kind, Kind::Working);
        assert_eq!(
            listing.counts(),
            transcript::TreeCounts { needs_you: 1, failed: 0, working: 2, working_subagents: 1, waiting: 0, finished: 2 }
        );
        assert_eq!(listing.most_pressing().map(|n| n.at.thread.as_str()), Some("th1a"), "the peek is the most pressing at any depth");
        assert_eq!(listing.finished(), (vec!["th4".to_owned(), "th2".to_owned()], 2));
    }

    #[test]
    fn a_child_settles_after_the_persons_settle_after() {
        let part = |settle: Option<f64>| tree_listing(settle, json!({})).children.iter().find(|c| c.at.thread == "th3").map(|c| c.part);
        assert_eq!(part(Some(2. * HOUR)), Some(ChildPart::Settled));
        assert_eq!(part(None), Some(ChildPart::Finished), "settleAfter never kept a quiet child out of Settled");
        assert_eq!(part(Some(86_400_000.)), Some(ChildPart::Finished), "settleAfter 1d settled a child quiet for 3h");
        let th2 = |settle| tree_listing(Some(settle), json!({})).children.iter().find(|c| c.at.thread == "th2").map(|c| c.part);
        assert_eq!(th2(15. * 60_000.), Some(ChildPart::Finished), "15 minutes settled a child quiet for 5");
        assert_eq!(th2(60_000.), Some(ChildPart::Settled), "a minute's settle left a child quiet for 5 out of Settled");
    }

    fn tree_window(listing: Listing, cx: &mut TestAppContext) -> (Entity<ThreadView>, &mut VisualTestContext) {
        RAN.with(|r| r.borrow_mut().clear());
        let (view, cx) = window(vec![base("session.start", 1, json!({"prompt": "go"}))], cx);
        listed(&view, listing, cx);
        (view, cx)
    }

    fn ran() -> Vec<String> {
        RAN.with(|r| r.borrow().clone())
    }

    #[gpui_kit::test]
    fn a_section_the_preferences_shut_says_its_counts_over_a_fading_peek(cx: &mut TestAppContext) {
        let (view, cx) = tree_window(tree_listing(Some(2. * HOUR), json!({"threadsShut": {"t": true}})), cx);
        assert!(view.read_with(cx, |v, _| v.rows().iter().any(|r| matches!(r.kind, RowKind::Threads { open: false, .. }))));
        let head = bounds(cx, "threads-head");
        assert!(cx.debug_bounds("tree-counts").is_some(), "the shut head says no counts");
        for count in ["tree-count-needs-you", "tree-count-working"] {
            assert!(cx.debug_bounds(count).is_some(), "{count} not drawn");
        }
        assert!(cx.debug_bounds("tree-count-finished").is_none(), "finished is said only with nothing live");
        assert!(cx.debug_bounds("threads-settle").is_none() && cx.debug_bounds("threads-fold-Finished").is_none());
        let peek = bounds(cx, "threads-peek");
        assert_eq!((peek.top(), peek.size.height), (head.bottom(), px(48.)), "the peek is the asking grandchild's two-line row");
        cx.simulate_click(peek.center(), Modifiers::none());
        cx.run_until_parked();
        assert_eq!(ran(), ["shut t false"], "a press on the peek did not open the section through the preferences");
        assert!(cx.debug_bounds("threads-fold-Finished").is_some(), "the section did not open at once");
    }

    #[gpui_kit::test]
    fn the_open_head_settles_the_finished_part_and_shuts_through_the_preferences(cx: &mut TestAppContext) {
        let (_view, cx) = tree_window(tree_listing(Some(2. * HOUR), json!({})), cx);
        assert!(cx.debug_bounds("tree-counts").is_none(), "the open head says counts");
        let (head, toggle, settle) = (bounds(cx, "threads-head"), bounds(cx, "threads-toggle"), bounds(cx, "threads-settle"));
        assert!(toggle.right() <= settle.left() && settle.right() < head.right(), "Settle stands between the toggle and the chevron");
        assert_eq!(settle.size.height, px(24.));
        cx.simulate_click(settle.center(), Modifiers::none());
        cx.run_until_parked();
        assert_eq!(ran(), ["settle th4 th2"]);
        let at = bounds(cx, "threads-chevron").center();
        cx.simulate_click(at, Modifiers::none());
        cx.run_until_parked();
        assert_eq!(ran(), ["settle th4 th2", "shut t true"]);
        assert!(cx.debug_bounds("threads-peek").is_some(), "the chevron did not shut the section at once");
    }

    #[gpui_kit::test]
    fn a_grandchild_hangs_on_the_rail_under_its_parent(cx: &mut TestAppContext) {
        let (_view, cx) = tree_window(tree_listing(Some(2. * HOUR), json!({})), cx);
        let (parent, kid, list) = (bounds(cx, "tree-row-th1"), bounds(cx, "tree-row-th1a"), bounds(cx, "tree-list"));
        // `ml-3.5` then the item's `pl-2`: the grandchild's row starts 22 px in from its parent's.
        assert_eq!((list.left() - parent.left(), kid.left() - parent.left()), (px(14.), px(22.)));
        assert_eq!(list.top(), parent.bottom(), "the rail list stands right under its parent's row");
        assert!(cx.debug_bounds("tree-sub-t-call-1").is_some(), "the lead's subagent is not among the children");
    }

    #[gpui_kit::test]
    fn a_childs_state_act_runs_through_the_sidebar_and_stop_takes_two_presses(cx: &mut TestAppContext) {
        let (view, cx) = tree_window(tree_listing(Some(2. * HOUR), json!({})), cx);
        let stop = bounds(cx, "tree-row-th1-act-stop");
        cx.simulate_click(stop.center(), Modifiers::none());
        cx.run_until_parked();
        assert!(ran().is_empty(), "one press stopped the thread");
        assert!(routed_after_clear().is_empty(), "a press on the act opened the thread");
        let at = bounds(cx, "tree-row-th1-act-stop").center();
        cx.simulate_click(at, Modifiers::none());
        cx.run_until_parked();
        assert_eq!(ran(), ["stop s-th1"]);
        let at = bounds(cx, "threads-fold-Finished").center();
        cx.simulate_click(at, Modifiers::none());
        let at = bounds(cx, "threads-fold-Settled").center();
        cx.simulate_click(at, Modifiers::none());
        cx.run_until_parked();
        let at = bounds(cx, "tree-row-th2-act-settle").center();
        cx.simulate_click(at, Modifiers::none());
        cx.run_until_parked();
        let at = bounds(cx, "tree-row-th3-act-restore").center();
        cx.simulate_click(at, Modifiers::none());
        cx.run_until_parked();
        assert_eq!(ran(), ["stop s-th1", "settle th2", "restore th3"]);
        assert!(cx.debug_bounds("tree-row-th1a-act-settle").is_none(), "an asking thread offers Settle");
        assert!(cx.debug_bounds("tree-row-th2-act-more").is_some(), "a thread's row offers no More");
        let _ = view;
    }

    #[gpui_kit::test]
    fn a_quiet_thread_whose_child_works_offers_stop_and_holds_settle(cx: &mut TestAppContext) {
        let row = |id: &str, parent: &str, status: &str| {
            json!({"id": format!("s-{id}"), "workspaceId": "w2", "harness": "codex", "status": status, "threadId": id,
                "parentThreadId": parent, "startedAt": NOW - 60_000., "endedAt": NOW - 30_000., "readAt": NOW})
        };
        let lead =
            json!({"id": "s-t", "workspaceId": "w", "harness": "claude", "status": "completed", "threadId": "t", "startedAt": NOW - HOUR});
        let mut kid = row("th5a", "th5", "running");
        kid.as_object_mut().unwrap().remove("endedAt");
        let rows: Vec<crate::screens::sidebar::model::Row> =
            [lead, row("th5", "t", "completed"), kid].into_iter().filter_map(crate::screens::sidebar::model::Row::owned).collect();
        let listing = Listing::listed(&rows, "t", NOW, Some(2. * HOUR), &json!({}), "");
        let th5 = &listing.children[0];
        assert_eq!((th5.running, th5.working), (false, true), "a working child left its quiet parent not working");
        let (_view, cx) = tree_window(listing, cx);
        assert!(cx.debug_bounds("tree-row-th5-act-stop").is_some(), "no Stop on a thread whose child works");
        assert!(cx.debug_bounds("tree-row-th5-act-settle").is_none(), "Settle offered while a thread under it works");
    }

    #[gpui_kit::test]
    fn a_right_click_on_a_row_opens_the_acts_more_holds(cx: &mut TestAppContext) {
        let (view, cx) = tree_window(tree_listing(Some(2. * HOUR), json!({})), cx);
        let at = bounds(cx, "threads-fold-Finished").center();
        cx.simulate_click(at, Modifiers::none());
        cx.run_until_parked();
        let row = bounds(cx, "tree-row-th2").center();
        cx.simulate_event(gpui_kit::MouseDownEvent {
            button: gpui_kit::MouseButton::Right,
            position: row,
            modifiers: Modifiers::none(),
            click_count: 1,
            first_mouse: false,
        });
        cx.run_until_parked();
        assert!(view.read_with(cx, |v, _| v.row_menu.is_open()), "a right-click opened no menu");
        // Send a message is held, so the second row, Settle thread, takes the pick.
        cx.simulate_keystrokes("down down enter");
        cx.run_until_parked();
        assert_eq!(ran(), ["settle th2"]);
        assert!(routed_after_clear().is_empty(), "the right-click opened the thread");
    }

    #[gpui_kit::test]
    fn a_later_arm_of_stop_keeps_its_own_two_seconds(cx: &mut TestAppContext) {
        let (_view, cx) = tree_window(tree_listing(Some(2. * HOUR), json!({})), cx);
        let stop = bounds(cx, "tree-row-th1-act-stop").center();
        cx.simulate_click(stop, Modifiers::none());
        cx.run_until_parked();
        // The pointer leaves the row, which disarms, and comes back to arm again a second later.
        cx.simulate_mouse_move(gpui_kit::point(px(1.), px(1.)), None, Modifiers::none());
        cx.run_until_parked();
        cx.executor().advance_clock(std::time::Duration::from_millis(1_000));
        cx.simulate_mouse_move(stop, None, Modifiers::none());
        cx.simulate_click(stop, Modifiers::none());
        cx.run_until_parked();
        // The first arm's two seconds run out while the second still has one left.
        cx.executor().advance_clock(std::time::Duration::from_millis(1_100));
        cx.run_until_parked();
        cx.simulate_click(stop, Modifiers::none());
        cx.run_until_parked();
        assert_eq!(ran(), ["stop s-th1"], "the first arm's lapse cut the second arm short");
    }

    fn routed_after_clear() -> Vec<Route> {
        let out = routed();
        ROUTED.with(|r| r.borrow_mut().clear());
        out
    }

    #[gpui_kit::test]
    fn the_strips_push_again_asks_the_thread_and_an_act_with_no_screen_is_held(cx: &mut TestAppContext) {
        RAN.with(|r| r.borrow_mut().clear());
        let row = json!({"id": "s1", "workspaceId": "w", "harness": "claude", "status": "completed", "threadId": "t", "startedAt": 1.0,
            "sub": {"state": "running", "on": "box", "branch": "wsp/fix", "held": {"files": 2, "commits": 1}}});
        let rows: Vec<crate::screens::sidebar::model::Row> =
            [row].into_iter().filter_map(crate::screens::sidebar::model::Row::owned).collect();
        let strip_of = Listing::listed(&rows, "t", 10.0, None, &json!({}), "Mac mini").strip.expect("no strip for work not pushed");
        assert_eq!(strip_of.acts, [transcript::StripAct::CopyHome("Mac mini".into()), transcript::StripAct::PushAgain("wsp/fix".into())]);
        let view = cx.new(ThreadView::new);
        let cx = piece(cx, move |_, cx| strip(&view, &strip_of, cx));
        let acts = cx.debug_bounds("sub-strip-not-pushed").expect("the strip");
        let _ = acts;
        let (copy, push) = (bounds(cx, "sub-strip-act-0"), bounds(cx, "sub-strip-act-1"));
        assert!(push.left() > copy.right(), "the two acts stand side by side under the words");
        cx.simulate_click(copy.center(), Modifiers::none());
        cx.simulate_click(push.center(), Modifiers::none());
        cx.run_until_parked();
        assert_eq!(ran(), ["send Push wsp/fix again, and say what the remote answers."], "Push again asked nothing, or the held copy ran");
    }

    #[gpui_kit::test]
    fn the_listing_follows_the_sidebars_rows_and_the_persons_preferences(cx: &mut TestAppContext) {
        use crate::screens::sidebar::host::{Change, Snapshot};
        cx.update(|cx| {
            gpui_kit::init(cx);
            crate::ui::init(crate::ui::Side::Light, cx);
        });
        let sidebar = cx.new(crate::sidebar::Sidebar::new);
        cx.update(|cx| crate::screens::terminal::read_workspaces_from(&sidebar, cx));
        let snapshot = Snapshot { sessions: tree_rows(), preferences: json!({"settleAfter": "never"}), ..Snapshot::default() };
        sidebar.update(cx, |s, cx| s.apply(Change::Whole(Box::new(snapshot)), cx));
        let view = cx.new(ThreadView::new);
        let (_, cx) = cx.add_window_view(|window, cx| Root::new(view.clone(), window, cx));
        view.update(cx, |v, cx| {
            v.show("t", cx);
            v.relist(cx);
        });
        cx.run_until_parked();
        let part = |cx: &mut VisualTestContext, id: &str| {
            view.read_with(cx, |v, _| v.listing().and_then(|l| l.children.iter().find(|c| c.at.thread == id).map(|c| (c.part, c.kind))))
        };
        assert_eq!(part(cx, "th3").map(|p| p.0), Some(ChildPart::Finished), "the sidebar's settleAfter never did not reach the listing");
        // A row the host pushes reaches the listing with no read of its own.
        let mut asks = tree_rows().remove(4);
        asks["status"] = json!("running");
        asks["asking"] = json!("Deploy?");
        sidebar.update(cx, |s, cx| s.apply(Change::Row { id: "s-th3".into(), row: Some(asks) }, cx));
        cx.run_until_parked();
        assert_eq!(part(cx, "th3"), Some((ChildPart::Live, Kind::NeedsYou)), "the listing did not follow the sidebar's row");
        sidebar.update(cx, |s, cx| s.apply(Change::Preferences(json!({"threadsShut": {"t": true}})), cx));
        cx.run_until_parked();
        assert!(view.read_with(cx, |v, _| v.listing().is_some_and(|l| l.shut)), "threadsShut in the preferences did not shut the section");
        // A switch the sidebar has nothing new for still reads the new thread's listing.
        view.update(cx, |v, cx| {
            v.show("th1", cx);
            v.relist(cx);
        });
        cx.run_until_parked();
        let lead = view.read_with(cx, |v, _| v.listing().map(|l| (l.thread.clone(), l.children.len())));
        assert_eq!(lead, Some(("th1".to_owned(), 1)), "the listing stayed on the thread switched away from");
    }

    #[gpui_kit::test]
    fn an_open_fold_pages_twenty_rows_and_n_more_adds_twenty(cx: &mut TestAppContext) {
        let lead =
            json!({"id": "s-t", "workspaceId": "w", "harness": "claude", "status": "completed", "threadId": "t", "startedAt": NOW - HOUR});
        let mut rows = vec![lead];
        for i in 0..25 {
            rows.push(json!({"id": format!("s{i}"), "workspaceId": "w2", "harness": "codex", "status": "completed", "threadId": format!("c{i}"),
                "parentThreadId": "t", "startedAt": NOW - 600_000. + f64::from(i), "endedAt": NOW - 300_000. + f64::from(i), "readAt": NOW}));
        }
        let rows: Vec<crate::screens::sidebar::model::Row> =
            rows.into_iter().filter_map(crate::screens::sidebar::model::Row::owned).collect();
        let (view, cx) = tree_window(Listing::listed(&rows, "t", NOW, Some(2. * HOUR), &json!({}), ""), cx);
        let at = bounds(cx, "threads-fold-Finished").center();
        cx.simulate_click(at, Modifiers::none());
        cx.run_until_parked();
        let count = |_: &Entity<ThreadView>, cx: &mut VisualTestContext| {
            (0..25).filter(|i| cx.debug_bounds(Box::leak(format!("tree-row-c{i}").into_boxed_str())).is_some()).count()
        };
        assert_eq!(count(&view, cx), 20, "an open fold draws twenty rows");
        let more = bounds(cx, "threads-more-Finished");
        cx.simulate_click(more.center(), Modifiers::none());
        cx.run_until_parked();
        assert_eq!(count(&view, cx), 25, "N more did not add the rest");
        assert!(cx.debug_bounds("threads-more-Finished").is_none());
    }

    #[gpui_kit::test]
    fn a_thread_behind_another_s_question_draws_that_question(cx: &mut TestAppContext) {
        let (view, cx) = window(vec![base("session.start", 1, json!({"prompt": "go"}))], cx);
        let waiting = json!({"threadId": "th9", "workspaceId": "w", "sessionId": "s9", "title": "Lint pass", "prompt": {
            "askId": "a9", "toolName": "Bash", "input": "{\"command\":\"rm -rf out\"}",
            "options": [{"id": "y", "label": "Allow", "effect": "allow"}, {"id": "n", "label": "Deny", "effect": "deny"}]}});
        let own = json!({"id": "s1", "workspaceId": "w", "harness": "claude", "status": "running", "threadId": "t", "startedAt": 1.0, "waitingOn": waiting});
        listed(&view, Listing::of(&[own], "t", 10.0), cx);
        let rows = view.read_with(cx, |v, _| v.rows().to_vec());
        assert!(rows.iter().any(|r| r.text == "Waiting for you"), "{rows:?}");
        assert!(!rows.iter().any(|r| matches!(r.kind, RowKind::Thinking)));
        let asked = rows.last().unwrap();
        assert!(
            matches!(&asked.kind, RowKind::Permission { session, asker: Some(a), .. } if session == "s9" && a == "Lint pass asks; this thread waits on the answer")
        );
        assert!(cx.debug_bounds("options").is_some(), "the borrowed question offers no answer");
    }

    #[gpui_kit::test]
    fn the_machine_line_and_the_starting_notice_stand_at_the_foot(cx: &mut TestAppContext) {
        let (view, cx) = window(plan("Plan."), cx);
        assert!(cx.debug_bounds("machine-line").is_none());
        let starting = Some(transcript::agent_starting_line("Claude Code", false));
        listed(&view, Listing { machine: Some("Out of memory, 3.8 of 4 GB".into()), starting, ..Listing::default() }, cx);
        let rows = view.read_with(cx, |v, _| v.rows().to_vec());
        let texts: Vec<&str> = rows.iter().rev().take(2).map(|r| r.text.as_str()).collect();
        assert_eq!(texts, ["Out of memory, 3.8 of 4 GB", "Starting Claude Code"]);
        assert_eq!(bounds(cx, "machine-line").size.height, px(24.));
    }

    type Build = Box<dyn Fn(&mut Window, &mut App) -> AnyElement>;

    struct Piece(Build);

    impl Render for Piece {
        fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            div().relative().size_full().child((self.0)(window, cx))
        }
    }

    fn piece(cx: &mut TestAppContext, draw: impl Fn(&mut Window, &mut App) -> AnyElement + 'static) -> &mut VisualTestContext {
        cx.update(|cx| {
            gpui_kit::init(cx);
            crate::ui::init(crate::ui::Side::Light, cx);
        });
        let (_, cx) = cx.add_window_view(|_, _| Piece(Box::new(draw)));
        cx.simulate_resize(size(px(900.), px(700.)));
        cx.run_until_parked();
        cx
    }

    #[gpui_kit::test]
    fn the_setup_card_stands_centred_with_every_step_done(cx: &mut TestAppContext) {
        let setup = Setup { name: "spoo-landing".into(), steps: vec!["Making the copy".into(), "Ready".into()] };
        let cx = piece(cx, move |_, _| setup_card(&setup, px(120.)).into_any_element());
        let room = bounds(cx, "setting-up-room");
        assert_eq!(room.bottom(), px(700. - 120.), "the room ends at the composer");
        assert!(cx.debug_bounds("step-row").is_some() || cx.debug_bounds("card").is_some());
    }

    #[gpui_kit::test]
    fn a_fork_on_its_way_says_loading_in_the_middle(cx: &mut TestAppContext) {
        let cx = piece(cx, |_, cx| loading(px(100.), cx).into_any_element());
        let shown = bounds(cx, "transcript-loading");
        assert_eq!((shown.top(), shown.size.height), (px(0.), px(700.)));
    }

    #[gpui_kit::test]
    fn a_sub_thread_waiting_for_a_machine_says_so_over_its_composer(cx: &mut TestAppContext) {
        let row = json!({"id": "s1", "workspaceId": "w", "harness": "claude", "status": "running", "threadId": "t", "startedAt": 1.0,
            "capped": {"placeId": "p", "place": "box", "running": 3, "atOnce": 3}, "sub": {"state": "waiting", "on": "box", "spawn": {"startedAt": 1.0, "log": []}}});
        let strip_of = Listing::of(&[row], "t", 10.0).strip.expect("no strip for a capped sub-thread");
        assert_eq!(strip_of.title, "Waiting for a machine on Boat");
        assert_eq!(strip_of.note, "3 of 3 machines are running. It starts on its own when one frees.");
        assert_eq!(strip_of.acts, [transcript::StripAct::Raise]);
        let view = cx.new(ThreadView::new);
        let cx = piece(cx, move |_, cx| strip(&view, &strip_of, cx));
        assert!(cx.debug_bounds("sub-strip-waiting-machines").is_some());
        assert_eq!(bounds(cx, "sub-strip-title").size.height, px(24.));
    }

    #[gpui_kit::test]
    fn a_subagent_page_is_the_lead_cut_to_its_lines(cx: &mut TestAppContext) {
        let (view, cx) = window(subagent(), cx);
        let task = "{\"description\":\"Survey the code\",\"prompt\":\"look\"}";
        assert!(task.contains("look"));
        view.update(cx, |v, cx| v.set_page(Some("t".into()), cx));
        cx.run_until_parked();
        let rows = view.read_with(cx, |v, _| v.rows().to_vec());
        let shown: Vec<&str> = rows.iter().map(|r| r.text.as_str()).collect();
        assert_eq!(shown.first(), Some(&"look"), "{shown:?}");
        assert!(matches!(rows[0].kind, RowKind::Person { .. }));
        assert!(rows.iter().any(|r| matches!(r.kind, RowKind::Agent { .. }) && r.text == "Reading."), "{shown:?}");
        assert!(
            !rows.iter().any(|r| matches!(r.kind, RowKind::Working { .. } | RowKind::Fold { .. } | RowKind::Subagent { .. })),
            "{shown:?}"
        );
        let page = view.read_with(cx, |v, _| v.transcript().page_of("t")).expect("the page knows no subagent");
        assert_eq!((page.title.as_str(), page.state.as_str()), ("Survey the code", "done"));
        assert_eq!(super::super::subagent::ended_line(&page).as_deref(), Some("Worked for 1m 5s"));
        assert_eq!(
            super::super::subagent::parse("w/t/call"),
            Some((ThreadAt { workspace: "w".into(), thread: "t".into() }, "call".into()))
        );
        view.update(cx, |v, cx| v.set_page(None, cx));
        assert!(view.read_with(cx, |v, _| v.rows().iter().any(|r| matches!(r.kind, RowKind::Subagent { .. }))));
    }

    #[gpui_kit::test]
    fn find_paints_its_match_in_a_subagents_line(cx: &mut TestAppContext) {
        let mut events = subagent();
        events[2] = base("session.delta", 3, json!({"kind": "text", "text": "Reading the kiwi notes.", "parentToolUseId": "t"}));
        let (view, cx) = window(events, cx);
        view.update(cx, |v, cx| v.toggle("subagent:t", cx));
        cx.update(|window, cx| view.update(cx, |v, cx| find::open(v, window, cx)));
        cx.simulate_input("kiwi");
        PAINTED.with(|p| p.borrow_mut().clear());
        view.update(cx, |_, cx| cx.notify());
        cx.run_until_parked();
        let painted = PAINTED.with(|p| p.borrow().clone());
        assert!(
            painted.iter().any(|(key, part, spans)| key == "subagent:t"
                && *part == find::subagent_part(0, false)
                && spans.len() == 1
                && spans[0] == (12..16)),
            "the subagent's line was drawn without its match: {painted:?}"
        );
    }

    #[gpui_kit::test]
    fn the_rows_follow_a_listing_that_changes_and_a_closed_ask_answers_nothing(cx: &mut TestAppContext) {
        SENT.with(|s| s.borrow_mut().clear());
        let (view, cx) = window(spawned(), cx);
        let waiting = json!({"threadId": "th9", "workspaceId": "w", "sessionId": "s9", "title": "Lint pass", "prompt": {
            "askId": "a9", "toolName": "Bash", "input": "{\"command\":\"ls\"}", "options": [{"id": "y", "label": "Allow", "effect": "allow"}]}});
        let kid = |status: &str| json!({"id": "c1", "workspaceId": "w2", "harness": "codex", "status": status, "threadId": "th1", "parentThreadId": "t", "startedAt": 5.0, "endedAt": 6.0});
        let own = |waiting: Option<&serde_json::Value>| {
            let mut own = json!({"id": "s1", "workspaceId": "w", "harness": "claude", "status": "running", "threadId": "t", "startedAt": 1.0,
                "subagents": [{"id": "k1", "title": "Survey it", "state": "running", "parentToolUseId": "t", "startedAt": 2.0}]});
            if let Some(w) = waiting {
                own["waitingOn"] = w.clone();
            }
            own
        };
        let kind_of = |view: &Entity<ThreadView>, cx: &mut VisualTestContext| {
            view.read_with(cx, |v, _| {
                let spawn = v.rows().iter().find_map(|r| match &r.kind {
                    RowKind::Spawn { children } => children.iter().find_map(|c| c.listed.as_ref().map(|l| l.kind)),
                    _ => None,
                });
                let threads = v.rows().iter().find_map(|r| match &r.kind {
                    RowKind::Threads { children, .. } => children.iter().find(|c| c.subagent.is_none()).map(|c| c.kind),
                    _ => None,
                });
                let asked = v.rows().iter().any(|r| r.key == "waiting-on:th9:a9");
                (spawn, threads, asked)
            })
        };
        listed(&view, Listing::of(&[own(Some(&waiting)), kid("running")], "t", 10.0), cx);
        assert_eq!(kind_of(&view, cx), (Some(Kind::Working), Some(Kind::Working), true));
        listed(&view, Listing::of(&[own(None), kid("completed")], "t", 10.0), cx);
        assert_eq!(kind_of(&view, cx), (Some(Kind::Done), Some(Kind::Done), false), "the rows kept the listing read at open");
        view.update(cx, |v, cx| answer(v, "s9", "a9", "y", cx));
        assert!(SENT.with(|s| s.borrow().is_empty()), "a question answered elsewhere was answered again");
    }

    #[gpui_kit::test]
    fn a_subagents_page_follows_its_fold_before_the_listing(cx: &mut TestAppContext) {
        let lead = Listing {
            thread: "t".into(),
            subagents: vec![transcript::ListedSubagent {
                call: "t".into(),
                title: "Survey the code".into(),
                state: "running".into(),
                model: None,
                asked: None,
                started: 0.,
                ended: None,
                failure: None,
            }],
            ..Listing::default()
        };
        let (view, cx) = window_listed(subagent(), Some(lead), cx);
        let page = view.read_with(cx, |v, _| v.transcript().page_of("t")).unwrap();
        assert_eq!((page.state.as_str(), super::super::subagent::ended_line(&page).as_deref()), ("done", Some("Worked for 1m 5s")));
    }

    #[gpui_kit::test]
    fn the_slate_line_stands_once_under_the_turns_last_reply_and_eases_its_ink(cx: &mut TestAppContext) {
        let slate = |pos, turn: &str, by: &str| {
            base("session.slate", pos, json!({"turnId": turn, "cause": "write", "version": pos, "by": by, "pieces": []}))
        };
        let mut events = changes();
        events.insert(2, slate(10, "u1", "agent"));
        events.insert(3, slate(11, "u1", "agent"));
        events.extend([
            base("session.start", 20, json!({"prompt": "and more", "turnId": "u2"})),
            base("session.delta", 21, json!({"kind": "text", "text": "Also done.", "turnId": "u2"})),
            slate(22, "u2", "person"),
            base("session.done", 23, json!({"result": {"status": "completed"}, "turnId": "u2"})),
            base("session.end", 24, json!({"exitCode": 0, "sawResult": true, "turnId": "u2"})),
        ]);
        let (view, cx) = window(events, cx);
        let rows = view.read_with(cx, |v, _| v.rows().to_vec());
        let lines: Vec<usize> = rows.iter().enumerate().filter(|(_, r)| matches!(r.kind, RowKind::Slate)).map(|(i, _)| i).collect();
        let changed = rows.iter().position(|r| matches!(r.kind, RowKind::ChangedFiles { .. })).unwrap();
        assert_eq!(lines, [changed + 1], "one line under the first turn's reply and its files, none for a person's write");
        let line = bounds(cx, "slate-updated");
        let muted = cx.update(|_, cx| cx.palette().muted_foreground);
        let ink = || SLATE_INK.with(|i| i.get());
        assert_eq!(ink(), Some(muted));
        cx.simulate_mouse_move(line.center(), None, Modifiers::none());
        cx.run_until_parked();
        cx.executor().advance_clock(std::time::Duration::from_millis(60));
        cx.update(|window, cx| window.simulate_next_frame(cx));
        cx.run_until_parked();
        let mid = ink().unwrap();
        let foreground = cx.update(|_, cx| cx.palette().foreground);
        assert!(mid != muted && mid != foreground, "the ink did not ease: {mid:?}");
        cx.executor().advance_clock(std::time::Duration::from_millis(200));
        cx.update(|window, cx| window.simulate_next_frame(cx));
        cx.run_until_parked();
        assert_eq!(ink(), Some(foreground));
    }
}
