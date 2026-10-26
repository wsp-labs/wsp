// SPDX-License-Identifier: AGPL-3.0-only
//! Tests of src/slate_sources.rs and src/slate_sources/*.rs. Spec: spec/slate_sources.md. Every expected value is
//! what the TS printed under node (probe pmisc.ts, and the same inputs with more cases).

use std::cell::Cell;

use futures::future::LocalBoxFuture;
use futures::FutureExt;
use serde_json::{json, Value};

use wsp_slates::slate_sources::context::{json as to_json, AccountRow, Checkout, PullRequestSeen, SessionView, TurnResult};
use wsp_slates::slate_sources::{
    host_slate_sources, path_segments, resolve_in, view_sources, walk, Seg, SlateSourceContext, SlateWorkspaceFacts, Views,
};

struct Fake {
    thread_id: String,
    workspace_id: String,
    now: i64,
    rows: Vec<SessionView>,
    results: Vec<TurnResult>,
    account: Option<AccountRow>,
    facts: Option<SlateWorkspaceFacts>,
    account_reads: Cell<usize>,
}

impl SlateSourceContext for Fake {
    fn thread_id(&self) -> &str {
        &self.thread_id
    }
    fn workspace_id(&self) -> &str {
        &self.workspace_id
    }
    fn now(&self) -> i64 {
        self.now
    }
    fn rows(&self) -> Vec<SessionView> {
        self.rows.clone()
    }
    fn results(&self) -> Vec<TurnResult> {
        self.results.clone()
    }
    fn account(&self) -> LocalBoxFuture<'_, Option<AccountRow>> {
        self.account_reads.set(self.account_reads.get() + 1);
        let account = self.account.clone();
        async move { account }.boxed_local()
    }
    fn workspace(&self) -> Option<SlateWorkspaceFacts> {
        self.facts.clone()
    }
}

fn empty(thread: &str, workspace: &str, now: i64) -> Fake {
    Fake {
        thread_id: thread.into(),
        workspace_id: workspace.into(),
        now,
        rows: vec![],
        results: vec![],
        account: None,
        facts: None,
        account_reads: Cell::new(0),
    }
}

/// A row as the TS probe wrote it, with the fields the wire shape requires filled in when the probe left them out.
fn row(mut v: Value) -> SessionView {
    let o = v.as_object_mut().unwrap();
    o.entry("workspaceId").or_insert(json!("w1"));
    o.entry("harness").or_insert(json!("claude"));
    o.entry("status").or_insert(json!("completed"));
    serde_json::from_value(v).unwrap()
}

fn result(v: Value) -> TurnResult {
    serde_json::from_value(v).unwrap()
}

fn account(v: Value) -> AccountRow {
    serde_json::from_value(v).unwrap()
}

fn facts(v: Value) -> SlateWorkspaceFacts {
    let o = v.as_object().unwrap();
    let text = |k: &str| o.get(k).map(|x| x.as_str().unwrap().to_string());
    SlateWorkspaceFacts {
        computer: text("computer"),
        project: text("project"),
        folder: text("folder"),
        checkout: o.get("checkout").map(|c| serde_json::from_value::<Checkout>(c.clone()).unwrap()),
        pr: o.get("pr").map(|p| serde_json::from_value::<PullRequestSeen>(p.clone()).unwrap()),
        rate_usd_per_hour: o.get("rateUsdPerHour").map(|x| x.as_f64().unwrap()),
        accrued_usd: o.get("accruedUsd").map(|x| x.as_f64().unwrap()),
    }
}

