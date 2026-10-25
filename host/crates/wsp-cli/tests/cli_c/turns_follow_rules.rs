// SPDX-License-Identifier: AGPL-3.0-only
//! The rules of `turns_help/follow.rs` the recorded cases pin only as a whole, each held by its own named test over a
//! recorded case. Spec: `spec/turns-follow.md`, rule table.
use serde_json::json;
use wsp_words::words::thread::{HOST_STOPPING_LINE, NOT_DELIVERED_LINE};

use super::common::{run, Ran};
use super::drivers_turns_follow as d;

const UNIT: &str = "turns_follow";

fn is_v4_uuid(s: &str) -> bool {
    let parts: Vec<&str> = s.split('-').collect();
    parts.len() == 5
        && [8, 4, 4, 4, 12].iter().zip(&parts).all(|(n, p)| p.len() == *n && p.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()))
        && parts[2].starts_with('4')
        && "89ab".contains(&parts[3][..1])
}

fn keys_of(ran: &Ran, op: &str, nth: usize) -> Vec<String> {
    ran.frame(op, nth).keys().cloned().collect()
}

fn err(ran: &Ran) -> String {
    ran.got.as_ref().expect("it settled").as_ref().expect_err("it failed").message.clone()
}

fn value(ran: &Ran) -> serde_json::Value {
    ran.got.as_ref().expect("it settled").as_ref().expect("it returned").clone()
}

#[test]
fn a_start_carries_a_v4_request_id_and_goes_again_with_the_same_one() {
    let ran = run(UNIT, "startDetached", "the host stops under the start, dialled again", d::start_detached);
    assert_eq!(ran.ids.len(), 2, "the start is sent twice: {:?}", ran.log);
    assert_eq!(ran.ids[0], ran.ids[1], "a start sent again is the same message to the host that comes back");
    assert!(is_v4_uuid(&ran.ids[0]), "{} is no v4 uuid", ran.ids[0]);
}

#[test]
fn two_starts_mint_two_request_ids() {
    let one = run(UNIT, "startDetached", "started", d::start_detached);
    let two = run(UNIT, "startDetached", "started", d::start_detached);
    assert_ne!(one.ids[0], two.ids[0]);
}

#[test]
fn the_start_goes_out_with_its_fields_then_started_by_request_id_and_answer_held() {
    let ran = run(UNIT, "startDetached", "the fields go out in order, then startedBy, requestId and answerHeld", d::start_detached);
    assert_eq!(keys_of(&ran, "sessions.start", 0), ["workspaceId", "prompt", "harness", "thread", "attachments", "model", "startedBy", "requestId", "answerHeld", "id", "op"]);
    assert_eq!(ran.frame("sessions.start", 0)["answerHeld"], json!(true));
    assert_eq!(ran.frame("sessions.start", 0)["startedBy"], json!("cli"));
}

#[test]
fn a_follow_adds_its_followed_mark_after_the_starts_own_fields() {
    let ran = run(UNIT, "follow", "the start's followed mark goes last among the start's own fields", d::follow);
    assert_eq!(keys_of(&ran, "sessions.start", 0), ["workspaceId", "prompt", "harness", "thread", "model", "followed", "startedBy", "requestId", "answerHeld", "id", "op"]);
    assert_eq!(ran.frame("sessions.start", 0)["followed"], json!(true));
}

#[test]
fn events_are_subscribed_once_before_the_start_on_each_socket() {
    let ran = run(UNIT, "startDetached", "started", d::start_detached);
    assert_eq!(ran.ops(), ["events.subscribe", "sessions.start"]);
    let ran = run(UNIT, "follow", "the host stops under the turn twice", d::follow);
    assert_eq!(ran.ops().iter().filter(|o| *o == "events.subscribe").count(), 3, "one subscription for each of the three sockets: {:?}", ran.log);
}

#[test]
fn a_start_the_host_refused_is_never_sent_again() {
    let ran = run(UNIT, "startDetached", "the host refuses then is not asked again", d::start_detached);
    assert_eq!(ran.ops().iter().filter(|o| *o == "sessions.start").count(), 1);
    assert!(!ran.log.iter().any(|l| l == "<dialled again>"), "{:?}", ran.log);
}

#[test]
fn a_socket_the_host_did_not_stop_is_not_dialled_again() {
    let ran = run(UNIT, "startDetached", "the host closes with another code under the start", d::start_detached);
    assert!(!ran.log.iter().any(|l| l == "<dialled again>"), "{:?}", ran.log);
    assert_eq!(err(&ran), "the host closed the connection");
}

