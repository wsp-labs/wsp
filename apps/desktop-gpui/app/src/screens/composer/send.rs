// SPDX-License-Identifier: AGPL-3.0-only
//! The composer's sends: a new turn, a steer or a message onto the queue, and what the host answers of each.

use gpui_kit::{prelude::*, App, Context, Window};

use super::model::{self, File, Hold, Queued, Submit};
use super::{store, Composer, ComposerEvent, Outgoing};
use crate::local_store::LocalStore;

/// A typed send the host refused where no box holds its thread: its words go ahead of the draft the store keeps for it.
fn refused_into_store(key: &str, prompt: &str, cx: &mut App) {
    let Some(s) = cx.try_global::<LocalStore>() else { return };
    let draft = store::draft(s, key).map(|(d, _)| d).unwrap_or_default();
    let text = if draft.trim().is_empty() { prompt.to_owned() } else { format!("{prompt}\n{draft}") };
    if let Err(e) = cx.update_global(|s: &mut LocalStore, _| store::set_draft(s, key, &text, text.len())) {
        eprintln!("desktop-gpui: store: {e}");
    }
}

impl Composer {
    /// Whether the thread has a message or a running turn, and whether a turn runs now. A turn that ends sends the
    /// queue's head.
    pub fn set_turn(&mut self, compact: bool, running: bool, cx: &mut Context<Self>) {
        self.compact = compact || running;
        self.running = running;
        if running && self.hold == Some(Hold::InFlight) {
            self.hold = None;
        }
        if running {
            self.answered();
        }
        if !running {
            self.stopping = false;
        }
        if !running {
            self.drain(cx);
        }
        cx.notify();
    }

    /// Sends the queue's head as the next turn, once no turn runs, nothing holds the send and no steer is out.
    fn drain(&mut self, cx: &mut Context<Self>) {
        let out = self.thread_key.as_ref().is_some_and(|k| self.sending.contains(k));
        if self.running
            || self.hold.is_some()
            || self.absent.is_some()
            || self.steering.is_some()
            || self.queue_held
            || self.queue.is_empty()
            || out
        {
            return;
        }
        let next = self.queue.remove(0);
        self.hold = Some(Hold::InFlight);
        self.sending.extend(self.thread_key.clone());
        let mut out = self.outgoing(next.prompt, next.files);
        out.from_queue = true;
        cx.emit(ComposerEvent::Start(out));
        self.keep(cx);
    }

    /// The thread on screen has no send out any more.
    fn answered(&mut self) {
        if let Some(key) = &self.thread_key {
            self.sending.remove(key);
        }
    }

    /// The host answered a send on the thread `key`; answered after a switch, it lets that thread send again.
    pub fn sent_on(&mut self, key: &str, cx: &mut Context<Self>) {
        if self.holds(key) {
            return self.sent(cx);
        }
        self.sending.remove(key);
    }

    /// The host took a send: a turn starts on it, so nothing waits on the reply any more.
    pub fn sent(&mut self, cx: &mut Context<Self>) {
        self.answered();
        if self.hold == Some(Hold::InFlight) {
            self.hold = None;
        }
        // The conversation is continued once the host took the send; a refused one keeps the banner for the retry.
        if self.banner.as_ref().is_some_and(|b| b.resume.is_some()) {
            self.banner = None;
        }
        self.compact = true;
        cx.notify();
    }

    pub fn set_hold(&mut self, hold: Option<Hold>, cx: &mut Context<Self>) {
        self.hold = hold;
        cx.notify();
    }

    /// Whether a message sent mid-turn goes into the turn (the person's `midTurn`).
    pub fn steer_preferred(&self) -> bool {
        self.steer_preferred
    }

    pub fn set_steer_preferred(&mut self, steer: bool, cx: &mut Context<Self>) {
        self.steer_preferred = steer;
        cx.notify();
    }

    /// Whether Cmd or Ctrl and Enter sends and Enter alone breaks the line (the person's `sendWith`).
    pub fn send_with_mod(&self) -> bool {
        self.send_with_mod
    }

