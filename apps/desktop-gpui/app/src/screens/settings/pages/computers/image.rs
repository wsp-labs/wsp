// SPDX-License-Identifier: AGPL-3.0-only
//! Your image on one computer, after `image.ts`, `imageState.ts` and `ImageCard.tsx` as a computer's or a cloud's page
//! draws it, the IMAGE list: one line of where the copy stands and what the image holds, and the one press that state
//! invites, Copy through image.build. A running build is its stage on the line; the build's own view (`ImageBuild.tsx`)
//! stands under it in a box only while it needs the person or once it ended short. The agents a cloud's image holds
//! stand above it, off the image's own record.

use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{h_flex, v_flex, ActiveTheme as _, Icon, Sizable as _};
use gpui_kit::{div, prelude::*, px, AnyElement, App, Entity, SharedString, Window};
use serde_json::json;
use wsp_host_client::protocol::{
    ImageBuildRequest, InitCancelRequest, InitJob, InitJobRowsItem, InitKeysRequest, InitRetryRequest, InitSignInCodeRequest, PlaceView,
    SealedImageViewCopiesItem, SealedImageViewImage, SealedImageViewImageLoginsItemState as Login,
};

use super::glyph::stroked_svg;
use super::remove::call_whole;
use super::words::{
    built_when, catalog_name, fmt_bytes, here_of, names_place, now_ms, plural, provider_words, sub_threads_only, where_words,
};
use crate::screens::settings::model::{req, Reread};
use crate::screens::settings::page::{CardData, Ctx, Item};
use crate::ui::grid::PAGE_COLUMNS;
use crate::ui::{framed, glyph, num, spinner, text, ActivePalette, Button, Glyph, Grid, GridHead, GridName, GridRow, HeadCell, Refusal};

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
    pub const NEW_THREAD_ELSEWHERE: &str = "New thread here opens from the sidebar in this build of the app.";
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
fn job_over(phase: &str) -> bool {
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
fn job_of<'a>(ctx: &Ctx<'a>) -> Option<&'a InitJob> {
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
    chips.extend(image.used_bytes.map(|b| fmt_bytes(b as f64)));
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

#[derive(Clone, Copy, PartialEq)]
enum PressRun {
    Copy,
    NewThread,
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
}

