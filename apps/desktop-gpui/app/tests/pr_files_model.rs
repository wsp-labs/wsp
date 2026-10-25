// SPDX-License-Identifier: AGPL-3.0-only
//! The Files pane's pure rules: the tree's rows (folders first, natural order, note rows last, closed folders, the
//! search), the crumbs and roots of `files/root.ts`, the finder's ranking, the words with counts and sizes in them, and
//! what a rendered markdown file may reach.

use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use desktop_gpui::screens::pr_pane::changes::host::Read as Rows;
use desktop_gpui::screens::pr_pane::files::at::{moves, row_pushed, Facts, Moves};
use desktop_gpui::screens::pr_pane::files::markdown;
use desktop_gpui::screens::pr_pane::files::open_split::editor_svg;
use desktop_gpui::screens::pr_pane::files::paths::{crumbs, parent_within, roots, Crumb};
use desktop_gpui::screens::pr_pane::files::rank::{file_query, rank};
use desktop_gpui::screens::pr_pane::files::rows::{errors, loaded, rows, Entry, Kind, Level, Levels};
use desktop_gpui::screens::pr_pane::files::words::{more_entries, truncated};
use desktop_gpui::ui::{self, fmt_bytes, Side};
use gpui_kit::component::text::TextViewState;
use gpui_kit::{point, px, AppContext as _, Modifiers, MouseButton};
use serde_json::json;

fn level(dir: &str, names: &[(&str, bool)]) -> Level {
    let entries = names.iter().map(|(n, d)| Entry { name: n.to_string(), path: format!("{dir}/{n}"), dir: *d }).collect();
    Level { entries: Some(entries), ..Level::default() }
}

fn shown(levels: &Levels, open: &[&str], search: &str) -> Vec<String> {
    let open: HashSet<String> = open.iter().map(|s| s.to_string()).collect();
    rows("/r", levels, &open, search).into_iter().map(|r| format!("{}{}", "  ".repeat(r.depth), r.name)).collect()
}

#[test]
fn folders_come_first_in_natural_order_and_start_closed() {
    let mut levels = Levels::new();
    levels.insert("/r".into(), level("/r", &[("b10.txt", false), ("Zeta", true), ("b2.txt", false), ("alpha", true), ("A.md", false)]));
    levels.insert("/r/alpha".into(), level("/r/alpha", &[("x.ts", false)]));
    assert_eq!(shown(&levels, &[], ""), ["alpha", "Zeta", "A.md", "b2.txt", "b10.txt"]);
    assert_eq!(shown(&levels, &["/r/alpha"], ""), ["alpha", "  x.ts", "Zeta", "A.md", "b2.txt", "b10.txt"]);
}

#[test]
fn a_cut_folder_ends_in_its_note_row() {
    let mut levels = Levels::new();
    let mut cut = level("/r", &[("z.txt", false), ("a", true)]);
    cut.truncated = true;
    cut.total = 10_002;
    levels.insert("/r".into(), cut);
    let rows = rows("/r", &levels, &HashSet::new(), "");
    assert_eq!(rows.last().map(|r| (r.name.as_str(), r.kind.clone())), Some(("… 10,000 more entries not shown", Kind::Note)));
    assert_eq!(rows[0].name, "a", "the folder still leads");
    assert_eq!(more_entries(1), "… 1 more entry not shown");
}

#[test]
fn a_search_shows_the_matches_and_the_folders_above_them_open() {
    let mut levels = Levels::new();
    levels.insert("/r".into(), level("/r", &[("src", true), ("readme.md", false)]));
    levels.insert("/r/src".into(), level("/r/src", &[("plan.ts", false), ("other.ts", false)]));
    assert_eq!(shown(&levels, &[], "PLAN"), ["src", "  plan.ts"]);
    assert_eq!(shown(&levels, &[], "rc/pl"), ["src", "  plan.ts"], "the path below the root is searched, not the name alone");
    assert_eq!(shown(&levels, &[], " SRC\\PL "), ["src", "  plan.ts"], "trimmed, a backslash read as a slash");
}