    pub fn set_send_with_mod(&mut self, mod_enter: bool, cx: &mut Context<Self>) {
        self.send_with_mod = mod_enter;
        cx.notify();
    }

    /// A steer the host has answered: taken by the turn, or left to wait for its end.
    pub fn steered(&mut self, taken: bool, cx: &mut Context<Self>) {
        if let Some(id) = self.steering.take() {
            if taken {
                self.queue.retain(|q| q.id != id);
                self.keep(cx);
            }
        }
        self.drain(cx);
        cx.notify();
    }

    /// A steer on the thread `key` the host has answered. Answered after a switch, it settles that thread's parked queue,
    /// and nothing of the thread on screen.
    pub fn steered_on(&mut self, key: &str, taken: bool, cx: &mut Context<Self>) {
        if self.holds(key) {
            return self.steered(taken, cx);
        }
        if let Some(parked) = self.parked.get_mut(key) {
            if let Some(id) = parked.steering.take().filter(|_| taken) {
                parked.queue.retain(|q| q.id != id);
                let (queue, here) = (parked.queue.clone(), self.thread_key.clone());
                if cx.has_global::<LocalStore>() {
                    if let Err(e) = cx.update_global(|s: &mut LocalStore, _| store::set_queue(s, key, &queue, here.as_deref())) {
                        eprintln!("desktop-gpui: store: {e}");
                    }
                }
            }
        }
    }

    /// A send on the thread `key` the host refused. Refused after a switch, a queued send goes back to the head of that
    /// thread's parked queue, held, and a typed one ahead of the draft kept for it.
    pub fn refused_on(&mut self, key: &str, sent: Outgoing, window: &mut Window, cx: &mut Context<Self>) {
        if self.holds(key) {
            return self.refused_send(sent, window, cx);
        }
        self.sending.remove(key);
        if sent.from_queue {
            self.next_id += 1;
            let parked = self.parked.entry(key.to_owned()).or_default();
            parked.queue.insert(0, Queued { id: self.next_id, prompt: sent.prompt, files: sent.files });
            parked.held = true;
            let (queue, here) = (parked.queue.clone(), self.thread_key.clone());
            if cx.has_global::<LocalStore>() {
                if let Err(e) = cx.update_global(|s: &mut LocalStore, _| store::set_queue(s, key, &queue, here.as_deref())) {
                    eprintln!("desktop-gpui: store: {e}");
                }
            }
            return;
        }
        if !sent.files.is_empty() {
            self.refused_files.entry(key.to_owned()).or_default().extend(sent.files);
        }
        refused_into_store(key, &sent.prompt, cx);
    }

    /// `prompt` went to the thread `key` another way, as the start line's Try again sends it: the draft that is those
    /// words goes, so it is not sent twice (SpawnLine.tsx).
    pub fn sent_elsewhere(&mut self, key: &str, prompt: &str, window: &mut Window, cx: &mut Context<Self>) {
        if self.holds(key) && self.text(cx) == prompt {
            self.set_text("", window, cx);
        }
    }

    /// A send the host refused: its words go back in the box ahead of any draft written since, and its files with them.
    pub fn refused_send(&mut self, sent: Outgoing, window: &mut Window, cx: &mut Context<Self>) {
        self.answered();
        self.hold = None;
        if sent.from_queue {
            self.next_id += 1;
            self.queue.insert(0, Queued { id: self.next_id, prompt: sent.prompt, files: sent.files });
            self.queue_held = true;
            self.keep(cx);
            cx.notify();
            return;
        }
        let draft = self.text(cx);
        let text = if draft.trim().is_empty() { sent.prompt } else { format!("{}\n{draft}", sent.prompt) };
        self.set_text(&text, window, cx);
        self.files.extend(sent.files);
        cx.notify();
    }

    pub fn stopped(&mut self, cx: &mut Context<Self>) {
        self.stopping = false;
        cx.notify();
    }

