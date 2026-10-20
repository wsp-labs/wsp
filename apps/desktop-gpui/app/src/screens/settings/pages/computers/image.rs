// SPDX-License-Identifier: AGPL-3.0-only
//! Your image on one computer, after `image.ts`, `imageState.ts` and `ImageCard.tsx` as a computer's or a cloud's page
//! draws it, the IMAGE list: one line of where the copy stands and what the image holds, and the one press that state
//! invites, Copy through image.build. A running build is its stage on the line; the build's own view (`ImageBuild.tsx`)
//! stands under it in a box only while it needs the person or once it ended short. The agents a cloud's image holds
//! stand above it, off the image's own record.

mod build;

use std::collections::HashMap;

use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::*;
use gpui_kit::{div, px, AnyElement, App, ScrollHandle, SharedString, Window};
use serde_json::json;
use wsp_host_client::ops::sidebar::Workspaces;
use wsp_host_client::protocol::{
    ImageBuildRequest, InitJob, InitJobRowsItem, PlaceView, ProjectView, SealedImageViewCopiesItem, SealedImageViewImage,
    SealedImageViewImageLoginsItemState as Login, WorkspaceOut, WorkspacesListRequest,
};

use super::words::{built_when, catalog_name, fmt_bytes, here_of, names_place, now_ms, plural, sub_threads_only, where_words, HERE};
use crate::screens::app_shell::{self, Route, Screen as _};
use crate::screens::composer::screen::ComposerScreen;
use crate::screens::settings::model::{req, Reread};
use crate::screens::settings::page::{CardData, Ctx, Item};
use crate::screens::settings::Acts;
use crate::ui::grid::PAGE_COLUMNS;
use crate::ui::{
    catalog, framed, glyph, num, scroll_view, spinner, ActivePalette, Button, Glyph, Grid, GridHead, GridName, GridRow, HeadCell, Refusal,
};
use build::image_build;

/// `IMAGE_WORDS` and the build's `CLOUD_SETUP_WORDS.build`.
pub mod words {
    pub const NOTHING: &str = "Not built yet";
    pub const NOT_HERE: &str = "Not here yet";
    pub const BUILDING: &str = "Building your image here";
    pub const READING: &str = "Reading the image";
    pub const COPYING: &str = "Copying your image here";
    pub const STALE: &str = "An older copy here";
    pub const READY: &str = "Ready";
    pub const CHOOSE_AND_BUILD: &str = "Choose what goes on your image and build it here.";
    pub const HOLDS: &str = "On your image";
    pub fn copy_comes(version: u64) -> String {
        format!("v{version} comes with your sign-ins. Nothing is asked again.")
    }
    pub fn copy_asks(version: u64) -> String {
        format!("v{version} holds no sign-ins, so each is asked again here.")
    }
    pub const COPY_HERE: &str = "Copy your image here";
    pub const COPY_ANYWAY: &str = "Copy anyway";
    pub const TRY_AGAIN: &str = "Try again";
    pub fn steps(done: u64, total: u64) -> String {
        format!("{done} of {total} steps done")
    }
    pub const START_TASK: &str = "New thread here";
    pub fn start_held(computer: &str) -> String {
        format!("No project lands its threads on {computer} yet.")
    }
    pub fn step_now(stage: Option<&str>, done: u64, total: u64) -> String {
        match stage {
            None => format!("{done} of {total} steps done"),
            Some(stage) => format!("{stage}, {done} of {total}"),
        }
    }
    pub fn built_chip(when: &str) -> String {
        format!("built {when}")
    }
    pub fn behind_chip(version: u64) -> String {
        format!("behind your image v{version}")
    }
    pub fn sign_in_wait(label: &str) -> String {
        format!("Waiting for your sign-in to {label}")
    }
    pub const CLOSE: &str = "Close";
    pub const BUILD_HEADLINE: &str = "Building your image";
    pub const BUILD_TOP: &str = "The computer starts, installs what you ticked and is saved as the image every task starts from";
    pub const SIGNING_IN: &str = "Signing in on the computer";
    pub const SLIDE_HEADLINE: &str = "Sign in on the computer";
    pub const SLIDE_TOP: &str = "Each one opens a page on this computer, and the image keeps the sign-in";
    pub const OPEN: &str = "Open sign-in";
    pub const RETRY: &str = "Retry";
    pub const CODE_SUBMIT: &str = "Submit";
    pub const CODE_ASK: &str = "Paste the code from the page";
    pub const CANCEL: &str = "Cancel the build";
    pub const CANNOT_STOP: &str = "The image is being saved. The snapshot and the save cannot be stopped.";
    pub const CANCEL_SURE: &str = "Stop the build";
    pub const CANCEL_WHY: &str = "The computer it was building on is thrown away and nothing is saved";
    pub const CANCEL_KEEP: &str = "Keep building";
    pub const DONE: &str = "Your image is ready";
    pub const FAILED: &str = "The build stopped";
    pub const STOPPED: &str = "You stopped the build";
    pub const CHANGE_KEY: &str = "Change the key";
    /// `copyStoppedLine()` with no reason of its own.
    pub const COPY_STOPPED: &str = "the build stopped";
    pub fn no_workspace(project: &str) -> String {
        format!("{project} has no workspace to start a thread in yet.")
    }

