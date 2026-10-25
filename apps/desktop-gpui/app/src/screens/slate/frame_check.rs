// SPDX-License-Identifier: AGPL-3.0-only
//! `desktop-gpui --screen frame-check`: a slate frame's sealed web view on the page that tries every forbidden thing
//! (`apps/web/test/slate-render/pages/hostile.html`, which the web app's frame test loads too), aimed at a listener this
//! process holds on loopback: TCP for its fetches, posts, sockets and pages, UDP for its STUN. The page can tell the app
//! nothing but its height, so it says its outcome that way; the checks read that and the listener, one line each.
//! `--screen frame-check-without-trusted-types` runs WebKit with Trusted Types off, as an engine older than Safari 26:
//! the frame must draw no page and say why, and a page sealed there anyway must run none of its script, a pointer
//! moved across its frame included.

use std::io::Read as _;
use std::net::{TcpListener, UdpSocket};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui_kit::component::Root;
use gpui_kit::{div, prelude::*, px, size, App, Bounds, Context, Entity, Subscription, Window, WindowBounds, WindowOptions};

use super::pieces::frame::{height_of, refused_of, shell, theme, SEALABLE_CHECK};
use crate::screens::browser::check::log;
use crate::screens::browser::page::{Page, PageEvent};
use crate::ui::ActivePalette;

pub const NAME: &str = "frame-check";
pub const NAME_WITHOUT: &str = "frame-check-without-trusted-types";

const HOSTILE: &str = include_str!("../../../../../web/test/slate-render/pages/hostile.html");
/// What the hostile page's height is once it has tried everything, 10 more for each attempt that reached anything; it
/// says a far larger one on the way, as a slate action would.
const DONE_HEIGHT: f32 = 4000.;
const DONE_MOST: f32 = 4999.;
/// What the hostile page's first script and its handler on <html> say, as a height, once either runs.
const RAN_HEIGHT: f32 = 3210.;

/// Everything that reached the listener, counted from its own threads.
#[derive(Default)]
struct Reached {
    tcp: AtomicU32,
    udp: AtomicU32,
}

fn listen(reached: &Arc<Reached>) -> std::io::Result<(String, String)> {
    let tcp = TcpListener::bind("127.0.0.1:0")?;
    let udp = UdpSocket::bind("127.0.0.1:0")?;
    let (at, stun) = (tcp.local_addr()?.to_string(), udp.local_addr()?.to_string());
    let heard = reached.clone();
    std::thread::spawn(move || {
        for mut stream in tcp.incoming().flatten() {
            heard.tcp.fetch_add(1, Ordering::SeqCst);
            let mut first = [0u8; 256];
            let n = stream.read(&mut first).unwrap_or(0);
            log(format!("listener: tcp {}", String::from_utf8_lossy(&first[..n]).lines().next().unwrap_or("")));
        }
    });
    let heard = reached.clone();
    std::thread::spawn(move || {
        let mut packet = [0u8; 512];
        while udp.recv_from(&mut packet).is_ok() {
            heard.udp.fetch_add(1, Ordering::SeqCst);
            log("listener: udp");
        }
    });
    Ok((at, stun))
}

/// The window: the sealed pages side by side, and every title each page's shell set.
struct FrameCheck {
    pages: Vec<Entity<Page>>,
    titles: Vec<Vec<String>>,
    _heard: Vec<Subscription>,
}

impl Render for FrameCheck {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let ground = cx.palette().background;
        div()
            .size_full()
            .flex()
            .bg(ground)
            .children(self.pages.iter().map(|page| div().flex().flex_col().flex_1().min_w_0().child(Page::slot_on(page, ground, cx))))
    }
}

