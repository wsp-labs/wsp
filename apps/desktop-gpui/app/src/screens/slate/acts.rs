// SPDX-License-Identifier: AGPL-3.0-only
//! What a press, a write, an approval and the menu's acts send the host, and the steps the window runs itself.

use std::collections::HashMap;
use std::time::Duration;

use gpui_kit::{ClipboardItem, Context, Task};
use serde_json::{json, Map, Value};
use wsp_host_client::protocol;

use super::model::{self, Doc};
use super::{call, consent, open_pane, refusal, words, Load, Outcome, Raise, SlateScreen};
use crate::screens::Pane;

impl SlateScreen {
    /// Writes one value the person typed or picked: drawn at once, then sent with `slates.state`.
    pub fn write(&mut self, path: String, value: Value, cx: &mut Context<Self>) {
        self.write_then(path, value, |_, _| {}, cx);
    }

    /// `write`, then `then` once the host has answered it, taken or not, as `StateSender` never fails a write: a pick's
    /// change event reads the value it wrote.
    pub fn write_then(
        &mut self,
        path: String,
        value: Value,
        then: impl FnOnce(&mut Self, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) {
        let Some(thread) = self.opened.thread_id.clone() else { return };
        if let Load::Drawn(slate) = &mut self.load {
            slate.queue(path.clone(), value.clone());
        }
        let sent = self.writes.entry(path.clone()).or_default();
        *sent += 1;
        let sent = *sent;
        cx.notify();
        let (asked, sending) = (path.clone(), value.clone());
        let asking = self.host.run(move |host| async move {
            let mut values = Map::new();
            values.insert(asked, sending);
            call::<protocol::SlatesStateRequest>(&host, json!({ "threadId": thread, "values": values })).await
        });
        cx.spawn(async move |this, cx| {
            let answer = asking.await;
            _ = this.update(cx, |this, cx| {
                if let Load::Drawn(slate) = &mut this.load {
                    if this.writes.get(&path) == Some(&sent) {
                        this.writes.remove(&path);
                        // Words typed while it was out are still the field's to send.
                        if slate.mine.get(&path) == Some(&value) {
                            slate.queued.remove(&path);
                            if answer.is_ok() {
                                slate.mine.remove(&path);
                            }
                        }
                        if answer.is_ok() {
                            slate.record.set_own(&path, value);
                        }
                    }
                }
                this.resolve(cx);
                then(this, cx);
            });
        })
        .detach();
    }

    /// A press, a submit or a change: the host runs the piece's steps off its stored version through
    /// `slates.event` and answers an outcome, with a held run's ask when it held one.
    pub fn raise(&mut self, piece: String, event: &'static str, raise: Raise, cx: &mut Context<Self>) {
        let (Some(thread), Some(slate)) = (self.opened.thread_id.clone(), self.slate()) else { return };
        let version = slate.record.version;
        let steps = raise.steps.clone().or_else(|| slate.doc().and_then(|d| d.piece(&piece)).and_then(|p| p.on.get(event).cloned()));
        let steps: Vec<Value> = match steps {
            Some(Value::Array(a)) => a,
            Some(one @ Value::Object(_)) => vec![one],
            _ => Vec::new(),
        };
        let host_steps = steps.iter().any(|s| {
            matches!(s.get("do").and_then(Value::as_str), Some("set" | "toggle" | "start" | "cancel" | "send" | "steer" | "queue"))
        });
        let window_steps: Vec<Value> = steps
            .iter()
            .filter(|s| matches!(s.get("do").and_then(Value::as_str), Some("open" | "copy" | "fill" | "pane")))
            .cloned()
            .collect();
        if let Load::Drawn(slate) = &mut self.load {
            slate.busy.insert(piece.clone());
            slate.outcomes.remove(&piece);
        }
        cx.notify();
        let request = REQUESTS.with(|n| {
            let next = n.get() + 1;
            n.set(next);
            format!("{thread}:{version}:{piece}:{}:{next}", window_id())
        });
        let mut params = json!({ "threadId": thread, "version": version, "piece": piece, "event": event, "requestId": request });
        if let Some((item, index)) = &raise.row {
            params["scope"] = json!({ "item": item, "index": index });
        }
        if let Some(action) = raise.row_action {
            params["rowAction"] = json!(action);
        }
        // What a row's own open, copy or fill reads is asked with the row as the one element of a list, the only row
        // scope a formula sent to the host can carry.
        let row_asks: Vec<(String, String)> = match &raise.row {
            Some((item, index)) => window_steps
                .iter()
                .flat_map(|step| {
                    let with =
                        step.get("with").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).map(str::to_owned);
                    ["target", "text"].into_iter().filter_map(|k| step.get(k)).flat_map(model::leaves).chain(with).collect::<Vec<_>>()
                })
                .map(|leaf| (format!("[{}] | map(_slate0: {})", model::literal(item), model::shifted(&leaf, *index)), leaf))
                .filter(|(formula, _)| formula.len() <= model::EXPR_CHARS)
                .collect(),
            None => Vec::new(),
        };
        let row_thread = thread.clone();
        let asking = self.host.run(move |host| async move {
            let answer = if host_steps {
                call::<protocol::SlatesEventRequest>(&host, params).await.map(|a| serde_json::to_value(a).ok())?
            } else {
                None
            };
            let mut rows = HashMap::new();
            if !row_asks.is_empty() {
                let paths: Vec<&String> = row_asks.iter().map(|(f, _)| f).collect();
                let read = call::<protocol::SlatesResolveRequest>(&host, json!({ "threadId": row_thread, "paths": paths })).await?;
                for (formula, leaf) in row_asks {
                    if let Some(v) = read.values.get(&formula).and_then(|v| v.get(0)).and_then(|v| v.get("_slate0")) {
                        rows.insert(leaf, v.clone());
                    }
                }
            }
            Ok((answer, rows))
        });
        cx.spawn(async move |this, cx| {
            let answer = asking.await;
            _ = this.update(cx, |this, cx| {
                if let Load::Drawn(slate) = &mut this.load {
                    slate.busy.remove(&piece);
                }
                match answer {
                    Err(failure) => this.set_outcome(&piece, Outcome::Refused(refusal(&failure))),
                    Ok((answer, rows)) => {
                        let answer = answer.unwrap_or(Value::Null);
                        if let Some(ask) = answer.get("ask").and_then(|a| serde_json::from_value::<model::Ask>(a.clone()).ok()) {
                            this.consent.asking = Some(ask);
                            // The held run's row reads the record's asks, which no push carries.
                            this.reload(cx);
                        }
                        if let Some(said) = answer.get("said").and_then(Value::as_str).filter(|s| !s.is_empty()) {
                            this.set_outcome(&piece, Outcome::Said(said.to_owned()));
                        }
                        for step in &window_steps {
                            if let Some(outcome) = this.window_step(step, &rows, cx) {
                                let refused = matches!(outcome, Outcome::Refused(_));
                                this.set_outcome(&piece, outcome);
                                if refused {
                                    break;
                                }
                            }
                        }
                    }
                }
                this.fade(piece.clone(), cx);
                cx.notify();
            });
        })
        .detach();
    }

