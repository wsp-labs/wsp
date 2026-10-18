// SPDX-License-Identifier: AGPL-3.0-only
//! The pair screen and the failed start: the screen's own rules on a road of fixtures, the same screen spending codes
//! on a host started for the test alone, the shell drawing it for a host it holds no token for, and the failed start
//! read off a workspace and opening the thread that asked for it.

#[path = "../../host-client/tests/common/mod.rs"]
mod common;

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use common::Throwaway;
use desktop_gpui::screens::dialogs::failed_start::{Failed, FailedStart, Lead, ASKED, OPEN_LEAD};
use desktop_gpui::screens::dialogs::pair::{self, PairScreen, Road};
use desktop_gpui::screens::Route;
use desktop_gpui::shell::{Launch, Shell};
use desktop_gpui::sidebar::host::Change;
use desktop_gpui::ui::{self, Side};
use gpui_kit::component::Root;
use gpui_kit::{div, prelude::*, px, size, Context, Entity, Modifiers, Task, TestAppContext, VisualTestContext, Window};
use serde_json::json;
use wsp_host_client::protocol::PairIssueRequest;
use wsp_host_client::HostClient;

/// `PAIR_CODE_REFUSAL`, the one sentence a host gives every code it will not spend.
const NOT_HELD: &str = "that pairing code is not one this host is waiting for";

fn init(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
    });
}

fn runtime() -> Arc<tokio::runtime::Runtime> {
    Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap())
}

fn screen(cx: &mut TestAppContext, road: Road) -> (Entity<PairScreen>, &mut VisualTestContext) {
    init(cx);
    let mut made = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let screen = cx.new(|cx| PairScreen::new(road, window, cx));
        made = Some(screen.clone());
        Root::new(screen, window, cx)
    });
    cx.simulate_resize(size(px(1280.), px(800.)));
    cx.run_until_parked();
    (made.unwrap(), cx)
}

