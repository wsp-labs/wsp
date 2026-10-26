// SPDX-License-Identifier: AGPL-3.0-only
//! `packages/host/src/verbs/activity-table.ts` at 65f6b89c7: `wsp activity` as Settings > Activity reads, the
//! totals, then wsp's own, each thread heaviest first and No thread last, a turn's run script and input pump folded
//! into its agent's row, the figures in aligned columns and nothing wider than the terminal. `--wide` adds the pid
//! and the whole command line and folds nothing. Spec: `spec/activity.md`.

use std::collections::HashMap;

use wsp_catalog::catalog::{agent_name, catalog_agents};
use wsp_frames::{ProcEntry, ProcKind};
use wsp_wire::views::activity::{is_turn_plumbing, proc_bins, ActivityProc, ActivityThread, ActivityView};
use wsp_wire::views::activity_fold::{fold_tree, Folded};
use wsp_wire::views::activity_names::{
    activity_name, agent_by_bin, AgentBin, NameFacts, ACTIVITY_GROUP_WORDS, ACTIVITY_KIND_WORDS, UNTAGGED_KINDS,
};
use wsp_words::exit::HostError;
use wsp_words::js::{collate, js_round, js_trim_end, number_string};
use wsp_words::words::units::fmt_bytes;

use crate::init_layout::{cell_width, ellipsize, pad_cells, Align, GUTTER};

/// `NAME_MIN` (activity-table.ts:13): the narrowest a name column is cut to before the kind column goes instead.
pub const NAME_MIN: usize = 16;
/// `LEFT_MAX` (activity-table.ts:15): the widest the names and kinds stand, whatever the terminal.
pub const LEFT_MAX: usize = 72;

/// The table's two choices: `width` in cells, `None` where the line has no terminal to redraw on (the TS
/// `Infinity`), and `wide`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TableOpts {
    pub width: Option<usize>,
    pub wide: bool,
}

/// One printed line: a group's head, or a process `depth` steps under one.
#[derive(Clone, Debug, Default)]
struct Row {
    label: String,
    tag: String,
    pid: String,
    mem: String,
    cpu: String,
    command: String,
}

/// A process with the figures its group gives it: its own memory and CPU plus what it folds in.
struct Node {
    p: ActivityProc,
    mem: f64,
    cpu: f64,
    kids: Vec<Node>,
}

fn as_entry(p: &ActivityProc) -> ProcEntry {
    ProcEntry {
        pid: p.pid.get() as u32,
        ppid: p.ppid.get() as u32,
        user: String::new(),
        state: String::new(),
        comm: p.name.clone(),
        cmdline: p.command.clone(),
        cpu: 0.0,
        rss: 0,
        started_at: 0,
        pty: None,
        cgroup: None,
        energy_mw: None,
        pgid: None,
        owner: None,
        kind: None,
        id: None,
    }
}

pub(super) fn kind_word(kind: ProcKind) -> &'static str {
    match kind {
        ProcKind::Agent => "agent",
        ProcKind::Tool => "tool",
        ProcKind::Dev => "dev",
        ProcKind::Test => "test",
        ProcKind::Build => "build",
        ProcKind::Slate => "slate",
        ProcKind::Terminal => "terminal",
        ProcKind::Tab => "tab",
        ProcKind::App => "app",
        ProcKind::Wsp => "wsp",
        ProcKind::Other => "other",
    }
}

fn kind_name(kind: ProcKind) -> &'static str {
    ACTIVITY_KIND_WORDS.iter().find(|(key, _)| *key == kind_word(kind)).map_or("", |(_, word)| *word)
}

fn group_word(key: &str) -> &'static str {
    ACTIVITY_GROUP_WORDS.iter().find(|(name, _)| *name == key).map_or("", |(_, word)| *word)
}

pub(super) fn cpu_word(cpu: f64) -> String {
    format!("{}%", wsp_words::js::to_fixed(cpu, 1))
}

