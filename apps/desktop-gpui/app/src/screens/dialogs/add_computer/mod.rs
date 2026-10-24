// SPDX-License-Identifier: AGPL-3.0-only
//! Add a computer (`settings/add/AddComputerDialog.tsx`, `addFlow.ts`, `adds.ts`): one step dialog walking Where, the
//! checks and the install, the picks step by step and the summary, then the setup running and the ready page. The
//! install is the host's `places.add` and the steps its `place.stage` frames; the picks are kept on the pending add
//! 400 ms after they move, so closing the dialog leaves the computer pending with them; Set up hands them to the host.
//! Closing while a setup runs leaves it running on the host.

pub mod choices;
pub mod model;
mod rows;
mod view;
pub mod words;

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use futures::StreamExt as _;
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::{prelude::*, App, Context, Entity, FocusHandle, Global, ScrollHandle, Subscription, Task, Window};
use serde_json::{json, Value};
use wsp_host_client::protocol::{
    PlacesAddRequest, PlacesChooseRequest, PlacesEstimateRequest, PlacesListRequest, PlacesSetupRequest, PlacesSshHostsRequest,
    PreferencesGetRequest, PreferencesSetRequest, RecipesListRequest, RecipesOptionsRequest, RecipesSaveRequest,
};
use wsp_host_client::{Event, Failure, Op};

use self::model::{str_of, AddStep};
use super::layer;
use super::picks::PickListState;
use super::sign_in::{self, SignIns};
use crate::local_store::LocalStore;
use crate::screens::notices::{self, words::OPEN};
use crate::screens::palette::overlays;
use crate::screens::sidebar::Sidebar;
use crate::screens::{navigate, Host, Route};
use crate::ui::{self, Notice, NoticeAction, SelectItem, SelectState};

/// `KEEP_AFTER_MS`: a burst of ticks or a name typed is one write.
pub const KEEP_AFTER: Duration = Duration::from_millis(400);
/// `OUTPUT_EVERY_MS`: an open step's output is read this often while it runs.
pub const OUTPUT_EVERY: Duration = Duration::from_millis(3000);

/// `REACHED_KEY`: the step each pending add was left at in this window, by the pending add's id.
const REACHED_KEY: &str = "wsp:add-reached";

/// The step this window left the pending add `pending` at; none where it keeps none.
fn reached(pending: &str, cx: &App) -> Option<AddStep> {
    let store = cx.try_global::<LocalStore>()?;
    store.get(REACHED_KEY)?.get(pending)?.as_str().and_then(AddStep::of)
}

/// `keepReached`: the step kept for `pending`, or dropped with none. A window that keeps nothing opens a pending add at
/// its first pick.
fn keep_reached(pending: &str, step: Option<AddStep>, cx: &mut App) {
    if step.is_some() && reached(pending, cx) == step {
        return;
    }
    let Some(store) = cx.try_global::<LocalStore>() else { return };
    let mut held = store.get(REACHED_KEY).filter(|v| v.is_object()).cloned().unwrap_or_else(|| json!({}));
    match step {
        Some(step) => held[pending] = json!(step.id()),
        None => _ = held.as_object_mut().map(|o| o.remove(pending)),
    }
    _ = cx.global_mut::<LocalStore>().set(REACHED_KEY, held);
}

/// The two things a setup's notices say once a run, whichever road says it first (`firstTold`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Told {
    Going,
    End,
}

#[derive(Default)]
struct ToldRuns(HashSet<(Told, String)>);

impl Global for ToldRuns {}

/// Whether `line` is said for the setup run `run` for the first time, and marks it said.
pub fn first_told(run: &str, line: Told, cx: &mut App) -> bool {
    cx.default_global::<ToldRuns>().0.insert((line, run.to_owned()))
}

/// Whether Add a computer stands on the computer `place`, whose rows say what its notices would.
pub fn stands_on(place: &str, cx: &App) -> bool {
    layer::view(super::ADD_COMPUTER, cx)
        .and_then(|v| v.downcast::<AddComputer>().ok())
        .is_some_and(|v| v.read(cx).place_id.as_deref() == Some(place))
}

/// A notice's Open onto Add a computer's running view of the computer `place`.
pub fn open_setup(place: String) -> NoticeAction {
    NoticeAction::new(OPEN, move |window, cx| {
        if let Some(host) = overlays::of(window, cx).and_then(|o| o.read(cx).host.clone()) {
            open(&host, Some(place.clone()), window, cx);
        }
    })
}

/// `sayClosed`: a setup's dialog closed, said where the person lands. One still running or waiting on a sign-in goes
/// on, and says so with what waits; one that ended says how it ended; a failed one says nothing more than its row.
fn say_closed(place: &Value, cx: &mut App) {
    let Some(setup) = place.get("setup").filter(|s| !s.is_null()) else { return };
    if str_of(setup, "state") == Some("failed") {
        return;
    }
    let wait = setup.get("waiting").and_then(Value::as_array).and_then(|w| w.first());
    if str_of(setup, "state") != Some("running") && wait.is_none() {
        return say_ended(place, cx);
    }
    if !first_told(str_of(setup, "addId").unwrap_or_default(), Told::Going, cx) {
        return;
    }
    let name = model::place_name(place);
    let detail = wait.map_or_else(|| words::SAYS_WHEN_READY.to_owned(), |w| format!("{}.", notices::words::wait_line(w)));
    let open = open_setup(str_of(place, "id").unwrap_or_default().to_owned());
    ui::notify(cx, Notice::note(words::being_set_up(&name)).detail(detail).place(name).action(open));
}

