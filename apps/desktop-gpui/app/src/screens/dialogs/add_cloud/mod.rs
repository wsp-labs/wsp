// SPDX-License-Identifier: AGPL-3.0-only
//! Add a cloud provider (`settings/add/AddCloudDialog.tsx`, `cloudFlow.ts`, `cloudKey.ts`): one step dialog. The key
//! step says where the key is made, what it needs and what the machines cost, and checks it with the provider on a
//! paste or Enter; the picks are the host's own setup screens as pick lists; the summary names the sign-ins on the
//! computer the host runs on, which cloud machines use through wsp; then the image builds, and the ready page starts
//! the first thread. The flow's state is [`flow::Flow`]; this view sends what it asks and hands the answers back.

pub mod calls;
pub mod flow;
pub use super::sign_in;
mod view;
pub mod words;

use std::collections::BTreeMap;

use futures::channel::mpsc;
use futures::StreamExt as _;
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::{App, AppContext as _, BorrowAppContext as _, Context, Entity, SharedString, Subscription, Task, Window};
use serde_json::{json, Value};
use wsp_host_client::Failure;

use super::layer::{self, Closes};
use super::picks::{PickListState, AS_FOUND, MOST_USED};
use super::ADD_CLOUD;
use crate::local_store::LocalStore;
use crate::screens::app_shell::{navigate, Route};
use crate::screens::composer::store;
use crate::screens::notices::{self, words::OPEN};
use crate::screens::palette::overlays;
use crate::screens::sidebar::looks::place_name;
use crate::screens::Host;
use crate::ui::{self, Notice, NoticeAction};
use flow::{Ask, Flow, Step};

/// `READY_OWED_KEY`: the build each cloud's ready page is owed for, by provider: one the dialog stood on and whose ready
/// page has not shown.
const READY_OWED_KEY: &str = "wsp:cloud-ready-owed";

fn ready_owed(provider: &str, cx: &App) -> Option<String> {
    cx.try_global::<LocalStore>()?.get(READY_OWED_KEY)?.get(provider)?.as_str().map(str::to_owned)
}

/// `oweReady`: the dialog stood on the build `job`, owing its ready page; none once the page has shown. A window that
/// keeps nothing sends the seal's Open to the cloud's page.
fn owe_ready(provider: &str, job: Option<&str>, cx: &mut App) {
    if ready_owed(provider, cx).as_deref() == job {
        return;
    }
    let Some(store) = cx.try_global::<LocalStore>() else { return };
    let mut held = store.get(READY_OWED_KEY).filter(|v| v.is_object()).cloned().unwrap_or_else(|| json!({}));
    match job {
        Some(job) => held[provider] = json!(job),
        None => _ = held.as_object_mut().map(|o| o.remove(provider)),
    }
    _ = cx.global_mut::<LocalStore>().set(READY_OWED_KEY, held);
}

/// The host and the places the window's sidebar holds, once the shell is connected.
fn connected(window: &Window, cx: &App) -> Option<(Host, Vec<Value>)> {
    let overlays = overlays::of(window, cx)?;
    let overlays = overlays.read(cx);
    let places = overlays.sidebar.as_ref().and_then(|s| s.upgrade()).map(|s| s.read(cx).around().places.clone()).unwrap_or_default();
    Some((overlays.host.clone()?, places))
}

/// `openReadyOwed`: the ready page of `provider`'s cloud, where the build `job` still owes it and the cloud is listed.
fn open_ready_owed(provider: &str, job: &str, window: &mut Window, cx: &mut App) -> bool {
    let Some((host, places)) = connected(window, cx) else { return false };
    if ready_owed(provider, cx).as_deref() != Some(job) || !places.iter().any(|p| p.get("id").and_then(Value::as_str) == Some(provider)) {
        return false;
    }
    let mut flow = Flow::new(Some(provider));
    flow.step = Step::Ready;
    open_on(Some(&host), flow, window, cx);
    true
}

