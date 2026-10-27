// SPDX-License-Identifier: AGPL-3.0-only
//! Ported from packages/runtime/src/runtime.ts at 65f6b89c7: the hub. `create_runtime` builds the core, then each
//! area in its fixed order, then the public runtime. Specs: spec/runtime_core.md, spec/runtime_of.md.
mod api;
pub mod core;
pub mod ctx;
pub mod join;
pub mod probe;
pub mod public;

use std::rc::Rc;

use wsp_core::types::wiring::RuntimeOptions;
use wsp_words::exit::HostError;

pub use public::AssembledRuntime;

/// `createRuntime(opts)` (runtime.ts:425-489, spec C1): the core, every area in the order spec C1 lists, the pipe
/// area with the brokered rule, then the public runtime. Where the TS throws on a bad option (no store, or a daemon
/// token seed that is not lowercase hex) the error is the TS's, answered and not a panic.
pub fn create_runtime(opts: RuntimeOptions) -> Result<Rc<AssembledRuntime>, HostError> {
    Ok(Rc::new(AssembledRuntime::over(ctx::Ctx::build(opts)?)))
}
