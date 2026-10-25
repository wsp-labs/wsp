// SPDX-License-Identifier: AGPL-3.0-only
//! The shapes of plan-alerts.ts against what zod answered for the same inputs. Generated.

use super::common::{cases, cases_tagged};
use wsp_wire::types::plan_alerts::*;

const FIXTURES: &str = include_str!("../fixtures/shapes/types_plan_alerts.json");

/// 4 cases: what zod's parse of PlanAlert wrote, or the first issue it raised.
#[test]
fn plan_alert_reads_and_writes_as_zod_does() {
    cases::<PlanAlert>(FIXTURES, "PlanAlert");
}

/// 3 cases: what zod's parse of UsageAlertEvent wrote, or the first issue it raised.
#[test]
fn usage_alert_event_reads_and_writes_as_zod_does() {
    cases_tagged::<UsageAlertEvent>(FIXTURES, "UsageAlertEvent", "type", UsageAlertEvent::TYPE);
}
