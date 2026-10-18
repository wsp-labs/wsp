// SPDX-License-Identifier: AGPL-3.0-only
//! Add a project (`sidebar/AddProjectDialog.tsx`): a project is a git repo on one computer, so the dialog opens on the
//! repos the chosen computer holds and the field searches them by name and path. A path typed from / or ~ walks the
//! disk instead. A repository address is cloned by the computer picked: a box or a provider clones where it keeps its
//! checkouts, and the host's own computer into a folder of the repo's name inside the one the person picks. The list
//! keeps one height across every state, so nothing around it moves.

use std::collections::HashSet;

use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{h_flex, v_flex, ActiveTheme as _, Icon, Sizable as _};
use gpui_kit::{
    div, prelude::*, px, AnyElement, App, Context, Entity, Focusable as _, Global, KeyBinding, KeyDownEvent, NoAction, PathPromptOptions,
    SharedString, Subscription, Task, Window,
};
use serde::Deserialize;
use serde_json::{json, Value};
use wsp_host_client::protocol::{HostFoldersRequest, PlacesListRequest, PreferencesSetRequest, ProjectsAddRequest, ProjectsListRequest};
use wsp_host_client::Failure;

use super::add_computer::words::capitalised;
use super::add_computer::{HUES, ICONS};
use super::layer::{self, Closes};
use super::{open_add_computer, ADD_PROJECT};
use crate::screens::sidebar::looks::{hue, place_name, project_glyph, Images};
use crate::screens::Host;
use crate::ui::card::keycap;
use crate::ui::{size, text, tip, ActivePalette, Button, Field, Glyph, Palette, Select, SelectItem, SelectState};

pub const TITLE: &str = "Add a project";
pub const SEARCH: &str = "Search your repos, or type a path";
pub const ADDRESS_ONLY: &str = "Paste a repository address";
pub const CHOOSE: &str = "Choose";
pub const ADD: &str = "Add";
pub const COMPUTERS: &str = "Computers";
pub const ADD_COMPUTER: &str = "Add a computer";
pub const LOOK: &str = "Look";
pub const NAVIGATE: &str = "Navigate";
pub const OPEN: &str = "Open";
pub const BY_PATH: &str = "Type a path";
pub const NO_REPOS: &str = "No git repos found under your home folder. Type a path to one.";
pub const NO_FOLDERS: &str = "No folders here.";
pub const NO_MATCH: &str = "No repo matches.";
pub const ADDED: &str = "Added";

pub fn repos_on(name: &str) -> String {
    format!("Repos on {name}")
}

pub fn clone_line(url: &str, on: &str) -> String {
    format!("Clone {url} on {on}")
}

pub fn clone_into(url: &str) -> String {
    format!("Clone {url} into")
}

pub fn clone_into_placeholder(name: &str) -> String {
    format!("The folder to put {name} in, like ~/code")
}

pub fn box_says(name: &str) -> String {
    format!("Repos on {name} show here soon. Paste a repository address above to clone it there.")
}

pub fn provider_says(name: &str) -> String {
    format!("{name} clones a project from its repository address. Paste one above.")
}

/// `sm:max-w-3xl`, the list's `h-[400px]` and the side's `210px`.
const WIDTH: f32 = 768.;
const LIST_HEIGHT: f32 = 400.;
const SIDE: f32 = 210.;
/// `rounded-lg`, from `--radius`.
const ROW_RADIUS: f32 = 10.;
/// The search field's key context: Tab there completes the lit row, as the web's field does, instead of moving focus.
const FIELD_KEYS: &str = "AddProjectField";

/// Whether Tab is taken from Root in the search field: bound once, however often the dialog opens.
#[derive(Default)]
struct TabBound(bool);

impl Global for TabBound {}

/// One folder as `host.folders` lists it.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Folder {
    pub path: String,
    pub repo: bool,
    #[serde(default)]
    pub branch: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Listing {
    pub dir: String,
    pub folders: Vec<Folder>,
}

/// How the field names a repository: by its address, which nothing else looks like, or as owner/repo, which a search
/// of paths can look like too.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RepoWord {
    Address,
    Short,
}

fn word_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '-'
}

fn words(s: &str) -> bool {
    !s.is_empty() && s.chars().all(word_char)
}

