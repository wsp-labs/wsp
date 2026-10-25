// SPDX-License-Identifier: AGPL-3.0-only
//! The flat list of rows a virtual list paints from the sections and trees, and the marks a move writes.

use serde_json::{json, Map, Value};
use std::collections::{HashMap, HashSet};

use crate::local_store::LocalStore;

use super::{
    thread_section, Ctx, Lead, Machine, Part, Section, Thread, ThreadSection, Tree, View, ABOVE_TOP_MS, BELOW_LAST_MS, REKEY_GAP_MS,
    SETTLED_PAGE,
};

#[derive(Clone, Debug)]
pub struct Folds {
    /// Sections folded by their heads; `wsp:sidebar-folded`, default Settled alone. Threads has no head.
    pub sections: HashSet<Section>,
    /// How many tiles the open Settled fold draws.
    pub settled_shown: usize,
    /// The leads whose Finished fold is open, by fold key, each with how many rows it draws.
    pub finished_open: HashMap<String, usize>,
    /// Group heads shut by their chevron, by attempt id.
    pub groups_shut: HashSet<String>,
    /// The thread open in the centre: a settled tree holding it stays on the drawn page.
    pub open: Option<String>,
    /// The root carried by the pointer (`data-drag-source`): its tree is not drawn, and every section stands.
    pub dragging: Option<String>,
    /// The workspaces being made and not refused, drawn under Working's head and counted in it (`madeIn`).
    pub making: usize,
}

impl Default for Folds {
    fn default() -> Self {
        Self {
            sections: HashSet::from([Section::Settled]),
            settled_shown: SETTLED_PAGE,
            finished_open: HashMap::new(),
            groups_shut: HashSet::new(),
            open: None,
            dragging: None,
            making: 0,
        }
    }
}

/// Where the app's store keeps the folds the heads make, as the web's localStorage does (`picks.ts`).
pub const FOLDED_KEY: &str = "wsp:sidebar-folded";
const FOLDABLE: [Section; 4] = [Section::Pinned, Section::NeedsYou, Section::Working, Section::Settled];
const FOLDS: [&str; 5] = ["pinned", "needs-you", "working", "settled", "setting-up"];

/// The sections folded as the store kept them, and whether Setting up is.
pub fn stored_folds(store: Option<&LocalStore>) -> (HashSet<Section>, bool) {
    let ids = kept_folds(store);
    (FOLDABLE.into_iter().filter(|s| ids.contains(&s.id())).collect(), ids.contains(&"setting-up"))
}

/// `foldedCodec`: Settled alone where the store keeps nothing, an id no head makes left out.
fn kept_folds(store: Option<&LocalStore>) -> Vec<&'static str> {
    let Some(ids) = store.and_then(|s| s.get_as::<Vec<Value>>(FOLDED_KEY)) else { return vec![Section::Settled.id()] };
    ids.iter().filter_map(|id| FOLDS.into_iter().find(|f| id.as_str() == Some(f))).collect()
}

/// Folds or opens one head in the store and leaves the others as stored, since another window may have written them.
pub fn keep_fold(store: &mut LocalStore, id: &str, folded: bool) -> std::io::Result<()> {
    let mut ids = kept_folds(Some(store));
    ids.retain(|f| *f != id);
    ids.extend(FOLDS.into_iter().filter(|f| folded && *f == id));
    store.set(FOLDED_KEY, json!(ids))
}

/// How a tile is drawn: the two-row tile, or a slim row in a Finished fold or a settled tree.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    Full,
    Finished,
    Settled,
}

