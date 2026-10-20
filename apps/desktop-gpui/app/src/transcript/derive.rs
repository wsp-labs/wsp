// SPDX-License-Identifier: AGPL-3.0-only
//! The rows the fold's entries and turns draw.

use std::collections::{HashMap, HashSet};

use crate::words::fmt_duration;

use super::{
    block_at,
    groups::{
        active_activity, calls_row, elapsed, failure, group_id, group_kind, kind_glyph, stands_alone, success, summary, tool_like,
        visible_in_group, work_of, work_row, Live,
    },
    model::{listed_failure, listed_last_line, sub_word},
    plan::plan_title,
    prompt::prompt_row,
    Body, Can, Child, Entry, FooterWord, Kid, Listing, Model, Ran, Row, RowKind, Run, State, Status, SubLine, SubState, Summary, Work,
    EARLIER,
};

/// The key of the opening message a thread with no event yet draws off its running row.
const OPENING: &str = "opening";

/// What derive treats as open: the ids the person opened, or with `all` every fold and group (a call still by its id).
#[derive(Clone, Copy)]
pub(super) struct Opened<'a> {
    pub(super) ids: &'a HashSet<String>,
    pub(super) shut: &'a HashSet<String>,
    pub(super) all: bool,
    pub(super) can: Can,
    pub(super) source: Option<&'a str>,
    /// What the listing holds: a call stands as its child's row only where the listing holds that child.
    pub(super) listing: Option<&'a Listing>,
}

impl Opened<'_> {
    fn contains(&self, id: &str) -> bool {
        self.all || self.ids.contains(id)
    }
}

