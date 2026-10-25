// SPDX-License-Identifier: AGPL-3.0-only
//! A frame: an HTML page the agent wrote, read by the host (`slates.page`) and drawn in a web view of its own under the
//! web app's seal (`record/frame-seal.js`, bundled from `apps/web/src/slate/pieces/frame-seal.ts`), two frames deep, so
//! it loads, sends and reaches nothing. The web view's own document is ours: it hands the page the theme, seals it and
//! passes on its height alone, as its title, the one road out of it; nothing the page does reaches the slate. A window
//! holds at most `LIVE_MOST` of them inline, on screen; any other waits in its place until it is opened or scrolled to.

use gpui_kit::component::{h_flex, v_flex, WindowExt as _};
use std::collections::HashMap;

use gpui_kit::{
    canvas, div, prelude::*, px, AnyElement, App, Context, Entity, Global, Hsla, Pixels, SharedString, Subscription, Task, WeakEntity,
    Window, WindowId,
};
use serde_json::{json, Value};
use wsp_host_client::protocol;

use super::PieceCx;
use crate::screens::browser::page::{Page, PageEvent};
use crate::screens::slate::{call, refusal, SlateScreen};
use crate::ui::{spinner, ActivePalette, Button, Glyph};

/// The frame's own words, as frame.tsx says them.
pub mod words {
    pub const EXPAND: &str = "Expand";
    pub const CLOSE: &str = "Close";
    pub const NO_WINDOW: &str = "This window reads no pages";
    pub const PAST_CAP: &str = "Two pages show at a time; expand this one to see it";
}

/// The seal, the web app's own, as one script that sets `window.wspSeal`.
const SEAL: &str = include_str!("../../../../record/frame-seal.js");
/// A page's tallest inline, `INLINE_CAP` in frame.tsx; past it the page scrolls inside its frame.
pub const INLINE_CAP: f32 = 560.;
/// The height a frame holds before its page says its own, `FIRST_HEIGHT`.
pub const FIRST_HEIGHT: f32 = 160.;
/// What the shell's title says the page's height with.
pub const HEIGHT_TITLE: &str = "wsp-frame-height:";
/// What the shell's title says, then why, where this engine cannot seal a page.
pub const REFUSED_TITLE: &str = "wsp-frame-refused:";
/// The live web views a window holds inline at most; the expand view's is its own.
pub const LIVE_MOST: usize = 2;
/// The shell's first act: an engine the seal cannot stand on draws no page and says why.
pub const SEALABLE_CHECK: &str = "if(!wspSeal.sealable()){document.title=\"wsp-frame-refused:\"+wspSeal.UNSEALABLE;return}";

