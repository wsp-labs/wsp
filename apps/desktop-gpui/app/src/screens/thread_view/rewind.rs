// SPDX-License-Identifier: AGPL-3.0-only
//! Copy, Rewind to here and Fork from here beside a message's stamp (`timeline/rows.tsx`), and `RewindDialog.tsx`.

use std::time::Duration;

use gpui_kit::component::dialog::DialogEntrance;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{h_flex, v_flex, Icon, Sizable as _, WindowExt as _};
use gpui_kit::{
    canvas, div, prelude::*, px, AnyElement, AnyView, App, ClipboardItem, Context, Entity, Global, Pixels, SharedString, WeakEntity, Window,
};
use wsp_host_client::protocol::{HarnessesListRequest, SessionsRewindRequest};

use crate::thread_view::{ThreadView, ThreadViewEvent};
use crate::transcript::{RewindFacts, Row, RowKind};
use crate::ui::{catalog, radio, ActivePalette, AlertDialog, Button, Glyph, Refusal};

/// `COPIED_MS`: how long Copy stands as a check, held.
const COPIED: Duration = Duration::from_millis(1_500);
/// `max-w-110`, `ui::AlertDialog`'s width.
const WIDTH: Pixels = px(440.);

const THREAD_WORDS: &str = include_str!("../../../../../../packages/protocol/src/words/thread.ts");

/// The first `"..."` after `at` in thread.ts.
fn quoted(at: &str) -> &'static str {
    let from = &THREAD_WORDS[THREAD_WORDS.find(at).unwrap_or(0)..];
    from.split('"').nth(1).unwrap_or_default()
}

/// A template literal of thread.ts with each `${..}` it holds filled by `fill`.
fn filled(template: &str, fill: impl Fn(&str) -> String) -> String {
    let mut out = String::new();
    let mut rest = template;
    while let Some(open) = rest.find("${") {
        out.push_str(&rest[..open]);
        let Some(close) = rest[open..].find('}') else { break };
        out.push_str(&fill(&rest[open + 2..open + close]));
        rest = &rest[open + close + 1..];
    }
    out.push_str(rest);
    out
}

/// `rewindNote` in packages/protocol/src/words/thread.ts, read from its source: the case's template, filled.
pub fn rewind_note(turns: usize, files: bool, cuts_conversation: bool, agent: &str, kept: Option<&str>) -> String {
    let body = &THREAD_WORDS[THREAD_WORDS.find("export function rewindNote").unwrap_or(0)..];
    let body = &body[..body.find("\n}").unwrap_or(body.len())];
    let templates: Vec<&str> = body.split('`').skip(1).step_by(2).collect();
    let case = if kept.is_some() {
        0
    } else if !cuts_conversation {
        1
    } else if !files {
        2
    } else {
        3
    };
    let turns_line = THREAD_WORDS.find("const turnsAfter").map(|at| THREAD_WORDS[at..].lines().next().unwrap_or_default());
    let turns_after = || match turns_line {
        Some(line) if turns == 1 => line.split('"').nth(1).unwrap_or_default().to_owned(),
        Some(line) => filled(line.split('`').nth(1).unwrap_or_default(), |_| turns.to_string()),
        None => String::new(),
    };
    let kept = kept.unwrap_or_default();
    let fill = |expr: &str| match expr {
        "SHARED_FOLDER" => quoted("const SHARED_FOLDER").to_owned(),
        "undo" => quoted("const undo").to_owned(),
        "o.agent" => agent.to_owned(),
        "turnsAfter(o.turns)" => turns_after(),
        "o.kept.charAt(0).toUpperCase()" => kept.chars().next().map(|c| c.to_uppercase().collect()).unwrap_or_default(),
        "o.kept.slice(1)" => kept.chars().skip(1).collect(),
        _ => String::new(),
    };
    templates.get(case).map(|t| filled(t, fill)).unwrap_or_default()
}

/// `asSentence`: the host's lowercase line as the app says it, capitalised and ending in a stop.
pub fn as_sentence(line: &str) -> String {
    let mut chars = line.chars();
    let mut out: String = chars.next().map(|c| c.to_uppercase().chain(chars).collect()).unwrap_or_default();
    if !out.ends_with(['.', '!', '?']) {
        out.push('.');
    }
    out
}

/// The row whose Copy stands as a check, and which press put it there.
#[derive(Default)]
struct Copied {
    key: Option<String>,
    press: u64,
}

impl Global for Copied {}

/// The open Rewind dialog's state, while one is open.
struct Opened(WeakEntity<Rewind>);