/// `openSealed`: a sealed build's Open, Add a cloud provider's ready page where that build still owes it, else the
/// computer's page.
pub fn open_sealed(place: Option<String>, job: String) -> NoticeAction {
    NoticeAction::new(OPEN, move |window, cx| {
        if !place.as_deref().is_some_and(|p| open_ready_owed(p, &job, window, cx)) {
            let at = place.as_deref().map_or_else(|| "settings:computers".to_owned(), |p| format!("settings:computers:{p}"));
            navigate(Route::Screen(at.into()), cx);
        }
    })
}

/// `saySealed`'s notice for the build `job` (`InitJob`): its computer by the name `places` lists, else the name the
/// job carries, and its image's words.
pub fn sealed(job: &Value, places: &[Value]) -> Notice {
    let at = job.pointer("/place/id").and_then(Value::as_str);
    let name = places
        .iter()
        .find(|p| at.is_some() && p.get("id").and_then(Value::as_str) == at)
        .map(place_name)
        .or_else(|| job.pointer("/place/name").and_then(Value::as_str).map(str::to_owned));
    let sealed = notices::words::image_sealed(job.pointer("/golden/version").and_then(Value::as_u64));
    let Some(name) = name else { return Notice::done(sealed) };
    let id = job.get("id").and_then(Value::as_str).unwrap_or_default().to_owned();
    Notice::done(notices::words::setup_ready(&name)).detail(format!("{sealed}.")).place(name).action(open_sealed(at.map(str::to_owned), id))
}

/// `cloudBuildOnScreen`: Add a cloud provider stands on `place`'s build, whose own heading says how it ended.
pub fn build_on_screen(place: &str, cx: &App) -> bool {
    layer::view(ADD_CLOUD, cx)
        .and_then(|v| v.downcast::<AddCloud>().ok())
        .is_some_and(|v| v.read(cx).flow.provider == place && v.read(cx).flow.step == Step::Building)
}

/// Opens Add a cloud provider on the setup that stands for a cloud, else where the host's setup says.
pub fn open(host: &Host, window: &mut Window, cx: &mut App) {
    open_on(Some(host), Flow::new(None), window, cx);
}

/// Opens the dialog on `flow` over `host`; with none, on a flow staged from fixture answers, as a shot or a test draws
/// it. The key field takes the keys once the dialog stands, as the web's `autoFocus` does.
pub fn open_on(host: Option<&Host>, flow: Flow, window: &mut Window, cx: &mut App) -> Entity<AddCloud> {
    let view = cx.new(|cx| AddCloud::new(host.cloned(), flow, window, cx));
    let weak = view.downgrade();
    let on_close: layer::OnClose = std::rc::Rc::new(move |window, cx| {
        _ = weak.update(cx, |this, cx| this.closing(window, cx));
    });
    layer::open(ADD_CLOUD, view.clone().into(), Closes::Outside, Some(on_close), window, cx);
    view.update(cx, |this, cx| {
        if this.flow.step == Step::Key {
            this.key_input.update(cx, |s, cx| s.focus(window, cx));
        }
    });
    view
}

pub struct AddCloud {
    host: Option<Host>,
    pub flow: Flow,
    key_input: Entity<InputState>,
    placeholder: SharedString,
    /// The paste chord was pressed in the key field: its change is checked at once.
    pasted: bool,
    lists: BTreeMap<String, Entity<PickListState>>,
    /// The build rows turned open or shut by hand.
    turned: BTreeMap<String, bool>,
    /// A sign-in's field for its code or its key, by row.
    fields: BTreeMap<String, Entity<InputState>>,
    _subs: Vec<Subscription>,
    _follow: Task<()>,
    _reads: Task<()>,
}

impl AddCloud {
    fn new(host: Option<Host>, flow: Flow, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let key_input = cx.new(|cx| {
            let mut s = InputState::new(window, cx);
            s.toggle_masked(window, cx);
            s
        });
        if !flow.key.is_empty() {
            key_input.update(cx, |s, cx| s.set_value(flow.key.clone(), window, cx));
        }
        let subs = vec![cx.subscribe_in(&key_input, window, Self::on_key_input)];
        let mut this = AddCloud {
            host,
            flow,
            key_input,
            placeholder: SharedString::default(),
            pasted: false,
            lists: BTreeMap::new(),
            turned: BTreeMap::new(),
            fields: BTreeMap::new(),
            _subs: subs,
            _follow: Task::ready(()),
            _reads: Task::ready(()),
        };
        this.follow(window, cx);
        this.read(cx);
        this
    }

