// SPDX-License-Identifier: AGPL-3.0-only
//! General (`general.tsx`): the composer's send key and what a message does mid-turn, how each kind of moment is said
//! outside the app, what New thread opens on, when a read thread settles and whether a delete asks, the editor Open in
//! editor goes to and whether it opens a workspace over ssh, and what quitting does and whether this computer is kept
//! awake. A row off its default carries the arrow that puts it back.

use gpui_kit::{prelude::*, App, ElementId, SharedString, Window};
use serde_json::{json, Value};
use wsp_host_client::ops::settings::SshInclude;
use wsp_host_client::protocol::SshIncludeRequest;

use crate::screens::settings::model::{req, Data};
use crate::screens::settings::page::{CardData, Ctx, Item};
use crate::ui::{Row, Segment, SegmentedControl, Select, SelectItem, SelectState, Switch};

/// `GENERAL_WORDS` and `AWAKE_WORDS` in `format.ts`, `EDITOR_SSH_WORDS` in `EditorConsent.tsx`.
mod words {
    pub const COMPOSER: &str = "Composer";
    pub const SEND_WITH: &str = "Send with";
    pub const SEND_WITH_NOTE: &str = "The other key makes a new line. ⌘ on a Mac, Ctrl elsewhere.";
    pub const MID_TURN: &str = "A message while a thread works";
    pub const MID_TURN_NOTE: &str = "Queue waits for the turn to end; steer hands it to the agent now.";
    pub const NOTIFICATIONS: &str = "Notifications";
    pub const NOTIFY_NEEDS: &str = "When a thread needs you";
    pub const NOTIFY_NEEDS_NOTE: &str = "A question, a permission prompt, a sign-in.";
    pub const NOTIFY_DONE: &str = "When a thread finishes";
    pub const NOTIFY_DONE_NOTE: &str = "A thread you or the command line started; one an agent started reports to that agent.";
    pub const PLAN_ALERTS: &str = "When a plan window runs low";
    pub const PLAN_ALERTS_NOTE: &str = "At 70% and 90% of a window, once each, and when an account is blocked.";
    pub const THREADS: &str = "Threads";
    pub const NEW_THREAD_IN: &str = "New thread starts in";
    pub const NEW_THREAD_IN_NOTE: &str = "Ask every time lists your projects before a new thread opens.";
    pub const SETTLE_AFTER: &str = "Settle a thread after";
    pub const SETTLE_AFTER_NOTE: &str = "A read thread moves to Settled once it has been quiet this long.";
    pub const ASK_DELETE: &str = "Ask before deleting";
    pub const ASK_DELETE_NOTE: &str = "A workspace with unpushed work always asks.";
    pub const OPEN_IN: &str = "Open in";
    pub const EDITOR: &str = "Open files in";
    pub const EDITOR_NOTE: &str = "Where Open in editor goes, for a file in a thread or a whole workspace.";
    pub const NO_EDITOR: &str = "No editor wsp opens files in is installed: VS Code, Cursor, Zed or a JetBrains IDE.";
    pub const EDITOR_SSH: &str = "Editors over ssh";
    pub const EDITOR_SSH_NOTE: &str = "One line at the top of ~/.ssh/config lets your editor open a workspace on another computer.";
    pub const STARTUP: &str = "Startup and quit";
    pub const ON_QUIT: &str = "When you quit";
    pub const KEEP_AWAKE_NOTE: &str = "The wsp app stops it sleeping on its own while a thread works there.";

    pub fn send_keys(mac: bool) -> [(&'static str, &'static str); 2] {
        [("enter", "Enter"), ("mod-enter", if mac { "⌘ Enter" } else { "Ctrl Enter" })]
    }
    pub const MID_TURN_CHOICES: [(&str, &str); 2] = [("queue", "Queue"), ("steer", "Steer")];
    pub const NOTIFY_CHOICES: [(&str, &str); 4] =
        [("off", "Off"), ("notify", "Notify"), ("sound", "Sound"), ("notify-sound", "Notify and sound")];
    pub const NEW_THREAD_IN_CHOICES: [(&str, &str); 2] = [("current", "Current project"), ("ask", "Ask every time")];
    pub const SETTLE_CHOICES: [(&str, &str); 5] =
        [("15m", "15 minutes"), ("1h", "1 hour"), ("2h", "2 hours"), ("1d", "1 day"), ("never", "Never")];
    pub const ON_QUIT_CHOICES: [(&str, &str); 3] = [("ask", "Ask each time"), ("keep", "Keep threads running"), ("stop", "Stop wsp too")];

