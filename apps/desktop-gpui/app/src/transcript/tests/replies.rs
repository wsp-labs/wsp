// SPDX-License-Identifier: AGPL-3.0-only

use super::*;

fn changes(extra: serde_json::Value) -> Vec<Row> {
    let mut v = json!({"from": "a", "to": "b", "moved": [], "files": [
        {"path": "src/a.rs", "kind": "modified", "additions": 3, "deletions": 1},
        {"path": "b.rs", "kind": "added", "additions": 9, "deletions": 0}
    ]});
    v.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
    let mut events = turn(true);
    events.push(base("session.changes", 11, v));
    rows(&events, &HashSet::new())
}

#[test]
fn a_turns_changes_hang_under_its_last_reply() {
    let rows = changes(json!({}));
    assert_eq!(shown(&rows), [("person", "fix it"), ("fold", "Worked for 12s"), ("agent", "Done here."), ("changes", "2 changed files")]);
    let files =
        vec![ChangedFile { path: "src/a.rs".into(), added: 3, removed: 1 }, ChangedFile { path: "b.rs".into(), added: 9, removed: 0 }];
    let range = ("a".into(), "b".into());
    assert_eq!(rows[3].kind, RowKind::ChangedFiles { label: "2 changed files".into(), files, others: Vec::new(), range });
    assert_eq!((rows[3].key.as_str(), rows[3].turn.as_deref()), ("changes:u1", Some("u1")));
    assert_eq!(changes(json!({"shared": true}))[3].text, "2 changed files in this folder");
    let others = json!({"files": [], "others": [{"path": "c.rs", "kind": "modified", "additions": 1, "deletions": 1}]});
    let lone = changes(others);
    assert_eq!(lone[3].text, "No files from this thread's edits");
    assert!(matches!(&lone[3].kind, RowKind::ChangedFiles { others, .. } if others.len() == 1));
    let one = json!({"files": [{"path": "a", "kind": "modified", "additions": 1, "deletions": 0}]});
    assert_eq!(changes(one)[3].text, "1 changed file");
    assert_eq!(changes(json!({"files": []})).len(), 3);
}

fn forked() -> Vec<SessionEvent> {
    let mut events = turn(true);
    for e in &mut events {
        let mut v = serde_json::to_value(&*e).unwrap();
        v["copiedFrom"] = json!("th-source");
        *e = event(v);
    }
    events.push(base("session.start", 20, json!({"prompt": "and now", "turnId": "u2"})));
    events
}

#[test]
fn a_fork_says_where_it_came_from_before_its_own_first_message() {
    let mut t = Transcript::new();
    t.load(forked()[..10].to_vec());
    assert!(!shown(&t.rows).iter().any(|(kind, _)| *kind == "fork"), "the rule stands before the listing names its thread");
    let source = ForkSource { title: "Fix the build".into(), at: None };
    t.listing(Listing { source: Some(source), ..Listing::default() });
    assert_eq!(shown(&t.rows).last(), Some(&("fork", "Forked from Fix the build")));
    t.push(forked()[10].clone());
    let at = t.rows.iter().position(|r| name(&r.kind) == "fork").unwrap();
    assert_eq!(shown(&t.rows[at..at + 2]), [("fork", "Forked from Fix the build"), ("person", "and now")]);
    assert_eq!(t.rows[at].kind, RowKind::Fork { source: "Fix the build".into(), open: None });
    t.forked_from(Some("Fix the build".into()));
    assert_eq!(t.rows[at].text, "Forked from Fix the build");
    assert!(!rows(&turn(true), &HashSet::new()).iter().any(|r| name(&r.kind) == "fork"));
}

