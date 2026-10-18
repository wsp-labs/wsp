// SPDX-License-Identifier: AGPL-3.0-only
//! Your image on one computer, after `image.ts`, `imageState.ts` and `ImageCard.tsx` as a computer's or a cloud's page
//! draws it, the IMAGE list: one line of where the copy stands and what the image holds, and the one press that state
//! invites, Copy through image.build. A running build is its stage on the line; the build's own view (`ImageBuild.tsx`)
//! stands under it in a box only while it needs the person or once it ended short. The agents a cloud's image holds
//! stand above it, off the image's own record.

use std::collections::HashMap;
use std::rc::Rc;

use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{h_flex, v_flex, ActiveTheme as _, Icon, Sizable as _};

use gpui_kit::{div, prelude::*, px, AnyElement, App, Entity, ScrollHandle, SharedString, Window};
use serde_json::json;
use wsp_host_client::ops::sidebar::Workspaces;
use wsp_host_client::protocol::{
    ImageBuildRequest, InitCancelRequest, InitJob, InitJobRowsItem, InitKeysRequest, InitRetryRequest, InitSignInCodeRequest, PlaceView,
    ProjectView, SealedImageViewCopiesItem, SealedImageViewImage, SealedImageViewImageLoginsItemState as Login, WorkspaceOut,
    WorkspacesListRequest,
};

use super::remove::call_whole;
use super::words::{
    built_when, catalog_name, fmt_bytes, here_of, names_place, now_ms, plural, provider_words, sub_threads_only, where_words, HERE,
};
use crate::screens::app_shell::{self, Route, Screen as _};
use crate::screens::composer::screen::ComposerScreen;
use crate::screens::settings::model::{req, Reread};
use crate::screens::settings::page::{CardData, Ctx, Item};
use crate::screens::settings::pages::agents::keyed::{self, typed};
use crate::screens::settings::Acts;
use crate::ui::catalog;
use crate::ui::computer_glyph::stroked_svg;
use crate::ui::grid::PAGE_COLUMNS;
use crate::ui::{
    framed, glyph, num, scroll_view, spinner, text, ActivePalette, Button, Glyph, Grid, GridHead, GridName, GridRow, HeadCell, Refusal,
};

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

/// The machine's sign-ins and stages under the IMAGE row, after `ImageBuild.tsx`: the build's title and sentence,
/// its stages or, while the sign-in stage runs, the sign-ins alone, Cancel asking once, and Close once it ended. While a
/// press is out every link but Close is held.
fn image_build(ctx: &Ctx, place: &PlaceView, job: &InitJob, kept: &Entity<Kept>, window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.palette().clone();
    let phase = job.phase.to_string();
    let building = job_building(&phase);
    let signs: Vec<&InitJobRowsItem> = job.rows.iter().filter(|r| r.kind.to_string() == "sign-in").collect();
    let stage_state = sign_in_stage_state(&signs);
    let slide = building && !signs.is_empty() && (stage_state == OPEN || stage_state == RUNNING);
    let headline = if slide {
        words::SLIDE_HEADLINE
    } else if phase == "done" {
        words::DONE
    } else if phase == "cancelled" {
        words::STOPPED
    } else if job_over(&phase) {
        words::FAILED
    } else {
        words::BUILD_HEADLINE
    };
    let top = if building { Some((if slide { words::SLIDE_TOP } else { words::BUILD_TOP }).to_owned()) } else { job.error.clone() };
    let (asking, changing, busy, step_refusal) = {
        let k = kept.read(cx);
        (k.asking, k.changing, k.step_busy, k.step_refusal.clone())
    };
    let key_words = (job.key_refused == Some(true)).then(|| provider_words(&place.id)).flatten();
    let marked = signs.iter().any(|s| s.tool.as_deref().is_some_and(has_row_mark));
    let mut rows: Vec<AnyElement> = Vec::new();
    if slide {
        for s in &signs {
            let retry = row_failed(s).then(|| s.tool.clone().unwrap_or_else(|| s.id.clone()));
            rows.push(slide_row(ctx, s, marked, retry, kept, window, cx));
        }
    } else {
        let mut folded = false;
        for row in &job.rows {
            let kind = row.kind.to_string();
            if kind == "agent" {
                continue;
            }
            if kind == "sign-in" {
                if folded {
                    continue;
                }
                folded = true;
                let stage = Stage {
                    id: SIGN_IN_STAGE,
                    label: words::SIGNING_IN,
                    state: &stage_state,
                    login: None,
                    lines: &[],
                    since: None,
                    detail: None,
                };
                rows.push(stage_row(ctx, &stage, Some((&signs, marked)), building, kept, window, cx));
                continue;
            }
            let login = row.login.as_ref().map(|l| l.to_string());
            let stage = Stage {
                id: &row.id,
                label: &row.label,
                state: &row.state,
                login: login.as_deref(),
                lines: &row.lines,
                since: row.since,
                detail: row.detail.as_deref(),
            };
            rows.push(stage_row(ctx, &stage, None, building, kept, window, cx));
        }
    }
    let total = job.rows.iter().filter(|r| r.kind.to_string() == "stage").count();
    let done = job
        .rows
        .iter()
        .filter(|r| r.kind.to_string() == "stage" && row_over(r) && !row_failed(r) && !UNRUN.contains(&r.state.as_str()))
        .count();
    let fraction = if total > 0 { done as f32 / total as f32 } else { 0. };
    let card = v_flex()
        .w_full()
        .rounded(crate::ui::size::CARD_RADIUS)
        .border_1()
        .border_color(p.border.opacity(0.6))
        .bg(p.card.opacity(0.4))
        .when(!slide, |el| {
            el.child(
                div()
                    .h(px(2.))
                    .w_full()
                    .bg(p.muted_foreground.opacity(0.2))
                    .child(div().h_full().w(gpui_kit::relative(fraction)).bg(p.foreground.opacity(0.7))),
            )
        })
        .children(rows.into_iter().enumerate().map(|(ix, el)| div().when(ix > 0, |d| d.border_t_1().border_color(p.border)).child(el)));
    let mut links: Vec<AnyElement> = Vec::new();
    if !building {
        let kept = kept.clone();
        let id = job.id.clone();
        links.push(
            Button::ghost("build-close", words::CLOSE)
                .xs()
                .on_click(move |_, _, cx| kept.update(cx, |k, _| k.put_away = Some(id.clone())))
                .into_any_element(),
        );
    } else if asking {
        let (acts, kept2) = (ctx.acts.clone(), kept.clone());
        links.push(
            div()
                .debug_selector(|| "build-sure".into())
                .child(Button::destructive("build-sure", words::CANCEL_SURE).xs().held(busy).on_click(move |_, _, cx| {
                    kept2.update(cx, |k, _| k.asking = false);
                    attempt(&acts, &kept2, req::<InitCancelRequest>(json!({})), cx);
                }))
                .into_any_element(),
        );
        let kept3 = kept.clone();
        links.push(
            div()
                .debug_selector(|| "build-keep".into())
                .child(
                    Button::ghost("build-keep", words::CANCEL_KEEP)
                        .xs()
                        .held(busy)
                        .on_click(move |_, _, cx| kept3.update(cx, |k, _| k.asking = false)),
                )
                .into_any_element(),
        );
    } else {
        let kept2 = kept.clone();
        links.push(
            Button::ghost_danger("build-cancel", words::CANCEL)
                .xs()
                .held(!job.stoppable || busy)
                .on_click(move |_, _, cx| kept2.update(cx, |k, _| k.asking = true))
                .into_any_element(),
        );
    }
    let primary = (!building && !changing && key_words.is_some()).then(|| {
        let kept = kept.clone();
        Button::outline("build-primary", words::CHANGE_KEY)
            .xs()
            .held(busy)
            .on_click(move |_, _, cx| kept.update(cx, |k, _| k.changing = true))
    });
    let waiting = (building && !job.stoppable).then_some(words::CANNOT_STOP);
    let mut slot = Refusal::new();
    if let Some(said) = step_refusal {
        slot = slot.said(said);
    }
    if let Some(waiting) = waiting {
        slot = slot.quiet(waiting);
    }
    let mut step = v_flex()
        .gap_4()
        .child(
            v_flex()
                .gap(px(6.))
                .child(
                    div()
                        .text_size(text::SETTING_TITLE.0)
                        .line_height(text::SETTING_TITLE.1)
                        .font_weight(gpui_kit::FontWeight::MEDIUM)
                        .child(headline),
                )
                .children(
                    top.filter(|t| !t.is_empty())
                        .map(|t| div().text_size(text::XS.0).line_height(text::XS.1).text_color(p.muted_foreground).child(t)),
                ),
        )
        .child(card);
    if changing {
        if let Some(words) = key_words {
            step = step.child(div().pt_5().child(provider_key(ctx, place, words, kept, window, cx)));
        }
    }
    if asking && building {
        step = step.child(div().text_size(text::XS.0).line_height(text::XS.1).text_color(p.muted_foreground).child(words::CANCEL_WHY));
    }
    step.child(h_flex().gap_4().child(h_flex().flex_1().min_w_0().gap_4().children(links)).children(primary)).child(slot).into_any_element()
}

