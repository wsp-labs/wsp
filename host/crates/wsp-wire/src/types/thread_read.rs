// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/thread-read.ts at a3b870a6e: its zod shapes and plain constants, written by the generator.

use serde::{Deserialize, Serialize};

use crate::types::check::{Check, SchemaError};
use crate::types::js_number::JsNumber;

/// TS: packages/protocol/src/thread-read.ts:35 at a3b870a6e.
pub const THREAD_SEED_CHARS: u64 = 100_000;

/// TS: packages/protocol/src/thread-read.ts:18 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ThreadVoice {
    #[serde(rename = "person")]
    Person,
    #[serde(rename = "agent")]
    Agent,
    #[serde(rename = "tool")]
    Tool,
    #[serde(rename = "turn")]
    Turn,
}

impl Check for ThreadVoice {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["person", "agent", "tool", "turn"]))
    }
}

/// TS: packages/protocol/src/thread-read.ts:21 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadMessage {
    pub who: ThreadVoice,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub at: Option<JsNumber>,
    pub text: String,
}

impl Check for ThreadMessage {
    /// Nested checks: who.
    fn check(&self) -> Result<(), SchemaError> {
        self.who.check().map_err(|e| e.under("who"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "who" => <ThreadVoice as Check>::describe(rest),
                "at" => <Option<JsNumber> as Check>::describe(rest),
                "text" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
