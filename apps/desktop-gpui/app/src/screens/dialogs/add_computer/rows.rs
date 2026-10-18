// SPDX-License-Identifier: AGPL-3.0-only
//! What the running view's rows ask of the host (`SetupList` in `AddComputerDialog.tsx`): Retry, Skip, Copy the keys
//! and each way of signing in, each on its own row; a sign-in run watched under its row; and each step's last lines of
//! output, read when a row opens and every 3 s while an open row runs.

use std::collections::HashMap;

use gpui_kit::{Context, Task};
use serde_json::{json, Value};
use wsp_host_client::protocol::{PlacesSetupLogRequest, PlacesSetupRequest, PlacesSkipRequest};
use wsp_host_client::Failure;

use super::model::{self, str_of};
use super::{ask, value, words, AddComputer, OUTPUT_EVERY};
use crate::screens::dialogs::sign_in::{self, Ask, Heard, Start};
use crate::ui::{catalog, Step};

/// One act on one row of the setup.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowAct {
    Retry,
    Skip,
    CopyKeys,
    /// The row's own sign-in on the computer, the way the catalog says it goes there.
    SignIn,
    /// One way of an agent with several: "machine" its login on the computer, "token" made on the computer running the
    /// host, "key" the key pasted into this host's vault or the row run again taking it.
    Way(&'static str),
    /// The sign-in drawn under the row goes, and a run stops on the host.
    Cancel,
}

/// The log as the rows last asked for it: which rows stood open, whether a read is out, and the 3 s read while an open
/// row runs.
#[derive(Default)]
pub struct Logs {
    open: Vec<String>,
    reading: bool,
    poll: Option<Task<()>>,
    /// How many reads were sent while the dialog stands.
    pub sent: usize,
}

impl AddComputer {
    /// The rows of the running view that stand open on their log, and whether each runs.
    fn open_rows(&self) -> Vec<(String, bool)> {
        let Some(place) = self.place() else { return Vec::new() };
        model::setup_rows(place, &model::place_name(place))
            .into_iter()
            .filter(|r| model::opens_log(r) && *self.turned.get(&r.id).unwrap_or(&(r.state == Step::Failed)))
            .map(|r| (r.id, r.state == Step::Running))
            .collect()
    }

    /// Reads the log once when a row opens, then every 3 s while an open row runs; stops once none does.
    pub(super) fn sync_log(&mut self, cx: &mut Context<Self>) {
        let rows = if matches!(self.step, super::AddStep::Running) { self.open_rows() } else { Vec::new() };
        let ids: Vec<String> = rows.iter().map(|(id, _)| id.clone()).collect();
        let fresh = ids.iter().any(|id| !self.logs_asked.open.contains(id));
        self.logs_asked.open = ids;
        let live = rows.iter().any(|(_, running)| *running);
        if !live {
            self.logs_asked.poll = None;
        }
        if fresh {
            self.read_log(cx);
        }
        if live && self.logs_asked.poll.is_none() {
            self.logs_asked.poll = Some(cx.spawn(async move |this, cx| loop {
                cx.background_executor().timer(OUTPUT_EVERY).await;
                if this.update(cx, |this, cx| this.read_log(cx)).is_err() {
                    return;
                }
            }));
        }
    }

    /// One read of the box's setup log; a read still out is not doubled.
    fn read_log(&mut self, cx: &mut Context<Self>) {
        if self.logs_asked.reading || self.host.is_none() {
            return;
        }
        let Some(place) = self.place_id.clone() else { return };
        let Ok(op) = ask::<PlacesSetupLogRequest>(json!({ "placeId": place })) else { return };
        self.logs_asked.reading = true;
        self.logs_asked.sent += 1;
        self.call(op, cx, |this, r, cx| {
            this.logs_asked.reading = false;
            if let Ok(read) = r {
                this.take_log(&read.lines, cx);
            }
        });
    }