/// `agentOf` (activity-table.ts:33): the catalog agent the process's binary names, else `Agent`.
fn agent_of(p: &ActivityProc) -> String {
    let agents: Vec<AgentBin> = catalog_agents().iter().map(|a| AgentBin { id: a.id.clone(), bin: a.bin.clone() }).collect();
    match agent_by_bin(&as_entry(p), &agents) {
        Some(id) => agent_name(&id),
        None => kind_name(ProcKind::Agent).to_owned(),
    }
}

/// An agent that stands as its own row: kind agent, not turn plumbing, and with no agent below it.
fn is_stretch_agent(p: &ActivityProc, kids: &HashMap<i64, Vec<&ActivityProc>>) -> bool {
    p.kind == ProcKind::Agent
        && !is_turn_plumbing(&p.command)
        && !kids.get(&p.pid.get()).is_some_and(|ks| ks.iter().any(|k| k.kind == ProcKind::Agent))
}

/// The first agent under `p` on its stretch of the tree, a stretch agent standing as itself. A table has no more
/// nested steps than it has processes, so one deeper than `limit` is two agents naming each other as parent: node's
/// recursion overflows there (activity-table.ts:52-57) and the line fails.
fn agent_below<'a>(
    p: &ActivityProc,
    kids: &HashMap<i64, Vec<&'a ActivityProc>>,
    depth: usize,
    limit: usize,
) -> Result<Option<&'a ActivityProc>, HostError> {
    if depth > limit {
        return Err(HostError::new("Maximum call stack size exceeded"));
    }
    for k in kids.get(&p.pid.get()).into_iter().flatten() {
        if k.kind != ProcKind::Agent {
            continue;
        }
        let hit = if is_stretch_agent(k, kids) { Some(*k) } else { agent_below(k, kids, depth + 1, limit)? };
        if hit.is_some() {
            return Ok(hit);
        }
    }
    Ok(None)
}

fn sorted(mut list: Vec<Node>) -> Vec<Node> {
    list.sort_by(|a, b| {
        let by_mem = b.mem - a.mem;
        if by_mem.is_nan() || by_mem == 0.0 {
            a.p.pid.get().cmp(&b.p.pid.get())
        } else {
            by_mem.partial_cmp(&0.0).unwrap_or(std::cmp::Ordering::Equal)
        }
    });
    list
}

fn node_of(folded: Folded<ActivityProc>) -> Node {
    let mem = folded.held.iter().fold(folded.p.memory.0, |n, q| n + q.memory.0);
    let cpu = folded.held.iter().fold(folded.p.cpu.0, |n, q| n + q.cpu.0);
    Node { p: folded.p, mem, cpu, kids: sorted(folded.kids.into_iter().map(node_of).collect()) }
}

/// `treeOf` (activity-table.ts:45-71): a group's rows as a tree. Folded, every stretch agent's rows join the agent's
/// own row and the run's pump and leftover shells join the group's first agent.
fn tree_of(rows: &[ActivityProc], fold: bool) -> Result<Vec<Node>, HostError> {
    let mut kids: HashMap<i64, Vec<&ActivityProc>> = HashMap::new();
    for p in rows {
        if p.ppid.get() != p.pid.get() {
            kids.entry(p.ppid.get()).or_default().push(p);
        }
    }
    let mut into: HashMap<i64, i64> = HashMap::new();
    if fold {
        let first = rows.iter().find(|p| is_stretch_agent(p, &kids)).or_else(|| rows.iter().find(|p| p.kind == ProcKind::Agent));
        for p in rows {
            if p.kind != ProcKind::Agent || is_stretch_agent(p, &kids) || Some(p.pid.get()) == first.map(|f| f.pid.get()) {
                continue;
            }
            if let Some(target) = agent_below(p, &kids, 0, rows.len())?.or(first) {
                into.insert(p.pid.get(), target.pid.get());
            }
        }
    }
    let folded = fold_tree(rows, |pid| into.get(&pid).copied());
    Ok(sorted(folded.into_iter().map(node_of).collect()))
}

