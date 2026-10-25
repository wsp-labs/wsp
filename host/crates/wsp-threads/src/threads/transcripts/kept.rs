// SPDX-License-Identifier: AGPL-3.0-only
//! transcripts_kept: sent images, the sessions document, daemon tokens. transcripts.ts:17-92, 513-545, 590-657,
//! 716-735. Spec: spec/transcripts_kept.md.
use std::sync::Arc;

use futures::future::LocalBoxFuture;
use wsp_core::types::internal::KeptImages;
use wsp_engine::machine::Machine;
use wsp_wire::types::attachments::Attachment;
use wsp_wire::types::harness_picks::RanPicks;
use wsp_wire::views::session_events_more::SessionEvent;
use wsp_words::exit::HostError;

use super::super::threads::Entry;
use super::TranscriptsArea;


pub use wsp_core::types::internal::{ATTACHMENT_KEYS, ATTACHMENTS, SESSIONS, SESSION_INDEX_CAP, DAEMON_TOKEN_MISS_TTL_MS};
/// One step of the kept-images chain: the document as read, answering the document to write, or `None` where the
/// step wrote its own (transcripts.ts:24).
pub type KeptStep = Box<dyn FnOnce(KeptImages) -> LocalBoxFuture<'static, Result<Option<KeptImages>, HostError>>>;

/// Which threads' images `drop_sent_images` takes (transcripts.ts:86).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ThreadsNamed {
    All,
    These(Vec<String>),
}

/// transcripts.ts:67 `imageKeysOf`: K5.
pub fn image_keys_of(e: &SessionEvent) -> Vec<String> {
    todo!()
}

impl TranscriptsArea {
    /// transcripts.ts:24 `onKept`: K1.
    pub fn on_kept(&self, workspace_id: &str, step: KeptStep) -> LocalBoxFuture<'static, ()> {
        todo!()
    }
    /// transcripts.ts:39: K2.
    pub fn keep_sent_images(&self, workspace_id: &str, thread_id: &str, request_id: Option<&str>, attachments: &[Attachment]) -> LocalBoxFuture<'static, ()> {
        todo!()
    }
    /// transcripts.ts:55: K3.
    pub fn drop_kept_keys(&self, workspace_id: &str, keys: &[String]) -> LocalBoxFuture<'static, ()> {
        todo!()
    }
    /// transcripts.ts:86: K4.
    pub fn drop_sent_images(&self, workspace_id: &str, threads: ThreadsNamed) -> LocalBoxFuture<'static, ()> {
        todo!()
    }
    /// transcripts.ts:590: K6. Synchronous.
    pub fn cap_sessions(&self, workspace_id: &str) {
        todo!()
    }
    /// transcripts.ts:617: K7. The document is built in the call; the future is the put on the chain.
    pub fn persist_sessions(&self, workspace_id: &str) -> LocalBoxFuture<'static, ()> {
        todo!()
    }
    /// transcripts.ts:521: K8.
    pub fn token_for_machine(&self, machine_id: &str) -> String {
        todo!()
    }
    /// transcripts.ts:532: K9.
    pub fn daemon_token_of(&self, machine: Arc<dyn Machine>, path: Option<String>) -> LocalBoxFuture<'static, Result<Option<String>, HostError>> {
        todo!()
    }
    /// transcripts.ts:723: K11.
    pub fn ran_on(&self, workspace_id: &str, thread_id: &str, session: Option<&str>) -> RanPicks {
        todo!()
    }
    /// transcripts.ts:731: K12.
    pub fn box_of(&self, thread_id: &str) -> Option<Entry> {
        todo!()
    }
}