/// One press of the build sent to the host, its refusal said in the step's slot.
fn attempt<O>(acts: &crate::screens::settings::Acts, kept: &Entity<Kept>, op: O, cx: &mut App)
where
    O: wsp_host_client::Op + Send + 'static,
    O::Reply: Send + 'static,
{
    kept.update(cx, |k, _| {
        k.step_busy = true;
        k.step_refusal = None;
    });
    let (acts2, kept) = (acts.clone(), kept.clone());
    acts.call(op, cx, move |answer, cx| {
        kept.update(cx, |k, _| {
            k.step_busy = false;
            if let Err(said) = answer {
                k.step_refusal = Some(said);
            }
        });
        acts2.notify(cx);
    });
}

/// `signInStageState`: the folded sign-in stage reads as its sign-ins do.
fn sign_in_stage_state(signs: &[&InitJobRowsItem]) -> String {
    if signs.iter().any(|r| r.state == OPEN) {
        return OPEN.into();
    }
    if signs.iter().any(|r| r.state == RUNNING) {
        return RUNNING.into();
    }
    if !signs.iter().all(|r| row_over(r)) {
        return if signs.iter().all(|r| r.state == WAITING) { WAITING.into() } else { RUNNING.into() };
    }
    if let Some(out) = signs.iter().find(|r| row_failed(r)) {
        return out.state.clone();
    }
    if signs.iter().all(|r| r.state == "skipped") {
        return "skipped".into();
    }
    "done".into()
}

