// SPDX-License-Identifier: AGPL-3.0-only
//! What a moment says outside the app, and under which of the person's choices on General. Port of the functions of
//! packages/protocol/src/outside-line.ts at a3b870a6e (OutsideLine and OUTSIDE_HELD are wsp_wire::types::outside_line).
//! See spec/costs_and_alerts.md.

use wsp_wire::types::general_prefs::GeneralPreferences;
use wsp_wire::types::outside_line::OutsideLine;
use wsp_wire::types::plan_alerts::PlanAlert;

use crate::general_prefs::{notify_by, NotifyHow};

// One moment said outside the app: its title, its line, whether a notification shows it, and whether it makes a
// sound, which with no notification is the sound alone. The id is the page's own for the line, handed back on a
// click so the click opens what that line was about and not whatever was said after it.
// outside-line.ts:13
// export const OutsideLine = z.object({ title: z.string(), body: z.string(), show: z.boolean(), sound: z.boolean(), id: z.string().optional() });
// in wsp_wire::types::outside_line: OutsideLine

// outside-line.ts:14
// export type OutsideLine = z.infer<typeof OutsideLine>;

// outside-line.ts:18
// in wsp_wire::types::outside_line: OUTSIDE_HELD

/// The moments said outside the app, each with what its line is made of.
// outside-line.ts:21
// export type OutsideMoment =
//   | { kind: "asks"; line: string }
//   | { kind: "signIn"; what: string }
//   | { kind: "finished"; thread: string; where: string }
//   | { kind: "failed"; thread: string; where: string; error?: string | undefined }
//   | { kind: "awake"; workspace: string }
//   | { kind: "plan"; label: string; alert: PlanAlert }
//   | { kind: "setupFailed"; computer: string; said: string }
//   | { kind: "setupNeedsYou"; computer: string; what: string }
//   | { kind: "setupReady"; computer: string; missed?: string }
//   | { kind: "imageReady"; computer: string; image: string };
/// One moment said outside the app (outside-line.ts:20-30), a variant per TS `kind`.
#[derive(Clone, Debug, PartialEq)]
pub enum OutsideMoment {
    Asks { line: String },
    SignIn { what: String },
    Finished { thread: String, r#where: String },
    Failed { thread: String, r#where: String, error: Option<String> },
    Awake { workspace: String },
    Plan { label: String, alert: PlanAlert },
    SetupFailed { computer: String, said: String },
    SetupNeedsYou { computer: String, what: String },
    SetupReady { computer: String, missed: Option<String> },
    ImageReady { computer: String, image: String },
}

/// What a setup's three moments say.
// outside-line.ts:34
// export const setupFailedLine = (computer: string): string => `${computer}: setup failed`;
pub fn setup_failed_line(computer: &str) -> String {
    format!("{computer}: setup failed")
}

// outside-line.ts:35
// export const setupNeedsYouLine = (computer: string, what: string): string => `${computer} needs you: ${what}`;
pub fn setup_needs_you_line(computer: &str, what: &str) -> String {
    format!("{computer} needs you: {what}")
}

// outside-line.ts:36
// export const setupReadyLine = (computer: string, missed?: string): string => `${computer} is ready${missed === undefined ? "" : `; ${missed}`}`;
pub fn setup_ready_line(computer: &str, missed: Option<&str>) -> String {
    match missed {
        None => format!("{computer} is ready"),
        Some(missed) => format!("{computer} is ready; {missed}"),
    }
}

// outside-line.ts:38
// private in the TS: port it as a private helper
// const said = (how: { show: boolean; sound: boolean } | undefined, title: string, body: string): OutsideLine | undefined => (how === undefined ? undefined : { title, body, ...how });

/// The line a moment makes under the person's choices, or nothing where they chose not to hear it. A prompt, a
/// question and a sign-in need the person; a finish, a failure and a machine that came up are things that finished;
/// a plan alert has its own switch and shows with no sound.
// outside-line.ts:43
// export function outsideLine(moment: OutsideMoment, choices: Pick<GeneralPreferences, "notifyNeeds" | "notifyDone" | "planAlerts">): OutsideLine | undefined {
//   switch (moment.kind) {
//     case "asks":
//       return said(notifyBy(choices.notifyNeeds), NEEDS_YOU, moment.line);
//     case "signIn":
//       return said(notifyBy(choices.notifyNeeds), NEEDS_YOU, moment.what);
//     case "finished":
//       return said(notifyBy(choices.notifyDone), threadFinishedLine(moment.thread), moment.where);
//     case "failed":
//       return said(notifyBy(choices.notifyDone), threadStoppedLine(moment.thread), moment.error === undefined ? moment.where : oneLine(moment.error));
//     case "awake":
//       return said(notifyBy(choices.notifyDone), NEEDS_YOU, workspaceAwakeLine(moment.workspace));
//     case "plan":
//       return said(choices.planAlerts ? { show: true, sound: false } : undefined, planAlertLine(moment.label, moment.alert), "");
//     case "setupFailed":
//       return said(notifyBy(choices.notifyNeeds), setupFailedLine(moment.computer), oneLine(moment.said));
//     case "setupNeedsYou":
//       return said(notifyBy(choices.notifyNeeds), NEEDS_YOU, setupNeedsYouLine(moment.computer, moment.what));
//     case "setupReady":
//       return said(notifyBy(choices.notifyDone), setupReadyLine(moment.computer, moment.missed), "");
//     case "imageReady":
//       return said(notifyBy(choices.notifyDone), setupReadyLine(moment.computer), moment.image);
//   }
// }
pub fn outside_line(moment: &OutsideMoment, choices: &GeneralPreferences) -> Option<OutsideLine> {
    let needs = notify_by(choices.notify_needs);
    let done = notify_by(choices.notify_done);
    let said = |how: Option<NotifyHow>, title: String, body: String| {
        how.map(|how| OutsideLine { title, body, show: how.show, sound: how.sound, id: None })
    };
    match moment {
        OutsideMoment::Asks { line } => said(needs, crate::words::init::NEEDS_YOU.to_string(), line.clone()),
        OutsideMoment::SignIn { what } => said(needs, crate::words::init::NEEDS_YOU.to_string(), what.clone()),
        OutsideMoment::Finished { thread, r#where } => said(done, crate::words::places::thread_finished_line(thread), r#where.clone()),
        OutsideMoment::Failed { thread, r#where, error } => said(
            done,
            crate::words::places::thread_stopped_line(thread),
            error.as_deref().map(crate::quote::one_line).unwrap_or_else(|| r#where.clone()),
        ),
        OutsideMoment::Awake { workspace } => {
            said(done, crate::words::init::NEEDS_YOU.to_string(), crate::words::computer::workspace_awake_line(workspace))
        }
        OutsideMoment::Plan { label, alert } => {
            let how = choices.plan_alerts.then_some(NotifyHow { show: true, sound: false });
            said(how, crate::plan_alerts::plan_alert_line(label, alert), String::new())
        }
        OutsideMoment::SetupFailed { computer, said: error } => said(needs, setup_failed_line(computer), crate::quote::one_line(error)),
        OutsideMoment::SetupNeedsYou { computer, what } => {
            said(needs, crate::words::init::NEEDS_YOU.to_string(), setup_needs_you_line(computer, what))
        }
        OutsideMoment::SetupReady { computer, missed } => said(done, setup_ready_line(computer, missed.as_deref()), String::new()),
        OutsideMoment::ImageReady { computer, image } => said(done, setup_ready_line(computer, None), image.clone()),
    }
}
