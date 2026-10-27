// SPDX-License-Identifier: AGPL-3.0-only
//! `token-shapes.ts`: the one reading of what a secret looks like in a recipe or a file it lands, and the words that name one.
use std::rc::Rc;

/// `tokenShaped` (`token-shapes.ts:7`), re-exported from the protocol and not ported in Rust yet.
pub fn token_shaped(_text: &str) -> bool {
    todo!()
}

/// `holdsPrivateKey` (`token-shapes.ts:7`), re-exported from the protocol and not ported in Rust yet.
pub fn holds_private_key(_text: &str) -> bool {
    todo!()
}

/// `TokenMark` (`protocol/src/token-shapes.ts:35`), the `mark` of `blankTokens`: a text, or a function of each run.
pub enum TokenMark {
    Text(String),
    Run(Rc<dyn Fn(&str) -> String>),
}

/// `blankTokens` (`token-shapes.ts:7`), re-exported from the protocol and not ported in Rust yet.
pub fn blank_tokens(_text: &str, _mark: TokenMark) -> String {
    todo!()
}

/// `TOKEN_NAME` (`token-shapes.ts:9`).
pub const TOKEN_NAME: &str = "a value shaped like a token";

/// `LOGIN_NAME` (`token-shapes.ts:10`).
pub const LOGIN_NAME: &str = "a login in an address";

/// `ADDRESS_RUN` (`token-shapes.ts:16`), the regex source; the global flag is the caller's.
pub const ADDRESS_RUN: &str = r#"[A-Za-z][A-Za-z0-9+.-]*://[^\s"'<>`]+"#;

/// `unquoted` (`token-shapes.ts:17`).
pub fn unquoted(_v: &str) -> String {
    todo!()
}

/// `carries` (`token-shapes.ts:20`).
pub fn carries(_value: &str) -> bool {
    todo!()
}

/// `querySecret` (`token-shapes.ts:29`).
pub fn query_secret(_name: &str, _value: &str) -> bool {
    todo!()
}

/// `secretIn` (`token-shapes.ts:43`).
pub fn secret_in(_text: &str) -> Option<String> {
    todo!()
}
