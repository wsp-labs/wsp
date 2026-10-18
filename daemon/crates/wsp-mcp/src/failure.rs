// SPDX-License-Identifier: AGPL-3.0-only
//! A refusal or a failure as the one contract every door answers with: its sentence, and the class the kind stamped on
//! it where it was born belongs to, never read off its words. A tool call answers it as a tool error carrying the
//! failure object; a refusal before the server runs is one line on stderr and the class's exit code.

use serde::Serialize;
use serde_json::value::RawValue;

use crate::record;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
    pub message: String,
    pub kind: Option<String>,
    /// The lists a refused slate write carries beside its sentence, each in the bytes the host wrote it:
    /// `problemListsOf` in packages/protocol/src/exit.ts.
    pub errors: Option<String>,
    pub warnings: Option<String>,
    /// What to do about it, where the host said: `message` ends with it, as `refusal` joins the two; `said()` is the
    /// sentence before it (`refusalParts` in packages/protocol/src/exit.ts).
    pub fix: Option<Box<str>>,
    /// The threads a `processes-left` refusal names, each with what it still runs, as the host sent them
    /// (`processesLeftOf`, the protocol's `LeftThread[]`).
    pub left: Option<Box<serde_json::Value>>,
}

/// The failure object: what --json prints on stderr and a tool error carries beside its text.
#[derive(Debug, Serialize)]
pub struct Object {
    pub error: String,
    pub class: String,
    pub exit: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub errors: Option<Box<RawValue>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warnings: Option<Box<RawValue>>,
}

impl Failure {
    /// A failure with no kind, which is the provider's class: the host, the runtime or the machine said no.
    pub fn new(message: impl Into<String>) -> Self {
        Failure { message: message.into(), kind: None, errors: None, warnings: None, fix: None, left: None }
    }

    pub fn of_kind(message: impl Into<String>, kind: &str) -> Self {
        Failure { kind: Some(kind.to_owned()), ..Failure::new(message) }
    }

    pub fn usage(message: impl Into<String>) -> Self {
        Self::of_kind(message, "usage")
    }

    pub fn auth(message: impl Into<String>) -> Self {
        Self::of_kind(message, "auth")
    }

    /// The sentence before the fix, as `refusalSaid` takes the fix back off the end; the whole message where none was
    /// joined on.
    pub fn said(&self) -> &str {
        match &self.fix {
            Some(fix) => self.message.strip_suffix(&**fix).and_then(|m| m.strip_suffix(' ')).unwrap_or(&self.message),
            None => &self.message,
        }
    }

    pub fn object(&self) -> Object {
        let exit = record::exit();
        let class = self.kind.as_ref().and_then(|kind| exit.kinds.get(kind)).map_or("provider", String::as_str).to_owned();
        let code = exit.codes.get(&class).copied().unwrap_or(1);
        let raw = |list: &Option<String>| list.as_ref().and_then(|l| RawValue::from_string(l.clone()).ok());
        Object { error: self.message.clone(), class, exit: code, errors: raw(&self.errors), warnings: raw(&self.warnings) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_kind_names_its_class_and_no_kind_is_the_providers() {
        let auth = Failure::auth("no").object();
        assert_eq!((auth.class.as_str(), auth.exit), ("auth", 2));
        let usage = Failure::of_kind("no", "not-found").object();
        assert_eq!((usage.class.as_str(), usage.exit), ("usage", 3));
        for failure in [Failure::new("no"), Failure::of_kind("no", "unreachable")] {
            let provider = failure.object();
            assert_eq!((provider.class.as_str(), provider.exit), ("provider", 1));
        }
    }

    /// Catches: `said()` cutting a message its fix does not end, or keeping the space `refusal` joined the two with.
    #[test]
    fn said_takes_the_fix_off_only_where_it_ends_the_message() {
        let with = |message: &str, fix: Option<&str>| Failure { fix: fix.map(Box::from), ..Failure::new(message) };
        assert_eq!(with("no host serves it. start one.", Some("start one.")).said(), "no host serves it.");
        assert_eq!(with("no host serves it.", None).said(), "no host serves it.");
        assert_eq!(with("no host serves it.\nstart one. and more", Some("start one.")).said(), "no host serves it.\nstart one. and more");
        assert_eq!(with("start one.", Some("start one.")).said(), "start one.");
        assert_eq!(with("no host serves it. ", Some("")).said(), "no host serves it.");
        assert_eq!(with("no", Some("a fix longer than the message")).said(), "no");
    }
}
