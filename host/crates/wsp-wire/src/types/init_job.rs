// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/init-job.ts at a3b870a6e: its zod shapes and plain constants, written by the generator.

use serde::{Deserialize, Serialize};

use crate::types::check::{Check, SchemaError};
use crate::types::js_number::JsNumber;
use crate::views::place_kind::PlaceKind;
use indexmap::IndexMap;

/// TS: packages/protocol/src/init-job.ts:93 at a3b870a6e.
pub const LOGIN_CHOICES: &[&str] = &["copy", "machine", "later", "key", "skip", "token"];

/// TS: packages/protocol/src/init-job.ts:106 at a3b870a6e.
pub const SIGN_IN_CODE_MAX: u64 = 256;

/// TS: packages/protocol/src/init-job.ts:147 at a3b870a6e.
pub const INIT_BUILD_STEP: &str = "build";

/// TS: packages/protocol/src/init-job.ts:162 at a3b870a6e.
pub const KEY_REFUSED: &str = "keyRefused";

/// TS: packages/protocol/src/init-job.ts:163 at a3b870a6e.
pub const KEY_UNCHECKED: &str = "keyUnchecked";

/// TS: packages/protocol/src/init-job.ts:13 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum InitRoad {
    #[serde(rename = "manual")]
    Manual,
    #[serde(rename = "agent")]
    Agent,
    #[serde(rename = "terminal")]
    Terminal,
}

impl Check for InitRoad {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["manual", "agent", "terminal"]))
    }
}

/// TS: packages/protocol/src/init-job.ts:19 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum InitPhase {
    #[serde(rename = "agent")]
    Agent,
    #[serde(rename = "reading")]
    Reading,
    #[serde(rename = "answering")]
    Answering,
    #[serde(rename = "building")]
    Building,
    #[serde(rename = "signing-in")]
    SigningIn,
    #[serde(rename = "sealing")]
    Sealing,
    #[serde(rename = "finishing")]
    Finishing,
    #[serde(rename = "done")]
    Done,
    #[serde(rename = "failed")]
    Failed,
    #[serde(rename = "cancelled")]
    Cancelled,
}

impl Check for InitPhase {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["agent", "reading", "answering", "building", "signing-in", "sealing", "finishing", "done", "failed", "cancelled"]))
    }
}

/// TS: packages/protocol/src/init-job.ts:23 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum InitScreenId {
    #[serde(rename = "agents")]
    Agents,
    #[serde(rename = "tools")]
    Tools,
    #[serde(rename = "also")]
    Also,
    #[serde(rename = "logins")]
    Logins,
    #[serde(rename = "wsp")]
    Wsp,
}

impl Check for InitScreenId {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["agents", "tools", "also", "logins", "wsp"]))
    }
}

/// TS: packages/protocol/src/init-job.ts:26 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitChoice {
    pub value: String,
    pub label: String,
}

impl Check for InitChoice {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "value" => <String as Check>::describe(rest),
                "label" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum InitScreenItemLock {
    #[serde(rename = "on")]
    On,
    #[serde(rename = "off")]
    Off,
}

impl Check for InitScreenItemLock {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["on", "off"]))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitScreenItemKey {
    pub name: String,
    pub saved: bool,
}

