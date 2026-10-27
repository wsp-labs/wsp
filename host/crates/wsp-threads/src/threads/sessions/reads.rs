// SPDX-License-Identifier: AGPL-3.0-only
//! sessions.ts:901-975, 1175-1187: the reads of threads. Spec: spec/sessions_reads.md.
use crate::threads::transcripts::kept::base64_of;
use std::cmp::Ordering;
use std::collections::HashSet;

use futures::future::{join_all, try_join_all};
use wsp_core::types::api::{HistoryAsk, PageWindow};
use wsp_core::types::internal::ThreadRecord;
use wsp_core::types::internal::{named_only, KeptImage, KeptImages, SearchedHit, TreeTalk, ATTACHMENTS, ATTACHMENT_KEYS};
use wsp_machines::machines::boot::entries::{EntryAsk, RowAt};
use wsp_store::transcript_reader::{head_shape, ThreadRead};
use wsp_wire::types::attachments::KeptAttachment;
use wsp_wire::types::js_number::JsInt;
use wsp_wire::views::preferences::HEAD_BYTES;
use wsp_wire::views::preferences::{HistoryPage, ThreadHead};
use wsp_wire::views::preferences::{HISTORY_PAGE_BYTES, HISTORY_PAGE_EVENTS};
use wsp_wire::views::session::{thread_key_of, SessionView};
use wsp_wire::views::session_events::TurnStatus;
use wsp_wire::views::session_events_more::{HeadEvents, SessionEvent};
use wsp_wire::views::workspace::Caller;
use wsp_wire::wire::runtime::{SessionSearchHit, SessionSearchResult};
use wsp_words::attachments::attachment_key;
use wsp_words::exit::{not_found_refusal, usage_refusal, HostError};
use wsp_words::js::js_trim;
use wsp_words::thread_read::thread_result;
use wsp_words::words::computer::thread_word;

use super::SessionsArea;
use crate::threads::trimmed_row::trimmed_row;

/// The thread an event names (`e.threadId`), None where it names none.
fn event_thread(e: &SessionEvent) -> Option<String> {
    serde_json::to_value(e).ok()?.get("threadId")?.as_str().map(str::to_string)
}

fn entry_asks(being_made: bool, now: bool) -> EntryAsk {
    EntryAsk { now, act: None, thread: None, being_made }
}

impl SessionsArea {
    /// sessions.ts:901. Rules Q1-Q3.
    pub async fn list_rows(&self, workspace_id: Option<String>, origin: Option<Caller>) -> Result<Vec<SessionView>, HostError> {
        self.ctx.ready().await?;
        if let Some(workspace_id) = &workspace_id {
            if !self.ctx.tree_stands_on(workspace_id, origin.as_ref(), Some(TreeTalk::List))? {
                self.ctx.refuse_named(workspace_id, origin.as_ref())?;
            }
        }
        let every: Vec<SessionView> = {
            let sessions = self.ctx.sessions();
            let views: Vec<SessionView> = sessions.borrow().values().map(|r| r.borrow().view.clone()).collect();
            views
        };
        let mut reached: Vec<SessionView> = Vec::new();
        for view in &every {
            if self.ctx.reaches_row(view, origin.as_ref(), Some(TreeTalk::List))? {
                reached.push(view.clone());
            }
        }
        let rows: Vec<SessionView> = match &workspace_id {
            None => reached,
            Some(workspace_id) => reached.into_iter().filter(|v| &v.workspace_id == workspace_id).collect(),
        };
        let unbooting: Vec<SessionView> = rows.iter().filter(|v| !self.ctx.booting(&v.workspace_id)).cloned().collect();
        let mut first_reads = Vec::new();
        for view in self.ctx.title_rows(unbooting) {
            let first = view.harness_title.is_none();
            if first {
                first_reads.push(self.ctx.refresh_title(view, false));
            } else {
                let ctx = self.ctx.clone();
                self.ctx.spawn(Box::pin(async move {
                    let _ = ctx.refresh_title(view, false).await;
                }));
            }
        }
        try_join_all(first_reads).await?;
        let rowed: HashSet<String> = every.iter().map(thread_key_of).collect();
        let trimmed: Vec<(String, ThreadRecord)> = self
            .ctx
            .tree_records(origin.as_ref(), TreeTalk::List, workspace_id.as_deref())?
            .into_iter()
            .filter(|(thread_id, _)| !rowed.contains(thread_id))
            .collect();
        let ends: Vec<Option<TurnStatus>> = join_all(trimmed.iter().map(|(thread_id, record)| async move {
            match record.ended {
                Some(ended) => Ok(Some(ended)),
                None => {
                    let events = self.ctx.open_thread(&record.workspace_id, thread_id).await?;
                    Ok(thread_result(&events, thread_id).map(|result| result.status))
                }
            }
        }))
        .await
        .into_iter()
        .collect::<Result<Vec<_>, HostError>>()?;
        let mut listed: Vec<SessionView> = Vec::new();
        for view in self.ctx.listed_rows(rows) {
            listed.push(if self.ctx.reaches_row(&view, origin.as_ref(), None)? { view } else { named_only(&view) });
        }
        listed.extend(trimmed.iter().zip(ends).map(|((thread_id, record), ended)| trimmed_row(&*self.ctx, thread_id, record, ended)));
        Ok(self.ctx.placed_rows(listed).await)
    }

