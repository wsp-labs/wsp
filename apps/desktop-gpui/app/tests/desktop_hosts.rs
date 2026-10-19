// SPDX-License-Identifier: AGPL-3.0-only
//! The Hosts menu's move, through the roads the app takes: two hosts started for this test alone, one serving the
//! state file here and one saved under an alias as a host on the account, the window's shell and the feed moved from
//! one to the other by a pick, and a pick the dial refuses.

#[path = "../../host-client/tests/common/mod.rs"]
mod common;

use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use common::Throwaway;
use desktop_gpui::desktop::{app_procs, feed, hosts, links, log};
use desktop_gpui::screens::settings::model::req;
use desktop_gpui::screens::Route;
use desktop_gpui::shell::{Launch, Shell};
use gpui_kit::component::Root;
use gpui_kit::{px, size, App, AppContext, Entity, TestAppContext, VisualTestContext};
use serde_json::json;
use wsp_host_client::ops::activity::AppProcs;
use wsp_host_client::protocol::AppProcsWatchRequest;
use wsp_host_client::{lock, Event, HostClient};

const WAIT: Duration = Duration::from_secs(30);

/// Saves `account` in `here`'s wsp home as the host on the account under `alias`, holding its own token, as a record
/// `wsp hosts` wrote and a first dial filled; its address.
fn save(here: &Throwaway, alias: &str, account: &Throwaway) -> String {
    let port = lock::serving(&account.state()).expect("the account's host serves").port;
    let url = format!("http://127.0.0.1:{port}");
    let token = std::fs::read_to_string(account.state().with_file_name("host-token")).unwrap();
    let record = json!({ "url": url, "deviceId": "d-test", "deviceToken": token.trim(), "via": { "kind": "account", "hostId": format!("h-{alias}") } });
    let folder = std::path::Path::new(&here.env()["HOME"]).join(".wsp/hosts");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(folder.join(format!("{alias}.json")), record.to_string()).unwrap();
    url
}

