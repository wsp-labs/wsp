// SPDX-License-Identifier: AGPL-3.0-only

use super::add::{add_server, file_of};
use super::drawing::{fact_line, opens, skill_body, skill_preview};
use super::plugins::source_page;
use super::servers::{icon_host, server_lead};
use super::*;
use crate::screens::settings::Acts;
use crate::ui::{Button, Card, Glyph};
use gpui_kit::prelude::*;
use gpui_kit::{div, px, AnyElement, Window};

fn mcp(agent: &str, name: &str, line: &str) -> McpRow {
    serde_json::from_value(json!({
        "agent": agent, "name": name, "scope": "user", "transport": { "kind": "stdio", "line": line },
        "envNames": [], "auth": "open", "enabled": true
    }))
    .unwrap()
}

#[test]
fn one_server_set_up_for_two_agents_is_one_entry() {
    let rows = vec![mcp("claude", "docs", "npx docs"), mcp("codex", "docs", "npx docs"), mcp("codex", "other", "npx other")];
    let folded = fold_servers(&rows);
    assert_eq!(folded.len(), 2);
    assert_eq!(folded[0].rows.len(), 2);
    assert_eq!(folded[0].key, "server-global-docs-stdio-npx docs");
}

#[test]
fn a_skill_file_drops_its_frontmatter() {
    assert_eq!(skill_body("---\nname: x\n---\n# Body\n"), "# Body\n");
    assert_eq!(skill_body("# No front\n"), "# No front\n");
    assert_eq!(source_page(Some("owner/name")).as_deref(), Some("https://github.com/owner/name"));
    assert_eq!(source_page(Some("a/b/c")), None);
}

#[test]
fn a_skill_files_links_open_only_web_pages_and_mail() {
    assert!(opens("https://skills.sh/x") && opens("HTTP://a.b") && opens("mailto:a@b.c"));
    assert!(!opens("file:///Applications/Calculator.app") && !opens("vscode://open?x"));
    assert!(!opens("smb://host/share") && !opens("#usage"));
}

type Drawn = Box<dyn Fn(&mut Window, &mut App) -> AnyElement>;

/// A view drawing what `draw` makes each frame, wide enough for a page.
struct Draw(Drawn);

impl Render for Draw {
    fn render(&mut self, window: &mut Window, cx: &mut gpui_kit::Context<Self>) -> impl IntoElement {
        div().w(px(720.)).child((self.0)(window, cx))
    }
}

struct Page {
    store: Entity<Store>,
    rows: Rows,
    report: Report,
    acts: Acts,
}

fn page(cx: &mut gpui_kit::TestAppContext) -> Rc<Page> {
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::ui::init(crate::ui::Side::Light, cx);
    });
    let runtime = std::sync::Arc::new(tokio::runtime::Builder::new_current_thread().build().unwrap());
    let acts = Acts::new(gpui_kit::WeakEntity::new_invalid(), None, runtime);
    let store = cx.update(|cx| cx.new(|_| Store::new(acts.clone())));
    let rows = Rows { place: "here".into(), here: true, name: "this Mac".into(), held: None, broker: None, misses: Vec::new() };
    let mut report = super::super::model::records::report(json!({}));
    report.agents = vec![super::super::model::records::agent(json!({ "id": "claude", "name": "Claude Code" }))];
    Rc::new(Page { store, rows, report, acts })
}

fn open<'a>(
    page: &Rc<Page>,
    cx: &'a mut gpui_kit::TestAppContext,
    draw: impl Fn(&Page, &mut Window, &mut App) -> AnyElement + 'static,
) -> &'a mut gpui_kit::VisualTestContext {
    let page = page.clone();
    let view = cx.update(|cx| cx.new(|_| Draw(Box::new(move |window, cx| draw(&page, window, cx)))));
    let (_, cx) = cx.add_window_view(|window, cx| gpui_kit::component::Root::new(view, window, cx));
    cx.simulate_resize(gpui_kit::size(px(800.), px(1200.)));
    cx.run_until_parked();
    cx
}

