// SPDX-License-Identifier: AGPL-3.0-only

use super::*;

fn prompt(p: &[(&str, &str, &str)], outcome: Option<&str>, picked: Option<&str>) -> Prompt {
    Prompt {
        ask: "k".into(),
        session: "s".into(),
        tool: "Bash".into(),
        input: "{}".into(),
        detail: None,
        options: p.iter().map(|(a, b, c)| ((*a).into(), (*b).into(), (*c).into(), None)).collect(),
        outcome: outcome.map(str::to_owned),
        picked: picked.map(str::to_owned),
    }
}

#[test]
fn a_permission_reads_its_lead_and_waits_until_answered() {
    let options = json!([{"id": "y", "label": "Allow", "effect": "allow"}, {"id": "n", "label": "Deny", "effect": "deny"}]);
    let mut events = vec![
        base("session.start", 1, json!({"prompt": "go"})),
        base(
            "session.delta",
            2,
            json!({"kind": "tool_use", "toolName": "Bash", "toolUseId": "a", "text": "{\"command\":\"cargo test --all\",\"timeout\":5}"}),
        ),
        base(
            "session.permission",
            3,
            json!({"askId": "k", "toolName": "Bash", "toolUseId": "a", "input": "{\"command\":\"cargo test --all\",\"timeout\":5}", "options": options}),
        ),
    ];
    let waiting = rows(&events, &HashSet::new());
    assert_eq!(find(&waiting, "working").kind, RowKind::Working { since: None, waiting: true, retry: None, wait: None });
    assert_eq!(find(&waiting, "working").text, "Waiting for you");
    assert!(!waiting.iter().any(|r| r.kind == RowKind::Thinking));
    let row = find(&waiting, "permission");
    assert_eq!((row.key.as_str(), row.text.as_str()), ("permission:k", "Run a command: cargo test --all"));
    assert_eq!(
        row.kind,
        RowKind::Permission {
            ask: "k".into(),
            session: "s".into(),
            asker: None,
            glyph: Glyph::Terminal,
            lead: "Run a command:".into(),
            code: Some("cargo test --all".into()),
            fields: vec![("timeout".into(), "5".into())],
            file: None,
            options: vec![
                PromptOption { id: "y".into(), label: "Allow".into(), description: None, effect: "allow".into(), mode: None },
                PromptOption { id: "n".into(), label: "Deny".into(), description: None, effect: "deny".into(), mode: None },
            ],
            outcome: None,
            open: true,
        }
    );
    events.push(base("session.permission.closed", 4, json!({"askId": "k", "outcome": "allowed", "optionId": "y"})));
    let answered = rows(&events, &HashSet::new());
    assert!(matches!(&find(&answered, "permission").kind, RowKind::Permission { outcome: Some(o), open: false, .. } if o == "Allowed"));
    assert_eq!(find(&answered, "working").kind, RowKind::Working { since: None, waiting: false, retry: None, wait: None });
}

#[test]
fn outcome_words_read_as_the_web_app_writes_them() {
    let consent = [("y", "Allow", "allow"), ("m", "Allow, then auto", "mode"), ("n", "Deny", "deny")];
    let line = |outcome, picked| outcome_line(&prompt(&consent, Some(outcome), picked));
    assert_eq!(line("allowed", Some("y")).as_deref(), Some("Allowed"));
    assert_eq!(line("allowed", Some("m")).as_deref(), Some("Allowed: Allow, then auto"));
    assert_eq!(line("denied", Some("n")).as_deref(), Some("Denied"));
    assert_eq!(line("unanswered", None).as_deref(), Some("Nobody answered; denied"));
    assert_eq!(line("cancelled", None).as_deref(), Some("Cancelled with the turn"));
    assert_eq!(outcome_line(&prompt(&consent, None, None)), None);
    let answers = [("q0:o0", "Red", "answer"), ("q1:o0", "Big", "answer")];
    let answered = |picked| outcome_line(&prompt(&answers, Some("allowed"), Some(picked)));
    assert_eq!(answered("q0:o0|q1:o0").as_deref(), Some("You answered: Red, Big"));
    assert_eq!(answered("q0:other:Light%20blue").as_deref(), Some("You answered: Light blue"));
}

