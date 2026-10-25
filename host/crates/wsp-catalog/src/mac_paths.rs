// SPDX-License-Identifier: AGPL-3.0-only
//! The absolute paths a Mac writes that a Linux image has to answer for. Port of packages/catalog/src/mac-paths.ts at
//! a3b870a6e; the spec is spec/roads.md.
pub const MAC_BREW: &str = "/opt/homebrew";
pub const MAC_ONLY: [&str; 6] = ["/Applications/", "/System/", "/Library/", "/Volumes/", "/private/", "/opt/homebrew/Caskroom/"];
pub const MAC_BIN_DIRS: [&str; 2] = ["/opt/homebrew/bin/", "/opt/homebrew/sbin/"];
