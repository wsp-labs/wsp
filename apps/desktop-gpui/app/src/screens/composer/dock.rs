// SPDX-License-Identifier: AGPL-3.0-only
//! The thread's own prompt in the composer's place (PromptDock.tsx, in Dock.tsx's frame): a head with the agent's mark
//! and the question, the command as a copy row, the file the call changes, the options as pick rows, the field Deny and
//! Other open, and the foot. Digits pick a row, the arrows move the pick, Space ticks, Enter is the primary button,
//! Esc leaves the field and then folds the dock to the drawer's first row. The dock closes on the event the host
//! records, never on the reply; a reply that refuses the pick is said under the options.

use gpui_kit::component::input::{Enter, Escape, InputState};
use gpui_kit::component::{h_flex, v_flex, ActiveTheme as _};
use gpui_kit::{div, img, prelude::*, px, AnyElement, Context, Focusable as _, FontWeight, KeyDownEvent, SharedString, Window};
use serde_json::{Map, Value};

use super::{Composer, ComposerEvent};
use crate::screens::thread_view::cards::Refusal;
use crate::transcript::{Docked, RowKind};
use crate::ui::{catalog, glyph_frame, size, text, ActivePalette, Button, Card, CardItem, Choice, CopyRow, Field, Glyph, Line, PickRow};

/// The last row of every question, as the CLI draws it: a pick that is none of the above, typed.
const OTHER: &str = "other";
const OTHER_LABEL: &str = "Other";
const OTHER_FIELD: &str = "Type your answer";
const DENY_FIELD: &str = "What to do instead";
pub const WRITE_INSTEAD: &str = "Write a message instead";

const TOOL_WORDS: &str = include_str!("../../../../../../packages/protocol/src/words/tools.ts");

/// `ANSWER_WORDS`, read from its source: why the host would not take a pick it answered.
pub fn answer_words(outcome: &str) -> Option<&'static str> {
    let block = &TOOL_WORDS[TOOL_WORDS.find("export const ANSWER_WORDS")?..];
    block[..block.find("\n};")?].lines().find_map(|line| {
        let (key, words) = line.trim().split_once(": \"")?;
        (key.trim_matches('"') == outcome).then(|| words.strip_suffix("\",")).flatten()
    })
}

/// `sentence` in answerPrompt.ts: capitalised, ending in a full stop.
pub fn sentence(line: &str) -> String {
    let line = if line.ends_with('.') { line.to_owned() } else { format!("{line}.") };
    let mut chars = line.chars();
    chars.next().map(|c| c.to_uppercase().chain(chars).collect()).unwrap_or_default()
}

/// What the dock says of an answer the host refused (answerPrompt.ts): its sentence and, where the host gave one, the
/// fix, each a sentence, the fix drawn in the body ink after it (`PromptRefusal`).
pub fn refusal_of(failure: &wsp_host_client::Failure) -> Refusal {
    Refusal { said: sentence(failure.said()), fix: failure.fix.as_deref().map(sentence) }
}

/// One row the person can pick: a consent's option, or one answer to a question.
#[derive(Clone, Debug, PartialEq)]
pub struct Pick {
    pub id: String,
    pub label: String,
    pub description: String,
    /// allow, deny, mode or answer.
    pub effect: String,
    pub mode: Option<String>,
}

/// One question of a form, where the call put it (`AskedQuestion`).
#[derive(Clone, Debug, PartialEq)]
pub struct Question {
    pub index: usize,
    pub header: String,
    pub question: String,
    pub options: Vec<Pick>,
    pub multiple: bool,
}

/// The file a call writes or edits: the lines that go and come, or the text it writes.
#[derive(Clone, Debug, PartialEq)]
pub struct FileCard {
    pub path: String,
    pub edit: Option<(String, String)>,
    pub body: Option<String>,
}

/// The prompt the dock answers, worded as the transcript words it.
#[derive(Clone, Debug, PartialEq)]
pub struct Prompt {
    pub session: String,
    pub ask: String,
    /// The agent asking, whose mark heads the dock.
    pub agent: Option<String>,
    pub title: String,
    pub note: Option<String>,
    pub code: Option<String>,
    /// The folder a command runs in, where it is not the thread's.
    pub runs_in: Option<String>,
    pub fields: Vec<(String, String)>,
    pub file: Option<FileCard>,
    pub cwd: Option<String>,
    pub options: Vec<Pick>,
    pub questions: Option<Vec<Question>>,
    /// What the drawer's first row says while the dock is folded.
    pub asked: String,
}

/// A pick sent to `sessions.answer`; on a deny, the words typed into its field.
#[derive(Clone, Debug, PartialEq)]
pub struct Answer {
    pub session: String,
    pub ask: String,
    pub option: String,
    pub reason: Option<String>,
}

fn object(input: &str) -> Option<Map<String, Value>> {
    match serde_json::from_str(input) {
        Ok(Value::Object(fields)) => Some(fields),
        _ => None,
    }
}

/// `editStrings`: the strings an edit swaps, off the first of several edits, else the call's own.
fn edit_strings(fields: &Map<String, Value>) -> Option<(String, String)> {
    let first = fields.get("edits").and_then(Value::as_array).and_then(|e| e.first()).and_then(Value::as_object).unwrap_or(fields);
    Some((first.get("old_string")?.as_str()?.to_owned(), first.get("new_string")?.as_str()?.to_owned()))
}

