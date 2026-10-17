// SPDX-License-Identifier: AGPL-3.0-only
//! Privacy (`privacy.tsx`): what this wsp asks of a service outside the person's computers and what it reads on this
//! one, each as one switch.

use gpui_kit::{div, prelude::*, App, Window};
use serde_json::{json, Value};
use wsp_host_client::protocol::Preferences;

use super::general::{field_switch, here_name, once_named, record, reset_to};
use crate::screens::settings::page::{CardData, Ctx, Item};
use crate::ui::{tip, Switch};

/// `PRIVACY_WORDS` in `format.ts`.
mod words {
    pub const SERVER_ICONS: &str = "Server icons from Google";
    pub const SERVER_ICONS_NOTE: &str =
        "wsp asks Google for each public server's icon by host name; turning this off deletes the saved icons.";
    pub const AGENT_VERSIONS: &str = "Newest agent versions";
    pub const AGENT_VERSIONS_NOTE: &str = "wsp asks npm, GitHub and each agent's maker for every agent's newest version, once a day.";
    pub const AGENT_VERSIONS_HELD: &str = "Off on the host: WSP_UPDATE_CHECK is 0.";
    pub const USAGE_LOGS: &str = "Agent logs";
    pub const PRODUCT_USAGE: &str = "Anonymous usage counts";
    pub const PRODUCT_USAGE_NOTE: &str =
        "wsp sends PostHog counts of threads, turns, setups and failures; never a path, a prompt, a name or a key.";

    pub fn usage_logs_note(here: &str) -> String {
        super::once_named(here, |h| {
            format!(
                "Usage counts what Claude Code, Codex and OpenCode logged on {h} and on every computer you added, wsp's own threads there included. Each computer reads its own logs, and wsp shows what they count on the Usage page alone, never to an agent."
            )
        })
    }
}

/// The four switches, each on by default.
const FIELDS: [&str; 4] = ["serverIcons", "agentVersions", "usageLogs", "productUsage"];

/// `PRIVACY_DEFAULTS`: the one patch Restore defaults writes.
pub fn defaults() -> Value {
    Value::Object(FIELDS.iter().map(|f| (f.to_string(), json!(true))).collect())
}

/// `privacyOffDefaults`.
pub fn off_defaults(p: &Preferences) -> bool {
    !(p.server_icons && p.agent_versions && p.usage_logs && p.product_usage)
}

pub fn cards(ctx: &Ctx, _window: &mut Window, _cx: &mut App) -> Vec<CardData> {
    let Some(p) = record(ctx) else { return Vec::new() };
    // The host's environment switch outvotes the record, so the row says so rather than offering a switch that asks.
    let env_off = ctx.data.release.as_ref().is_some_and(|r| r.state.to_string() == "off");
    let on = json!(true);
    vec![CardData::new("asks").items([
        Item::row("server-icons", words::SERVER_ICONS, Some(words::SERVER_ICONS_NOTE), |row| {
            reset_to(row.control(field_switch(ctx, &p, "server-icons", "serverIcons")), ctx, &p, "serverIcons", on.clone())
        }),
        Item::row("agent-versions", words::AGENT_VERSIONS, Some(words::AGENT_VERSIONS_NOTE), |row| {
            let checked = !env_off && p["agentVersions"].as_bool().unwrap_or(false);
            let acts = ctx.acts.clone();
            let switch = Switch::new("agent-versions", checked)
                .held(env_off)
                .on_change(move |on, _, cx| acts.set_preferences(json!({ "agentVersions": *on }), cx));
            row.control(div().id("agent-versions-held").when(env_off, |el| el.tooltip(tip(words::AGENT_VERSIONS_HELD))).child(switch))
        }),
        Item::row("usage-logs", words::USAGE_LOGS, Some(words::usage_logs_note(&here_name(ctx.data)).as_str()), |row| {
            reset_to(row.control(field_switch(ctx, &p, "usage-logs", "usageLogs")), ctx, &p, "usageLogs", on.clone())
        }),
        Item::row("product-usage", words::PRODUCT_USAGE, Some(words::PRODUCT_USAGE_NOTE), |row| {
            reset_to(row.control(field_switch(ctx, &p, "product-usage", "productUsage")), ctx, &p, "productUsage", on.clone())
        }),
    ])]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restore_writes_every_switch_back_on() {
        assert_eq!(defaults(), json!({ "serverIcons": true, "agentVersions": true, "usageLogs": true, "productUsage": true }));
        assert_eq!(words::usage_logs_note(""), "");
        assert!(words::usage_logs_note("box").contains("logged on box and on every computer"));
    }

    #[test]
    fn one_switch_off_is_off_the_defaults() {
        let base = json!({ "theme": "system", "sidebarMode": "list", "terminalSize": "app", "terminalZoom": {}, "access": {},
            "projectLook": {}, "projectOrder": [], "labs": false });
        let p: Preferences = serde_json::from_value(base.clone()).unwrap();
        assert!(!off_defaults(&p));
        let mut off = base;
        off["usageLogs"] = json!(false);
        assert!(off_defaults(&serde_json::from_value(off).unwrap()));
    }
}
