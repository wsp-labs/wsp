// SPDX-License-Identifier: AGPL-3.0-only
//! The palette's rows and how a query narrows them, after `components/palette/paletteItems.tsx` and
//! `CommandPalette.logic.ts`: the commands of `keys::COMMANDS` held by what the window holds, the tasks, the recent
//! threads at rest, the threads whose title holds the query and those whose messages the host found it in, and the
//! page of projects New thread picks from. Pure, over [`Seen`], so the lists are testable without a window.

use gpui_kit::SharedString;

use crate::screens::app_shell::keys::{self, Command, Group as Home, Held, Need, Run, Says};
use crate::screens::sidebar::model::Kind;
use crate::ui::Glyph;

/// `RECENT_THREAD_LIMIT`.
pub const RECENT: usize = 12;
/// `WORKSPACE_SELECT_SLOTS`: the rows a mod digit reaches.
pub const SLOTS: usize = 9;

/// A project as the page of projects lists it.
#[derive(Clone, Debug, PartialEq)]
pub struct Project {
    pub id: String,
    pub name: String,
    /// The computer it is on by name.
    pub computer: Option<String>,
    /// Whether that is the computer the app runs on, which its row names without a glyph.
    pub here: bool,
    pub path: String,
    /// The task its New thread page opens on: the project's own folder.
    pub workspace: Option<String>,
}

/// A task as the Tasks list reads it, in the sidebar's order.
#[derive(Clone, Debug, PartialEq)]
pub struct Task {
    pub id: String,
    /// The thread the row opens: the task's first in the sidebar.
    pub thread: String,
    pub name: String,
    /// Its one state word (`workspaceWord`): Running, Stopped, Waking.
    pub state: String,
    pub computer: Option<String>,
    pub project: Option<String>,
}

/// A thread as its row reads it.
#[derive(Clone, Debug, PartialEq)]
pub struct Thread {
    pub id: String,
    pub workspace: String,
    pub title: String,
    pub harness: String,
    /// The task it runs on, by name, and that task's computer.
    pub task: String,
    pub computer: Option<String>,
    pub started_at: Option<f64>,
    /// Its state, and the time or age its slot shows.
    pub kind: Kind,
    pub slot: Option<String>,
}

/// One host message hit (`SessionSearchHit`).
#[derive(Clone, Debug, PartialEq)]
pub struct Hit {
    pub workspace: String,
    pub thread: String,
    pub snippet: String,
}

/// What the window shows: the task on screen and its open thread.
#[derive(Clone, Debug, PartialEq)]
pub struct Open {
    pub workspace: String,
    pub thread: Option<String>,
    pub task: String,
    pub project: Option<String>,
}

/// One of the sidebar's acts for the open thread or its task (`actionItem`): a move of the thread's tree or the
/// task's own, refused with its reason.
#[derive(Clone, Debug, PartialEq)]
pub struct Act {
    pub id: String,
    pub title: String,
    pub glyph: Glyph,
    pub terms: Vec<String>,
    pub refusal: Option<String>,
    /// What the row says while it is not refused.
    pub says: String,
    pub chord: Option<&'static str>,
    /// Its place in the list the sidebar handed, which the run gives back.
    pub index: usize,
}

/// Everything the lists read, taken off the sidebar and the shell as the palette opens and as they change.
#[derive(Clone, Debug, Default)]
pub struct Seen {
    pub connected: bool,
    /// New thread asks for its project every time and there is more than one.
    pub asks: bool,
    /// Every project, in the order New thread offers them.
    pub projects: Vec<Project>,
    pub tasks: Vec<Task>,
    pub threads: Vec<Thread>,
    pub open: Option<Open>,
    /// The threads a thread step walks: those of the task on screen.
    pub walk: usize,
    pub acts: Vec<Act>,
    /// Each chord command's label as bound now.
    pub labels: Vec<(String, String)>,
}

impl Seen {
    fn label(&self, command: &str) -> Option<SharedString> {
        self.labels.iter().find(|(c, _)| c == command).map(|(_, l)| SharedString::from(l.clone()))
    }
}

/// What a row draws at its left.
#[derive(Clone, Debug, PartialEq)]
pub enum Lead {
    Glyph(Glyph),
    /// The agent's mark.
    Harness(String),
    /// The project's glyph.
    Project(Option<String>),
    None,
}

