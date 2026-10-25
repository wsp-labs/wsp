// SPDX-License-Identifier: AGPL-3.0-only
//! Ported from packages/runtime/src/serve-conversations.ts at 65f6b89c7: the conversation ops a socket asks. Spec:
//! spec/serve_helpers.md, H26.
use wsp_core::types::api::ConversationsApi;
use wsp_wire::views::workspace::Caller;
use wsp_wire::wire::runtime_ops::RuntimeRequest;
use wsp_words::exit::HostError;

use crate::serve::served::Payload;

/// `msg.op.startsWith("conversations.")`.
pub fn is_conversations_request(msg: &RuntimeRequest) -> bool {
    todo!()
}

/// `answerConversations(conversations, msg, origin)`. serve-conversations.ts:10-17.
pub async fn answer_conversations(conversations: &dyn ConversationsApi, msg: &RuntimeRequest, origin: Option<Caller>) -> Result<Payload, HostError> {
    todo!()
}