    pub fn on_quit_note(here: &str) -> String {
        super::once_named(here, |h| format!("Quitting the window leaves threads running on {h}; quit and stop ends them too."))
    }
    pub fn keep_awake(here: &str) -> String {
        format!("Keep {here} awake")
    }
}

/// `onceNamed`: a sentence that names the computer the host runs on, said only once that name is known.
pub(super) fn once_named(here: &str, sentence: impl FnOnce(&str) -> String) -> String {
    if here.is_empty() {
        String::new()
    } else {
        sentence(here)
    }
}

/// `hereName`: the computer the host runs on, by its own name; empty until the places list holds that row.
pub(super) fn here_name(data: &Data) -> String {
    data.places.iter().find(|p| p.id == "here").map(super::computers::place_name).unwrap_or_default()
}

/// The record as the wire spells it, so a field reads and resets by its one name.
pub(super) fn record(ctx: &Ctx) -> Option<Value> {
    ctx.data.preferences().and_then(|p| serde_json::to_value(p).ok())
}

/// `DEFAULT_PREFERENCES` for the fields General draws.
fn default_of(field: &str) -> Value {
    match field {
        "sendWith" => json!("enter"),
        "midTurn" => json!("queue"),
        "notifyNeeds" => json!("notify-sound"),
        "notifyDone" => json!("notify"),
        "settleAfter" => json!("2h"),
        "onQuit" => json!("ask"),
        "newThreadIn" => json!("current"),
        _ => json!(true),
    }
}

/// `resetOf`: the arrow a row off the record's default carries, which writes that one field back.
pub(super) fn reset_to(row: Row, ctx: &Ctx, record: &Value, field: &'static str, default: Value) -> Row {
    if record[field] == default {
        return row;
    }
    let acts = ctx.acts.clone();
    row.on_reset(move |_, _, cx| acts.set_preferences(json!({ field: default.clone() }), cx))
}

fn reset(row: Row, ctx: &Ctx, record: &Value, field: &'static str) -> Row {
    reset_to(row, ctx, record, field, default_of(field))
}

/// A switch over one boolean field of the record.
pub(super) fn field_switch(ctx: &Ctx, record: &Value, id: &'static str, field: &'static str) -> Switch {
    let acts = ctx.acts.clone();
    Switch::new(id, record[field].as_bool().unwrap_or(false)).on_change(move |on, _, cx| acts.set_preferences(json!({ field: *on }), cx))
}

/// What a row's select shows, kept beside it so the items are laid again only when they move.
struct Choices {
    state: gpui_kit::Entity<SelectState<String>>,
    shown: Vec<(String, SharedString)>,
}

fn items_of(choices: &[(String, SharedString)]) -> Vec<SelectItem<String>> {
    choices.iter().map(|(value, word)| SelectItem::new(value.clone(), word.clone())).collect()
}

/// A select over `choices`, each a value of the record and its word, kept on `value` as the record moves; a pick is
/// handed to `pick` as the record's value.
pub(super) fn choice_select(
    key: impl Into<ElementId>,
    choices: Vec<(String, SharedString)>,
    value: &str,
    window: &mut Window,
    cx: &mut App,
    pick: impl Fn(String, &mut App) + 'static,
) -> Select<String> {
    let value = value.to_owned();
    let held = window.use_keyed_state(key, cx, |_, cx| Choices {
        state: cx.new(|_| SelectState::new(items_of(&choices), Some(&value))),
        shown: choices.clone(),
    });
    let state = held.read(cx).state.clone();
    if held.read(cx).shown != choices {
        state.update(cx, |state, cx| state.set_items(items_of(&choices), cx));
        held.update(cx, |held, _| held.shown = choices);
    }
    if state.read(cx).value() != Some(&value) {
        state.update(cx, |state, cx| state.set_value(Some(&value), cx));
    }
    Select::new(&state).on_change(move |next: &String, _, cx| pick(next.clone(), cx))
}