/// `procRows` (activity-table.ts:72-85): each head's tree as rows, depth one under the head.
fn proc_rows(heads: &[Node], wide: bool, at: &NameFacts) -> Vec<Row> {
    fn walk(n: &Node, depth: usize, wide: bool, at: &NameFacts, out: &mut Vec<Row>) {
        let p = &n.p;
        let mut facts = at.clone();
        if p.kind == ProcKind::Agent && (!wide || !is_turn_plumbing(&p.command)) {
            facts.agent = Some(agent_of(p));
        }
        let name = activity_name(&as_entry(p), &facts);
        let ports = if p.ports.is_empty() {
            String::new()
        } else {
            format!(" :{}", p.ports.iter().map(|q| q.get().to_string()).collect::<Vec<_>>().join(" :"))
        };
        let tag = if UNTAGGED_KINDS.contains(&kind_word(p.kind)) { String::new() } else { kind_name(p.kind).to_owned() };
        out.push(Row {
            label: format!("{}{name}{ports}", "  ".repeat(depth)),
            tag,
            pid: p.pid.get().to_string(),
            mem: fmt_bytes(n.mem),
            cpu: cpu_word(n.cpu),
            command: if wide { p.command.clone() } else { String::new() },
        });
        for k in &n.kids {
            walk(k, depth + 1, wide, at, out);
        }
    }
    let mut out = Vec::new();
    for n in heads {
        walk(n, 1, wide, at, &mut out);
    }
    out
}

fn head(label: String, mem: f64, cpu: f64) -> Row {
    Row { label, mem: fmt_bytes(mem), cpu: cpu_word(cpu), ..Row::default() }
}

fn sum(rows: &[ActivityProc]) -> (f64, f64) {
    (rows.iter().map(|p| p.memory.0).sum(), rows.iter().map(|p| p.cpu.0).sum())
}

/// `cell.padStart(width)` in UTF-16 units, as the TS pads the figure columns.
fn pad_start(text: &str, width: usize) -> String {
    let len = wsp_words::js::utf16::len(text);
    format!("{}{text}", " ".repeat(width.saturating_sub(len)))
}

fn widest(list: &[&Row], f: fn(&Row) -> &str) -> usize {
    list.iter().map(|r| cell_width(f(r))).max().unwrap_or(0)
}

