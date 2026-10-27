// SPDX-License-Identifier: AGPL-3.0-only
//! packages/runtime/test/steer-images.test.ts at 65f6b89c7, case for case. A steer or a send that carries an image
//! into a thread whose turn is running: what the harness is handed, what the row records, and how long the host
//! keeps the image. Each test names the TS test above it. The TS `memoryStore()` is the sqlite store here.
mod common;

use std::rc::Rc;

use common::*;
use serde_json::{json, Value};
use wsp_core::types::api::{StartOptions, SteerAsk};
use wsp_store::store::Store;
use wsp_wire::types::attachments::Attachment;
use wsp_words::attachments::attachment_record;
use wsp_words::exit::Kind;

const PNG_BYTES: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==";

fn png() -> Attachment {
    Attachment { media_type: "image/png".into(), bytes: PNG_BYTES.into(), name: Some("dot.png".into()) }
}

fn notes() -> Attachment {
    use base64::Engine as _;
    Attachment {
        media_type: "text/plain".into(),
        bytes: base64::engine::general_purpose::STANDARD.encode("notes"),
        name: Some("notes.txt".into()),
    }
}

fn image_json(a: &Attachment) -> Value {
    json!({ "mediaType": a.media_type, "bytes": a.bytes })
}

/// `harness(steersImages)`: steers and reads images inline, its turns run until the test ends them.
fn harness(steers_images: bool) -> Rc<Live> {
    Live::new(LiveOptions { steers: true, steers_images, inline: true, announces: true, session: None })
}

struct Running {
    h: Rc<Live>,
    fx: Fx,
    ws: String,
    first: Rc<dyn wsp_core::types::wiring::SessionHandle>,
    thread_id: String,
    events: Events,
}

impl Running {
    fn steers(&self) -> Vec<Value> {
        self.events.of("session.steer")
    }
}

async fn running(steers_images: bool, store: Option<&Rc<dyn Store>>) -> Running {
    let h = harness(steers_images);
    let backend = StubBackend::new(None);
    let mut wiring = runtime(&backend).adapter("claude", h.factory());
    if let Some(store) = store {
        wiring = wiring.store(store);
    }
    let fx = wiring.build();
    let events = fx.events();
    let ws = create_on(&fx.rt, CreateOn::golden("a"), None).await.expect("a workspace").view.id;
    let first = fx
        .rt
        .sessions()
        .start(&ws, StartOptions { prompt: "look around".into(), request_id: Some("req_1".into()), ..Default::default() }, None)
        .await
        .expect("a start");
    let thread_id = first.view().thread_id.expect("a thread");
    Running { h, fx, ws, first, thread_id, events }
}

fn steer(prompt: &str, request_id: &str, files: Vec<Attachment>) -> SteerAsk {
    SteerAsk { prompt: prompt.into(), request_id: Some(request_id.into()), attachments: Some(files) }
}

fn steered_of(h: &Live) -> Vec<Value> {
    h.steered
        .borrow()
        .iter()
        .map(|s| {
            let mut doc = json!({ "prompt": s.prompt });
            if !s.images.is_empty() {
                doc["images"] = json!(s.images.iter().map(|i| json!({ "mediaType": i.media_type, "bytes": i.bytes })).collect::<Vec<_>>());
            }
            doc
        })
        .collect()
}

