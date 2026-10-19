// SPDX-License-Identifier: AGPL-3.0-only
//! A newer host's replies still read: a word no release of this app knows reads as `Unknown`, a field the record does
//! not list is passed over, a known word reads and writes as it always did, and a pushed event of a type the record
//! does not name comes as it was sent. Pure reads, no host.

use serde_json::{json, Value};
use wsp_host_client::ops::settings::PreferencesReply;
use wsp_host_client::ops::sidebar::{Sessions, Workspaces};
use wsp_host_client::protocol::{
    PreferencesComputerLookValueIcon, PreferencesGetRequest, PreferencesTheme, SessionEvent, SessionViewStatus,
    SessionViewSubagentsItemState, SessionsListRequest, WorkspaceOutPhase, WorkspacesListRequest,
};
use wsp_host_client::{Event, Op};

/// A reply as `HostClient::call` reads it, once the frame's id and ok are taken off.
fn reply<O: Op>(value: Value) -> O::Reply {
    serde_json::from_value(value).unwrap_or_else(|e| panic!("{} answered what its reply type does not read: {e}", O::NAME))
}

fn sessions(status: &str, state: &str) -> Value {
    json!({ "sessions": [{
        "id": "s1", "workspaceId": "w1", "harness": "claude", "status": status,
        "subagents": [{ "id": "a1", "title": "look", "startedAt": 1.0, "state": state }]
    }] })
}

fn workspaces(phase: &str) -> Value {
    json!({ "workspaces": [{
        "id": "w1", "name": "boat", "golden": "default", "machineId": "m1", "createdAt": "2026-10-19T00:00:00Z", "phase": phase,
        "project": { "id": "p1", "name": "wsp", "path": "/root/wsp", "computer": "Boat" }
    }] })
}

fn preferences(icon: &str, look: Value) -> Value {
    let mut look_of = json!({ "icon": icon });
    look_of.as_object_mut().unwrap().extend(look.as_object().unwrap().clone());
    json!({ "preferences": {
        "access": {}, "labs": false, "projectLook": {}, "projectOrder": [], "sidebarMode": "list", "terminalSize": "app", "terminalZoom": {},
        "theme": "dark", "computerLook": { "m1": look_of }
    } })
}

#[test]
fn a_word_no_release_knows_reads_as_unknown() {
    let listed: Sessions = reply::<SessionsListRequest>(sessions("parked", "dozing"));
    assert_eq!(listed.sessions[0].status, SessionViewStatus::Unknown);
    assert_eq!(listed.sessions[0].subagents[0].state, SessionViewSubagentsItemState::Unknown);

    let listed: Workspaces = reply::<WorkspacesListRequest>(workspaces("hibernating"));
    assert_eq!(listed.workspaces[0].phase, WorkspaceOutPhase::Unknown);

    let read: PreferencesReply = reply::<PreferencesGetRequest>(preferences("rocket", json!({})));
    assert_eq!(read.preferences.computer_look["m1"].icon, PreferencesComputerLookValueIcon::Unknown);

    let history: Vec<SessionEvent> = serde_json::from_value(json!([
        { "type": "session.dream", "sessionId": "s1", "workspaceId": "w1", "pos": 1 },
        { "type": "session.steer", "sessionId": "s1", "workspaceId": "w1", "prompt": "go on", "pos": 2 }
    ]))
    .unwrap();
    assert!(matches!(history[0], SessionEvent::Unknown), "{:?}", history[0]);
    assert!(matches!(history[1], SessionEvent::SessionSteer { .. }), "{:?}", history[1]);
}

#[test]
fn a_field_the_record_does_not_list_reads() {
    let read: PreferencesReply = reply::<PreferencesGetRequest>(preferences("laptop", json!({ "tint": "teal" })));
    assert_eq!(read.preferences.computer_look["m1"].icon, PreferencesComputerLookValueIcon::Laptop);
    let mut sessions = sessions("running", "running");
    sessions["sessions"][0]["mood"] = json!("calm");
    let listed: Sessions = reply::<SessionsListRequest>(sessions);
    assert_eq!(listed.sessions[0].status, SessionViewStatus::Running);
}

#[test]
fn known_words_read_and_write_as_before() {
    let sent = sessions("interrupted", "done");
    let listed: Sessions = reply::<SessionsListRequest>(sent.clone());
    assert_eq!(listed.sessions[0].status, SessionViewStatus::Interrupted);
    assert_eq!(serde_json::to_value(&listed.sessions).unwrap(), sent["sessions"]);

    let sent = workspaces("napping");
    let listed: Workspaces = reply::<WorkspacesListRequest>(sent.clone());
    assert_eq!(listed.workspaces[0].phase, WorkspaceOutPhase::Napping);
    assert_eq!(serde_json::to_value(&listed.workspaces).unwrap(), sent["workspaces"]);

    // Preferences write every default the host left out, so the whole object they write is what reads back.
    let read: PreferencesReply = reply::<PreferencesGetRequest>(preferences("mac-mini", json!({})));
    assert_eq!(read.preferences.theme, PreferencesTheme::Dark);
    let whole = serde_json::to_value(&read.preferences).unwrap();
    assert_eq!((&whole["theme"], &whole["computerLook"]["m1"]), (&json!("dark"), &json!({ "icon": "mac-mini" })));
    let again: PreferencesReply = reply::<PreferencesGetRequest>(json!({ "preferences": whole.clone() }));
    assert_eq!(serde_json::to_value(&again.preferences).unwrap(), whole);

    // An unknown word writes as one no host takes and reads back as itself; a word the app parses is never unknown.
    assert_eq!(serde_json::to_value(SessionViewStatus::Unknown).unwrap(), json!("unknown"));
    assert_eq!(serde_json::from_value::<SessionViewStatus>(json!("unknown")).unwrap(), SessionViewStatus::Unknown);
    assert_eq!(SessionViewStatus::Unknown.to_string(), "unknown");
    assert!("unknown".parse::<SessionViewStatus>().is_err() && "parked".parse::<SessionViewStatus>().is_err());
    assert_eq!("failed".parse::<SessionViewStatus>().unwrap(), SessionViewStatus::Failed);
}

#[test]
fn an_unknown_pushed_event_comes_as_it_was_sent() {
    let dream = json!({ "type": "session.dream", "sessionId": "s1", "threadId": "t1" });
    assert!(matches!(Event::read("session.dream", dream.clone()), Ok(Event::Other(frame)) if frame == dream));
    let daemon = json!({ "type": "daemon.resized", "channel": "c1", "cols": 80 });
    assert!(matches!(Event::read("daemon.resized", daemon.clone()), Ok(Event::Other(frame)) if frame == daemon));
    let creating = json!({
        "type": "workspace.creating", "workspaceId": "w1", "name": "boat", "message": "Thawing", "elapsedMs": 1.0,
        "stage": "thawing", "mood": "calm"
    });
    match Event::read("workspace.creating", creating) {
        Ok(Event::Host(_)) => {}
        other => panic!("a known event with a newer stage and field reads as the host's: {other:?}"),
    }
}