#[test]
fn no_road_back_is_a_message_not_delivered_never_a_turn_that_goes_on() {
    for name in ["the host stops under the start, no way back", "the host stops under the start, the dial is refused", "the host stops under the start, then no second dial is left"] {
        let ran = run(UNIT, "startDetached", name, d::start_detached);
        assert_eq!(err(&ran), NOT_DELIVERED_LINE, "{name}");
        assert_ne!(err(&ran), HOST_STOPPING_LINE, "{name}");
    }
}

#[test]
fn a_host_that_stops_under_a_named_turn_says_the_turn_goes_on_where_nothing_can_dial_again() {
    let ran = run(UNIT, "follow", "the host stops under the turn, no way back", d::follow);
    assert_eq!(err(&ran), HOST_STOPPING_LINE);
}

#[test]
fn the_dial_that_fails_after_the_turn_is_named_fails_with_its_own_words() {
    let ran = run(UNIT, "follow", "the host stops under the turn, the dial is refused", d::follow);
    assert_eq!(err(&ran), "no host serving the state file");
    assert_eq!(ran.sinks.said.lock().unwrap().last().map(String::as_str), Some("redialed"));
}

#[test]
fn a_follow_on_a_new_host_reads_the_whole_transcript_of_its_thread() {
    let ran = run(UNIT, "follow", "the host stops under the turn, the host's transcript holds its done", d::follow);
    let history = ran.frame("sessions.history", 0);
    assert_eq!(history["workspaceId"], json!("ws-1"));
    assert_eq!(history["threadId"], json!("thread-9d8c7b6a5f4e"));
    assert_eq!(history["whole"], json!(true));
    assert_eq!(value(&ran)["result"]["text"], json!("from the transcript"));
}

#[test]
fn an_end_already_in_the_transcript_ends_the_follow_with_its_reason() {
    let ran = run(UNIT, "follow", "the host stops under the turn, the transcript holds its end", d::follow);
    let turn = value(&ran);
    assert_eq!(turn["reason"], json!("the machine went to sleep"));
    assert!(turn.get("result").is_none(), "an end carries no result: {turn}");
}

#[test]
fn first_ended_subscribes_before_it_lists() {
    let ran = run(UNIT, "firstEnded", "a done pushed for a named thread", d::first_ended);
    assert_eq!(ran.ops(), ["events.subscribe", "sessions.list"], "no transcript is read for a done that carries its result");
}

#[test]
fn a_thread_the_listing_lost_is_answered_failed_off_its_transcript_alone() {
    let ran = run(UNIT, "firstEnded", "a named thread the listing no longer holds, nothing in its transcript", d::first_ended);
    assert_eq!(ran.ops(), ["events.subscribe", "sessions.list", "sessions.history"]);
    assert_eq!(value(&ran), json!({"ended": {"threadId": "thread-9d8c7b6a5f4e", "result": {"status": "failed"}}}));
}

#[test]
fn a_gone_thread_is_answered_before_one_that_is_over() {
    let ran = run(UNIT, "firstEnded", "two named, one gone and one over: the gone one is answered first", d::first_ended);
    assert_eq!(value(&ran)["ended"]["threadId"], json!("thread-b0b0b0b0c1c1"));
}

#[test]
fn an_end_reads_the_transcript_of_the_workspace_the_end_names() {
    let ran = run(UNIT, "firstEnded", "an end pushed reads the workspace the end names", d::first_ended);
    assert_eq!(ran.frame("sessions.history", 0)["workspaceId"], json!("ws-9"));
}

#[test]
fn a_deadline_that_passes_answers_the_deadline_never_an_error() {
    let ran = run(UNIT, "firstEnded", "the deadline passes first", d::first_ended);
    assert_eq!(value(&ran), json!({"timedOutMs": 60}));
    let ran = run(UNIT, "firstEnded", "a deadline of zero", d::first_ended);
    assert_eq!(value(&ran), json!({"timedOutMs": 0}));
}

#[test]
fn no_deadline_waits_for_ever() {
    let ran = run(UNIT, "firstEnded", "no deadline and nothing to hear", d::first_ended);
    assert!(ran.got.is_none(), "it settled: {:?}", ran.got);
}

#[test]
fn a_sub_thread_end_asks_for_its_machine_after_the_read() {
    let ran = run(UNIT, "waitThrough", "a sub-thread's end carries what its machine was read to hold", d::wait_through);
    assert_eq!(ran.ops(), ["events.subscribe", "sessions.list", "sessions.history", "sessions.head"]);
    let head = ran.frame("sessions.head", 0);
    assert_eq!(head["threadId"], json!("thread-9d8c7b6a5f4e"));
    assert_eq!(head["afterRead"], json!(true));
    assert_eq!(value(&ran)["ended"]["sub"]["commits"], json!(3));
}

