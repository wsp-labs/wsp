// SPDX-License-Identifier: AGPL-3.0-only
//! The Version card under General (`about.tsx`): the version this wsp runs, whether a newer release waits, and the
//! road to it. Once newer files are installed under a running host, Restart host stands in Get's place; level with
//! the release, Check for updates asks the host at once. This app's half and the host's get a line each only once
//! they run apart.

use std::cmp::Ordering;

use gpui_kit::{prelude::*, px, svg, App, SharedString, Window};
use serde_json::json;
use wsp_host_client::protocol::{HostRestartRequest, ReleaseCheckRequest, ReleaseView};

use crate::screens::settings::model::req;
use crate::screens::settings::page::{CardData, Ctx, Item};
use crate::screens::settings::words::{built_when, read_when, stamp_ms};
use crate::ui::{framed, ActivePalette, Button, MARK};

/// `RELEASES` in `bundles.mjs`.
const RELEASES: &str = "https://github.com/wsp-labs/wsp/releases";

/// `ABOUT_WORDS` in `format.ts`, and `UPDATE_WORDS.notRestarted` in `shell/update.ts`.
mod words {
    pub const TITLE: &str = "Version";
    pub const WSP: &str = "wsp";
    pub const CHECKING: &str = "Checking for updates";
    pub const CHECKS_OFF: &str = "Update checks are off";
    pub const CHECK_NOW: &str = "Check for updates";
    pub const WHATS_NEW: &str = "What's new";
    pub const UNKNOWN: &str = "unknown";
    pub const OFF_HOVER: &str = "Update checks are off on the host: WSP_UPDATE_CHECK is 0.";
    pub const RESTART_HOST: &str = "Restart host";
    pub const RESTART_HOVER: &str = "Drops open terminal panes, localhost forwards and any sign-in in progress; running turns continue.";
    pub const APP: &str = "App";
    pub const APP_HOVER: &str = "The desktop shell holding this window.";
    pub const HOST: &str = "Host";
    pub const HOST_HOVER: &str = "The wsp that serves this page.";
    pub const RESTART_RUNS: &str = "Restart host runs them.";
    pub const RESTART_THERE: &str = "A restart on the computer it runs on runs them.";

    pub fn host_update_hover(line: &str, version: &str) -> String {
        format!("The wsp that serves this page. {line} gets {version}.")
    }
    pub fn host_installed_hover(installed: &str, then: &str) -> String {
        format!("The wsp that serves this page. Its files carry {installed} now. {then}")
    }

    pub fn up_to_date(when: &str) -> String {
        format!("Up to date, checked {when}")
    }
    pub fn unreached(checked: Option<&str>, missed: &str) -> String {
        match checked {
            None => format!("Could not reach GitHub {missed}"),
            Some(checked) => format!("Checked {checked}; could not reach GitHub {missed}"),
        }
    }
    pub fn available(version: &str) -> String {
        format!("{version} is out")
    }
    pub fn read_hover(when: &str) -> String {
        format!("Read from the releases page {when}.")
    }
    pub fn missed_hover(when: &str, at: &str) -> String {
        format!("Read {when}; the releases page was not reached {at}.")
    }
    pub fn unreached_hover(at: &str) -> String {
        format!("The releases page was not reached {at}.")
    }
    pub fn get(version: &str) -> String {
        format!("Get {version}")
    }
    pub fn not_restarted(said: &str) -> String {
        format!("wsp did not restart: {said}")
    }
}

pub use super::agents::model::compare_versions;

/// The release this app is, standing in for the desktop shell's `window.wsp.version`: the one a host's is read against.
pub fn app_version() -> &'static str {
    static VERSION: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    VERSION.get_or_init(wsp_host_client::HostClient::app_release)
}

/// Both halves as this app reads them: itself, and the host it is attached to.
#[derive(Clone, Copy)]
struct Halves<'a> {
    app: Option<&'a str>,
    host: Option<&'a str>,
}

