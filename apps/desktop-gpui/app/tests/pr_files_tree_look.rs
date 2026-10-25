// SPDX-License-Identifier: AGPL-3.0-only
//! The Files tree laid out as today's `@pierre/trees` lays it out at `density: "compact"`, against a host started for
//! the test alone (host-client's tests/common) with a wsp-daemon serving fs.*: past the sheet's viewport the search
//! row is 28 px with 12 px under it; rows sit 16 px in from the tree's left and 16 px short of its right, the bar's
//! lane kept; a folder level is 18.7 px; with a search typed its first hit is ringed. Without a daemon it says so and
//! passes nothing.

#[path = "../../host-client/tests/common/mod.rs"]
mod common;
#[path = "pr_pane/stand_in.rs"]
mod stand_in;

use std::sync::Arc;
use std::time::{Duration, Instant};

use common::Throwaway;
use desktop_gpui::screens::pr_pane::files::FilesScreen;
use desktop_gpui::screens::{Opened, Screen};
use desktop_gpui::ui::{self, Side};
use gpui_kit::component::Root;
use gpui_kit::{px, size, AppContext, Modifiers, TestAppContext, VisualTestContext};

const WAIT: Duration = Duration::from_secs(30);

fn until(cx: &mut VisualTestContext, what: &str, mut done: impl FnMut(&mut VisualTestContext) -> bool) {
    let end = Instant::now() + WAIT;
    loop {
        cx.executor().advance_clock(Duration::from_millis(100));
        cx.run_until_parked();
        if done(cx) {
            return;
        }
        assert!(Instant::now() < end, "{what} within {} s", WAIT.as_secs());
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn row(cx: &mut VisualTestContext, path: &str) -> Option<gpui_kit::Bounds<gpui_kit::Pixels>> {
    cx.debug_bounds(Box::leak(format!("tree-{path}").into_boxed_str()))
}

#[gpui_kit::test]
fn the_tree_stands_where_pierres_does_with_its_first_hit_ringed(cx: &mut TestAppContext) {
    let server = Throwaway::start_with(stand_in::STAND_IN, true);
    if server.daemon.is_none() {
        println!("skipped: the host serves files through a wsp-daemon: {}", common::wsp_daemon().unwrap_err());
        return;
    }
    let folder = server.repo_with("repo", &[("docs/notes.md", "# notes\n"), ("src/a.ts", "export {};\n"), ("logo.bin", "x\n")]);
    server.wsp(&["add", &folder.to_string_lossy()]);
    server.wsp(&["run", "repo", "first", "--agent", "claude"]);
    let runtime = Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap());
    let client = Arc::new(runtime.block_on(server.client()));
    let threads = runtime.block_on(client.threads()).unwrap_or_else(|f| panic!("{}", f.message));
    let thread = threads.iter().find(|t| t.prompt.as_deref() == Some("first")).expect("the thread wsp run opened");
    let folder = folder.canonicalize().unwrap();
    let at = |rel: &str| folder.join(rel).to_string_lossy().into_owned();
    let opened = Opened {
        workspace_id: Some(thread.workspace_id.clone()),
        thread_id: thread.thread_id.clone(),
        tab: Some("files".into()),
        arg: None,
    };

    cx.executor().allow_parking();
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
    });
    let shell = desktop_gpui::screens::Host::new(client.clone(), runtime.clone());
    let mut made = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let screen = cx.new(|cx| FilesScreen::new(shell, opened, window, cx));
        made = Some(screen.clone());
        Root::new(screen, window, cx)
    });
    let screen = made.unwrap();
    cx.simulate_resize(size(px(1200.), px(700.)));
    let docs = at("docs");
    until(cx, "the tree listing the folder", |cx| row(cx, &docs).is_some());

    let (head, field, tree) =
        (cx.debug_bounds("files-subheader").unwrap(), cx.debug_bounds("files-search").unwrap(), cx.debug_bounds("files-tree").unwrap());
    assert_eq!((head.size.height, field.origin.y), (px(28.), head.origin.y), "inline, the search row is 28 px and the field fills it");
    assert_eq!(tree.origin.y, head.bottom() + px(12.), "12 px under the search row");
    let first = row(cx, &docs).unwrap();
    assert!((first.left() - tree.left() - px(16.)).abs() < px(0.01), "rows start 16 px in: {:?} in {:?}", first, tree);
    assert!(
        (tree.right() - first.right() - px(16.)).abs() <= px(0.5),
        "and end 16 px short, within a device px, the lane kept: {:?} in {:?}",
        first,
        tree
    );
    assert!(cx.debug_bounds("files-tree-ring").is_none(), "no ring without a search");

    // A search matches only what is listed, so docs is opened first.
    let notes = at("docs/notes.md");
    cx.simulate_click(first.center(), Modifiers::none());
    until(cx, "docs listed under it", |cx| row(cx, &notes).is_some());
    let field = cx.debug_bounds("files-search").unwrap();
    cx.simulate_click(field.center(), Modifiers::none());
    cx.simulate_input("notes");
    until(cx, "the search applied", |cx| row(cx, &at("src")).is_none());
    let (ring, hit, folder_row) =
        (cx.debug_bounds("files-tree-ring").expect("a ring with a search typed"), row(cx, &notes).unwrap(), row(cx, &docs).unwrap());
    assert_eq!(ring, hit, "the ring stands on the first hit, not the folder above it");
    assert!(screen.read_with(cx, |s, cx| s.tree().unwrap().read(cx).rows().iter().any(|r| r.path == notes && r.hit)));
    let name = |cx: &mut VisualTestContext, path: &str| {
        cx.debug_bounds(Box::leak(format!("tree-name-{path}").into_boxed_str())).expect("the row's name draws").left()
    };
    let step = name(cx, &notes) - name(cx, &docs);
    assert!((step - px(18.7)).abs() <= px(0.5), "a level is 18.7 px in, within a device px, found {step:?} ({folder_row:?})");
}

#[test]
fn the_apps_assets_serve_every_glyph_the_tree_names() {
    use desktop_gpui::screens::pr_pane::files::rows::file_glyph;
    use gpui_kit::AssetSource as _;
    let assets = desktop_gpui::screens::app_shell::assets::AppAssets;
    for name in [".gitignore", "logo.bin", "main.rs", "notes.md"] {
        let path = format!("pierre/{}.svg", file_glyph(name));
        let svg = assets.load(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
        assert!(svg.is_some_and(|svg| svg.starts_with(b"<svg")), "the app serves no {path} for {name}");
    }
}
