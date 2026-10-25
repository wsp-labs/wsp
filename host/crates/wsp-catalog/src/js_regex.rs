// SPDX-License-Identifier: AGPL-3.0-only
//! A JavaScript regular expression kept as data on a catalog row (a token's shape, a sign-in's question, a code's
//! shape): its source as the TS writes it, matched with JavaScript's meaning. Lookbehind is supported, so the
//! implementation sits on fancy-regex (rule 8.7). The spec is spec/jsonc.md.
use serde::{Serialize, Serializer};

/// The source between the slashes and the flags after them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsRegex {
    pub source: &'static str,
    pub flags: &'static str,
}

impl JsRegex {
    pub const fn new(source: &'static str) -> JsRegex {
        JsRegex { source, flags: "" }
    }

    pub const fn with_flags(source: &'static str, flags: &'static str) -> JsRegex {
        JsRegex { source, flags }
    }

    /// `re.test(text)`.
    pub fn is_match(&self, text: &str) -> bool {
        todo!()
    }

    /// `re.exec(text)?.[0]`.
    pub fn find(&self, text: &str) -> Option<String> {
        todo!()
    }

    /// `new RegExp(`^(?:${source})$`).test(text)`: the whole text is one match.
    pub fn is_whole_match(&self, text: &str) -> bool {
        todo!()
    }
}

impl Serialize for JsRegex {
    /// The source alone, as the oracle fixtures write a regex.
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.source)
    }
}
