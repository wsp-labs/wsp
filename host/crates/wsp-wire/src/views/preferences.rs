// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/views/preferences.ts at a3b870a6e: its zod shapes and plain constants, written by the generator.

use serde::{Deserialize, Serialize};

use crate::types::check::from_json;
use crate::types::check::{Check, SchemaError};
use crate::types::general_prefs::MidTurn;
use crate::types::general_prefs::NewThreadIn;
use crate::types::general_prefs::NotifyChoice;
use crate::types::general_prefs::OnQuit;
use crate::types::general_prefs::SendKey;
use crate::types::general_prefs::SettleAfter;
use crate::types::js_number::JsInt;
use crate::types::js_number::JsNumber;
use crate::types::project_look::ProjectHue;
use crate::types::project_look::ProjectIcon;
use crate::types::project_look::ProjectIconHash;
use crate::types::thread_defaults::AgentDefaults;
use crate::types::thread_defaults::AgentDefaultsPatch;
use crate::types::thread_defaults::ProjectOverrides;
use crate::types::thread_defaults::ProjectOverridesPatch;
use crate::types::usage::TurnLimit;
use crate::views::session::ForkedFrom;
use crate::views::session::SessionOrigin;
use crate::views::session::SessionStatus;
use crate::views::session::ThreadCapWait;
use crate::views::session::ThreadPlacement;
use crate::views::session::ThreadWaitingOn;
use crate::views::session_events::SessionEvent;
use crate::views::sub_row::SubRow;
use indexmap::IndexMap;
use wsp_frames::True;

/// TS: packages/protocol/src/views/preferences.ts:73 at a3b870a6e.
pub const SSH_BEHIND_KIND: &str = "sshBehind";

/// TS: packages/protocol/src/views/preferences.ts:77 at a3b870a6e.
pub const SSH_TICKET_REFUSAL: &str =
    "a socket let in on a ticket cannot reach a workspace's ssh; use the wsp command or the app on the computer the host runs on";

/// TS: packages/protocol/src/views/preferences.ts:80 at a3b870a6e.
pub const EDITOR_TICKET_REFUSAL: &str =
    "a socket let in on a ticket cannot list or open the editors on this computer; use the app on the computer the host runs on";

/// TS: packages/protocol/src/views/preferences.ts:84 at a3b870a6e.
pub const TEXT_SIZES: &[u64] = &[13, 14, 15, 16, 17];

/// TS: packages/protocol/src/views/preferences.ts:85 at a3b870a6e.
pub const CODE_SIZES: &[u64] = &[11, 12, 13, 14, 15];

/// TS: packages/protocol/src/views/preferences.ts:307 at a3b870a6e.
pub const HEAD_BYTES: u64 = 65_536;

/// TS: packages/protocol/src/views/preferences.ts:309 at a3b870a6e.
pub const HEAD_RESULT_CHARS: u64 = 2_048;

/// TS: packages/protocol/src/views/preferences.ts:311 at a3b870a6e.
pub const HISTORY_PAGE_EVENTS: u64 = 200;

/// TS: packages/protocol/src/views/preferences.ts:312 at a3b870a6e.
pub const HISTORY_PAGE_MAX: u64 = 1_000;

/// TS: packages/protocol/src/views/preferences.ts:315 at a3b870a6e.
pub const HISTORY_PAGE_BYTES: u64 = 409_600;

/// views/preferences.ts:209 preferencesFrom's Preferences.partial(): every key optional, a missing one left absent (no default applies).

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreferencesStored {
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub theme: Option<ThemePreference>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub light_theme: Option<ThemeId>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub dark_theme: Option<ThemeId>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub sidebar_mode: Option<SidebarMode>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub sidebar_width: Option<JsInt>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub terminal_size: Option<TerminalSizeSource>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::types::kept::record_opt",
        serialize_with = "crate::types::check::js_record_opt"
    )]
    pub terminal_zoom: Option<IndexMap<String, JsInt>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::types::kept::record_opt",
        serialize_with = "crate::types::check::js_record_opt"
    )]
    pub access: Option<IndexMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub target: Option<PreferencesTarget>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::types::kept::record_opt",
        serialize_with = "crate::types::check::js_record_opt"
    )]
    pub project_look: Option<IndexMap<String, ProjectLook>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::types::kept::record_opt",
        serialize_with = "crate::types::check::js_record_opt"
    )]
    pub project_icon: Option<IndexMap<String, ProjectIconHash>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::types::kept::record_opt",
        serialize_with = "crate::types::check::js_record_opt"
    )]
    pub computer_look: Option<IndexMap<String, ComputerLook>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::types::kept::record_opt",
        serialize_with = "crate::types::check::js_record_opt"
    )]
    pub recipe_look: Option<IndexMap<String, RecipeLook>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::types::kept::record_opt",
        serialize_with = "crate::types::check::js_record_opt"
    )]
    pub threads_shut: Option<IndexMap<String, True>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub server_icons: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub agent_versions: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub usage_logs: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub product_usage: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub editor: Option<EditorId>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub update_dismissed: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub cloud_card_dismissed: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub keep_awake: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub transparency: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub project_order: Option<Vec<String>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::types::kept::record_opt",
        serialize_with = "crate::types::check::js_record_opt"
    )]
    pub keybindings: Option<IndexMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub app_font: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub code_font: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub text_size: Option<JsInt>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub code_size: Option<JsInt>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub default_agent: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::types::kept::record_opt",
        serialize_with = "crate::types::check::js_record_opt"
    )]
    pub agent_defaults: Option<IndexMap<String, Fields<AgentDefaults>>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::types::kept::record_opt",
        serialize_with = "crate::types::check::js_record_opt"
    )]
    pub project_defaults: Option<IndexMap<String, Fields<ProjectOverrides>>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub send_with: Option<SendKey>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub mid_turn: Option<MidTurn>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub notify_needs: Option<NotifyChoice>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub notify_done: Option<NotifyChoice>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub plan_alerts: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub settle_after: Option<SettleAfter>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub ask_delete: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub on_quit: Option<OnQuit>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub new_thread_in: Option<NewThreadIn>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub labs: Option<bool>,
}

