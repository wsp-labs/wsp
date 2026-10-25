// SPDX-License-Identifier: AGPL-3.0-only
//! The rules of `turns_help/stream.rs` the recorded cases pin only as a whole, each held by its own named test over a
//! recorded case. Spec: `spec/turns-stream.md`, rule table.
use serde_json::{json, Value};
use wsp_words::exit::Kind;

use super::common::{run, Ran};
use super::drivers_turns_stream as d;

const UNIT: &str = "turns_stream";

fn err(ran: &Ran) -> wsp_words::exit::HostError {
    ran.got.as_ref().expect("it settled").as_ref().expect_err("it failed").clone()
}

fn value(ran: &Ran) -> Value {
    ran.got.as_ref().expect("it settled").as_ref().expect("it returned").clone()
}

fn lines(v: &std::sync::Mutex<Vec<String>>) -> Vec<String> {
    v.lock().unwrap().clone()
}

fn streamed(ran: &Ran) -> String {
    ran.sinks.streamed.lock().unwrap().clone()
}

#[test]
fn a_sign_in_refusal_is_the_auth_class_with_the_agents_words() {
    let ran = run(UNIT, "turnRefusal", "refused for a sign-in", d::turn_refusal);
    assert_eq!(value(&ran), json!({"error": {"message": "not logged in", "kind": "auth"}}));
    let ran = run(UNIT, "followVerb", "refused for a sign-in", d::follow_verb);
    let e = err(&ran);
    assert_eq!((e.message.as_str(), e.kind.clone()), ("not logged in", Some(Kind::Auth)));
    assert_eq!(e.exit_code(), 2);
}

#[test]
fn the_class_is_read_off_the_stamp_never_off_the_words() {
    let ran = run(UNIT, "turnRefusal", "failed with the agent's error", d::turn_refusal);
    assert_eq!(value(&ran), json!({"error": {"message": "boom"}}));
    let ran = run(UNIT, "followVerb", "a failed turn", d::follow_verb);
    let e = err(&ran);
    assert_eq!((e.message.as_str(), e.kind.clone()), ("the model refused", None));
    assert_eq!(e.exit_code(), 1, "a turn that failed is the provider class");
}

#[test]
fn a_completed_turn_has_no_failure_whatever_it_says() {
    let ran = run(UNIT, "turnFailure", "completed with an error text", d::turn_failure);
    assert_eq!(value(&ran), Value::Null);
    let ran = run(UNIT, "turnRefusal", "completed with an error text", d::turn_refusal);
    assert_eq!(value(&ran), Value::Null);
}

#[test]
fn an_empty_error_is_still_the_failure_and_an_empty_reason_too() {
    assert_eq!(value(&run(UNIT, "turnFailure", "failed with an empty error", d::turn_failure)), json!(""));
    assert_eq!(value(&run(UNIT, "turnFailure", "no result, an empty reason", d::turn_failure)), json!(""));
}

#[test]
fn a_turn_that_failed_still_prints_its_reply_before_it_fails() {
    let ran = run(UNIT, "followVerb", "a failed turn", d::follow_verb);
    assert_eq!(lines(&ran.sinks.stdout), ["x"]);
    assert_eq!(streamed(&ran), "failed\n");
    assert_eq!(err(&ran).message, "the model refused");
}

#[test]
fn json_offers_no_keys_and_sends_no_answer() {
    let ran = run(UNIT, "followVerb", "a prompt at a keyboard: the keys answer it, y (--json)", d::follow_verb);
    assert_eq!(ran.ops(), ["events.subscribe", "sessions.start"], "a key was offered under --json: {:?}", ran.log);
}

#[test]
fn json_writes_no_stream() {
    let ran = run(UNIT, "followVerb", "prose, then the done (--json)", d::follow_verb);
    assert_eq!(streamed(&ran), "");
    assert!(lines(&ran.sinks.stdout).iter().all(|l| l.starts_with('{')), "{:?}", ran.sinks.stdout.lock().unwrap());
}

