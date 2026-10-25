// SPDX-License-Identifier: AGPL-3.0-only
//! The sentences of forgetting a computer that holds work, and of a place that is no workspace. Port of the functions of packages/protocol/src/wire/machine-link.ts that wsp-wire left to this crate, at
//! a3b870a6e; that file's shapes are wsp-wire's. See spec/wire_machine_link.md.

/// What a forget of a computer whose link is down does, in its order, the one sentence the command line asks with
/// and the app's dialog says before its button: the leave tried over the ssh login it was added on where it has one,
/// then the records leaving this wsp, and nothing else done on it.
// machine-link.ts:347
// export function placeForgetLine(name: string, held: PlaceHeld, ssh?: string): string {
//   const goes = [heldPart(held.forks, "fork"), heldPart(held.projects, "project")].filter(p => p !== undefined).map(p => `its ${p}`);
//   const leaves = `${name} leaves this wsp${goes.length === 0 ? "" : ` with ${goes.join(" and ")}`}`;
//   const tried = ssh === undefined ? `${leaves}, and nothing is done on ${name}` : `wsp tries to take itself off ${name} over ${ssh} first. Then ${leaves}, and nothing more is done on ${name}`;
//   return `${tried}: whatever of wsp's stays there comes off with ${PLACE_LEAVE_LINE} run on that computer.`;
// }
/// A row of what a computer holds: a fork or a project by name, with its thread count.
#[derive(Clone, Debug, PartialEq)]
pub struct HeldRow {
    pub name: String,
    pub threads: f64,
}

/// The TS private PlaceHeld: the forks and projects a computer holds.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlaceHeld {
    pub forks: Vec<HeldRow>,
    pub projects: Vec<HeldRow>,
}

pub fn place_forget_line(name: &str, held: &PlaceHeld, ssh: Option<&str>) -> String {
    todo!()
}

/// What goes with a computer, the one sentence its remove asks with and answers with: its forks deleted, its projects
/// out of this wsp, each with its threads. Nothing where it holds neither.
// machine-link.ts:356
// export function placeHoldsLine(held: PlaceHeld): string | undefined {
//   const forks = heldPart(held.forks, "fork");
//   const projects = heldPart(held.projects, "project");
//   if (forks === undefined && projects === undefined) return undefined;
//   return [forks === undefined ? undefined : `its ${forks} deleted`, projects === undefined ? undefined : `its ${projects} out of this wsp`].filter(p => p !== undefined).join(", and ");
// }
pub fn place_holds_line(held: &PlaceHeld) -> Option<String> {
    todo!()
}

/// What a verb aimed at the bare computer a place is, rather than at a workspace forked on it, is refused with. A
/// place holds its facts and its forks; the forks are the workspaces, so the refusal names the computer and the one
/// road to a workspace there.
// machine-link.ts:371
// export const placeNotAWorkspaceLine = (place: string): string => `${place} is a computer you joined, not a workspace; its forks are the workspaces`;
pub fn place_not_a_workspace_line(place: &str) -> String {
    todo!()
}

// machine-link.ts:372
// export const placeNotAWorkspaceFix = (place: string): string => `Name a machine on it: wsp threads lists what runs on ${place}.`;
pub fn place_not_a_workspace_fix(place: &str) -> String {
    todo!()
}

/// What a person asking for a second workspace on the computer the app itself runs on is told. Its local mode is
/// one workspace, the one it already has; every other workspace is forked at a place.
// machine-link.ts:401
// export const localRunsOneLine = (workspace: string): string => `${THIS_COMPUTER} is already a workspace, ${workspace}, the only one it can be`;
pub fn local_runs_one_line(workspace: &str) -> String {
    todo!()
}
