// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/src/provision.ts at a4033dc34. Specs: spec/provision_plan.md (provision.ts:21-250) and
//! spec/provision_step.md (provision.ts:252-551).
//!
//! The recipe on a computer somebody owns: the floor first, then every ticked row the computer does not already
//! satisfy, then the machine context. Nothing here knows how the computer is reached: it drives a Machine.

mod plan;
mod step;

pub use plan::*;
pub use step::*;