impl Check for PreferencesStored {
    /// Rules: sidebarWidth: > 0; keybindings{}: at least 1 UTF-16 units; keybindings{}: at most 64 UTF-16 units; appFont: at most 128 UTF-16 units; codeFont: at most 128 UTF-16 units; textSize: a refine (spec/shapes.md rule 7); codeSize: a refine (spec/shapes.md rule 7).
    /// Nested checks: theme, lightTheme, darkTheme, sidebarMode, terminalSize, target, projectLook, projectIcon, computerLook, recipeLook, editor, agentDefaults, projectDefaults, sendWith, midTurn, notifyNeeds, notifyDone, settleAfter, onQuit, newThreadIn.
    fn check(&self) -> Result<(), SchemaError> {
        self.theme.check().map_err(|e| e.under("theme"))?;
        self.sidebar_mode.check().map_err(|e| e.under("sidebarMode"))?;
        if let Some(v) = self.sidebar_width {
            crate::types::check::num_above(v.0 .0, 0.0).map_err(|e| e.under("sidebarWidth"))?;
        }
        self.terminal_size.check().map_err(|e| e.under("terminalSize"))?;
        self.target.check().map_err(|e| e.under("target"))?;
        self.project_look.check().map_err(|e| e.under("projectLook"))?;
        self.computer_look.check().map_err(|e| e.under("computerLook"))?;
        self.recipe_look.check().map_err(|e| e.under("recipeLook"))?;
        self.editor.check().map_err(|e| e.under("editor"))?;
        if let Some(list) = &self.keybindings {
            for (k, item) in list {
                crate::types::check::text_at_least(item, 1).map_err(|e| e.under(k.as_str()).under("keybindings"))?;
            }
        }
        if let Some(list) = &self.keybindings {
            for (k, item) in list {
                crate::types::check::text_at_most(item, 64).map_err(|e| e.under(k.as_str()).under("keybindings"))?;
            }
        }
        if let Some(v) = &self.app_font {
            crate::types::check::text_at_most(v, 128).map_err(|e| e.under("appFont"))?;
        }
        if let Some(v) = &self.code_font {
            crate::types::check::text_at_most(v, 128).map_err(|e| e.under("codeFont"))?;
        }
        if let Some(v) = self.text_size {
            crate::types::check::refine(crate::views::preferences::TEXT_SIZES.contains(&(v.get() as u64)), "one of 13, 14, 15, 16, 17")
                .map_err(|e| e.under("textSize"))?;
        }
        if let Some(v) = self.code_size {
            crate::types::check::refine(crate::views::preferences::CODE_SIZES.contains(&(v.get() as u64)), "one of 11, 12, 13, 14, 15")
                .map_err(|e| e.under("codeSize"))?;
        }
        self.agent_defaults.check().map_err(|e| e.under("agentDefaults"))?;
        self.project_defaults.check().map_err(|e| e.under("projectDefaults"))?;
        self.send_with.check().map_err(|e| e.under("sendWith"))?;
        self.mid_turn.check().map_err(|e| e.under("midTurn"))?;
        self.notify_needs.check().map_err(|e| e.under("notifyNeeds"))?;
        self.notify_done.check().map_err(|e| e.under("notifyDone"))?;
        self.settle_after.check().map_err(|e| e.under("settleAfter"))?;
        self.on_quit.check().map_err(|e| e.under("onQuit"))?;
        self.new_thread_in.check().map_err(|e| e.under("newThreadIn"))?;
        self.light_theme.check().map_err(|e| e.under("lightTheme"))?;
        self.dark_theme.check().map_err(|e| e.under("darkTheme"))?;
        self.project_icon.check().map_err(|e| e.under("projectIcon"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::opt_key::<ThemePreference>(obj, "theme", &mut out);
        crate::types::keys::opt_key::<ThemeId>(obj, "lightTheme", &mut out);
        crate::types::keys::opt_key::<ThemeId>(obj, "darkTheme", &mut out);
        crate::types::keys::opt_key::<SidebarMode>(obj, "sidebarMode", &mut out);
        crate::types::keys::opt_key_with::<JsInt>(obj, "sidebarWidth", &mut out, Self::rule_sidebar_width);
        crate::types::keys::opt_key::<TerminalSizeSource>(obj, "terminalSize", &mut out);
        crate::types::keys::opt_key::<IndexMap<String, JsInt>>(obj, "terminalZoom", &mut out);
        crate::types::keys::opt_key::<IndexMap<String, String>>(obj, "access", &mut out);
        crate::types::keys::opt_key::<PreferencesTarget>(obj, "target", &mut out);
        crate::types::keys::opt_key::<IndexMap<String, ProjectLook>>(obj, "projectLook", &mut out);
        crate::types::keys::opt_key::<IndexMap<String, ProjectIconHash>>(obj, "projectIcon", &mut out);
        crate::types::keys::opt_key::<IndexMap<String, ComputerLook>>(obj, "computerLook", &mut out);
        crate::types::keys::opt_key::<IndexMap<String, RecipeLook>>(obj, "recipeLook", &mut out);
        crate::types::keys::opt_key::<IndexMap<String, True>>(obj, "threadsShut", &mut out);
        crate::types::keys::opt_key::<bool>(obj, "serverIcons", &mut out);
        crate::types::keys::opt_key::<bool>(obj, "agentVersions", &mut out);
        crate::types::keys::opt_key::<bool>(obj, "usageLogs", &mut out);
        crate::types::keys::opt_key::<bool>(obj, "productUsage", &mut out);
        crate::types::keys::opt_key::<EditorId>(obj, "editor", &mut out);
        crate::types::keys::opt_key::<String>(obj, "updateDismissed", &mut out);
        crate::types::keys::opt_key::<bool>(obj, "cloudCardDismissed", &mut out);
        crate::types::keys::opt_key::<bool>(obj, "keepAwake", &mut out);
        crate::types::keys::opt_key::<bool>(obj, "transparency", &mut out);
        crate::types::keys::opt_key::<Vec<String>>(obj, "projectOrder", &mut out);
        crate::types::keys::opt_key_with::<IndexMap<String, String>>(obj, "keybindings", &mut out, Self::rule_keybindings);
        crate::types::keys::opt_key_with::<String>(obj, "appFont", &mut out, Self::rule_app_font);
        crate::types::keys::opt_key_with::<String>(obj, "codeFont", &mut out, Self::rule_code_font);
        crate::types::keys::opt_key_with::<JsInt>(obj, "textSize", &mut out, Self::rule_text_size);
        crate::types::keys::opt_key_with::<JsInt>(obj, "codeSize", &mut out, Self::rule_code_size);
        crate::types::keys::opt_key::<String>(obj, "defaultAgent", &mut out);
        crate::types::keys::opt_key::<IndexMap<String, AgentDefaults>>(obj, "agentDefaults", &mut out);
        crate::types::keys::opt_key::<IndexMap<String, ProjectOverrides>>(obj, "projectDefaults", &mut out);
        crate::types::keys::opt_key::<SendKey>(obj, "sendWith", &mut out);
        crate::types::keys::opt_key::<MidTurn>(obj, "midTurn", &mut out);
        crate::types::keys::opt_key::<NotifyChoice>(obj, "notifyNeeds", &mut out);
        crate::types::keys::opt_key::<NotifyChoice>(obj, "notifyDone", &mut out);
        crate::types::keys::opt_key::<bool>(obj, "planAlerts", &mut out);
        crate::types::keys::opt_key::<SettleAfter>(obj, "settleAfter", &mut out);
        crate::types::keys::opt_key::<bool>(obj, "askDelete", &mut out);
        crate::types::keys::opt_key::<OnQuit>(obj, "onQuit", &mut out);
        crate::types::keys::opt_key::<NewThreadIn>(obj, "newThreadIn", &mut out);
        crate::types::keys::opt_key::<bool>(obj, "labs", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "theme" => <Option<ThemePreference> as Check>::describe(rest),
                "lightTheme" => <Option<ThemeId> as Check>::describe(rest),
                "darkTheme" => <Option<ThemeId> as Check>::describe(rest),
                "sidebarMode" => <Option<SidebarMode> as Check>::describe(rest),
                "sidebarWidth" => <Option<i64> as Check>::describe(rest),
                "terminalSize" => <Option<TerminalSizeSource> as Check>::describe(rest),
                "terminalZoom" => <Option<IndexMap<String, i64>> as Check>::describe(rest),
                "access" => <Option<IndexMap<String, String>> as Check>::describe(rest),
                "target" => <Option<PreferencesTarget> as Check>::describe(rest),
                "projectLook" => <Option<IndexMap<String, ProjectLook>> as Check>::describe(rest),
                "projectIcon" => None,
                "computerLook" => <Option<IndexMap<String, ComputerLook>> as Check>::describe(rest),
                "recipeLook" => <Option<IndexMap<String, RecipeLook>> as Check>::describe(rest),
                "threadsShut" => None,
                "serverIcons" => <Option<bool> as Check>::describe(rest),
                "agentVersions" => <Option<bool> as Check>::describe(rest),
                "usageLogs" => <Option<bool> as Check>::describe(rest),
                "productUsage" => <Option<bool> as Check>::describe(rest),
                "editor" => <Option<EditorId> as Check>::describe(rest),
                "updateDismissed" => <Option<String> as Check>::describe(rest),
                "cloudCardDismissed" => <Option<bool> as Check>::describe(rest),
                "keepAwake" => <Option<bool> as Check>::describe(rest),
                "transparency" => <Option<bool> as Check>::describe(rest),
                "projectOrder" => <Option<Vec<String>> as Check>::describe(rest),
                "keybindings" => <Option<IndexMap<String, String>> as Check>::describe(rest),
                "appFont" => <Option<String> as Check>::describe(rest),
                "codeFont" => <Option<String> as Check>::describe(rest),
                "textSize" => <Option<i64> as Check>::describe(rest),
                "codeSize" => <Option<i64> as Check>::describe(rest),
                "defaultAgent" => <Option<String> as Check>::describe(rest),
                "agentDefaults" => <Option<IndexMap<String, AgentDefaults>> as Check>::describe(rest),
                "projectDefaults" => <Option<IndexMap<String, ProjectOverrides>> as Check>::describe(rest),
                "sendWith" => <Option<SendKey> as Check>::describe(rest),
                "midTurn" => <Option<MidTurn> as Check>::describe(rest),
                "notifyNeeds" => <Option<NotifyChoice> as Check>::describe(rest),
                "notifyDone" => <Option<NotifyChoice> as Check>::describe(rest),
                "planAlerts" => <Option<bool> as Check>::describe(rest),
                "settleAfter" => <Option<SettleAfter> as Check>::describe(rest),
                "askDelete" => <Option<bool> as Check>::describe(rest),
                "onQuit" => <Option<OnQuit> as Check>::describe(rest),
                "newThreadIn" => <Option<NewThreadIn> as Check>::describe(rest),
                "labs" => <Option<bool> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl PreferencesStored {
    fn rule_sidebar_width(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = *x {
            crate::types::check::num_above(v.0 .0, 0.0)?;
        }
        Ok(())
    }
    fn rule_keybindings(x: &Option<IndexMap<String, String>>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(list) = x {
            for (k, item) in list {
                crate::types::check::text_at_least(item, 1).map_err(|e| e.under(k.as_str()))?;
            }
        }
        if let Some(list) = x {
            for (k, item) in list {
                crate::types::check::text_at_most(item, 64).map_err(|e| e.under(k.as_str()))?;
            }
        }
        Ok(())
    }
    fn rule_app_font(x: &Option<String>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = x {
            crate::types::check::text_at_most(v, 128)?;
        }
        Ok(())
    }
    fn rule_code_font(x: &Option<String>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = x {
            crate::types::check::text_at_most(v, 128)?;
        }
        Ok(())
    }
    fn rule_text_size(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = *x {
            crate::types::check::refine(crate::views::preferences::TEXT_SIZES.contains(&(v.get() as u64)), "one of 13, 14, 15, 16, 17")?;
        }
        Ok(())
    }
    fn rule_code_size(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = *x {
            crate::types::check::refine(crate::views::preferences::CODE_SIZES.contains(&(v.get() as u64)), "one of 11, 12, 13, 14, 15")?;
        }
        Ok(())
    }
}

/// TS: packages/protocol/src/views/preferences.ts:14 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ThemePreference {
    #[serde(rename = "system")]
    System,
    #[serde(rename = "light")]
    Light,
    #[serde(rename = "dark")]
    Dark,
}

impl Check for ThemePreference {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["system", "light", "dark"]))
    }
}

/// TS: packages/protocol/src/views/preferences.ts:18 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SidebarMode {
    #[serde(rename = "list")]
    List,
    #[serde(rename = "spaces")]
    Spaces,
}

impl Check for SidebarMode {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["list", "spaces"]))
    }
}

/// TS: packages/protocol/src/views/preferences.ts:22 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TerminalSizeSource {
    #[serde(rename = "app")]
    App,
    #[serde(rename = "file")]
    File,
}

impl Check for TerminalSizeSource {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["app", "file"]))
    }
}

/// TS: packages/protocol/src/views/preferences.ts:27 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PreferencesTarget {
    pub workspace: String,
}

impl Check for PreferencesTarget {
    /// Strict: an unknown key is refused (zod .strict()).
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "workspace", &mut out);
        let known: &[&str] = &["workspace"];
        let extra: Vec<String> = obj.keys().filter(|k| !known.contains(&k.as_str())).cloned().collect();
        if !extra.is_empty() {
            out.push(crate::types::check::unrecognized(Vec::new(), extra));
        }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "workspace" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/preferences.ts:33 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectLook {
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub icon: Option<ProjectIcon>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub hue: Option<ProjectHue>,
}