/// What a row says under its title.
#[derive(Clone, Debug, PartialEq)]
pub enum Description {
    Words(SharedString),
    /// Facts side by side (`Facts`), the gap the only separator.
    Facts(Vec<SharedString>),
    /// A project's computer, with its glyph, and its folder.
    Where {
        computer: Option<SharedString>,
        here: bool,
        path: SharedString,
    },
}

/// What stands at a row's right end before its chord.
#[derive(Clone, Debug, PartialEq)]
pub enum Trailing {
    /// "Current task".
    Current,
    Status(Kind, Option<String>),
    /// A page row's pick chord, `⌘1`.
    Pick(SharedString),
}

/// What a row runs.
#[derive(Clone, Debug, PartialEq)]
pub enum Go {
    Command(Run),
    /// Opens a task on its first thread.
    Task {
        workspace: String,
        thread: String,
    },
    Thread {
        workspace: String,
        thread: String,
    },
    /// New thread on one project.
    Project(String),
    /// The sidebar's act at this place in its list.
    Act(usize),
    Nothing,
}

/// One row, `CommandPaletteActionItem` and `CommandPaletteSubmenuItem`.
#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub value: SharedString,
    /// Opens a page rather than running.
    pub page: bool,
    pub terms: Vec<String>,
    pub title: SharedString,
    pub says: Option<Description>,
    pub lead: Lead,
    pub held: bool,
    pub trailing: Option<Trailing>,
    /// The chord's label, drawn only on a row that is not held.
    pub shortcut: Option<SharedString>,
    pub keep_open: bool,
    pub go: Go,
}

impl Item {
    fn new(value: impl Into<SharedString>, title: impl Into<SharedString>, go: Go) -> Item {
        Item {
            value: value.into(),
            page: false,
            terms: Vec::new(),
            title: title.into(),
            says: None,
            lead: Lead::None,
            held: false,
            trailing: None,
            shortcut: None,
            keep_open: false,
            go,
        }
    }
}

/// One list under its label.
#[derive(Clone, Debug, PartialEq)]
pub struct Group {
    pub value: &'static str,
    pub label: &'static str,
    pub items: Vec<Item>,
}

/// Whether `command` has a row here at all: Copy as Markdown has none while no thread is on screen.
pub fn listed(command: &Command, seen: &Seen) -> bool {
    command.need != Need::Thread || seen.open.as_ref().and_then(|o| o.thread.as_ref()).is_some()
}

/// Why `command` is held here, or None where it runs.
pub fn held(command: &Command, seen: &Seen) -> Option<Held> {
    match command.need {
        Need::Nothing => None,
        Need::Create if !seen.connected => Some(Held::NotConnected),
        Need::Create if seen.projects.is_empty() => Some(Held::NoProject),
        Need::Create => None,
        Need::Pick => (!seen.connected || seen.projects.is_empty()).then_some(Held::NoPick),
        Need::Tasks => (seen.tasks.len() < 2).then_some(Held::OneTask),
        Need::Walk if seen.walk >= 2 => None,
        Need::Walk if seen.walk > 0 => Some(Held::OneThread),
        Need::Walk => Some(seen.open.as_ref().map_or(Held::NoThreads, |open| Held::NothingToStep(open.task.clone()))),
        Need::Task => seen.open.is_none().then_some(Held::NoTask),
        Need::Project => seen.open.as_ref().and_then(|o| o.project.as_ref()).is_none().then_some(Held::NoProjectOpen),
        Need::Thread => None,
    }
}

fn command_item(command: &Command, seen: &Seen) -> Option<Item> {
    if !listed(command, seen) {
        return None;
    }
    let held = held(command, seen);
    let says = match (&held, command.says) {
        // The page's own line names what it waits on whether or not it is held.
        (Some(reason), _) => Some(reason.words().into()),
        (None, Says::Nothing) => None,
        (None, Says::Words(words)) => Some(words.into()),
        (None, Says::Task) => seen.open.as_ref().map(|o| o.task.clone().into()),
        (None, Says::Project) => seen
            .open
            .as_ref()
            .and_then(|o| o.project.as_ref())
            .and_then(|id| seen.projects.iter().find(|p| p.id == *id))
            .map(|p| p.name.clone().into()),
    };
    let mut item = Item::new(command.id, command.title, Go::Command(command.run));
    item.terms = command.terms.iter().map(|t| (*t).to_owned()).collect();
    item.page = command.run == Run::PickProject;
    item.says = says.map(Description::Words);
    item.lead = Lead::Glyph(command.glyph);
    item.held = held.is_some();
    item.shortcut = command.chord.and_then(|c| seen.label(c));
    item.keep_open = command.keep_open || command.run == Run::NewThread && seen.asks;
    Some(item)
}