#[test]
fn a_search_that_matches_nothing_listed_shows_every_row_closed() {
    let mut levels = Levels::new();
    levels.insert("/r".into(), level("/r", &[("docs", true), ("src", true), (".gitignore", false), ("logo.bin", false)]));
    levels.insert("/r/src".into(), level("/r/src", &[("plan.ts", false)]));
    assert_eq!(
        shown(&levels, &["/r/src"], "NOTES"),
        ["docs", "src", ".gitignore", "logo.bin"],
        "nothing hidden and every folder closed, as the web tree draws a search with no match"
    );
    assert_eq!(shown(&levels, &["/r/src"], ""), ["docs", "src", "  plan.ts", ".gitignore", "logo.bin"], "the open folders come back");
    assert_eq!(shown(&levels, &[], "docs/"), ["docs"], "a folder not listed yet matches by its path, which holds its slash");
}

#[test]
fn a_folder_that_failed_is_said_beside_the_tree_and_a_refresh_asks_what_it_holds() {
    let mut levels = Levels::new();
    levels.insert("/r".into(), level("/r", &[("a", true), ("b", true)]));
    levels.insert("/r/a".into(), level("/r/a", &[]));
    levels.insert("/r/b".into(), Level { error: Some("permission denied".into()), ..Level::default() });
    assert_eq!(errors("/r", &levels), [("b".to_owned(), "permission denied".to_owned())]);
    assert_eq!(loaded("/r", &levels), ["/r", "/r/a"]);
}

#[test]
fn crumbs_name_the_root_whole_then_each_folder_and_up_stops_at_a_root() {
    let roots = roots(Some("/home/u"), Some("/home/u/code/repo"));
    assert_eq!(roots, ["/home/u", "/home/u/code/repo"]);
    let c = crumbs(&roots, "/home/u/code/repo/src/lib");
    let want = [("/home/u/code/repo", "/home/u/code/repo"), ("src", "/home/u/code/repo/src"), ("lib", "/home/u/code/repo/src/lib")];
    assert_eq!(c, want.map(|(n, p)| Crumb { name: n.into(), path: p.into() }), "the nearest root leads");
    assert_eq!(parent_within(&roots, "/home/u/code"), Some("/home/u".into()));
    assert_eq!(parent_within(&roots, "/home/u"), None, "inert at a root's edge");
}

#[test]
fn the_finder_ranks_a_name_before_a_folder_and_trims_its_query() {
    assert_eq!(file_query("  @./Plan md "), "planmd");
    let paths: Vec<String> = ["plan/notes.txt", "docs/plan.md", "src/explain.rs", "other.txt"].map(String::from).to_vec();
    let ranked: Vec<String> = rank(&paths, "plan", 50).into_iter().map(|m| m.path).collect();
    assert_eq!(ranked, ["docs/plan.md", "plan/notes.txt", "src/explain.rs"], "the name, then the folders, then letters in order");
    assert_eq!(rank(&paths, "", 2).len(), 2, "no query: the daemon's first few");
}

/// The svg in a catalog module's `svg: \`...\``.
fn svg_in(module: &str) -> Option<&str> {
    let start = module.find("svg: `")? + "svg: `".len();
    Some(&module[start..start + module[start..].find('`')?])
}