    /// `GOLDEN_STAGE_WORDS`, lowered as a row says them.
    fn stage_words(stage: &str) -> Option<&'static str> {
        Some(match stage {
            "creating" => "creating the machine",
            "deploying-daemon" => "installing the base tools",
            "applying-setup" => "applying your setup",
            "uploading-files" => "copying your files",
            "installing-harness" => "installing agents",
            "installing-tools" => "installing tools",
            "installing-mcp" => "installing MCP servers",
            "ready" => "checking the machine answers",
            "snapshotting" => "taking the snapshot",
            "promoting" => "saving the image",
            "smoke-forking" => "checking a fork boots",
            "sealed" => "finishing",
            _ => return None,
        })
    }

    /// `copyBuildingLine`.
    pub fn copy_building_line(stage: &str) -> Option<String> {
        stage_words(stage).map(|said| format!("copying your image: {said}"))
    }

    /// `initStoppedLine`: the reason's parts once each, or the network's line where every part is the network gone.
    fn stopped_line(error: &str) -> String {
        let mut parts: Vec<&str> = Vec::new();
        for part in error.split(';').map(str::trim).filter(|p| !p.is_empty()) {
            if !parts.contains(&part) {
                parts.push(part);
            }
        }
        if parts.is_empty() {
            return error.to_owned();
        }
        const OFFLINE: [&str; 6] = ["ENOTFOUND", "EAI_AGAIN", "ECONNREFUSED", "ENETDOWN", "ENETUNREACH", "EHOSTUNREACH"];
        let word_in = |part: &str, word: &str| part.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_')).any(|w| w == word);
        if parts.iter().all(|p| *p == "fetch failed" || OFFLINE.iter().any(|w| word_in(p, w))) {
            return "This computer lost its network".to_owned();
        }
        parts.join("; ")
    }

    /// `copyStoppedLine`.
    pub fn copy_stopped_line(reason: Option<&str>) -> String {
        match reason.map(stopped_line).filter(|said| !said.is_empty()) {
            Some(said) => format!("{COPY_STOPPED}: {said}"),
            None => COPY_STOPPED.to_owned(),
        }
    }

    /// `copyBuildOf`: what one golden.stage frame says of the copy at its place, the line and whether it stopped;
    /// nothing once one sealed, when what stands is the copies' to say.
    pub fn copy_build_of(stage: &str, detail: Option<&str>) -> Option<(String, bool)> {
        match stage {
            "sealed" => None,
            "failed" => Some((copy_stopped_line(detail), true)),
            stage => copy_building_line(stage).map(|line| (line, false)),
        }
    }
}

/// `ImageState`: one reading of the image on one computer.
#[derive(Clone, Debug, PartialEq)]
pub enum State {
    None,
    Building,
    Copying(String),
    Stopped(String),
    Stale,
    Ready,
}

/// `initJobBuilding`.
pub fn job_building(phase: &str) -> bool {
    matches!(phase, "building" | "signing-in" | "sealing" | "finishing")
}

/// `initJobOver`.
pub(super) fn job_over(phase: &str) -> bool {
    matches!(phase, "done" | "failed" | "cancelled")
}

/// `initAgentStep`.
fn agent_step(job: &InitJob) -> bool {
    let phase = job.phase.to_string();
    job.road.to_string() == "agent" && (phase == "agent" || (job_over(&phase) && job.screens.is_empty()))
}