    fn on_key_input(&mut self, input: &Entity<InputState>, event: &InputEvent, _: &mut Window, cx: &mut Context<Self>) {
        let text = input.read(cx).value().to_string();
        let ask = match event {
            InputEvent::Change if text != self.flow.key => {
                let now = std::mem::take(&mut self.pasted);
                self.flow.type_key(&text, now)
            }
            InputEvent::PressEnter { .. } => self.flow.type_key(&text, true),
            _ => return,
        };
        if let Some(ask) = ask {
            self.send(ask, cx);
        }
        cx.notify();
    }

    /// The reads the dialog opens on: the setup, the computer the host runs on, its agents and its GitHub login.
    fn read(&mut self, cx: &mut Context<Self>) {
        let Some(host) = self.host.clone() else { return };
        let setup = host.run(|c| async move { calls::setup(&c).await });
        let here = host.run(|c| async move { calls::here(&c).await });
        let agents = host.run(|c| async move { calls::agents(&c).await });
        let github = host.run(|c| async move { calls::github(&c).await });
        self._reads = cx.spawn(async move |this, cx| {
            let setup = setup.await;
            _ = this.update(cx, |this, cx| {
                match setup {
                    Ok(setup) => this.flow.on_setup(setup),
                    Err(f) => this.flow.on_setup_refused(f),
                }
                cx.notify();
            });
            let here = here.await;
            _ = this.update(cx, |this, cx| {
                this.flow.here = here.unwrap_or_default();
                cx.notify();
            });
            let github = github.await.ok().flatten();
            let agents = agents.await.ok();
            _ = this.update(cx, |this, cx| {
                this.flow.github = github;
                this.flow.report = agents;
                cx.notify();
            });
        });
    }

    /// The agents on the computer the host runs on, read again after one changed.
    fn read_agents(&mut self, cx: &mut Context<Self>) {
        let Some(host) = self.host.clone() else { return };
        let agents = host.run(|c| async move { calls::agents(&c).await });
        cx.spawn(async move |this, cx| {
            if let Ok(agents) = agents.await {
                _ = this.update(cx, |this, cx| {
                    this.flow.report = Some(agents);
                    cx.notify();
                });
            }
        })
        .detach();
    }

    /// Follows the host's pushes for as long as the dialog stands: the job, the agents here, the sign-ins' steps.
    fn follow(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(host) = self.host.clone() else { return };
        let (tx, mut rx) = mpsc::unbounded();
        let following = calls::follow(&host, tx);
        self._follow = cx.spawn_in(window, async move |this, cx| {
            let _following = following;
            while let Some(pushed) = rx.next().await {
                let alive = this.update(cx, |this, cx| {
                    this.on_pushed(pushed, cx);
                    cx.notify();
                });
                if alive.is_err() {
                    break;
                }
            }
        });
    }

    /// One push the host sent, as the flow reads it.
    pub fn on_pushed(&mut self, pushed: calls::Pushed, cx: &mut Context<Self>) {
        match pushed {
            calls::Pushed::Job(job) => self.flow.on_job(*job),
            calls::Pushed::Agents => self.read_agents(cx),
            calls::Pushed::SignIn(heard) => {
                if self.flow.sign_ins.hear(&heard) {
                    self.read_agents(cx);
                }
            }
        }
    }