impl Prompt {
    /// The dock's prompt off the transcript's, the agent asking named by its harness.
    pub fn of(docked: &Docked, agent: Option<String>) -> Option<Prompt> {
        let fields = object(docked.input);
        let text =
            |name: &str| fields.as_ref().and_then(|f| f.get(name)).and_then(Value::as_str).filter(|t| !t.is_empty()).map(str::to_owned);
        let row = &docked.row;
        let base = |session: &str, ask: &str| Prompt {
            session: session.to_owned(),
            ask: ask.to_owned(),
            agent: agent.clone(),
            title: String::new(),
            note: None,
            code: None,
            runs_in: None,
            fields: Vec::new(),
            file: None,
            cwd: docked.cwd.map(str::to_owned),
            options: Vec::new(),
            questions: None,
            asked: row.text.clone(),
        };
        match &row.kind {
            RowKind::Permission { ask, session, asker: None, lead, code, fields: rest, file: body, options, open: true, .. } => {
                let path = text("file_path");
                let note = match docked.tool {
                    "WebFetch" => text("prompt"),
                    "command_execution" => docked.detail.filter(|d| !d.is_empty()).map(str::to_owned),
                    _ => None,
                };
                let shown_rest = if body.is_none() && path.is_none() { rest.clone() } else { Vec::new() };
                let file = path.map(|path| FileCard { path, edit: fields.as_ref().and_then(edit_strings), body: body.clone() });
                let options = options
                    .iter()
                    .map(|o| Pick {
                        id: o.id.clone(),
                        label: o.label.clone(),
                        description: String::new(),
                        effect: o.effect.clone(),
                        mode: o.mode.clone(),
                    })
                    .collect();
                Some(Prompt {
                    title: lead.strip_suffix(':').unwrap_or(lead).to_owned(),
                    note,
                    code: code.clone(),
                    runs_in: text("cwd").filter(|c| Some(c.as_str()) != docked.cwd),
                    fields: shown_rest,
                    file,
                    options,
                    ..base(session, ask)
                })
            }
            RowKind::Question { ask, session, asker: None, questions, outcome: None } => {
                let questions = questions
                    .iter()
                    .enumerate()
                    .map(|(at, q)| Question {
                        index: q.options.first().and_then(|o| o.id.strip_prefix('q')?.split_once(':')?.0.parse().ok()).unwrap_or(at),
                        header: q.header.clone().unwrap_or_default(),
                        question: q.question.clone(),
                        options: q
                            .options
                            .iter()
                            .map(|o| Pick {
                                id: o.id.clone(),
                                label: o.label.clone(),
                                description: o.description.clone().unwrap_or_default(),
                                effect: o.effect.clone(),
                                mode: None,
                            })
                            .collect(),
                        multiple: q.multiple,
                    })
                    .collect::<Vec<_>>();
                let asked = questions.first().map(|q| q.question.clone()).unwrap_or_else(|| row.text.clone());
                Some(Prompt { questions: Some(questions), asked, ..base(session, ask) })
            }
            _ => None,
        }
    }
}

/// One step's answer as it was left: its picks and its typed words.
#[derive(Clone, Debug, Default, PartialEq)]
struct Settled {
    ids: Vec<String>,
    typed: Option<String>,
}

/// One row of the options card: a name, its sentence, and the field it opens.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub id: String,
    pub label: String,
    pub description: String,
    pub field: Option<&'static str>,
}

/// What the primary button does with the pick as it stands.
#[derive(Debug, PartialEq)]
pub enum Confirmed {
    /// Nothing can go yet: a tick list with no tick, a field with no words.
    Held,
    /// The next question, with the words its field opens on.
    Next(String),
    Answer {
        option: String,
        reason: Option<String>,
    },
}

/// The dock's state: the prompt, the question it is on, the pick, every step's answer, and whether it is folded.
#[derive(Clone, Debug, PartialEq)]
pub struct Dock {
    pub prompt: Prompt,
    pub step: usize,
    pub picked: String,
    answers: Vec<Settled>,
    pub folded: bool,
    /// What the host said about a pick it would not take.
    pub refused: Option<Refusal>,
    typed_some: bool,
    /// The tick row the arrows and Space act on in a tick list.
    cursor: Option<usize>,
}

