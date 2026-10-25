// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/src/machine-facts.ts. Spec: spec/machine_facts.md.
//!
//! What a machine that already existed says about itself, read through `key value` lines that one shell's printf
//! prints. The `*_READ` constants are the TS strings byte for byte (tests/fixtures/oracle/machine_facts.json).
use indexmap::IndexMap;

use crate::errors::Result;
use crate::machine::ExecResult;

/// machine-facts.ts:14. The last value of each key, keys in first-seen order. A key whose line printed nothing is
/// present with `""`.
pub fn read_values(stdout: &str) -> IndexMap<String, String> {
    todo!()
}

/// machine-facts.ts:19. Every value of each key, in order; keys in first-seen order.
pub fn read_lists(stdout: &str) -> IndexMap<String, Vec<String>> {
    todo!()
}

/// machine-facts.ts:34.
pub const OS_READ: [&str; 3] = [
    r#"printf "pretty %s\n" "$(sed -n 's/^PRETTY_NAME=//p' /etc/os-release 2>/dev/null | head -1 | tr -d '"')""#,
    r#"printf "mac %s\n" "$(sw_vers -productVersion 2>/dev/null)""#,
    r#"printf "kernel %s %s\n" "$(uname -s)" "$(uname -r)""#,
];

/// machine-facts.ts:42.
pub const UPTIME_READ: [&str; 2] = [
    r#"printf "uptime %s\n" "$(cut -d' ' -f1 /proc/uptime 2>/dev/null)""#,
    r#"printf "boot %s\n" "$(sysctl -n kern.boottime 2>/dev/null)""#,
];

/// machine-facts.ts:49.
pub const HOME_READ: &str = r#"printf "home %s\n" "$HOME""#;

/// machine-facts.ts:53.
pub const SYSTEM_READ: &str = r#"printf "system %s\n" "$(uname -s)""#;

/// machine-facts.ts:58.
pub const ARCH_READ: &str = r#"printf "arch %s\n" "$(uname -m)""#;

/// machine-facts.ts:61.
pub const MEM_READ: &str = r#"printf "memkb %s\n" "$(awk '/MemTotal/{print $2}' /proc/meminfo 2>/dev/null || echo $(( $(sysctl -n hw.memsize 2>/dev/null || echo 0) / 1024 )))""#;

/// machine-facts.ts:69.
pub const SHELL_READ: &str = r#"shell=$(getent passwd "$(id -un)" 2>/dev/null | cut -d: -f7); shell=${shell##*/}; shell=${shell:-${SHELL##*/}}; shell=${shell:-bash}"#;

/// machine-facts.ts:73.
pub fn arch_of(values: &IndexMap<String, String>) -> Option<String> {
    todo!()
}

/// machine-facts.ts:79.
pub fn system_of(values: &IndexMap<String, String>) -> Option<String> {
    todo!()
}

/// machine-facts.ts:85. Whole MB, rounded half up as `Math.round`; `None` for a figure that is not a finite number
/// above 0 under JavaScript's `Number()`.
pub fn mem_mb_of(values: &IndexMap<String, String>) -> Option<u64> {
    todo!()
}

/// machine-facts.ts:93.
pub fn os_name_of(values: &IndexMap<String, String>) -> Option<String> {
    todo!()
}

/// machine-facts.ts:106. `now` is ms since the epoch on this computer's clock. Negative where /proc says so: the TS
/// rounds whatever finite figure it reads.
pub fn uptime_ms_of(values: &IndexMap<String, String>, now: i64) -> Option<i64> {
    todo!()
}

/// machine-facts.ts:115. `None` where the exec failed, exited non-zero, or the answer names no system.
pub async fn read_os_name<F, Fut>(exec: F) -> Option<String>
where
    F: FnOnce(String) -> Fut,
    Fut: std::future::Future<Output = Result<ExecResult>>,
{
    todo!()
}