#[test]
fn a_key_answers_with_the_option_its_effect_names_never_by_the_options_id() {
    for (case, option) in [("keys pick by effect, y", "2"), ("keys pick by effect, n", "1"), ("keys pick by effect, a", "3")] {
        let ran = run(UNIT, "followVerb", case, d::follow_verb);
        let answer = ran.frame("sessions.answer", 0);
        assert_eq!(answer["optionId"], json!(option), "{case}");
        assert_eq!(answer["askId"], json!("ask-1"), "{case}");
        assert_eq!(answer["sessionId"], json!("turn-9"), "{case}");
        assert!(!answer.contains_key("reason"), "{case}: a key carries no reason");
    }
}

#[test]
fn a_key_no_road_has_sends_nothing() {
    let ran = run(UNIT, "followVerb", "a prompt at a keyboard, a key no road has", d::follow_verb);
    assert_eq!(ran.ops(), ["events.subscribe", "sessions.start"]);
}

#[test]
fn a_key_for_a_road_the_prompt_has_no_option_for_sends_nothing() {
    let ran = run(UNIT, "followVerb", "a prompt at a keyboard, a key whose option the prompt lacks", d::follow_verb);
    assert_eq!(ran.ops(), ["events.subscribe", "sessions.start"]);
}

#[test]
fn a_prompt_with_its_own_answers_offers_no_key_and_points_to_the_app() {
    let ran = run(UNIT, "followVerb", "a prompt at a keyboard that carries only its own answers", d::follow_verb);
    assert!(streamed(&ran).contains("answer it in the app: this prompt carries its own answers\n"));
    assert_eq!(ran.ops(), ["events.subscribe", "sessions.start"]);
}

#[test]
fn a_second_prompt_releases_the_read_of_the_first() {
    let ran = run(UNIT, "followVerb", "a second prompt replaces the first read of keys", d::follow_verb);
    assert_eq!(ran.ops().iter().filter(|o| *o == "sessions.answer").count(), 1, "{:?}", ran.log);
    assert_eq!(ran.frame("sessions.answer", 0)["askId"], json!("ask-2"));
}

#[test]
fn a_failed_answer_goes_to_stderr_and_the_turn_goes_on() {
    let ran = run(UNIT, "followVerb", "a key that the host refuses goes to stderr", d::follow_verb);
    assert_eq!(lines(&ran.sinks.stderr), ["the prompt closed before the answer reached it"]);
    assert_eq!(lines(&ran.sinks.stdout), ["done"]);
    let ran = run(UNIT, "followVerb", "a key that the host fails on goes to stderr", d::follow_verb);
    assert_eq!(lines(&ran.sinks.stderr), ["no turn"]);
}

#[test]
fn the_wait_lines_are_held_back_under_json_but_the_join_lines_are_not() {
    let text = run(UNIT, "followVerb", "queued behind the running turn", d::follow_verb);
    assert_eq!(lines(&text.sinks.stderr), ["waiting behind the running turn", "queued behind the running turn; it has ended and this turn started"]);
    let json = run(UNIT, "followVerb", "queued behind the running turn (--json)", d::follow_verb);
    assert_eq!(lines(&json.sinks.stderr), ["queued behind the running turn; it has ended and this turn started"]);
    let text = run(UNIT, "followVerb", "the agent is starting", d::follow_verb);
    assert_eq!(lines(&text.sinks.stderr), ["Starting Claude Code"]);
    let json = run(UNIT, "followVerb", "the agent is starting (--json)", d::follow_verb);
    assert!(lines(&json.sinks.stderr).is_empty());
}

#[test]
fn a_failure_object_stays_the_one_line_on_stderr_under_json() {
    let ran = run(UNIT, "followVerb", "the host stops under the start, no way back (--json)", d::follow_verb);
    assert!(lines(&ran.sinks.stderr).is_empty(), "{:?}", ran.sinks.stderr.lock().unwrap());
    assert_eq!(err(&ran).message, "the host stopped before it took the message, so it was not delivered; send it again");
}

