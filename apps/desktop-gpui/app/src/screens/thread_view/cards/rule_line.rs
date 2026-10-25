// SPDX-License-Identifier: AGPL-3.0-only
//! `TimelineRuleLine`: the quiet lines about the workspace, the machine line and its readings, the slate line, the fork rule.

use gpui_kit::component::h_flex;
use gpui_kit::{div, prelude::*, px, AnyElement, App, Context, Hsla, SharedString, Window};

use super::spawn::open_thread;
use super::threads::on_sidebar;
use crate::thread_view::ThreadView;
use crate::transcript::{self, Row, ThreadAt};
use crate::ui::{tip, ActivePalette, Button, Palette};

/// `TimelineRuleLine`: one quiet line about the workspace, 24 px, 11 px muted, cut short and whole on hover, then any
/// act after it. The fork rule, the machine line and the failed start's words are this piece.
pub(super) fn rule_line(id: &'static str, line: &str, act: Option<AnyElement>, ink: Hsla) -> AnyElement {
    h_flex()
        .debug_selector(move || id.into())
        .h(px(24.))
        .min_w_0()
        .gap(px(8.))
        .px(px(4.))
        .text_size(px(11.))
        .text_color(ink)
        .child(div().id(id).min_w_0().truncate().tooltip(tip(line.to_owned())).child(line.to_owned()))
        .children(act)
        .into_any_element()
}

/// Wake beside a rule line (`WAKE_ACT`): `workspaces.wake` through the sidebar's runner, which says a refusal.
pub(super) fn wake_button(id: &'static str, workspace: Option<&str>) -> Option<AnyElement> {
    let workspace = workspace?.to_owned();
    let button = Button::outline(id, "Wake").xs().on_click(move |_, _, cx| {
        let params = serde_json::json!({ "workspaceId": workspace });
        on_sidebar(format!("wake {workspace}"), cx, |s, cx| s.write_op::<wsp_host_client::protocol::WorkspacesWakeRequest>(params, cx));
    });
    Some(div().debug_selector(move || id.into()).flex_none().child(button).into_any_element())
}

/// `machineWait` in `WorkingTimelineRow`: the line in place of the working line while the workspace cannot run the
/// turn, `my-1`, its elapsed in the mono face where the wait counts, then Wake where a wake answers it.
pub fn machine_wait(
    view: &mut ThreadView,
    row: &Row,
    wait: &transcript::Wait,
    since: Option<f64>,
    selector: &'static str,
    cx: &mut Context<ThreadView>,
) -> AnyElement {
    let clock = since.filter(|_| wait.elapsed).map(|since| {
        view.clock(row, since, cx);
        let look = super::super::shimmer::ClockLook { size: px(11.), line_height: None, height: px(24.) };
        div().flex_none().ml(px(4.)).child(super::super::shimmer::clock_slot(view, row, look, cx))
    });
    let workspace = view.followed_workspace().or(view.listing().and_then(|l| l.workspace.as_deref())).map(str::to_owned);
    let wake = wait.wake.then(|| wake_button("wait-wake", workspace.as_deref())).flatten();
    let acts =
        (clock.is_some() || wake.is_some()).then(|| h_flex().flex_none().gap(px(8.)).children(clock).children(wake).into_any_element());
    div()
        .debug_selector(move || selector.into())
        .my(px(4.))
        .child(rule_line("machine-wait-line", &wait.label, acts, cx.palette().muted_foreground))
        .into_any_element()
}

/// The readings the machine line reads while a thread on `workspace` shows (`useOutOfMemoryReadings`): this computer's
/// own workspace held on the host (`sys.subscribe`), every other computer's watched down a daemon channel of its own
/// while it runs (`sys.watch`, as today's link per running workspace asks), each until the view lets go of this. Only
/// the newest reading near full is held here.
pub struct Readings {
    workspace: String,
    /// The workspace by its name, which a refused ask names.
    name: String,
    road: Option<crate::screens::computer::model::Road>,
    memory: Option<crate::screens::terminal::words::Memory>,
    _observed: gpui_kit::Subscription,
}

impl Readings {
    pub fn memory(&self) -> Option<crate::screens::terminal::words::Memory> {
        self.memory
    }
}

/// `outOfMemoryReading` off what is kept of a computer: the newest sample near full while its road is down.
fn near_full(kept: &crate::screens::computer::live::Readings) -> Option<crate::screens::terminal::words::Memory> {
    let last = kept.ring.last().filter(|_| !kept.live)?;
    let memory = crate::screens::terminal::words::Memory { used: last.mem.used, total: last.mem.total, load1: last.load1 };
    memory.near_full().then_some(memory)
}