/// `sourceKind`: what a word added as a project is, or nothing for a word that is none of its forms.
pub fn source_kind(word: &str) -> Option<&'static str> {
    if word.starts_with('/') || word.starts_with('~') || word.starts_with('.') {
        return Some("folder");
    }
    for (host, kind) in [("github.com/", "github"), ("gitlab.com/", "gitlab")] {
        if let Some(rest) = word.strip_prefix(host) {
            let parts: Vec<&str> = rest.split('/').collect();
            if parts.len() == 2 && parts.iter().all(|p| !p.is_empty()) {
                return Some(kind);
            }
        }
    }
    let at_host = |s: &str| s.split_once('@').is_some_and(|(user, host)| words(user) && words(host));
    let scp = word.split_once(':').is_some_and(|(head, _)| at_host(head));
    if word.contains("://") || scp || word.ends_with(".git") {
        return Some("git");
    }
    if at_host(word) {
        return Some("computer");
    }
    if word.split_once('/').is_some_and(|(owner, repo)| words(owner) && words(repo)) {
        return Some("github");
    }
    None
}

/// `repositoryWord`.
pub fn repository_word(word: &str) -> Option<RepoWord> {
    match source_kind(word.trim())? {
        "git" => Some(RepoWord::Address),
        "github" | "gitlab" if word.contains(".com/") => Some(RepoWord::Address),
        "github" => Some(RepoWord::Short),
        _ => None,
    }
}

/// What a repository word would be called as a project: its last word without `.git`.
pub fn repo_name(word: &str) -> String {
    match source_kind(word) {
        None | Some("folder") | Some("computer") => "repo".to_owned(),
        Some(_) => {
            let last = word.trim_end_matches('/').rsplit(['/', ':']).next().unwrap_or(word);
            last.strip_suffix(".git").unwrap_or(last).to_owned()
        }
    }
}

/// A typed path split into the folder to list and the start of the name being typed in it.
pub fn path_parts(typed: &str, home: &str) -> (String, String) {
    let full = match typed.strip_prefix('~') {
        Some(rest) => format!("{home}{rest}"),
        None => typed.to_owned(),
    };
    let cut = full.rfind('/').unwrap_or(0);
    let dir = if full[..cut].is_empty() { "/".to_owned() } else { full[..cut].to_owned() };
    (dir, full[cut + 1..].to_lowercase())
}

/// A path under `home` written from `~`; any other path, or no home known, whole.
pub fn tilde(path: &str, home: &str) -> String {
    if !home.is_empty() && (path == home || path.starts_with(&format!("{home}/"))) {
        format!("~{}", &path[home.len()..])
    } else {
        path.to_owned()
    }
}

fn base_name(path: &str) -> &str {
    path.trim_end_matches('/').rsplit('/').next().unwrap_or(path)
}

/// What the dialog shows for what is typed and picked now.
struct Shown {
    here: bool,
    computer: Option<Value>,
    path: Option<(String, String)>,
    rows: Vec<Folder>,
    clones: bool,
    clones_here: bool,
    target: Option<String>,
    clone_ready: bool,
}

pub struct AddProject {
    /// None on a dialog staged from fixtures, which asks nothing.
    host: Option<Host>,
    computers: Vec<Value>,
    on: Option<String>,
    /// The paths recorded as projects on this computer.
    added: HashSet<String>,
    field: Entity<InputState>,
    clone_to: Entity<InputState>,
    repos: Option<Listing>,
    level: Option<Listing>,
    lit: usize,
    refusal: Option<String>,
    adding: bool,
    icon: Entity<SelectState<String>>,
    hue: Entity<SelectState<String>>,
    _read: Vec<Task<()>>,
    _level: Option<Task<()>>,
    _add: Option<Task<()>>,
    _subscriptions: Vec<Subscription>,
}

/// Stands Add a project on the layer, its field taking the keys.
pub fn open(host: &Host, window: &mut Window, cx: &mut App) -> Entity<AddProject> {
    if !std::mem::replace(&mut cx.default_global::<TabBound>().0, true) {
        cx.bind_keys([KeyBinding::new("tab", NoAction, Some(FIELD_KEYS))]);
    }
    let view = cx.new(|cx| AddProject::new(host.clone(), window, cx));
    layer::open(ADD_PROJECT, view.clone().into(), Closes::Outside, None, window, cx);
    view.update(cx, |this, cx| this.focus(window, cx));
    view
}

fn icon_items(hue: &str) -> Vec<SelectItem<String>> {
    ICONS
        .iter()
        .map(|icon| {
            let look = json!({ "projectLook": { "new": { "icon": icon, "hue": hue } } });
            SelectItem::new(icon.to_string(), capitalised(icon)).mark(move |cx| {
                let p = cx.palette();
                project_glyph(Some("new"), &look, &Images::default(), px(16.), p.foreground, true, p)
            })
        })
        .collect()
}

