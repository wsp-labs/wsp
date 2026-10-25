// SPDX-License-Identifier: AGPL-3.0-only
//! `workspaces-help.ts:111-296` and `:158-184`, `:869-881` at 65f6b89c7: listings, picking a thread or a workspace,
//! the thread rows, waking, napping, resizing, rebuilding, the image, ssh. Spec: `spec/ws-reads.md`.
//! The first seven items are the ones every other unit calls (signatures fixed by the lead).

use serde::{Deserialize, Serialize};
use wsp_wire::views::golden_image::{SealedImage, SealedImageBuilt};
use wsp_wire::views::session::ThreadView;
use wsp_wire::views::snapshot_lineage::SealedImageView;
use wsp_wire::views::workspace_more::{WorkspaceListing, WorkspaceOut, WorkspaceView};
use wsp_wire::wire::capabilities::WorkspaceSize;
use wsp_words::exit::HostError;

use crate::verbs::client::{HostClient, IoBytes, Out};

/// `workspaces(client)` (workspaces-help.ts:111): `workspaces.list`, the `workspaces` array as the host wrote it
/// (not parsed: the TS hands the reply's objects on as they came).
pub async fn workspaces(client: &dyn HostClient) -> Result<Vec<WorkspaceOut>, HostError> {
    let _ = client;
    todo!("workspaces-help.ts:111")
}

/// `workspaceStatuses(client)` (workspaces-help.ts:118): `status.list`, its `statuses` as the host wrote them.
pub async fn workspace_statuses(client: &dyn HostClient) -> Result<Vec<WorkspaceListing>, HostError> {
    let _ = client;
    todo!("workspaces-help.ts:118")
}

/// `threads(client, workspaceId?)` (workspaces-help.ts:122): `sessions.list` (with `{workspaceId}` where given, even
/// an empty one; no params else), folded by `fold_threads`.
pub async fn threads(client: &dyn HostClient, workspace_id: Option<&str>) -> Result<Vec<ThreadView>, HostError> {
    let _ = (client, workspace_id);
    todo!("workspaces-help.ts:122")
}