/// Runs the app's tasks until `done` holds, as the host answers from tokio's threads.
fn wait(cx: &mut VisualTestContext, what: &str, mut done: impl FnMut(&mut VisualTestContext) -> bool) {
    let until = Instant::now() + Duration::from_secs(30);
    loop {
        cx.run_until_parked();
        if done(cx) {
            return;
        }
        assert!(Instant::now() < until, "never: {what}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// A road that records what it was asked and answers as told.
#[derive(Default)]
struct Asked {
    codes: RefCell<Vec<String>>,
    asks: Cell<u32>,
    paired: Cell<u32>,
}

fn fixture(asked: &Rc<Asked>, redeemed: Result<(), &'static str>, held: bool) -> Road {
    let (a, b, c) = (asked.clone(), asked.clone(), asked.clone());
    Road {
        redeem: Rc::new(move |code, _| {
            a.codes.borrow_mut().push(code);
            Task::ready(redeemed.map_err(str::to_owned))
        }),
        ask: Rc::new(move |_| {
            b.asks.set(b.asks.get() + 1);
            Task::ready(held)
        }),
        paired: Rc::new(move |_, _| c.paired.set(c.paired.get() + 1)),
    }
}

fn click_pair(cx: &mut VisualTestContext) {
    let at = cx.debug_bounds("button").expect("Pair is drawn");
    cx.simulate_click(at.center(), Modifiers::none());
    cx.run_until_parked();
}

/// Catches: Pair taking a press with nothing typed; a paste that is no code reaching the host; the code sent with its
/// dash or in lower case; the refusal not drawn.
#[gpui_kit::test]
fn pair_waits_for_a_code_refuses_a_shape_and_spends_a_code_once(cx: &mut TestAppContext) {
    let asked = Rc::new(Asked::default());
    let (screen, cx) = screen(cx, fixture(&asked, Err("the host said no"), false));
    click_pair(cx);
    assert!(asked.codes.borrow().is_empty(), "Pair is held while the field is empty");

    screen.update_in(cx, |s, window, cx| s.set_code("000000", window, cx));
    click_pair(cx);
    assert!(asked.codes.borrow().is_empty(), "a paste that is no code is refused before any host is asked");
    assert_eq!(screen.read_with(cx, |s, _| s.error().map(str::to_owned)).as_deref(), Some(pair::SHAPE_REFUSAL));
    assert!(cx.debug_bounds("refusal").is_some_and(|r| r.size.height >= px(36.)), "the two lines stand for the refusal");

    screen.update_in(cx, |s, window, cx| s.set_code(" abcd-efgh ", window, cx));
    click_pair(cx);
    assert_eq!(*asked.codes.borrow(), ["ABCDEFGH"], "the code goes as the host takes it");
    assert_eq!(screen.read_with(cx, |s, _| s.error().map(str::to_owned)).as_deref(), Some("the host said no"));
    assert!(!screen.read_with(cx, |s, _| s.busy()), "a refusal hands Pair back");
    assert_eq!(asked.paired.get(), 0);
}

/// Catches: a spent code not opening the app, or opening it twice.
#[gpui_kit::test]
fn a_spent_code_opens_the_app(cx: &mut TestAppContext) {
    let asked = Rc::new(Asked::default());
    let (screen, cx) = screen(cx, fixture(&asked, Ok(()), false));
    screen.update_in(cx, |s, window, cx| s.set_code("ABCD-EFGH", window, cx));
    click_pair(cx);
    assert_eq!((asked.codes.borrow().len(), asked.paired.get()), (1, 1), "a spent code opens the app once");
}

/// Catches: the screen not asking again as the window comes forward; the app not opening once a token is held.
#[gpui_kit::test]
fn the_code_screen_asks_again_as_the_window_comes_forward(cx: &mut TestAppContext) {
    let asked = Rc::new(Asked::default());
    let (_, cx) = screen(cx, fixture(&asked, Err("no"), true));
    let before = asked.asks.get();
    cx.deactivate_window();
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    assert!(asked.asks.get() > before, "coming forward asks whether a token is held now");
    assert_eq!(asked.paired.get(), asked.asks.get() - before, "a token held opens the app");
}

/// A code `pair.issue` minted on the throwaway host, in the words `wsp host pair` prints it.
fn issued(host: &Throwaway, rt: &tokio::runtime::Runtime) -> String {
    rt.block_on(async {
        let client = HostClient::connect(&host.state(), &host.env()).await.unwrap();
        client.call(common::ask::<PairIssueRequest>(json!({}))).await
    })
    .unwrap_or_else(|f| panic!("pair.issue: {}", f.message))
    .code
}

/// Catches: a code the host is not holding answered with anything but the host's own refusal, drawn as it said it.
#[gpui_kit::test]
fn the_host_refuses_a_code_it_is_not_holding(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let host = Throwaway::start();
    let road = Road::host(host.state(), host.env(), runtime());
    let (screen, cx) = screen(cx, road);
    screen.update_in(cx, |s, window, cx| s.set_code("ZZZZ-ZZZZ", window, cx));
    click_pair(cx);
    wait(cx, "the host's answer", |cx| screen.read_with(cx, |s, _| !s.busy() && s.error().is_some()));
    let said = screen.read_with(cx, |s, _| s.error().unwrap_or_default().to_owned());
    assert!(said.starts_with(NOT_HELD), "{said}");
}

fn bindings(cx: &mut VisualTestContext) -> usize {
    cx.update(|_, cx| cx.key_bindings().borrow().bindings().count())
}

/// Catches: the shell not drawing the pair screen for a host it holds no token for; a code `pair.issue` minted not
/// pairing over a socket that presents no token; the app not opening on the token it bought; the shell built after
/// pairing binding its chords a second time; a shell built again in the same process (a dock reopen) not dialling with
/// the token the code bought.
#[gpui_kit::test]
fn a_host_with_no_token_here_pairs_by_its_code_and_opens(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    init(cx);
    let host = Throwaway::start();
    let rt = runtime();
    let code = issued(&host, &rt);
    std::fs::remove_file(host.state().with_file_name("host-token")).unwrap();
    let (state, vars, again) = (host.state(), host.env(), rt.clone());
    let (_, cx) = cx.add_window_view(|window, cx| {
        let shell = cx.new(|cx| Shell::new(state, vars, rt, Launch::default(), window, cx));
        Root::new(shell, window, cx)
    });
    cx.simulate_resize(size(px(1280.), px(800.)));
    wait(cx, "the pair screen", |cx| cx.debug_bounds("pair-screen").is_some());
    let bound = bindings(cx);
    cx.simulate_input(&code);
    click_pair(cx);
    wait(cx, "the app on the token the code bought", |cx| {
        cx.debug_bounds("pair-screen").is_none() && cx.debug_bounds("sidebar-column").is_some()
    });
    assert_eq!(bindings(cx), bound, "the shell built after pairing binds no chord twice");

    let (state, vars) = (host.state(), host.env());
    let shell = cx.update(|window, cx| {
        let shell = cx.new(|cx| Shell::new(state, vars, again, Launch::default(), window, cx));
        window.replace_root(cx, |window, cx| Root::new(shell.clone(), window, cx));
        shell
    });
    wait(cx, "a second shell connected on the bought token", |cx| shell.read_with(cx, |s, _| s.connected()));
    assert!(cx.debug_bounds("pair-screen").is_none(), "only a new launch asks for a code again");
}

/// Catches: a code spent on a host on the account not writing the device token it bought into that host's record, or
/// the record not being what the next connection dials with.
#[test]
fn a_code_spent_on_an_account_host_is_kept_in_its_record() {
    let host = Throwaway::start();
    let rt = runtime();
    let code = issued(&host, &rt);
    let lock: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(host.state().with_file_name("host.lock")).unwrap()).unwrap();
    let url = format!("http://127.0.0.1:{}", lock["port"]);
    // A state file no host serves, so the one host on the account is where the app aims.
    let elsewhere = host.dir.path().join("elsewhere/state.json");
    let record = host.dir.path().join(".wsp/hosts/box.json");
    std::fs::create_dir_all(record.parent().unwrap()).unwrap();
    let blank = json!({ "url": url, "deviceId": "", "deviceToken": "", "via": { "kind": "account", "hostId": "h1" } });
    std::fs::write(&record, blank.to_string()).unwrap();
    let vars = host.env();
    rt.block_on(HostClient::redeem(&elsewhere, &vars, &code, "a Linux computer in a browser at box"))
        .unwrap_or_else(|f| panic!("pair.redeem: {}", f.message));
    let kept: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&record).unwrap()).unwrap();
    assert!(kept["deviceToken"].as_str().is_some_and(|t| !t.is_empty()), "the record keeps the token: {kept}");
    let client = rt.block_on(HostClient::connect(&elsewhere, &vars)).unwrap_or_else(|f| panic!("{}", f.message));
    assert!(client.paired_token().is_none(), "an account host's token lives in its record");
    rt.block_on(client.projects()).unwrap_or_else(|f| panic!("projects.list on the record's token: {}", f.message));
}

