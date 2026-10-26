// SPDX-License-Identifier: AGPL-3.0-only
//! kinds.ts:317-505: the three kind modules. Spec: spec/kinds_table.md.
use std::rc::{Rc, Weak};
use std::sync::Arc;

use futures::future::LocalBoxFuture;
use futures::FutureExt;
use futures::StreamExt;
use indexmap::IndexMap;

use wsp_adapters::port::{
    ExecFailure, ExecFailureKind, ExecOptions, ExecStream, ExecStreamFactory, SharedFuture,
    WriteOutcome,
};
use wsp_catalog::roads::GUEST_HOME;
use wsp_core::daemon_roots::write_daemon_roots_script;
use wsp_core::machine_exec::machine_exec_stream;
use wsp_core::types::internal::{
    EndThread, FolderAdmit, ImportLanded, ImportReport, KindModule, PortRoute, ServerValuesRoad,
};
use wsp_core::types::pending::{MachineExecOptions, ScopedRoad, TurnWaiting};
use wsp_core::types::wiring::{
    LiveWorkspace, Local, ProjectImportOptions, StageReport, WorkspaceRecord,
};
use wsp_engine::exec_detached::put::{GUEST_TMP, INLINE_EXEC_MS};
use wsp_engine::machine::{Machine, MachineBackend, Size};
use wsp_engine::place_machine::place_folder_backend;
use wsp_engine::project_state::project_state_key;
use wsp_wire::types::env::SCOPED_MCP_ARG;
use wsp_wire::types::js_number::{JsInt, JsNumber};
use wsp_wire::views::harness::McpServerSpec;
use wsp_wire::views::workspace::{DaemonReachView, ProjectView};
use wsp_wire::views::workspace_more::WorkspaceOrigin;
use wsp_wire::wire::machine_link::{place_forks_nowhere_line, place_serves_daemon_line};
use wsp_words::exit::{HostError, Kind};
use wsp_words::js::utf16::tail;
use wsp_words::project_path::{place_daemon_paths, roots_path_in};
use wsp_words::projects::place_login_not_root_line;
use wsp_words::thread_cgroup::thread_cgroup;
use wsp_words::words::computer::no_kind_line;

use super::KindsArea;
use crate::machines::area_error::AreaError;
use crate::machines::brokered::with_gh_door;
use crate::places::pane_ports::PANE_HOLD_MS;

/// kinds.ts:323. A fork at a provider or on a computer somebody joined.
pub struct CloudKind {
    pub(crate) area: Weak<KindsArea>,
}

/// kinds.ts:373. A folder on this computer.
pub struct LocalKind {
    pub(crate) area: Weak<KindsArea>,
}

/// kinds.ts:434. A project folder on a joined computer.
pub struct PlaceKind {
    pub(crate) area: Weak<KindsArea>,
}

/// The area a kind module answers for. The area holds its modules, so it is alive while any of them is asked.
fn area_of(area: &Weak<KindsArea>) -> Rc<KindsArea> {
    area.upgrade()
        .expect("a kind module answers for the area that holds it")
}

/// A read whose answer the module's signature cannot carry: its words go to the host's warn and the answer is none.
fn warned<T>(area: &KindsArea, read: Result<T, HostError>) -> Option<T> {
    read.map_err(|err| area.ctx.warn(&err.message)).ok()
}

/// A future that fails at once with these words.
fn failing<T: 'static>(err: HostError) -> Local<'static, T> {
    Box::pin(async move { Err(err) })
}

/// The words of a record that stands on no computer (kinds.ts `placeOf`).
fn no_computer(name: &str) -> HostError {
    HostError::new(format!(
        "{name} stands on no computer, which this host no longer holds"
    ))
}

/// The computer a place record stands on, read off the entry.
fn place_of_entry(entry: &LiveWorkspace) -> Result<String, HostError> {
    let record = entry.record.borrow();
    record
        .place
        .clone()
        .ok_or_else(|| no_computer(&record.name))
}

/// The wsp server a guest's agent is handed: `wsp mcp` on the machine's own PATH (agents-read.ts:19).
fn guest_wsp_mcp() -> McpServerSpec {
    McpServerSpec {
        command: "wsp".to_string(),
        args: vec!["mcp".to_string()],
        no_slate: None,
    }
}

