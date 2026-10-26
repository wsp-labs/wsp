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

pub use public::AssembledRuntime;

/// `createRuntime(opts)` (runtime.ts:425-489, spec C1): the core, every area in the order spec C1 lists, the pipe
/// area with the brokered rule, then the public runtime. Panics where the TS throws on a bad option: no backend or
/// store, or a daemon token seed of the wrong shape.
pub fn create_runtime(opts: RuntimeOptions) -> Rc<AssembledRuntime> {
    Rc::new(AssembledRuntime::over(ctx::Ctx::build(opts)))
}