/// `initSignInWaitedOn`: the sign-in whose page waits for the person.
fn waited_on(job: &InitJob) -> Option<&str> {
    job.rows.iter().find(|r| r.kind.to_string() == "sign-in" && r.state == OPEN).map(|r| r.label.as_str())
}

/// `INIT_ROW_STATES` the build reads.
const OPEN: &str = "waiting for you";
const RUNNING: &str = "running";
const SLOT: &str = "waiting for a machine slot";
const WAITING: &str = "waiting";
const RETRYING: &str = "machine still running, retrying";
const OVER: [&str; 10] = ["done", "failed", "stopped", "forked", "imported", "key set", "MCP added", "skipped", "not made", "gone"];
const UNRUN: [&str; 3] = ["skipped", "not made", "stopped"];

fn row_failed(row: &InitJobRowsItem) -> bool {
    row.state == "failed" || row.login.as_ref().is_some_and(|l| l.to_string() == "not-signed-in")
}

fn row_over(row: &InitJobRowsItem) -> bool {
    row.login.is_some() || OVER.contains(&row.state.as_str())
}

/// `copyOn`: the copy one place holds.
pub fn copy_on<'a>(copies: &'a [SealedImageViewCopiesItem], place: &PlaceView) -> Option<&'a SealedImageViewCopiesItem> {
    copies.iter().find(|c| names_place(place, &c.place))
}

/// `copyAsksSignIns`.
fn copy_asks_sign_ins(image: &SealedImageViewImage) -> bool {
    image.vault.is_none() && image.no_secrets != Some(true)
}

/// `imageState`, off the row's own build line where no golden.stage frame has been read.
pub fn image_state(place: &PlaceView, ctx: &Ctx) -> Option<State> {
    if place.builds_images == Some(false) {
        return None;
    }
    let row_build = place.build.as_ref().map(|line| {
        if place.build_stopped == Some(true) {
            State::Stopped(line.clone())
        } else {
            State::Copying(line.clone())
        }
    });
    if place.builds_images.is_none() {
        return row_build;
    }
    let job = job_of(ctx);
    let job_here = job.is_some_and(|j| j.place.as_ref().is_some_and(|p| p.id == place.id));
    if job_here && job.is_some_and(|j| job_building(&j.phase.to_string())) {
        return Some(State::Building);
    }
    if let Some(build) = row_build {
        return Some(build);
    }
    let view = ctx.data.image.as_ref();
    if let Some((image, copy)) = view.and_then(|v| v.image.as_ref().and_then(|i| copy_on(&v.copies, place).map(|c| (i, c)))) {
        let stale = (image.vault.is_some() || image.no_secrets == Some(true)) && copy.hash.as_deref() != Some(image.hash.as_str());
        return Some(if stale { State::Stale } else { State::Ready });
    }
    if job_here && job.is_some_and(|j| j.phase.to_string() == "failed") {
        let said = job.and_then(|j| j.error.clone()).unwrap_or_else(|| words::COPY_STOPPED.to_owned());
        return Some(State::Stopped(said));
    }
    Some(State::None)
}

/// The image's own build as the host last told it.
pub(super) fn job_of<'a>(ctx: &Ctx<'a>) -> Option<&'a InitJob> {
    ctx.data.setup.as_ref().and_then(|s| s.job.as_ref())
}

/// The title a state stands under: with no image anywhere, nothing here is a copy yet.
fn title_of(state: &State, image_exists: bool) -> &'static str {
    match state {
        State::None | State::Stopped(_) => {
            if image_exists {
                words::NOT_HERE
            } else {
                words::NOTHING
            }
        }
        State::Building => words::BUILDING,
        State::Copying(_) => words::COPYING,
        State::Stale => words::STALE,
        State::Ready => words::READY,
    }
}

/// `recordChips`: the version, what the image came to on disk and how many sign-ins it holds.
fn record_chips(image: &SealedImageViewImage) -> Vec<String> {
    let held = image.logins.iter().filter(|l| matches!(l.state, Login::SignedIn | Login::Copied | Login::NotVerified)).count();
    let mut chips = vec![format!("v{}", image.version)];
    chips.extend(image.used_bytes.map(fmt_bytes));
    chips.push(plural(held as i64, "sign-in"));
    chips
}