/// A launch that refuses at once with these words, the answer of an `execStream` the kind has no road for.
fn refused(message: String) -> Arc<dyn ExecStreamFactory> {
    Arc::new(Refused(message))
}

struct Refused(String);

impl ExecStreamFactory for Refused {
    fn launch(&self, _command: &str, _options: ExecOptions) -> Arc<dyn ExecStream> {
        Arc::new(RefusedRun(self.0.clone()))
    }
}

struct RefusedRun(String);

impl RefusedRun {
    fn failure(&self) -> ExecFailure {
        ExecFailure {
            kind: ExecFailureKind::Other,
            message: self.0.clone(),
        }
    }
}

impl ExecStream for RefusedRun {
    fn lines(&self) -> futures::stream::LocalBoxStream<'static, Result<String, ExecFailure>> {
        futures::stream::once(futures::future::ready(Err(self.failure()))).boxed_local()
    }
    fn teardown(&self) {}
    fn kill(&self) {}
    fn write(&self, _line: String) -> LocalBoxFuture<'static, Result<WriteOutcome, ExecFailure>> {
        let failure = self.failure();
        Box::pin(async move { Err(failure) })
    }
    fn close_input(&self) {}
    fn exited(&self) -> SharedFuture<Option<i32>> {
        async { None::<i32> }.boxed_local().shared()
    }
}

