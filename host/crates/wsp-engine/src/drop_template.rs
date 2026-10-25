// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/src/drop-template.ts. Spec: spec/small.md.
use crate::errors::Result;
use crate::machine::MachineBackend;

/// drop-template.ts:8. `None` does nothing. A backend with templates (capabilities.templates and offers.templates, as
/// golden.ts:291 `templatesOf` reads them) deletes the template; a delete answered missing is no failure, any other
/// failure is. A backend without templates does nothing.
pub async fn drop_template(backend: &dyn MachineBackend, template_id: Option<&str>) -> Result<()> {
    todo!()
}