struct Page {
    failed: Failed,
    opened: Rc<RefCell<Vec<Lead>>>,
}

impl Render for Page {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let opened = self.opened.clone();
        div().size_full().child(FailedStart::new(self.failed.clone(), move |lead, _, _| opened.borrow_mut().push(lead.clone())))
    }
}

/// Catches: the reason or the act missing from the failed row; Open that thread opening anything but the asking thread.
#[gpui_kit::test]
fn the_failed_start_opens_the_thread_that_asked(cx: &mut TestAppContext) {
    init(cx);
    let workspace = json!({ "id": "w9", "name": "fix-login", "phase": "gone", "machineId": "", "gone": "no room",
        "parentWorkspaceId": "w1", "parentThreadId": "t1" });
    let failed = Failed::of(&workspace).expect("a fork gone before its machine stood");
    assert_eq!(failed.title, "Could not start fix-login");
    let opened = Rc::new(RefCell::new(Vec::new()));
    let page = Page { failed, opened: opened.clone() };
    let (_, cx) = cx.add_window_view(|window, cx| Root::new(cx.new(|_| page), window, cx));
    cx.simulate_resize(size(px(1280.), px(800.)));
    cx.run_until_parked();
    assert!(cx.debug_bounds("step-refusal").is_some(), "the reason stands under {ASKED}");
    let at = cx.debug_bounds("button").unwrap_or_else(|| panic!("{OPEN_LEAD} is drawn"));
    cx.simulate_click(at.center(), Modifiers::none());
    assert_eq!(*opened.borrow(), [Lead { workspace_id: "w1".into(), thread_id: "t1".into() }]);
}