impl KindModule for CloudKind {
    fn backend(&self, record: &WorkspaceRecord) -> Result<Arc<dyn MachineBackend>, HostError> {
        let area = area_of(&self.area);
        let Some(place) = record.place.as_deref() else {
            return Ok(area.ctx.own_backend());
        };
        if !area.ctx.has_place_door() {
            return Err(HostError::new(no_kind_line("place")));
        }
        area.ctx
            .door_backend_of(place)
            .ok_or_else(|| HostError::new(place_forks_nowhere_line(&area.ctx.door_name_of(place))))
    }
    fn exec_stream(
        &self,
        entry: &LiveWorkspace,
        opts: Option<MachineExecOptions>,
        waiting: Option<TurnWaiting>,
    ) -> Arc<dyn ExecStreamFactory> {
        let launch: Arc<dyn ExecStreamFactory> =
            machine_exec_stream(entry.machine.clone(), opts.unwrap_or_default(), waiting);
        with_gh_door(launch)
    }
    fn admit_folder(&self, _record: FolderAdmit) -> Option<Local<'static, ()>> {
        None
    }
    fn folder(&self, record: &WorkspaceRecord) -> Result<Option<String>, HostError> {
        Ok(record.sub.as_ref().map(|sub| sub.folder.clone()))
    }
    fn home(&self, _entry: &LiveWorkspace, agent_id: &str) -> String {
        let area = area_of(&self.area);
        warned(&area, area.cloud_home(agent_id)).unwrap_or_default()
    }
    fn home_dir(&self, _record: &WorkspaceRecord) -> Option<String> {
        Some(GUEST_HOME.to_string())
    }
    fn env(
        &self,
        entry: &LiveWorkspace,
        agent_id: Option<&str>,
        _home_of: &dyn Fn(&str) -> String,
    ) -> IndexMap<String, String> {
        let area = area_of(&self.area);
        let record = entry.record.borrow().clone();
        area.cloud_env(&record, agent_id)
    }
    fn login_stands(&self, entry: &LiveWorkspace, agent_id: &str) -> bool {
        area_of(&self.area).place_login_stands(entry, agent_id)
    }
    fn import(
        &self,
        entry: &LiveWorkspace,
        o: ProjectImportOptions,
        report: ImportReport,
    ) -> Local<'static, ImportLanded> {
        let area = area_of(&self.area);
        let asked = area.ctx.copy_import(entry, o, report);
        Box::pin(async move { asked.await.map_err(AreaError::into_host) })
    }
    fn roots(&self, entry: &LiveWorkspace, dests: &[String]) -> Local<'static, ()> {
        let area = area_of(&self.area);
        let asked = area.write_roots(entry, dests, None);
        Box::pin(async move { asked.await.map_err(AreaError::into_host) })
    }
    fn land_project(
        &self,
        entry: &LiveWorkspace,
        project: ProjectView,
        report: StageReport,
    ) -> Local<'static, ()> {
        let area = area_of(&self.area);
        let asked = area.clone_project(entry, project, report);
        Box::pin(async move { asked.await.map_err(AreaError::into_host) })
    }
    fn memory_key(
        &self,
        entry: &LiveWorkspace,
        agent_id: &str,
    ) -> Result<Option<String>, HostError> {
        let area = area_of(&self.area);
        let project_id = entry.record.borrow().project.clone();
        let project = area
            .ctx
            .project_held(&project_id)
            .map_err(AreaError::into_host)?;
        Ok(area.project_memory_key(agent_id, &project))
    }
    fn end_thread(
        &self,
        _entry: &LiveWorkspace,
        _thread_id: &str,
        _o: EndThread,
    ) -> Option<Local<'static, Option<String>>> {
        None
    }
    fn end_owed(
        &self,
        _entry: &LiveWorkspace,
        _thread_id: &str,
    ) -> Option<Local<'static, Option<Local<'static, ()>>>> {
        None
    }
    fn port_reach(
        &self,
        _entry: &LiveWorkspace,
        _port: u16,
        _standing: bool,
    ) -> Option<Local<'static, PortRoute>> {
        None
    }
    fn relayed(&self, _machine_id: Option<&str>) -> bool {
        true
    }
    fn wsp_mcp(&self, _entry: &LiveWorkspace) -> Option<McpServerSpec> {
        Some(guest_wsp_mcp())
    }
    fn turn_reach(&self, _entry: &LiveWorkspace) -> Option<Option<String>> {
        Some(None)
    }
    fn turn_road(&self) -> ScopedRoad {
        WorkspaceOrigin::Relayed
    }
    fn keeps_agents(&self) -> bool {
        false
    }
    fn server_values(&self) -> ServerValuesRoad {
        ServerValuesRoad::Environment
    }
    fn has_daemon(&self, entry: &LiveWorkspace) -> bool {
        let area = area_of(&self.area);
        !area.ctx.served_by_its_computer(entry) && entry.machine.offers().preview_url
    }
    fn shared_daemon(&self) -> bool {
        false
    }
    fn daemon_road(&self, entry: &LiveWorkspace) -> Local<'static, DaemonReachView> {
        let area = area_of(&self.area);
        if area.ctx.served_by_its_computer(entry) {
            let name = entry.record.borrow().name.clone();
            return failing(HostError::new(place_serves_daemon_line(
                &name,
                &area.ctx.computer_of(entry),
            )));
        }
        let asked = area.cloud_road(entry);
        Box::pin(async move { asked.await.map_err(AreaError::into_host) })
    }
    fn scratch(&self, _entry: &LiveWorkspace) -> String {
        GUEST_TMP.to_string()
    }
    fn dropped(&self, _entry: &LiveWorkspace) -> Local<'static, ()> {
        Box::pin(async { Ok(()) })
    }
    fn daemon_version(&self, entry: &LiveWorkspace) -> Local<'static, Option<u32>> {
        let area = area_of(&self.area);
        let asked = area.ctx.hello_version(entry);
        Box::pin(async move { Ok(asked.await) })
    }
    fn restart_daemon(&self, _entry: &LiveWorkspace) -> Option<Local<'static, ()>> {
        None
    }
    fn deploy_daemon(&self, entry: &LiveWorkspace) -> Option<Local<'static, Option<String>>> {
        let area = area_of(&self.area);
        if !area.ctx.has_deploy() {
            return None;
        }
        let asked = area.cloud_deploy(entry);
        Some(Box::pin(async move {
            asked.await.map_err(AreaError::into_host)?;
            Ok(None)
        }))
    }
}