#[test]
fn a_prompt_says_its_kind_of_call_in_words() {
    let write = prompt_words("Write", "{\"file_path\":\"src/app.ts\",\"content\":\"hello\"}", None);
    assert_eq!((write.says.as_str(), write.body.as_deref(), write.rest.len()), ("Write app.ts in src (5 B)", Some("hello"), 0));
    let edit = prompt_words("Edit", "{\"file_path\":\"/w/src/app.ts\",\"edits\":[{},{}]}", None);
    assert_eq!(edit.says, "Edit app.ts in src (2 places)");
    assert_eq!(prompt_words("WebFetch", "{\"url\":\"https://a.b\"}", None).code.as_deref(), Some("https://a.b"));
    assert_eq!(prompt_words("Frob", "{\"x\":\"a  b\"}", Some("why")).says, "Permission for Frob: why");
    assert_eq!(prompt_words("Frob", "{\"x\":\"a  b\"}", None).rest, vec![("x".to_owned(), "a b".to_owned())]);
    assert_eq!(prompt_words("mcp__wsp__run", "{}", None).says, "Use wsp's run");
    assert_eq!(prompt_glyph("Edit", "{}"), Glyph::SquarePen);
    assert_eq!(prompt_glyph("WebSearch", "{}"), Glyph::Globe);
    assert_eq!(prompt_glyph("Frob", "{}"), Glyph::Wrench);
}

#[test]
fn a_question_lists_its_choices_and_the_answer() {
    let input = json!({"questions": [{"question": "Which colour?", "header": "Colour", "multiSelect": true, "options": [
        {"label": "Red", "description": "warm"}, {"label": "Blue"}
    ]}]});
    let options = json!([{"id": "q0:o0", "label": "Red", "effect": "answer"}, {"id": "q0:o1", "label": "Blue", "effect": "answer"}]);
    let mut events = vec![
        base("session.start", 1, json!({"prompt": "go"})),
        base("session.permission", 2, json!({"askId": "w", "toolName": "AskUserQuestion", "input": input.to_string(), "options": options})),
    ];
    let open = rows(&events, &HashSet::new());
    let row = find(&open, "question");
    assert_eq!(row.text, "Which colour?");
    let asked = Asked {
        header: Some("Colour".into()),
        question: "Which colour?".into(),
        options: vec![
            PromptOption { id: "q0:o0".into(), label: "Red".into(), description: Some("warm".into()), effect: "answer".into(), mode: None },
            PromptOption { id: "q0:o1".into(), label: "Blue".into(), description: None, effect: "answer".into(), mode: None },
        ],
        multiple: true,
    };
    assert_eq!(row.kind, RowKind::Question { ask: "w".into(), session: "s".into(), asker: None, questions: vec![asked], outcome: None });
    assert!(matches!(find(&open, "working").kind, RowKind::Working { waiting: true, .. }));
    events.push(base("session.permission.closed", 3, json!({"askId": "w", "outcome": "allowed", "optionId": "q0:o1"})));
    let closed = rows(&events, &HashSet::new());
    assert!(matches!(&find(&closed, "question").kind, RowKind::Question { outcome: Some(o), .. } if o == "You answered: Blue"));
}

#[test]
fn a_plan_stays_through_the_fold_and_opens_when_long() {
    let plan = "# Fix the build\n\n## Summary\n\nFirst.\n\n- one\n- two";
    let events = vec![
        base("session.start", 1, json!({"prompt": "plan it"})),
        base("session.delta", 2, json!({"kind": "tool_use", "toolName": "Read", "toolUseId": "a", "text": "{\"file_path\":\"a\"}"})),
        base("session.delta", 3, json!({"kind": "tool_result", "toolUseId": "a", "text": "x"})),
        base("session.plan", 4, json!({"text": plan})),
        base("session.delta", 5, json!({"kind": "text", "text": "Here it is."})),
        base("session.done", 6, json!({"result": {"status": "completed", "durationMs": 2_000}})),
        base("session.end", 7, json!({"exitCode": 0, "sawResult": true})),
    ];
    let mut t = Transcript::new();
    t.load(events);
    assert_eq!(shown(&t.rows), [("person", "plan it"), ("fold", "Worked for 2.0s"), ("plan", plan), ("agent", "Here it is.")]);
    assert_eq!(t.rows[2].kind, RowKind::Plan { title: "Fix the build".into(), open: false });
    t.toggle("plan:plan:u1");
    assert_eq!(t.rows[2].kind, RowKind::Plan { title: "Fix the build".into(), open: true });
    assert_eq!(plan_shown(plan, false), "First.\n\n- one\n- two");
    assert!(!plan_folds(plan));
    let long: String = (0..25).map(|n| format!("- step {n}\n")).collect();
    assert!(plan_folds(&long));
    let preview = plan_shown(&long, false);
    assert!(preview.starts_with("- step 0\n") && preview.ends_with("- step 9\n\n..."), "{preview}");
    assert_eq!(plan_shown(&long, true), long.trim_end());
    assert_eq!(plan_title("no heading"), None);
}
