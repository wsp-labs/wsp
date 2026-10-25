// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/conversations.ts at a3b870a6e: its zod shapes and plain constants, written by the generator.

use serde::{Deserialize, Serialize};

use crate::types::check::{Check, SchemaError};
use crate::types::js_number::JsNumber;

/// TS: packages/protocol/src/conversations.ts:107 at a3b870a6e.
pub const EARLIER_MESSAGES: u64 = 200;

/// TS: packages/protocol/src/conversations.ts:110 at a3b870a6e.
pub const PREVIEW_MESSAGES: u64 = 4;

/// TS: packages/protocol/src/conversations.ts:111 at a3b870a6e.
pub const PREVIEW_TAIL_BYTES: u64 = 262_144;

/// TS: packages/protocol/src/conversations.ts:124 at a3b870a6e.
pub const CONVERSATION_OPEN_KIND: &str = "conversation-open";

/// TS: packages/protocol/src/conversations.ts:167 at a3b870a6e.
pub const CONVERSATION_GONE_FIX: &str = "Pick one that is still there from wsp conversations.";

/// TS: packages/protocol/src/conversations.ts:171 at a3b870a6e.
pub const NO_CONVERSATIONS_FIX: &str = "Resume a Claude Code or Codex conversation.";

/// TS: packages/protocol/src/conversations.ts:172 at a3b870a6e.
pub const RESUME_WHERE_LINE: &str = "a resumed conversation runs where it ran, so --resume takes no --branch, --cwd or --beside";

/// TS: packages/protocol/src/conversations.ts:173 at a3b870a6e.
pub const RESUME_WHERE_FIX: &str = "Leave --branch, --cwd and --beside off; the thread opens in the folder the conversation ran in.";

/// TS: packages/protocol/src/conversations.ts:174 at a3b870a6e.
pub const RESUME_A_THREAD_LINE: &str = "--resume opens a thread, so it does not go to one that is already open";

/// TS: packages/protocol/src/conversations.ts:175 at a3b870a6e.
pub const RESUME_A_THREAD_FIX: &str =
    "Start a thread on the conversation with wsp run --resume <id>, or send into the open one without it.";

/// TS: packages/protocol/src/conversations.ts:176 at a3b870a6e.
pub const RESUME_HERE_LINE: &str =
    "--resume picks up a conversation on the project's own computer, and this project's threads run on machines wsp makes for them";

/// TS: packages/protocol/src/conversations.ts:177 at a3b870a6e.
pub const RESUME_HERE_FIX: &str = "Name a project on this computer or on a box.";

/// TS: packages/protocol/src/conversations.ts:178 at a3b870a6e.
pub const COPY_ALONE_LINE: &str = "--copy copies the conversation --resume names, so it goes with --resume";

/// TS: packages/protocol/src/conversations.ts:179 at a3b870a6e.
pub const COPY_ALONE_FIX: &str = "Add --resume <id>, or leave --copy off.";

/// TS: packages/protocol/src/conversations.ts:182 at a3b870a6e.
pub const CONVERSATIONS_AGAIN_FIX: &str = "Ask again in a moment.";

/// TS: packages/protocol/src/conversations.ts:10 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ConversationOrigin {
    #[serde(rename = "terminal")]
    Terminal,
    #[serde(rename = "wsp")]
    Wsp,
    #[serde(rename = "script")]
    Script,
}

impl Check for ConversationOrigin {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["terminal", "wsp", "script"]))
    }
}

/// TS: packages/protocol/src/conversations.ts:17 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutsideConversation {
    pub agent: String,
    pub id: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub first_prompt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub branch: Option<String>,
    pub cwd: String,
    pub last_at: JsNumber,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub bytes: Option<JsNumber>,
    pub origin: ConversationOrigin,
    pub live: bool,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub thread: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub lets_go: Option<String>,
}