impl Check for InitScreenItemKey {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "name" => <String as Check>::describe(rest),
                "saved" => <bool as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/init-job.ts:33 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitScreenItem {
    pub id: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none", with = "crate::types::check::double_option")]
    pub size: Option<Option<JsNumber>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub why: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub group: Option<String>,
    pub detail: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub lock: Option<InitScreenItemLock>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub choices: Option<Vec<InitChoice>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub key: Option<InitScreenItemKey>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub state: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub mark: Option<String>,
}

impl Check for InitScreenItem {
    /// Rules: size: >= 0.
    /// Nested checks: choices, key.
    fn check(&self) -> Result<(), SchemaError> {
        if let Some(Some(v)) = self.size {
            crate::types::check::num_at_least(v.0, 0.0).map_err(|e| e.under("size"))?;
        }
        self.lock.check().map_err(|e| e.under("lock"))?;
        self.choices.check().map_err(|e| e.under("choices"))?;
        self.key.check().map_err(|e| e.under("key"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "id" => <String as Check>::describe(rest),
                "label" => <String as Check>::describe(rest),
                "size" => <Option<Option<JsNumber>> as Check>::describe(rest),
                "why" => <Option<String> as Check>::describe(rest),
                "group" => <Option<String> as Check>::describe(rest),
                "detail" => <Vec<String> as Check>::describe(rest),
                "lock" => <Option<InitScreenItemLock> as Check>::describe(rest),
                "choices" => <Option<Vec<InitChoice>> as Check>::describe(rest),
                "key" => <Option<InitScreenItemKey> as Check>::describe(rest),
                "state" => <Option<String> as Check>::describe(rest),
                "mark" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum InitFooterLineTone {
    #[serde(rename = "yellow")]
    Yellow,
    #[serde(rename = "yellowBright")]
    YellowBright,
    #[serde(rename = "red")]
    Red,
}

impl Check for InitFooterLineTone {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["yellow", "yellowBright", "red"]))
    }
}

/// TS: packages/protocol/src/init-job.ts:51 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitFooterLine {
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub tone: Option<InitFooterLineTone>,
}

impl Check for InitFooterLine {
    fn check(&self) -> Result<(), SchemaError> {
        self.tone.check().map_err(|e| e.under("tone"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "text" => <String as Check>::describe(rest),
                "tone" => <Option<InitFooterLineTone> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/init-job.ts:54 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitScreen {
    pub id: InitScreenId,
    pub title: String,
    pub top: String,
    pub items: Vec<InitScreenItem>,
    pub ticks: Vec<String>,
    #[serde(serialize_with = "crate::types::check::js_record")]
    pub answers: IndexMap<String, String>,
    pub footer: Vec<InitFooterLine>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub tally: Option<String>,
}

impl Check for InitScreen {
    /// Nested checks: id, items, footer.
    fn check(&self) -> Result<(), SchemaError> {
        self.id.check().map_err(|e| e.under("id"))?;
        self.items.check().map_err(|e| e.under("items"))?;
        self.footer.check().map_err(|e| e.under("footer"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "id" => <InitScreenId as Check>::describe(rest),
                "title" => <String as Check>::describe(rest),
                "top" => <String as Check>::describe(rest),
                "items" => <Vec<InitScreenItem> as Check>::describe(rest),
                "ticks" => <Vec<String> as Check>::describe(rest),
                "answers" => <IndexMap<String, String> as Check>::describe(rest),
                "footer" => <Vec<InitFooterLine> as Check>::describe(rest),
                "tally" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/init-job.ts:78 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum InitRowKind {
    #[serde(rename = "fact")]
    Fact,
    #[serde(rename = "stage")]
    Stage,
    #[serde(rename = "sign-in")]
    SignIn,
    #[serde(rename = "secret")]
    Secret,
    #[serde(rename = "agent")]
    Agent,
    #[serde(rename = "workspace")]
    Workspace,
    #[serde(rename = "project")]
    Project,
    #[serde(rename = "machine")]
    Machine,
}

impl Check for InitRowKind {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["fact", "stage", "sign-in", "secret", "agent", "workspace", "project", "machine"]))
    }
}

/// TS: packages/protocol/src/init-job.ts:86 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LoginState {
    #[serde(rename = "signed-in")]
    SignedIn,
    #[serde(rename = "not-signed-in")]
    NotSignedIn,
    #[serde(rename = "not-verified")]
    NotVerified,
    #[serde(rename = "skipped")]
    Skipped,
    #[serde(rename = "copied")]
    Copied,
    #[serde(rename = "deferred")]
    Deferred,
}

impl Check for LoginState {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["signed-in", "not-signed-in", "not-verified", "skipped", "copied", "deferred"]))
    }
}

/// TS: packages/protocol/src/init-job.ts:94 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LoginChoice {
    #[serde(rename = "copy")]
    Copy,
    #[serde(rename = "machine")]
    Machine,
    #[serde(rename = "later")]
    Later,
    #[serde(rename = "key")]
    Key,
    #[serde(rename = "skip")]
    Skip,
    #[serde(rename = "token")]
    Token,
}

impl Check for LoginChoice {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["copy", "machine", "later", "key", "skip", "token"]))
    }
}

/// TS: packages/protocol/src/init-job.ts:101 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SignInFinish {
    #[serde(rename = "callback")]
    Callback,
    #[serde(rename = "code")]
    Code,
    #[serde(rename = "none")]
    None,
}

impl Check for SignInFinish {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["callback", "code", "none"]))
    }
}

/// TS: packages/protocol/src/init-job.ts:108 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitRow {
    pub id: String,
    pub kind: InitRowKind,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub tool: Option<String>,
    pub label: String,
    pub state: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub login: Option<LoginState>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub detail: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub page: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub finish: Option<SignInFinish>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub ms: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub since: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub lines: Option<Vec<String>>,
}