/// `ChoiceSelect`: a select over a fixed set of words, its value read and written as one of the record's choices.
fn field_select(
    ctx: &Ctx,
    record: &Value,
    id: &'static str,
    field: &'static str,
    choices: &[(&str, &str)],
    window: &mut Window,
    cx: &mut App,
) -> Select<String> {
    let acts = ctx.acts.clone();
    let value = record[field].as_str().unwrap_or_default();
    let choices = choices.iter().map(|(v, w)| (v.to_string(), SharedString::from(w.to_string()))).collect();
    choice_select(SharedString::from(format!("general-select-{id}")), choices, value, window, cx, move |next, cx| {
        acts.set_preferences(json!({ field: next }), cx)
    })
}

fn segmented(
    ctx: &Ctx,
    record: &Value,
    id: &'static str,
    field: &'static str,
    segments: &[(&'static str, &'static str)],
) -> SegmentedControl<String> {
    let acts = ctx.acts.clone();
    let segments = segments.iter().map(|(value, label)| Segment::new(value.to_string(), *label)).collect();
    SegmentedControl::new(id, record[field].as_str().unwrap_or_default().to_owned(), segments)
        .on_change(move |next, _, cx| acts.set_preferences(json!({ field: next }), cx))
}

pub fn cards(ctx: &Ctx, window: &mut Window, cx: &mut App) -> Vec<CardData> {
    let Some(p) = record(ctx) else { return Vec::new() };
    let mac = cfg!(target_os = "macos");
    let here = here_name(ctx.data);
    let notify = |id: &'static str, field: &'static str, title: &'static str, note: &'static str, window: &mut Window, cx: &mut App| {
        let select = field_select(ctx, &p, id, field, &words::NOTIFY_CHOICES, window, cx);
        Item::row(id, title, Some(note), |row| reset(row.control(select), ctx, &p, field))
    };
    let mut cards = vec![
        CardData::new("composer").head(words::COMPOSER).items([
            Item::row("send-with", words::SEND_WITH, Some(words::SEND_WITH_NOTE), |row| {
                reset(row.control(segmented(ctx, &p, "send-with", "sendWith", &words::send_keys(mac))), ctx, &p, "sendWith")
            }),
            Item::row("mid-turn", words::MID_TURN, Some(words::MID_TURN_NOTE), |row| {
                reset(row.control(segmented(ctx, &p, "mid-turn", "midTurn", &words::MID_TURN_CHOICES)), ctx, &p, "midTurn")
            }),
        ]),
        CardData::new("notifications").head(words::NOTIFICATIONS).items([
            notify("notify-needs", "notifyNeeds", words::NOTIFY_NEEDS, words::NOTIFY_NEEDS_NOTE, window, cx),
            notify("notify-done", "notifyDone", words::NOTIFY_DONE, words::NOTIFY_DONE_NOTE, window, cx),
            Item::row("plan-alerts", words::PLAN_ALERTS, Some(words::PLAN_ALERTS_NOTE), |row| {
                reset(row.control(field_switch(ctx, &p, "plan-alerts", "planAlerts")), ctx, &p, "planAlerts")
            }),
        ]),
    ];
    let new_thread_in = field_select(ctx, &p, "new-thread-in", "newThreadIn", &words::NEW_THREAD_IN_CHOICES, window, cx);
    let settle_after = field_select(ctx, &p, "settle-after", "settleAfter", &words::SETTLE_CHOICES, window, cx);
    cards.push(CardData::new("threads").head(words::THREADS).items([
        Item::row("new-thread-in", words::NEW_THREAD_IN, Some(words::NEW_THREAD_IN_NOTE), |row| {
            reset(row.control(new_thread_in), ctx, &p, "newThreadIn")
        }),
        Item::row("settle-after", words::SETTLE_AFTER, Some(words::SETTLE_AFTER_NOTE), |row| {
            reset(row.control(settle_after), ctx, &p, "settleAfter")
        }),
        Item::row("ask-delete", words::ASK_DELETE, Some(words::ASK_DELETE_NOTE), |row| {
            reset(row.control(field_switch(ctx, &p, "ask-delete", "askDelete")), ctx, &p, "askDelete")
        }),
    ]));
    cards.push(CardData::new("open-in").head(words::OPEN_IN).items(open_in(ctx, &p, window, cx)));

    let on_quit = field_select(ctx, &p, "on-quit", "onQuit", &words::ON_QUIT_CHOICES, window, cx);
    let mut startup = vec![Item::row("on-quit", words::ON_QUIT, Some(words::on_quit_note(&here).as_str()), |row| {
        reset(row.control(on_quit), ctx, &p, "onQuit")
    })];
    // The switch's title is the computer's name, so it stands only once that name is known.
    if !here.is_empty() {
        startup.push(Item::row("keep-awake", words::keep_awake(&here), Some(words::KEEP_AWAKE_NOTE), |row| {
            reset(row.control(field_switch(ctx, &p, "keep-awake", "keepAwake")), ctx, &p, "keepAwake")
        }));
    }
    cards.push(CardData::new("startup").head(words::STARTUP).items(startup));
    cards
}