/// The theme a page is handed, `FrameTheme` in frame-seal.ts: the scheme and the tokens as CSS values.
pub struct Theme {
    pub scheme: &'static str,
    pub vars: Vec<(&'static str, String)>,
}

impl Theme {
    /// Everything the shell is made from but the page: a theme of the same scheme with other colours moves it.
    fn key(&self) -> String {
        self.vars.iter().fold(self.scheme.to_owned(), |key, (k, v)| format!("{key};{k}:{v}"))
    }
}

fn css(c: Hsla) -> String {
    let c = c.to_rgb();
    let byte = |v: f32| (v.clamp(0., 1.) * 255.).round() as u8;
    format!("rgb({} {} {} / {})", byte(c.r), byte(c.g), byte(c.b), (c.a.clamp(0., 1.) * 1000.).round() / 1000.)
}

/// The window's theme as the web app hands it, `TOKENS` in frame-seal.ts read off this app's palette.
pub fn theme(cx: &App) -> Theme {
    let p = cx.palette();
    let t = gpui_kit::component::ActiveTheme::theme(cx);
    let font = |f: &SharedString| format!("\"{f}\", system-ui, sans-serif");
    Theme {
        scheme: if t.is_dark() { "dark" } else { "light" },
        vars: vec![
            ("background", css(p.background)),
            ("foreground", css(p.foreground)),
            ("card", css(p.card)),
            ("card-foreground", css(p.foreground)),
            ("muted", css(p.muted)),
            ("muted-foreground", css(p.muted_foreground)),
            ("accent", css(p.accent)),
            ("accent-foreground", css(p.foreground)),
            ("primary", css(p.primary)),
            ("primary-foreground", css(p.primary_foreground)),
            ("border", css(p.border)),
            ("input", css(p.input)),
            ("ring", css(p.ring)),
            ("success", css(p.success)),
            ("success-foreground", css(p.success_foreground)),
            ("warning", css(p.warning)),
            ("warning-foreground", css(p.warning_foreground)),
            ("error-foreground", css(p.error_foreground)),
            ("info-foreground", css(p.info_foreground)),
            ("slate-accent", css(p.slate_accent)),
            ("font-sans", font(&t.font_family)),
            ("font-mono", font(&t.mono_font_family)),
        ],
    }
}

/// JSON a script may hold: no `</script>` or `<!--` can close or open anything in it.
fn script_json(value: &Value) -> String {
    value.to_string().replace('<', "\\u003c").replace('\u{2028}', "\\u2028").replace('\u{2029}', "\\u2029")
}

/// The web view's own document: the seal, then the page sealed in the shell frame-seal.ts makes, its height passed on
/// as the title. It loads nothing itself and frames nothing but the shell.
pub fn shell(html: &str, theme: &Theme) -> String {
    let ground = theme.vars.iter().find(|(k, _)| *k == "background").map(|(_, v)| v.clone()).unwrap_or_else(|| "transparent".into());
    let vars: serde_json::Map<String, Value> = theme.vars.iter().map(|(k, v)| ((*k).to_owned(), json!(v))).collect();
    let page = script_json(&json!(html));
    let scheme = theme.scheme;
    let theme = script_json(&json!({ "scheme": theme.scheme, "vars": vars }));
    // The shell's own scheme is the page's: a frame of another scheme than its embedder is given an opaque ground.
    format!(
        r#"<!doctype html><html><head><meta charset="utf-8"><meta http-equiv="Content-Security-Policy" content="default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'"><title></title><style>:root{{color-scheme:{scheme}}}html,body{{margin:0;height:100%;overflow:hidden;background:{ground}}}iframe{{display:block;border:0;width:100%;height:100%;background:transparent}}</style></head><body><script>{SEAL}</script><script>(()=>{{{SEALABLE_CHECK}const page={page},theme={theme};const f=document.createElement("iframe");f.setAttribute("sandbox","allow-scripts");f.title="page";f.srcdoc=wspSeal.frameShell(wspSeal.sealPage(page,theme),theme.scheme);document.body.append(f);addEventListener("message",e=>{{const d=e.data;if(e.source===f.contentWindow&&d&&d.wspFrame==="height")document.title="{HEIGHT_TITLE}"+Math.max(0,Math.ceil(Number(d.height)||0))}})}})()</script></body></html>"#
    )
}

/// The height a title says, and nothing for any other title.
pub fn height_of(title: &str) -> Option<f32> {
    let n: f32 = title.strip_prefix(HEIGHT_TITLE)?.parse().ok()?;
    n.is_finite().then_some(n)
}

/// Why this engine draws no page, as the shell's title says it.
pub fn refused_of(title: &str) -> Option<&str> {
    title.strip_prefix(REFUSED_TITLE).filter(|why| !why.is_empty())
}

/// Whether a frame holds a live web view this frame: one it has stays while it is on screen, and a new one is made only
/// on screen and under the window's cap.
fn holds_view(on_screen: bool, has: bool, live_others: usize) -> bool {
    on_screen && (has || live_others < LIVE_MOST)
}

/// Every frame view of each window, to count the web views it holds.
#[derive(Default)]
struct Frames(HashMap<WindowId, Vec<WeakEntity<FrameView>>>);

impl Global for Frames {}

/// The live web views the window's other frames hold.
fn live_others(window: WindowId, this: &Entity<FrameView>, cx: &mut App) -> usize {
    let views = cx.default_global::<Frames>().0.entry(window).or_default();
    views.retain(|v| v.upgrade().is_some());
    let views: Vec<Entity<FrameView>> = views.iter().filter_map(WeakEntity::upgrade).filter(|v| v != this).collect();
    views.iter().filter(|v| v.read(cx).page.is_some()).count()
}

/// What the frame shows: its page on its way, the page, or why there is none.
#[derive(Clone, Debug, PartialEq)]
enum Read {
    Loading,
    Shown { html: String, version: Option<String> },
    Refused(String),
}

/// One frame piece's page and its web view, kept while the piece draws.
pub struct FrameView {
    src: String,
    mark: String,
    read: Read,
    asking: Option<Task<()>>,
    page: Option<Entity<Page>>,
    /// The html and theme key the web view was made for; either moving makes it again.
    made_for: Option<(String, String)>,
    height: f32,
    /// Whether the frame's place is on screen, as the last draw found it.
    on_screen: bool,
    /// Why this window's engine draws no page, once its shell has said so.
    unsealable: Option<String>,
    _heard: Option<Subscription>,
}

impl FrameView {
    fn new() -> FrameView {
        FrameView {
            src: String::new(),
            mark: String::new(),
            read: Read::Loading,
            asking: None,
            page: None,
            made_for: None,
            height: FIRST_HEIGHT,
            on_screen: false,
            unsealable: None,
            _heard: None,
        }
    }

