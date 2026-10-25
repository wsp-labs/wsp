// SPDX-License-Identifier: AGPL-3.0-only
//! The shapes of thread-read.ts against what zod answered for the same inputs. Generated.

use super::common::{cases, cases_tagged};
use wsp_wire::types::thread_read::*;

const FIXTURES: &str = include_str!("../fixtures/shapes/types_thread_read.json");

/// 5 cases: what zod's parse of ThreadMessage wrote, or the first issue it raised.
#[test]
fn thread_message_reads_and_writes_as_zod_does() {
    cases::<ThreadMessage>(FIXTURES, "ThreadMessage");
}

/// 5 cases: what zod's parse of ThreadVoice wrote, or the first issue it raised.
#[test]
fn thread_voice_reads_and_writes_as_zod_does() {
    cases::<ThreadVoice>(FIXTURES, "ThreadVoice");
}
