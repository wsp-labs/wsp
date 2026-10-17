// SPDX-License-Identifier: AGPL-3.0-only
//! Projects and each project's page (`projects.tsx`): one row per project, its source and the computer it lives on,
//! opening the project's own page; that page says the record's facts, its look, and the one act on it. Remove is held
//! while a workspace stands on the project, in the runtime's own sentence, and otherwise asks first.

pub mod look;
mod new_threads;

use std::rc::Rc;

use gpui_kit::component::{Icon, Sizable as _};
use gpui_kit::{prelude::*, px, AnyElement, App, Entity, SharedString, Window};
use serde_json::{json, Value};
use wsp_host_client::ops::sidebar::{Sessions, Workspaces};
use wsp_host_client::protocol::{
    PlaceView, ProjectView, ProjectsIconRequest, ProjectsRemoveRequest, SessionView, SessionsListRequest, WorkspaceOut,
    WorkspacesListRequest,
};

use super::computers::place_name;
use super::devices::CANCEL;
use crate::screens::settings::model::{req, At, GroupId};
use crate::screens::settings::page::{CardData, Ctx, Item};
use crate::ui::{framed, word, ActivePalette, AlertDialog, Button, HeadRow, Refusal, Select, SelectItem, WordClass, Words};

/// `PROJECTS_WORDS`.
const LOOK: &str = "Look";
const ICON: &str = "Icon";
const ICON_DESCRIPTION: &str = "A glyph, or an image you choose. Drawn wherever the project shows.";
const HUE: &str = "Colour";
const HUE_DESCRIPTION: &str = "The icon's colour, so the project reads at a glance.";
const ADD: &str = "Add a project";
const NONE: &str = "No projects yet.";
const NONE_DESCRIPTION: &str = "A project is a folder on one of your computers.";
const REPOSITORY: &str = "Repository";
const OPEN: &str = "Open";
const REMOVE: &str = "Remove";
const REMOVE_HERE: &str = "Its record leaves this wsp. Your folder stays as it is.";
const HERE: &str = "here";
/// The icon select's value while the project wears an image of its own.
const IMAGE: &str = "image";

/// `PROJECT_WORDS.notRead`.
fn not_read(said: &str) -> String {
    format!("Projects not read: {said}")
}

fn where_line(source: &str, computer: &str) -> String {
    format!("{source} on {computer}")
}

fn threads_line(n: usize, running: usize) -> String {
    let noun = if n == 1 { "thread" } else { "threads" };
    if running == 0 {
        format!("{n} {noun}")
    } else {
        format!("{n} {noun}, {running} running")
    }
}

fn in_use(n: usize) -> String {
    if n == 1 {
        "A workspace still uses it. Delete it first.".to_owned()
    } else {
        format!("{n} workspaces still use it. Delete them first.")
    }
}

/// The word for where a project comes from, by the kind of its source: the folder, the url or the repository.
fn source_word(project: &ProjectView) -> String {
    let source = serde_json::to_value(&project.source).unwrap_or(Value::Null);
    ["path", "url", "repo"].iter().find_map(|k| source.get(*k).and_then(Value::as_str)).unwrap_or_default().to_owned()
}

/// The computer the host runs on, by its own name; empty until the places list holds it.
fn here_name(places: &[PlaceView]) -> String {
    places.iter().find(|p| p.id == HERE).map(place_name).unwrap_or_default()
}

fn is_provider(place: &PlaceView) -> bool {
    place.kind.to_string() == "provider"
}

/// The one line a remove says by the computer's kind, matching the runtime's three landings.
fn remove_line(places: &[PlaceView], project: &ProjectView) -> String {
    match places.iter().find(|p| p.id == project.computer).filter(|_| project.computer != HERE) {
        None => REMOVE_HERE.to_owned(),
        Some(place) if is_provider(place) => format!("Its record leaves this wsp, and its image at {} with it.", place_name(place)),
        Some(place) => format!("Its record leaves this wsp, and wsp's own clone of it on {} goes with it.", place_name(place)),
    }
}