    fn close(&mut self, cx: &mut App) {
        if let Some(old) = self.page.take() {
            old.update(cx, |p, _| p.close());
        }
        self.made_for = None;
        self._heard = None;
    }

    /// Asks the host again where the src or the mark moved: each write of the slate, each run that ended.
    fn want(&mut self, src: &str, mark: &str, screen: &WeakEntity<SlateScreen>, cx: &mut Context<Self>) {
        if self.src == src && self.mark == mark {
            return;
        }
        if self.src != src {
            self.read = Read::Loading;
        }
        self.src = src.to_owned();
        self.mark = mark.to_owned();
        let have = match &self.read {
            Read::Shown { version, .. } => version.clone(),
            _ => None,
        };
        // Asked once the draw that wanted it is done: the screen is mid-update while its pieces draw.
        let (screen, this, src) = (screen.clone(), cx.entity().downgrade(), src.to_owned());
        cx.defer(move |cx| {
            let Ok(asked) = screen.update(cx, |s, cx| s.page(src, have, cx)) else {
                _ = this.update(cx, |v, cx| {
                    v.read = Read::Refused(words::NO_WINDOW.to_owned());
                    cx.notify();
                });
                return;
            };
            _ = this.update(cx, |v, cx| {
                v.asking = Some(cx.spawn(async move |this, cx| {
                    let said = asked.await;
                    _ = this.update(cx, |v, cx| {
                        v.read = match said {
                            Err(why) => Read::Refused(why),
                            Ok(answer) => read_of(&answer).unwrap_or_else(|| v.read.clone()),
                        };
                        cx.notify();
                    });
                }));
            });
        });
    }

