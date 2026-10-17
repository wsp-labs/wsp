// SPDX-License-Identifier: AGPL-3.0-only
//! The transcript's cards as the web app draws them: prompts, plans, subagent folds, children, changed files, forks, files.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use gpui_kit::component::{h_flex, v_flex, ActiveTheme, Icon, Sizable as _, WindowExt as _};
use gpui_kit::{
    anchored, deferred, div, img, linear_color_stop, linear_gradient, point, prelude::*, px, Anchor, AnyElement, App, BoxShadow,
    ClipboardItem, Context, FontWeight, Hsla, Image, ImageFormat, ImageSource, ObjectFit, Pixels, SharedString, Window,
};
use wsp_host_client::protocol::{SessionsAnswerRequest, SessionsAttachmentRequest};

use super::markdown;
use super::rows::{icon_slot, SM};
use crate::thread_view::ThreadView;
use crate::transcript::{self, Asked, ChangedFile, Child, FileTile, Glyph as RowGlyph, PromptOption, Row, RowKind};
use crate::ui::{ActivePalette, Button, Glyph, Palette, Status, StatusMark};

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
const PICK_JOIN: &str = "|";

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
        RowKind::Subagent { .. } => px(0.),
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
            subagent(id, title, state.as_deref(), *open, lines, rail_follows(view, ix), &p, cx)
        }
        RowKind::Spawn { children } => spawn(&row.key, children, &p),
        RowKind::ChangedFiles { label, files, others } => changed_files(&row.key, label, files, others, &p, window, cx),
        RowKind::Fork { .. } => fork(&row.text, &p),
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

/// `sentence` in answerPrompt.ts: capitalised, ending in a full stop.
fn sentence(line: &str) -> String {
    let line = if line.ends_with('.') { line.to_owned() } else { format!("{line}.") };
    let mut chars = line.chars();
    chars.next().map(|c| c.to_uppercase().chain(chars).collect()).unwrap_or_default()
}

const TOOL_WORDS: &str = include_str!("../../../../../../packages/protocol/src/words/tools.ts");