fn hue_items() -> Vec<SelectItem<String>> {
    HUES.iter()
        .map(|word| {
            let id = word.to_string();
            SelectItem::new(id.clone(), capitalised(word)).mark(move |cx| {
                div().size(px(12.)).rounded_full().bg(hue(&id, cx.palette()).unwrap_or(cx.palette().muted_foreground)).into_any_element()
            })
        })
        .collect()
}

fn ask<T: serde::de::DeserializeOwned>(params: Value) -> Result<T, Failure> {
    serde_json::from_value(params).map_err(|e| Failure::new(e.to_string()))
}

impl AddProject {
    pub fn new(host: Host, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut this = AddProject::with(Some(host), window, cx);
        this.read(window, cx);
        this
    }

    /// The dialog over fixture answers: the computers as `places.list` gives them, the repos under the home folder,
    /// and the paths already recorded.
    pub fn staged(computers: Vec<Value>, repos: Option<Listing>, added: &[&str], window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut this = AddProject::with(None, window, cx);
        this.computers = computers;
        this.repos = repos;
        this.added = added.iter().map(|a| a.to_string()).collect();
        this
    }

    fn with(host: Option<Host>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let field = cx.new(|cx| InputState::new(window, cx).placeholder(SEARCH));
        let clone_to = cx.new(|cx| InputState::new(window, cx));
        let icon = cx.new(|_| SelectState::new(icon_items("neutral"), Some(&"folder".to_owned())));
        let hue = cx.new(|_| SelectState::new(hue_items(), Some(&"neutral".to_owned())));
        let typed = cx.subscribe_in(&field, window, |this, _, e: &InputEvent, window, cx| {
            if matches!(e, InputEvent::Change) {
                this.typed(window, cx);
            }
        });
        let into = cx.subscribe_in(&clone_to, window, |this, _, e: &InputEvent, window, cx| match e {
            InputEvent::Change => {
                this.refusal = None;
                cx.notify();
            }
            InputEvent::PressEnter { .. } => this.add_target(window, cx),
            _ => {}
        });
        AddProject {
            host,
            computers: Vec::new(),
            on: None,
            added: HashSet::new(),
            field,
            clone_to,
            repos: None,
            level: None,
            lit: 0,
            refusal: None,
            adding: false,
            icon,
            hue,
            _read: Vec::new(),
            _level: None,
            _add: None,
            _subscriptions: vec![typed, into],
        }
    }

    /// The computers, the projects already recorded, and the repos under the home folder, read once per open.
    fn read(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(host) = self.host.clone() else { return };
        let places = host.run(|c| async move { c.call(ask::<PlacesListRequest>(json!({}))?).await.map(|r| r.places) });
        let projects = host.run(|c| async move { c.call(ask::<ProjectsListRequest>(json!({}))?).await.map(|r| r.projects) });
        let repos = host.run(|c| async move { c.call(ask::<HostFoldersRequest>(json!({ "hidden": false, "repos": true }))?).await });
        self._read.push(cx.spawn_in(window, async move |this, cx| {
            let places = places.await.unwrap_or_default();
            _ = this.update(cx, |this, cx| {
                let places: Vec<Value> = places.iter().filter_map(|p| serde_json::to_value(p).ok()).collect();
                this.computers = places
                    .into_iter()
                    .enumerate()
                    .filter(|(at, p)| *at == 0 || p.get("takesForks") == Some(&json!(true)))
                    .map(|(_, p)| p)
                    .collect();
                cx.notify();
            });
        }));
        self._read.push(cx.spawn_in(window, async move |this, cx| {
            let projects = projects.await.unwrap_or_default();
            _ = this.update(cx, |this, cx| {
                this.added = projects
                    .iter()
                    .filter_map(|p| serde_json::to_value(p).ok())
                    .filter(|p| p.get("computer").and_then(Value::as_str).is_none_or(|c| c == "here"))
                    .filter_map(|p| p.get("path")?.as_str().map(str::to_owned))
                    .collect();
                cx.notify();
            });
        }));
        self._read.push(cx.spawn_in(window, async move |this, cx| {
            let read = repos.await;
            _ = this.update(cx, |this, cx| {
                match read
                    .and_then(|f| serde_json::to_value(f.listing).and_then(serde_json::from_value).map_err(|e| Failure::new(e.to_string())))
                {
                    Ok(listing) => this.repos = Some(listing),
                    Err(f) => this.refusal = Some(f.message),
                }
                cx.notify();
            });
        }));
    }