/// `StageGlyph`: a ring while it waits, a dot while it runs, a check once it ended well, a cross once it failed.
fn stage_glyph(state: &str, login: Option<&str>, cx: &App) -> AnyElement {
    let p = cx.palette();
    let ring = || div().size_2().rounded_full().border_1().border_color(p.muted_foreground.opacity(0.6)).into_any_element();
    let dot = || div().size_2().rounded_full().bg(p.foreground).into_any_element();
    let failed = state == "failed" || login == Some("not-signed-in");
    if UNRUN.contains(&state) {
        return ring();
    }
    if state == RETRYING {
        return dot();
    }
    if failed {
        return stroked_svg("0 0 16 16", r#"<path d="M4.5 4.5l7 7M11.5 4.5l-7 7"/>"#, Some(1.75), p.error_foreground, px(14.));
    }
    if login.is_some() || OVER.contains(&state) {
        return stroked_svg("0 0 16 16", r#"<path d="M3.5 8.5 6.5 11.5 12.5 5"/>"#, Some(1.75), p.foreground, px(14.));
    }
    if state == RUNNING || state == SLOT || state == OPEN {
        return dot();
    }
    ring()
}

/// `RowState`: the spinner while it runs, a muted mono word otherwise, its whole word on the hover.
fn row_state(id: &str, state: &str, cx: &App) -> AnyElement {
    if state == RUNNING {
        return spinner(cx).into_any_element();
    }
    let p = cx.palette();
    div()
        .id(SharedString::from(format!("build-state-{id}")))
        .flex_none()
        .max_w(px(224.))
        .truncate()
        .font_family(cx.theme().mono_font_family.clone())
        .text_size(text::XS.0)
        .line_height(text::XS.1)
        .text_color(if state == "failed" { p.error_foreground } else { p.muted_foreground })
        .tooltip(crate::ui::tip(state.to_owned()))
        .child(state.to_owned())
        .into_any_element()
}

/// The folded sign-in stage's row id.
const SIGN_IN_STAGE: &str = "stage/sign-ins";

/// `GOLDEN_STAGE_TIMED`: the stages the provider says nothing during, which count their own seconds.
fn timed(id: &str) -> bool {
    matches!(id, "stage/snapshotting" | "stage/promoting")
}

/// `STAGE_BLOCK_LINES`, and `STAGE_BLOCK_LINES_SHORT` under a 700 px high window.
fn block_lines(window_height: gpui_kit::Pixels) -> usize {
    if window_height < px(700.) {
        6
    } else {
        8
    }
}

/// `initElapsedLine`: whole seconds under a minute, then minutes and seconds.
pub fn elapsed_line(ms: f64) -> String {
    if ms < 60_000. {
        format!("{}s", (ms / 1_000.).floor().max(0.))
    } else {
        crate::screens::settings::words::fmt_duration(ms)
    }
}

/// `Elapsed`: a timed stage's seconds, its own entity ticking once a second, so only it draws again for the clock.
pub struct Elapsed {
    since: f64,
    next: Option<gpui_kit::Task<()>>,
    /// How often it drew, which a test reads.
    pub drawn: usize,
}

impl Elapsed {
    pub fn new(since: f64) -> Self {
        Elapsed { since, next: None, drawn: 0 }
    }
}

impl Render for Elapsed {
    fn render(&mut self, _: &mut Window, cx: &mut gpui_kit::Context<Self>) -> impl IntoElement {
        self.drawn += 1;
        if self.next.is_none() {
            self.next = Some(cx.spawn(async move |this, cx| {
                cx.background_executor().timer(std::time::Duration::from_secs(1)).await;
                _ = this.update(cx, |this, cx| {
                    this.next = None;
                    cx.notify();
                });
            }));
        }
        let p = cx.palette().clone();
        div()
            .debug_selector(|| "elapsed".into())
            .flex_none()
            .font_family(cx.theme().mono_font_family.clone())
            .text_size(text::XS.0)
            .line_height(text::XS.1)
            .text_color(p.muted_foreground)
            .child(elapsed_line(now_ms() as f64 - self.since))
    }
}

/// A stage as its row draws it.
struct Stage<'a> {
    id: &'a str,
    label: &'a str,
    state: &'a str,
    login: Option<&'a str>,
    lines: &'a [String],
    since: Option<f64>,
    detail: Option<&'a str>,
}

/// `BuildRow`: a stage's glyph, its name, its seconds where it counts them, its state and, where it folds, the
/// chevron; under it the machine's lines, or the sign-ins for the sign-in stage. A running stage and the sign-ins
/// waited on stand open; a failed one, or one with a sign-in to retry, opens on its own and folds on a click; a done
/// one opens on a click.
fn stage_row(
    ctx: &Ctx,
    stage: &Stage,
    signs: Option<(&[&InitJobRowsItem], bool)>,
    building: bool,
    kept: &Entity<Kept>,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let p = cx.palette().clone();
    let running = stage.state == RUNNING || stage.state == SLOT;
    let failed = stage.state == "failed";
    let live = running || stage.state == OPEN;
    let has = match signs {
        Some((signs, _)) => !signs.is_empty(),
        None => !stage.lines.is_empty(),
    };
    let attention = building && signs.is_some_and(|(signs, _)| signs.iter().any(|s| row_failed(s)));
    let can_open = has && !live;
    let opened = kept.read(cx).opened.get(stage.id).copied();
    let open = has && (live || opened.unwrap_or(failed || attention));
    let elapsed = stage.since.filter(|_| running && timed(stage.id)).map(|since| {
        window.use_keyed_state(SharedString::from(format!("build-elapsed-{}", stage.id)), cx, move |_, _| Elapsed::new(since))
    });
    let chevron = can_open.then(|| {
        let turn = crate::ui::motion::ease_f32(
            SharedString::from(format!("build-{}-chevron", stage.id)),
            "turn",
            if open { 0. } else { -90. },
            crate::ui::motion::TURN,
            window,
            cx,
        );
        Icon::new(Glyph::ChevronDown)
            .with_size(px(14.))
            .flex_none()
            .text_color(p.muted_foreground)
            .rotate(gpui_kit::radians(turn.to_radians()))
    });
    let (id, kept2) = (stage.id.to_owned(), kept.clone());
    let selector = format!("build-row-{}", stage.id);
    let row = h_flex()
        .id(SharedString::from(format!("build-row-{}", stage.id)))
        .debug_selector(move || selector)
        .h(px(48.))
        .gap_3()
        .pl_4()
        .pr(px(10.))
        .when(can_open, |el| {
            let hover = p.accent.opacity(0.3);
            el.cursor_pointer().hover(move |s| s.bg(hover)).on_click(move |_, _, cx| {
                kept2.update(cx, |k, cx| {
                    k.opened.insert(id.clone(), !open);
                    cx.notify();
                })
            })
        })
        .when_some(stage.detail.map(str::to_owned), |el, detail| el.tooltip(crate::ui::tip(detail)))
        .child(div().flex().flex_none().items_center().justify_center().size(px(18.)).child(stage_glyph(stage.state, stage.login, cx)))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_size(px(15.))
                .text_color(if stage.state == WAITING { p.muted_foreground } else { p.foreground })
                .child(stage.label.to_owned()),
        )
        .child(
            h_flex()
                .ml_auto()
                .min_w(px(96.))
                .flex_none()
                .justify_end()
                .gap_2()
                .children(elapsed)
                .child(row_state(stage.id, stage.state, cx))
                .children(chevron),
        );
    let under: Option<AnyElement> = match signs {
        _ if !open => None,
        Some((signs, marked)) => Some(
            v_flex()
                .border_t_1()
                .border_color(p.border)
                .children(signs.iter().enumerate().map(|(ix, s)| {
                    let retry = (building && row_failed(s)).then(|| s.tool.clone().unwrap_or_else(|| s.id.clone()));
                    div()
                        .when(ix > 0, |d| d.border_t_1().border_color(p.border))
                        .child(sign_in_row(ctx, s, marked, retry, kept, window, cx))
                }))
                .into_any_element(),
        ),
        None => Some(stage_lines(stage.id, stage.lines, running, failed, kept, window, cx)),
    };
    v_flex().child(row).children(under).into_any_element()
}