#[test]
fn every_editor_the_catalog_marks_draws_its_own_mark() {
    let marks = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../packages/catalog/src/marks/");
    let read = |path: &str| std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let index = read(&format!("{marks}index.ts"));
    let listed =
        index.split("export const EDITOR_MARKS").nth(1).and_then(|rest| rest.split_once("= [")).and_then(|(_, rest)| rest.split_once(']'));
    let (listed, _) = listed.expect("index.ts lists EDITOR_MARKS");
    let mut ids = Vec::new();
    for name in listed.split(',').map(str::trim).filter(|n| !n.is_empty()) {
        let import =
            index.lines().find(|l| l.starts_with(&format!("import {{ {name} }} from \""))).unwrap_or_else(|| panic!("{name} is imported"));
        let path = format!("{marks}{}", import.split('"').nth(1).unwrap().trim_start_matches("./").replace(".js", ".ts"));
        let module = read(&path);
        // A mark that wears another row's (`...CURSOR_AGENT.mark!`) has its svg in the module it imports.
        let own = match svg_in(&module) {
            Some(svg) => svg.to_owned(),
            None => {
                let from = module.lines().find(|l| l.starts_with("import {") && !l.contains("type ")).expect("the row it wears");
                let dir = std::path::Path::new(&path).parent().unwrap();
                let worn = read(&dir.join(from.split('"').nth(1).unwrap().replace(".js", ".ts")).to_string_lossy());
                svg_in(&worn).expect("the worn row's svg").to_owned()
            }
        };
        let editors =
            module.split("editors: [").nth(1).and_then(|rest| rest.split(']').next()).unwrap_or_else(|| panic!("{path} names its editors"));
        for id in editors.split(',').map(|id| id.trim().trim_matches('"')).filter(|id| !id.is_empty()) {
            ids.push(id.to_owned());
            assert_eq!(editor_svg(id), Some(own.as_str()), "{id} draws the mark {path} gives it");
        }
    }
    assert!(ids.len() >= 14, "the catalog's editors: {ids:?}");
}

#[test]
fn a_row_push_of_the_thread_reads_again_and_nothing_else_does() {
    let row = |workspace: &str, thread: &str| json!({ "type": "session.row", "workspaceId": workspace, "id": "s", "row": { "workspaceId": workspace, "threadId": thread, "cwd": "/r" } });
    assert_eq!(moves(&row("w", "t"), "w", Some("t")), Moves::Row, "the thread's row");
    assert_eq!(moves(&row("w", "other"), "w", Some("t")), Moves::Nothing, "another thread's row");
    assert_eq!(moves(&row("x", "t"), "w", Some("t")), Moves::Nothing, "another workspace's row");
    assert_eq!(moves(&row("w", "t"), "w", None), Moves::Nothing, "no thread follows no row");
    assert_eq!(moves(&json!({ "type": "session.row", "workspaceId": "w", "id": "s" }), "w", Some("t")), Moves::Row, "a row gone");
    assert_eq!(moves(&json!({ "type": "session.start", "workspaceId": "w", "threadId": "t" }), "w", Some("t")), Moves::Nothing);
    assert_eq!(moves(&json!({ "type": "workspace.status", "status": { "id": "w" } }), "w", Some("t")), Moves::Workspace);
    assert_eq!(moves(&json!({ "type": "workspace.status", "status": { "id": "x" } }), "w", Some("t")), Moves::Nothing);
    assert_eq!(moves(&json!({ "type": "workspace.updated", "workspaceId": "w" }), "w", None), Moves::Workspace);
    assert_eq!(moves(&json!({ "type": "workspace.updated", "workspaceId": "x" }), "w", None), Moves::Nothing, "another workspace's update");
    assert_eq!(moves(&json!({ "type": "workspace.viewed", "workspaceId": "w" }), "w", None), Moves::Nothing);
}

#[test]
fn the_threads_folder_is_the_newest_its_rows_name_whatever_order_the_pushes_come_in() {
    let push = |workspace: &str, id: &str, at: f64, cwd: &str| json!({ "type": "session.row", "workspaceId": workspace, "id": id, "row": { "workspaceId": workspace, "threadId": "t", "startedAt": at, "cwd": cwd } });
    let (mut rows, mut facts) = (Rows::default(), Facts::default());
    let mut take = |frame: serde_json::Value| {
        facts = row_pushed(&mut rows, &facts, &frame, "w", "t");
        facts.cwd.clone()
    };
    assert_eq!(take(push("w", "s2", 2., "/r/new")), Some("/r/new".into()));
    assert_eq!(take(push("w", "s1", 1., "/r/old")), Some("/r/new".into()), "a late push of the turn before moves nothing");
    assert_eq!(take(push("x", "s9", 9., "/r/else")), Some("/r/new".into()), "another workspace's row");
    assert_eq!(take(push("w", "s3", 3., "")), Some("/r/new".into()), "a new turn's row naming no folder yet keeps the last one");
    assert_eq!(take(push("w", "s3", 3., "/r/newest")), Some("/r/newest".into()));
}

