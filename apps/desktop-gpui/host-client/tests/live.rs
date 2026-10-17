// SPDX-License-Identifier: AGPL-3.0-only
//! Against a host started for this test alone (tests/common): a project, a thread whose first turn ran before the client
//! looked, and a second turn started after the client subscribed, whose events have to come through as they happen.

mod common;

use std::time::{Duration, Instant};

use common::Throwaway;
use wsp_host_client::protocol::{SessionEvent, SessionEventResultStatus, WorkspacesDeleteRequest};
use wsp_host_client::{HostClient, Live};

fn prompt_of(event: &SessionEvent) -> Option<&str> {
    match event {
        SessionEvent::SessionStart { prompt, .. } => prompt.as_deref(),
        _ => None,
    }
}

#[tokio::test]
async fn lists_threads_and_receives_a_turn_started_after_it_subscribed() {
    let host = Throwaway::start();
    let folder = host.repo("repo");
    host.wsp(&["add", &folder.to_string_lossy()]);
    host.wsp(&["run", "repo", "first", "--agent", "claude"]);

    // What a shell inside a thread carries, aimed at a closed port: the client keeps to the host serving the state file.
    let mut env = host.env();
    env.extend(
        [("WSP_HOST", "http://127.0.0.1:9"), ("WSP_HOST_URL", "http://127.0.0.1:9"), ("WSP_HOST_TOKEN", "x")]
            .map(|(k, v)| (k.to_owned(), v.to_owned())),
    );
    let client = HostClient::connect(&host.state(), &env).await.unwrap_or_else(|f| panic!("{}", f.message));

    let projects = client.projects().await.unwrap_or_else(|f| panic!("{}", f.message));
    println!("projects: {:?}", projects.iter().map(|p| (&p.id, &p.name, &p.path)).collect::<Vec<_>>());
    assert!(projects.iter().any(|p| p.name == "repo" && p.path.ends_with("/repo")), "{projects:?}");

    let threads = client.threads().await.unwrap_or_else(|f| panic!("{}", f.message));
    println!("threads: {:?}", threads.iter().map(|t| (&t.workspace_id, &t.thread_id, &t.prompt, &t.status)).collect::<Vec<_>>());
    let row = threads.iter().find(|t| t.prompt.as_deref() == Some("first")).expect("the thread wsp run opened");
    let thread_id = row.thread_id.clone().expect("a thread id");

    let mut thread = client.open(&row.workspace_id, &thread_id).await.unwrap_or_else(|f| panic!("{}", f.message));
    println!("history: {} events, types {:?}", thread.history.len(), thread.history.iter().map(event_type).collect::<Vec<_>>());
    assert!(thread.history.iter().any(|e| prompt_of(e) == Some("first")));
    let replies = thread.history.iter().filter(|e| matches!(e, SessionEvent::SessionDelta { text, .. } if text == "ok")).count();
    assert_eq!(replies, 250, "every reply of the first turn, more than the 200 events a first page held");
    assert!(!thread.history.iter().any(|e| prompt_of(e) == Some("second")));

    let sent = Instant::now();
    host.wsp(&["send", &thread_id, "--detach", "second"]);
    println!("{:>6} ms  wsp send --detach returned", sent.elapsed().as_millis());
    let mut seen = Vec::new();
    let (mut started, mut said, mut completed) = (false, 0, false);
    loop {
        let next = tokio::time::timeout(Duration::from_secs(60), thread.next()).await.expect("the turn's events within 60 s");
        let live = next.expect("the socket stays open").unwrap_or_else(|f| panic!("{}", f.message));
        let Live::Event(event) = live else {
            if let Live::Other(frame) = live {
                println!("{:>6} ms  {} (outside the transcript)", sent.elapsed().as_millis(), frame["type"]);
            }
            continue;
        };
        println!("{:>6} ms  {}", sent.elapsed().as_millis(), event_type(&event));
        seen.push(event_type(&event));
        match &*event {
            SessionEvent::SessionStart { prompt, .. } => started |= prompt.as_deref() == Some("second"),
            SessionEvent::SessionDelta { text, .. } => said += usize::from(text == "ok"),
            SessionEvent::SessionDone { result, .. } => completed |= result.status == SessionEventResultStatus::Completed,
            SessionEvent::SessionEnd { .. } => break,
            _ => {}
        }
    }
    assert!(started && said == 250 && completed, "the second turn's start, every reply once and result, saw {seen:?}");
}

