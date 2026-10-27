// SPDX-License-Identifier: AGPL-3.0-only
//! `create_runtime` packet 1 (#2476): the runtime builds on a throwaway state over `FakeBackend`, and every area
//! trait the context implements answers one call. A trait's test is red until its packet (2 to 17) writes its bodies
//! (the plan, section 3); `create_runtime_builds` and `the_public_members_forward` are this packet's own.
use std::rc::Rc;
use std::sync::Arc;

use serde_json::json;
use wsp_core::types::api::Runtime;
use wsp_core::types::wiring::{RuntimeOptions, WorkspaceRecord};
use wsp_engine::fake_backend::{FakeBackend, FakeBackendOptions};
use wsp_engine::machine::MachineBackend;
use wsp_serve::runtime::ctx::Ctx;
use wsp_serve::{create_runtime, AssembledRuntime};
use wsp_store::store::memory_store;

fn backend() -> Arc<dyn MachineBackend> {
    Arc::new(FakeBackend::new(FakeBackendOptions::default()))
}

fn options() -> RuntimeOptions {
    RuntimeOptions { backend: Some(backend()), store: Some(Rc::new(memory_store())), ..Default::default() }
}

fn record() -> WorkspaceRecord {
    serde_json::from_value(json!({
        "id": "w_1", "name": "n", "machineId": "m_1", "phase": "running", "golden": "g", "createdAt": "2026-10-26T00:00:00.000Z",
        "project": "p_1", "spec": {}, "size": { "cpu": 2, "memMb": 4096 }, "firstLife": false,
    }))
    .expect("a workspace record")
}

/// The runtime on a throwaway state, inside the task set the owner thread runs.
fn run<Fut: std::future::Future<Output = ()>>(f: impl FnOnce(Rc<AssembledRuntime>) -> Fut) {
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().expect("a tokio runtime");
    let local = tokio::task::LocalSet::new();
    local.block_on(&rt, async move { f(create_runtime(options()).expect("a runtime")).await });
}

#[test]
fn create_runtime_builds() {
    run(|runtime| async move {
        drop(runtime);
    });
}

#[test]
fn a_runtime_wired_without_place_links_has_no_place_door() {
    run(|runtime| async move {
        assert!(runtime.places().is_none());
        assert!(runtime.recipes().is_none());
    });
}

#[test]
fn the_public_members_forward() {
    run(|runtime| async move {
        let _ = runtime.events();
        let _ = runtime.backend();
        let _ = runtime.devices();
        let _ = runtime.agents();
        let _ = runtime.workspaces();
        let _ = runtime.projects();
        let _ = runtime.sessions();
        let _ = runtime.harnesses();
        let _ = runtime.golden();
        let _ = runtime.image();
        let _ = runtime.conversations();
        let _ = runtime.preferences();
        let _ = runtime.status();
        let _ = runtime.usage();
        let _ = runtime.slates();
        let _ = runtime.activity();
    });
}

macro_rules! answers {
    ($name:ident, |$c:ident| $body:expr) => {
        #[test]
        fn $name() {
            run(|runtime| async move {
                let $c: Rc<Ctx> = runtime.context();
                let _ = $body;
            });
        }
    };
}