    /// The press of one row's act. Busy holds the row's acts from the moment a call is sent until it answers.
    pub(super) fn row_act(&mut self, row: String, act: RowAct, cx: &mut Context<Self>) {
        let Some(place) = self.place().cloned() else { return };
        let id = str_of(&place, "id").unwrap_or_default().to_owned();
        let agent = model::sign_in_of_row(&row).unwrap_or_default();
        let picked =
            place.get("picks").and_then(|p| p.pointer(&format!("/agents/{agent}/signin"))).and_then(Value::as_str).map(str::to_owned);
        let start = |this: &mut Self, start: Start, at: String, cx: &mut Context<Self>| {
            this.sign_ins_at.insert(row.clone(), at);
            if let Some(ask) = this.sign_ins.start(&row, &start) {
                this.send_sign_in(ask, cx);
            }
            cx.notify();
        };
        let params = match act {
            RowAct::Cancel => {
                if let Some(ask) = self.sign_ins.cancel(&row) {
                    self.send_sign_in(ask, cx);
                }
                cx.notify();
                return;
            }
            RowAct::Way("key") if picked.as_deref() == Some("key") => {
                return start(self, Start::Vault { agent, token: false, mint: None }, "here".into(), cx);
            }
            RowAct::Way("token") => return start(self, Start::Run { agent, pastes: false }, "here".into(), cx),
            RowAct::Way("machine") => return start(self, Start::Run { pastes: catalog::there_pastes(&agent), agent }, id, cx),
            RowAct::SignIn => {
                let road = catalog::sign_in(&agent).map_or("oauth", |s| s.road);
                // On a computer the person added, a terminal's login runs there and is watched in place.
                let begun = match sign_in::start_of(&agent, road) {
                    Some(Start::Copy { .. }) => Some(Start::Run { agent, pastes: false }),
                    other => other,
                };
                let Some(begun) = begun else { return };
                return start(self, begun, id, cx);
            }
            RowAct::Way(way) => {
                let Some(mut picks) = self.picks_or_say(&place, &row) else { return };
                picks["agents"][agent.as_str()] = json!({ "signin": way });
                json!({ "ref": id, "choices": picks, "row": row })
            }
            RowAct::CopyKeys => {
                let Some(mut picks) = self.picks_or_say(&place, &row) else { return };
                picks["copyKeys"] = json!(true);
                json!({ "ref": id, "choices": picks, "row": row })
            }
            RowAct::Retry => json!({ "ref": id, "row": row }),
            RowAct::Skip => {
                let Ok(op) = ask::<PlacesSkipRequest>(json!({ "placeId": id, "row": row })) else { return };
                self.busy = self.call(op, cx, move |this, r, _| {
                    this.busy = false;
                    match r {
                        Ok(p) => {
                            this.row_refused = None;
                            this.show_setup(value(&p.place), None);
                        }
                        Err(f) => this.refuse_row(&row, f),
                    }
                });
                cx.notify();
                return;
            }
        };
        let Ok(op) = ask::<PlacesSetupRequest>(params) else { return };
        self.busy = self.call(op, cx, move |this, r, _| {
            this.busy = false;
            match r {
                Ok(answer) => {
                    this.row_refused = None;
                    this.show_setup(value(&answer.place), answer.setup.as_ref().map(value));
                    if let Some(said) = answer.said {
                        this.refuse_row(&row, Failure::new(said));
                    }
                }
                Err(f) => this.refuse_row(&row, f),
            }
        });
        cx.notify();
    }

    /// The picks a row's act sends again with a change (`copyKeysOn`, `setupWithWay`); a computer that carries none
    /// says so under the row, and nothing is sent.
    fn picks_or_say(&mut self, place: &Value, row: &str) -> Option<Value> {
        let picks = place.get("picks").filter(|p| !p.is_null()).cloned();
        if picks.is_none() {
            self.row_refused = Some((row.to_owned(), model::Said::plain(words::no_picks(&model::place_name(place)))));
        }
        picks
    }

    /// A refusal said under the row whose act asked; the host's conflict is a press while the computer is busy.
    fn refuse_row(&mut self, row: &str, f: Failure) {
        let name = self.place().map(model::place_name).unwrap_or_default();
        let said =
            if f.kind.as_deref() == Some("conflict") { model::Said::plain(words::still_setting_up(&name)) } else { model::Said::from(&f) };
        self.row_refused = Some((row.to_owned(), said));
    }

    /// Sends one call a sign-in asked for; a run starts on the computer its row's way named.
    pub(super) fn send_sign_in(&mut self, ask: Ask, cx: &mut Context<Self>) {
        let Some(host) = self.host.clone() else { return };
        match ask {
            Ask::Begin { row, agent } => {
                let at = self.sign_ins_at.get(&row).cloned().unwrap_or_else(|| "here".into());
                let begun = host.run(move |c| async move { sign_in::begin(&c, at, agent).await });
                cx.spawn(async move |this, cx| {
                    let begun = begun.await;
                    let _ = this.update(cx, |this, cx| {
                        if let Some(stop) = this.sign_ins.on_begun(&row, begun) {
                            this.send_sign_in(stop, cx);
                        }
                        cx.notify();
                    });
                })
                .detach();
            }
            Ask::Stop { id } => {
                let stopped = host.run(move |c| async move { sign_in::stop(&c, id).await });
                cx.spawn(async move |_, _| drop(stopped.await)).detach();
            }
            Ask::Code { row, id, code } => {
                let sent = host.run(move |c| async move { sign_in::code(&c, id, code).await });
                cx.spawn(async move |this, cx| {
                    let sent = sent.await;
                    let _ = this.update(cx, |this, cx| {
                        this.sign_ins.on_code(&row, sent);
                        cx.notify();
                    });
                })
                .detach();
            }
            Ask::Key { row, agent, key, token } => {
                let saved = host.run(move |c| async move { sign_in::key(&c, agent, key, token).await });
                cx.spawn(async move |this, cx| {
                    let saved = saved.await;
                    let landed = saved.is_ok();
                    let _ = this.update(cx, |this, cx| {
                        this.sign_ins.on_saved(&row, saved);
                        if landed {
                            this.sign_in_fields.remove(&row);
                            this.reread_places(cx);
                        }
                        cx.notify();
                    });
                })
                .detach();
            }
        }
    }

    /// A step the host pushed for a sign-in run; one that signed in leaves its row, and the places are read again.
    pub fn hear_sign_in(&mut self, heard: &Heard, cx: &mut Context<Self>) {
        if self.sign_ins.hear(heard) {
            self.reread_places(cx);
        }
        cx.notify();
    }

    /// The sign-in standing under a row, if one does.
    pub fn sign_in_of(&self, row: &str) -> Option<&sign_in::Flow> {
        self.sign_ins.of(row)
    }

    /// What the host said under the row whose act it refused, and that row.
    pub fn row_refused(&self) -> Option<&(String, model::Said)> {
        self.row_refused.as_ref()
    }

    /// How many reads of the setup log the dialog sent.
    pub fn log_reads(&self) -> usize {
        self.logs_asked.sent
    }
}

/// Where each row's sign-in runs, by row.
pub type At = HashMap<String, String>;