/// The probe's context in pmisc.ts, with the shape's required fields added where the probe left them out.
fn probe() -> Fake {
    let mut f = empty("t1", "w1", 1760000000000);
    f.rows = vec![
        row(
            json!({ "id": "s1", "threadId": "t1", "status": "completed", "startedAt": 1, "endedAt": 2, "prompt": "first line\nsecond", "harness": "claude", "model": "opus", "effort": "high", "permissionMode": "default", "cwd": "/x", "costUsd": 0.5 }),
        ),
        row(
            json!({ "id": "s2", "threadId": "t1", "status": "running", "startedAt": 5, "asking": "a question", "harness": "claude", "costUsd": 0.25, "harnessTitle": "Fix it" }),
        ),
    ];
    f.results = vec![
        result(json!({ "status": "completed", "durationMs": 10, "tokens": { "input": 1, "output": 2, "context": 100, "window": 1000 } })),
        result(json!({ "status": "failed", "durationMs": 20, "waitedMs": 3, "tokens": { "input": 4, "output": 0, "cached": 5 } })),
    ];
    f.account = Some(account(
        json!({ "key": "k", "label": "Me", "agent": "claude", "computers": ["here"], "windows": [{ "kind": "session", "usedPercent": 12, "resetsAt": 99 }, { "kind": "week", "usedPercent": 40 }], "status": "ok", "burn": { "tokensPerMinute": 5, "threads": 2 }, "readAt": 7 }),
    ));
    f.facts = Some(facts(
        json!({ "computer": "spoo", "project": "wsp", "folder": "/root/wsp", "checkout": { "branch": "main", "ahead": 1, "behind": 0, "changed": 2, "readAt": 3 }, "pr": { "number": 7, "url": "u", "state": "open", "base": "main", "readAt": 4, "checks": [{ "name": "ci", "state": "pass" }] }, "rateUsdPerHour": 0.1, "accruedUsd": 2 }),
    ));
    f
}

async fn view_of(name: &str, ctx: &Fake) -> Option<Value> {
    view_sources(&[name.to_string()], ctx).await.get(name).cloned().flatten()
}

/// The whole value and its key order, against the string the TS printed.
fn is(got: &Option<Value>, ts: &str) {
    let got = got.as_ref().expect("the view is none");
    assert_eq!(serde_json::to_string(got).unwrap(), ts);
    assert_eq!(got, &serde_json::from_str::<Value>(ts).unwrap());
}

