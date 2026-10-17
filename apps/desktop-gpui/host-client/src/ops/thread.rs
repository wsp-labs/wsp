// SPDX-License-Identifier: AGPL-3.0-only
use serde::Deserialize;

use crate::protocol;

ops! {
    "sessions.start" => SessionsStartRequest => protocol::SessionStartResult,
    "sessions.warm" => SessionsWarmRequest => protocol::SessionWarmResult,
    "sessions.history" => SessionsHistoryRequest => History,
    "sessions.head" => SessionsHeadRequest => protocol::ThreadHead,
    "sessions.steer" => SessionsSteerRequest => protocol::SessionSteerResult,
    "sessions.interrupt" => SessionsInterruptRequest => protocol::SessionInterruptResult,
    "sessions.answer" => SessionsAnswerRequest => protocol::SessionAnswerResult,
    "sessions.access" => SessionsAccessRequest => protocol::SessionAccessResult,
    "sessions.rename" => SessionsRenameRequest => protocol::SessionRenameResult,
    "sessions.aside" => SessionsAsideRequest => protocol::SessionAsideResult,
    "sessions.rewind" => SessionsRewindRequest => protocol::SessionRewindResult,
    "sessions.run" => SessionsRunRequest => Run,
    "sessions.attachment" => SessionsAttachmentRequest => Attachment,
    "harnesses.list" => HarnessesListRequest => Harnesses,
    "conversations.list" => ConversationsListRequest => protocol::ConversationsAnswer,
    "conversations.read" => ConversationsReadRequest => protocol::ConversationRead,
    "workspaces.start" => WorkspacesStartRequest => protocol::StartResult,
    "workspaces.review" => WorkspacesReviewRequest => protocol::StartResult,
    "editor.open" => EditorOpenRequest => Editor,
    // Each ask adds a listener on the socket, so a second one doubles every frame: HostClient::events() asks once.
    "events.subscribe" => EventsSubscribeRequest => protocol::EventsSubscribeReply,
}

/// A page of one thread's events when the request names a thread and not `whole`, else every event asked for.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum History {
    Page(protocol::HistoryPage),
    Whole { events: Vec<protocol::SessionEvent> },
}

#[derive(Debug, Deserialize)]
pub struct Run {
    pub run: protocol::SessionRunEvent,
}

#[derive(Debug, Deserialize)]
pub struct Attachment {
    pub attachment: protocol::KeptAttachment,
}

#[derive(Debug, Deserialize)]
pub struct Harnesses {
    pub harnesses: Vec<protocol::HarnessCatalog>,
}

#[derive(Debug, Deserialize)]
pub struct Editor {
    pub editor: protocol::EditorId,
}
