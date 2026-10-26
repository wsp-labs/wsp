// Review of #2447: TS reset.ts:36 and command.ts:257 throw, and runtime/src/usage-reset.ts:147-151 refuses with the words.
use wsp_adapters::codex::reset::CodexPlanResets;
use wsp_adapters::port::{PlanResets, ResetRoad};

fn road(home: &str) -> ResetRoad {
    ResetRoad { home: home.to_string(), env: None, launch: None }
}

// Compiles today and fails: a usage.reset frame's creditId reaches spend_command, and the one-thread host panics.
#[test]
fn a_credit_id_that_is_no_slug_does_not_panic_the_host() {
    let resets: &dyn PlanResets = &CodexPlanResets;
    let got = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| resets.spend_command(&road("/root/.codex"), "k-1", Some("rc 1"))));
    assert!(got.is_ok(), "spend_command panicked where reset.ts:36 throws a refusal");
}
