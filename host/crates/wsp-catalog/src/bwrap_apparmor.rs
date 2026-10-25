// SPDX-License-Identifier: AGPL-3.0-only
//! The setup row that lets bwrap make its namespaces for users other than root where Ubuntu's AppArmor stops them.
//! Port of packages/catalog/src/bwrap-apparmor.ts at a3b870a6e; the spec is spec/catalog.md.
use std::sync::LazyLock;

use crate::catalog::SetupNeed;

/// bwrap-apparmor.ts:33: id `bwrap-apparmor`, after `bubblewrap`, its three scripts built as the TS joins them.
pub static BWRAP_APPARMOR: LazyLock<SetupNeed> = LazyLock::new(|| todo!());