/// `ANSWER_WORDS`, read from its source: why the host would not take a pick it answered.
fn answer_words(outcome: &str) -> Option<&'static str> {
    let block = &TOOL_WORDS[TOOL_WORDS.find("export const ANSWER_WORDS")?..];
    block[..block.find("\n};")?].lines().find_map(|line| {
        let (key, words) = line.trim().split_once(": \"")?;
        (key.trim_matches('"') == outcome).then(|| words.strip_suffix("\",")).flatten()
    })
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
    let (asker, body) = match &row.kind {
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
fn refusal(refused: Refusal, p: &Palette) -> impl IntoElement {
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
                .child(div().mt(px(2.)).child(checkbox(ticked.contains(&option.id), p, cx)))
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

/// The question's checkbox, `tone="neutral"`: 16 px, a 4 px radius on the field's edge; ticked, the foreground fill.
fn checkbox(checked: bool, p: &Palette, cx: &App) -> impl IntoElement {
    let ground = if cx.theme().is_dark() { p.input.opacity(0.32) } else { p.background };
    div().debug_selector(|| "checkbox".into()).flex().flex_none().items_center().justify_center().size(px(16.)).rounded(px(4.)).map(|el| {
        if checked {
            el.bg(p.foreground).child(Icon::empty().path(crate::ui::icons::TICK).with_size(px(12.)).text_color(p.background))
        } else {
            el.border_1().border_color(p.input).bg(ground).shadow(p.control_shadow.clone())
        }
    })
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

/// Where a browser puts a download of `name` in `folder`: the name itself, or `name (1).md` and on while it is taken.
pub fn download_path(folder: &Path, name: &str) -> PathBuf {
    let (stem, ext) = name.rsplit_once('.').map_or((name, ""), |(s, e)| (s, e));
    let mut path = folder.join(name);
    let mut n = 1;
    while path.exists() {
        path = folder.join(if ext.is_empty() { format!("{stem} ({n})") } else { format!("{stem} ({n}).{ext}") });
        n += 1;
    }
    path
}

/// Download as markdown: writes the plan into the Downloads folder as a browser would, and says where on the log.
fn download(markdown: &str) {
    let Some(home) = std::env::var_os("HOME") else { return eprintln!("desktop-gpui: no HOME to download the plan into") };
    let folder = PathBuf::from(home).join("Downloads");
    let path = download_path(&folder, &plan_filename(plan_heading(markdown).as_deref()));
    let written = std::fs::create_dir_all(&folder).and_then(|()| std::fs::write(&path, format!("{}\n", markdown.trim_end())));
    if let Err(e) = written {
        eprintln!("desktop-gpui: downloading the plan to {}: {e}", path.display());
    }
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
                        .child(item("plan-download", "Download as markdown", Box::new(move |_| download(&downloaded))))
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
        .child(markdown::view(&body, false, None, cx))
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
    id: &str,
    title: &str,
    state: Option<&str>,
    open: bool,
    lines: &[transcript::SubLine],
    rail_follows: bool,
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
    let page = h_flex()
        .debug_selector(|| "subagent-page".into())
        .flex_none()
        .justify_center()
        .size(px(20.))
        .rounded(px(6.))
        .child(Icon::new(Glyph::ArrowUpRight).with_size(px(12.)).text_color(p.muted_foreground));
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
            .children(lines.iter().map(|line| {
                let code = |text: String| {
                    div().font_family(mono.clone()).text_size(XS.0).line_height(XS.1).text_color(p.muted_foreground).child(text)
                };
                let label = (!line.text.is_empty()).then(|| {
                    if line.mono {
                        code(line.text.clone())
                    } else {
                        div().text_size(SM.0).line_height(SM.1).text_color(p.foreground.opacity(0.8)).child(line.text.clone())
                    }
                });
                v_flex().min_w_0().children(label).children(line.detail.clone().map(code))
            }))
    });
    v_flex()
        .min_w_0()
        .px(px(4.))
        .pb(if rail_follows { px(0.) } else { FOOT })
        .child(h_flex().min_w_0().gap(px(6.)).child(trigger).child(page))
        .children(body)
        .into_any_element()
}

/// `SpawnTiles`: one column, each child a `ThreadRow` or `SubagentRow` in the row's box.
fn spawn(key: &str, children: &[Child], p: &Palette) -> AnyElement {
    v_flex()
        .debug_selector(|| "spawn".into())
        .min_w_0()
        .children(children.iter().enumerate().map(|(at_ix, child)| spawn_row(key, at_ix, child, p)))
        .into_any_element()
}

/// The row's box (`rowBox`): 36 px, or at least 48 px with a note; the mark, the title over its note, and the status.
fn spawn_row(key: &str, ix: usize, child: &Child, p: &Palette) -> AnyElement {
    let two = child.note.is_some();
    let status = match (child.settled, child.status.as_deref()) {
        (true, _) | (_, None) => None,
        (false, Some("running")) => Some(Status::Working(SharedString::default())),
        (false, Some("done")) => Some(Status::Done),
        (false, Some("failed")) => Some(Status::Failed),
        _ => None,
    };
    h_flex()
        .id(SharedString::from(format!("spawn-{key}-{ix}")))
        .debug_selector(|| "spawn-row".into())
        .min_w_0()
        .gap(px(10.))
        .px(px(8.))
        .rounded(px(8.))
        .text_size(SM_TIGHT.0)
        .line_height(SM_TIGHT.1)
        .hover(|s| s.bg(p.accent))
        .map(|el| if two { el.min_h(px(48.)).py(px(6.)) } else { el.h(px(36.)) })
        .child(
            div()
                .flex_none()
                .size(px(13.))
                .when(child.subagent, |el| el.child(Icon::new(Glyph::Bot).with_size(px(13.)).text_color(p.muted_foreground))),
        )
        .child(
            v_flex()
                .flex_1()
                .min_w(px(112.))
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .text_color(if child.settled { p.muted_foreground } else { p.foreground })
                        .child(child.title.clone()),
                )
                .children(child.note.clone().map(|note| {
                    div().min_w_0().truncate().text_size(px(11.)).line_height(px(14.)).text_color(p.muted_foreground).child(note)
                })),
        )
        .children(status.map(StatusMark::new))
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
    let open_diff = ghost_xs(SharedString::from(format!("open-diff-{key}")), Glyph::FileDiff, Some("Open diff"), false, p)
        .debug_selector(|| "open-diff".into());
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
                        .when(*added > 0 || *removed > 0, |el| el.child(div().ml_auto().flex_none().child(stat(*added, *removed, p, cx))))
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
                    .child(div().ml_auto().flex_none().child(stat(*added, *removed, p, cx)))
                    .into_any_element(),
            ),
        }
    }
}