/// `sayEnded`: a setup that ended, said once a run: Needs you where a folder or the GitHub sign-in did not land, else
/// ready, with what it left and the computer's page to finish it on, or the running view where it left nothing.
pub fn say_ended(place: &Value, cx: &mut App) {
    let Some(run) = place.get("setup").filter(|s| !s.is_null()).and_then(|s| str_of(s, "addId")) else { return };
    if !first_told(run, Told::End, cx) {
        return;
    }
    let name = model::place_name(place);
    let id = str_of(place, "id").unwrap_or_default().to_owned();
    if let Some(line) = model::needs_you_line(place) {
        return ui::notify(cx, Notice::note(notices::words::setup_needs_you(&name, &line)).place(name).action(open_setup(id)));
    }
    let left = model::left_rows(&model::setup_rows(place, &name));
    let ready = Notice::done(notices::words::computer_ready(&name)).place(name.clone());
    ui::notify(
        cx,
        match left.is_empty() {
            true => ready.action(open_setup(id)),
            false => ready
                .detail(words::left(&left, &name))
                .action(NoticeAction::new(OPEN, move |_, cx| navigate(Route::Screen(format!("settings:computer:{id}").into()), cx))),
        },
    );
}

/// The pick lists, one per step that ticks rows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum List {
    Agents,
    Servers,
    Clis,
    Skills,
    Plugins,
    Projects,
    Configs,
}

/// The project icons a recipe or a folder can wear (`ProjectIcon`).
pub const ICONS: [&str; 20] = [
    "folder", "code", "terminal", "globe", "rocket", "box", "database", "server", "cpu", "zap", "flame", "leaf", "star", "heart", "book",
    "music", "camera", "gamepad", "shield", "wrench",
];
/// `ProjectHue`.
pub const HUES: [&str; 9] = ["neutral", "red", "orange", "amber", "green", "teal", "blue", "violet", "pink"];

/// One folder a person may import: a project of the computer running the host, or a folder they picked.
#[derive(Clone, Debug)]
pub struct Folder {
    pub key: String,
    pub name: String,
    pub path: String,
    pub remote: Option<String>,
    pub icon: String,
    pub hue: String,
    pub bytes: Option<u64>,
    pub private: bool,
    pub unpushed: Option<u64>,
}

/// The fields a ticked folder opens: its name, icon and colour.
struct FolderFields {
    name: Entity<InputState>,
    icon: Entity<SelectState<String>>,
    hue: Entity<SelectState<String>>,
    _subs: Vec<Subscription>,
}

/// Add a computer's dialog, standing on the layer under [`super::ADD_COMPUTER`].
pub struct AddComputer {
    host: Option<Host>,
    step: AddStep,
    address: Entity<InputState>,
    sudo: Entity<InputState>,
    recipe_name: Entity<InputState>,
    recipe_icon: Entity<SelectState<String>>,
    add_id: Option<String>,
    job: Option<Value>,
    place_id: Option<String>,
    pending_id: Option<String>,
    picks: Option<Value>,
    from: String,
    options: Option<Value>,
    options_refused: Option<model::Said>,
    save_on: bool,
    saves: usize,
    starting: bool,
    refused: Option<model::Said>,
    estimate: Option<Value>,
    /// The host key the person trusted, with the address it was trusted for.
    trusted_key: Option<(String, String)>,
    hosts: Vec<Value>,
    hosts_refused: Option<model::Said>,
    places: Vec<Value>,
    recipes: Vec<Value>,
    projects: Vec<Value>,
    preferences: Value,
    added: Vec<Folder>,
    logs: Option<HashMap<String, Vec<String>>>,
    turned: HashMap<String, bool>,
    row_refused: Option<(String, model::Said)>,
    /// The setup's and the sync's states when the row's refusal came.
    refused_on: (Option<String>, Option<String>),
    busy: bool,
    lists: HashMap<List, Entity<PickListState>>,
    ways: HashMap<String, Entity<SelectState<String>>>,
    folders: HashMap<String, FolderFields>,
    writes: Vec<(String, Value)>,
    typed: Option<String>,
    chose: Option<Result<(), String>>,
    hosts_read: bool,
    keeping: Option<Task<()>>,
    logs_asked: rows::Logs,
    sign_ins: SignIns,
    sign_ins_at: rows::At,
    sign_in_fields: HashMap<String, Entity<InputState>>,
    /// The newest places.list asked; an older answer landing after it is dropped.
    listed: u64,
    /// The add this dialog asked for and the host has not answered yet: a list that does not hold it has not lost it.
    asking: bool,
    recipes_read: bool,
    /// A pending add opened before recipes.list answered: its first pick is chosen once it does.
    pick_waits: bool,
    scroll: ScrollHandle,
    focus: FocusHandle,
    /// Whether the running view has opened on the first row that waits on the person.
    shown_asks: std::rc::Rc<std::cell::Cell<bool>>,
    /// The places as the window's sidebar last held them, taken whenever they change there.
    kept: Vec<Value>,
    _events: Option<Task<()>>,
    _subs: Vec<Subscription>,
}

/// Opens Add a computer on the layer: on Where, or, for `setup`, on the running view of that computer's setup or where
/// its pending add was left.
pub fn open(host: &Host, setup: Option<String>, window: &mut Window, cx: &mut App) -> Entity<AddComputer> {
    let view = cx.new(|cx| AddComputer::new(Some(host.clone()), window, cx));
    view.update(cx, |this, cx| {
        this.keep_sidebar(window, cx);
        this.start(setup, cx);
    });
    stand(view.clone(), window, cx);
    view
}