/// The readings for the thread followed at `at` (its host and workspace): the ones held while the workspace stays,
/// new ones for another, none while nothing is followed. Dropping the last handle lets go of the host's.
pub fn readings(
    held: Option<gpui_kit::Entity<Readings>>,
    at: Option<(&crate::screens::Host, &str)>,
    sidebar: &gpui_kit::Entity<crate::sidebar::Sidebar>,
    cx: &mut Context<ThreadView>,
) -> Option<gpui_kit::Entity<Readings>> {
    let (host, workspace) = at?;
    // The kind is known once the sidebar has read the workspace's record.
    let record = sidebar.read(cx).workspace(workspace).0;
    let kind = record.and_then(|w| w.get("kind")?.as_str().map(str::to_owned));
    let name = record.and_then(|w| w.get("name")?.as_str().map(str::to_owned)).unwrap_or_else(|| workspace.to_owned());
    use crate::screens::computer::model::{metrics_road, Road};
    let status = sidebar.read(cx).workspace(workspace).1;
    let phase = status.or(record).and_then(|w| w.get("phase")?.as_str()).unwrap_or("running");
    let road = match metrics_road(kind.as_deref()) {
        Road::Host => Some(Road::Host),
        Road::Daemon => (record.is_some() && phase == "running").then_some(Road::Daemon),
    };
    if let Some(held) = held.filter(|h| h.read(cx).workspace == workspace && h.read(cx).road == road) {
        return Some(held);
    }
    let (live, view, workspace, host) = (crate::screens::computer::live::live(cx), cx.weak_entity(), workspace.to_owned(), host.clone());
    Some(cx.new(|cx: &mut Context<Readings>| {
        let key = workspace.clone();
        match road {
            Some(Road::Host) => {
                live.update(cx, |l, cx| l.hold_host(&host, &workspace, cx));
                let live = live.clone();
                cx.on_release(move |_, cx| live.update(cx, |l, cx| l.release_host(&key, cx))).detach();
            }
            Some(Road::Daemon) => {
                let target = crate::screens::computer::live::Target::Workspace(workspace.clone());
                let (run, mut heard, stop) = crate::screens::computer::live::watch_daemon(&host, &target);
                let (fed, feed_key) = (live.clone(), key.clone());
                let task = cx.spawn(async move |_, cx| {
                    let _run = run;
                    while let Some(heard) = futures::StreamExt::next(&mut heard).await {
                        fed.update(cx, |l, cx| l.hear(&feed_key, heard, cx));
                    }
                });
                let live = live.clone();
                // The watch closes its own channel, one still opening included, and ends.
                cx.on_release(move |_, cx| {
                    _ = stop.send(futures::channel::oneshot::channel().0);
                    task.detach();
                    live.update(cx, |l, _| l.keep(&key, |r| r.live = false));
                })
                .detach();
            }
            None => {}
        }
        let memory = near_full(&live.read(cx).readings(&workspace));
        let observed = cx.observe(&live, move |this: &mut Readings, live, cx| {
            // `wireHostLive`: the host's refusal is said once on each socket.
            if let Some(words) = live.update(cx, |l, _| l.unsaid(&this.workspace)).filter(|_| this.road == Some(Road::Host)) {
                let said = crate::screens::computer::live::refused_line(&this.name, &words);
                crate::ui::notify(cx, crate::ui::Notice::error(said));
            }
            let memory = near_full(&live.read(cx).readings(&this.workspace));
            if memory != this.memory {
                this.memory = memory;
                // After this update ends, as the relist reads what this holds.
                let view = view.clone();
                cx.defer(move |cx| _ = view.update(cx, |v, cx| v.reread(cx)));
            }
        });
        Readings { workspace, name, road, memory, _observed: observed }
    }))
}

#[cfg(test)]
thread_local! {
    /// The slate line's last ink in this test thread.
    pub static SLATE_INK: std::cell::Cell<Option<Hsla>> = const { std::cell::Cell::new(None) };
}

/// `SlateUpdatedLine`: the rule line as a button, `mt-1` under the reply, its ink easing to the foreground under the
/// pointer over 150 ms; a press opens the Slate tab.
pub(super) fn slate_line(key: &str, line: &str, p: &Palette, window: &mut Window, cx: &mut Context<ThreadView>) -> AnyElement {
    let id = SharedString::from(format!("slate-updated-{key}"));
    let hovered = crate::ui::motion::hovered(id.clone(), window, cx);
    let target = if *hovered.read(cx) { p.foreground } else { p.muted_foreground };
    let ink = crate::ui::motion::ease_ink(id.clone(), "ink", target, crate::ui::motion::COLORS, window, cx);
    #[cfg(test)]
    SLATE_INK.with(|i| i.set(Some(ink)));
    div()
        .id(id)
        .debug_selector(|| "slate-updated".into())
        .mt(px(4.))
        .rounded(px(8.))
        .cursor_pointer()
        .on_hover(crate::ui::motion::track_hover(hovered))
        .on_click(|_, _, cx| crate::screens::slate::open_tab(cx))
        .child(rule_line("slate-updated-line", line, None, ink))
        .into_any_element()
}

/// `ForkRuleRow`: "Forked from {title}", and Open while the thread it came from is listed.
pub(super) fn fork(line: &str, open: Option<ThreadAt>, p: &Palette, _cx: &App) -> AnyElement {
    let act = open.map(|at| {
        let open = Button::outline("fork-open", "Open").xs().on_click(move |_, _, cx| open_thread(&at, cx));
        div().debug_selector(|| "fork-open".into()).flex_none().child(open).into_any_element()
    });
    rule_line("fork", line, act, p.muted_foreground)
}