impl Global for Opened {}

/// A tooltip as `tooltip.tsx` draws one: 13 px on the popover, 8 px round, `px-2 py-1`.
fn tip(label: &'static str) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
    move |window, cx| {
        let p = cx.palette().clone();
        Tooltip::new(label)
            .m_0()
            .rounded(px(8.))
            .py(px(4.))
            .px(px(8.))
            .border_color(p.border)
            .bg(p.popover)
            .text_size(px(13.))
            .line_height(px(19.5))
            .text_color(p.foreground)
            .shadow(p.tooltip_shadow.clone())
            .build(window, cx)
    }
}

fn with_tip(id: String, label: &'static str, button: impl IntoElement) -> AnyElement {
    let selector = id.clone();
    div().id(SharedString::from(id)).debug_selector(move || selector).tooltip(tip(label)).child(button).into_any_element()
}

/// Copy, Rewind to here and Fork from here, in that order, where the row offers each: 24 px ghost buttons 2 px apart.
pub(super) fn buttons(row: &Row, thread: &str, cx: &mut Context<ThreadView>) -> impl IntoElement {
    let (rewind, fork) = match &row.kind {
        RowKind::Agent { rewind, fork, .. } => (rewind.clone(), fork.clone()),
        RowKind::Person { fork, .. } => (None, fork.clone()),
        _ => (None, None),
    };
    h_flex()
        .gap(px(2.))
        .when(!row.text.trim().is_empty(), |el| el.child(copy(row, cx)))
        .children(rewind.map(|turn| {
            let button = Button::icon_xs(SharedString::from(format!("rewind-{}", row.key)), Glyph::Undo2)
                .on_click(cx.listener(move |view, _, window, cx| open(view, &turn, window, cx)));
            with_tip(format!("rewind-to-here-{}", row.key), "Rewind to here", button)
        }))
        .children(fork.map(|turn| {
            let turn = (!turn.is_empty()).then_some(turn);
            // The thread as drawn, so a press on a frame drawn before a switch names the thread it showed.
            let thread = thread.to_owned();
            let button = Button::icon_xs(SharedString::from(format!("fork-{}", row.key)), Glyph::Split)
                .on_click(cx.listener(move |_, _, _, cx| cx.emit(ThreadViewEvent::Fork { thread: thread.clone(), turn: turn.clone() })));
            with_tip(format!("fork-from-here-{}", row.key), "Fork from here", button)
        }))
}

/// `MessageCopyButton`: Copy, and once pressed a check in the primary ink, held at 64% for 1.5 s.
fn copy(row: &Row, cx: &mut Context<ThreadView>) -> AnyElement {
    let key = row.key.clone();
    let id = format!("copy-{key}");
    if cx.try_global::<Copied>().is_some_and(|c| c.key.as_deref() == Some(key.as_str())) {
        let selector = format!("copied-{key}");
        return div()
            .id(SharedString::from(id))
            .debug_selector(move || selector)
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(px(24.))
            .opacity(0.64)
            .child(Icon::new(Glyph::Check).with_size(px(14.)).text_color(cx.palette().primary))
            .into_any_element();
    }
    let text = row.text.clone();
    let button = Button::icon_xs(SharedString::from(id), Glyph::Copy).on_click(cx.listener(move |_, _, _, cx| {
        cx.write_to_clipboard(ClipboardItem::new_string(text.clone()));
        let copied = cx.default_global::<Copied>();
        copied.press += 1;
        copied.key = Some(key.clone());
        let press = copied.press;
        cx.notify();
        cx.spawn(async move |view, cx| {
            cx.background_executor().timer(COPIED).await;
            _ = view.update(cx, |_, cx| {
                let copied = cx.default_global::<Copied>();
                if copied.press == press {
                    copied.key = None;
                }
                cx.notify();
            });
        })
        .detach();
    }));
    with_tip(format!("copy-to-clipboard-{}", row.key), "Copy to clipboard", button)
}

/// One Rewind to here: the reply's turn, what the dialog says about it, and the choice, the wait and the refusal.
pub struct Rewind {
    thread: String,
    turn: String,
    facts: RewindFacts,
    /// The agent's label once `harnesses.list` answers; the catalog's name stands in until then.
    agent: Option<String>,
    files: bool,
    busy: bool,
    refusal: Option<String>,
    height: Option<Pixels>,
}

impl Rewind {
    /// An agent that keeps its own history is rewound in its files alone, so there is nothing to choose.
    fn files(&self) -> bool {
        !self.facts.cuts_conversation || self.files
    }

