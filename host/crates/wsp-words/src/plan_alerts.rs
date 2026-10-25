// SPDX-License-Identifier: AGPL-3.0-only
//! When an account's plan running low is worth telling the person, and the line it is told in. Port of the functions of
//! packages/protocol/src/plan-alerts.ts at a3b870a6e (PlanAlert, PLAN_ALERT_STEPS and UsageAlertEvent are
//! wsp_wire::types::plan_alerts). See spec/costs_and_alerts.md.

use wsp_wire::types::js_number::JsNumber;
use wsp_wire::types::plan_alerts::PlanAlert;
use wsp_wire::types::plan_alerts::{PlanAlertBack, PlanAlertBlocked, PlanAlertLow};
use wsp_wire::types::usage::{AccountLimit, LimitKind, LimitStatus, LimitWindow};

// The share of a window used at which the person is told it runs low.
// plan-alerts.ts:10
// export const PLAN_ALERT_STEPS = [70, 90] as const;
// in wsp_wire::types::plan_alerts: PLAN_ALERT_STEPS

// plan-alerts.ts:12
// export const PlanAlert = z.discriminatedUnion("kind", [
//   z.object({ kind: z.literal("low"), window: LimitKind, step: z.number() }),
//   z.object({ kind: z.literal("blocked") }),
//   z.object({ kind: z.literal("back") }),
// ]);
// in wsp_wire::types::plan_alerts: PlanAlert

// plan-alerts.ts:17
// export type PlanAlert = z.infer<typeof PlanAlert>;

// plan-alerts.ts:19
// type Reading = Pick<AccountLimit, "windows" | "status">;
/// The two fields of an account limit the alerts read.
pub type Reading = AccountLimit;

// Overage is use past the plan, not a window of it, so it neither runs low nor blocks.
// plan-alerts.ts:22
// private in the TS: port it as a private helper
// const counted = (w: LimitWindow): boolean => w.kind !== "overage";

/// Whether the account is blocked: its agent said so, or a window of its plan is used up.
// plan-alerts.ts:25
// export const planBlocked = (r: Reading): boolean => r.status === "reached" || r.windows.some(w => counted(w) && w.usedPercent >= 100);
pub fn plan_blocked(r: &Reading) -> bool {
    matches!(r.status, Some(LimitStatus::Reached)) || r.windows.iter().any(|w| counted(w) && w.used_percent.0 >= 100.0)
}

/// When a blocked account starts again: the latest reset among its used-up windows, else among all of them; nothing
/// where it is not blocked or no window named a reset.
// plan-alerts.ts:29
// export function blockEndsAt(r: Reading): number | undefined {
//   if (!planBlocked(r)) return undefined;
//   const resets = (ws: readonly LimitWindow[]): number[] => ws.flatMap(w => (counted(w) && w.resetsAt !== undefined ? [w.resetsAt] : []));
//   const ends = resets(r.windows.filter(w => w.usedPercent >= 100));
//   const all = ends.length > 0 ? ends : resets(r.windows);
//   return all.length === 0 ? undefined : Math.max(...all);
// }
pub fn block_ends_at(r: &Reading) -> Option<f64> {
    if !plan_blocked(r) {
        return None;
    }
    let resets = |windows: Vec<&LimitWindow>| -> Vec<f64> {
        windows.into_iter().filter(|w| counted(w)).filter_map(|w| w.resets_at.as_ref().map(|at| at.0)).collect()
    };
    let ends = resets(r.windows.iter().filter(|w| w.used_percent.0 >= 100.0).collect());
    let all = if ends.is_empty() { resets(r.windows.iter().collect()) } else { ends };
    all.into_iter().reduce(f64::max)
}

// How much of a window the reading before used, 0 where there was none or that window has started again since.
// plan-alerts.ts:38
// private in the TS: port it as a private helper
// function usedBefore(before: Reading | undefined, w: LimitWindow, at: number): number {
//   const prior = before?.windows.find(p => p.kind === w.kind);
//   if (prior === undefined || (prior.resetsAt !== undefined && prior.resetsAt <= at)) return 0;
//   return prior.usedPercent;
// }

