// SPDX-License-Identifier: AGPL-3.0-only
use serde_json::json;
use wsp_host_client::protocol::*;

use crate::common::{answered, ask, first, refused, shared};

/// A 128 px RGB PNG of black, the one shape projects.icon keeps, as base64.
const ICON: &str =
    "const z=require('zlib');const c=(t,d)=>{const l=Buffer.alloc(4);l.writeUInt32BE(d.length);const b=Buffer.concat([Buffer.from(t),d]);\
const r=Buffer.alloc(4);r.writeUInt32BE(z.crc32(b));return Buffer.concat([l,b,r])};const h=Buffer.alloc(13);h.writeUInt32BE(128,0);\
h.writeUInt32BE(128,4);h[8]=8;h[9]=2;process.stdout.write(Buffer.concat([Buffer.from([137,80,78,71,13,10,26,10]),c('IHDR',h),\
c('IDAT',z.deflateSync(Buffer.alloc(128*385))),c('IEND',Buffer.alloc(0))]).toString('base64'))";

#[tokio::test]
async fn sidebar() {
    let client = shared().host.client().await;
    let first = first(&client).await;
    let projects = answered("projects.list", client.call(ask::<ProjectsListRequest>(json!({}))).await);
    assert!(projects.projects.iter().any(|p| p.name == "repo"));
    let defaults = answered("projects.defaults", client.call(ask::<ProjectsDefaultsRequest>(json!({}))).await);
    assert!(defaults.defaults.contains_key(&first.project_id));
    let workspaces = answered("workspaces.list", client.call(ask::<WorkspacesListRequest>(json!({}))).await);
    assert!(workspaces.workspaces.iter().any(|w| w.id == first.workspace_id));
    let statuses = answered("status.subscribe", client.call(ask::<StatusSubscribeRequest>(json!({}))).await);
    assert!(statuses.statuses.iter().any(|s| s.id == first.workspace_id));
    let sessions = answered("sessions.list", client.call(ask::<SessionsListRequest>(json!({ "workspaceId": first.workspace_id }))).await);
    assert!(sessions.sessions.iter().any(|s| s.id == first.session_id));
    let folder = shared().folder.to_string_lossy().into_owned();
    let listing = answered("host.folders", client.call(ask::<HostFoldersRequest>(json!({ "dir": folder }))).await);
    assert_eq!(listing.listing.dir, folder);
}