/// Opens the dialog with no host, its answers staged by `stage` as the host would give them; for the shots and tests.
pub fn staged(
    window: &mut Window,
    cx: &mut App,
    stage: impl FnOnce(&mut AddComputer, &mut Window, &mut Context<AddComputer>),
) -> Entity<AddComputer> {
    let view = cx.new(|cx| AddComputer::new(None, window, cx));
    view.update(cx, |this, cx| stage(this, window, cx));
    stand(view.clone(), window, cx);
    view
}

fn stand(view: Entity<AddComputer>, window: &mut Window, cx: &mut App) {
    let weak = view.downgrade();
    let on_close: layer::OnClose = std::rc::Rc::new(move |_, cx| {
        // `openAdd` over itself starts afresh with nothing said: the one it replaces only keeps its picks and stops its sign-ins.
        let replaced = layer::view(super::ADD_COMPUTER, cx).is_some_and(|v| v.entity_id() != weak.entity_id());
        let _ = weak.update(cx, |this, cx| this.closing(!replaced, cx));
    });
    layer::open(super::ADD_COMPUTER, view.clone().into(), layer::Closes::Outside, Some(on_close), window, cx);
    view.update(cx, |this, cx| {
        if this.step == AddStep::Where {
            this.address.update(cx, |a, cx| a.focus(window, cx));
        }
    });
}

/// The place frames the dialog follows itself: the sidebar keeps the place records, read again on a place joined,
/// changed, present, absent or removed, and the dialog takes them from it.
const FOLLOWED: [&str; 4] = ["place.stage", "place.setup", "place.sync", "place.pending"];

/// What the host pushed that the dialog reads.
enum Pushed {
    Frame(Value),
    SignIn(sign_in::Heard),
}

/// A request read from the JSON the web app sends.
fn ask<T: serde::de::DeserializeOwned>(params: Value) -> Result<T, Failure> {
    serde_json::from_value(params).map_err(|e| Failure::new(e.to_string()))
}

fn value<T: serde::Serialize>(t: &T) -> Value {
    serde_json::to_value(t).unwrap_or(Value::Null)
}

impl AddComputer {
    fn new(host: Option<Host>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let address = cx.new(|cx| InputState::new(window, cx).placeholder(words::PLACEHOLDER));
        let sudo = cx.new(|cx| InputState::new(window, cx).masked(true));
        let recipe_name = cx.new(|cx| InputState::new(window, cx));
        let icons = ICONS
            .iter()
            .map(|i| SelectItem::new(i.to_string(), view::icon_word(i)).glyph(crate::screens::sidebar::looks::project_icon(i)))
            .collect();
        let recipe_icon = cx.new(|_| SelectState::new(icons, Some(&"rocket".to_owned())));
        let lists = [
            (List::Agents, Some(super::picks::AS_FOUND)),
            (List::Servers, None),
            (List::Clis, Some(super::picks::MOST_USED)),
            (List::Skills, None),
            (List::Plugins, None),
            (List::Projects, Some(super::picks::AS_FOUND)),
            (List::Configs, None),
        ]
        .into_iter()
        .map(|(list, own)| (list, cx.new(|cx| PickListState::new(own, window, cx))))
        .collect();
        let subs = vec![
            cx.subscribe(&address, |_, _, e: &InputEvent, cx| {
                if matches!(e, InputEvent::Change) {
                    cx.notify();
                }
            }),
            cx.subscribe_in(&address, window, |this, _, e: &InputEvent, window, cx| {
                if matches!(e, InputEvent::PressEnter { .. }) && this.step == AddStep::Where {
                    this.connect(None, None, window, cx);
                }
            }),
            cx.subscribe(&sudo, |_, _, e: &InputEvent, cx| {
                if matches!(e, InputEvent::Change) {
                    cx.notify();
                }
            }),
            cx.subscribe_in(&sudo, window, |this, _, e: &InputEvent, window, cx| {
                if matches!(e, InputEvent::PressEnter { .. }) && !this.sudo.read(cx).value().is_empty() {
                    this.try_again(window, cx);
                }
            }),
        ];
        AddComputer {
            host,
            step: AddStep::Where,
            address,
            sudo,
            recipe_name,
            recipe_icon,
            add_id: None,
            job: None,
            place_id: None,
            pending_id: None,
            picks: None,
            from: "here".into(),
            options: None,
            options_refused: None,
            save_on: false,
            saves: 0,
            starting: false,
            refused: None,
            estimate: None,
            trusted_key: None,
            hosts: Vec::new(),
            hosts_refused: None,
            places: Vec::new(),
            recipes: Vec::new(),
            projects: Vec::new(),
            preferences: json!({}),
            added: Vec::new(),
            logs: None,
            turned: HashMap::new(),
            row_refused: None,
            refused_on: (None, None),
            busy: false,
            lists,
            ways: HashMap::new(),
            folders: HashMap::new(),
            writes: Vec::new(),
            typed: None,
            chose: None,
            hosts_read: false,
            keeping: None,
            logs_asked: rows::Logs::default(),
            sign_ins: SignIns::default(),
            sign_ins_at: rows::At::new(),
            sign_in_fields: HashMap::new(),
            listed: 0,
            asking: false,
            recipes_read: false,
            pick_waits: false,
            scroll: ScrollHandle::new(),
            focus: cx.focus_handle(),
            shown_asks: Default::default(),
            kept: Vec::new(),
            _events: None,
            _subs: subs,
        }
    }

