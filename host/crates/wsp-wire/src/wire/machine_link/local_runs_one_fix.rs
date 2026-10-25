// SPDX-License-Identifier: AGPL-3.0-only
use super::*;

// machine-link.ts:402
// export const localRunsOneFix = (workspace: string): string => `Use ${workspace}, or name a computer that forks: wsp computers.`;
pub fn local_runs_one_fix(workspace: &str) -> String {
    format!("Use {workspace}, or name a computer that forks: wsp computers.")
}

// machine-link.ts:404
// /** What a computer's kernel must have before wsp runs workspaces on it, asked in this order so the reason a person
//  * reads is the first thing missing rather than the last. `read` answers a file's text or nothing when it is not
//  * there; `modprobe` runs the kernel's module loader; `euid` is the effective user the check runs as. Nothing here
//  * touches a disk: the join typed at a box reads its own files through this rule, and its steps and words are the
//  * daemon doctor's (`daemon/crates/wsp-runtime/src/doctor.rs`), so what the join says and what a create does
//  * cannot part ways. */
// export function workspacesBlockedBy(at: { platform: string; read: (path: string) => string | undefined; modprobe: ModprobeRun; euid?: number }): string | undefined {
//   if (at.platform !== "linux") return "wsp runs workspaces on a Linux computer";
//   const controllers = at.read(CGROUP_CONTROLLERS_PATH);
//   if (controllers === undefined) {
//     return "this computer mounts cgroup v1 at /sys/fs/cgroup, and wsp runs workspaces on cgroup v2 alone: boot it with systemd.unified_cgroup_hierarchy=1";
//   }
//   const has = new Set(controllers.split(/\s+/));
//   for (const wanted of ["memory", "cpu"]) if (!has.has(wanted)) return `this computer's cgroup root offers no ${wanted} controller, which wsp needs to run workspaces here`;
//   const overlay = overlayMissing(at.read, at.modprobe);
//   if (overlay !== undefined) return overlay;
//   if (at.euid !== 0) return "wsp runs workspaces on this computer as root, and this daemon is not root";
//   return undefined;
// }
pub fn workspaces_blocked_by(at: &KernelAt<'_>) -> Option<String> {
    if at.platform != "linux" {
        return Some("wsp runs workspaces on a Linux computer".into());
    }
    let Some(controllers) = (at.read)(CGROUP_CONTROLLERS_PATH) else {
        return Some("this computer mounts cgroup v1 at /sys/fs/cgroup, and wsp runs workspaces on cgroup v2 alone: boot it with systemd.unified_cgroup_hierarchy=1".into());
    };
    let has: Vec<&str> = controllers.split(crate::types::check::js_space).collect();
    for wanted in ["memory", "cpu"] {
        if !has.contains(&wanted) {
            return Some(format!("this computer's cgroup root offers no {wanted} controller, which wsp needs to run workspaces here"));
        }
    }
    if let Some(overlay) = overlay_missing(at) {
        return Some(overlay);
    }
    if at.euid != Some(0) {
        return Some("wsp runs workspaces on this computer as root, and this daemon is not root".into());
    }
    None
}

fn overlay_missing(at: &KernelAt<'_>) -> Option<String> {
    let listed =
        || (at.read)(PROC_FILESYSTEMS_PATH).is_some_and(|text| text.split(crate::types::check::js_space).any(|fs| fs == "overlay"));
    if listed() {
        return None;
    }
    match (at.modprobe)(&["overlay"]) {
        ModprobeRun::Failed { error } => Some(overlay_wont_load(&format!("{MODPROBE_PATH} could not run: {error}"))),
        ModprobeRun::Ran { ok, stderr } => {
            if ok && listed() {
                return None;
            }
            match (at.modprobe)(&["-n", "overlay"]) {
                ModprobeRun::Ran { ok: true, .. } => {}
                _ => return Some("this computer's kernel has no overlay filesystem and no overlay module to load, which a workspace here reads this computer's own directories through".into()),
            }
            let last = crate::types::check::js_trim(
                stderr.trim_end_matches(crate::types::check::js_space).split('\n').next_back().unwrap_or_default(),
            );
            let said = if last.is_empty() { "modprobe loaded it and the kernel still lists no overlay" } else { last };
            Some(overlay_wont_load(said))
        }
    }
}

fn overlay_wont_load(said: &str) -> String {
    format!("this computer's kernel has an overlay module that would not load ({said}), and a workspace here reads this computer's own directories through an overlay")
}

// machine-link.ts:453
// /** What a join of a computer whose kernel cannot boot the image is refused with, in the one sentence the daemon's
//  * own doctor named the reason in. A computer that cannot boot your image is not a place, so the join stops here
//  * and nothing is written on it. */
// export const placeCannotBootLine = (place: string, reason?: string): string =>
//   reason === undefined ? `${place} cannot run wsp workspaces, so it cannot be a place` : `${place} cannot run wsp workspaces: ${reason.replace(/this computer('s)?/g, (_, owns) => (owns === undefined ? "it" : "its"))}`;
pub fn place_cannot_boot_line(place: &str, reason: Option<&str>) -> String {
    match reason {
        None => format!("{place} cannot run wsp workspaces, so it cannot be a place"),
        Some(reason) => {
            format!("{place} cannot run wsp workspaces: {}", reason.replace("this computer's", "its").replace("this computer", "it"))
        }
    }
}