fn until(vcx: &mut VisualTestContext, what: &str, done: impl Fn(&mut VisualTestContext) -> bool) {
    let at = Instant::now();
    while !done(vcx) {
        assert!(at.elapsed() < WAIT, "no {what} within {WAIT:?}");
        vcx.run_until_parked();
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// The shell the window holds now, which a move builds again.
fn shell_of(vcx: &mut VisualTestContext) -> Entity<Shell> {
    let root = vcx.update(|window, _| window.root::<Root>()).flatten().expect("the window's root");
    root.read_with(vcx, |root, _| root.view().clone().downcast::<Shell>().unwrap())
}

fn projects(shell: &Entity<Shell>, vcx: &mut VisualTestContext) -> Vec<String> {
    shell.read_with(vcx, |s, cx| s.sidebar().read(cx).items().iter().map(|i| i.thread_id.clone()).collect())
}

/// The threads a host lists, read over a dial of its own.
fn listed(runtime: &tokio::runtime::Runtime, here: &Throwaway, alias: Option<&str>) -> Vec<String> {
    runtime.block_on(async {
        let client = HostClient::connect_on(&here.state(), &here.env(), alias, None).await.unwrap();
        client.threads().await.unwrap().into_iter().map(|t| t.thread_id.unwrap_or(t.id)).collect()
    })
}

/// Catches: an account host missing from the list, or this computer's own record on the account listed beside the row
/// that already is it.
#[test]
fn the_list_is_every_account_host_less_this_computers_own() {
    let (here, account) = (Throwaway::start(), Throwaway::start());
    let url = save(&here, "studio", &account);
    assert_eq!(wsp_host_client::account_hosts(&here.state(), &here.env()), [("studio".to_owned(), url)]);
    assert_eq!(wsp_host_client::default_alias(&here.state(), &here.env()), None, "a host serves the state file here");
    let relay = json!({ "relayUrl": "wss://relay.example", "hostId": "h-studio", "token": "t" });
    std::fs::write(here.state().with_file_name("relay.json"), relay.to_string()).unwrap();
    assert!(wsp_host_client::account_hosts(&here.state(), &here.env()).is_empty(), "the record naming this computer's relay id is here");
}

/// Catches: a pick that leaves the window's shell or the feed on the host it was on, a check that does not follow the
/// pick, the tray not titled by the alias, a refused pick that moves anything or says nothing in the log, a move home
/// that stays away, a pick's answer that lands after a later pick, and a link that opens on a host on the account
/// rather than moving home, with the window standing or reopened after it closed.
#[gpui_kit::test]
fn a_pick_moves_the_window_and_the_feed_and_a_refusal_moves_nothing(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let (here, account) = (Throwaway::start(), Throwaway::start());
    for (host, name) in [(&here, "here-repo"), (&account, "studio-repo")] {
        let folder = host.repo(name);
        host.wsp(&["add", &folder.to_string_lossy()]);
        host.wsp(&["run", name, "hello", "--agent", "claude"]);
    }
    save(&here, "studio", &account);
    let runtime = Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap());
    let (on_here, on_studio) = (listed(&runtime, &here, None), listed(&runtime, &here, Some("studio")));
    assert!(!on_here.is_empty() && on_here != on_studio, "the two hosts list different threads: {on_here:?} {on_studio:?}");
    let (state, env) = (here.state(), here.env());
    log::init(&std::path::Path::new(&env["HOME"]).join(".wsp/logs"), &env);
    cx.update(|cx| {
        gpui_kit::init(cx);
        desktop_gpui::ui::init(desktop_gpui::ui::Side::Light, cx);
        desktop_gpui::screens::palette::init(cx);
        hosts::init(state.clone(), env.clone(), runtime.clone(), cx);
        feed::start(state.clone(), env.clone(), runtime.clone(), Rc::new(|_: &mut App| {}), cx);
        assert_eq!(hosts::view(cx), None, "no Hosts menu before an app window stands");
    });
    let (_, vcx) = cx.add_window_view(|w, cx| {
        let view = cx.new(|cx| Shell::new(state, env, runtime.clone(), Launch::default(), w, cx));
        links::attach(w, &view, cx);
        Root::new(view, w, cx)
    });
    vcx.simulate_resize(size(px(1280.), px(800.)));
    let shell = shell_of(vcx);
    until(vcx, "the host here's threads", |vcx| projects(&shell, vcx) == on_here);
    let view = vcx.update(|_, cx| hosts::view(cx)).expect("the menu shows once a window stands");
    assert_eq!((view.current, view.hosts), (None, vec!["studio".to_owned()]));
    until(vcx, "the feed on the host here", |vcx| vcx.update(|_, cx| feed::fed(cx)).is_some_and(|f| !f.lost && !f.remote));

    vcx.update(|_, cx| hosts::to(Some("studio".to_owned()), cx));
    until(vcx, "the window on studio", |vcx| shell_of(vcx) != shell && projects(&shell_of(vcx), vcx) == on_studio);
    assert_eq!(vcx.update(|_, cx| hosts::picked(cx)), Some("studio".to_owned()));
    assert_eq!(vcx.update(|_, cx| hosts::view(cx)).unwrap().current.as_deref(), Some("studio"), "the check follows the pick");
    until(vcx, "the feed on studio", |vcx| vcx.update(|_, cx| feed::fed(cx)).is_some_and(|f| !f.lost && f.remote));
    let fed = vcx.update(|_, cx| feed::fed(cx)).unwrap();
    assert!(fed.tray().title.starts_with("studio"), "{:?}", fed.tray().title);

    let moved = shell_of(vcx);
    vcx.update(|_, cx| hosts::to(Some("nowhere".to_owned()), cx));
    let app_log = std::path::Path::new(&here.env()["HOME"]).join(".wsp/logs/app.log");
    let logged = |vcx: &mut VisualTestContext| {
        vcx.run_until_parked();
        std::fs::read_to_string(&app_log).unwrap_or_default().lines().any(|l| l.contains(" error ") && l.contains("nowhere"))
    };
    until(vcx, "the refusal in the log", logged);
    assert_eq!(shell_of(vcx), moved, "a refused pick builds nothing again");
    assert_eq!(vcx.update(|_, cx| hosts::picked(cx)), Some("studio".to_owned()), "and the check stays");

    vcx.update(|_, cx| hosts::to(None, cx));
    until(vcx, "the window back here", |vcx| projects(&shell_of(vcx), vcx) == on_here);
    assert_eq!(vcx.update(|_, cx| hosts::view(cx)).unwrap().current, None);
    until(vcx, "the feed back here", |vcx| vcx.update(|_, cx| feed::fed(cx)).is_some_and(|f| !f.lost && !f.remote));

    vcx.update(|_, cx| hosts::to(Some("studio".to_owned()), cx));
    until(vcx, "the window on studio again", |vcx| projects(&shell_of(vcx), vcx) == on_studio);
    let thread = on_here[0].clone();
    vcx.update(|_, cx| links::open(&format!("wsp://thread/{thread}"), cx));
    let opened = |vcx: &mut VisualTestContext| matches!(shell_of(vcx).read_with(vcx, |s, _| s.route().clone()), Route::Thread { thread_id, .. } if thread_id == thread);
    until(vcx, "a link's thread opened back here", opened);
    assert_eq!(vcx.update(|_, cx| hosts::picked(cx)), None, "a link moves the window home first");

    let home = shell_of(vcx);
    vcx.update(|_, cx| {
        hosts::to(Some("studio".to_owned()), cx);
        hosts::to(None, cx);
    });
    let at = Instant::now();
    while at.elapsed() < Duration::from_secs(3) {
        vcx.run_until_parked();
        std::thread::sleep(Duration::from_millis(50));
    }
    assert_eq!(vcx.update(|_, cx| hosts::picked(cx)), None, "the last pick stands");
    until(vcx, "the window here after both picks", |vcx| shell_of(vcx) != home && projects(&shell_of(vcx), vcx) == on_here);

    vcx.update(|_, cx| hosts::to(Some("studio".to_owned()), cx));
    until(vcx, "the window on studio before it closes", |vcx| projects(&shell_of(vcx), vcx) == on_studio);
    vcx.update(|window, _| window.remove_window());
    vcx.run_until_parked();
    cx.update(|cx| links::open(&format!("wsp://thread/{thread}"), cx));
    let (state, env) = (here.state(), here.env());
    let (_, vcx) = cx.add_window_view(|w, cx| {
        let view = cx.new(|cx| Shell::new(state, env, runtime.clone(), Launch::default(), w, cx));
        links::attach(w, &view, cx);
        Root::new(view, w, cx)
    });
    vcx.simulate_resize(size(px(1280.), px(800.)));
    until(vcx, "a link held while no window stood opened back here", opened);
    assert_eq!(vcx.update(|_, cx| hosts::picked(cx)), None);
}

/// Catches: a check recomputed from where a dial would land now rather than kept from the host the window dialled, so
/// a host coming up here moves the check while the window stays on the account's host.
#[gpui_kit::test]
fn the_check_is_the_host_the_window_dialled(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let (mut here, account) = (Throwaway::start(), Throwaway::start());
    let folder = account.repo("studio-repo");
    account.wsp(&["add", &folder.to_string_lossy()]);
    account.wsp(&["run", "studio-repo", "hello", "--agent", "claude"]);
    save(&here, "studio", &account);
    here.stop();
    let runtime = Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap());
    let on_studio = listed(&runtime, &here, Some("studio"));
    let (state, env) = (here.state(), here.env());
    cx.update(|cx| {
        gpui_kit::init(cx);
        desktop_gpui::ui::init(desktop_gpui::ui::Side::Light, cx);
        desktop_gpui::screens::palette::init(cx);
        hosts::init(state.clone(), env.clone(), runtime.clone(), cx);
    });
    let (_, vcx) = cx.add_window_view(|w, cx| {
        let view = cx.new(|cx| Shell::new(state, env, runtime.clone(), Launch::default(), w, cx));
        Root::new(view, w, cx)
    });
    vcx.simulate_resize(size(px(1280.), px(800.)));
    until(vcx, "the window on the account's one host", |vcx| projects(&shell_of(vcx), vcx) == on_studio);
    assert_eq!(vcx.update(|_, cx| hosts::view(cx)).unwrap().current.as_deref(), Some("studio"));
    here.restart();
    assert_eq!(vcx.update(|_, cx| hosts::view(cx)).unwrap().current.as_deref(), Some("studio"), "a host here coming up moves no check");
    assert_eq!(vcx.update(|_, cx| hosts::picked(cx)).as_deref(), Some("studio"));
}

