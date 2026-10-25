// SPDX-License-Identifier: AGPL-3.0-only
//! The person's preferences as the app holds them, the one copy every window and page reads: the host's last record
//! with each write still out laid over it in the order it was made. A record the host sent from before a write cannot
//! paint over the pick, a write the host refuses falls away, and every reader moves on the same notify.

use std::sync::Arc;

use gpui_kit::{App, AppContext, Context, Entity, Global};
use serde_json::Value;
use tokio::runtime::Runtime;
use wsp_host_client::protocol::Preferences;
use wsp_host_client::{HostClient, Op};

#[derive(Default)]
pub struct Prefs {
    /// The host's last record, from a read or `preferences.changed`.
    host: Option<Value>,
    /// Writes sent and not yet answered, oldest first.
    out: Vec<(u64, Value)>,
    next: u64,
    shown: Option<Preferences>,
    /// The record `shown` is read off, whole, for a reader of a key the typed record does not name.
    record: Option<Value>,
}

struct Held(Entity<Prefs>);

impl Global for Held {}

/// The app's one store, made on first use.
pub fn store(cx: &mut App) -> Entity<Prefs> {
    if let Some(held) = cx.try_global::<Held>() {
        return held.0.clone();
    }
    let store = cx.new(|_| Prefs::default());
    cx.set_global(Held(store.clone()));
    store
}

/// The record every reader draws, once the host has sent one.
pub fn shown(cx: &App) -> Option<&Preferences> {
    cx.try_global::<Held>().and_then(|held| held.0.read(cx).shown.as_ref())
}

/// [`shown`] as the host's words, whole.
pub fn record(cx: &App) -> Option<&Value> {
    cx.try_global::<Held>().and_then(|held| held.0.read(cx).record.as_ref())
}

/// Hands a record a reader heard from the host to the store, which every reader then draws.
pub fn land(record: Value, cx: &mut App) {
    store(cx).update(cx, |store, cx| store.land(record, cx));
}

impl Prefs {
    pub fn shown(&self) -> Option<&Preferences> {
        self.shown.as_ref()
    }

    pub fn record(&self) -> Option<&Value> {
        self.record.as_ref()
    }

    /// How many writes are out.
    pub fn writes_out(&self) -> usize {
        self.out.len()
    }

    /// A record the host sent, read or pushed.
    pub fn landed(&mut self, record: &Preferences, cx: &mut Context<Self>) {
        if let Ok(record) = serde_json::to_value(record) {
            self.land(record, cx);
        }
    }

    /// A record the host sent, as its words.
    pub fn land(&mut self, record: Value, cx: &mut Context<Self>) {
        self.host = Some(record);
        self.reshow(cx);
    }

    /// Lays `patch` over what every reader draws at once and sends it on `host`. `then` hears the answer once the store
    /// has taken it: a notice the host kept the write with, or why it refused it.
    pub fn write(
        &mut self,
        patch: Value,
        host: Option<(Arc<HostClient>, Arc<Runtime>)>,
        cx: &mut Context<Self>,
        then: impl FnOnce(Result<Option<String>, String>, &mut App) + 'static,
    ) {
        let id = self.next;
        self.next += 1;
        self.out.push((id, patch.clone()));
        self.reshow(cx);
        let Some((host, runtime)) = host else {
            self.answered(id, Err("No host answered.".to_owned()), cx, then);
            return;
        };
        let (tx, rx) = futures::channel::oneshot::channel();
        runtime.spawn(async move {
            _ = tx.send(host.call(PatchPreferences { patch }).await.map(|a| a.notice).map_err(|f| f.message));
        });
        cx.spawn(async move |this, cx| {
            let answer = rx.await.unwrap_or_else(|_| Err("No host answered.".to_owned()));
            _ = this.update(cx, |this, cx| this.answered(id, answer, cx, then));
        })
        .detach();
    }

    fn answered(
        &mut self,
        id: u64,
        answer: Result<Option<String>, String>,
        cx: &mut Context<Self>,
        then: impl FnOnce(Result<Option<String>, String>, &mut App) + 'static,
    ) {
        let Some(ix) = self.out.iter().position(|(out, _)| *out == id) else { return };
        let (_, patch) = self.out.remove(ix);
        // The host kept it, and its record may come after the answer: the pick stands until it does.
        if answer.is_ok() {
            if let Some(host) = &mut self.host {
                merge(host, &patch);
            }
        }
        self.reshow(cx);
        cx.defer(move |cx| then(answer, cx));
    }

    fn reshow(&mut self, cx: &mut Context<Self>) {
        let Some(mut record) = self.host.clone() else { return };
        for (_, patch) in &self.out {
            merge(&mut record, patch);
        }
        // A record this build cannot read keeps what every reader drew from the last one it could.
        let shown = serde_json::from_value(record.clone())
            .ok()
            .or_else(|| self.host.clone().and_then(|h| serde_json::from_value(h).ok()))
            .or_else(|| self.shown.clone());
        if self.record.as_ref() != Some(&record) {
            self.shown = shown;
            self.record = Some(record);
            cx.notify();
        }
    }
}

/// Lays a preferences patch onto a record as the host does: a null deletes the key, an object merges into one.
pub fn merge(into: &mut Value, patch: &Value) {
    match (into, patch) {
        (Value::Object(into), Value::Object(patch)) => {
            for (key, value) in patch {
                match value {
                    Value::Null => {
                        into.remove(key);
                    }
                    Value::Object(_) if into.get(key).is_some_and(Value::is_object) => merge(into.get_mut(key).unwrap(), value),
                    _ => {
                        into.insert(key.clone(), value.clone());
                    }
                }
            }
        }
        (into, patch) => *into = patch.clone(),
    }
}