// "a steer that carries an image" > "goes into the running turn with its bytes, the row records it, and the host keeps the image past a restart"
#[test]
fn goes_into_the_running_turn_with_its_bytes_the_row_records_it_and_the_host_keeps_the_image_past_a_restart() {
    drive(|| async {
        let state = sqlite_state();
        let r = running(true, Some(&state.store)).await;
        let listed = r.fx.rt.harnesses().list(Some(r.ws.clone()), None, None).await.expect("a list");
        let claude = listed.iter().map(j).find(|c| c["harness"] == "claude").expect("claude's catalog");
        assert_eq!(claude["steersImages"], true);
        let taken =
            r.fx.rt.sessions().steer(&r.first.id(), steer("what is in this picture?", "req_2", vec![png()]), None).await.expect("a steer");
        assert_eq!(j(&taken), json!({ "outcome": "accepted" }));
        assert_eq!(steered_of(&r.h), [json!({ "prompt": "what is in this picture?", "images": [image_json(&png())] })]);
        let steers = r.steers();
        assert_eq!(steers.len(), 1);
        assert_matches(
            &steers[0],
            json!({ "prompt": "what is in this picture?", "requestId": "req_2", "attachments": [j(&attachment_record(&png()))] }),
        );
        r.h.end(0, "ok");
        r.first.finished().await;
        r.fx.rt.close().await.expect("a close");

        let backend = StubBackend::new(None);
        let again = runtime(&backend).store(&state.store).adapter("claude", harness(true).factory()).build();
        let history = again.rt.sessions().history(&r.ws, None).await.expect("a history");
        let steer_event = history.iter().map(j).find(|e| e["type"] == "session.steer").expect("a steer row");
        assert_matches(&steer_event, json!({ "requestId": "req_2", "attachments": [j(&attachment_record(&png()))] }));
        let kept = again.rt.sessions().attachment(&r.ws, &r.thread_id, "req_2", 0, None).await.expect("the image");
        assert_eq!(j(&kept), json!({ "mediaType": "image/png", "bytes": PNG_BYTES }));
        again.rt.close().await.expect("a close");
    });
}

// "a steer that carries an image" > "goes when the steer that names it leaves the transcript, as a start's image does"
#[test]
fn goes_when_the_steer_that_names_it_leaves_the_transcript_as_a_starts_image_does() {
    drive(|| async {
        let state = sqlite_state();
        let r = running(true, Some(&state.store)).await;
        r.fx.rt.sessions().steer(&r.first.id(), steer("look", "req_2", vec![png()]), None).await.expect("a steer");
        r.h.end(0, "ok");
        r.first.finished().await;
        let kept = r.fx.rt.sessions().attachment(&r.ws, &r.thread_id, "req_2", 0, None).await.expect("the image");
        assert_eq!(j(&kept), json!({ "mediaType": "image/png", "bytes": PNG_BYTES }));
        let big = "w".repeat(256 * 1024);
        for i in 1..=20usize {
            let next = r
                .fx
                .rt
                .sessions()
                .start(&r.ws, StartOptions { thread: Some(r.thread_id.clone()), prompt: format!("write {i}"), ..Default::default() }, None)
                .await
                .expect("a start");
            r.h.write(i, &big);
            r.h.end(i, "ok");
            next.finished().await;
        }
        let history = r.fx.rt.sessions().history(&r.ws, None).await.expect("a history");
        assert!(!history.iter().map(j).any(|e| e["type"] == "session.steer"));
        let gone = r.fx.rt.sessions().attachment(&r.ws, &r.thread_id, "req_2", 0, None).await.err().expect("a refusal");
        assert_eq!(gone.kind, Some(Kind::NotFound));
        assert!(state.store.get_blob("attachments", &format!("{}.req_2.0", r.thread_id)).await.expect("a read").is_none());
        r.fx.rt.close().await.expect("a close");
    });
}

// "a steer that carries an image" > "is refused on a harness whose steer reads no image, naming it, and nothing reaches the turn or the transcript"
#[test]
fn is_refused_on_a_harness_whose_steer_reads_no_image_naming_it_and_nothing_reaches_the_turn_or_the_transcript() {
    drive(|| async {
        let r = running(false, None).await;
        let image = r.fx.rt.sessions().steer(&r.first.id(), steer("and this?", "req_2", vec![png()]), None).await.err().expect("a refusal");
        assert!(image.message.contains("claude takes no image into a running turn"), "{}", image.message);
        let file =
            r.fx.rt.sessions().steer(&r.first.id(), steer("and these?", "req_3", vec![notes()]), None).await.err().expect("a refusal");
        assert!(file.message.contains("a file that is not an image goes only with a message that starts a turn"), "{}", file.message);
        assert!(r.h.steered.borrow().is_empty());
        assert!(r.steers().is_empty());
        r.h.end(0, "ok");
        r.first.finished().await;
        r.fx.rt.close().await.expect("a close");
    });
}

