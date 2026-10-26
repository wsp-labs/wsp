// SPDX-License-Identifier: AGPL-3.0-only
//! golden.ts:289-358: the template road of a backend, and the one promote the seal and the doctor share.
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;

use wsp_wire::wire::machine_link::TemplateRowStatus;
use wsp_words::words::image::{template_failed_line, template_status_line, template_waited_line, SNAPSHOT_GONE_REASON};

use super::types::plain;
use crate::errors::{is_missing, EngineError, RetryClock};
use crate::golden_seam::has_templates;
use crate::machine::{MachineBackend, TemplateRow};

/// golden.ts:303 `Templates`: a backend's template calls.
#[async_trait(?Send)]
pub trait Templates {
    async fn promote(&self, snapshot_id: &str, name: &str) -> Result<String, EngineError>;
    async fn get(&self, id: &str) -> Result<TemplateRow, EngineError>;
    async fn list(&self) -> Result<Vec<TemplateRow>, EngineError>;
    async fn delete(&self, id: &str) -> Result<(), EngineError>;
}

/// golden.ts:291 `templatesOf`: the template calls of a backend whose capabilities say it has them and which offers all
/// four, or nothing.
pub fn templates_of(backend: Arc<dyn MachineBackend>) -> Option<Rc<dyn Templates>> {
    if !has_templates(&*backend) {
        return None;
    }
    Some(Rc::new(BackendTemplates(backend)))
}

/// The four template calls of a backend, as `templatesOf` hands them.
struct BackendTemplates(Arc<dyn MachineBackend>);

#[async_trait(?Send)]
impl Templates for BackendTemplates {
    async fn promote(&self, snapshot_id: &str, name: &str) -> Result<String, EngineError> {
        self.0.promote_snapshot(snapshot_id, name).await
    }
    async fn get(&self, id: &str) -> Result<TemplateRow, EngineError> {
        self.0.get_template(id).await
    }
    async fn list(&self) -> Result<Vec<TemplateRow>, EngineError> {
        self.0.list_templates().await
    }
    async fn delete(&self, id: &str) -> Result<(), EngineError> {
        self.0.delete_template(id).await
    }
}

/// golden.ts:312.
pub const TEMPLATE_READY_MS: u64 = 5 * 60_000;
/// golden.ts:313.
pub const TEMPLATE_POLL_MS: u64 = 3_000;

/// A line callback: each read of the template, in the promoting stage's words.
pub type OnStatus = Arc<dyn Fn(&str)>;

/// golden.ts:315 `TemplateWait`.
#[derive(Clone, Default)]
pub struct TemplateWait {
    pub ready_ms: Option<u64>,
    pub poll_ms: Option<u64>,
    /// Each read of the template, in the promoting stage's words.
    pub on_status: Option<OnStatus>,
}

/// golden.ts:324. Reads the template until the provider says ready. A failed template or one still building at the
/// deadline is an error with the provider's words.
pub async fn await_template(
    templates: &dyn Templates,
    template_id: &str,
    wait: &TemplateWait,
    clock: &dyn RetryClock,
) -> Result<(), EngineError> {
    let ready_ms = wait.ready_ms.unwrap_or(TEMPLATE_READY_MS);
    let start = clock.now();
    loop {
        let row = templates.get(template_id).await?;
        let status = match row.status {
            TemplateRowStatus::Building => "building",
            TemplateRowStatus::Ready => "ready",
            TemplateRowStatus::Failed => "failed",
        };
        if let Some(on_status) = &wait.on_status {
            on_status(&template_status_line(status));
        }
        match row.status {
            TemplateRowStatus::Ready => return Ok(()),
            TemplateRowStatus::Failed => return Err(plain(template_failed_line(template_id, row.error.as_deref()))),
            TemplateRowStatus::Building => {}
        }
        if clock.now() - start >= ready_ms as i64 {
            return Err(plain(template_waited_line(template_id, status, ready_ms as f64)));
        }
        clock.sleep(wait.poll_ms.unwrap_or(TEMPLATE_POLL_MS)).await;
    }
}

/// golden.ts:343 `promoteVersion`'s answer.
#[derive(Clone, Debug, PartialEq)]
pub struct Promoted {
    pub template_id: String,
    pub sharing: Option<u32>,
}

/// golden.ts:343. A fresh template for a version, promoted from its snapshot and ready. A template whose wait fails or
/// runs out is deleted before the failure is returned.
pub async fn promote_version(
    templates: &dyn Templates,
    snapshot_id: &str,
    name: &str,
    wait: &TemplateWait,
    clock: &dyn RetryClock,
) -> Result<Promoted, EngineError> {
    // A template names no source snapshot, so none the provider already holds under the name is ever taken as this
    // version's; the listing is read only to say how many carry the name already.
    let sharing = templates.list().await.ok().map(|rows| rows.iter().filter(|t| t.name == name).count() as u32);
    let template_id = match templates.promote(snapshot_id, name).await {
        Ok(id) => id,
        Err(e) if is_missing(&e) => return Err(plain(SNAPSHOT_GONE_REASON)),
        Err(e) => return Err(e),
    };
    if let Err(e) = await_template(templates, &template_id, wait, clock).await {
        let _ = templates.delete(&template_id).await;
        return Err(e);
    }
    Ok(Promoted { template_id, sharing })
}