impl Check for ProjectLook {
    /// Nested checks: icon, hue.
    /// Strict: an unknown key is refused (zod .strict()).
    fn check(&self) -> Result<(), SchemaError> {
        self.icon.check().map_err(|e| e.under("icon"))?;
        self.hue.check().map_err(|e| e.under("hue"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::opt_key::<ProjectIcon>(obj, "icon", &mut out);
        crate::types::keys::opt_key::<ProjectHue>(obj, "hue", &mut out);
        let known: &[&str] = &["icon", "hue"];
        let extra: Vec<String> = obj.keys().filter(|k| !known.contains(&k.as_str())).cloned().collect();
        if !extra.is_empty() {
            out.push(crate::types::check::unrecognized(Vec::new(), extra));
        }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "icon" => <Option<ProjectIcon> as Check>::describe(rest),
                "hue" => <Option<ProjectHue> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/preferences.ts:36 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ComputerIcon {
    #[serde(rename = "laptop")]
    Laptop,
    #[serde(rename = "desktop")]
    Desktop,
    #[serde(rename = "mac-mini")]
    MacMini,
    #[serde(rename = "mac-studio")]
    MacStudio,
    #[serde(rename = "tower")]
    Tower,
    #[serde(rename = "server")]
    Server,
    #[serde(rename = "cloud")]
    Cloud,
    #[serde(rename = "cpu")]
    Cpu,
    #[serde(rename = "drive")]
    Drive,
    #[serde(rename = "container")]
    Container,
    #[serde(rename = "home")]
    Home,
}

impl Check for ComputerIcon {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(
            path,
            crate::types::check::Zod::Options(&[
                "laptop",
                "desktop",
                "mac-mini",
                "mac-studio",
                "tower",
                "server",
                "cloud",
                "cpu",
                "drive",
                "container",
                "home",
            ]),
        )
    }
}

/// TS: packages/protocol/src/views/preferences.ts:38 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ComputerLook {
    pub icon: ComputerIcon,
}

impl Check for ComputerLook {
    /// Nested checks: icon.
    /// Strict: an unknown key is refused (zod .strict()).
    fn check(&self) -> Result<(), SchemaError> {
        self.icon.check().map_err(|e| e.under("icon"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<ComputerIcon>(obj, "icon", &mut out);
        let known: &[&str] = &["icon"];
        let extra: Vec<String> = obj.keys().filter(|k| !known.contains(&k.as_str())).cloned().collect();
        if !extra.is_empty() {
            out.push(crate::types::check::unrecognized(Vec::new(), extra));
        }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "icon" => <ComputerIcon as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/preferences.ts:41 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RecipeLook {
    pub icon: ProjectIcon,
}

impl Check for RecipeLook {
    /// Nested checks: icon.
    /// Strict: an unknown key is refused (zod .strict()).
    fn check(&self) -> Result<(), SchemaError> {
        self.icon.check().map_err(|e| e.under("icon"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<ProjectIcon>(obj, "icon", &mut out);
        let known: &[&str] = &["icon"];
        let extra: Vec<String> = obj.keys().filter(|k| !known.contains(&k.as_str())).cloned().collect();
        if !extra.is_empty() {
            out.push(crate::types::check::unrecognized(Vec::new(), extra));
        }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "icon" => <ProjectIcon as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/preferences.ts:46 at a3b870a6e.
/// TS: views/preferences.ts:46 at a3b870a6e..
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ThemeId(pub String);

impl Check for ThemeId {
    fn check(&self) -> Result<(), SchemaError> {
        crate::types::check::text_at_least(&self.0, 1)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Word("string"))
    }
}

/// TS: packages/protocol/src/views/preferences.ts:57 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EditorId {
    #[serde(rename = "vscode")]
    Vscode,
    #[serde(rename = "cursor")]
    Cursor,
    #[serde(rename = "vscode-insiders")]
    VscodeInsiders,
    #[serde(rename = "zed")]
    Zed,
    #[serde(rename = "idea")]
    Idea,
    #[serde(rename = "webstorm")]
    Webstorm,
    #[serde(rename = "pycharm")]
    Pycharm,
    #[serde(rename = "goland")]
    Goland,
    #[serde(rename = "rustrover")]
    Rustrover,
    #[serde(rename = "clion")]
    Clion,
    #[serde(rename = "phpstorm")]
    Phpstorm,
    #[serde(rename = "rubymine")]
    Rubymine,
    #[serde(rename = "rider")]
    Rider,
    #[serde(rename = "finder")]
    Finder,
}

impl Check for EditorId {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(
            path,
            crate::types::check::Zod::Options(&[
                "vscode",
                "cursor",
                "vscode-insiders",
                "zed",
                "idea",
                "webstorm",
                "pycharm",
                "goland",
                "rustrover",
                "clion",
                "phpstorm",
                "rubymine",
                "rider",
                "finder",
            ]),
        )
    }
}

/// TS: packages/protocol/src/views/preferences.ts:61 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorChoice {
    pub id: EditorId,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub remote: Option<True>,
}

impl Check for EditorChoice {
    /// Nested checks: id.
    fn check(&self) -> Result<(), SchemaError> {
        self.id.check().map_err(|e| e.under("id"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<EditorId>(obj, "id", &mut out);
        crate::types::keys::req_key::<String>(obj, "name", &mut out);
        crate::types::keys::opt_key::<True>(obj, "remote", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "id" => <EditorId as Check>::describe(rest),
                "name" => <String as Check>::describe(rest),
                "remote" => None,
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/preferences.ts:88 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Preferences {
    pub theme: ThemePreference,
    #[serde(default = "default_preferences_light_theme")]
    pub light_theme: ThemeId,
    #[serde(default = "default_preferences_dark_theme")]
    pub dark_theme: ThemeId,
    pub sidebar_mode: SidebarMode,
    pub terminal_size: TerminalSizeSource,
    #[serde(serialize_with = "crate::types::check::js_record", deserialize_with = "crate::types::kept::record")]
    pub terminal_zoom: IndexMap<String, JsInt>,
    #[serde(serialize_with = "crate::types::check::js_record", deserialize_with = "crate::types::kept::record")]
    pub access: IndexMap<String, String>,
    #[serde(serialize_with = "crate::types::check::js_record", deserialize_with = "crate::types::kept::record")]
    pub project_look: IndexMap<String, ProjectLook>,
    #[serde(
        default = "default_preferences_project_icon",
        serialize_with = "crate::types::check::js_record",
        deserialize_with = "crate::types::kept::record"
    )]
    pub project_icon: IndexMap<String, ProjectIconHash>,
    #[serde(
        default = "default_preferences_computer_look",
        serialize_with = "crate::types::check::js_record",
        deserialize_with = "crate::types::kept::record"
    )]
    pub computer_look: IndexMap<String, ComputerLook>,
    #[serde(default = "default_preferences_server_icons")]
    pub server_icons: bool,
    #[serde(default = "default_preferences_agent_versions")]
    pub agent_versions: bool,
    #[serde(default = "default_preferences_usage_logs")]
    pub usage_logs: bool,
    #[serde(default = "default_preferences_product_usage")]
    pub product_usage: bool,
    #[serde(default = "default_preferences_keep_awake")]
    pub keep_awake: bool,
    #[serde(default = "default_preferences_transparency")]
    pub transparency: bool,
    pub project_order: Vec<String>,
    #[serde(
        default = "default_preferences_keybindings",
        serialize_with = "crate::types::check::js_record",
        deserialize_with = "crate::types::kept::record"
    )]
    pub keybindings: IndexMap<String, String>,
    #[serde(default = "default_preferences_app_font")]
    pub app_font: String,
    #[serde(default = "default_preferences_code_font")]
    pub code_font: String,
    #[serde(
        default = "default_preferences_agent_defaults",
        serialize_with = "crate::types::check::js_record",
        deserialize_with = "crate::types::kept::record"
    )]
    pub agent_defaults: IndexMap<String, Fields<AgentDefaults>>,
    #[serde(
        default = "default_preferences_project_defaults",
        serialize_with = "crate::types::check::js_record",
        deserialize_with = "crate::types::kept::record"
    )]
    pub project_defaults: IndexMap<String, Fields<ProjectOverrides>>,
    #[serde(default = "default_preferences_send_with")]
    pub send_with: SendKey,
    #[serde(default = "default_preferences_mid_turn")]
    pub mid_turn: MidTurn,
    #[serde(default = "default_preferences_notify_needs")]
    pub notify_needs: NotifyChoice,
    #[serde(default = "default_preferences_notify_done")]
    pub notify_done: NotifyChoice,
    #[serde(default = "default_preferences_plan_alerts")]
    pub plan_alerts: bool,
    #[serde(default = "default_preferences_settle_after")]
    pub settle_after: SettleAfter,
    #[serde(default = "default_preferences_ask_delete")]
    pub ask_delete: bool,
    #[serde(default = "default_preferences_on_quit")]
    pub on_quit: OnQuit,
    #[serde(default = "default_preferences_new_thread_in")]
    pub new_thread_in: NewThreadIn,
    pub labs: bool,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub sidebar_width: Option<JsInt>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub target: Option<PreferencesTarget>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub editor: Option<EditorId>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub update_dismissed: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub cloud_card_dismissed: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub text_size: Option<JsInt>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub code_size: Option<JsInt>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub default_agent: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::types::kept::record_opt",
        serialize_with = "crate::types::check::js_record_opt"
    )]
    pub recipe_look: Option<IndexMap<String, RecipeLook>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::types::kept::record_opt",
        serialize_with = "crate::types::check::js_record_opt"
    )]
    pub threads_shut: Option<IndexMap<String, True>>,
}