impl<'a> Halves<'a> {
    fn of(ctx: &'a Ctx) -> Self {
        Halves { app: Some(app_version()).filter(|v| !v.is_empty()), host: ctx.data.host_version.as_deref() }
    }

    /// Whether the two run apart, so each gets a line.
    fn apart(self) -> Option<(&'a str, &'a str)> {
        let (app, host) = (self.app?, self.host?);
        (app != host).then_some((app, host))
    }
}

/// `releaseAhead`: the release this app or its host is behind, or nothing.
fn release_behind<'a>(release: &'a ReleaseView, halves: Halves) -> Option<&'a str> {
    let latest = release.latest.as_ref()?;
    let behind = |running: &str| compare_versions(&latest.version, running) == Ordering::Greater;
    (halves.host.is_some_and(behind) || halves.app.is_some_and(behind)).then_some(latest.version.as_str())
}

/// `hostHover`: the files installed under the running host first, since only its restart is left, then the line that
/// installs the release while the host is behind it.
fn host_hover(release: Option<&ReleaseView>, elsewhere: bool) -> String {
    if let Some(release) = release {
        if let Some(installed) = &release.installed {
            let then = release.restart_refusal.as_deref().unwrap_or(if restart_shown(release, elsewhere) {
                words::RESTART_RUNS
            } else {
                words::RESTART_THERE
            });
            return words::host_installed_hover(installed, then);
        }
        if let (Some(update), Some(latest)) = (&release.update, &release.latest) {
            return words::host_update_hover(update, &latest.version);
        }
    }
    words::HOST_HOVER.to_owned()
}

fn ago(at: Option<&str>, now: i64) -> String {
    read_when(at.and_then(stamp_ms).map_or(0, |ms| now - ms) as f64)
}

/// `latestHover`: the What's new button's hover, when the host last read the releases page.
fn latest_hover(release: &ReleaseView, now: i64) -> Option<String> {
    let state = release.state.to_string();
    if state == "off" {
        return Some(words::OFF_HOVER.to_owned());
    }
    let missed = (state == "unreached").then_some(release.tried_at.as_deref()).flatten().map(|at| built_when(at, now));
    let (Some(_), Some(checked)) = (&release.latest, &release.checked_at) else { return missed.map(|m| words::unreached_hover(&m)) };
    let when = ago(Some(checked), now);
    Some(match missed {
        None => words::read_hover(&when),
        Some(missed) => words::missed_hover(&when, &missed),
    })
}

/// `stateLine`: whether a newer release waits, as the host last read it, and when it last asked.
fn state_line(release: Option<&ReleaseView>, behind: Option<&str>, now: i64) -> String {
    if let Some(version) = behind {
        return words::available(version);
    }
    let Some(release) = release else { return words::CHECKING.to_owned() };
    match release.state.to_string().as_str() {
        "checking" => words::CHECKING.to_owned(),
        "off" => words::CHECKS_OFF.to_owned(),
        state => {
            let checked = release.checked_at.as_deref().map(|at| ago(Some(at), now));
            match (state, &checked) {
                ("read", Some(checked)) => words::up_to_date(checked),
                _ => words::unreached(checked.as_deref(), &ago(release.tried_at.as_deref(), now)),
            }
        }
    }
}

/// `restartShown`: newer files are installed under the host, a restart brings it back, and the host runs on this
/// computer, since it refuses a restart asked from anywhere else.
fn restart_shown(release: &ReleaseView, elsewhere: bool) -> bool {
    release.installed.is_some() && release.restart_refusal.is_none() && !elsewhere
}

/// `UpdateStep`: Restart host where newer files wait under the host, else the release page's Get.
fn update_step(ctx: &Ctx, release: &ReleaseView, behind: Option<&str>) -> Option<Button> {
    if let (Some(_), true) = (&release.installed, restart_shown(release, ctx.data.host_elsewhere)) {
        let acts = ctx.acts.clone();
        return Some(Button::outline("restart-host", words::RESTART_HOST).xs().tooltip(words::RESTART_HOVER).on_click(move |_, _, cx| {
            let failed = acts.clone();
            acts.call(req::<HostRestartRequest>(json!({})), cx, move |answer, cx| {
                if let Err(said) = answer {
                    failed.failed(words::not_restarted(&said), cx);
                }
            });
        }));
    }
    let version = behind?;
    let url = release.latest.as_ref().map_or(RELEASES.to_owned(), |l| l.url.clone());
    Some(Button::outline("get-release", words::get(version)).xs().on_click(move |_, _, cx| cx.open_url(&url)))
}

