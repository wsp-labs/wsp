// SPDX-License-Identifier: AGPL-3.0-only
//! Devices (`devices.tsx`): every computer and browser paired with this wsp, one row each with when it paired and when
//! it was last seen, and Revoke at the right behind its confirm. A scoped record is a running thread's token, not a
//! device, so it is not a row. A page served on a ticket socket is refused the list and says so in one line.

use std::rc::Rc;

use chrono::{DateTime, Local};
use gpui_kit::{prelude::*, App, SharedString, Window};
use serde_json::json;
use wsp_host_client::protocol::{DeviceView, DevicesRevokeRequest};

use crate::screens::settings::model::{req, Reread};
use crate::screens::settings::page::{CardData, Ctx, Item};
use crate::ui::{AlertDialog, Button, RowSkeleton, Words};

/// `DEVICES_WORDS`.
const THIS_BROWSER: &str = "this browser";
const SEEN_NOW: &str = "seen just now";
const REVOKE: &str = "Revoke";
const REVOKE_DESCRIPTION: &str = "It can no longer reach this wsp until it pairs again.";
const NONE: &str = "Nothing is paired with this wsp yet.";
const REFUSED: &str = "Who is paired is read on the computer running wsp.";
/// `WHERE_WORDS.cancel`.
pub const CANCEL: &str = "Cancel";

/// The devices a person revokes: every record with no scope.
fn revocable(devices: &[DeviceView]) -> Vec<&DeviceView> {
    devices.iter().filter(|d| d.scope.is_none()).collect()
}

/// The name a row reads: the device's own, or this browser for the one wsp init opened here.
fn device_name(device: &DeviceView) -> String {
    if device.here == Some(true) {
        THIS_BROWSER.to_owned()
    } else {
        device.name.clone()
    }
}

