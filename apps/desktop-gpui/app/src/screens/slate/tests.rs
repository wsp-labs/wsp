// SPDX-License-Identifier: AGPL-3.0-only
use super::acts::copied;
use super::*;

#[gpui_kit::test]
fn first_showings_are_kept_for_the_last_hundred_threads(cx: &mut gpui_kit::TestAppContext) {
    cx.update(|cx| {
        assert!(first_showing("t0", cx));
        assert!(!first_showing("t0", cx), "a thread shown here opens once");
        for i in 1..=KEPT_THREADS {
            assert!(first_showing(&format!("t{i}"), cx));
        }
        assert_eq!(cx.global::<ShownHere>().0.len(), KEPT_THREADS);
        assert!(first_showing("t0", cx), "the oldest went past the hundredth");
    });
}

#[gpui_kit::test]
fn kept_slates_are_the_last_hundred_threads(cx: &mut gpui_kit::TestAppContext) {
    cx.update(|cx| {
        for i in 0..=KEPT_THREADS {
            let mut s = slate(json!({ "root": { "type": "column" } }));
            s.folds.insert("sec".into(), true);
            keep(&format!("t{i}"), &mut s, cx);
        }
        let kept = kept_threads(cx);
        assert_eq!(kept.len(), KEPT_THREADS);
        assert_eq!(kept.first().map(String::as_str), Some("t1"), "the oldest went past the hundredth");
    });
}

fn slate(pieces: Value) -> Slate {
    let record = json!({ "threadId": "t", "version": 1, "document": { "schema": model::SCHEMA, "root": "root", "pieces": pieces } });
    Slate::new(Record::read(&record).expect("a record"))
}

#[test]
fn a_hidden_piece_asks_its_when_and_nothing_else() {
    let mut s = slate(json!({
        "root": { "type": "column", "children": ["shown", "hidden", "fold"] },
        "shown": { "type": "text", "props": { "value": { "bind": "$a + 1" } } },
        "hidden": { "type": "column", "when": "$show", "children": ["clock"] },
        "clock": { "type": "timeline", "props": { "items": { "bind": "$spans" } } },
        "fold": { "type": "section", "props": { "title": "F", "collapsible": true, "open": false }, "children": ["inside"] },
        "inside": { "type": "text", "props": { "value": { "bind": "$b + 1" } } },
    }));
    let asked = s.formulas();
    assert!(asked.contains(&"($show)".to_owned()) && asked.contains(&"($a + 1)".to_owned()), "{asked:?}");
    assert!(!asked.iter().any(|f| f == "time.now" || f.contains("$spans")), "a hidden timeline reads nothing: {asked:?}");
    assert!(!asked.contains(&"($b + 1)".to_owned()), "a folded section's body asks nothing: {asked:?}");
    s.resolved.insert("($show)".into(), json!(true));
    s.folds.insert("fold".into(), true);
    let asked = s.formulas();
    assert!(asked.contains(&"time.now".to_owned()) && asked.contains(&"($b + 1)".to_owned()), "shown and opened: {asked:?}");
}

#[test]
fn a_copy_writes_what_the_web_writes() {
    assert_eq!(copied(Some(json!("plain"))), "plain");
    assert_eq!(copied(Some(Value::Null)), "\"\"");
    assert_eq!(copied(None), "\"\"");
    assert_eq!(copied(Some(json!({"a": [1, 2]}))), r#"{"a":[1,2]}"#);
}

#[test]
fn a_batch_keeps_a_pieces_formulas_together() {
    let groups = vec![vec!["a".to_owned(); 150], vec!["b".to_owned(); 100], vec!["c".to_owned(); 30]];
    let batches = batched(groups, 200);
    assert_eq!(batches.iter().map(Vec::len).collect::<Vec<_>>(), [150, 130]);
    assert!(batches[1].iter().all(|f| f != "a"));
    assert_eq!(batched(vec![vec!["x".to_owned(); 450]], 200).iter().map(Vec::len).collect::<Vec<_>>(), [200, 200, 50]);
}

#[test]
fn each_push_moves_the_sources_the_web_folds_it_into() {
    let moved = |frame: Value| match moved_by(&frame, "t") {
        Some(Change::Moved { sources, workspace }) => Some((sources, workspace)),
        _ => None,
    };
    let w = Some("w".to_owned());
    assert_eq!(moved(json!({"type": "workspace.status", "status": {"id": "w"}})), Some((vec!["git", "pr"], w.clone())));
    assert_eq!(moved(json!({"type": "workspace.cost", "workspaceId": "w"})), Some((vec!["cost"], w)));
    assert_eq!(moved(json!({"type": "usage.account", "key": "k"})), Some((vec!["usage"], None)));
    assert_eq!(moved(json!({"type": "session.done", "threadId": "t"})), Some((vec!["thread"], None)));
    assert_eq!(moved(json!({"type": "thread.head", "threadId": "t"})), Some((vec!["thread"], None)));
    assert_eq!(moved(json!({"type": "session.row", "threadId": "t"})), Some((vec!["thread"], None)));
    assert_eq!(moved(json!({"type": "session.delta", "threadId": "t"})), None, "a streaming reply moves no thread field");
    assert_eq!(moved(json!({"type": "session.done", "threadId": "other"})), None);
    assert_eq!(moved(json!({"type": "slate.values", "threadId": "t"})), None, "the slate's own pushes are read apart");
    assert_eq!(moved(json!({"type": "workspace.status", "status": {}})), None);
}

#[test]
fn no_host_to_answer_reads_as_the_offline_sentence() {
    for gone in [
        Failure::new("no wsp host is serving /tmp/x/state/state.json; start one with wsp up --state /tmp/x/state/state.json"),
        Failure::new("the host closed the connection"),
        Failure::new("no host answered for /tmp/x/state.json within 20s"),
        Failure::of_kind("the host at ws://127.0.0.1:4 did not answer: Connection refused", "unreachable"),
    ] {
        assert_eq!(refusal(&gone), words::OFFLINE, "{}", gone.message);
    }
    let refused = Failure::new("Z800 thread-not-yours: a thread writes its own slate");
    assert_eq!(refusal(&refused), refused.message, "the host's own refusals stay its own");
    assert_eq!(refusal(&Failure::new("the host closed the connection twice")), "the host closed the connection twice");
}
