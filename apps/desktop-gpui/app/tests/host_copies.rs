// SPDX-License-Identifier: AGPL-3.0-only
//! The host's session rows held once however many readers follow them: the sidebar, the menu bar's feed and Activity
//! each read `sessions.list` on a road of their own from a host started for this test alone, whose claude is the
//! bench's stub playing the session fixture's threads (bench/session.mjs, with subagents). Every row they read is the
//! one copy `shared` holds, and a settled thread keeps only what the sidebar draws of it: its opening words cut to
//! their title and no subagent's asked text, last line or failure. Opened, a settled thread's rows are read whole as
//! the sidebar's own while the rest stay shared, and left, they are cut and shared again. The only test in its binary,
//! so the rows `shared::held` counts are this test's alone.

#[path = "../../host-client/tests/common/mod.rs"]
mod common;

use std::collections::HashMap;
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};

use desktop_gpui::desktop::feed::{self, Fed, FeedState};
use desktop_gpui::screens::app_shell::title::opening_title;
use desktop_gpui::screens::settings::pages::activity::live::{self, Listing};
use desktop_gpui::screens::sidebar::model::Row;
use desktop_gpui::screens::sidebar::shared;
use desktop_gpui::sidebar::{Link, Sidebar};
use desktop_gpui::ui::{self, Side};
use futures::channel::mpsc;
use gpui_kit::{AppContext, TestAppContext, VisualTestContext};
use serde_json::{json, Map, Value};
use wsp_host_client::protocol::SessionsSettleRequest;
use wsp_host_client::HostClient;

const WAIT: Duration = Duration::from_secs(60);
/// The bench threads recorded; the first and the last are settled, and the first is opened.
const THREADS: usize = 3;