/// `offlineFor`: a span in whole minutes, hours or days.
fn offline_for(ms: i64) -> String {
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

/// `builtWhen`: the clock alone with today or yesterday, the day and the clock before that, in local time.
pub fn built_when(at: &str, now: DateTime<Local>) -> String {
    let Ok(when) = DateTime::parse_from_rfc3339(at) else { return at.to_owned() };
    let when = when.with_timezone(&Local);
    let clock = when.format("%H:%M");
    match (now.date_naive() - when.date_naive()).num_days() {
        ..=0 => format!("today {clock}"),
        1 => format!("yesterday {clock}"),
        _ => format!("{} {clock}", when.format("%b %-d")),
    }
}

/// When it paired and when it was last heard from; inside the minute the span would read 0 min, so it says just now.
fn device_description(device: &DeviceView, now: DateTime<Local>) -> Vec<String> {
    let seen = match DateTime::parse_from_rfc3339(&device.last_seen_at) {
        Ok(at) => {
            let since = (now.timestamp_millis() - at.timestamp_millis()).max(0);
            if since < 60_000 {
                SEEN_NOW.to_owned()
            } else {
                format!("seen {} ago", offline_for(since))
            }
        }
        Err(_) => format!("seen {} ago", offline_for(0)),
    };
    vec![format!("paired {}", built_when(&device.created_at, now)), seen]
}

pub fn cards(ctx: &Ctx, _window: &mut Window, _cx: &mut App) -> Vec<CardData> {
    let card = CardData::new("devices");
    if ctx.data.devices_refused {
        return vec![card.item(Item::line("refused", REFUSED, None, |line| line))];
    }
    let Some(devices) = ctx.data.devices.as_ref() else { return vec![card.body(RowSkeleton::new("devices-loading"))] };
    let rows = revocable(devices);
    if rows.is_empty() {
        return vec![card.item(Item::empty("none", NONE))];
    }
    let now = Local::now();
    vec![card.items(rows.into_iter().map(|device| {
        let name = device_name(device);
        let note = Words::from(device_description(device, now));
        let words = vec![SharedString::from(name.clone()), note.line()];
        let control = revoke_control(ctx, device);
        let mut item = Item::row(device.id.clone(), name, None, |row| row.note(note).mono().control(control));
        item.words = words;
        item
    }))]
}

/// Revoke: asks, then takes the device's token away and reads the list again.
fn revoke_control(ctx: &Ctx, device: &DeviceView) -> impl IntoElement {
    let acts = ctx.acts.clone();
    let device = device.clone();
    Button::danger(SharedString::from(format!("revoke-{}", device.id)), REVOKE).xs().on_click(move |_, _, cx| {
        let device = device.clone();
        acts.open_dialog(
            Rc::new(move |ctx, window, cx| {
                let busy = window.use_keyed_state(SharedString::from(format!("revoke-{}-busy", device.id)), cx, |_, _| false);
                let (acts, id) = (ctx.acts.clone(), device.id.clone());
                let cancel = {
                    let acts = ctx.acts.clone();
                    Button::outline("revoke-cancel", CANCEL).neutral_ring().on_click(move |_, _, cx| acts.close_dialog(cx))
                };
                let confirm = Button::destructive("revoke-confirm", REVOKE).held(*busy.read(cx)).on_click(move |_, _, cx| {
                    busy.update(cx, |b, _| *b = true);
                    acts.notify(cx);
                    let (acts, busy) = (acts.clone(), busy.clone());
                    let ask = req::<DevicesRevokeRequest>(json!({ "deviceId": id }));
                    acts.clone().call(ask, cx, move |answer, cx| {
                        busy.update(cx, |b, _| *b = false);
                        match answer {
                            Ok(_) => {
                                acts.close_dialog(cx);
                                acts.reread(Reread::Devices, cx);
                            }
                            Err(said) => acts.failed(said, cx),
                        }
                    });
                });
                AlertDialog::new(format!("Revoke {}?", device_name(&device)))
                    .sentence(REVOKE_DESCRIPTION)
                    .act(cancel)
                    .act(confirm)
                    .into_any_element()
            }),
            cx,
        );
    })
}

/// A page's cards drawn off `data` in a test window: each card's items by their words, its acts under it, its body.
#[cfg(test)]
pub(crate) fn shape(
    data: crate::screens::settings::model::Data,
    page: impl Fn(&Ctx, &mut Window, &mut App) -> Vec<CardData>,
    cx: &mut gpui_kit::TestAppContext,
) -> Vec<(Vec<Vec<SharedString>>, usize, bool)> {
    use crate::screens::settings::model::{At, GroupId};
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::ui::init(crate::ui::Side::Light, cx);
    });
    let runtime = std::sync::Arc::new(tokio::runtime::Builder::new_current_thread().build().unwrap());
    let window = cx.add_empty_window();
    window.update(|window, cx| {
        let at = At::Group(GroupId::General);
        let acts = crate::screens::settings::Acts::new(gpui_kit::WeakEntity::new_invalid(), None, runtime);
        let ctx = Ctx::new(&data, &at, None, acts);
        page(&ctx, window, cx)
            .into_iter()
            .map(|card| (card.items.iter().map(|i| i.words.clone()).collect(), card.under.len(), card.body.is_some()))
            .collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::screens::settings::model::Data;
    use chrono::TimeZone;

    #[gpui_kit::test]
    fn the_list_says_why_it_is_not_there(cx: &mut gpui_kit::TestAppContext) {
        let refused = Data { devices_refused: true, ..Data::default() };
        assert_eq!(shape(refused, cards, cx), [(vec![vec![SharedString::from(REFUSED)]], 0, false)]);
        assert_eq!(shape(Data::default(), cards, cx), [(vec![], 0, true)], "a skeleton while the list is on its way");
        let only_tokens = Data { devices: Some(vec![]), ..Data::default() };
        assert_eq!(shape(only_tokens, cards, cx), [(vec![vec![SharedString::from(NONE)]], 0, false)]);
    }

    fn device(created: &str, seen: &str) -> DeviceView {
        serde_json::from_value(json!({ "id": "d1", "name": "laptop", "createdAt": created, "lastSeenAt": seen })).unwrap()
    }

    #[test]
    fn a_span_reads_in_its_largest_whole_unit() {
        assert_eq!(offline_for(59_000), "0 min");
        assert_eq!(offline_for(5 * 60_000), "5 min");
        assert_eq!(offline_for(3 * 3_600_000), "3 h");
        assert_eq!(offline_for(50 * 3_600_000), "2 d");
    }

    #[test]
    fn a_row_says_when_it_paired_and_was_seen() {
        let now = Local.with_ymd_and_hms(2026, 10, 17, 15, 0, 0).unwrap();
        let today = Local.with_ymd_and_hms(2026, 10, 17, 9, 5, 0).unwrap().to_rfc3339();
        let yesterday = Local.with_ymd_and_hms(2026, 10, 16, 22, 30, 0).unwrap().to_rfc3339();
        let earlier = Local.with_ymd_and_hms(2026, 10, 3, 8, 0, 0).unwrap().to_rfc3339();
        assert_eq!(device_description(&device(&today, &now.to_rfc3339()), now), ["paired today 09:05", "seen just now"]);
        assert_eq!(device_description(&device(&yesterday, &today), now), ["paired yesterday 22:30", "seen 5 h ago"]);
        assert_eq!(built_when(&earlier, now), "Oct 3 08:00");
    }

    #[test]
    fn a_thread_token_is_no_device() {
        let mut scoped = device("2026-10-17T00:00:00Z", "2026-10-17T00:00:00Z");
        scoped.scope =
            serde_json::from_value(json!({ "kind": "thread", "threadId": "t", "workspaceId": "w", "rootThreadId": "t" })).unwrap();
        let mut here = device("2026-10-17T00:00:00Z", "2026-10-17T00:00:00Z");
        here.here = Some(true);
        let all = [scoped, here];
        let rows = revocable(&all);
        assert_eq!(rows.len(), 1);
        assert_eq!(device_name(rows[0]), "this browser");
    }
}