    /// The web view for what is shown, made again where the page or the theme moved.
    fn page_for(
        &mut self,
        html: &str,
        theme: &Theme,
        ground: Hsla,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<Entity<Page>, String> {
        let key = theme.key();
        if let (Some(page), Some((made, made_key))) = (&self.page, &self.made_for) {
            if made == html && *made_key == key {
                return Ok(page.clone());
            }
        }
        self.close(cx);
        let page = Page::sealed(&shell(html, theme), ground.to_rgb(), true, window, cx)?;
        self._heard = Some(cx.subscribe(&page, |v: &mut FrameView, _, event: &PageEvent, cx| {
            if let PageEvent::Titled(title) = event {
                if let Some(why) = refused_of(title) {
                    v.unsealable = Some(why.to_owned());
                    v.close(cx);
                    cx.notify();
                } else if let Some(h) = height_of(title) {
                    v.height = h.clamp(24., INLINE_CAP);
                    cx.notify();
                }
            }
        }));
        self.page = Some(page.clone());
        self.made_for = Some((html.to_owned(), key));
        Ok(page)
    }
}

/// The host's answer read into what the frame shows; an unchanged one keeps what it holds.
fn read_of(answer: &Value) -> Option<Read> {
    if let Some(problem) = answer.get("problem") {
        let said = problem.get("message").and_then(Value::as_str).unwrap_or_default();
        return Some(Read::Refused(sentence(said)));
    }
    if answer.get("unchanged").is_some() {
        return None;
    }
    let html = answer.get("html").and_then(Value::as_str)?.to_owned();
    Some(Read::Shown { html, version: answer.get("version").and_then(Value::as_str).map(str::to_owned) })
}

/// A refusal as the frame says it, `sentence` in frame.tsx: a lower-case first word made a capital.
fn sentence(said: &str) -> String {
    let mut chars = said.chars();
    match chars.next() {
        Some(first) if first.is_ascii_lowercase() && said.split_once(' ').is_some() => {
            first.to_ascii_uppercase().to_string() + chars.as_str()
        }
        _ => said.to_owned(),
    }
}

impl SlateScreen {
    /// A frame's page from the host's `slates.page`, naming the version held so an unchanged file is not sent again.
    pub fn page(&mut self, src: String, have: Option<String>, cx: &mut Context<Self>) -> Task<Result<Value, String>> {
        let Some(thread) = self.opened.thread_id.clone() else { return Task::ready(Err("No thread".into())) };
        let asking = self.host().run(move |host| async move {
            let mut params = json!({ "threadId": thread, "src": src });
            if let Some(have) = have {
                params["have"] = json!(have);
            }
            call::<protocol::SlatesPageRequest>(&host, params).await.map(|a| serde_json::to_value(a).unwrap_or(Value::Null))
        });
        cx.spawn(async move |_, _| asking.await.map_err(|f| refusal(&f)))
    }
}

pub fn draw(p: &PieceCx, window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    (p.piece.kind == "frame").then(|| frame(p, window, cx))
}

fn frame(p: &PieceCx, window: &mut Window, cx: &mut App) -> AnyElement {
    let pal = cx.palette().clone();
    let src = p.str("src").unwrap_or_default();
    let cap = p.str("caption").filter(|c| !c.is_empty());
    let key = SharedString::from(format!("slate-frame-{}-{}", p.slate.record.thread_id, p.id()));
    let view = window.use_keyed_state(key, cx, |window, cx| {
        let this = cx.entity().downgrade();
        cx.default_global::<Frames>().0.entry(window.window_handle().window_id()).or_default().push(this);
        FrameView::new()
    });
    let mark = format!("{}:{}", p.slate.record.version, super::media::runs_ended(p.slate));
    view.update(cx, |v, cx| v.want(&src, &mark, &p.screen, cx));
    let read = view.read(cx).read.clone();
    let theme = theme(cx);
    let title = cap.clone().unwrap_or_else(|| src.clone());
    let live_others = live_others(window.window_handle().window_id(), &view, cx);
    let (on_screen, has, height, unsealable) = {
        let v = view.read(cx);
        (v.on_screen, v.page.is_some(), v.height, v.unsealable.clone())
    };
    let body = match (&read, unsealable) {
        (Read::Shown { .. }, Some(why)) => waiting(Some(why), FIRST_HEIGHT, cx),
        (Read::Shown { html, .. }, None) if holds_view(on_screen, has, live_others) => {
            match view.update(cx, |v, cx| v.page_for(html, &theme, pal.background, window, cx)) {
                Ok(page) => div()
                    .debug_selector(|| "slate-frame-page".into())
                    .flex()
                    .flex_col()
                    .w_full()
                    .h(px(height))
                    .child(Page::slot_on(&page, pal.background, cx))
                    .into_any_element(),
                Err(why) => waiting(Some(why), FIRST_HEIGHT, cx),
            }
        }
        (Read::Shown { .. }, None) => {
            if has {
                view.update(cx, |v, cx| v.close(cx));
                window.refresh();
            }
            // Its place keeps the page's height, so nothing above or below moves when the view goes or comes.
            waiting(on_screen.then(|| words::PAST_CAP.to_owned()), height, cx)
        }
        (Read::Loading, _) => waiting(None, FIRST_HEIGHT, cx),
        (Read::Refused(why), _) => waiting(Some(why.clone()), FIRST_HEIGHT, cx),
    };
    let body = div().relative().w_full().child(body).child(on_screen_probe(&view)).into_any_element();
    let shown = matches!(read, Read::Shown { .. });
    let html = match &read {
        Read::Shown { html, .. } => Some(html.clone()),
        _ => None,
    };
    v_flex()
        .debug_selector(|| "slate-frame".into())
        .min_w_0()
        .gap(px(8.))
        .child(body)
        .child(h_flex().min_h(px(24.)).gap(px(12.)).child(div().flex_1().min_w_0().children(cap.map(|c| caption(c, cx)))).when_some(
            html.filter(|_| shown),
            |el, html| {
                el.child(
                    Button::icon_xs(p.el("expand"), Glyph::Maximize2)
                        .tooltip(words::EXPAND)
                        .on_click(move |_, window, cx| open(html.clone(), title.clone(), window, cx)),
                )
            },
        ))
        .into_any_element()
}

/// Watches whether the frame's place is on screen inside the box it scrolls in; it sits outside the web view's slot,
/// whose own clip would hide the scroll box's.
fn on_screen_probe(view: &Entity<FrameView>) -> AnyElement {
    let view = view.downgrade();
    canvas(
        move |bounds, window, cx| {
            let seen = window.content_mask().bounds.intersects(&bounds);
            _ = view.update(cx, |v, cx| {
                if v.on_screen != seen {
                    v.on_screen = seen;
                    cx.notify();
                }
            });
        },
        |_, _, _, _| {},
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
    .into_any_element()
}

fn caption(text: String, cx: &App) -> gpui_kit::Div {
    div().min_w_0().truncate().text_size(px(12.)).line_height(px(16.)).text_color(cx.palette().muted_foreground).child(text)
}

/// The frame's place while its page is on its way or waits for a view, or why there is none, `height` tall.
fn waiting(why: Option<String>, height: f32, cx: &App) -> AnyElement {
    let p = cx.palette();
    div()
        .debug_selector(|| "slate-frame-waiting".into())
        .flex()
        .items_center()
        .h(px(height))
        .px(px(16.))
        .py(px(12.))
        .rounded(crate::ui::size::CARD_RADIUS)
        .border_1()
        .border_color(p.border)
        .text_size(px(12.))
        .line_height(px(16.))
        .text_color(p.muted_foreground)
        .map(|el| match why {
            Some(why) => el.child(why),
            None => el.justify_center().child(spinner(cx).size(px(16.))),
        })
        .into_any_element()
}

/// The expand view: the page in a web view of its own over most of the window, as tall as the view, scrolling inside.
pub struct FrameOpen {
    page: Option<Entity<Page>>,
    title: String,
}

fn open(html: String, title: String, window: &mut Window, cx: &mut App) {
    let (theme, ground) = (theme(cx), cx.palette().background);
    let page = Page::sealed(&shell(&html, &theme), ground.to_rgb(), true, window, cx).ok();
    let opened = cx.new(|cx| {
        cx.on_release(|v: &mut FrameOpen, cx| {
            if let Some(page) = v.page.take() {
                page.update(cx, |p, _| p.close());
            }
        })
        .detach();
        FrameOpen { page, title }
    });
    let (w, h) = popup_size(window);
    let top = (window.viewport_size().height - h) / 2.;
    window.open_dialog(cx, move |dialog, _, _| {
        let opened = opened.clone();
        dialog.close_button(false).w(w).h(h).margin_top(top).p_0().content(move |content, _, _| content.child(opened.clone()))
    });
}

/// `EXPANDED_POPUP`, as the image's viewer opens.
fn popup_size(window: &Window) -> (Pixels, Pixels) {
    let view = window.viewport_size();
    ((view.width * 0.9).min(px(1400.)), view.height * 0.85)
}

impl Render for FrameOpen {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = cx.palette().clone();
        let (_, h) = popup_size(window);
        let close = |window: &mut Window, cx: &mut App| window.close_dialog(cx);
        v_flex()
            .debug_selector(|| "slate-frame-expanded".into())
            .h(h)
            .child(
                h_flex()
                    .gap(px(4.))
                    .px(px(20.))
                    .pt(px(16.))
                    .pb(px(12.))
                    .child(div().flex_1().min_w_0().truncate().text_size(px(14.)).text_color(p.foreground).child(self.title.clone()))
                    .child(
                        Button::icon_xs("slate-frame-close", Glyph::X)
                            .tooltip(words::CLOSE)
                            .on_click(move |_, window, cx| close(window, cx)),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .mx(px(20.))
                    .mb(px(20.))
                    .children(self.page.as_ref().map(|page| Page::slot_on(page, p.background, cx))),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOSTILE: &str = "<p>hi</p></script><script>parent.document.title='wsp-frame-height:9999'</script><!--";

    fn bare() -> Theme {
        Theme { scheme: "dark", vars: vec![("background", "rgb(1 2 3 / 1)".into())] }
    }

    #[test]
    fn the_shell_holds_the_page_as_text_no_tag_in_it_can_close_its_script() {
        let shell = shell(HOSTILE, &bare());
        let script = shell.rsplit("<script>").next().unwrap();
        assert_eq!(script.matches("</script>").count(), 1, "the page's own </script> stays text");
        assert!(!script.contains("<!--") && script.contains("\\u003c/script>"));
        assert!(shell.contains(r#"f.setAttribute("sandbox","allow-scripts")"#));
        assert!(shell.contains("default-src 'none'"));
        assert!(shell.contains("var wspSeal"));
    }

    #[test]
    fn an_engine_without_trusted_types_draws_no_page_and_its_shell_says_why() {
        let shell = shell("<p>x</p>", &bare());
        let body = shell.rsplit("<script>").next().unwrap();
        assert!(body.starts_with(&format!("(()=>{{{SEALABLE_CHECK}")), "the check comes before the page is sealed");
        assert!(SEALABLE_CHECK.contains(REFUSED_TITLE));
        assert_eq!(refused_of("wsp-frame-refused:Too old"), Some("Too old"));
        assert_eq!(refused_of("wsp-frame-refused:"), None);
        assert_eq!(refused_of("wsp-frame-height:12"), None);
        assert!(SEAL.contains("UNSEALABLE") && SEAL.contains("sealable"));
    }

    #[test]
    fn a_theme_of_the_same_scheme_with_other_colours_makes_the_view_again() {
        let graphite = Theme { scheme: "dark", vars: vec![("background", "rgb(10 10 10 / 1)".into())] };
        let moss = Theme { scheme: "dark", vars: vec![("background", "rgb(20 24 20 / 1)".into())] };
        assert_ne!(graphite.key(), moss.key());
        assert_eq!(graphite.key(), Theme { scheme: "dark", vars: vec![("background", "rgb(10 10 10 / 1)".into())] }.key());
    }

    #[test]
    fn a_window_holds_two_live_frames_on_screen_and_the_rest_wait() {
        assert!(holds_view(true, false, 0));
        assert!(holds_view(true, false, 1));
        assert!(!holds_view(true, false, 2), "a third on screen waits");
        assert!(holds_view(true, true, 2), "one it holds stays");
        assert!(!holds_view(false, true, 0), "off screen it lets its view go");
        assert!(!holds_view(false, false, 0));
    }

    #[test]
    fn a_height_is_read_off_the_shells_title_alone() {
        assert_eq!(height_of("wsp-frame-height:233"), Some(233.));
        assert_eq!(height_of("wsp-frame-height:NaN"), None);
        assert_eq!(height_of("wsp-frame-height:inf"), None);
        assert_eq!(height_of("A page's own title"), None);
        assert_eq!(height_of("x wsp-frame-height:12"), None);
    }

    #[test]
    fn the_hosts_answer_reads_as_the_page_its_refusal_or_unchanged() {
        assert!(
            matches!(read_of(&json!({ "html": "<p>x</p>", "version": "1:2" })), Some(Read::Shown { version: Some(v), .. }) if v == "1:2")
        );
        assert!(read_of(&json!({ "unchanged": true, "version": "1:2" })).is_none());
        assert_eq!(
            read_of(&json!({ "problem": { "code": "R900", "name": "data-missing", "message": "no file at pages/gone.html" } })),
            Some(Read::Refused("No file at pages/gone.html".into()))
        );
    }

    /// Needs node and the web app's dependencies installed (pnpm install).
    #[test]
    fn the_seal_is_the_web_apps_own() {
        let script = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("frame-seal.mjs");
        let out = std::process::Command::new("node").arg(script).arg("--check").output().expect("node on PATH");
        assert!(out.status.success(), "{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
        for name in ["sealPage", "frameShell"] {
            assert!(SEAL.contains(name), "{name}");
        }
    }
}