/// deriveMessagesTimelineRows from `from`: the rows and how many precede the latest turn; None if the working line lands earlier.
pub(super) fn derive(model: &Model, from: usize, turns_from: usize, opened: Opened) -> Option<(Vec<Row>, Option<usize>)> {
    let entries = &model.entries;
    let len = entries.len();
    let by_id: HashMap<&str, &Summary> = model.turns[turns_from..].iter().map(|s| (s.id.as_str(), s)).collect();
    let latest = model.turns.last();
    let unsettled = latest.filter(|t| t.state == State::Running).map(|t| t.id.as_str());
    let listing = opened.listing;
    let working = unsettled.is_some() && !listing.is_some_and(|l| l.spawning);
    let boundary = model.latest.as_ref().map(|m| m.entries);
    let turn_of = |ix: usize| entries[ix].turn.as_deref();

    // A fork's rule stands before its first entry of no turn it was handed: its own first message, or the end.
    let copied: HashSet<&str> = model.turns.iter().filter(|t| t.copied).map(|t| t.id.as_str()).collect();
    let rule_at = (!copied.is_empty())
        .then(|| (0..len).find(|&ix| owner(&entries[ix]).is_none_or(|t| !copied.contains(t))).unwrap_or(len))
        .filter(|&at| at >= from);
    let source = opened.source.or(model.source.as_deref()).unwrap_or_default();

    // Rewind to here and Fork from here (rewindableReplies in RewindDialog.tsx, forkableMessages in forks.ts).
    let can = opened.can;
    let at_of: HashMap<&str, usize> = model.turns.iter().enumerate().skip(turns_from).map(|(i, t)| (t.id.as_str(), i)).collect();
    let kept_why = model.turns.iter().any(|t| t.kept.as_ref().is_some_and(|k| k.kept));
    let anchored = |t: &Summary| t.kept.as_ref().is_some_and(|k| k.anchor);
    let cuts = |t: &Summary| can.forks_by_count || anchored(t);
    let rewind_of = |i: usize| {
        let t = &model.turns[i];
        let files = t.kept.as_ref().is_some_and(|k| k.files);
        let conversation = !kept_why && can.rewinds_conversation && (anchored(t) || can.rewinds_by_count);
        let earlier = i + 1 < model.turns.len() && t.state != State::Running && !t.copied;
        (earlier && (files || conversation)).then(|| t.id.clone())
    };
    let reply_fork = |i: usize| {
        let t = &model.turns[i];
        (can.forks && t.state != State::Running && cuts(t)).then(|| t.id.clone())
    };
    let opener_fork = |i: usize| {
        if !can.forks || model.turns[i].state == State::Running {
            return None;
        }
        match i.checked_sub(1) {
            None => (model.from_start && model.earlier == 0).then(String::new),
            Some(before) => cuts(&model.turns[before]).then(|| model.turns[before].id.clone()),
        }
    };
    let mut last_reply: HashMap<&str, usize> = HashMap::new();
    let mut openers: HashSet<usize> = HashSet::new();
    let mut opened_turns: HashSet<&str> = HashSet::new();
    for (ix, e) in entries.iter().enumerate().skip(from) {
        match &e.body {
            Body::Agent { .. } => {
                if let Some(turn) = e.turn.as_deref() {
                    last_reply.insert(turn, ix);
                }
            }
            Body::Person { turn: Some(turn), steered: false, .. } if opened_turns.insert(turn.as_str()) => _ = openers.insert(ix),
            _ => {}
        }
    }
    let mut runs: HashMap<&str, Vec<(usize, &Ran)>> = HashMap::new();
    for (block, ran) in &model.runs {
        if let Some((message, offset)) = block.rsplit_once('@').and_then(|(m, o)| Some((m, o.parse::<usize>().ok()?))) {
            runs.entry(message).or_default().push((offset, ran));
        }
    }

    // The children calls started: spawnCallOf in adapt/spawned.ts, with the subagents session.subagent names.
    let child_of = |call: &String, kid: &Kid| {
        let settled = kid.state != SubState::Running && !(working && kid.turn.as_deref() == unsettled);
        let note = match (settled, kid.state, kid.summary.as_deref()) {
            (false, SubState::Failed, Some(why)) => Some(listed_failure(why)),
            (false, SubState::Done, Some(said)) => listed_last_line(said),
            _ => None,
        };
        let status = Some(sub_word(kid.state).to_owned());
        Child {
            thread: None,
            title: kid.title.clone(),
            note,
            status,
            subagent: true,
            settled,
            call: Some(call.clone()),
            listed: None,
            workspace: None,
        }
    };
    // spawnCallOf: a call is its child's row only where the listing holds that child, else it stands as itself.
    let holds_thread = |thread: &str| opened.listing.is_some_and(|l| l.child(thread).is_some());
    let holds_call = |call: &str| opened.listing.is_some_and(|l| l.subagent(call).is_some());
    let spawn_of = |ix: usize| match &entries[ix].body {
        Body::Subagent(s) if holds_call(&s.parent) => model.kids.get(&s.parent).map(|kid| child_of(&s.parent, kid)),
        Body::Subagent(_) => None,
        Body::Work(w) => match &w.spawned {
            Some((thread, title)) if holds_thread(thread) => Some(Child {
                thread: Some(thread.clone()),
                title: title.clone(),
                note: None,
                status: None,
                subagent: false,
                settled: false,
                call: None,
                listed: None,
                workspace: None,
            }),
            _ => w.call.as_ref().filter(|call| holds_call(call)).and_then(|call| model.kids.get(call).map(|kid| child_of(call, kid))),
        },
        _ => None,
    };

    let mut replies: HashMap<(Option<&str>, usize), usize> = HashMap::new();
    let mut users = 0;
    for (ix, e) in entries.iter().enumerate().skip(from) {
        match &e.body {
            Body::Person { .. } => users += 1,
            Body::Agent { .. } => _ = replies.insert((e.turn.as_deref(), if e.turn.is_some() { 0 } else { users }), ix),
            _ => {}
        }
    }
    let terminal: HashSet<usize> = replies.into_values().collect();

    let mut groups: Vec<(&str, Vec<usize>, bool)> = Vec::new();
    let mut group_at: HashMap<&str, usize> = HashMap::new();
    for (ix, e) in entries.iter().enumerate().skip(from) {
        let (Some(turn), false) = (e.turn.as_deref(), matches!(e.body, Body::Person { .. })) else { continue };
        let at = *group_at.entry(turn).or_insert_with(|| {
            groups.push((turn, Vec::new(), false));
            groups.len() - 1
        });
        groups[at].1.push(ix);
        groups[at].2 |= matches!(e.body, Body::Agent { streaming: true, .. });
    }
    let mut folds: HashMap<usize, (&str, String)> = HashMap::new();
    let mut stops: Vec<(usize, &str, String)> = Vec::new();
    let mut collapsed = vec![false; len - from];
    for (turn, members, streaming) in &groups {
        if Some(*turn) == unsettled || *streaming {
            continue;
        }
        let summary = by_id.get(turn);
        let (first, last) = (members[0], members[members.len() - 1]);
        let worked = summary.and_then(|s| s.worked()).or_else(|| {
            elapsed(summary.and_then(|s| s.started).or(entries[first].at), summary.and_then(|s| s.completed).or(entries[last].at))
        });
        if summary.is_some_and(|s| s.state == State::Interrupted) {
            let label = worked.map_or("You stopped this response".to_owned(), |ms| format!("You stopped after {}", fmt_duration(ms)));
            stops.push((last, *turn, label));
            continue;
        }
        let kept = |ix: &usize| {
            terminal.contains(ix) || matches!(&entries[*ix].body, Body::Plan(_)) || work_of(&entries[*ix]).is_some_and(stands_alone)
        };
        let hidden: Vec<usize> = members.iter().copied().filter(|ix| !kept(ix)).collect();
        let Some(&first_hidden) = hidden.first() else { continue };
        let label = worked.map_or("Worked".to_owned(), |ms| format!("Worked for {}", fmt_duration(ms)));
        folds.insert(first_hidden, (*turn, label));
        if !opened.contains(turn) {
            for ix in hidden {
                collapsed[ix - from] = true;
            }
        }
    }
    stops.sort_by_key(|(last, _, _)| *last);
    let tail_earlier = entries[from..].iter().filter(|e| e.earlier).count();
    let first_earlier = (model.earlier == tail_earlier).then(|| (from..len).find(|&ix| entries[ix].earlier)).flatten();
    if !opened.contains(EARLIER) {
        for ix in from..len {
            collapsed[ix - from] |= entries[ix].earlier;
        }
    }

    let header = if working {
        let last_user = (from..len).rev().find(|&ix| matches!(entries[ix].body, Body::Person { .. }));
        let owned = (last_user.map_or(from, |u| u + 1)..len).find(|&ix| turn_of(ix) == unsettled);
        match (owned, last_user) {
            (Some(ix), _) => ix,
            (None, Some(u)) => u + 1,
            (None, None) if from == 0 => 0,
            (None, None) => return None,
        }
    } else {
        len
    };
    let in_run = |ix: usize, w: &Work| w.status == Some(Status::Running) && working && turn_of(ix) == unsettled;

    let mut trailing: Vec<usize> = Vec::new();
    if working {
        for ix in (header..len).rev() {
            match work_of(&entries[ix]) {
                Some(w) if turn_of(ix) == unsettled && !stands_alone(w) && spawn_of(ix).is_none() => trailing.push(ix),
                _ => break,
            }
        }
        trailing.reverse();
    }
    let shown: Vec<usize> =
        trailing.iter().copied().filter(|&ix| work_of(&entries[ix]).is_some_and(|w| visible_in_group(w, true))).collect();
    let latest_shown = shown.last().copied();
    let latest_running = shown.iter().rev().copied().find(|&ix| work_of(&entries[ix]).is_some_and(active_activity));
    let latest_failed = latest_running.is_none() && latest_shown.and_then(|ix| work_of(&entries[ix])).is_some_and(failure);
    let keeps_live = latest_running.is_some() || latest_shown.and_then(|ix| work_of(&entries[ix])).is_some_and(success);
    let live = match (trailing.first(), latest_running.or(latest_shown)) {
        (Some(&anchor), Some(ix)) if !latest_failed => {
            let anchor_work = work_of(&entries[anchor])?;
            let group = group_id(&entries[anchor], anchor_work);
            Some(Live {
                key: if keeps_live { "live".to_owned() } else { group.clone() },
                open: opened.contains(&group),
                group,
                entry: &entries[ix],
                work: work_of(&entries[ix])?,
                calls: shown.clone(),
                active: keeps_live,
            })
        }
        _ => None,
    };
    let taken: HashSet<usize> = if live.is_some() || latest_failed { trailing.iter().copied().collect() } else { HashSet::new() };
    let waiting = unsettled.is_some()
        && entries[from..].iter().any(|e| e.turn.as_deref() == unsettled && matches!(&e.body, Body::Prompt(p) if p.outcome.is_none()));
    let retry = model.retry.as_ref().filter(|r| working && r.turn.as_deref() == unsettled).map(|r| r.line.clone());
    let since = latest.and_then(|t| t.started).map(|at| listing.and_then(|l| l.running_since).map_or(at, |row| row.min(at)));
    let working_row = || Row {
        key: "working".to_owned(),
        kind: RowKind::Working { since, waiting, retry: retry.clone(), wait: None },
        text: if waiting { "Waiting for you" } else { "Working" }.to_owned(),
        turn: unsettled.map(str::to_owned),
    };

    let mut rows: Vec<Row> = Vec::new();
    if let Some(text) =
        listing.filter(|l| from == 0 && len == 0 && model.turns.is_empty() && l.sent.is_none()).and_then(|l| l.opening.clone())
    {
        let full = opened.ids.contains(&format!("full:{OPENING}"));
        let kind = RowKind::Person { at: listing.and_then(|l| l.running_since), files: Vec::new(), fork: None, full, steered: false };
        rows.push(Row { key: OPENING.to_owned(), kind, text, turn: None });
    }
    let mut split = None;
    let mut activity = false;
    let mut unsaid = stops.into_iter().peekable();
    let mut ix = from;
    loop {
        while let Some((_, turn, label)) = unsaid.next_if(|(last, _, _)| *last < ix) {
            rows.push(Row { key: format!("stop:{turn}"), kind: RowKind::Stopped, text: label, turn: Some(turn.to_owned()) });
        }
        if boundary == Some(ix) {
            split = Some(rows.len());
        }
        if rule_at == Some(ix) {
            let kind = RowKind::Fork { source: source.to_owned(), open: None };
            rows.push(Row { key: "fork-rule".to_owned(), kind, text: format!("Forked from {source}"), turn: None });
        }
        if working && header == ix {
            rows.push(working_row());
        }
        if ix == len {
            break;
        }
        let e = &entries[ix];
        if let Some(live) = live.as_ref().filter(|_| latest_shown == Some(ix)) {
            rows.push(live.row());
            activity |= live.active;
            if live.open {
                rows.push(calls_row(&live.group, &live.calls, entries, opened.ids));
            }
        }
        if first_earlier == Some(ix) {
            let text = "Earlier, outside wsp".to_owned();
            rows.push(Row {
                key: format!("fold:{EARLIER}"),
                kind: RowKind::Fold { open: opened.contains(EARLIER) },
                text,
                turn: Some(EARLIER.to_owned()),
            });
        }
        if let Some((turn, label)) = folds.get(&ix) {
            let kind = RowKind::Fold { open: opened.contains(turn) };
            rows.push(Row { key: format!("fold:{turn}"), kind, text: label.clone(), turn: Some((*turn).to_owned()) });
        }
        if collapsed[ix - from] || taken.contains(&ix) {
            ix += 1;
            continue;
        }
        if let Some(first) = spawn_of(ix) {
            let mut children = vec![first];
            let mut end = ix + 1;
            while end < len && !collapsed[end - from] && !taken.contains(&end) && !folds.contains_key(&end) && entries[end].turn == e.turn {
                let Some(child) = spawn_of(end) else { break };
                children.push(child);
                end += 1;
            }
            rows.push(Row {
                key: format!("spawn:{}", e.key),
                kind: RowKind::Spawn { children },
                text: String::new(),
                turn: e.turn.clone(),
            });
            ix = end;
            continue;
        }
        match &e.body {
            Body::Work(w) if stands_alone(w) => rows.push(work_row(&e.key, e, w, &opened)),
            Body::Work(_) => {
                let mut end = ix + 1;
                while end < len
                    && work_of(&entries[end]).is_some_and(|w| !stands_alone(w))
                    && !taken.contains(&end)
                    && !collapsed[end - from]
                    && !folds.contains_key(&end)
                    && entries[end].turn == e.turn
                    && spawn_of(end).is_none()
                {
                    end += 1;
                }
                let members: Vec<usize> =
                    (ix..end).filter(|&k| work_of(&entries[k]).is_some_and(|w| visible_in_group(w, in_run(k, w)))).collect();
                let works: Vec<&Work> = members.iter().filter_map(|&k| work_of(&entries[k])).collect();
                match works.as_slice() {
                    [] => {}
                    [only] if !tool_like(only) => rows.push(work_row(&e.key, &entries[members[0]], only, &opened)),
                    _ => {
                        let group = work_of(e).map(|w| group_id(e, w)).unwrap_or_default();
                        let open = opened.contains(&group);
                        let running: Vec<usize> =
                            members.iter().copied().filter(|&k| work_of(&entries[k]).is_some_and(|w| in_run(k, w))).collect();
                        if let Some(&last) = running.last() {
                            let live = Live {
                                key: group.clone(),
                                group: group.clone(),
                                entry: &entries[last],
                                work: work_of(&entries[last])?,
                                calls: members.clone(),
                                active: true,
                                open,
                            };
                            rows.push(live.row());
                            activity = true;
                        } else {
                            let kind = RowKind::Group { group: group.clone(), glyph: kind_glyph(group_kind(&works)), open };
                            rows.push(Row { key: group.clone(), kind, text: summary(&works), turn: e.turn.clone() });
                        }
                        if open {
                            rows.push(calls_row(&group, &members, entries, opened.ids));
                        }
                    }
                }
                ix = end;
                continue;
            }
            Body::Plan(text) => {
                let title = plan_title(text).unwrap_or_else(|| "Proposed plan".to_owned());
                let kind = RowKind::Plan { title, open: opened.contains(&format!("plan:{}", e.key)) };
                rows.push(Row { key: e.key.clone(), kind, text: text.clone(), turn: e.turn.clone() });
            }
            Body::Prompt(p) => rows.push(prompt_row(&e.key, e.turn.clone(), p, None)),
            Body::Subagent(s) => {
                let mut waiting = s.prompts.iter().filter(|p| p.outcome.is_none()).peekable();
                let open = match waiting.peek() {
                    Some(_) => opened.all || waiting.any(|p| !opened.shut.contains(&p.ask)),
                    None => opened.contains(&e.key),
                };
                let word = match s.state {
                    SubState::Running => None,
                    SubState::Done => Some("Done"),
                    SubState::Failed => Some("Failed"),
                    SubState::Stopped => Some("Stopped"),
                };
                let took = s.started.zip(s.ended).map(|(from, to)| to - from).filter(|ms| *ms > 0.0).map(fmt_duration);
                let state = match (word, took) {
                    (Some(word), Some(took)) => Some(format!("{word} {took}")),
                    (word, took) => word.map(str::to_owned).or(took),
                };
                let lines = s.lines.iter().map(|l| SubLine { text: l.label.clone(), mono: l.mono, detail: l.detail.clone() }).collect();
                let kind = RowKind::Subagent { id: e.key.clone(), title: s.title.clone(), state, open, lines };
                rows.push(Row { key: e.key.clone(), kind, text: s.title.clone(), turn: e.turn.clone() });
                if open {
                    for p in &s.prompts {
                        let asker = Some(format!("{} asks", s.title));
                        rows.push(prompt_row(&format!("permission:{}", p.ask), e.turn.clone(), p, asker));
                    }
                }
            }
            Body::Person { text, files, turn, steered } => {
                let fork = turn.as_deref().and_then(|t| at_of.get(t)).filter(|_| openers.contains(&ix)).and_then(|&i| opener_fork(i));
                rows.push(Row {
                    key: e.key.clone(),
                    kind: RowKind::Person {
                        at: e.at,
                        files: files.clone(),
                        fork,
                        full: opened.ids.contains(&format!("full:{}", e.key)),
                        steered: *steered,
                    },
                    text: text.clone(),
                    turn: None,
                });
            }
            Body::Agent { text, streaming, updated } => {
                let still = e.turn.is_some() && e.turn.as_deref() == unsettled;
                let last = e.turn.as_deref().filter(|t| last_reply.get(t) == Some(&ix));
                let at = last.and_then(|t| at_of.get(t).copied());
                let mut ran: Vec<Run> = runs
                    .get(e.key.as_str())
                    .into_iter()
                    .flatten()
                    .filter_map(|(offset, r)| {
                        Some(Run {
                            run_id: r.run_id.clone(),
                            block: block_at(text, *offset)?,
                            command: r.command.clone(),
                            state: r.state.clone(),
                            output: r.output.clone(),
                            exit_code: r.exit_code,
                            signal: r.signal.clone(),
                            pty_id: r.pty_id.clone(),
                        })
                    })
                    .collect();
                ran.sort_by(|a, b| a.block.cmp(&b.block).then_with(|| a.run_id.cmp(&b.run_id)));
                let kind = RowKind::Agent {
                    at: *updated,
                    meta: terminal.contains(&ix) && !still,
                    streaming: *streaming,
                    rewind: at.and_then(rewind_of),
                    fork: at.and_then(reply_fork),
                    runs: ran,
                };
                rows.push(Row { key: e.key.clone(), kind, text: text.clone(), turn: e.turn.clone() });
                if let Some(row) = last.and_then(|t| by_id.get(t)).and_then(|t| changes_row(t)) {
                    rows.push(row);
                }
            }
        }
        ix += 1;
    }
    if working && boundary.is_some_and(|b| header < b) {
        split = None;
    }
    if working && !waiting && (!activity || latest_failed) {
        // Nothing thinks while the model call is being made again: the row keeps its height and says nothing.
        rows.push(Row {
            key: "thinking".to_owned(),
            kind: RowKind::Thinking,
            text: if retry.is_some() { String::new() } else { "Thinking".to_owned() },
            turn: unsettled.map(str::to_owned),
        });
    }
    if let Some(t) = latest.filter(|t| t.state != State::Running) {
        let on_reply = t.state == State::Completed || (t.state == State::Failed && t.error.is_some() && !t.limit);
        if !on_reply {
            let (word, text) = match t.state {
                _ if t.limit => (FooterWord::LimitReached, "limit reached"),
                State::Interrupted => (FooterWord::Interrupted, "interrupted"),
                _ => (FooterWord::Failed, "failed"),
            };
            // Any other stop is the person's own, and its word says it all whatever the agent wrote as its reason.
            let why = t.error.clone().filter(|_| t.state == State::Interrupted && t.unreached);
            rows.push(Row {
                key: "footer".to_owned(),
                kind: RowKind::Footer { word, why },
                text: text.to_owned(),
                turn: Some(t.id.clone()),
            });
        }
    }
    Some((rows, split))
}

/// The turn an entry belongs to, a person's message included.
pub(super) fn owner(e: &Entry) -> Option<&str> {
    match &e.body {
        Body::Person { turn, .. } => turn.as_deref(),
        _ => e.turn.as_deref(),
    }
}

/// The files a turn changed, under its last reply (ChangedFilesCard in ChangedFilesTree.tsx).
fn changes_row(t: &Summary) -> Option<Row> {
    let changes = t.changes.as_ref()?;
    let others = changes.others.clone().unwrap_or_default();
    if changes.files.is_empty() && others.is_empty() {
        return None;
    }
    let n = changes.files.len();
    let count = format!("{n} changed file{}", if n == 1 { "" } else { "s" });
    let label = if n == 0 {
        "No files from this thread's edits".to_owned()
    } else if changes.others.is_none() && changes.shared {
        format!("{count} in this folder")
    } else {
        count
    };
    let range = (changes.from.clone(), changes.to.clone());
    let kind = RowKind::ChangedFiles { label: label.clone(), files: changes.files.clone(), others, range };
    Some(Row { key: format!("changes:{}", t.id), kind, text: label, turn: Some(t.id.clone()) })
}