#[test]
fn sizes_read_as_the_web_writes_them() {
    assert_eq!(fmt_bytes(512), "512 B");
    assert_eq!(fmt_bytes(2 * 1024 * 1024), "2 MB");
    assert_eq!(fmt_bytes(3 * 1024 * 1024 * 1024 / 2), "1.5 GB");
    assert_eq!(truncated(2 * 1024 * 1024, 3 * 1024 * 1024), "Showing the first 2 MB of 3 MB; the daemon caps reads at 2 MB.");
}

#[gpui_kit::test]
fn the_markdown_toggle_is_kept_in_the_apps_store(cx: &mut gpui_kit::TestAppContext) {
    use desktop_gpui::screens::pr_pane::files::RenderMarkdown;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("local-store.json");
    cx.update(|cx| {
        desktop_gpui::local_store::init(path.clone(), cx);
        assert!(RenderMarkdown::on(cx), "rendered until turned off");
        RenderMarkdown::set(false, cx);
        cx.remove_global::<RenderMarkdown>();
        assert!(!RenderMarkdown::on(cx), "off, read back from the store");
    });
}

/// A markdown file with each road out of it: an image, a raw `<img>`, and links to the web, the disk, a script and a
/// file beside it.
const ROADS_OUT: &str = "![the logo](https://example.invalid/logo.png)\n\n<img src=\"https://example.invalid/pixel.png\" alt=\"pixel\">\n\n[web](https://example.com/) [disk](file:///etc/passwd) [script](javascript:alert(1)) [near](./other.md)\n";

struct Preview(gpui_kit::Entity<TextViewState>);

impl gpui_kit::Render for Preview {
    fn render(&mut self, _: &mut gpui_kit::Window, cx: &mut gpui_kit::Context<Self>) -> impl gpui_kit::IntoElement {
        markdown::preview(&self.0, cx)
    }
}

fn preview<'a>(text: &'static str, cx: &'a mut gpui_kit::TestAppContext) -> &'a mut gpui_kit::VisualTestContext {
    let (_, cx) = cx.add_window_view(|_, cx| Preview(cx.new(|cx| TextViewState::markdown(text, cx))));
    cx.run_until_parked();
    cx
}

#[gpui_kit::test]
fn a_markdown_file_draws_its_images_as_words_and_fetches_nothing(cx: &mut gpui_kit::TestAppContext) {
    use gpui_kit::http_client::{FakeHttpClient, Response};
    let asked = Arc::new(Mutex::new(Vec::<String>::new()));
    let heard = asked.clone();
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
        cx.set_http_client(FakeHttpClient::create(move |request| {
            heard.lock().unwrap().push(request.uri().to_string());
            async { Ok(Response::builder().status(404).body(Default::default()).unwrap()) }
        }));
    });
    let cx = preview(ROADS_OUT, cx);
    assert!(cx.debug_bounds("markdown-image").is_some(), "an image is its alt text in a chip");
    assert!(cx.debug_bounds("markdown-dead-link").is_some(), "a link that does not open is plain text");
    assert_eq!(*asked.lock().unwrap(), Vec::<String>::new(), "nothing the file names is fetched");
    for (url, open) in [
        ("https://example.com/", true),
        ("HTTP://example.com", true),
        ("file:///etc/passwd", false),
        ("javascript:alert(1)", false),
        ("./other.md", false),
        ("mailto:a@example.com", false),
    ] {
        assert_eq!(markdown::opens(url), open, "{url}");
    }
}

#[gpui_kit::test]
fn a_markdown_link_opens_on_a_press_and_not_on_a_right_click(cx: &mut gpui_kit::TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
    });
    let cx = preview("[web](https://example.com/)", cx);
    let on = cx.debug_bounds("markdown-preview").expect("the preview draws").origin + point(px(32.), px(32.));
    cx.simulate_mouse_down(on, MouseButton::Right, Modifiers::none());
    cx.simulate_mouse_up(on, MouseButton::Right, Modifiers::none());
    assert_eq!(cx.opened_url(), None, "a right-click on a link opens nothing");
    cx.simulate_click(on, Modifiers::none());
    assert_eq!(cx.opened_url().as_deref(), Some("https://example.com/"), "a press opens it");
}
