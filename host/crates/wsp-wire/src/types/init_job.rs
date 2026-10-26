// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/setup.ts at 65f6b89c7 (the dead-code cut; init-job.ts is gone and these shapes moved there): its zod shapes and plain constants, written by the generator.

use serde::{Deserialize, Serialize};

use crate::types::check::{Check, SchemaError};
use crate::views::place_kind::PlaceKind;

/// TS: packages/protocol/src/setup.ts:19 at 65f6b89c7.
pub const LOGIN_CHOICES: &[&str] = &["copy", "machine", "later", "key", "skip", "token"];

/// TS: packages/protocol/src/setup.ts:32 at 65f6b89c7.
pub const SIGN_IN_CODE_MAX: u64 = 256;

/// TS: packages/protocol/src/setup.ts:42 at 65f6b89c7.
pub const KEY_REFUSED: &str = "keyRefused";

/// TS: packages/protocol/src/setup.ts:43 at 65f6b89c7.
pub const KEY_UNCHECKED: &str = "keyUnchecked";

/// TS: packages/protocol/src/setup.ts:12 at 65f6b89c7.
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
        crate::types::check::leaf(
            path,
            crate::types::check::Zod::Options(&["signed-in", "not-signed-in", "not-verified", "skipped", "copied", "deferred"]),
        )
    }
}

/// TS: packages/protocol/src/setup.ts:20 at 65f6b89c7.
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

/// TS: packages/protocol/src/setup.ts:27 at 65f6b89c7.
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

/// TS: packages/protocol/src/setup.ts:37 at 65f6b89c7.
pub type InitKeys = crate::types::kept::Record<bool>;

/// TS: packages/protocol/src/setup.ts:47 at 65f6b89c7.
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

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "id", &mut out);
        crate::types::keys::req_key::<String>(obj, "name", &mut out);
        crate::types::keys::opt_key::<PlaceKind>(obj, "kind", &mut out);
        Some(out)
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