/// `StageLines`: the open stage's block, as tall as its lines up to eight (six under a 700 px high window), scrolling
/// inside itself and pinned to the newest line while the stage runs; the padding stands outside the scroller, so a
/// pinned block shows whole lines.
fn stage_lines(
    id: &str,
    lines: &[String],
    running: bool,
    failed: bool,
    kept: &Entity<Kept>,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let p = cx.palette().clone();
    let shown = lines.len().clamp(1, block_lines(window.viewport_size().height));
    let scroll = kept.update(cx, |k, _| {
        let (handle, seen) = k.lines.entry(id.to_owned()).or_insert_with(|| (ScrollHandle::new(), 0));
        if running && *seen != lines.len() {
            handle.scroll_to_bottom();
        }
        *seen = lines.len();
        handle.clone()
    });
    let dark = cx.theme().is_dark();
    let last = lines.len().saturating_sub(1);
    let lines = div()
        .pl(px(46.))
        .pr(px(10.))
        .font_family(cx.theme().mono_font_family.clone())
        .text_size(text::XS.0)
        .line_height(px(20.))
        .text_color(p.muted_foreground)
        .children(lines.iter().enumerate().map(|(ix, line)| {
            let line = terminal_line(line, failed && ix == last, dark, &p);
            if ix == last {
                div().debug_selector(|| "stage-line-newest".into()).child(line).into_any_element()
            } else {
                line
            }
        }));
    div()
        .debug_selector(|| "stage-lines".into())
        .border_t_1()
        .border_color(p.border)
        .bg(p.background.opacity(0.4))
        .pt(px(7.))
        .pb_2()
        .h(px(shown as f32 * 20. + 16.))
        .child(scroll_view(SharedString::from(format!("build-lines-{id}")), &scroll, lines, window, cx).h_full())
        .into_any_element()
}

/// `TerminalLine`: a tool's prefix at 60%, the machine's colours kept, a failed stage's last line in the danger ink.
fn terminal_line(line: &str, danger: bool, dark: bool, p: &crate::ui::Palette) -> AnyElement {
    use gpui_kit::{HighlightStyle, StyledText};
    let base = if danger { p.error_foreground } else { p.muted_foreground };
    let (prefix, rest) = super::ansi::tool_prefix(line).map_or(("", line), |(prefix, rest)| (prefix, rest));
    let mut text = prefix.to_owned();
    let mut highlights = Vec::new();
    if !prefix.is_empty() {
        highlights.push((0..prefix.len(), HighlightStyle { color: Some(base.opacity(0.6)), ..Default::default() }));
    }
    for span in super::ansi::spans(rest) {
        let start = text.len();
        text.push_str(&span.text);
        let ink = span.slot.map_or(base, |slot| super::ansi::slot_ink(slot, dark));
        let ink = if span.dim { ink.opacity(0.6) } else { ink };
        let weight = span.bold.then_some(gpui_kit::FontWeight::MEDIUM);
        if ink != base || weight.is_some() {
            highlights.push((start..text.len(), HighlightStyle { color: Some(ink), font_weight: weight, ..Default::default() }));
        }
    }
    let selector = if danger { "stage-line-danger" } else { "stage-line" };
    div()
        .debug_selector(move || selector.into())
        .text_color(base)
        .child(StyledText::new(text).with_highlights(highlights))
        .into_any_element()
}

/// `hasRowMark`: the tool has a mark to lead with, its agent's or the company's whose page it opens.
fn has_row_mark(tool: &str) -> bool {
    catalog::image(tool, false, gpui_kit::black()).is_some() || sign_in_mark(tool).is_some()
}

/// `SIGN_IN_MARKS` for the tools no agent's mark covers.
fn sign_in_mark(tool: &str) -> Option<&'static str> {
    const GITHUB: &str = include_str!("../../../../../../../../packages/catalog/src/marks/github.ts");
    const GOOGLE: &str = include_str!("../../../../../../../../apps/web/src/assets/signins/google.svg");
    match tool {
        "gh" => GITHUB.find("svg: `").map(|at| &GITHUB[at + 6..]).and_then(|rest| rest.find('`').map(|end| &rest[..end])),
        "gemini" | "gcloud" => Some(GOOGLE.trim()),
        _ => None,
    }
}

/// `MarkColumn`: the tool's mark in its 18 px, or the same 18 px of nothing where it has none.
fn mark_column(tool: Option<&str>, cx: &App) -> AnyElement {
    let ink = cx.palette().foreground;
    let mark = tool.and_then(|tool| match catalog::image(tool, cx.theme().is_dark(), ink) {
        Some(image) => Some(gpui_kit::img(image).size(px(18.)).flex_none().into_any_element()),
        None => sign_in_mark(tool).map(|svg| crate::ui::computer_glyph::mark_svg(svg, ink, px(18.))),
    });
    div()
        .debug_selector(|| "mark-column".into())
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(18.))
        .children(mark)
        .into_any_element()
}

/// The field a page that hands a code back takes, with Submit, Enter doing what Submit does.
fn code_line(ctx: &Ctx, row: &InitJobRowsItem, kept: &Entity<Kept>, window: &mut Window, cx: &mut App) -> gpui_kit::Div {
    let p = cx.palette().clone();
    let tool = row.tool.clone().unwrap_or_else(|| row.id.clone());
    let (acts, kept) = (ctx.acts.clone(), kept.clone());
    let send = Rc::new(move |field: &Entity<InputState>, window: &mut Window, cx: &mut App| {
        let code = field.read(cx).value().trim().to_owned();
        if !code.is_empty() {
            field.update(cx, |f, cx| f.set_value("", window, cx));
            attempt(&acts, &kept, req::<InitSignInCodeRequest>(json!({ "tool": tool, "code": code })), cx);
        }
    });
    let on_enter = send.clone();
    let input = typed(
        format!("build-code-{}", row.id),
        words::CODE_ASK,
        "",
        false,
        move |field, window, cx| on_enter(field, window, cx),
        window,
        cx,
    );
    let field = input.clone();
    let held = keyed::words(&input, cx).trim().is_empty();
    let mut submit = Button::outline(SharedString::from(format!("code-{}", row.id)), words::CODE_SUBMIT).xs().held(held);
    if !held {
        submit = submit.on_click(move |_, window, cx| send(&field, window, cx));
    }
    h_flex()
        .min_w_0()
        .flex_1()
        .gap_3()
        .child(
            div()
                .debug_selector(|| "build-code-field".into())
                .flex_1()
                .min_w(px(180.))
                .h(px(32.))
                .rounded(crate::ui::size::BUTTON_RADIUS)
                .border_1()
                .border_color(p.input)
                .bg(p.background)
                .px_2()
                .flex()
                .items_center()
                .font_family(cx.theme().mono_font_family.clone())
                .text_size(text::XS.0)
                .child(Input::new(&input).appearance(false).w_full()),
        )
        .child(div().debug_selector(move || if held { "build-code-submit-held" } else { "build-code-submit" }.into()).child(submit))
}

