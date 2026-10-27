// SPDX-License-Identifier: AGPL-3.0-only
//! Port of `providers-key.ts`: a provider key put to the provider before it is saved, with the account, plan and
//! machines it reads, or the refusal in the person's words.
use wsp_engine::box_backend::BoxLimits;
use wsp_wire::wire::runtime::ProviderKeyWorks;

/// `KEY_CHECK_WORDS` (`providers-key.ts:27`), the sentences a key check is refused with, one associated function each.
pub struct KeyCheckWords;

impl KeyCheckWords {
    /// `refused` (`providers-key.ts:28`).
    pub fn refused(_name: &str) -> String {
        todo!()
    }

    /// `expired` (`providers-key.ts:29`).
    pub fn expired(_name: &str) -> String {
        todo!()
    }

    /// `lacks` (`providers-key.ts:30`).
    pub fn lacks(_action: &str) -> String {
        todo!()
    }

    /// `other` (`providers-key.ts:31`).
    pub fn other(_name: &str, _said: &str) -> String {
        todo!()
    }

    /// `older` (`providers-key.ts:32`).
    pub fn older(_name: &str) -> String {
        todo!()
    }

    /// `unreachable` (`providers-key.ts:33`).
    pub fn unreachable(_name: &str) -> String {
        todo!()
    }
}

/// `KeyRefusal` (`providers-key.ts:38`), an error that carries its kind and the provider's own words.
#[derive(Clone, Debug, PartialEq)]
pub struct KeyRefusal {
    pub message: String,
    pub kind: KeyRefusalKind,
    pub said: String,
}

/// `KeyRefusal["kind"]` (`providers-key.ts:38`), which is `KEY_REFUSED` or `KEY_UNCHECKED`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyRefusalKind {
    KeyRefused,
    KeyUnchecked,
}

/// `planWords` (`providers-key.ts:96`).
pub fn plan_words(_limits: &BoxLimits) -> String {
    todo!()
}

/// The inline return of `boatKeyCheck` (`providers-key.ts:101`).
#[derive(Clone, Debug, PartialEq)]
pub struct BoatKeyCheck {
    pub works: ProviderKeyWorks,
    pub limits: BoxLimits,
}

/// `boatKeyCheck` (`providers-key.ts:101`).
pub async fn boat_key_check(_key: &str, _name: &str, _base: &str) -> Result<BoatKeyCheck, KeyRefusal> {
    todo!()
}