impl Check for Preferences {
    /// Rules: keybindings{}: at least 1 UTF-16 units; keybindings{}: at most 64 UTF-16 units; appFont: at most 128 UTF-16 units; codeFont: at most 128 UTF-16 units; sidebarWidth: > 0; textSize: a refine (spec/shapes.md rule 7); codeSize: a refine (spec/shapes.md rule 7).
    /// Nested checks: theme, lightTheme, darkTheme, sidebarMode, terminalSize, projectLook, projectIcon, computerLook, agentDefaults, projectDefaults, sendWith, midTurn, notifyNeeds, notifyDone, settleAfter, onQuit, newThreadIn, target, editor, recipeLook.
    fn check(&self) -> Result<(), SchemaError> {
        self.theme.check().map_err(|e| e.under("theme"))?;
        self.sidebar_mode.check().map_err(|e| e.under("sidebarMode"))?;
        self.terminal_size.check().map_err(|e| e.under("terminalSize"))?;
        self.project_look.check().map_err(|e| e.under("projectLook"))?;
        for (k, hash) in &self.project_icon {
            crate::types::check::regex_check(crate::types::check::hex_64(&hash.0)).map_err(|e| e.under(k.as_str()).under("projectIcon"))?;
        }
        self.computer_look.check().map_err(|e| e.under("computerLook"))?;
        for (k, item) in &self.keybindings {
            crate::types::check::text_at_least(item, 1).map_err(|e| e.under(k.as_str()).under("keybindings"))?;
        }
        for (k, item) in &self.keybindings {
            crate::types::check::text_at_most(item, 64).map_err(|e| e.under(k.as_str()).under("keybindings"))?;
        }
        crate::types::check::text_at_most(&self.app_font, 128).map_err(|e| e.under("appFont"))?;
        crate::types::check::text_at_most(&self.code_font, 128).map_err(|e| e.under("codeFont"))?;
        self.agent_defaults.check().map_err(|e| e.under("agentDefaults"))?;
        self.project_defaults.check().map_err(|e| e.under("projectDefaults"))?;
        self.send_with.check().map_err(|e| e.under("sendWith"))?;
        self.mid_turn.check().map_err(|e| e.under("midTurn"))?;
        self.notify_needs.check().map_err(|e| e.under("notifyNeeds"))?;
        self.notify_done.check().map_err(|e| e.under("notifyDone"))?;
        self.settle_after.check().map_err(|e| e.under("settleAfter"))?;
        self.on_quit.check().map_err(|e| e.under("onQuit"))?;
        self.new_thread_in.check().map_err(|e| e.under("newThreadIn"))?;
        if let Some(v) = self.sidebar_width {
            crate::types::check::num_above(v.0 .0, 0.0).map_err(|e| e.under("sidebarWidth"))?;
        }
        self.target.check().map_err(|e| e.under("target"))?;
        self.editor.check().map_err(|e| e.under("editor"))?;
        if let Some(v) = self.text_size {
            crate::types::check::refine(crate::views::preferences::TEXT_SIZES.contains(&(v.get() as u64)), "one of 13, 14, 15, 16, 17")
                .map_err(|e| e.under("textSize"))?;
        }
        if let Some(v) = self.code_size {
            crate::types::check::refine(crate::views::preferences::CODE_SIZES.contains(&(v.get() as u64)), "one of 11, 12, 13, 14, 15")
                .map_err(|e| e.under("codeSize"))?;
        }
        self.recipe_look.check().map_err(|e| e.under("recipeLook"))?;
        self.light_theme.check().map_err(|e| e.under("lightTheme"))?;
        self.dark_theme.check().map_err(|e| e.under("darkTheme"))?;
        self.project_icon.check().map_err(|e| e.under("projectIcon"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<ThemePreference>(obj, "theme", &mut out);
        crate::types::keys::any_key::<ThemeId>(obj, "lightTheme", &mut out);
        crate::types::keys::any_key::<ThemeId>(obj, "darkTheme", &mut out);
        crate::types::keys::req_key::<SidebarMode>(obj, "sidebarMode", &mut out);
        crate::types::keys::opt_key_with::<JsInt>(obj, "sidebarWidth", &mut out, Self::rule_sidebar_width);
        crate::types::keys::req_key::<TerminalSizeSource>(obj, "terminalSize", &mut out);
        crate::types::keys::req_key::<IndexMap<String, JsInt>>(obj, "terminalZoom", &mut out);
        crate::types::keys::req_key::<IndexMap<String, String>>(obj, "access", &mut out);
        crate::types::keys::opt_key::<PreferencesTarget>(obj, "target", &mut out);
        crate::types::keys::req_key::<IndexMap<String, ProjectLook>>(obj, "projectLook", &mut out);
        crate::types::keys::any_key::<IndexMap<String, ProjectIconHash>>(obj, "projectIcon", &mut out);
        crate::types::keys::any_key::<IndexMap<String, ComputerLook>>(obj, "computerLook", &mut out);
        crate::types::keys::opt_key::<IndexMap<String, RecipeLook>>(obj, "recipeLook", &mut out);
        crate::types::keys::opt_key::<IndexMap<String, True>>(obj, "threadsShut", &mut out);
        crate::types::keys::any_key::<bool>(obj, "serverIcons", &mut out);
        crate::types::keys::any_key::<bool>(obj, "agentVersions", &mut out);
        crate::types::keys::any_key::<bool>(obj, "usageLogs", &mut out);
        crate::types::keys::any_key::<bool>(obj, "productUsage", &mut out);
        crate::types::keys::opt_key::<EditorId>(obj, "editor", &mut out);
        crate::types::keys::opt_key::<String>(obj, "updateDismissed", &mut out);
        crate::types::keys::opt_key::<bool>(obj, "cloudCardDismissed", &mut out);
        crate::types::keys::any_key::<bool>(obj, "keepAwake", &mut out);
        crate::types::keys::any_key::<bool>(obj, "transparency", &mut out);
        crate::types::keys::req_key::<Vec<String>>(obj, "projectOrder", &mut out);
        crate::types::keys::any_key_with::<IndexMap<String, String>>(obj, "keybindings", &mut out, Self::rule_keybindings);
        crate::types::keys::any_key_with::<String>(obj, "appFont", &mut out, Self::rule_app_font);
        crate::types::keys::any_key_with::<String>(obj, "codeFont", &mut out, Self::rule_code_font);
        crate::types::keys::opt_key_with::<JsInt>(obj, "textSize", &mut out, Self::rule_text_size);
        crate::types::keys::opt_key_with::<JsInt>(obj, "codeSize", &mut out, Self::rule_code_size);
        crate::types::keys::opt_key::<String>(obj, "defaultAgent", &mut out);
        crate::types::keys::any_key::<IndexMap<String, AgentDefaults>>(obj, "agentDefaults", &mut out);
        crate::types::keys::any_key::<IndexMap<String, ProjectOverrides>>(obj, "projectDefaults", &mut out);
        crate::types::keys::any_key::<SendKey>(obj, "sendWith", &mut out);
        crate::types::keys::any_key::<MidTurn>(obj, "midTurn", &mut out);
        crate::types::keys::any_key::<NotifyChoice>(obj, "notifyNeeds", &mut out);
        crate::types::keys::any_key::<NotifyChoice>(obj, "notifyDone", &mut out);
        crate::types::keys::any_key::<bool>(obj, "planAlerts", &mut out);
        crate::types::keys::any_key::<SettleAfter>(obj, "settleAfter", &mut out);
        crate::types::keys::any_key::<bool>(obj, "askDelete", &mut out);
        crate::types::keys::any_key::<OnQuit>(obj, "onQuit", &mut out);
        crate::types::keys::any_key::<NewThreadIn>(obj, "newThreadIn", &mut out);
        crate::types::keys::req_key::<bool>(obj, "labs", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "theme" => <ThemePreference as Check>::describe(rest),
                "lightTheme" => <ThemeId as Check>::describe(rest),
                "darkTheme" => <ThemeId as Check>::describe(rest),
                "sidebarMode" => <SidebarMode as Check>::describe(rest),
                "terminalSize" => <TerminalSizeSource as Check>::describe(rest),
                "terminalZoom" => <IndexMap<String, i64> as Check>::describe(rest),
                "access" => <IndexMap<String, String> as Check>::describe(rest),
                "projectLook" => <IndexMap<String, ProjectLook> as Check>::describe(rest),
                "projectIcon" => None,
                "computerLook" => <IndexMap<String, ComputerLook> as Check>::describe(rest),
                "serverIcons" => <bool as Check>::describe(rest),
                "agentVersions" => <bool as Check>::describe(rest),
                "usageLogs" => <bool as Check>::describe(rest),
                "productUsage" => <bool as Check>::describe(rest),
                "keepAwake" => <bool as Check>::describe(rest),
                "transparency" => <bool as Check>::describe(rest),
                "projectOrder" => <Vec<String> as Check>::describe(rest),
                "keybindings" => <IndexMap<String, String> as Check>::describe(rest),
                "appFont" => <String as Check>::describe(rest),
                "codeFont" => <String as Check>::describe(rest),
                "agentDefaults" => <IndexMap<String, AgentDefaults> as Check>::describe(rest),
                "projectDefaults" => <IndexMap<String, ProjectOverrides> as Check>::describe(rest),
                "sendWith" => <SendKey as Check>::describe(rest),
                "midTurn" => <MidTurn as Check>::describe(rest),
                "notifyNeeds" => <NotifyChoice as Check>::describe(rest),
                "notifyDone" => <NotifyChoice as Check>::describe(rest),
                "planAlerts" => <bool as Check>::describe(rest),
                "settleAfter" => <SettleAfter as Check>::describe(rest),
                "askDelete" => <bool as Check>::describe(rest),
                "onQuit" => <OnQuit as Check>::describe(rest),
                "newThreadIn" => <NewThreadIn as Check>::describe(rest),
                "labs" => <bool as Check>::describe(rest),
                "sidebarWidth" => <Option<i64> as Check>::describe(rest),
                "target" => <Option<PreferencesTarget> as Check>::describe(rest),
                "editor" => <Option<EditorId> as Check>::describe(rest),
                "updateDismissed" => <Option<String> as Check>::describe(rest),
                "cloudCardDismissed" => <Option<bool> as Check>::describe(rest),
                "textSize" => <Option<i64> as Check>::describe(rest),
                "codeSize" => <Option<i64> as Check>::describe(rest),
                "defaultAgent" => <Option<String> as Check>::describe(rest),
                "recipeLook" => <Option<IndexMap<String, RecipeLook>> as Check>::describe(rest),
                "threadsShut" => None,
                _ => None,
            },
        }
    }
}
impl Preferences {
    fn rule_keybindings(x: &IndexMap<String, String>) -> Result<(), crate::types::check::SchemaError> {
        for (k, item) in x {
            crate::types::check::text_at_least(item, 1).map_err(|e| e.under(k.as_str()))?;
        }
        for (k, item) in x {
            crate::types::check::text_at_most(item, 64).map_err(|e| e.under(k.as_str()))?;
        }
        Ok(())
    }
    fn rule_app_font(x: &String) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::text_at_most(x, 128)?;
        Ok(())
    }
    fn rule_code_font(x: &String) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::text_at_most(x, 128)?;
        Ok(())
    }
    fn rule_sidebar_width(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = *x {
            crate::types::check::num_above(v.0 .0, 0.0)?;
        }
        Ok(())
    }
    fn rule_text_size(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = *x {
            crate::types::check::refine(crate::views::preferences::TEXT_SIZES.contains(&(v.get() as u64)), "one of 13, 14, 15, 16, 17")?;
        }
        Ok(())
    }
    fn rule_code_size(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = *x {
            crate::types::check::refine(crate::views::preferences::CODE_SIZES.contains(&(v.get() as u64)), "one of 11, 12, 13, 14, 15")?;
        }
        Ok(())
    }
}

fn default_preferences_light_theme() -> ThemeId {
    from_json("\"paper\"")
}

fn default_preferences_dark_theme() -> ThemeId {
    from_json("\"graphite\"")
}

fn default_preferences_project_icon() -> IndexMap<String, ProjectIconHash> {
    from_json("{}")
}

fn default_preferences_computer_look() -> IndexMap<String, ComputerLook> {
    from_json("{}")
}

fn default_preferences_server_icons() -> bool {
    from_json("true")
}

fn default_preferences_agent_versions() -> bool {
    from_json("true")
}

fn default_preferences_usage_logs() -> bool {
    from_json("true")
}

fn default_preferences_product_usage() -> bool {
    from_json("true")
}

fn default_preferences_keep_awake() -> bool {
    from_json("true")
}

fn default_preferences_transparency() -> bool {
    from_json("true")
}

fn default_preferences_keybindings() -> IndexMap<String, String> {
    from_json("{}")
}

fn default_preferences_app_font() -> String {
    from_json("\"\"")
}

fn default_preferences_code_font() -> String {
    from_json("\"\"")
}

fn default_preferences_agent_defaults() -> IndexMap<String, Fields<AgentDefaults>> {
    from_json("{}")
}

fn default_preferences_project_defaults() -> IndexMap<String, Fields<ProjectOverrides>> {
    from_json("{}")
}

fn default_preferences_send_with() -> SendKey {
    from_json("\"enter\"")
}

fn default_preferences_mid_turn() -> MidTurn {
    from_json("\"queue\"")
}

fn default_preferences_notify_needs() -> NotifyChoice {
    from_json("\"notify-sound\"")
}

fn default_preferences_notify_done() -> NotifyChoice {
    from_json("\"notify\"")
}

fn default_preferences_plan_alerts() -> bool {
    from_json("true")
}

fn default_preferences_settle_after() -> SettleAfter {
    from_json("\"2h\"")
}

fn default_preferences_ask_delete() -> bool {
    from_json("true")
}

fn default_preferences_on_quit() -> OnQuit {
    from_json("\"ask\"")
}

fn default_preferences_new_thread_in() -> NewThreadIn {
    from_json("\"current\"")
}

/// TS: packages/protocol/src/views/preferences.ts:185 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PreferencesPatch {
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub theme: Option<ThemePreference>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub light_theme: Option<ThemeId>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub dark_theme: Option<ThemeId>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub sidebar_mode: Option<SidebarMode>,
    #[serde(default, skip_serializing_if = "Option::is_none", with = "crate::types::check::double_option")]
    pub sidebar_width: Option<Option<JsInt>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub terminal_size: Option<TerminalSizeSource>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::types::kept::record_opt",
        serialize_with = "crate::types::check::js_record_opt"
    )]
    pub terminal_zoom: Option<IndexMap<String, Option<JsInt>>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::types::kept::record_opt",
        serialize_with = "crate::types::check::js_record_opt"
    )]
    pub access: Option<IndexMap<String, Option<String>>>,
    #[serde(default, skip_serializing_if = "Option::is_none", with = "crate::types::check::double_option")]
    pub target: Option<Option<PreferencesTarget>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::types::kept::record_opt",
        serialize_with = "crate::types::check::js_record_opt"
    )]
    pub project_look: Option<IndexMap<String, Option<ProjectLook>>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::types::kept::record_opt",
        serialize_with = "crate::types::check::js_record_opt"
    )]
    pub computer_look: Option<IndexMap<String, Option<ComputerLook>>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::types::kept::record_opt",
        serialize_with = "crate::types::check::js_record_opt"
    )]
    pub recipe_look: Option<IndexMap<String, Option<RecipeLook>>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::types::kept::record_opt",
        serialize_with = "crate::types::check::js_record_opt"
    )]
    pub threads_shut: Option<IndexMap<String, Option<True>>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub server_icons: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub agent_versions: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub usage_logs: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub product_usage: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub editor: Option<EditorId>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub update_dismissed: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub cloud_card_dismissed: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub keep_awake: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub transparency: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub project_order: Option<Vec<String>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::types::kept::record_opt",
        serialize_with = "crate::types::check::js_record_opt"
    )]
    pub keybindings: Option<IndexMap<String, Option<String>>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub app_font: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub code_font: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", with = "crate::types::check::double_option")]
    pub text_size: Option<Option<JsInt>>,
    #[serde(default, skip_serializing_if = "Option::is_none", with = "crate::types::check::double_option")]
    pub code_size: Option<Option<JsInt>>,
    #[serde(default, skip_serializing_if = "Option::is_none", with = "crate::types::check::double_option")]
    pub default_agent: Option<Option<String>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::types::kept::record_opt",
        serialize_with = "crate::types::check::js_record_opt"
    )]
    pub agent_defaults: Option<IndexMap<String, Option<AgentDefaultsPatch>>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::types::kept::record_opt",
        serialize_with = "crate::types::check::js_record_opt"
    )]
    pub project_defaults: Option<IndexMap<String, Option<ProjectOverridesPatch>>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub send_with: Option<SendKey>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub mid_turn: Option<MidTurn>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub notify_needs: Option<NotifyChoice>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub notify_done: Option<NotifyChoice>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub plan_alerts: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub settle_after: Option<SettleAfter>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub ask_delete: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub on_quit: Option<OnQuit>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub new_thread_in: Option<NewThreadIn>,
}

