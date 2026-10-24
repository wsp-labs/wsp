// SPDX-License-Identifier: AGPL-3.0-only
//! Projects and each project's page (`projects.tsx`): one row per project, its source and the computer it lives on,
//! opening the project's own page; that page says the record's facts, its look, and the one act on it. Remove is held
//! while a workspace stands on the project, in the runtime's own sentence, and otherwise asks first.

mod fit;
pub mod image;
pub mod look;
mod new_threads;
mod sheet;
pub mod standing;

use std::rc::Rc;

use gpui_kit::{prelude::*, px, AnyElement, App, SharedString, Window};
use serde_json::{json, Value};
use wsp_host_client::protocol::{PlaceView, ProjectView, ProjectsIconRequest, ProjectsRemoveRequest};

use super::computers::{is_provider, place_name};
use super::devices::CANCEL;
use crate::screens::settings::model::{req, At, GroupId, Reread};
use crate::screens::settings::page::{CardData, Ctx, DialogBuild, Item};
use crate::screens::settings::words::{here_name, HERE};
use crate::ui::{
    framed, tip, word, ActivePalette, AlertDialog, Button, Glyph, HeadRow, Refusal, Select, SelectItem, Taken, WordClass, Words,
};
use sheet::Source;
use standing::Count;

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
const ICON_IMAGE: &str = "Icon";
const CHOOSE_IMAGE: &str = "Choose an image";
const CHOOSE_ANOTHER: &str = "Choose another image";
/// What a count says where the host would not list the workspaces or the threads.
const NOT_READ: &str = "not read";
const THREADS_NOT_READ: &str = "Threads not read";
/// The icon select's value while the project wears an image of its own, and the foot item that opens the chooser.
const IMAGE: &str = "image";
const CHOOSE: &str = "choose";

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

/// The one line a remove says by the computer's kind, matching the runtime's three landings.
fn remove_line(places: &[PlaceView], project: &ProjectView) -> String {
    match places.iter().find(|p| p.id == project.computer).filter(|_| project.computer != HERE) {
        None => REMOVE_HERE.to_owned(),
        Some(place) if is_provider(place) => format!("Its record leaves this wsp, and its image at {} with it.", place_name(place)),
        Some(place) => format!("Its record leaves this wsp, and wsp's own clone of it on {} goes with it.", place_name(place)),
    }
}

/// The workspaces and threads the counts read, asked for the first time a projects page draws.
fn standing<'a>(ctx: &Ctx<'a>, cx: &mut App) -> &'a standing::Standing {
    if !ctx.data.standing.asked() {
        ctx.acts.reread(Reread::Standing, cx);
    }
    &ctx.data.standing
}

/// Why Add a project is held: its dialog comes with a later build (D117).
const ADD_LATER: &str = "Add a project opens in the next build of this app.";

/// An Add whose dialog this build does not draw: refused, its hover saying why (D117).
pub fn held_add(id: &'static str, label: &'static str, why: &'static str) -> AnyElement {
    let held = format!("{id}-held");
    gpui_kit::div()
        .id(SharedString::from(held.clone()))
        .debug_selector(move || held.clone())
        .tooltip(tip(why))
        .child(Button::add(id, label).held(true))
        .into_any_element()
}

/// The one Add under the list.
fn add_button() -> AnyElement {
    held_add("add-project-button", ADD, ADD_LATER)
}

