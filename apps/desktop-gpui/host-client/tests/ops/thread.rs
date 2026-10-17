// SPDX-License-Identifier: AGPL-3.0-only
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use wsp_host_client::ops::thread::History;
use wsp_host_client::protocol::*;
use wsp_host_client::{Event, HostClient};

use crate::common::{answered, ask, first, refused, shared};

const PNG: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNkYAAAAAYAAjCB0C8AAAAASUVORK5CYII=";

fn text<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_value(value).unwrap().as_str().unwrap_or_default().to_owned()
}

/// A project of this group's own, so a turn here never runs beside another group's in one folder.
fn project(name: &str) -> String {
    let folder = shared().host.repo(name);
    shared().host.wsp(&["add", &folder.to_string_lossy()]);
    name.to_owned()
}

async fn ended(client: &HostClient, thread_id: &str) {
    let until = Instant::now() + Duration::from_secs(30);
    loop {
        let row = client.threads().await.unwrap().into_iter().find(|t| t.thread_id.as_deref() == Some(thread_id));
        if row.is_some_and(|r| text(&r.status) != "running") {
            return;
        }
        assert!(Instant::now() < until, "the turn on {thread_id} did not end");
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

#[tokio::test]
async fn thread() {
    let host = &shared().host;
    let client = host.client().await;
    let first = first(&client).await;
    let project = project("thread-ops");

    let harnesses = answered("harnesses.list", client.call(ask::<HarnessesListRequest>(json!({}))).await);
    assert!(harnesses.harnesses.iter().any(|h| h.harness == "claude"));
    let warm = answered("sessions.warm", client.call(ask::<SessionsWarmRequest>(json!({ "project": project, "harness": "claude" }))).await);
    assert!(["started", "standing"].contains(&text(&warm.warm).as_str()), "{warm:?}");
    let subscribed = answered("events.subscribe", client.call(ask::<EventsSubscribeRequest>(json!({}))).await);
    assert!(subscribed.stream.is_some());

    let request_id = "thread-ops-1";
    let start = json!({
        "project": project,
        "prompt": "thread ops",
        "harness": "claude",
        "requestId": request_id,
        "attachments": [{ "mediaType": "image/png", "bytes": PNG, "name": "dot.png" }],
    });
    let started = answered("sessions.start", client.call(ask::<SessionsStartRequest>(start)).await);
    assert_eq!(text(&started.outcome), "started");
    let session_id = started.session.id.clone();
    let workspace_id = started.session.workspace_id.clone();
    let thread_id = started.session.thread_id.clone().expect("a thread id");
    ended(&client, &thread_id).await;

    let head = answered("sessions.head", client.call(ask::<SessionsHeadRequest>(json!({ "threadId": thread_id }))).await);
    assert!(head.total > 0);
    let page = answered(
        "sessions.history",
        client.call(ask::<SessionsHistoryRequest>(json!({ "workspaceId": workspace_id, "threadId": thread_id }))).await,
    );
    let History::Page(page) = page else { panic!("a thread's history without whole is a page") };
    assert_eq!(page.total, head.total);
    let whole = answered(
        "sessions.history",
        client.call(ask::<SessionsHistoryRequest>(json!({ "workspaceId": workspace_id, "threadId": thread_id, "whole": true }))).await,
    );
    let History::Whole { events } = whole else { panic!("a whole thread's history is its events alone") };
    assert_eq!(events.len() as u64, page.total);

    let kept = answered(
        "sessions.attachment",
        client
            .call(ask::<SessionsAttachmentRequest>(
                json!({ "workspaceId": workspace_id, "threadId": thread_id, "requestId": request_id, "index": 0 }),
            ))
            .await,
    );
    assert_eq!((kept.attachment.media_type.as_str(), kept.attachment.bytes.as_str()), ("image/png", PNG));

    let steer =
        answered("sessions.steer", client.call(ask::<SessionsSteerRequest>(json!({ "sessionId": session_id, "prompt": "more" }))).await);
    assert_eq!(text(&steer.outcome), "not-running");
    let stop = answered("sessions.interrupt", client.call(ask::<SessionsInterruptRequest>(json!({ "sessionId": session_id }))).await);
    assert_eq!(text(&stop.outcome), "not-running");
    let answer = answered(
        "sessions.answer",
        client.call(ask::<SessionsAnswerRequest>(json!({ "sessionId": session_id, "askId": "a", "optionId": "o" }))).await,
    );
    assert_eq!(text(&answer.outcome), "gone");
    let access = answered(
        "sessions.access",
        client.call(ask::<SessionsAccessRequest>(json!({ "sessionId": session_id, "permissionMode": "default" }))).await,
    );
    assert_eq!(text(&access.outcome), "set");
    let rename = answered(
        "sessions.rename",
        client.call(ask::<SessionsRenameRequest>(json!({ "sessionId": session_id, "title": "renamed" }))).await,
    );
    // The stand-in keeps no session store for the name to land in.
    assert_eq!(text(&rename.outcome), "no-session");
    refused(
        "sessions.aside",
        client.call(ask::<SessionsAsideRequest>(json!({ "sessionId": session_id, "question": "why?" }))).await,
        "No conversation found with session ID",
    );

    let run = json!({
        "runId": "run-1",
        "block": "m1:0",
        "command": "true",
        "state": "exited",
        "exitCode": 0,
        "threadId": thread_id,
        "turnId": started.turn_id,
    });
    let run = answered("sessions.run", client.call(ask::<SessionsRunRequest>(run)).await);
    assert_eq!((run.run.run_id.as_str(), text(&run.run.state).as_str()), ("run-1", "exited"));

    let again = json!({ "workspaceId": workspace_id, "thread": thread_id, "prompt": "again", "harness": "claude" });
    let again = answered("sessions.start", client.call(ask::<SessionsStartRequest>(again)).await);
    assert_eq!(again.session.thread_id.as_deref(), Some(thread_id.as_str()));
    ended(&client, &thread_id).await;
    // The stand-in's replies carry no uuid for the cut to anchor on, and with no daemon no turn kept a checkpoint.
    refused(
        "sessions.rewind",
        client.call(ask::<SessionsRewindRequest>(json!({ "threadId": thread_id, "turnId": started.turn_id, "files": false }))).await,
        "left no point in that reply to cut its conversation at",
    );

    let conversations = answered("conversations.list", client.call(ask::<ConversationsListRequest>(json!({ "project": "repo" }))).await);
    assert!(conversations.rows.is_empty() && !conversations.held.is_empty(), "{conversations:?}");
    refused(
        "conversations.read",
        client
            .call(ask::<ConversationsReadRequest>(json!({ "project": "repo", "agent": "claude", "conversation": "no-such-conversation" })))
            .await,
        "wsp-daemon binary missing",
    );

    refused(
        "workspaces.start",
        client.call(ask::<WorkspacesStartRequest>(json!({ "url": "https://github.com/nobody/nothing/pull/1" }))).await,
        "no project here is a checkout of nobody/nothing",
    );
    refused(
        "workspaces.review",
        client.call(ask::<WorkspacesReviewRequest>(json!({ "workspaceId": first.workspace_id }))).await,
        "a review needs a pull request's link",
    );
    refused(
        "editor.open",
        client.call(ask::<EditorOpenRequest>(json!({ "workspaceId": first.workspace_id, "path": shared().folder }))).await,
        "No editor wsp knows is installed",
    );
}

#[tokio::test]
async fn events_decode_a_turn() {
    let host = &shared().host;
    let client = host.client().await;
    let project = project("thread-events");
    let mut events = client.events().await.unwrap();
    let started = answered(
        "sessions.start",
        client.call(ask::<SessionsStartRequest>(json!({ "project": project, "prompt": "events", "harness": "claude" }))).await,
    );
    let thread_id = started.session.thread_id.expect("a thread id");
    let mut seen = Vec::new();
    let until = tokio::time::Instant::now() + Duration::from_secs(30);
    while !seen.iter().any(|t| t == "session.end") {
        let next =
            tokio::time::timeout_at(until, events.next()).await.unwrap_or_else(|_| panic!("no session.end; host events seen: {seen:?}"));
        let Event::Host(event) = next.expect("the socket closed").unwrap() else { continue };
        let frame: Value = serde_json::to_value(&*event).unwrap();
        if frame["threadId"] == thread_id.as_str() {
            seen.push(frame["type"].as_str().unwrap_or_default().to_owned());
        }
    }
    assert!(seen.iter().any(|t| t == "session.start"), "{seen:?}");
}