    /// Takes the place records from the window's sidebar, now and whenever they change there.
    fn keep_sidebar(&mut self, window: &Window, cx: &mut Context<Self>) {
        let Some(sidebar) = overlays::of(window, cx).and_then(|o| o.read(cx).sidebar.clone()).and_then(|s| s.upgrade()) else { return };
        self.take_kept(&sidebar, cx);
        self._subs.push(cx.observe(&sidebar, |this, sidebar, cx| this.take_kept(&sidebar, cx)));
    }

    fn take_kept(&mut self, sidebar: &Entity<Sidebar>, cx: &mut Context<Self>) {
        let places = &sidebar.read(cx).around().places;
        if places.is_empty() || *places == self.kept {
            return;
        }
        self.kept = places.clone();
        self.places = places.clone();
        self.sync_log(cx);
        cx.notify();
    }

    /// One op on the host, its answer handed to `then` on this view; whether it was sent, which it is not with no host.
    fn call<O>(
        &self,
        op: O,
        cx: &mut Context<Self>,
        then: impl FnOnce(&mut Self, Result<O::Reply, Failure>, &mut Context<Self>) + 'static,
    ) -> bool
    where
        O: Op + Send + 'static,
        O::Reply: Send + 'static,
    {
        let Some(host) = self.host.clone() else { return false };
        let asked = host.run(move |c| async move { c.call(op).await });
        cx.spawn(async move |this, cx| {
            let answer = asked.await;
            let _ = this.update(cx, |this, cx| {
                then(this, answer, cx);
                cx.notify();
            });
        })
        .detach();
        true
    }

    /// The reads every open makes, and the host's pushes followed while the dialog stands.
    fn start(&mut self, setup: Option<String>, cx: &mut Context<Self>) {
        if let Some(id) = setup {
            self.place_id = Some(id);
            self.step = AddStep::Running;
        }
        self.follow(cx);
        self.reread_places(cx);
        if let Ok(op) = ask::<RecipesListRequest>(json!({})) {
            self.call(op, cx, |this, r, cx| {
                if let Ok(r) = r {
                    this.take_recipes(r.recipes.iter().map(value).collect(), cx);
                }
            });
        }
        if !matches!(self.step, AddStep::Running | AddStep::Ready) {
            self.read_options(cx);
        }
        if let Ok(op) = ask::<PlacesSshHostsRequest>(json!({})) {
            self.call(op, cx, |this, r, cx| this.take_hosts(model::hosts_read(r.map(|h| h.hosts.iter().map(value).collect())), cx));
        }
        if let Ok(op) = ask::<PreferencesGetRequest>(json!({})) {
            self.call(op, cx, |this, r, _| {
                if let Ok(r) = r {
                    this.preferences = value(&r.preferences);
                }
            });
        }
        if let Some(host) = self.host.clone() {
            let asked = host.run(|c| async move { c.projects().await });
            cx.spawn(async move |this, cx| {
                if let Ok(projects) = asked.await {
                    let _ = this.update(cx, |this, cx| this.take_projects(projects.iter().map(value).collect(), cx));
                }
            })
            .detach();
        }
    }

    fn read_options(&mut self, cx: &mut Context<Self>) {
        if let Ok(op) = ask::<RecipesOptionsRequest>(json!({})) {
            self.call(op, cx, |this, r, cx| match r {
                Ok(r) => this.take_options(r.options, cx),
                Err(f) => this.options_refused = Some(model::Said::from(&f)),
            });
        }
    }

    /// Follows the host's place frames while the dialog stands; the task ends with the view.
    fn follow(&mut self, cx: &mut Context<Self>) {
        let Some(host) = self.host.clone() else { return };
        let (tx, mut rx) = futures::channel::mpsc::unbounded::<Pushed>();
        let pump = host.run(move |c| async move {
            let mut events = c.events().await?;
            while let Some(event) = events.next().await {
                let pushed = match event {
                    Ok(Event::Host(event)) => {
                        Some(value(&*event)).filter(|f| str_of(f, "type").is_some_and(|t| FOLLOWED.contains(&t))).map(Pushed::Frame)
                    }
                    Ok(Event::SignIn(event)) => sign_in::heard_of(&event).map(Pushed::SignIn),
                    _ => None,
                };
                if pushed.is_some_and(|p| tx.unbounded_send(p).is_err()) {
                    break;
                }
            }
            Ok(())
        });
        self._events = Some(cx.spawn(async move |this, cx| {
            let take = async {
                while let Some(pushed) = rx.next().await {
                    let alive = this.update(cx, |this, cx| match pushed {
                        Pushed::Frame(frame) => this.apply(&frame, cx),
                        Pushed::SignIn(heard) => this.hear_sign_in(&heard, cx),
                    });
                    if alive.is_err() {
                        break;
                    }
                }
            };
            let _ = futures::join!(pump, take);
        }));
    }

    // The answers, as the host gives them; the shots stage the dialog through these.