pub fn cards(ctx: &Ctx, _window: &mut Window, cx: &mut App) -> Vec<CardData> {
    let card = CardData::new("projects");
    if let Some(said) = &ctx.data.projects_refused {
        let mut refusal = Refusal::new().said(not_read(said));
        if let Some(fix) = &ctx.data.projects_fix {
            refusal = refusal.fix(fix.clone());
        }
        return vec![card.under(gpui_kit::div().flex_1().min_w_0().child(refusal)).under(add_button())];
    }
    let projects = ctx.data.projects_in_order();
    if projects.is_empty() {
        return vec![card.item(Item::empty("none", format!("{NONE} {NONE_DESCRIPTION}"))).under(add_button())];
    }
    let standing = standing(ctx, cx);
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
            // A project nothing stands on reads 0, and so does one whose lists are on their way, as the web counts none.
            let count = match standing.on(&project.id) {
                Some(n) => n.to_string(),
                None if standing.refused().is_some() => NOT_READ.to_owned(),
                None => "0".to_owned(),
            };
            let (acts, at) = (ctx.acts.clone(), At::Project(project.id.clone()));
            let lead = framed(look::project_mark(ctx, &project.id, None, cx), cx);
            let mut item = Item::row(project.id.clone(), project.name.clone(), None, |row| {
                row.lead(lead).note(note).mono().fact(count).on_open(move |_, window, cx| acts.go(at.clone(), window, cx))
            });
            item.words = words;
            item
        })
        .collect();
    vec![card.items(rows).under(add_button())]
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
    let standing = standing(ctx, cx);
    let refusal = standing.on(&project.id).filter(|n| *n > 0).map(in_use);
    let line = remove_line(places, project);
    let icon = look::project_icon(ctx, &project.id);
    let hue = look::project_hue(ctx, &project.id);
    let image = ctx.data.preferences().and_then(|p| p.project_icon.get(&project.id)).map(|hash| hash.to_string());
    let landed = look::project_image(ctx, &project.id);

    let head = HeadRow::new("project-head", look::project_mark(ctx, &project.id, None, cx), project.name.clone())
        .line(where_line(&source_word(project), &computer));
    let head = match standing.threads(&project.id) {
        Count::Threads(n, running) => head.slot(word(threads_line(n, running), WordClass::Value, cx)),
        Count::NotRead => head.slot(word(THREADS_NOT_READ, WordClass::Value, cx)),
        Count::Reading => head,
    };
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
    let themes = ctx.data.preferences().map(|p| (p.light_theme.clone(), p.dark_theme.clone()));
    let icon_select = {
        let picked = if image.is_some() { IMAGE.to_owned() } else { icon.clone() };
        let signature = format!("{hue} {image:?} {}", landed.is_some());
        let (item_hue, worn, icon_word, landed) = (hue.clone(), image.is_some(), icon.clone(), landed.clone());
        let items = move || {
            let mut items = look::icon_items(&item_hue);
            items.push(
                SelectItem::new(CHOOSE.to_owned(), if worn { CHOOSE_ANOTHER } else { CHOOSE_IMAGE }).glyph(Glyph::Image).rule_above(),
            );
            if worn {
                let (landed, icon, hue) = (landed.clone(), icon_word.clone(), item_hue.clone());
                items.push(SelectItem::new(IMAGE.to_owned(), ICON_IMAGE).hidden().mark(move |cx| match &landed {
                    Some(image) => look::image_mark(image.clone(), px(16.), cx.palette().foreground),
                    None => look::mark(&icon, &hue, cx).into_any_element(),
                }));
            }
            items
        };
        let state = look::select_state(format!("project-{}-icon", project.id).into(), items, signature, &picked, window, cx);
        let (set_look, hue, clear) = (set_look.clone(), hue.clone(), image.is_some().then(|| clear_image(ctx, &project.id)));
        let (acts, id, themes) = (ctx.acts.clone(), project.id.clone(), themes.clone());
        Select::new(&state).on_change(move |next: &String, _, cx| match next.as_str() {
            CHOOSE => sheet::choose(&id, &acts, themes.clone(), cx),
            IMAGE => {}
            next => {
                set_look(next, &hue, cx);
                if let Some(clear) = &clear {
                    clear(cx);
                }
            }
        })
    };
    let hue_select = {
        let state = look::select_state(format!("project-{}-hue", project.id).into(), look::hue_items, String::new(), &hue, window, cx);
        let icon = icon.clone();
        Select::new(&state).on_change(move |next: &String, _, cx| set_look(&icon, next, cx))
    };
    let reset = image.is_some().then(|| clear_image(ctx, &project.id));
    let take = {
        let (acts, id) = (ctx.acts.clone(), project.id.clone());
        move |taken: Taken, _: &mut Window, cx: &mut App| {
            let source = match taken {
                Taken::Dropped(paths) => Source::of_paths(paths),
                Taken::Pasted => Source::of_clipboard(cx),
            };
            if let Some(source) = source {
                sheet::open(&id, source, &acts, themes.as_ref().map(|(l, d)| (l.as_str(), d.as_str())), cx);
            }
        }
    };
    let looks = CardData::new("look")
        .head(LOOK)
        .item(Item::row("icon", ICON, Some(ICON_DESCRIPTION), |row| {
            let row = row.control(gpui_kit::div().debug_selector(|| "project-icon".into()).child(icon_select)).on_take(take);
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
            DialogBuild::alert(move |ctx, window, cx| {
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
                AlertDialog::new(format!("Remove {name}?")).layered().sentence(line.clone()).act(cancel).act(confirm).into_any_element()
            }),
            cx,
        );
    })
}