/// `encodeURIComponent`: the words a typed answer rides its id in.
fn escape(words: &str) -> String {
    words
        .bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'!' | b'~' | b'*' | b'\'' | b'(' | b')' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

impl Dock {
    pub fn new(prompt: Prompt) -> Dock {
        let steps = prompt.questions.as_ref().map_or(0, Vec::len);
        let mut dock = Dock {
            prompt,
            step: 0,
            picked: String::new(),
            answers: vec![Settled::default(); steps],
            folded: false,
            refused: None,
            typed_some: false,
            cursor: None,
        };
        dock.picked = dock.rows().first().map(|r| r.id.clone()).unwrap_or_default();
        dock
    }

    pub fn question(&self) -> Option<&Question> {
        self.prompt.questions.as_ref().and_then(|q| q.get(self.step))
    }

    fn consent(&self) -> bool {
        self.prompt.questions.is_none()
    }

    pub fn multi(&self) -> bool {
        self.question().is_some_and(|q| q.multiple)
    }

    /// A question with nothing to pick from is the field alone, open from the start.
    pub fn text_only(&self) -> bool {
        self.question().is_some_and(|q| q.options.is_empty())
    }

    pub fn rows(&self) -> Vec<Row> {
        match self.question() {
            Some(q) => q
                .options
                .iter()
                .map(|o| Row { id: o.id.clone(), label: o.label.clone(), description: o.description.clone(), field: None })
                .chain([Row { id: OTHER.to_owned(), label: OTHER_LABEL.to_owned(), description: String::new(), field: Some(OTHER_FIELD) }])
                .collect(),
            None => self
                .prompt
                .options
                .iter()
                .map(|o| Row {
                    id: o.id.clone(),
                    label: o.label.clone(),
                    description: String::new(),
                    field: (o.effect == "deny").then_some(DENY_FIELD),
                })
                .collect(),
        }
    }

    pub fn ticked(&self) -> &[String] {
        self.answers.get(self.step).map_or(&[], |a| a.ids.as_slice())
    }

    /// The field a row opens is open: the question's own, Other's while ticked, or the picked row's.
    pub fn field_open(&self) -> bool {
        if self.text_only() {
            return true;
        }
        if self.multi() {
            return self.ticked().iter().any(|t| t == OTHER);
        }
        self.rows().iter().any(|r| r.id == self.picked && r.field.is_some())
    }

    /// The placeholder of the open field.
    pub fn field_word(&self) -> &'static str {
        if self.consent() {
            DENY_FIELD
        } else {
            OTHER_FIELD
        }
    }

    /// Whether the primary button is held: a tick list needs a tick or words, a field row needs its words.
    pub fn held(&self) -> bool {
        if self.multi() {
            let ticked = self.ticked();
            return !ticked.iter().any(|t| t != OTHER) && !(ticked.iter().any(|t| t == OTHER) && self.typed_some);
        }
        self.field_open() && !self.consent() && !self.typed_some
    }

    pub fn primary_word(&self) -> &'static str {
        match &self.prompt.questions {
            None => match self.prompt.options.iter().find(|o| o.id == self.picked) {
                Some(o) if o.effect == "deny" => "Deny",
                _ => "Allow",
            },
            Some(q) if self.step + 1 < q.len() => "Next",
            Some(_) => "Answer",
        }
    }

    /// "n of N" while the form holds more than one question.
    pub fn count(&self) -> Option<String> {
        self.prompt.questions.as_ref().filter(|q| q.len() > 1).map(|q| format!("{} of {}", self.step + 1, q.len()))
    }

    /// The questions already answered, each as "<header>: <picks>".
    pub fn earlier(&self) -> Vec<String> {
        let Some(questions) = &self.prompt.questions else { return Vec::new() };
        questions[..self.step.min(questions.len())]
            .iter()
            .zip(&self.answers)
            .map(|(q, a)| {
                let picks: Vec<String> = a
                    .ids
                    .iter()
                    .map(|id| q.options.iter().find(|o| o.id == *id).map_or(id.clone(), |o| o.label.clone()))
                    .chain(a.typed.clone())
                    .collect();
                format!("{}: {}", if q.header.is_empty() { &q.question } else { &q.header }, picks.join(", "))
            })
            .collect()
    }

    pub fn set_typed(&mut self, typed: &str) {
        self.typed_some = !typed.trim().is_empty();
    }

    pub fn tick(&mut self, id: &str, on: bool) {
        if let Some(answer) = self.answers.get_mut(self.step) {
            answer.ids.retain(|t| t != id);
            if on {
                answer.ids.push(id.to_owned());
            }
        }
    }

    /// A digit: a tick list ticks its row; a list of one picks it, and on the picked row that opens a field, says to
    /// put the keys in the field.
    pub fn digit(&mut self, n: usize) -> bool {
        let rows = self.rows();
        let Some(row) = n.checked_sub(1).and_then(|at| rows.get(at)) else { return false };
        if self.multi() {
            let on = !self.ticked().contains(&row.id);
            self.tick(&row.id, on);
            self.cursor = Some(n - 1);
            return false;
        }
        if self.picked == row.id && row.field.is_some() {
            return true;
        }
        self.picked = row.id.clone();
        false
    }

    /// An arrow: the pick moves round the rows; in a tick list the row Space ticks moves instead.
    pub fn arrow(&mut self, by: isize) {
        let rows = self.rows();
        let n = rows.len() as isize;
        if n == 0 {
            return;
        }
        if self.multi() {
            let from = self.cursor.map_or(if by > 0 { -1 } else { n }, |c| c as isize);
            self.cursor = Some((from + by).rem_euclid(n) as usize);
            return;
        }
        let at = rows.iter().position(|r| r.id == self.picked).unwrap_or(0) as isize;
        self.picked = rows[(at + by).rem_euclid(n) as usize].id.clone();
    }

    /// Space in a tick list: the row the arrows stand on ticks or unticks.
    pub fn space(&mut self) {
        if !self.multi() {
            return;
        }
        let rows = self.rows();
        if let Some(row) = self.cursor.and_then(|c| rows.get(c)) {
            let on = !self.ticked().contains(&row.id);
            self.tick(&row.id, on);
        }
    }

    /// Esc in the field: a list of one goes back to its first row; a tick list keeps its ticks.
    pub fn leave_field(&mut self) {
        if !self.multi() && !self.text_only() {
            self.picked = self.rows().first().map(|r| r.id.clone()).unwrap_or_default();
        }
    }

    /// Onto a step as it was last left; answers the words its field opens on.
    fn go_to(&mut self, at: usize) -> String {
        let Some(coming) = self.prompt.questions.as_ref().and_then(|q| q.get(at)).cloned() else { return String::new() };
        let held = self.answers.get(at).cloned().unwrap_or_default();
        let typed = held.typed.is_some();
        self.step = at;
        self.picked = held.ids.first().cloned().unwrap_or_else(|| match coming.options.first() {
            Some(first) if !typed => first.id.clone(),
            _ => OTHER.to_owned(),
        });
        if coming.multiple && typed && !held.ids.iter().any(|i| i == OTHER) {
            if let Some(answer) = self.answers.get_mut(at) {
                answer.ids.push(OTHER.to_owned());
            }
        }
        self.typed_some = typed;
        self.cursor = None;
        held.typed.unwrap_or_default()
    }

    /// Back: the step before, as it was left.
    pub fn back(&mut self) -> Option<String> {
        (self.step > 0).then(|| self.go_to(self.step - 1))
    }

    fn settle(&mut self, settled: Settled) -> Confirmed {
        let Some(questions) = self.prompt.questions.clone() else { return Confirmed::Held };
        if let Some(answer) = self.answers.get_mut(self.step) {
            *answer = settled;
        }
        if self.step + 1 < questions.len() {
            return Confirmed::Next(self.go_to(self.step + 1));
        }
        let ids: Vec<String> = self
            .answers
            .iter()
            .zip(&questions)
            .flat_map(|(a, q)| a.ids.iter().cloned().chain(a.typed.as_ref().map(|t| format!("q{}:other:{}", q.index, escape(t)))))
            .collect();
        Confirmed::Answer { option: ids.join(crate::screens::thread_view::cards::PICK_JOIN), reason: None }
    }

    /// Enter or the primary button: the pick as it stands, with the field's words where a field is open.
    pub fn confirm(&mut self, typed: &str) -> Confirmed {
        if self.held() {
            return Confirmed::Held;
        }
        let typed = if self.field_open() { typed.trim().to_owned() } else { String::new() };
        if self.consent() {
            let rows = self.rows();
            let Some(row) = rows.iter().find(|r| r.id == self.picked) else { return Confirmed::Held };
            let reason = row.field.and(Some(typed)).filter(|t| !t.is_empty());
            return Confirmed::Answer { option: row.id.clone(), reason };
        }
        if self.multi() {
            let ids = self.ticked().iter().filter(|t| *t != OTHER).cloned().collect();
            return self.settle(Settled { ids, typed: (!typed.is_empty()).then_some(typed) });
        }
        if self.picked == OTHER || self.text_only() {
            return self.settle(Settled { ids: Vec::new(), typed: Some(typed) });
        }
        let ids = vec![self.picked.clone()];
        self.settle(Settled { ids, typed: None })
    }
}