/// What one reading tells the person against the one before it. A block says itself and nothing lower, a window that
/// crossed both steps in one reading says the higher, and a reading no longer blocked after a blocked one says the
/// account is back.
// plan-alerts.ts:47
// export function planAlertsOf(before: Reading | undefined, after: Reading, at: number): PlanAlert[] {
//   const was = before !== undefined && planBlocked(before);
//   if (planBlocked(after)) return was ? [] : [{ kind: "blocked" }];
//   if (was) return [{ kind: "back" }];
//   return after.windows.filter(counted).flatMap((w): PlanAlert[] => {
//     const prior = usedBefore(before, w, at);
//     const step = [...PLAN_ALERT_STEPS].reverse().find(s => prior < s && w.usedPercent >= s);
//     return step === undefined ? [] : [{ kind: "low", window: w.kind, step }];
//   });
// }
pub fn plan_alerts_of(before: Option<&Reading>, after: &Reading, at: f64) -> Vec<PlanAlert> {
    let was = before.is_some_and(plan_blocked);
    if plan_blocked(after) {
        return if was { Vec::new() } else { vec![PlanAlert::Blocked(PlanAlertBlocked {})] };
    }
    if was {
        return vec![PlanAlert::Back(PlanAlertBack {})];
    }
    after
        .windows
        .iter()
        .filter(|w| counted(w))
        .filter_map(|w| {
            let prior = used_before(before, w, at);
            [90.0, 70.0]
                .into_iter()
                .find(|step| prior < *step && w.used_percent.0 >= *step)
                .map(|step| PlanAlert::Low(PlanAlertLow { window: w.kind, step: JsNumber(step) }))
        })
        .collect()
}

// plan-alerts.ts:58
// private in the TS: port it as a private helper
// const WINDOW_WORDS: Readonly<Record<LimitKind, string>> = { session: "session", week: "week", week_opus: "Opus week", week_sonnet: "Sonnet week", month: "month", overage: "extra usage" };

/// The notice and the notification's line for an alert on the account the label names.
// plan-alerts.ts:61
// export function planAlertLine(label: string, alert: PlanAlert): string {
//   if (alert.kind === "low") return `${label} has used ${alert.step}% of its ${WINDOW_WORDS[alert.window]}`;
//   return alert.kind === "blocked" ? `${label} reached its plan limit` : `${label} can run again: its plan limit reset`;
// }
pub fn plan_alert_line(label: &str, alert: &PlanAlert) -> String {
    match alert {
        PlanAlert::Low(low) => format!("{label} has used {}% of its {}", crate::js::number_string(low.step.0), window_word(&low.window)),
        PlanAlert::Blocked(_) => format!("{label} reached its plan limit"),
        PlanAlert::Back(_) => format!("{label} can run again: its plan limit reset"),
    }
}

// An account's plan crossed a step, was blocked or came back, for every client to tell the person as they chose.
// plan-alerts.ts:67
// export const UsageAlertEvent = z.object({ type: z.literal("usage.alert"), key: z.string(), agent: z.string(), label: z.string(), alert: PlanAlert });
// in wsp_wire::types::plan_alerts: UsageAlertEvent

// plan-alerts.ts:68
// export type UsageAlertEvent = z.infer<typeof UsageAlertEvent>;

// Overage is use past the plan, not a window of it, so it neither runs low nor blocks.
fn counted(w: &LimitWindow) -> bool {
    !matches!(w.kind, LimitKind::Overage)
}

// How much of a window the reading before used, 0 where there was none or that window has started again since.
fn used_before(before: Option<&Reading>, w: &LimitWindow, at: f64) -> f64 {
    let Some(prior) = before.and_then(|before| before.windows.iter().find(|p| p.kind == w.kind)) else {
        return 0.0;
    };
    match &prior.resets_at {
        Some(reset) if reset.0 <= at => 0.0,
        _ => prior.used_percent.0,
    }
}

fn window_word(kind: &LimitKind) -> &'static str {
    match kind {
        LimitKind::Session => "session",
        LimitKind::Week => "week",
        LimitKind::WeekOpus => "Opus week",
        LimitKind::WeekSonnet => "Sonnet week",
        LimitKind::Month => "month",
        LimitKind::Overage => "extra usage",
    }
}