/// `workspaceOf(client, ref)` (workspaces-help.ts:130): `workspaces.resolve {ref}`; a `workspace` that does not read
/// as a `WorkspaceOut` fails with a plain error `other_version("workspaces.resolve")`. The record answered is parsed:
/// keys in the schema's order, unknown keys dropped.
pub async fn workspace_of(client: &dyn HostClient, r#ref: &str) -> Result<WorkspaceOut, HostError> {
    let _ = (client, r#ref);
    todo!("workspaces-help.ts:130")
}

/// `pickThread(all, ref)` (workspaces-help.ts:138): the thread whose id is `ref`, else the one whose id starts with
/// it where exactly one does; several fail `<n> threads start with <ref>; give more of the id` (plain error);
/// none fail `no thread <ref>` (`not_found_refusal`). The empty ref is a prefix of every id.
pub fn pick_thread(all: &[ThreadView], r#ref: &str) -> Result<ThreadView, HostError> {
    let _ = (all, r#ref);
    todo!("workspaces-help.ts:138")
}

/// `threadOf(client, ref)` (workspaces-help.ts:148): one `threads` read, then `pick_thread`.
pub async fn thread_of(client: &dyn HostClient, r#ref: &str) -> Result<ThreadView, HostError> {
    let _ = (client, r#ref);
    todo!("workspaces-help.ts:148")
}

/// `threadsOf(client, refs)` (workspaces-help.ts:153): one `threads` read, each ref picked in the order named; the
/// first ref that picks nothing ends the call. No refs, no rows, still one read.
pub async fn threads_of(client: &dyn HostClient, refs: &[String]) -> Result<Vec<ThreadView>, HostError> {
    let _ = (client, refs);
    todo!("workspaces-help.ts:153")
}

/// `ThreadRow` (workspaces-help.ts:158): the thread's row, then `projectName`, `folder`, `branch`, `computerName`.
pub type ThreadRow = crate::verbs::io::ThreadRowOut;

/// `threadRows(client, within?)` (workspaces-help.ts:164-184): `workspaces.list`, then `sessions.list {}`, then (only
/// where `within` names a project no session row names) `projects_here`. A `within` that no session row's project
/// and no listed project names fails `refusal(no_project_line(within), READ_PROJECTS_FIX, not-found)`. The rows are
/// the folded threads, those of the named project only when `within` is given, each with the project and computer
/// names its session row carries (empty where absent), its folder (`thread_cwd`, else the workspace's folder, else
/// its project's path, else empty) and its branch: `branch_here(folder)` where the workspace is local, else the
/// workspace's worktree branch unless that worktree is gone, else empty. One branch read per folder.
pub async fn thread_rows(client: &dyn HostClient, within: Option<&str>) -> Result<Vec<ThreadRow>, HostError> {
    let _ = (client, within);
    todo!("workspaces-help.ts:164")
}

/// `branchHere(folder)` (workspaces-help.ts:869-881): the branch a folder on this computer has checked out. The repo
/// root is the nearest folder up the tree holding a `.git` (`git_root_of`); a `.git` file's `gitdir:` line points at
/// the git dir (relative to the root); the branch is the first `ref: refs/heads/<name>` line of its `HEAD`, trimmed.
/// Nothing where the folder is not here, holds no repo, has no readable `HEAD`, or stands on no branch.
#[allow(dead_code)]
fn branch_here(folder: &str) -> Option<String> {
    let _ = folder;
    todo!("workspaces-help.ts:871")
}

/// `stateLine(workspace)` (workspaces-help.ts:189): `workspace_state_line(name, workspace_state({phase}), pauseMode)`:
/// the phase is the whole of what it reads.
pub fn state_line(workspace: &WorkspaceView) -> String {
    let _ = workspace;
    todo!("workspaces-help.ts:189")
}

/// `wokeLine(client, workspace)` (workspaces-help.ts:196): `status.list`, the entry of the workspace's id read as
/// the state word the next listing prints (`workspace_state_of(listed, listed)`: the listing's own phase, machine
/// state and reach), else the phase of the workspace given; with its pause mode.
pub async fn woke_line(client: &dyn HostClient, workspace: &WorkspaceOut) -> Result<String, HostError> {
    let _ = (client, workspace);
    todo!("workspaces-help.ts:196")
}

/// `nap(client, ref)` (workspaces-help.ts:202): `workspace_of`, then `workspaces.nap {workspaceId}`, the `workspace`
/// as the host wrote it.
pub async fn nap(client: &dyn HostClient, r#ref: &str) -> Result<WorkspaceOut, HostError> {
    let _ = (client, r#ref);
    todo!("workspaces-help.ts:202")
}

/// `resize(client, ref, size)` (workspaces-help.ts:209): `thread_of`, then `workspaces.resize {workspaceId (the
/// thread's), size}`, the `workspace` as the host wrote it.
pub async fn resize(client: &dyn HostClient, r#ref: &str, size: &WorkspaceSize) -> Result<WorkspaceOut, HostError> {
    let _ = (client, r#ref, size);
    todo!("workspaces-help.ts:209")
}

/// `rebuild(client, ref)` (workspaces-help.ts:218): `workspace_of`, `status.list`, then (unless the workspace needs
/// no rebuild: not gone, no zombie reach, no `wakeRefused`, `needs_rebuild`) a plain error
/// `gone_road_refusal(state, Rebuild, pauseMode)` with no frame after; else `workspaces.rebuild {workspaceId}`.
pub async fn rebuild(client: &dyn HostClient, r#ref: &str) -> Result<WorkspaceOut, HostError> {
    let _ = (client, r#ref);
    todo!("workspaces-help.ts:218")
}

/// `rebuiltLine(workspace)` (workspaces-help.ts:229): `<state_line> on <machineId>`; no pause mode is read.
pub fn rebuilt_line(workspace: &WorkspaceView) -> String {
    let _ = workspace;
    todo!("workspaces-help.ts:229")
}

/// `imageView(client)` (workspaces-help.ts:234): `image.get {}`, its `view` parsed as a `SealedImageView` (keys in
/// the schema's order, unknown keys dropped).
pub async fn image_view(client: &dyn HostClient) -> Result<SealedImageView, HostError> {
    let _ = client;
    todo!("workspaces-help.ts:234")
}

/// What `buildImageAt` answers: the record the host holds after the build and what the build came to.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ImageBuilt {
    pub image: SealedImage,
    pub built: SealedImageBuilt,
}

/// `buildImageAt(client, out, word, force?)` (workspaces-help.ts:242-265): `places.list`; the place the word names
/// (`names_place`) gives its id and `table_name`, else the word stands for both; `events` is subscribed, then
/// `image.build {place: word, force?}` (`force` only where true) runs while every `golden.stage` frame of that
/// place's id streams one line `<place name>: <stage word>` (`Failed` for a failed stage), with the frame's detail
/// after two spaces, to `out.stream`; then `image.get`; no image there fails a plain error `NO_SEALED_IMAGE`;
/// else the image and the parsed build. The stream is stopped whatever came.
pub async fn build_image_at(client: &dyn HostClient, out: &Out, word: &str, force: Option<bool>) -> Result<ImageBuilt, HostError> {
    let _ = (client, out, word, force);
    todo!("workspaces-help.ts:242")
}

/// `imageLines(view)` (workspaces-help.ts:268): `[NO_SEALED_IMAGE]` for no record, else `sealed_image_line` then one
/// `sealed_copy_line` per copy in the host's order.
pub fn image_lines(view: &SealedImageView) -> Vec<String> {
    let _ = view;
    todo!("workspaces-help.ts:268")
}

/// `HOST_SIDE_VAULT` (workspaces-help.ts:279).
pub const HOST_SIDE_VAULT: &str = "The vault leaves the host only as a sealed file on the computer that typed the line.";

/// `SSH_PIPES_HERE_LINE` (workspaces-help.ts:282).
pub const SSH_PIPES_HERE_LINE: &str = "wsp ssh pipes the stdin and stdout of the ssh client that runs it, on the computer the host runs on; there is none here.";

/// `sshWorkspaceOf(client, ref)` (workspaces-help.ts:286): a ref that starts with `wsp-` (`SSH_ALIAS_PREFIX`) reads
/// `workspaces.list` and takes the workspaces whose `ssh_alias(name)` is the ref: two or more fail
/// `usage_refusal("<n> workspaces go by <ref>: <name> (<id>), ...", "Rename one with wsp rename and open it again.")`;
/// one is answered as the list gave it (no resolve); none, or a ref without the prefix, falls to `workspace_of`.
pub async fn ssh_workspace_of(client: &dyn HostClient, r#ref: &str) -> Result<WorkspaceOut, HostError> {
    let _ = (client, r#ref);
    todo!("workspaces-help.ts:286")
}

/// `pipeBytes(port, bytes)` (workspaces-help.ts:296-309): connects to `127.0.0.1:<port>`; this process's input is
/// piped to the socket and the socket to its output (the output is not ended); settles when the socket closes; a
/// connect error fails the call.
pub async fn pipe_bytes(port: u16, bytes: IoBytes) -> Result<(), HostError> {
    let _ = (port, bytes);
    todo!("workspaces-help.ts:296")
}