impl Composer {
    /// The prompt the thread's own agent waits on, or none. A new ask opens the dock unfolded and takes the keys; the
    /// same ask keeps its step and its pick.
    pub fn set_prompt(&mut self, prompt: Option<Prompt>, window: &mut Window, cx: &mut Context<Self>) {
        match prompt {
            None if self.dock.is_none() => return,
            None => {
                self.dock_edit = None;
                if self.dock.take().is_some_and(|d| !d.folded) && self.dock_focus.contains_focused(window, cx) {
                    self.editor.focus_handle(cx).focus(window, cx);
                }
            }
            Some(prompt) if self.dock.as_ref().is_some_and(|d| d.prompt.ask == prompt.ask) => return,
            Some(prompt) => {
                let mut dock = Dock::new(prompt);
                // A prompt folded on this thread stays folded when the thread shows again (composerBar.ts `folded`).
                dock.folded = self.thread_key.as_ref().is_some_and(|k| self.folded.get(k) == Some(&dock.prompt.ask));
                let (word, folded) = (dock.field_word(), dock.folded);
                self.dock_edit = dock.prompt.file.as_ref().and_then(|f| f.edit.as_ref()).map(|(old, next)| EditLines::of(old, next));
                self.dock = Some(dock);
                self.dock_field.update(cx, |f, cx| {
                    f.set_value("", window, cx);
                    f.set_placeholder(word, window, cx);
                });
                if !folded {
                    self.bar = None;
                    self.popup = None;
                    self.menu = None;
                    self.dock_focus.focus(window, cx);
                }
            }
        }
        self.sync_docked(cx);
        cx.notify();
    }

    /// Says in `Docked` whether this thread's prompt stands in the composer's place, where that changed.
    pub(super) fn sync_docked(&mut self, cx: &mut gpui_kit::App) {
        let standing = self.thread_key.clone().filter(|_| self.dock.as_ref().is_some_and(|d| !d.folded));
        if standing == self.docked_key {
            return;
        }
        let left = std::mem::replace(&mut self.docked_key, standing.clone());
        if !cx.has_global::<super::Docked>() {
            cx.set_global(super::Docked::default());
        }
        cx.update_global(|d: &mut super::Docked, _| {
            if let Some(left) = left {
                d.0.remove(&left);
            }
            d.0.extend(standing);
        });
    }

    pub fn dock(&self) -> Option<&Dock> {
        self.dock.as_ref()
    }

    /// Whether the dock has the keys.
    pub fn dock_focused(&self, window: &Window, cx: &gpui_kit::App) -> bool {
        self.dock_focus.contains_focused(window, cx)
    }