    /// The field takes the keys, as `autoFocus` gives it them.
    pub fn focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.field.update(cx, |f, cx| f.focus(window, cx));
    }

    pub fn typed_value(&self, cx: &App) -> String {
        self.field.read(cx).value().to_string()
    }

    pub fn refusal(&self) -> Option<&str> {
        self.refusal.as_deref()
    }

    pub fn adding(&self) -> bool {
        self.adding
    }

    /// The rows the list shows now, by path.
    pub fn rows(&self, cx: &App) -> Vec<String> {
        self.shown(cx).rows.into_iter().map(|f| f.path).collect()
    }

    pub fn set_field(&mut self, typed: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.field.update(cx, |f, cx| f.set_value(typed.to_owned(), window, cx));
        self.typed(window, cx);
    }

    pub fn set_clone_to(&mut self, folder: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.clone_to.update(cx, |f, cx| f.set_value(folder.to_owned(), window, cx));
        cx.notify();
    }

    fn home(&self) -> String {
        self.repos.as_ref().map(|r| r.dir.clone()).unwrap_or_default()
    }

    fn computer(&self) -> Option<&Value> {
        self.on
            .as_ref()
            .and_then(|on| self.computers.iter().find(|c| c.get("id").and_then(Value::as_str) == Some(on)))
            .or(self.computers.first())
    }

    fn shown(&self, cx: &App) -> Shown {
        let field = self.typed_value(cx);
        let computer = self.computer().cloned();
        let here = computer.is_none() || computer.as_ref() == self.computers.first();
        let shape = repository_word(&field);
        let query = field.trim().to_lowercase();
        let searched: Vec<Folder> = self
            .repos
            .iter()
            .flat_map(|r| &r.folders)
            .filter(|f| query.is_empty() || f.path.to_lowercase().contains(&query))
            .cloned()
            .collect();
        let repo_word = shape == Some(RepoWord::Address) || (shape == Some(RepoWord::Short) && (!here || searched.is_empty()));
        let typing_path = !repo_word && (field.starts_with('/') || field.starts_with('~'));
        let path = typing_path.then(|| path_parts(&field, &self.home()));
        let rows = match (&path, here && !repo_word) {
            (_, false) => Vec::new(),
            (Some((dir, stem)), true) => match &self.level {
                Some(level) if &level.dir == dir => {
                    level.folders.iter().filter(|f| base_name(&f.path).to_lowercase().starts_with(stem.as_str())).cloned().collect()
                }
                _ => Vec::new(),
            },
            (None, true) => searched,
        };
        let clones = !here && repo_word;
        let clones_here = here && repo_word;
        let folder = self.clone_to.read(cx).value().trim().to_owned();
        let clone_ready = clones_here && (folder.starts_with('/') || folder.starts_with('~'));
        let lit = rows.get(self.lit);
        let target =
            if clones || clone_ready { Some(field.trim().to_owned()) } else { lit.filter(|r| self.addable(r)).map(|r| r.path.clone()) };
        Shown { here, computer, path, rows, clones, clones_here, target, clone_ready }
    }

    fn addable(&self, row: &Folder) -> bool {
        row.repo && !self.added.contains(&row.path)
    }

    /// A typed path lists the folder it names, one level, as the person types into it.
    fn typed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.refusal = None;
        self.lit = 0;
        cx.notify();
        let Some((dir, _)) = self.shown(cx).path else { return };
        if self.level.as_ref().is_some_and(|l| l.dir == dir) {
            return;
        }
        let Some(host) = self.host.clone() else { return };
        let asked = dir.clone();
        let listed = host.run(move |c| async move { c.call(ask::<HostFoldersRequest>(json!({ "dir": asked }))?).await });
        self._level = Some(cx.spawn_in(window, async move |this, cx| {
            let read = listed.await;
            _ = this.update(cx, |this, cx| {
                match read
                    .and_then(|f| serde_json::to_value(f.listing).and_then(serde_json::from_value).map_err(|e| Failure::new(e.to_string())))
                {
                    Ok(listing) => {
                        this.level = Some(listing);
                        this.refusal = None;
                    }
                    Err(f) => this.refusal = Some(f.message),
                }
                cx.notify();
            });
        }));
    }

    fn pick_computer(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        self.on = Some(id);
        self.lit = 0;
        let placeholder = if self.shown(cx).here { SEARCH } else { ADDRESS_ONLY };
        self.field.update(cx, |f, cx| f.set_placeholder(placeholder, window, cx));
        cx.notify();
    }

    fn open_folder(&mut self, folder: &Folder, window: &mut Window, cx: &mut Context<Self>) {
        let typed = format!("{}/", tilde(&folder.path, &self.home()));
        self.set_field(&typed, window, cx);
    }

    pub fn add_target(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let shown = self.shown(cx);
        if let Some(target) = shown.target {
            let into = shown.clone_ready.then(|| self.clone_to.read(cx).value().trim().to_owned());
            self.add(target, into, window, cx);
        }
    }

    fn add(&mut self, source: String, into: Option<String>, window: &mut Window, cx: &mut Context<Self>) {
        let Some(host) = self.host.clone().filter(|_| !self.adding) else { return };
        self.adding = true;
        self.refusal = None;
        cx.notify();
        let shown = self.shown(cx);
        let on = (!shown.here).then(|| shown.computer.as_ref().and_then(|c| c.get("id")?.as_str().map(str::to_owned))).flatten();
        let (icon, hue) = (self.icon.read(cx).value().cloned(), self.hue.read(cx).value().cloned());
        let look = (icon.as_deref() != Some("folder") || hue.as_deref() != Some("neutral")).then(|| json!({ "icon": icon, "hue": hue }));
        let mut params = json!({ "source": source });
        if let Some(on) = on {
            params["on"] = json!(on);
        }
        if let Some(into) = into {
            params["into"] = json!(into);
        }
        let added = host.run(move |c| async move {
            let added = c.call(ask::<ProjectsAddRequest>(params)?).await?;
            if let Some(look) = look {
                let id = added.project.id.clone();
                _ = c.call(ask::<PreferencesSetRequest>(json!({ "patch": { "projectLook": { id: look } } }))?).await;
            }
            Ok(())
        });
        self._add = Some(cx.spawn_in(window, async move |this, cx| {
            let added = added.await;
            _ = this.update_in(cx, |this, window, cx| {
                this.adding = false;
                match added {
                    Ok(()) => layer::close(ADD_PROJECT, window, cx),
                    Err(f) => this.refusal = Some(f.message),
                }
                cx.notify();
            });
        }));
    }

    /// A folder the system's picker gave, added as it stands.
    pub fn add_folder(&mut self, folder: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.add(folder.to_owned(), None, window, cx);
    }

    /// The system's folder picker; `then` takes the folder picked.
    fn choose(&mut self, then: fn(&mut Self, String, &mut Window, &mut Context<Self>), window: &mut Window, cx: &mut Context<Self>) {
        let picked = cx.prompt_for_paths(PathPromptOptions { files: false, directories: true, multiple: false, prompt: None });
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(paths))) = picked.await else { return };
            let Some(path) = paths.into_iter().next() else { return };
            _ = this.update_in(cx, |this, window, cx| then(this, path.to_string_lossy().into_owned(), window, cx));
        })
        .detach();
    }

    fn key(&mut self, e: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let shown = self.shown(cx);
        match e.keystroke.key.as_str() {
            "down" => self.lit = (self.lit + 1).min(shown.rows.len().saturating_sub(1)),
            "up" => self.lit = self.lit.saturating_sub(1),
            "tab" if !e.keystroke.modifiers.shift => {
                let Some(row) = shown.rows.get(self.lit).cloned() else {
                    if let Ok(tab) = cx.build_action("root::Tab", None) {
                        window.dispatch_action(tab, cx);
                    }
                    return;
                };
                cx.stop_propagation();
                match shown.path {
                    Some(_) => self.open_folder(&row, window, cx),
                    None => self.set_field(&tilde(&row.path, &self.home()), window, cx),
                }
                return;
            }
            "enter" if !self.clone_to.read(cx).focus_handle(cx).is_focused(window) => {
                cx.stop_propagation();
                if shown.target.is_some() {
                    return self.add_target(window, cx);
                }
                if let (Some(_), Some(row)) = (&shown.path, shown.rows.get(self.lit).cloned()) {
                    self.open_folder(&row, window, cx);
                }
                return;
            }
            _ => return,
        }
        cx.stop_propagation();
        cx.notify();
    }

    fn list(&self, shown: &Shown, field: &str, p: &Palette, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let say = |words: String| {
            div().px_3().pt_3().text_size(px(14.)).line_height(px(20.)).text_color(p.muted_foreground).child(words).into_any_element()
        };
        let name = shown.computer.as_ref().map(place_name).unwrap_or_default();
        if shown.clones {
            let me = cx.weak_entity();
            let source = field.trim().to_owned();
            let line = clone_line(&source, &name);
            return vec![pane_row("clone", p)
                .bg(p.accent)
                .text_color(p.foreground)
                .on_click(move |_, window, cx| _ = me.update(cx, |this, cx| this.add(source.clone(), None, window, cx)))
                .child(Icon::new(Glyph::GitBranch).with_size(px(16.)).text_color(p.muted_foreground))
                .child(div().min_w_0().truncate().child(line))
                .into_any_element()];
        }
        if !shown.here {
            let provider = shown.computer.as_ref().and_then(|c| c.get("kind")?.as_str()) == Some("provider");
            return vec![say(if provider { provider_says(&name) } else { box_says(&name) })];
        }
        if shown.clones_here {
            let me = cx.weak_entity();
            return vec![v_flex()
                .debug_selector(|| "clone-into".into())
                .w_full()
                .gap_2()
                .px_3()
                .py_3()
                .rounded(px(ROW_RADIUS))
                .bg(p.accent)
                .text_size(px(14.))
                .text_color(p.foreground)
                .child(
                    h_flex()
                        .min_w_0()
                        .gap_3()
                        .child(Icon::new(Glyph::GitBranch).with_size(px(16.)).text_color(p.muted_foreground))
                        .child(div().min_w_0().truncate().child(clone_into(field.trim()))),
                )
                .child(
                    h_flex()
                        .min_w_0()
                        .gap_2()
                        .pl(px(28.))
                        .child(div().flex_1().min_w_0().child(Field::new(&self.clone_to).sheet().held(self.adding)))
                        .child(Button::outline("clone-into-choose", CHOOSE).glyph(Glyph::FolderOpen).on_click(move |_, window, cx| {
                            _ = me.update(cx, |this, cx| {
                                this.choose(|this, picked, window, cx| this.set_clone_to(&picked, window, cx), window, cx)
                            });
                        })),
                )
                .into_any_element()];
        }
        if self.repos.is_none() && shown.path.is_none() {
            return Vec::new();
        }
        if shown.rows.is_empty() {
            let words = if shown.path.is_some() {
                NO_FOLDERS
            } else if field.trim().is_empty() {
                NO_REPOS
            } else {
                NO_MATCH
            };
            return vec![say(words.to_owned())];
        }
        let home = self.home();
        shown
            .rows
            .iter()
            .enumerate()
            .map(|(at, row)| {
                let added = self.added.contains(&row.path);
                let lit = at == self.lit;
                let me = cx.weak_entity();
                let (clicked, addable) = (row.clone(), self.addable(row));
                let ink = if added {
                    p.muted_foreground
                } else if lit {
                    p.foreground
                } else {
                    p.foreground.opacity(0.9)
                };
                pane_row(SharedString::from(format!("folder-{at}")), p)
                    .debug_selector(|| "folder-row".into())
                    .text_color(ink)
                    .when(lit, |el| el.bg(p.accent))
                    .when(!lit && !added, |el| el.hover(|s| s.bg(p.accent.opacity(0.6))))
                    .when(added, |el| el.cursor_default())
                    .on_hover(cx.listener(move |this, over: &bool, _, cx| {
                        if *over && this.lit != at {
                            this.lit = at;
                            cx.notify();
                        }
                    }))
                    .on_click(move |_, window, cx| {
                        _ = me.update(cx, |this, cx| {
                            if addable {
                                this.add(clicked.path.clone(), None, window, cx);
                            } else if !clicked.repo {
                                this.open_folder(&clicked, window, cx);
                            }
                        });
                    })
                    .child(
                        Icon::new(if row.repo { Glyph::FolderGit } else { Glyph::Folder })
                            .with_size(px(16.))
                            .text_color(p.muted_foreground),
                    )
                    .child(div().flex_none().truncate().child(base_name(&row.path).to_owned()))
                    .child(match shown.path {
                        None => div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .font_family(cx.theme().mono_font_family.clone())
                            .text_size(text::XS.0)
                            .text_color(p.muted_foreground)
                            .child(tilde(&row.path, &home)),
                        Some(_) => div().flex_1(),
                    })
                    .children(if added {
                        Some(div().flex_none().text_size(text::XS.0).text_color(p.muted_foreground).child(ADDED))
                    } else {
                        row.branch.clone().map(|branch| {
                            div()
                                .flex_none()
                                .max_w(px(160.))
                                .truncate()
                                .font_family(cx.theme().mono_font_family.clone())
                                .text_size(text::XS.0)
                                .text_color(p.muted_foreground)
                                .child(branch)
                        })
                    })
                    .into_any_element()
            })
            .collect()
    }

    fn side(&self, shown: &Shown, p: &Palette, cx: &mut Context<Self>) -> gpui_kit::Stateful<gpui_kit::Div> {
        let label = |words: &'static str| {
            h_flex()
                .flex_none()
                .h(px(40.))
                .px(px(10.))
                .text_size(text::XS.0)
                .line_height(text::XS.1)
                .text_color(p.muted_foreground)
                .child(words)
        };
        let picked = shown.computer.as_ref().and_then(|c| c.get("id")?.as_str().map(str::to_owned));
        let rows = self.computers.iter().enumerate().map(|(at, place)| {
            let id = place.get("id").and_then(Value::as_str).unwrap_or_default().to_owned();
            let on = picked.as_deref() == Some(id.as_str());
            let glyph = if at == 0 {
                Glyph::Laptop
            } else if place.get("kind").and_then(Value::as_str) == Some("provider") {
                Glyph::Cloud
            } else {
                Glyph::Server
            };
            let me = cx.weak_entity();
            side_row(SharedString::from(format!("computer-{id}")), on, p)
                .on_click(move |_, window, cx| _ = me.update(cx, |this, cx| this.pick_computer(id.clone(), window, cx)))
                .child(Icon::new(glyph).with_size(px(16.)))
                .child(div().min_w_0().truncate().child(place_name(place)))
        });
        let host = self.host.clone();
        let add_computer = side_row("add-computer", false, p)
            .on_click(move |_, window, cx| {
                layer::close(ADD_PROJECT, window, cx);
                if let Some(host) = &host {
                    open_add_computer(host, None, window, cx);
                }
            })
            .child(Icon::new(Glyph::Plus).with_size(px(16.)))
            .child(div().min_w_0().truncate().child(ADD_COMPUTER));
        let width = px(SIDE - 16. - 20.);
        let icon = self.icon.clone();
        v_flex()
            .id("add-project-side")
            .w(px(SIDE))
            .flex_none()
            .min_h_0()
            .overflow_y_scroll()
            .p_2()
            .pt_0()
            .child(label(COMPUTERS))
            .children(rows)
            .child(add_computer)
            .child(div().mt_4().child(label(LOOK)))
            .child(
                v_flex()
                    .debug_selector(|| "new-project-look".into())
                    .gap_2()
                    .px(px(10.))
                    .child(Select::new(&self.icon).width(width))
                    .child(Select::new(&self.hue).width(width).on_change(move |hue: &String, _, cx| {
                        let items = icon_items(hue);
                        icon.update(cx, |icon, cx| icon.set_items(items, cx));
                    })),
            )
    }
}

