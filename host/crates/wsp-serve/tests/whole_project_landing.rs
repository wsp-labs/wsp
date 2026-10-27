// SPDX-License-Identifier: AGPL-3.0-only
//! packages/runtime/test/project-landing.test.ts at 65f6b89c7, case for case. What an add does on the computer holding the
//! project: the clone, the files the person ticked, the install the lockfile picks, and on a provider the image every workspace
//! of the project forks. Over the stub backend, so every command the machine was asked to run is read back here. Each test names
//! the TS test above it. The TS `memoryStore()` is the sqlite store here.
mod common;

use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use common::local::local_wiring;
use common::*;
use indexmap::IndexMap;
use serde_json::{json, Value};
use wsp_core::types::api::ProjectAddAsk;
use wsp_core::types::internal::copy_key;
use wsp_core::types::wiring::{SeedPack, SeedWiring};
use wsp_engine::copier::{fake_copier, Copier};
use wsp_store::store::Store;
use wsp_wire::types::place_word::HERE_PLACE_ID;
use wsp_wire::views::workspace::{SeedChoice, SeedPlan};
use wsp_words::exit::HostError;

const REMOTE: &str = "https://github.com/spoo-me/frontend.git";

fn version() -> Value {
    json!({ "version": 1, "snapshotId": "snap_golden-v1", "baseTemplate": "base", "setupSha": "s1", "createdAt": "2026-09-17T00:00:00.000Z", "smoke": { "cmd": "true", "exitCode": 0 } })
}

/// A git repo in a folder of its own on this computer: the source a seed reads. Resolved, as the host keys a folder's remembered
/// picks.
fn repo(keep: &mut Vec<tempfile::TempDir>) -> String {
    let dir = tempfile::Builder::new().prefix("wsp-seed-add-").tempdir().expect("a folder");
    let path = std::fs::canonicalize(dir.path()).expect("a real path");
    let ok = std::process::Command::new("git").arg("init").arg("-q").arg(&path).status().expect("git").success();
    assert!(ok, "git init");
    keep.push(dir);
    path.display().to_string()
}

fn plan(folder: &str, files: Value) -> SeedPlan {
    serde_json::from_value(json!({
        "source": folder, "remote": REMOTE, "branch": "refactor/dashboard-polish", "defaultBranch": "main", "unpushed": null, "uncommitted": 2,
        "memory": { "key": "-Users-dev-spoo-landing", "files": 3, "bytes": 20_000 },
        "files": files,
        "remembered": false,
    }))
    .expect("a plan")
}

fn default_files() -> Value {
    json!([
        { "path": ".env.local", "dir": false, "bytes": 4096, "kind": "config", "row": { "id": "next", "name": "Next" }, "ticked": true },
        { "path": "node_modules", "dir": true, "bytes": 2_600_000_000u64, "kind": "rebuilt", "row": { "id": "node", "name": "Node" }, "ticked": false },
    ])
}

/// The folder reader that answers the plan handed in, and a packer that notes each ask.
struct Reader {
    plan: SeedPlan,
}

#[async_trait(?Send)]
impl SeedWiring for Reader {
    async fn plan(&self, _folder: &str, _homes: &IndexMap<String, String>) -> Result<SeedPlan, HostError> {
        Ok(self.plan.clone())
    }
    async fn pack(&self, _plan: SeedPlan, choice: SeedChoice, _homes: &IndexMap<String, String>) -> Result<SeedPack, HostError> {
        Ok(SeedPack { tar: b"seed archive".to_vec(), files: choice.files.len() as u64, bytes: 12, commits: 0, left: vec![] })
    }
}

struct World {
    fx: Fx,
    backend: Arc<StubBackend>,
    store: Rc<dyn Store>,
    _keep: (tempfile::TempDir, State),
}

/// A host with an image sealed on the computer named, the stub provider as its backend and a folder reader that answers the plan
/// handed in: what an add of a folder on this computer onto a computer that clones needs.
async fn with_image(plan: SeedPlan) -> World {
    let state = sqlite_state();
    state.store.put("goldens", &copy_key("default", "default"), json!({ "head": 1, "versions": [version()] })).await.expect("an image");
    let root = tempfile::tempdir().expect("a folder");
    let copier: Rc<dyn Copier> = Rc::new(fake_copier(None, None));
    let mut local = local_wiring(&std::fs::canonicalize(root.path()).expect("a real path"), copier);
    local.daemon_road = None;
    let backend = StubBackend::new(None);
    let seed: Rc<dyn SeedWiring> = Rc::new(Reader { plan });
    let fx = runtime(&backend)
        .store(&state.store)
        .with(move |o| {
            o.local = Some(local);
            o.seed = Some(seed);
            o.kill_confirm = Some(wsp_engine::gone::KillConfirm { grace_ms: Some(20), poll_ms: Some(1) });
        })
        .build();
    World { fx, backend, store: state.store.clone(), _keep: (root, state) }
}

/// Every command every machine of this backend was asked to run, oldest first.
fn commands(backend: &Arc<StubBackend>) -> String {
    backend.machines().iter().flat_map(|m| m.exec_log()).collect::<Vec<_>>().join("\n")
}

/// What the machine answers every command with, over the probe the stub answers by default.
fn answering(backend: &Arc<StubBackend>) {
    backend.set_exec_impl(exec_fn(|_, _| Ok(exec_ok(""))));
}

async fn add(w: &World, source: &str, on: &str) -> Value {
    let ask = ProjectAddAsk { source: source.into(), on: Some(on.into()), ..Default::default() };
    j(&w.fx.rt.projects().add(ask, None).await.expect("a project").project)
}

