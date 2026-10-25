// SPDX-License-Identifier: AGPL-3.0-only
//! Review of wsp-words, joined: the numeric status a TS error carries (ruling 3 of 6401022701 on #2448).

use wsp_words::exit::*;

// runtime/src/machines/create.ts:156 and runtime/src/images/golden.ts:635 stamp it:
//   Object.assign(new Error(line), { kind: "concurrency", status: (refusal as WspError).status })
// runtime/src/machines/machines.ts:88 reads it: an answer with a numeric status ends the attempt.
#[test]
fn an_error_keeps_the_numeric_status_its_provider_answered() {
    let e = HostError { status: Some(409u16.into()), ..HostError::with_kind("the provider refused the create", Kind::Concurrency) };
    assert_eq!(e.status.map(|s| s.to_string()).as_deref(), Some("409"));
    assert_eq!(HostError::new("plain").status, None);
}