fn hover_says(cx: &mut gpui_kit::VisualTestContext, selector: &'static str, words: &str) -> bool {
    let at = cx.debug_bounds(selector).unwrap_or_else(|| panic!("{selector} is drawn")).center();
    cx.simulate_mouse_move(at, None, gpui_kit::Modifiers::none());
    cx.executor().advance_clock(std::time::Duration::from_millis(800));
    cx.run_until_parked();
    cx.update(|window, cx| window.simulate_next_frame(cx));
    cx.run_until_parked();
    cx.debug_bounds(Box::leak(format!("tip:{words}").into_boxed_str())).is_some()
}

#[gpui_kit::test]
fn a_cut_facts_value_says_itself_whole_on_the_hover(cx: &mut gpui_kit::TestAppContext) {
    let page = page(cx);
    let url = "https://mcp.example.com/a/very/long/address/that/a/narrow/line/cuts/before/its/end/sse";
    let fact = Fact::new("reach", w::URL).value(url);
    let cx = open(&page, cx, move |p, _, cx| Card::new("facts").item(fact_line(&fact, w::URL, &p.acts, cx)).into_any_element());
    assert!(hover_says(cx, "fact-reach-value", url), "a fact's value says itself whole on its hover");
}

#[gpui_kit::test]
fn the_adds_file_says_itself_whole_on_the_hover(cx: &mut gpui_kit::TestAppContext) {
    let page = page(cx);
    let cx = open(&page, cx, |p, window, cx| {
        let k = K { store: &p.store, rows: &p.rows, report: Some(&p.report), icons: true };
        add_server(&k, window, cx)
    });
    let file = file_of("claude", None, &[]).expect("claude names the file a server lands in");
    assert!(hover_says(cx, "add-server-file", &file), "the add's file says itself whole on its hover");
}

#[gpui_kit::test]
fn a_long_skill_file_scrolls_in_its_box_on_the_shared_bar(cx: &mut gpui_kit::TestAppContext) {
    let page = page(cx);
    let text = (0..80).map(|n| format!("Line {n} of a long skill file.\n\n")).collect::<String>();
    let doc = Doc { size: text.len() as u64, text: Some(text), error: None };
    let cx = open(&page, cx, move |_, window, cx| skill_preview("long", &doc, window, cx));
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    let (bar, body) = (cx.debug_bounds("scrollbar").expect("an overflowing file draws its bar"), cx.debug_bounds("skill-preview").unwrap());
    assert_eq!(body.size.height, px(480.), "max-h-[480px]");
    assert_eq!((body.right() - bar.right(), bar.size.width), (px(1.), px(6.)), "the bar inside the box's right edge");
    assert_eq!((bar.top() - body.top(), body.bottom() - bar.bottom()), (px(4.), px(4.)));
}

#[gpui_kit::test]
fn the_skills_sh_link_draws_its_glyph_after_the_word(cx: &mut gpui_kit::TestAppContext) {
    let page = page(cx);
    let cx = open(&page, cx, |_, _, _| Button::ghost("add-link", w::SKILLS_SH).xs().after(Glyph::ExternalLink).into_any_element());
    let (button, glyph) = (cx.debug_bounds("button").unwrap(), cx.debug_bounds("button-after").unwrap());
    assert!(glyph.origin.x > button.center().x, "the external glyph stands after skills.sh");
    assert_eq!(glyph.size.width, px(12.));
}