    /// `places.list`: the places, the adds the host holds and the pending adds. A pending add or a place the dialog was
    /// opened on decides the step it stands on.
    pub fn take_places(&mut self, places: Vec<Value>, adds: Vec<Value>, pending: Vec<Value>, cx: &mut Context<Self>) {
        self.places = places;
        match self.add_id.as_ref().and_then(|id| adds.iter().find(|a| str_of(a, "addId") == Some(id))) {
            Some(add) => {
                // A read that left before an add ended never takes the end back.
                let ended = self.job.as_ref().is_some_and(|j| str_of(j, "state") != Some("running"));
                if !ended || str_of(add, "state") != Some("running") {
                    self.job = Some(add.clone());
                }
                self.joined(cx);
            }
            None if !self.asking => {
                if let Some(job) = self.job.as_mut().filter(|j| str_of(j, "state") == Some("running")) {
                    job["state"] = json!("failed");
                    job["said"] = json!(words::HOST_LOST);
                }
            }
            None => {}
        }
        let by_id = |id: &str| adds.iter().find(|a| str_of(a, "addId") == Some(id)).cloned();
        if self.step == AddStep::Running && self.place().is_none() {
            let id = self.place_id.clone().unwrap_or_default();
            if let Some(p) = pending.iter().find(|p| str_of(p, "id") == Some(&id)) {
                self.open_pending(p, by_id(&id), cx);
            }
        } else if self.step == AddStep::Running && self.place().and_then(|p| p.get("setup")).is_none_or(Value::is_null) {
            let id = self.place_id.clone().unwrap_or_default();
            if let Some(p) = pending.iter().find(|p| str_of(p, "placeId") == Some(&id)) {
                self.open_pending(p, by_id(str_of(p, "id").unwrap_or_default()), cx);
            }
        }
        self.sync_log(cx);
        cx.notify();
    }

    /// `openPending`: an install still going, or one closed on its checks before a pick, opens on its checks; a computer
    /// whose setup started, on its running steps; else where this window left it, with the picks it kept. An add the
    /// host holds no install for opens at Where.
    fn open_pending(&mut self, pending: &Value, job: Option<Value>, cx: &mut Context<Self>) {
        let id = str_of(pending, "id").unwrap_or_default().to_owned();
        self.pending_id = Some(id.clone());
        self.address_text(str_of(pending, "address").unwrap_or_default(), cx);
        self.picks = pending.get("choices").filter(|c| !c.is_null()).cloned();
        self.from = str_of(pending, "recipe").unwrap_or("here").to_owned();
        let Some(place) = str_of(pending, "placeId").or_else(|| job.as_ref().and_then(|j| str_of(j, "placeId"))).map(str::to_owned) else {
            self.place_id = None;
            self.add_id = job.as_ref().map(|_| id);
            self.job = job;
            self.step = if self.job.is_some() { AddStep::Checks } else { AddStep::Where };
            return;
        };
        self.place_id = Some(place);
        if self.place().and_then(|p| p.get("setup")).is_some_and(|s| !s.is_null()) {
            self.step = AddStep::Running;
            return;
        }
        match (reached(&id, cx), job) {
            (None, Some(job)) => {
                self.add_id = Some(id);
                self.job = Some(job);
                self.step = AddStep::Checks;
            }
            (Some(AddStep::StartFrom), _) if self.recipes_read() && self.recipes.is_empty() => self.step = AddStep::Agents,
            (Some(step), _) => self.step = step,
            (None, None) => {
                self.step = model::first_pick(self.recipes.len());
                self.pick_waits = !self.recipes_read && self.host.is_some();
            }
        }
        if self.options.is_none() {
            self.read_options(cx);
        }
    }

    pub fn take_hosts(&mut self, hosts: Result<Vec<Value>, model::Said>, cx: &mut Context<Self>) {
        self.hosts_read = true;
        match hosts {
            Ok(hosts) => {
                self.hosts = hosts;
                self.hosts_refused = None;
            }
            Err(said) => {
                self.hosts.clear();
                self.hosts_refused = Some(said);
            }
        }
        cx.notify();
    }

    pub fn take_recipes(&mut self, recipes: Vec<Value>, cx: &mut Context<Self>) {
        self.recipes = recipes;
        self.recipes_read = true;
        if std::mem::take(&mut self.pick_waits) && self.step == model::first_pick(0) {
            self.step = model::first_pick(self.recipes.len());
        }
        // A pending add opened again on Start from before the recipes were read has nothing to start from.
        if self.step == AddStep::StartFrom && self.recipes.is_empty() {
            self.step = AddStep::Agents;
        }
        cx.notify();
    }

    /// Whether a row's act is with the host, which holds the row acts.
    pub fn busy(&self) -> bool {
        self.busy
    }

    /// Whether recipes.list has answered, which the first pick step waits on.
    pub fn recipes_read(&self) -> bool {
        self.recipes_read || self.host.is_none()
    }

    /// `recipes.options`; a first pick step with nothing picked starts from everything here, once.
    pub fn take_options(&mut self, options: Value, cx: &mut Context<Self>) {
        self.options = Some(options);
        self.options_refused = None;
        self.fill(cx);
        cx.notify();
    }

    /// The projects the host lists; the ones of the computer running it are what folders are imported from.
    pub fn take_projects(&mut self, projects: Vec<Value>, cx: &mut Context<Self>) {
        self.projects = projects;
        cx.notify();
    }

    /// `places.estimate`'s answer for the picks as they stand.
    pub fn take_estimate(&mut self, estimate: Value, cx: &mut Context<Self>) {
        self.estimate = Some(estimate);
        cx.notify();
    }

    /// The box's setup log, read into each step's last lines.
    pub fn take_log(&mut self, lines: &[String], cx: &mut Context<Self>) {
        self.logs = Some(model::step_logs(lines));
        cx.notify();
    }

    /// Stands the dialog on `step` with what it was given; a staged dialog's road to any state.
    pub fn stage(
        &mut self,
        step: AddStep,
        address: &str,
        job: Option<Value>,
        place_id: Option<&str>,
        picks: Option<Value>,
        cx: &mut Context<Self>,
    ) {
        self.step = step;
        self.address_text(address, cx);
        self.add_id = job.as_ref().and_then(|j| str_of(j, "addId")).map(str::to_owned);
        self.job = job;
        self.place_id = place_id.map(str::to_owned);
        self.picks = picks;
        self.sync_log(cx);
        cx.notify();
    }