/// The keycap that opens a sign-in's page.
fn open_button(row: &InitJobRowsItem, page: String) -> Button {
    Button::outline(SharedString::from(format!("open-{}", row.id)), format!("{} \u{2197}", words::OPEN))
        .xs()
        .on_click(move |_, _, cx| crate::ui::open_link(&page, cx))
}

fn retry_button(ctx: &Ctx, row: &InitJobRowsItem, tool: String, kept: &Entity<Kept>) -> Button {
    let (acts, kept) = (ctx.acts.clone(), kept.clone());
    Button::outline(SharedString::from(format!("retry-{}", row.id)), words::RETRY)
        .xs()
        .on_click(move |_, _, cx| attempt(&acts, &kept, req::<InitRetryRequest>(json!({ "tool": tool })), cx))
}

/// `SignInRow`: one sign-in under its stage, a step in: the mark's column, the name, the code its page asks for, and
/// in the slot the keycap that opens the page or retries, then the state word; the code field on a line of its own.
fn sign_in_row(
    ctx: &Ctx,
    row: &InitJobRowsItem,
    marked: bool,
    retry: Option<String>,
    kept: &Entity<Kept>,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let p = cx.palette().clone();
    let waiting = row.state == OPEN;
    let takes_code = row.finish.as_ref().is_some_and(|f| f.to_string() == "code");
    let head = h_flex()
        .debug_selector(|| "sign-in-row".into())
        .h(px(48.))
        .gap_3()
        .pl_10()
        .pr(px(10.))
        .when(marked, |el| el.child(mark_column(row.tool.as_deref(), cx)))
        .child({
            let selector = format!("sign-in-name-{}", row.id);
            div()
                .debug_selector(move || selector)
                .flex_1()
                .min_w_0()
                .truncate()
                .text_size(px(15.))
                .text_color(p.foreground)
                .child(row.label.clone())
        })
        .children(row.code.clone().filter(|_| waiting).map(|code| {
            div().flex_none().font_family(cx.theme().mono_font_family.clone()).text_size(text::XS.0).text_color(p.foreground).child(code)
        }))
        .child(
            h_flex()
                .ml_auto()
                .min_w(px(96.))
                .flex_none()
                .justify_end()
                .gap_2()
                .children(row.page.clone().filter(|_| waiting).map(|page| open_button(row, page)))
                .children(retry.map(|tool| retry_button(ctx, row, tool, kept)))
                .child(row_state(&row.id, &row.state, cx)),
        );
    let code = (waiting && takes_code)
        .then(|| code_line(ctx, row, kept, window, cx).h(px(48.)).pl_10().pr(px(10.)).border_t_1().border_color(p.border));
    v_flex().child(head).children(code).into_any_element()
}

/// `SignInSlideRow`: a sign-in's mark, name and state, and under it, where the person has something to do, the code,
/// the keycap that opens its page or retries, and the field a page that hands a code back takes.
fn slide_row(
    ctx: &Ctx,
    row: &InitJobRowsItem,
    marked: bool,
    retry: Option<String>,
    kept: &Entity<Kept>,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let p = cx.palette().clone();
    let waiting = row.state == OPEN;
    let takes_code = row.finish.as_ref().is_some_and(|f| f.to_string() == "code");
    let acts_line = (waiting && (row.code.is_some() || row.page.is_some() || takes_code)) || retry.is_some();
    let head = h_flex()
        .h(px(48.))
        .gap_3()
        .pl_4()
        .pr(px(10.))
        .when(marked, |el| el.child(mark_column(row.tool.as_deref(), cx)))
        .child(div().flex_1().min_w_0().truncate().text_size(px(15.)).text_color(p.foreground).child(row.label.clone()))
        .child(h_flex().ml_auto().min_w(px(96.)).flex_none().justify_end().gap_2().child(row_state(&row.id, &row.state, cx)));
    let mut line =
        h_flex()
            .min_h(px(40.))
            .flex_wrap()
            .gap_x_3()
            .gap_y_2()
            .pb_2()
            .pr(px(10.))
            .map(|el| if marked { el.pl(px(46.)) } else { el.pl_4() });
    if waiting {
        if let Some(code) = &row.code {
            line = line.child(
                div()
                    .flex_none()
                    .font_family(cx.theme().mono_font_family.clone())
                    .text_size(px(20.))
                    .line_height(px(28.))
                    .text_color(p.foreground)
                    .child(code.clone()),
            );
        }
        if let Some(page) = row.page.clone() {
            line = line.child(open_button(row, page));
        }
    }
    if let Some(tool) = retry {
        line = line.child(retry_button(ctx, row, tool, kept));
    }
    if waiting && takes_code {
        line = line.child(code_line(ctx, row, kept, window, cx));
    } else if waiting {
        let said = if row.code.is_some() || takes_code { "waiting for the code" } else { "the page is open on this computer" };
        line = line.child(
            div()
                .min_w_0()
                .truncate()
                .font_family(cx.theme().mono_font_family.clone())
                .text_size(text::XS.0)
                .text_color(p.muted_foreground)
                .child(said),
        );
    }
    v_flex().child(head).when(acts_line, |el| el.child(line)).into_any_element()
}

