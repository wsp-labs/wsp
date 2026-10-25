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
    pub sidebar_width: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub terminal_size: Option<TerminalSizeSource>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub terminal_zoom: Option<IndexMap<String, i64>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub access: Option<IndexMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub target: Option<PreferencesTarget>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub project_look: Option<IndexMap<String, ProjectLook>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub project_icon: Option<IndexMap<String, ProjectIconHash>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub computer_look: Option<IndexMap<String, ComputerLook>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub recipe_look: Option<IndexMap<String, RecipeLook>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
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
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub keybindings: Option<IndexMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub app_font: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub code_font: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub text_size: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub code_size: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub default_agent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub agent_defaults: Option<IndexMap<String, AgentDefaults>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub project_defaults: Option<IndexMap<String, ProjectOverrides>>,
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
            crate::types::check::num_above(v as f64, 0.0).map_err(|e| e.under("sidebarWidth"))?;
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
            crate::types::check::refine(crate::views::preferences::TEXT_SIZES.contains(&(v as u64)), "one of 13, 14, 15, 16, 17").map_err(|e| e.under("textSize"))?;
        }
        if let Some(v) = self.code_size {
            crate::types::check::refine(crate::views::preferences::CODE_SIZES.contains(&(v as u64)), "one of 11, 12, 13, 14, 15").map_err(|e| e.under("codeSize"))?;
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
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["laptop", "desktop", "mac-mini", "mac-studio", "tower", "server", "cloud", "cpu", "drive", "container", "home"]))
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
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["vscode", "cursor", "vscode-insiders", "zed", "idea", "webstorm", "pycharm", "goland", "rustrover", "clion", "phpstorm", "rubymine", "rider", "finder"]))
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
    #[serde(serialize_with = "crate::types::check::js_record")]
    pub terminal_zoom: IndexMap<String, i64>,
    #[serde(serialize_with = "crate::types::check::js_record")]
    pub access: IndexMap<String, String>,
    #[serde(serialize_with = "crate::types::check::js_record")]
    pub project_look: IndexMap<String, ProjectLook>,
    #[serde(default = "default_preferences_project_icon", serialize_with = "crate::types::check::js_record")]
    pub project_icon: IndexMap<String, ProjectIconHash>,
    #[serde(default = "default_preferences_computer_look", serialize_with = "crate::types::check::js_record")]
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
    #[serde(default = "default_preferences_keybindings", serialize_with = "crate::types::check::js_record")]
    pub keybindings: IndexMap<String, String>,
    #[serde(default = "default_preferences_app_font")]
    pub app_font: String,
    #[serde(default = "default_preferences_code_font")]
    pub code_font: String,
    #[serde(default = "default_preferences_agent_defaults", serialize_with = "crate::types::check::js_record")]
    pub agent_defaults: IndexMap<String, AgentDefaults>,
    #[serde(default = "default_preferences_project_defaults", serialize_with = "crate::types::check::js_record")]
    pub project_defaults: IndexMap<String, ProjectOverrides>,
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
    pub sidebar_width: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub target: Option<PreferencesTarget>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub editor: Option<EditorId>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub update_dismissed: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub cloud_card_dismissed: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub text_size: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub code_size: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub default_agent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub recipe_look: Option<IndexMap<String, RecipeLook>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
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
            crate::types::check::num_above(v as f64, 0.0).map_err(|e| e.under("sidebarWidth"))?;
        }
        self.target.check().map_err(|e| e.under("target"))?;
        self.editor.check().map_err(|e| e.under("editor"))?;
        if let Some(v) = self.text_size {
            crate::types::check::refine(crate::views::preferences::TEXT_SIZES.contains(&(v as u64)), "one of 13, 14, 15, 16, 17").map_err(|e| e.under("textSize"))?;
        }
        if let Some(v) = self.code_size {
            crate::types::check::refine(crate::views::preferences::CODE_SIZES.contains(&(v as u64)), "one of 11, 12, 13, 14, 15").map_err(|e| e.under("codeSize"))?;
        }
        self.recipe_look.check().map_err(|e| e.under("recipeLook"))?;
        self.light_theme.check().map_err(|e| e.under("lightTheme"))?;
        self.dark_theme.check().map_err(|e| e.under("darkTheme"))?;
        self.project_icon.check().map_err(|e| e.under("projectIcon"))?;
        Ok(())
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

