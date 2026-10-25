// SPDX-License-Identifier: AGPL-3.0-only

use super::*;

fn subagent(result: Option<serde_json::Value>) -> Vec<SessionEvent> {
    let task = "{\"description\":\"Survey the code\\nmore\",\"prompt\":\"look\"}";
    let mut events = vec![
        base("session.start", 1, json!({"prompt": "go", "at": 1_000.0})),
        base("session.delta", 2, json!({"kind": "tool_use", "toolName": "Task", "toolUseId": "t", "text": task, "at": 2_000.0})),
        base("session.delta", 3, json!({"kind": "text", "text": "Reading.", "parentToolUseId": "t"})),
        base(
            "session.delta",
            4,
            json!({"kind": "tool_use", "toolName": "Read", "toolUseId": "c", "parentToolUseId": "t", "text": "{\"file_path\":\"src/b.rs\"}"}),
        ),
        base("session.delta", 5, json!({"kind": "tool_result", "toolUseId": "c", "parentToolUseId": "t", "text": "fn b() {}\nmore"})),
    ];
    events.extend(result.map(|r| base("session.delta", 6, r)));
    events
}

#[test]
fn a_subagents_command_skips_a_first_line_the_protocol_reads_as_blank() {
    let mut events = subagent(None);
    let call = json!({"kind": "tool_use", "toolName": "Bash", "toolUseId": "d", "parentToolUseId": "t", "text": "{\"command\":\"\\ufeff\\nls -la\"}"});
    events.push(base("session.delta", 6, call));
    let rows = rows(&events, &opened(&["subagent:t"]));
    let RowKind::Subagent { lines, .. } = &find(&rows, "subagent").kind else { panic!("no subagent row: {:?}", shown(&rows)) };
    assert_eq!(lines.last().map(|l| l.text.as_str()), Some("$ ls -la"));
}

#[test]
fn a_subagent_folds_its_lines_under_its_title() {
    let done = json!({"kind": "tool_result", "toolUseId": "t", "text": "Found it.", "at": 67_000.0});
    let rows = rows(&subagent(Some(done)), &opened(&["subagent:t"]));
    let row = find(&rows, "subagent");
    let line = |text: &str, mono, detail: Option<&str>| SubLine { text: text.into(), mono, detail: detail.map(str::to_owned) };
    assert_eq!(
        row.kind,
        RowKind::Subagent {
            id: "subagent:t".into(),
            title: "Survey the code".into(),
            state: Some("Done 1m 5s".into()),
            open: true,
            lines: vec![line("Reading.", false, None), line("read src/b.rs", true, Some("fn b() {}")), line("Found it.", false, None)],
        }
    );
    assert_eq!(row.key, "subagent:t");
    assert!(!rows.iter().any(|r| name(&r.kind) == "group"), "{:?}", shown(&rows));
    let failed = json!({"kind": "tool_result", "toolUseId": "t", "text": "boom", "isError": true, "at": 14_000.0});
    let failed = super::tests::rows(&subagent(Some(failed)), &HashSet::new());
    assert!(matches!(&find(&failed, "subagent").kind, RowKind::Subagent { state: Some(s), open: false, .. } if s == "Failed 12s"));
    let mut cut = subagent(None);
    cut.push(base("session.done", 7, json!({"result": {"status": "interrupted"}, "at": 12_000.0})));
    let stopped = super::tests::rows(&cut, &HashSet::new());
    assert!(matches!(&find(&stopped, "subagent").kind, RowKind::Subagent { state: Some(s), .. } if s == "Stopped 10s"));
    let running = super::tests::rows(&subagent(None), &HashSet::new());
    assert!(matches!(&find(&running, "subagent").kind, RowKind::Subagent { state: None, .. }));
}

#[test]
fn a_question_opens_its_subagents_fold_and_the_persons_shut_stands_until_the_next() {
    let fold = |t: &Transcript| matches!(find(&t.rows, "subagent").kind, RowKind::Subagent { open: true, .. });
    let ask = |id: &str, pos| {
        let options = json!([{"id": "y", "label": "Allow", "effect": "allow"}]);
        base("session.permission", pos, json!({"askId": id, "toolName": "Bash", "parentToolUseId": "t", "input": "{}", "options": options}))
    };
    let mut t = Transcript::new();
    t.load(subagent(None));
    assert!(!fold(&t));
    t.push(ask("s1", 6));
    assert!(fold(&t), "a question opens the fold");
    t.toggle("subagent:t");
    assert!(!fold(&t), "the person's shut stands while the question waits");
    t.push(ask("s2", 7));
    assert!(fold(&t), "the next question opens it again");
    t.toggle("subagent:t");
    t.toggle("subagent:t");
    assert!(fold(&t), "a second press opens it again");
}