// "a repo named by owner and repo" > "is cloned through the host's own command line, which the computer's image is checked for first"
#[test]
fn is_cloned_through_the_hosts_own_command_line_which_the_computers_image_is_checked_for_first() {
    drive(|| async {
        let w = with_image(plan("/x", default_files())).await;
        answering(&w.backend);
        let project = add(&w, "spoo-me/frontend", "default").await;
        assert_eq!(project["source"], json!({ "kind": "github", "repo": "spoo-me/frontend" }));
        assert_eq!(project["remote"], "https://github.com/spoo-me/frontend.git");
        // Nothing runs at the add for a repo the computer clones by itself; the workspace's own create does that.
        assert!(!commands(&w.backend).contains("gh repo clone"));
        let before = w.backend.machines().len();
        create_on(
            &w.fx.rt,
            CreateOn { project: Some(project["id"].as_str().expect("an id").to_string()), ..CreateOn::named("work") },
            None,
        )
        .await
        .expect("a workspace");
        let ran = commands(&w.backend);
        assert!(ran.contains("command -v gh"), "{ran}");
        assert!(ran.contains("gh auth setup-git"), "{ran}");
        assert!(ran.contains("gh repo clone spoo-me/frontend /root/frontend"), "{ran}");
        assert!(w.backend.machine(before).spec_now().binds.is_none());
        w.fx.rt.close().await.expect("a close");
    });
}

// "a repo named by owner and repo" > "a gitlab repo is named with its host and cloned through glab"
#[test]
fn a_gitlab_repo_is_named_with_its_host_and_cloned_through_glab() {
    drive(|| async {
        let w = with_image(plan("/x", default_files())).await;
        answering(&w.backend);
        let project = add(&w, "gitlab.com/dev/thing", "default").await;
        assert_eq!(project["source"], json!({ "kind": "gitlab", "repo": "dev/thing" }));
        create_on(
            &w.fx.rt,
            CreateOn { project: Some(project["id"].as_str().expect("an id").to_string()), ..CreateOn::named("work") },
            None,
        )
        .await
        .expect("a workspace");
        assert!(commands(&w.backend).contains("glab repo clone dev/thing /root/thing"));
        w.fx.rt.close().await.expect("a close");
    });
}

// "a file of values or a key on the menu" > "stays ticked for a computer of the person's own, remembered or not, and is unticked for a provider place, which the plan names"
#[test]
fn stays_ticked_for_a_computer_of_the_persons_own_remembered_or_not_and_is_unticked_for_a_provider_place_which_the_plan_names() {
    drive(|| async {
        let mut keep = Vec::new();
        let folder = repo(&mut keep);
        let values = json!([{ "path": ".env", "dir": false, "bytes": 120, "kind": "secret", "row": { "id": "node", "name": "Node" }, "ticked": true }]);
        let w = with_image(plan(&folder, values)).await;
        answering(&w.backend);
        let ticked = |p: &Value| -> Vec<String> {
            p["files"]
                .as_array()
                .expect("files")
                .iter()
                .filter(|f| f["ticked"] == true)
                .map(|f| f["path"].as_str().unwrap_or_default().to_string())
                .collect()
        };
        let here = j(&w.fx.rt.projects().seed_plan(&folder, None).await.expect("a plan"));
        assert_eq!(ticked(&here), [".env"]);
        let cloud = j(&w.fx.rt.projects().seed_plan(&folder, Some("default".into())).await.expect("a plan"));
        assert_eq!(cloud["provider"], true);
        assert!(ticked(&cloud).is_empty());
        // A choice remembered from an add to a box keeps the file ticked for the next box, and no provider gets it.
        w.store
            .put("seed-choices", &folder, json!({ "files": [".env"], "memory": false, "commits": false, "remember": true }))
            .await
            .expect("a choice");
        let boxed = j(&w.fx.rt.projects().seed_plan(&folder, None).await.expect("a plan"));
        assert_eq!((boxed["remembered"].clone(), ticked(&boxed)), (json!(true), vec![".env".to_string()]));
        let remembered = j(&w.fx.rt.projects().seed_plan(&folder, Some("default".into())).await.expect("a plan"));
        assert_eq!((remembered["remembered"].clone(), ticked(&remembered)), (json!(true), vec![]));
        w.fx.rt.close().await.expect("a close");
    });
}

// "a project taken away from this computer or from a provider" > "a folder here is recorded on a host that has sealed no image, since nothing of it is forked anywhere"
#[test]
fn a_folder_here_is_recorded_on_a_host_that_has_sealed_no_image_since_nothing_of_it_is_forked_anywhere() {
    drive(|| async {
        let mut keep = Vec::new();
        let backend = StubBackend::new(None);
        let root = tempfile::tempdir().expect("a folder");
        let copier: Rc<dyn Copier> = Rc::new(fake_copier(None, None));
        let mut local = local_wiring(&std::fs::canonicalize(root.path()).expect("a real path"), copier);
        local.daemon_road = None;
        // No goldens document at all: this host has sealed nothing.
        let fx = runtime(&backend).with(move |o| o.local = Some(local)).build();
        // The folder as this host resolved it, which on a Mac is the real path under /private.
        let folder = repo(&mut keep);
        let recorded =
            j(&fx.rt.projects().add(ProjectAddAsk { source: folder, ..Default::default() }, None).await.expect("a project").project);
        assert_eq!(recorded["computer"], HERE_PLACE_ID);
        assert_eq!(recorded["source"]["kind"], "folder");
        assert!(recorded["path"].as_str().unwrap_or_default().contains("wsp-seed-add-"), "{recorded}");
        assert!(backend.machines().is_empty());
        fx.rt.close().await.expect("a close");
    });
}