/// `recipeNames`: the agents ticked, then the tools ticked and the rows added by hand.
pub fn recipe_names(image: &SealedImageViewImage) -> Option<String> {
    let recipe = image.recipe.as_ref()?;
    let named = |kind: &str| -> Vec<String> {
        recipe.rows.iter().filter(|r| r.on && r.kind.to_string() == kind).map(|r| catalog_name(&r.id)).collect()
    };
    let mut names = named("agent");
    names.extend(named("tool"));
    names.extend(recipe.custom.iter().map(|c| c.name.clone()));
    Some(names.join(", "))
}

/// What the row's line holds and the one press it invites.
struct Line {
    description: Option<String>,
    chips: Option<Vec<String>>,
    busy: bool,
    note: Option<String>,
    press: Option<Press>,
}

#[derive(Clone)]
struct Press {
    word: String,
    held_why: Option<String>,
    run: PressRun,
}

#[derive(Clone, PartialEq)]
enum PressRun {
    Copy,
    /// The project whose threads land here, by id and name.
    NewThread(String, String),
    Nothing,
}

/// What the row keeps across frames: a copy press on its way, its refusal, and the build put away by Close.
#[derive(Default)]
struct Kept {
    pressing: bool,
    refused: Option<String>,
    put_away: Option<String>,
    asking: bool,
    changing: bool,
    step_busy: bool,
    step_refusal: Option<String>,
    /// The stages the person opened or folded, by row.
    opened: HashMap<String, bool>,
    /// Each stage block's scroll and how many lines it last held.
    lines: HashMap<String, (ScrollHandle, usize)>,
    /// The key was saved: its field empties on the next frame.
    clear_key: bool,
}

/// `projectOn`: the first project whose workspaces land on this place.
fn project_on<'a>(ctx: &'a Ctx, place: &PlaceView) -> Option<&'a ProjectView> {
    ctx.data.projects.iter().find(|p| {
        if p.computer.is_empty() || p.computer == HERE {
            here_of(place)
        } else {
            names_place(place, &p.computer)
        }
    })
}

/// The workspace a project's new thread opens on: its own folder's where it has one, else any standing one of its own.
fn home_of<'a>(workspaces: &'a [WorkspaceOut], project: &str) -> Option<&'a str> {
    let own = || workspaces.iter().filter(|w| w.project.id == project && w.gone.is_none() && w.parent_thread_id.is_none());
    let in_folder = |w: &&WorkspaceOut| w.kind.as_ref().is_some_and(|k| matches!(k.to_string().as_str(), "local" | "place"));
    own().find(in_folder).or_else(|| own().next()).map(|w| w.id.as_str())
}

/// `openProjectHome`: Settings gives way to the composer on a new thread in the project's own workspace.
fn open_project_home(acts: &Acts, project: String, name: String, cx: &mut App) {
    let told = acts.clone();
    acts.call(req::<WorkspacesListRequest>(json!({})), cx, move |answer: Result<Workspaces, String>, cx| match answer {
        Ok(Workspaces { workspaces }) => match home_of(&workspaces, &project) {
            Some(id) => app_shell::navigate(Route::Screen(format!("{}:{id}", ComposerScreen::NAME).into()), cx),
            None => told.failed(words::no_workspace(&name), cx),
        },
        Err(said) => told.failed(format!("Threads not read: {said}"), cx),
    });
}