impl Check for OutsideConversation {
    /// Nested checks: origin.
    fn check(&self) -> Result<(), SchemaError> {
        self.origin.check().map_err(|e| e.under("origin"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "agent", &mut out);
        crate::types::keys::req_key::<String>(obj, "id", &mut out);
        crate::types::keys::req_key::<String>(obj, "title", &mut out);
        crate::types::keys::opt_key::<String>(obj, "firstPrompt", &mut out);
        crate::types::keys::opt_key::<String>(obj, "branch", &mut out);
        crate::types::keys::req_key::<String>(obj, "cwd", &mut out);
        crate::types::keys::req_key::<JsNumber>(obj, "lastAt", &mut out);
        crate::types::keys::opt_key::<JsNumber>(obj, "bytes", &mut out);
        crate::types::keys::req_key::<ConversationOrigin>(obj, "origin", &mut out);
        crate::types::keys::req_key::<bool>(obj, "live", &mut out);
        crate::types::keys::opt_key::<String>(obj, "thread", &mut out);
        crate::types::keys::opt_key::<String>(obj, "letsGo", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "agent" => <String as Check>::describe(rest),
                "id" => <String as Check>::describe(rest),
                "title" => <String as Check>::describe(rest),
                "firstPrompt" => <Option<String> as Check>::describe(rest),
                "branch" => <Option<String> as Check>::describe(rest),
                "cwd" => <String as Check>::describe(rest),
                "lastAt" => <JsNumber as Check>::describe(rest),
                "bytes" => <Option<JsNumber> as Check>::describe(rest),
                "origin" => <ConversationOrigin as Check>::describe(rest),
                "live" => <bool as Check>::describe(rest),
                "thread" => <Option<String> as Check>::describe(rest),
                "letsGo" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/conversations.ts:35 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationsHeld {
    pub agent: String,
    pub said: String,
    pub fix: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub why: Option<String>,
}

impl Check for ConversationsHeld {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "agent", &mut out);
        crate::types::keys::req_key::<String>(obj, "said", &mut out);
        crate::types::keys::req_key::<String>(obj, "fix", &mut out);
        crate::types::keys::opt_key::<String>(obj, "why", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "agent" => <String as Check>::describe(rest),
                "said" => <String as Check>::describe(rest),
                "fix" => <String as Check>::describe(rest),
                "why" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/conversations.ts:39 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationsAnswer {
    pub rows: Vec<OutsideConversation>,
    pub held: Vec<ConversationsHeld>,
}

impl Check for ConversationsAnswer {
    /// Nested checks: rows, held.
    fn check(&self) -> Result<(), SchemaError> {
        self.rows.check().map_err(|e| e.under("rows"))?;
        self.held.check().map_err(|e| e.under("held"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<Vec<OutsideConversation>>(obj, "rows", &mut out);
        crate::types::keys::req_key::<Vec<ConversationsHeld>>(obj, "held", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "rows" => <Vec<OutsideConversation> as Check>::describe(rest),
                "held" => <Vec<ConversationsHeld> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/conversations.ts:43 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResumeAsk {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub copy: Option<bool>,
}

impl Check for ResumeAsk {
    /// Rules: id: at least 1 UTF-16 units; id: at most 200 UTF-16 units.
    fn check(&self) -> Result<(), SchemaError> {
        crate::types::check::text_at_least(&self.id, 1).map_err(|e| e.under("id"))?;
        crate::types::check::text_at_most(&self.id, 200).map_err(|e| e.under("id"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key_with::<String>(obj, "id", &mut out, Self::rule_id);
        crate::types::keys::opt_key::<bool>(obj, "copy", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "id" => <String as Check>::describe(rest),
                "copy" => <Option<bool> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl ResumeAsk {
    fn rule_id(x: &String) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::text_at_least(x, 1)?;
        crate::types::check::text_at_most(x, 200)?;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ConversationReadRowsItemWho {
    #[serde(rename = "person")]
    Person,
    #[serde(rename = "agent")]
    Agent,
    #[serde(rename = "tool")]
    Tool,
    #[serde(rename = "note")]
    Note,
}

impl Check for ConversationReadRowsItemWho {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["person", "agent", "tool", "note"]))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationReadRowsItem {
    pub who: ConversationReadRowsItemWho,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub at: Option<JsNumber>,
}

impl Check for ConversationReadRowsItem {
    fn check(&self) -> Result<(), SchemaError> {
        self.who.check().map_err(|e| e.under("who"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<ConversationReadRowsItemWho>(obj, "who", &mut out);
        crate::types::keys::req_key::<String>(obj, "text", &mut out);
        crate::types::keys::opt_key::<JsNumber>(obj, "at", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "who" => <ConversationReadRowsItemWho as Check>::describe(rest),
                "text" => <String as Check>::describe(rest),
                "at" => <Option<JsNumber> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/conversations.ts:116 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationRead {
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub cwd: Option<String>,
    pub title: String,
    pub rows: Vec<ConversationReadRowsItem>,
}

impl Check for ConversationRead {
    /// Nested checks: rows.
    fn check(&self) -> Result<(), SchemaError> {
        self.rows.check().map_err(|e| e.under("rows"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::opt_key::<String>(obj, "cwd", &mut out);
        crate::types::keys::req_key::<String>(obj, "title", &mut out);
        crate::types::keys::req_key::<Vec<ConversationReadRowsItem>>(obj, "rows", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "cwd" => <Option<String> as Check>::describe(rest),
                "title" => <String as Check>::describe(rest),
                "rows" => <Vec<ConversationReadRowsItem> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