/// `ProviderKey` where a build found the saved key refused: the field, Replace or Enter, and the provider's refusal.
/// The field is emptied only once the key is saved, so a refused key stays to be corrected.
fn provider_key(
    ctx: &Ctx,
    place: &PlaceView,
    words: &'static super::words::ProviderWords,
    kept: &Entity<Kept>,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let p = cx.palette().clone();
    let save: SaveKey = {
        let (acts, kept, id) = (ctx.acts.clone(), kept.clone(), place.id.clone());
        Rc::new(move |field, cx| {
            let value = field.read(cx).value().trim().to_owned();
            if value.is_empty() || kept.read(cx).step_busy {
                return;
            }
            kept.update(cx, |k, _| {
                k.step_busy = true;
                k.step_refusal = None;
            });
            let (acts2, kept3, id2) = (acts.clone(), kept.clone(), id.clone());
            call_whole(&acts, req::<InitKeysRequest>(json!({ "provider": id, "key": value })), cx, move |answer, cx| {
                kept3.update(cx, |k, _| {
                    k.step_busy = false;
                    match answer {
                        Ok(setup) if setup.setup.keys.get(&id2) == Some(&true) => {
                            k.changing = false;
                            k.clear_key = true;
                            k.put_away = setup.setup.job.as_ref().map(|j| j.id.clone()).or(k.put_away.take());
                        }
                        Ok(_) => {
                            let words = provider_words(&id2);
                            let name = words.map(|w| w.name).unwrap_or_default();
                            let fix = words
                                .and_then(|w| w.key_console)
                                .map(|c| format!("Check it at {c} and paste it again."))
                                .unwrap_or_else(|| "Check it and paste it again.".into());
                            k.step_refusal = Some(format!("{name} refused that key. {fix}"));
                        }
                        Err(failure) => k.step_refusal = Some(failure.message),
                    }
                });
                acts2.reread(Reread::Places, cx);
            });
        })
    };
    let on_enter = save.clone();
    let key = typed(format!("computer-{}-key", place.id), KEY_ASK, "", true, move |field, _, cx| on_enter(field, cx), window, cx);
    if kept.update(cx, |k, _| std::mem::take(&mut k.clear_key)) {
        keyed::clear(&key, window, cx);
    }
    let typed_key = key.read(cx).value().trim().to_owned();
    let busy = kept.read(cx).step_busy;
    let field = key.clone();
    let save = Button::outline("cloud-save", if busy { "Checking" } else { "Replace" })
        .held(busy || typed_key.is_empty())
        .on_click(move |_, _, cx| save(&field, cx));
    let console = words.key_console.map(|c| {
        let url = format!("https://{c}");
        h_flex()
            .id("get-key")
            .ml_auto()
            .gap_1()
            .text_size(px(12.))
            .text_color(p.muted_foreground)
            .cursor_pointer()
            .hover(|s| s.text_color(p.foreground))
            .on_click(move |_, _, cx| cx.open_url(&url))
            .child("Get a key")
            .child(Icon::new(Glyph::ExternalLink).with_size(px(12.)))
    });
    v_flex()
        .gap_3()
        .child(
            h_flex()
                .gap(px(10.))
                .child(Icon::new(Glyph::Cloud).with_size(px(16.)).text_color(p.muted_foreground))
                .child(div().text_size(px(14.)).child(words.name))
                .child(
                    h_flex()
                        .gap_1()
                        .font_family(cx.theme().mono_font_family.clone())
                        .text_size(px(11.))
                        .text_color(p.foreground.opacity(0.7))
                        .child(Icon::new(Glyph::Check).with_size(px(12.)))
                        .child("key saved"),
                )
                .children(console),
        )
        .child(
            h_flex()
                .gap_2()
                .child(
                    h_flex()
                        .debug_selector(|| "cloud-key".into())
                        .flex_1()
                        .min_w_0()
                        .h(px(40.))
                        .gap_2()
                        .px_3()
                        .rounded(crate::ui::size::BUTTON_RADIUS)
                        .border_1()
                        .border_color(p.input)
                        .bg(p.input_fill)
                        .font_family(cx.theme().mono_font_family.clone())
                        .text_size(px(13.))
                        .child(Icon::new(Glyph::KeyRound).with_size(px(16.)).text_color(p.muted_foreground))
                        .child(div().flex_1().min_w_0().child(Input::new(&key).appearance(false).w_full())),
                )
                .child(save),
        )
        .into_any_element()
}

/// Whether any row has a version, which the Version head stands for.
fn versioned(tags: &[&str]) -> bool {
    tags.iter().any(|t| !t.is_empty())
}

/// The key field's save, shared by Replace and Enter.
type SaveKey = Rc<dyn Fn(&Entity<InputState>, &mut App)>;

/// What the key field says while empty.
const KEY_ASK: &str = "Paste a new key to replace it";

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
mod tests {
    use super::*;

    use crate::screens::settings::model::{At, Data, GroupId};
    use gpui_kit::{Modifiers, TestAppContext, VisualTestContext};
    use std::sync::Arc;

    type Draw = Box<dyn Fn(&Ctx, &mut Window, &mut App) -> AnyElement>;

    /// Draws `draw` in a window as a page of Settings would, with no host behind it.
    struct Harness(Draw, Data, Arc<tokio::runtime::Runtime>);

    impl Render for Harness {
        fn render(&mut self, window: &mut Window, cx: &mut gpui_kit::Context<Self>) -> impl IntoElement {
            let acts = crate::screens::settings::Acts::new(gpui_kit::WeakEntity::new_invalid(), None, self.2.clone());
            let activity = cx.new(|_| crate::screens::settings::pages::activity::Pick::default());
            let agents = cx.new(|_| crate::screens::settings::pages::agents::Store::new(acts.clone()));
            let at = At::Group(GroupId::Computers);
            let ctx = Ctx::new(
                &self.1,
                &at,
                None,
                acts,
                activity,
                agents,
                cx.new(|_| crate::screens::settings::pages::computers::RowMenu::default()),
            );
            div().w(px(900.)).child((self.0)(&ctx, window, cx))
        }
    }

    fn draw(height: f32, draw: Draw, cx: &mut TestAppContext) -> &mut VisualTestContext {
        cx.update(|cx| {
            gpui_kit::init(cx);
            crate::ui::init(crate::ui::Side::Light, cx);
        });
        let runtime = Arc::new(tokio::runtime::Builder::new_current_thread().build().unwrap());
        let (_, cx) = cx.add_window_view(move |_, _| Harness(draw, Data::default(), runtime));
        cx.simulate_resize(gpui_kit::size(px(1000.), px(height)));
        cx.run_until_parked();
        cx
    }

    fn solari() -> PlaceView {
        serde_json::from_value(json!({ "id": "solari", "kind": "provider", "name": "solari", "default": false })).unwrap()
    }