/// `preferences.set` with its patch sent as written: the typed request drops a null, and a null is how a patch deletes a
/// key (a row's reset, a project's default put back).
#[derive(serde::Serialize)]
struct PatchPreferences {
    patch: Value,
}

/// The answer's notice alone: the record comes as preferences.changed, and a record this build cannot read does not
/// turn a write the host kept into a refusal.
#[derive(serde::Deserialize)]
struct PatchAnswer {
    notice: Option<String>,
}

impl Op for PatchPreferences {
    const NAME: &'static str = "preferences.set";
    type Reply = PatchAnswer;
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use gpui_kit::{div, prelude::*, Context, TestAppContext, VisualTestContext, Window};
    use serde_json::json;

    use crate::desktop::feed::{switched, FeedState, OnQuit};
    use crate::screens::pr_pane::files::open_split::{want_editors, Editors};
    use crate::screens::pr_pane::pull_request::link::default_agent;
    use crate::screens::settings::door::{self, Door};
    use crate::screens::sidebar::host::Change;

    struct Blank;

    impl Render for Blank {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
        }
    }

    fn waits(cx: &mut VisualTestContext, what: &str, mut done: impl FnMut(&mut VisualTestContext) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            cx.run_until_parked();
            if done(cx) {
                return;
            }
            assert!(Instant::now() < deadline, "never came to: {what}");
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    /// Catches: a view that follows the host's record on its own road, so while a write is out it shows the host's old
    /// value, and the host's record from before the write, heard on that road, paints over the pick.
    #[gpui_kit::test]
    fn a_write_still_out_shows_in_every_view_that_reads_the_preferences(cx: &mut TestAppContext) {
        cx.executor().allow_parking();
        cx.update(|cx| {
            gpui_kit::init(cx);
            crate::ui::init(crate::ui::Side::Light, cx);
        });
        let old = json!({ "theme": "system", "sidebarMode": "list", "terminalSize": "app", "terminalZoom": {}, "access": {},
            "projectLook": {}, "projectOrder": [], "labs": false, "settleAfter": "15m", "sendWith": "enter", "onQuit": "ask",
            "defaultAgent": "claude", "editor": "zed" });
        let door = Door::start(json!({
            "preferences.get": { "reply": { "preferences": old } },
            "preferences.set": { "delayMs": 60000, "reply": {} },
        }));
        let runtime = door::runtime();
        let client = door.client(&runtime);
        let host = crate::screens::Host::new(client.clone(), runtime.clone());
        let mut views = None;
        let (_, cx) = cx.add_window_view(|window, cx| {
            let sidebar = cx.new(crate::screens::sidebar::Sidebar::new);
            let composer = cx.new(|cx| crate::screens::composer::Composer::new(window, cx));
            let usage = cx.new(|cx| crate::screens::usage::UsagePage::new(host.clone(), window, cx));
            let add = crate::screens::dialogs::add_computer::staged(window, cx, |_, _, _| {});
            views = Some((sidebar, composer, usage, add));
            gpui_kit::component::Root::new(cx.new(|_| Blank), window, cx)
        });
        let (sidebar, composer, usage, add) = views.unwrap();
        sidebar.update(cx, |s, cx| s.apply(Change::Preferences(old.clone()), cx));
        let mine = json!({ "settleAfter": "1h", "sendWith": "mod-enter", "onQuit": "stop", "defaultAgent": "codex",
            "editor": "cursor", "projectLook": { "p1": { "icon": "rocket" } } });
        cx.update(|_, cx| super::store(cx).update(cx, |store, cx| store.write(mine, Some((client, runtime)), cx, |_, _| {})));
        cx.run_until_parked();
        // The host's record from before the write, as the sidebar's own road hears it again.
        sidebar.update(cx, |s, cx| s.apply(Change::Preferences(old.clone()), cx));
        cx.run_until_parked();
        assert_eq!(cx.update(|_, cx| super::store(cx).read(cx).writes_out()), 1, "the write is still out");
        let read = |cx: &mut VisualTestContext| {
            cx.update(|_, cx| {
                let mut fed = FeedState::default();
                switched(&mut fed, cx);
                let looks = usage.read(cx).preferences.as_ref().is_some_and(|p| p.project_look.contains_key("p1"));
                vec![
                    ("sidebar", sidebar.read(cx).preferences().get("settleAfter") == Some(&json!("1h"))),
                    ("thread header", sidebar.read(cx).preferences().pointer("/projectLook/p1/icon") == Some(&json!("rocket"))),
                    ("composer", composer.read(cx).send_with_mod()),
                    ("feed", fed.prefs.on_quit == OnQuit::Stop),
                    ("usage", looks),
                    ("add computer", add.read(cx).project_look().pointer("/p1/icon") == Some(&json!("rocket"))),
                    ("pull request agent", default_agent(cx).as_deref() == Some("codex")),
                ]
            })
        };
        let stale: Vec<&str> = read(cx).into_iter().filter(|(_, new)| !new).map(|(view, _)| view).collect();
        assert!(stale.is_empty(), "these show the host's old value while the write is out: {stale:?}");
        cx.update(|_, cx| want_editors(&host, cx));
        waits(cx, "the editor pick", |cx| cx.update(|_, cx| cx.try_global::<Editors>().is_some_and(|e| e.picked.is_some())));
        let picked = cx.update(|_, cx| cx.global::<Editors>().picked.clone());
        assert_eq!(picked.as_deref(), Some("cursor"), "the pull request pane's editor");
    }
}
