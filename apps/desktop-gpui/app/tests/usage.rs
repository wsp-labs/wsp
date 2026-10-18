// SPDX-License-Identifier: AGPL-3.0-only
//! Settings > Usage against a host started for this test alone (host-client's tests/common), seeded by
//! `tests/usage/seed.mjs`: what the page reads and draws, what it reads again on its own, and the reset it spends.

#[path = "../../host-client/tests/common/mod.rs"]
mod common;

use std::process::Command;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use common::Throwaway;
use desktop_gpui::screens::usage::model::{self, Range, Split};
use desktop_gpui::screens::usage::words;
use desktop_gpui::screens::usage::{self as usage_screen, frame, Tab, UsagePage};
use desktop_gpui::screens::{self, entry, Host, Opened, Parts, Place, Route, Screen};
use desktop_gpui::shell::{Launch, Shell};
use desktop_gpui::ui::{self, Side};
use gpui_kit::component::dialog::Confirm;
use gpui_kit::component::{Root, WindowExt as _};
use gpui_kit::{prelude::*, AppContext, Context, Entity, Modifiers, TestAppContext, VisualTestContext, Window};
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

fn open<'a>(cx: &'a mut TestAppContext, host: &Throwaway) -> (Entity<UsagePage>, Host, &'a mut VisualTestContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
    });
    cx.executor().allow_parking();
    let runtime = Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap());
    let client = Arc::new(runtime.block_on(HostClient::connect(&host.state(), &host.env())).unwrap_or_else(|f| panic!("{}", f.message)));
    let handle = Host::new(client, runtime);
    let mut page = None;
    // Wrapped in gpui-kit's Root as the app wraps every window, which holds the notice a reset raises, under the header
    // row holding its tabs as the shell's does.
    let (_, cx) = cx.add_window_view(|window, cx| {
        let usage = cx.new(|cx| UsagePage::with_hold(handle.clone(), HOLD, window, cx));
        page = Some(usage.clone());
        let framed = cx.new(|_| Framed { page: usage });
        Root::new(framed, window, cx)
    });
    (page.unwrap(), handle, cx)
}

struct Framed {
    page: Entity<UsagePage>,
}

impl Render for Framed {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let tabs = self.page.update(cx, |page, cx| page.tabs(cx).into_any_element());
        frame(Some(tabs), Some(self.page.clone().into_any_element()), cx)
    }
}

/// Whether two reads hold the one kept answer, not two copies of it.
fn same<T>(a: &Option<Rc<T>>, b: &Option<Rc<T>>) -> bool {
    matches!((a, b), (Some(a), Some(b)) if Rc::ptr_eq(a, b))
}