    fn note(&self) -> String {
        let agent = self.agent.clone().unwrap_or_else(|| self.facts.harness.as_deref().map(catalog::name).unwrap_or_default());
        rewind_note(self.facts.turns_after, self.files(), self.facts.cuts_conversation, &agent, self.facts.kept.as_deref())
    }

    fn request(&self) -> SessionsRewindRequest {
        SessionsRewindRequest { thread_id: self.thread.clone(), turn_id: Some(self.turn.clone()), files: Some(self.files()), undo: None }
    }
}

/// What the open Rewind dialog shows, for a test or a capture.
#[derive(Clone, Debug, PartialEq)]
pub struct Shown {
    pub title: String,
    /// Each radio line: its words, whether it is picked, whether it is held.
    pub choices: Vec<(String, bool, bool)>,
    /// The slot under the choices: the note, or the refusal in its place.
    pub slot: String,
    pub refused: bool,
    pub busy: bool,
    /// The `sessions.rewind` Rewind sends for the choice as it stands.
    pub request: serde_json::Value,
}

/// The Rewind dialog open in this app, while one is.
pub fn shown(cx: &App) -> Option<Shown> {
    let rewind = cx.try_global::<Opened>()?.0.upgrade()?;
    let r = rewind.read(cx);
    let choices = if r.facts.cuts_conversation {
        vec![("Conversation only".to_owned(), !r.files, false), ("Conversation and files".to_owned(), r.files, !r.facts.files)]
    } else {
        Vec::new()
    };
    Some(Shown {
        title: "Rewind to here?".to_owned(),
        choices,
        slot: r.refusal.clone().unwrap_or_else(|| r.note()),
        refused: r.refusal.is_some(),
        busy: r.busy,
        request: serde_json::to_value(r.request()).unwrap_or_default(),
    })
}

/// Opens the dialog for a rewind of the thread to `turn`'s last reply.
fn open(view: &mut ThreadView, turn: &str, window: &mut Window, cx: &mut Context<ThreadView>) {
    let (Some(thread), Some(facts)) = (view.thread().map(str::to_owned), view.transcript().rewind_facts(turn)) else { return };
    let names_agent = !facts.cuts_conversation && facts.kept.is_none();
    let harness = facts.harness.clone();
    let rewind =
        cx.new(|_| Rewind { thread, turn: turn.to_owned(), facts, agent: None, files: false, busy: false, refusal: None, height: None });
    if let (true, Some(workspace), Some(harness)) = (names_agent, view.followed_workspace(), harness) {
        if let Ok(ask) = serde_json::from_value::<HarnessesListRequest>(serde_json::json!({ "workspaceId": workspace })) {
            let named = rewind.downgrade();
            view.call(ask, cx, move |_, answer, cx| {
                let label = answer.ok().and_then(|catalogs| {
                    catalogs.harnesses.iter().filter_map(|c| serde_json::to_value(c).ok()).find_map(|c| {
                        (c.get("harness")?.as_str()? == harness).then(|| c.get("label")?.as_str().map(str::to_owned)).flatten()
                    })
                });
                _ = named.update(cx, |r, cx| {
                    r.agent = label;
                    cx.notify();
                });
            });
        }
    }
    cx.set_global(Opened(rewind.downgrade()));
    let thread_view = cx.entity().downgrade();
    // gpui-kit's layer gives the scrim, the focus, Escape and the occlusion; the surface is `ui::AlertDialog`'s.
    window.open_dialog(cx, move |dialog, window, cx| {
        let height = rewind.read(cx).height.unwrap_or(px(240.));
        dialog
            .close_button(false)
            .overlay_closable(false)
            .entrance(DialogEntrance::Fade)
            .w(WIDTH)
            .margin_top(((window.viewport_size().height - height) / 2.).max(px(16.)))
            .p_0()
            .gap_0()
            .min_h(px(0.))
            .border_0()
            .bg(gpui_kit::transparent_black())
            .child(body(&rewind, &thread_view, cx))
    });
}