    /// sessions.ts:928. Rule Q4.
    pub async fn history_of(&self, workspace_id: &str, origin: Option<Caller>) -> Result<Vec<SessionEvent>, HostError> {
        self.ctx.ready().await?;
        if !self.ctx.tree_stands_on(workspace_id, origin.as_ref(), None)? {
            self.ctx.entry_of_with(workspace_id, origin.as_ref(), entry_asks(true, false)).await?;
        }
        let events = self.ctx.open_transcript(workspace_id).await?;
        Ok(events.into_iter().filter(|e| self.ctx.drives_thread(event_thread(e).as_deref(), origin.as_ref())).collect())
    }

    /// sessions.ts:936. Rule Q5.
    pub async fn thread_history_of(
        &self,
        workspace_id: &str,
        asked: HistoryAsk,
        origin: Option<Caller>,
    ) -> Result<Vec<SessionEvent>, HostError> {
        let HistoryAsk { thread_id, before, limit } = asked;
        let Some(thread_id) = thread_id.filter(|_| before.is_none() && limit.is_none()) else {
            return Err(usage_refusal("whole reads one thread's every event.", "Name the thread with threadId, and no before or limit."));
        };
        self.ctx.ready().await?;
        if !self.ctx.tree_stands_on(workspace_id, origin.as_ref(), None)? {
            self.ctx.entry_of_with(workspace_id, origin.as_ref(), entry_asks(true, false)).await?;
        }
        if !self.ctx.drives_thread(Some(&thread_id), origin.as_ref()) {
            return Ok(Vec::new());
        }
        self.ctx.opened_words(workspace_id, &thread_id);
        self.ctx.open_thread(workspace_id, &thread_id).await
    }

    /// sessions.ts:945. Rule Q6.
    pub async fn page_of(&self, workspace_id: &str, window: PageWindow, origin: Option<Caller>) -> Result<HistoryPage, HostError> {
        self.ctx.ready().await?;
        if !self.ctx.tree_stands_on(workspace_id, origin.as_ref(), None)? {
            self.ctx.entry_of_with(workspace_id, origin.as_ref(), entry_asks(true, false)).await?;
        }
        if !self.ctx.drives_thread(Some(&window.thread_id), origin.as_ref()) {
            let pos = self.ctx.index_pos(workspace_id).unwrap_or(0);
            return Ok(HistoryPage { events: Vec::new(), pos: JsInt::from(pos as i64), total: JsInt::from(0), trimmed: None });
        }
        self.ctx.opened_words(workspace_id, &window.thread_id);
        self.ctx
            .read_page(
                workspace_id,
                &window.thread_id,
                ThreadRead {
                    before: window.before.map(|before| before as i64),
                    limit: window.limit.unwrap_or(HISTORY_PAGE_EVENTS) as usize,
                    bytes: HISTORY_PAGE_BYTES,
                    strict: false,
                    shape: None,
                },
            )
            .await
    }