fn has_late(p: &UsagePage) -> bool {
    p.accounts.value.as_ref().is_some_and(|a| a.accounts.iter().any(|a| a.key == "codex:acct_late"))
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
    let (page, _, cx) = open(cx, &host);
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
    let (page, _, cx) = open(cx, &host);
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
fn reads_again_on_coming_back_to_the_window_once_a_hold_has_passed(cx: &mut TestAppContext) {
    let host = seeded();
    let (page, _, cx) = open(cx, &host);
    until(cx, &page, "the first read", |p| landed(p, Range::Week, Split::Agent));
    let has = |p: &UsagePage| p.accounts.value.as_ref().is_some_and(|a| a.accounts.iter().any(|a| a.key == "codex:acct_late"));
    file_reading(&host, "codex:acct_late", 40.);
    let come_back = |cx: &mut VisualTestContext| {
        cx.deactivate_window();
        cx.update(|window, _| window.activate_window());
    };

    come_back(cx);
    cx.run_until_parked();
    std::thread::sleep(Duration::from_millis(500));
    cx.run_until_parked();
    assert!(!cx.read(|cx| has(page.read(cx))), "coming back inside the hold reads nothing");

    // Coming back weighs the read's age by the wall clock, so the read is aged by hand rather than by the test's timers.
    let aged = |at: Option<f64>| at.map(|at| at - 2. * HOLD.as_millis() as f64);
    cx.update(|_, cx| page.update(cx, |p, _| p.accounts.at = aged(p.accounts.at)));
    come_back(cx);
    until(cx, &page, "the reading filed before coming back", has);
}

#[gpui_kit::test]
fn spends_a_reset_through_the_host_and_says_what_it_answered(cx: &mut TestAppContext) {
    let host = seeded();
    let (page, _, cx) = open(cx, &host);
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
    until(cx, &page, "the host's answer to the reset", |p| p.said.is_some() && p.resetting.is_empty());
    let said = cx.read(|cx| page.read(cx).said.clone().unwrap());
    println!("the host said: {said}");
    // The seeded account has no login on any computer here, so the host refuses the spend in its own words.
    assert!(said.contains(&row.label), "the host's sentence names the account: {said}");
    assert_eq!(cx.update(|window, cx| window.notifications(cx).len()), 1, "the answer is raised as a notice");
}

#[gpui_kit::test]
fn holds_each_account_whose_reset_is_on_the_way(cx: &mut TestAppContext) {
    let host = seeded();
    let (page, _, cx) = open(cx, &host);
    until(cx, &page, "the accounts", |p| landed(p, Range::Week, Split::Agent));
    let first = cx.read(|cx| {
        page.read(cx).accounts.value.as_ref().unwrap().accounts.iter().find(|a| a.credits.as_ref().is_some_and(|c| c.count > 0)).cloned()
    });
    let first = first.expect("an account with a banked reset");
    let second = wsp_host_client::protocol::AccountsAnswerAccountsItem { key: "codex:acct_second".into(), ..first.clone() };
    // Both asked before either answer can land: the first account's button still waits, and a second ask for it sends nothing.
    let held = cx.update(|window, cx| {
        page.update(cx, |p, cx| {
            p.use_reset(first.clone(), window, cx);
            p.use_reset(second.clone(), window, cx);
            p.use_reset(first.clone(), window, cx);
            p.resetting.clone()
        })
    });
    assert_eq!(held, [first.key.clone(), second.key.clone()], "every reset on the way holds its own button");
    until(cx, &page, "both answers", |p| p.resetting.is_empty());
    assert_eq!(cx.update(|window, cx| window.notifications(cx).len()), 2, "one notice per reset sent");
}

#[gpui_kit::test]
fn a_visit_inside_the_hold_draws_the_kept_answers_and_one_after_it_reads_again(cx: &mut TestAppContext) {
    let host = seeded();
    let (page, handle, cx) = open(cx, &host);
    until(cx, &page, "the first read", |p| landed(p, Range::Week, Split::Agent) && p.models.value.is_some());
    file_reading(&host, "codex:acct_late", 40.);

    let again = cx.update(|window, cx| usage_screen::page(handle.clone(), window, cx));
    assert!(!cx.read(|cx| again.read(cx).reading()), "a second visit inside the hold asks the host nothing");
    cx.run_until_parked();
    std::thread::sleep(Duration::from_millis(500));
    cx.run_until_parked();
    let (kept, models) = cx.read(|cx| {
        let (first, second) = (page.read(cx), again.read(cx));
        (
            same(&first.accounts.value, &second.accounts.value) && same(&first.used.value, &second.used.value),
            same(&first.models.value, &second.models.value),
        )
    });
    assert!(kept && models, "the second visit draws the answers the first one read");
    assert!(!cx.read(|cx| has_late(again.read(cx))), "and not the reading filed since");

    // A visit holding answers for a second, two seconds on: the first page's own minute is still running.
    cx.executor().advance_clock(Duration::from_secs(2));
    let later = cx.update(|window, cx| cx.new(|cx| UsagePage::with_hold(handle.clone(), Duration::from_secs(1), window, cx)));
    assert!(cx.read(|cx| later.read(cx).reading()), "a visit after the hold reads again");
    until(cx, &later, "the reading filed after the first read", has_late);
}

#[gpui_kit::test]
fn the_range_split_and_tabs_take_the_arrows_and_the_refresh_takes_enter(cx: &mut TestAppContext) {
    let host = seeded();
    let (page, _, cx) = open(cx, &host);
    until(cx, &page, "the first read", |p| landed(p, Range::Week, Split::Agent));

    // The tab order: the refresh, the tabs, the range, the split.
    cx.update(|window, cx| window.focus_next(cx));
    assert!(cx.update(|window, cx| page.read(cx).refresh.is_focused(window)), "the refresh is the first stop");
    let before = cx.read(|cx| page.read(cx).read_at());
    cx.simulate_keystrokes("enter");
    until(cx, &page, "the read enter asked for", |p| landed(p, Range::Week, Split::Agent) && p.read_at() > before);

    cx.update(|window, cx| window.focus_next(cx));
    cx.simulate_keystrokes("right");
    assert_eq!(cx.read(|cx| page.read(cx).tab), Tab::Limits, "right moves the tabs to Limits");
    cx.simulate_keystrokes("right");
    assert_eq!(cx.read(|cx| page.read(cx).tab), Tab::Used, "and round the end back to Usage");

    cx.update(|window, cx| window.focus_next(cx));
    cx.simulate_keystrokes("right");
    assert_eq!(cx.read(|cx| page.read(cx).range), Range::Month);
    cx.simulate_keystrokes("left left");
    assert_eq!(cx.read(|cx| page.read(cx).range), Range::Day);
    until(cx, &page, "today by agent", |p| landed(p, Range::Day, Split::Agent));

    cx.update(|window, cx| window.focus_next(cx));
    cx.simulate_keystrokes("left");
    assert_eq!(cx.read(|cx| page.read(cx).split), Split::Source, "left from the first split goes round to the last");
    until(cx, &page, "today by source", |p| landed(p, Range::Day, Split::Source));
}

thread_local! {
    static MOUNTED: std::cell::RefCell<Option<Entity<UsagePage>>> = const { std::cell::RefCell::new(None) };
}

/// Usage as the shell mounts it, keeping its page where the test can read it.
struct UsageProbe {
    screen: Entity<usage_screen::UsageScreen>,
}

impl Render for UsageProbe {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.screen.clone()
    }
}