    /// Ticks Save as a recipe with its name, as a person would.
    pub fn stage_save(&mut self, name: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.save_on = true;
        self.recipe_name.update(cx, |f, cx| f.set_value(name.to_owned(), window, cx));
        cx.notify();
    }

    /// Opens a step row's log, as a press on it does.
    pub fn open_row(&mut self, row: &str, open: bool, cx: &mut Context<Self>) {
        self.turned.insert(row.to_owned(), open);
        self.sync_log(cx);
        cx.notify();
    }

    /// Puts `text` in the address field at the next draw, where the field can take it.
    fn address_text(&mut self, text: &str, cx: &mut Context<Self>) {
        if self.address.read(cx).value() != text {
            self.typed = Some(text.to_owned());
        }
    }

    pub fn step(&self) -> AddStep {
        self.step
    }

    pub fn picks(&self) -> Option<&Value> {
        self.picks.as_ref()
    }

    /// The `places.choose` writes sent so far, each with the place it named; nothing else is kept.
    pub fn writes(&self) -> &[(String, Value)] {
        &self.writes
    }

    /// The hosts the ssh config named, or why they were not read.
    pub fn hosts(&self) -> (&[Value], Option<&model::Said>) {
        (&self.hosts, self.hosts_refused.as_ref())
    }

    /// Whether `places.sshHosts` answered, with hosts, none, or a refusal.
    pub fn hosts_read(&self) -> bool {
        self.hosts_read
    }

    /// Types `text` into the address field, as a person would.
    pub fn type_address(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.address.update(cx, |a, cx| {
            a.set_value(text.to_owned(), window, cx);
            a.focus(window, cx);
        });
        cx.notify();
    }

    /// Types `words` into a pick list's filter.
    pub fn filter(&mut self, list: List, words: &str, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(state) = self.lists.get(&list).cloned() {
            state.update(cx, |s, cx| s.set_filter(words, window, cx));
        }
    }

    /// The places as the dialog last read and followed them.
    pub fn places(&self) -> &[Value] {
        &self.places
    }

    /// How far the body is scrolled down.
    pub fn scrolled(&self) -> gpui_kit::Pixels {
        -self.scroll.offset().y
    }

    pub fn options(&self) -> Option<&Value> {
        self.options.as_ref()
    }

    pub fn estimate(&self) -> Option<&Value> {
        self.estimate.as_ref()
    }

    /// How the host answered the last `places.choose`: taken, or its refusal.
    pub fn chose(&self) -> Option<&Result<(), String>> {
        self.chose.as_ref()
    }

    pub fn job(&self) -> Option<&Value> {
        self.job.as_ref()
    }

    pub fn place(&self) -> Option<&Value> {
        let id = self.place_id.as_deref()?;
        self.places.iter().find(|p| str_of(p, "id") == Some(id))
    }

    /// The computer being added, by name: its own once it joined, else the address.
    pub fn box_name(&self, cx: &App) -> String {
        match self.place() {
            Some(place) => model::place_name(place),
            None => self.address.read(cx).value().trim().to_owned(),
        }
    }

    /// The computer running the host, by name.
    pub fn here(&self) -> String {
        self.places.iter().find(|p| str_of(p, "id") == Some("here")).map(model::place_name).unwrap_or_default()
    }

