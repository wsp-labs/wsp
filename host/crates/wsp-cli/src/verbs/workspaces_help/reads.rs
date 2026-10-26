// SPDX-License-Identifier: AGPL-3.0-only
//! `workspaces-help.ts:111-296` and `:158-184`, `:869-881` at 65f6b89c7: listings, picking a thread or a workspace,
//! the thread rows, waking, napping, resizing, rebuilding, the image, ssh. Spec: `spec/ws-reads.md`.
//! The first seven items are the ones every other unit calls (signatures fixed by the lead).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use regex::Regex;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::io::AsyncWriteExt;
use wsp_wire::types::app_ports::LOOPBACK;
use wsp_wire::views::golden_image::{GoldenStage, GoldenStageEvent, SealedImage, SealedImageBuilt};
use wsp_wire::views::place_kind::PlaceKind;
use wsp_wire::views::place_more::PlaceView;
use wsp_wire::views::session::{SessionView, ThreadView};
use wsp_wire::views::snapshot_lineage::SealedImageView;
use wsp_wire::views::workspace_more::{WorkspaceListing, WorkspaceOut, WorkspaceView};
use wsp_wire::wire::capabilities::WorkspaceSize;
use wsp_words::exit::{not_found_refusal, refusal, usage_refusal, HostError, Kind};
use wsp_words::place_name::table_name;
use wsp_words::place_word::{names_place, PlaceRef};
use wsp_words::views::session::fold_threads;
use wsp_words::words::image::{sealed_copy_line, sealed_image_line, NO_SEALED_IMAGE};
use wsp_words::words::init::golden_stage_word;
use wsp_words::words::thread::{no_project_line, READ_PROJECTS_FIX};
use wsp_words::workspace_state::{gone_road_refusal, needs_rebuild, workspace_state, workspace_state_line, GoneRoad, WorkspaceStateInput};

use crate::repo_root::git_root_of;
use crate::verbs::client::{other_version, pick, Frame, HostClient, IoBytes, Out, PushedFrames};
use crate::verbs::io::ThreadRowOut;

use super::life::thread_cwd;
use super::rows::{projects_here, ProjectPick};

pub(super) fn reply_field<T: DeserializeOwned>(reply: &Frame, op: &str, key: &str) -> Result<T, HostError> {
    reply.get(key).cloned().and_then(|value| serde_json::from_value(value).ok()).ok_or_else(|| HostError::new(other_version(op)))
}

pub(super) fn params(value: Value) -> Frame {
    match value {
        Value::Object(map) => map,
        _ => Frame::new(),
    }
}

pub(super) fn place_ref(p: &PlaceView) -> PlaceRef {
    let kind = if matches!(p.kind, PlaceKind::Provider) { "provider" } else { "computer" };
    PlaceRef { id: p.id.clone(), name: p.name.clone(), kind: Some(kind.to_owned()) }
}

pub(super) fn names_project(id: &str, name: &str, within: Option<&str>) -> bool {
    within.is_some_and(|w| id == w || name == w)
}

fn io_error(e: std::io::Error) -> HostError {
    HostError::new(e.to_string())
}

/// `workspaces(client)` (workspaces-help.ts:111): `workspaces.list`, the `workspaces` array as the host wrote it
/// (not parsed: the TS hands the reply's objects on as they came).
pub async fn workspaces(client: &dyn HostClient) -> Result<Vec<WorkspaceOut>, HostError> {
    let reply = client.request("workspaces.list", Frame::new()).await?;
    reply_field(&reply, "workspaces.list", "workspaces")
}

/// `workspaceStatuses(client)` (workspaces-help.ts:118): `status.list`, its `statuses` as the host wrote them.
pub async fn workspace_statuses(client: &dyn HostClient) -> Result<Vec<WorkspaceListing>, HostError> {
    let reply = client.request("status.list", Frame::new()).await?;
    reply_field(&reply, "status.list", "statuses")
}

/// `threads(client, workspaceId?)` (workspaces-help.ts:122): `sessions.list` (with `{workspaceId}` where given, even
/// an empty one; no params else), folded by `fold_threads`.
pub async fn threads(client: &dyn HostClient, workspace_id: Option<&str>) -> Result<Vec<ThreadView>, HostError> {
    let reply = client.request("sessions.list", pick(vec![("workspaceId", workspace_id.map(|id| json!(id)))])).await?;
    let sessions: Vec<SessionView> = reply_field(&reply, "sessions.list", "sessions")?;
    Ok(fold_threads(&sessions))
}