    /// sessions.ts:957. Rule Q7.
    pub async fn head_of(&self, thread_id: &str, origin: Option<Caller>, after_read: bool) -> Result<ThreadHead, HostError> {
        self.ctx.ready().await?;
        let facts =
            if after_read { self.ctx.facts_after_read(thread_id, origin.as_ref()).await? } else { self.ctx.thread_facts(thread_id) };
        let not_reached = || not_found_refusal(&format!("no thread {}", thread_word(thread_id)));
        let Some(facts) = facts else {
            return Err(not_reached());
        };
        let facts_thread = facts.thread_id.clone().unwrap_or_else(|| thread_id.to_string());
        let at = RowAt { thread_id: Some(facts_thread.clone()), workspace_id: facts.workspace_id.clone() };
        if self.ctx.entry_of_row(&at, origin.as_ref(), None, true).await?.is_none() {
            return Err(not_reached());
        }
        let facts_len = serde_json::to_string(&facts).map(|text| text.len()).unwrap_or(0);
        let room = HEAD_BYTES.saturating_sub(facts_len as u64).saturating_sub(100);
        let page = self
            .ctx
            .read_page(
                &facts.workspace_id,
                &facts_thread,
                ThreadRead { before: None, limit: usize::MAX, bytes: room, strict: true, shape: Some(head_shape) },
            )
            .await?;
        Ok(ThreadHead { facts, events: HeadEvents(page.events), pos: page.pos, total: page.total, trimmed: page.trimmed })
    }

    /// sessions.ts:966. Rule Q8.
    pub async fn attachment_of(
        &self,
        workspace_id: &str,
        thread_id: &str,
        request_id: &str,
        index: u32,
        origin: Option<Caller>,
    ) -> Result<KeptAttachment, HostError> {
        self.ctx.ready().await?;
        if !self.ctx.tree_stands_on(workspace_id, origin.as_ref(), None)? {
            self.ctx.entry_of_with(workspace_id, origin.as_ref(), entry_asks(false, false)).await?;
        }
        if let Some(writes) = self.ctx.kept_writes(workspace_id) {
            writes.await;
        }
        let key = attachment_key(thread_id, Some(request_id), index as f64);
        let held: Option<KeptImage> = match key {
            Some(key) if self.ctx.drives_thread(Some(thread_id), origin.as_ref()) => {
                let kept =
                    self.ctx.store_get(ATTACHMENT_KEYS, workspace_id).await?.and_then(|doc| serde_json::from_value::<KeptImages>(doc).ok());
                kept.and_then(|kept| kept.threads.get(thread_id).and_then(|images| images.iter().find(|image| image.key == key).cloned()))
            }
            _ => None,
        };
        let bytes = match &held {
            Some(image) => self.ctx.store_get_blob(ATTACHMENTS, &image.key).await?,
            None => None,
        };
        let (Some(held), Some(bytes)) = (held, bytes) else {
            return Err(not_found_refusal(&format!("no image {} kept on that message", u64::from(index) + 1)));
        };
        Ok(KeptAttachment { media_type: held.media_type, bytes: base64_of(&bytes) })
    }

    /// sessions.ts:1175. Rule Q9.
    pub async fn search_for(&self, query: &str, origin: Option<Caller>) -> Result<SessionSearchResult, HostError> {
        self.ctx.ready().await?;
        let words = js_trim(query).to_lowercase();
        if words.is_empty() {
            return Ok(SessionSearchResult { hits: Vec::new(), unsearched: None });
        }
        let mut found: Vec<SearchedHit> = Vec::new();
        let mut unsearched: u32 = 0;
        for workspace_id in self.ctx.index_workspaces() {
            if !self.ctx.tree_stands_on(&workspace_id, origin.as_ref(), None)?
                && self.ctx.entry_of_with(&workspace_id, origin.as_ref(), entry_asks(false, true)).await.is_err()
            {
                continue;
            }
            let drives = |thread_id: &str| self.ctx.drives_thread(Some(thread_id), origin.as_ref());
            unsearched += self.ctx.search_in(&workspace_id, &words, &drives, &mut found);
        }
        found.sort_by(|a, b| b.last.0.partial_cmp(&a.last.0).unwrap_or(Ordering::Equal));
        let hits = found
            .into_iter()
            .map(|f| SessionSearchHit { workspace_id: f.hit.workspace_id, thread_id: f.hit.thread_id, snippet: f.hit.snippet })
            .collect();
        Ok(SessionSearchResult { hits, unsearched: (unsearched > 0).then(|| JsInt::from(i64::from(unsearched))) })
    }
}