    /// Sends one call the flow asked for and hands its answer back. With no host, as a staged flow has, nothing goes.
    fn send(&mut self, ask: Ask, cx: &mut Context<Self>) {
        let Some(host) = self.host.clone() else { return };
        match ask {
            Ask::Check { provider, key } => {
                let typed = key.clone();
                let answer = host.run(move |c| async move { calls::check_key(&c, provider, key).await });
                cx.spawn(async move |this, cx| {
                    let answer = answer.await;
                    _ = this.update(cx, |this, cx| {
                        this.flow.on_check(&typed, answer);
                        cx.notify();
                    });
                })
                .detach();
            }
            Ask::Draft { at, ticks } => {
                host.run(move |c| async move { calls::draft(&c, at, ticks).await }).detach_from(cx);
            }
            Ask::StepTo { at } => {
                let answer = host.run(move |c| async move { calls::step(&c, at).await });
                self.land(answer, None, cx);
            }
            Ask::Answer { screen, ticks, answers, then } => {
                let answers = serde_json::to_value(answers).ok();
                let answer = host.run(move |c| async move { calls::answer(&c, screen, ticks, answers).await });
                self.land(answer, Some(then), cx);
            }
            Ask::Build { first_launch, on } => {
                let answer = host.run(move |c| async move {
                    if let Some(ticks) = first_launch {
                        calls::answer(&c, words::FIRST_LAUNCH_SCREEN.into(), ticks, None).await?;
                    }
                    calls::build(&c, on).await
                });
                self.land(answer, Some(Step::Building), cx);
            }
            Ask::StartOver { on, then } => {
                let answer = host.run(move |c| async move { calls::start(&c, on).await.map(drop) });
                self.land(answer, Some(then), cx);
            }
            Ask::KeyOn { provider, key, start, then } => {
                let on = provider.clone();
                let saved = key.map(|key| host.run(move |c| async move { calls::save_key(&c, provider, key).await }));
                let started = host.clone();
                cx.spawn(async move |this, cx| {
                    if let Some(saved) = saved {
                        match saved.await {
                            Ok(setup) => _ = this.update(cx, |this, _| this.flow.on_key_saved(setup)),
                            Err(f) => {
                                _ = this.update(cx, |this, cx| {
                                    this.flow.on_key_on(then, Err(f));
                                    cx.notify();
                                });
                                return;
                            }
                        }
                    }
                    let answer =
                        if start { started.run(move |c| async move { calls::start(&c, on).await.map(drop) }).await } else { Ok(()) };
                    _ = this.update(cx, |this, cx| {
                        this.flow.on_key_on(then, answer);
                        cx.notify();
                    });
                })
                .detach();
            }
        }
    }

    /// A pick, the build or Start over came back.
    fn land(
        &mut self,
        answer: impl std::future::Future<Output = Result<(), Failure>> + 'static,
        then: Option<Step>,
        cx: &mut Context<Self>,
    ) {
        cx.spawn(async move |this, cx| {
            let answer = answer.await;
            _ = this.update(cx, |this, cx| {
                this.flow.on_recipe(then, answer);
                cx.notify();
            });
        })
        .detach();
    }

    /// Sends one call a sign-in asked for.
    fn send_sign_in(&mut self, ask: sign_in::Ask, cx: &mut Context<Self>) {
        let Some(host) = self.host.clone() else { return };
        match ask {
            sign_in::Ask::Begin { row, agent } => {
                let begun = host.run(move |c| async move { sign_in::begin(&c, calls::HERE.into(), agent).await });
                cx.spawn(async move |this, cx| {
                    let begun = begun.await;
                    let id = begun.as_ref().ok().cloned();
                    let gone = this.update(cx, |this, cx| {
                        if let Some(stop) = this.flow.sign_ins.on_begun(&row, begun) {
                            this.send_sign_in(stop, cx);
                        }
                        cx.notify();
                    });
                    if let (Err(_), Some(id)) = (gone, id) {
                        _ = host.run(move |c| async move { sign_in::stop(&c, id).await }).await;
                    }
                })
                .detach();
            }
            sign_in::Ask::Stop { id } => host.run(move |c| async move { sign_in::stop(&c, id).await }).detach_from(cx),
            sign_in::Ask::Code { row, id, code } => {
                let sent = host.run(move |c| async move { sign_in::code(&c, id, code).await });
                cx.spawn(async move |this, cx| {
                    let sent = sent.await;
                    _ = this.update(cx, |this, cx| {
                        this.flow.sign_ins.on_code(&row, sent);
                        cx.notify();
                    });
                })
                .detach();
            }
            sign_in::Ask::Key { row, agent, key, token } => {
                let saved = host.run(move |c| async move { sign_in::key(&c, agent, key, token).await });
                cx.spawn(async move |this, cx| {
                    let saved = saved.await;
                    let landed = saved.is_ok();
                    _ = this.update(cx, |this, cx| {
                        this.flow.sign_ins.on_saved(&row, saved);
                        if landed {
                            this.fields.remove(&row);
                            this.read_agents(cx);
                        }
                        cx.notify();
                    });
                })
                .detach();
            }
        }
    }