impl Check for PreferencesPatch {
    /// Rules: sidebarWidth: > 0; keybindings{}: at least 1 UTF-16 units; keybindings{}: at most 64 UTF-16 units; appFont: at most 128 UTF-16 units; codeFont: at most 128 UTF-16 units; textSize: a refine (spec/shapes.md rule 7); codeSize: a refine (spec/shapes.md rule 7).
    /// Nested checks: theme, lightTheme, darkTheme, sidebarMode, terminalSize, target, projectLook, computerLook, recipeLook, editor, agentDefaults, projectDefaults, sendWith, midTurn, notifyNeeds, notifyDone, settleAfter, onQuit, newThreadIn.
    /// Strict: an unknown key is refused (zod .strict()).
    fn check(&self) -> Result<(), SchemaError> {
        self.theme.check().map_err(|e| e.under("theme"))?;
        self.sidebar_mode.check().map_err(|e| e.under("sidebarMode"))?;
        if let Some(Some(v)) = self.sidebar_width {
            crate::types::check::num_above(v.as_f64(), 0.0).map_err(|e| e.under("sidebarWidth"))?;
        }
        self.terminal_size.check().map_err(|e| e.under("terminalSize"))?;
        self.target.check().map_err(|e| e.under("target"))?;
        self.project_look.check().map_err(|e| e.under("projectLook"))?;
        self.computer_look.check().map_err(|e| e.under("computerLook"))?;
        self.recipe_look.check().map_err(|e| e.under("recipeLook"))?;
        self.editor.check().map_err(|e| e.under("editor"))?;
        if let Some(list) = &self.keybindings {
            for (k, item) in list {
                if let Some(v) = item {
                    crate::types::check::text_at_least(v, 1).map_err(|e| e.under(k.as_str()).under("keybindings"))?;
                }
            }
        }
        if let Some(list) = &self.keybindings {
            for (k, item) in list {
                if let Some(v) = item {
                    crate::types::check::text_at_most(v, 64).map_err(|e| e.under(k.as_str()).under("keybindings"))?;
                }
            }
        }
        if let Some(v) = &self.app_font {
            crate::types::check::text_at_most(v, 128).map_err(|e| e.under("appFont"))?;
        }
        if let Some(v) = &self.code_font {
            crate::types::check::text_at_most(v, 128).map_err(|e| e.under("codeFont"))?;
        }
        if let Some(Some(v)) = self.text_size {
            crate::types::check::refine(crate::views::preferences::TEXT_SIZES.contains(&(v.get() as u64)), "one of 13, 14, 15, 16, 17")
                .map_err(|e| e.under("textSize"))?;
        }
        if let Some(Some(v)) = self.code_size {
            crate::types::check::refine(crate::views::preferences::CODE_SIZES.contains(&(v.get() as u64)), "one of 11, 12, 13, 14, 15")
                .map_err(|e| e.under("codeSize"))?;
        }
        self.agent_defaults.check().map_err(|e| e.under("agentDefaults"))?;
        self.project_defaults.check().map_err(|e| e.under("projectDefaults"))?;
        self.send_with.check().map_err(|e| e.under("sendWith"))?;
        self.mid_turn.check().map_err(|e| e.under("midTurn"))?;
        self.notify_needs.check().map_err(|e| e.under("notifyNeeds"))?;
        self.notify_done.check().map_err(|e| e.under("notifyDone"))?;
        self.settle_after.check().map_err(|e| e.under("settleAfter"))?;
        self.on_quit.check().map_err(|e| e.under("onQuit"))?;
        self.new_thread_in.check().map_err(|e| e.under("newThreadIn"))?;
        self.light_theme.check().map_err(|e| e.under("lightTheme"))?;
        self.dark_theme.check().map_err(|e| e.under("darkTheme"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::opt_key::<ThemePreference>(obj, "theme", &mut out);
        crate::types::keys::opt_key::<ThemeId>(obj, "lightTheme", &mut out);
        crate::types::keys::opt_key::<ThemeId>(obj, "darkTheme", &mut out);
        crate::types::keys::opt_key::<SidebarMode>(obj, "sidebarMode", &mut out);
        crate::types::keys::dbl_key_with::<JsInt>(obj, "sidebarWidth", &mut out, Self::rule_sidebar_width);
        crate::types::keys::opt_key::<TerminalSizeSource>(obj, "terminalSize", &mut out);
        crate::types::keys::opt_key::<IndexMap<String, Option<JsInt>>>(obj, "terminalZoom", &mut out);
        crate::types::keys::opt_key::<IndexMap<String, Option<String>>>(obj, "access", &mut out);
        crate::types::keys::dbl_key::<PreferencesTarget>(obj, "target", &mut out);
        crate::types::keys::opt_key::<IndexMap<String, Option<ProjectLook>>>(obj, "projectLook", &mut out);
        crate::types::keys::opt_key::<IndexMap<String, Option<ComputerLook>>>(obj, "computerLook", &mut out);
        crate::types::keys::opt_key::<IndexMap<String, Option<RecipeLook>>>(obj, "recipeLook", &mut out);
        crate::types::keys::opt_key::<IndexMap<String, Option<True>>>(obj, "threadsShut", &mut out);
        crate::types::keys::opt_key::<bool>(obj, "serverIcons", &mut out);
        crate::types::keys::opt_key::<bool>(obj, "agentVersions", &mut out);
        crate::types::keys::opt_key::<bool>(obj, "usageLogs", &mut out);
        crate::types::keys::opt_key::<bool>(obj, "productUsage", &mut out);
        crate::types::keys::opt_key::<EditorId>(obj, "editor", &mut out);
        crate::types::keys::opt_key::<String>(obj, "updateDismissed", &mut out);
        crate::types::keys::opt_key::<bool>(obj, "cloudCardDismissed", &mut out);
        crate::types::keys::opt_key::<bool>(obj, "keepAwake", &mut out);
        crate::types::keys::opt_key::<bool>(obj, "transparency", &mut out);
        crate::types::keys::opt_key::<Vec<String>>(obj, "projectOrder", &mut out);
        crate::types::keys::opt_key_with::<IndexMap<String, Option<String>>>(obj, "keybindings", &mut out, Self::rule_keybindings);
        crate::types::keys::opt_key_with::<String>(obj, "appFont", &mut out, Self::rule_app_font);
        crate::types::keys::opt_key_with::<String>(obj, "codeFont", &mut out, Self::rule_code_font);
        crate::types::keys::dbl_key_with::<JsInt>(obj, "textSize", &mut out, Self::rule_text_size);
        crate::types::keys::dbl_key_with::<JsInt>(obj, "codeSize", &mut out, Self::rule_code_size);
        crate::types::keys::dbl_key::<String>(obj, "defaultAgent", &mut out);
        crate::types::keys::opt_key::<IndexMap<String, Option<AgentDefaultsPatch>>>(obj, "agentDefaults", &mut out);
        crate::types::keys::opt_key::<IndexMap<String, Option<ProjectOverridesPatch>>>(obj, "projectDefaults", &mut out);
        crate::types::keys::opt_key::<SendKey>(obj, "sendWith", &mut out);
        crate::types::keys::opt_key::<MidTurn>(obj, "midTurn", &mut out);
        crate::types::keys::opt_key::<NotifyChoice>(obj, "notifyNeeds", &mut out);
        crate::types::keys::opt_key::<NotifyChoice>(obj, "notifyDone", &mut out);
        crate::types::keys::opt_key::<bool>(obj, "planAlerts", &mut out);
        crate::types::keys::opt_key::<SettleAfter>(obj, "settleAfter", &mut out);
        crate::types::keys::opt_key::<bool>(obj, "askDelete", &mut out);
        crate::types::keys::opt_key::<OnQuit>(obj, "onQuit", &mut out);
        crate::types::keys::opt_key::<NewThreadIn>(obj, "newThreadIn", &mut out);
        let known: &[&str] = &[
            "theme",
            "lightTheme",
            "darkTheme",
            "sidebarMode",
            "sidebarWidth",
            "terminalSize",
            "terminalZoom",
            "access",
            "target",
            "projectLook",
            "computerLook",
            "recipeLook",
            "threadsShut",
            "serverIcons",
            "agentVersions",
            "usageLogs",
            "productUsage",
            "editor",
            "updateDismissed",
            "cloudCardDismissed",
            "keepAwake",
            "transparency",
            "projectOrder",
            "keybindings",
            "appFont",
            "codeFont",
            "textSize",
            "codeSize",
            "defaultAgent",
            "agentDefaults",
            "projectDefaults",
            "sendWith",
            "midTurn",
            "notifyNeeds",
            "notifyDone",
            "planAlerts",
            "settleAfter",
            "askDelete",
            "onQuit",
            "newThreadIn",
        ];
        let extra: Vec<String> = obj.keys().filter(|k| !known.contains(&k.as_str())).cloned().collect();
        if !extra.is_empty() {
            out.push(crate::types::check::unrecognized(Vec::new(), extra));
        }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "theme" => <Option<ThemePreference> as Check>::describe(rest),
                "lightTheme" => <Option<ThemeId> as Check>::describe(rest),
                "darkTheme" => <Option<ThemeId> as Check>::describe(rest),
                "sidebarMode" => <Option<SidebarMode> as Check>::describe(rest),
                "sidebarWidth" => <Option<Option<JsInt>> as Check>::describe(rest),
                "terminalSize" => <Option<TerminalSizeSource> as Check>::describe(rest),
                "terminalZoom" => <Option<IndexMap<String, Option<JsInt>>> as Check>::describe(rest),
                "access" => <Option<IndexMap<String, Option<String>>> as Check>::describe(rest),
                "target" => <Option<Option<PreferencesTarget>> as Check>::describe(rest),
                "projectLook" => <Option<IndexMap<String, Option<ProjectLook>>> as Check>::describe(rest),
                "computerLook" => <Option<IndexMap<String, Option<ComputerLook>>> as Check>::describe(rest),
                "recipeLook" => <Option<IndexMap<String, Option<RecipeLook>>> as Check>::describe(rest),
                "threadsShut" => None,
                "serverIcons" => <Option<bool> as Check>::describe(rest),
                "agentVersions" => <Option<bool> as Check>::describe(rest),
                "usageLogs" => <Option<bool> as Check>::describe(rest),
                "productUsage" => <Option<bool> as Check>::describe(rest),
                "editor" => <Option<EditorId> as Check>::describe(rest),
                "updateDismissed" => <Option<String> as Check>::describe(rest),
                "cloudCardDismissed" => <Option<bool> as Check>::describe(rest),
                "keepAwake" => <Option<bool> as Check>::describe(rest),
                "transparency" => <Option<bool> as Check>::describe(rest),
                "projectOrder" => <Option<Vec<String>> as Check>::describe(rest),
                "keybindings" => <Option<IndexMap<String, Option<String>>> as Check>::describe(rest),
                "appFont" => <Option<String> as Check>::describe(rest),
                "codeFont" => <Option<String> as Check>::describe(rest),
                "textSize" => <Option<Option<JsInt>> as Check>::describe(rest),
                "codeSize" => <Option<Option<JsInt>> as Check>::describe(rest),
                "defaultAgent" => <Option<Option<String>> as Check>::describe(rest),
                "agentDefaults" => <Option<IndexMap<String, Option<AgentDefaultsPatch>>> as Check>::describe(rest),
                "projectDefaults" => <Option<IndexMap<String, Option<ProjectOverridesPatch>>> as Check>::describe(rest),
                "sendWith" => <Option<SendKey> as Check>::describe(rest),
                "midTurn" => <Option<MidTurn> as Check>::describe(rest),
                "notifyNeeds" => <Option<NotifyChoice> as Check>::describe(rest),
                "notifyDone" => <Option<NotifyChoice> as Check>::describe(rest),
                "planAlerts" => <Option<bool> as Check>::describe(rest),
                "settleAfter" => <Option<SettleAfter> as Check>::describe(rest),
                "askDelete" => <Option<bool> as Check>::describe(rest),
                "onQuit" => <Option<OnQuit> as Check>::describe(rest),
                "newThreadIn" => <Option<NewThreadIn> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl PreferencesPatch {
    fn rule_sidebar_width(x: &Option<Option<JsInt>>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(Some(v)) = *x {
            crate::types::check::num_above(v.as_f64(), 0.0)?;
        }
        Ok(())
    }
    fn rule_keybindings(x: &Option<IndexMap<String, Option<String>>>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(list) = x {
            for (k, item) in list {
                if let Some(v) = item {
                    crate::types::check::text_at_least(v, 1).map_err(|e| e.under(k.as_str()))?;
                }
            }
        }
        if let Some(list) = x {
            for (k, item) in list {
                if let Some(v) = item {
                    crate::types::check::text_at_most(v, 64).map_err(|e| e.under(k.as_str()))?;
                }
            }
        }
        Ok(())
    }
    fn rule_app_font(x: &Option<String>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = x {
            crate::types::check::text_at_most(v, 128)?;
        }
        Ok(())
    }
    fn rule_code_font(x: &Option<String>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = x {
            crate::types::check::text_at_most(v, 128)?;
        }
        Ok(())
    }
    fn rule_text_size(x: &Option<Option<JsInt>>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(Some(v)) = *x {
            crate::types::check::refine(crate::views::preferences::TEXT_SIZES.contains(&(v.get() as u64)), "one of 13, 14, 15, 16, 17")?;
        }
        Ok(())
    }
    fn rule_code_size(x: &Option<Option<JsInt>>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(Some(v)) = *x {
            crate::types::check::refine(crate::views::preferences::CODE_SIZES.contains(&(v.get() as u64)), "one of 11, 12, 13, 14, 15")?;
        }
        Ok(())
    }
}

/// TS: packages/protocol/src/views/preferences.ts:290 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadMarkedEvent {
    pub workspace_id: String,
    pub thread_ids: Vec<String>,
}

impl ThreadMarkedEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "thread.marked";
}

impl Check for ThreadMarkedEvent {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
        crate::types::keys::req_key::<Vec<String>>(obj, "threadIds", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "workspaceId" => <String as Check>::describe(rest),
                "threadIds" => <Vec<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/preferences.ts:297 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadFacts {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub thread_id: Option<String>,
    pub workspace_id: String,
    pub harness: String,
    pub started_by: SessionOrigin,
    pub status: SessionStatus,
    pub title: String,
    pub session_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub claude_session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub started_at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub ended_at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub cwd: Option<String>,
    pub turns: JsInt,
    pub ran: bool,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub parent_thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub root_thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub attempt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub asking: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub waiting_on: Option<ThreadWaitingOn>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub setup_refusal: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub sub: Option<SubRow>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub disk_kept_until: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub disk_gone: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub capped: Option<ThreadCapWait>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub limit: Option<TurnLimit>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub resume_at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub cost_usd: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub last_line: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub failure: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub pid: Option<JsInt>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub agent_pid: Option<JsInt>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub read_at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub settled_at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub pinned_at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub order: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub folded_at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub replaces: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub replaced_by: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub snoozed_until: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub woke_at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub section: Option<ThreadPlacement>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub rewound_at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub forked_from: Option<ForkedFrom>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub permission_mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub fast: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub effort: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub context_window: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub turn_id: Option<String>,
}

impl Check for ThreadFacts {
    /// Rules: turns: > 0.
    /// Nested checks: startedBy, status, waitingOn, sub, capped, limit, section, forkedFrom.
    fn check(&self) -> Result<(), SchemaError> {
        self.started_by.check().map_err(|e| e.under("startedBy"))?;
        self.status.check().map_err(|e| e.under("status"))?;
        crate::types::check::num_above(self.turns.0 .0, 0.0).map_err(|e| e.under("turns"))?;
        self.waiting_on.check().map_err(|e| e.under("waitingOn"))?;
        self.sub.check().map_err(|e| e.under("sub"))?;
        self.capped.check().map_err(|e| e.under("capped"))?;
        self.limit.check().map_err(|e| e.under("limit"))?;
        self.section.check().map_err(|e| e.under("section"))?;
        self.forked_from.check().map_err(|e| e.under("forkedFrom"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "id", &mut out);
        crate::types::keys::opt_key::<String>(obj, "threadId", &mut out);
        crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
        crate::types::keys::req_key::<String>(obj, "harness", &mut out);
        crate::types::keys::req_key::<SessionOrigin>(obj, "startedBy", &mut out);
        crate::types::keys::req_key::<SessionStatus>(obj, "status", &mut out);
        crate::types::keys::req_key::<String>(obj, "title", &mut out);
        crate::types::keys::req_key::<String>(obj, "sessionId", &mut out);
        crate::types::keys::opt_key::<String>(obj, "claudeSessionId", &mut out);
        crate::types::keys::opt_key::<JsNumber>(obj, "startedAt", &mut out);
        crate::types::keys::opt_key::<JsNumber>(obj, "endedAt", &mut out);
        crate::types::keys::opt_key::<String>(obj, "cwd", &mut out);
        crate::types::keys::req_key_with::<JsInt>(obj, "turns", &mut out, Self::rule_turns);
        crate::types::keys::req_key::<bool>(obj, "ran", &mut out);
        crate::types::keys::opt_key::<String>(obj, "parentThreadId", &mut out);
        crate::types::keys::opt_key::<String>(obj, "rootThreadId", &mut out);
        crate::types::keys::opt_key::<String>(obj, "attempt", &mut out);
        crate::types::keys::opt_key::<String>(obj, "asking", &mut out);
        crate::types::keys::opt_key::<ThreadWaitingOn>(obj, "waitingOn", &mut out);
        crate::types::keys::opt_key::<String>(obj, "setupRefusal", &mut out);
        crate::types::keys::opt_key::<SubRow>(obj, "sub", &mut out);
        crate::types::keys::opt_key::<JsNumber>(obj, "diskKeptUntil", &mut out);
        crate::types::keys::opt_key::<String>(obj, "diskGone", &mut out);
        crate::types::keys::opt_key::<ThreadCapWait>(obj, "capped", &mut out);
        crate::types::keys::opt_key::<TurnLimit>(obj, "limit", &mut out);
        crate::types::keys::opt_key::<JsNumber>(obj, "resumeAt", &mut out);
        crate::types::keys::opt_key::<JsNumber>(obj, "costUsd", &mut out);
        crate::types::keys::opt_key::<String>(obj, "lastLine", &mut out);
        crate::types::keys::opt_key::<String>(obj, "failure", &mut out);
        crate::types::keys::opt_key::<JsInt>(obj, "pid", &mut out);
        crate::types::keys::opt_key::<JsInt>(obj, "agentPid", &mut out);
        crate::types::keys::opt_key::<JsNumber>(obj, "readAt", &mut out);
        crate::types::keys::opt_key::<JsNumber>(obj, "settledAt", &mut out);
        crate::types::keys::opt_key::<JsNumber>(obj, "pinnedAt", &mut out);
        crate::types::keys::opt_key::<JsNumber>(obj, "order", &mut out);
        crate::types::keys::opt_key::<JsNumber>(obj, "foldedAt", &mut out);
        crate::types::keys::opt_key::<String>(obj, "replaces", &mut out);
        crate::types::keys::opt_key::<String>(obj, "replacedBy", &mut out);
        crate::types::keys::opt_key::<JsNumber>(obj, "snoozedUntil", &mut out);
        crate::types::keys::opt_key::<JsNumber>(obj, "wokeAt", &mut out);
        crate::types::keys::opt_key::<ThreadPlacement>(obj, "section", &mut out);
        crate::types::keys::opt_key::<JsNumber>(obj, "rewoundAt", &mut out);
        crate::types::keys::opt_key::<ForkedFrom>(obj, "forkedFrom", &mut out);
        crate::types::keys::opt_key::<String>(obj, "permissionMode", &mut out);
        crate::types::keys::opt_key::<bool>(obj, "fast", &mut out);
        crate::types::keys::opt_key::<String>(obj, "model", &mut out);
        crate::types::keys::opt_key::<String>(obj, "effort", &mut out);
        crate::types::keys::opt_key::<String>(obj, "contextWindow", &mut out);
        crate::types::keys::opt_key::<String>(obj, "turnId", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "id" => <String as Check>::describe(rest),
                "threadId" => <Option<String> as Check>::describe(rest),
                "workspaceId" => <String as Check>::describe(rest),
                "harness" => <String as Check>::describe(rest),
                "startedBy" => <SessionOrigin as Check>::describe(rest),
                "status" => <SessionStatus as Check>::describe(rest),
                "title" => <String as Check>::describe(rest),
                "sessionId" => <String as Check>::describe(rest),
                "claudeSessionId" => <Option<String> as Check>::describe(rest),
                "startedAt" => <Option<JsNumber> as Check>::describe(rest),
                "endedAt" => <Option<JsNumber> as Check>::describe(rest),
                "cwd" => <Option<String> as Check>::describe(rest),
                "turns" => <i64 as Check>::describe(rest),
                "ran" => <bool as Check>::describe(rest),
                "parentThreadId" => <Option<String> as Check>::describe(rest),
                "rootThreadId" => <Option<String> as Check>::describe(rest),
                "attempt" => <Option<String> as Check>::describe(rest),
                "asking" => <Option<String> as Check>::describe(rest),
                "waitingOn" => <Option<ThreadWaitingOn> as Check>::describe(rest),
                "setupRefusal" => <Option<String> as Check>::describe(rest),
                "sub" => <Option<SubRow> as Check>::describe(rest),
                "diskKeptUntil" => <Option<JsNumber> as Check>::describe(rest),
                "diskGone" => <Option<String> as Check>::describe(rest),
                "capped" => <Option<ThreadCapWait> as Check>::describe(rest),
                "limit" => <Option<TurnLimit> as Check>::describe(rest),
                "resumeAt" => <Option<JsNumber> as Check>::describe(rest),
                "costUsd" => <Option<JsNumber> as Check>::describe(rest),
                "lastLine" => <Option<String> as Check>::describe(rest),
                "failure" => <Option<String> as Check>::describe(rest),
                "pid" => <Option<i64> as Check>::describe(rest),
                "agentPid" => <Option<i64> as Check>::describe(rest),
                "readAt" => <Option<JsNumber> as Check>::describe(rest),
                "settledAt" => <Option<JsNumber> as Check>::describe(rest),
                "pinnedAt" => <Option<JsNumber> as Check>::describe(rest),
                "order" => <Option<JsNumber> as Check>::describe(rest),
                "foldedAt" => <Option<JsNumber> as Check>::describe(rest),
                "replaces" => <Option<String> as Check>::describe(rest),
                "replacedBy" => <Option<String> as Check>::describe(rest),
                "snoozedUntil" => <Option<JsNumber> as Check>::describe(rest),
                "wokeAt" => <Option<JsNumber> as Check>::describe(rest),
                "section" => <Option<ThreadPlacement> as Check>::describe(rest),
                "rewoundAt" => <Option<JsNumber> as Check>::describe(rest),
                "forkedFrom" => <Option<ForkedFrom> as Check>::describe(rest),
                "permissionMode" => <Option<String> as Check>::describe(rest),
                "fast" => <Option<bool> as Check>::describe(rest),
                "model" => <Option<String> as Check>::describe(rest),
                "effort" => <Option<String> as Check>::describe(rest),
                "contextWindow" => <Option<String> as Check>::describe(rest),
                "turnId" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl ThreadFacts {
    fn rule_turns(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::num_above(x.0 .0, 0.0)?;
        Ok(())
    }
}

/// TS: packages/protocol/src/views/preferences.ts:321 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadHead {
    pub facts: ThreadFacts,
    pub events: Vec<SessionEvent>,
    pub pos: JsInt,
    pub total: JsInt,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub trimmed: Option<True>,
}

impl Check for ThreadHead {
    /// Rules: pos: >= 0; total: >= 0.
    /// Nested checks: facts, events.
    fn check(&self) -> Result<(), SchemaError> {
        self.facts.check().map_err(|e| e.under("facts"))?;
        self.events.check().map_err(|e| e.under("events"))?;
        crate::types::check::num_at_least(self.pos.0 .0, 0.0).map_err(|e| e.under("pos"))?;
        crate::types::check::num_at_least(self.total.0 .0, 0.0).map_err(|e| e.under("total"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<ThreadFacts>(obj, "facts", &mut out);
        crate::types::keys::req_key::<Vec<SessionEvent>>(obj, "events", &mut out);
        crate::types::keys::req_key_with::<JsInt>(obj, "pos", &mut out, Self::rule_pos);
        crate::types::keys::req_key_with::<JsInt>(obj, "total", &mut out, Self::rule_total);
        crate::types::keys::opt_key::<True>(obj, "trimmed", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "facts" => <ThreadFacts as Check>::describe(rest),
                "events" => <Vec<SessionEvent> as Check>::describe(rest),
                "pos" => <i64 as Check>::describe(rest),
                "total" => <i64 as Check>::describe(rest),
                "trimmed" => None,
                _ => None,
            },
        }
    }
}
impl ThreadHead {
    fn rule_pos(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::num_at_least(x.0 .0, 0.0)?;
        Ok(())
    }
    fn rule_total(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::num_at_least(x.0 .0, 0.0)?;
        Ok(())
    }
}

/// TS: packages/protocol/src/views/preferences.ts:333 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryPage {
    pub events: Vec<SessionEvent>,
    pub pos: JsInt,
    pub total: JsInt,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub trimmed: Option<True>,
}

impl Check for HistoryPage {
    /// Rules: pos: >= 0; total: >= 0.
    /// Nested checks: events.
    fn check(&self) -> Result<(), SchemaError> {
        self.events.check().map_err(|e| e.under("events"))?;
        crate::types::check::num_at_least(self.pos.0 .0, 0.0).map_err(|e| e.under("pos"))?;
        crate::types::check::num_at_least(self.total.0 .0, 0.0).map_err(|e| e.under("total"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<Vec<SessionEvent>>(obj, "events", &mut out);
        crate::types::keys::req_key_with::<JsInt>(obj, "pos", &mut out, Self::rule_pos);
        crate::types::keys::req_key_with::<JsInt>(obj, "total", &mut out, Self::rule_total);
        crate::types::keys::opt_key::<True>(obj, "trimmed", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "events" => <Vec<SessionEvent> as Check>::describe(rest),
                "pos" => <i64 as Check>::describe(rest),
                "total" => <i64 as Check>::describe(rest),
                "trimmed" => None,
                _ => None,
            },
        }
    }
}
impl HistoryPage {
    fn rule_pos(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::num_at_least(x.0 .0, 0.0)?;
        Ok(())
    }
    fn rule_total(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::num_at_least(x.0 .0, 0.0)?;
        Ok(())
    }
}

/// TS: packages/protocol/src/views/preferences.ts:343 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadHeadEvent {
    pub workspace_id: String,
    pub thread_id: String,
    pub facts: ThreadFacts,
    pub pos: JsInt,
}

impl ThreadHeadEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "thread.head";
}

impl Check for ThreadHeadEvent {
    /// Rules: pos: >= 0.
    /// Nested checks: facts.
    fn check(&self) -> Result<(), SchemaError> {
        self.facts.check().map_err(|e| e.under("facts"))?;
        crate::types::check::num_at_least(self.pos.0 .0, 0.0).map_err(|e| e.under("pos"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
        crate::types::keys::req_key::<String>(obj, "threadId", &mut out);
        crate::types::keys::req_key::<ThreadFacts>(obj, "facts", &mut out);
        crate::types::keys::req_key_with::<JsInt>(obj, "pos", &mut out, Self::rule_pos);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "workspaceId" => <String as Check>::describe(rest),
                "threadId" => <String as Check>::describe(rest),
                "facts" => <ThreadFacts as Check>::describe(rest),
                "pos" => <i64 as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl ThreadHeadEvent {
    fn rule_pos(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::num_at_least(x.0 .0, 0.0)?;
        Ok(())
    }
}

/// TS: packages/protocol/src/views/preferences.ts:354 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadRewoundEvent {
    pub workspace_id: String,
    pub thread_id: String,
}

impl ThreadRewoundEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "thread.rewound";
}

impl Check for ThreadRewoundEvent {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
        crate::types::keys::req_key::<String>(obj, "threadId", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "workspaceId" => <String as Check>::describe(rest),
                "threadId" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/preferences.ts:358 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreferencesChangedEvent {
    pub preferences: Preferences,
}

impl PreferencesChangedEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "preferences.changed";
}

impl Check for PreferencesChangedEvent {
    /// Nested checks: preferences.
    fn check(&self) -> Result<(), SchemaError> {
        self.preferences.check().map_err(|e| e.under("preferences"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<Preferences>(obj, "preferences", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "preferences" => <Preferences as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

// ---- Hand-written below: the functions of the unit.

use std::collections::HashMap;

use crate::types::env::{CLOUD_ENV, LABS_ENV};
use crate::types::general_prefs::{general_defaults, patched_general, GeneralPatch, GeneralPreferences};
use crate::types::thread_defaults::Fields;

// views/preferences.ts:65 at a3b870a6e
// export const editorOpensHereLine = (name: string): string => `These files are on ${name}, so they open here.`;
pub fn editor_opens_here_line(name: &str) -> String {
    format!("These files are on {name}, so they open here.")
}

// views/preferences.ts:68 at a3b870a6e
// export const sshIncludeLine = (name: string): string => `${name} opens in your editor over ssh, which needs one line at the top of ~/.ssh/config.`;
pub fn ssh_include_line(name: &str) -> String {
    format!("{name} opens in your editor over ssh, which needs one line at the top of ~/.ssh/config.")
}

// views/preferences.ts:70 at a3b870a6e
// export const sshCopyHereLine = (name: string): string => `${name} is a copy on this computer, so its folder opens here with no ssh.`;
pub fn ssh_copy_here_line(name: &str) -> String {
    format!("{name} is a copy on this computer, so its folder opens here with no ssh.")
}

// views/preferences.ts:74 at a3b870a6e
// export const sshBehindLine = (name: string): string => `${name}'s computer runs an older wsp; it updates when its running turn ends, then Open works.`;
pub fn ssh_behind_line(name: &str) -> String {
    format!("{name}'s computer runs an older wsp; it updates when its running turn ends, then Open works.")
}

// views/preferences.ts:174 at a3b870a6e
// export const labsFromEnv = (env: Record<string, string | undefined>): boolean => env[LABS_ENV] === "1";
/// `WSP_LABS` (crate::types::env::LABS_ENV) set to exactly "1"; nothing else counts.
pub fn labs_from_env(env: &HashMap<String, String>) -> bool {
    env.get(LABS_ENV).is_some_and(|v| v == "1")
}

// views/preferences.ts:177 at a3b870a6e
// export const cloudFromEnv = (env: Readonly<Record<string, string | undefined>>): boolean => env[CLOUD_ENV] === "1";
/// crate::types::env::CLOUD_ENV set to exactly "1"; nothing else counts.
pub fn cloud_from_env(env: &HashMap<String, String>) -> bool {
    env.get(CLOUD_ENV).is_some_and(|v| v == "1")
}

// views/preferences.ts:206 at a3b870a6e
// export const DEFAULT_PREFERENCES: Preferences = { theme: "system", ...THEME_PICK_DEFAULTS, sidebarMode: "list", terminalSize: "app", terminalZoom: {}, access: {}, projectLook: {}, projectIcon: {}, computerLook: {}, serverIcons: true, agentVersions: true, usageLogs: true, productUsage: true, keepAwake: true, transparency: true, projectOrder: [], keybindings: {}, appFont: "", codeFont: "", agentDefaults: {}, projectDefaults: {}, ...GENERAL_DEFAULTS, labs: false };
/// The record before anything is set; spec/preferences.md lists every value.
pub fn default_preferences() -> Preferences {
    let general = general_defaults();
    Preferences {
        theme: ThemePreference::System,
        light_theme: default_preferences_light_theme(),
        dark_theme: default_preferences_dark_theme(),
        sidebar_mode: SidebarMode::List,
        terminal_size: TerminalSizeSource::App,
        terminal_zoom: IndexMap::new(),
        access: IndexMap::new(),
        project_look: IndexMap::new(),
        project_icon: IndexMap::new(),
        computer_look: IndexMap::new(),
        server_icons: true,
        agent_versions: true,
        usage_logs: true,
        product_usage: true,
        keep_awake: true,
        transparency: true,
        project_order: Vec::new(),
        keybindings: IndexMap::new(),
        app_font: String::new(),
        code_font: String::new(),
        agent_defaults: IndexMap::new(),
        project_defaults: IndexMap::new(),
        send_with: general.send_with,
        mid_turn: general.mid_turn,
        notify_needs: general.notify_needs,
        notify_done: general.notify_done,
        plan_alerts: general.plan_alerts,
        settle_after: general.settle_after,
        ask_delete: general.ask_delete,
        on_quit: general.on_quit,
        new_thread_in: general.new_thread_in,
        labs: false,
        sidebar_width: None,
        target: None,
        editor: None,
        update_dismissed: None,
        cloud_card_dismissed: None,
        text_size: None,
        code_size: None,
        default_agent: None,
        recipe_look: None,
        threads_shut: None,
    }
}

// views/preferences.ts:209 at a3b870a6e
// export function preferencesFrom(stored: unknown): Preferences {
//   const parsed = Preferences.partial().safeParse(stored ?? {});
//   if (!parsed.success) return DEFAULT_PREFERENCES;
//   const { projectIcon, ...patch } = parsed.data;
//   return { ...applyPreferencesPatch(DEFAULT_PREFERENCES, patch), projectIcon: projectIcon ?? {} };
// }
/// The record as stored, over the defaults. `None` (and JSON null) reads as `{}`; a stored value that does not parse
/// as `PreferencesStored` reads as the defaults whole. The stored `projectIcon` is kept as it was (`{}` when absent),
/// and the stored `labs` is dropped: labs is always the default's.
pub fn preferences_from(stored: Option<&serde_json::Value>) -> Preferences {
    let stored = match stored {
        None | Some(serde_json::Value::Null) => serde_json::Value::Object(serde_json::Map::new()),
        Some(value) => value.clone(),
    };
    let Ok(parsed) = crate::types::check::parse::<PreferencesStored>(&stored) else {
        return default_preferences();
    };
    let project_icon = parsed.project_icon.clone().unwrap_or_default();
    let serde_json::Value::Object(mut fields) = serde_json::to_value(&parsed).unwrap_or(serde_json::Value::Null) else {
        return default_preferences();
    };
    fields.shift_remove("projectIcon");
    fields.shift_remove("labs");
    let Ok(patch) = serde_json::from_value::<PreferencesPatch>(serde_json::Value::Object(fields)) else {
        return default_preferences();
    };
    Preferences { project_icon, ..apply_preferences_patch(&default_preferences(), &patch) }
}

/// `perWorkspace`: a null entry takes its key away, a value sets it; a key set again after it was taken away goes last.
fn per_key<V: Clone>(kept: &IndexMap<String, V>, moved: Option<&IndexMap<String, Option<V>>>) -> IndexMap<String, V> {
    let mut next = kept.clone();
    for (id, value) in moved.into_iter().flatten() {
        match value {
            Some(value) => {
                next.insert(id.clone(), value.clone());
            }
            None => {
                next.shift_remove(id);
            }
        }
    }
    next
}

/// `fieldsById`: a null entry takes its id away, a field patch merges through `patched_fields`, and an id left with no
/// field goes with it.
fn fields_by_id<T, P>(kept: &IndexMap<String, Fields<T>>, moved: Option<&IndexMap<String, Option<P>>>) -> IndexMap<String, Fields<T>>
where
    T: Clone + Serialize + serde::de::DeserializeOwned,
    P: Serialize,
{
    let mut next = kept.clone();
    for (id, value) in moved.into_iter().flatten() {
        match value.as_ref().and_then(|patch| Fields::patched(next.get(id), patch)) {
            Some(merged) => {
                next.insert(id.clone(), merged);
            }
            None => {
                next.shift_remove(id);
            }
        }
    }
    next
}

// views/preferences.ts:218 at a3b870a6e
// export function applyPreferencesPatch(current: Preferences, patch: PreferencesPatch): Preferences {
//   const sidebarWidth = patch.sidebarWidth === undefined ? current.sidebarWidth : patch.sidebarWidth;
//   const perWorkspace = <T,>(kept: Record<string, T>, moved: Record<string, T | null> | undefined): Record<string, T> => {
//     const next = { ...kept };
//     for (const [workspaceId, value] of Object.entries(moved ?? {})) {
//       if (value === null) delete next[workspaceId];
//       else next[workspaceId] = value;
//     }
//     return next;
//   };
//   const target = patch.target === undefined ? current.target : patch.target;
//   const editor = patch.editor ?? current.editor;
//   const updateDismissed = patch.updateDismissed ?? current.updateDismissed;
//   const cloudCardDismissed = patch.cloudCardDismissed ?? current.cloudCardDismissed;
//   const textSize = patch.textSize === undefined ? current.textSize : patch.textSize;
//   const codeSize = patch.codeSize === undefined ? current.codeSize : patch.codeSize;
//   const defaultAgent = patch.defaultAgent === undefined ? current.defaultAgent : patch.defaultAgent;
//   const recipeLook = patch.recipeLook === undefined ? current.recipeLook : perWorkspace(current.recipeLook ?? {}, patch.recipeLook);
//   const threadsShut = patch.threadsShut === undefined ? current.threadsShut : perWorkspace(current.threadsShut ?? {}, patch.threadsShut);
//   const fieldsById = <T extends object>(kept: Record<string, T>, moved: Record<string, { [K in keyof T]?: T[K] | null } | null> | undefined): Record<string, T> => {
//     const next = { ...kept };
//     for (const [id, value] of Object.entries(moved ?? {})) {
//       const merged = value === null ? undefined : patchedFields(next[id], value);
//       if (merged === undefined) delete next[id];
//       else next[id] = merged;
//     }
//     return next;
//   };
//   return {
//     theme: patch.theme ?? current.theme,
//     lightTheme: patch.lightTheme ?? current.lightTheme,
//     darkTheme: patch.darkTheme ?? current.darkTheme,
//     sidebarMode: patch.sidebarMode ?? current.sidebarMode,
//     terminalSize: patch.terminalSize ?? current.terminalSize,
//     terminalZoom: perWorkspace(current.terminalZoom, patch.terminalZoom),
//     access: perWorkspace(current.access, patch.access),
//     projectLook: perWorkspace(current.projectLook, patch.projectLook),
//     projectIcon: current.projectIcon,
//     computerLook: perWorkspace(current.computerLook, patch.computerLook),
//     serverIcons: patch.serverIcons ?? current.serverIcons,
//     agentVersions: patch.agentVersions ?? current.agentVersions,
//     usageLogs: patch.usageLogs ?? current.usageLogs,
//     productUsage: patch.productUsage ?? current.productUsage,
//     keepAwake: patch.keepAwake ?? current.keepAwake,
//     transparency: patch.transparency ?? current.transparency,
//     projectOrder: patch.projectOrder ?? current.projectOrder,
//     keybindings: perWorkspace(current.keybindings, patch.keybindings),
//     appFont: patch.appFont ?? current.appFont,
//     codeFont: patch.codeFont ?? current.codeFont,
//     agentDefaults: fieldsById(current.agentDefaults, patch.agentDefaults),
//     projectDefaults: fieldsById(current.projectDefaults, patch.projectDefaults),
//     ...patchedGeneral(current, patch),
//     labs: current.labs,
//     ...(sidebarWidth === null || sidebarWidth === undefined ? {} : { sidebarWidth }),
//     ...(target === null || target === undefined ? {} : { target }),
//     ...(editor === undefined ? {} : { editor }),
//     ...(updateDismissed === undefined ? {} : { updateDismissed }),
//     ...(cloudCardDismissed === undefined ? {} : { cloudCardDismissed }),
//     ...(textSize === null || textSize === undefined ? {} : { textSize }),
//     ...(codeSize === null || codeSize === undefined ? {} : { codeSize }),
//     ...(defaultAgent === null || defaultAgent === undefined ? {} : { defaultAgent }),
//     ...(recipeLook === undefined ? {} : { recipeLook }),
//     ...(threadsShut === undefined ? {} : { threadsShut }),
//   };
// }
/// The record with the patch's fields over it: the one merge rule the host and a painting client share.
/// spec/preferences.md gives the rule field by field.
pub fn apply_preferences_patch(current: &Preferences, patch: &PreferencesPatch) -> Preferences {
    let general = patched_general(
        &GeneralPreferences {
            send_with: current.send_with,
            mid_turn: current.mid_turn,
            notify_needs: current.notify_needs,
            notify_done: current.notify_done,
            plan_alerts: current.plan_alerts,
            settle_after: current.settle_after,
            ask_delete: current.ask_delete,
            on_quit: current.on_quit,
            new_thread_in: current.new_thread_in,
        },
        &GeneralPatch {
            send_with: patch.send_with,
            mid_turn: patch.mid_turn,
            notify_needs: patch.notify_needs,
            notify_done: patch.notify_done,
            plan_alerts: patch.plan_alerts,
            settle_after: patch.settle_after,
            ask_delete: patch.ask_delete,
            on_quit: patch.on_quit,
            new_thread_in: patch.new_thread_in,
        },
    );
    let recipe_look = match &patch.recipe_look {
        None => current.recipe_look.clone(),
        Some(moved) => Some(per_key(&current.recipe_look.clone().unwrap_or_default(), Some(moved))),
    };
    let threads_shut = match &patch.threads_shut {
        None => current.threads_shut.clone(),
        Some(moved) => Some(per_key(&current.threads_shut.clone().unwrap_or_default(), Some(moved))),
    };
    Preferences {
        theme: patch.theme.unwrap_or(current.theme),
        light_theme: patch.light_theme.clone().unwrap_or_else(|| current.light_theme.clone()),
        dark_theme: patch.dark_theme.clone().unwrap_or_else(|| current.dark_theme.clone()),
        sidebar_mode: patch.sidebar_mode.unwrap_or(current.sidebar_mode),
        terminal_size: patch.terminal_size.unwrap_or(current.terminal_size),
        terminal_zoom: per_key(&current.terminal_zoom, patch.terminal_zoom.as_ref()),
        access: per_key(&current.access, patch.access.as_ref()),
        project_look: per_key(&current.project_look, patch.project_look.as_ref()),
        project_icon: current.project_icon.clone(),
        computer_look: per_key(&current.computer_look, patch.computer_look.as_ref()),
        server_icons: patch.server_icons.unwrap_or(current.server_icons),
        agent_versions: patch.agent_versions.unwrap_or(current.agent_versions),
        usage_logs: patch.usage_logs.unwrap_or(current.usage_logs),
        product_usage: patch.product_usage.unwrap_or(current.product_usage),
        keep_awake: patch.keep_awake.unwrap_or(current.keep_awake),
        transparency: patch.transparency.unwrap_or(current.transparency),
        project_order: patch.project_order.clone().unwrap_or_else(|| current.project_order.clone()),
        keybindings: per_key(&current.keybindings, patch.keybindings.as_ref()),
        app_font: patch.app_font.clone().unwrap_or_else(|| current.app_font.clone()),
        code_font: patch.code_font.clone().unwrap_or_else(|| current.code_font.clone()),
        agent_defaults: fields_by_id(&current.agent_defaults, patch.agent_defaults.as_ref()),
        project_defaults: fields_by_id(&current.project_defaults, patch.project_defaults.as_ref()),
        send_with: general.send_with,
        mid_turn: general.mid_turn,
        notify_needs: general.notify_needs,
        notify_done: general.notify_done,
        plan_alerts: general.plan_alerts,
        settle_after: general.settle_after,
        ask_delete: general.ask_delete,
        on_quit: general.on_quit,
        new_thread_in: general.new_thread_in,
        labs: current.labs,
        sidebar_width: patch.sidebar_width.unwrap_or(current.sidebar_width),
        target: patch.target.clone().unwrap_or_else(|| current.target.clone()),
        editor: patch.editor.or(current.editor),
        update_dismissed: patch.update_dismissed.clone().or_else(|| current.update_dismissed.clone()),
        cloud_card_dismissed: patch.cloud_card_dismissed.or(current.cloud_card_dismissed),
        text_size: patch.text_size.unwrap_or(current.text_size),
        code_size: patch.code_size.unwrap_or(current.code_size),
        default_agent: patch.default_agent.clone().unwrap_or_else(|| current.default_agent.clone()),
        recipe_look,
        threads_shut,
    }
}

// views/preferences.ts:285 at a3b870a6e
// export const serverIconsLeftLine = (folder: string, reason: string): string =>
//   `Server icons are off, but ${folder} could not be deleted: ${reason}. Delete it by hand.`;
pub fn server_icons_left_line(folder: &str, reason: &str) -> String {
    format!("Server icons are off, but {folder} could not be deleted: {reason}. Delete it by hand.")
}
