// SPDX-License-Identifier: AGPL-3.0-only
//! How a notify choice is said outside the app. Port of the one function of packages/protocol/src/general-prefs.ts at
//! the cut (383708c78) that wsp_wire::types::general_prefs leaves to this crate. See spec/costs_and_alerts.md.

use wsp_wire::types::general_prefs::NotifyChoice;

/// Whether a notification shows and whether it sounds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NotifyHow {
    pub show: bool,
    pub sound: bool,
}

/// off: None; notify: show alone; sound: sound alone; notify-sound: both.
// general-prefs.ts:25
// export function notifyBy(choice: NotifyChoice): { show: boolean; sound: boolean } | undefined {
//   if (choice === "off") return undefined;
//   return { show: choice !== "sound", sound: choice !== "notify" };
// }
pub fn notify_by(choice: NotifyChoice) -> Option<NotifyHow> {
    if matches!(choice, NotifyChoice::Off) {
        return None;
    }
    Some(NotifyHow { show: !matches!(choice, NotifyChoice::Sound), sound: !matches!(choice, NotifyChoice::Notify) })
}