/// The project's mark as its row under Projects in the sidebar draws it: its image, else its glyph in its hue, else in
/// the sidebar's icon ink.
pub fn glyph(id: &str, ctx: &Ctx, cx: &App) -> Option<AnyElement> {
    ctx.data.projects.iter().any(|p| p.id == id).then(|| look::project_mark(ctx, id, Some(cx.palette().sidebar_icon_color), cx))
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

    /// The Projects list drawn in Settings off `data`, in a window as the app draws it: the refusal slot's height.
    fn refusal_height(data: Data, cx: &mut gpui_kit::TestAppContext) -> gpui_kit::Pixels {
        use crate::screens::settings::SettingsScreen;
        let runtime = std::sync::Arc::new(tokio::runtime::Builder::new_current_thread().build().unwrap());
        let (_, cx) = cx.add_window_view(|window, cx| {
            let view = cx.new(|cx| SettingsScreen::with_data(data, At::Group(GroupId::Projects), runtime, window, cx));
            gpui_kit::component::Root::new(view, window, cx)
        });
        cx.simulate_resize(gpui_kit::size(gpui_kit::px(1280.), gpui_kit::px(800.)));
        cx.run_until_parked();
        cx.debug_bounds("refusal").expect("the refusal slot is drawn under the card").size.height
    }

    #[gpui_kit::test]
    fn a_refused_or_empty_list_keeps_its_add(cx: &mut gpui_kit::TestAppContext) {
        let said = "the host is starting and will answer once its records are read from disk ".repeat(16);
        let refused = Data { projects_refused: Some(said.clone()), ..Data::default() };
        assert_eq!(shape(refused, cards, cx), [(vec![], 2, false)], "the refusal and Add a project under the card");
        let quiet = refusal_height(Data { projects_refused: Some(String::new()), ..Data::default() }, cx);
        let spoken = refusal_height(Data { projects_refused: Some(said), ..Data::default() }, cx);
        assert!(spoken > quiet, "the slot says the host's words, which wrap past its two empty lines: {spoken:?} against {quiet:?}");
        let empty = shape(Data::default(), cards, cx);
        assert_eq!(empty, [(vec![vec![SharedString::from(format!("{NONE} {NONE_DESCRIPTION}"))]], 1, false)]);
        assert!(shape(Data::default(), |ctx, w, cx| page("nope", ctx, w, cx), cx).is_empty());
    }

    #[gpui_kit::test]
    fn a_refused_list_says_the_hosts_fix_after_its_words(cx: &mut gpui_kit::TestAppContext) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            crate::ui::init(crate::ui::Side::Light, cx);
        });
        let said = || Some("the host is starting.".to_owned());
        let bare = refusal_height(Data { projects_refused: said(), ..Data::default() }, cx);
        let fix = "Run wsp up on the computer it runs on, then open Settings again. ".repeat(8);
        let fixed = refusal_height(Data { projects_refused: said(), projects_fix: Some(fix), ..Data::default() }, cx);
        assert!(fixed > bare, "the fix follows the words in the slot: {fixed:?} against {bare:?}");
    }

    /// Settings standing on one project's page, drawn off `data` with no host behind it.
    fn project_page(cx: &mut gpui_kit::TestAppContext) -> &mut gpui_kit::VisualTestContext {
        settings_at(At::Project("p1".into()), cx)
    }

    /// Settings standing on `at` with one project, its workspaces and threads not read, and no host behind it.
    fn settings_at(at: At, cx: &mut gpui_kit::TestAppContext) -> &mut gpui_kit::VisualTestContext {
        use crate::screens::settings::SettingsScreen;
        cx.update(|cx| {
            gpui_kit::init(cx);
            crate::ui::init(crate::ui::Side::Light, cx);
        });
        let preferences = json!({ "theme": "light", "sidebarMode": "list", "terminalSize": "app", "terminalZoom": {}, "access": {},
            "projectLook": {}, "projectOrder": [], "labs": false });
        let project = json!({ "id": "p1", "name": "alpha", "computer": "here", "source": { "kind": "folder", "path": "/a" }, "path": "/a",
            "remote": "", "defaultBranch": "main", "memoryKey": "k", "memoryDir": "/m", "createdAt": "2026-10-17T00:00:00Z" });
        let data = Data {
            preferences: serde_json::from_value(preferences).ok(),
            projects: vec![serde_json::from_value(project).unwrap()],
            projects_read: true,
            ..Data::default()
        };
        let runtime = std::sync::Arc::new(tokio::runtime::Builder::new_current_thread().build().unwrap());
        let (_, cx) = cx.add_window_view(|window, cx| {
            let view = cx.new(|cx| SettingsScreen::with_data(data, at, runtime, window, cx));
            gpui_kit::component::Root::new(view, window, cx)
        });
        cx.simulate_resize(gpui_kit::size(gpui_kit::px(1280.), gpui_kit::px(1400.)));
        cx.run_until_parked();
        cx
    }

    #[gpui_kit::test]
    fn a_projects_count_reads_0_while_its_workspaces_are_read(cx: &mut gpui_kit::TestAppContext) {
        let cx = settings_at(At::Group(GroupId::Projects), cx);
        let count = cx.debug_bounds("word").expect("the project's row carries its count");
        assert!(count.size.width > gpui_kit::px(0.), "a 0 in the count, as the web counts none, not a blank: {count:?}");
    }

    fn png() -> Vec<u8> {
        let mut png = Vec::new();
        ::image::RgbaImage::from_pixel(64, 32, ::image::Rgba([230, 90, 12, 255]))
            .write_to(&mut std::io::Cursor::new(&mut png), ::image::ImageFormat::Png)
            .unwrap();
        png
    }

    #[gpui_kit::test]
    fn a_file_dropped_or_pasted_on_the_icon_row_opens_its_sheet(cx: &mut gpui_kit::TestAppContext) {
        use gpui_kit::{ClipboardItem, ExternalPaths, FileDropEvent, Image, ImageFormat};
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("icon.png");
        std::fs::write(&file, png()).unwrap();
        let cx = project_page(cx);
        let trigger = cx.debug_bounds("project-icon").expect("the icon select").center();
        assert!(cx.debug_bounds("project-icon-sheet").is_none());
        cx.simulate_event(FileDropEvent::Entered { position: trigger, paths: ExternalPaths(vec![file].into()) });
        cx.run_until_parked();
        cx.simulate_event(FileDropEvent::Submit { position: trigger });
        cx.run_until_parked();
        assert!(cx.debug_bounds("project-icon-sheet").is_some(), "a file dropped on the Icon row opens the sheet");
        assert!(cx.debug_bounds("project-icon-preview").is_some(), "the fitted image stands on the grounds");
        let standing = |cx: &mut gpui_kit::VisualTestContext| {
            cx.update(|_, cx| crate::screens::dialogs::layer::is_open(crate::screens::settings::DIALOG, cx))
        };
        assert!(standing(cx), "the sheet stands on the window's dialogs layer");
        cx.simulate_keystrokes("escape");
        cx.run_until_parked();
        assert!(!standing(cx), "Escape closes it");

        cx.write_to_clipboard(ClipboardItem::new_image(&Image::from_bytes(ImageFormat::Png, png())));
        // A press on the row's words, left of its control, gives the row the focus a paste reaches.
        cx.simulate_click(trigger - gpui_kit::point(gpui_kit::px(400.), gpui_kit::px(0.)), gpui_kit::Modifiers::none());
        cx.simulate_keystrokes(if cfg!(target_os = "macos") { "cmd-v" } else { "ctrl-v" });
        cx.run_until_parked();
        assert!(standing(cx), "a picture pasted on the Icon row opens the sheet");
        assert!(cx.debug_bounds("project-icon-preview").is_some(), "over it");
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
}