impl KindModule for LocalKind {
    fn backend(&self, _record: &WorkspaceRecord) -> Result<Arc<dyn MachineBackend>, HostError> {
        area_of(&self.area)
            .ctx
            .local_backend()
            .ok_or_else(|| HostError::new(no_kind_line("local")))
    }
    fn exec_stream(
        &self,
        _entry: &LiveWorkspace,
        _opts: Option<MachineExecOptions>,
        _waiting: Option<TurnWaiting>,
    ) -> Arc<dyn ExecStreamFactory> {
        refused("a turn on this computer cannot start: the host has not handed the kinds area its local exec road".to_string())
    }
    fn admit_folder(&self, _record: FolderAdmit) -> Option<Local<'static, ()>> {
        None
    }
    fn folder(&self, record: &WorkspaceRecord) -> Result<Option<String>, HostError> {
        area_of(&self.area)
            .ctx
            .checkout_of(record)
            .map(Some)
            .map_err(AreaError::into_host)
    }
    fn home(&self, _entry: &LiveWorkspace, agent_id: &str) -> String {
        area_of(&self.area).ctx.local_home(agent_id)
    }
    fn home_dir(&self, _record: &WorkspaceRecord) -> Option<String> {
        Some(area_of(&self.area).ctx.local_home_dir())
    }
    fn env(
        &self,
        _entry: &LiveWorkspace,
        _agent_id: Option<&str>,
        _home_of: &dyn Fn(&str) -> String,
    ) -> IndexMap<String, String> {
        area_of(&self.area).ctx.local_env()
    }
    fn login_stands(&self, _entry: &LiveWorkspace, agent_id: &str) -> bool {
        let area = area_of(&self.area);
        area.shared_login_under(&area.ctx.local_home(agent_id), agent_id)
    }
    fn import(
        &self,
        _entry: &LiveWorkspace,
        o: ProjectImportOptions,
        report: ImportReport,
    ) -> Local<'static, ImportLanded> {
        let area = area_of(&self.area);
        Box::pin(async move {
            area.register_import(&o, &report)
                .await
                .map_err(AreaError::into_host)
        })
    }
    fn roots(&self, entry: &LiveWorkspace, dests: &[String]) -> Local<'static, ()> {
        let area = area_of(&self.area);
        let roots_path = area.ctx.local_roots_path();
        let asked = area.write_roots(entry, dests, roots_path.as_deref());
        Box::pin(async move { asked.await.map_err(AreaError::into_host) })
    }
    fn land_project(
        &self,
        _entry: &LiveWorkspace,
        _project: ProjectView,
        _report: StageReport,
    ) -> Local<'static, ()> {
        Box::pin(async { Ok(()) })
    }
    fn memory_key(
        &self,
        entry: &LiveWorkspace,
        agent_id: &str,
    ) -> Result<Option<String>, HostError> {
        let area = area_of(&self.area);
        let (project_id, made_tree) = {
            let record = entry.record.borrow();
            (
                record.project.clone(),
                record
                    .worktree
                    .as_ref()
                    .filter(|tree| tree.made && tree.gone.is_none())
                    .map(|tree| tree.path.clone()),
            )
        };
        let project = area
            .ctx
            .project_held(&project_id)
            .map_err(AreaError::into_host)?;
        if let Some(path) = made_tree {
            return Ok(project_state_key(agent_id, &path));
        }
        Ok(match project.git.as_ref().map(|git| git.top.clone()) {
            Some(top) if top != project.path => project_state_key(agent_id, &top),
            _ => area.project_memory_key(agent_id, &project),
        })
    }
    fn end_thread(
        &self,
        _entry: &LiveWorkspace,
        _thread_id: &str,
        _o: EndThread,
    ) -> Option<Local<'static, Option<String>>> {
        None
    }
    fn end_owed(
        &self,
        _entry: &LiveWorkspace,
        _thread_id: &str,
    ) -> Option<Local<'static, Option<Local<'static, ()>>>> {
        None
    }
    fn port_reach(
        &self,
        _entry: &LiveWorkspace,
        _port: u16,
        _standing: bool,
    ) -> Option<Local<'static, PortRoute>> {
        None
    }
    fn relayed(&self, _machine_id: Option<&str>) -> bool {
        false
    }
    fn wsp_mcp(&self, _entry: &LiveWorkspace) -> Option<McpServerSpec> {
        area_of(&self.area).ctx.agents_wsp_mcp().map(|mut spec| {
            spec.args.push(SCOPED_MCP_ARG.to_string());
            spec
        })
    }
    fn turn_reach(&self, _entry: &LiveWorkspace) -> Option<Option<String>> {
        match area_of(&self.area).ctx.agents_here_url() {
            Some(url) if !url.is_empty() => Some(Some(url)),
            _ => None,
        }
    }
    fn turn_road(&self) -> ScopedRoad {
        WorkspaceOrigin::Here
    }
    fn keeps_agents(&self) -> bool {
        true
    }
    fn server_values(&self) -> ServerValuesRoad {
        ServerValuesRoad::File
    }
    fn has_daemon(&self, _entry: &LiveWorkspace) -> bool {
        area_of(&self.area).ctx.has_local_daemon_road()
    }
    fn shared_daemon(&self) -> bool {
        true
    }
    fn daemon_road(&self, _entry: &LiveWorkspace) -> Local<'static, DaemonReachView> {
        let area = area_of(&self.area);
        Box::pin(async move { area.local_road().await.map_err(AreaError::into_host) })
    }
    fn scratch(&self, _entry: &LiveWorkspace) -> String {
        area_of(&self.area).ctx.local_scratch()
    }
    fn dropped(&self, _entry: &LiveWorkspace) -> Local<'static, ()> {
        Box::pin(async { Ok(()) })
    }
    fn daemon_version(&self, _entry: &LiveWorkspace) -> Local<'static, Option<u32>> {
        let ctx = Rc::clone(&area_of(&self.area).ctx);
        Box::pin(async move {
            match ctx.here_daemon_version().await {
                None => Ok(None),
                Some(read) => read.map_err(AreaError::into_host),
            }
        })
    }
    fn restart_daemon(&self, _entry: &LiveWorkspace) -> Option<Local<'static, ()>> {
        area_of(&self.area).ctx.has_local_restart().then(|| {
            failing(HostError::new(
                "this host cannot restart its own daemon from here yet",
            ))
        })
    }
    fn deploy_daemon(&self, _entry: &LiveWorkspace) -> Option<Local<'static, Option<String>>> {
        None
    }
}

