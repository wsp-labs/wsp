// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/src/key-check.ts. Spec: spec/small.md.
//!
//! Whether the provider takes a key: taken, refused in the provider's own words, or never answered at all.
use serde_json::Value;

use crate::errors::ErrorKind;
use crate::machine::MachineBackend;

/// key-check.ts:10.
#[derive(Debug, Clone, PartialEq)]
pub enum KeyCheck {
    /// `answer` is what the check read answered, where it answered with anything (`Value::Null` is no answer).
    Taken { answer: Option<Value> },
    /// The provider answered about the key and refused it: `provider_said_line(status, message)`.
    Refused { said: String },
    /// Nothing came back about the key: the failure's own message.
    Unchecked { said: String },
}

/// key-check.ts:21. The kinds that answer about the key and nothing else.
pub const REFUSING: [ErrorKind; 2] = [ErrorKind::Auth, ErrorKind::Plan];

/// key-check.ts:23. A backend whose offers have no `check_key` is `Taken { answer: None }`, nothing called. A failure
/// carrying a kind and a status, with the kind `auth` or `plan`, is `Refused`; any other failure is `Unchecked`.
pub async fn check_provider_key(backend: &dyn MachineBackend) -> KeyCheck {
    todo!()
}

/// key-check.ts:38. `None` for taken; `key_unchecked_line(said, provider)` for unchecked whatever `saved` says;
/// `saved_key_refused_line` or `key_refused_line` for refused.
pub fn key_check_line(check: &KeyCheck, provider: &str, saved: bool) -> Option<String> {
    todo!()
}