impl Check for InitRow {
    /// Nested checks: kind, login, finish.
    fn check(&self) -> Result<(), SchemaError> {
        self.kind.check().map_err(|e| e.under("kind"))?;
        self.login.check().map_err(|e| e.under("login"))?;
        self.finish.check().map_err(|e| e.under("finish"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "id" => <String as Check>::describe(rest),
                "kind" => <InitRowKind as Check>::describe(rest),
                "tool" => <Option<String> as Check>::describe(rest),
                "label" => <String as Check>::describe(rest),
                "state" => <String as Check>::describe(rest),
                "login" => <Option<LoginState> as Check>::describe(rest),
                "detail" => <Option<String> as Check>::describe(rest),
                "page" => <Option<String> as Check>::describe(rest),
                "code" => <Option<String> as Check>::describe(rest),
                "finish" => <Option<SignInFinish> as Check>::describe(rest),
                "ms" => <Option<JsNumber> as Check>::describe(rest),
                "since" => <Option<JsNumber> as Check>::describe(rest),
                "lines" => <Option<Vec<String>> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/init-job.ts:142 at a3b870a6e.
pub type InitKeys = IndexMap<String, bool>;

/// TS: packages/protocol/src/init-job.ts:152 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitDraft {
    pub at: String,
    pub ticks: Vec<String>,
    #[serde(serialize_with = "crate::types::check::js_record")]
    pub answers: IndexMap<String, String>,
}

impl Check for InitDraft {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "at" => <String as Check>::describe(rest),
                "ticks" => <Vec<String> as Check>::describe(rest),
                "answers" => <IndexMap<String, String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/init-job.ts:168 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitNeedsYou {
    pub what: String,
    pub since: i64,
}

impl Check for InitNeedsYou {
    /// Rules: since: >= 0.
    fn check(&self) -> Result<(), SchemaError> {
        crate::types::check::num_at_least(self.since as f64, 0.0).map_err(|e| e.under("since"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "what" => <String as Check>::describe(rest),
                "since" => <i64 as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/init-job.ts:173 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetupPlace {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub kind: Option<PlaceKind>,
}

impl Check for SetupPlace {
    /// Nested checks: kind.
    fn check(&self) -> Result<(), SchemaError> {
        self.kind.check().map_err(|e| e.under("kind"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "id" => <String as Check>::describe(rest),
                "name" => <String as Check>::describe(rest),
                "kind" => <Option<PlaceKind> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum InitSaidLineLevel {
    #[serde(rename = "info")]
    Info,
    #[serde(rename = "step")]
    Step,
    #[serde(rename = "success")]
    Success,
    #[serde(rename = "warn")]
    Warn,
}

impl Check for InitSaidLineLevel {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["info", "step", "success", "warn"]))
    }
}

/// TS: packages/protocol/src/init-job.ts:177 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitSaidLine {
    pub level: InitSaidLineLevel,
    pub line: String,
}

impl Check for InitSaidLine {
    fn check(&self) -> Result<(), SchemaError> {
        self.level.check().map_err(|e| e.under("level"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "level" => <InitSaidLineLevel as Check>::describe(rest),
                "line" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitJobDisk {
    pub fixed: JsNumber,
    pub total: JsNumber,
}

impl Check for InitJobDisk {
    /// Rules: fixed: >= 0; total: > 0.
    fn check(&self) -> Result<(), SchemaError> {
        crate::types::check::num_at_least(self.fixed.0, 0.0).map_err(|e| e.under("fixed"))?;
        crate::types::check::num_above(self.total.0, 0.0).map_err(|e| e.under("total"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "fixed" => <JsNumber as Check>::describe(rest),
                "total" => <JsNumber as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitJobProgress {
    pub done: i64,
    pub total: i64,
}

impl Check for InitJobProgress {
    /// Rules: done: >= 0; total: >= 0.
    fn check(&self) -> Result<(), SchemaError> {
        crate::types::check::num_at_least(self.done as f64, 0.0).map_err(|e| e.under("done"))?;
        crate::types::check::num_at_least(self.total as f64, 0.0).map_err(|e| e.under("total"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "done" => <i64 as Check>::describe(rest),
                "total" => <i64 as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitJobThread {
    pub id: String,
    pub workspace_id: String,
    pub session: String,
    pub harness: String,
}

impl Check for InitJobThread {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "id" => <String as Check>::describe(rest),
                "workspaceId" => <String as Check>::describe(rest),
                "session" => <String as Check>::describe(rest),
                "harness" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitJobGolden {
    pub version: i64,
}

impl Check for InitJobGolden {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "version" => <i64 as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitJobWorkspace {
    pub id: String,
    pub name: String,
}

impl Check for InitJobWorkspace {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "id" => <String as Check>::describe(rest),
                "name" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/init-job.ts:180 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitJob {
    pub id: String,
    pub road: InitRoad,
    pub phase: InitPhase,
    pub keys: InitKeys,
    pub step: i64,
    pub stoppable: bool,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub disk: Option<InitJobDisk>,
    pub screens: Vec<InitScreen>,
    pub rows: Vec<InitRow>,
    pub progress: InitJobProgress,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub needs_you: Option<InitNeedsYou>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub drafts: Option<Vec<InitDraft>>,
    pub log: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub thread: Option<InitJobThread>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub line: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub key_refused: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub golden: Option<InitJobGolden>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub cleanup: Option<Vec<InitSaidLine>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub update: Option<Vec<InitSaidLine>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub builders: Option<Vec<InitSaidLine>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub workspace: Option<InitJobWorkspace>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub place: Option<SetupPlace>,
}

impl Check for InitJob {
    /// Rules: step: >= 0.
    /// Nested checks: road, phase, keys, disk, screens, rows, progress, needsYou, drafts, thread, golden, cleanup, update, builders, workspace, place.
    fn check(&self) -> Result<(), SchemaError> {
        self.road.check().map_err(|e| e.under("road"))?;
        self.phase.check().map_err(|e| e.under("phase"))?;
        crate::types::check::num_at_least(self.step as f64, 0.0).map_err(|e| e.under("step"))?;
        self.disk.check().map_err(|e| e.under("disk"))?;
        self.screens.check().map_err(|e| e.under("screens"))?;
        self.rows.check().map_err(|e| e.under("rows"))?;
        self.progress.check().map_err(|e| e.under("progress"))?;
        self.needs_you.check().map_err(|e| e.under("needsYou"))?;
        self.drafts.check().map_err(|e| e.under("drafts"))?;
        self.thread.check().map_err(|e| e.under("thread"))?;
        self.golden.check().map_err(|e| e.under("golden"))?;
        self.cleanup.check().map_err(|e| e.under("cleanup"))?;
        self.update.check().map_err(|e| e.under("update"))?;
        self.builders.check().map_err(|e| e.under("builders"))?;
        self.workspace.check().map_err(|e| e.under("workspace"))?;
        self.place.check().map_err(|e| e.under("place"))?;
        self.keys.check().map_err(|e| e.under("keys"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "id" => <String as Check>::describe(rest),
                "road" => <InitRoad as Check>::describe(rest),
                "phase" => <InitPhase as Check>::describe(rest),
                "keys" => <InitKeys as Check>::describe(rest),
                "step" => <i64 as Check>::describe(rest),
                "stoppable" => <bool as Check>::describe(rest),
                "disk" => <Option<InitJobDisk> as Check>::describe(rest),
                "screens" => <Vec<InitScreen> as Check>::describe(rest),
                "rows" => <Vec<InitRow> as Check>::describe(rest),
                "progress" => <InitJobProgress as Check>::describe(rest),
                "needsYou" => <Option<InitNeedsYou> as Check>::describe(rest),
                "drafts" => <Option<Vec<InitDraft>> as Check>::describe(rest),
                "log" => <Vec<String> as Check>::describe(rest),
                "thread" => <Option<InitJobThread> as Check>::describe(rest),
                "line" => <Option<String> as Check>::describe(rest),
                "error" => <Option<String> as Check>::describe(rest),
                "keyRefused" => <Option<bool> as Check>::describe(rest),
                "golden" => <Option<InitJobGolden> as Check>::describe(rest),
                "cleanup" => <Option<Vec<InitSaidLine>> as Check>::describe(rest),
                "update" => <Option<Vec<InitSaidLine>> as Check>::describe(rest),
                "builders" => <Option<Vec<InitSaidLine>> as Check>::describe(rest),
                "workspace" => <Option<InitJobWorkspace> as Check>::describe(rest),
                "place" => <Option<SetupPlace> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/init-job.ts:237 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitAgent {
    pub id: String,
    pub name: String,
    pub configured: bool,
    pub takes_tools: bool,
}

impl Check for InitAgent {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "id" => <String as Check>::describe(rest),
                "name" => <String as Check>::describe(rest),
                "configured" => <bool as Check>::describe(rest),
                "takesTools" => <bool as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/init-job.ts:241 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitJobEvent {
    pub job: InitJob,
}

impl InitJobEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "init.job";
}

impl Check for InitJobEvent {
    /// Nested checks: job.
    fn check(&self) -> Result<(), SchemaError> {
        self.job.check().map_err(|e| e.under("job"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "job" => <InitJob as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/init-job.ts:246 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitNeedsYouEvent {
    pub job_id: String,
    pub needs_you: InitNeedsYou,
}

impl InitNeedsYouEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "job.needs-you";
}

impl Check for InitNeedsYouEvent {
    /// Nested checks: needsYou.
    fn check(&self) -> Result<(), SchemaError> {
        self.needs_you.check().map_err(|e| e.under("needsYou"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "jobId" => <String as Check>::describe(rest),
                "needsYou" => <InitNeedsYou as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