/// The Actions list: the commands in their table's order, the open thread's moves and task's acts before Next task.
pub fn actions(seen: &Seen) -> Vec<Item> {
    let mut out = Vec::new();
    for command in keys::COMMANDS.iter().filter(|c| c.group == Home::Actions) {
        if command.id == "action:next-workspace" {
            out.extend(seen.acts.iter().map(|act| act_item(act, seen)));
        }
        out.extend(command_item(command, seen));
    }
    out
}

fn act_item(act: &Act, seen: &Seen) -> Item {
    let mut item = Item::new(format!("action:{}", act.id), act.title.clone(), Go::Act(act.index));
    item.terms = std::iter::once(act.title.clone()).chain(act.terms.iter().cloned()).collect();
    item.lead = Lead::Glyph(act.glyph);
    item.says = Some(Description::Words(act.refusal.clone().unwrap_or_else(|| act.says.clone()).into()));
    item.held = act.refusal.is_some();
    item.shortcut = act.chord.and_then(|c| seen.label(c));
    item
}

/// The page of projects (`newThreadPage`): every project in pick order, the first nine on a mod digit, then Add a
/// project.
pub fn projects_page(seen: &Seen, mac: bool) -> Vec<Item> {
    let mut out: Vec<Item> = seen
        .projects
        .iter()
        .enumerate()
        .map(|(at, project)| {
            let mut item = Item::new(format!("project:{}", project.id), project.name.clone(), Go::Project(project.id.clone()));
            item.terms = vec![project.name.clone(), project.computer.clone().unwrap_or_default(), project.path.clone()];
            item.lead = Lead::Project(Some(project.id.clone()));
            item.says = Some(Description::Where {
                computer: project.computer.clone().map(SharedString::from),
                here: project.here,
                path: project.path.clone().into(),
            });
            if at < SLOTS {
                item.trailing = keys::label(&format!("mod+{}", at + 1), mac).map(|l| Trailing::Pick(l.into()));
            }
            item
        })
        .collect();
    out.extend(keys::COMMANDS.iter().filter(|c| c.group == Home::Projects).filter_map(|c| command_item(c, seen)));
    out
}

/// The Tasks list (`workspaceItems`): one row per task in the sidebar's order, the one on screen marked, the first
/// nine with their chords.
pub fn tasks(seen: &Seen) -> Vec<Item> {
    seen.tasks
        .iter()
        .enumerate()
        .map(|(at, task)| {
            let mut item = Item::new(
                format!("workspace:{}", task.id),
                task.name.clone(),
                Go::Task { workspace: task.id.clone(), thread: task.thread.clone() },
            );
            item.terms = vec![task.name.clone()];
            item.lead = Lead::Project(task.project.clone());
            let on = task.computer.as_ref().map(|c| format!(" on {c}")).unwrap_or_default();
            item.says = Some(Description::Words(format!("{}{on}", task.state).into()));
            if seen.open.as_ref().is_some_and(|o| o.workspace == task.id) {
                item.trailing = Some(Trailing::Current);
            }
            if at < SLOTS {
                item.shortcut = seen.label(&format!("workspace.select.{}", at + 1));
            }
            item
        })
        .collect()
}

fn thread_item(thread: &Thread) -> Item {
    let mut item = Item::new(
        format!("thread:{}", thread.id),
        thread.title.clone(),
        Go::Thread { workspace: thread.workspace.clone(), thread: thread.id.clone() },
    );
    item.terms = vec![thread.title.clone()];
    item.lead = Lead::Harness(thread.harness.clone());
    item.says = Some(Description::Facts(
        [Some(thread.task.clone()), thread.computer.clone()]
            .into_iter()
            .flatten()
            .filter(|f| !f.is_empty())
            .map(SharedString::from)
            .collect(),
    ));
    item.trailing = Some(Trailing::Status(thread.kind, thread.slot.clone()));
    item
}