/// Catches: the main column not drawing the failed start for a workspace whose record says its start failed, or Open
/// that thread not taking the window to the thread that asked for the fork.
#[gpui_kit::test]
fn a_failed_start_routed_to_draws_the_card_and_opens_the_asking_thread(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    init(cx);
    let host = &common::shared().host;
    let (state, vars, rt) = (host.state(), host.env(), runtime());
    let mut made = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let shell = cx.new(|cx| Shell::new(state, vars, rt, Launch::default(), window, cx));
        made = Some(shell.clone());
        Root::new(shell, window, cx)
    });
    let shell = made.unwrap();
    cx.simulate_resize(size(px(1280.), px(800.)));
    wait(cx, "the sidebar's first read", |cx| shell.read_with(cx, |s, cx| !s.sidebar().read(cx).items().is_empty()));
    let lead = shell.read_with(cx, |s, cx| s.sidebar().read(cx).items()[0].clone());
    let fork = json!({ "id": "w9", "name": "fix-login", "phase": "gone", "machineId": "", "kind": "cloud", "gone": "no room",
        "parentWorkspaceId": lead.workspace_id, "parentThreadId": lead.thread_id });
    shell.update_in(cx, |s, window, cx| {
        s.sidebar().update(cx, |sidebar, cx| {
            let mut records: Vec<_> = sidebar.items().iter().filter_map(|i| sidebar.workspace(&i.workspace_id).cloned()).collect();
            records.push(fork);
            sidebar.apply(Change::Workspaces(records), cx);
        });
        s.go(Route::Thread { workspace_id: "w9".into(), thread_id: "t9".into() }, window, cx);
    });
    cx.run_until_parked();
    assert!(cx.debug_bounds("failed-start").is_some(), "the card stands in the main column");
    let at = cx.debug_bounds("failed-start-lead").expect("Open that thread is drawn");
    cx.simulate_click(at.center(), Modifiers::none());
    let route = shell.read_with(cx, |s, _| s.route().clone());
    assert_eq!(route, Route::Thread { workspace_id: lead.workspace_id, thread_id: lead.thread_id });
}

/// Catches: the host's fix lost off a refusal, or not the end of the sentence it was joined to.
#[test]
fn a_refusal_from_the_host_reads_its_sentence_and_its_fix_apart() {
    let host = Throwaway::start();
    let rt = runtime();
    let folder = host.repo("talk");
    host.wsp(&["add", &folder.to_string_lossy(), "--name", "talk"]);
    let client = rt.block_on(HostClient::connect(&host.state(), &host.env())).unwrap_or_else(|f| panic!("{}", f.message));
    let project = rt.block_on(client.projects()).unwrap().into_iter().find(|p| p.name == "talk").unwrap().id;
    let asked = common::ask::<wsp_host_client::protocol::ConversationsListRequest>(json!({ "agent": "opencode", "project": project }));
    let refused = rt.block_on(client.call(asked)).expect_err("wsp picks up no OpenCode conversation");
    let (said, fix) = ("wsp cannot pick up OpenCode conversations.", "Resume a Claude Code or Codex conversation.");
    assert_eq!(refused.message, format!("{said} {fix}"), "the message stays the whole line the host sent");
    assert_eq!((refused.said(), refused.fix.as_deref()), (said, Some(fix)));
}
