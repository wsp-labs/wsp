// SPDX-License-Identifier: AGPL-3.0-only
//! packages/runtime/test/session-opening.test.ts at 65f6b89c7, case for case. A session row keeps the head of the message
//! its thread opened with, never the whole of it: a sub-thread's brief is tens of thousands of characters long, and every
//! row was held in the host, written whole to the session index at every save and sent to every window.
#[macro_use]
mod common;

use std::rc::Rc;

use common::*;
use serde_json::{json, Value};
use wsp_core::types::api::StartOptions;
use wsp_serve::serve::{serve_runtime, ServeOptions};
use wsp_store::store::Store;
use wsp_words::views::session::fold_threads;
use wsp_words::words::turn::OPENING_KEPT;

fn opening() -> String {
    format!("Build the ticket, and say what you find. {}", "Read every comment first, then the code it names. ".repeat(700))
}

fn quick() -> Rc<Scripted> {
    Scripted::new().session("22222222-2222-4222-8222-222222222222").when(When::Sync)
}

async fn index_rows(store: &Rc<dyn Store>, ws: &str) -> Vec<Value> {
    store.get("sessions", ws).await.expect("a read").and_then(|v| v["sessions"].as_array().cloned()).unwrap_or_default()
}

fn kept(prompt: Option<&str>) -> bool {
    prompt.is_some_and(|p| p.chars().count() <= OPENING_KEPT && opening().starts_with(p))
}

fn title_of(rows: &[Value]) -> String {
    let views: Vec<wsp_wire::views::session::SessionView> =
        rows.iter().map(|r| serde_json::from_value(r.clone()).expect("a row")).collect();
    j(&fold_threads(&views)[0])["title"].as_str().unwrap_or_default().to_string()
}

fn as_values<T: serde::Serialize>(rows: &[T]) -> Vec<Value> {
    rows.iter().map(j).collect()
}

// "the opening on a session row" > "is its first 600 characters in the host, the session index, every listing and every pushed row, and the title stands across a restart"
#[test]
fn is_its_first_600_characters_in_the_host_the_session_index_every_listing_and_every_pushed_row_and_the_title_stands_across_a_restart() {
    drive(|| async {
        assert!(opening().len() > 35_000);
        let state = sqlite_state();
        let backend = StubBackend::new(None);
        let a = quick();
        let fx = runtime(&backend).store(&state.store).adapter("claude", a.factory()).build();
        let heard = fx.events();
        let served = serve_runtime(fx.rt.clone(), ServeOptions { port: Some(0), auth_token: "host-token".into(), ..Default::default() })
            .await
            .expect("serves");
        let client = WsClient::connect(served.port(), "host-token").await;
        let ws = create_on(&fx.rt, CreateOn::golden("a"), None).await.expect("a workspace").view.id;

        fx.rt
            .sessions()
            .start(&ws, StartOptions { prompt: opening(), ..Default::default() }, None)
            .await
            .expect("a start")
            .finished()
            .await;
        let rows = as_values(&fx.rt.sessions().list(Some(ws.clone()), None).await.expect("a list"));
        assert_eq!(rows.iter().map(|r| kept(r["prompt"].as_str())).collect::<Vec<_>>(), [true]);
        let listed = client.request("sessions.list", json!({ "workspaceId": ws })).await;
        assert!(listed.to_string().len() < OPENING_KEPT + 2_000);
        let pushed = heard.of("session.row");
        assert!(!pushed.is_empty());
        for e in &pushed {
            assert!(e.to_string().len() < OPENING_KEPT + 2_000);
        }
        let (store, w) = (state.store.clone(), ws.clone());
        wait!([store, w] index_rows(&store, &w).await.len() == 1);
        assert_eq!(index_rows(&state.store, &ws).await.iter().map(|r| kept(r["prompt"].as_str())).collect::<Vec<_>>(), [true]);
        // The transcript keeps the message whole: the thread reads back as it was sent.
        let history: Vec<Value> = fx.rt.sessions().history(&ws, None).await.expect("a history").iter().map(j).collect();
        assert_matches(history.iter().find(|e| e["type"] == "session.start").expect("a start row"), json!({ "prompt": opening() }));
        let title = title_of(&rows);

        client.close().await;
        served.close().await.expect("a close");
        fx.rt.close().await.expect("a close");
        let again = runtime(&backend).store(&state.store).adapter("claude", quick().factory()).build();
        assert_eq!(title_of(&as_values(&again.rt.sessions().list(Some(ws), None).await.expect("a list"))), title);
        again.rt.close().await.expect("a close");
    });
}

// "the opening on a session row" > "a session index an older host wrote with the whole opening is read back cut, under the same title, and written back cut once"
#[test]
fn a_session_index_an_older_host_wrote_with_the_whole_opening_is_read_back_cut_under_the_same_title_and_written_back_cut_once() {
    drive(|| async {
        let state = sqlite_state();
        let backend = StubBackend::new(None);
        let fx = runtime(&backend).store(&state.store).adapter("claude", quick().factory()).build();
        let ws = create_on(&fx.rt, CreateOn::golden("a"), None).await.expect("a workspace").view.id;
        fx.rt
            .sessions()
            .start(&ws, StartOptions { prompt: opening(), ..Default::default() }, None)
            .await
            .expect("a start")
            .finished()
            .await;
        let title = title_of(&as_values(&fx.rt.sessions().list(Some(ws.clone()), None).await.expect("a list")));
        fx.rt.close().await.expect("a close");
        let mut index = state.store.get("sessions", &ws).await.expect("a read").expect("an index");
        for row in index["sessions"].as_array_mut().expect("rows") {
            row["prompt"] = json!(opening());
        }
        state.store.put("sessions", &ws, index).await.expect("a write");

        let counted = WrappedStore::over(&state.store);
        let again = runtime(&backend).store(&counted.store()).adapter("claude", quick().factory()).build();
        let rows = as_values(&again.rt.sessions().list(Some(ws.clone()), None).await.expect("a list"));
        assert_eq!(rows.iter().map(|r| kept(r["prompt"].as_str())).collect::<Vec<_>>(), [true]);
        assert_eq!(title_of(&rows), title);
        // Nothing else may ever save this workspace's rows again, and every launch would parse the whole index.
        let (store, w) = (state.store.clone(), ws.clone());
        wait!([store, w] index_rows(&store, &w).await.iter().all(|r| kept(r["prompt"].as_str())));
        again.rt.close().await.expect("a close");
        let written = counted.puts_into("sessions");
        let last = runtime(&backend).store(&counted.store()).adapter("claude", quick().factory()).build();
        last.rt.sessions().list(Some(ws.clone()), None).await.expect("a list");
        sleep_ms(100).await;
        assert_eq!(counted.puts_into("sessions"), written);
        last.rt.close().await.expect("a close");
    });
}
