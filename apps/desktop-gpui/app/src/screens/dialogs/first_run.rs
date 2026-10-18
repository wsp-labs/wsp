// SPDX-License-Identifier: AGPL-3.0-only
//! First run (`shell/FirstRun.tsx`): the whole centre while this wsp holds no project and no workspace
//! (`useFirstRun`), the one thing to do next and the key that does it. It reads the host's projects and workspaces
//! again whenever one is added or goes, and stands aside for the view behind it once the host holds either.

use std::net::{TcpStream, ToSocketAddrs};
use std::sync::Arc;
use std::time::Duration;

use futures::StreamExt as _;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::{div, prelude::*, px, svg, AnyView, Context, FontWeight, Task, TextAlign, Window};
use serde_json::{json, Value};
use wsp_host_client::protocol::{PlacesListRequest, WorkspacesListRequest};
use wsp_host_client::{Event, Failure, HostClient};

use crate::screens::{navigate, Host, Route};
use crate::ui::{text, ActivePalette, Button, Glyph, MARK};

pub const TITLE: &str = "Add a project to get started";
pub const ADD: &str = "Add a project";
pub const USAGE: &str = "wsp sends anonymous usage counts to PostHog.";
pub const PRIVACY: &str = "Privacy settings";

/// `FIRST_RUN_WORDS.sentence`: nothing until the computer's name is known.
pub fn sentence(here: &str) -> String {
    if here.is_empty() {
        String::new()
    } else {
        format!("A project is a folder on {here}. Its threads work in that folder.")
    }
}

/// The most of the host's page read for its boot line, which stands in the page's head.
const PAGE_CAP: usize = 512 * 1024;
const PAGE_WAIT: Duration = Duration::from_secs(5);

/// `productUsageOff` in the boot line of the page the host serves (`window.__WSP__ = {...};`, server.ts `loadPage`):
/// why the host sends no usage counts, none where its switch decides; nothing where the page holds no boot line.
pub fn usage_off_in(page: &str) -> Option<Option<String>> {
    let at = page.find("window.__WSP__ = ")? + "window.__WSP__ = ".len();
    let end = at + page[at..].find(";</script>")?;
    let boot: Value = serde_json::from_str(&page[at..end]).ok()?;
    Some(boot.get("productUsageOff").and_then(Value::as_str).map(str::to_owned))
}

/// Reads the boot line off the host's page at `page` (`http://127.0.0.1:<port>/`), one bounded GET.
pub fn usage_off(page: &str) -> Result<Option<String>, String> {
    use std::io::{Read, Write};
    let authority = page.trim_start_matches("http://").trim_end_matches('/');
    let address = authority.to_socket_addrs().map_err(|e| e.to_string())?.next().ok_or("no address")?;
    let mut socket = TcpStream::connect_timeout(&address, PAGE_WAIT).map_err(|e| e.to_string())?;
    socket.set_read_timeout(Some(PAGE_WAIT)).map_err(|e| e.to_string())?;
    let ask = format!("GET / HTTP/1.1\r\nHost: {authority}\r\nConnection: close\r\n\r\n");
    socket.write_all(ask.as_bytes()).map_err(|e| e.to_string())?;
    let mut read = Vec::new();
    let mut chunk = [0u8; 16 * 1024];
    while read.len() < PAGE_CAP {
        let n = socket.read(&mut chunk).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        read.extend_from_slice(&chunk[..n]);
        if let Some(said) = usage_off_in(&String::from_utf8_lossy(&read)) {
            return Ok(said);
        }
    }
    Err("the host's page holds no boot line".into())
}

/// What the host holds, as first run reads it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Holds {
    pub projects: usize,
    pub workspaces: usize,
    /// `hereName`: this computer's row's label, else its name.
    pub here: String,
}

impl Holds {
    pub fn nothing(&self) -> bool {
        self.projects == 0 && self.workspaces == 0
    }
}

/// The projects, workspaces and places lists, once.
pub async fn read(client: &HostClient) -> Result<Holds, Failure> {
    let projects = client.projects().await?.len();
    let list: WorkspacesListRequest = serde_json::from_value(json!({})).map_err(|e| Failure::new(e.to_string()))?;
    let workspaces = client.call(list).await?.workspaces.len();
    let places: PlacesListRequest = serde_json::from_value(json!({})).map_err(|e| Failure::new(e.to_string()))?;
    let here = client
        .call(places)
        .await
        .ok()
        .and_then(|p| {
            let here = p.places.iter().map(|p| serde_json::to_value(p).unwrap_or_default()).find(|p| p["id"] == "here")?;
            let name = here.get("label").and_then(Value::as_str).or_else(|| here.get("name").and_then(Value::as_str))?;
            Some(name.to_owned())
        })
        .unwrap_or_default();
    Ok(Holds { projects, workspaces, here })
}

/// Reads once, then again on each frame that adds or takes a project or a workspace, until the receiver goes.
async fn follow(client: Arc<HostClient>, tx: futures::channel::mpsc::UnboundedSender<Holds>) {
    let Ok(mut events) = client.events().await else { return };
    if let Ok(holds) = read(&client).await {
        if tx.unbounded_send(holds).is_err() {
            return;
        }
    }
    while let Some(event) = events.next().await {
        let Ok(Event::Host(event)) = event else { continue };
        let kind = serde_json::to_value(&event).ok().and_then(|v| v.get("type").and_then(Value::as_str).map(str::to_owned));
        if !matches!(kind.as_deref(), Some("project.added" | "project.removed" | "workspace.created" | "workspace.deleted")) {
            continue;
        }
        if let Ok(holds) = read(&client).await {
            if tx.unbounded_send(holds).is_err() {
                return;
            }
        }
    }
}