    /// Write a message instead, or Esc: the dock folds to the drawer's first row and the box has the keys.
    pub fn fold_prompt(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.fold_dock() {
            self.sync_docked(cx);
            self.editor.focus_handle(cx).focus(window, cx);
            cx.notify();
        }
    }

    /// Folds the dock and keeps its ask as this thread's folded one; false where no prompt stands.
    pub(super) fn fold_dock(&mut self) -> bool {
        let Some(dock) = self.dock.as_mut() else { return false };
        dock.folded = true;
        if let Some(key) = self.thread_key.clone() {
            self.folded.insert(key, dock.prompt.ask.clone());
        }
        true
    }

    /// The drawer's question row: the dock stands in the composer's place again.
    pub fn unfold_prompt(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(key) = &self.thread_key {
            self.folded.remove(key);
        }
        if let Some(dock) = &mut self.dock {
            dock.folded = false;
            self.bar = None;
            self.popup = None;
            self.dock_focus.focus(window, cx);
            self.sync_docked(cx);
            cx.notify();
        }
    }

    /// What the host said about a pick on `ask` it would not take, said under the options; none clears it.
    pub fn prompt_refused(&mut self, ask: &str, said: Option<Refusal>, cx: &mut Context<Self>) {
        if let Some(dock) = self.dock.as_mut().filter(|d| d.prompt.ask == ask) {
            dock.refused = said;
            cx.notify();
        }
    }

    /// A key on the dock while it has the keys and no field does: digits, arrows, Space, Enter and Esc are the dock's;
    /// a letter folds it and starts a message.
    fn dock_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.dock_field.focus_handle(cx).is_focused(window) {
            return;
        }
        let keys = &event.keystroke;
        if keys.modifiers.platform || keys.modifiers.control || keys.modifiers.alt {
            return;
        }
        let Some(was_open) = self.dock.as_ref().map(Dock::field_open) else { return };
        match keys.key.as_str() {
            "escape" => self.fold_prompt(window, cx),
            "enter" => self.dock_confirm(window, cx),
            "up" | "down" | "space" => {
                let dock = self.dock.as_mut().expect("checked above");
                match keys.key.as_str() {
                    "up" => dock.arrow(-1),
                    "down" => dock.arrow(1),
                    _ => dock.space(),
                }
                self.focus_opened_field(was_open, window, cx);
            }
            digit if digit.len() == 1 && matches!(digit.as_bytes()[0], b'1'..=b'9') => {
                self.dock_digit((digit.as_bytes()[0] - b'0') as usize, window, cx);
            }
            _ => {
                if let Some(key) = super::screen::types_into_composer(event) {
                    let key = key.to_owned();
                    self.type_key(&key, window, cx);
                    cx.stop_propagation();
                }
                return;
            }
        }
        cx.stop_propagation();
        cx.notify();
    }

    /// A digit for the dock, typed on it or outside it: its row is picked or ticked, and a field the pick opens or a
    /// second press on a field's row gives the field the keys.
    pub(super) fn dock_digit(&mut self, n: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(was_open) = self.dock.as_ref().map(Dock::field_open) else { return };
        if self.dock.as_mut().is_some_and(|d| d.digit(n)) {
            self.dock_field.focus_handle(cx).focus(window, cx);
        }
        self.focus_opened_field(was_open, window, cx);
        cx.notify();
    }

    /// A field a pick opens takes the keys as it opens, as PromptDock.tsx focuses it once `fieldOpen` turns true.
    fn focus_opened_field(&mut self, was_open: bool, window: &mut Window, cx: &mut Context<Self>) {
        if !was_open && self.dock.as_ref().is_some_and(Dock::field_open) {
            self.dock_field.focus_handle(cx).focus(window, cx);
        }
    }

    fn dock_confirm(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let typed = self.dock_field.read(cx).value().to_string();
        let Some(dock) = self.dock.as_mut() else { return };
        match dock.confirm(&typed) {
            Confirmed::Held => {}
            Confirmed::Next(words) => {
                self.dock_field.update(cx, |f, cx| f.set_value(words, window, cx));
                self.dock_focus.focus(window, cx);
            }
            Confirmed::Answer { option, reason } => {
                dock.refused = None;
                let answer = Answer { session: dock.prompt.session.clone(), ask: dock.prompt.ask.clone(), option, reason };
                cx.emit(ComposerEvent::Answer(answer));
            }
        }
        cx.notify();
    }

    fn dock_back(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(words) = self.dock.as_mut().and_then(Dock::back) {
            self.dock_field.update(cx, |f, cx| f.set_value(words, window, cx));
            self.dock_focus.focus(window, cx);
            cx.notify();
        }
    }

    fn on_dock_escape(&mut self, _: &Escape, window: &mut Window, cx: &mut Context<Self>) {
        if !self.dock_field.focus_handle(cx).is_focused(window) {
            return;
        }
        cx.stop_propagation();
        // A row's field goes with the row it stood on, so a pick of that row again opens it empty (`key={step:row.id}`).
        if let Some(dock) = self.dock.as_mut() {
            dock.leave_field();
            if !dock.field_open() {
                dock.set_typed("");
                self.dock_field.update(cx, |f, cx| f.set_value("", window, cx));
            }
        }
        self.dock_focus.focus(window, cx);
        cx.notify();
    }

    fn on_dock_enter(&mut self, _: &Enter, window: &mut Window, cx: &mut Context<Self>) {
        if self.dock_field.focus_handle(cx).is_focused(window) {
            cx.stop_propagation();
            self.dock_confirm(window, cx);
        }
    }

    pub(super) fn on_dock_field(&mut self, state: &gpui_kit::Entity<InputState>, cx: &mut Context<Self>) {
        let typed = state.read(cx).value().to_string();
        if let Some(dock) = self.dock.as_mut() {
            dock.set_typed(&typed);
            cx.notify();
        }
    }
}