/// The workspaces and threads this host records, read as the page opens: what a project's count and its removal's
/// refusal are counted from.
#[derive(Default)]
struct Standing {
    workspaces: Vec<WorkspaceOut>,
    sessions: Vec<SessionView>,
}

impl Standing {
    fn sessions_on(&self, workspace: &str) -> impl Iterator<Item = &SessionView> {
        let workspace = workspace.to_owned();
        self.sessions.iter().filter(move |s| s.workspace_id == workspace)
    }

    /// `bareFolder`: a workspace in a project's own folder that holds no thread is the folder, not a workspace on it.
    fn bare(&self, workspace: &WorkspaceOut) -> bool {
        let in_folder = workspace.kind.as_ref().is_some_and(|k| matches!(k.to_string().as_str(), "local" | "place"));
        in_folder && self.sessions_on(&workspace.id).next().is_none()
    }

    /// `workspacesOn`: the workspaces standing on a project, which its removal is refused for.
    fn on(&self, project: &str) -> Vec<&WorkspaceOut> {
        self.workspaces.iter().filter(|w| w.project.id == project && !self.bare(w)).collect()
    }

    /// The threads on a project's workspaces, one per thread however many attempts it took, and how many run.
    fn threads(&self, project: &str) -> (usize, usize) {
        let mut threads: Vec<(&str, bool)> = Vec::new();
        for workspace in self.on(project) {
            for session in self.sessions_on(&workspace.id) {
                let key = session.thread_id.as_deref().unwrap_or(&session.id);
                let running = session.status.to_string() == "running";
                match threads.iter_mut().find(|(k, _)| *k == key) {
                    Some(thread) => thread.1 |= running,
                    None => threads.push((key, running)),
                }
            }
        }
        (threads.len(), threads.iter().filter(|(_, running)| *running).count())
    }
}

/// The page's own read of the workspaces and threads, started once while the page is drawn.
fn standing(ctx: &Ctx, window: &mut Window, cx: &mut App) -> Entity<Standing> {
    let acts = ctx.acts.clone();
    window.use_keyed_state("projects-standing", cx, move |_, cx| {
        let (into, again) = (cx.weak_entity(), acts.clone());
        acts.call(req::<WorkspacesListRequest>(json!({})), cx, move |answer, cx| {
            if let Ok(Workspaces { workspaces }) = answer {
                _ = into.update(cx, |s: &mut Standing, _| s.workspaces = workspaces);
                again.notify(cx);
            }
        });
        let (into, again) = (cx.weak_entity(), acts.clone());
        acts.call(req::<SessionsListRequest>(json!({})), cx, move |answer, cx| {
            if let Ok(Sessions { sessions }) = answer {
                _ = into.update(cx, |s: &mut Standing, _| s.sessions = sessions);
                again.notify(cx);
            }
        });
        Standing::default()
    })
}

/// The one Add under the list; this app draws no Add a project dialog, so the press says so.
fn add_button(ctx: &Ctx) -> Button {
    let acts = ctx.acts.clone();
    Button::add("add-project-button", ADD).on_click(move |_, _, cx| acts.note("Add a project is not in this app yet.", cx))
}