/// `PANE_ROW`: 40 px, `rounded-lg`, `px-3 gap-3`, the small text.
fn pane_row(id: impl Into<SharedString>, _p: &Palette) -> gpui_kit::Stateful<gpui_kit::Div> {
    h_flex()
        .id(id.into())
        .flex_none()
        .h(px(40.))
        .w_full()
        .min_w_0()
        .gap_3()
        .px_3()
        .rounded(px(ROW_RADIUS))
        .cursor_pointer()
        .text_size(px(14.))
        .line_height(px(20.))
}

/// `SIDE_ROW`: 36 px, `rounded-lg`, `px-2.5 gap-2.5`; the picked one on the accent, the rest muted until hovered.
fn side_row(id: impl Into<SharedString>, on: bool, p: &Palette) -> gpui_kit::Stateful<gpui_kit::Div> {
    let (accent, over, fore) = (p.accent, p.accent.opacity(0.6), p.foreground);
    h_flex()
        .id(id.into())
        .flex_none()
        .h(px(36.))
        .w_full()
        .min_w_0()
        .gap(px(10.))
        .px(px(10.))
        .rounded(px(ROW_RADIUS))
        .cursor_pointer()
        .text_size(px(14.))
        .line_height(px(20.))
        .when(on, |el| el.bg(accent).text_color(fore))
        .when(!on, |el| el.text_color(p.muted_foreground).hover(move |s| s.bg(over).text_color(fore)))
}