fn default_preferences_agent_defaults() -> IndexMap<String, AgentDefaults> {
    from_json("{}")
}

fn default_preferences_project_defaults() -> IndexMap<String, ProjectOverrides> {
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
    pub sidebar_width: Option<Option<i64>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub terminal_size: Option<TerminalSizeSource>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub terminal_zoom: Option<IndexMap<String, Option<i64>>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub access: Option<IndexMap<String, Option<String>>>,
    #[serde(default, skip_serializing_if = "Option::is_none", with = "crate::types::check::double_option")]
    pub target: Option<Option<PreferencesTarget>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub project_look: Option<IndexMap<String, Option<ProjectLook>>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub computer_look: Option<IndexMap<String, Option<ComputerLook>>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub recipe_look: Option<IndexMap<String, Option<RecipeLook>>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
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
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub keybindings: Option<IndexMap<String, Option<String>>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub app_font: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub code_font: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", with = "crate::types::check::double_option")]
    pub text_size: Option<Option<i64>>,
    #[serde(default, skip_serializing_if = "Option::is_none", with = "crate::types::check::double_option")]
    pub code_size: Option<Option<i64>>,
    #[serde(default, skip_serializing_if = "Option::is_none", with = "crate::types::check::double_option")]
    pub default_agent: Option<Option<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub agent_defaults: Option<IndexMap<String, Option<AgentDefaultsPatch>>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
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
            crate::types::check::num_above(v as f64, 0.0).map_err(|e| e.under("sidebarWidth"))?;
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
            crate::types::check::refine(crate::views::preferences::TEXT_SIZES.contains(&(v as u64)), "one of 13, 14, 15, 16, 17").map_err(|e| e.under("textSize"))?;
        }
        if let Some(Some(v)) = self.code_size {
            crate::types::check::refine(crate::views::preferences::CODE_SIZES.contains(&(v as u64)), "one of 11, 12, 13, 14, 15").map_err(|e| e.under("codeSize"))?;
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

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "theme" => <Option<ThemePreference> as Check>::describe(rest),
                "lightTheme" => <Option<ThemeId> as Check>::describe(rest),
                "darkTheme" => <Option<ThemeId> as Check>::describe(rest),
                "sidebarMode" => <Option<SidebarMode> as Check>::describe(rest),
                "sidebarWidth" => <Option<Option<i64>> as Check>::describe(rest),
                "terminalSize" => <Option<TerminalSizeSource> as Check>::describe(rest),
                "terminalZoom" => <Option<IndexMap<String, Option<i64>>> as Check>::describe(rest),
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
                "textSize" => <Option<Option<i64>> as Check>::describe(rest),
                "codeSize" => <Option<Option<i64>> as Check>::describe(rest),
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
    pub turns: i64,
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
    pub pid: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub agent_pid: Option<i64>,
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
        crate::types::check::num_above(self.turns as f64, 0.0).map_err(|e| e.under("turns"))?;
        self.waiting_on.check().map_err(|e| e.under("waitingOn"))?;
        self.sub.check().map_err(|e| e.under("sub"))?;
        self.capped.check().map_err(|e| e.under("capped"))?;
        self.limit.check().map_err(|e| e.under("limit"))?;
        self.section.check().map_err(|e| e.under("section"))?;
        self.forked_from.check().map_err(|e| e.under("forkedFrom"))?;
        Ok(())
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

/// TS: packages/protocol/src/views/preferences.ts:321 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadHead {
    pub facts: ThreadFacts,
    pub events: Vec<SessionEvent>,
    pub pos: i64,
    pub total: i64,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub trimmed: Option<True>,
}