fn names(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

fn n(s: &str) -> Seg {
    Seg::Name(s.into())
}

fn i(x: i64) -> Seg {
    Seg::Index(x)
}

fn show(v: Option<Value>) -> String {
    v.map_or("U".to_string(), |v| serde_json::to_string(&v).unwrap())
}

fn keys(views: &Views) -> Vec<&str> {
    views.keys().map(String::as_str).collect()
}

// spec rule 4
#[test]
fn the_six_sources_in_order() {
    let got: Vec<&str> = host_slate_sources().iter().map(|s| s.name()).collect();
    assert_eq!(got, ["thread", "usage", "cost", "time", "git", "pr"]);
}

// spec rule 3
#[test]
fn json_drops_absent_fields_and_turns_none_into_null() {
    assert_eq!(to_json(None::<&Value>), Value::Null);
    assert_eq!(to_json(Some(&json!({ "a": [1, null] }))), json!({ "a": [1, null] }));
    let bare = row(json!({ "id": "s" }));
    assert_eq!(to_json(Some(&bare)), json!({ "id": "s", "workspaceId": "w1", "harness": "claude", "status": "completed" }));
}

// spec rule 5
#[test]
fn path_segments_reads_names_and_indexes() {
    assert_eq!(path_segments("pr.checks[0].name"), [n("pr"), n("checks"), i(0), n("name")]);
    assert_eq!(path_segments("a[-1]"), [n("a"), i(-1)]);
    assert_eq!(path_segments("a..b[x]"), [n("a"), n("b"), n("x")]);
    assert_eq!(path_segments(""), Vec::<Seg>::new());
    assert_eq!(path_segments("thread.lastTurn.model"), [n("thread"), n("lastTurn"), n("model")]);
    assert_eq!(path_segments("a[0][12]"), [n("a"), i(0), i(12)]);
}

// spec rule 6
#[test]
fn walk_takes_own_keys_and_indexes_and_refuses_unsafe_names() {
    let some = |v: Value| Some(v);
    assert_eq!(walk(Some(&json!({ "a": [1, 2, { "b": 3 }] })), &[n("a"), i(-1), n("b")]), some(json!(3)));
    assert_eq!(walk(Some(&json!({ "a": [1] })), &[n("a"), n("0")]), None);
    assert_eq!(walk(Some(&json!({ "a": 1 })), &[n("__proto__")]), None);
    assert_eq!(walk(Some(&json!({ "a": { "toString": 1 } })), &[n("a"), n("toString")]), some(json!(1)));
    assert_eq!(walk(Some(&json!({ "a": {} })), &[n("a"), n("toString")]), None);
    assert_eq!(walk(Some(&Value::Null), &[]), some(Value::Null));
    assert_eq!(walk(Some(&json!([5])), &[i(-2)]), None);
    assert_eq!(walk(Some(&json!({ "a": [1, 2] })), &[n("a"), i(5)]), None);

    let unsafe_keys: Value = serde_json::from_str(r#"{"__proto__":1,"constructor":2,"prototype":3,"ok":4}"#).unwrap();
    assert_eq!(walk(Some(&unsafe_keys), &[n("__proto__")]), None);
    assert_eq!(walk(Some(&unsafe_keys), &[n("constructor")]), None);
    assert_eq!(walk(Some(&unsafe_keys), &[n("prototype")]), None);
    assert_eq!(walk(Some(&unsafe_keys), &[n("ok")]), some(json!(4)));

    let list = json!([5, 6, 7]);
    assert_eq!(walk(Some(&list), &[i(-3)]), some(json!(5)));
    assert_eq!(walk(Some(&list), &[i(-4)]), None);
    assert_eq!(walk(Some(&list), &[i(3)]), None);
    assert_eq!(walk(Some(&list), &[i(2)]), some(json!(7)));

    assert_eq!(walk(None, &[]), None);
    assert_eq!(walk(None, &[n("a")]), None);
    assert_eq!(walk(Some(&json!({ "a": 1 })), &[i(0)]), None);
    assert_eq!(walk(Some(&json!({ "a": 1 })), &[n("a"), n("b")]), None);
    assert_eq!(walk(Some(&json!({ "a": null })), &[n("a"), n("b")]), None);
    assert_eq!(walk(Some(&json!({ "a": null })), &[n("a")]), some(Value::Null));
}

// spec rules 7 and 4
#[tokio::test(flavor = "current_thread")]
async fn view_sources_views_each_name_once_and_skips_unknown() {
    let ctx = probe();
    let all = names(&["thread", "usage", "cost", "time", "git", "pr", "nope", "thread"]);
    let views = view_sources(&all, &ctx).await;
    assert_eq!(keys(&views), ["thread", "usage", "cost", "time", "git", "pr"]);
    assert!(views.get("nope").is_none());
    assert_eq!(ctx.account_reads.get(), 1);

    let ctx = empty("t", "w", 0);
    let views = view_sources(&names(&["pr", "thread", "pr", "zzz"]), &ctx).await;
    assert_eq!(keys(&views), ["pr", "thread"]);

    let ctx = probe();
    view_sources(&names(&["usage", "usage", "usage"]), &ctx).await;
    assert_eq!(ctx.account_reads.get(), 1);

    assert!(view_sources(&[], &ctx).await.is_empty());
}

// spec rule 7 and the edge case of a view that fails: its name stays in the map with none
#[tokio::test(flavor = "current_thread")]
async fn a_view_that_fails_reads_as_none() {
    let ctx = empty("t2", "w2", 0);
    let views = view_sources(&names(&["usage"]), &ctx).await;
    assert_eq!(keys(&views), ["usage"]);
    assert_eq!(views.get("usage"), Some(&None));
    assert_eq!(resolve_in(&views, "usage.account.key"), None);
}

// spec rule 8 and the edge case of a path that starts with a bracket
#[tokio::test(flavor = "current_thread")]
async fn resolve_in_reads_a_path_out_of_the_views() {
    let ctx = probe();
    let views = view_sources(&names(&["thread", "usage", "cost", "time", "git", "pr", "nope", "thread"]), &ctx).await;
    assert_eq!(resolve_in(&views, "pr.checks[0].name"), Some(json!("ci")));
    assert_eq!(resolve_in(&views, "nope.x"), None);
    assert_eq!(resolve_in(&views, "[0]"), None);

    let mut ctx = empty("t", "w", 0);
    ctx.facts = Some(facts(json!({ "pr": {
        "number": 12, "url": "https://x/pr/12", "state": "open", "host": "github.com", "draft": true, "base": "main", "branch": "feat",
        "headOid": "abc123", "headSubject": "Add it", "mergeable": "mergeable", "mergeState": "clean", "review": "approved",
        "checks": [{ "name": "ci", "state": "fail" }, { "name": "lint", "state": "pending" }],
        "additions": 10, "deletions": 2, "changedFiles": 3, "commits": 4, "readAt": 77 } })));
    let views = view_sources(&names(&["thread", "pr"]), &ctx).await;
    assert_eq!(show(resolve_in(&views, "thread.model")), "null");
    assert_eq!(
        show(resolve_in(&views, "thread.lastTurn")),
        r#"{"status":null,"durationMs":null,"waitedMs":null,"startedAt":null,"endedAt":null,"model":null}"#
    );
    assert_eq!(show(resolve_in(&views, "thread.cost.usd")), "0");
    assert_eq!(show(resolve_in(&views, "thread.turns")), "0");
    assert_eq!(show(resolve_in(&views, "thread.id")), r#""t""#);
    assert_eq!(show(resolve_in(&views, "pr.checks[-1].name")), r#""lint""#);
    assert_eq!(show(resolve_in(&views, "pr.checks[2]")), "U");
    assert_eq!(show(resolve_in(&views, "pr.checks.0")), "U");
    assert_eq!(show(resolve_in(&views, "pr.__proto__")), "U");
    assert_eq!(show(resolve_in(&views, "pr.checks.length")), "U");
    assert_eq!(show(resolve_in(&views, "pr.number.x")), "U");
    assert_eq!(show(resolve_in(&views, "pr.draft")), "true");
    assert_eq!(show(resolve_in(&views, "time.now")), "U");
    assert_eq!(show(resolve_in(&views, "")), "U");
    assert_eq!(resolve_in(&views, "thread"), views.get("thread").cloned().flatten());
}

// spec rule 9
#[tokio::test(flavor = "current_thread")]
async fn thread_view_matches_the_ts() {
    let ctx = probe();
    is(
        &view_of("thread", &ctx).await,
        r#"{"id":"t1","title":"Fix it","agent":"claude","model":null,"effort":null,"status":"needs-you","access":null,"computer":"spoo","project":"wsp","folder":"/root/wsp","turns":2,"lastTurn":{"status":"failed","durationMs":20,"waitedMs":3,"startedAt":5,"endedAt":null,"model":null},"cost":{"usd":0.75},"tokens":{"input":5,"output":2,"cached":5,"cacheWrite":0,"reasoning":0},"context":{"used":100,"window":1000,"free":900,"percent":10},"waitingOn":"a question"}"#,
    );

    // the title: harnessTitle, else the prompt's first line cut at 80 UTF-16 units (78 letters and one emoji of two units), else null
    let long = format!("{}\u{1F600}tail words beyond eighty\nsecond", "a".repeat(78));
    let mut c = empty("t", "w", 0);
    c.rows = vec![row(json!({ "id": "s", "threadId": "t", "prompt": long }))];
    let title = view_of("thread", &c).await.unwrap()["title"].clone();
    assert_eq!(title, json!(format!("{}\u{1F600}", "a".repeat(78))));
    c.rows = vec![row(json!({ "id": "s", "prompt": "" }))];
    assert_eq!(view_of("thread", &c).await.unwrap()["title"], json!(""));
    c.rows = vec![row(json!({ "id": "s" }))];
    assert_eq!(view_of("thread", &c).await.unwrap()["title"], Value::Null);
    c.rows = vec![row(json!({ "id": "s", "prompt": "p", "harnessTitle": "H" }))];
    assert_eq!(view_of("thread", &c).await.unwrap()["title"], json!("H"));

    // the status word and what the thread waits on
    let waiting_on = json!({ "threadId": "x", "workspaceId": "w", "sessionId": "s", "title": "T", "prompt": { "askId": "a", "toolName": "Bash", "input": "ls", "options": [] } });
    let cases = [
        (json!({ "status": "running" }), "working", Value::Null),
        (json!({ "status": "running", "asking": "q" }), "needs-you", json!("q")),
        (json!({ "status": "running", "waitingOn": waiting_on }), "needs-you", Value::Null),
        (json!({ "status": "failed" }), "failed", Value::Null),
        (json!({ "status": "interrupted" }), "resting", Value::Null),
        (json!({ "status": "completed" }), "done", Value::Null),
    ];
    for (r, status, asking) in cases {
        let mut c = empty("t", "w", 0);
        let mut r = r;
        r["id"] = json!("s");
        c.rows = vec![row(r)];
        let v = view_of("thread", &c).await.unwrap();
        assert_eq!(v["status"], json!(status));
        assert_eq!(v["waitingOn"], asking);
    }

    // the latest row is the greatest startedAt, a tie goes to the later row; the folder is the latest row's own cwd
    let mut c = empty("t", "w", 0);
    c.rows = vec![
        row(json!({ "id": "a", "harness": "claude", "model": "m1", "startedAt": 5, "cwd": "/a", "costUsd": 1 })),
        row(
            json!({ "id": "b", "harness": "codex", "model": "m2", "startedAt": 5, "endedAt": 9, "permissionMode": "plan", "effort": "low", "costUsd": 2.5 }),
        ),
        row(json!({ "id": "c", "harness": "x", "startedAt": 3 })),
    ];
    is(
        &view_of("thread", &c).await,
        r#"{"id":"t","title":null,"agent":"codex","model":"m2","effort":"low","status":"done","access":"plan","computer":null,"project":null,"folder":null,"turns":3,"lastTurn":{"status":null,"durationMs":null,"waitedMs":null,"startedAt":5,"endedAt":9,"model":"m2"},"cost":{"usd":3.5},"tokens":{"input":0,"output":0,"cached":0,"cacheWrite":0,"reasoning":0},"context":{"used":null,"window":null,"free":null,"percent":null},"waitingOn":null}"#,
    );
    c.rows = vec![row(json!({ "id": "a", "model": "m1", "startedAt": 0 })), row(json!({ "id": "b", "model": "m2" }))];
    assert_eq!(view_of("thread", &c).await.unwrap()["model"], json!("m2"));

    // with no cwd on the latest row the folder is the facts' folder
    let mut c = empty("t", "w", 0);
    c.rows = vec![row(json!({ "id": "a" }))];
    c.facts = Some(facts(json!({ "folder": "/f", "computer": "c", "project": "p" })));
    is(
        &view_of("thread", &c).await,
        r#"{"id":"t","title":null,"agent":"claude","model":null,"effort":null,"status":"done","access":null,"computer":"c","project":"p","folder":"/f","turns":1,"lastTurn":{"status":null,"durationMs":null,"waitedMs":null,"startedAt":null,"endedAt":null,"model":null},"cost":{"usd":0},"tokens":{"input":0,"output":0,"cached":0,"cacheWrite":0,"reasoning":0},"context":{"used":null,"window":null,"free":null,"percent":null},"waitingOn":null}"#,
    );

    // tokens are summed over the results that have them; the context is the newest result that carries one
    let mut c = empty("t", "w", 0);
    c.results = vec![
        result(
            json!({ "status": "completed", "durationMs": 4, "tokens": { "input": 1, "output": 2, "cached": 3, "cacheWrite": 4, "reasoning": 5, "context": 10, "window": 40 } }),
        ),
        result(json!({ "status": "interrupted", "tokens": { "input": 10, "output": 20, "context": 30, "window": 40 } })),
        result(json!({ "status": "completed" })),
    ];
    let v = view_of("thread", &c).await.unwrap();
    assert_eq!(serde_json::to_string(&v["tokens"]).unwrap(), r#"{"input":11,"output":22,"cached":3,"cacheWrite":4,"reasoning":5}"#);
    assert_eq!(serde_json::to_string(&v["context"]).unwrap(), r#"{"used":30,"window":40,"free":10,"percent":75}"#);
    assert_eq!(
        serde_json::to_string(&v["lastTurn"]).unwrap(),
        r#"{"status":"completed","durationMs":null,"waitedMs":null,"startedAt":null,"endedAt":null,"model":null}"#
    );

    // a window of 0 gives a free count and no percent; a context with no window gives neither
    c.results = vec![
        result(json!({ "status": "completed", "tokens": { "input": 1, "output": 1, "context": 50, "window": 0 } })),
        result(json!({ "status": "completed" })),
        result(json!({ "status": "completed", "tokens": { "input": 2, "output": 2 } })),
    ];
    let v = view_of("thread", &c).await.unwrap();
    assert_eq!(serde_json::to_string(&v["context"]).unwrap(), r#"{"used":50,"window":0,"free":-50,"percent":null}"#);
    c.results = vec![result(json!({ "status": "completed", "tokens": { "input": 1, "output": 1, "context": 5 } }))];
    let v = view_of("thread", &c).await.unwrap();
    assert_eq!(serde_json::to_string(&v["context"]).unwrap(), r#"{"used":5,"window":null,"free":null,"percent":null}"#);
}

// spec rule 9 and the empty thread edge case
#[tokio::test(flavor = "current_thread")]
async fn thread_view_of_an_empty_thread() {
    let ctx = empty("t2", "w2", 0);
    is(
        &view_of("thread", &ctx).await,
        r#"{"id":"t2","title":null,"agent":null,"model":null,"effort":null,"status":"starting","access":null,"computer":null,"project":null,"folder":null,"turns":0,"lastTurn":{"status":null,"durationMs":null,"waitedMs":null,"startedAt":null,"endedAt":null,"model":null},"cost":{"usd":0},"tokens":{"input":0,"output":0,"cached":0,"cacheWrite":0,"reasoning":0},"context":{"used":null,"window":null,"free":null,"percent":null},"waitingOn":null}"#,
    );
}

// spec rule 10
#[tokio::test(flavor = "current_thread")]
async fn usage_view_matches_the_ts() {
    let ctx = probe();
    is(
        &view_of("usage", &ctx).await,
        r#"{"account":{"key":"k","label":"Me","agent":"claude","plan":null,"address":null,"computers":["here"]},"windows":[{"kind":"session","usedPercent":12,"resetsAt":99},{"kind":"week","usedPercent":40,"resetsAt":null}],"session":{"percent":12,"resetsAt":99},"week":{"percent":40,"resetsAt":null},"status":"ok","note":null,"burn":{"tokensPerMinute":5,"threads":2},"credits":{"count":null,"nextExpiresAt":null},"readAt":7}"#,
    );

    // every field set; session and week come from the first window of their kind
    let mut c = empty("t", "w", 0);
    c.account = Some(account(
        json!({ "key": "k2", "label": "Key", "agent": "codex", "computers": [], "plan": "Plus", "address": "me@x.io",
        "windows": [{ "kind": "week_opus", "usedPercent": 3.5 }, { "kind": "week", "usedPercent": 50, "resetsAt": 5 }, { "kind": "week", "usedPercent": 60, "resetsAt": 6 }],
        "status": "warning", "note": "reports no plan limit", "burn": { "tokensPerMinute": 12.5, "threads": 2 }, "credits": { "count": 3, "nextExpiresAt": 999, "readAt": 1 }, "readAt": 10 }),
    ));
    is(
        &view_of("usage", &c).await,
        r#"{"account":{"key":"k2","label":"Key","agent":"codex","plan":"Plus","address":"me@x.io","computers":[]},"windows":[{"kind":"week_opus","usedPercent":3.5,"resetsAt":null},{"kind":"week","usedPercent":50,"resetsAt":5},{"kind":"week","usedPercent":60,"resetsAt":6}],"session":{"percent":null,"resetsAt":null},"week":{"percent":50,"resetsAt":5},"status":"warning","note":"reports no plan limit","burn":{"tokensPerMinute":12.5,"threads":2},"credits":{"count":3,"nextExpiresAt":999},"readAt":10}"#,
    );

    // nothing optional set
    c.account = Some(account(json!({ "key": "k3", "label": "L", "agent": "claude", "computers": ["a", "b"] })));
    is(
        &view_of("usage", &c).await,
        r#"{"account":{"key":"k3","label":"L","agent":"claude","plan":null,"address":null,"computers":["a","b"]},"windows":[],"session":{"percent":null,"resetsAt":null},"week":{"percent":null,"resetsAt":null},"status":null,"note":null,"burn":{"tokensPerMinute":null,"threads":null},"credits":{"count":null,"nextExpiresAt":null},"readAt":null}"#,
    );

    // zero is a value, not an absence; credits with no expiry
    c.account = Some(account(
        json!({ "key": "k3", "label": "L", "agent": "claude", "computers": ["a", "b"], "credits": { "count": 0, "readAt": 2 }, "burn": { "tokensPerMinute": 0, "threads": 0 } }),
    ));
    is(
        &view_of("usage", &c).await,
        r#"{"account":{"key":"k3","label":"L","agent":"claude","plan":null,"address":null,"computers":["a","b"]},"windows":[],"session":{"percent":null,"resetsAt":null},"week":{"percent":null,"resetsAt":null},"status":null,"note":null,"burn":{"tokensPerMinute":0,"threads":0},"credits":{"count":0,"nextExpiresAt":null},"readAt":null}"#,
    );
}

// spec rule 10, no account
#[tokio::test(flavor = "current_thread")]
async fn usage_view_with_no_account_is_none() {
    let ctx = empty("t2", "w2", 0);
    assert_eq!(view_of("usage", &ctx).await, None);
    let views = view_sources(&names(&["usage"]), &ctx).await;
    assert_eq!(views.get("usage"), Some(&None));
}

// spec rules 11 and 13
#[tokio::test(flavor = "current_thread")]
async fn cost_and_git_views_match_the_ts() {
    let ctx = probe();
    is(&view_of("cost", &ctx).await, r#"{"rateUsdPerHour":0.1,"accruedUsd":2}"#);
    is(
        &view_of("git", &ctx).await,
        r#"{"branch":"main","head":null,"ahead":1,"behind":0,"changed":2,"stashes":0,"editsUnread":false,"countsUnknown":false,"readAt":3}"#,
    );

    // no workspace facts, then facts with only one of the two numbers: each absent one is 0
    let c = empty("t2", "w2", 0);
    is(&view_of("cost", &c).await, r#"{"rateUsdPerHour":0,"accruedUsd":0}"#);
    let mut c = empty("t", "w", 0);
    c.facts = Some(facts(json!({ "computer": "c", "rateUsdPerHour": 0.5 })));
    is(&view_of("cost", &c).await, r#"{"rateUsdPerHour":0.5,"accruedUsd":0}"#);
    c.facts = Some(facts(json!({ "accruedUsd": 1.25 })));
    is(&view_of("cost", &c).await, r#"{"rateUsdPerHour":0,"accruedUsd":1.25}"#);

    // a checkout that carries head, stashes and the two flags passes them through
    c.facts = Some(facts(
        json!({ "checkout": { "branch": "dev", "ahead": 0, "behind": 3, "changed": 0, "editsUnread": true, "countsUnknown": true, "stashes": 4, "head": "deadbeef", "readAt": 5 } }),
    ));
    is(
        &view_of("git", &c).await,
        r#"{"branch":"dev","head":"deadbeef","ahead":0,"behind":3,"changed":0,"stashes":4,"editsUnread":true,"countsUnknown":true,"readAt":5}"#,
    );
}

// spec rule 13, no checkout
#[tokio::test(flavor = "current_thread")]
async fn git_view_without_a_checkout_is_none() {
    let ctx = empty("t2", "w2", 0);
    assert_eq!(view_of("git", &ctx).await, None);
    let mut c = empty("t", "w", 0);
    c.facts = Some(facts(json!({ "computer": "spoo", "project": "wsp" })));
    assert_eq!(view_of("git", &c).await, None);
}

// spec rule 12; the zone's name depends on the machine, so the zone is only required to be named
#[tokio::test(flavor = "current_thread")]
async fn time_view_names_the_zone_and_todays_date_in_it() {
    let shape = regex::Regex::new(r#"^\{"now":(-?\d+),"today":"(\d{4}-\d{2}-\d{2})","zone":"([^"]+)"\}$"#).unwrap();

    let ctx = probe();
    let view = view_of("time", &ctx).await.expect("the time view is none");
    let text = serde_json::to_string(&view).unwrap();
    let caps = shape.captures(&text).unwrap_or_else(|| panic!("the time view reads {text}"));
    assert_eq!(&caps[1], "1760000000000");
    // 2025-10-09T08:53:20Z: the 8th west of UTC-9, the 9th from there to UTC+14, so the 10th never happens
    assert!(["2025-10-08", "2025-10-09", "2025-10-10"].contains(&&caps[2]), "today is {}", &caps[2]);
    assert_eq!(view["now"], json!(1760000000000i64));

    // 1970-01-01T00:00:00Z
    let ctx = empty("t", "w", 0);
    let view = view_of("time", &ctx).await.unwrap();
    let text = serde_json::to_string(&view).unwrap();
    let caps = shape.captures(&text).unwrap_or_else(|| panic!("the time view reads {text}"));
    assert_eq!(&caps[1], "0");
    assert!(["1969-12-31", "1970-01-01"].contains(&&caps[2]), "today is {}", &caps[2]);
}

// spec rule 14
#[tokio::test(flavor = "current_thread")]
async fn pr_view_of_a_fact() {
    // the probe's pull request as the record kept it: the fact-only fields are null
    let ctx = probe();
    is(
        &view_of("pr", &ctx).await,
        r#"{"number":7,"url":"u","state":"open","base":"main","draft":null,"branch":null,"headOid":null,"headSubject":null,"mergeable":null,"review":null,"checks":[{"name":"ci","workflow":null,"state":"pass","link":null,"description":null,"startedAt":null,"completedAt":null}],"additions":null,"deletions":null,"changedFiles":null,"commits":null,"behindBase":null,"author":null,"readAt":4,"word":"open","unread":null}"#,
    );

    let fact = json!({
        "number": 12, "url": "https://x/pr/12", "state": "open", "host": "github.com", "draft": true, "base": "main", "branch": "feat",
        "headOid": "abc123", "headSubject": "Add it", "mergeable": "mergeable", "mergeState": "clean", "review": "approved",
        "checks": [
            { "name": "ci", "workflow": "build", "state": "fail", "link": "https://x/run", "description": "boom", "startedAt": "2025-10-09T00:00:00Z", "completedAt": "2025-10-09T00:01:00Z", "run": { "runId": 1, "jobId": 2 } },
            { "name": "lint", "state": "pending" }
        ],
        "additions": 10, "deletions": 2, "changedFiles": 3, "commits": 4, "behindBase": 6, "author": "zingzy", "readAt": 77 });
    let mut c = empty("t", "w", 0);
    c.facts = Some(facts(json!({ "pr": fact })));
    is(
        &view_of("pr", &c).await,
        r#"{"number":12,"url":"https://x/pr/12","state":"open","base":"main","draft":true,"branch":"feat","headOid":"abc123","headSubject":"Add it","mergeable":"mergeable","review":"approved","checks":[{"name":"ci","workflow":"build","state":"fail","link":"https://x/run","description":"boom","startedAt":"2025-10-09T00:00:00Z","completedAt":"2025-10-09T00:01:00Z"},{"name":"lint","workflow":null,"state":"pending","link":null,"description":null,"startedAt":null,"completedAt":null}],"additions":10,"deletions":2,"changedFiles":3,"commits":4,"behindBase":6,"author":"zingzy","readAt":77,"word":"checks failed","unread":null}"#,
    );

    // a merged fact with no checks, behindBase or author
    let mut merged = fact.clone();
    merged["draft"] = json!(false);
    merged["state"] = json!("merged");
    merged["checks"] = json!([]);
    merged["review"] = json!("none");
    merged["mergeable"] = json!("conflicting");
    merged.as_object_mut().unwrap().remove("behindBase");
    merged.as_object_mut().unwrap().remove("author");
    c.facts = Some(facts(json!({ "pr": merged })));
    is(
        &view_of("pr", &c).await,
        r#"{"number":12,"url":"https://x/pr/12","state":"merged","base":"main","draft":false,"branch":"feat","headOid":"abc123","headSubject":"Add it","mergeable":"conflicting","review":"none","checks":[],"additions":10,"deletions":2,"changedFiles":3,"commits":4,"behindBase":null,"author":null,"readAt":77,"word":"merged","unread":null}"#,
    );

    // a settled one the record kept: the fact-only fields are null, and with no checks the list is empty
    c.facts = Some(facts(
        json!({ "pr": { "number": 3, "url": "u3", "state": "merged", "base": "dev", "branch": "b", "mergedAt": 5, "readAt": 8 } }),
    ));
    is(
        &view_of("pr", &c).await,
        r#"{"number":3,"url":"u3","state":"merged","base":"dev","draft":null,"branch":null,"headOid":null,"headSubject":null,"mergeable":null,"review":null,"checks":[],"additions":null,"deletions":null,"changedFiles":null,"commits":null,"behindBase":null,"author":null,"readAt":8,"word":"merged","unread":null}"#,
    );
    c.facts = Some(facts(json!({ "pr": { "number": 3, "url": "u3", "state": "closed", "base": "dev", "readAt": 8 } })));
    is(
        &view_of("pr", &c).await,
        r#"{"number":3,"url":"u3","state":"closed","base":"dev","draft":null,"branch":null,"headOid":null,"headSubject":null,"mergeable":null,"review":null,"checks":[],"additions":null,"deletions":null,"changedFiles":null,"commits":null,"behindBase":null,"author":null,"readAt":8,"word":"closed","unread":null}"#,
    );
}

// spec rule 14, no pull request seen and one that could not be read
#[tokio::test(flavor = "current_thread")]
async fn pr_view_unread_and_unseen() {
    let ctx = empty("t2", "w2", 0);
    is(&view_of("pr", &ctx).await, r#"{"number":null,"word":null,"unread":null,"checks":[]}"#);

    let mut c = empty("t2", "w2", 0);
    c.facts = Some(facts(json!({ "computer": "spoo" })));
    is(&view_of("pr", &c).await, r#"{"number":null,"word":null,"unread":null,"checks":[]}"#);

    c.facts = Some(facts(json!({ "pr": { "why": "gh is not signed in", "readAt": 9 } })));
    is(&view_of("pr", &c).await, r#"{"number":null,"word":"not read","unread":"gh is not signed in","checks":[],"readAt":9}"#);
}
