// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/src/provision-files.ts at a4033dc34. Specs: spec/files_land.md, spec/files_ledger.md,
//! spec/files_outside.md.
//!
//! The person's own agent files on a computer they own, landed in the agents' homes there. A file already there with
//! other content is never written over; what wsp put there it may replace, which is what the list beside the job
//! (`place_provision_paths(home).landed`) records. Outside the home, a second list under `/opt/wsp/landed` records
//! what an install step made under `/usr/local` and `/opt`, so a leave takes exactly that.
//!
//! Every script here is generated text that runs on the far computer, and three of them have a twin in the daemon held
//! to a fixture byte for byte (`daemon/fixtures/contract/landed-files.sh`, `landed-outside.sh`): a script's text is
//! part of its contract, pinned against the TS's own output in tests/fixtures/oracle.
mod land;
mod ledger;
mod outside;

pub use land::*;
pub use ledger::*;
pub use outside::*;

/// provision-files.ts:39. Where each stage of the round says what it came to.
pub type FilesSay = std::sync::Arc<dyn Fn(&str)>;

/// provision-files.ts:43. One planned file of the person's: the recipe row it came from and where it lands under the
/// home there (home-relative; a directory covers every file under it).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProvisionLanding {
    pub id: String,
    pub label: String,
    pub dest: String,
    /// A file an agent writes for itself, or one the recipe marks volatile: lands where missing, stands where there.
    pub once: bool,
}

/// provision-files.ts:113. The folder under the home the plan names an agent's files under (home-relative), and the
/// store elsewhere they land in instead, in catalog order.
pub type StoreRoots = Vec<(String, String)>;

/// provision-files.ts:475. Absolute paths under the home there that hold what wsp landed, each `installed` (this run
/// put it there) or `present` (the same bytes were there), in the order the landing reported them.
pub type OwnedPaths = indexmap::IndexMap<String, OwnedHow>;

/// provision-files.ts:475's two values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OwnedHow {
    Installed,
    Present,
}
