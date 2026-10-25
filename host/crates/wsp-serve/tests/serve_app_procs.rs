// SPDX-License-Identifier: AGPL-3.0-only
//! The app's processes through the host: the app's socket is told to read them only while a page watches, and each
//! list it sends reaches the pages watching and leaves with the socket that sent it. Several sockets on one
//! `Served`, `appProcs.*` over each, the pushes read off each sink. Spec: spec/serve_helpers.md, H1-H8.
mod support;
use support::*;

use std::rc::Rc;

use serde_json::{json, Value};
use wsp_serve::serve::door::Served;
use wsp_serve::serve::socket::Socket;
use wsp_serve::serve::ServeOptions;
use wsp_serve::serve_app_procs::APP_PROCS_UNOFFERED;

fn procs() -> Value {
    json!([
        { "pid": 501, "kind": "main" },
        { "pid": 502, "kind": "gpu" },
        { "pid": 503, "kind": "network" },
        { "pid": 504, "kind": "renderer" },
        { "pid": 505, "kind": "tab", "origins": ["https://example.com"] },
    ])
}

fn wanted(sink: &RecordingSink) -> Vec<Value> {
    sink.pushes().into_iter().filter(|p| p["type"] == "app.procs.wanted").map(|p| p["wanted"].clone()).collect()
}

fn lists(sink: &RecordingSink) -> Vec<Value> {
    sink.pushes().into_iter().filter(|p| p["type"] == "app.procs").map(|p| p["procs"].clone()).collect()
}

async fn client(served: &Rc<Served>) -> (Rc<Socket>, Rc<RecordingSink>) {
    host_socket(served).await
}

// H4-H7. serve-app-procs.test.ts:46
#[test]
fn app_procs_are_asked_for_while_a_page_watches() {
    block(async {
        let rt = FakeRuntime::new();
        let served = served_with(&rt, opts());
        let (app, app_sink) = client(&served).await;
        let offered = request(&app, &app_sink, 1, "appProcs.offer", json!({})).await;
        assert_eq!(offered, json!({ "id": 1, "ok": true, "wanted": false }));

        let (page, page_sink) = client(&served).await;
        let watched = request(&page, &page_sink, 1, "appProcs.watch", json!({})).await;
        assert_eq!(watched, json!({ "id": 1, "ok": true, "procs": [] }));
        assert_eq!(wanted(&app_sink), vec![json!(true)]);

        // A second pane on the same socket and a second page each ride the one ask.
        request(&page, &page_sink, 2, "appProcs.watch", json!({})).await;
        let (other, other_sink) = client(&served).await;
        request(&other, &other_sink, 1, "appProcs.watch", json!({})).await;
        assert_eq!(wanted(&app_sink), vec![json!(true)]);

        let set = request(&app, &app_sink, 2, "appProcs.set", json!({ "procs": procs() })).await;
        assert_eq!(set, json!({ "id": 2, "ok": true }));
        assert_eq!(lists(&page_sink), vec![procs()]);
        assert_eq!(lists(&other_sink), vec![procs()]);
        assert!(page_sink.pushes().iter().all(|p| p.get("id").is_none()), "a push carries no id");

        let left = request(&other, &other_sink, 2, "appProcs.unwatch", json!({})).await;
        assert_eq!(left, json!({ "id": 2, "ok": true }));
        page.closed();
        assert_eq!(wanted(&app_sink), vec![json!(true), json!(false)], "the app is told once the last page has gone");

        // What the app read for pages that have all gone is not handed to the next one.
        request(&app, &app_sink, 3, "appProcs.set", json!({ "procs": procs() })).await;
        let again = request(&other, &other_sink, 3, "appProcs.watch", json!({})).await;
        assert_eq!(again, json!({ "id": 3, "ok": true, "procs": [] }));
        assert_eq!(lists(&other_sink).len(), 1, "the list sent with no page watching reached nobody");
        assert_eq!(wanted(&app_sink), vec![json!(true), json!(false), json!(true)]);
    })
}

