// SPDX-License-Identifier: AGPL-3.0-only
//! Which threads a step walks, and in what order: the threads of the task on screen (`threadWalk`), and the open
//! thread's own level as the list draws it, which Ctrl+Tab steps through.

use std::collections::{HashMap, HashSet};

use serde_json::Value;

use gpui_kit::Context;

use super::model::{self, Item, Section, Shape, Thread, Tree};
use super::rows::Drawn;
use super::Sidebar;

/// A step's landing: a thread and the workspace it runs in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Target {
    pub workspace_id: String,
    pub thread_id: String,
}

fn target(t: &Thread) -> Option<Target> {
    t.thread_id.as_ref()?;
    Some(Target { workspace_id: t.workspace_id.clone(), thread_id: t.key.clone() })
}

/// A tree's threads a walk can land on, each before the ones it opened.
fn drawn(node: &Tree, threads: &[Thread], out: &mut Vec<Target>) {
    out.extend(node.thread.and_then(|i| target(&threads[i])));
    for child in &node.children {
        drawn(child, threads, out);
    }
}

/// The threads of one task as its rows order them (`sidebarThreadOrder`): the working ones newest first, then the
/// rest by when they ended, each under its opener. A thread joins the task its opener's chain started in, unless its
/// own workspace is a fork with a row of its own (`threadTree.ts` drawnUnder).
pub fn in_workspace(threads: &[Thread], workspaces: &HashMap<String, Value>, workspace_id: &str) -> Vec<Target> {
    let by_key: HashMap<&str, &Thread> = threads.iter().map(|t| (t.key.as_str(), t)).collect();
    let forked = |w: &str| workspaces.get(w).is_some_and(|w| w.get("parentThreadId").is_some_and(|p| !p.is_null()));
    let under = |t: &Thread| -> String {
        if forked(&t.workspace_id) {
            return t.workspace_id.clone();
        }
        let mut seen = HashSet::from([t.key.as_str()]);
        let mut at = t;
        while let Some(opener) = at.parent.as_deref().and_then(|p| by_key.get(p)) {
            if !seen.insert(opener.key.as_str()) {
                return t.workspace_id.clone();
            }
            at = opener;
        }
        at.workspace_id.clone()
    };
    let mine: Vec<&Thread> = threads.iter().filter(|t| under(t) == workspace_id).collect();
    let working = |t: &&Thread| t.status == model::Status::Running && !t.asks;
    let mut active: Vec<&Thread> = mine.iter().copied().filter(working).collect();
    let mut rest: Vec<&Thread> = mine.iter().copied().filter(|t| !working(t)).collect();
    let newest = |at: fn(&Thread) -> f64| move |a: &&Thread, b: &&Thread| at(b).total_cmp(&at(a)).then_with(|| a.key.cmp(&b.key));
    active.sort_by(newest(|t| t.started_at.unwrap_or(0.)));
    rest.sort_by(newest(|t| t.ended_at.or(t.started_at).unwrap_or(0.)));
    nested(&active).into_iter().chain(nested(&rest)).collect()
}

/// `threadWalk`: what every thread step walks, the chords', the palette's Next and Previous thread and its held rows.
pub(crate) fn thread_walk(sidebar: &Sidebar, workspace_id: &str) -> Vec<Target> {
    in_workspace(sidebar.threads(), &sidebar.around().workspaces, workspace_id)
}

/// `nestSpawnedThreads` through the sidebar's own forest, over the part alone, so a thread whose opener stands in
/// the other part keeps the place it sorted to.
fn nested(part: &[&Thread]) -> Vec<Target> {
    let part: Vec<Thread> = part.iter().map(|&t| t.clone()).collect();
    let order: Vec<usize> = (0..part.len()).collect();
    let mut out = Vec::with_capacity(part.len());
    for tree in model::forest(&part, &order) {
        drawn(&tree, &part, &mut out);
    }
    out
}

/// The threads a level step walks among: the live list's top-level threads, Settled's, or those one lead started, its
/// live ones apart from those in its Finished fold.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Level {
    Top,
    Settled,
    Under { lead: String, folded: bool },
}

impl Sidebar {
    /// The lead of a thread, where it stands in the list; a thread whose opener the host no longer lists is a root.
    fn lead_of(&self, thread: &Thread) -> Option<String> {
        thread.parent.clone().filter(|p| *p != thread.key && self.threads.iter().any(|t| t.key == *p))
    }