fn until(cx: &mut VisualTestContext, what: &str, mut done: impl FnMut(&mut VisualTestContext) -> bool) {
    let end = Instant::now() + WAIT;
    loop {
        cx.run_until_parked();
        if done(cx) {
            return;
        }
        assert!(Instant::now() < end, "{what} within {} s", WAIT.as_secs());
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// The prompts of a bench thread's turns, each with whether a subagent runs in it, as content.mjs makes them.
fn turns_of(host: &common::Throwaway, name: &str) -> Vec<(String, bool)> {
    let content = common::root().join("apps/desktop-gpui/bench/content.mjs");
    let script = "const { turnsOf } = await import(process.argv[1]); console.log(JSON.stringify(turnsOf(process.argv[2]).map(t => [t.prompt, t.tools.some(c => c.children !== undefined)])))";
    let out = Command::new("node")
        .args(["--input-type=module", "-e", script])
        .arg(content.to_string_lossy().as_ref())
        .arg(name)
        .env_clear()
        .envs(host.env())
        .current_dir(host.dir.path())
        .output()
        .unwrap();
    assert!(out.status.success(), "content.mjs: {}", String::from_utf8_lossy(&out.stderr));
    serde_json::from_slice(&out.stdout).unwrap()
}

/// Records the bench thread `name` on the host up to the first turn a subagent runs in, and answers its thread id.
fn record(host: &common::Throwaway, name: &str) -> String {
    let turns = turns_of(host, name);
    let last = turns.iter().position(|(_, subagent)| *subagent).unwrap_or_else(|| panic!("{name} runs no subagent"));
    let turn =
        |i: usize| std::fs::write(host.dir.path().join("bench-turn.json"), json!({ "thread": name, "turn": i }).to_string()).unwrap();
    turn(0);
    let said = host.wsp(&["run", "repo", "--agent", "claude", &turns[0].0]);
    let id = regex::Regex::new(r"\bthread ([0-9a-f-]{36})\b").unwrap();
    let thread = id.captures(&said).unwrap_or_else(|| panic!("wsp run named no thread: {said}"))[1].to_owned();
    for (i, (prompt, _)) in turns.iter().enumerate().take(last + 1).skip(1) {
        turn(i);
        host.wsp(&["send", &thread, prompt]);
    }
    thread
}

/// `sessions.list` as the host sent it, by row id.
fn list(runtime: &tokio::runtime::Runtime, client: &HostClient) -> HashMap<String, Value> {
    let sent = runtime.block_on(client.call_raw("sessions.list", Map::new())).unwrap_or_else(|f| panic!("{}", f.message));
    let Some(Value::Array(rows)) = sent.get("sessions").cloned() else { panic!("sessions.list answered no sessions: {sent:?}") };
    rows.into_iter().map(|r| (r["id"].as_str().expect("a row's id").to_owned(), r)).collect()
}

/// The feed's newest state and Activity's newest listing, taken off their channels.
fn drain(
    fed: &mut mpsc::UnboundedReceiver<Fed>,
    listed: &mut mpsc::UnboundedReceiver<Listing>,
    state: &mut FeedState,
    listing: &mut Listing,
) {
    while let Ok(next) = fed.try_recv() {
        if let Fed::State(next) = next {
            *state = *next;
        }
    }
    while let Ok(next) = listed.try_recv() {
        *listing = next;
    }
}

/// Whether `row` stands in the tree of `thread`, as the sidebar reads it.
fn in_tree(row: &Row, thread: &str) -> bool {
    [row.thread_id.as_deref(), row.root_thread_id.as_deref(), row.parent_thread_id.as_deref()].contains(&Some(thread))
}

/// Every row of `ids` is one copy across the three readers, else which reader holds its own.
fn one_copy<'a>(ids: impl IntoIterator<Item = &'a String>, sidebar: &[Arc<Row>], feed: &[Arc<Row>], activity: &[Arc<Row>], when: &str) {
    let (sidebar, feed, activity) = (by_id(sidebar), by_id(feed), by_id(activity));
    for id in ids {
        let at = |rows: &HashMap<&str, &Arc<Row>>, reader: &str| -> Arc<Row> {
            Arc::clone(rows.get(id.as_str()).unwrap_or_else(|| panic!("{reader} holds no row {id} {when}")))
        };
        let (s, f, a) = (at(&sidebar, "the sidebar"), at(&feed, "the feed"), at(&activity, "Activity"));
        let own = match (Arc::ptr_eq(&s, &f), Arc::ptr_eq(&s, &a), Arc::ptr_eq(&f, &a)) {
            (true, true, _) => continue,
            (true, false, _) => "Activity",
            (false, true, _) => "the feed",
            (false, false, true) => "the sidebar",
            (false, false, false) => "every reader",
        };
        panic!("{own} holds its own copy of row {id} {when}");
    }
}

fn by_id(rows: &[Arc<Row>]) -> HashMap<&str, &Arc<Row>> {
    rows.iter().map(|r| (r.id.as_str(), r)).collect()
}

/// Whether a subagent as the host sent it carries a field the sidebar never draws.
fn undrawn(subagent: &Value) -> bool {
    ["asked", "lastLine", "failure"].iter().any(|k| subagent.get(k).is_some_and(|v| !v.is_null()))
}

/// The three readers on one host hold one copy of each row between them, and a settled thread's rows hold its title
/// and its subagents' titles, states and times, not the words the host sends for its open view, except while that
/// thread is open in the sidebar.
#[gpui_kit::test]
fn the_readers_hold_one_copy_of_each_row_and_a_settled_row_keeps_only_what_the_sidebar_draws(cx: &mut TestAppContext) {
    let stub = common::root().join("apps/desktop-gpui/bench/claude-stub.mjs");
    let host = common::Throwaway::start_with(&format!("#!/bin/sh\nexec node '{}' \"$@\"\n", stub.display()), false);
    let folder = host.repo("repo");
    host.wsp(&["add", &folder.to_string_lossy()]);
    let threads: Vec<String> = (1..=THREADS).map(|i| record(&host, &format!("session-{i}"))).collect();

    let runtime = Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap());
    let client = Arc::new(runtime.block_on(host.client()));
    let open = threads[0].as_str();
    let settle: SessionsSettleRequest = common::ask(json!({ "threadIds": [open, threads[2]] }));
    common::answered("sessions.settle", runtime.block_on(client.call(settle)));
    let raw = list(&runtime, &client);
    let settled = |r: &Value| r.get("settledAt").is_some_and(|v| !v.is_null());
    let subagents = |r: &Value| r.get("subagents").and_then(Value::as_array).cloned().unwrap_or_default();
    assert!(raw.values().any(settled), "settling {open} left no row settled");
    assert!(
        raw.values().filter(|r| settled(r)).filter_map(|r| r.get("prompt").and_then(Value::as_str)).any(|p| opening_title(p) != p),
        "no settled row's prompt runs past its title, so the cut proves nothing: {raw:?}"
    );
    assert!(
        raw.values().filter(|r| settled(r)).flat_map(subagents).any(|s| undrawn(&s)),
        "no settled row's subagents carry asked text, a last line or a failure, so the cut proves nothing: {raw:?}"
    );

    cx.executor().allow_parking();
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
    });
    let (_, cx) = cx.add_window_view(|_, _| gpui_kit::EmptyView);
    let sidebar = cx.new(Sidebar::new);
    sidebar.update(cx, |s, cx| s.attach(Link { host: client.clone(), runtime: runtime.clone(), page: None }, cx));
    let (tx, mut fed) = mpsc::unbounded();
    let _feed = feed::follow(None, host.state(), host.env(), &runtime, feed::SETTLE, feed::RETRY, tx);
    let (tx, mut listed) = mpsc::unbounded();
    let _activity = live::follow_lists(client.clone(), &runtime, tx);

    let mut state = FeedState::default();
    let mut listing = Listing::default();
    until(cx, "the sidebar, the feed and Activity each reading every row", |cx| {
        drain(&mut fed, &mut listed, &mut state, &mut listing);
        let read = sidebar.read_with(cx, |s, _| s.rows().len());
        !state.lost && state.sessions.len() == raw.len() && listing.sessions.len() == raw.len() && read == raw.len()
    });

    let rows = sidebar.read_with(cx, |s, _| s.rows().to_vec());
    one_copy(raw.keys(), &rows, &state.sessions, &listing.sessions, "once every reader has read");
    let sidebar_rows = by_id(&rows);
    assert_eq!(shared::held(), raw.len(), "the rows held are not one for each of the host's {} rows", raw.len());

    let mut kept_asked = 0;
    for (id, sent) in &raw {
        let row = sidebar_rows[id.as_str()];
        if settled(sent) {
            let title = sent.get("prompt").and_then(Value::as_str).map(opening_title);
            assert_eq!(row.prompt, title, "settled row {id} keeps more of its prompt than its title");
            let undrawn: Vec<_> =
                row.subagents.iter().filter(|s| s.asked.is_some() || s.last_line.is_some() || s.failure.is_some()).map(|s| &s.id).collect();
            assert!(undrawn.is_empty(), "settled row {id} keeps asked text, a last line or a failure of subagents {undrawn:?}");
        } else {
            for s in subagents(sent).iter().filter(|s| s.get("asked").is_some_and(Value::is_string)) {
                let id_of = s["id"].as_str().unwrap_or_default();
                let kept = row.subagents.iter().find(|k| k.id == id_of).unwrap_or_else(|| panic!("row {id} lost subagent {id_of}"));
                assert_eq!(kept.asked.as_deref(), s["asked"].as_str(), "unsettled row {id} lost what subagent {id_of} was asked");
                kept_asked += 1;
            }
        }
    }
    assert!(kept_asked > 0, "no unsettled row's subagents were asked anything, so the cut was not shown to spare them");
    println!("{} rows held once by three readers; {kept_asked} subagents of unsettled rows keep what they were asked", raw.len());

    sidebar.update(cx, |s, cx| s.mark(open, cx)).unwrap_or_else(|| panic!("the sidebar lists no thread {open}"));
    let whole = |row: &Row| {
        let sent = &raw[&row.id];
        row.prompt.as_deref() == sent["prompt"].as_str()
            && subagents(sent)
                .iter()
                .all(|s| row.subagents.iter().any(|k| Some(k.id.as_str()) == s["id"].as_str() && k.asked.as_deref() == s["asked"].as_str()))
    };
    until(cx, "the open settled thread's rows read whole", |cx| {
        sidebar.read_with(cx, |s, _| s.rows().iter().filter(|r| in_tree(r, open)).all(|r| whole(r)))
    });
    let opened: Vec<Arc<Row>> = sidebar.read_with(cx, |s, _| s.rows().iter().filter(|r| in_tree(r, open)).cloned().collect());
    assert!(!opened.is_empty(), "the sidebar holds no row of the open thread {open}");
    for row in &opened {
        assert!(!shared::holds(row), "the open settled thread's row {} read whole is the copy every reader shares", row.id);
    }

    // A thread on a new workspace makes the feed read every row again; Activity and the sidebar take only pushes.
    let (feed_had, activity_had) = (state.workspaces.len(), listing.workspaces.len());
    let other = host.repo("other");
    host.wsp(&["add", &other.to_string_lossy()]);
    host.wsp(&["run", "other", "--agent", "claude", "elsewhere"]);
    until(cx, "the feed reading again and Activity taking the new workspace", |_| {
        drain(&mut fed, &mut listed, &mut state, &mut listing);
        state.workspaces.len() > feed_had && listing.workspaces.len() > activity_had
    });
    let rows = sidebar.read_with(cx, |s, _| s.rows().to_vec());
    let rest: Vec<&String> =
        raw.keys().filter(|id| !in_tree(rows.iter().find(|r| r.id == **id).expect("the sidebar's row"), open)).collect();
    assert!(
        rest.iter().any(|id| settled(&raw[*id])),
        "no settled row stands outside the open thread, so a whole read shared with the others would go unseen"
    );
    one_copy(rest, &rows, &state.sessions, &listing.sessions, "outside the open thread once the feed read again");

    sidebar.update(cx, |s, cx| s.mark(&threads[1], cx)).unwrap_or_else(|| panic!("the sidebar lists no thread {}", threads[1]));
    cx.run_until_parked();
    let rows = sidebar.read_with(cx, |s, _| s.rows().to_vec());
    one_copy(raw.keys(), &rows, &state.sessions, &listing.sessions, "once the settled thread was left");
    for row in rows.iter().filter(|r| in_tree(r, open)) {
        let title = raw[&row.id]["prompt"].as_str().map(opening_title);
        assert_eq!(row.prompt, title, "the settled thread's row {} left open keeps more of its prompt than its title", row.id);
    }
    println!("the open settled thread read {} rows whole as the sidebar's own, and left, shares them again", opened.len());

    // A new turn's row is pushed running and then done, so the readers pass through two states of it at their own pace.
    let name = "session-2";
    let turns = turns_of(&host, name);
    let next = turns.iter().position(|(_, subagent)| *subagent).expect("the subagent's turn recorded") + 1;
    std::fs::write(host.dir.path().join("bench-turn.json"), json!({ "thread": name, "turn": next }).to_string()).unwrap();
    host.wsp(&["send", &threads[1], &turns[next].0]);
    let after = list(&runtime, &client);
    let turn = after.values().find(|r| r["threadId"].as_str() == Some(threads[1].as_str())).expect("the new turn's row");
    assert_ne!(turn["status"], "running", "the new turn's row is not done: {turn}");
    assert_ne!(raw.get(turn["id"].as_str().unwrap_or_default()), Some(turn), "the new turn left its thread's row as it was");
    let rows_now: HashMap<&String, Row> = after.iter().filter_map(|(id, v)| Some((id, Row::owned(v.clone())?.kept()))).collect();
    until(cx, "the sidebar, the feed and Activity each holding the new turn's row done", |cx| {
        drain(&mut fed, &mut listed, &mut state, &mut listing);
        let reads = |rows: &[Arc<Row>]| rows_now.iter().all(|(id, want)| rows.iter().any(|r| r.id == **id && **r == *want));
        sidebar.read_with(cx, |s, _| reads(s.rows())) && reads(&state.sessions) && reads(&listing.sessions)
    });
    let rows = sidebar.read_with(cx, |s, _| s.rows().to_vec());
    one_copy(after.keys(), &rows, &state.sessions, &listing.sessions, "once a new turn was pushed running and then done");
    println!("{} rows held once by three readers after a new turn", after.len());
}