#[test]
fn a_subagent_prompt_opens_its_fold_under_its_name() {
    let mut events = subagent(None);
    let options = json!([{"id": "y", "label": "Allow", "effect": "allow"}]);
    let asked = json!({"askId": "s1", "toolName": "Bash", "parentToolUseId": "t", "toolUseId": "x", "input": "{\"command\":\"rm x\"}", "options": options});
    events.push(base("session.permission", 6, asked));
    let open = rows(&events, &HashSet::new());
    let at = open.iter().position(|r| name(&r.kind) == "subagent").unwrap();
    assert!(matches!(open[at].kind, RowKind::Subagent { open: true, .. }));
    assert!(matches!(&open[at + 1].kind, RowKind::Permission { asker: Some(a), open: true, .. } if a == "Survey the code asks"));
    assert_eq!(open[at + 1].key, "permission:s1");
    assert!(!matches!(find(&open, "working").kind, RowKind::Working { waiting: true, .. }));
    events.push(base("session.permission.closed", 7, json!({"askId": "s1", "outcome": "denied"})));
    let shut = rows(&events, &HashSet::new());
    assert!(matches!(find(&shut, "subagent").kind, RowKind::Subagent { open: false, .. }));
    assert!(!shut.iter().any(|r| name(&r.kind) == "permission"));
}

#[test]
fn calls_that_started_children_draw_as_one_spawn_row() {
    let events = vec![
        base("session.start", 1, json!({"prompt": "go"})),
        base("session.delta", 2, json!({"kind": "tool_use", "toolName": "Task", "toolUseId": "t", "text": "{\"description\":\"Survey\"}"})),
        base("session.subagent", 3, json!({"task": "k1", "state": "running", "parentToolUseId": "t", "title": "Survey it"})),
        base("session.delta", 4, json!({"kind": "tool_result", "toolUseId": "t", "text": "Found it."})),
        base("session.subagent", 5, json!({"task": "k1", "state": "failed", "parentToolUseId": "t", "summary": "\nout of room\nmore"})),
        base(
            "session.delta",
            6,
            json!({"kind": "tool_use", "toolName": "mcp__wsp__run", "toolUseId": "r", "text": "{\"title\":\"Fix docs\"}"}),
        ),
        base("session.delta", 7, json!({"kind": "tool_result", "toolUseId": "r", "text": "{\"threadId\":\"th1\"}"})),
        base("session.delta", 8, json!({"kind": "text", "text": "Started."})),
    ];
    assert!(!shown(&rows(&events, &HashSet::new())).iter().any(|(kind, _)| *kind == "spawn"), "a child the listing does not hold");
    let listing = Listing::holding(&events);
    let mut t = Transcript::new();
    t.listing(listing.clone());
    t.load(events.clone());
    let rows = t.rows;
    assert_eq!(
        shown(&rows),
        [("person", "go"), ("working", "Working"), ("spawn", ""), ("agent", "Started."), ("thinking", "Thinking"), ("threads", "Threads")]
    );
    let RowKind::Spawn { children } = &rows[2].kind else { unreachable!() };
    let survey = Child {
        thread: None,
        title: "Survey it".into(),
        note: Some("out of room".into()),
        status: Some("failed".into()),
        subagent: true,
        settled: false,
        call: Some("t".into()),
        listed: None,
        workspace: None,
    };
    let docs = Child {
        thread: Some("th1".into()),
        title: "Fix docs".into(),
        note: None,
        status: None,
        subagent: false,
        settled: false,
        call: None,
        listed: listing.child("th1").cloned(),
        workspace: None,
    };
    assert_eq!(children, &vec![survey, docs]);
    assert_eq!(rows[2].key, "spawn:u1:call:t");
    let mut named = subagent(None);
    named.push(base("session.subagent", 6, json!({"task": "k1", "state": "running", "parentToolUseId": "t", "title": "Survey"})));
    let mut held = Transcript::new();
    held.listing(Listing::holding(&named));
    held.load(named);
    let named = held.rows;
    assert!(named.iter().any(|r| r.key == "spawn:subagent:t"), "{:?}", shown(&named));
    assert!(!named.iter().any(|r| name(&r.kind) == "subagent"));
    assert_eq!(named_thread("thread th2 on wsp in /w"), Some("th2".into()));
    assert_eq!(named_thread("[{\"type\":\"text\",\"text\":\"{\\\"turn\\\":{\\\"threadId\\\":\\\"th3\\\"}}\"}]"), Some("th3".into()));
    assert!(starts_thread("cd /w && ~/bin/wsp run --project x 'go'"));
    assert!(starts_thread("bash -lc 'wsp fork t1'"));
    assert!(!starts_thread("echo 'wsp run'"));
}
