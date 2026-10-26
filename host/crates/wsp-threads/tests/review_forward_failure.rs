// SPDX-License-Identifier: AGPL-3.0-only
//! Review of wsp-threads (#2448), paths 1 and 2: what failed a turn's stream, and whether the computer was silent.
//! turns.ts:547 keeps `event.failure` itself on `turnLive.failure`, and `failureOf(handle)` (threads.ts:658) hands that
//! same error to `computerSilent` (threads.ts:782), which reads its class (MachineUnreached, MachineUnreachableError,
//! DeadlineError, LaunchUnanswered, a place that is absent, a daemon that did not answer). A line whose try ended on
//! such a failure goes again on the clock for an hour (threads.ts:728-739).
//! The port turns the failure into `HostError::new(failure.message)` (forward.rs:564), which carries no mark, so
//! `computer_silent` reads false for every kind and the line falls to the person after one try.
#![allow(dead_code, unused_imports)]

#[path = "support_a/mod.rs"]
mod support;
include!("turns_run/world.rs");

use wsp_threads::threads::threads::computer_silent;

fn ended_with(kind: ExecFailureKind) -> Option<HostError> {
    let rig = rig();
    let mut end = adapter_event(json!({ "type": "session.end", "sessionId": SESS, "exitCode": null, "sawResult": false }));
    if let AdapterEvent::SessionEnd { failure, .. } = &mut end {
        *failure = Some(ExecFailure { kind, message: "the stream gave out".into() });
    }
    (rig.forward)(end);
    let live = rig.row().borrow().turn_live.clone().expect("the row's live state");
    live.failure
}

/// Every kind of failure that says the computer did not answer reads as silent on the row, as the class does in TS.
#[test]
fn a_failure_that_says_the_computer_was_silent_is_kept_as_silent() {
    run_local(async {
        let silent_kinds = [
            ExecFailureKind::Unreached,
            ExecFailureKind::Unreachable,
            ExecFailureKind::PlaceAbsent,
            ExecFailureKind::LaunchUnanswered,
            ExecFailureKind::Deadline,
            ExecFailureKind::NoAnswer,
        ];
        let lost: Vec<String> =
            silent_kinds.into_iter().filter(|kind| !computer_silent(ended_with(*kind).as_ref())).map(|kind| format!("{kind:?}")).collect();
        assert_eq!(lost, Vec::<String>::new(), "kinds whose mark the port dropped, so the line is not tried again");
    });
}

/// And a refusal stays what it was: not silent, so the line falls to the person at once.
#[test]
fn any_other_failure_is_not_silent() {
    run_local(async {
        assert!(!computer_silent(ended_with(ExecFailureKind::Other).as_ref()));
    });
}