// machine-link.ts:459
// /** The same sentence off a computer's own report, or nothing while it runs workspaces: one reading for the join's
//  * refusal, the row, and every act that runs inside a copy there. */
// export const placeBlocked = (place: string, report: Pick<PlaceReport, "runsWorkspaces" | "workspacesBlocked">): string | undefined =>
//   report.runsWorkspaces ? undefined : placeCannotBootLine(place, report.workspacesBlocked);
pub fn place_blocked(place: &str, runs_workspaces: bool, workspaces_blocked: Option<&str>) -> Option<String> {
    if runs_workspaces {
        return None;
    }
    Some(place_cannot_boot_line(place, workspaces_blocked))
}

// machine-link.ts:464
// /** What a computer's row says while the threads on that computer itself get no wsp tools: its daemon could not open
//  * the door they reach the tools through, in its own words, and the restart that opens it again, of the unit the
//  * daemon says it runs under. An update does not do it: one of a daemon already current sends no binary and restarts
//  * nothing. A daemon in no unit the join writes names none, and is added again to get one. Nothing from a daemon older
//  * than that door, which the row's behind word already answers, or from one whose door stands. */
// export const placeNoToolsLine = (place: string, report: Pick<PlaceReport, "wspDoor" | "wspDoorBlocked" | "daemonUnit">): string | undefined =>
//   report.wspDoor !== false
//     ? undefined
//     : `threads on ${place} get no wsp tools: ${report.wspDoorBlocked ?? "its daemon opened no door for them"}; ${report.daemonUnit === undefined ? `remove ${place} and add it again` : `restart wsp's daemon there with systemctl restart ${report.daemonUnit}`}`;
pub fn place_no_tools_line(
    place: &str,
    wsp_door: Option<bool>,
    wsp_door_blocked: Option<&str>,
    daemon_unit: Option<&str>,
) -> Option<String> {
    if wsp_door != Some(false) {
        return None;
    }
    let why = wsp_door_blocked.unwrap_or("its daemon opened no door for them");
    let fix = match daemon_unit {
        None => format!("remove {place} and add it again"),
        Some(unit) => format!("restart wsp's daemon there with systemctl restart {unit}"),
    };
    Some(format!("threads on {place} get no wsp tools: {why}; {fix}"))
}

// machine-link.ts:482
// /** What a word that names no place this host holds is refused with, naming the ones it does. */
// export const noSuchPlaceRefusal = (word: string, held: readonly string[]): string => `no place named ${word}; you have ${held.join(", ")}`;
pub fn no_such_place_refusal(word: &str, held: &[String]) -> String {
    format!("no place named {word}; you have {}", held.join(", "))
}

// machine-link.ts:485
// /** Whether a row is a computer somebody joined to this wsp: a computer, and not the one the host runs on, whose
//  * files and threads are that host's own. The one reading, so the road that runs on the link, the road at the
//  * terminal and the list of places a fork could stand on cannot disagree about which rows are those computers. */
// export const isJoinedComputer = (place: { id: string; kind: string }): boolean => place.kind === "computer" && place.id !== HERE_PLACE_ID;
pub fn is_joined_computer(id: &str, kind: &str) -> bool {
    kind == "computer" && id != "here"
}

// machine-link.ts:490
// /** What a build of the image at a place that cannot take one is refused with: a copy needs a builder forked there
//  * and that builder's disk copied, and a computer somebody joined does neither. */
// export const placeBuildsNoImageLine = (place: string): string =>
//   `${place} takes no copy of your image: a copy is built by forking a machine there and copying its disk, and ${place} does neither`;
pub fn place_builds_no_image_line(place: &str) -> String {
    format!(
        "{place} takes no copy of your image: a copy is built by forking a machine there and copying its disk, and {place} does neither"
    )
}

// machine-link.ts:499
// /** What a vault archive is refused with: where the reading stopped, why, and what that refusal did, which is not
//  * the same on the two roads. The archive is refused whole, since a builder that wrote one member nobody asked for
//  * wrote every other member too. */
// export const vaultMemberRefusal = (road: VaultRoad, member: string, why: string): string =>
//   `the image's sign-in archive is refused at ${member}: ${why}; ${road === "seal" ? "the seal is refused and no version is recorded" : "nothing of it was imported"}`;
pub fn vault_member_refusal(road: VaultRoad, member: &str, why: &str) -> String {
    let refused = match road {
        VaultRoad::Seal => "the seal is refused and no version is recorded",
        VaultRoad::Import => "nothing of it was imported",
    };
    format!("the image's sign-in archive is refused at {member}: {why}; {refused}")
}

