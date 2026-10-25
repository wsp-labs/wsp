// SPDX-License-Identifier: AGPL-3.0-only
//! The shapes of conversations.ts against what zod answered for the same inputs. Generated.

use super::common::{cases, cases_tagged};
use wsp_wire::types::conversations::*;

const FIXTURES: &str = include_str!("../fixtures/shapes/types_conversations.json");

/// 4 cases: what zod's parse of ConversationOrigin wrote, or the first issue it raised.
#[test]
fn conversation_origin_reads_and_writes_as_zod_does() {
    cases::<ConversationOrigin>(FIXTURES, "ConversationOrigin");
}

/// 4 cases: what zod's parse of ConversationRead wrote, or the first issue it raised.
#[test]
fn conversation_read_reads_and_writes_as_zod_does() {
    cases::<ConversationRead>(FIXTURES, "ConversationRead");
}

/// 3 cases: what zod's parse of ConversationsAnswer wrote, or the first issue it raised.
#[test]
fn conversations_answer_reads_and_writes_as_zod_does() {
    cases::<ConversationsAnswer>(FIXTURES, "ConversationsAnswer");
}

/// 4 cases: what zod's parse of ConversationsHeld wrote, or the first issue it raised.
#[test]
fn conversations_held_reads_and_writes_as_zod_does() {
    cases::<ConversationsHeld>(FIXTURES, "ConversationsHeld");
}

/// 6 cases: what zod's parse of OutsideConversation wrote, or the first issue it raised.
#[test]
fn outside_conversation_reads_and_writes_as_zod_does() {
    cases::<OutsideConversation>(FIXTURES, "OutsideConversation");
}

/// 7 cases: what zod's parse of ResumeAsk wrote, or the first issue it raised.
#[test]
fn resume_ask_reads_and_writes_as_zod_does() {
    cases::<ResumeAsk>(FIXTURES, "ResumeAsk");
}