/// The tree under a tile: none drawn, drawn open, or folded with how many rows it would draw.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Under {
    None,
    Open,
    Folded(usize),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Tile {
    pub thread: usize,
    pub shape: Shape,
    pub under: Under,
    /// A section's root that a move or a drag takes.
    pub moves: bool,
    /// A tile of the Needs you inbox, and the threads from its tree's top down to the one that started it there.
    pub inbox: bool,
    pub started_by: Vec<usize>,
    pub snoozed_working: Option<usize>,
    /// The moment a resting slot's age counts from.
    pub age_at: Option<f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Item {
    Head {
        section: Section,
        count: usize,
        folded: bool,
    },
    Tile(Tile),
    /// One send to several models: its title is `title_of`'s.
    Group {
        attempt: String,
        title_of: usize,
        copies: usize,
        open: bool,
        moves: bool,
    },
    Subagent {
        lead: usize,
        index: usize,
        part: Part,
    },
    /// A lead's Finished fold.
    Finished {
        lead: usize,
        count: usize,
        open: bool,
    },
    /// "n more", under a lead's open Finished fold, or under the Settled fold where `lead` is None.
    More {
        n: usize,
        lead: Option<usize>,
    },
    /// A machine its lead started that holds no thread yet, by its workspace id.
    Machine {
        workspace: String,
    },
}

/// One drawn row. `depth` is how many child lists in it stands; `last` that it ends its list, so its rail stops at
/// its elbow; bit d of `rails` that the rail of depth d runs past it, an item above it at that depth not being last.
#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    pub depth: u8,
    pub last: bool,
    pub rails: u32,
    pub item: Item,
    /// The row opens a section with a drawn section above it, or is the Settled head: the sections' 12 px gap
    /// stands over it (WorkspaceSidebar.tsx gapsAtRest).
    pub gap: bool,
    /// The section the row stands in.
    pub section: Section,
}

struct Entry {
    item: Item,
    kids: Vec<Entry>,
}

fn leaf(item: Item) -> Entry {
    Entry { item, kids: Vec::new() }
}

struct Draw<'v, 'a> {
    view: &'v View<'a>,
    folds: &'v Folds,
}

impl Draw<'_, '_> {
    /// WorkspaceSidebar.tsx tileItem: a tile and what it draws under it, nested two levels in and then at the
    /// second level's x, as siblings in the list it stands in.
    fn tile(&self, node: &Tree, depth: usize, settled: bool, part: Part, root: bool, out: &mut Vec<Entry>) {
        let cx = &self.view.cx;
        let carried = root && self.folds.dragging.as_deref() == Some(node.key.as_str());
        let Some(i) = node.thread else {
            let attempt = node.key.trim_start_matches("attempt:").to_owned();
            let open = !self.folds.groups_shut.contains(&attempt);
            let mut kids = Vec::new();
            for child in node.children.iter().filter(|_| open && !carried) {
                self.tile(child, depth + 1, settled, Part::Live, false, &mut kids);
            }
            let title_of = node.children.first().and_then(|c| c.thread).unwrap_or(0);
            out.push(Entry { item: Item::Group { attempt, title_of, copies: node.children.len(), open, moves: root && !settled }, kids });
            return;
        };
        let t = &cx.all[i];
        let machines = self.view.machines.iter().filter(|m| m.lead == t.key).count();
        let draws = !settled
            && part == Part::Live
            && node.inbox.is_none()
            && node.snoozed_working.is_none()
            && (cx.draws_under(node) || machines > 0);
        let under = match (draws, t.folded_at.is_some()) {
            (false, _) => Under::None,
            (true, true) => Under::Folded(cx.rows_under(node) + machines),
            (true, false) => Under::Open,
        };
        let tile = Tile {
            thread: i,
            shape: if settled {
                Shape::Settled
            } else if part == Part::Finished {
                Shape::Finished
            } else {
                Shape::Full
            },
            under,
            moves: root && !settled && node.snoozed_working.is_none() && node.inbox.as_ref().is_none_or(Vec::is_empty),
            inbox: node.inbox.is_some(),
            started_by: node.inbox.clone().unwrap_or_default(),
            snoozed_working: node.snoozed_working,
            age_at: if settled && part == Part::Live { Some(cx.tree_settled_ms(node)) } else { t.ended_at.or(t.started_at) },
        };
        let lead = if under == Under::Open && !carried { self.lead_tree(node, depth + 1) } else { Vec::new() };
        let mut kids = Vec::new();
        if settled {
            for child in &node.children {
                self.tile(child, depth + 1, true, Part::Live, false, &mut kids);
            }
        }
        let nests = depth < 2;
        if nests {
            kids.extend(lead);
            out.push(Entry { item: Item::Tile(tile), kids });
        } else {
            out.push(Entry { item: Item::Tile(tile), kids });
            out.extend(lead);
        }
    }

