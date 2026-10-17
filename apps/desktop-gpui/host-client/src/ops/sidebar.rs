// SPDX-License-Identifier: AGPL-3.0-only
use std::collections::HashMap;

use serde::Deserialize;

use crate::protocol;
use crate::Done;

ops! {
    "projects.list" => ProjectsListRequest => Projects,
    "projects.add" => ProjectsAddRequest => ProjectAdded,
    "projects.remove" => ProjectsRemoveRequest => ProjectRemoved,
    "projects.icon" => ProjectsIconRequest => ProjectImage,
    "projects.icons" => ProjectsIconsRequest => ProjectImages,
    "projects.defaults" => ProjectsDefaultsRequest => ProjectsDefaults,
    "projects.branch" => ProjectsBranchRequest => protocol::ProjectBranch,
    "workspaces.list" => WorkspacesListRequest => Workspaces,
    "workspaces.get" => WorkspacesGetRequest => Workspace,
    "workspaces.create" => WorkspacesCreateRequest => WorkspaceCreated,
    "workspaces.nap" => WorkspacesNapRequest => Workspace,
    "workspaces.wake" => WorkspacesWakeRequest => Workspace,
    "workspaces.stopWake" => WorkspacesStopWakeRequest => Workspace,
    "workspaces.rename" => WorkspacesRenameRequest => Workspace,
    "workspaces.look" => WorkspacesLookRequest => Workspace,
    "workspaces.delete" => WorkspacesDeleteRequest => Done,
    "workspaces.forget" => WorkspacesForgetRequest => Done,
    "workspaces.rebuild" => WorkspacesRebuildRequest => Workspace,
    "workspaces.restartDaemon" => WorkspacesRestartDaemonRequest => Done,
    "workspaces.landing" => WorkspacesLandingRequest => protocol::WorkspaceLanding,
    "workspaces.snapshot" => WorkspacesSnapshotRequest => Snapshotted,
    // Each ask adds a status watcher for the socket's life: ask once per socket.
    "status.subscribe" => StatusSubscribeRequest => Statuses,
    "sessions.list" => SessionsListRequest => Sessions,
    "sessions.mark" => SessionsMarkRequest => Done,
    "sessions.settle" => SessionsSettleRequest => protocol::SessionSettleResult,
    "sessions.restore" => SessionsRestoreRequest => protocol::SessionRestoreResult,
    "sessions.search" => SessionsSearchRequest => protocol::SessionSearchResult,
    "sessions.read" => SessionsReadRequest => Done,
    "sessions.forget" => SessionsForgetRequest => Done,
    "sessions.delete" => SessionsDeleteRequest => ThreadDeleted,
    "folder.make" => FolderMakeRequest => Workspace,
    "project.export" => ProjectExportRequest => ProjectExported,
    "host.folders" => HostFoldersRequest => Folders,
}

#[derive(Debug, Deserialize)]
pub struct Projects {
    pub projects: Vec<protocol::ProjectView>,
}

#[derive(Debug, Deserialize)]
pub struct ProjectAdded {
    pub project: protocol::ProjectView,
    pub notice: Option<String>,
}

/// The protocol says {}; the host answers the sentence the command line prints.
#[derive(Debug, Deserialize)]
pub struct ProjectRemoved {
    pub said: String,
    pub unsaved: Option<String>,
}

/// The hash the image is kept under, none once it is taken off.
#[derive(Debug, Deserialize)]
pub struct ProjectImage {
    pub image: Option<String>,
}

/// Each hash asked, its image as a data url, none for one the host does not keep.
#[derive(Debug, Deserialize)]
pub struct ProjectImages {
    pub icons: HashMap<String, Option<String>>,
}

/// By project id.
#[derive(Debug, Deserialize)]
pub struct ProjectsDefaults {
    pub defaults: HashMap<String, protocol::ThreadDefaults>,
}

#[derive(Debug, Deserialize)]
pub struct Workspaces {
    pub workspaces: Vec<protocol::WorkspaceOut>,
}

#[derive(Debug, Deserialize)]
pub struct Workspace {
    pub workspace: protocol::WorkspaceOut,
}

/// WorkspaceCreateResult's notice beside the view every verb answers with, and the start of a sub-thread asked for.
#[derive(Debug, Deserialize)]
pub struct WorkspaceCreated {
    pub workspace: protocol::WorkspaceOut,
    pub notice: Option<String>,
    pub sub: Option<protocol::SubStarted>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshotted {
    pub project_golden: protocol::ProjectGolden,
}

#[derive(Debug, Deserialize)]
pub struct Statuses {
    pub statuses: Vec<protocol::WorkspaceStatus>,
}

#[derive(Debug, Deserialize)]
pub struct Sessions {
    pub sessions: Vec<protocol::SessionView>,
}

/// What a delete took, or with check what it would take and the question a person is put first.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadDeleted {
    pub workspace_id: String,
    pub worktree: Option<String>,
    pub threads: u64,
    pub ended: Option<Vec<protocol::ActivityEnded>>,
    /// "deleted" where a sub-thread's machine went with it.
    pub machine: Option<String>,
    pub on: Option<String>,
    pub question: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ProjectExported {
    pub exported: protocol::ProjectExportResult,
}

#[derive(Debug, Deserialize)]
pub struct Folders {
    pub listing: protocol::HostFolderListing,
}