/// `ChoiceLine`: 44 px, the radio and its words 12 px apart, the whole line a press target; held at 64%.
fn choice(rewind: &Entity<Rewind>, files: bool, label: &'static str, held: bool, cx: &App) -> impl IntoElement {
    let picked = rewind.read(cx).files == files;
    let rewind = rewind.clone();
    h_flex()
        .id(if files { "rewind-files" } else { "rewind-conversation" })
        .debug_selector(move || if files { "rewind-files".into() } else { "rewind-conversation".into() })
        .h(px(44.))
        .gap(px(12.))
        .text_size(px(14.))
        .line_height(px(20.))
        .text_color(cx.palette().foreground)
        .when(held, |el| el.opacity(0.64))
        .child(radio(picked, held, cx))
        .child(label)
        .when(!held, |el| {
            el.cursor_pointer().on_click(move |_, _, cx| {
                rewind.update(cx, |r, cx| {
                    r.files = files;
                    cx.notify();
                })
            })
        })
}

/// The popup: the title, the choice where the agent cuts its conversation, the note or the refusal, Cancel and Rewind.
fn body(rewind: &Entity<Rewind>, view: &WeakEntity<ThreadView>, cx: &App) -> impl IntoElement {
    let r = rewind.read(cx);
    let (cuts, kept_files, busy) = (r.facts.cuts_conversation, r.facts.files, r.busy);
    let slot = match &r.refusal {
        Some(said) => Refusal::new().said(said.clone()),
        None => Refusal::new().quiet(r.note()),
    };
    let mut alert = AlertDialog::new("Rewind to here?");
    if cuts {
        alert = alert.lead(v_flex().child(choice(rewind, false, "Conversation only", false, cx)).child(choice(
            rewind,
            true,
            "Conversation and files",
            !kept_files,
            cx,
        )));
    }
    let measured = rewind.clone();
    let (sending, view) = (rewind.clone(), view.clone());
    let alert = alert
        .child(div().debug_selector(|| "rewind-slot".into()).child(slot))
        .act(Button::outline("rewind-cancel", "Cancel").neutral_ring().on_click(|_, window, cx| window.close_dialog(cx)))
        .act(Button::primary("rewind-confirm", "Rewind").held(busy).on_click(move |_, window, cx| go(&sending, &view, window, cx)));
    div().debug_selector(|| "rewind-dialog".into()).relative().child(alert).child(
        canvas(
            |_, _, _| (),
            move |bounds, _, _, cx| {
                let height = bounds.size.height;
                if measured.read(cx).height != Some(height) {
                    let measured = measured.clone();
                    cx.defer(move |cx| {
                        measured.update(cx, |r, cx| {
                            r.height = Some(height);
                            cx.notify();
                        })
                    });
                }
            },
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full(),
    )
}

/// Asks the host for the rewind once, never while it waits on the last ask; closes on its answer, or shows its refusal
/// in the note's slot.
pub fn go(rewind: &Entity<Rewind>, view: &WeakEntity<ThreadView>, window: &mut Window, cx: &mut App) {
    if rewind.read(cx).busy {
        return;
    }
    let request = rewind.read(cx).request();
    rewind.update(cx, |r, cx| {
        r.busy = true;
        r.refusal = None;
        cx.notify();
    });
    let (answered, handle) = (rewind.clone(), window.window_handle());
    _ = view.update(cx, |view, cx| {
        view.call(request, cx, move |_, answer, cx| {
            let refusal = answer.err().map(|failure| as_sentence(&failure.message));
            let done = refusal.is_none();
            answered.update(cx, |r, cx| {
                r.busy = false;
                r.refusal = refusal;
                cx.notify();
            });
            let still_open = cx.try_global::<Opened>().is_some_and(|o| o.0 == answered.downgrade());
            if done && still_open {
                cx.defer(move |cx| {
                    _ = handle.update(cx, |_, window, cx| window.close_dialog(cx));
                });
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use gpui_kit::component::Root;
    use gpui_kit::TestAppContext;

    use super::*;

    #[gpui_kit::test]
    fn a_press_while_the_rewind_waits_asks_nothing(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            crate::ui::init(crate::ui::Side::Light, cx);
        });
        let view = cx.new(ThreadView::new);
        let (_, cx) = cx.add_window_view(|window, cx| Root::new(view.clone(), window, cx));
        let facts = RewindFacts { turns_after: 1, files: true, cuts_conversation: false, kept: None, harness: None };
        let rewind = cx.new(|_| Rewind {
            thread: "t".into(),
            turn: "u0".into(),
            facts,
            agent: None,
            files: false,
            busy: true,
            refusal: None,
            height: None,
        });
        let weak = view.downgrade();
        cx.update(|window, cx| go(&rewind, &weak, window, cx));
        // With no host a call refuses at once, so an ask would leave the refusal in the slot and the wait over.
        rewind.read_with(cx, |r, _| assert!(r.busy && r.refusal.is_none(), "a press while the rewind waits asked the host again"));
    }
}