/// The threads newest first, at most [`RECENT`].
pub fn recent(seen: &Seen) -> Vec<Item> {
    let mut threads: Vec<&Thread> = seen.threads.iter().collect();
    threads.sort_by(|a, b| b.started_at.unwrap_or(f64::MIN).total_cmp(&a.started_at.unwrap_or(f64::MIN)));
    threads.into_iter().take(RECENT).map(thread_item).collect()
}

/// `searchSidebarThreadsByTitle`: every thread whose title holds the query, case aside.
fn by_title<'a>(seen: &'a Seen, query: &str) -> Vec<&'a Thread> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return Vec::new();
    }
    seen.threads.iter().filter(|t| t.title.to_lowercase().contains(&q)).collect()
}

/// The In messages list: each hit on a thread the sidebar holds and its title did not already find, the snippet as
/// its line; then how many older threads the host did not look in.
fn by_message(seen: &Seen, query: &str, hits: &[Hit], unsearched: usize) -> Vec<Item> {
    let titled = by_title(seen, query);
    let mut out: Vec<Item> = hits
        .iter()
        .filter_map(|hit| {
            let thread = seen.threads.iter().find(|t| t.id == hit.thread && t.workspace == hit.workspace)?;
            if titled.iter().any(|t| std::ptr::eq(*t, thread)) {
                return None;
            }
            let mut item = thread_item(thread);
            item.value = format!("message:{}", thread.id).into();
            item.terms = vec![query.to_owned(), hit.snippet.clone()];
            item.says = Some(Description::Words(hit.snippet.clone().into()));
            Some(item)
        })
        .collect();
    if unsearched > 0 {
        let mut item = Item::new(
            "message:unsearched",
            format!("{unsearched} older {} not searched", if unsearched == 1 { "thread" } else { "threads" }),
            Go::Nothing,
        );
        item.terms = vec![query.to_owned()];
        item.says = Some(Description::Words("Open one to search its messages".into()));
        item.held = true;
        out.push(item);
    }
    out
}

/// `normalizeSearchText`, case and runs of space aside.
pub fn normalize(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

/// `rankSearchFieldMatch`: exact 3, prefix 2, contains 1, every word somewhere 0, else none.
fn rank_field(field: &str, query: &str, words: &[&str]) -> Option<i32> {
    let field = normalize(field);
    if field.is_empty() || !words.iter().all(|w| field.contains(w)) {
        return None;
    }
    Some(if field == query {
        3
    } else if field.starts_with(query) {
        2
    } else if field.contains(query) {
        1
    } else {
        0
    })
}

/// `rankCommandPaletteItemMatch`: the first term that holds every word ranks the row, an earlier term above a later.
pub fn rank(item: &Item, query: &str, words: &[&str]) -> i32 {
    item.terms
        .iter()
        .filter(|t| !t.is_empty())
        .enumerate()
        .find_map(|(at, term)| rank_field(term, query, words).map(|r| 1_000 - at as i32 * 100 + r))
        .unwrap_or(0)
}

/// `filterCommandPaletteGroups`: at rest every list; a query starting `>` keeps Actions alone; any other query drops
/// Recent threads, adds Threads and In messages, and keeps in each list the rows whose terms hold every word, ranked.
pub fn filter(groups: Vec<Group>, query: &str, page: bool, threads: Vec<Item>, messages: Vec<Item>) -> Vec<Group> {
    let actions_only = query.starts_with('>');
    let q = normalize(if actions_only { &query[1..] } else { query });
    if q.is_empty() {
        return if actions_only { groups.into_iter().filter(|g| g.value == "actions").collect() } else { groups };
    }
    let words: Vec<&str> = q.split(' ').collect();
    let mut base: Vec<Group> = if actions_only {
        groups.into_iter().filter(|g| g.value == "actions").collect()
    } else if !page {
        groups.into_iter().filter(|g| g.value != "recent-threads").collect()
    } else {
        groups
    };
    if !page && !actions_only {
        if !threads.is_empty() {
            base.push(Group { value: "threads-search", label: "Threads", items: threads });
        }
        if !messages.is_empty() {
            base.push(Group { value: "messages-search", label: "In messages", items: messages });
        }
    }
    base.into_iter()
        .filter_map(|group| {
            let mut kept: Vec<(i32, usize, Item)> = group
                .items
                .into_iter()
                .enumerate()
                .filter(|(_, item)| {
                    let hay = normalize(&item.terms.join(" "));
                    words.iter().all(|w| hay.contains(w))
                })
                .map(|(at, item)| (rank(&item, &q, &words), at, item))
                .collect();
            kept.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
            (!kept.is_empty()).then(|| Group {
                value: group.value,
                label: group.label,
                items: kept.into_iter().map(|(_, _, i)| i).collect(),
            })
        })
        .collect()
}

/// The lists the palette shows for `query`: the page's, or the root's with Tasks second.
pub fn lists(seen: &Seen, query: &str, page: bool, hits: &[Hit], unsearched: usize, mac: bool) -> Vec<Group> {
    if page {
        let groups = vec![Group { value: "projects", label: "Projects", items: projects_page(seen, mac) }];
        return filter(groups, query, true, Vec::new(), Vec::new());
    }
    let mut groups = Vec::new();
    let actions = actions(seen);
    if !actions.is_empty() {
        groups.push(Group { value: "actions", label: "Actions", items: actions });
    }
    let tasks = tasks(seen);
    if !tasks.is_empty() {
        groups.push(Group { value: "workspaces", label: "Tasks", items: tasks });
    }
    let recent = recent(seen);
    if !recent.is_empty() {
        groups.push(Group { value: "recent-threads", label: "Recent threads", items: recent });
    }
    let titled = by_title(seen, query).into_iter().map(thread_item).collect();
    let messages = by_message(seen, query, hits, unsearched);
    filter(groups, query, false, titled, messages)
}

/// `getCommandPaletteInputPlaceholder`, and the page's own ghost.
pub fn placeholder(page: bool) -> &'static str {
    if page {
        "New thread in..."
    } else {
        "Search commands, tasks, and threads..."
    }
}

