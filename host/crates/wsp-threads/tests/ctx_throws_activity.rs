// SPDX-License-Identifier: AGPL-3.0-only
//! The context's `refusalFor` throws where a record's project is not held (account/rules.ts:174 reads `projectHeld`);
//! activity.ts:209 compares its answer with no catch, so the read rejects with it.
#![allow(dead_code, unused_imports)]

#[path = "support_a/mod.rs"]
mod support;
include!("activity/world.rs");

// activity.ts:209: a thread's reach reads the rule for its workspace, and the throw is the answer.
#[test]
fn a_rule_that_throws_fails_the_read_of_a_thread_as_it_is() {
    run_local(async {
        let lab = Lab::main();
        lab.host.hooks.set::<dyn Fn(&WorkspaceRecord, Option<&Caller>) -> Result<Option<HostError>, HostError>>(
            "ActivityCtx::refusal_for",
            Rc::new(|_r: &WorkspaceRecord, _c: Option<&Caller>| Err(HostError::new("no project p1"))),
        );
        let said = said_of(lab.read(ask_of(Some(T1), None), None).await);
        assert_eq!(said, ("no project p1".to_string(), None, None));
    });
}

// activity.ts:358: the same read opens the plan of an end, and a rule that throws ends nothing.
#[test]
fn a_rule_that_throws_fails_the_end_and_signals_nothing() {
    run_local(async {
        let lab = Lab::main();
        lab.host.hooks.set::<dyn Fn(&WorkspaceRecord, Option<&Caller>) -> Result<Option<HostError>, HostError>>(
            "ActivityCtx::refusal_for",
            Rc::new(|_r: &WorkspaceRecord, _c: Option<&Caller>| Err(HostError::new("no project p1"))),
        );
        let said = said_of(lab.end(4700, STARTED + 7_000, None).await);
        assert_eq!(said.0, "no project p1");
        assert_eq!(lab.here.signals.borrow().clone(), vec![]);
    });
}