pub fn cards(ctx: &Ctx, _window: &mut Window, cx: &mut App) -> Vec<CardData> {
    let now = chrono::Utc::now().timestamp_millis();
    let release = ctx.data.release.as_ref();
    let halves = Halves::of(ctx);
    let behind = release.and_then(|r| release_behind(r, halves));
    let hover = release.and_then(|r| latest_hover(r, now));
    let notes = release.and_then(|r| r.latest.as_ref()).map_or(RELEASES.to_owned(), |l| l.url.clone());
    let step = match release {
        Some(r) if restart_shown(r, ctx.data.host_elsewhere) || behind.is_some() => update_step(ctx, r, behind),
        Some(r) if r.state.to_string() != "off" => {
            let acts = ctx.acts.clone();
            Some(Button::outline("check-updates", words::CHECK_NOW).xs().held(r.state.to_string() == "checking").on_click(
                move |_, _, cx| {
                    let failed = acts.clone();
                    acts.call(req::<ReleaseCheckRequest>(json!({ "force": true })), cx, move |answer, cx| {
                        if let Err(said) = answer {
                            failed.failed(said, cx);
                        }
                    });
                },
            ))
        }
        _ => None,
    };
    let ink = cx.palette().foreground;
    let mark = SharedString::from(halves.host.or(halves.app).unwrap_or(words::UNKNOWN).to_owned());
    let line = state_line(release, behind, now);
    let row = Item::row("version", words::WSP, Some(&line), |row| {
        let row = row.lead(framed(svg().path(MARK).size(px(16.)).text_color(ink), cx)).mark(mark);
        match step {
            Some(step) => row.control(step),
            None => row,
        }
    });
    let mut whats_new = Button::outline("whats-new", words::WHATS_NEW).xs().on_click(move |_, _, cx| cx.open_url(&notes));
    if let Some(hover) = hover {
        whats_new = whats_new.tooltip(hover);
    }
    // A card holds rows or lines, never both: the halves that run apart stand in a card of their own under the version.
    let version = CardData::new("version").head(words::TITLE).item(row);
    match halves.apart() {
        None => vec![version.under(whats_new)],
        Some((app, host)) => {
            let host_hover = host_hover(release, ctx.data.host_elsewhere);
            let lines = [
                Item::line("app-version", words::APP, Some(words::APP_HOVER), |line| line.value(app.to_owned()).hover(words::APP_HOVER)),
                Item::line("host-version", words::HOST, Some(&host_hover), |line| line.value(host.to_owned()).hover(host_hover.clone())),
            ];
            vec![version, CardData::new("version-parts").items(lines).under(whats_new)]
        }
    }
}