answers!(transcripts_ctx, |c| wsp_threads::threads::transcripts::TranscriptsCtx::ids(&*c));
answers!(channels_ctx, |c| wsp_account::account::channels::ChannelsCtx::has_door(&*c));
answers!(proc_scopes_ctx, |c| wsp_account::account::proc_scopes::ProcScopesCtx::kept_agents(&*c));
answers!(box_panels_ctx, |c| wsp_account::account::box_panels::BoxPanelsCtx::thread_records(&*c));
answers!(trimmed_ctx, |c| wsp_threads::threads::trimmed_row::TrimmedCtx::started_as(&*c, "w", "t"));
answers!(waits_ctx, |c| wsp_threads::threads::waits::WaitsCtx::closing(&*c));
answers!(kinds_ctx, |c| wsp_machines::machines::kinds::KindsCtx::now(&*c));
answers!(backend_for, |c| wsp_machines::machines::sub_life::BackendFor::backend_for(&*c, &record()));
answers!(thread_ends_runtime, |c| wsp_machines::machines::thread_ends::ThreadEndsRuntime::running_on(&*c, "t"));
answers!(thread_ends_door, |c| wsp_machines::machines::thread_ends::ThreadEndsDoor::name_of(&*c, "p"));
answers!(rules_ctx, |c| wsp_account::account::rules::RulesCtx::no_machines_line(&*c));
answers!(records_ctx, |c| wsp_machines::images::records::RecordsCtx::wired(&*c));
answers!(held_ctx, |c| wsp_machines::images::held::HeldCtx::wired(&*c));
answers!(branch_ctx, |c| wsp_machines::projects::branch_of_record::BranchCtx::broker_ready(&*c));
answers!(image_door, |c| wsp_machines::images::builders::ImageDoor::name_of(&*c, "p"));
answers!(agents_places, |c| wsp_machines::agents_read::AgentsPlaces::list(&*c, 0).await);
answers!(reach_ctx, |c| wsp_machines::machines::reach::ReachCtx::closing(&*c));
answers!(daemon_runtime, |c| wsp_machines::machines::daemon::DaemonRuntime::daemon_hello_timeout_ms(&*c));
answers!(machines_runtime, |c| wsp_machines::machines::machines::MachinesRuntime::ids(&*c));
answers!(idle_door, |c| wsp_machines::machines::machines::IdleDoor::touch(&*c, "w"));
answers!(claim, |c| wsp_machines::images::builders::Claim::backend(&*wsp_machines::images::builders::BuildersCtx::claiming(
    &*c, "p", None, None
)));
answers!(boot_runtime, |c| wsp_machines::machines::boot::BootRuntime::hostname(&*c));
answers!(machines_landing_road, |c| wsp_machines::machines::create::LandingRoad::kind(&*c));
answers!(create_runtime_ctx, |c| wsp_machines::machines::create::CreateRuntime::clock(&*c));
answers!(sub_spawn_ctx, |c| wsp_machines::machines::sub_spawn::SubSpawnCtx::brokered_start(&*c));
answers!(sub_queue_ctx, |c| wsp_machines::machines::sub_queue::SubQueueCtx::uuid(&*c));
answers!(sub_life_ctx, |c| wsp_machines::machines::sub_life::SubLifeCtx::session_views(&*c));
answers!(pull_requests_ctx, |c| wsp_account::account::pull_requests::PullRequestsCtx::now(&*c));
answers!(base, |c| wsp_account::projects::workspaces::seam::Base::clock(&*c));
answers!(folders_ctx, |c| wsp_account::projects::folders::FoldersCtx::has_state_path(&*c));
answers!(start_from_ctx, |c| wsp_account::projects::start_from::StartFromCtx::places_wired(&*c));
answers!(create_ctx, |c| wsp_account::projects::workspaces::create::CreateCtx::places_wired(&*c));
answers!(phases_ctx, |c| wsp_account::projects::workspaces::phases::PhasesCtx::shared_daemon_replaced(&*c));
answers!(git_ctx, |c| wsp_account::projects::workspaces::git::GitCtx::viewed_marks(&*c));
answers!(review_ctx, |c| wsp_account::projects::workspaces::review::ReviewCtx::reviewers(&*c));
answers!(projects_ctx, |c| wsp_account::projects::projects::ProjectsCtx::has_state_path(&*c));
answers!(move_roads, |c| wsp_account::projects::projects::moves::MoveRoads::temp_dir(&*c));
answers!(account_landing_road, |c| wsp_account::projects::projects::add::LandingRoad::takes_files(&*c));
answers!(project_icons_ctx, |c| wsp_account::projects::images::ProjectIconsCtx::has_state_path(&*c));
answers!(agents_ctx, |c| wsp_threads::threads::agents::AgentsCtx::clock(&*c));
answers!(launch_ctx, |c| wsp_threads::threads::launch::LaunchCtx::closing(&*c));
answers!(threads_ctx, |c| wsp_threads::threads::threads::ThreadsCtx::ids(&*c));
answers!(turns_ctx, |c| wsp_threads::threads::turns::TurnsCtx::clock(&*c));
answers!(forks_ctx, |c| wsp_threads::threads::forks::ForksCtx::sessions(&*c));
answers!(conversations_ctx, |c| wsp_threads::threads::conversations::ConversationsCtx::bus(&*c));
answers!(sessions_ctx, |c| wsp_threads::threads::sessions::SessionsCtx::sessions(&*c));
answers!(builders_ctx, |c| wsp_machines::images::builders::BuildersCtx::roads(&*c));
answers!(golden_ctx, |c| wsp_machines::images::golden::GoldenCtx::roads(&*c));
answers!(image_ctx, |c| wsp_machines::images::image::ImageCtx::places(&*c));
answers!(golden_roads, |c| wsp_machines::images::engine::GoldenRoads::templates_of(&*c, backend()));
answers!(templates, |c| wsp_machines::images::engine::Templates::list(&*c).await);
answers!(usage_ctx, |c| wsp_account::account::usage::UsageCtx::has_door(&*c));
answers!(usage_reset_deps, |c| wsp_account::usage_reset::UsageResetDeps::uuid(&*c));
answers!(resets, |c| wsp_account::usage_reset::Resets::too_old(&*c));
answers!(status_ctx, |c| wsp_account::account::status::StatusCtx::wall_now(&*c));
answers!(preferences_ctx, |c| wsp_account::account::preferences::PreferencesCtx::home_dir(&*c));
answers!(server_icons, |c| wsp_account::account::preferences::ServerIcons::folder(&*c));
answers!(activity_ctx, |c| wsp_threads::activity::ActivityCtx::has_door(&*c));
answers!(daemon_ask, |c| wsp_threads::activity::DaemonAsk::close(&*c));
answers!(slates_area_ctx, |c| wsp_slates::account::slates::SlatesAreaCtx::home_dir(&*c));
answers!(pipe_area_context, |c| wsp_core::pipe::area::PipeAreaContext::live_ids(&*c));
answers!(core_ctx, |c| wsp_serve::runtime::core::CoreCtx::host_key(&*c));
answers!(public_ctx, |c| wsp_serve::runtime::public::PublicCtx::wall_now(&*c));
answers!(harnesses_ctx, |c| wsp_serve::runtime::public::HarnessesCtx::talks_to_tree_on(&*c, "w", None));
answers!(serve_status_ctx, |c| wsp_serve::runtime::public::StatusCtx::listed_for(&*c, None));