    fn set_outcome(&mut self, piece: &str, outcome: Outcome) {
        if let Load::Drawn(slate) = &mut self.load {
            slate.outcomes.insert(piece.to_owned(), outcome);
        }
    }

    /// Keeps what the person typed drawn over a write from elsewhere until they keep it or take the other.
    pub fn keep_drawn(&mut self, path: String, value: Value, cx: &mut Context<Self>) {
        if let Load::Drawn(slate) = &mut self.load {
            slate.queued.remove(&path);
            slate.mine.insert(path, value);
        }
        cx.notify();
    }

    /// Clears a piece's outcome line after two seconds.
    fn fade(&mut self, piece: String, cx: &mut Context<Self>) {
        let Some(shown) = self.slate().and_then(|s| s.outcomes.get(&piece)).cloned() else { return };
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_secs(2)).await;
            _ = this.update(cx, |this, cx| {
                if let Load::Drawn(slate) = &mut this.load {
                    if slate.outcomes.get(&piece) != Some(&shown) {
                        return;
                    }
                    slate.outcomes.remove(&piece);
                    cx.notify();
                }
            });
        })
        .detach();
    }

    /// The steps the window runs itself: open a link, copy, fill the composer, open a pane.
    fn window_step(&mut self, step: &Value, rows: &HashMap<String, Value>, cx: &mut Context<Self>) -> Option<Outcome> {
        let slate = self.slate()?;
        let value_of = |v: Option<&Value>| {
            let v = v?;
            model::filled(v, rows).or_else(|| model::filled(v, &slate.resolved))
        };
        match step.get("do").and_then(Value::as_str)? {
            "open" => {
                let href = value_of(step.get("target")).and_then(|v| v.as_str().map(str::to_owned));
                match href {
                    Some(h) if consent::opens(&h) => {
                        self.open_link(h, cx);
                        None
                    }
                    _ => Some(Outcome::Refused("Only http, https and mailto links open from a slate.".into())),
                }
            }
            "copy" => {
                cx.write_to_clipboard(ClipboardItem::new_string(copied(value_of(step.get("text")))));
                Some(Outcome::Said("Copied".into()))
            }
            "fill" => {
                let text = step.get("text").and_then(Value::as_str).unwrap_or_default();
                let with: Vec<String> = step
                    .get("with")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .map(|path| format!("> {path}: {}", rows.get(path).or_else(|| slate.resolved.get(path)).unwrap_or(&Value::Null)))
                    .collect();
                self.filling = Some(if with.is_empty() { text.to_owned() } else { format!("{text}\n\n{}", with.join("\n")) });
                cx.notify();
                None
            }
            "pane" => match step.get("kind").and_then(Value::as_str).and_then(pane_of) {
                Some(pane) if open_pane(pane, cx) => None,
                _ => Some(Outcome::Refused(words::NO_PANE.into())),
            },
            _ => None,
        }
    }

    /// Cancels one of the slate's runs, as the output's Cancel and the menu's Stop runs do.
    pub fn cancel(&mut self, run: String, cx: &mut Context<Self>) {
        let Some(thread) = self.opened.thread_id.clone() else { return };
        let asking = self
            .host
            .run(move |host| async move { call::<protocol::SlatesCancelRequest>(&host, json!({ "threadId": thread, "run": run })).await });
        cx.spawn(async move |this, cx| {
            let answer = asking.await;
            _ = this.update(cx, |this, cx| {
                if let Err(f) = answer {
                    this.refused = Some(refusal(&f));
                }
                this.reload(cx);
            });
        })
        .detach();
    }

    /// The menu's write: undo or clear, the record read again after, a refusal under the header.
    pub fn menu_write(&mut self, text: &'static str, cx: &mut Context<Self>) {
        let Some(thread) = self.opened.thread_id.clone() else { return };
        self.refused = None;
        let asking = self
            .host
            .run(move |host| async move { call::<protocol::SlatesWriteRequest>(&host, json!({ "threadId": thread, "text": text })).await });
        cx.spawn(async move |this, cx| {
            let answer = asking.await;
            _ = this.update(cx, |this, cx| {
                match answer {
                    Err(f) => this.refused = Some(refusal(&f)),
                    Ok(_) => this.reload(cx),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Copy as text: the sketch the agent reads.
    pub fn copy_sketch(&mut self, cx: &mut Context<Self>) {
        let Some(thread) = self.opened.thread_id.clone() else { return };
        let asking = self.host.run(move |host| async move {
            call::<protocol::SlatesReadRequest>(&host, json!({ "threadId": thread, "sketch": true, "text": false })).await
        });
        cx.spawn(async move |this, cx| {
            let answer = asking.await;
            _ = this.update(cx, |this, cx| match answer {
                Ok(read) => cx.write_to_clipboard(ClipboardItem::new_string(read.text)),
                Err(f) => {
                    this.refused = Some(refusal(&f));
                    cx.notify();
                }
            });
        })
        .detach();
    }

    /// Stop runs: cancels every run whose record says it is running.
    pub fn stop_runs(&mut self, cx: &mut Context<Self>) {
        let Some(slate) = self.slate() else { return };
        let running: Vec<String> = slate
            .doc()
            .map(|d| d.runs.keys().filter(|r| slate.record.run_state(r) == Some("running")).cloned().collect())
            .unwrap_or_default();
        for run in running {
            self.cancel(run, cx);
        }
    }

    /// An approval's answer for one ask: once, for the thread, or a refusal; the record read again after.
    pub fn approve(&mut self, key: String, scope: &'static str, cx: &mut Context<Self>) -> Task<Result<(), String>> {
        let Some(thread) = self.opened.thread_id.clone() else { return Task::ready(Err("No thread".into())) };
        let asking = self.host.run(move |host| async move {
            call::<protocol::SlatesApproveRequest>(&host, json!({ "threadId": thread, "key": key, "scope": scope })).await
        });
        cx.spawn(async move |this, cx| {
            let answer = asking.await.map(|_| ()).map_err(|f| refusal(&f));
            _ = this.update(cx, |this, cx| this.reload(cx));
            answer
        })
    }

    /// Keeps a section's fold where no state path keeps it.
    pub fn fold(&mut self, id: String, open: bool, cx: &mut Context<Self>) {
        if let Load::Drawn(slate) = &mut self.load {
            slate.folds.insert(id, open);
            cx.notify();
        }
        self.resolve(cx);
    }

    /// Keeps a bar switch's pick, by the first list it switches.
    pub fn pick(&mut self, first: String, list: String, cx: &mut Context<Self>) {
        if let Load::Drawn(slate) = &mut self.load {
            slate.picks.insert(first, list);
            cx.notify();
        }
    }

    pub fn revoke(&mut self, key: String, cx: &mut Context<Self>) -> Task<Result<(), String>> {
        let Some(thread) = self.opened.thread_id.clone() else { return Task::ready(Err("No thread".into())) };
        let asking = self
            .host
            .run(move |host| async move { call::<protocol::SlatesRevokeRequest>(&host, json!({ "threadId": thread, "key": key })).await });
        cx.spawn(async move |this, cx| {
            let answer = asking.await.map(|_| ()).map_err(|f| refusal(&f));
            _ = this.update(cx, |this, cx| this.reload(cx));
            answer
        })
    }

    /// Forget secrets: each secret written empty, which the host takes as cleared.
    pub fn forget_secrets(&mut self, cx: &mut Context<Self>) -> Task<Result<(), String>> {
        let (Some(thread), Some(slate)) = (self.opened.thread_id.clone(), self.slate()) else { return Task::ready(Ok(())) };
        let secrets = slate.doc().map(Doc::secrets).unwrap_or_default();
        let asking = self.host.run(move |host| async move {
            let values: Map<String, Value> = secrets.into_iter().map(|s| (s, Value::String(String::new()))).collect();
            call::<protocol::SlatesStateRequest>(&host, json!({ "threadId": thread, "values": values })).await
        });
        cx.spawn(async move |this, cx| {
            let answer = asking.await.map(|_| ()).map_err(|f| refusal(&f));
            _ = this.update(cx, |this, cx| this.reload(cx));
            answer
        })
    }
}

thread_local! {
    static REQUESTS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
    static WINDOW: String = format!("{:x}", std::process::id() as u64 ^ rand_seed());
}

fn rand_seed() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos() as u64)
}

/// This window's own part of every request id: the host keeps ids from every window for minutes.
fn window_id() -> String {
    WINDOW.with(Clone::clone)
}

/// What a copy step writes, as the web's `JSON.stringify(text ?? "")` does: a string as it is, nothing as `""`, and
/// anything else as JSON.
pub(super) fn copied(text: Option<Value>) -> String {
    match text.unwrap_or(Value::Null) {
        Value::String(s) => s,
        Value::Null => "\"\"".to_owned(),
        other => other.to_string(),
    }
}

/// The right panel's pane a `pane` step names (`SLATE_PANE_KINDS`).
fn pane_of(kind: &str) -> Option<Pane> {
    Some(match kind {
        "preview" => Pane::Browser,
        "terminal" => Pane::Terminal,
        "diff" => Pane::Changes,
        "pr" => Pane::PullRequest,
        "files" => Pane::Files,
        "machine" => Pane::Computer,
        "processes" => Pane::Processes,
        "agents" => Pane::Agents,
        "slate" => Pane::Slate,
        _ => return None,
    })
}
