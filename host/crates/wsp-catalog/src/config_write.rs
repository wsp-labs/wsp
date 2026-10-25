// SPDX-License-Identifier: AGPL-3.0-only
//! The one write of an agent's config file on any computer, as a line for that computer's own sh. Port of
//! packages/catalog/src/config-write.ts at a3b870a6e; the spec is spec/mcp_launch.md.
use crate::pending::HostError;

/// The home where the store sits under it, else the store (config-write.ts:341).
pub fn store_base(store: &str, home: &str) -> String {
    todo!()
}

pub const CONFIG_LINK_EXIT: i32 = 4;
pub const CONFIG_CHANGED_EXIT: i32 = 5;
pub const CONFIG_LINKS_EXIT: i32 = 6;

/// One write (config-write.ts:358).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigWrite {
    pub file: String,
    pub base: String,
    /// `config_sum` of the file as read; `None` where there was no file.
    pub sum: Option<String>,
    pub bytes: u64,
    /// A landing beside the file the bytes wait in; `None`, stdin.
    pub from: Option<String>,
}

/// `<file's folder>/.wsp-config-tmp.<id>`; errors "a landing's id is twelve hex digits" (config-write.ts:373).
pub fn config_landing(file: &str, id: &str) -> Result<String, HostError> {
    todo!()
}

/// config-write.ts:380.
pub fn under_base(inside: &str, outside: &str) -> String {
    todo!()
}

/// config-write.ts:383.
pub fn inside_base(said: &str) -> String {
    todo!()
}

/// The sh line (config-write.ts:389).
pub fn config_write_line(w: &ConfigWrite) -> String {
    todo!()
}

/// The sentence for a refused write, the path named on stdout up to its NUL first, else `file`; nothing for any other
/// exit (config-write.ts:425).
pub fn config_refusal(exit_code: i32, stdout: &str, file: &str, shown: &dyn Fn(&str) -> String) -> Option<String> {
    todo!()
}

/// What POSIX cksum prints: `<crc> <length>` (config-write.ts:441).
pub fn config_sum(bytes: &[u8]) -> String {
    todo!()
}
