// SPDX-License-Identifier: AGPL-3.0-only
//! The newest pages of the warm threads off screen ([`warm`]), read before a switch lands on one. Each is the host's
//! raw events of one page, never a followed thread, whose socket buffers every frame pushed to it, and never a folded
//! transcript. A page goes as soon as its thread leaves the warm set, so the view never holds more than [`warm::HELD`].

use gpui_kit::Task;
use wsp_host_client::protocol::SessionEvent;
use wsp_host_client::Page;

use crate::screens::warm::{self, Key};

/// A thread's newest page as the host answered it: its events, how many the host keeps, and whether its caps dropped
/// older ones.
pub(crate) struct Newest {
    pub events: Vec<SessionEvent>,
    pub total: u64,
    pub trimmed: bool,
}

impl Newest {
    pub fn of(page: Page) -> Self {
        Newest { events: page.events, total: page.total, trimmed: page.trimmed == Some(true) }
    }
}

/// One thread wanted: its page once read, and the read while it runs, which dropping the slot stops.
struct Slot {
    key: Key,
    page: Option<Newest>,
    _read: Option<Task<()>>,
}

#[derive(Default)]
pub(crate) struct Ahead {
    slots: Vec<Slot>,
    /// The thread on screen when the slots were last kept.
    open: Option<Key>,
}

impl Ahead {
    /// `open` shows with `warm` the warm threads off screen: every page of a thread outside them goes, and the warm
    /// threads with no page held or read are answered, for the caller to read.
    pub fn keep(&mut self, open: &Key, warm: &[Key]) -> Vec<Key> {
        if self.open.as_ref() != Some(open) {
            // A read that failed is asked again on the next switch, not on every redraw of the sidebar that calls this.
            self.slots.retain(|s| s.page.is_some() || s._read.is_some());
            self.open = Some(open.clone());
        }
        self.slots.retain(|s| warm.contains(&s.key));
        warm.iter().take(warm::HELD).filter(|k| !self.slots.iter().any(|s| s.key == **k)).cloned().collect()
    }

    /// Holds the read of `key`'s page while it runs.
    pub fn reading(&mut self, key: Key, read: Task<()>) {
        self.slots.push(Slot { key, page: None, _read: Some(read) });
    }

    /// `key`'s page was read, or failed, which holds its slot empty until the next switch asks again.
    pub fn landed(&mut self, key: &Key, page: Option<Newest>) {
        let Some(slot) = self.slots.iter_mut().find(|s| s.key == *key) else { return };
        slot.page = page;
        slot._read = None;
    }

    /// Takes `key`'s page out, once read; a read still running stops.
    pub fn take(&mut self, key: &Key) -> Option<Newest> {
        let at = self.slots.iter().position(|s| s.key == *key)?;
        self.slots.remove(at).page
    }

    /// The threads whose pages are read.
    pub fn held(&self) -> impl Iterator<Item = &Key> {
        self.slots.iter().filter(|s| s.page.is_some()).map(|s| &s.key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::screens::warm::Set;

    fn key(i: usize) -> Key {
        ("w".to_owned(), format!("t{i:02}"))
    }

    fn page() -> Newest {
        Newest { events: Vec::new(), total: 0, trimmed: false }
    }

    /// 60 switches over 30 threads with every read answered at once: the pages held are the warm threads' and no more.
    #[test]
    fn the_pages_held_are_the_warm_threads_over_a_60_switch_walk() {
        let n = 30;
        let (mut set, mut ahead) = (Set::default(), Ahead::default());
        let steps: Vec<usize> = (0..n).chain((10..n).rev()).chain([3, 17, 9, 28, 0, 22, 5, 14, 26, 11]).collect();
        for open in steps {
            let around: Vec<Key> =
                [1, -1, 2, -2].iter().map(|by: &isize| key((open as isize + by).rem_euclid(n as isize) as usize)).collect();
            set.show(key(open));
            set.surround(around);
            let warm = set.held();
            let before: Vec<Key> = ahead.slots.iter().map(|s| s.key.clone()).collect();
            for read in ahead.keep(&key(open), &warm) {
                ahead.reading(read.clone(), Task::ready(()));
                ahead.landed(&read, Some(page()));
            }
            let held: Vec<Key> = ahead.held().cloned().collect();
            assert!(ahead.slots.len() <= warm::HELD, "{} pages held at t{open:02}", ahead.slots.len());
            assert_eq!(held.len(), warm.len(), "at t{open:02}");
            assert!(warm.iter().all(|k| held.contains(k)), "a warm thread's page is not held at t{open:02}: {held:?}");
            let gone: Vec<&Key> = before.iter().filter(|k| !held.contains(k)).collect();
            assert!(gone.iter().all(|k| !warm.contains(k)), "a page let go at t{open:02} is still warm: {gone:?}");
        }
    }

    #[test]
    fn a_switch_takes_its_page_and_a_failed_read_waits_for_the_next_switch() {
        let mut ahead = Ahead::default();
        for read in ahead.keep(&key(0), &[key(10), key(41)]) {
            ahead.reading(read, Task::ready(()));
        }
        ahead.landed(&key(10), Some(page()));
        ahead.landed(&key(41), None);
        assert!(ahead.take(&key(10)).is_some(), "the page read ahead was not handed over");
        assert!(ahead.take(&key(10)).is_none(), "a page taken stays held");
        assert_eq!(ahead.keep(&key(0), &[key(41)]), Vec::<Key>::new(), "a failed read is asked again before the next switch");
        assert!(ahead.held().all(|k| *k != key(41)));
        assert_eq!(ahead.keep(&key(10), &[key(41)]), [key(41)], "the next switch does not ask a failed read again");
    }
}