fn event_type(event: &SessionEvent) -> String {
    serde_json::to_value(event).unwrap()["type"].as_str().unwrap_or_default().to_owned()
}

#[tokio::test]
async fn a_deleted_workspace_leaves_the_statuses() {
    let host = Throwaway::start();
    let folder = host.repo("repo");
    host.wsp(&["add", &folder.to_string_lossy()]);
    host.wsp(&["run", "repo", "first", "--agent", "claude"]);
    let client = HostClient::connect(&host.state(), &host.env()).await.unwrap_or_else(|f| panic!("{}", f.message));
    let threads = client.threads().await.unwrap_or_else(|f| panic!("{}", f.message));
    let workspace = threads.iter().find(|t| t.prompt.as_deref() == Some("first")).expect("the thread wsp run opened").workspace_id.clone();
    let listed = |rows: Vec<serde_json::Value>| rows.iter().any(|s| s["id"] == workspace.as_str());
    assert!(listed(client.statuses().await.unwrap_or_else(|f| panic!("{}", f.message))), "the workspace has a status");
    let delete: WorkspacesDeleteRequest = serde_json::from_value(serde_json::json!({ "workspaceId": workspace })).unwrap();
    client.call(delete).await.unwrap_or_else(|f| panic!("{}", f.message));
    let until = Instant::now() + Duration::from_secs(30);
    while listed(client.statuses().await.unwrap_or_else(|f| panic!("{}", f.message))) {
        assert!(Instant::now() < until, "the deleted workspace's status still listed after 30 s");
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

/// A thread longer than a page opens on its newest 200 events, and a page before the oldest of them brings the rest.
#[tokio::test]
async fn a_long_thread_opens_on_its_newest_page_and_pages_back() {
    let host = Throwaway::start();
    let folder = host.repo("repo");
    host.wsp(&["add", &folder.to_string_lossy()]);
    host.wsp(&["run", "repo", "first", "--agent", "claude"]);
    let client = HostClient::connect(&host.state(), &host.env()).await.unwrap_or_else(|f| panic!("{}", f.message));
    let threads = client.threads().await.unwrap_or_else(|f| panic!("{}", f.message));
    let row = threads.iter().find(|t| t.prompt.as_deref() == Some("first")).expect("the thread wsp run opened");
    let thread_id = row.thread_id.clone().expect("a thread id");
    let whole = client.open(&row.workspace_id, &thread_id).await.unwrap_or_else(|f| panic!("{}", f.message)).history;
    assert!(whole.len() > 200, "a thread longer than a page: {} events", whole.len());

    let opened = client.open_page(&row.workspace_id, &thread_id).await.unwrap_or_else(|u| panic!("{u:?}"));
    assert_eq!(opened.history.len(), 200, "the newest page");
    assert_eq!(opened.total, whole.len() as u64);
    assert!(!opened.trimmed);
    let oldest = wsp_host_client::pos(&opened.history[0]).expect("a recorded event");
    assert_eq!(Some(oldest), wsp_host_client::pos(&whole[whole.len() - 200]), "the newest 200, oldest first");

    let older = client.page(&row.workspace_id, &thread_id, oldest).await.unwrap_or_else(|f| panic!("{}", f.message));
    let positions: Vec<Option<u64>> = older.events.iter().map(wsp_host_client::pos).collect();
    let expected: Vec<Option<u64>> = whole[..whole.len() - 200].iter().map(wsp_host_client::pos).collect();
    assert_eq!(positions, expected, "the page before the oldest held is every older event");
}
