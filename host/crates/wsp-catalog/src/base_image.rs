// SPDX-License-Identifier: AGPL-3.0-only
//! What Debian bookworm answers before wsp installs anything. Port of packages/catalog/src/base-image.ts at a3b870a6e;
//! the spec is spec/roads.md.
use std::sync::LazyLock;

use indexmap::IndexSet;

/// Every command of base-image.ts:8, in its order.
pub static BASE_IMAGE_COMMANDS: LazyLock<IndexSet<&'static str>> = LazyLock::new(|| todo!());