// "a send with files to a thread whose turn is running" > "steers its image into the turn where the harness's steer reads one"
#[test]
fn steers_its_image_into_the_turn_where_the_harnesss_steer_reads_one() {
    drive(|| async {
        let r = running(true, None).await;
        let opts = StartOptions {
            prompt: "and this one".into(),
            thread: Some(r.thread_id.clone()),
            request_id: Some("req_2".into()),
            attachments: Some(vec![png()]),
            ..Default::default()
        };
        let joined = r.fx.rt.sessions().start(&r.ws, opts, None).await.expect("a start");
        assert_eq!(j(&joined.outcome()), "steered");
        assert_eq!(r.h.starts.borrow().len(), 1);
        assert_eq!(steered_of(&r.h), [json!({ "prompt": "and this one", "images": [image_json(&png())] })]);
        let steers = r.steers();
        assert_eq!(steers.len(), 1);
        assert_matches(&steers[0], json!({ "requestId": "req_2", "attachments": [j(&attachment_record(&png()))] }));
        let kept = r.fx.rt.sessions().attachment(&r.ws, &r.thread_id, "req_2", 0, None).await.expect("the image");
        assert_eq!(j(&kept), json!({ "mediaType": "image/png", "bytes": PNG_BYTES }));
        r.h.end(0, "ok");
        r.first.finished().await;
        r.fx.rt.close().await.expect("a close");
    });
}

// "a send with files to a thread whose turn is running" > "waits for the turn to end and starts the next with its image where the steer reads none, and with a file that is not an image anywhere"
#[test]
fn waits_for_the_turn_to_end_and_starts_the_next_with_its_image_where_the_steer_reads_none_and_with_a_file_that_is_not_an_image_anywhere() {
    drive(|| async {
        for (steers_images, file) in [(false, png()), (true, notes())] {
            let r = running(steers_images, None).await;
            let outcome: Rc<std::cell::RefCell<Option<String>>> = Rc::default();
            let opts = StartOptions {
                prompt: "with a file".into(),
                thread: Some(r.thread_id.clone()),
                request_id: Some("req_2".into()),
                attachments: Some(vec![file.clone()]),
                ..Default::default()
            };
            let sessions = r.fx.rt.clone();
            let ws = r.ws.clone();
            let got = outcome.clone();
            let pending = tokio::task::spawn_local(async move {
                let s = sessions.sessions().start(&ws, opts, None).await.expect("a start");
                *got.borrow_mut() = Some(j(&s.outcome()).as_str().unwrap_or_default().to_string());
            });
            settle().await;
            assert!(outcome.borrow().is_none());
            assert!(r.h.steered.borrow().is_empty());
            r.h.end(0, "ok");
            pending.await.expect("the pending start");
            assert_eq!(outcome.borrow().as_deref(), Some("queued"));
            assert!(r.steers().is_empty());
            let prompts: Vec<String> = r.h.starts.borrow().iter().map(|s| s.prompt.clone()).collect();
            assert_eq!(prompts[0], "look around");
            assert!(prompts[1].contains("with a file"), "{}", prompts[1]);
            if file.media_type == "image/png" {
                let images = r.h.starts.borrow()[1].images.clone();
                assert_eq!(
                    images.iter().map(|i| json!({ "mediaType": i.media_type, "bytes": i.bytes })).collect::<Vec<_>>(),
                    [image_json(&png())]
                );
            }
            r.h.end(1, "ok");
            r.fx.rt.close().await.expect("a close");
        }
    });
}