    /// LeadTree.tsx: the live children, then the Finished fold of finished and settled threads, paged.
    fn lead_tree(&self, node: &Tree, depth: usize) -> Vec<Entry> {
        let cx = &self.view.cx;
        let lead = node.thread.unwrap_or(0);
        let [live, finished, settled] = cx.child_parts(cx.lead_nodes(node));
        let mut out = Vec::new();
        // leadTree.ts ranks a machine with no thread as a quiet live child (2), newest first within its rank.
        let mut machines = self.view.machines.iter().filter(|m| m.lead == cx.all[lead].key).peekable();
        let machine = |m: &Machine| leaf(Item::Machine { workspace: m.workspace.clone() });
        for l in live {
            let (rank, start) = (cx.tree_rank(l), cx.start_of(l));
            while let Some(m) = machines.next_if(|m| rank > 2 || (rank == 2 && start < m.started_at)) {
                out.push(machine(m));
            }
            match l {
                Lead::Thread(child) => self.tile(child, depth, false, Part::Live, false, &mut out),
                Lead::Sub(of, index) => out.push(leaf(Item::Subagent { lead: of, index, part: Part::Live })),
            }
        }
        out.extend(machines.map(machine));
        let folded: Vec<(Lead, Part)> = finished
            .into_iter()
            .map(|l| (l, Part::Finished))
            .chain(settled.into_iter().filter(|l| matches!(l, Lead::Thread(_))).map(|l| (l, Part::Settled)))
            .collect();
        if folded.is_empty() {
            return out;
        }
        let shown = self.folds.finished_open.get(&cx.all[lead].key).copied();
        out.push(leaf(Item::Finished { lead, count: folded.len(), open: shown.is_some() }));
        if let Some(shown) = shown {
            for &(l, part) in folded.iter().take(shown) {
                match l {
                    Lead::Thread(child) => self.tile(child, depth, part == Part::Settled, part, false, &mut out),
                    Lead::Sub(of, index) => out.push(leaf(Item::Subagent { lead: of, index, part })),
                }
            }
            if folded.len() > shown {
                out.push(leaf(Item::More { n: folded.len() - shown, lead: Some(lead) }));
            }
        }
        out
    }
}

fn tile_count(node: &Tree) -> usize {
    usize::from(node.thread.is_some()) + node.children.iter().map(tile_count).sum::<usize>()
}

fn flatten(list: Vec<Entry>, depth: usize, rails: u32, out: &mut Vec<Line>) {
    let n = list.len();
    for (k, entry) in list.into_iter().enumerate() {
        let last = k + 1 == n;
        out.push(Line { depth: depth.min(u8::MAX as usize) as u8, last, rails, item: entry.item, gap: false, section: Section::Threads });
        let below = if (1..32).contains(&depth) && !last { rails | 1 << depth } else { rails };
        flatten(entry.kids, depth + 1, below, out);
    }
}

/// Where a root stands among the trees of its section that move.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RootPlace {
    pub section: Section,
    pub index: usize,
    pub count: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Move {
    Up,
    Down,
    Top,
    /// A drop at a place in Pinned or the list, `at` counted among the section's trees with the moving one left out.
    To {
        section: Section,
        at: usize,
    },
}

/// One `sessions.mark` a move sends: the threads it names and the marks it writes on each.
#[derive(Clone, Debug, PartialEq)]
pub struct MoveMark {
    pub thread_ids: Vec<String>,
    pub marks: Value,
}

