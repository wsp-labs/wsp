// SPDX-License-Identifier: AGPL-3.0-only
//! What the pane reads off the sidebar's own table and records: the footer as `MachineSurface` filters the workspace's
//! acts (`menu::workspace_items`), where a workspace runs (`whereRuns`) and a sub-machine's facts (`subMachineFacts`).

use serde_json::Value;

use crate::screens::sidebar::looks::{place_name, provider_key_name};
use crate::screens::sidebar::menu::{Act, Item};
use crate::sidebar::Sidebar;

/// The acts the pane draws, in its order; the open verbs live on the tab strip and the row.
const PANE_ACTS: [fn(&Act) -> bool; 7] = [
    |a| *a == Act::Phase,
    |a| *a == Act::Reconnect,
    |a| *a == Act::Rebuild,
    |a| *a == Act::BringBack,
    |a| *a == Act::ExportProject,
    |a| *a == Act::DeleteTask,
    |a| *a == Act::ForgetTask,
];

/// One footer button: the act the sidebar runs, its word, and its refusal or else its hint on the hover.
#[derive(Clone, Debug, PartialEq)]
pub struct Button {
    pub act: Act,
    pub word: String,
    pub refusal: Option<String>,
    pub hint: Option<String>,
    pub destructive: bool,
}

impl Button {
    /// The web's action id, which the button's element ids carry.
    pub fn id(&self) -> &'static str {
        match self.act {
            Act::Phase => "phase",
            Act::Reconnect => "start-daemon",
            Act::Rebuild => "rebuild",
            Act::BringBack => "bring-back",
            Act::ExportProject => "export-project",
            Act::DeleteTask => "delete",
            _ => "forget",
        }
    }

    pub fn tooltip(&self) -> Option<&str> {
        self.refusal.as_deref().or(self.hint.as_deref())
    }
}

/// The footer: the workspace's acts as the sidebar resolves them, the three a sub-machine has no use for left out,
/// and a sub-machine's export said as the copy to this computer it is.
pub fn footer(items: Vec<Item>, sub: bool, here: &str) -> Vec<Button> {
    let leaves = |a: &Act| sub && matches!(a, Act::Phase | Act::Rebuild | Act::BringBack);
    PANE_ACTS
        .iter()
        .filter_map(|is| items.iter().find(|i| is(&i.act)))
        .filter(|i| !leaves(&i.act))
        .map(|i| Button {
            act: i.act.clone(),
            word: match i.act {
                Act::ExportProject if sub => format!("Copy to {here}"),
                _ => i.word.map_or_else(|| i.label.to_string(), str::to_owned),
            },
            refusal: i.refusal.clone(),
            hint: i.hint.clone(),
            destructive: i.destructive,
        })
        .collect()
}

/// `whereRuns`: the computer by its row's name, with what that row is unless it is this computer; a workspace with
/// no row by its provider, else its kind's word, else its machine.
pub fn where_runs(s: &Sidebar, id: &str, ws: &Value) -> String {
    let at = s.place_of(id).and_then(|at| s.around().places.iter().find(|p| p["id"] == at.as_str()));
    let kind = ws["kind"].as_str().unwrap_or("cloud");
    let Some(at) = at else {
        if kind == "local" {
            return s.here_name();
        }
        let provider = ws["provider"].as_str().map(provider_key_name);
        let word = provider.or_else(|| (kind == "cloud").then(|| "a provider".to_owned()));
        return word.or_else(|| ws["machineId"].as_str().map(str::to_owned)).unwrap_or_default();
    };
    let name = place_name(at);
    match at["id"].as_str() {
        Some("here") => name,
        _ if at["kind"] == "provider" => format!("{name} (cloud)"),
        _ => format!("{name} (a computer you joined)"),
    }
}

/// `kindWords(kind).cpu`.
pub fn cpu_word(ws: &Value) -> &'static str {
    if ws["kind"].as_str().unwrap_or("cloud") == "cloud" {
        "vCPU"
    } else {
        "cores"
    }
}

/// `fmtRateDollars`: a tenth of a cent where it shows, else cents.
fn rate_dollars(x: f64) -> String {
    let mils = super::model::to_fixed(x, 3);
    if mils.ends_with('0') {
        format!("${}", super::model::to_fixed(x, 2))
    } else {
        format!("${mils}")
    }
}