/// What a search that finds nothing says (`CommandPaletteResults`).
pub fn nothing(query: &str, page: bool) -> &'static str {
    if page {
        "No matching projects."
    } else if query.starts_with('>') {
        "No matching actions."
    } else {
        "No matching commands, projects, or threads."
    }
}

/// The projects in the order New thread offers them (`inPickOrder`): the leads first, then by the newest turn among
/// each one's threads, ties in the dragged order `ordered` already holds.
pub fn pick_order(ordered: Vec<Project>, leads: &[&str], newest: impl Fn(&str) -> Option<f64>) -> Vec<Project> {
    let lead = |p: &Project| leads.iter().position(|l| *l == p.id).unwrap_or(leads.len());
    let mut out = ordered;
    out.sort_by(|a, b| lead(a).cmp(&lead(b)).then_with(|| newest(&b.id).unwrap_or(f64::MIN).total_cmp(&newest(&a.id).unwrap_or(f64::MIN))));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn values(groups: &[Group], group: &str) -> Vec<String> {
        groups.iter().find(|g| g.value == group).map(|g| g.items.iter().map(|i| i.value.to_string()).collect()).unwrap_or_default()
    }

    fn named(groups: &[Group]) -> Vec<&'static str> {
        groups.iter().map(|g| g.value).collect()
    }

    fn row<'a>(groups: &'a [Group], value: &str) -> &'a Item {
        groups.iter().flat_map(|g| &g.items).find(|i| i.value == value).unwrap_or_else(|| panic!("no row {value}"))
    }

    fn said(item: &Item) -> String {
        match &item.says {
            Some(Description::Words(w)) => w.to_string(),
            other => format!("{other:?}"),
        }
    }

    fn thread(id: &str, workspace: &str, title: &str, at: f64) -> Thread {
        Thread {
            id: id.into(),
            workspace: workspace.into(),
            title: title.into(),
            harness: "claude".into(),
            task: workspace.into(),
            computer: None,
            started_at: Some(at),
            kind: Kind::Resting,
            slot: Some("2h".into()),
        }
    }

    fn task(id: &str) -> Task {
        Task { id: id.into(), thread: format!("{id}-t"), name: id.into(), state: "Running".into(), computer: None, project: None }
    }

    fn project(n: usize) -> Project {
        Project {
            id: format!("p{n}"),
            name: format!("project {n}"),
            computer: None,
            here: true,
            path: format!("/src/p{n}"),
            workspace: None,
        }
    }

    /// A host with two tasks, thirteen threads, one project, the first task on screen with two threads to walk.
    fn busy() -> Seen {
        let mut threads: Vec<Thread> = (0..13).map(|n| thread(&format!("t{n}"), "w1", &format!("thread number {n}"), n as f64)).collect();
        threads.push(thread("parity", "w2", "Parity check", 0.5));
        Seen {
            connected: true,
            projects: vec![project(1)],
            tasks: vec![task("w1"), task("w2")],
            threads,
            open: Some(Open { workspace: "w1".into(), thread: Some("t1".into()), task: "w1".into(), project: Some("p1".into()) }),
            walk: 2,
            ..Seen::default()
        }
    }

    #[test]
    fn every_typed_word_must_be_in_a_rows_terms() {
        let groups = lists(&busy(), "toggle panel", false, &[], 0, false);
        assert_eq!(values(&groups, "actions"), ["action:toggle-right-panel"], "toggle sidebar holds toggle but not panel");
        let groups = lists(&busy(), "  Toggle   SIDEBAR ", false, &[], 0, false);
        assert_eq!(values(&groups, "actions"), ["action:toggle-sidebar"], "case and runs of space aside");
    }

    #[test]
    fn an_earlier_term_ranks_first_then_exact_prefix_contains() {
        let item = |value: &str, terms: &[&str]| {
            let mut item = Item::new(value.to_owned(), value.to_owned(), Go::Nothing);
            item.terms = terms.iter().map(|t| (*t).to_owned()).collect();
            item
        };
        let group = Group {
            value: "actions",
            label: "Actions",
            items: vec![
                item("contains", &["foo bar"]),
                item("later-exact", &["x", "bar"]),
                item("words-apart", &["b a r", "zbarz"]),
                item("prefix", &["bar baz"]),
                item("exact", &["bar"]),
                item("none", &["baz"]),
            ],
        };
        let ranked = filter(vec![group], "bar", false, Vec::new(), Vec::new());
        assert_eq!(values(&ranked, "actions"), ["exact", "prefix", "contains", "later-exact", "words-apart"]);
    }

    #[test]
    fn at_rest_actions_tasks_and_twelve_recent_threads_typing_swaps_recent_for_threads_and_messages() {
        let seen = busy();
        let rest = lists(&seen, "", false, &[], 0, false);
        assert_eq!(named(&rest), ["actions", "workspaces", "recent-threads"]);
        let recent = values(&rest, "recent-threads");
        assert_eq!(recent.len(), RECENT);
        assert_eq!(recent[0], "thread:t12", "newest first");
        assert!(!recent.contains(&"thread:parity".to_owned()), "the thirteenth newest is past the twelve");
        let hits = [
            Hit { workspace: "w2".into(), thread: "parity".into(), snippet: "a parity run".into() },
            Hit { workspace: "w1".into(), thread: "t3".into(), snippet: "words in thread 3".into() },
            Hit { workspace: "w9".into(), thread: "gone".into(), snippet: "a thread the sidebar does not hold".into() },
        ];
        let typed = lists(&seen, "parity", false, &hits, 4, false);
        assert_eq!(named(&typed), ["threads-search", "messages-search"]);
        assert_eq!(values(&typed, "threads-search"), ["thread:parity"]);
        assert_eq!(values(&typed, "messages-search"), ["message:t3", "message:unsearched"], "a title hit is listed once");
        assert_eq!(said(row(&typed, "message:t3")), "words in thread 3", "a hit says its snippet");
        let skipped = row(&typed, "message:unsearched");
        assert_eq!((skipped.title.as_ref(), skipped.held), ("4 older threads not searched", true));
        assert_eq!(named(&lists(&seen, "thread number", false, &[], 0, false)), ["threads-search"], "Recent threads give way to Threads");
        let typed = lists(&seen, "w2", false, &[], 0, false);
        assert_eq!(values(&typed, "workspaces"), ["workspace:w2"]);
    }

    #[test]
    fn a_leading_angle_keeps_actions_alone() {
        let seen = busy();
        assert_eq!(named(&lists(&seen, ">", false, &[], 0, false)), ["actions"]);
        assert_eq!(values(&lists(&seen, ">settings", false, &[], 0, false), "actions"), ["action:settings"]);
        assert!(lists(&seen, ">thread number", false, &[], 0, false).iter().all(|g| g.value == "actions"));
        assert!(lists(&seen, ">zzqx", false, &[], 0, false).is_empty());
        assert_eq!(nothing(">zzqx", false), "No matching actions.");
        assert_eq!(nothing("zzqx", false), "No matching commands, projects, or threads.");
    }

    #[test]
    fn held_rows_say_why() {
        let shut = Seen::default();
        let rows = actions(&shut);
        let held = |rows: &[Item], value: &str| {
            let item = rows.iter().find(|i| i.value == value).unwrap_or_else(|| panic!("no {value}"));
            assert!(item.held, "{value} is held");
            said(item)
        };
        assert_eq!(held(&rows, "action:new-thread"), "Not connected to the runtime");
        assert_eq!(held(&rows, "page:new-thread"), "Pick the project first");
        assert_eq!(held(&rows, "action:next-workspace"), "Only one task");
        assert_eq!(held(&rows, "action:previous-thread"), "No threads to walk");
        assert_eq!(held(&rows, "action:toggle-right-panel"), "Select a task first");
        assert_eq!(held(&rows, "action:resume-conversation"), "Open a project first");
        assert!(rows.iter().all(|i| i.value != "action:copy-markdown"), "no thread on screen, no Copy as Markdown");
        let connected = Seen { connected: true, ..Seen::default() };
        assert_eq!(held(&actions(&connected), "action:new-thread"), "Add a project first");
        let mut one = busy();
        one.walk = 1;
        assert_eq!(held(&actions(&one), "action:next-thread"), "Only one thread");
        one.walk = 0;
        assert_eq!(held(&actions(&one), "action:next-thread"), "Nothing to step to on w1; a walk stays inside one task");
        let open = actions(&busy());
        for value in
            ["action:new-thread", "action:next-workspace", "action:next-thread", "action:toggle-right-panel", "action:copy-markdown"]
        {
            assert!(!open.iter().find(|i| i.value == value).unwrap().held, "{value} runs");
        }
        assert_eq!(said(open.iter().find(|i| i.value == "action:toggle-right-panel").unwrap()), "w1");
        assert_eq!(said(open.iter().find(|i| i.value == "action:resume-conversation").unwrap()), "project 1");
    }

    #[test]
    fn the_page_lists_projects_on_mod_digits_then_add_a_project() {
        let seen = Seen { connected: true, projects: (1..=10).map(project).collect(), ..Seen::default() };
        let page = lists(&seen, "", true, &[], 0, false);
        assert_eq!(named(&page), ["projects"]);
        let rows = &page[0].items;
        assert_eq!(rows.len(), 11);
        assert_eq!(rows[0].trailing, Some(Trailing::Pick("Ctrl+1".into())));
        assert_eq!(rows[8].trailing, Some(Trailing::Pick("Ctrl+9".into())));
        assert_eq!(rows[9].trailing, None, "the tenth has no digit");
        assert_eq!((rows[10].value.as_ref(), rows[10].title.as_ref()), ("action:new-project", "Add a project"));
        let mac = lists(&seen, "", true, &[], 0, true);
        assert_eq!(mac[0].items[0].trailing, Some(Trailing::Pick("⌘1".into())));
        assert_eq!(values(&lists(&seen, "project 10", true, &[], 0, false), "projects"), ["project:p10"]);
        assert!(lists(&seen, "zzqx", true, &[], 0, false).is_empty());
        assert_eq!(nothing("zzqx", true), "No matching projects.");
        assert_eq!(placeholder(true), "New thread in...");
        assert_eq!(placeholder(false), "Search commands, tasks, and threads...");
    }

    #[test]
    fn new_thread_picks_the_leads_then_the_newest() {
        let ordered: Vec<Project> = (1..=4).map(project).collect();
        let newest = |id: &str| match id {
            "p2" => Some(5.),
            "p3" => Some(9.),
            _ => None,
        };
        let picked: Vec<String> = pick_order(ordered, &["p4"], newest).into_iter().map(|p| p.id).collect();
        assert_eq!(picked, ["p4", "p3", "p2", "p1"]);
    }
}
