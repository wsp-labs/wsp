// SPDX-License-Identifier: AGPL-3.0-only
//! Port of the two readings of packages/protocol/src/thread-cgroup.ts at a3b870a6e that Activity calls; the scripts
//! that file builds are wsp-words'. See spec/activity.md.

use wsp_frames::CgroupMemory;

// thread-cgroup.ts:20 at a3b870a6e
// export function cgroupMemory(cgroups: readonly CgroupMemory[] | undefined, cgroup: string | undefined): number | undefined {
//   return cgroup === undefined ? undefined : cgroups?.find(c => c.cgroup === cgroup)?.memory;
// }
/// The memory the daemon read for the first entry naming this cgroup; None without a cgroup, without a list, or
/// with no entry for it.
pub fn cgroup_memory(cgroups: Option<&[CgroupMemory]>, cgroup: Option<&str>) -> Option<u64> {
    let cgroup = cgroup?;
    cgroups?.iter().find(|c| c.cgroup == cgroup).map(|c| c.memory)
}

// thread-cgroup.ts:25 at a3b870a6e
// export function inCgroup(at: string | undefined, cgroup: string): boolean {
//   return at !== undefined && (at === cgroup || at.startsWith(`${cgroup}/`));
// }
/// Whether a row's cgroup is this one or one under it (`<cgroup>/...`); never for a row with none.
pub fn in_cgroup(at: Option<&str>, cgroup: &str) -> bool {
    at.is_some_and(|at| at.strip_prefix(cgroup).is_some_and(|rest| rest.is_empty() || rest.starts_with('/')))
}