pub fn cards(ctx: &Ctx, window: &mut Window, cx: &mut App) -> Vec<CardData> {
    let card = CardData::new("projects");
    if let Some(said) = &ctx.data.projects_refused {
        return vec![card.under(gpui_kit::div().flex_1().child(Refusal::new().said(not_read(said)))).under(add_button(ctx))];
    }
    let projects = ctx.data.projects_in_order();
    if projects.is_empty() {
        return vec![card.item(Item::empty("none", format!("{NONE} {NONE_DESCRIPTION}"))).under(add_button(ctx))];
    }
    let standing = standing(ctx, window, cx);
    let standing = standing.read(cx);
    let places = &ctx.data.places;
    let here = here_name(places);
    let rows: Vec<Item> = projects
        .into_iter()
        .map(|project| {
            let computer = if project.computer == HERE {
                here.clone()
            } else {
                places.iter().find(|p| p.id == project.computer).map_or_else(|| project.computer.clone(), place_name)
            };
            let note = Words::from(vec![computer, source_word(project)]);
            let words = vec![SharedString::from(project.name.clone()), note.line()];
            let count = standing.on(&project.id).len().to_string();
            let (acts, at) = (ctx.acts.clone(), At::Project(project.id.clone()));
            let lead = framed(look::mark(&look::project_icon(ctx, &project.id), &look::project_hue(ctx, &project.id), cx), cx);
            let mut item = Item::row(project.id.clone(), project.name.clone(), None, |row| {
                row.lead(lead).note(note).mono().fact(count).on_open(move |_, window, cx| acts.go(at.clone(), window, cx))
            });
            item.words = words;
            item
        })
        .collect();
    vec![card.items(rows).under(add_button(ctx))]
}

/// The repository's page in the browser, where the remote is one a browser can open.
fn remote_url(remote: &str) -> Option<String> {
    if remote.starts_with("http://") || remote.starts_with("https://") {
        return Some(remote.to_owned());
    }
    let rest = remote.strip_prefix("git@")?;
    let (host, path) = rest.split_once(':')?;
    let path = path.strip_suffix(".git").unwrap_or(path);
    (!host.is_empty() && !path.is_empty()).then(|| format!("https://{host}/{path}"))
}

/// One project's own page.
pub fn page(id: &str, ctx: &Ctx, window: &mut Window, cx: &mut App) -> Vec<CardData> {
    let Some(project) = ctx.data.projects.iter().find(|p| p.id == id) else { return Vec::new() };
    let places = &ctx.data.places;
    let computer = if project.computer == HERE {
        here_name(places)
    } else {
        places.iter().find(|p| p.id == project.computer).map_or_else(|| project.computer.clone(), place_name)
    };
    let standing = standing(ctx, window, cx);
    let (standing_on, (threads, running)) = {
        let s = standing.read(cx);
        (s.on(&project.id).len(), s.threads(&project.id))
    };
    let refusal = (standing_on > 0).then(|| in_use(standing_on));
    let line = remove_line(places, project);
    let icon = look::project_icon(ctx, &project.id);
    let hue = look::project_hue(ctx, &project.id);
    let image = ctx.data.preferences().and_then(|p| p.project_icon.get(&project.id)).is_some();

    let head = HeadRow::new("project-head", look::mark(&icon, &hue, cx), project.name.clone())
        .line(where_line(&source_word(project), &computer))
        .slot(word(threads_line(threads, running), WordClass::Value, cx));
    let mut facts = CardData::new("project").item(Item::any("project-head", vec![project.name.clone().into()], head));
    if !project.remote.is_empty() {
        let open =
            remote_url(&project.remote).map(|url| Button::outline("remote-open", OPEN).xs().on_click(move |_, _, cx| cx.open_url(&url)));
        facts = facts.item(Item::row("remote", REPOSITORY, Some(&project.remote), |row| {
            let row = row.mono();
            match open {
                Some(open) => row.control(open),
                None => row,
            }
        }));
    }

    let set_look = {
        let (acts, id) = (ctx.acts.clone(), project.id.clone());
        move |icon: &str, hue: &str, cx: &mut App| {
            acts.set_preferences(json!({ "projectLook": { id.clone(): { "icon": icon, "hue": hue } } }), cx)
        }
    };
    let icon_select = {
        let picked = if image { IMAGE.to_owned() } else { icon.clone() };
        let item_hue = hue.clone();
        let items = move || {
            let mut items = look::icon_items(&item_hue);
            if image {
                items.push(SelectItem::new(IMAGE.to_owned(), ICON));
            }
            items
        };
        let state = look::select_state(format!("project-{}-icon", project.id).into(), items, format!("{hue} {image}"), &picked, window, cx);
        let (set_look, hue, clear) = (set_look.clone(), hue.clone(), image.then(|| clear_image(ctx, &project.id)));
        Select::new(&state).on_change(move |next: &String, _, cx| {
            if next == IMAGE {
                return;
            }
            set_look(next, &hue, cx);
            if let Some(clear) = &clear {
                clear(cx);
            }
        })
    };
    let hue_select = {
        let state = look::select_state(format!("project-{}-hue", project.id).into(), look::hue_items, String::new(), &hue, window, cx);
        let icon = icon.clone();
        Select::new(&state).on_change(move |next: &String, _, cx| set_look(&icon, next, cx))
    };
    let reset = image.then(|| clear_image(ctx, &project.id));
    let looks = CardData::new("look")
        .head(LOOK)
        .item(Item::row("icon", ICON, Some(ICON_DESCRIPTION), |row| {
            let row = row.control(icon_select);
            match reset {
                Some(reset) => row.on_reset(move |_, _, cx| reset(cx)),
                None => row,
            }
        }))
        .item(Item::row("hue", HUE, Some(HUE_DESCRIPTION), |row| row.control(hue_select)));

    let remove_note = refusal.clone().unwrap_or_else(|| line.clone());
    let control = remove_control(ctx, project, refusal.is_some(), line);
    let acts =
        CardData::new("acts").item(Item::row("remove", format!("Remove {}", project.name), Some(&remove_note), |row| row.control(control)));
    let threads = new_threads::card(&project.id, ctx, window, cx);
    [Some(facts), threads, Some(looks), Some(acts)].into_iter().flatten().collect()
}