/// The IMAGE list of a computer's or a cloud's page, where the image stands there and the computer can hold one.
pub fn image_row(ctx: &Ctx, place: &PlaceView, window: &mut Window, cx: &mut App) -> Option<CardData> {
    let view = ctx.data.image.as_ref()?;
    let state = image_state(place, ctx)?;
    let name = super::words::place_name(place);
    let image = view.image.as_ref();
    let kept = window.use_keyed_state(SharedString::from(format!("computer-{}-image", place.id)), cx, |_, _| Kept::default());
    let (pressing, refused, put_away) = {
        let k = kept.read(cx);
        (k.pressing, k.refused.clone(), k.put_away.clone())
    };
    let job = job_of(ctx);
    let force = image.is_some_and(copy_asks_sign_ins);
    let copy_press =
        Press { word: if force { words::COPY_ANYWAY } else { words::COPY_HERE }.to_owned(), held_why: None, run: PressRun::Copy };
    let title = title_of(&state, image.is_some());
    let now = now_ms();
    let built =
        |copy: Option<&SealedImageViewCopiesItem>| copy.map(|c| words::built_chip(&built_when(&c.built_at, now))).unwrap_or_default();
    let copy = copy_on(&view.copies, place);
    let line = match &state {
        State::None => match image {
            None => Line { description: Some(words::CHOOSE_AND_BUILD.to_owned()), chips: None, busy: false, note: None, press: None },
            Some(image) => Line {
                description: Some(if force { words::copy_asks(image.version.get()) } else { words::copy_comes(image.version.get()) }),
                chips: None,
                busy: false,
                note: None,
                press: Some(copy_press.clone()),
            },
        },
        State::Building => {
            let job = job.expect("a build here has its job");
            match waited_on(job) {
                None => Line {
                    description: Some(words::steps(job.progress.done, job.progress.total)),
                    chips: None,
                    busy: true,
                    note: None,
                    press: None,
                },
                Some(label) => Line { description: Some(words::sign_in_wait(label)), chips: None, busy: false, note: None, press: None },
            }
        }
        State::Copying(said) => Line { description: Some(said.clone()), chips: None, busy: true, note: None, press: None },
        State::Stopped(said) => match image {
            None => Line { description: Some(said.clone()), chips: None, busy: false, note: None, press: None },
            Some(image) => Line {
                description: Some(said.clone()),
                chips: None,
                busy: false,
                note: force.then(|| words::copy_asks(image.version.get())),
                press: Some(Press { word: if force { words::COPY_ANYWAY } else { words::TRY_AGAIN }.to_owned(), ..copy_press.clone() }),
            },
        },
        State::Stale => Line {
            description: None,
            chips: Some(vec![words::behind_chip(image.map(|i| i.version.get()).unwrap_or(0)), built(copy)]),
            busy: false,
            note: None,
            press: Some(copy_press.clone()),
        },
        State::Ready => {
            let mut chips = image.map(record_chips).unwrap_or_default();
            chips.push(built(copy));
            let starts = !(place.kind.to_string() == "provider" && sub_threads_only(&place.id));
            let press = starts.then(|| {
                if let Some(project) = project_on(ctx, place) {
                    Press {
                        word: words::START_TASK.to_owned(),
                        held_why: None,
                        run: PressRun::NewThread(project.id.clone(), project.name.clone()),
                    }
                } else {
                    Press { word: words::START_TASK.to_owned(), held_why: Some(words::start_held(&name)), run: PressRun::Nothing }
                }
            });
            Line { description: None, chips: Some(chips), busy: false, note: None, press }
        }
    };
    let build_here = job.filter(|j| {
        let phase = j.phase.to_string();
        j.place.as_ref().is_some_and(|p| p.id == place.id)
            && !agent_step(j)
            && (job_building(&phase) || phase == "failed" || phase == "cancelled")
            && put_away.as_deref() != Some(j.id.as_str())
    });
    // The build's box answers what to do now while it stands, so the row's press goes until Close.
    let press = line.press.clone().filter(|_| build_here.is_none());
    let control: Option<AnyElement> = if line.busy {
        Some(spinner(cx).into_any_element())
    } else {
        press.clone().map(|press| {
            let (acts, id, kept) = (ctx.acts.clone(), place.id.clone(), kept.clone());
            Button::outline("image-press", press.word.clone())
                .xs()
                .held(press.held_why.is_some() || pressing)
                .on_click(move |_, _, cx| match &press.run {
                    PressRun::Copy => {
                        kept.update(cx, |k, _| {
                            k.pressing = true;
                            k.refused = None;
                        });
                        let mut params = json!({ "place": id });
                        if force {
                            params["force"] = json!(true);
                        }
                        let (acts2, kept) = (acts.clone(), kept.clone());
                        acts.call(req::<ImageBuildRequest>(params), cx, move |answer, cx| {
                            kept.update(cx, |k, _| {
                                k.pressing = false;
                                if let Err(said) = answer {
                                    k.refused = Some(said);
                                }
                            });
                            acts2.reread(Reread::Image, cx);
                            acts2.reread(Reread::Places, cx);
                        });
                        acts.notify(cx);
                    }
                    PressRun::NewThread(project, name) => open_project_home(&acts, project.clone(), name.clone(), cx),
                    PressRun::Nothing => {}
                })
                .into_any_element()
        })
    };
    let standing = match &state {
        State::Ready => built(copy),
        State::Building if job.and_then(waited_on).is_none() => {
            let job = job.expect("a build here has its job");
            let stage = job.rows.iter().find(|r| r.state == RUNNING).map(|r| r.label.as_str());
            words::step_now(stage, job.progress.done, job.progress.total)
        }
        _ => line.description.clone().or_else(|| line.chips.as_ref().map(|c| c.join(", "))).unwrap_or_else(|| title.to_owned()),
    };
    let sentence = |part: String| if part.ends_with(['.', '!', '?']) { part } else { format!("{part}.") };
    let mut parts = vec![standing];
    if let Some(names) = image.and_then(recipe_names) {
        parts.push(format!("{}: {names}", words::HOLDS));
    }
    let note = parts.into_iter().filter(|s| !s.is_empty()).map(sentence).collect::<Vec<_>>().join(" ");
    let glyph_el = framed(glyph(Glyph::File, cx), cx);
    let row = GridRow::new("image-state", PAGE_COLUMNS)
        .tight()
        .cell(GridName::new(glyph_el, where_words::YOUR_IMAGE).note(note))
        .cell(num("", cx))
        .cell(h_flex().w_full().justify_end().gap_3().children(control));
    let grid = Grid::new("image").head(GridHead::new(Some(PAGE_COLUMNS), vec![HeadCell::new(where_words::HEAD_IMAGE)])).row(row);
    let waiting = press.and_then(|p| p.held_why).or(line.note.clone());
    let mut body = v_flex().w_full().gap_3().child(grid);
    if refused.is_some() || waiting.is_some() {
        let mut slot = Refusal::new();
        if let Some(said) = refused {
            slot = slot.said(said);
        }
        if let Some(waiting) = waiting {
            slot = slot.quiet(waiting);
        }
        body = body.child(slot);
    }
    if let Some(job) = build_here.filter(|j| !job_building(&j.phase.to_string()) || waited_on(j).is_some()) {
        let build = image_build(ctx, place, job, &kept, window, cx);
        let scroll = window.use_keyed_state(SharedString::from(format!("image-build-box-{}", place.id)), cx, |_, _| ScrollHandle::new());
        let scroll = scroll.read(cx).clone();
        body = body.child(scroll_view("image-build-box", &scroll, div().max_h(px(320.)).child(build), window, cx));
    }
    Some(CardData::new("image").body(body))
}

