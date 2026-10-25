// SPDX-License-Identifier: AGPL-3.0-only
use serde_json::json;
use wsp_host_client::protocol::*;
use wsp_host_client::HostClient;

use crate::common::{answered, ask, refused, shared};

const PNG: &[u8] = &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 0x0d, b'I', b'H', b'D', b'R'];

const SLATE: &str = r#"<slate title="Counter">
  <value name="n" start={0} />
  <run name="hi" cmd="true" timeout={10} />
  <column>
    <number id="count" label="Count" value={$n} />
    <button id="add" label="Add" onPress={set($n, $n + 1)} />
    <button id="go" label="Go" onPress={start($hi)} />
  </column>
</slate>"#;

/// A thread of this group's own, on a project of its own, so its slate is nobody else's.
async fn own_thread(client: &HostClient) -> String {
    let host = &shared().host;
    let folder = host.repo("slates-x");
    host.wsp(&["add", &folder.to_string_lossy()]);
    host.wsp(&["run", "slates-x", "slates own thread", "--agent", "claude"]);
    let row = client
        .threads()
        .await
        .unwrap()
        .into_iter()
        .find(|t| t.prompt.as_deref() == Some("slates own thread"))
        .expect("the thread wsp run opened");
    row.thread_id.expect("a thread id")
}

#[tokio::test]
async fn slates() {
    let client = shared().host.client().await;
    let thread = own_thread(&client).await;
    let t = thread.as_str();

    let none = answered("slates.get", client.call(ask::<SlatesGetRequest>(json!({ "threadId": t }))).await);
    assert!(none.slate.is_none());
    refused("slates.image", client.call(ask::<SlatesImageRequest>(json!({ "threadId": t, "src": "/nowhere.png" }))).await, "Z802");

    let bad = client.call(ask::<SlatesWriteRequest>(json!({ "threadId": t, "text": "<slate><nonsense /></slate>" }))).await;
    let failure = bad.as_ref().err().cloned();
    refused("slates.write", bad, "slate refused");
    assert!(failure.and_then(|f| f.errors).is_some_and(|errors| errors.starts_with('[')), "a refused write carries its errors");

    let wrote = answered("slates.write", client.call(ask::<SlatesWriteRequest>(json!({ "threadId": t, "text": SLATE }))).await);
    assert_eq!(wrote.version, 1);
    assert!(wrote.problems.is_empty(), "{:?}", wrote.problems);

    let got =
        answered("slates.get", client.call(ask::<SlatesGetRequest>(json!({ "threadId": t }))).await).slate.expect("the slate written");
    assert_eq!(got.version, 1);
    assert!(!got.shown_once);

    answered("slates.shown", client.call(ask::<SlatesShownRequest>(json!({ "threadId": t }))).await);
    assert!(
        answered("slates.get", client.call(ask::<SlatesGetRequest>(json!({ "threadId": t }))).await).slate.expect("the slate").shown_once
    );

    answered("slates.subscribe", client.call(ask::<SlatesSubscribeRequest>(json!({ "threadId": t, "sources": ["thread"] }))).await);
    answered("slates.unsubscribe", client.call(ask::<SlatesUnsubscribeRequest>(json!({ "threadId": t, "sources": ["thread"] }))).await);

    let state = answered("slates.state", client.call(ask::<SlatesStateRequest>(json!({ "threadId": t, "values": { "n": 5 } }))).await);
    assert_eq!(state.version, 1);

    let press = json!({ "threadId": t, "version": 1, "piece": "add", "event": "press", "requestId": "slates-add" });
    let added = answered("slates.event", client.call(ask::<SlatesEventRequest>(press)).await);
    assert_eq!(added.outcome, SlateEventAnswerOutcome::Done);

    let resolved = answered("slates.resolve", client.call(ask::<SlatesResolveRequest>(json!({ "threadId": t, "paths": ["$n"] }))).await);
    assert_eq!(resolved.values.get("$n"), Some(&json!(6)));

    let read =
        answered("slates.read", client.call(ask::<SlatesReadRequest>(json!({ "threadId": t, "sketch": true, "text": false }))).await);
    assert_eq!(read.version, 1);
    assert_eq!(read.state.get("$n"), Some(&json!(6)));

    let go = json!({ "threadId": t, "version": 1, "piece": "go", "event": "press", "requestId": "slates-go" });
    let held = answered("slates.event", client.call(ask::<SlatesEventRequest>(go)).await);
    assert_eq!(held.outcome, SlateEventAnswerOutcome::Held);
    let key = match held.ask.expect("the consent sheet") {
        SlateEventAnswerAsk::Cmd { key, run, .. } => {
            assert_eq!(run, "hi");
            key
        }
        other => panic!("a command's sheet, not {other:?}"),
    };

    answered("slates.approve", client.call(ask::<SlatesApproveRequest>(json!({ "threadId": t, "key": key, "scope": "once" }))).await);
    answered("slates.cancel", client.call(ask::<SlatesCancelRequest>(json!({ "threadId": t, "run": "hi" }))).await);
    answered("slates.approve", client.call(ask::<SlatesApproveRequest>(json!({ "threadId": t, "key": key, "scope": "refuse" }))).await);
    answered("slates.revoke", client.call(ask::<SlatesRevokeRequest>(json!({ "threadId": t, "key": key }))).await);
    refused("slates.revoke", client.call(ask::<SlatesRevokeRequest>(json!({ "threadId": t, "key": key }))).await, "holds no approval");

    let png = shared().host.dir.path().join("slates-x/pic.png");
    std::fs::write(&png, PNG).unwrap();
    let src = png.to_string_lossy();
    let image = answered("slates.image", client.call(ask::<SlatesImageRequest>(json!({ "threadId": t, "src": src }))).await);
    let SlatesImageAnswer::Variant0 { media_type, version: Some(version), .. } = image else { panic!("the image's bytes, not {image:?}") };
    assert_eq!(media_type, "image/png");
    let again =
        answered("slates.image", client.call(ask::<SlatesImageRequest>(json!({ "threadId": t, "src": src, "have": version }))).await);
    assert!(matches!(again, SlatesImageAnswer::Variant1 { unchanged: true, .. }), "{again:?}");
}