    fn job(phase: &str, rows: serde_json::Value) -> InitJob {
        serde_json::from_value(json!({ "id": "j", "road": "manual", "phase": phase, "keys": {}, "step": 0, "stoppable": true,
            "screens": [], "rows": rows, "progress": { "done": 0, "total": 0 }, "log": [], "place": { "id": "solari", "name": "solari" } }))
        .unwrap()
    }

    /// The build box of `job`, its kept state the test's.
    fn build_box(job: InitJob, kept: Entity<Kept>) -> Draw {
        Box::new(move |ctx, window, cx| image_build(ctx, &solari(), &job, &kept, window, cx))
    }

    fn lines(n: usize) -> Vec<String> {
        (0..n).map(|i| format!("pnpm: \u{1b}[32mstep {i}\u{1b}[0m done")).collect()
    }

    fn stages() -> InitJob {
        job(
            "failed",
            json!([
                { "id": "stage/creating", "kind": "stage", "label": "Creating the machine", "state": "done", "lines": lines(10) },
                { "id": "stage/deploying-daemon", "kind": "stage", "label": "Installing the base tools", "state": "failed", "lines": lines(10),
                  "detail": "the base tools did not install" },
            ]),
        )
    }

    #[gpui_kit::test]
    fn a_failed_stage_stands_open_on_its_lines_and_a_done_one_opens_on_a_click(cx: &mut TestAppContext) {
        let kept = cx.new(|_| Kept::default());
        let cx = draw(800., build_box(stages(), kept), cx);
        assert!(cx.debug_bounds("stage-line-danger").is_some(), "the failed stage is open, its last line in the danger ink");
        assert_eq!(cx.debug_bounds("stage-lines").unwrap().size.height, px(8. * 20. + 16.), "eight lines at most");
        let failed = cx.debug_bounds("build-row-stage/deploying-daemon").unwrap().center();
        cx.simulate_click(failed, Modifiers::none());
        cx.run_until_parked();
        assert!(cx.debug_bounds("stage-lines").is_none(), "a click folds the failed stage, and the done one stands folded");
        let done = cx.debug_bounds("build-row-stage/creating").unwrap().center();
        cx.simulate_click(done, Modifiers::none());
        cx.run_until_parked();
        assert!(cx.debug_bounds("stage-lines").is_some() && cx.debug_bounds("stage-line-danger").is_none(), "the done stage opens");
    }

    #[test]
    fn a_projects_new_thread_opens_on_its_own_folders_workspace() {
        let workspace = |id: &str, project: &str, kind: &str, more: serde_json::Value| -> WorkspaceOut {
            let mut w = json!({ "id": id, "name": id, "machineId": "m", "phase": "running", "golden": "", "createdAt": "2026-10-17T00:00:00Z",
                "project": { "id": project, "name": project, "path": "/p", "computer": "here" }, "kind": kind });
            w.as_object_mut().unwrap().extend(more.as_object().unwrap().clone());
            serde_json::from_value(w).unwrap()
        };
        let listed = vec![
            workspace("w-other", "p2", "local", json!({})),
            workspace("w-fork", "p1", "cloud", json!({})),
            workspace("w-sub", "p1", "local", json!({ "parentThreadId": "t0" })),
            workspace("w-gone", "p1", "local", json!({ "gone": "2026-10-17T00:00:00Z" })),
            workspace("w-home", "p1", "local", json!({})),
        ];
        assert_eq!(home_of(&listed, "p1"), Some("w-home"), "the project's own folder, past a cloud one, a sub-thread's and a gone one");
        assert_eq!(home_of(&listed[..2], "p1"), Some("w-fork"), "else a standing one of its own");
        assert_eq!(home_of(&listed, "p3"), None);
    }

    #[gpui_kit::test]
    fn a_short_window_shows_six_lines(cx: &mut TestAppContext) {
        let kept = cx.new(|_| Kept::default());
        let cx = draw(650., build_box(stages(), kept), cx);
        assert_eq!(cx.debug_bounds("stage-lines").unwrap().size.height, px(6. * 20. + 16.));
    }

    #[gpui_kit::test]
    fn a_running_stage_stands_on_its_newest_line(cx: &mut TestAppContext) {
        let kept = cx.new(|_| Kept::default());
        let running = job(
            "failed",
            json!([{ "id": "stage/creating", "kind": "stage", "label": "Creating", "state": "running", "lines": lines(20) }]),
        );
        let cx = draw(800., build_box(running, kept), cx);
        cx.run_until_parked();
        let block = cx.debug_bounds("stage-lines").unwrap();
        let scroller = block.bottom() - px(8.);
        let newest = cx.debug_bounds("stage-line-newest").expect("the newest line");
        assert!(newest.bottom() <= scroller + px(0.5) && newest.top() >= block.top(), "{newest:?} in {block:?}");
        let bar = cx.debug_bounds("scrollbar").expect("the overflowing lines draw their bar");
        assert_eq!((block.right() - bar.right(), bar.size.width), (px(1.), px(6.)), "the shared bar at the block's right edge");
        assert_eq!((bar.top() - (block.top() + px(8.)), scroller - bar.bottom()), (px(4.), px(4.)), "my-1 in the lines' scroller");
    }

    #[gpui_kit::test]
    fn the_sign_ins_names_start_on_one_line_after_the_mark_column(cx: &mut TestAppContext) {
        let kept = cx.new(|_| Kept::default());
        let signs = job(
            "failed",
            json!([
                { "id": "signin/claude", "kind": "sign-in", "tool": "claude", "label": "Claude Code", "state": "failed" },
                { "id": "signin/foo", "kind": "sign-in", "tool": "foo", "label": "Foo", "state": "done" },
            ]),
        );
        let cx = draw(800., build_box(signs, kept), cx);
        let a = cx.debug_bounds("sign-in-name-signin/claude").expect("the failed sign-in stands open under its stage");
        let b = cx.debug_bounds("sign-in-name-signin/foo").unwrap();
        let row = cx.debug_bounds("build-row-stage/sign-ins").unwrap();
        assert_eq!(a.origin.x, b.origin.x, "a tool with no mark keeps the column");
        assert_eq!(a.origin.x, row.origin.x + px(40. + 18. + 12.), "pl-10, the 18 px mark and the gap");
    }

