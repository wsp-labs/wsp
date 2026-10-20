// SPDX-License-Identifier: AGPL-3.0-only

use super::*;

/// A thread of `turns` turns using every event the fold reads, some landing on a turn after the next one opened.
fn thread(turns: usize) -> Vec<SessionEvent> {
    let mut events = Vec::new();
    let mut pos = 0u64;
    let mut at = 1_000.0;
    let mut push = |kind: &str, turn: usize, extra: serde_json::Value| {
        pos += 1;
        at += 250.0;
        let mut v = json!({"type": kind, "workspaceId": "w", "sessionId": "s", "turnId": format!("u{turn}"), "pos": pos, "at": at});
        v.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        events.push(event(v));
        pos
    };
    let mut replies = Vec::new();
    let files = |n: usize| json!([{"path": format!("src/f{n}.rs"), "kind": "modified", "additions": 3, "deletions": 1}]);
    for n in 0..turns {
        let mut start = json!({"prompt": format!("ask {n}")});
        if n < 2 {
            start["copiedFrom"] = json!("source-thread");
        }
        if n % 4 == 0 {
            start["requestId"] = json!(format!("q{n}"));
            start["attachments"] = json!([{"mediaType": "image/png", "bytes": 10, "name": "a.png"}]);
        }
        push("session.start", n, start);
        if n % 4 == 3 {
            push("session.changes", n - 1, json!({"from": "a", "to": "b", "files": files(n), "moved": []}));
        }
        if n % 5 == 1 {
            push("session.checkpoint", n - 1, json!({"ref": "c", "anchor": "x"}));
        }
        if n % 6 == 2 && n >= 3 {
            let block: &String = &replies[n - 2];
            push("session.run", n - 2, json!({"runId": format!("run{n}"), "block": block, "command": "ls", "state": "running"}));
        }
        push("session.delta", n, json!({"kind": "thinking", "text": "First "}));
        push("session.delta", n, json!({"kind": "thinking", "text": "thought"}));
        for word in ["Let ", "me ", "look."] {
            push("session.delta", n, json!({"kind": "text", "text": word, "messageId": format!("a{n}")}));
        }
        push(
            "session.delta",
            n,
            json!({"kind": "tool_use", "toolName": "Bash", "toolUseId": format!("b{n}"), "text": "{\"command\":\"cargo test\"}"}),
        );
        if n % 5 == 2 {
            push("session.steer", n, json!({"prompt": "also check docs"}));
            let options = json!([{"id": "y", "label": "Allow", "effect": "allow"}, {"id": "n", "label": "Deny", "effect": "deny"}]);
            let input = "{\"command\":\"cargo test\"}";
            push("session.permission", n, json!({"askId": format!("k{n}"), "toolName": "Bash", "input": input, "options": options}));
            push("session.permission.closed", n, json!({"askId": format!("k{n}"), "outcome": "allowed", "optionId": "y"}));
        }
        push("session.delta", n, json!({"kind": "tool_result", "toolUseId": format!("b{n}"), "text": "ok\n", "exitCode": 0}));
        push(
            "session.delta",
            n,
            json!({"kind": "tool_use", "toolName": "Read", "toolUseId": format!("r{n}"), "text": "{\"file_path\":\"src/a.rs\"}"}),
        );
        push("session.delta", n, json!({"kind": "tool_result", "toolUseId": format!("r{n}"), "text": "fn a() {}"}));
        if n % 3 == 0 {
            let task = format!("t{n}");
            let input = "{\"description\":\"Survey the code\",\"prompt\":\"look around\"}";
            push("session.delta", n, json!({"kind": "tool_use", "toolName": "Task", "toolUseId": task, "text": input}));
            if n % 6 == 0 {
                push(
                    "session.subagent",
                    n,
                    json!({"task": format!("k-{task}"), "state": "running", "parentToolUseId": task, "title": "Survey"}),
                );
            }
            push("session.delta", n, json!({"kind": "text", "text": "Reading.", "parentToolUseId": task}));
            let read = json!({"kind": "tool_use", "toolName": "Read", "toolUseId": format!("c{n}"), "parentToolUseId": task, "text": "{\"file_path\":\"src/b.rs\"}"});
            push("session.delta", n, read);
            push(
                "session.delta",
                n,
                json!({"kind": "tool_result", "toolUseId": format!("c{n}"), "parentToolUseId": task, "text": "fn b() {}"}),
            );
            if n % 9 == 0 {
                let options = json!([{"id": "y", "label": "Allow", "effect": "allow"}]);
                let asked = json!({"askId": format!("s{n}"), "toolName": "Bash", "parentToolUseId": task, "input": "{\"command\":\"rm x\"}", "options": options});
                push("session.permission", n, asked);
                push("session.permission.closed", n, json!({"askId": format!("s{n}"), "outcome": "denied"}));
            }
            if n % 12 != 3 {
                push("session.delta", n, json!({"kind": "tool_result", "toolUseId": task, "text": "Found it."}));
            }
            if n % 6 == 0 {
                push(
                    "session.subagent",
                    n,
                    json!({"task": format!("k-{task}"), "state": "done", "parentToolUseId": task, "summary": "Found it."}),
                );
            }
        }
        if n % 5 == 3 {
            let input =
                json!({"questions": [{"question": "Which colour?", "header": "Colour", "options": [{"label": "Red"}, {"label": "Blue"}]}]});
            let options =
                json!([{"id": "q0:o0", "label": "Red", "effect": "answer"}, {"id": "q0:o1", "label": "Blue", "effect": "answer"}]);
            push(
                "session.permission",
                n,
                json!({"askId": format!("w{n}"), "toolName": "AskUserQuestion", "input": input.to_string(), "options": options}),
            );
            push("session.permission.closed", n, json!({"askId": format!("w{n}"), "outcome": "allowed", "optionId": "q0:o1"}));
        }
        if n % 4 == 1 {
            push("session.notice", n, json!({"text": "the machine slept"}));
        }
        if n % 4 == 2 {
            push("session.plan", n, json!({"text": "# Fix plan\n\n- step one"}));
        }
        push(
            "session.delta",
            n,
            json!({"kind": "tool_use", "toolName": "Edit", "toolUseId": format!("e{n}"), "text": "{\"file_path\":\"src/a.rs\"}"}),
        );
        push(
            "session.delta",
            n,
            json!({"kind": "tool_result", "toolUseId": format!("e{n}"), "text": "done", "patch": [{"path": "src/a.rs", "hunks": [
                {"oldStart": 1, "oldLines": 1, "newStart": 1, "newLines": 1, "lines": ["-a", "+b"]}
            ]}]}),
        );
        for (i, word) in ["All ", "done:\n```sh\nls\n```\n", "now."].into_iter().enumerate() {
            let at = push("session.delta", n, json!({"kind": "text", "text": word, "messageId": format!("z{n}")}));
            if i == 0 {
                replies.push(format!("u{n}:m@{at}@10"));
            }
        }
        if n % 6 == 4 {
            let input = "{\"file_path\":\"src/app.ts\",\"old_string\":\"a\",\"new_string\":\"b\"}";
            let options = json!([{"id": "y", "label": "Allow", "effect": "allow"}]);
            push("session.permission", n, json!({"askId": format!("o{n}"), "toolName": "Edit", "input": input, "options": options}));
            continue;
        }
        let result = match n % 7 {
            3 => json!({"status": "failed", "error": "API down", "durationMs": 3_000}),
            5 => json!({"status": "interrupted", "durationMs": 2_000}),
            6 => json!({"status": "failed"}),
            _ => json!({"status": "completed", "durationMs": 9_000, "waitedMs": 1_000}),
        };
        push("session.done", n, json!({"result": result}));
        if n % 3 != 1 {
            push("session.end", n, json!({"exitCode": 0, "sawResult": true}));
        }
        if n % 2 == 0 {
            let others = if n % 4 == 0 { json!({"shared": true}) } else { json!({"others": files(n + 100)}) };
            let mut changes = json!({"from": "a", "to": "b", "files": files(n), "moved": []});
            changes.as_object_mut().unwrap().extend(others.as_object().unwrap().clone());
            push("session.changes", n, changes);
        }
        if n % 3 != 2 {
            push("session.checkpoint", n, json!({"ref": "c", "anchor": "x"}));
        }
        if n % 4 == 1 {
            let block = replies[n].clone();
            push(
                "session.run",
                n,
                json!({"runId": format!("run{n}"), "block": block, "command": "ls", "state": "exited", "exitCode": 0, "output": "a\n"}),
            );
        }
    }
    events
}