/// `versionMeta`: General's one word in the sidebar, the newer version while this wsp is behind it.
pub fn meta(ctx: &Ctx) -> Option<SharedString> {
    ctx.data.release.as_ref().and_then(|r| release_behind(r, Halves::of(ctx))).map(|v| SharedString::from(v.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_order_as_semver_does() {
        assert_eq!(compare_versions("0.1.10", "0.1.9"), Ordering::Greater);
        assert_eq!(compare_versions("0.2", "0.2.0"), Ordering::Equal);
        assert_eq!(compare_versions("1.0.0", "1.0.0-rc.2"), Ordering::Greater);
        assert_eq!(compare_versions("1.0.0-rc.10", "1.0.0-rc.2"), Ordering::Greater);
        assert_eq!(compare_versions("1.0.0-rc", "1.0.0-rc.1"), Ordering::Less);
        assert_eq!(compare_versions("0.3.7+abc", "0.3.7"), Ordering::Equal);
    }

    #[test]
    fn either_half_behind_the_release_is_behind_and_apart_halves_get_lines() {
        let read: ReleaseView = serde_json::from_value(json!({ "state": "read", "checkedAt": "2026-10-17T11:00:00Z",
            "latest": { "version": "0.3.8", "tag": "v0.3.8", "url": "u", "publishedAt": "2026-10-16T00:00:00Z" } }))
        .unwrap();
        let both = |app, host| Halves { app: Some(app), host: Some(host) };
        assert_eq!(release_behind(&read, both("0.3.7", "0.3.8")), Some("0.3.8"), "an app behind a current host is behind");
        assert_eq!(release_behind(&read, both("0.3.8", "0.3.7")), Some("0.3.8"), "a host behind a current app is behind");
        assert_eq!(release_behind(&read, both("0.3.8", "0.3.8")), None);
        assert_eq!(release_behind(&read, Halves { app: Some("0.3.7"), host: None }), Some("0.3.8"), "the app alone still counts");
        assert_eq!(both("0.3.7", "0.3.8").apart(), Some(("0.3.7", "0.3.8")));
        assert_eq!(both("0.3.8", "0.3.8").apart(), None, "one number while the halves agree");
        assert_eq!(Halves { app: Some("0.3.8"), host: None }.apart(), None);
        let installed: ReleaseView = serde_json::from_value(json!({ "state": "read", "installed": "0.3.9" })).unwrap();
        assert_eq!(
            host_hover(Some(&installed), false),
            "The wsp that serves this page. Its files carry 0.3.9 now. Restart host runs them."
        );
        let update: ReleaseView = serde_json::from_value(json!({ "state": "read", "update": "wsp update",
            "latest": { "version": "0.3.8", "tag": "v0.3.8", "url": "u", "publishedAt": "2026-10-16T00:00:00Z" } }))
        .unwrap();
        assert_eq!(host_hover(Some(&update), false), "The wsp that serves this page. wsp update gets 0.3.8.");
        assert_eq!(host_hover(None, false), "The wsp that serves this page.");
        assert!(app_version().split('.').count() == 3, "the app knows its release: {:?}", app_version());
    }

    #[test]
    fn restart_host_shows_only_where_the_host_runs_on_this_computer() {
        let installed: ReleaseView = serde_json::from_value(json!({ "state": "read", "installed": "0.3.9" })).unwrap();
        assert!(restart_shown(&installed, false));
        assert!(!restart_shown(&installed, true), "a host on another computer refuses the restart, so none is offered");
        assert_eq!(
            host_hover(Some(&installed), true),
            "The wsp that serves this page. Its files carry 0.3.9 now. A restart on the computer it runs on runs them."
        );
    }

    #[test]
    fn the_state_line_says_when_the_host_last_asked() {
        let now = stamp_ms("2026-10-17T12:00:00Z").unwrap();
        let release = |v: serde_json::Value| -> ReleaseView { serde_json::from_value(v).unwrap() };
        let read = release(json!({ "state": "read", "checkedAt": "2026-10-17T11:00:00Z",
            "latest": { "version": "0.3.8", "tag": "v0.3.8", "url": "u", "publishedAt": "2026-10-16T00:00:00Z" } }));
        assert_eq!(state_line(Some(&read), None, now), "Up to date, checked 1 h ago");
        let host = |v| Halves { app: None, host: Some(v) };
        assert_eq!(state_line(Some(&read), release_behind(&read, host("0.3.7")), now), "0.3.8 is out");
        assert_eq!(release_behind(&read, host("0.3.8")), None);
        assert_eq!(state_line(None, None, now), "Checking for updates");
        let unreached = release(json!({ "state": "unreached", "triedAt": "2026-10-17T11:59:30Z" }));
        assert_eq!(state_line(Some(&unreached), None, now), "Could not reach GitHub just now");
        assert_eq!(state_line(Some(&release(json!({ "state": "off" }))), None, now), "Update checks are off");
        assert_eq!(read_when(3. * 86_400_000.), "3 d ago");
    }
}