impl<'a> View<'a> {
    pub fn all(&self) -> &'a [Thread] {
        self.cx.all
    }

    pub fn section(&self, section: Section) -> &[Tree] {
        match section {
            Section::Pinned => &self.pinned,
            Section::NeedsYou => &self.needs_you,
            Section::Working => &self.working,
            Section::Threads => &self.threads,
            Section::Settled => &self.settled,
        }
    }

    /// The rows the sidebar draws top to bottom: Pinned, Needs you and Working under their heads, the list with none,
    /// then the Settled head and its page of trees.
    pub fn lines(&self, folds: &Folds) -> Vec<Line> {
        let draw = Draw { view: self, folds };
        let mut top = Vec::new();
        let mut gaps = Vec::new();
        let mut sections: Vec<(usize, Section)> = Vec::new();
        // While a tree is dragged every section that takes a drop stands to take it (`data-drop-only`), Working only while it holds one.
        let dragging = folds.dragging.is_some();
        for section in [Section::Pinned, Section::NeedsYou, Section::Working, Section::Threads] {
            let roots = self.section(section);
            let making = if section == Section::Working { folds.making } else { 0 };
            if roots.is_empty() && making == 0 && !(dragging && matches!(section, Section::Pinned | Section::NeedsYou)) {
                continue;
            }
            if !top.is_empty() {
                gaps.push(top.len());
            }
            sections.push((top.len(), section));
            let folded = section != Section::Threads && folds.sections.contains(&section);
            if section != Section::Threads {
                let count = roots.iter().map(|n| self.cx.drawn_count(n)).sum::<usize>() + making;
                top.push(leaf(Item::Head { section, count, folded }));
            }
            for node in roots.iter().filter(|_| !folded) {
                draw.tile(node, 0, false, Part::Live, true, &mut top);
            }
        }
        if !self.settled.is_empty() || dragging {
            gaps.push(top.len());
            sections.push((top.len(), Section::Settled));
            let count: usize = self.settled.iter().map(tile_count).sum();
            let folded = folds.sections.contains(&Section::Settled);
            top.push(leaf(Item::Head { section: Section::Settled, count, folded }));
            if !folded {
                let holding = |n: &Tree| folds.open.as_deref().is_some_and(|open| holds(n, self.cx.all, open));
                let through_open = self.settled.iter().position(holding).map_or(0, |at| self.settled[..=at].iter().map(tile_count).sum());
                let shown = folds.settled_shown.max(through_open);
                let mut tiles = 0;
                let mut drawn = 0;
                for (at, node) in self.settled.iter().enumerate() {
                    tiles += tile_count(node);
                    if tiles > shown && at > 0 {
                        break;
                    }
                    drawn = tiles;
                    draw.tile(node, 0, true, Part::Live, false, &mut top);
                }
                if count > drawn {
                    top.push(leaf(Item::More { n: count - drawn, lead: None }));
                }
            }
        }
        fn size(e: &Entry) -> usize {
            1 + e.kids.iter().map(size).sum::<usize>()
        }
        let mut starts = Vec::with_capacity(top.len());
        let mut at = 0;
        for e in &top {
            starts.push(at);
            at += size(e);
        }
        let mut out = Vec::with_capacity(self.cx.all.len() + 8);
        flatten(top, 0, 0, &mut out);
        for g in gaps {
            if let Some(&line) = starts.get(g) {
                out[line].gap = true;
            }
        }
        for (k, &(entry, section)) in sections.iter().enumerate() {
            let from = starts.get(entry).copied().unwrap_or(out.len());
            let to = sections.get(k + 1).and_then(|&(e, _)| starts.get(e).copied()).unwrap_or(out.len());
            out[from..to].iter_mut().for_each(|l| l.section = section);
        }
        out
    }

    fn root(&self, key: &str) -> Option<&Tree> {
        let top = |n: &&Tree| n.key == key && n.inbox.as_ref().is_none_or(Vec::is_empty);
        self.pinned.iter().chain(&self.working).chain(&self.threads).chain(&self.needs_you).find(top)
    }

    /// Every finished thread under a tree's tiles and every thread under those, which "Settle n finished" takes
    /// (leadTree.ts finishedTake); none where the key stands in no tree drawn.
    pub fn finished_under(&self, key: &str) -> Vec<String> {
        fn find<'t>(nodes: &'t [Tree], key: &str) -> Option<&'t Tree> {
            nodes.iter().find_map(|n| if n.key == key { Some(n) } else { find(&n.children, key) })
        }
        fn all_under(node: &Tree, all: &[Thread], out: &mut Vec<String>) {
            if let Some(i) = node.thread {
                out.push(all[i].key.clone());
            }
            node.children.iter().for_each(|c| all_under(c, all, out));
        }
        fn walk(cx: &Ctx, node: &Tree, out: &mut Vec<String>) {
            let [live, finished, settled] = cx.child_parts(cx.lead_nodes(node));
            for l in finished.iter().chain(&settled) {
                if let Lead::Thread(n) = *l {
                    if n.thread.is_some_and(|i| cx.all[i].settled_at.is_none()) {
                        all_under(n, cx.all, out);
                    }
                }
            }
            for l in live {
                if let Lead::Thread(n) = l {
                    walk(cx, n, out);
                }
            }
        }
        let tops = [&self.pinned, &self.working, &self.threads, &self.needs_you];
        let Some(node) = tops.into_iter().find_map(|t| find(t, key)) else { return Vec::new() };
        let mut out = Vec::new();
        walk(&self.cx, node, &mut out);
        out
    }

    pub fn root_place(&self, key: &str) -> Option<RootPlace> {
        [Section::Pinned, Section::Threads].into_iter().find_map(|section| {
            let roots: Vec<&Tree> = self.section(section).iter().filter(|r| r.snoozed_working.is_none()).collect();
            roots.iter().position(|r| r.key == key).map(|index| RootPlace { section, index, count: roots.len() })
        })
    }

    /// What dropping a root on a section's head writes, on every thread it stands for; None for a drop that changes
    /// nothing. A group goes as one: pinned, or unpinned where a member is (WorkspaceSidebar.tsx dropOnHead). A
    /// drop on Settled is a settle, not a mark.
    pub fn drop_marks(&self, key: &str, section: Section) -> Option<MoveMark> {
        let node = self.root(key)?;
        let thread_ids: Vec<String> = self.cx.root_threads(node).map(|i| self.cx.all[i].key.clone()).collect();
        let marks = if node.thread.is_some() {
            Value::Object(self.drop_map(node, section)?)
        } else if section == Section::Pinned {
            json!({ "pinned": true })
        } else if section != Section::Settled && self.cx.pin_key(node).is_some() {
            json!({ "pinned": false })
        } else {
            return None;
        };
        Some(MoveMark { thread_ids, marks })
    }

    fn drop_map(&self, node: &Tree, section: Section) -> Option<Map<String, Value>> {
        let t = &self.cx.all[node.thread?];
        let pinned = t.pinned_at.is_some();
        let mut marks = Map::new();
        match section {
            Section::Settled => return None,
            // Working holds a tree by its state alone, so a drop there only takes a working tree out of Pinned.
            Section::Working => {
                return (pinned && self.cx.working_in(node) > 0).then(|| Map::from_iter([("pinned".into(), Value::Bool(false))]))
            }
            Section::Pinned if pinned => return None,
            Section::Pinned => {
                marks.insert("pinned".into(), Value::Bool(true));
                return Some(marks);
            }
            _ => {}
        }
        let own_list = if thread_section(t) == ThreadSection::NeedsYou { Section::NeedsYou } else { Section::Threads };
        let own = own_list == section;
        if own && !pinned && t.section.is_none() {
            return None;
        }
        if pinned {
            marks.insert("pinned".into(), Value::Bool(false));
        }
        let placed = if section == Section::NeedsYou { ThreadSection::NeedsYou } else { ThreadSection::Idle };
        let place = if own { Value::Null } else { json!({ "name": placed.id(), "whileState": self.cx.placement_key(node) }) };
        marks.insert("section".into(), place);
        Some(marks)
    }

    /// Move up, Move down, Move to top, or a drop at a place: the marks to send, None where nothing moves
    /// (treeMoves.ts moveRoot). A section with no key to land beside keys off the view's clock sample.
    pub fn move_root(&self, key: &str, to: Move) -> Option<Vec<MoveMark>> {
        let node = self.root(key)?;
        let from = self.root_place(key);
        let section = match to {
            Move::To { section, .. } => section,
            _ => from?.section,
        };
        if !matches!(section, Section::Pinned | Section::Threads) {
            return None;
        }
        let at = match (to, from) {
            (Move::To { at, .. }, _) => at as isize,
            (Move::Top, _) => 0,
            (Move::Up, Some(f)) => f.index as isize - 1,
            (Move::Down, Some(f)) => f.index as isize + 1,
            _ => return None,
        };
        let others: Vec<&Tree> = self.section(section).iter().filter(|r| r.snoozed_working.is_none() && r.key != key).collect();
        if at < 0 || at as usize > others.len() || from.is_some_and(|f| f.section == section && f.index as isize == at) {
            return None;
        }
        let at = at as usize;
        let keys: Vec<f64> = others.iter().map(|r| self.cx.key_in(section, r)).collect();
        let (new_key, rekeyed) = key_at(&keys, at, self.cx.now);
        let ids = |n: &Tree| self.cx.root_threads(n).map(|i| self.cx.all[i].key.clone()).collect::<Vec<_>>();
        let field = if section == Section::Pinned { "pinned" } else { "order" };
        let own = if section == Section::Pinned {
            json!({ "pinned": new_key })
        } else {
            let mut marks = self.leaving(node).unwrap_or_default();
            if self.cx.pin_key(node).is_some() {
                marks.insert("pinned".into(), Value::Bool(false));
            }
            marks.insert("order".into(), json!(new_key));
            Value::Object(marks)
        };
        let mut out = vec![MoveMark { thread_ids: ids(node), marks: own }];
        out.extend(rekeyed.into_iter().map(|(index, k)| MoveMark { thread_ids: ids(others[index]), marks: json!({ field: k }) }));
        Some(out)
    }

    /// What a tree carries into the list beside its key: a placement where its state files it in Needs you, and a
    /// placement taken off where it has one.
    fn leaving(&self, node: &Tree) -> Option<Map<String, Value>> {
        let mut marks = self.drop_map(node, Section::Threads)?;
        let has_placement = node.thread.is_some_and(|i| self.cx.all[i].section.is_some());
        if marks.get("section").is_some_and(Value::is_null) && !has_placement {
            marks.remove("section");
        }
        Some(marks)
    }
}