/// The open field: in its row's right column wide enough for a sentence, or the question's own across the card.
fn field(c: &Composer, own: bool) -> AnyElement {
    if own {
        Field::new(&c.dock_field).sheet_words().into_any_element()
    } else {
        Field::new(&c.dock_field).width(size::SENTENCE_FIELD_WIDTH).into_any_element()
    }
}

/// `rowHover`: the keys that pick a row, since the foot carries no hint strip.
fn row_hover(at: usize, multi: bool) -> String {
    if multi {
        format!("{} or Space ticks this, Enter answers, Esc puts the question away", at + 1)
    } else {
        format!("{} picks this, Enter answers, Esc puts the question away", at + 1)
    }
}

/// An edit's lines, the ones that go then the ones that come, each with whether it comes; and the list that builds
/// only the lines in view.
pub(super) struct EditLines {
    lines: std::rc::Rc<Vec<(bool, String)>>,
    list: gpui_kit::ListState,
}

impl EditLines {
    fn of(old: &str, next: &str) -> EditLines {
        let lines: Vec<(bool, String)> =
            old.split('\n').map(|t| (false, t.to_owned())).chain(next.split('\n').map(|t| (true, t.to_owned()))).collect();
        let list = gpui_kit::ListState::new(lines.len(), gpui_kit::ListAlignment::Top, px(192.));
        EditLines { lines: std::rc::Rc::new(lines), list }
    }
}

/// The edit as the step log draws a step's output: the lines that go with their sign, then the lines that come; the
/// sign keeps a column of its own so a wrapped line hangs under its text.
fn edit_lines(edit: &EditLines, cx: &Context<Composer>) -> AnyElement {
    let p = cx.palette().clone();
    let lines = edit.lines.clone();
    // As tall as its lines, up to `max-h-48`; past it the list scrolls.
    let tall = (px(16.) * lines.len() as f32).min(px(192.));
    let rows = gpui_kit::list(edit.list.clone(), move |ix, _, _| {
        let (comes, text) = &lines[ix];
        let (sign, ink) = if *comes { ("+", p.success) } else { ("-", p.error_foreground) };
        h_flex()
            .items_start()
            .text_color(ink)
            .child(div().flex_none().w(px(16.)).child(sign))
            .child(div().flex_1().min_w_0().child(if text.is_empty() { " ".to_owned() } else { text.clone() }))
            .into_any_element()
    })
    .h(tall);
    v_flex()
        .debug_selector(|| "prompt-edit".into())
        .pl(px(64.))
        .pr(px(20.))
        .pb_4()
        .font_family(cx.theme().mono_font_family.clone())
        .text_size(px(11.))
        .line_height(px(16.))
        .child(rows)
        .into_any_element()
}

