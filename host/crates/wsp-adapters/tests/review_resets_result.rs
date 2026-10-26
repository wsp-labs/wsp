// Review of #2447: the reset commands refuse with the TS words once PlanResets answers Result. Red at compile time until then.
use wsp_adapters::codex::reset::CodexPlanResets;
use wsp_adapters::port::{PlanResets, ResetRoad};

fn road(home: &str) -> ResetRoad {
    ResetRoad { home: home.to_string(), env: None, launch: None }
}

// The answer the TS needs once PlanResets answers Result (reset.ts:36, command.ts:68 and :258, words printed by node).
#[test]
fn the_reset_commands_refuse_with_the_ts_words() {
    let resets: &dyn PlanResets = &CodexPlanResets;
    assert_eq!(
        resets.spend_command(&road("/root/.codex"), "k-1", Some("rc 1")),
        Err("creditId must be a plain slug, got \"rc 1\"".to_string())
    );
    assert_eq!(
        resets.spend_command(&road("/root/.codex"), "k 1", None),
        Err("idempotencyKey must be a plain slug, got \"k 1\"".to_string())
    );
    assert_eq!(resets.read_command(&road("relative")), Err("home must be an absolute path, got \"relative\"".to_string()));
    assert!(resets.spend_command(&road("/root/.codex"), "k-1", Some("rc_1")).is_ok());
}