#[test]
fn only_the_picks_a_steer_drops_are_named_in_flag_order() {
    let ran = run(UNIT, "followVerb", "joined with picks dropped", d::follow_verb);
    assert_eq!(lines(&ran.sinks.stderr)[0], "joined the running turn; --model, --effort, --access, --fast dropped, it keeps its own model, effort and access");
    let ran = run(UNIT, "followVerb", "joined with fast false", d::follow_verb);
    assert_eq!(lines(&ran.sinks.stderr), ["joined the running turn"]);
}

#[test]
fn a_redialed_follow_prints_the_reply_whole_even_on_the_persons_own_screen() {
    let ran = run(UNIT, "followVerb", "the host stops under the turn: the reply is printed whole", d::follow_verb);
    assert_eq!(lines(&ran.sinks.stdout), ["Hello\n"]);
    assert_eq!(lines(&ran.sinks.stderr), ["the host is restarting; waiting for it to come back"]);
    let ran = run(UNIT, "followVerb", "prose on the person's own screen", d::follow_verb);
    assert!(lines(&ran.sinks.stdout).is_empty(), "streamed prose is not printed twice: {:?}", ran.sinks.stdout.lock().unwrap());
}

#[test]
fn a_reply_on_the_same_screen_is_printed_only_where_nothing_was_streamed() {
    let ran = run(UNIT, "followVerb", "no prose streamed, on the person's own screen", d::follow_verb);
    assert_eq!(lines(&ran.sinks.stdout), ["the reply"]);
}

#[test]
fn another_message_opens_its_own_paragraph_only_where_prose_would_run_into_prose() {
    assert_eq!(streamed(&run(UNIT, "followVerb", "a second message opens its own paragraph", d::follow_verb)), "first part more\n\nsecond message\ncompleted\n");
    assert_eq!(streamed(&run(UNIT, "followVerb", "a line of work between two messages", d::follow_verb)), "before\n$ ls\nafter\ncompleted\n");
}

#[test]
fn a_cut_stream_line_is_closed_before_a_line_of_work_and_before_the_reply() {
    let ran = run(UNIT, "followVerb", "prose with no trailing newline", d::follow_verb);
    assert_eq!(streamed(&ran), "Hello\ncompleted\n");
    assert_eq!(lines(&ran.sinks.stdout), ["Hello"]);
}

#[test]
fn a_call_is_forgotten_once_its_result_is_read() {
    let ran = run(UNIT, "followVerb", "a result forgets its call", d::follow_verb);
    assert_eq!(streamed(&ran), "$ ls\na\nagain\ncompleted\n");
}

#[test]
fn the_announce_is_emitted_before_the_reply_with_the_threads_own_line() {
    let ran = run(UNIT, "followVerb", "announce with a line of its own and the folder", d::follow_verb);
    assert_eq!(lines(&ran.sinks.stdout)[0], "opened thread-9d8c7b6a5f4e in /root/work  wsp thread read, wsp send and wsp stop take its first characters");
    let ran = run(UNIT, "followVerb", "announce with a line of its own and the folder (--json)", d::follow_verb);
    let first: Value = serde_json::from_str(&lines(&ran.sinks.stdout)[0]).expect("a JSON line");
    assert_eq!(first["type"], json!("thread"));
    assert_eq!(first.as_object().unwrap().keys().collect::<Vec<_>>(), ["type", "id", "workspaceId", "harness", "startedBy"]);
}