#[tokio::test]
async fn a_project_of_its_own() {
    let host = &shared().host;
    let client = host.client().await;
    let folder = host.repo("sidebar-x").to_string_lossy().into_owned();

    let added = answered("projects.add", client.call(ask::<ProjectsAddRequest>(json!({ "source": folder }))).await);
    assert_eq!(added.project.name, "sidebar-x");
    let project = added.project.id;
    let branch = answered("projects.branch", client.call(ask::<ProjectsBranchRequest>(json!({ "projectId": project }))).await);
    assert!(branch.folder && branch.branch.is_some(), "{branch:?}");

    let png = std::process::Command::new("node").args(["-e", ICON]).env_clear().envs(host.env()).output().unwrap().stdout;
    let png = String::from_utf8(png).unwrap();
    let set = answered("projects.icon", client.call(ask::<ProjectsIconRequest>(json!({ "projectId": project, "png": png }))).await);
    let hash = set.image.expect("the hash the image is kept under");
    let none = "0".repeat(64);
    let icons = answered("projects.icons", client.call(ask::<ProjectsIconsRequest>(json!({ "hashes": [hash, none] }))).await);
    assert!(icons.icons[&hash].as_deref().is_some_and(|url| url.starts_with("data:image/png;base64,")));
    assert_eq!(icons.icons[&none], None);
    let off = answered("projects.icon", client.call(ask::<ProjectsIconRequest>(json!({ "projectId": project, "png": null }))).await);
    assert_eq!(off.image, None);

    let made = answered("folder.make", client.call(ask::<FolderMakeRequest>(json!({ "project": project }))).await);
    assert_eq!(made.workspace.folder.as_deref(), Some(folder.as_str()));
    let ws = made.workspace.id;
    let got = answered("workspaces.get", client.call(ask::<WorkspacesGetRequest>(json!({ "workspaceId": ws }))).await);
    assert_eq!(got.workspace.project.id, project);
    let landing = answered("workspaces.landing", client.call(ask::<WorkspacesLandingRequest>(json!({ "project": project }))).await);
    assert!(matches!(landing.kind, Some(WorkspaceLandingKind::Local)), "{landing:?}");
    let created = answered(
        "workspaces.create",
        client.call(ask::<WorkspacesCreateRequest>(json!({ "project": project, "name": "sidebar-made" }))).await,
    );
    assert_eq!(created.workspace.id, ws, "a project in a folder has the one workspace");
    let renamed = answered(
        "workspaces.rename",
        client.call(ask::<WorkspacesRenameRequest>(json!({ "workspaceId": ws, "name": "sidebar-renamed" }))).await,
    );
    assert_eq!(renamed.workspace.name, "sidebar-renamed");
    let look =
        answered("workspaces.look", client.call(ask::<WorkspacesLookRequest>(json!({ "workspaceId": ws, "glyph": "rocket" }))).await);
    assert!(matches!(look.workspace.glyph, Some(WorkspaceOutGlyph::Rocket)));
    answered("workspaces.wake", client.call(ask::<WorkspacesWakeRequest>(json!({ "workspaceId": ws }))).await);
    answered("workspaces.stopWake", client.call(ask::<WorkspacesStopWakeRequest>(json!({ "workspaceId": ws }))).await);
    refused(
        "workspaces.nap",
        client.call(ask::<WorkspacesNapRequest>(json!({ "workspaceId": ws }))).await,
        "is this computer, which wsp does not run; it cannot be paused",
    );
    refused("workspaces.rebuild", client.call(ask::<WorkspacesRebuildRequest>(json!({ "workspaceId": ws }))).await, "it cannot be rebuilt");
    refused(
        "workspaces.snapshot",
        client.call(ask::<WorkspacesSnapshotRequest>(json!({ "workspaceId": ws }))).await,
        "it cannot be snapshotted",
    );
    refused(
        "workspaces.restartDaemon",
        client.call(ask::<WorkspacesRestartDaemonRequest>(json!({ "workspaceId": ws }))).await,
        "wsp-daemon binary missing",
    );
    let export = ask::<ProjectExportRequest>(json!({ "workspaceId": ws, "source": folder, "dest": format!("{folder}-home") }));
    refused("project.export", client.call(export).await, "this computer serves no signed download URL");

    host.wsp(&["run", "sidebar-x", "sidebar-second", "--agent", "claude"]);
    let sessions = answered("sessions.list", client.call(ask::<SessionsListRequest>(json!({ "workspaceId": ws }))).await);
    let thread = sessions
        .sessions
        .iter()
        .find(|s| s.prompt.as_deref() == Some("sidebar-second"))
        .and_then(|s| s.thread_id.clone())
        .expect("the thread wsp run opened");
    answered("sessions.read", client.call(ask::<SessionsReadRequest>(json!({ "threadId": thread }))).await);
    answered("sessions.mark", client.call(ask::<SessionsMarkRequest>(json!({ "threadIds": [thread], "marks": { "pinned": true } }))).await);
    let settled = answered("sessions.settle", client.call(ask::<SessionsSettleRequest>(json!({ "threadIds": [thread] }))).await);
    assert!(settled.settled.iter().any(|t| t.thread_id == thread), "{settled:?}");
    let restored = answered("sessions.restore", client.call(ask::<SessionsRestoreRequest>(json!({ "threadIds": [thread] }))).await);
    assert!(restored.restored.iter().any(|t| t.thread_id == thread), "{restored:?}");
    let found = answered("sessions.search", client.call(ask::<SessionsSearchRequest>(json!({ "query": "sidebar-second" }))).await);
    assert!(found.hits.iter().any(|h| h.thread_id == thread), "{found:?}");
    let forget = client.call(ask::<SessionsForgetRequest>(json!({ "threadId": thread }))).await;
    refused("sessions.forget", forget, "has a turn that ran; only a thread no turn ever ran on can be forgotten");
    let asked = answered("sessions.delete", client.call(ask::<SessionsDeleteRequest>(json!({ "threadId": thread, "check": true }))).await);
    assert_eq!((asked.workspace_id.as_str(), asked.threads), (ws.as_str(), 1));
    answered("sessions.delete", client.call(ask::<SessionsDeleteRequest>(json!({ "threadId": thread }))).await);
    let left = answered("sessions.list", client.call(ask::<SessionsListRequest>(json!({ "workspaceId": ws }))).await);
    assert!(!left.sessions.iter().any(|s| s.thread_id.as_deref() == Some(thread.as_str())));

    refused(
        "workspaces.forget",
        client.call(ask::<WorkspacesForgetRequest>(json!({ "workspaceId": ws }))).await,
        "runs on this computer, which is not at a provider",
    );
    answered("workspaces.delete", client.call(ask::<WorkspacesDeleteRequest>(json!({ "workspaceId": ws }))).await);
    let removed = answered("projects.remove", client.call(ask::<ProjectsRemoveRequest>(json!({ "projectId": project }))).await);
    assert!(removed.said.contains("is no longer a project here"), "{removed:?}");
}
