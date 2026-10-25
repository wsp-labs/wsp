// SPDX-License-Identifier: AGPL-3.0-only
//! spec/launch.md. TS cases: thread-warm-agent.test.ts, lost-session-line.test.ts, cloud-notice.test.ts,
//! thread-defaults.test.ts, thread-keeps-picks.test.ts. Each TS case is ported at this unit's seam: the warm area's
//! answers and what it hands `hold_warm`, `reap_kept`, `rewarm` and the adapter; the picks a launch reads; the line a
//! lost session logs.
mod support;

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use support::*;
use wsp_core::console;
use wsp_core::types::internal::lost_session_log_line;
use wsp_wire::types::env::{NO_SLATE_MCP_ARG, SCOPED_MCP_ARG, THREAD_ENV, TURN_TOKEN_ENV};
use wsp_wire::views::harness::start_picks;
use wsp_wire::views::preferences::default_preferences;
use wsp_wire::wire::runtime::{SessionWarmResult, SessionWarmResultWarm};
use wsp_words::place_word::{place_said, PlaceRef};
use wsp_words::sub_thread::{cloud_added_line, cloud_removed_line};

fn err(message: &str, kind: Option<Kind>) -> HostError {
    match kind {
        Some(k) => HostError::with_kind(message, k),
        None => HostError::new(message),
    }
}

fn warm_result(warm: SessionWarmResultWarm) -> SessionWarmResult {
    SessionWarmResult { warm }
}

fn started() -> SessionWarmResult {
    warm_result(SessionWarmResultWarm::Started)
}

fn standing() -> SessionWarmResult {
    warm_result(SessionWarmResultWarm::Standing)
}

fn none() -> SessionWarmResult {
    warm_result(SessionWarmResultWarm::None)
}

