// SPDX-License-Identifier: AGPL-3.0-only
//! The Version card under General (`about.tsx`): the version this wsp runs, whether a newer release waits, and the
//! road to it. Once newer files are installed under a running host, Restart host stands in Get's place; level with
//! the release, Check for updates asks the host at once.

use std::cmp::Ordering;

use chrono::{DateTime, Local, TimeZone};
use gpui_kit::{prelude::*, px, svg, App, SharedString, Window};
use serde_json::json;
use wsp_host_client::protocol::{HostRestartRequest, ReleaseCheckRequest, ReleaseView};

use crate::screens::settings::model::req;
use crate::screens::settings::page::{CardData, Ctx, Item};
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
    pub fn read_when(ms: i64) -> String {
        if ms < 60_000 {
            "just now".to_owned()
        } else {
            format!("{} ago", offline_for(ms))
        }
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

    /// `offlineFor`: coarse on purpose, the figure is read once.
    pub fn offline_for(ms: i64) -> String {
        let minutes = (ms / 60_000).max(0);
        if minutes < 60 {
            return format!("{minutes} min");
        }
        let hours = minutes / 60;
        if hours < 24 {
            format!("{hours} h")
        } else {
            format!("{} d", hours / 24)
        }
    }
}

/// `compareVersions` (`semver.mjs`): the dotted core numerically, then the prerelease identifiers, build metadata
/// dropped.
pub fn compare_versions(a: &str, b: &str) -> Ordering {
    fn parts(v: &str) -> (Vec<&str>, Option<Vec<&str>>) {
        let rest = v.split('+').next().unwrap_or_default();
        match rest.split_once('-') {
            Some((core, pre)) => (core.split('.').collect(), Some(pre.split('.').collect())),
            None => (rest.split('.').collect(), None),
        }
    }
    fn ident(a: &str, b: &str) -> Ordering {
        match (a.parse::<u64>(), b.parse::<u64>()) {
            (Ok(x), Ok(y)) => x.cmp(&y),
            (Ok(_), Err(_)) => Ordering::Less,
            (Err(_), Ok(_)) => Ordering::Greater,
            _ => a.cmp(b),
        }
    }
    let ((left, left_pre), (right, right_pre)) = (parts(a), parts(b));
    for i in 0..left.len().max(right.len()) {
        let order = ident(left.get(i).unwrap_or(&"0"), right.get(i).unwrap_or(&"0"));
        if order != Ordering::Equal {
            return order;
        }
    }
    match (left_pre, right_pre) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Greater,
        (Some(_), None) => Ordering::Less,
        (Some(l), Some(r)) => {
            for i in 0..l.len().max(r.len()) {
                match (l.get(i), r.get(i)) {
                    (None, _) => return Ordering::Less,
                    (_, None) => return Ordering::Greater,
                    (Some(x), Some(y)) => {
                        let order = ident(x, y);
                        if order != Ordering::Equal {
                            return order;
                        }
                    }
                }
            }
            Ordering::Equal
        }
    }
}

/// The host's own version, which the web reads off the page the host served and no host op carries.
fn host_version(_ctx: &Ctx) -> Option<String> {
    None
}

/// `releaseAhead`: the release this wsp is behind, or nothing.
fn release_behind<'a>(release: &'a ReleaseView, host: Option<&str>) -> Option<&'a str> {
    let latest = release.latest.as_ref()?;
    host.is_some_and(|running| compare_versions(&latest.version, running) == Ordering::Greater).then_some(latest.version.as_str())
}

fn ms_of(at: &str) -> Option<i64> {
    DateTime::parse_from_rfc3339(at).ok().map(|t| t.timestamp_millis())
}

/// `builtWhen`: the clock alone on the day it happened, the day and the clock before that, `yesterday` between.
fn built_when(at: &str, now: i64) -> String {
    let (Some(ms), Some(now)) = (ms_of(at), Local.timestamp_millis_opt(now).single()) else { return at.to_owned() };
    let Some(when) = Local.timestamp_millis_opt(ms).single() else { return at.to_owned() };
    let clock = when.format("%H:%M");
    match (now.date_naive() - when.date_naive()).num_days() {
        ..=0 => format!("today {clock}"),
        1 => format!("yesterday {clock}"),
        _ => format!("{} {clock}", when.format("%b %-d")),
    }
}

fn ago(at: Option<&str>, now: i64) -> String {
    words::read_when(at.and_then(ms_of).map_or(0, |ms| now - ms))
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

/// `restartShown`: newer files are installed under the host and a restart brings it back.
fn restart_shown(release: &ReleaseView) -> bool {
    release.installed.is_some() && release.restart_refusal.is_none()
}

/// `UpdateStep`: Restart host where newer files wait under the host, else the release page's Get.
fn update_step(ctx: &Ctx, release: &ReleaseView, behind: Option<&str>) -> Option<Button> {
    if let (Some(_), true) = (&release.installed, restart_shown(release)) {
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
    let host = host_version(ctx);
    let behind = release.and_then(|r| release_behind(r, host.as_deref()));
    let hover = release.and_then(|r| latest_hover(r, now));
    let notes = release.and_then(|r| r.latest.as_ref()).map_or(RELEASES.to_owned(), |l| l.url.clone());
    let step = match release {
        Some(r) if restart_shown(r) || behind.is_some() => update_step(ctx, r, behind),
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
    let mark: SharedString = host.unwrap_or_else(|| words::UNKNOWN.to_owned()).into();
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
    vec![CardData::new("version").head(words::TITLE).item(row).under(whats_new)]
}

/// `versionMeta`: General's one word in the sidebar, the newer version while this wsp is behind it.
pub fn meta(ctx: &Ctx) -> Option<SharedString> {
    let host = host_version(ctx);
    ctx.data.release.as_ref().and_then(|r| release_behind(r, host.as_deref())).map(|v| SharedString::from(v.to_owned()))
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
    fn the_state_line_says_when_the_host_last_asked() {
        let now = ms_of("2026-10-17T12:00:00Z").unwrap();
        let release = |v: serde_json::Value| -> ReleaseView { serde_json::from_value(v).unwrap() };
        let read = release(json!({ "state": "read", "checkedAt": "2026-10-17T11:00:00Z",
            "latest": { "version": "0.3.8", "tag": "v0.3.8", "url": "u", "publishedAt": "2026-10-16T00:00:00Z" } }));
        assert_eq!(state_line(Some(&read), None, now), "Up to date, checked 1 h ago");
        assert_eq!(state_line(Some(&read), release_behind(&read, Some("0.3.7")), now), "0.3.8 is out");
        assert_eq!(release_behind(&read, Some("0.3.8")), None);
        assert_eq!(state_line(None, None, now), "Checking for updates");
        let unreached = release(json!({ "state": "unreached", "triedAt": "2026-10-17T11:59:30Z" }));
        assert_eq!(state_line(Some(&unreached), None, now), "Could not reach GitHub just now");
        assert_eq!(state_line(Some(&release(json!({ "state": "off" }))), None, now), "Update checks are off");
        assert_eq!(words::read_when(3 * 86_400_000), "3 d ago");
    }
}
