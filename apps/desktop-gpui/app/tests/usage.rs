// SPDX-License-Identifier: AGPL-3.0-only
//! Settings > Usage against a host started for this test alone (host-client's tests/common), seeded by
//! `tests/usage/seed.mjs`: what the page reads and draws, what it reads again on its own, and the reset it spends.

#[path = "../../host-client/tests/common/mod.rs"]
mod common;

use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};

use common::Throwaway;
use desktop_gpui::screens::usage::model::{self, Range, Split};
use desktop_gpui::screens::usage::words;
use desktop_gpui::screens::usage::{Tab, UsagePage};
use desktop_gpui::ui::{self, Side};
use gpui_kit::component::dialog::Confirm;
use gpui_kit::component::{Root, WindowExt as _};
use gpui_kit::{AppContext, Entity, Modifiers, TestAppContext, VisualTestContext};
use wsp_host_client::protocol::AccountsAnswerAccountsItemNote;
use wsp_host_client::HostClient;

const HOLD: Duration = Duration::from_secs(60);

/// A host with a project, seeded with thirty days of use and the five accounts the Limits tab names.
fn seeded() -> Throwaway {
    let host = Throwaway::start();
    let folder = host.repo("usage");
    host.wsp(&["add", &folder.to_string_lossy(), "--name", "usage"]);
    node(&host, &[&common::root().join("apps/desktop-gpui/app/tests/usage/seed.mjs").to_string_lossy(), &state_dir(&host)]);
    host
}

fn state_dir(host: &Throwaway) -> String {
    host.state().parent().unwrap().to_string_lossy().into_owned()
}

fn node(host: &Throwaway, args: &[&str]) -> String {
    let out = Command::new("node").args(args).env_clear().envs(host.env()).current_dir(host.dir.path()).output().unwrap();
    let said = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    assert!(out.status.success(), "node {args:?}: {said}");
    said
}

/// Files one more plan reading on the host, as a turn's limit event would, straight into its store.
fn file_reading(host: &Throwaway, key: &str, used: f64) {
    let script = format!(
        r#"process.removeAllListeners("warning");
const {{ DatabaseSync }} = await import("node:sqlite");
const db = new DatabaseSync(process.argv[1] + "/state.db");
const now = Date.now();
const row = {{ key: "{key}", agent: "codex", label: "late@example.com", road: "named", plan: "team",
  windows: [{{ kind: "week", usedPercent: {used}, resetsAt: now + 3 * 86400000 }}], status: "ok", readAt: now, computers: ["here"] }};
db.prepare("insert or replace into collections (collection, id, json) values ('limits', ?, ?)").run(row.key, JSON.stringify(row));"#
    );
    node(host, &["--input-type=module", "-e", &script, &state_dir(host)]);
}

fn open<'a>(cx: &'a mut TestAppContext, host: &Throwaway) -> (Entity<UsagePage>, &'a mut VisualTestContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
    });
    cx.executor().allow_parking();
    let runtime = Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap());
    let client = Arc::new(runtime.block_on(HostClient::connect(&host.state(), &host.env())).unwrap_or_else(|f| panic!("{}", f.message)));
    let mut page = None;
    // Wrapped in gpui-kit's Root as the app wraps every window, which holds the notice a reset raises.
    let (_, cx) = cx.add_window_view(|window, cx| {
        let usage = cx.new(|cx| UsagePage::with_hold(client, runtime, HOLD, window, cx));
        page = Some(usage.clone());
        Root::new(usage, window, cx)
    });
    (page.unwrap(), cx)
}