#[test]
fn a_live_event_folds_to_what_a_load_of_the_same_events_draws() {
    let events = thread(12);
    assert!(events.len() >= 200, "{} events", events.len());
    let ids = (0..12).map(|n| format!("u{n}")).chain(["group:tool:u7:b7", "u8:call:b8", "plan:plan:u6", "subagent:t3"].map(str::to_owned));
    let can = Can { forks: true, forks_by_count: false, rewinds_conversation: true, rewinds_by_count: false };
    for (opened, can) in [(HashSet::new(), Can::default()), (ids.collect::<HashSet<String>>(), can)] {
        let listing = Listing::holding(&events);
        let mut live = Transcript { opened: opened.clone(), can, ..Transcript::default() };
        live.listing(listing.clone());
        let mut kinds = HashSet::new();
        for (n, e) in events.iter().enumerate() {
            let before = live.rows.clone();
            let changed = live.push(e.clone());
            let mut load = Transcript { opened: opened.clone(), can, ..Transcript::default() };
            load.listing(listing.clone());
            load.load(events[..=n].to_vec());
            assert_eq!(live.rows, load.rows, "after event {n}");
            assert_eq!(changed, splice(&before, &load.rows), "splice after event {n}");
            kinds.extend(live.rows.iter().map(|r| name(&r.kind)));
            kinds.extend(live.rows.iter().filter(|r| matches!(r.kind, RowKind::Working { waiting: true, .. })).map(|_| "waiting"));
        }
        assert!(live.rows.iter().any(|r| r.kind == RowKind::Stopped));
        assert!(live.rows.iter().any(|r| r.kind == RowKind::Error && r.text == "Session restarted before it finished"));
        for kind in ["permission", "question", "plan", "subagent", "spawn", "changes", "fork", "waiting"] {
            assert!(kinds.contains(kind), "no {kind} row was drawn");
        }
        if can.forks {
            let agents = live.rows.iter().filter_map(|r| match &r.kind {
                RowKind::Agent { rewind, fork, runs, .. } => Some((rewind.is_some(), fork.is_some(), !runs.is_empty())),
                _ => None,
            });
            let (rewinds, forks, runs) = agents.fold((false, false, false), |a, b| (a.0 | b.0, a.1 | b.1, a.2 | b.2));
            assert!(rewinds && forks && runs, "rewind {rewinds}, fork {forks}, runs {runs}");
            assert!(live.rows.iter().any(|r| matches!(&r.kind, RowKind::Person { fork: Some(_), .. })));
        }
    }
}