/// `minutesWord`: at least a minute, whole hours and the minutes past them.
fn minutes_word(ms: f64) -> String {
    let m = ((ms / 60_000.).ceil() as i64).max(1);
    match (m / 60, m % 60) {
        (0, m) => format!("{m} min"),
        (h, 0) => format!("{h} h"),
        (h, m) => format!("{h} h {m} min"),
    }
}

/// `subMachineFacts`: what it costs, and when its disk goes; nothing once it is released.
pub fn sub_facts(
    sub: &Value,
    working: bool,
    accrued_usd: Option<f64>,
    now_ms: f64,
    disk_kept_until: Option<f64>,
) -> (Option<String>, Option<String>) {
    use chrono::TimeZone as _;
    if sub["state"] == "released" {
        return (None, None);
    }
    let cost: Vec<String> = [
        sub["rateUsdPerHour"].as_f64().map(|rate| format!("{}/hr", rate_dollars(rate))),
        accrued_usd.map(|usd| format!("{} so far", rate_dollars(usd))),
    ]
    .into_iter()
    .flatten()
    .collect();
    let held = sub.get("held").is_some_and(|h| !h.is_null());
    let goes = if working {
        None
    } else if let (true, Some(until)) = (held, disk_kept_until) {
        let day = chrono::Local.timestamp_millis_opt(until as i64).single().map(|d| d.format("%b %-d").to_string()).unwrap_or_default();
        Some(format!("{} may delete its disk from {day}", provider_key_name(sub["on"].as_str().unwrap_or_default())))
    } else if held {
        Some("Kept until its work is pushed".to_owned())
    } else {
        sub["deleteAt"].as_f64().map(|at| format!("Deleted in {}", minutes_word(at - now_ms)))
    };
    ((!cost.is_empty()).then(|| cost.join(", ")), goes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn item(act: Act, label: &str, word: Option<&'static str>) -> Item {
        Item { act, label: label.to_owned().into(), group: "g", refusal: None, keys: None, destructive: false, word, hint: None }
    }

    #[test]
    fn the_footer_draws_the_panes_acts_in_its_order_and_a_sub_machine_leaves_three() {
        let items = || {
            vec![
                item(Act::DeleteTask, "Delete task", Some("Delete")),
                item(Act::NewThreadHere, "New thread here", None),
                item(Act::Phase, "Pause fix", Some("Pause")),
                item(Act::ExportProject, "Export project", None),
                item(Act::BringBack, "Bring back", None),
                item(Act::RenameTask, "Rename task", None),
                item(Act::Reconnect, "Reconnect", Some("Start again")),
                item(Act::Rebuild, "Rebuild task", Some("Rebuild")),
            ]
        };
        let footer = footer(items(), false, "mini");
        let ids: Vec<&str> = footer.iter().map(Button::id).collect();
        assert_eq!(ids, ["phase", "start-daemon", "rebuild", "bring-back", "export-project", "delete"]);
        let words: Vec<&str> = footer.iter().map(|b| b.word.as_str()).collect();
        assert_eq!(words, ["Pause", "Start again", "Rebuild", "Bring back", "Export project", "Delete"]);
        let sub = super::footer(items(), true, "mini");
        let words: Vec<&str> = sub.iter().map(|b| b.word.as_str()).collect();
        assert_eq!(words, ["Start again", "Copy to mini", "Delete"]);
    }

    #[test]
    fn a_sub_machine_says_its_cost_and_when_it_goes() {
        let sub = json!({ "state": "napping", "on": "box", "rateUsdPerHour": 0.125, "deleteAt": 3_600_000. + 90_000. });
        let (cost, goes) = sub_facts(&sub, false, Some(0.5), 0., None);
        assert_eq!(cost.as_deref(), Some("$0.125/hr, $0.50 so far"));
        assert_eq!(goes.as_deref(), Some("Deleted in 1 h 2 min"));
        assert_eq!(sub_facts(&sub, true, None, 0., None).1, None, "a working one goes nowhere");
        let held = json!({ "state": "napping", "on": "box", "held": { "files": 1, "commits": 0 } });
        assert_eq!(sub_facts(&held, false, None, 0., None).1.as_deref(), Some("Kept until its work is pushed"));
        assert_eq!(sub_facts(&json!({ "state": "released" }), false, Some(1.), 0., None), (None, None));
    }
}