/// Takes the image off the project, so its glyph shows again.
fn clear_image(ctx: &Ctx, id: &str) -> Rc<dyn Fn(&mut App)> {
    let (acts, id) = (ctx.acts.clone(), id.to_owned());
    Rc::new(move |cx| {
        let failed = acts.clone();
        acts.call(req::<ProjectsIconRequest>(json!({ "projectId": id, "png": null })), cx, move |answer, cx| {
            if let Err(said) = answer {
                failed.failed(said, cx);
            }
        });
    })
}

/// Remove: held in the runtime's sentence while a workspace stands on the project, else asked once, then the runtime's
/// answer as the toast and the list again.
fn remove_control(ctx: &Ctx, project: &ProjectView, held: bool, line: String) -> impl IntoElement {
    let acts = ctx.acts.clone();
    let (id, name) = (project.id.clone(), project.name.clone());
    Button::danger("remove-project", REMOVE).xs().held(held).on_click(move |_, _, cx| {
        let (id, name, line) = (id.clone(), name.clone(), line.clone());
        acts.open_dialog(
            Rc::new(move |ctx, window, cx| {
                let busy = window.use_keyed_state(SharedString::from(format!("remove-project-{id}-busy")), cx, |_, _| false);
                let cancel = {
                    let acts = ctx.acts.clone();
                    Button::outline("remove-project-cancel", CANCEL).neutral_ring().on_click(move |_, _, cx| acts.close_dialog(cx))
                };
                let (acts, id) = (ctx.acts.clone(), id.clone());
                let confirm = Button::destructive("remove-project-confirm", REMOVE).held(*busy.read(cx)).on_click(move |_, _, cx| {
                    busy.update(cx, |b, _| *b = true);
                    acts.notify(cx);
                    let (acts, busy) = (acts.clone(), busy.clone());
                    acts.clone().call(req::<ProjectsRemoveRequest>(json!({ "projectId": id })), cx, move |answer, cx| {
                        busy.update(cx, |b, _| *b = false);
                        match answer {
                            Ok(removed) => {
                                acts.close_dialog(cx);
                                acts.done(removed.said, cx);
                                acts.go_later(At::Group(GroupId::Projects), cx);
                            }
                            Err(said) => acts.failed(said, cx),
                        }
                    });
                });
                AlertDialog::new(format!("Remove {name}?")).sentence(line.clone()).act(cancel).act(confirm).into_any_element()
            }),
            cx,
        );
    })
}

