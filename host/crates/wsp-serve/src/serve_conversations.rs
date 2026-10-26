// SPDX-License-Identifier: AGPL-3.0-only
//! Ported from packages/runtime/src/serve-conversations.ts at 65f6b89c7: the conversation ops a socket asks. Spec:
//! spec/serve_helpers.md, H26.
use serde::Serialize;
use serde_json::Value;
use wsp_core::types::api::{ConversationAsk, ConversationsApi};
use wsp_wire::views::workspace::Caller;
use wsp_wire::wire::runtime_ops::{RuntimeOp, RuntimeRequest};
use wsp_words::exit::HostError;

use crate::serve::served::Payload;

/// `msg.op.startsWith("conversations.")`.
pub fn is_conversations_request(msg: &RuntimeRequest) -> bool {
    msg.op.name().starts_with("conversations.")
}

/// `answerConversations(conversations, msg, origin)`. serve-conversations.ts:10-17.
pub async fn answer_conversations(
    conversations: &dyn ConversationsApi,
    msg: &RuntimeRequest,
    origin: Option<Caller>,
) -> Result<Payload, HostError> {
    match &msg.op {
        RuntimeOp::ConversationsList { project, agent, .. } => spread(&conversations.list(project.clone(), agent.clone(), origin).await?),
        RuntimeOp::ConversationsRead { project, agent, conversation, preview, .. } => {
            let ask = ConversationAsk {
                project: project.clone(),
                agent: agent.clone(),
                id: conversation.clone(),
                preview: preview.filter(|preview| *preview),
            };
            spread(&conversations.read(ask, origin).await?)
        }
        _ => Err(HostError::new(format!("{} is not answered here", msg.op.name()))),
    }
}

/// The answer's fields beside id and ok, as the TS spreads them.
fn spread<T: Serialize>(answer: &T) -> Result<Payload, HostError> {
    match serde_json::to_value(answer).map_err(|e| HostError::new(e.to_string()))? {
        Value::Object(fields) => Ok(fields),
        _ => Err(HostError::new("the answer is not an object")),
    }
}