fn two_turns(running: bool) -> Vec<SessionEvent> {
    let mut events = turn(true);
    events.push(base("session.checkpoint", 11, json!({"ref": "c1", "anchor": "x"})));
    events.push(base("session.start", 12, json!({"prompt": "more", "turnId": "u2"})));
    events.push(base("session.delta", 13, json!({"kind": "text", "text": "Ok.", "turnId": "u2"})));
    if !running {
        events.push(base("session.done", 14, json!({"result": {"status": "completed"}, "turnId": "u2"})));
        events.push(base("session.end", 15, json!({"exitCode": 0, "sawResult": true, "turnId": "u2"})));
    }
    events
}

#[test]
fn rewind_and_fork_stand_on_earlier_replies() {
    let can = Can { forks: true, ..Can::default() };
    let load = |events: Vec<SessionEvent>, can| {
        let mut t = Transcript { can, ..Transcript::default() };
        t.load(events);
        t.rows
    };
    let agent_of = |rows: &[Row], text: &str| rows.iter().find(|r| r.text == text).map(|r| r.kind.clone()).unwrap();
    let settled = load(two_turns(false), can);
    assert!(matches!(agent_of(&settled, "Done here."), RowKind::Agent { rewind: Some(r), fork: Some(f), .. } if r == "u1" && f == "u1"));
    assert!(matches!(agent_of(&settled, "Ok."), RowKind::Agent { rewind: None, fork: None, .. }));
    assert!(matches!(agent_of(&settled, "fix it"), RowKind::Person { fork: Some(f), .. } if f.is_empty()));
    assert!(matches!(agent_of(&settled, "more"), RowKind::Person { fork: Some(f), .. } if f == "u1"));
    let working = load(two_turns(true), can);
    assert!(matches!(agent_of(&working, "Done here."), RowKind::Agent { rewind: Some(_), fork: Some(_), .. }));
    assert!(matches!(agent_of(&working, "more"), RowKind::Person { fork: None, .. }));
    let cannot = load(two_turns(false), Can::default());
    assert!(matches!(agent_of(&cannot, "Done here."), RowKind::Agent { rewind: Some(_), fork: None, .. }));
    let mut live = Transcript { can, ..Transcript::default() };
    for e in two_turns(false) {
        live.push(e);
    }
    assert_eq!(live.rows, settled);
}

#[test]
fn a_reply_carries_its_block_runs() {
    let text = "Try:\n\n```sh\nls\n```\n\nthen\n\n~~~bash\npwd\n~~~\n";
    let events = vec![
        base("session.start", 1, json!({"prompt": "go"})),
        base("session.delta", 2, json!({"kind": "text", "text": text})),
        base("session.done", 3, json!({"result": {"status": "completed"}})),
        base("session.end", 4, json!({"exitCode": 0, "sawResult": true})),
        base("session.run", 5, json!({"runId": "r1", "block": "u1:m@2@6", "command": "ls", "state": "running"})),
        base(
            "session.run",
            6,
            json!({"runId": "r2", "block": "u1:m@2@28", "command": "pwd", "state": "exited", "exitCode": 1, "output": "/w"}),
        ),
        base("session.run", 7, json!({"runId": "r3", "block": "u1:m@2@6", "command": "ls", "state": "exited", "signal": 9})),
    ];
    let rows = rows(&events, &HashSet::new());
    let RowKind::Agent { runs, .. } = &find(&rows, "agent").kind else { unreachable!() };
    let run = |id: &str, block, command: &str, state: &str, output: Option<&str>, exit_code, signal: Option<&str>| Run {
        run_id: id.into(),
        block,
        command: command.into(),
        state: state.into(),
        output: output.map(str::to_owned),
        exit_code,
        signal: signal.map(str::to_owned),
        pty_id: None,
    };
    assert_eq!(runs, &vec![run("r3", 0, "ls", "exited", None, None, Some("9")), run("r2", 1, "pwd", "exited", Some("/w"), Some(1), None)]);
    assert_eq!((runs[0].words(), runs[1].words()), ("Stopped by signal 9".into(), "Exited 1".into()));
    assert_eq!(block_at("é\n```\nx\n```", 2), Some(0));
    assert_eq!(block_at("text", 0), None);
}