/// Runs the app until `done` holds of the page, the host answering on its own threads meanwhile.
fn until(cx: &mut VisualTestContext, page: &Entity<UsagePage>, what: &str, done: impl Fn(&UsagePage) -> bool) {
    let limit = Instant::now() + Duration::from_secs(30);
    loop {
        cx.run_until_parked();
        if cx.read(|cx| done(page.read(cx))) {
            return;
        }
        assert!(Instant::now() < limit, "the page never got to: {what}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// A selector built from a key, as `debug_bounds` holds the ones it is asked for for good.
fn leak(selector: String) -> &'static str {
    Box::leak(selector.into_boxed_str())
}

fn landed(page: &UsagePage, range: Range, split: Split) -> bool {
    let used = page.used.value.as_ref().is_some_and(|u| Range::from(u.range) == range && Split::from(u.split) == split);
    !page.reading() && used && page.accounts.value.is_some()
}

fn labels(page: &UsagePage) -> Vec<String> {
    page.used.value.as_ref().map(|u| u.rows.iter().map(|r| r.label.clone()).collect()).unwrap_or_default()
}

#[gpui_kit::test]
fn reads_both_tabs_from_the_host_and_draws_them(cx: &mut TestAppContext) {
    let host = seeded();
    let (page, cx) = open(cx, &host);
    until(cx, &page, "a week by agent and the accounts", |p| landed(p, Range::Week, Split::Agent) && p.models.value.is_some());
    let read = cx.read(|cx| labels(page.read(cx)));
    assert_eq!(read, ["Claude Code", "Codex", "OpenCode"], "the agents, most tokens first");
    cx.run_until_parked();
    let drawn = |cx: &mut VisualTestContext, selector: &'static str| cx.debug_bounds(selector).is_some();
    let (head, nested) = cx.read(|cx| {
        let page = page.read(cx);
        let (used, models) = (page.used.value.as_ref().unwrap(), &page.models.value.as_ref().unwrap().rows);
        let table = model::split_table(used, models);
        (table.head, table.groups.iter().flat_map(|g| g.models.iter().map(|m| m.row.key.clone())).collect::<Vec<_>>())
    });
    assert_eq!(head, words::AGENT_AND_MODEL);
    assert!(nested.len() >= 5, "the model split nested under the agents: {nested:?}");
    for key in &nested {
        assert!(drawn(cx, leak(format!("used-sub-{key}"))), "the table draws the model {key} under its agent");
    }
    for selector in ["usage-totals", "usage-chart", "used-row-claude", "used-row-codex", "usage-mix", "mix-cache-write"] {
        assert!(drawn(cx, selector), "the Usage tab draws {selector}");
    }

    cx.update(|_, cx| page.update(cx, |p, cx| p.pick_range(Range::Day, cx)));
    until(cx, &page, "today by agent", |p| landed(p, Range::Day, Split::Agent));
    let hours = cx.read(|cx| page.read(cx).used.value.as_ref().unwrap().series.len());
    assert_eq!(hours, 24, "today reads hour by hour");

    cx.update(|_, cx| page.update(cx, |p, cx| p.pick_split(Split::Source, cx)));
    until(cx, &page, "today by source", |p| landed(p, Range::Day, Split::Source));
    let read = cx.read(|cx| labels(page.read(cx)));
    assert!(read.iter().any(|l| l == "Outside wsp") && read.iter().any(|l| l == "wsp threads"), "{read:?}");
    assert!(cx.read(|cx| page.read(cx).models.value.is_none()), "only the agent split nests the models");

    cx.update(|_, cx| page.update(cx, |p, cx| p.pick_tab(Tab::Limits, cx)));
    cx.run_until_parked();
    let notes: Vec<_> = cx.read(|cx| page.read(cx).accounts.value.as_ref().unwrap().accounts.iter().filter_map(|a| a.note).collect());
    assert!(notes.contains(&AccountsAnswerAccountsItemNote::NotReadYetShowsAfterItsNextTurn), "{notes:?}");
    assert!(notes.contains(&AccountsAnswerAccountsItemNote::PaysPerTokenNoPlanLimit), "{notes:?}");
    for selector in ["usage-pool-codex", "usage-account-claude:vault-key", "usage-account-claude:vault-token", "no-limit"] {
        assert!(drawn(cx, selector), "the Limits tab draws {selector}");
    }
    assert!(drawn(cx, "usage-window"), "a pool draws its windows");
}

#[gpui_kit::test]
fn reads_again_each_hold_while_it_stands_open(cx: &mut TestAppContext) {
    let host = seeded();
    let (page, cx) = open(cx, &host);
    until(cx, &page, "the first read", |p| landed(p, Range::Week, Split::Agent));
    let has = |p: &UsagePage| p.accounts.value.as_ref().is_some_and(|a| a.accounts.iter().any(|a| a.key == "codex:acct_late"));
    assert!(!cx.read(|cx| has(page.read(cx))));

    file_reading(&host, "codex:acct_late", 40.);
    cx.run_until_parked();
    std::thread::sleep(Duration::from_millis(500));
    cx.run_until_parked();
    assert!(!cx.read(|cx| has(page.read(cx))), "nothing is read again inside the hold");

    cx.executor().advance_clock(HOLD);
    until(cx, &page, "the reading filed after the first read", has);
    let first = cx.read(|cx| page.read(cx).read_at().unwrap());
    file_reading(&host, "codex:acct_late", 97.);
    cx.executor().advance_clock(HOLD);
    let used = |p: &UsagePage| {
        p.accounts.value.as_ref().and_then(|a| a.accounts.iter().find(|a| a.key == "codex:acct_late")).map(|a| a.windows[0].used_percent)
    };
    until(cx, &page, "the account's second reading", |p| used(p) == Some(97.));
    assert!(cx.read(|cx| page.read(cx).read_at().unwrap()) > first, "the read's age starts again");
}

#[gpui_kit::test]
fn spends_a_reset_through_the_host_and_says_what_it_answered(cx: &mut TestAppContext) {
    let host = seeded();
    let (page, cx) = open(cx, &host);
    cx.update(|_, cx| page.update(cx, |p, cx| p.pick_tab(Tab::Limits, cx)));
    until(cx, &page, "the accounts", |p| landed(p, Range::Week, Split::Agent));
    let row = cx.read(|cx| {
        page.read(cx).accounts.value.as_ref().unwrap().accounts.iter().find(|a| a.credits.as_ref().is_some_and(|c| c.count > 0)).cloned()
    });
    let row = row.expect("an account with a banked reset");
    cx.run_until_parked();
    let button = cx.debug_bounds(leak(format!("use-reset-{}", row.key))).expect("its Use reset button");
    cx.simulate_click(button.center(), Modifiers::none());
    cx.run_until_parked();
    assert!(cx.update(|window, cx| window.has_active_dialog(cx)), "Use reset asks first");
    cx.dispatch_action(Confirm { secondary: false });
    until(cx, &page, "the host's answer to the reset", |p| p.said.is_some() && p.resetting.is_none());
    let said = cx.read(|cx| page.read(cx).said.clone().unwrap());
    println!("the host said: {said}");
    // The seeded account has no login on any computer here, so the host refuses the spend in its own words.
    assert!(said.contains(&row.label), "the host's sentence names the account: {said}");
    assert_eq!(cx.update(|window, cx| window.notifications(cx).len()), 1, "the answer is raised as a notice");
}