/// Whether a page watching `host`'s app processes hears this process named within `wait`.
fn named_to(runtime: &tokio::runtime::Runtime, host: &Throwaway, alias: Option<&str>, wait: Duration) -> bool {
    runtime.block_on(async {
        let page = HostClient::connect_on(&host.state(), &host.env(), alias, None).await.unwrap();
        let mut events = page.events().await.unwrap();
        let names = |procs: &wsp_host_client::protocol::AppProcList| {
            let rows = serde_json::to_value(procs).unwrap();
            rows.as_array().unwrap().iter().any(|p| p["pid"] == json!(std::process::id()) && p["kind"] == "main")
        };
        let AppProcs { procs } = page.call(req::<AppProcsWatchRequest>(json!({}))).await.unwrap();
        names(&procs)
            || tokio::time::timeout(wait, async {
                while let Some(next) = events.next().await {
                    if let Ok(Event::AppProcs(e)) = next {
                        if names(&e.procs) {
                            return true;
                        }
                    }
                }
                false
            })
            .await
            .unwrap_or(false)
    })
}

/// Catches: the app's processes offered to a host on the account, and the host here then offered nothing because the
/// account's took the one offer.
#[gpui_kit::test]
fn the_apps_processes_go_to_the_host_here_alone(cx: &mut TestAppContext) {
    let (here, account) = (Throwaway::start(), Throwaway::start());
    save(&here, "studio", &account);
    let runtime = Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap());
    let dial = |alias: Option<&str>| Arc::new(runtime.block_on(HostClient::connect_on(&here.state(), &here.env(), alias, None)).unwrap());
    let (studio, home) = (dial(Some("studio")), dial(None));
    cx.update(|cx| app_procs::offer_once(studio, &runtime, cx));
    assert!(!named_to(&runtime, &here, Some("studio"), Duration::from_secs(4)), "a host on the account was offered the app's processes");
    cx.update(|cx| app_procs::offer_once(home, &runtime, cx));
    assert!(named_to(&runtime, &here, None, Duration::from_secs(10)), "the host here was never offered them");
}