/// `trusted_types` false: the engine without them, the shell as it ships and the shell with its check taken out.
pub fn open(trusted_types: bool, cx: &mut App) {
    let reached = Arc::new(Reached::default());
    let (at, stun) = listen(&reached).unwrap_or_else(|e| {
        eprintln!("desktop-gpui: frame check: no listener: {e}");
        std::process::exit(1);
    });
    log(format!("listener: tcp {at}, udp {stun}"));
    let page = HOSTILE.replace("127.0.0.1:4400", &at).replace("stun:127.0.0.1:3478", &format!("stun:{stun}"));
    let bounds = Bounds::centered(None, size(px(760.), px(900.)), cx);
    let options = WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), ..WindowOptions::default() };
    let opened = cx.open_window(options, move |window, cx| {
        let (theme, ground) = (theme(cx), cx.palette().background);
        let shipped = shell(&page, &theme);
        let shells = if trusted_types { vec![shipped] } else { vec![shipped.replace(SEALABLE_CHECK, ""), shipped] };
        let made: Vec<Entity<Page>> =
            shells.iter().map(|s| Page::sealed(s, ground.to_rgb(), trusted_types, window, cx)).collect::<Result<_, _>>().unwrap_or_else(
                |why| {
                    eprintln!("desktop-gpui: frame check: {why}");
                    std::process::exit(1);
                },
            );
        let check = cx.new(|cx| {
            let heard = made
                .iter()
                .enumerate()
                .map(|(i, page)| {
                    cx.subscribe(page, move |c: &mut FrameCheck, _, event: &PageEvent, cx| {
                        if let PageEvent::Titled(t) = event {
                            log(format!("title {i}: {t}"));
                            c.titles[i].push(t.clone());
                            cx.notify();
                        }
                    })
                })
                .collect();
            FrameCheck { titles: vec![Vec::new(); made.len()], pages: made, _heard: heard }
        });
        let polled = check.clone();
        if !trusted_types {
            window
                .spawn(cx, async move |cx| {
                    cx.background_executor().timer(Duration::from_secs(4)).await;
                    // The page's own handler on its <html> tag is parsed before any script of the seal's runs: the
                    // pointer crossing the frame is what would run it.
                    #[cfg(target_os = "linux")]
                    cx.update(|_, cx| {
                        for page in &polled.read(cx).pages {
                            if let Some(native) = page.read(cx).native() {
                                native.sweep_pointer();
                            }
                        }
                    })
                    .ok();
                    cx.background_executor().timer(Duration::from_secs(4)).await;
                    let titles = cx.update(|_, cx| polled.read(cx).titles.clone()).unwrap_or_default();
                    let mut failed = 0;
                    let mut check = |name: &str, ok: bool, detail: String| {
                        failed += u32::from(!ok);
                        log(format!("check {name}: {} ({detail})", if ok { "yes" } else { "NO" }));
                    };
                    let said: Vec<&str> = titles[1].iter().filter_map(|t| refused_of(t)).collect();
                    check("the frame drew no page and said why", said.len() == 1, format!("{said:?}"));
                    let heights: Vec<f32> = titles[0].iter().filter_map(|t| height_of(t)).collect();
                    check(
                        "a page sealed anyway ran no script",
                        heights.is_empty(),
                        format!("heights {heights:?}, {RAN_HEIGHT} once it runs"),
                    );
                    let (tcp, udp) = (reached.tcp.load(Ordering::SeqCst), reached.udp.load(Ordering::SeqCst));
                    check("no request left either frame", tcp == 0 && udp == 0, format!("{tcp} tcp, {udp} udp at the listener"));
                    std::process::exit(if failed == 0 { 0 } else { 1 });
                })
                .detach();
            return cx.new(|cx| Root::new(check, window, cx));
        }
        window
            .spawn(cx, async move |cx| {
                let began = Instant::now();
                let done = loop {
                    cx.background_executor().timer(Duration::from_millis(200)).await;
                    let last = cx.update(|_, cx| polled.read(cx).titles[0].iter().rev().find_map(|t| height_of(t))).ok().flatten();
                    if let Some(h) = last.filter(|h| (DONE_HEIGHT..=DONE_MOST).contains(h)) {
                        break Some(h);
                    }
                    if began.elapsed() > Duration::from_secs(40) {
                        break None;
                    }
                };
                // Anything still on its way gets as long again to land.
                cx.background_executor().timer(Duration::from_millis(1500)).await;
                let titles = cx.update(|_, cx| polled.read(cx).titles[0].clone()).unwrap_or_default();
                let mut failed = 0;
                let mut check = |name: &str, ok: bool, detail: String| {
                    failed += u32::from(!ok);
                    log(format!("check {name}: {} ({detail})", if ok { "yes" } else { "NO" }));
                };
                check("the page tried everything and said so by its height", done.is_some(), format!("{done:?}"));
                let through = done.map(|h| ((h - DONE_HEIGHT) / 10.).round() as i64);
                check("every forbidden attempt failed", through == Some(0), format!("{through:?} reached"));
                let (tcp, udp) = (reached.tcp.load(Ordering::SeqCst), reached.udp.load(Ordering::SeqCst));
                check("no request left the frame", tcp == 0 && udp == 0, format!("{tcp} tcp, {udp} udp at the listener"));
                let other: Vec<&String> = titles.iter().filter(|t| !t.is_empty() && height_of(t).is_none()).collect();
                check("the app heard a height and nothing else", other.is_empty(), format!("{} titles, others {other:?}", titles.len()));
                std::process::exit(if failed == 0 { 0 } else { 1 });
            })
            .detach();
        cx.new(|cx| Root::new(check, window, cx))
    });
    if let Err(e) = opened {
        eprintln!("desktop-gpui: frame check: the window did not open: {e}");
        std::process::exit(1);
    }
    cx.activate(true);
}
