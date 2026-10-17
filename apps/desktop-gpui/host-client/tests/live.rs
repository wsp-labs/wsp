// SPDX-License-Identifier: AGPL-3.0-only
//! Against a host started for this test alone (tests/common): a project, a thread whose first turn ran before the client
//! looked, and a second turn started after the client subscribed, whose events have to come through as they happen.

mod common;

use std::time::{Duration, Instant};

use common::Throwaway;
use wsp_host_client::protocol::{SessionEvent, SessionEventResultStatus};
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
    assert!(!thread.history.iter().any(|e| prompt_of(e) == Some("second")));

    let sent = Instant::now();
    host.wsp(&["send", &thread_id, "--detach", "second"]);
    println!("{:>6} ms  wsp send --detach returned", sent.elapsed().as_millis());
    let mut seen = Vec::new();
    let (mut started, mut said, mut completed) = (false, false, false);
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
            SessionEvent::SessionDelta { text, .. } => said |= text == "ok",
            SessionEvent::SessionDone { result, .. } => completed |= result.status == SessionEventResultStatus::Completed,
            SessionEvent::SessionEnd { .. } => break,
            _ => {}
        }
    }
    assert!(started && said && completed, "the second turn's start, reply and result, saw {seen:?}");
}

fn event_type(event: &SessionEvent) -> String {
    serde_json::to_value(event).unwrap()["type"].as_str().unwrap_or_default().to_owned()
}