#[test]
fn a_detached_start_prints_the_thread_object_with_sub_then_notice_after_the_turn_views_keys() {
    let ran = run(UNIT, "detachVerb", "a sub-thread's answer rides it (--json)", d::detach_verb);
    let out: Value = serde_json::from_str(&lines(&ran.sinks.stdout)[0]).expect("a JSON line");
    assert_eq!(out.as_object().unwrap().keys().collect::<Vec<_>>(), ["threadId", "workspaceId", "harness", "outcome", "sub"]);
    let ran = run(UNIT, "detachVerb", "said beside the thread (--json)", d::detach_verb);
    let out: Value = serde_json::from_str(&lines(&ran.sinks.stdout)[0]).expect("a JSON line");
    assert_eq!(out.as_object().unwrap().keys().collect::<Vec<_>>(), ["threadId", "workspaceId", "harness", "outcome", "notice"]);
}

#[test]
fn a_held_turn_with_its_cap_says_the_wait_and_any_other_join_its_own_line() {
    let ran = run(UNIT, "detachVerb", "held with its cap", d::detach_verb);
    assert_eq!(lines(&ran.sinks.stderr), ["waiting while hetzner is running 2 of 2 threads"]);
    let ran = run(UNIT, "detachVerb", "steered", d::detach_verb);
    assert_eq!(lines(&ran.sinks.stderr), ["joined the running turn"]);
}

#[test]
fn a_read_that_failed_on_a_socket_the_host_stopped_is_a_message_not_delivered() {
    let ran = run(UNIT, "beforeSending", "the host stopped under the read", d::before_sending);
    assert_eq!(err(&ran).message, "the host stopped before it took the message, so it was not delivered; send it again");
    let ran = run(UNIT, "beforeSending", "the host closed under the read with another code", d::before_sending);
    assert_eq!(err(&ran).message, "the host closed the connection");
    let ran = run(UNIT, "beforeSending", "the read fails and the host did not stop", d::before_sending);
    let e = err(&ran);
    assert_eq!((e.message.as_str(), e.kind.clone()), ("no listing", Some(Kind::Auth)));
}

#[test]
fn answering_by_the_verb_refuses_where_the_thread_waits_on_no_prompt_or_the_prompt_lacks_the_answer() {
    let ran = run(UNIT, "answerOpenAsk", "the thread waits on no prompt", d::answer_open_ask);
    assert_eq!(err(&ran).message, "thread thread-9 is waiting on no prompt; wsp threads says which threads need you");
    assert_eq!(ran.ops(), ["sessions.list", "sessions.history"], "nothing is sent for a prompt nobody holds open");
    let ran = run(UNIT, "answerOpenAsk", "the prompt takes no deny", d::answer_open_ask);
    assert_eq!(err(&ran).message, "the prompt thread thread-9 is stopped on takes no deny; wsp thread read shows what it asks");
    assert_eq!(ran.ops(), ["sessions.list", "sessions.history"]);
}

#[test]
fn an_answer_carries_the_persons_words_only_where_they_were_given() {
    let ran = run(UNIT, "answerOpenAsk", "deny with the person's words", d::answer_open_ask);
    assert_eq!(ran.frame("sessions.answer", 0)["reason"], json!("use the other folder"));
    let ran = run(UNIT, "answerOpenAsk", "deny with no words", d::answer_open_ask);
    assert!(!ran.frame("sessions.answer", 0).contains_key("reason"));
    let ran = run(UNIT, "answerOpenAsk", "deny with an empty reason", d::answer_open_ask);
    assert_eq!(ran.frame("sessions.answer", 0)["reason"], json!(""), "an empty reason is still a reason");
}

#[test]
fn every_answer_that_is_not_answered_is_a_failure_in_the_hosts_words() {
    for (case, words) in [
        ("the answer lands too late", "the prompt closed before the answer reached it"),
        ("the agent raises no prompt the host can answer", "this thread's agent raises no prompt this host can answer"),
        ("the host holds no turn of that thread", "this host holds no turn of that thread"),
        ("the prompt has no option by that id", "the prompt carries no option by that id"),
    ] {
        assert_eq!(err(&run(UNIT, "answerOpenAsk", case, d::answer_open_ask)).message, words, "{case}");
    }
}