/// `workspaceOf(client, ref)` (workspaces-help.ts:130): `workspaces.resolve {ref}`; a `workspace` that does not read
/// as a `WorkspaceOut` fails with a plain error `other_version("workspaces.resolve")`. The record answered is parsed:
/// keys in the schema's order, unknown keys dropped.
pub async fn workspace_of(client: &dyn HostClient, r#ref: &str) -> Result<WorkspaceOut, HostError> {
    let reply = client.request("workspaces.resolve", params(json!({ "ref": r#ref }))).await?;
    reply_field(&reply, "workspaces.resolve", "workspace")
}

/// `pickThread(all, ref)` (workspaces-help.ts:138): the thread whose id is `ref`, else the one whose id starts with
/// it where exactly one does; several fail `<n> threads start with <ref>; give more of the id` (plain error);
/// none fail `no thread <ref>` (`not_found_refusal`). The empty ref is a prefix of every id.
pub fn pick_thread(all: &[ThreadView], r#ref: &str) -> Result<ThreadView, HostError> {
    if let Some(exact) = all.iter().find(|t| t.id == r#ref) {
        return Ok(exact.clone());
    }
    let prefixed: Vec<&ThreadView> = all.iter().filter(|t| t.id.starts_with(r#ref)).collect();
    match prefixed.as_slice() {
        [one] => Ok((*one).clone()),
        [] => Err(not_found_refusal(&format!("no thread {}", r#ref))),
        many => Err(HostError::new(format!("{} threads start with {}; give more of the id", many.len(), r#ref))),
    }
}

/// `threadOf(client, ref)` (workspaces-help.ts:148): one `threads` read, then `pick_thread`.
pub async fn thread_of(client: &dyn HostClient, r#ref: &str) -> Result<ThreadView, HostError> {
    pick_thread(&threads(client, None).await?, r#ref)
}

/// `threadsOf(client, refs)` (workspaces-help.ts:153): one `threads` read, each ref picked in the order named; the
/// first ref that picks nothing ends the call. No refs, no rows, still one read.
pub async fn threads_of(client: &dyn HostClient, refs: &[String]) -> Result<Vec<ThreadView>, HostError> {
    let all = threads(client, None).await?;
    refs.iter().map(|r| pick_thread(&all, r)).collect()
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
    let all = workspaces(client).await?;
    let reply = client.request("sessions.list", Frame::new()).await?;
    let sessions: Vec<SessionView> = reply_field(&reply, "sessions.list", "sessions")?;
    let mut stood: HashMap<&str, &SessionView> = HashMap::new();
    for session in &sessions {
        stood.insert(session.workspace_id.as_str(), session);
    }
    let session_project = |id: &str| stood.get(id).and_then(|s| s.project.as_ref());
    if let Some(word) = within {
        let session_names = sessions.iter().any(|s| s.project.as_ref().is_some_and(|p| names_project(&p.id, &p.name, within)));
        if !session_names
            && !projects_here(client).await.unwrap_or_default().iter().any(|p: &ProjectPick| names_project(&p.id, &p.name, within))
        {
            return Err(refusal(&no_project_line(word), READ_PROJECTS_FIX, Some(Kind::NotFound)));
        }
    }
    let rows = fold_threads(&sessions)
        .into_iter()
        .filter(|t| within.is_none() || session_project(&t.workspace_id).is_some_and(|p| names_project(&p.id, &p.name, within)));
    let mut branches: HashMap<String, String> = HashMap::new();
    let mut out = Vec::new();
    for t in rows {
        let workspace = all.iter().find(|w| w.id == t.workspace_id);
        let folder = thread_cwd(t.cwd.as_deref(), workspace.and_then(|w| w.worktree.as_ref()))
            .or_else(|| workspace.and_then(|w| w.folder.clone()))
            .or_else(|| workspace.map(|w| w.project.path.clone()))
            .unwrap_or_default();
        if !branches.contains_key(&folder) {
            let here = if wsp_words::workspace_state::is_local_workspace(workspace.and_then(|w| w.kind.as_ref())) {
                branch_here(&folder)
            } else {
                None
            };
            let branch = here
                .or_else(|| workspace.and_then(|w| w.worktree.as_ref()).filter(|wt| wt.gone.is_none()).and_then(|wt| wt.branch.clone()))
                .unwrap_or_default();
            branches.insert(folder.clone(), branch);
        }
        let branch = branches[&folder].clone();
        let stood_row = stood.get(t.workspace_id.as_str());
        out.push(ThreadRowOut {
            project_name: session_project(&t.workspace_id).map(|p| p.name.clone()).unwrap_or_default(),
            folder,
            branch,
            computer_name: stood_row.and_then(|s| s.computer_name.clone()).unwrap_or_default(),
            thread: t,
        });
    }
    Ok(out)
}

static GITDIR: std::sync::LazyLock<Regex> =
    std::sync::LazyLock::new(|| Regex::new(r"(?m)^gitdir:\s*([^\n\r\u{2028}\u{2029}]+)").expect("a fixed pattern"));
static REF_LINE: std::sync::LazyLock<Regex> =
    std::sync::LazyLock::new(|| Regex::new(r"(?m)^ref: refs/heads/([^\n\r\u{2028}\u{2029}]+)").expect("a fixed pattern"));

/// `branchHere(folder)` (workspaces-help.ts:869-881): the branch a folder on this computer has checked out. The repo
/// root is the nearest folder up the tree holding a `.git` (`git_root_of`); a `.git` file's `gitdir:` line points at
/// the git dir (relative to the root); the branch is the first `ref: refs/heads/<name>` line of its `HEAD`, trimmed.
/// Nothing where the folder is not here, holds no repo, has no readable `HEAD`, or stands on no branch.
#[allow(dead_code)]
fn branch_here(folder: &str) -> Option<String> {
    let root = git_root_of(folder)?;
    let dot_git = Path::new(&root).join(".git");
    let git_dir = if std::fs::metadata(&dot_git).ok()?.is_dir() {
        dot_git
    } else {
        let text = String::from_utf8_lossy(&std::fs::read(&dot_git).ok()?).into_owned();
        let rel = GITDIR.captures(&text).and_then(|c| c.get(1)).map_or("", |m| m.as_str().trim());
        if rel.starts_with('/') {
            PathBuf::from(rel)
        } else {
            Path::new(&root).join(rel)
        }
    };
    let head = String::from_utf8_lossy(&std::fs::read(git_dir.join("HEAD")).ok()?).into_owned();
    REF_LINE.captures(&head).and_then(|c| c.get(1)).map(|m| m.as_str().trim().to_owned())
}

/// `stateLine(workspace)` (workspaces-help.ts:189): `workspace_state_line(name, workspace_state({phase}), pauseMode)`:
/// the phase is the whole of what it reads.
pub fn state_line(workspace: &WorkspaceView) -> String {
    let state = workspace_state(&WorkspaceStateInput { phase: workspace.phase, machine_state: None, reach: None, wake_refused: None });
    workspace_state_line(&workspace.name, &state, workspace.pause_mode.as_ref())
}

/// `wokeLine(client, workspace)` (workspaces-help.ts:196): `status.list`, the entry of the workspace's id read as
/// the state word the next listing prints (`workspace_state_of(listed, listed)`: the listing's own phase, machine
/// state and reach), else the phase of the workspace given; with its pause mode.
pub async fn woke_line(client: &dyn HostClient, workspace: &WorkspaceOut) -> Result<String, HostError> {
    let listed = workspace_statuses(client).await?.into_iter().find(|w| w.id == workspace.id);
    let state = match listed {
        Some(listed) => workspace_state(&WorkspaceStateInput {
            phase: listed.phase,
            machine_state: Some(listed.machine_state),
            reach: Some(listed.reach.state),
            wake_refused: None,
        }),
        None => workspace_state(&WorkspaceStateInput { phase: workspace.phase, machine_state: None, reach: None, wake_refused: None }),
    };
    Ok(workspace_state_line(&workspace.name, &state, workspace.pause_mode.as_ref()))
}

/// `nap(client, ref)` (workspaces-help.ts:202): `workspace_of`, then `workspaces.nap {workspaceId}`, the `workspace`
/// as the host wrote it.
pub async fn nap(client: &dyn HostClient, r#ref: &str) -> Result<WorkspaceOut, HostError> {
    let source = workspace_of(client, r#ref).await?;
    let reply = client.request("workspaces.nap", params(json!({ "workspaceId": source.id }))).await?;
    reply_field(&reply, "workspaces.nap", "workspace")
}

/// `resize(client, ref, size)` (workspaces-help.ts:209): `thread_of`, then `workspaces.resize {workspaceId (the
/// thread's), size}`, the `workspace` as the host wrote it.
pub async fn resize(client: &dyn HostClient, r#ref: &str, size: &WorkspaceSize) -> Result<WorkspaceOut, HostError> {
    let thread = thread_of(client, r#ref).await?;
    let reply = client.request("workspaces.resize", params(json!({ "workspaceId": thread.workspace_id, "size": size }))).await?;
    reply_field(&reply, "workspaces.resize", "workspace")
}

/// `rebuild(client, ref)` (workspaces-help.ts:218): `workspace_of`, `status.list`, then (unless the workspace needs
/// no rebuild: not gone, no zombie reach, no `wakeRefused`, `needs_rebuild`) a plain error
/// `gone_road_refusal(state, Rebuild, pauseMode)` with no frame after; else `workspaces.rebuild {workspaceId}`.
pub async fn rebuild(client: &dyn HostClient, r#ref: &str) -> Result<WorkspaceOut, HostError> {
    let source = workspace_of(client, r#ref).await?;
    let listed = workspace_statuses(client).await?.into_iter().find(|w| w.id == source.id);
    let input = WorkspaceStateInput {
        phase: source.phase,
        machine_state: listed.as_ref().map(|l| l.machine_state),
        reach: listed.as_ref().map(|l| l.reach.state),
        wake_refused: source.wake_refused.clone(),
    };
    if !needs_rebuild(&input) {
        return Err(HostError::new(gone_road_refusal(&workspace_state(&input), GoneRoad::Rebuild, source.pause_mode.as_ref())));
    }
    let reply = client.request("workspaces.rebuild", params(json!({ "workspaceId": source.id }))).await?;
    reply_field(&reply, "workspaces.rebuild", "workspace")
}

/// `rebuiltLine(workspace)` (workspaces-help.ts:229): `<state_line> on <machineId>`; no pause mode is read.
pub fn rebuilt_line(workspace: &WorkspaceView) -> String {
    format!("{} on {}", state_line(workspace), workspace.machine_id)
}

/// `imageView(client)` (workspaces-help.ts:234): `image.get {}`, its `view` parsed as a `SealedImageView` (keys in
/// the schema's order, unknown keys dropped).
pub async fn image_view(client: &dyn HostClient) -> Result<SealedImageView, HostError> {
    let reply = client.request("image.get", Frame::new()).await?;
    reply_field(&reply, "image.get", "view")
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
    // The word goes to the host as typed, the one judge of what it names. The build's frames carry the place's id, so
    // the list is read only to follow them and say the place's name.
    let places: Vec<PlaceView> = reply_field(&client.request("places.list", Frame::new()).await?, "places.list", "places")?;
    let (place_id, place_name) = match places.iter().find(|p| names_place(&place_ref(p), word)) {
        None => (word.to_owned(), word.to_owned()),
        Some(p) => (p.id.clone(), table_name(&place_ref(p))),
    };
    let pushed = PushedFrames::new(client);
    let stage_out = out.clone();
    let stage_place = place_id.clone();
    pushed.follow(
        Arc::new(move |f: &Frame| {
            f.get("type").and_then(Value::as_str) == Some("golden.stage")
                && f.get("place").and_then(Value::as_str) == Some(stage_place.as_str())
        }),
        Arc::new(move |f: &Frame| {
            let Ok(event) = serde_json::from_value::<GoldenStageEvent>(Value::Object(f.clone())) else { return };
            let words = match event.stage {
                GoldenStage::Failed => "Failed".to_owned(),
                stage => golden_stage_word(stage).unwrap_or_default().to_owned(),
            };
            let mut line = format!("{place_name}: {words}");
            if let Some(detail) = &event.detail {
                line = format!("{line}  {detail}");
            }
            stage_out.stream(&format!("{line}\n"));
        }),
    );
    let result = build_and_read(client, word, force).await;
    pushed.stop();
    result
}

async fn build_and_read(client: &dyn HostClient, word: &str, force: Option<bool>) -> Result<ImageBuilt, HostError> {
    client.events().await?;
    let params = pick(vec![("place", Some(json!(word))), ("force", (force == Some(true)).then(|| json!(true)))]);
    let reply = client.request("image.build", params).await?;
    let view = image_view(client).await?;
    let image = view.image.ok_or_else(|| HostError::new(NO_SEALED_IMAGE))?;
    let build = reply.get("build").cloned().unwrap_or(Value::Null);
    let built = serde_json::from_value(build).map_err(|_| HostError::new(other_version("image.build")))?;
    Ok(ImageBuilt { image, built })
}

/// `imageLines(view)` (workspaces-help.ts:268): `[NO_SEALED_IMAGE]` for no record, else `sealed_image_line` then one
/// `sealed_copy_line` per copy in the host's order.
pub fn image_lines(view: &SealedImageView) -> Vec<String> {
    let Some(image) = &view.image else { return vec![NO_SEALED_IMAGE.to_owned()] };
    std::iter::once(sealed_image_line(image)).chain(view.copies.iter().map(|copy| sealed_copy_line(image, copy))).collect()
}

/// `HOST_SIDE_VAULT` (workspaces-help.ts:279).
pub const HOST_SIDE_VAULT: &str = "The vault leaves the host only as a sealed file on the computer that typed the line.";

/// `SSH_PIPES_HERE_LINE` (workspaces-help.ts:282).
pub const SSH_PIPES_HERE_LINE: &str =
    "wsp ssh pipes the stdin and stdout of the ssh client that runs it, on the computer the host runs on; there is none here.";

/// `sshWorkspaceOf(client, ref)` (workspaces-help.ts:286): a ref that starts with `wsp-` (`SSH_ALIAS_PREFIX`) reads
/// `workspaces.list` and takes the workspaces whose `ssh_alias(name)` is the ref: two or more fail
/// `usage_refusal("<n> workspaces go by <ref>: <name> (<id>), ...", "Rename one with wsp rename and open it again.")`;
/// one is answered as the list gave it (no resolve); none, or a ref without the prefix, falls to `workspace_of`.
pub async fn ssh_workspace_of(client: &dyn HostClient, r#ref: &str) -> Result<WorkspaceOut, HostError> {
    if r#ref.starts_with(wsp_wire::types::project_path::SSH_ALIAS_PREFIX) {
        let mut hits: Vec<WorkspaceOut> = workspaces(client).await?;
        hits.retain(|w| wsp_words::project_path::ssh_alias(&w.name) == r#ref);
        if hits.len() > 1 {
            let named: Vec<String> = hits.iter().map(|w| format!("{} ({})", w.name, w.id)).collect();
            return Err(usage_refusal(
                &format!("{} workspaces go by {}: {}", hits.len(), r#ref, named.join(", ")),
                "Rename one with wsp rename and open it again.",
            ));
        }
        if let Some(one) = hits.pop() {
            return Ok(one);
        }
    }
    workspace_of(client, r#ref).await
}

/// `pipeBytes(port, bytes)` (workspaces-help.ts:296-309): connects to `127.0.0.1:<port>`; this process's input is
/// piped to the socket and the socket to its output (the output is not ended); settles when the socket closes; a
/// connect error fails the call.
pub async fn pipe_bytes(port: u16, bytes: IoBytes) -> Result<(), HostError> {
    let socket = tokio::net::TcpStream::connect((LOOPBACK, port)).await.map_err(io_error)?;
    let (mut read, mut write) = socket.into_split();
    let IoBytes { mut input, mut output } = bytes;
    let up = async move {
        tokio::io::copy(&mut input, &mut write).await?;
        write.shutdown().await
    };
    let down = async move {
        tokio::io::copy(&mut read, &mut output).await?;
        output.flush().await
    };
    let mut up = Box::pin(up);
    let mut down = Box::pin(down);
    let mut up_done = false;
    loop {
        tokio::select! {
            done = &mut down => return done.map_err(io_error),
            done = &mut up, if !up_done => {
                up_done = true;
                done.map_err(io_error)?;
            }
        }
    }
}