/// Whether any row has a version, which the Version head stands for.
fn versioned(tags: &[&str]) -> bool {
    tags.iter().any(|t| !t.is_empty())
}

/// The AGENTS list of a cloud's page: the agents its image holds, off the image's own record, each with its mark and
/// its version, the Version head standing only where some row has one.
pub fn agents_lists(ctx: &Ctx, _place: &PlaceView, _window: &mut Window, cx: &mut App) -> Option<CardData> {
    let image = ctx.data.image.as_ref()?.image.as_ref()?;
    let pins: Vec<_> = image.pins.iter().filter(|pin| super::words::is_agent(&pin.id)).collect();
    if pins.is_empty() {
        return None;
    }
    let versioned = versioned(&pins.iter().map(|pin| pin.tag.as_str()).collect::<Vec<_>>());
    let rows: Vec<GridRow> = pins
        .iter()
        .map(|pin| {
            let signed = image.logins.iter().find(|l| l.name == pin.id).is_some_and(|l| matches!(l.state, Login::SignedIn | Login::Copied));
            let label = catalog_name(&pin.id);
            let lead = framed(catalog::mark(&pin.id, &label, px(16.), cx.palette().foreground, cx), cx);
            let mut name = GridName::new(div().debug_selector(|| "agent-lead".into()).child(lead), label);
            if signed {
                name = name.note("Signed in");
            }
            GridRow::new(SharedString::from(format!("kind-row-{}", pin.id)), PAGE_COLUMNS)
                .tight()
                .cell(name)
                .cell(num(pin.tag.clone(), cx))
                .cell(div())
        })
        .collect();
    let mut heads = vec![HeadCell::new(where_words::HEAD_AGENTS)];
    if versioned {
        heads.push(HeadCell::new(where_words::HEAD_VERSION).num());
    }
    let grid = Grid::new("agents").head(GridHead::new(Some(PAGE_COLUMNS), heads)).rows(rows);
    Some(CardData::new("agents").body(div().w_full().child(grid)).search(Vec::<Item>::new()))
}

#[cfg(test)]
mod tests;