impl Screen for UsageProbe {
    const NAME: &'static str = "usage-probe";
    const PLACE: Place = Place::Main;

    fn new(host: Host, opened: Opened, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let screen = cx.new(|cx| usage_screen::UsageScreen::new(host, opened, window, cx));
        MOUNTED.with(|m| *m.borrow_mut() = Some(screen.read(cx).page().clone()));
        UsageProbe { screen }
    }

    fn parts(this: &Entity<Self>, window: &mut Window, cx: &mut gpui_kit::App) -> Parts {
        let screen = this.read(cx).screen.clone();
        usage_screen::UsageScreen::parts(&screen, window, cx)
    }
}

#[gpui_kit::test]
fn tab_walks_the_pages_controls_in_the_shell(cx: &mut TestAppContext) {
    screens::add(UsageProbe::NAME, entry::<UsageProbe>());
    let host = seeded();
    cx.executor().allow_parking();
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
        desktop_gpui::screens::app_shell::menu::init(cx);
    });
    let runtime = Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap());
    let mut shell = None;
    let (state, env) = (host.state(), host.env());
    let (_, cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| Shell::new(state, env, runtime, Launch::default(), window, cx));
        shell = Some(view.clone());
        Root::new(view, window, cx)
    });
    let shell = shell.unwrap();
    let limit = Instant::now() + Duration::from_secs(30);
    while !shell.read_with(cx, |s, _| s.connected()) {
        assert!(Instant::now() < limit, "the shell never reached the host");
        cx.run_until_parked();
        std::thread::sleep(Duration::from_millis(20));
    }
    shell.update_in(cx, |s, window, cx| s.go(Route::Screen(UsageProbe::NAME.into()), window, cx));
    let page = MOUNTED.with(|m| m.borrow().clone()).expect("the shell mounted the page");
    until(cx, &page, "the first read", |p| landed(p, Range::Week, Split::Agent));

    // Tab from wherever the shell left the focus reaches the refresh, and the page's controls follow it in order.
    let on_refresh = |cx: &mut VisualTestContext| cx.update(|window, cx| page.read(cx).refresh.is_focused(window));
    let mut tabs = 0;
    while !on_refresh(cx) {
        assert!(tabs < 40, "Tab never reached the refresh");
        cx.simulate_keystrokes("tab");
        tabs += 1;
    }
    cx.simulate_keystrokes("tab right");
    assert_eq!(page.read_with(cx, |p, _| p.tab), Tab::Limits, "the tabs are the stop after the refresh");
    cx.simulate_keystrokes("left");
    cx.simulate_keystrokes("tab right");
    assert_eq!(page.read_with(cx, |p, _| p.range), Range::Month, "then the range");
    cx.simulate_keystrokes("tab left");
    assert_eq!(page.read_with(cx, |p, _| p.split), Split::Source, "then the split");
    cx.simulate_keystrokes("shift-tab left");
    assert_eq!(page.read_with(cx, |p, _| p.range), Range::Week, "and Shift-Tab walks back to the range");
    cx.simulate_keystrokes("shift-tab shift-tab");
    assert!(on_refresh(cx), "and on back to the refresh");
}