#[test]
fn a_live_event_folds_only_the_latest_turn() {
    let mut events = thread(900);
    let last = events.iter().rposition(|e| matches!(e, SessionEvent::SessionStart { .. })).unwrap();
    events.truncate(last + 6);
    let delta = base("session.delta", 999_999, json!({"kind": "text", "text": " more", "messageId": "a899", "turnId": "u899"}));
    cheap_push(events, delta);
}

#[test]
fn unfolded_rows_are_the_rows_with_every_fold_and_group_open() {
    let mut t = Transcript::new();
    t.load(thread(8));
    let shut = t.rows.clone();
    let unfolded = t.unfolded();
    assert_eq!(t.rows, shut);
    assert!(t.opened.is_empty());
    let ids: HashSet<String> = unfolded
        .iter()
        .filter_map(|r| match &r.kind {
            RowKind::Fold { .. } => r.turn.clone(),
            RowKind::Group { group, .. } | RowKind::Live { group, .. } => Some(group.clone()),
            RowKind::Plan { .. } => Some(format!("plan:{}", r.key)),
            RowKind::Subagent { id, .. } => Some(id.clone()),
            _ => None,
        })
        .collect();
    assert!(unfolded.len() > shut.len());
    assert_eq!(unfolded, rows(&thread(8), &ids));
}

