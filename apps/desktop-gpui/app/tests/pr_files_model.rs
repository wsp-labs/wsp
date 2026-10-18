// SPDX-License-Identifier: AGPL-3.0-only
//! The Files pane's pure rules: the tree's rows (folders first, natural order, note rows last, closed folders, the
//! search), the crumbs and roots of `files/root.ts`, the finder's ranking, and the words with counts and sizes in them.

use std::collections::HashSet;

use desktop_gpui::screens::pr_pane::files::paths::{crumbs, parent_within, roots, Crumb};
use desktop_gpui::screens::pr_pane::files::rank::{file_query, rank};
use desktop_gpui::screens::pr_pane::files::rows::{errors, loaded, rows, Entry, Kind, Level, Levels};
use desktop_gpui::screens::pr_pane::files::words::{fmt_bytes, more_entries, truncated};

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