/// The dock in the composer's place.
pub(super) fn view(c: &Composer, dock: &Dock, cx: &mut Context<Composer>) -> AnyElement {
    let p = cx.palette().clone();
    let prompt = &dock.prompt;
    let label = prompt.agent.as_ref().map(|h| c.agents.iter().find(|a| a.harness == *h).map_or(h.clone(), |a| a.label.clone()));
    // The dock's mark takes the glyph ink, `GLYPH`'s text colour winning over the mark's own (PromptDock.tsx).
    let ink = p.foreground.opacity(0.8);
    let mark = prompt.agent.as_ref().map(|h| match catalog::mono(h, ink) {
        Some(mark) => img(mark).size(px(16.)).flex_none().into_any_element(),
        None => catalog::mark(h, label.as_deref().unwrap_or(h), px(16.), ink, cx),
    });
    let question = dock.question();
    let title = question.map_or(prompt.title.clone(), |q| q.question.clone());
    // A question's header word stands under it only where the question does not already say it.
    let note = match question {
        None => prompt.note.clone(),
        Some(q) if q.header.is_empty() || q.question.to_lowercase().contains(&q.header.to_lowercase()) => None,
        Some(q) => Some(q.header.clone()),
    };
    let quiet = |text: String| div().text_size(px(13.)).line_height(px(20.)).text_color(p.muted_foreground).child(text);
    let head = h_flex()
        .items_start()
        .justify_between()
        .gap_6()
        .px_5()
        .pt_4()
        .pb_3()
        .child(
            v_flex()
                .min_w_0()
                .gap_1()
                .child(
                    h_flex()
                        .debug_selector(|| "prompt-title".into())
                        .min_w_0()
                        .items_center()
                        .gap_2()
                        .text_size(px(16.))
                        .line_height(px(24.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(p.foreground)
                        .children(mark)
                        .child(div().min_w_0().child(title)),
                )
                .children(note.map(quiet))
                .children(dock.earlier().into_iter().map(|line| quiet(line).truncate())),
        )
        .children(
            dock.count()
                .map(|count| div().flex_none().text_size(px(13.)).line_height(px(19.5)).text_color(p.muted_foreground).child(count)),
        );

    let mut body: Vec<AnyElement> = Vec::new();
    if question.is_none() {
        if let Some(code) = &prompt.code {
            let ground = p.background.blend(super::parts::glass(cx).tint);
            body.push(
                v_flex()
                    .gap_2()
                    .child(CopyRow::new("prompt-code", code.clone()).on(ground))
                    .children(prompt.runs_in.as_ref().map(|dir| {
                        div().truncate().text_size(px(13.)).line_height(px(19.5)).text_color(p.muted_foreground).child(format!("in {dir}"))
                    }))
                    .into_any_element(),
            );
        }
        if let Some(file) = &prompt.file {
            let name = file.path.trim_end_matches('/').rsplit('/').next().unwrap_or(&file.path).to_owned();
            let shown = match &prompt.cwd {
                Some(cwd) => file.path.strip_prefix(&format!("{cwd}/")).unwrap_or(&file.path).to_owned(),
                None => file.path.clone(),
            };
            let row = h_flex().min_h(px(60.)).py_3().px(px(20.)).gap_3().child(glyph_frame(Glyph::File, cx)).child(
                v_flex()
                    .min_w_0()
                    .child(div().truncate().text_size(px(15.)).line_height(px(24.)).font_weight(FontWeight::MEDIUM).child(name))
                    .child(div().truncate().text_size(px(13.)).line_height(px(19.5)).text_color(p.muted_foreground).child(shown)),
            );
            let under = match (&file.edit, &file.body) {
                (Some(_), _) => c.dock_edit.as_ref().map(|edit| edit_lines(edit, cx)),
                (None, Some(text)) => Some(
                    div()
                        .id("prompt-body")
                        .max_h(px(192.))
                        .overflow_y_scroll()
                        .pl(px(64.))
                        .pr(px(20.))
                        .pb_4()
                        .font_family(cx.theme().mono_font_family.clone())
                        .text_size(px(11.))
                        .line_height(px(16.))
                        .text_color(p.muted_foreground)
                        .child(text.clone())
                        .into_any_element(),
                ),
                (None, None) => None,
            };
            body.push(Card::new("prompt-file").item(CardItem::any(v_flex().child(row).children(under))).into_any_element());
        }
        if !prompt.fields.is_empty() {
            let lines = prompt.fields.iter().map(|(k, v)| Line::new(SharedString::from(format!("field-{k}")), k.clone()).fact(v.clone()));
            body.push(Card::new("prompt-fields").items(lines).into_any_element());
        }
    }
    let modes = prompt.agent.as_ref().and_then(|h| c.agents.iter().find(|a| a.harness == *h)).map(|a| a.permission_modes.clone());
    let note_of = |row: &Row| -> String {
        let mode = prompt.options.iter().find(|o| o.id == row.id && o.effect == "mode").and_then(|o| o.mode.clone());
        match mode {
            Some(mode) => {
                modes.as_ref().and_then(|m| m.iter().find(|c| c.value == mode)).and_then(|c| c.description.clone()).unwrap_or_default()
            }
            None => row.description.clone(),
        }
    };
    let this = cx.entity().downgrade();
    if dock.text_only() {
        body.push(field(c, true));
    } else if dock.multi() {
        let ticked = dock.ticked().to_vec();
        let rows = dock.rows().into_iter().enumerate().map(|(at, row)| {
            let on = ticked.contains(&row.id);
            let (this, id) = (this.clone(), row.id.clone());
            let note = note_of(&row);
            let pick = PickRow::new(SharedString::from(format!("prompt-option-{}", row.id)), row.label.clone(), on)
                .title(text::SETTING_TITLE)
                .hover(row_hover(at, true))
                .when(!note.is_empty(), |r| r.note(note))
                .on_click(move |_, window, cx| {
                    _ = this.update(cx, |c, cx| {
                        let was_open = c.dock.as_ref().is_some_and(Dock::field_open);
                        if let Some(dock) = c.dock.as_mut() {
                            dock.tick(&id, !on);
                        }
                        c.focus_opened_field(was_open, window, cx);
                        cx.notify();
                    });
                });
            if row.field.is_some() && on {
                pick.slot(field(c, false))
            } else {
                pick
            }
        });
        body.push(Card::new("prompt-options").items(rows).into_any_element());
    } else {
        let rows = dock.rows().into_iter().enumerate().map(|(at, row)| {
            let (this, id) = (this.clone(), row.id.clone());
            let note = note_of(&row);
            let picked = dock.picked == row.id;
            Choice::new(SharedString::from(format!("prompt-option-{}", row.id)), row.label.clone(), picked)
                .title(text::SETTING_TITLE)
                .hover(row_hover(at, false))
                .when(!note.is_empty(), |r| r.note(note))
                .when(picked && row.field.is_some(), |r| r.slot(field(c, false)))
                .on_click(move |_, window, cx| {
                    _ = this.update(cx, |c, cx| {
                        let was_open = c.dock.as_ref().is_some_and(Dock::field_open);
                        if let Some(dock) = c.dock.as_mut() {
                            dock.picked = id.clone();
                        }
                        c.focus_opened_field(was_open, window, cx);
                        cx.notify();
                    });
                })
        });
        body.push(Card::new("prompt-options").items(rows).into_any_element());
    }
    if let Some(refused) = dock.refused.clone() {
        body.push(crate::screens::thread_view::cards::refusal(refused, &p).into_any_element());
    }

    let held = dock.held();
    let foot = h_flex()
        .items_center()
        .gap_2()
        .px_5()
        .pb_5()
        .child(h_flex().flex_1().min_w_0().min_h(px(20.)).child(Button::text("prompt-write", WRITE_INSTEAD).on_click({
            let this = this.clone();
            move |_, window, cx| _ = this.update(cx, |c, cx| c.fold_prompt(window, cx))
        })))
        .when(dock.step > 0, |el| {
            let this = this.clone();
            el.child(
                Button::outline("prompt-back", "Back").on_click(move |_, window, cx| _ = this.update(cx, |c, cx| c.dock_back(window, cx))),
            )
        })
        .child(Button::primary("prompt-answer", dock.primary_word()).held(held).on_click({
            let this = this.clone();
            move |_, window, cx| _ = this.update(cx, |c, cx| c.dock_confirm(window, cx))
        }));

    let root = v_flex()
        .id("prompt-root")
        .debug_selector(|| "prompt-dock".into())
        .key_context("ComposerDock")
        .track_focus(&c.dock_focus)
        .min_w_0()
        .on_key_down(cx.listener(|c, event: &KeyDownEvent, window, cx| c.dock_key(event, window, cx)))
        .capture_action(cx.listener(Composer::on_dock_escape))
        .capture_action(cx.listener(Composer::on_dock_enter))
        .child(head)
        .when(!body.is_empty(), |el| el.child(v_flex().gap_5().px_5().pt_2().pb_5().children(body)))
        .child(foot);
    div()
        .id("composer")
        .debug_selector(|| "composer".into())
        .w_full()
        .px_4()
        .pb_4()
        .child(div().mx_auto().w_full().max_w(super::COLUMN).child(super::parts::glass_box(cx).child(root)))
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_refused_answer_draws_what_happened_and_its_fix_apart() {
        let failure = wsp_host_client::Failure {
            fix: Some("pick one of its options.".into()),
            ..wsp_host_client::Failure::new("the prompt carries no option by that id. pick one of its options.")
        };
        let refused = refusal_of(&failure);
        assert_eq!(refused.said, "The prompt carries no option by that id.");
        assert_eq!(refused.fix.as_deref(), Some("Pick one of its options."));
        assert_eq!(refusal_of(&wsp_host_client::Failure::new("no such ask")).fix, None);
    }

    fn pick(id: &str, label: &str, effect: &str) -> Pick {
        Pick { id: id.into(), label: label.into(), description: String::new(), effect: effect.into(), mode: None }
    }

    fn consent() -> Prompt {
        Prompt {
            session: "s".into(),
            ask: "k".into(),
            agent: Some("claude".into()),
            title: "Run a command".into(),
            note: None,
            code: Some("rm -rf build".into()),
            runs_in: None,
            fields: Vec::new(),
            file: None,
            cwd: None,
            options: vec![pick("y", "Allow", "allow"), pick("n", "Deny", "deny")],
            questions: None,
            asked: "Run a command: rm -rf build".into(),
        }
    }

    fn form() -> Prompt {
        let q = |index: usize, multiple: bool| Question {
            index,
            header: String::new(),
            question: format!("Question {index}"),
            options: vec![pick(&format!("q{index}:o0"), "Red", "answer"), pick(&format!("q{index}:o1"), "Blue", "answer")],
            multiple,
        };
        Prompt { options: Vec::new(), questions: Some(vec![q(0, false), q(1, true)]), ..consent() }
    }

    #[test]
    fn a_deny_carries_the_words_typed_into_its_field() {
        let mut dock = Dock::new(consent());
        assert_eq!((dock.picked.as_str(), dock.primary_word(), dock.field_open()), ("y", "Allow", false));
        assert!(!dock.digit(2));
        assert_eq!((dock.primary_word(), dock.field_open(), dock.held()), ("Deny", true, false));
        assert!(dock.digit(2), "the picked deny's digit again puts the keys in its field");
        assert_eq!(dock.confirm("  build it instead "), Confirmed::Answer { option: "n".into(), reason: Some("build it instead".into()) });
        dock.arrow(1);
        assert_eq!(dock.confirm("ignored"), Confirmed::Answer { option: "y".into(), reason: None });
    }

    #[test]
    fn a_form_steps_through_its_questions_and_answers_as_one_pick() {
        let mut dock = Dock::new(form());
        assert_eq!((dock.count().as_deref(), dock.primary_word()), (Some("1 of 2"), "Next"));
        dock.arrow(1);
        assert_eq!(dock.confirm(""), Confirmed::Next(String::new()));
        assert_eq!((dock.step, dock.primary_word(), dock.earlier()), (1, "Answer", vec!["Question 0: Blue".to_owned()]));
        assert!(dock.held(), "a tick list with nothing ticked holds the answer");
        dock.digit(1);
        dock.arrow(1);
        dock.arrow(1);
        dock.space();
        assert_eq!(dock.ticked(), ["q1:o0".to_owned(), "other".to_owned()]);
        dock.set_typed("green, a bit");
        assert_eq!(
            dock.confirm("green, a bit"),
            Confirmed::Answer { option: "q0:o1|q1:o0|q1:other:green%2C%20a%20bit".into(), reason: None }
        );
        assert_eq!(dock.back(), Some(String::new()));
        assert_eq!(dock.picked, "q0:o1", "back lands on the pick the step was left with");
    }

    #[test]
    fn the_shared_answer_words_read_from_the_protocol() {
        assert_eq!(answer_words("gone"), Some("the prompt closed before the answer reached it"));
        assert_eq!(answer_words("answered"), None);
        assert_eq!(sentence("the prompt closed"), "The prompt closed.");
    }
}