    /// The pick list's state for a screen, made the first time it shows.
    fn list(&mut self, screen: &str, window: &mut Window, cx: &mut Context<Self>) -> Entity<PickListState> {
        if let Some(list) = self.lists.get(screen) {
            return list.clone();
        }
        let own = if screen == "tools" { MOST_USED } else { AS_FOUND };
        let list = cx.new(|cx| PickListState::new(Some(own), window, cx));
        self.lists.insert(screen.to_owned(), list.clone());
        list
    }

    /// A sign-in's field, made the first time its row asks for one.
    fn field(&mut self, row: &str, window: &mut Window, cx: &mut Context<Self>) -> Entity<InputState> {
        if let Some(field) = self.fields.get(row) {
            return field.clone();
        }
        let field = cx.new(|cx| InputState::new(window, cx));
        self.fields.insert(row.to_owned(), field.clone());
        field
    }

    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.closing(window, cx);
        layer::close(ADD_CLOUD, window, cx);
        cx.notify();
    }

    /// However the dialog closes: a build that sealed while it stood on it says so, since its heading stood in for the
    /// host's notice.
    fn closing(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for stop in self.flow.sign_ins.end() {
            self.send_sign_in(stop, cx);
        }
        if self.flow.step != Step::Building || !self.flow.sealed() {
            return;
        }
        let Some(job) = self.flow.mine().and_then(|j| serde_json::to_value(j).ok()) else { return };
        let places = connected(window, cx).map(|(_, p)| p).unwrap_or_default();
        ui::notify(cx, sealed(&job, &places));
    }

    /// A close before the seal leaves the ready page to the seal's Open; the page shown pays it.
    fn keep_owed(&mut self, cx: &mut Context<Self>) {
        let Some(job) = self.flow.mine().map(|j| j.id.clone()) else { return };
        match self.flow.step {
            Step::Building => owe_ready(&self.flow.provider, Some(&job), cx),
            Step::Ready => owe_ready(&self.flow.provider, None, cx),
            _ => {}
        }
    }

    /// Start a thread: a new thread on the computer the host runs on, its composer holding the first ask with the
    /// caret at its end, on the workspace a thread last started on where it is that computer's, else its first.
    fn start_thread(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.close(window, cx);
        let Some(host) = self.host.clone() else { return };
        let prompt = format!("Start a sub-thread on {} to ", self.flow.cloud());
        let read = host.run(|c| async move { calls::local_workspace(&c).await });
        cx.spawn(async move |_, cx| {
            let Ok(Some(workspace)) = read.await else { return };
            cx.update(|cx| {
                if cx.has_global::<LocalStore>() {
                    cx.update_global(|store: &mut LocalStore, _| {
                        if store::draft(store, &workspace).is_none() {
                            _ = store::set_draft(store, &workspace, &prompt, prompt.len());
                        }
                    });
                }
                navigate(Route::Screen(format!("composer:{workspace}").into()), cx);
            });
        })
        .detach();
    }
}

/// A call whose answer nothing reads: sent, and its refusal dropped.
trait DetachFrom {
    fn detach_from(self, cx: &mut Context<AddCloud>);
}

impl<F: std::future::Future<Output = Result<(), Failure>> + 'static> DetachFrom for F {
    fn detach_from(self, cx: &mut Context<AddCloud>) {
        cx.spawn(async move |_, _| drop(self.await)).detach();
    }
}