    #[gpui_kit::test]
    fn every_link_but_close_is_held_while_a_press_is_out(cx: &mut TestAppContext) {
        let kept = cx.new(|_| Kept { asking: true, step_busy: true, ..Kept::default() });
        let building = job("building", json!([{ "id": "stage/creating", "kind": "stage", "label": "Creating", "state": "running" }]));
        let cx = draw(800., build_box(building, kept.clone()), cx);
        let keep = cx.debug_bounds("build-keep").expect("Keep building").center();
        cx.simulate_click(keep, Modifiers::none());
        cx.run_until_parked();
        assert!(kept.read_with(cx, |k, _| k.asking), "Keep building took no press while the cancel was out");
    }

    #[gpui_kit::test]
    fn enter_saves_the_key_and_a_refused_key_stays_in_its_field(cx: &mut TestAppContext) {
        let kept = cx.new(|_| Kept::default());
        let held = kept.clone();
        let words = super::super::words::provider_words("solari").unwrap();
        let cx = draw(800., Box::new(move |ctx, window, cx| provider_key(ctx, &solari(), words, &held, window, cx)), cx);
        let field = cx.debug_bounds("cloud-key").unwrap().center();
        cx.simulate_click(field, Modifiers::none());
        cx.simulate_input("sk-test-key");
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();
        assert_eq!(kept.read_with(cx, |k, _| k.step_refusal.clone()).as_deref(), Some("No host answered."), "Enter saved");
        kept.update(cx, |k, _| k.step_refusal = None);
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();
        assert!(kept.read_with(cx, |k, _| k.step_refusal.is_some()), "the refused key was still there to send again");
    }

    #[gpui_kit::test]
    fn elapsed_ticks_on_its_own_once_a_second(cx: &mut TestAppContext) {
        assert_eq!(elapsed_line(5_400.), "5s");
        assert_eq!(elapsed_line(-20.), "0s");
        assert_eq!(elapsed_line(65_000.), crate::screens::settings::words::fmt_duration(65_000.));
        cx.update(|cx| {
            gpui_kit::init(cx);
            crate::ui::init(crate::ui::Side::Light, cx);
        });
        let (view, cx) = cx.add_window_view(|_, _| Elapsed::new(now_ms() as f64 - 5_000.));
        cx.run_until_parked();
        let first = view.read_with(cx, |e, _| e.drawn);
        cx.executor().advance_clock(std::time::Duration::from_millis(1_000));
        cx.run_until_parked();
        assert_eq!(view.read_with(cx, |e, _| e.drawn), first + 1, "one draw a second");
    }

    /// box1's IMAGE row with an image to copy there and a build that failed on `on`, drawn with no host behind it.
    fn image_row_of<'a>(on: &str, cx: &'a mut TestAppContext) -> &'a mut VisualTestContext {
        let image = json!({ "image": { "name": "base", "version": 1, "hash": "h", "recipeHash": "r", "logins": [],
            "noSecrets": true, "sealedAt": "2026-10-17T00:00:00Z", "sealedFrom": "here" }, "copies": [], "projects": [] });
        let job = json!({ "id": "j", "road": "manual", "phase": "failed", "keys": {}, "step": 0, "stoppable": false, "screens": [],
            "rows": [{ "id": "stage/creating", "kind": "stage", "label": "Creating the machine", "state": "failed" }],
            "progress": { "done": 0, "total": 1 }, "log": [], "place": { "id": on, "name": on } });
        let setup = json!({ "keys": {}, "home": "/h", "agents": [], "pricing": null, "job": job });
        let data = Data {
            image: Some(serde_json::from_value(image).unwrap()),
            setup: Some(serde_json::from_value(setup).unwrap()),
            ..Data::default()
        };
        let place: PlaceView =
            serde_json::from_value(json!({ "id": "box1", "kind": "computer", "name": "box1", "default": false, "buildsImages": true }))
                .unwrap();
        cx.update(|cx| {
            gpui_kit::init(cx);
            crate::ui::init(crate::ui::Side::Light, cx);
        });
        let runtime = Arc::new(tokio::runtime::Builder::new_current_thread().build().unwrap());
        let row: Draw = Box::new(move |ctx, window, cx| match image_row(ctx, &place, window, cx) {
            Some(card) => card.render().into_any_element(),
            None => div().into_any_element(),
        });
        let (_, cx) = cx.add_window_view(move |_, _| Harness(row, data, runtime));
        cx.simulate_resize(gpui_kit::size(px(1000.), px(900.)));
        cx.run_until_parked();
        cx
    }

    /// Presses where the row's own act stands, at its right end: a copy there, with no host, is refused in a slot under
    /// the row, which stands the slot of a build box under it further down.
    fn press_the_rows_act(cx: &mut VisualTestContext) -> bool {
        let grid = cx.debug_bounds("grid").expect("the IMAGE list");
        let before = cx.debug_bounds("refusal");
        cx.simulate_click(gpui_kit::point(grid.right() - px(40.), grid.bottom() - px(24.)), Modifiers::none());
        cx.run_until_parked();
        // The refusal lands on the row's kept state, which tells no view; Settings draws every frame.
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
        cx.debug_bounds("refusal") != before
    }

    #[gpui_kit::test]
    fn a_rows_press_stands_down_beside_its_open_build_box_until_close(cx: &mut TestAppContext) {
        let cx = image_row_of("box1", cx);
        assert!(!press_the_rows_act(cx), "the failed build's box answers what to do now, so the row has no press");
        // Close, the box's one link, is the last button drawn.
        let close = cx.debug_bounds("button").expect("the box's Close");
        cx.simulate_click(close.center(), Modifiers::none());
        cx.run_until_parked();
        assert!(press_the_rows_act(cx), "with the box put away, Try again stands on the row again");
    }

    #[test]
    fn the_version_head_stands_where_a_row_has_one() {
        assert!(versioned(&["0.3.1"]));
        assert!(!versioned(&["", ""]));
    }

    #[test]
    fn a_build_is_on_its_stretch() {
        assert!(job_building("signing-in"));
        assert!(!job_building("answering"));
        assert!(job_over("cancelled"));
        assert_eq!(words::step_now(Some("Creating the machine"), 2, 7), "Creating the machine, 2 of 7");
        assert_eq!(words::step_now(None, 2, 7), "2 of 7 steps done");
    }
}