/// The editor row and, once the host has said whether ~/.ssh/config reads wsp's own, the ssh switch.
fn open_in(ctx: &Ctx, p: &Value, window: &mut Window, cx: &mut App) -> Vec<Item> {
    let editors = ctx.data.editors.as_deref().unwrap_or_default();
    let note = if ctx.data.editors.is_some() && editors.is_empty() { words::NO_EDITOR } else { words::EDITOR_NOTE };
    let picked = p["editor"].as_str().and_then(|id| editors.iter().find(|e| e.id.to_string() == id)).or(editors.first());
    let select = picked.map(|picked| {
        let choices = editors.iter().map(|e| (e.id.to_string(), SharedString::from(e.name.clone()))).collect();
        let acts = ctx.acts.clone();
        choice_select("general-select-editor", choices, &picked.id.to_string(), window, cx, move |next, cx| {
            acts.set_preferences(json!({ "editor": next }), cx)
        })
    });
    let mut items = vec![Item::row("editor", words::EDITOR, Some(note), |row| match select {
        Some(select) => row.control(select),
        None => row,
    })];
    // The host's answer to a turn of the switch stands over the read Settings opened with.
    let answered = window.use_keyed_state("general-ssh-include", cx, |_, _| None::<bool>);
    if let Some(on) = (*answered.read(cx)).or(ctx.data.ssh_include) {
        let acts = ctx.acts.clone();
        let switch =
            Switch::new("editor-ssh", on).on_change(move |&on, _, cx| {
                let (acts, answered) = (acts.clone(), answered.clone());
                acts.clone().call(req::<SshIncludeRequest>(json!({ "on": on })), cx, move |answer: Result<SshInclude, String>, cx| {
                    match answer {
                        Ok(reply) => {
                            answered.update(cx, |held, _| *held = Some(reply.ssh_include));
                            acts.notify(cx);
                        }
                        Err(said) => acts.failed(said, cx),
                    }
                });
            });
        items.push(Item::row("editor-ssh", words::EDITOR_SSH, Some(words::EDITOR_SSH_NOTE), |row| row.control(switch)));
    }
    items
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sentence_naming_this_computer_waits_for_its_name() {
        assert_eq!(words::on_quit_note(""), "");
        assert_eq!(words::on_quit_note("box"), "Quitting the window leaves threads running on box; quit and stop ends them too.");
        assert_eq!(words::keep_awake("box"), "Keep box awake");
    }

    #[test]
    fn the_send_keys_are_worded_per_platform() {
        assert_eq!(words::send_keys(true)[1].1, "⌘ Enter");
        assert_eq!(words::send_keys(false)[1].1, "Ctrl Enter");
    }
}