#[gpui_kit::test]
fn a_server_leads_with_its_brand_else_its_own_icon_while_icons_are_on(cx: &mut gpui_kit::TestAppContext) {
    let page = page(cx);
    let http = |name: &str, host: &str| -> McpRow {
        serde_json::from_value(json!({
            "agent": "claude", "name": name, "scope": "user", "transport": { "kind": "http", "host": host },
            "envNames": [], "auth": "open", "enabled": true
        }))
        .unwrap()
    };
    let github = fold_servers(&[http("github", "api.githubcopilot.com")]).remove(0);
    let notion = fold_servers(&[http("notion", "mcp.notion.com")]).remove(0);
    assert_eq!(icon_host(&github), None, "a server with its company's mark asks for no icon");
    assert_eq!(icon_host(&notion), Some("mcp.notion.com"));
    page.store.update(cx, |s, _| s.icon_answered("mcp.notion.com".into(), Some("data:image/png;base64,iVBORw0KGgo=".into())));
    let leads = |icons: bool, cx: &mut gpui_kit::TestAppContext| {
        cx.update(|cx| {
            let k = K { store: &page.store, rows: &page.rows, report: Some(&page.report), icons };
            (server_lead(&github, &k, cx), server_lead(&notion, &k, cx))
        })
    };
    let (Lead::Server { mark, .. }, Lead::Server { mark: none, icon }) = leads(true, cx) else { panic!("a server leads as a server") };
    assert_eq!((mark, none, icon.is_some()), (Some("github"), None, true), "github's mark; the other's own icon");
    let (_, Lead::Server { icon, .. }) = leads(false, cx) else { panic!("a server leads as a server") };
    assert!(icon.is_none(), "with server icons off the glyph stands");
}

#[test]
fn names_sort_as_locale_compare_sorts_them() {
    // The order node's own localeCompare gives these.
    let mut names = ["Zeta", "émile", "Eve", "alpha", "Alpha", "Ábaco", "beta", "a-b", "a_b", "ab"];
    names.sort_by(|a, b| by_name(a, b));
    assert_eq!(names, ["a_b", "a-b", "ab", "Ábaco", "alpha", "Alpha", "beta", "émile", "Eve", "Zeta"]);
}

/// A server on loopback that counts the requests it is sent and answers each with a 404.
fn image_server() -> (String, std::sync::Arc<std::sync::atomic::AtomicUsize>) {
    use std::io::{Read as _, Write as _};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let at = format!("http://{}", listener.local_addr().unwrap());
    let asked = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counted = asked.clone();
    std::thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            counted.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let mut head = [0u8; 2048];
            _ = stream.read(&mut head);
            _ = stream.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
        }
    });
    (at, asked)
}

#[gpui_kit::test]
fn a_skill_files_images_and_raw_html_fetch_nothing_and_draw_as_words(cx: &mut gpui_kit::TestAppContext) {
    use gpui_kit::http_client::{AsyncBody, FakeHttpClient, Response};
    use std::io::{Read as _, Write as _};
    let (server, asked) = image_server();
    let page = page(cx);
    // The app's road to the web, carried to the server for real, so a fetch the preview made would reach its count.
    cx.update(|cx| {
        cx.set_http_client(FakeHttpClient::create(|request| async move {
            let uri = request.uri().clone();
            let host = uri.authority().map(|a| a.to_string()).unwrap_or_default();
            if let Ok(mut stream) = std::net::TcpStream::connect(&host) {
                _ = write!(stream, "GET {} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n", uri.path());
                _ = stream.read_to_end(&mut Vec::new());
            }
            Ok(Response::builder().status(404).body(AsyncBody::default()).unwrap())
        }));
    });
    let text = format!(
        "# Logo\n\n![logo]({server}/logo.png)\n\nInline ![]({server}/bare.png) and ![local](./icon.png).\n\n<img src=\"{server}/raw.png\">\n\n<div><b>raw</b> markup</div>\n\nText with <img src=\"{server}/inline.png\"> inline.\n\n| Image |\n| --- |\n| ![cell]({server}/cell.png) |\n"
    );
    let doc = Doc { size: text.len() as u64, text: Some(text), error: None };
    let cx = open(&page, cx, move |_, window, cx| skill_preview("images", &doc, window, cx));
    for _ in 0..4 {
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    cx.run_until_parked();
    assert_eq!(asked.load(std::sync::atomic::Ordering::SeqCst), 0, "a skill file's image is never fetched");
    assert!(cx.debug_bounds("skill-image-fact").is_some(), "an image off the web is its alt and address as words");
    assert!(cx.debug_bounds("skill-raw-html").is_some(), "raw HTML is its own source text");
    let link = cx.debug_bounds("skill-image-link").expect("a web image is a link to its address");
    cx.simulate_click(link.center(), gpui_kit::Modifiers::none());
    assert_eq!(cx.opened_url(), Some(format!("{server}/cell.png")), "the last one drawn, the table's, opens its address");
}
