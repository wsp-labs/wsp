// SPDX-License-Identifier: AGPL-3.0-only
//! What a launch writes on its thread's record: the record a thread's first turn makes (sessions.ts:831), then the clouds
//! it takes in place, the resume it drops and the files folder it adds (sessions.ts:832-838), each key where node puts
//! it. Expected bytes: wsp-core/tests/fixtures/thread_record_order/gen.mjs, roads `create` and `launch`.
#![allow(dead_code, unused_imports)]

#[path = "support_a/mod.rs"]
mod support;
include!("sessions_launch/world.rs");

const CASES: &str = include_str!("../../wsp-core/tests/fixtures/thread_record_order/cases.json");

fn cases(road: &str) -> Vec<Value> {
    let all: Vec<Value> = serde_json::from_str(CASES).expect("the cases are JSON");
    all.into_iter().filter(|c| c["road"] == road).collect()
}

/// The record as a road made it (its keys in the order the TS inserted them), and again as a restart reads it back,
/// which writes the struct's order, where that is the same record.
fn variants(before: &Value) -> Vec<ThreadRecord> {
    let road = ThreadRecord::from_map(before.as_object().expect("a record").clone()).expect("the record literal");
    let hydrated: ThreadRecord = serde_json::from_value(before.clone()).expect("the record literal");
    let same = serde_json::to_string(&hydrated).expect("serializes") == serde_json::to_string(&road).expect("serializes");
    if same {
        vec![road, hydrated]
    } else {
        vec![road]
    }
}

fn text(v: &Value) -> Option<String> {
    v.as_str().map(str::to_string)
}

fn forked(v: &Value) -> ForkedFrom {
    ForkedFrom {
        thread_id: text(&v["threadId"]).expect("a thread"),
        turn_id: text(&v["turnId"]),
        title: text(&v["title"]).expect("a title"),
    }
}

/// A first turn that opens the thread with what the case says it was started with.
fn opens(r: &Rig, c: &Value) -> Prepared {
    let mut p = prepared(r);
    p.tree = TreeOf { parent_thread_id: text(&c["tree"]["parentThreadId"]), root_thread_id: text(&c["tree"]["rootThreadId"]) };
    p.opts.permission_mode = text(&c["permissionMode"]);
    if let Some(replaces) = text(&c["replaces"]) {
        r.host.add_record(&replaces, "w1", thread_record(json!({ "harness": "claude" })));
        r.host.hooks.set::<dyn Fn() -> LocalBoxFuture<'static, Result<Preferences, HostError>>>(
            "LaunchCtx::preferences",
            Rc::new(|| async { Ok(prefs()) }.boxed_local()),
        );
        p.replaces = Some(replaces);
    }
    if c.get("forkedFrom").is_some() {
        let mut plan = plan();
        plan.forked_from = forked(&c["forkedFrom"]);
        p.plan = Some(plan);
    }
    if c["worked"] == json!(true) {
        r.host.add_row(
            "old",
            row(view("old", "w1", json!({ "threadId": "t1", "status": "completed", "claudeSessionId": "s0" })), "old-turn"),
        );
    }
    p
}

fn wrote(r: &Rig) -> String {
    serde_json::to_string(r.host.records.borrow().get("t1").expect("the thread has a record")).expect("serializes")
}

#[test]
fn the_record_a_first_turn_writes_has_the_keys_in_the_order_node_writes_them() {
    let all = cases("create");
    assert_eq!(all.len(), 6);
    for case in all {
        run_local(async move {
            let r = rig();
            let p = opens(&r, &case["input"]);
            ok(r.run(p).await);
            assert_eq!(wrote(&r), case["after"].as_str().expect("the bytes"), "{}", case["name"]);
        });
    }
}

#[test]
fn a_launch_adds_clouds_and_a_files_folder_after_the_keys_the_record_holds() {
    let all = cases("launch");
    assert_eq!(all.len(), 9);
    for case in all {
        let befores: Vec<Option<ThreadRecord>> =
            if case["before"].is_object() { variants(&case["before"]).into_iter().map(Some).collect() } else { vec![None] };
        for before in befores {
            let case = case.clone();
            run_local(async move {
                let r = rig();
                let input = &case["input"];
                let mut p = match before {
                    Some(held) => {
                        r.host.add_record("t1", "w1", held);
                        let mut p = prepared(&r);
                        p.opens = false;
                        p.cleared = false;
                        p.opts.thread = Some("t1".to_string());
                        p
                    }
                    None => opens(&r, &input["create"]),
                };
                p.clouds = input["clouds"].as_array().map(|l| l.iter().filter_map(text).collect());
                if let Some(folder) = text(&input["filesFolder"]) {
                    p.opts.attachments =
                        Some(vec![serde_json::from_value(json!({ "mediaType": "application/pdf", "bytes": "cGRm" })).expect("a file")]);
                    r.host.hooks.set::<dyn Fn(&Entry, Option<&str>, &str) -> String>(
                        "LaunchCtx::runs_in",
                        Rc::new(move |_e: &Entry, _ran_in: Option<&str>, _folder: &str| folder.clone()),
                    );
                    r.host
                        .hooks
                        .set::<dyn Fn(&Entry, &str, &str, Vec<Attachment>) -> LocalBoxFuture<'static, Result<Vec<String>, HostError>>>(
                            "SessionsCtx::land_files",
                            Rc::new(|_e: &Entry, _landing: &str, _dir: &str, _files: Vec<Attachment>| {
                                async { Ok(vec!["/f/a.pdf".to_string()]) }.boxed_local()
                            }),
                        );
                }
                ok(r.run(p).await);
                assert_eq!(wrote(&r), case["after"].as_str().expect("the bytes"), "{}", case["name"]);
            });
        }
    }
}