/// The project's glyph as its row under Projects in the sidebar draws it: in its hue, else the sidebar's icon ink.
pub fn glyph(id: &str, ctx: &Ctx, cx: &App) -> Option<AnyElement> {
    ctx.data.projects.iter().any(|p| p.id == id).then(|| {
        let hue = look::project_hue(ctx, id);
        let ink = if hue == "neutral" { cx.palette().sidebar_icon_color } else { look::hue_ink(&hue, cx) };
        Icon::new(look::glyph_of(&look::project_icon(ctx, id))).with_size(px(16.)).flex_none().text_color(ink).into_any_element()
    })
}

/// The pages under Projects in the sidebar: one per project, in the person's order.
pub fn sub_pages(ctx: &Ctx) -> Vec<(At, SharedString)> {
    ctx.data.projects_in_order().into_iter().map(|p| (At::Project(p.id.clone()), p.name.clone().into())).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::screens::settings::model::Data;
    use crate::screens::settings::pages::devices::shape;

    #[gpui_kit::test]
    fn a_refused_or_empty_list_keeps_its_add(cx: &mut gpui_kit::TestAppContext) {
        let refused = Data { projects_refused: Some("the host is starting".into()), ..Data::default() };
        assert_eq!(shape(refused, cards, cx), [(vec![], 2, false)], "the refusal and Add a project under the card");
        let empty = shape(Data::default(), cards, cx);
        assert_eq!(empty, [(vec![vec![SharedString::from(format!("{NONE} {NONE_DESCRIPTION}"))]], 1, false)]);
        assert!(shape(Data::default(), |ctx, w, cx| page("nope", ctx, w, cx), cx).is_empty());
    }

    #[test]
    fn the_counts_read_as_the_web_says_them() {
        assert_eq!(threads_line(1, 0), "1 thread");
        assert_eq!(threads_line(3, 2), "3 threads, 2 running");
        assert_eq!(in_use(1), "A workspace still uses it. Delete it first.");
        assert_eq!(in_use(4), "4 workspaces still use it. Delete them first.");
    }

    #[test]
    fn a_remote_opens_where_a_browser_can() {
        assert_eq!(remote_url("git@github.com:wsp-labs/wsp.git").as_deref(), Some("https://github.com/wsp-labs/wsp"));
        assert_eq!(remote_url("https://gitlab.com/a/b").as_deref(), Some("https://gitlab.com/a/b"));
        assert_eq!(remote_url("/srv/git/x"), None);
    }

    #[test]
    fn a_bare_folder_is_no_workspace_on_its_project() {
        let workspace = |id: &str, kind: &str| -> WorkspaceOut {
            serde_json::from_value(json!({ "id": id, "name": id, "machineId": "m", "phase": "running", "kind": kind, "golden": "", "createdAt": "2026-10-17T00:00:00Z", "project": { "id": "p", "name": "p", "path": "/p", "computer": "here" } })).unwrap()
        };
        let session: SessionView =
            serde_json::from_value(json!({ "id": "s1", "workspaceId": "w2", "harness": "claude", "status": "running" })).unwrap();
        let standing = Standing {
            workspaces: vec![workspace("w1", "local"), workspace("w2", "local"), workspace("w3", "cloud")],
            sessions: vec![session],
        };
        assert_eq!(standing.on("p").iter().map(|w| w.id.as_str()).collect::<Vec<_>>(), ["w2", "w3"]);
        assert_eq!(standing.threads("p"), (1, 1));
    }
}