    /// Each thread's tile in its tree as drawn, top to bottom, with the level it stands at: its drawn row and its
    /// thread. A Needs you copy of a thread drawn in its tree is not its place.
    fn placed(&self) -> Vec<(usize, usize, Level)> {
        let rows = self.drawn_rows.iter().enumerate();
        rows.filter_map(|(row, d)| {
            let Drawn::Line(ix) = d else { return None };
            let line = self.lines.get(*ix)?;
            let Item::Tile(t) = &line.item else { return None };
            let level = match self.lead_of(&self.threads[t.thread]) {
                Some(lead) => Level::Under { lead, folded: t.shape != Shape::Full },
                None if line.section == Section::Settled => Level::Settled,
                None => Level::Top,
            };
            (!t.inbox).then_some((row, t.thread, level))
        })
        .collect()
    }

    /// The drawn threads at the open thread's level, as indices into `placed`, and where the open one stands among
    /// them. With none open, the live list's top-level threads; an open thread whose row is not drawn reads its drawn
    /// siblings, and a root not drawn reads none.
    fn level_of(&self, placed: &[(usize, usize, Level)]) -> (Vec<usize>, Option<usize>) {
        let open = self.selected.as_deref();
        let at = open.and_then(|open| placed.iter().position(|(_, i, _)| self.threads[*i].key == open));
        let level = match (open, at) {
            (None, _) => Some(Level::Top),
            (_, Some(at)) => Some(placed[at].2.clone()),
            (Some(open), None) => {
                let lead = self.threads.iter().find(|t| t.key == open).and_then(|t| self.lead_of(t));
                lead.map(|lead| Level::Under { lead, folded: true })
            }
        };
        let walk = (0..placed.len()).filter(|&k| level.as_ref() == Some(&placed[k].2)).collect();
        (walk, at)
    }

    /// How many threads the step walks among, the open one with them, for the palette's held row.
    pub fn level_len(&self) -> usize {
        self.level_of(&self.placed()).0.len()
    }

    /// The thread a step of `by` from the open one lands on among the threads drawn at its level, wrapping; none where
    /// it lands back on the open one. With none open, or the open one not among them, the first going down and the
    /// last going up.
    pub(crate) fn level_step(&self, by: isize) -> Option<String> {
        let placed = self.placed();
        let (walk, at) = self.level_of(&placed);
        let n = walk.len() as isize;
        if n == 0 {
            return None;
        }
        let next = match at.and_then(|at| walk.iter().position(|&k| k == at)) {
            Some(from) => (from as isize + by).rem_euclid(n),
            None if by > 0 => 0,
            None => n - 1,
        };
        let landed = walk[next as usize];
        (Some(landed) != at).then(|| self.threads[placed[landed].1].key.clone())
    }

    /// Ctrl+Tab and Ctrl+Shift+Tab: opens the thread [`Sidebar::level_step`] lands on as a press on its row does, and
    /// scrolls its row into view; the list's own keys walk on from that row. Whether a thread opened.
    pub fn step_level(&mut self, by: isize, cx: &mut Context<Self>) -> bool {
        let Some(key) = self.level_step(by) else { return false };
        self.select(&key, cx);
        if let Some(row) = self.tree_row(&key) {
            self.list.scroll_to_reveal_item(row);
            self.key_row = Some(row);
        }
        true
    }

    /// The drawn row of a thread's tile in its tree, not its Needs you copy.
    pub(crate) fn tree_row(&self, key: &str) -> Option<usize> {
        self.drawn_rows.iter().position(|d| match d {
            Drawn::Line(ix) => matches!(&self.lines[*ix].item, Item::Tile(t) if self.threads[t.thread].key == key && !t.inbox),
            _ => false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(key: &str, workspace: &str, started: f64) -> model::Row {
        model::Row {
            id: format!("r-{key}"),
            thread_id: Some(key.into()),
            workspace_id: workspace.into(),
            started_at: Some(started),
            ended_at: Some(started + 1.),
            read_at: Some(started + 2.),
            status: model::Status::Completed,
            ..model::Row::default()
        }
    }

    fn ids(targets: &[Target]) -> Vec<&str> {
        targets.iter().map(|t| t.thread_id.as_str()).collect()
    }

    #[test]
    fn a_tasks_walk_puts_working_threads_first_and_children_under_their_opener() {
        let working = model::Row { status: model::Status::Running, ended_at: None, ..row("run", "w", 5.) };
        let child = model::Row { parent_thread_id: Some("old".into()), ..row("kid", "elsewhere", 9.) };
        let threads = model::fold_threads(&[row("old", "w", 1.), row("new", "w", 3.), working, child, row("other", "w2", 4.)]);
        let walk = in_workspace(&threads, &HashMap::new(), "w");
        assert_eq!(ids(&walk), ["run", "new", "old", "kid"], "the child joins its opener's task, not its own workspace");
        let forks = HashMap::from([("elsewhere".to_owned(), serde_json::json!({ "parentThreadId": "old" }))]);
        assert_eq!(ids(&in_workspace(&threads, &forks, "w")), ["run", "new", "old"], "a fork's thread stays on its row");
    }
}
