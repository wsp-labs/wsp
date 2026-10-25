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

impl Prefs {
    pub fn shown(&self) -> Option<&Preferences> {
        self.shown.as_ref()
    }

    /// How many writes are out.
    pub fn writes_out(&self) -> usize {
        self.out.len()
    }

    /// A record the host sent, read or pushed.
    pub fn landed(&mut self, record: &Preferences, cx: &mut Context<Self>) {
        self.host = serde_json::to_value(record).ok();
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
        let shown = serde_json::from_value(record).ok().or_else(|| self.host.clone().and_then(|h| serde_json::from_value(h).ok()));
        if serde_json::to_value(&shown).ok() != serde_json::to_value(&self.shown).ok() {
            self.shown = shown;
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