/// `ForkRuleRow`: "Forked from {title}" in the meta size on a 24 px line.
fn fork(line: &str, p: &Palette) -> AnyElement {
    h_flex()
        .debug_selector(|| "fork".into())
        .h(px(24.))
        .min_w_0()
        .gap(px(8.))
        .px(px(4.))
        .text_size(px(11.))
        .text_color(p.muted_foreground)
        .child(div().min_w_0().truncate().child(line.to_owned()))
        .into_any_element()
}

/// `fmtBytes`: binary units, whole under a GB.
pub fn bytes(n: u64) -> String {
    const K: u64 = 1024;
    match n {
        n if n < K => format!("{n} B"),
        n if n < K * K => format!("{} KB", (n as f64 / K as f64).round()),
        n if n < K * K * K => format!("{} MB", (n as f64 / (K * K) as f64).round()),
        n => {
            let gb = n as f64 / (K * K * K) as f64;
            let tenth = format!("{gb:.1}");
            if tenth.ends_with(".0") {
                format!("{} GB", gb.round())
            } else {
                format!("{tenth} GB")
            }
        }
    }
}

/// `attachmentLine`: what an image the bubble holds no bytes of reads as, the line the command line prints.
pub fn attachment_line(tile: &FileTile) -> String {
    let size = bytes(tile.bytes.unwrap_or(0));
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
    window.open_dialog(cx, move |dialog, window, cx| {
        let p = cx.palette().clone();
        let view = window.viewport_size();
        let (max_w, max_h) = (view.width * 0.86, view.height * 0.8);
        let (w, h) = match size {
            Some((w, h)) => {
                let scale = (max_w / px(w)).min(max_h / px(h)).min(1.);
                (px(w) * scale, px(h) * scale)
            }
            None => (max_w, max_h),
        };
        dialog
            .close_button(false)
            .w(w + px(18.))
            .p(px(8.))
            .min_h(px(0.))
            .gap(px(0.))
            .rounded(px(14.))
            .border_color(p.popover_edge)
            .bg(p.popover)
            .shadow(p.dialog_shadow.clone())
            .child(
                img(ImageSource::Image(image.clone()))
                    .debug_selector(|| "image-dialog".into())
                    .id(name.clone())
                    .w(w)
                    .h(h)
                    .rounded(px(8.))
                    .object_fit(ObjectFit::Contain),
            )
    });
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
                    div().font_family(mono.clone()).text_size(META.0).line_height(META.1).text_color(p.muted_foreground).child(bytes(size)),
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

    fn base(kind: &str, pos: u64, extra: serde_json::Value) -> SessionEvent {
        let mut v = json!({"type": kind, "workspaceId": "w", "sessionId": "s", "threadId": "t", "turnId": "u1", "pos": pos});
        v.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        serde_json::from_value(v).unwrap()
    }

    fn window(events: Vec<SessionEvent>, cx: &mut TestAppContext) -> (Entity<ThreadView>, &mut VisualTestContext) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            crate::ui::init(crate::ui::Side::Light, cx);
        });
        let view = cx.new(ThreadView::new);
        let (_, cx) = cx.add_window_view(|window, cx| Root::new(view.clone(), window, cx));
        cx.simulate_resize(size(px(900.), px(1000.)));
        view.update(cx, |v, cx| {
            v.show("t", cx);
            v.load(events, cx);
            v.scroll_to_top(cx);
        });
        cx.run_until_parked();
        (view, cx)
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

    fn permission() -> Vec<SessionEvent> {
        let options = json!([{"id": "y", "label": "Allow", "effect": "allow"}, {"id": "n", "label": "Deny", "effect": "deny"}]);
        let input = "{\"command\":\"cargo test --all\",\"timeout\":5}";
        vec![
            base("session.start", 1, json!({"prompt": "go"})),
            base("session.delta", 2, json!({"kind": "tool_use", "toolName": "Bash", "toolUseId": "a", "text": input})),
            base("session.permission", 3, json!({"askId": "k", "toolName": "Bash", "toolUseId": "a", "input": input, "options": options})),
        ]
    }

    #[gpui_kit::test]
    fn an_open_permission_draws_as_the_web_and_a_pick_sends_its_ids(cx: &mut TestAppContext) {
        SENT.with(|s| s.borrow_mut().clear());
        let (view, cx) = window(permission(), cx);
        let row = frame(&view, cx, |k| matches!(k, RowKind::Permission { .. }));
        let lead = bounds(cx, "lead");
        // `pt-1 px-1`, then the lead: the command's `py-1` on a 16 px line stands taller than the 22.75 px words.
        assert_eq!((lead.left() - row.left(), lead.top() - row.top(), lead.size.height), (px(4.), px(4.), px(24.)));
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

    fn question(multiple: bool) -> Vec<SessionEvent> {
        let input = json!({"questions": [{"question": "Which colour?", "header": "Colour", "multiSelect": multiple, "options": [
            {"label": "Red", "description": "warm"}, {"label": "Blue"}
        ]}]});
        let options = json!([{"id": "q0:o0", "label": "Red", "effect": "answer"}, {"id": "q0:o1", "label": "Blue", "effect": "answer"}]);
        vec![
            base("session.start", 1, json!({"prompt": "go"})),
            base(
                "session.permission",
                2,
                json!({"askId": "w", "toolName": "AskUserQuestion", "input": input.to_string(), "options": options}),
            ),
        ]
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

    #[test]
    fn a_refused_pick_reads_the_protocols_answer_words() {
        let words = ["gone", "unsupported", "not-found", "no-option"].map(answer_words);
        assert_eq!(words[0], Some("the prompt closed before the answer reached it"));
        assert!(words.iter().all(|w| w.is_some_and(|w| !w.contains('"'))), "{words:?}");
        assert_eq!(answer_words("answered"), None, "an answered pick is no refusal");
        assert_eq!(sentence(words[2].unwrap()), "This host holds no turn of that thread.");
    }

    #[test]
    fn a_plan_downloads_under_its_titles_name_beside_what_is_there() {
        assert_eq!(plan_filename(Some("Fix the build's (CI) step!")), "fix-the-builds-ci-step.md");
        assert_eq!(plan_filename(None), "plan.md");
        let folder = tempfile::tempdir().unwrap();
        assert_eq!(download_path(folder.path(), "plan.md"), folder.path().join("plan.md"));
        std::fs::write(folder.path().join("plan.md"), "x").unwrap();
        std::fs::write(folder.path().join("plan (1).md"), "x").unwrap();
        assert_eq!(download_path(folder.path(), "plan.md"), folder.path().join("plan (2).md"));
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
        let (view, cx) = window(subagent(), cx);
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
        let (view, cx) = window(events, cx);
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
        cx.simulate_click(thumb.center(), Modifiers::none());
        cx.run_until_parked();
        assert!(cx.debug_bounds("image-dialog").is_some(), "a press opens the image whole");
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
        let row = frame(&view, cx, |k| matches!(k, RowKind::Fork { .. }));
        let line = bounds(cx, "fork");
        assert_eq!((line.size.height, row.size.height), (px(24.), px(24. + 16.)));
    }
}