/// Folds `events`, pushes `delta`, and holds the push under 1% of the load and its rows to a whole fold's.
fn cheap_push(mut events: Vec<SessionEvent>, delta: SessionEvent) {
    let mut t = Transcript::new();
    t.load(events.clone());
    let load = std::mem::take(&mut t.cost);
    t.push(delta.clone());
    assert!(t.cost * 100 < load, "a live event cost {} against a load's {load}", t.cost);
    events.push(delta);
    assert_eq!(t.rows, rows(&events, &HashSet::new()));
}

/// Cuts `events` at every event, reads the newer part, lays the older part under it as a page lands, and holds the
/// rows, the unfolded rows, the turns and a live event after it to a whole read of the thread; most cuts graft.
fn lands_as_a_whole_read(events: &[SessionEvent], can: Can, open: &[&str], live: SessionEvent) {
    let mut grafted = 0;
    for k in 1..events.len() {
        let whole = |events: &[SessionEvent]| {
            let mut t = Transcript { opened: opened(open), ..Transcript::default() };
            _ = t.can(can);
            t.load(events.to_vec());
            t
        };
        let mut all = whole(events);
        let mut t = whole(&events[k..]);
        t.prepend(events[..k].to_vec());
        grafted += t.grafts;
        assert_eq!(t.rows, all.rows, "the rows of a page cut at event {k}");
        assert_eq!(t.unfolded(), all.unfolded(), "the unfolded rows of a page cut at event {k}");
        assert_eq!(t.turn_contexts(), all.turn_contexts(), "the turns of a page cut at event {k}");
        assert_eq!(t.prepend(Vec::new()), None);
        let (a, b) = (t.push(live.clone()), all.push(live.clone()));
        assert_eq!((t.rows.clone(), a), (all.rows.clone(), b), "a live event after a page cut at event {k}");
    }
    assert!(grafted * 2 > events.len(), "only {grafted} of {} cuts kept the held fold", events.len() - 1);
}

#[test]
fn a_page_landing_draws_the_thread_as_a_whole_read_does() {
    let events = thread(30);
    let next = base("session.start", 999_000, json!({"prompt": "again", "turnId": "u30", "at": 99_000_000.0}));
    lands_as_a_whole_read(&events, Can::default(), &[], next.clone());
    let all = Can { forks: true, forks_by_count: true, rewinds_conversation: true, rewinds_by_count: true };
    lands_as_a_whole_read(&events, all, &["u3", "u12", EARLIER, "group:tool:u20:b20", "subagent:t9"], next);
    let mut running = thread(30);
    let last = running.iter().rposition(|e| matches!(e, SessionEvent::SessionStart { .. })).unwrap();
    running.truncate(last + 9);
    let delta = base("session.delta", 999_001, json!({"kind": "text", "text": " more", "messageId": "a29", "turnId": "u29"}));
    lands_as_a_whole_read(&running, Can::default(), &[], delta);
}

#[test]
fn the_docked_prompt_is_read_off_the_latest_turn_alone() {
    let mut events = thread(120);
    let options = json!([{"id": "y", "label": "Allow", "effect": "allow"}]);
    let ask = json!({"askId": "late", "toolName": "Bash", "input": "{\"command\":\"ls\"}", "options": options, "turnId": "u120"});
    events.push(base("session.start", 900_000, json!({"prompt": "again", "turnId": "u120"})));
    events.push(base("session.permission", 900_001, ask));
    let mut t = Transcript::new();
    t.load(events);
    assert_eq!(t.docked_key(), Some("permission:late"));
    assert_eq!(t.docked().map(|d| d.row.key), Some("permission:late".to_owned()));
    let looked = t.looked.get();
    assert!(looked < 10, "two reads of the docked prompt looked at {looked} of {} entries", t.model.entries.len());
}

