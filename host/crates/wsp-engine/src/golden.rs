// SPDX-License-Identifier: AGPL-3.0-only
//! packages/engine/src/golden.ts at 65f6b89c7, beyond what `golden_seam` (the tools, the base floor, the plans, the MCP
//! round) and `gone` (the read-backs) already hold: the import stages on a machine (`applyGoldenImport`), the delta of
//! an update, the two halves of a golden (`prepareBuilder`, `sealGolden`), the update's fork (`upgradeBuilder`), the
//! scripted pipeline (`buildGolden`), the template road (`promoteVersion`) and the manifest's own rules.
//!
//! The wizard's and the pipeline's one road, in the TS's own order. Every function answers what the TS answers and
//! fails with what it throws; a time it reads is the clock handed in, so a test moves it by hand.
mod delta;
mod import;
mod plugins;
mod prepare;
mod seal;
mod smoke;
mod templates;
mod types;

pub use delta::*;
pub use import::*;
pub use plugins::*;
pub use prepare::*;
pub use seal::*;
pub use smoke::*;
pub use templates::*;
pub use types::*;
