// SPDX-License-Identifier: AGPL-3.0-only
//! Solari's pause and resume settling, rules P5 to P7 and W4 in spec/solari_machine.md: which state read after a call
//! that did not answer earns the second send, and the exact warn line each failed move leaves.
#[path = "rest_common/mod.rs"]
mod common;

use std::sync::Arc;

use serde_json::json;
use wsp_engine::errors::EngineError;
use wsp_engine::machine::{Machine, MachineBackend, MachineSpec};
use wsp_engine::solari_backend::{MoveBudgetsGiven, SolariBackend, SolariBackendOptions};
use wsp_words::words::computer::RESUME_UNANSWERED;

use common::{fetch_failed, reply, FakeFetch, Play, Warned};

const PAUSE: &str = "POST /sandboxes/x/pause";
const RESUME: &str = "POST /sandboxes/x/resume";
const STATE: &str = "GET /sandboxes/x";

fn reads(state: &str) -> Play {
    reply(200, json!({ "sandboxId": "x", "kind": "sandbox", "state": state }))
}

fn not_found() -> Play {
    reply(404, json!({ "error": "sandbox not found" }))
}

async fn machine(routes: Vec<(&str, Vec<Play>)>, warned: &Warned) -> (Arc<FakeFetch>, Arc<dyn Machine>) {
    let mut all = vec![("POST /sandboxes", vec![reply(201, json!({ "sandboxId": "x", "kind": "sandbox" }))])];
    all.extend(routes);
    let f = FakeFetch::new(all);
    let b = SolariBackend::new(SolariBackendOptions {
        api_key: "k".into(),
        fetch: Some(f.clone()),
        budgets: MoveBudgetsGiven { pause_ms: Some(40.0), resume_cap_ms: Some(20.0), state_read_ms: Some(20.0), read_ms: None },
        warn: Some(warned.sink()),
        ..Default::default()
    });
    let spec: MachineSpec = serde_json::from_value(json!({ "kind": "sandbox" })).unwrap();
    let m = b.create(spec, None, None).await.unwrap();
    (f, m)
}

/// `pause did not complete in <n>ms; the provider did not answer and <rest>; try again`, the milliseconds a clock's.
fn assert_pause_words(message: &str, rest: &str) {
    let after = message.strip_prefix("pause did not complete in ").unwrap_or_else(|| panic!("not the pause words: {message}"));
    let digits = after.chars().take_while(|c| c.is_ascii_digit()).count();
    assert!(digits > 0, "no milliseconds in: {message}");
    assert_eq!(&after[digits..], format!("ms; the provider did not answer and {rest}; try again"));
}

/// P5: only `running` earns the second send. A call that did not answer followed by a read of `starting`, of a word
/// the table maps to `gone` (`archived`), or of a 404 ends the pause at once: one send, one read, the words name the
/// state, and the one warn line is `<id>: <those words>`.
#[tokio::test]
async fn a_pause_read_as_anything_but_running_or_paused_ends_at_once_with_one_warn_line() {
    for (read, word) in [(reads("starting"), "reads the machine starting"), (reads("archived"), "reads the machine gone"), (not_found(), "reads the machine gone")] {
        let warned = Warned::default();
        let (f, m) = machine(vec![(PAUSE, vec![Play::Hangs]), (STATE, vec![read])], &warned).await;
        let err = m.pause(None).await.unwrap_err();
        assert!(matches!(err, EngineError::MoveUnanswered { .. }), "{err:?}");
        assert_pause_words(&err.to_string(), word);
        assert_eq!(f.sent(PAUSE), 1, "{word}");
        assert_eq!(f.sent(STATE), 1, "{word}");
        assert_eq!(warned.lines(), vec![format!("x: {err}")]);
    }
}

/// P6: a state word the table never learned reads `running` (never `gone`), so it earns the second send; that send
/// not answering either and the next read saying `paused` is a pause, with no warn line.
#[tokio::test]
async fn an_unknown_state_word_reads_running_and_earns_the_second_send() {
    let warned = Warned::default();
    let (f, m) = machine(vec![(PAUSE, vec![Play::Hangs, Play::Hangs]), (STATE, vec![reads("hibernating"), reads("paused")])], &warned).await;
    m.pause(None).await.unwrap();
    assert_eq!(f.sent(PAUSE), 2);
    assert_eq!(f.sent(STATE), 2);
    assert_eq!(warned.lines(), Vec::<String>::new());
}

/// P7: a pause the provider answers, or refuses with its own words, warns nothing.
#[tokio::test]
async fn a_pause_that_lands_or_is_refused_warns_nothing() {
    let warned = Warned::default();
    let (_, m) = machine(vec![(PAUSE, vec![common::reply_text(200, "{}")])], &warned).await;
    m.pause(None).await.unwrap();
    let (_, refused) = machine(vec![(PAUSE, vec![reply(409, json!({ "error": "Not pausable" }))])], &warned).await;
    assert!(matches!(refused.pause(None).await.unwrap_err(), EngineError::NapRefused { .. }));
    assert_eq!(warned.lines(), Vec::<String>::new());
}

/// W4: a resume the read does not settle warns once, `<id>: <RESUME_UNANSWERED>, which still reads <state>`, the
/// state as read (`paused`, `gone` for a 404) or `nothing` where the read failed; `running` and `starting` warn
/// nothing.
#[tokio::test]
async fn a_resume_left_unsettled_warns_once_with_what_the_machine_still_reads() {
    for (read, word) in [(reads("paused"), "paused"), (not_found(), "gone"), (Play::Fail(fetch_failed()), "nothing")] {
        let warned = Warned::default();
        let (f, m) = machine(vec![(RESUME, vec![Play::Hangs]), (STATE, vec![read])], &warned).await;
        let err = m.resume(None).await.unwrap_err();
        assert!(matches!(err, EngineError::ResumeUnanswered { .. }), "{err:?}");
        assert_eq!(err.to_string(), RESUME_UNANSWERED);
        assert_eq!(f.sent(RESUME), 1);
        assert_eq!(f.sent(STATE), 1);
        assert_eq!(warned.lines(), vec![format!("x: {RESUME_UNANSWERED}, which still reads {word}")]);
    }
    for state in ["running", "starting"] {
        let warned = Warned::default();
        let (_, m) = machine(vec![(RESUME, vec![Play::Hangs]), (STATE, vec![reads(state)])], &warned).await;
        m.resume(None).await.unwrap();
        assert_eq!(warned.lines(), Vec::<String>::new());
    }
}
