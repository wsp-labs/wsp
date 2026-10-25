// SPDX-License-Identifier: AGPL-3.0-only
use super::*;

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

pub(super) fn default_preferences_light_theme() -> ThemeId {
    from_json("\"paper\"")
}

pub(super) fn default_preferences_dark_theme() -> ThemeId {
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