fn hint(keys: &str, word: &'static str, cx: &App) -> impl IntoElement {
    h_flex()
        .gap(px(6.))
        .text_size(text::XS.0)
        .line_height(text::XS.1)
        .text_color(cx.palette().muted_foreground)
        .child(keycap(SharedString::from(keys.to_owned()), cx))
        .child(word)
}

impl Render for AddProject {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = cx.palette().clone();
        let field = self.typed_value(cx);
        let shown = self.shown(cx);
        let me = cx.weak_entity();
        let choose = shown.here.then(|| {
            let me = me.clone();
            Button::outline("choose", CHOOSE).glyph(Glyph::FolderOpen).on_click(move |_, window, cx| {
                _ = me.update(cx, |this, cx| this.choose(|this, picked, window, cx| this.add_folder(&picked, window, cx), window, cx));
            })
        });
        let add = Button::primary("add", ADD)
            .glyph(Glyph::Plus)
            .busy(self.adding)
            .held(shown.target.is_none() || self.adding)
            .on_click(move |_, window, cx| _ = me.update(cx, |this, cx| this.add_target(window, cx)));
        let head = h_flex()
            .flex_none()
            .h(px(56.))
            .gap_3()
            .px_4()
            .border_b_1()
            .border_color(p.border)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .key_context(FIELD_KEYS)
                    .child(Input::new(&self.field).appearance(false).disabled(self.adding).text_size(px(15.)).px_0()),
            )
            .children(choose)
            .child(add)
            .child(keycap("esc".into(), cx));
        let home = self.home();
        let over_list = match &shown.path {
            Some((dir, _)) => div()
                .truncate()
                .font_family(cx.theme().mono_font_family.clone())
                .text_size(text::XS.0)
                .child(tilde(dir, &home))
                .into_any_element(),
            None if shown.here => div()
                .truncate()
                .text_size(text::XS.0)
                .line_height(text::XS.1)
                .child(repos_on(&shown.computer.as_ref().map(place_name).unwrap_or_default()))
                .into_any_element(),
            None => div().into_any_element(),
        };
        let rows = self.list(&shown, &field, &p, cx);
        let lists = v_flex()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .child(h_flex().flex_none().h(px(40.)).px_5().text_color(p.muted_foreground).child(over_list))
            .child(v_flex().id("add-project-list").flex_1().min_h_0().overflow_y_scroll().px_2().pb_2().children(rows));
        let lit_row = shown.rows.get(self.lit);
        let foot = h_flex().flex_none().h(px(44.)).gap_4().px_4().child(match &self.refusal {
            Some(said) => div()
                .id("add-project-refusal")
                .debug_selector(|| "add-project-refusal".into())
                .tooltip(tip(said.clone()))
                .min_w_0()
                .truncate()
                .text_size(px(13.))
                .text_color(p.error_foreground)
                .child(said.clone())
                .into_any_element(),
            None => h_flex()
                .gap_4()
                .child(hint("↑↓", NAVIGATE, cx))
                .child(hint("↵", if shown.path.is_some() && lit_row.is_some_and(|r| !r.repo) { OPEN } else { ADD }, cx))
                .child(hint("/", BY_PATH, cx))
                .into_any_element(),
        });
        let narrow = crate::ui::narrow(window);
        let width = (window.viewport_size().width - px(32.)).min(px(WIDTH));
        v_flex()
            .id("add-project")
            .debug_selector(|| "add-project".into())
            .w(width)
            .overflow_hidden()
            .rounded(size::DIALOG_RADIUS)
            .border_1()
            .border_color(p.popover_edge)
            .bg(p.popover)
            .shadow(p.dialog_shadow.clone())
            .text_color(p.foreground)
            .on_key_down(cx.listener(Self::key))
            .child(head)
            .child(
                h_flex()
                    .h(px(LIST_HEIGHT))
                    .items_start()
                    .child(lists.h_full())
                    .when(!narrow, |el| el.child(self.side(&shown, &p, cx).h_full())),
            )
            .child(foot)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_word_reads_as_the_protocol_reads_a_source() {
        assert_eq!(repository_word("https://github.com/a/b"), Some(RepoWord::Address));
        assert_eq!(repository_word("git@github.com:a/b.git"), Some(RepoWord::Address));
        assert_eq!(repository_word("github.com/a/b"), Some(RepoWord::Address));
        assert_eq!(repository_word("a/b"), Some(RepoWord::Short));
        assert_eq!(repository_word("~/code"), None);
        assert_eq!(repository_word("me@box"), None);
        assert_eq!(repository_word("api"), None);
        assert_eq!(repo_name("https://github.com/a/wsp.git"), "wsp");
        assert_eq!(repo_name("a/b"), "b");
    }

    #[test]
    fn a_typed_path_lists_its_folder_and_matches_its_stem() {
        assert_eq!(path_parts("~/co", "/home/me"), ("/home/me".into(), "co".into()));
        assert_eq!(path_parts("/Us", "/home/me"), ("/".into(), "us".into()));
        assert_eq!(path_parts("/srv/app/", ""), ("/srv/app".into(), "".into()));
        assert_eq!(tilde("/home/me/code", "/home/me"), "~/code");
        assert_eq!(tilde("/home/meow", "/home/me"), "/home/meow");
    }
}