pub struct FirstRun {
    host: Option<Host>,
    /// None until the host has answered: first run says nothing until then.
    holds: Option<Holds>,
    /// What the host's page said of its usage counts: why they are off, none where its switch decides, or why the page
    /// could not be read; None until it answers.
    usage_off: Option<Result<Option<String>, String>>,
    behind: AnyView,
    _follow: Option<Task<()>>,
    _usage: Option<Task<()>>,
}

impl FirstRun {
    /// First run on the host, over `behind`, which shows whenever the host holds a project or a workspace.
    /// The usage line shows once the page the host serves at `page` says its switch decides.
    pub fn new(host: Host, page: Option<String>, behind: AnyView, cx: &mut Context<Self>) -> Self {
        let (tx, mut rx) = futures::channel::mpsc::unbounded();
        let task = host.run(move |client| async move {
            follow(client, tx).await;
            Ok(())
        });
        let off = cx.background_spawn(async move { page.ok_or_else(|| "no host page".to_owned()).and_then(|p| usage_off(&p)) });
        let usage = cx.spawn(async move |this, cx| {
            let said = off.await;
            _ = this.update(cx, |this, cx| {
                this.usage_off = Some(said);
                cx.notify();
            });
        });
        let follow = cx.spawn(async move |this, cx| {
            let _task = task;
            while let Some(holds) = rx.next().await {
                if this
                    .update(cx, |this, cx| {
                        this.holds = Some(holds);
                        cx.notify();
                    })
                    .is_err()
                {
                    return;
                }
            }
        });
        FirstRun { host: Some(host), holds: None, usage_off: None, behind, _follow: Some(follow), _usage: Some(usage) }
    }

    /// First run as a host holding `holds` would draw it, with no host behind it.
    pub fn staged(holds: Holds, usage: bool, behind: AnyView) -> Self {
        let usage_off = Some(Ok((!usage).then(|| "build".to_owned())));
        FirstRun { host: None, holds: Some(holds), usage_off, behind, _follow: None, _usage: None }
    }

    /// What the host's page said of its usage counts, once it answered or failed.
    pub fn usage_off(&self) -> Option<&Result<Option<String>, String>> {
        self.usage_off.as_ref()
    }

    /// What the host holds, once it has answered.
    pub fn holds(&self) -> Option<&Holds> {
        self.holds.as_ref()
    }

    /// Whether the usage line shows, which it does once the host's page says no reason keeps its counts off.
    pub fn usage(&self) -> bool {
        matches!(self.usage_off, Some(Ok(None)))
    }

    pub fn showing(&self) -> bool {
        self.holds.as_ref().is_some_and(Holds::nothing)
    }
}

impl Render for FirstRun {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(holds) = self.holds.as_ref().filter(|h| h.nothing()) else {
            return div().size_full().child(self.behind.clone());
        };
        let p = cx.palette().clone();
        let host = self.host.clone();
        let add = Button::primary("add-project", ADD).glyph(Glyph::Plus).on_click(move |_, window, cx| {
            if let Some(host) = &host {
                crate::screens::dialogs::open_add_project(host, window, cx);
            }
        });
        let usage = self.usage().then(|| {
            h_flex()
                .debug_selector(|| "product-usage".into())
                .flex_wrap()
                .justify_center()
                .gap_x(px(4.))
                .mt(px(32.))
                .max_w(px(320.))
                .text_align(TextAlign::Center)
                .text_size(text::NOTE.0)
                .line_height(text::NOTE.1)
                .text_color(p.muted_foreground)
                .child(USAGE)
                .child(
                    div()
                        .id("privacy")
                        .debug_selector(|| "privacy".into())
                        .cursor_pointer()
                        .text_color(p.foreground.opacity(0.8))
                        .hover(|s| s.text_color(p.foreground))
                        .on_click(|_, _, cx| navigate(Route::Screen("settings:privacy".into()), cx))
                        .child(PRIVACY),
                )
        });
        div().size_full().child(
            v_flex()
                .id("first-run")
                .debug_selector(|| "first-run".into())
                .size_full()
                .items_center()
                .justify_center()
                .overflow_y_scroll()
                .px(px(24.))
                .py(px(48.))
                .child(svg().path(MARK).flex_none().size(px(48.)).text_color(p.muted_foreground.opacity(0.4)))
                .child(
                    div()
                        .debug_selector(|| "first-run-title".into())
                        .mt(px(24.))
                        .text_size(px(18.))
                        .line_height(px(28.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(p.foreground)
                        .child(TITLE),
                )
                .child(
                    div()
                        .debug_selector(|| "first-run-sentence".into())
                        .mt(px(6.))
                        .max_w(px(320.))
                        .text_align(TextAlign::Center)
                        .text_size(px(14.))
                        .line_height(px(20.))
                        .text_color(p.muted_foreground)
                        .child(sentence(&holds.here)),
                )
                .child(div().mt(px(24.)).child(add))
                .children(usage),
        )
    }
}
