// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/general-prefs.ts at a3b870a6e: its zod shapes and plain constants, written by the generator.

use serde::{Deserialize, Serialize};

use crate::types::check::Check;

/// TS: packages/protocol/src/general-prefs.ts:9 at a3b870a6e.
pub const SEND_KEYS: &[&str] = &["enter", "mod-enter"];

/// TS: packages/protocol/src/general-prefs.ts:14 at a3b870a6e.
pub const MID_TURN_CHOICES: &[&str] = &["queue", "steer"];

/// TS: packages/protocol/src/general-prefs.ts:19 at a3b870a6e.
pub const NOTIFY_CHOICES: &[&str] = &["off", "notify", "sound", "notify-sound"];

/// TS: packages/protocol/src/general-prefs.ts:30 at a3b870a6e.
pub const SETTLE_CHOICES: &[&str] = &["15m", "1h", "2h", "1d", "never"];

/// TS: packages/protocol/src/general-prefs.ts:40 at a3b870a6e.
pub const ON_QUIT_CHOICES: &[&str] = &["ask", "keep", "stop"];

/// TS: packages/protocol/src/general-prefs.ts:45 at a3b870a6e.
pub const NEW_THREAD_IN_CHOICES: &[&str] = &["current", "ask"];

/// TS: packages/protocol/src/general-prefs.ts:11 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SendKey {
    #[serde(rename = "enter")]
    Enter,
    #[serde(rename = "mod-enter")]
    ModEnter,
}

impl Check for SendKey {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["enter", "mod-enter"]))
    }
}

/// TS: packages/protocol/src/general-prefs.ts:16 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MidTurn {
    #[serde(rename = "queue")]
    Queue,
    #[serde(rename = "steer")]
    Steer,
}

impl Check for MidTurn {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["queue", "steer"]))
    }
}

/// TS: packages/protocol/src/general-prefs.ts:21 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NotifyChoice {
    #[serde(rename = "off")]
    Off,
    #[serde(rename = "notify")]
    Notify,
    #[serde(rename = "sound")]
    Sound,
    #[serde(rename = "notify-sound")]
    NotifySound,
}

impl Check for NotifyChoice {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["off", "notify", "sound", "notify-sound"]))
    }
}

/// TS: packages/protocol/src/general-prefs.ts:34 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SettleAfter {
    #[serde(rename = "15m")]
    V15m,
    #[serde(rename = "1h")]
    V1h,
    #[serde(rename = "2h")]
    V2h,
    #[serde(rename = "1d")]
    V1d,
    #[serde(rename = "never")]
    Never,
}

impl Check for SettleAfter {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["15m", "1h", "2h", "1d", "never"]))
    }
}

/// TS: packages/protocol/src/general-prefs.ts:42 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OnQuit {
    #[serde(rename = "ask")]
    Ask,
    #[serde(rename = "keep")]
    Keep,
    #[serde(rename = "stop")]
    Stop,
}

impl Check for OnQuit {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["ask", "keep", "stop"]))
    }
}

/// TS: packages/protocol/src/general-prefs.ts:47 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NewThreadIn {
    #[serde(rename = "current")]
    Current,
    #[serde(rename = "ask")]
    Ask,
}

impl Check for NewThreadIn {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["current", "ask"]))
    }
}

// ---- Hand-written below: the functions of the unit.

// general-prefs.ts:51 at a3b870a6e
// export const GENERAL_DEFAULTS = {
//   sendWith: "enter",
//   midTurn: "queue",
//   notifyNeeds: "notify-sound",
//   notifyDone: "notify",
//   planAlerts: true,
//   settleAfter: "2h",
//   askDelete: true,
//   onQuit: "ask",
//   newThreadIn: "current",
// } as const satisfies Record<string, unknown>;
/// The choices of Settings > General, as the preferences record carries them, in GENERAL_FIELDS' order.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralPreferences {
    pub send_with: SendKey,
    pub mid_turn: MidTurn,
    pub notify_needs: NotifyChoice,
    pub notify_done: NotifyChoice,
    pub plan_alerts: bool,
    pub settle_after: SettleAfter,
    pub ask_delete: bool,
    pub on_quit: OnQuit,
    pub new_thread_in: NewThreadIn,
}

/// A patch of them: an absent choice keeps the current one.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GeneralPatch {
    pub send_with: Option<SendKey>,
    pub mid_turn: Option<MidTurn>,
    pub notify_needs: Option<NotifyChoice>,
    pub notify_done: Option<NotifyChoice>,
    pub plan_alerts: Option<bool>,
    pub settle_after: Option<SettleAfter>,
    pub ask_delete: Option<bool>,
    pub on_quit: Option<OnQuit>,
    pub new_thread_in: Option<NewThreadIn>,
}

/// GENERAL_DEFAULTS: enter, queue, notify-sound, notify, true, 2h, true, ask, current.
pub fn general_defaults() -> GeneralPreferences {
    GeneralPreferences {
        send_with: SendKey::Enter,
        mid_turn: MidTurn::Queue,
        notify_needs: NotifyChoice::NotifySound,
        notify_done: NotifyChoice::Notify,
        plan_alerts: true,
        settle_after: SettleAfter::V2h,
        ask_delete: true,
        on_quit: OnQuit::Ask,
        new_thread_in: NewThreadIn::Current,
    }
}

// general-prefs.ts:79 at a3b870a6e
// export function patchedGeneral(current: GeneralPreferences, patch: Partial<GeneralPreferences>): GeneralPreferences {
//   return {
//     sendWith: patch.sendWith ?? current.sendWith,
//     midTurn: patch.midTurn ?? current.midTurn,
//     notifyNeeds: patch.notifyNeeds ?? current.notifyNeeds,
//     notifyDone: patch.notifyDone ?? current.notifyDone,
//     planAlerts: patch.planAlerts ?? current.planAlerts,
//     settleAfter: patch.settleAfter ?? current.settleAfter,
//     askDelete: patch.askDelete ?? current.askDelete,
//     onQuit: patch.onQuit ?? current.onQuit,
//     newThreadIn: patch.newThreadIn ?? current.newThreadIn,
//   };
// }
pub fn patched_general(current: &GeneralPreferences, patch: &GeneralPatch) -> GeneralPreferences {
    GeneralPreferences {
        send_with: patch.send_with.unwrap_or(current.send_with),
        mid_turn: patch.mid_turn.unwrap_or(current.mid_turn),
        notify_needs: patch.notify_needs.unwrap_or(current.notify_needs),
        notify_done: patch.notify_done.unwrap_or(current.notify_done),
        plan_alerts: patch.plan_alerts.unwrap_or(current.plan_alerts),
        settle_after: patch.settle_after.unwrap_or(current.settle_after),
        ask_delete: patch.ask_delete.unwrap_or(current.ask_delete),
        on_quit: patch.on_quit.unwrap_or(current.on_quit),
        new_thread_in: patch.new_thread_in.unwrap_or(current.new_thread_in),
    }
}
