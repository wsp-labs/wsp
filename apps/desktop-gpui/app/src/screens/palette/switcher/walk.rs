// SPDX-License-Identifier: AGPL-3.0-only
//! Which threads a switch walks, and in what order: the sidebar's live list (`shellCommands.ts` sidebarThreads), the
//! threads of the task on screen (`threadWalk`), the open one and the last opened first (`threadHistory.ts`
//! recentThreads), and one step around them (`workspaceSwitcher.ts` stepSwitcherAt).

use std::collections::{HashMap, HashSet, VecDeque};

use serde_json::Value;

use crate::screens::sidebar::model::{self, Prefs, Thread, Tree};
use crate::screens::sidebar::Sidebar;

/// One card's landing: a thread and the workspace it runs in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Target {
    pub workspace_id: String,
    pub thread_id: String,
}

/// `SWITCHER_THREADS`: one row whose titles still read at the narrowest window.
pub const CARDS: usize = 6;
/// `THREAD_HISTORY_CAP`: more than the switcher shows, so a thread that left its row comes back.
pub const VISITS: usize = 20;

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

/// Every thread the sidebar's live list draws that a walk can land on, in its order, children under their opener;
/// the Settled fold is left out. `open` is the thread on screen, which the time fold leaves in the list.
pub fn live(threads: &[Thread], prefs: &Prefs, now_ms: f64, open: Option<&str>) -> Vec<Target> {
    let view = model::sidebar(threads, prefs, now_ms, open);
    // The inbox's copies of threads drawn in their trees are not walked; a root standing in the inbox alone is.
    let roots = view.pinned.iter().chain(view.needs_you.iter().filter(|n| n.inbox.is_none())).chain(&view.threads);
    let mut out = Vec::new();
    for root in roots {
        drawn(root, threads, &mut out);
    }
    out
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

/// The cards out of a walk: the open thread first, then the rest in the order they were last opened, then the ones
/// never opened in the walk's order, cut at [`CARDS`].
pub fn recent(walk: &[Target], visits: &VecDeque<Target>, open: Option<&str>) -> Vec<Target> {
    let walked = |id: &str| walk.iter().any(|t| t.thread_id == id);
    let mut order: Vec<&str> = Vec::new();
    let ids = open.filter(|id| walked(id)).into_iter().chain(visits.iter().map(|v| v.thread_id.as_str()).filter(|id| walked(id)));
    for id in ids.chain(walk.iter().map(|t| t.thread_id.as_str())) {
        if !order.contains(&id) {
            order.push(id);
        }
    }
    order.into_iter().take(CARDS).filter_map(|id| walk.iter().find(|t| t.thread_id == id).cloned()).collect()
}

/// Where the highlight opens, or None where there is no other thread to land on: a step from the open thread, else
/// the first card on a step forward and the last on a step back.
pub fn opening(cards: &[Target], open: Option<&str>, by: isize) -> Option<usize> {
    let from_open = cards.first().is_some_and(|t| Some(t.thread_id.as_str()) == open);
    if cards.len() < if from_open { 2 } else { 1 } {
        return None;
    }
    Some(if from_open {
        stepped(cards.len(), 0, by)
    } else if by > 0 {
        0
    } else {
        cards.len() - 1
    })
}

/// One step of the highlight around the cards, wrapping; it always moves, even with one card.
pub fn stepped(len: usize, at: usize, by: isize) -> usize {
    if len == 0 {
        return 0;
    }
    (at as isize + by).rem_euclid(len as isize) as usize
}

/// Puts `visit` at the front of the threads last opened, kept to [`VISITS`].
pub fn visit(visits: &mut VecDeque<Target>, visit: Target) {
    visits.retain(|v| *v != visit);
    visits.push_front(visit);
    visits.truncate(VISITS);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(w: &str, id: &str) -> Target {
        Target { workspace_id: w.into(), thread_id: id.into() }
    }

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
    fn the_open_thread_leads_then_the_last_opened_then_the_walk() {
        let walk = vec![t("w", "a"), t("w", "b"), t("w", "c"), t("w", "d")];
        let visits = VecDeque::from([t("w", "c"), t("w", "x"), t("w", "a")]);
        assert_eq!(ids(&recent(&walk, &visits, Some("b"))), ["b", "c", "a", "d"]);
        assert_eq!(ids(&recent(&walk, &visits, Some("gone"))), ["c", "a", "b", "d"], "an open thread the walk lacks leads nothing");
        let long: Vec<Target> = (0..9).map(|i| t("w", &i.to_string())).collect();
        assert_eq!(recent(&long, &VecDeque::new(), None).len(), CARDS);
    }

    #[test]
    fn a_step_opens_beside_the_open_thread_and_wraps() {
        let cards = vec![t("w", "a"), t("w", "b"), t("w", "c")];
        assert_eq!(opening(&cards, Some("a"), 1), Some(1), "a tap lands on the thread before this one");
        assert_eq!(opening(&cards, Some("a"), -1), Some(2), "back wraps to the last card");
        assert_eq!(opening(&cards, None, 1), Some(0), "with no thread open the walk starts on the first card");
        assert_eq!(opening(&cards, None, -1), Some(2));
        assert_eq!(stepped(3, 2, 1), 0);
        assert_eq!(stepped(3, 0, -1), 2);
        assert_eq!(stepped(1, 0, 1), 0);
    }

    #[test]
    fn nothing_opens_without_another_thread_to_land_on() {
        assert_eq!(opening(&[t("w", "a")], Some("a"), 1), None);
        assert_eq!(opening(&[], None, 1), None);
        assert_eq!(opening(&[t("w", "a")], None, 1), Some(0), "one thread not open is somewhere to go");
    }

    #[test]
    fn visits_keep_the_newest_first_and_twenty() {
        let mut visits = VecDeque::new();
        for i in 0..25 {
            visit(&mut visits, t("w", &i.to_string()));
        }
        visit(&mut visits, t("w", "20"));
        assert_eq!(visits.len(), VISITS);
        assert_eq!(visits[0].thread_id, "20");
        assert_eq!(visits[1].thread_id, "24");
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

    #[test]
    fn the_live_walk_leaves_settled_out_and_runs_down_each_tree() {
        let now = 1_000_000_000_000.;
        let lead = model::Row { status: model::Status::Running, ended_at: None, ..row("lead", "w", now - 1_000.) };
        let kid = model::Row { parent_thread_id: Some("lead".into()), ..row("kid", "w", now - 2_000.) };
        let threads = model::fold_threads(&[row("old", "w", now - 1e9), lead, kid]);
        assert_eq!(ids(&live(&threads, &Prefs::default(), now, None)), ["lead", "kid"]);
        assert_eq!(ids(&live(&threads, &Prefs::default(), now, Some("old"))), ["lead", "kid", "old"], "the open thread stays live");
    }
}