#[test]
fn a_page_landing_folds_the_page_and_the_turn_it_meets() {
    let events = thread(200);
    let k = events.iter().position(|e| matches!(e, SessionEvent::SessionStart { turn_id: Some(t), .. } if t == "u40")).unwrap() - 7;
    let mut t = Transcript::new();
    t.load(events[k..].to_vec());
    let read = t.take_cost();
    t.prepend(events[..k].to_vec());
    let landed = t.take_cost();
    assert!(landed * 3 < read, "a page of {k} events cost {landed} to land under a thread that cost {read} to read");
}

#[test]
fn a_live_event_stays_cheap_for_an_agent_that_rewinds_by_count() {
    let mut events = thread(900);
    let last = events.iter().rposition(|e| matches!(e, SessionEvent::SessionStart { .. })).unwrap();
    events.truncate(last + 6);
    let mut t = Transcript::new();
    t.can(Can { forks: true, forks_by_count: true, rewinds_conversation: true, rewinds_by_count: true });
    t.load(events);
    let load = std::mem::take(&mut t.cost);
    t.push(base("session.delta", 999_999, json!({"kind": "text", "text": " more", "messageId": "a899", "turnId": "u899"})));
    assert!(t.cost * 100 < load, "a live event cost {} against a load's {load}", t.cost);
}

#[test]
fn a_live_event_stays_cheap_after_an_earlier_turns_run_lands_in_the_latest_turn() {
    let mut events = thread(897);
    let last = events.iter().rposition(|e| matches!(e, SessionEvent::SessionStart { .. })).unwrap();
    events.truncate(last + 6);
    let late = |e: &SessionEvent| matches!(e, SessionEvent::SessionRun { turn_id, .. } if turn_id.as_deref() == Some("u894"));
    assert!(events[last..].iter().any(late));
    let delta = base("session.delta", 999_999, json!({"kind": "text", "text": " more", "messageId": "a896", "turnId": "u896"}));
    cheap_push(events, delta);
}

#[test]
fn a_live_event_stays_cheap_after_an_earlier_line_lands_in_the_latest_turn() {
    let earlier = |pos: u64, text: &str| {
        event(json!({"type": "session.earlier", "workspaceId": "w", "sessionId": "s", "pos": pos, "who": "person", "text": text}))
    };
    let mut events = vec![earlier(100_000, "from before")];
    events.extend(thread(300));
    let last = events.iter().rposition(|e| matches!(e, SessionEvent::SessionStart { .. })).unwrap();
    events.truncate(last + 6);
    events.insert(last + 1, earlier(100_001, "also from before"));
    let delta = base("session.delta", 999_999, json!({"kind": "text", "text": " more", "messageId": "a299", "turnId": "u299"}));
    cheap_push(events, delta);
}

#[test]
fn each_turn_keeps_its_context_figures_and_the_latest_start_names_the_harness() {
    let events = vec![
        base("session.start", 1, json!({"prompt": "a", "agent": "codex"})),
        base("session.context", 2, json!({"context": 40.0, "window": 100.0})),
        base("session.done", 3, json!({"result": {"status": "completed", "tokens": {"input": 1, "output": 2, "context": 50.0}}})),
        base("session.end", 4, json!({"exitCode": 0, "sawResult": true})),
        base("session.start", 5, json!({"prompt": "b", "turnId": "u2"})),
        base("session.context", 6, json!({"context": 70.0, "turnId": "u2"})),
    ];
    let mut t = Transcript::new();
    t.load(events);
    let first = TurnContext {
        counted: true,
        tokens_context: Some(50.0),
        held_context: Some(40.0),
        held_window: Some(100.0),
        ..TurnContext::default()
    };
    let second = TurnContext { running: true, held_context: Some(70.0), ..TurnContext::default() };
    assert_eq!(t.turn_contexts(), vec![first, second]);
    assert_eq!(t.harness().as_deref(), Some("codex"));
}