impl Check for ThreadHead {
    /// Rules: pos: >= 0; total: >= 0.
    /// Nested checks: facts, events.
    fn check(&self) -> Result<(), SchemaError> {
        self.facts.check().map_err(|e| e.under("facts"))?;
        self.events.check().map_err(|e| e.under("events"))?;
        crate::types::check::num_at_least(self.pos as f64, 0.0).map_err(|e| e.under("pos"))?;
        crate::types::check::num_at_least(self.total as f64, 0.0).map_err(|e| e.under("total"))?;
        Ok(())
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

/// TS: packages/protocol/src/views/preferences.ts:333 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryPage {
    pub events: Vec<SessionEvent>,
    pub pos: i64,
    pub total: i64,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub trimmed: Option<True>,
}

impl Check for HistoryPage {
    /// Rules: pos: >= 0; total: >= 0.
    /// Nested checks: events.
    fn check(&self) -> Result<(), SchemaError> {
        self.events.check().map_err(|e| e.under("events"))?;
        crate::types::check::num_at_least(self.pos as f64, 0.0).map_err(|e| e.under("pos"))?;
        crate::types::check::num_at_least(self.total as f64, 0.0).map_err(|e| e.under("total"))?;
        Ok(())
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

/// TS: packages/protocol/src/views/preferences.ts:343 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadHeadEvent {
    pub workspace_id: String,
    pub thread_id: String,
    pub facts: ThreadFacts,
    pub pos: i64,
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
        crate::types::check::num_at_least(self.pos as f64, 0.0).map_err(|e| e.under("pos"))?;
        Ok(())
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

// views/preferences.ts:65 at a3b870a6e
// export const editorOpensHereLine = (name: string): string => `These files are on ${name}, so they open here.`;
pub fn editor_opens_here_line(name: &str) -> String {
    todo!()
}

// views/preferences.ts:68 at a3b870a6e
// export const sshIncludeLine = (name: string): string => `${name} opens in your editor over ssh, which needs one line at the top of ~/.ssh/config.`;
pub fn ssh_include_line(name: &str) -> String {
    todo!()
}

// views/preferences.ts:70 at a3b870a6e
// export const sshCopyHereLine = (name: string): string => `${name} is a copy on this computer, so its folder opens here with no ssh.`;
pub fn ssh_copy_here_line(name: &str) -> String {
    todo!()
}

// views/preferences.ts:74 at a3b870a6e
// export const sshBehindLine = (name: string): string => `${name}'s computer runs an older wsp; it updates when its running turn ends, then Open works.`;
pub fn ssh_behind_line(name: &str) -> String {
    todo!()
}

// views/preferences.ts:174 at a3b870a6e
// export const labsFromEnv = (env: Record<string, string | undefined>): boolean => env[LABS_ENV] === "1";
/// `WSP_LABS` (crate::types::env::LABS_ENV) set to exactly "1"; nothing else counts.
pub fn labs_from_env(env: &HashMap<String, String>) -> bool {
    todo!()
}

// views/preferences.ts:177 at a3b870a6e
// export const cloudFromEnv = (env: Readonly<Record<string, string | undefined>>): boolean => env[CLOUD_ENV] === "1";
/// crate::types::env::CLOUD_ENV set to exactly "1"; nothing else counts.
pub fn cloud_from_env(env: &HashMap<String, String>) -> bool {
    todo!()
}

// views/preferences.ts:206 at a3b870a6e
// export const DEFAULT_PREFERENCES: Preferences = { theme: "system", ...THEME_PICK_DEFAULTS, sidebarMode: "list", terminalSize: "app", terminalZoom: {}, access: {}, projectLook: {}, projectIcon: {}, computerLook: {}, serverIcons: true, agentVersions: true, usageLogs: true, productUsage: true, keepAwake: true, transparency: true, projectOrder: [], keybindings: {}, appFont: "", codeFont: "", agentDefaults: {}, projectDefaults: {}, ...GENERAL_DEFAULTS, labs: false };
/// The record before anything is set; spec/preferences.md lists every value.
pub fn default_preferences() -> Preferences {
    todo!()
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
    todo!()
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
    todo!()
}

// views/preferences.ts:285 at a3b870a6e
// export const serverIconsLeftLine = (folder: string, reason: string): string =>
//   `Server icons are off, but ${folder} could not be deleted: ${reason}. Delete it by hand.`;
pub fn server_icons_left_line(folder: &str, reason: &str) -> String {
    todo!()
}