// machine-link.ts:505
// /** What a copy is refused with for a record whose vault kept no path list: sealed before the record held which
//  * paths its sign-ins live at, so no other place can tell a member the seal asked for from one it did not. */
// export const vaultUnlistedRefusal = (name: string, version: number): string =>
//   `${name} v${version} was sealed before its record kept which paths its sign-ins live at, so no other place can check its archive against them; cut the next version`;
pub fn vault_unlisted_refusal(name: &str, version: i64) -> String {
    format!("{name} v{version} was sealed before its record kept which paths its sign-ins live at, so no other place can check its archive against them; cut the next version")
}

// machine-link.ts:693
// /** The snapshot a proc.changes frame leaves, rows by pid as the daemon lists them, the containers it carried or else
//  * those held, and the cgroups' memory it carried, never an older tick's; undefined where the frame does not
//  * follow the snapshot held, which a client answers by watching again for a whole one. */
// export function applyProcChanges(snapshot: ProcSnapshot, changes: ProcChanges): ProcSnapshot | undefined {
//   if (changes.base !== snapshot.seq) return undefined;
//   const rows = new Map(snapshot.procs.map(p => [p.pid, p]));
//   for (const pid of changes.gone) rows.delete(pid);
//   for (const p of changes.procs) rows.set(p.pid, p);
//   return { type: "proc.snapshot", at: changes.at, daemon: changes.daemon, total: changes.total, procs: [...rows.values()].sort((a, b) => a.pid - b.pid), seq: changes.seq, ...(changes.cost === undefined ? {} : { cost: changes.cost }), ...((changes.containers ?? snapshot.containers) === undefined ? {} : { containers: changes.containers ?? snapshot.containers }), ...(changes.cgroups === undefined ? {} : { cgroups: changes.cgroups }) };
// }
pub fn apply_proc_changes(snapshot: &ProcSnapshot, changes: &ProcChanges) -> Option<ProcSnapshot> {
    if snapshot.seq != Some(changes.base) {
        return None;
    }
    let mut rows: std::collections::BTreeMap<i64, ProcEntry> = snapshot.procs.iter().map(|p| (p.pid.get(), p.clone())).collect();
    for pid in &changes.gone {
        rows.remove(&pid.get());
    }
    for p in &changes.procs {
        rows.insert(p.pid.get(), p.clone());
    }
    Some(ProcSnapshot {
        at: changes.at,
        daemon: changes.daemon,
        total: changes.total,
        procs: rows.into_values().collect(),
        seq: Some(changes.seq),
        cost: changes.cost.clone(),
        containers: changes.containers.clone().or_else(|| snapshot.containers.clone()),
        cgroups: changes.cgroups.clone(),
    })
}

/// Reads one machine-link request as zod's discriminated union on `op` reads it, so a refusal's first issue is zod's: a value
/// that is not an object is one invalid_type issue at the root (expected "object"); an absent or unknown `op` is
/// one invalid_union_discriminator issue on ["op"] whose detail `options` lists every op of MachineLinkRequest in order
/// and whose message is "Invalid discriminator value. Expected 'a' | 'b' | ..."; otherwise the op's own fields in the
/// schema's order, then the variant's checks. Unknown keys are dropped.
pub fn parse_machine_link_request(frame: &Value) -> Result<MachineLinkRequest, SchemaError> {
    parse::<MachineLinkRequest>(frame)
}

/// Every op MachineLinkRequest holds, in the TS union's order.
pub const MACHINE_LINK_OPS: &[&str] = &[
    "machine.backend",
    "machine.capacity",
    "machine.checkKey",
    "machine.create",
    "machine.get",
    "machine.list",
    "machine.exec",
    "machine.pause",
    "machine.resume",
    "machine.kill",
    "machine.state",
    "machine.describe",
    "machine.facts",
    "machine.metrics",
    "machine.daemonAnswers",
    "machine.previewUrl",
    "machine.downloadUrl",
    "machine.uploadUrl",
    "machine.putBytes",
];

// A word list zod reads with z.enum: serde refuses any other word, so nothing is left to check.
impl Check for DaemonSupervisor {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["systemd", "entrypoint"]))
    }
}

impl Check for MachineSpecOnIdle {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["pause", "kill"]))
    }
}

impl Check for TemplateRowStatus {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["building", "ready", "failed"]))
    }
}

impl Check for MachineErrorKind {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(
            path,
            crate::types::check::Zod::Options(&[
                "concurrency",
                "plan",
                "missing",
                "conflict",
                "snapshotUnavailable",
                "transient",
                "auth",
                "unknown",
                "absent",
            ]),
        )
    }
}

impl Check for DaemonErrorCode {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(
            path,
            crate::types::check::Zod::Options(&[
                "unsupported",
                "outside-root",
                "not-found",
                "not-a-directory",
                "not-a-file",
                "not-a-git-repo",
                "bad-request",
                "forbidden",
                "no-host-cli",
                "no-git-credential",
                "rate-limited",
                "on-default-branch",
            ]),
        )
    }
}

/// machine.backend answers the backend's facts and machine.capacity its room: the same shapes, by the same name
/// the TS gives them (machine-link.ts:245-246).
pub type MachineBackendReply = BackendFacts;

pub type MachineCapacityReply = crate::wire::capabilities::PlaceCapacity;
