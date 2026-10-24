// SPDX-License-Identifier: AGPL-3.0-only
//! Every notice the app says away from where a person is looking (`notices/store.ts`): shown as a toast for a few
//! seconds, kept in the list after it goes.

use std::time::{Duration, SystemTime};

use gpui_kit::SharedString;

use super::Notice;

pub const NOTICE_MS: Duration = Duration::from_millis(8_000);
pub const NOTICE_WITH_ACTION_MS: Duration = Duration::from_millis(12_000);
pub const NOTICE_CAP: usize = 50;
/// As much of a sentence as a notice keeps; a refusal can echo the frame it was sent, as the host's log line cuts it.
pub const NOTICE_CHARS: usize = 400;
/// Toasts on screen at once; a fourth sends the oldest to the list only.
pub const NOTICE_LIMIT: usize = 3;

/// How long a notice's toast stands: none for a keyed one, which stands until its wait ends.
pub fn timeout(notice: &Notice) -> Option<Duration> {
    match (&notice.key, &notice.action) {
        (Some(_), _) => None,
        (None, Some(_)) => Some(NOTICE_WITH_ACTION_MS),
        (None, None) => Some(NOTICE_MS),
    }
}

/// `text` cut to [`NOTICE_CHARS`], with an ellipsis where it was cut.
pub fn cut(text: &SharedString) -> SharedString {
    match text.char_indices().nth(NOTICE_CHARS) {
        Some((at, _)) => format!("{}…", &text[..at]).into(),
        None => text.clone(),
    }
}

/// A notice once added: its id and when.
#[derive(Clone)]
pub struct Stored {
    pub id: u64,
    pub notice: Notice,
    pub at: SystemTime,
}

#[derive(Default)]
pub struct Store {
    /// Newest first.
    notices: Vec<Stored>,
    /// The ids whose toast stands, oldest first.
    toasts: Vec<u64>,
    seq: u64,
}

impl Store {
    pub fn add(&mut self, mut notice: Notice) -> u64 {
        self.seq += 1;
        let id = self.seq;
        notice.text = cut(&notice.text);
        if let Some(key) = notice.key.clone() {
            self.end(&key);
        }
        self.notices.insert(0, Stored { id, notice, at: SystemTime::now() });
        self.toasts.push(id);
        // A keyed toast stands for a wait, so the oldest plain one goes before it.
        while self.toasts.len() > NOTICE_LIMIT {
            let plain = self.toasts.iter().position(|t| !self.keyed(*t)).unwrap_or(0);
            self.toasts.remove(plain);
        }
        // A toast that still stands keeps its row past the fifty, as Sonner keeps the toast it was handed.
        let (toasts, mut kept) = (&self.toasts, 0);
        self.notices.retain(|n| {
            kept += 1;
            kept <= NOTICE_CAP || toasts.contains(&n.id)
        });
        id
    }

    fn keyed(&self, id: u64) -> bool {
        self.notices.iter().any(|n| n.id == id && n.notice.key.is_some())
    }

    /// Takes the toast off the screen; the row stays in the list.
    pub fn dismiss(&mut self, id: u64) -> bool {
        let before = self.toasts.len();
        self.toasts.retain(|t| *t != id);
        self.toasts.len() != before
    }

    /// The wait a keyed notice stood for is over: its toast and its row go.
    pub fn end(&mut self, key: &str) -> bool {
        let gone: Vec<u64> = self.notices.iter().filter(|n| n.notice.key.as_deref() == Some(key)).map(|n| n.id).collect();
        if gone.is_empty() {
            return false;
        }
        self.notices.retain(|n| !gone.contains(&n.id));
        self.toasts.retain(|t| !gone.contains(t));
        true
    }

    /// The ends of every keyed notice whose key starts with `prefix`.
    pub fn end_prefixed(&mut self, prefix: &str) -> bool {
        let keys: Vec<SharedString> = self.notices.iter().filter_map(|n| n.notice.key.clone()).filter(|k| k.starts_with(prefix)).collect();
        let mut ended = false;
        for key in &keys {
            ended |= self.end(key);
        }
        ended
    }

    pub fn toasts(&self) -> &[u64] {
        &self.toasts
    }

    pub fn notices(&self) -> &[Stored] {
        &self.notices
    }

    pub fn get(&self, id: u64) -> Option<&Stored> {
        self.notices.iter().find(|n| n.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::toast::{Action, Kind};
    use std::rc::Rc;

    fn keyed(key: &str) -> Notice {
        Notice { key: Some(key.to_owned().into()), ..Notice::new(Kind::Waiting, key.to_owned()) }
    }

    fn texts(store: &Store) -> Vec<String> {
        store.toasts().iter().map(|id| store.get(*id).unwrap().notice.text.to_string()).collect()
    }

    #[test]
    fn a_fourth_drops_the_oldest_plain_toast_and_keeps_its_row() {
        let mut store = Store::default();
        store.add(keyed("wait"));
        for text in ["one", "two", "three"] {
            store.add(Notice::note(text));
        }
        assert_eq!(texts(&store), ["wait", "two", "three"]);
        assert_eq!(store.notices().len(), 4);
    }

    #[test]
    fn a_key_added_again_replaces_its_toast_and_row() {
        let mut store = Store::default();
        store.add(keyed("wait"));
        store.add(Notice::note("one"));
        let again = store.add(Notice { text: "again".into(), ..keyed("wait") });
        assert_eq!(texts(&store), ["one", "again"]);
        assert_eq!(store.notices().iter().filter(|n| n.notice.key.is_some()).map(|n| n.id).collect::<Vec<_>>(), [again]);
        assert!(store.end("wait"));
        assert_eq!(texts(&store), ["one"]);
        assert_eq!(store.notices().len(), 1);
    }

    #[test]
    fn a_long_sentence_is_cut_at_400_characters() {
        let mut store = Store::default();
        let id = store.add(Notice::error("é".repeat(450)));
        let text = &store.get(id).unwrap().notice.text;
        assert_eq!(text.chars().count(), NOTICE_CHARS + 1);
        assert!(text.ends_with("é…"));
        let short = store.add(Notice::error("x".repeat(NOTICE_CHARS)));
        assert_eq!(store.get(short).unwrap().notice.text.len(), NOTICE_CHARS);
    }

    #[test]
    fn a_toast_stands_8_s_12_s_with_an_action_and_until_ended_with_a_key() {
        let act = Action { word: "Open".into(), run: Rc::new(|_, _| {}) };
        assert_eq!(timeout(&Notice::note("x")), Some(Duration::from_secs(8)));
        assert_eq!(timeout(&Notice { action: Some(act.clone()), ..Notice::note("x") }), Some(Duration::from_secs(12)));
        assert_eq!(timeout(&Notice { action: Some(act), ..keyed("k") }), None);
    }

    #[test]
    fn the_list_keeps_fifty() {
        let mut store = Store::default();
        for n in 0..60 {
            store.add(Notice::note(n.to_string()));
        }
        assert_eq!(store.notices().len(), NOTICE_CAP);
        assert_eq!(store.notices()[0].notice.text.as_ref(), "59");
    }

    #[test]
    fn a_wait_still_standing_keeps_its_toast_past_the_fifty() {
        let mut store = Store::default();
        store.add(keyed("wait"));
        for n in 0..60 {
            store.add(Notice::note(n.to_string()));
        }
        assert_eq!(texts(&store), ["wait", "58", "59"]);
        assert!(store.end("wait"), "and its wait still ends it");
    }
}