// H4, H8. serve-app-procs.test.ts:77
#[test]
fn app_procs_leave_with_the_socket_that_sent_them() {
    block(async {
        let rt = FakeRuntime::new();
        let served = served_with(&rt, opts());
        let (page, page_sink) = client(&served).await;
        request(&page, &page_sink, 1, "appProcs.watch", json!({})).await;
        let (app, app_sink) = client(&served).await;
        let unoffered = request(&app, &app_sink, 1, "appProcs.set", json!({ "procs": procs() })).await;
        assert_eq!(unoffered, json!({ "id": 1, "ok": false, "error": APP_PROCS_UNOFFERED }));
        let offered = request(&app, &app_sink, 2, "appProcs.offer", json!({})).await;
        assert_eq!(offered, json!({ "id": 2, "ok": true, "wanted": true }));
        request(&app, &app_sink, 3, "appProcs.set", json!({ "procs": procs() })).await;
        assert_eq!(lists(&page_sink), vec![procs()]);

        app.closed();
        assert_eq!(lists(&page_sink), vec![procs(), json!([])], "the pages are told the app's processes went with it");
        page.closed();
    })
}

// H8. serve-app-procs.test.ts:92. The schema refuses what is not the app's process kinds, at the door.
#[test]
fn app_procs_refuse_a_list_that_is_not_the_apps() {
    block(async {
        let rt = FakeRuntime::new();
        let (log, lines) = log_lines();
        let served = served_with(&rt, ServeOptions { log: Some(log), ..opts() });
        let (app, app_sink) = client(&served).await;
        request(&app, &app_sink, 1, "appProcs.offer", json!({})).await;
        let (page, page_sink) = client(&served).await;
        request(&page, &page_sink, 1, "appProcs.watch", json!({})).await;

        let agent = request_unchecked(&app, &app_sink, 2, "appProcs.set", json!({ "procs": [{ "pid": 7, "kind": "agent" }] })).await;
        assert_eq!(agent["ok"], json!(false));
        let long = request_unchecked(&app, &app_sink, 3, "appProcs.set", json!({ "procs": [{ "pid": 7, "kind": "tab", "origins": ["x".repeat(300)] }] })).await;
        assert_eq!(long["ok"], json!(false));
        assert!(lists(&page_sink).is_empty(), "a refused list reaches no page");
        assert_eq!(lines.borrow().len(), 2, "each refusal is logged once");
        assert!(lines.borrow().iter().all(|l| l.starts_with("refused appProcs.set kind=")));
    })
}

// H3. serve-app-procs.test.ts:102, through the door: a socket let in on a ticket is not the own road.
#[test]
fn the_hub_takes_nothing_off_the_own_road_through_a_socket() {
    block(async {
        let rt = FakeRuntime::new();
        let served = served_with(&rt, opts());
        let (host, host_sink) = client(&served).await;
        let ticket = request(&host, &host_sink, 1, "ticket.issue", json!({ "purpose": "relay" })).await;
        let (relayed, relayed_sink) = open(&served, &format!("?ticket={}", ticket["ticket"].as_str().expect("a ticket")));
        let offer = request(&relayed, &relayed_sink, 1, "appProcs.offer", json!({})).await;
        assert_eq!(offer["ok"], json!(false));
        assert_eq!(offer["error"], json!(wsp_serve::serve_app_procs::APP_PROCS_OFF_ROAD));
        let watch = request(&relayed, &relayed_sink, 2, "appProcs.watch", json!({})).await;
        assert_eq!(watch["error"], json!(wsp_serve::serve_app_procs::APP_PROCS_WATCH_OFF_ROAD));
        assert!(relayed_sink.pushes().is_empty());
    })
}

// H2. Two servers over one runtime meet in one hub: an app on one is asked for by a page on the other.
#[test]
fn two_servers_over_one_runtime_share_the_hub() {
    block(async {
        let rt = FakeRuntime::new();
        let first = served_with(&rt, opts());
        let second = served_with(&rt, opts());
        let (app, app_sink) = client(&first).await;
        request(&app, &app_sink, 1, "appProcs.offer", json!({})).await;
        let (page, page_sink) = client(&second).await;
        request(&page, &page_sink, 1, "appProcs.watch", json!({})).await;
        assert_eq!(wanted(&app_sink), vec![json!(true)]);
        let other_rt = FakeRuntime::new();
        let third = served_with(&other_rt, opts());
        let (stranger, stranger_sink) = client(&third).await;
        request(&stranger, &stranger_sink, 1, "appProcs.watch", json!({})).await;
        assert_eq!(wanted(&app_sink), vec![json!(true)], "another runtime's page asks nobody here");
    })
}
