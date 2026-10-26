// SPDX-License-Identifier: AGPL-3.0-only
//! golden.ts:289-358: the template road of a backend, and the one promote the seal and the doctor share.
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;

use crate::errors::{EngineError, RetryClock};
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
pub fn templates_of(_backend: Arc<dyn MachineBackend>) -> Option<Rc<dyn Templates>> {
    todo!("golden.ts:291")
}

/// golden.ts:312.
pub const TEMPLATE_READY_MS: u64 = 5 * 60_000;
/// golden.ts:313.
pub const TEMPLATE_POLL_MS: u64 = 3_000;

/// golden.ts:315 `TemplateWait`.
#[derive(Clone, Default)]
pub struct TemplateWait {
    pub ready_ms: Option<u64>,
    pub poll_ms: Option<u64>,
    /// Each read of the template, in the promoting stage's words.
    pub on_status: Option<Arc<dyn Fn(&str)>>,
}

/// golden.ts:324. Reads the template until the provider says ready. A failed template or one still building at the
/// deadline is an error with the provider's words.
pub async fn await_template(
    _templates: &dyn Templates,
    _template_id: &str,
    _wait: &TemplateWait,
    _clock: &dyn RetryClock,
) -> Result<(), EngineError> {
    todo!("golden.ts:324")
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
    _templates: &dyn Templates,
    _snapshot_id: &str,
    _name: &str,
    _wait: &TemplateWait,
    _clock: &dyn RetryClock,
) -> Result<Promoted, EngineError> {
    todo!("golden.ts:343")
}
