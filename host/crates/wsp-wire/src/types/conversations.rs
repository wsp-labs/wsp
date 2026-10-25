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

impl Check for ConversationOrigin {}

/// TS: packages/protocol/src/conversations.ts:17 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutsideConversation {
    pub agent: String,
    pub id: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_prompt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    pub cwd: String,
    pub last_at: JsNumber,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bytes: Option<JsNumber>,
    pub origin: ConversationOrigin,
    pub live: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lets_go: Option<String>,
}

impl Check for OutsideConversation {
    /// Nested checks: origin.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/conversations.ts:35 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationsHeld {
    pub agent: String,
    pub said: String,
    pub fix: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub why: Option<String>,
}

impl Check for ConversationsHeld {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
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
        todo!()
    }
}

/// TS: packages/protocol/src/conversations.ts:43 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResumeAsk {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub copy: Option<bool>,
}

impl Check for ResumeAsk {
    /// Rules: id: at least 1 UTF-16 units; id: at most 200 UTF-16 units.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
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

impl Check for ConversationReadRowsItemWho {}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationReadRowsItem {
    pub who: ConversationReadRowsItemWho,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<JsNumber>,
}

impl Check for ConversationReadRowsItem {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/conversations.ts:116 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationRead {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    pub title: String,
    pub rows: Vec<ConversationReadRowsItem>,
}

impl Check for ConversationRead {
    /// Nested checks: rows.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}
