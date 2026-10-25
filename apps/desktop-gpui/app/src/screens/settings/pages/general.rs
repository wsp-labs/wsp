// SPDX-License-Identifier: AGPL-3.0-only
//! General (`general.tsx`): the composer's send key and what a message does mid-turn, how each kind of moment is said
//! outside the app, what New thread opens on, when a read thread settles and whether a delete asks, the editor Open in
//! editor goes to and whether it opens a workspace over ssh, what quitting does, whether the host's service starts at
//! login and whether this computer is kept awake, and the app's logs. A row off its default carries the arrow that puts
//! it back.

use gpui_kit::{prelude::*, px, App, ElementId, Entity, SharedString, Window};
use serde_json::{json, Value};
use wsp_host_client::ops::settings::SshInclude;
use wsp_host_client::protocol::SshIncludeRequest;

use crate::desktop::{self, Desktop, Launched};
use crate::screens::settings::model::{req, Read};
use crate::screens::settings::page::{CardData, Ctx, Item};
use crate::screens::settings::words::here_name;
use crate::screens::settings::Acts;
use crate::ui::{catalog, ActivePalette, Button, Row, Segmented, Select, SelectItem, SelectState, Switch};

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
    pub const LOGIN_START: &str = "Start wsp at login";
    pub const LOGS: &str = "Logs";
    pub const APP_LOGS: &str = "App logs";
    pub const APP_LOGS_NOTE: &str = "The app's log and its crash reports. Nothing in them is sent anywhere.";
    pub const OPEN_LOGS: &str = "Open logs";

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
    pub fn login_start_note(here: &str) -> String {
        super::once_named(here, |h| format!("wsp keeps running on {h} with no window open, so threads carry on."))
    }
    /// The rejection `ipcRenderer.invoke("logs:open")` hands the page, which its notice says whole.
    pub fn logs_not_opened(said: &str) -> String {
        format!("Error invoking remote method 'logs:open': Error: {said}")
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

/// How a select's items are drawn past their words: a mark, a face.
type Dress = fn(SelectItem<String>) -> SelectItem<String>;

fn items_of(choices: &[(String, SharedString)], dress: Dress) -> Vec<SelectItem<String>> {
    choices.iter().map(|(value, word)| dress(SelectItem::new(value.clone(), word.clone()))).collect()
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
    dressed_select(key, choices, value, |item| item, window, cx, pick)
}

/// [`choice_select`] with each item dressed by `dress`.
pub(super) fn dressed_select(
    key: impl Into<ElementId>,
    choices: Vec<(String, SharedString)>,
    value: &str,
    dress: Dress,
    window: &mut Window,
    cx: &mut App,
    pick: impl Fn(String, &mut App) + 'static,
) -> Select<String> {
    let value = value.to_owned();
    let held = window.use_keyed_state(key, cx, |_, cx| Choices {
        state: cx.new(|_| SelectState::new(items_of(&choices, dress), Some(&value))),
        shown: choices.clone(),
    });
    let state = held.read(cx).state.clone();
    if held.read(cx).shown != choices {
        state.update(cx, |state, cx| state.set_items(items_of(&choices, dress), cx));
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

fn segmented(ctx: &Ctx, record: &Value, id: &'static str, field: &'static str, segments: &[(&'static str, &'static str)]) -> Segmented {
    let acts = ctx.acts.clone();
    let values: Vec<&'static str> = segments.iter().map(|(value, _)| *value).collect();
    let picked = values.iter().position(|v| Some(*v) == record[field].as_str());
    segments
        .iter()
        .fold(Segmented::new(id, picked), |control, (_, label)| control.segment(*label))
        .on_pick(move |ix, _, cx| acts.set_preferences(json!({ field: values[ix] }), cx))
}

pub fn cards(ctx: &Ctx, window: &mut Window, cx: &mut App) -> Vec<CardData> {
    let Some(p) = record(ctx) else { return Vec::new() };
    let mac = cfg!(target_os = "macos");
    let here = here_name(&ctx.data.places);
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
    let login = login_start(window, cx);
    if let Some(on) = login.read(cx).on {
        let acts = ctx.acts.clone();
        let held = login.clone();
        let switch = Switch::new("login-start", on).on_change(move |&on, _, cx| turn_login(&held, on, &acts, cx));
        startup.push(Item::row("login-start", words::LOGIN_START, Some(words::login_start_note(&here).as_str()), |row| {
            // The service starts at login unless the person turned it off, so on is this row's default.
            let row = row.control(switch);
            if on {
                return row;
            }
            let acts = ctx.acts.clone();
            row.on_reset(move |_, _, cx| turn_login(&login, true, &acts, cx))
        }));
    }
    // The switch's title is the computer's name, so it stands only once that name is known.
    if !here.is_empty() {
        startup.push(Item::row("keep-awake", words::keep_awake(&here), Some(words::KEEP_AWAKE_NOTE), |row| {
            reset(row.control(field_switch(ctx, &p, "keep-awake", "keepAwake")), ctx, &p, "keepAwake")
        }));
    }
    cards.push(CardData::new("startup").head(words::STARTUP).items(startup));
    if let Some((road, launched)) = road(cx) {
        let (folder, vars, acts) = (launched.logs(), launched.env.clone(), ctx.acts.clone());
        let open = Button::outline("open-logs", words::OPEN_LOGS).xs().on_click(move |_, _, cx| open_logs(road, &folder, &vars, &acts, cx));
        cards.push(CardData::new("logs").head(words::LOGS).items([Item::row(
            "app-logs",
            words::APP_LOGS,
            Some(words::APP_LOGS_NOTE),
            |row| row.control(open),
        )]));
    }
    cards
}

/// This platform's roads and the launch they answer for: none in a window opened without a launch, as a test opens it.
fn road(cx: &App) -> Option<(&'static dyn Desktop, &Launched)> {
    Some((desktop::here()?, cx.try_global::<Launched>()?))
}

/// What this computer's manager last said of the host's service starting at login (`reads.loginStart`): nothing
/// until it answers, and nothing where no service serves this launch's state file here.
struct LoginStart {
    on: Option<bool>,
}

/// Asks the manager in the background when General first draws, as Settings asks the desktop bridge when it opens.
fn login_start(window: &mut Window, cx: &mut App) -> Entity<LoginStart> {
    let asked = road(cx).map(|(road, launched)| (road, launched.address(), launched.env.clone()));
    window.use_keyed_state("general-login-start", cx, move |_, cx| {
        if let Some((road, at, vars)) = asked {
            let read = cx.background_executor().spawn(async move { road.starts_at_login(&at, &vars) });
            cx.spawn(async move |this, cx| {
                let on = read.await;
                _ = this.update(cx, |state: &mut LoginStart, cx| {
                    state.on = on;
                    cx.notify();
                });
            })
            .detach();
        }
        LoginStart { on: None }
    })
}

/// `setLoginStart`, then the reading after it; a refusal is said as a notice and leaves the switch where it was.
fn turn_login(login: &Entity<LoginStart>, on: bool, acts: &Acts, cx: &mut App) {
    let Some((road, launched)) = road(cx) else { return };
    let (at, vars) = (launched.address(), launched.env.clone());
    let task =
        cx.background_executor().spawn(async move { road.set_login_start(&at, on, &vars).map(|()| road.starts_at_login(&at, &vars)) });
    let (login, acts) = (login.downgrade(), acts.clone());
    cx.spawn(async move |cx| {
        let answer = task.await;
        cx.update(|cx| match answer {
            Ok(on) => {
                _ = login.update(cx, |state, cx| {
                    state.on = on;
                    cx.notify();
                });
            }
            Err(said) => acts.failed(said, cx),
        });
    })
    .detach();
}

/// `logs:open`: the folder the Electron app keeps its log and crash reports in. Nothing here writes under the wsp
/// home (`packages/own-file` is its one writer), so a folder that is not there is the open's refusal, said as the
/// web app says a refused bridge call.
fn open_logs(road: &'static dyn Desktop, folder: &std::path::Path, vars: &wsp_host_client::Env, acts: &Acts, cx: &mut App) {
    let (folder, vars, acts) = (folder.to_owned(), vars.clone(), acts.clone());
    let task = cx.background_executor().spawn(async move { road.open(&folder, &vars) });
    cx.spawn(async move |cx| {
        if let Err(said) = task.await {
            cx.update(|cx| acts.failed(words::logs_not_opened(&said), cx));
        }
    })
    .detach();
}

/// `EditorGlyph` before an editor's name, on the trigger and in the list; an editor no mark claims keeps its square.
fn with_editor_glyph(item: SelectItem<String>) -> SelectItem<String> {
    let editor = item.value().clone();
    item.mark(move |cx| {
        catalog::editor_mark(&editor, px(16.), cx.palette().foreground, cx)
            .unwrap_or_else(|| gpui_kit::div().size(px(16.)).flex_none().into_any_element())
    })
}

/// The editor row and, once the host has said whether ~/.ssh/config reads wsp's own, the ssh switch.
fn open_in(ctx: &Ctx, p: &Value, window: &mut Window, cx: &mut App) -> Vec<Item> {
    let editors = ctx.data.editors.as_deref().unwrap_or_default();
    let note = if ctx.data.editors.is_some() && editors.is_empty() { words::NO_EDITOR } else { words::EDITOR_NOTE };
    let picked = p["editor"].as_str().and_then(|id| editors.iter().find(|e| e.id.to_string() == id)).or(editors.first());
    let select = picked.map(|picked| {
        let choices = editors.iter().map(|e| (e.id.to_string(), SharedString::from(e.name.clone()))).collect();
        let acts = ctx.acts.clone();
        dressed_select("general-select-editor", choices, &picked.id.to_string(), with_editor_glyph, window, cx, move |next, cx| {
            acts.set_preferences(json!({ "editor": next }), cx)
        })
    });
    // The web hands its row a null control, which still draws the empty slot and, under 640 px, its gap.
    let mut items = vec![Item::row("editor", words::EDITOR, Some(note), |row| match select {
        Some(select) => row.control(select),
        None => row.control(gpui_kit::div().debug_selector(|| "editor-slot".into())),
    })];
    if let Some(on) = ctx.data.ssh_include {
        let acts = ctx.acts.clone();
        let switch =
            Switch::new("editor-ssh", on).on_change(move |&on, _, cx| {
                let acts = acts.clone();
                acts.clone().call(req::<SshIncludeRequest>(json!({ "on": on })), cx, move |answer: Result<SshInclude, String>, cx| {
                    match answer {
                        Ok(reply) => acts.take(Read::SshInclude(Ok(reply.ssh_include)), cx),
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
        assert_eq!(words::login_start_note(""), "");
        assert_eq!(words::login_start_note("box"), "wsp keeps running on box with no window open, so threads carry on.");
    }

    #[test]
    fn the_send_keys_are_worded_per_platform() {
        assert_eq!(words::send_keys(true)[1].1, "⌘ Enter");
        assert_eq!(words::send_keys(false)[1].1, "Ctrl Enter");
    }

    #[test]
    fn each_reset_writes_the_records_own_default() {
        let bare = json!({ "theme": "system", "sidebarMode": "list", "terminalSize": "app", "terminalZoom": {}, "access": {},
            "projectLook": {}, "projectOrder": [], "labs": false });
        let defaults: wsp_host_client::protocol::Preferences = serde_json::from_value(bare).unwrap();
        let defaults = serde_json::to_value(defaults).unwrap();
        for field in "sendWith midTurn notifyNeeds notifyDone planAlerts settleAfter askDelete onQuit newThreadIn keepAwake".split(' ') {
            assert_eq!(default_of(field), defaults[field], "{field}");
        }
    }
}