/// `projectOn`: the first project whose workspaces land on this place.
fn lands_here(ctx: &Ctx, place: &PlaceView) -> bool {
    ctx.data.projects.iter().any(|p| {
        if p.computer.is_empty() || p.computer == "here" {
            here_of(place)
        } else {
            names_place(place, &p.computer)
        }
    })
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
                if lands_here(ctx, place) {
                    Press { word: words::START_TASK.to_owned(), held_why: None, run: PressRun::NewThread }
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
    let press = line.press.clone();
    let control: Option<AnyElement> = if line.busy {
        Some(spinner(cx).into_any_element())
    } else {
        press.clone().map(|press| {
            let (acts, id, kept) = (ctx.acts.clone(), place.id.clone(), kept.clone());
            Button::outline("image-press", press.word.clone())
                .xs()
                .held(press.held_why.is_some() || pressing)
                .on_click(move |_, _, cx| match press.run {
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
                    PressRun::NewThread => acts.note(words::NEW_THREAD_ELSEWHERE, cx),
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
        body = body.child(div().id("image-build-box").max_h(px(320.)).overflow_y_scroll().child(build));
    }
    Some(CardData::new("image").body(body))
}

/// The machine's sign-ins and stages under the IMAGE row, after `ImageBuild.tsx`: the build's title and sentence,
/// its stages or, while the sign-in stage runs, the sign-ins alone, Cancel asking once, and Close once it ended.
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
    let mut rows: Vec<AnyElement> = Vec::new();
    let retry = |tool: String| {
        let (acts, kept) = (ctx.acts.clone(), kept.clone());
        move |_: &gpui_kit::ClickEvent, _: &mut Window, cx: &mut App| {
            attempt(&acts, &kept, req::<InitRetryRequest>(json!({ "tool": tool })), cx)
        }
    };
    if slide {
        for s in &signs {
            rows.push(sign_in_row(ctx, s, kept, row_failed(s).then(|| retry(s.tool.clone().unwrap_or_else(|| s.id.clone()))), window, cx));
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
                let open = stage_state == OPEN || stage_state == RUNNING || signs.iter().any(|s| row_failed(s));
                rows.push(stage_row("stage/sign-ins", words::SIGNING_IN, &stage_state, None, &[], cx));
                if open {
                    for s in &signs {
                        let retry = (building && row_failed(s)).then(|| retry(s.tool.clone().unwrap_or_else(|| s.id.clone())));
                        rows.push(div().pl(px(24.)).child(sign_in_row(ctx, s, kept, retry, window, cx)).into_any_element());
                    }
                }
                continue;
            }
            let lines = if row.state == RUNNING || row.state == SLOT || row.state == "failed" { row.lines.as_slice() } else { &[] };
            rows.push(stage_row(&row.id, &row.label, &row.state, row.login.as_ref().map(|l| l.to_string()), lines, cx));
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
            Button::destructive("build-sure", words::CANCEL_SURE)
                .xs()
                .on_click(move |_, _, cx| {
                    kept2.update(cx, |k, _| k.asking = false);
                    attempt(&acts, &kept2, req::<InitCancelRequest>(json!({})), cx);
                })
                .into_any_element(),
        );
        let kept3 = kept.clone();
        links.push(
            Button::ghost("build-keep", words::CANCEL_KEEP)
                .xs()
                .on_click(move |_, _, cx| kept3.update(cx, |k, _| k.asking = false))
                .into_any_element(),
        );
    } else {
        let kept2 = kept.clone();
        links.push(
            Button::ghost("build-cancel", words::CANCEL)
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

/// `RowState`: the spinner while it runs, a muted mono word otherwise.
fn row_state(state: &str, cx: &App) -> AnyElement {
    if state == RUNNING {
        return spinner(cx).into_any_element();
    }
    let p = cx.palette();
    div()
        .flex_none()
        .max_w(px(224.))
        .truncate()
        .font_family(cx.theme().mono_font_family.clone())
        .text_size(text::XS.0)
        .line_height(text::XS.1)
        .text_color(if state == "failed" { p.error_foreground } else { p.muted_foreground })
        .child(state.to_owned())
        .into_any_element()
}

/// `BuildRow`: a stage's glyph, its name and its state, with the machine's latest lines under it while it runs.
fn stage_row(id: &str, label: &str, state: &str, login: Option<String>, lines: &[String], cx: &App) -> AnyElement {
    let p = cx.palette().clone();
    let row = h_flex()
        .id(SharedString::from(format!("build-row-{id}")))
        .h(px(48.))
        .gap_3()
        .pl_4()
        .pr(px(10.))
        .child(div().flex().flex_none().items_center().justify_center().size(px(18.)).child(stage_glyph(state, login.as_deref(), cx)))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_size(px(15.))
                .text_color(if state == WAITING { p.muted_foreground } else { p.foreground })
                .child(label.to_owned()),
        )
        .child(h_flex().ml_auto().min_w(px(96.)).flex_none().justify_end().gap_2().child(row_state(state, cx)));
    let shown = lines.len().clamp(1, 8);
    v_flex()
        .child(row)
        .when(!lines.is_empty(), |el| {
            el.child(
                div()
                    .border_t_1()
                    .border_color(p.border)
                    .bg(p.background.opacity(0.4))
                    .pt(px(7.))
                    .pb_2()
                    .h(px(shown as f32 * 20. + 16.))
                    .child(
                        div()
                            .id(SharedString::from(format!("build-lines-{id}")))
                            .h_full()
                            .overflow_y_scroll()
                            .pl(px(46.))
                            .pr(px(10.))
                            .font_family(cx.theme().mono_font_family.clone())
                            .text_size(text::XS.0)
                            .line_height(px(20.))
                            .text_color(p.muted_foreground)
                            .children(lines.iter().map(|l| div().child(l.clone()))),
                    ),
            )
        })
        .into_any_element()
}

/// `SignInSlideRow`: a sign-in's name and state, and under it, where the person has something to do, the code, the
/// keycap that opens its page or retries, and the field a page that hands a code back takes.
fn sign_in_row(
    ctx: &Ctx,
    row: &InitJobRowsItem,
    kept: &Entity<Kept>,
    retry: Option<impl Fn(&gpui_kit::ClickEvent, &mut Window, &mut App) + 'static>,
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
        .child(div().flex_1().min_w_0().truncate().text_size(px(15.)).text_color(p.foreground).child(row.label.clone()))
        .child(h_flex().ml_auto().min_w(px(96.)).flex_none().justify_end().gap_2().child(row_state(&row.state, cx)));
    let mut line = h_flex().min_h(px(40.)).flex_wrap().gap_x_3().gap_y_2().pb_2().pl_4().pr(px(10.));
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
            line = line.child(
                Button::outline(SharedString::from(format!("open-{}", row.id)), format!("{} \u{2197}", words::OPEN))
                    .xs()
                    .on_click(move |_, _, cx| cx.open_url(&page)),
            );
        }
    }
    if let Some(retry) = retry {
        line = line.child(Button::outline(SharedString::from(format!("retry-{}", row.id)), words::RETRY).xs().on_click(retry));
    }
    if waiting && takes_code {
        let tool = row.tool.clone().unwrap_or_else(|| row.id.clone());
        let input = window.use_keyed_state(SharedString::from(format!("build-code-{}", row.id)), cx, InputState::new);
        let (acts, kept, field) = (ctx.acts.clone(), kept.clone(), input.clone());
        line = line
            .child(
                div()
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
            .child(Button::outline(SharedString::from(format!("code-{}", row.id)), words::CODE_SUBMIT).xs().on_click(move |_, _, cx| {
                let code = field.read(cx).value().trim().to_owned();
                if !code.is_empty() {
                    attempt(&acts, &kept, req::<InitSignInCodeRequest>(json!({ "tool": tool, "code": code })), cx);
                }
            }));
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

/// `ProviderKey` where a build found the saved key refused: the field, Replace, and the provider's refusal.
fn provider_key(
    ctx: &Ctx,
    place: &PlaceView,
    words: &'static super::words::ProviderWords,
    kept: &Entity<Kept>,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let p = cx.palette().clone();
    let key = window.use_keyed_state(SharedString::from(format!("computer-{}-key", place.id)), cx, |window, cx| {
        InputState::new(window, cx).masked(true).placeholder("Paste a new key to replace it")
    });
    let typed = key.read(cx).value().trim().to_owned();
    let (acts, kept2, id, field) = (ctx.acts.clone(), kept.clone(), place.id.clone(), key.clone());
    let busy = kept.read(cx).step_busy;
    let save = Button::outline("cloud-save", if busy { "Checking" } else { "Replace" }).held(busy || typed.is_empty()).on_click(
        move |_, window, cx| {
            let value = field.read(cx).value().trim().to_owned();
            if value.is_empty() {
                return;
            }
            kept2.update(cx, |k, _| {
                k.step_busy = true;
                k.step_refusal = None;
            });
            field.update(cx, |f, cx| f.set_value("", window, cx));
            let (acts2, kept3, id2) = (acts.clone(), kept2.clone(), id.clone());
            call_whole(&acts, req::<InitKeysRequest>(json!({ "provider": id, "key": value })), cx, move |answer, cx| {
                kept3.update(cx, |k, _| {
                    k.step_busy = false;
                    match answer {
                        Ok(setup) if setup.setup.keys.get(&id2) == Some(&true) => {
                            k.changing = false;
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
        },
    );
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

/// The AGENTS list of a cloud's page: the agents its image holds, off the image's own record, each with its version.
pub fn agents_lists(ctx: &Ctx, _place: &PlaceView, _window: &mut Window, cx: &mut App) -> Option<CardData> {
    let image = ctx.data.image.as_ref()?.image.as_ref()?;
    let rows: Vec<GridRow> = image
        .pins
        .iter()
        .filter(|pin| super::words::is_agent(&pin.id))
        .map(|pin| {
            let signed = image.logins.iter().find(|l| l.name == pin.id).is_some_and(|l| matches!(l.state, Login::SignedIn | Login::Copied));
            let mut name = GridName::new(framed(glyph(Glyph::Bot, cx), cx), catalog_name(&pin.id));
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
    if rows.is_empty() {
        return None;
    }
    let grid = Grid::new("agents")
        .head(GridHead::new(
            Some(PAGE_COLUMNS),
            vec![HeadCell::new(where_words::HEAD_AGENTS), HeadCell::new(where_words::HEAD_VERSION).num()],
        ))
        .rows(rows);
    Some(CardData::new("agents").body(div().w_full().child(grid)).search(Vec::<Item>::new()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_build_is_on_its_stretch() {
        assert!(job_building("signing-in"));
        assert!(!job_building("answering"));
        assert!(job_over("cancelled"));
        assert_eq!(words::step_now(Some("Creating the machine"), 2, 7), "Creating the machine, 2 of 7");
        assert_eq!(words::step_now(None, 2, 7), "2 of 7 steps done");
    }
}
