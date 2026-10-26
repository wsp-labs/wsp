// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/src/snapshot-names.ts. Spec: spec/sweeps.md.
//!
//! The mark wsp writes into the name of every snapshot and template it makes, and the one parser that reads it back.
use crate::errors::{EngineError, Result};

/// snapshot-names.ts:9. Every name below opens with this word.
pub const MARK: &str = "wsp";

/// snapshot-names.ts:23. `wsp-<host>-<golden>-v<version>`. Refuses a host that is not `^[a-z0-9]+$` (empty too) with
/// `a snapshot name needs a host of lowercase letters and digits, got <JSON.stringify(host)>` as an `Other`.
pub fn golden_name(host: &str, golden: &str, version: u64) -> Result<String> {
    marked(host, &format!("{golden}-v{version}"))
}

/// snapshot-names.ts:26. `wsp-<host>-project-<project>-<stamp>`, the same host refusal.
pub fn project_snapshot_name(host: &str, project: &str, stamp: &str) -> Result<String> {
    marked(host, &format!("project-{project}-{stamp}"))
}

/// snapshot-names.ts:29. The host that made this snapshot or template: the `[a-z0-9]+` after `wsp-` of a name matching
/// `^wsp-([a-z0-9]+)-(.+)$`; `None` for no name or a name without the mark.
pub fn name_owner(name: Option<&str>) -> Option<String> {
    // `^wsp-([a-z0-9]+)-(.+)$`: the host is the whole run of lowercase letters and digits, then a hyphen and at least
    // one character that is no line terminator.
    let rest = name?.strip_prefix("wsp-")?;
    let host_len = rest.bytes().take_while(|b| b.is_ascii_lowercase() || b.is_ascii_digit()).count();
    let after = rest[host_len..].strip_prefix('-')?;
    let plain = !after.is_empty() && !after.contains(['\n', '\r', '\u{2028}', '\u{2029}']);
    (host_len > 0 && plain).then(|| rest[..host_len].to_string())
}

/// snapshot-names.ts:12. A host that is not `^[a-z0-9]+$` is refused: the name is the only place ownership rides.
fn marked(host: &str, rest: &str) -> Result<String> {
    if host.is_empty() || !host.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit()) {
        return Err(EngineError::Other {
            message: format!(
                "a snapshot name needs a host of lowercase letters and digits, got {}",
                serde_json::to_string(host).unwrap_or_default()
            ),
            code: None,
        });
    }
    Ok(format!("{MARK}-{host}-{rest}"))
}