#[test]
fn a_thread_that_is_no_sub_thread_asks_nothing_more() {
    let ran = run(UNIT, "waitThrough", "a sub-thread that is not named a sub-thread asks nothing more", d::wait_through);
    assert!(!ran.ops().contains(&"sessions.head".to_string()));
}

#[test]
fn wait_through_says_redialed_once_for_each_stop() {
    let ran = run(UNIT, "waitThrough", "the host stops under the wait twice", d::wait_through);
    assert_eq!(*ran.sinks.said.lock().unwrap(), ["redialed", "redialed"]);
}

#[test]
fn a_deadline_that_passes_while_the_host_is_coming_back_is_the_deadlines_answer() {
    let ran = run(UNIT, "waitThrough", "the host stops and never comes back before the deadline", d::wait_through);
    assert_eq!(value(&ran), json!({"timedOutMs": 100}));
    let ran = run(UNIT, "waitThrough", "the host stops and never comes back, no deadline", d::wait_through);
    assert_eq!(err(&ran), "no host serving the state file");
}

#[test]
fn a_socket_closed_with_another_code_is_not_dialled_again_by_a_wait() {
    let ran = run(UNIT, "waitThrough", "the host closes under the wait with another code", d::wait_through);
    assert_eq!(err(&ran), "the host closed the connection");
    assert!(!ran.log.iter().any(|l| l == "<dialled again>"));
}

#[test]
fn restart_dials_again_only_after_the_host_closed_and_lists_nothing_else() {
    let ran = run(UNIT, "restartHost", "the host restarts and one thread runs", d::restart_host);
    let host_restart = ran.log.iter().position(|l| l.contains("\"op\":\"host.restart\"")).expect("host.restart sent");
    let dialled = ran.log.iter().position(|l| l == "<dialled again>").expect("dialled again");
    let listed = ran.log.iter().position(|l| l.contains("\"op\":\"sessions.list\"")).expect("sessions.list sent");
    assert!(host_restart < dialled && dialled < listed, "{:?}", ran.log);
    assert_eq!(ran.ops(), ["host.restart", "sessions.list"]);
}

#[test]
fn restart_names_the_running_threads_in_the_listings_order() {
    let ran = run(UNIT, "restartHost", "the host restarts and two threads run, in the listing's order", d::restart_host);
    assert_eq!(value(&ran), json!({"running": ["thread-b0b0b0b0c1c1", "thread-9d8c7b6a5f4e"]}));
}

#[test]
fn a_restart_the_road_refuses_stops_nothing_and_dials_nothing() {
    let ran = run(UNIT, "restartHost", "the restart is refused in the road's own words", d::restart_host);
    assert_eq!(ran.ops(), ["host.restart"]);
    assert_eq!(err(&ran), "this host was not started by a service");
}

#[test]
fn a_host_that_took_the_restart_and_kept_serving_is_told_after_twenty_seconds() {
    let ran = run(UNIT, "restartHost", "the host took the restart and kept serving", d::restart_host);
    assert_eq!(err(&ran), "the host took the restart and was still serving after 20s");
}

#[test]
fn a_read_reads_the_transcript_before_it_moves_the_stamp() {
    let ran = run(UNIT, "readThread", "the messages", d::read_thread);
    assert_eq!(ran.ops(), ["sessions.history", "sessions.read"]);
    assert_eq!(ran.frame("sessions.read", 0)["threadId"], json!("t-1111aaaa"));
    let ran = run(UNIT, "readThread", "the transcript read is refused", d::read_thread);
    assert_eq!(ran.ops(), ["sessions.history"], "a refused read moves no stamp");
}

#[test]
fn a_read_reads_one_threads_whole_transcript() {
    let ran = run(UNIT, "readThread", "the messages", d::read_thread);
    let h = ran.frame("sessions.history", 0);
    assert_eq!((h["workspaceId"].clone(), h["threadId"].clone(), h["whole"].clone()), (json!("ws-1"), json!("t-1111aaaa"), json!(true)));
}

#[test]
fn a_head_keeps_the_hosts_four_parts_and_nothing_else() {
    let ran = run(UNIT, "threadHead", "a head keeps only its four parts", d::thread_head);
    let head = value(&ran);
    assert_eq!(head.as_object().unwrap().keys().collect::<Vec<_>>(), ["facts", "events", "pos", "total"]);
}