impl KindModule for PlaceKind {
    fn backend(&self, record: &WorkspaceRecord) -> Result<Arc<dyn MachineBackend>, HostError> {
        let area = area_of(&self.area);
        let at = area.place_of(record)?;
        let machine = at.machine.clone();
        let shape = Size {
            cpu: f64::from(at.cpu),
            mem_mb: at.mem_mb as f64,
        };
        #[allow(clippy::arc_with_non_send_sync)]
        let machine_of: Arc<dyn Fn() -> Arc<dyn Machine>> =
            Arc::new(move || -> Arc<dyn Machine> { machine.clone() });
        Ok(place_folder_backend(machine_of, shape))
    }
    fn exec_stream(
        &self,
        entry: &LiveWorkspace,
        opts: Option<MachineExecOptions>,
        waiting: Option<TurnWaiting>,
    ) -> Arc<dyn ExecStreamFactory> {
        let area = area_of(&self.area);
        let record = entry.record.borrow().clone();
        let at = match area.place_of(&record) {
            Ok(at) => at,
            Err(err) => return refused(err.message),
        };
        let paths = place_daemon_paths(&at.home);
        let mut o = opts.unwrap_or_default();
        o.run_dir.get_or_insert(paths.run_dir);
        if let Some(thread) = o.thread.clone() {
            o.launch_on = Some(
                at.machine
                    .in_cgroup(&thread_cgroup(&thread), Some(&paths.guest_bin)),
            );
        }
        let launch: Arc<dyn ExecStreamFactory> =
            machine_exec_stream(entry.machine.clone(), o, waiting);
        launch
    }
    fn admit_folder(&self, record: FolderAdmit) -> Option<Local<'static, ()>> {
        let area = area_of(&self.area);
        let Some(place) = record.place.clone() else {
            return Some(failing(no_computer(&record.name)));
        };
        let name = area.ctx.door_name_of(&place);
        let ctx = Rc::clone(&area.ctx);
        Some(Box::pin(async move {
            let user = ctx
                .door_login_user(&place)
                .await
                .map_err(AreaError::into_host)?;
            if user != "root" {
                return Err(HostError::with_kind(
                    place_login_not_root_line(&name, &user),
                    Kind::Usage,
                ));
            }
            Ok(())
        }))
    }
    fn folder(&self, record: &WorkspaceRecord) -> Result<Option<String>, HostError> {
        area_of(&self.area)
            .ctx
            .checkout_of(record)
            .map(Some)
            .map_err(AreaError::into_host)
    }
    fn home(&self, entry: &LiveWorkspace, agent_id: &str) -> String {
        let area = area_of(&self.area);
        let record = entry.record.borrow().clone();
        let answer = area
            .place_of(&record)
            .and_then(|at| area.place_agent_home(record.place.as_deref(), &at.home, agent_id));
        warned(&area, answer).unwrap_or_default()
    }
    fn home_dir(&self, record: &WorkspaceRecord) -> Option<String> {
        let area = area_of(&self.area);
        warned(&area, area.place_of(record).map(|at| at.home))
    }
    fn env(
        &self,
        entry: &LiveWorkspace,
        _agent_id: Option<&str>,
        home_of: &dyn Fn(&str) -> String,
    ) -> IndexMap<String, String> {
        let area = area_of(&self.area);
        warned(&area, area.place_env(entry, home_of)).unwrap_or_default()
    }
    fn login_stands(&self, entry: &LiveWorkspace, agent_id: &str) -> bool {
        area_of(&self.area).place_login_stands(entry, agent_id)
    }
    fn import(
        &self,
        entry: &LiveWorkspace,
        _o: ProjectImportOptions,
        _report: ImportReport,
    ) -> Local<'static, ImportLanded> {
        let area = area_of(&self.area);
        let computer = area.ctx.computer_of(entry);
        let name = entry.record.borrow().name.clone();
        failing(HostError::with_kind(
            format!("{name} is a project folder on {computer}; a folder from this computer goes there as a project of its own, with wsp add <folder> --on {computer}"),
            Kind::Usage,
        ))
    }
    fn roots(&self, entry: &LiveWorkspace, dests: &[String]) -> Local<'static, ()> {
        let area = area_of(&self.area);
        let record = entry.record.borrow().clone();
        let at = match area.place_of(&record) {
            Ok(at) => at,
            Err(err) => return failing(err),
        };
        let Some(place) = record.place.clone() else {
            return failing(no_computer(&record.name));
        };
        let script = write_daemon_roots_script(dests, Some(&roots_path_in(&at.home)));
        let last = dests
            .last()
            .cloned()
            .unwrap_or_else(|| "undefined".to_string());
        let ctx = Rc::clone(&area.ctx);
        Box::pin(async move {
            let wrote = ctx
                .door_exec(&place, &script, INLINE_EXEC_MS)
                .await
                .map_err(AreaError::into_host)?;
            if wrote.exit_code != JsInt::from(0) {
                let name = ctx.door_name_of(&place);
                return Err(HostError::new(format!(
                    "could not make {last} browsable on {name}: {}",
                    tail(&wrote.stderr, 200)
                )));
            }
            Ok(())
        })
    }
    fn land_project(
        &self,
        _entry: &LiveWorkspace,
        _project: ProjectView,
        _report: StageReport,
    ) -> Local<'static, ()> {
        Box::pin(async { Ok(()) })
    }
    fn memory_key(
        &self,
        entry: &LiveWorkspace,
        agent_id: &str,
    ) -> Result<Option<String>, HostError> {
        let area = area_of(&self.area);
        let project_id = entry.record.borrow().project.clone();
        let project = area
            .ctx
            .project_held(&project_id)
            .map_err(AreaError::into_host)?;
        Ok(area.project_memory_key(agent_id, &project))
    }
    fn end_thread(
        &self,
        entry: &LiveWorkspace,
        thread_id: &str,
        o: EndThread,
    ) -> Option<Local<'static, Option<String>>> {
        let area = area_of(&self.area);
        let place = match place_of_entry(entry) {
            Ok(place) => place,
            Err(err) => return Some(failing(err)),
        };
        let ctx = Rc::clone(&area.ctx);
        let thread = thread_id.to_string();
        Some(Box::pin(async move {
            ctx.end_thread(&place, &thread, o)
                .await
                .map_err(AreaError::into_host)
        }))
    }
    fn end_owed(
        &self,
        entry: &LiveWorkspace,
        thread_id: &str,
    ) -> Option<Local<'static, Option<Local<'static, ()>>>> {
        let area = area_of(&self.area);
        let place = match place_of_entry(entry) {
            Ok(place) => place,
            Err(err) => return Some(failing(err)),
        };
        let ctx = Rc::clone(&area.ctx);
        let thread = thread_id.to_string();
        Some(Box::pin(async move {
            let Some(owed) = ctx
                .end_owed(&place, &thread)
                .await
                .map_err(AreaError::into_host)?
            else {
                return Ok(None);
            };
            let paid: Local<'static, ()> = Box::pin(async move {
                owed.paid.await;
                Ok(())
            });
            Ok(Some(paid))
        }))
    }
    fn port_reach(
        &self,
        entry: &LiveWorkspace,
        port: u16,
        standing: bool,
    ) -> Option<Local<'static, PortRoute>> {
        let area = area_of(&self.area);
        let place = match place_of_entry(entry) {
            Ok(place) => place,
            Err(err) => return Some(failing(err)),
        };
        let pane = {
            let record = entry.record.borrow();
            (record.id.clone(), record.name.clone())
        };
        let ctx = Rc::clone(&area.ctx);
        Some(Box::pin(async move {
            let local_port = ctx
                .door_forward(&place, port, Some(pane), standing)
                .await
                .map_err(AreaError::into_host)?;
            Ok(PortRoute {
                url: format!("http://localhost:{local_port}/"),
                expires_at: JsNumber(ctx.now() + PANE_HOLD_MS as f64),
            })
        }))
    }
    fn relayed(&self, _machine_id: Option<&str>) -> bool {
        true
    }
    fn wsp_mcp(&self, entry: &LiveWorkspace) -> Option<McpServerSpec> {
        let area = area_of(&self.area);
        let place = place_of_entry(entry).ok()?;
        let tools = area
            .ctx
            .door_folder_computer(&place)
            .is_some_and(|at| at.tools);
        tools.then(guest_wsp_mcp)
    }
    fn turn_reach(&self, _entry: &LiveWorkspace) -> Option<Option<String>> {
        Some(None)
    }
    fn turn_road(&self) -> ScopedRoad {
        WorkspaceOrigin::Relayed
    }
    fn keeps_agents(&self) -> bool {
        true
    }
    fn server_values(&self) -> ServerValuesRoad {
        ServerValuesRoad::Launch
    }
    fn has_daemon(&self, _entry: &LiveWorkspace) -> bool {
        false
    }
    fn shared_daemon(&self) -> bool {
        true
    }
    fn daemon_road(&self, entry: &LiveWorkspace) -> Local<'static, DaemonReachView> {
        let area = area_of(&self.area);
        let name = entry.record.borrow().name.clone();
        failing(HostError::new(place_serves_daemon_line(
            &name,
            &area.ctx.computer_of(entry),
        )))
    }
    fn scratch(&self, entry: &LiveWorkspace) -> String {
        let area = area_of(&self.area);
        let record = entry.record.borrow().clone();
        warned(
            &area,
            area.place_of(&record)
                .map(|at| place_daemon_paths(&at.home).put_dir),
        )
        .unwrap_or_default()
    }
    fn dropped(&self, _entry: &LiveWorkspace) -> Local<'static, ()> {
        Box::pin(async { Ok(()) })
    }
    fn daemon_version(&self, entry: &LiveWorkspace) -> Local<'static, Option<u32>> {
        let area = area_of(&self.area);
        let place = place_of_entry(entry).ok();
        let ctx = Rc::clone(&area.ctx);
        Box::pin(async move {
            Ok(match place {
                Some(place) => ctx.door_report_daemon_version(&place).await,
                None => None,
            })
        })
    }
    fn restart_daemon(&self, _entry: &LiveWorkspace) -> Option<Local<'static, ()>> {
        None
    }
    fn deploy_daemon(&self, _entry: &LiveWorkspace) -> Option<Local<'static, Option<String>>> {
        None
    }
}