    /// One frame the host pushed that the sidebar does not keep: an install's step, a setup's frame, a place syncing, a
    /// pending add.
    pub fn apply(&mut self, frame: &Value, cx: &mut Context<Self>) {
        let place_id = str_of(frame, "placeId").map(str::to_owned);
        fn place_mut<'a>(this: &'a mut AddComputer, id: &str) -> Option<&'a mut Value> {
            this.places.iter_mut().find(|p| str_of(p, "id") == Some(id))
        }
        match str_of(frame, "type").unwrap_or_default() {
            "place.stage" => {
                let mine = self.add_id.as_deref().is_some_and(|id| str_of(frame, "addId") == Some(id));
                if let (true, Some(job)) = (mine, self.job.as_ref()) {
                    self.job = Some(model::with_place_stage(job, frame));
                    self.joined(cx);
                }
            }
            "place.setup" => {
                if let Some(place) = place_id.and_then(|id| place_mut(self, &id)) {
                    if let Some(setup) = place.get("setup").filter(|s| !s.is_null()) {
                        let next = model::fold_setup(setup, frame);
                        place["setup"] = next;
                    }
                }
            }
            "place.sync" => {
                if let Some(place) = place_id.and_then(|id| place_mut(self, &id)) {
                    match frame.get("sync").filter(|s| !s.is_null()) {
                        Some(sync) => place["sync"] = sync.clone(),
                        None => {
                            if let Some(o) = place.as_object_mut() {
                                o.remove("sync");
                            }
                        }
                    }
                    if let Some(applied) = frame.get("applied").filter(|a| !a.is_null()) {
                        place["applied"] = applied.clone();
                    }
                }
            }
            "place.pending" => {
                let mine = self.pending_id.as_deref().is_some_and(|id| str_of(frame, "id") == Some(id));
                if mine && frame.get("pending").is_none_or(Value::is_null) && self.step.picks() {
                    self.reread_places(cx);
                }
            }
            _ => return,
        }
        self.sync_log(cx);
        cx.notify();
    }

    /// places.list, stamped so that only the newest read asked lands: an older one may predate what asked for it.
    fn reread_places(&mut self, cx: &mut Context<Self>) {
        let Ok(op) = ask::<PlacesListRequest>(json!({})) else { return };
        self.listed += 1;
        let stamp = self.listed;
        self.call(op, cx, move |this, r, cx| {
            if let (Ok(l), true) = (r, stamp == this.listed) {
                this.take_places(
                    l.places.iter().map(value).collect(),
                    l.adds.iter().map(value).collect(),
                    l.pending.iter().map(value).collect(),
                    cx,
                );
            }
        });
    }

    /// The computer joined: the add goes on to its picks, the pending add standing for it under the same id.
    fn joined(&mut self, cx: &mut Context<Self>) {
        let Some(job) = &self.job else { return };
        if str_of(job, "state") == Some("done") && self.place_id.is_none() {
            if let Some(place) = str_of(job, "placeId") {
                self.place_id = Some(place.to_owned());
                self.pending_id = self.add_id.clone();
                self.reread_places(cx);
            }
        }
    }

    /// `connect`: the host adds the computer over ssh with the key the person trusted and the sudo password typed.
    pub fn connect(&mut self, host_key: Option<String>, sudo_password: Option<String>, _: &mut Window, cx: &mut Context<Self>) {
        let address = self.address.read(cx).value().trim().to_owned();
        if address.is_empty() {
            return;
        }
        let key = model::key_for(host_key, self.trusted_key.as_ref(), &address);
        let add_id = mint_add_id();
        self.add_id = Some(add_id.clone());
        self.job = Some(json!({ "addId": add_id, "address": address, "startedAt": now_iso(), "state": "running", "steps": [] }));
        self.trusted_key = key.clone().map(|k| (address.clone(), k));
        self.step = AddStep::Checks;
        let mut params = json!({ "addId": add_id, "address": address });
        if let Some(key) = key {
            params["hostKey"] = json!(key);
        }
        if let Some(pw) = sudo_password {
            params["sudoPassword"] = json!(pw);
        }
        let Ok(op) = ask::<PlacesAddRequest>(params) else { return };
        self.asking = self.call(op, cx, move |this, r, cx| {
            if this.add_id.as_deref() != Some(add_id.as_str()) {
                return;
            }
            this.asking = false;
            let Some(job) = this.job.as_mut() else { return };
            match r {
                Ok(added) => {
                    job["state"] = json!("done");
                    job["placeId"] = json!(added.place.id);
                    let place = value(&added.place);
                    let id = added.place.id.clone();
                    this.places.retain(|p| str_of(p, "id") != Some(&id));
                    this.places.push(place);
                    this.joined(cx);
                }
                Err(f) => {
                    job["state"] = json!("failed");
                    job["said"] = json!(f.said());
                    if let Some(fix) = &f.fix {
                        job["fix"] = json!(fix);
                    }
                    if let Some(kind) = &f.kind {
                        job["kind"] = json!(kind);
                    }
                    if f.kind.as_deref() == Some(words::HOST_KEY_KIND) {
                        this.reread_places(cx);
                    }
                }
            }
        });
        cx.notify();
    }

    /// Try again on the checks: with the sudo password where the root row asked for it, else back to Where.
    pub fn try_again(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if model::asked_sudo(self.job.as_ref()) {
            let typed = self.sudo.read(cx).value().to_string();
            self.sudo.update(cx, |s, cx| s.set_value(String::new(), window, cx));
            self.connect(None, Some(typed), window, cx);
        } else {
            self.go(AddStep::Where, cx);
        }
    }

    pub fn go(&mut self, step: AddStep, cx: &mut Context<Self>) {
        self.step = step;
        if let Some(pending) = self.pending_id.clone().filter(|_| !matches!(step, AddStep::Running | AddStep::Ready)) {
            keep_reached(&pending, Some(step), cx);
        }
        self.scroll = ScrollHandle::new();
        if step == AddStep::Summary {
            self.weigh(cx);
        }
        self.fill(cx);
        self.sync_log(cx);
        cx.notify();
    }

    /// A first pick step with nothing picked starts from everything here, once; a sign-in nobody here can serve moves
    /// onto one that can.
    fn fill(&mut self, cx: &mut Context<Self>) {
        let Some(options) = self.options.clone() else { return };
        if self.step.picks() && self.from == "here" && self.saves == 0 && model::empty(self.picks.as_ref()) {
            let name = self.picks.as_ref().and_then(|p| str_of(p, "name")).map(str::to_owned).unwrap_or_else(|| self.box_name(cx));
            let all =
                choices::everything(&name, &options, &self.here_projects(), self.preferences.get("projectLook").unwrap_or(&json!({})));
            self.set_picks(all, None, cx);
        }
        if let Some(picks) = &self.picks {
            let next = choices::servable(picks, &options);
            if &next != picks {
                self.set_picks(next, None, cx);
            }
        }
    }

    pub fn here_projects(&self) -> Vec<Value> {
        self.projects.iter().filter(|p| str_of(p, "computer") == Some("here")).cloned().collect()
    }

    /// The picks as they now stand, kept on the pending add 400 ms after the last change.
    pub fn set_picks(&mut self, picks: Value, from: Option<String>, cx: &mut Context<Self>) {
        self.picks = Some(picks);
        if let Some(from) = from {
            self.from = from;
        }
        self.saves += 1;
        self.estimate = None;
        self.keeping = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(KEEP_AFTER).await;
            let _ = this.update(cx, |this, cx| this.keep_now(cx));
        }));
        if self.step == AddStep::Summary {
            self.weigh(cx);
        }
        cx.notify();
    }

    /// The write that waits, made now: on close, and once the picks rested.
    pub fn keep_now(&mut self, cx: &mut Context<Self>) {
        let Some(task) = self.keeping.take() else { return };
        drop(task);
        let (Some(place), Some(picks)) = (self.place_id.clone(), self.picks.clone()) else { return };
        if self.writes.len() >= 64 {
            self.writes.remove(0);
        }
        self.writes.push((place.clone(), picks.clone()));
        let mut params = json!({ "ref": place, "choices": picks });
        if self.from != "here" && self.from != "none" {
            params["recipe"] = json!(self.from);
        }
        if let Ok(op) = ask::<PlacesChooseRequest>(params) {
            self.call(op, cx, |this, r, _| this.chose = Some(r.map(|_| ()).map_err(|f| f.message)));
        }
    }

    /// `weigh`: the picks against the computer's room; an answer for picks that moved on since is dropped.
    fn weigh(&mut self, cx: &mut Context<Self>) {
        let (Some(place), Some(picks)) = (self.place_id.clone(), self.picks.clone()) else { return };
        let Ok(op) = ask::<PlacesEstimateRequest>(json!({ "ref": place, "choices": picks })) else { return };
        self.call(op, cx, move |this, r, _| {
            if let (Ok(r), Some(now)) = (r, this.picks.as_ref()) {
                if now == &picks {
                    this.estimate = Some(value(&r.estimate));
                }
            }
        });
    }

    /// Set up: the picks to the host, then the running steps; then, where asked, the picks saved as a recipe.
    pub fn set_up(&mut self, cx: &mut Context<Self>) {
        let (Some(place), Some(picks)) = (self.place_id.clone(), self.picks.clone()) else { return };
        let Ok(op) = ask::<PlacesSetupRequest>(json!({ "ref": place, "choices": picks })) else { return };
        let icon = self.recipe_icon.read(cx).value().cloned().unwrap_or_else(|| "rocket".into());
        let save = (self.save_on && !self.recipe_name.read(cx).value().trim().is_empty())
            .then(|| (self.recipe_name.read(cx).value().trim().to_owned(), place.clone(), icon));
        self.keeping = None;
        self.refused = None;
        self.starting = self.call(op, cx, move |this, r, cx| {
            this.starting = false;
            match r {
                Ok(answer) => {
                    this.show_setup(value(&answer.place), answer.setup.as_ref().map(value));
                    if let Some(said) = answer.said {
                        this.refused = Some(model::Said::plain(said));
                        return;
                    }
                    if let Some(pending) = this.pending_id.clone() {
                        keep_reached(&pending, None, cx);
                    }
                    this.step = AddStep::Running;
                    this.sync_log(cx);
                    if let Some((name, computer, icon)) = save {
                        if let Ok(op) = ask::<RecipesSaveRequest>(json!({ "name": name, "from": computer })) {
                            this.call(op, cx, move |this, r, cx| this.take_saved(r.map(|saved| saved.recipe.slug), &icon, cx));
                        }
                    }
                }
                Err(f) => this.refused = Some(model::Said::from(&f)),
            }
        });
        cx.notify();
    }

    /// `recipes.save` after Set up: the recipe's icon kept with the person's other looks, or the refusal said as a notice,
    /// since the setup it would have named is already running.
    pub fn take_saved(&mut self, saved: Result<String, Failure>, icon: &str, cx: &mut Context<Self>) {
        match saved {
            Ok(slug) => {
                if let Ok(op) = ask::<PreferencesSetRequest>(model::recipe_look(&slug, icon)) {
                    self.call(op, cx, |_, _, _| {});
                }
            }
            Err(f) => crate::ui::notify(cx, crate::ui::Notice::error(f.message)),
        }
    }

    /// The setup a start answered, onto the computer's row at once.
    fn show_setup(&mut self, place: Value, setup: Option<Value>) {
        let id = str_of(&place, "id").unwrap_or_default().to_owned();
        let mut place = place;
        if let Some(setup) = setup {
            place["setup"] = setup;
        }
        match self.places.iter_mut().find(|p| str_of(p, "id") == Some(&id)) {
            Some(held) => *held = place,
            None => self.places.push(place),
        }
    }

    /// Closes the dialog by its own button: picks not yet kept are kept now. A setup still running goes on there.
    pub fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.closing(true, cx);
        layer::close(super::ADD_COMPUTER, window, cx);
    }

    /// Start a task here: the dialog closes with nothing said, as the person goes on to the computer's work.
    pub fn start_task(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.keep_now(cx);
        for stop in self.sign_ins.end() {
            self.send_sign_in(stop, cx);
        }
        layer::close(super::ADD_COMPUTER, window, cx);
    }

    /// However the dialog closes: picks not yet kept are kept now, and with `say`, a setup it stood on says where it
    /// stands.
    fn closing(&mut self, say: bool, cx: &mut Context<Self>) {
        self.keep_now(cx);
        for stop in self.sign_ins.end() {
            self.send_sign_in(stop, cx);
        }
        if let Some(place) = self.place().filter(|_| say && matches!(self.step, AddStep::Running | AddStep::Ready)).cloned() {
            say_closed(&place, cx);
        }
    }
}

fn mint_add_id() -> String {
    use std::hash::{BuildHasher, Hasher};
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0));
    format!("a_{:012x}", h.finish() & 0xffff_ffff_ffff)
}

fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}