#[test]
fn a_preview_is_the_first_line_worth_reading_cut_at_84_characters() {
    assert_eq!(preview("\n  \n```\n  first   line  \nsecond").as_deref(), Some("first line"));
    assert_eq!(preview("```\n\n```").as_deref(), Some("2 lines"));
    assert_eq!(preview("```"), None);
    assert_eq!(preview(""), None);
    let exact = "x".repeat(84);
    assert_eq!(preview(&exact), Some(exact.clone()));
    let words = format!("{} tail of the line", "y".repeat(82));
    assert_eq!(preview(&words), Some(format!("{}…", "y".repeat(82))));
    let long = "z".repeat(100_000);
    assert_eq!(preview(&long), Some(format!("{}…", "z".repeat(83))));
}

#[test]
fn a_streamed_delta_changes_only_its_reply_row() {
    let mut t = Transcript::new();
    t.load(thread(3));
    t.push(base("session.start", 900, json!({"prompt": "next", "turnId": "u9"})));
    t.push(base("session.delta", 901, json!({"kind": "text", "text": "Hel", "turnId": "u9"})));
    let reply = t.rows.iter().position(|r| r.key == "u9:m@901").unwrap();
    let changed = t.push(base("session.delta", 902, json!({"kind": "text", "text": "lo", "turnId": "u9"})));
    assert_eq!(changed, Some(Splice { start: reply, removed: 1, inserted: 1 }));
    assert_eq!(t.rows[reply].text, "Hello");
}
#[test]
fn a_turn_the_person_opened_stays_open_when_the_next_starts() {
    let mut t = Transcript::new();
    t.load(turn(true));
    t.toggle("u1");
    assert!(t.rows().iter().any(|r| matches!(r.kind, RowKind::Fold { open: true })), "the fold did not open");
    t.push(base("session.start", 12, json!({"prompt": "more", "turnId": "u2"})));
    t.push(base("session.delta", 13, json!({"kind": "text", "text": "Ok.", "turnId": "u2"})));
    assert!(t.rows().iter().any(|r| matches!(r.kind, RowKind::Fold { open: true })), "the next turn shut the opened fold");
    t.push(base("session.done", 14, json!({"result": {"status": "completed"}, "turnId": "u2"})));
    let folds: Vec<bool> = t
        .rows()
        .iter()
        .filter_map(|r| match r.kind {
            RowKind::Fold { open } => Some(open),
            _ => None,
        })
        .collect();
    assert_eq!(folds.first(), Some(&true), "the next turn's settling shut the opened fold: {folds:?}");
}

#[test]
fn a_retry_stands_until_the_agent_writes_into_its_turn() {
    let retrying = |events: &[SessionEvent]| {
        rows(events, &HashSet::new()).iter().find_map(|r| match &r.kind {
            RowKind::Working { retry, .. } => Some(retry.clone()),
            _ => None,
        })
    };
    let mut events = vec![
        base("session.start", 1, json!({"prompt": "go"})),
        base("session.retry", 2, json!({"attempt": 2, "of": 10, "why": "overloaded"})),
    ];
    assert_eq!(retrying(&events), Some(Some("Overloaded, trying again (2 of 10)".to_owned())));
    events.push(base("session.retry", 3, json!({"attempt": 3})));
    assert_eq!(retrying(&events), Some(Some("Trying again (3)".to_owned())));
    events.push(base("session.notice", 4, json!({"text": "Machine woke up"})));
    assert_eq!(retrying(&events), Some(Some("Trying again (3)".to_owned())), "a notice is no sign the call got through");
    events.push(base("session.delta", 5, json!({"kind": "text", "text": "Here"})));
    assert_eq!(retrying(&events), Some(None), "the reply left the retry standing");
    let mut t = Transcript::new();
    t.load(events[..2].to_vec());
    t.push(events[2].clone());
    assert!(t.rows().iter().any(|r| matches!(&r.kind, RowKind::Working { retry: Some(line), .. } if line == "Trying again (3)")));
    t.push(events[4].clone());
    assert!(t.rows().iter().any(|r| matches!(r.kind, RowKind::Working { retry: None, .. })), "a live reply left the retry standing");
}