    fn outgoing(&mut self, prompt: String, files: Vec<File>) -> Outgoing {
        let picks = self.resolved();
        let fast = self.fast() && picks.model.as_ref().is_some_and(|m| m.fast == Some(true));
        let resume = self.banner.as_ref().and_then(|b| b.resume.clone());
        Outgoing { prompt, files, picks, fast, resume, from_queue: false, fork: self.forking.clone() }
    }

    /// The draft as it goes: each pull request's or issue's chip opened back into its quoted text.
    pub(super) fn prompt(&self, cx: &App) -> String {
        let editor = self.editor.read(cx);
        let mut text = editor.value().to_string();
        let mut spans: Vec<_> = editor.tokens().iter().map(|t| (t.range(), t.token().text().to_string())).collect();
        spans.sort_by_key(|(range, _)| std::cmp::Reverse(range.start));
        for (range, head) in spans {
            if let Some(whole) = self.expansions.get(&head) {
                text.replace_range(range, whole);
            }
        }
        text
    }

    /// Enter: a new turn, a steer, or a message onto the queue.
    pub fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Enter holds what the send button holds (ChatComposer.tsx `send`): the draft stays in the box and a note says
        // why in the button's hover words. A turn in flight is the button's word alone, not a hold on Enter.
        let in_flight = self.hold == Some(Hold::InFlight);
        let held = self
            .send_held(cx)
            .filter(|why| !in_flight || why != Hold::InFlight.words())
            .or_else(|| model::slash_hold(&self.text(cx), &self.commands_offered()));
        if let Some(why) = held {
            crate::ui::notify(cx, crate::ui::Notice::note(why));
            return;
        }
        let asides = self.asides();
        if let Some(question) = crate::screens::aside::question(&self.text(cx)).filter(|_| asides) {
            cx.emit(ComposerEvent::Aside(question));
            return;
        }
        let prompt = self.prompt(cx);
        let empty = prompt.trim().is_empty();
        let resolved = self.resolved();
        // One steer is out at a time: the steer's answer names the row it carried (ChatComposer.tsx `canSteer`).
        let steers = self.steer_preferred && self.steering.is_none();
        // A fork's draft starts a thread of its own, whatever this one is doing.
        let running = self.running && self.forking.is_none();
        let how = model::submit(empty, self.hold.as_ref(), running, steers, resolved.agent.as_ref(), &self.files);
        if how == Submit::Nothing {
            return;
        }
        let files = std::mem::take(&mut self.files);
        self.refused.clear();
        self.menu = None;
        self.clear_box(window, cx);
        self.saving = None;
        self.save_draft(cx);
        let out = self.outgoing(prompt.trim_end().to_owned(), files);
        self.expansions.clear();
        self.recalled = None;
        let held = std::mem::take(&mut self.queue_held);
        match how {
            Submit::Start => {
                self.hold = Some(Hold::InFlight);
                self.sending.extend(self.thread_key.clone());
                self.compact = true;
                cx.emit(ComposerEvent::Start(out));
            }
            Submit::Steer => {
                self.next_id += 1;
                self.queue.insert(0, Queued { id: self.next_id, prompt: out.prompt.clone(), files: out.files.clone() });
                self.steering = Some(self.next_id);
                cx.emit(ComposerEvent::Steer(out));
            }
            Submit::Queue => {
                self.next_id += 1;
                let row = Queued { id: self.next_id, prompt: out.prompt, files: out.files };
                // Over a queue read back from the store, the new message goes first (composerDraftStore.ts).
                if held {
                    self.queue.insert(0, row);
                } else {
                    self.queue.push(row);
                }
            }
            Submit::Nothing => {}
        }
        self.keep(cx);
        cx.notify();
    }

    pub fn stop(&mut self, cx: &mut Context<Self>) {
        if self.running && !self.stopping {
            self.stopping = true;
            cx.emit(ComposerEvent::Stop);
            cx.notify();
        }
    }
}