fn holds(node: &Tree, all: &[Thread], key: &str) -> bool {
    node.thread.is_some_and(|i| all[i].key == key) || node.children.iter().any(|c| holds(c, all, key))
}

/// treeOrder.ts keyAt: a tree's key where it lands among a section's keys in drawn order, and the trees a gap that
/// ran out re-keys, by index. Between two trees it is their midpoint.
pub fn key_at(keys: &[f64], at: usize, now_ms: f64) -> (f64, Vec<(usize, f64)>) {
    let (Some(&first), Some(&last)) = (keys.first(), keys.last()) else { return (now_ms, Vec::new()) };
    if at == 0 {
        return (first + ABOVE_TOP_MS, Vec::new());
    }
    if at >= keys.len() {
        return (last - BELOW_LAST_MS, Vec::new());
    }
    let (above, below) = (keys[at - 1], keys[at]);
    let mid = above / 2.0 + below / 2.0;
    if mid < above && mid > below {
        return (mid, Vec::new());
    }
    let mut end = at;
    while end + 1 < keys.len() && keys[end] - keys[end + 1] < REKEY_GAP_MS {
        end += 1;
    }
    let floor = if end + 1 < keys.len() { keys[end + 1] } else { keys[end] - BELOW_LAST_MS };
    let step = (above - floor) / (end - at + 3) as f64;
    (above - step, (0..=end - at).map(|i| (at + i, above - step * (i + 2) as f64)).collect())
}