/// The calls, in order, must hold each of `wanted` one after the other (others may sit between).
fn in_order(calls: &[String], wanted: &[&str]) {
    let mut at = 0;
    for w in wanted {
        match calls[at..].iter().position(|c| c == w) {
            Some(i) => at += i + 1,
            None => panic!("{w} not found in order after position {at} of {calls:?}"),
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// served_to

fn wsp_server(args: &[&str]) -> McpServerSpec {
    McpServerSpec { command: "/usr/local/bin/node".into(), args: args.iter().map(|a| a.to_string()).collect(), no_slate: None }
}

// L1: launch.ts:212
#[test]
fn a_root_thread_keeps_its_server() {
    let wsp = wsp_server(&["/opt/wsp/bin.js", "mcp", SCOPED_MCP_ARG]);
    assert_eq!(served_to(Some(&wsp), "t1", "t1"), Some(wsp.clone()));
    assert_eq!(served_to(None, "t1", "t1"), None);
    assert_eq!(served_to(None, "root", "child"), None, "no server to hand a child either");
}

// L2: launch.ts:212
#[test]
fn a_child_gets_no_slate_and_the_scoped_arg_list_gains_the_flag() {
    let wsp = wsp_server(&["/opt/wsp/bin.js", "mcp", "--state", "/Users/me/.wsp/state.json", SCOPED_MCP_ARG]);
    let got = served_to(Some(&wsp), "root", "child").expect("a server for the child");
    assert_eq!(
        serde_json::to_string(&got).unwrap(),
        format!(
            r#"{{"command":"/usr/local/bin/node","args":["/opt/wsp/bin.js","mcp","--state","/Users/me/.wsp/state.json","{SCOPED_MCP_ARG}","{NO_SLATE_MCP_ARG}"],"noSlate":true}}"#
        ),
        "the server's own keys, then noSlate, with args overwritten in place"
    );
    assert_eq!(wsp.args.last().map(String::as_str), Some(SCOPED_MCP_ARG), "the host's own server is left as it was");
}

// L2: launch.ts:212
#[test]
fn a_child_server_without_the_scoped_arg_keeps_its_args() {
    let wsp = wsp_server(&["/opt/wsp/bin.js", "mcp"]);
    let got = served_to(Some(&wsp), "root", "child").expect("a server for the child");
    assert_eq!(serde_json::to_string(&got).unwrap(), r#"{"command":"/usr/local/bin/node","args":["/opt/wsp/bin.js","mcp"],"noSlate":true}"#);
}

// ---------------------------------------------------------------------------------------------------------------
// clouds_told

fn provider(name: &str) -> PlaceView {
    serde_json::from_value(json!({ "id": name, "kind": "provider", "name": name, "default": false })).expect("a provider row")
}

fn computer(name: &str) -> PlaceView {
    serde_json::from_value(json!({ "id": name, "kind": "computer", "name": name, "default": true })).expect("a computer row")
}

/// What the cloud named `name` is called to a person.
fn said_of(name: &str) -> String {
    place_said(&PlaceRef { id: name.to_string(), name: name.to_string(), kind: Some("provider".into()) })
}

fn with_clouds(host: &FakeHost, clouds: Option<Vec<&str>>) {
    let mut doc = json!({ "harness": "claude" });
    if let Some(c) = clouds {
        doc["clouds"] = json!(c);
    }
    host.add_record("t1", "w1", thread_record(doc));
}

// L4, L6: cloud-notice.test.ts:20
#[test]
fn a_thread_with_no_record_is_told_nothing() {
    let host = FakeHost::new();
    let mut prompt = "hello".to_string();
    let held = clouds_told(Some(vec![provider("box")]), &host.records, "t1", &mut prompt);
    assert_eq!(prompt, "hello");
    assert_eq!(held, vec![said_of("box")], "the clouds held are answered whatever the record says");
    assert_eq!(host.records.borrow().len(), 0, "the record is not written here");
}

// L4: cloud-notice.test.ts:20
#[test]
fn a_record_from_before_the_clouds_were_kept_is_told_nothing_and_starts_keeping_them() {
    let host = FakeHost::new();
    with_clouds(&host, None);
    let before = host.records.borrow().clone();
    let mut prompt = "hello".to_string();
    let held = clouds_told(Some(vec![provider("box")]), &host.records, "t1", &mut prompt);
    assert_eq!(prompt, "hello");
    assert_eq!(held, vec![said_of("box")], "the caller writes these into the record once the turn launched");
    assert_eq!(*host.records.borrow(), before, "this call writes nothing");
}

// L5: cloud-notice.test.ts:34
#[test]
fn added_then_removed_clouds_lead_the_prompt() {
    let host = FakeHost::new();
    with_clouds(&host, Some(vec![]));
    let mut prompt = "can you spawn a cloud thread to run the test suite?".to_string();
    let held = clouds_told(Some(vec![computer("mac"), provider("box")]), &host.records, "t1", &mut prompt);
    assert_eq!(prompt, format!("{}\n\ncan you spawn a cloud thread to run the test suite?", cloud_added_line(&said_of("box"))));
    assert_eq!(held, vec![said_of("box")]);
    // The record took the held cloud once its turn launched; the cloud is then taken away (cloud-notice.test.ts:38).
    with_clouds(&host, Some(vec![&said_of("box")]));
    let mut prompt = "is it still there?".to_string();
    let held = clouds_told(Some(vec![computer("mac")]), &host.records, "t1", &mut prompt);
    assert_eq!(prompt, format!("{}\n\nis it still there?", cloud_removed_line(&said_of("box"))));
    assert_eq!(held, Vec::<String>::new());
}

// L5: launch.ts:221
#[test]
fn added_lines_come_in_held_order_then_removed_lines_in_the_records_order() {
    let host = FakeHost::new();
    let box_said = said_of("box");
    with_clouds(&host, Some(vec!["zed", "gone", &box_said]));
    let mut prompt = "go".to_string();
    let held = clouds_told(Some(vec![provider("solari"), provider("box")]), &host.records, "t1", &mut prompt);
    let lines = [cloud_added_line(&said_of("solari")), cloud_removed_line("zed"), cloud_removed_line("gone")];
    assert_eq!(prompt, format!("{}\n\ngo", lines.join("\n")));
    assert_eq!(held, vec![said_of("box"), said_of("solari")]);
}

// L5: cloud-notice.test.ts:35
#[test]
fn nothing_changed_leaves_the_prompt() {
    let host = FakeHost::new();
    let box_said = said_of("box");
    with_clouds(&host, Some(vec![&box_said]));
    let mut prompt = "and again".to_string();
    let held = clouds_told(Some(vec![provider("box")]), &host.records, "t1", &mut prompt);
    assert_eq!(prompt, "and again", "no blank lines added where there is nothing to say");
    assert_eq!(held, vec![box_said]);
}

// L3: launch.ts:221
#[test]
fn held_clouds_are_sorted() {
    let host = FakeHost::new();
    let mut prompt = String::new();
    let rows = vec![provider("solari"), computer("mac"), provider("alpha"), provider("box")];
    let held = clouds_told(Some(rows), &host.records, "t1", &mut prompt);
    assert_eq!(held, vec![said_of("box"), said_of("solari"), said_of("alpha")], "by code unit, so an upper case name leads a lower case one, and a computer is no cloud");
}

// L3: launch.ts:221
#[test]
fn a_host_with_no_door_holds_no_clouds_and_says_every_recorded_one_gone() {
    let host = FakeHost::new();
    with_clouds(&host, Some(vec!["Boat"]));
    let mut prompt = "hi".to_string();
    let held = clouds_told(None, &host.records, "t1", &mut prompt);
    assert_eq!(held, Vec::<String>::new());
    assert_eq!(prompt, format!("{}\n\nhi", cloud_removed_line("Boat")));
}

// ---------------------------------------------------------------------------------------------------------------
// launch_picks

fn catalog_json() -> Value {
    json!({
        "harness": "claude", "label": "Claude Code", "source": "harness", "version": "2.1.0",
        "models": [
            { "value": "claude-opus-5", "label": "Opus 5", "isDefault": true, "efforts": ["low", "high", "max"], "contextWindows": ["1m"] },
            { "value": "claude-sonnet-5", "label": "Sonnet 5", "efforts": ["low", "high", "max"], "contextWindows": [] },
        ],
        "efforts": [{ "value": "low", "label": "low" }, { "value": "high", "label": "high", "isDefault": true }, { "value": "max", "label": "max" }],
        "contextWindows": [{ "value": "1m", "label": "1M" }],
        "permissionModes": [
            { "value": "default", "label": "default" },
            { "value": "acceptEdits", "label": "acceptEdits" },
            { "value": "bypassPermissions", "label": "bypassPermissions", "isDefault": true },
        ],
        "steers": false, "renames": false, "images": false,
    })
}

fn catalog_over(over: Value) -> HarnessCatalog {
    let mut whole = catalog_json();
    for (k, v) in over.as_object().expect("over is an object") {
        whole[k] = v.clone();
    }
    serde_json::from_value(whole).expect("a catalog the fixture builds")
}

fn catalog() -> HarnessCatalog {
    catalog_over(json!({}))
}

fn set_catalog(host: &FakeHost, c: Option<HarnessCatalog>) {
    host.hooks.set::<dyn Fn(&str, &Entry, &dyn HarnessAdapter) -> LocalBoxFuture<'static, Result<Option<HarnessCatalog>, HostError>>>(
        "LaunchCtx::catalog_on",
        Rc::new(move |_h: &str, _e: &Entry, _a: &dyn HarnessAdapter| {
            let c = c.clone();
            async move { Ok(c) }.boxed_local()
        }),
    );
}

fn set_ran(host: &FakeHost, ran: Rc<RefCell<RanPicks>>) {
    host.hooks.set::<dyn Fn(&str, &str, Option<&str>) -> RanPicks>("LaunchCtx::ran_on", Rc::new(move |_w: &str, _t: &str, _s: Option<&str>| ran.borrow().clone()));
}

fn ran(model: Option<&str>, effort: Option<&str>, window: Option<&str>) -> RanPicks {
    RanPicks { model: model.map(String::from), effort: effort.map(String::from), context_window: window.map(String::from), permission_mode: None }
}

fn picks(model: Option<&str>, effort: Option<&str>, mode: Option<&str>) -> StartPicks {
    StartPicks { model: model.map(String::from), effort: effort.map(String::from), permission_mode: mode.map(String::from), fast: None }
}

fn launch_picks_of(p: StartPicks, window: Option<&str>) -> LaunchPicks {
    LaunchPicks { picks: p, context_window: window.map(String::from) }
}

/// A host holding workspace w1 whose catalog is `catalog()`, and the area over it.
fn picks_rig() -> (Rc<FakeHost>, LaunchArea, Entry) {
    let host = FakeHost::new();
    let e = host.add_entry(entry("w1", json!({})));
    set_catalog(&host, Some(catalog()));
    let area = LaunchArea::new(host.clone());
    (host, area, e)
}

async fn read_picks(area: &LaunchArea, e: &Entry, asked: &LaunchAsked) -> Result<PicksRead, HostError> {
    let adapter = FakeAdapter::warming();
    area.launch_picks(e, "claude", &adapter, &default_preferences(), "t1", asked).await
}

// L8, L9: thread-defaults.test.ts:111
#[test]
fn picks_named_win_over_the_threads_own() {
    run_local(async {
        let (host, area, e) = picks_rig();
        set_ran(&host, Rc::new(RefCell::new(ran(Some("claude-opus-5"), Some("low"), None))));
        host.hooks.set::<dyn Fn(&str, &str, Option<&str>) -> Option<String>>("LaunchCtx::access_of", Rc::new(|_w: &str, _t: &str, _s: Option<&str>| Some("default".to_string())));
        host.hooks.set::<dyn Fn(Option<&HarnessCatalog>, &str, &AccessChoice) -> Option<String>>(
            "LaunchCtx::named_mode",
            Rc::new(|_c: Option<&HarnessCatalog>, _h: &str, a: &AccessChoice| (*a == AccessChoice::Full).then(|| "bypassPermissions".to_string())),
        );
        // A permission mode named is the access, and the access word is never read.
        let read = read_picks(&area, &e, &LaunchAsked { permission_mode: Some("acceptEdits".into()), access: Some(AccessChoice::Full), ..Default::default() }).await.unwrap();
        assert_eq!(read.catalog, Some(catalog()));
        assert_eq!((read.picks_for)(Some("sess-1")).unwrap(), launch_picks_of(picks(Some("claude-opus-5"), Some("low"), Some("acceptEdits")), None));
        assert_eq!(host.hooks.called("named_mode"), Vec::<String>::new());
        assert_eq!(host.hooks.called("access_of"), Vec::<String>::new(), "a named access leaves the thread's own unread");
        // An access word is named through the catalog.
        let read = read_picks(&area, &e, &LaunchAsked { access: Some(AccessChoice::Full), ..Default::default() }).await.unwrap();
        assert_eq!((read.picks_for)(Some("sess-1")).unwrap(), launch_picks_of(picks(Some("claude-opus-5"), Some("low"), Some("bypassPermissions")), None));
        assert_eq!(host.hooks.called("named_mode"), vec!["named_mode(claude)".to_string()]);
        // A model named wins over the thread's, and the thread's effort rides where the new model takes it.
        let read = read_picks(&area, &e, &LaunchAsked { model: Some("claude-sonnet-5".into()), ..Default::default() }).await.unwrap();
        assert_eq!((read.picks_for)(Some("sess-1")).unwrap(), launch_picks_of(picks(Some("claude-sonnet-5"), Some("low"), Some("default")), None));
    });
}

// L9: thread-defaults.test.ts:111
#[test]
fn a_threads_own_access_applies_only_where_the_harness_lists_it() {
    run_local(async {
        let (host, area, e) = picks_rig();
        let own = Rc::new(RefCell::new(Some("acceptEdits".to_string())));
        let o = own.clone();
        host.hooks.set::<dyn Fn(&str, &str, Option<&str>) -> Option<String>>("LaunchCtx::access_of", Rc::new(move |_w: &str, _t: &str, _s: Option<&str>| o.borrow().clone()));
        let read = read_picks(&area, &e, &LaunchAsked::default()).await.unwrap();
        assert_eq!((read.picks_for)(Some("sess-1")).unwrap(), launch_picks_of(picks(None, None, Some("acceptEdits")), None));
        assert_eq!(host.hooks.called("access_of"), vec![r#"access_of(w1, t1, Some("sess-1"))"#.to_string()]);
        // A mode this harness does not take is a pick that does not apply here, not a refusal.
        *own.borrow_mut() = Some("plan".to_string());
        assert_eq!((read.picks_for)(Some("sess-1")).unwrap(), launch_picks_of(StartPicks::default(), None));
    });
}

// L7, L9: thread-defaults.test.ts:111
#[test]
fn a_thread_with_none_opens_on_the_defaults() {
    run_local(async {
        let (host, area, e) = picks_rig();
        // The defaults put the agent's own model, effort and access on the catalog: sonnet at low, access ask.
        let marked = catalog_over(json!({
            "permissionModes": [
                { "value": "default", "label": "default", "isDefault": true },
                { "value": "acceptEdits", "label": "acceptEdits" },
                { "value": "bypassPermissions", "label": "bypassPermissions" },
            ],
        }));
        let resolved = Resolved { catalog: Some(marked.clone()), open_model: Some("claude-sonnet-5".into()), open_effort: Some("low".into()) };
        host.hooks.set::<dyn Fn(HarnessCatalog, &Preferences, &str) -> Resolved>("LaunchCtx::defaults_on", Rc::new(move |_c: HarnessCatalog, _p: &Preferences, _project: &str| resolved.clone()));
        let read = read_picks(&area, &e, &LaunchAsked::default()).await.unwrap();
        assert_eq!(read.catalog, Some(marked), "the catalog the defaults marked, not the one the harness answered");
        assert_eq!((read.picks_for)(None).unwrap(), launch_picks_of(picks(Some("claude-sonnet-5"), Some("low"), Some("default")), None));
        assert_eq!(host.hooks.called("defaults_on"), vec!["defaults_on(p1)".to_string()], "read against the workspace's project");
        // A pick the start names is over all of them.
        let read = read_picks(&area, &e, &LaunchAsked { model: Some("claude-opus-5".into()), permission_mode: Some("bypassPermissions".into()), ..Default::default() }).await.unwrap();
        assert_eq!((read.picks_for)(None).unwrap(), launch_picks_of(picks(Some("claude-opus-5"), Some("low"), Some("bypassPermissions")), None));
    });
}

// L9: thread-keeps-picks.test.ts:140
#[test]
fn a_new_thread_with_no_defaults_opens_on_the_catalogs_marks() {
    run_local(async {
        let (host, area, e) = picks_rig();
        let read = read_picks(&area, &e, &LaunchAsked::default()).await.unwrap();
        assert_eq!((read.picks_for)(None).unwrap(), launch_picks_of(picks(Some("claude-opus-5"), Some("high"), Some("bypassPermissions")), None));
        assert_eq!(host.hooks.called("resumed_fact"), Vec::<String>::new(), "a new session resumes nothing");
    });
}

// L9, L11: thread-keeps-picks.test.ts:82
#[test]
fn a_thread_keeps_the_model_effort_and_window_its_turns_ran_on() {
    run_local(async {
        let (host, area, e) = picks_rig();
        set_ran(&host, Rc::new(RefCell::new(ran(Some("claude-opus-5"), Some("low"), Some("1m")))));
        let read = read_picks(&area, &e, &LaunchAsked::default()).await.unwrap();
        assert_eq!((read.picks_for)(Some("sess-1")).unwrap(), launch_picks_of(picks(Some("claude-opus-5"), Some("low"), None), Some("1m")));
        assert_eq!(host.hooks.called("ran_on"), vec![r#"ran_on(w1, t1, Some("sess-1"))"#.to_string()]);
        assert_eq!(host.hooks.called("resumed_fact"), vec!["resumed_fact(w1, sess-1, model)".to_string()]);
    });
}

// L9, L11: thread-keeps-picks.test.ts:92
#[test]
fn what_a_send_names_wins_and_the_effort_it_leaves_out_stays_where_the_new_model_takes_it() {
    run_local(async {
        let (host, area, e) = picks_rig();
        set_ran(&host, Rc::new(RefCell::new(ran(Some("claude-opus-5"), Some("low"), Some("1m")))));
        let read = read_picks(&area, &e, &LaunchAsked { effort: Some("max".into()), ..Default::default() }).await.unwrap();
        assert_eq!((read.picks_for)(Some("sess-1")).unwrap(), launch_picks_of(picks(Some("claude-opus-5"), Some("max"), None), Some("1m")));
        // Sonnet 5 takes the effort but offers no window, so the window does not ride with it.
        set_ran(&host, Rc::new(RefCell::new(ran(Some("claude-opus-5"), Some("max"), Some("1m")))));
        let read = read_picks(&area, &e, &LaunchAsked { model: Some("claude-sonnet-5".into()), ..Default::default() }).await.unwrap();
        assert_eq!((read.picks_for)(Some("sess-1")).unwrap(), launch_picks_of(picks(Some("claude-sonnet-5"), Some("max"), None), None));
        // A window the send names is the window kept.
        let read = read_picks(&area, &e, &LaunchAsked { context_window: Some("1m".into()), ..Default::default() }).await.unwrap();
        assert_eq!((read.picks_for)(Some("sess-1")).unwrap(), launch_picks_of(picks(Some("claude-opus-5"), Some("max"), None), Some("1m")));
    });
}

// L9: thread-keeps-picks.test.ts:132
#[test]
fn a_model_the_catalog_dropped_is_left_out_rather_than_refused_and_the_effort_still_rides() {
    run_local(async {
        let (host, area, e) = picks_rig();
        set_ran(&host, Rc::new(RefCell::new(ran(Some("claude-opus-6-preview"), Some("low"), None))));
        let read = read_picks(&area, &e, &LaunchAsked::default()).await.unwrap();
        assert_eq!((read.picks_for)(Some("sess-1")).unwrap(), launch_picks_of(picks(None, Some("low"), None), None));
    });
}

// L9: thread-defaults.test.ts:111
#[test]
fn a_resume_never_takes_the_defaults_open_picks() {
    run_local(async {
        let (host, area, e) = picks_rig();
        let resolved = Resolved { catalog: Some(catalog()), open_model: Some("claude-sonnet-5".into()), open_effort: Some("low".into()) };
        host.hooks.set::<dyn Fn(HarnessCatalog, &Preferences, &str) -> Resolved>("LaunchCtx::defaults_on", Rc::new(move |_c: HarnessCatalog, _p: &Preferences, _project: &str| resolved.clone()));
        let on = Rc::new(RefCell::new(RanPicks::default()));
        set_ran(&host, on.clone());
        let read = read_picks(&area, &e, &LaunchAsked::default()).await.unwrap();
        assert_eq!((read.picks_for)(Some("sess-1")).unwrap(), launch_picks_of(StartPicks::default(), None), "a resume opens on nothing");
        // A new session on a thread that already ran a model opens on that model, and the catalog's mark for the effort.
        *on.borrow_mut() = ran(Some("claude-opus-5"), None, None);
        assert_eq!((read.picks_for)(None).unwrap(), launch_picks_of(picks(Some("claude-opus-5"), Some("high"), Some("bypassPermissions")), None));
        // One that ran only an effort keeps it, and the model is the catalog's mark, not the defaults' open one.
        *on.borrow_mut() = ran(None, Some("max"), None);
        assert_eq!((read.picks_for)(None).unwrap(), launch_picks_of(picks(Some("claude-opus-5"), Some("max"), Some("bypassPermissions")), None));
    });
}

// L9: thread-defaults.test.ts:123
#[test]
fn an_open_catalog_takes_any_model_named_and_starts_on_the_one_set_for_it() {
    run_local(async {
        let (host, area, e) = picks_rig();
        let open = catalog_over(json!({
            "harness": "opencode", "models": [], "efforts": [], "contextWindows": [],
            "permissionModes": [{ "value": "auto", "label": "auto", "isDefault": true }],
        }));
        set_catalog(&host, Some(open.clone()));
        let resolved = Resolved { catalog: Some(open), open_model: Some("anthropic/claude-sonnet-5".into()), open_effort: None };
        host.hooks.set::<dyn Fn(HarnessCatalog, &Preferences, &str) -> Resolved>("LaunchCtx::defaults_on", Rc::new(move |_c: HarnessCatalog, _p: &Preferences, _project: &str| resolved.clone()));
        let read = read_picks(&area, &e, &LaunchAsked::default()).await.unwrap();
        assert_eq!((read.picks_for)(None).unwrap(), launch_picks_of(picks(Some("anthropic/claude-sonnet-5"), None, Some("auto")), None));
        let read = read_picks(&area, &e, &LaunchAsked { model: Some("openai/gpt-x".into()), ..Default::default() }).await.unwrap();
        assert_eq!((read.picks_for)(None).unwrap(), launch_picks_of(picks(Some("openai/gpt-x"), None, Some("auto")), None));
    });
}

// L10: thread-defaults.test.ts:133
#[test]
fn a_refused_model_reads_the_not_run_words() {
    run_local(async {
        let (host, area, e) = picks_rig();
        let asked = LaunchAsked { model: Some("claude-nope".into()), ..Default::default() };
        let refusal = |cat: &HarnessCatalog| start_picks(Some(cat), &picks(Some("claude-nope"), None, None), true, None).unwrap_err();
        // Where nothing says the model once ran, the start's own refusal stands.
        let read = read_picks(&area, &e, &asked).await.unwrap();
        let got = (read.picks_for)(None).unwrap_err();
        let own = refusal(&catalog());
        assert_eq!(said(&got), (own.message, None, Some(Kind::Usage)));
        assert_eq!(host.hooks.called("not_run_refused"), vec![r#"not_run_refused(Some("claude-nope"))"#.to_string()]);
        // Where the runtime has words for a model that is not run, they replace it.
        let words = err("claude-nope is not run on this computer", Some(Kind::Usage));
        let w = words.clone();
        host.hooks.set::<dyn Fn(Option<&HarnessCatalog>, Option<&str>) -> Option<HostError>>("LaunchCtx::not_run_refused", Rc::new(move |_c: Option<&HarnessCatalog>, _m: Option<&str>| Some(w.clone())));
        let got = (read.picks_for)(None).unwrap_err();
        assert_eq!(said(&got), said(&words));
    });
}

// L7, L8: launch.ts:237
#[test]
fn no_table_no_catalog() {
    run_local(async {
        let host = FakeHost::new();
        let e = host.add_entry(entry("w1", json!({})));
        let area = LaunchArea::new(host.clone());
        set_ran(&host, Rc::new(RefCell::new(ran(Some("ran-model"), Some("ran-effort"), None))));
        let read = read_picks(&area, &e, &LaunchAsked::default()).await.unwrap();
        assert_eq!(read.catalog, None);
        // The thread's own latest picks stand for whatever the send leaves out, with nothing to check them against.
        assert_eq!((read.picks_for)(Some("sess-1")).unwrap(), launch_picks_of(picks(Some("ran-model"), Some("ran-effort"), None), None));
        let asked = LaunchAsked { model: Some("m1".into()), effort: Some("e1".into()), permission_mode: Some("p1".into()), context_window: Some("cw".into()), fast: Some(true), ..Default::default() };
        let read = read_picks(&area, &e, &asked).await.unwrap();
        let got = (read.picks_for)(None).unwrap();
        assert_eq!(got, LaunchPicks { picks: StartPicks { model: Some("m1".into()), effort: Some("e1".into()), permission_mode: Some("p1".into()), fast: Some(true) }, context_window: Some("cw".into()) });
        assert_eq!(host.hooks.called("defaults_on"), Vec::<String>::new());
        assert_eq!(host.hooks.called("access_of"), Vec::<String>::new());
    });
}

// ---------------------------------------------------------------------------------------------------------------
// launch_fixed

fn fixed_rig() -> (Rc<FakeHost>, LaunchArea, Entry) {
    let host = FakeHost::new();
    let e = host.add_entry(entry("w1", json!({})));
    let area = LaunchArea::new(host.clone());
    (host, area, e)
}

// L12: launch.ts:268
#[test]
fn the_fixed_key_writes_only_present_keys_in_order() {
    let (host, area, e) = fixed_rig();
    assert_eq!(area.launch_fixed(&e, "claude", "/work/w1", None, None), r#"{"harness":"claude","cwd":"/work/w1"}"#);
    assert_eq!(area.launch_fixed(&e, "claude", "/work/w1", None, Some(None)), r#"{"harness":"claude","cwd":"/work/w1","version":null}"#);
    assert_eq!(area.launch_fixed(&e, "claude", "/work/w1", None, Some(Some("2.1.0"))), r#"{"harness":"claude","cwd":"/work/w1","version":"2.1.0"}"#);
    // A place whose setup says nothing for the agent writes no setup key.
    host.hooks.set::<dyn Fn(&Entry) -> Option<String>>("LaunchCtx::setup_place", Rc::new(|_e: &Entry| Some("here".to_string())));
    assert_eq!(area.launch_fixed(&e, "claude", "/work/w1", None, None), r#"{"harness":"claude","cwd":"/work/w1"}"#);
    assert_eq!(host.hooks.called("launch_of"), vec!["launch_of(here, claude)".to_string()]);
}

// L12: launch.ts:268
#[test]
fn the_fixed_key_carries_the_servers_the_version_and_the_setup_whole() {
    let (host, area, e) = fixed_rig();
    host.hooks.set::<dyn Fn(&Entry) -> Option<String>>("LaunchCtx::setup_place", Rc::new(|_e: &Entry| Some("here".to_string())));
    host.hooks.set::<dyn Fn(&str, &str) -> Option<AgentLaunch>>(
        "LaunchCtx::launch_of",
        Rc::new(|_p: &str, _h: &str| Some(AgentLaunch { program: Some("/opt/claude".into()), args: Some(vec!["--debug".into()]) })),
    );
    let servers = json!({ "wsp": { "command": "/bin/wsp", "args": ["mcp"] } });
    assert_eq!(
        area.launch_fixed(&e, "claude", "/work/w1", Some(&servers), Some(Some("2.1.0"))),
        r#"{"harness":"claude","cwd":"/work/w1","mcpServers":{"wsp":{"command":"/bin/wsp","args":["mcp"]}},"version":"2.1.0","setup":{"program":"/opt/claude","args":["--debug"]}}"#
    );
    host.hooks.set::<dyn Fn(&str, &str) -> Option<AgentLaunch>>("LaunchCtx::launch_of", Rc::new(|_p: &str, _h: &str| Some(AgentLaunch { program: None, args: Some(vec!["--debug".into()]) })));
    assert_eq!(area.launch_fixed(&e, "codex", "/c", None, Some(None)), r#"{"harness":"codex","cwd":"/c","version":null,"setup":{"args":["--debug"]}}"#);
}

// ---------------------------------------------------------------------------------------------------------------
// warm

#[derive(Clone, Debug, PartialEq)]
struct Warmed {
    cwd: Option<String>,
    model: Option<String>,
    effort: Option<String>,
    permission_mode: Option<String>,
    context_window: Option<String>,
    fast: Option<bool>,
    mcp_servers: Option<Value>,
    server_values: Option<ServerValues>,
    version: Option<String>,
}

/// An adapter whose `warm` road records what it was handed and answers a kept agent named `kept-<n>`.
struct FakeAdapter {
    warms: bool,
    mcp: bool,
    denies: bool,
    fail: Option<String>,
    log: Mutex<Vec<Warmed>>,
}

impl FakeAdapter {
    fn warming() -> FakeAdapter {
        FakeAdapter { warms: true, mcp: false, denies: false, fail: None, log: Mutex::new(Vec::new()) }
    }
    fn warmed(&self) -> Vec<Warmed> {
        self.log.lock().unwrap().clone()
    }
}

impl HarnessAdapter for FakeAdapter {
    fn start(&self, _options: HarnessStartOptions) -> Result<Arc<dyn HarnessSession>, AdapterError> {
        Err(AdapterError("the warm tests never start a turn".into()))
    }
    fn warm(&self, o: HarnessWarmOptions) -> Option<Result<Box<dyn KeptAgent>, AdapterError>> {
        if !self.warms {
            return None;
        }
        let mut log = self.log.lock().unwrap();
        log.push(Warmed {
            cwd: o.cwd,
            model: o.model,
            effort: o.effort,
            permission_mode: o.permission_mode,
            context_window: o.context_window,
            fast: o.fast,
            mcp_servers: o.mcp_servers.map(|m| serde_json::to_value(m).expect("servers serialize")),
            server_values: o.server_values,
            version: o.version,
        });
        if let Some(why) = &self.fail {
            return Some(Err(AdapterError(why.clone())));
        }
        Some(Ok(Box::new(FakeKept { tag: format!("kept-{}", log.len()) })))
    }
    fn steers(&self) -> bool {
        false
    }
    fn mcp_servers(&self) -> bool {
        self.mcp
    }
    fn denies_servers(&self) -> bool {
        self.denies
    }
}

/// A kept agent that names itself through its session file's folder.
struct FakeKept {
    tag: String,
}

impl KeptAgent for FakeKept {
    fn next(&self, _turn: KeptTurn) -> Result<Arc<dyn HarnessSession>, AdapterError> {
        Err(AdapterError("the warm tests never take a turn".into()))
    }
    fn close(&self, _now: bool) -> BoxFuture<'static, ()> {
        futures::future::ready(()).boxed()
    }
    fn exited(&self) -> SharedFuture<()> {
        futures::future::ready(()).boxed().shared()
    }
    fn session_file(&self) -> Option<SessionFile> {
        Some(SessionFile { folder: self.tag.clone(), name: String::new(), depth: 0 })
    }
}

fn kept_tag(h: &WarmHold) -> String {
    h.agent.session_file().expect("a fake kept agent").folder
}

type Standing = Rc<RefCell<IndexMap<(String, String), (String, KeptLaunch)>>>;

/// The host with workspace w1, the fake adapter on every `adapter_for`, and the keeper's one road the area reads and
/// writes: `hold_warm` stands an agent per workspace and agent, `reap_kept` takes one away.
struct Rig {
    host: Rc<FakeHost>,
    area: LaunchArea,
    adapter: Arc<FakeAdapter>,
    asks: Rc<RefCell<Vec<(String, AdapterAsk)>>>,
    held: Rc<RefCell<Vec<(String, WarmHold)>>>,
    standing: Standing,
}

fn rig() -> Rig {
    rig_with(FakeAdapter::warming())
}

fn rig_with(adapter: FakeAdapter) -> Rig {
    let host = FakeHost::new();
    host.add_entry(entry("w1", json!({})));
    *host.ids.uuids.borrow_mut() = (1..=8).map(|n| format!("thr-{n}")).collect();
    let adapter = Arc::new(adapter);
    let asks: Rc<RefCell<Vec<(String, AdapterAsk)>>> = Rc::new(RefCell::new(Vec::new()));
    let held: Rc<RefCell<Vec<(String, WarmHold)>>> = Rc::new(RefCell::new(Vec::new()));
    let standing: Standing = Rc::new(RefCell::new(IndexMap::new()));
    let (a, log) = (adapter.clone(), asks.clone());
    host.hooks.set::<dyn Fn(&Entry, &str, AdapterAsk) -> Result<BuiltAdapter, HostError>>(
        "LaunchCtx::adapter_for",
        Rc::new(move |_e: &Entry, h: &str, ask: AdapterAsk| {
            log.borrow_mut().push((h.to_string(), ask));
            Ok(BuiltAdapter { harness: h.to_string(), adapter: a.clone() })
        }),
    );
    let warms = adapter.warms;
    host.hooks.set::<dyn Fn(&dyn HarnessAdapter) -> bool>("LaunchCtx::adapter_warms", Rc::new(move |_a: &dyn HarnessAdapter| warms));
    let (st, hd) = (standing.clone(), held.clone());
    host.hooks.set::<dyn Fn(&str, WarmHold) -> ()>(
        "LaunchCtx::hold_warm",
        Rc::new(move |t: &str, hold: WarmHold| {
            st.borrow_mut().insert((hold.workspace_id.clone(), hold.harness.clone()), (t.to_string(), hold.launch.clone()));
            hd.borrow_mut().push((t.to_string(), hold));
        }),
    );
    let st = standing.clone();
    host.hooks.set::<dyn Fn(&str, &str) -> Option<(String, KeptLaunch)>>(
        "LaunchCtx::warm_on",
        Rc::new(move |w: &str, h: &str| st.borrow().get(&(w.to_string(), h.to_string())).cloned()),
    );
    let st = standing.clone();
    host.hooks.set::<dyn Fn(&str) -> ()>("LaunchCtx::reap_kept", Rc::new(move |t: &str| st.borrow_mut().retain(|_, v| v.0 != t)));
    let area = LaunchArea::new(host.clone());
    Rig { host, area, adapter, asks, held, standing }
}

fn ask() -> WarmAsk {
    WarmAsk { harness: Some("claude".into()), ..Default::default() }
}

fn ask_with(model: Option<&str>, effort: Option<&str>, mode: Option<&str>) -> WarmAsk {
    WarmAsk { model: model.map(String::from), effort: effort.map(String::from), permission_mode: mode.map(String::from), ..ask() }
}

async fn warm(r: &Rig, a: WarmAsk) -> Result<SessionWarmResult, HostError> {
    r.area.warm("w1", a, None).await
}

fn thread_scope() -> Caller {
    let by: ThreadScope = serde_json::from_value(json!({ "kind": "thread", "threadId": "t-lead", "workspaceId": "w1", "rootThreadId": "t-lead" })).expect("a scope");
    Caller::Scoped { origin: WorkspaceOrigin::Here, by }
}

/// A launch's picks as the held agent's `launch` writes them.
fn held_picks(r: &Rig, at: usize) -> Value {
    serde_json::to_value(&r.held.borrow()[at].1.launch.picks).unwrap()
}

// L13: thread-warm-agent.test.ts:197
#[test]
fn a_threads_token_warms_nothing() {
    run_local(async {
        let r = rig();
        assert_eq!(r.area.warm("w1", ask(), Some(thread_scope())).await.unwrap(), none());
        assert_eq!(r.host.hooks.calls(), vec!["ready".to_string()], "nothing is read for a thread's token");
        // The person's own road, with no thread behind it, is asked as usual.
        assert_eq!(r.area.warm("w1", ask(), Some(Caller::Road(WorkspaceOrigin::Here))).await.unwrap(), started());
        assert_eq!(r.held.borrow().len(), 1);
    });
}

// L14: launch.ts:316
#[test]
fn an_unknown_workspace_is_the_callers_refusal() {
    run_local(async {
        let r = rig();
        let got = r.area.warm("nope", ask(), None).await.unwrap_err();
        assert_eq!(said(&got), ("no workspace nope".to_string(), None, Some(Kind::NotFound)));
        assert_eq!(r.host.hooks.called("adapter_for"), Vec::<String>::new());
    });
}

// L14: launch.ts:320
#[test]
fn a_kind_that_keeps_no_agents_warms_nothing() {
    run_local(async {
        let r = rig();
        r.host.hooks.set::<dyn Fn(&Entry) -> bool>("LaunchCtx::keeps_agents", Rc::new(|_e: &Entry| false));
        assert_eq!(warm(&r, ask()).await.unwrap(), none());
        assert_eq!(r.host.hooks.called("preferences_held"), Vec::<String>::new(), "nothing more is read");
        assert_eq!(r.host.hooks.called("adapter_for"), Vec::<String>::new());
        assert_eq!(r.held.borrow().len(), 0);
    });
}

// L14: launch.ts:320
#[test]
fn a_kept_box_too_small_to_keep_one_warms_nothing_and_one_with_room_warms() {
    run_local(async {
        let r = rig();
        r.host.hooks.set::<dyn Fn(&str) -> Option<u32>>("LaunchCtx::kept_box_cap", Rc::new(|_w: &str| Some(0)));
        assert_eq!(warm(&r, ask()).await.unwrap(), none());
        assert_eq!(r.host.hooks.called("adapter_for"), Vec::<String>::new());
        assert_eq!(r.host.hooks.called("kept_box_cap"), vec!["kept_box_cap(w1)".to_string()]);
        r.host.hooks.set::<dyn Fn(&str) -> Option<u32>>("LaunchCtx::kept_box_cap", Rc::new(|_w: &str| Some(2)));
        assert_eq!(warm(&r, ask()).await.unwrap(), started());
    });
}

// L14: launch.ts:320
#[test]
fn a_closing_host_warms_nothing() {
    run_local(async {
        let r = rig();
        r.host.closing.set(true);
        assert_eq!(warm(&r, ask()).await.unwrap(), none());
        assert_eq!(r.host.hooks.called("adapter_for"), Vec::<String>::new());
        assert_eq!(r.held.borrow().len(), 0);
    });
}

// L15: launch.ts:323
#[test]
fn an_agent_turned_off_warms_nothing() {
    run_local(async {
        let r = rig();
        r.host.hooks.set::<dyn Fn(&Entry, &str) -> bool>("LaunchCtx::agent_off", Rc::new(|_e: &Entry, h: &str| h == "codex"));
        let asked = WarmAsk { harness: Some("codex".into()), ..Default::default() };
        assert_eq!(warm(&r, asked).await.unwrap(), none());
        assert_eq!(r.host.hooks.called("agent_off"), vec!["agent_off(codex)".to_string()]);
        assert_eq!(r.host.hooks.called("adapter_for"), Vec::<String>::new());
        assert_eq!(r.host.hooks.called("default_agent_of"), Vec::<String>::new(), "an agent named is not looked up");
        assert_eq!(warm(&r, ask()).await.unwrap(), started(), "another agent on the same computer warms");
    });
}

// L15: launch.ts:322
#[test]
fn the_default_agent_is_warmed_where_none_is_named_and_held_preferences_are_read_before_the_store() {
    run_local(async {
        let r = rig();
        let mut held = default_preferences();
        held.default_agent = Some("held-agent".into());
        r.host.hooks.set::<dyn Fn() -> Option<Preferences>>("LaunchCtx::preferences_held", Rc::new(move || Some(held.clone())));
        r.host.hooks.set::<dyn Fn(&Preferences, &Entry) -> String>("LaunchCtx::default_agent_of", Rc::new(|p: &Preferences, _e: &Entry| p.default_agent.clone().expect("a default agent")));
        assert_eq!(warm(&r, WarmAsk::default()).await.unwrap(), started());
        assert_eq!(r.asks.borrow().iter().map(|a| a.0.clone()).collect::<Vec<_>>(), vec!["held-agent".to_string(), "held-agent".to_string()]);
        assert_eq!(r.held.borrow()[0].1.harness, "held-agent");
        assert_eq!(r.host.hooks.called("preferences"), Vec::<String>::new(), "the store is not read where the host holds them");
        // Where it holds none, the stored ones are read.
        let r = rig();
        let mut stored = default_preferences();
        stored.default_agent = Some("stored-agent".into());
        r.host.hooks.set::<dyn Fn() -> LocalBoxFuture<'static, Result<Preferences, HostError>>>(
            "LaunchCtx::preferences",
            Rc::new(move || {
                let p = stored.clone();
                async move { Ok(p) }.boxed_local()
            }),
        );
        r.host.hooks.set::<dyn Fn(&Preferences, &Entry) -> String>("LaunchCtx::default_agent_of", Rc::new(|p: &Preferences, _e: &Entry| p.default_agent.clone().expect("a default agent")));
        assert_eq!(warm(&r, WarmAsk::default()).await.unwrap(), started());
        assert_eq!(r.host.hooks.called("preferences"), vec!["preferences".to_string()]);
        assert_eq!(r.held.borrow()[0].1.harness, "stored-agent");
    });
}

// L17: launch.ts:276
#[test]
fn an_adapter_with_no_warm_road_answers_none() {
    run_local(async {
        let r = rig_with(FakeAdapter { warms: false, ..FakeAdapter::warming() });
        assert_eq!(warm(&r, ask()).await.unwrap(), none());
        let asks = r.asks.borrow();
        assert_eq!(asks.len(), 1, "only the probe is built");
        assert_eq!(asks[0].0, "claude");
        assert_eq!(asks[0].1.env.len(), 0, "a probe has no env");
        assert_eq!(asks[0].1.thread_id.as_deref(), Some("thr-1"));
        for later in ["confine_setup", "catalog_on", "thread_launch", "hold_warm"] {
            assert_eq!(r.host.hooks.called(later), Vec::<String>::new(), "{later} comes after the warm road is found");
        }
    });
}

// L17: launch.ts:283
#[test]
fn a_missing_local_folder_answers_none() {
    run_local(async {
        let r = rig();
        r.host.hooks.set::<dyn Fn(&str) -> bool>("LaunchCtx::folder_exists", Rc::new(|p: &str| p != "/work/w1"));
        assert_eq!(warm(&r, ask()).await.unwrap(), none());
        assert_eq!(r.host.hooks.called("folder_exists"), vec!["folder_exists(/work/w1)".to_string()]);
        assert_eq!(r.host.hooks.called("thread_launch"), Vec::<String>::new());
        assert_eq!(r.held.borrow().len(), 0);
        // The folder the start names is the one looked for.
        let asked = WarmAsk { cwd: Some("/proj/sub".into()), ..ask() };
        assert_eq!(warm(&r, asked).await.unwrap(), started());
        assert_eq!(r.host.hooks.called("thread_folder"), vec!["thread_folder(None)".to_string(), r#"thread_folder(Some("/proj/sub"))"#.to_string()]);
        assert_eq!(r.adapter.warmed()[0].cwd.as_deref(), Some("/proj/sub"));
    });
}

// L17: launch.ts:283
#[test]
fn a_cloud_workspace_never_checks_this_computers_disk() {
    run_local(async {
        let r = rig();
        r.host.add_entry(entry("w2", json!({ "kind": "cloud" })));
        r.host.hooks.set::<dyn Fn(&str) -> bool>("LaunchCtx::folder_exists", Rc::new(|_p: &str| false));
        assert_eq!(r.area.warm("w2", ask(), None).await.unwrap(), started());
        assert_eq!(r.host.hooks.called("folder_exists"), Vec::<String>::new());
        assert_eq!(r.adapter.warmed()[0].cwd.as_deref(), Some("/work/w2"));
    });
}

// L17, L19: thread-warm-agent.test.ts:82
#[test]
fn the_warm_launch_runs_on_the_catalogs_marks_and_is_held_for_the_threads_first_send() {
    run_local(async {
        let r = rig();
        set_catalog(&r.host, Some(catalog()));
        assert_eq!(warm(&r, ask()).await.unwrap(), started());
        // The process comes up on the model the catalog marks, as the composer's first send would run on.
        let w = r.adapter.warmed();
        assert_eq!(w.len(), 1);
        assert_eq!((w[0].model.as_deref(), w[0].effort.as_deref(), w[0].permission_mode.as_deref()), (Some("claude-opus-5"), Some("high"), Some("bypassPermissions")));
        let held = r.held.borrow();
        assert_eq!(held.len(), 1);
        assert_eq!(held[0].0, "thr-1");
        assert_eq!(held[0].1.workspace_id, "w1");
        assert_eq!(held[0].1.harness, "claude");
        assert_eq!(kept_tag(&held[0].1), "kept-1");
        assert_eq!(serde_json::to_value(&held[0].1.launch.picks).unwrap(), json!({ "model": "claude-opus-5", "effort": "high", "permissionMode": "bypassPermissions" }));
        assert_eq!(held[0].1.launch.fixed, r#"{"harness":"claude","cwd":"/work/w1","version":"2.1.0"}"#);
    });
}

// L19: launch.ts:296
#[test]
fn the_warm_launch_carries_the_token_thread_and_servers_in_order() {
    run_local(async {
        let r = rig_with(FakeAdapter { mcp: true, ..FakeAdapter::warming() });
        set_catalog(&r.host, Some(catalog()));
        let wsp = McpServerSpec { command: "/bin/wsp".into(), args: vec!["mcp".into()], no_slate: None };
        let minted = ThreadLaunch {
            scoped_device_id: Some("dev-1".into()),
            env: IndexMap::from([("WSP_HOST".to_string(), "h1".to_string()), ("WSP_X".to_string(), "x".to_string())]),
            wsp: Some(wsp),
        };
        r.host.hooks.set::<dyn Fn(&Entry, &str, &str, bool) -> LocalBoxFuture<'static, Result<ThreadLaunch, HostError>>>(
            "LaunchCtx::thread_launch",
            Rc::new(move |_e: &Entry, _t: &str, _r: &str, _a: bool| {
                let m = minted.clone();
                async move { Ok(m) }.boxed_local()
            }),
        );
        let vault = ServerValues { entries: Some(IndexMap::from([("vault".to_string(), IndexMap::from([("url".to_string(), json!("http://v"))]))])), config: None };
        let v = vault.clone();
        r.host.hooks.set::<dyn Fn() -> ServerValues>("LaunchCtx::vault_server_values", Rc::new(move || v.clone()));
        let launch = ServerValues { entries: Some(IndexMap::from([("launch".to_string(), IndexMap::from([("url".to_string(), json!("http://l"))]))])), config: Some(IndexMap::from([("K".to_string(), "v".to_string())])) };
        let l = LaunchServers { server_values: Some(launch.clone()), off_servers: Some(vec!["gone".into()]) };
        r.host.hooks.set::<dyn Fn(&Entry, &str, &dyn HarnessAdapter, &str) -> LocalBoxFuture<'static, Result<LaunchServers, HostError>>>(
            "LaunchCtx::launch_servers_for",
            Rc::new(move |_e: &Entry, _h: &str, _a: &dyn HarnessAdapter, _c: &str| {
                let l = l.clone();
                async move { Ok(l) }.boxed_local()
            }),
        );
        assert_eq!(warm(&r, ask()).await.unwrap(), started());
        // The token is 16 bytes drawn from Ids, in hex; the first draw is the first byte string.
        let token = "01".repeat(16);
        let asks = r.asks.borrow();
        assert_eq!(asks.len(), 2);
        assert_eq!(asks[0].1.env.len(), 0, "the probe has no env");
        assert_eq!(asks[0].1.thread_id.as_deref(), Some("thr-1"));
        let env: Vec<(String, String)> = asks[1].1.env.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        assert_eq!(
            env,
            vec![
                (TURN_TOKEN_ENV.to_string(), token.clone()),
                (THREAD_ENV.to_string(), "thr-1".to_string()),
                ("WSP_HOST".to_string(), "h1".to_string()),
                ("WSP_X".to_string(), "x".to_string()),
            ]
        );
        assert_eq!(asks[1].1.server_values, Some(vault));
        assert_eq!(asks[1].1.thread_id.as_deref(), Some("thr-1"));
        assert!(!asks[1].1.asks_until_stopped);
        let waiting = asks[1].1.waiting.clone().expect("the adapter reads the waiting box");
        let w = r.adapter.warmed();
        assert_eq!(
            w,
            vec![Warmed {
                cwd: Some("/work/w1".into()),
                model: Some("claude-opus-5".into()),
                effort: Some("high".into()),
                permission_mode: Some("bypassPermissions".into()),
                context_window: None,
                fast: None,
                mcp_servers: Some(json!({ "wsp": { "command": "/bin/wsp", "args": ["mcp"] } })),
                server_values: Some(launch),
                version: Some("2.1.0".into()),
            }]
        );
        let held = r.held.borrow();
        assert_eq!(held[0].0, "thr-1");
        let h = &held[0].1;
        assert_eq!((h.workspace_id.as_str(), h.harness.as_str(), h.turn_token.as_str(), h.scope_device_id.as_deref()), ("w1", "claude", token.as_str(), Some("dev-1")));
        assert!(Rc::ptr_eq(&h.waiting, &waiting), "the held agent shares the box the adapter reads");
        assert!(!h.waiting.get());
        assert_eq!(r.host.hooks.called("thread_launch"), vec!["thread_launch(thr-1, thr-1, false)".to_string()]);
        assert_eq!(r.host.hooks.called("launch_servers_for"), vec!["launch_servers_for(claude, /work/w1)".to_string()]);
        in_order(
            &r.host.hooks.calls(),
            &["ready", "entry_of(w1)", "adapter_for(claude)", "confine_setup(claude)", "catalog_on(claude)", "thread_launch(thr-1, thr-1, false)", "adapter_for(claude)", "launch_servers_for(claude, /work/w1)", "hold_warm(thr-1)"],
        );
    });
}

// L19: launch.ts:300
#[test]
fn an_agent_that_takes_no_servers_is_handed_none_and_asked_for_none() {
    run_local(async {
        let r = rig();
        let wsp = McpServerSpec { command: "/bin/wsp".into(), args: vec!["mcp".into()], no_slate: None };
        r.host.hooks.set::<dyn Fn(&Entry, &str, &str, bool) -> LocalBoxFuture<'static, Result<ThreadLaunch, HostError>>>(
            "LaunchCtx::thread_launch",
            Rc::new(move |_e: &Entry, _t: &str, _r: &str, _a: bool| {
                let m = ThreadLaunch { wsp: Some(wsp.clone()), ..Default::default() };
                async move { Ok(m) }.boxed_local()
            }),
        );
        assert_eq!(warm(&r, ask()).await.unwrap(), started());
        let w = r.adapter.warmed();
        assert_eq!((w[0].mcp_servers.clone(), w[0].server_values.clone()), (None, None));
        assert_eq!(r.host.hooks.called("launch_servers_for"), Vec::<String>::new());
    });
}

// L19: launch.ts:301
#[test]
fn an_agent_that_denies_servers_is_asked_for_the_launch_servers_but_handed_no_wsp_server() {
    run_local(async {
        let r = rig_with(FakeAdapter { denies: true, ..FakeAdapter::warming() });
        let wsp = McpServerSpec { command: "/bin/wsp".into(), args: vec!["mcp".into()], no_slate: None };
        r.host.hooks.set::<dyn Fn(&Entry, &str, &str, bool) -> LocalBoxFuture<'static, Result<ThreadLaunch, HostError>>>(
            "LaunchCtx::thread_launch",
            Rc::new(move |_e: &Entry, _t: &str, _r: &str, _a: bool| {
                let m = ThreadLaunch { wsp: Some(wsp.clone()), ..Default::default() };
                async move { Ok(m) }.boxed_local()
            }),
        );
        let launch = ServerValues { entries: None, config: Some(IndexMap::from([("K".to_string(), "v".to_string())])) };
        let l = LaunchServers { server_values: Some(launch.clone()), off_servers: None };
        r.host.hooks.set::<dyn Fn(&Entry, &str, &dyn HarnessAdapter, &str) -> LocalBoxFuture<'static, Result<LaunchServers, HostError>>>(
            "LaunchCtx::launch_servers_for",
            Rc::new(move |_e: &Entry, _h: &str, _a: &dyn HarnessAdapter, _c: &str| {
                let l = l.clone();
                async move { Ok(l) }.boxed_local()
            }),
        );
        assert_eq!(warm(&r, ask()).await.unwrap(), started());
        let w = r.adapter.warmed();
        assert_eq!((w[0].mcp_servers.clone(), w[0].server_values.clone()), (None, Some(launch)));
        assert_eq!(r.host.hooks.called("launch_servers_for"), vec!["launch_servers_for(claude, /work/w1)".to_string()]);
    });
}

// L19: launch.ts:307
#[test]
fn the_version_rides_only_from_a_harness_catalog_that_has_one_and_always_marks_the_fixed_key() {
    run_local(async {
        let cases: [(Option<HarnessCatalog>, Option<&str>, &str); 4] = [
            (None, None, r#"{"harness":"claude","cwd":"/work/w1"}"#),
            (Some(catalog_over(json!({ "version": null }))), None, r#"{"harness":"claude","cwd":"/work/w1","version":null}"#),
            (Some(catalog_over(json!({ "source": "table", "version": "9.9.9" }))), None, r#"{"harness":"claude","cwd":"/work/w1","version":"9.9.9"}"#),
            (Some(catalog_over(json!({ "version": "2.1.0" }))), Some("2.1.0"), r#"{"harness":"claude","cwd":"/work/w1","version":"2.1.0"}"#),
        ];
        for (cat, rides, fixed) in cases {
            let r = rig();
            set_catalog(&r.host, cat);
            assert_eq!(warm(&r, ask()).await.unwrap(), started());
            assert_eq!(r.adapter.warmed()[0].version.as_deref(), rides, "{fixed}");
            assert_eq!(r.held.borrow()[0].1.launch.fixed, fixed);
        }
    });
}

// L11, L19: launch.ts:249
#[test]
fn the_window_kept_rides_with_the_warm_picks() {
    run_local(async {
        let r = rig();
        set_catalog(&r.host, Some(catalog()));
        let asked = WarmAsk { context_window: Some("1m".into()), model: Some("claude-opus-5".into()), ..ask() };
        assert_eq!(warm(&r, asked).await.unwrap(), started());
        assert_eq!(r.adapter.warmed()[0].context_window.as_deref(), Some("1m"));
        assert_eq!(held_picks(&r, 0)["model"], "claude-opus-5");
    });
}

// L20: launch.ts:310
#[test]
fn a_failed_warm_revokes_its_device() {
    run_local(async {
        let r = rig_with(FakeAdapter { fail: Some("the agent would not start".into()), ..FakeAdapter::warming() });
        r.host.hooks.set::<dyn Fn(&Entry, &str, &str, bool) -> LocalBoxFuture<'static, Result<ThreadLaunch, HostError>>>(
            "LaunchCtx::thread_launch",
            Rc::new(|_e: &Entry, _t: &str, _r: &str, _a: bool| async move { Ok(ThreadLaunch { scoped_device_id: Some("dev-1".into()), ..Default::default() }) }.boxed_local()),
        );
        let got = warm(&r, ask()).await.unwrap_err();
        assert_eq!(got.message, "the agent would not start", "the failure is the caller's");
        assert_eq!(r.host.hooks.called("revoke_device"), vec!["revoke_device(dev-1)".to_string()]);
        assert_eq!(r.held.borrow().len(), 0);
        in_order(&r.host.hooks.calls(), &["thread_launch(thr-1, thr-1, false)", "revoke_device(dev-1)"]);
    });
}

// L20: launch.ts:310
#[test]
fn a_failure_building_the_launch_adapter_revokes_too_and_the_revokes_own_failure_is_swallowed() {
    run_local(async {
        let r = rig();
        r.host.hooks.set::<dyn Fn(&Entry, &str, &str, bool) -> LocalBoxFuture<'static, Result<ThreadLaunch, HostError>>>(
            "LaunchCtx::thread_launch",
            Rc::new(|_e: &Entry, _t: &str, _r: &str, _a: bool| async move { Ok(ThreadLaunch { scoped_device_id: Some("dev-1".into()), ..Default::default() }) }.boxed_local()),
        );
        let calls = Rc::new(Cell::new(0));
        let (c, a) = (calls.clone(), r.adapter.clone());
        let refused = err("no such agent here", Some(Kind::Usage));
        let rf = refused.clone();
        r.host.hooks.set::<dyn Fn(&Entry, &str, AdapterAsk) -> Result<BuiltAdapter, HostError>>(
            "LaunchCtx::adapter_for",
            Rc::new(move |_e: &Entry, h: &str, _ask: AdapterAsk| {
                c.set(c.get() + 1);
                if c.get() == 2 {
                    return Err(rf.clone());
                }
                Ok(BuiltAdapter { harness: h.to_string(), adapter: a.clone() })
            }),
        );
        r.host.hooks.set::<dyn Fn(&str) -> LocalBoxFuture<'static, Result<(), HostError>>>("LaunchCtx::revoke_device", Rc::new(|_d: &str| async move { Err(err("the device is gone already", None)) }.boxed_local()));
        let got = warm(&r, ask()).await.unwrap_err();
        assert_eq!(said(&got), said(&refused));
        assert_eq!(r.host.hooks.called("revoke_device"), vec!["revoke_device(dev-1)".to_string()]);
    });
}

// L20: launch.ts:297
#[test]
fn a_failure_before_a_device_was_minted_revokes_nothing() {
    run_local(async {
        let r = rig();
        r.host.hooks.set::<dyn Fn(&Entry, &str, &str, bool) -> LocalBoxFuture<'static, Result<ThreadLaunch, HostError>>>(
            "LaunchCtx::thread_launch",
            Rc::new(|_e: &Entry, _t: &str, _r: &str, _a: bool| async move { Err(err("could not mint a device", None)) }.boxed_local()),
        );
        let got = warm(&r, ask()).await.unwrap_err();
        assert_eq!(got.message, "could not mint a device");
        assert_eq!(r.host.hooks.called("revoke_device"), Vec::<String>::new());
        // A launch that minted no scoped device has none to revoke when the warm road fails.
        let r = rig_with(FakeAdapter { fail: Some("boom".into()), ..FakeAdapter::warming() });
        assert_eq!(warm(&r, ask()).await.unwrap_err().message, "boom");
        assert_eq!(r.host.hooks.called("revoke_device"), Vec::<String>::new());
    });
}

// L18: thread-warm-agent.test.ts:124
#[test]
fn a_standing_agent_at_the_same_launch_is_rewarmed() {
    run_local(async {
        let r = rig();
        assert_eq!(warm(&r, ask()).await.unwrap(), started());
        // Asked a minute apart for five minutes, as the composer does while the person types.
        for _ in 0..5 {
            r.host.clock.advance(60_000);
            assert_eq!(warm(&r, ask()).await.unwrap(), standing());
        }
        assert_eq!(r.host.hooks.called("rewarm"), vec!["rewarm(thr-1)".to_string(); 5], "each ask starts the window over");
        assert_eq!(r.held.borrow().len(), 1, "one process was held, the one the first ask started");
        assert_eq!(r.adapter.warmed().len(), 1);
        assert_eq!(r.host.hooks.called("reap_kept"), Vec::<String>::new());
        assert_eq!(r.host.hooks.called("thread_launch").len(), 1, "no device minted for a standing one");
        assert_eq!(r.host.ids.drawn.get(), 1, "the only bytes drawn were the first ask's token");
        in_order(&r.host.hooks.calls(), &["warm_on(w1, claude)", "rewarm(thr-1)"]);
    });
}

// L18: thread-warm-agent.test.ts:100
#[test]
fn an_agent_that_ended_in_its_window_is_not_standing_and_the_next_ask_starts_one() {
    run_local(async {
        let r = rig();
        assert_eq!(warm(&r, ask()).await.unwrap(), started());
        // The keeper's three minutes went by: it ended the agent and no longer stands one.
        r.standing.borrow_mut().clear();
        assert_eq!(warm(&r, ask()).await.unwrap(), started());
        let held = r.held.borrow();
        assert_eq!(held.iter().map(|h| h.0.clone()).collect::<Vec<_>>(), vec!["thr-1".to_string(), "thr-2".to_string()]);
        assert_eq!(held.iter().map(|h| kept_tag(&h.1)).collect::<Vec<_>>(), vec!["kept-1".to_string(), "kept-2".to_string()]);
        assert_eq!(r.host.hooks.called("rewarm"), Vec::<String>::new());
        assert_eq!(r.host.hooks.called("reap_kept"), Vec::<String>::new(), "nothing stood to reap");
    });
}

// L18: thread-warm-agent.test.ts:141
#[test]
fn a_changed_pick_reaps_the_standing_one_and_starts_another() {
    run_local(async {
        let r = rig();
        assert_eq!(warm(&r, ask()).await.unwrap(), started());
        assert_eq!(warm(&r, ask_with(Some("claude-sonnet-5"), None, None)).await.unwrap(), started());
        assert_eq!(warm(&r, ask_with(Some("claude-sonnet-5"), Some("low"), None)).await.unwrap(), started());
        assert_eq!(warm(&r, ask_with(Some("claude-sonnet-5"), Some("low"), Some("acceptEdits"))).await.unwrap(), started());
        assert_eq!(r.host.hooks.called("reap_kept"), vec!["reap_kept(thr-1)".to_string(), "reap_kept(thr-2)".to_string(), "reap_kept(thr-3)".to_string()]);
        assert_eq!(held_picks(&r, 0), json!({}));
        assert_eq!(held_picks(&r, 1), json!({ "model": "claude-sonnet-5" }));
        assert_eq!(held_picks(&r, 2), json!({ "model": "claude-sonnet-5", "effort": "low" }));
        assert_eq!(held_picks(&r, 3), json!({ "model": "claude-sonnet-5", "effort": "low", "permissionMode": "acceptEdits" }));
        let w = r.adapter.warmed();
        assert_eq!((w[1].model.as_deref(), w[2].effort.as_deref(), w[3].permission_mode.as_deref()), (Some("claude-sonnet-5"), Some("low"), Some("acceptEdits")));
        assert_eq!(r.host.hooks.called("rewarm"), Vec::<String>::new());
        // The one standing is the last, and each ask reaped the one before it before it started its own.
        in_order(&r.host.hooks.calls(), &["reap_kept(thr-1)", "thread_launch(thr-2, thr-2, false)", "hold_warm(thr-2)", "reap_kept(thr-2)", "thread_launch(thr-3, thr-3, false)"]);
        assert_eq!(r.standing.borrow().values().map(|v| v.0.clone()).collect::<Vec<_>>(), vec!["thr-4".to_string()]);
    });
}

// L18: thread-warm-agent.test.ts:174
#[test]
fn a_standing_agent_with_a_pick_the_ask_leaves_out_is_reaped_and_so_is_one_lacking_a_pick_the_ask_names() {
    run_local(async {
        let r = rig();
        // Fast never rides unasked: the process launched with it is not the launch of an ask that leaves it out.
        assert_eq!(warm(&r, WarmAsk { fast: Some(true), ..ask() }).await.unwrap(), started());
        assert_eq!(r.adapter.warmed()[0].fast, Some(true));
        assert_eq!(held_picks(&r, 0), json!({ "fast": true }));
        assert_eq!(warm(&r, ask()).await.unwrap(), started());
        assert_eq!(r.host.hooks.called("reap_kept"), vec!["reap_kept(thr-1)".to_string()]);
        assert_eq!(r.adapter.warmed()[1].fast, None);
        // And the other way: an ask naming a pick the standing one lacks.
        assert_eq!(warm(&r, WarmAsk { fast: Some(true), ..ask() }).await.unwrap(), started());
        assert_eq!(r.host.hooks.called("reap_kept"), vec!["reap_kept(thr-1)".to_string(), "reap_kept(thr-2)".to_string()]);
        assert_eq!(r.host.hooks.called("rewarm"), Vec::<String>::new());
    });
}

// L18: launch.ts:289
#[test]
fn a_different_folder_is_a_different_launch_and_reaps_the_standing_one() {
    run_local(async {
        let r = rig();
        assert_eq!(warm(&r, WarmAsk { cwd: Some("/a".into()), ..ask() }).await.unwrap(), started());
        assert_eq!(warm(&r, WarmAsk { cwd: Some("/a".into()), ..ask() }).await.unwrap(), standing());
        assert_eq!(warm(&r, WarmAsk { cwd: Some("/b".into()), ..ask() }).await.unwrap(), started());
        assert_eq!(r.host.hooks.called("reap_kept"), vec!["reap_kept(thr-1)".to_string()]);
        assert_eq!(r.held.borrow().iter().map(|h| h.1.launch.fixed.clone()).collect::<Vec<_>>(), vec![r#"{"harness":"claude","cwd":"/a"}"#.to_string(), r#"{"harness":"claude","cwd":"/b"}"#.to_string()]);
    });
}

// L18: thread-warm-agent.test.ts:252
#[test]
fn each_workspace_and_agent_keeps_its_own_standing_one() {
    run_local(async {
        let r = rig();
        for id in ["w2", "w3", "w4"] {
            r.host.add_entry(entry(id, json!({})));
        }
        for id in ["w1", "w2", "w3"] {
            assert_eq!(r.area.warm(id, ask(), None).await.unwrap(), started());
        }
        assert_eq!(r.host.hooks.called("warm_on"), vec!["warm_on(w1, claude)".to_string(), "warm_on(w2, claude)".to_string(), "warm_on(w3, claude)".to_string()]);
        // Asked for again, the second is standing; asking in a fourth folder reaps none of the three: how many stand is
        // the keeper's to hold.
        assert_eq!(r.area.warm("w2", ask(), None).await.unwrap(), standing());
        assert_eq!(r.area.warm("w4", ask(), None).await.unwrap(), started());
        assert_eq!(r.host.hooks.called("rewarm"), vec!["rewarm(thr-2)".to_string()]);
        assert_eq!(r.host.hooks.called("reap_kept"), Vec::<String>::new());
        assert_eq!(r.held.borrow().iter().map(|h| h.1.workspace_id.clone()).collect::<Vec<_>>(), vec!["w1".to_string(), "w2".to_string(), "w3".to_string(), "w4".to_string()]);
        // Another agent in a folder with one standing is no standing one for it.
        assert_eq!(r.area.warm("w1", WarmAsk { harness: Some("codex".into()), ..Default::default() }, None).await.unwrap(), started());
        assert_eq!(r.host.hooks.called("reap_kept"), Vec::<String>::new());
    });
}

/// The gates `confine_setup` waits at, one per call in order; a call with no gate left passes.
fn gate_confine_setup(host: &FakeHost, gates: Vec<futures::channel::oneshot::Receiver<Result<(), HostError>>>) {
    let queue = Rc::new(RefCell::new(VecDeque::from(gates)));
    host.hooks.set::<dyn Fn(&Entry, &str) -> LocalBoxFuture<'static, Result<(), HostError>>>(
        "LaunchCtx::confine_setup",
        Rc::new(move |_e: &Entry, _h: &str| {
            let gate = queue.borrow_mut().pop_front();
            async move {
                match gate {
                    Some(g) => g.await.expect("the gate is opened"),
                    None => Ok(()),
                }
            }
            .boxed_local()
        }),
    );
}

type Asked = tokio::task::JoinHandle<Result<SessionWarmResult, HostError>>;

fn ask_in_flight(r: &Rig, workspace: &'static str, a: WarmAsk) -> Asked {
    let area = r.area.clone();
    tokio::task::spawn_local(async move { area.warm(workspace, a, None).await })
}

// L16: thread-warm-agent.test.ts:124
#[test]
fn a_second_ask_waits_for_the_first_and_one_stands() {
    run_local(async {
        let r = rig();
        let (open, gate) = futures::channel::oneshot::channel();
        gate_confine_setup(&r.host, vec![gate]);
        let first = ask_in_flight(&r, "w1", ask());
        settle().await;
        let second = ask_in_flight(&r, "w1", ask());
        settle().await;
        assert_eq!(r.host.hooks.called("confine_setup").len(), 1, "the second has not begun while the first runs");
        assert_eq!(r.asks.borrow().len(), 1, "no adapter built for the second yet");
        open.send(Ok(())).unwrap();
        assert_eq!(first.await.unwrap().unwrap(), started());
        assert_eq!(second.await.unwrap().unwrap(), standing(), "the second reads the one the first started");
        assert_eq!(r.held.borrow().len(), 1);
        assert_eq!(r.host.hooks.called("rewarm"), vec!["rewarm(thr-1)".to_string()]);
        assert_eq!(r.adapter.warmed().len(), 1);
    });
}

// L16: launch.ts:326
#[test]
fn a_third_ask_waits_for_the_second_not_only_the_first() {
    run_local(async {
        let r = rig();
        let (open_one, gate_one) = futures::channel::oneshot::channel();
        let (open_two, gate_two) = futures::channel::oneshot::channel();
        gate_confine_setup(&r.host, vec![gate_one, gate_two]);
        let first = ask_in_flight(&r, "w1", ask());
        settle().await;
        let second = ask_in_flight(&r, "w1", ask());
        let third = ask_in_flight(&r, "w1", ask());
        settle().await;
        open_one.send(Ok(())).unwrap();
        assert_eq!(first.await.unwrap().unwrap(), started());
        settle().await;
        assert_eq!(r.host.hooks.called("confine_setup").len(), 2, "the second runs, the third still waits on it");
        open_two.send(Ok(())).unwrap();
        assert_eq!(second.await.unwrap().unwrap(), standing());
        assert_eq!(third.await.unwrap().unwrap(), standing());
        assert_eq!(r.host.hooks.called("confine_setup").len(), 3);
        assert_eq!(r.held.borrow().len(), 1);
        assert_eq!(r.host.hooks.called("rewarm"), vec!["rewarm(thr-1)".to_string(), "rewarm(thr-1)".to_string()]);
    });
}

// L16: launch.ts:326
#[test]
fn a_second_ask_runs_its_own_after_the_first_failed() {
    run_local(async {
        let r = rig();
        let (open, gate) = futures::channel::oneshot::channel();
        gate_confine_setup(&r.host, vec![gate]);
        let first = ask_in_flight(&r, "w1", ask());
        settle().await;
        let second = ask_in_flight(&r, "w1", ask());
        settle().await;
        let refused = err("the setup folder is gone", Some(Kind::Usage));
        open.send(Err(refused.clone())).unwrap();
        assert_eq!(said(&first.await.unwrap().unwrap_err()), said(&refused));
        assert_eq!(second.await.unwrap().unwrap(), started(), "the first's failure is not the second's");
        assert_eq!(r.held.borrow().len(), 1);
        // The key was dropped once the newest settled: a later ask is not held back.
        assert_eq!(warm(&r, ask()).await.unwrap(), standing());
    });
}

// L16: launch.ts:325
#[test]
fn an_ask_for_another_agent_or_workspace_does_not_wait() {
    run_local(async {
        let r = rig();
        r.host.add_entry(entry("w2", json!({})));
        let (open, gate) = futures::channel::oneshot::channel();
        gate_confine_setup(&r.host, vec![gate]);
        let first = ask_in_flight(&r, "w1", ask());
        settle().await;
        let other_agent = ask_in_flight(&r, "w1", WarmAsk { harness: Some("codex".into()), ..Default::default() });
        let other_workspace = ask_in_flight(&r, "w2", ask());
        assert_eq!(other_agent.await.unwrap().unwrap(), started());
        assert_eq!(other_workspace.await.unwrap().unwrap(), started());
        assert_eq!(r.held.borrow().iter().map(|h| (h.1.workspace_id.clone(), h.1.harness.clone())).collect::<Vec<_>>(), vec![("w1".to_string(), "codex".to_string()), ("w2".to_string(), "claude".to_string())]);
        open.send(Ok(())).unwrap();
        assert_eq!(first.await.unwrap().unwrap(), started());
    });
}

// ---------------------------------------------------------------------------------------------------------------
// seed_after_loss

const FIRST: &str = "01a1424a-7c2e-7d31-9f00-5b2a1c3d4e5f";
const SECOND: &str = "01a1424b-fa29-7aa0-8c11-0d9e8f7a6b5c";

fn lost_at() -> LostAt {
    LostAt { session_id: FIRST.into(), thread_id: SECOND.into(), workspace_id: "ws_a".into() }
}

/// A machine exec that says what the machine holds and keeps every command it was asked.
struct HeldExec {
    commands: Mutex<Vec<String>>,
}

impl HarnessExec for HeldExec {
    fn exec(&self, command: &str, _env: Option<&Env>) -> BoxFuture<'static, Result<String, ExecFailure>> {
        self.commands.lock().unwrap().push(command.to_string());
        futures::future::ready(Ok("boot b1; rollout files: none; index row: state_5.sqlite".to_string())).boxed()
    }
}

/// A seed that counts its asks and answers the thread so far.
fn counting_seed(asked: Rc<Cell<u32>>) -> Seed {
    Rc::new(move || {
        asked.set(asked.get() + 1);
        async { Ok("the thread so far".to_string()) }.boxed_local()
    })
}

/// A `session_held` road that reads the machine through the exec it is handed and keeps the sessions it was asked for.
fn reading(asked: Rc<RefCell<Vec<String>>>) -> SessionHeld {
    Rc::new(move |session: String, exec: Arc<dyn HarnessExec>| {
        asked.borrow_mut().push(session.clone());
        async move { exec.exec(&format!("held {session}"), None).await.map_err(|e| e.message) }.boxed_local()
    })
}

// L21: lost-session-line.test.ts:102
#[test]
fn logs_one_line_naming_what_the_machine_holds() {
    run_local(async {
        let lines = console::capture();
        let (sessions, seeds) = (Rc::new(RefCell::new(Vec::new())), Rc::new(Cell::new(0)));
        let exec = Arc::new(HeldExec { commands: Mutex::new(Vec::new()) });
        let seed = seed_after_loss(Some(reading(sessions.clone())), exec.clone(), lost_at(), counting_seed(seeds.clone()));
        assert_eq!(seed().await.unwrap(), "the thread so far");
        settle().await;
        let words = "boot b1; rollout files: none; index row: state_5.sqlite";
        assert_eq!(lines.lines(), vec![lost_session_log_line(FIRST, SECOND, "ws_a", words)]);
        assert_eq!(*sessions.borrow(), vec![FIRST.to_string()], "the read names the lost session");
        assert_eq!(*exec.commands.lock().unwrap(), vec![format!("held {FIRST}")], "and runs on the machine's exec it was handed");
        assert!(!lines.lines()[0].contains("the thread so far"), "the seed is never logged");
    });
}

// L21, L22: lost-session-line.test.ts:114
#[test]
fn hands_the_seed_over_and_logs_why_when_the_reading_throws() {
    run_local(async {
        let lines = console::capture();
        let held: SessionHeld = Rc::new(|_s: String, _e: Arc<dyn HarnessExec>| async { Err("threadId must be a plain slug".to_string()) }.boxed_local());
        let seeds = Rc::new(Cell::new(0));
        let seed = seed_after_loss(Some(held), Arc::new(NoExec), lost_at(), counting_seed(seeds.clone()));
        assert_eq!(seed().await.unwrap(), "the thread so far");
        settle().await;
        assert_eq!(lines.lines(), vec![lost_session_log_line(FIRST, SECOND, "ws_a", "not read: threadId must be a plain slug")]);
    });
}

// L22: launch.ts:343
#[test]
fn the_seed_is_handed_over_while_the_read_is_still_pending_and_its_own_failure_is_its_own() {
    run_local(async {
        let lines = console::capture();
        let (open, gate) = futures::channel::oneshot::channel::<()>();
        let gate = Rc::new(RefCell::new(Some(gate)));
        let held: SessionHeld = Rc::new(move |_s: String, _e: Arc<dyn HarnessExec>| {
            let g = gate.borrow_mut().take();
            async move {
                if let Some(g) = g {
                    let _ = g.await;
                }
                Ok("boot b2".to_string())
            }
            .boxed_local()
        });
        let seeds = Rc::new(Cell::new(0));
        let seed = seed_after_loss(Some(held), Arc::new(NoExec), lost_at(), counting_seed(seeds.clone()));
        assert_eq!(seed().await.unwrap(), "the thread so far", "the seed does not wait for the read");
        settle().await;
        assert_eq!(lines.lines(), Vec::<String>::new(), "nothing is said while the machine has not answered");
        open.send(()).unwrap();
        settle().await;
        assert_eq!(lines.lines(), vec![lost_session_log_line(FIRST, SECOND, "ws_a", "boot b2")]);
        // A seed that fails fails its caller as it did, and still the read happened once.
        let lines = console::capture();
        let failing: Seed = Rc::new(|| async { Err(err("the thread could not be read", None)) }.boxed_local());
        let sessions = Rc::new(RefCell::new(Vec::new()));
        let seed = seed_after_loss(Some(reading(sessions.clone())), Arc::new(HeldExec { commands: Mutex::new(Vec::new()) }), lost_at(), failing);
        assert_eq!(seed().await.unwrap_err().message, "the thread could not be read");
        settle().await;
        assert_eq!(lines.lines().len(), 1);
        assert_eq!(sessions.borrow().len(), 1);
    });
}

// L23: lost-session-line.test.ts:125
#[test]
fn logs_nothing_and_reads_nothing_without_session_held() {
    run_local(async {
        let lines = console::capture();
        let exec = Arc::new(HeldExec { commands: Mutex::new(Vec::new()) });
        let seeds = Rc::new(Cell::new(0));
        let seed = seed_after_loss(None, exec.clone(), lost_at(), counting_seed(seeds.clone()));
        assert_eq!(seed().await.unwrap(), "the thread so far");
        assert_eq!(seed().await.unwrap(), "the thread so far");
        settle().await;
        assert_eq!(lines.lines(), Vec::<String>::new());
        assert_eq!(*exec.commands.lock().unwrap(), Vec::<String>::new());
        assert_eq!(seeds.get(), 2, "every ask is answered by the seed");
    });
}

// L21, L22: launch.ts:343
#[test]
fn a_second_ask_reads_nothing_more() {
    run_local(async {
        let lines = console::capture();
        let (sessions, seeds) = (Rc::new(RefCell::new(Vec::new())), Rc::new(Cell::new(0)));
        let exec = Arc::new(HeldExec { commands: Mutex::new(Vec::new()) });
        let seed = seed_after_loss(Some(reading(sessions.clone())), exec.clone(), lost_at(), counting_seed(seeds.clone()));
        for _ in 0..3 {
            assert_eq!(seed().await.unwrap(), "the thread so far");
            settle().await;
        }
        assert_eq!(*sessions.borrow(), vec![FIRST.to_string()], "one read for the first ask");
        assert_eq!(lines.lines().len(), 1);
        assert_eq!(exec.commands.lock().unwrap().len(), 1);
        assert_eq!(seeds.get(), 3);
    });
}