/// `activityTable` (activity-table.ts:91-134), its failure as the line's error.
pub fn try_activity_table(view: &ActivityView, o: TableOpts) -> Result<String, HostError> {
    let fold = !o.wide;
    let mut rows: Vec<Row> = Vec::new();
    let (mut mem, mut cpu, mut processes) = (0.0_f64, 0.0_f64, 0usize);
    let mut group = |label: String, figures: (f64, f64), procs: Vec<Row>| {
        rows.push(head(label, figures.0, figures.1));
        processes += procs.len();
        rows.extend(procs);
        mem += figures.0;
        cpu += figures.1;
    };

    if !view.wsp.is_empty() {
        let daemon = view
            .wsp
            .iter()
            .find(|p| p.kind == ProcKind::Wsp && proc_bins(&as_entry(p)).iter().any(|b| b.as_str() == "wsp-daemon"))
            .map(|p| p.pid.get());
        let host = view.wsp.iter().find(|p| p.kind == ProcKind::Wsp && Some(p.pid.get()) != daemon).map(|p| p.pid.get());
        let note = if host.is_none() { group_word("wspNoteBox") } else { group_word("wspNote") };
        let facts = NameFacts { daemon, host, agent: None };
        group(format!("{}  {note}", group_word("wsp")), sum(&view.wsp), proc_rows(&tree_of(&view.wsp, false)?, o.wide, &facts));
    }
    let mut threads: Vec<&ActivityThread> = view.threads.iter().collect();
    threads.sort_by(|a, b| {
        let by_mem = b.memory.0 - a.memory.0;
        if by_mem.is_nan() || by_mem == 0.0 {
            collate(&a.thread_id, &b.thread_id)
        } else {
            by_mem.partial_cmp(&0.0).unwrap_or(std::cmp::Ordering::Equal)
        }
    });
    for t in &threads {
        group(t.title.clone(), (t.memory.0, t.cpu.0), proc_rows(&tree_of(&t.processes, fold)?, o.wide, &NameFacts::default()));
    }
    if !view.unowned.is_empty() {
        let label = format!("{}  {}", group_word("loose"), group_word("looseNote"));
        group(label, sum(&view.unowned), proc_rows(&tree_of(&view.unowned, false)?, o.wide, &NameFacts::default()));
    }
    if rows.is_empty() {
        return Ok(format!("no thread runs a process on {}", view.computer));
    }

    let thread_count = threads.len();
    let totals = [
        view.computer.clone(),
        format!("{} {}", group_word("wsp"), fmt_bytes(mem)),
        format!("CPU {}%", number_string(js_round(cpu))),
        format!("{processes} {}", if processes == 1 { "process" } else { "processes" }),
        format!("{thread_count} {}", if thread_count == 1 { "thread" } else { "threads" }),
    ]
    .join("   ");
    let titles = Row {
        pid: if o.wide { "PID".to_owned() } else { String::new() },
        mem: "MEMORY".to_owned(),
        cpu: "CPU".to_owned(),
        command: if o.wide { "COMMAND".to_owned() } else { String::new() },
        ..Row::default()
    };
    let all: Vec<&Row> = std::iter::once(&titles).chain(rows.iter()).collect();
    let pid_w = widest(&all, |r| &r.pid);
    let mem_w = widest(&all, |r| &r.mem);
    let cpu_w = widest(&all, |r| &r.cpu);
    let figures = (if o.wide { pid_w + GUTTER.len() } else { 0 }) + mem_w + GUTTER.len() + cpu_w;
    let procs: Vec<&Row> = rows.iter().filter(|r| !r.pid.is_empty()).collect();
    let all_labels: Vec<&Row> = rows.iter().collect();
    let room: i64 = if o.wide { i64::MAX } else { o.width.map_or(i64::MAX, |w| w as i64) };
    let gutter = GUTTER.len() as i64;

    let mut tag_w = widest(&procs, |r| &r.tag);
    let tag_part = |tag_w: usize| if tag_w > 0 { gutter + tag_w as i64 } else { 0 };
    let proc_label_w = widest(&procs, |r| &r.label) as i64;
    let left = 1_i64.max(
        (proc_label_w + tag_part(tag_w))
            .max(widest(&all_labels, |r| &r.label) as i64)
            .min(LEFT_MAX as i64)
            .min(room - gutter - figures as i64),
    );
    if left - tag_part(tag_w) < (NAME_MIN as i64).min(proc_label_w) {
        tag_w = 0;
    }
    let name_w = proc_label_w.min(left - tag_part(tag_w));

    let line = |r: &Row| -> String {
        let lead = if r.pid.is_empty() {
            ellipsize(&r.label, left)
        } else {
            let name = pad_cells(&ellipsize(&r.label, name_w), name_w.max(0) as usize, Align::Left);
            let tag = if tag_w > 0 { format!("{GUTTER}{}", r.tag) } else { String::new() };
            format!("{name}{tag}")
        };
        let mut cells = vec![pad_cells(&lead, left as usize, Align::Left)];
        if o.wide {
            cells.push(pad_start(&r.pid, pid_w));
        }
        cells.push(pad_start(&r.mem, mem_w));
        cells.push(pad_start(&r.cpu, cpu_w));
        if o.wide && !r.command.is_empty() {
            cells.push(r.command.clone());
        }
        ellipsize(js_trim_end(&cells.join(GUTTER)), room)
    };
    let mut out = vec![ellipsize(&totals, room), String::new()];
    out.extend(all.iter().map(|r| line(r)));
    Ok(out.join("\n"))
}

/// `activityTable` (activity-table.ts:91-134) for a view that holds no agent cycle.
pub fn activity_table(view: &ActivityView, o: TableOpts) -> String {
    try_activity_table(view, o).unwrap_or_else(|e| e.message)
}
