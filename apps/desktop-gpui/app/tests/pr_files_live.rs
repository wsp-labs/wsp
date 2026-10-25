// SPDX-License-Identifier: AGPL-3.0-only
//! The Files pane, the file tab and the finder against a host started for each test alone (host-client's
//! tests/common), carrying a wsp-daemon that serves fs.*: the tree lists the project's folder with folders first and
//! gitignored files hidden and reads a folder as it opens; a file tab reads a file and lights the line it was opened
//! at, and notes a file past the daemon's 2 MB; the finder finds by name and by content, stops at the daemon's 500
//! lines, keeps itself open under a press inside and closes on Escape. Without a daemon every test says so and passes
//! nothing.

#[path = "../../host-client/tests/common/mod.rs"]
mod common;
#[path = "pr_pane/stand_in.rs"]
mod stand_in;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use common::Throwaway;
use desktop_gpui::screens::dialogs::layer;
use desktop_gpui::screens::pr_pane::daemon::DaemonLink;
use desktop_gpui::screens::pr_pane::files::file_tab::Read;
use desktop_gpui::screens::pr_pane::files::finder::{self, Answer, Finder, Mode};
use desktop_gpui::screens::pr_pane::files::open_split;
use desktop_gpui::screens::pr_pane::files::rows::Kind;
use desktop_gpui::screens::pr_pane::files::words::finder as words;
use desktop_gpui::screens::pr_pane::files::{self, FilesScreen};
use desktop_gpui::screens::{Opened, Screen};
use desktop_gpui::ui::{self, Side};
use gpui_kit::component::Root;
use gpui_kit::{px, size, AppContext, Entity, Modifiers, TestAppContext, VisualTestContext};
use stand_in::STAND_IN;
use tokio::runtime::Runtime;
use wsp_host_client::HostClient;

const WAIT: Duration = Duration::from_secs(30);

/// The stand-in claude, which also runs `cd <folder>` as a Bash call when `$HOME/next-cd` names a folder.
fn stand_in() -> String {
    let text = r#"  printf '{"type":"assistant","session_id":"%s","message":{"role":"assistant","content":[{"type":"text""#;
    let cd = r#"  [ -f "$HOME/next-cd" ] && printf '{"type":"assistant","session_id":"%s","message":{"role":"assistant","content":[{"type":"tool_use","id":"cd-%s","name":"Bash","input":{"command":"cd %s"}}]}}\n' "$session" "$(date +%s%N)" "$(cat "$HOME/next-cd")"
"#;
    assert!(STAND_IN.contains(text), "the stand-in answers with a text line to put the call before");
    STAND_IN.replacen(text, &format!("{cd}{text}"), 1)
}

/// A host with a project `repo` and a thread whose first turn ran.
struct Host {
    client: Arc<HostClient>,
    runtime: Arc<Runtime>,
    workspace_id: String,
    thread_id: String,
    folder: PathBuf,
    server: Throwaway,
}

impl Host {
    fn start() -> Option<Host> {
        let server = Throwaway::start_with(&stand_in(), true);
        if server.daemon.is_none() {
            println!("skipped: the host serves files through a wsp-daemon: {}", common::wsp_daemon().unwrap_err());
            return None;
        }
        let mut files: Vec<(String, String)> = vec![
            (".gitignore".into(), "secret.log\n".into()),
            ("secret.log".into(), "kept out\n".into()),
            ("Alpha.md".into(), "# Alpha\n".into()),
            ("b.txt".into(), "one\ntwo\nthree\nfour\n".into()),
            ("src/a10.ts".into(), "export const ten = 10;\n".into()),
            ("src/a2.ts".into(), "export const two = 2;\n".into()),
            ("notes/plan.md".into(), "# plan\n\nfirst step\nsecond step\n".into()),
            ("notes/deep/later.md".into(), "# later\n".into()),
        ];
        for n in 0..300 {
            files.push((format!("many/f{n}.txt"), "needle one\nneedle two\n".into()));
        }
        let refs: Vec<(&str, &str)> = files.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
        let folder = server.repo_with("repo", &refs);
        std::fs::write(folder.join("big.txt"), "x".repeat(3 * 1024 * 1024)).unwrap();
        server.wsp(&["add", &folder.to_string_lossy()]);
        server.wsp(&["run", "repo", "first", "--agent", "claude"]);
        let runtime = Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap());
        let client = runtime.block_on(server.client());
        let threads = runtime.block_on(client.threads()).unwrap_or_else(|f| panic!("{}", f.message));
        let row = threads.iter().find(|t| t.prompt.as_deref() == Some("first")).expect("the thread wsp run opened");
        let (workspace_id, thread_id) = (row.workspace_id.clone(), row.thread_id.clone().expect("the row names its thread"));
        let folder = folder.canonicalize().unwrap();
        Some(Host { client: Arc::new(client), runtime, workspace_id, thread_id, folder, server })
    }

    fn shell(&self) -> desktop_gpui::screens::Host {
        desktop_gpui::screens::Host::new(self.client.clone(), self.runtime.clone())
    }

    fn at(&self, rel: &str) -> String {
        self.folder.join(rel).to_string_lossy().into_owned()
    }

    /// One more turn on the thread, its agent reporting `cwd` as the folder it works in.
    fn turn_in(&self, cwd: &str) {
        std::fs::write(self.server.dir.path().join("next-cwd"), cwd).unwrap();
        self.server.wsp(&["send", &self.thread_id, "move"]);
    }

    /// One more turn on the thread, its agent running `cd <dir>` in its tool shell.
    fn cd_in(&self, dir: &str) {
        std::fs::write(self.server.dir.path().join("next-cd"), dir).unwrap();
        self.server.wsp(&["send", &self.thread_id, "cd"]);
    }

    /// Waits for the thread's history to hold `n` tool calls that moved the agent's shell.
    fn until_history_moves_shell(&self, n: usize) {
        let moves = || {
            let thread = self
                .runtime
                .block_on(self.client.open(&self.workspace_id, &self.thread_id))
                .unwrap_or_else(|f| panic!("the thread's history: {}", f.message));
            thread
                .history
                .iter()
                .filter(|e| serde_json::to_value(e).is_ok_and(|v| v["type"] == "session.delta" && v.get("cwd").is_some()))
                .count()
        };
        let end = Instant::now() + WAIT;
        while moves() < n {
            assert!(Instant::now() < end, "the thread's history holding {n} cds within {} s", WAIT.as_secs());
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    fn opened(&self, tab: &str) -> Opened {
        Opened {
            workspace_id: Some(self.workspace_id.clone()),
            thread_id: Some(self.thread_id.clone()),
            tab: Some(gpui_kit::SharedString::from(tab.to_owned())),
            arg: None,
        }
    }
}

/// The Files screen mounted as the panel mounts it, in a window under gpui-kit's Root.
fn mount<'a>(host: &Host, tab: &str, cx: &'a mut TestAppContext) -> (Entity<FilesScreen>, &'a mut VisualTestContext) {
    cx.executor().allow_parking();
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
    });
    let (shell, opened) = (host.shell(), host.opened(tab));
    let mut made = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let screen = cx.new(|cx| FilesScreen::new(shell, opened, window, cx));
        made = Some(screen.clone());
        Root::new(screen, window, cx)
    });
    cx.simulate_resize(size(px(900.), px(700.)));
    (made.unwrap(), cx)
}

fn until(cx: &mut VisualTestContext, what: &str, mut done: impl FnMut(&mut VisualTestContext) -> bool) {
    let end = Instant::now() + WAIT;
    loop {
        // The finder's settle timer runs on the test's clock, which moves only when told.
        cx.executor().advance_clock(Duration::from_millis(100));
        cx.run_until_parked();
        if done(cx) {
            return;
        }
        assert!(Instant::now() < end, "{what} within {} s", WAIT.as_secs());
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn names(screen: &Entity<FilesScreen>, cx: &mut VisualTestContext) -> Vec<(String, usize)> {
    screen.read_with(cx, |s, cx| s.tree().unwrap().read(cx).rows().iter().map(|r| (r.name.clone(), r.depth)).collect())
}

#[gpui_kit::test]
fn the_tree_lists_the_folder_folders_first_without_ignored_files_and_reads_a_folder_as_it_opens(cx: &mut TestAppContext) {
    let Some(host) = Host::start() else { return };
    let (screen, cx) = mount(&host, "files", cx);
    until(cx, "the tree listing the project's folder", |cx| !names(&screen, cx).is_empty());
    let folder = screen.read_with(cx, |s, cx| s.tree().unwrap().read(cx).folder().map(str::to_owned));
    assert_eq!(folder.as_deref(), Some(host.at("").trim_end_matches('/')), "the tree roots at the thread's folder");
    let top: Vec<String> = names(&screen, cx).into_iter().map(|(n, d)| format!("{n}@{d}")).collect();
    assert_eq!(
        top,
        ["many@0", "notes@0", "src@0", ".gitignore@0", "Alpha.md@0", "b.txt@0", "big.txt@0"],
        "folders first, closed, natural name order; secret.log is gitignored"
    );
    assert!(
        cx.debug_bounds("files-location").is_some() && cx.debug_bounds("files-subheader").is_some(),
        "the crumb row and the search row draw"
    );

    let src = host.at("src");
    let listed = screen.read_with(cx, |s, cx| s.tree().unwrap().read(cx).levels().contains_key(&src));
    assert!(!listed, "a closed folder is not read until it opens");
    let row = screen.read_with(cx, |s, cx| s.tree().unwrap().read(cx).rows().iter().find(|r| r.path == src).cloned().unwrap());
    assert_eq!(row.kind, Kind::Folder { open: false });
    let bounds = cx.debug_bounds(Box::leak(format!("tree-{src}").into_boxed_str())).expect("the src row draws");
    cx.simulate_click(bounds.center(), Modifiers::none());
    until(cx, "src's listing read and shown under it", |cx| names(&screen, cx).iter().any(|(n, d)| n == "a2.ts" && *d == 1));
    let under: Vec<String> = names(&screen, cx).into_iter().filter(|(_, d)| *d == 1).map(|(n, _)| n).collect();
    assert_eq!(under, ["a2.ts", "a10.ts"], "the opened folder's files, a2 before a10");

    let b = host.at("b.txt");
    cx.update(|_, cx| files::open_file(&host.workspace_id, &b, Some(4), cx));
    let row = cx.debug_bounds(Box::leak(format!("tree-{b}").into_boxed_str())).expect("the b.txt row draws");
    cx.simulate_click(row.center(), Modifiers::none());
    cx.run_until_parked();
    let asked = cx.update(|_, cx| cx.try_global::<files::Reveal>().cloned()).unwrap();
    assert_eq!((asked.path, asked.line), (b.clone(), None), "a file opened from the tree asks for no line");

    let tree = screen.read_with(cx, |s, _| s.tree().cloned().unwrap());
    let shown = names(&screen, cx);
    let field = cx.debug_bounds("files-search").expect("the search field draws");
    cx.simulate_click(field.center(), Modifiers::none());
    cx.simulate_input("a10");
    cx.run_until_parked();
    let found: Vec<String> = names(&screen, cx).into_iter().map(|(n, _)| n).collect();
    assert_eq!(found, ["src", "a10.ts"], "Search files hides the rows that do not match");
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert_eq!(names(&screen, cx), shown, "Escape clears the search");
    let field = cx.debug_bounds("files-search").expect("the search field draws");
    cx.simulate_click(field.center(), Modifiers::none());
    cx.simulate_input("many");
    until(cx, "the folder the search matches read and shown open", |cx| names(&screen, cx).iter().any(|(n, d)| n == "f29.txt" && *d == 1));
    cx.simulate_input("/f29.");
    cx.run_until_parked();
    let found: Vec<String> = names(&screen, cx).into_iter().map(|(n, _)| n).collect();
    assert_eq!(found, ["many", "f29.txt"], "and the matches inside it narrow as the search goes on");
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();

    let row = cx.debug_bounds(Box::leak(format!("tree-{b}").into_boxed_str())).expect("the b.txt row draws");
    cx.simulate_mouse_down(row.center(), gpui_kit::MouseButton::Right, Modifiers::none());
    cx.run_until_parked();
    let asked: Vec<String> = tree.read_with(cx, |t, _| t.menu_asked().iter().map(ToString::to_string).collect());
    assert_eq!(asked, ["Open file", "Show in Changes", "Copy path"], "a right-click asks for the row's menu");
    // On the Mac AppKit draws the menu, which the test platform cannot drive.
    if cfg!(target_os = "macos") {
        return;
    }
    assert!(tree.read_with(cx, |t, _| t.menu_open()), "a right-click opens the row's menu");
    cx.simulate_keystrokes("down down enter");
    cx.run_until_parked();
    let copied = cx.update(|_, cx| cx.read_from_clipboard().and_then(|c| c.text()));
    assert_eq!(copied.as_deref(), Some(b.as_str()), "Copy path, the menu's third row, copies the file's path");
    println!("tree: {top:?}, then src: {under:?}");
}

#[gpui_kit::test]
fn the_tree_follows_the_agents_folder_until_pinned(cx: &mut TestAppContext) {
    let Some(host) = Host::start() else { return };
    let (screen, cx) = mount(&host, "files", cx);
    let folder = |cx: &mut VisualTestContext| screen.read_with(cx, |s, cx| s.tree().unwrap().read(cx).folder().map(str::to_owned));
    until(cx, "the tree listing the project's folder", |cx| !names(&screen, cx).is_empty());
    host.turn_in(&host.at("src"));
    until(cx, "the tree moving to src with the agent", |cx| folder(cx).as_deref() == Some(host.at("src").as_str()));
    until(cx, "src listed", |cx| names(&screen, cx).iter().any(|(n, _)| n == "a2.ts"));
    let kept: Vec<String> = screen.read_with(cx, |s, cx| s.tree().unwrap().read(cx).levels().keys().cloned().collect());
    assert!(kept.iter().all(|dir| dir.starts_with(&host.at("src"))), "the listings outside the new folder go: {kept:?}");
    let pin = cx.debug_bounds("files-pin-button").expect("the pin draws");
    cx.simulate_click(pin.center(), Modifiers::none());
    cx.run_until_parked();
    host.turn_in(&host.at("notes"));
    host.turn_in(&host.at("notes"));
    cx.run_until_parked();
    std::thread::sleep(Duration::from_millis(1500));
    cx.run_until_parked();
    assert_eq!(folder(cx).as_deref(), Some(host.at("src").as_str()), "pinned, the tree stays when the agent moves");
    let pin = cx.debug_bounds("files-pin-button").expect("the pin draws");
    cx.simulate_click(pin.center(), Modifiers::none());
    until(cx, "unpinned, the tree going where the agent is", |cx| folder(cx).as_deref() == Some(host.at("notes").as_str()));
    cx.simulate_keystrokes("backspace");
    cx.run_until_parked();
    assert_eq!(folder(cx).as_deref(), Some(host.at("").trim_end_matches('/')), "Backspace goes up a folder");
    let home = host.server.dir.path().canonicalize().unwrap().to_string_lossy().into_owned();
    cx.simulate_keystrokes("backspace");
    cx.run_until_parked();
    assert_eq!(folder(cx), Some(home.clone()), "up again to the daemon's home, the root the project sits in");
    cx.simulate_keystrokes("backspace");
    cx.run_until_parked();
    assert_eq!(folder(cx), Some(home), "and no further, at the root's edge");
    println!("followed the agent to src, stayed pinned, then followed to notes");
}

#[gpui_kit::test]
fn the_tree_opens_on_the_agents_last_shell_from_its_history_then_follows_each_cd_as_it_lands(cx: &mut TestAppContext) {
    let Some(host) = Host::start() else { return };
    host.cd_in(&host.at("src"));
    host.cd_in(&host.at("notes"));
    host.until_history_moves_shell(2);
    let (screen, cx) = mount(&host, "files", cx);
    let folder = |cx: &mut VisualTestContext| screen.read_with(cx, |s, cx| s.tree().unwrap().read(cx).folder().map(str::to_owned));
    until(cx, "the tree opening on notes, the last folder the history says the agent's shell went to", |cx| {
        folder(cx).as_deref() == Some(host.at("notes").as_str())
    });
    until(cx, "notes listed", |cx| names(&screen, cx).iter().any(|(n, _)| n == "plan.md"));
    host.cd_in(&host.at("src"));
    until(cx, "the tree moving to src as the agent's cd lands", |cx| folder(cx).as_deref() == Some(host.at("src").as_str()));
    println!("opened on the agent's last shell from its history, then followed its cd to src");
}

#[gpui_kit::test]
fn a_threads_shell_goes_with_its_link_and_the_next_link_reads_it_off_the_history(cx: &mut TestAppContext) {
    let Some(host) = Host::start() else { return };
    cx.executor().allow_parking();
    let link = cx.update(|cx| DaemonLink::of(&host.shell(), &host.workspace_id, cx));
    host.cd_in(&host.at("src"));
    let end = Instant::now() + WAIT;
    while link.read_with(cx, |l, _| l.shell(&host.thread_id)).as_deref() != Some(host.at("src").as_str()) {
        assert!(Instant::now() < end, "the link hearing the agent's cd within {} s", WAIT.as_secs());
        cx.run_until_parked();
        std::thread::sleep(Duration::from_millis(100));
    }
    let first = link.entity_id();
    drop(link);
    cx.run_until_parked();
    let again = cx.update(|cx| DaemonLink::of(&host.shell(), &host.workspace_id, cx));
    assert_ne!(again.entity_id(), first, "the link went with its last holder");
    assert_eq!(again.read_with(cx, |l, _| l.shell(&host.thread_id)), None, "the thread's shell went with the link that heard it");
    again.read_with(cx, |l, _| l.follow_shell(&host.thread_id));
    let end = Instant::now() + WAIT;
    while again.read_with(cx, |l, _| l.shell(&host.thread_id)).as_deref() != Some(host.at("src").as_str()) {
        assert!(Instant::now() < end, "the next link reading the shell off the history within {} s", WAIT.as_secs());
        cx.run_until_parked();
        std::thread::sleep(Duration::from_millis(100));
    }
}

#[gpui_kit::test]
fn alt_up_goes_up_one_folder(cx: &mut TestAppContext) {
    let Some(host) = Host::start() else { return };
    let (screen, cx) = mount(&host, "files", cx);
    let folder = |cx: &mut VisualTestContext| screen.read_with(cx, |s, cx| s.tree().unwrap().read(cx).folder().map(str::to_owned));
    until(cx, "the tree listing the project's folder", |cx| !names(&screen, cx).is_empty());
    let tree = screen.read_with(cx, |s, _| s.tree().cloned().unwrap());
    tree.update(cx, |t, cx| t.go(&host.at("notes/deep"), cx));
    until(cx, "the tree on notes/deep", |cx| folder(cx).as_deref() == Some(host.at("notes/deep").as_str()));
    cx.simulate_keystrokes("alt-up");
    cx.run_until_parked();
    assert_eq!(folder(cx).as_deref(), Some(host.at("notes").as_str()), "Alt+Up goes up one folder");
}

#[gpui_kit::test]
fn a_pin_outlives_the_panes_and_the_link_that_held_it(cx: &mut TestAppContext) {
    let Some(host) = Host::start() else { return };
    cx.executor().allow_parking();
    let link = cx.update(|cx| DaemonLink::of(&host.shell(), &host.workspace_id, cx));
    let src = host.at("src");
    link.update(cx, |link, cx| link.pin(&host.thread_id, &src, cx));
    let first = link.entity_id();
    drop(link);
    cx.run_until_parked();
    let again = cx.update(|cx| DaemonLink::of(&host.shell(), &host.workspace_id, cx));
    assert_ne!(again.entity_id(), first, "the link went with its last holder");
    let pinned = again.read_with(cx, |link, _| link.pinned(&host.thread_id).map(|p| p.to_string()));
    assert_eq!(pinned, Some(src), "the thread's pin stays, as root.ts keeps it");
}

#[gpui_kit::test]
fn a_pin_stands_when_its_thread_shows_again(cx: &mut TestAppContext) {
    let Some(host) = Host::start() else { return };
    cx.executor().allow_parking();
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
    });
    let empty = cx.new(|_| Blank);
    let (_, cx) = cx.add_window_view(|window, cx| Root::new(empty, window, cx));
    let open = |cx: &mut VisualTestContext| {
        let (shell, opened) = (host.shell(), host.opened("files"));
        cx.update(|window, cx| cx.new(|cx| FilesScreen::new(shell, opened, window, cx)))
    };
    let folder = |screen: &Entity<FilesScreen>, cx: &mut VisualTestContext| {
        screen.read_with(cx, |s, cx| s.tree().unwrap().read(cx).folder().map(str::to_owned))
    };
    let first = open(cx);
    until(cx, "the tree rooted at the thread's folder", |cx| folder(&first, cx).is_some());
    let src = host.at("src");
    let tree = first.read_with(cx, |s, _| s.tree().cloned().unwrap());
    tree.update(cx, |t, cx| t.go(&src, cx));
    until(cx, "the tree pinned on src", |cx| folder(&first, cx).as_deref() == Some(src.as_str()));
    let link = cx.update(|_, cx| DaemonLink::of(&host.shell(), &host.workspace_id, cx)).downgrade();
    // The panel drops a thread's panes while another thread shows, and builds them again when it comes back.
    drop((tree, first));
    until(cx, "the link let go with its last pane", |_| link.upgrade().is_none());
    let again = open(cx);
    until(cx, "the tree rooted again", |cx| folder(&again, cx).is_some());
    assert_eq!(folder(&again, cx).as_deref(), Some(src.as_str()), "the pin stands, as the web app's root store keeps it");
}

#[gpui_kit::test]
fn a_refused_workspace_read_is_said_on_a_relative_file_tab(cx: &mut TestAppContext) {
    let Some(host) = Host::start() else { return };
    let opened = Opened { workspace_id: Some("w-none".into()), thread_id: None, tab: Some("file:b.txt".into()), arg: None };
    cx.executor().allow_parking();
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
    });
    let shell = host.shell();
    let mut made = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let screen = cx.new(|cx| FilesScreen::new(shell, opened, window, cx));
        made = Some(screen.clone());
        Root::new(screen, window, cx)
    });
    let file = made.unwrap().read_with(cx, |s, _| s.file().cloned().unwrap());
    until(cx, "the host's refusal said", |cx| file.read_with(cx, |f, _| matches!(f.read(), Read::Error { .. })));
    let said = file.read_with(cx, |f, _| match f.read() {
        Read::Error { message, .. } => message.clone(),
        _ => String::new(),
    });
    assert!(said.contains("w-none"), "the host's own sentence: {said}");
    cx.run_until_parked();
    assert!(cx.debug_bounds("file-error").is_some(), "drawn in place of the spinner");
    println!("refused: {said}");
}

#[gpui_kit::test]
fn a_file_tab_reads_its_file_and_lights_the_line_it_opened_at(cx: &mut TestAppContext) {
    let Some(host) = Host::start() else { return };
    let path = host.at("b.txt");
    cx.update(|cx| files::open_file(&host.workspace_id, &path, Some(3), cx));
    let (screen, cx) = mount(&host, &format!("file:{path}"), cx);
    let file = screen.read_with(cx, |s, _| s.file().cloned().unwrap());
    until(cx, "the file read", |cx| file.read_with(cx, |f, _| matches!(f.read(), Read::Ready(_))));
    let (reply, lit) = file.read_with(cx, |f, cx| (*f.read().reply().unwrap(), f.code().read(cx).lit()));
    assert_eq!((reply.shown, reply.size, reply.binary), (19, 19, false), "the whole small text file");
    assert_eq!(lit, Some(3), "the line it was opened at is lit");
    assert!(cx.debug_bounds("file-code").is_some(), "the code view draws");
    assert!(cx.debug_bounds("file-truncated").is_none(), "a small file carries no note");
    let title = cx.update(|window, cx| FilesScreen::parts(&screen, window, cx).title.map(|t| t(cx).to_string()));
    assert_eq!(title.as_deref(), Some("b.txt"), "the tab is titled by the file's name");
    println!("file tab: {} bytes, lit {lit:?}, titled {title:?}", reply.shown);
}

#[gpui_kit::test]
fn a_missing_file_says_why_and_markdown_at_a_line_shows_its_source(cx: &mut TestAppContext) {
    let Some(host) = Host::start() else { return };
    let plan = host.at("notes/plan.md");
    cx.update(|cx| files::open_file(&host.workspace_id, &plan, Some(2), cx));
    let (screen, cx) = mount(&host, &format!("file:{plan}"), cx);
    let file = screen.read_with(cx, |s, _| s.file().cloned().unwrap());
    until(cx, "the markdown read", |cx| file.read_with(cx, |f, _| matches!(f.read(), Read::Ready(_))));
    let (rendered, lit) = file.read_with(cx, |f, cx| (f.rendered(cx), f.code().read(cx).lit()));
    assert_eq!((rendered, lit), (false, Some(2)), "markdown opened at a line shows its source with the line lit");
    assert!(cx.debug_bounds("file-code").is_some() && cx.debug_bounds("file-markdown-toggle").is_none(), "and no toggle");
    let missing = host.at("gone.txt");
    let opened = host.opened(&format!("file:{missing}"));
    let shell = host.shell();
    let gone = cx.update(|window, cx| cx.new(|cx| FilesScreen::new(shell, opened, window, cx)));
    let gone = gone.read_with(cx, |s, _| s.file().cloned().unwrap());
    until(cx, "the refusal of a missing file", |cx| gone.read_with(cx, |f, _| matches!(f.read(), Read::Error { .. })));
    let said = gone.read_with(cx, |f, _| match f.read() {
        Read::Error { message, last: None } => message.clone(),
        _ => String::new(),
    });
    assert!(!said.is_empty(), "the daemon's sentence, with nothing read before");
    println!("markdown at a line: source, lit {lit:?}; missing file: {said}");
}

#[gpui_kit::test]
fn a_file_over_two_megabytes_shows_the_note(cx: &mut TestAppContext) {
    let Some(host) = Host::start() else { return };
    let (screen, cx) = mount(&host, "file:big.txt", cx);
    let file = screen.read_with(cx, |s, _| s.file().cloned().unwrap());
    until(cx, "the big file read", |cx| file.read_with(cx, |f, _| matches!(f.read(), Read::Ready(_))));
    let (reply, read_at) = file.read_with(cx, |f, _| (f.read().reply().cloned().unwrap(), f.path().to_owned()));
    assert_eq!(read_at, host.at("big.txt"), "a route's relative path is read in the thread's project");
    assert!(reply.truncated && reply.size == 3 * 1024 * 1024, "the daemon cut the read: {} of {}", reply.shown, reply.size);
    cx.run_until_parked();
    assert!(cx.debug_bounds("file-truncated").is_some(), "the 2 MB note draws over the cut file");
    println!("big file: {} of {} bytes, the note drawn", reply.shown, reply.size);
}

fn finder_in<'a>(host: &Host, mode: Mode, cx: &'a mut TestAppContext) -> (Entity<Finder>, &'a mut VisualTestContext) {
    cx.executor().allow_parking();
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
    });
    let empty = cx.new(|_| Blank);
    let (_, cx) = cx.add_window_view(|window, cx| Root::new(empty, window, cx));
    cx.simulate_resize(size(px(1000.), px(800.)));
    let shell = host.shell();
    let finder = cx.update(|window, cx| finder::open(mode, &shell, &host.workspace_id, window, cx));
    cx.run_until_parked();
    (finder, cx)
}

struct Blank;

/// An empty page with the dialogs' layer drawn last, as the shell draws it.
impl gpui_kit::Render for Blank {
    fn render(&mut self, window: &mut gpui_kit::Window, cx: &mut gpui_kit::Context<Self>) -> impl gpui_kit::IntoElement {
        use gpui_kit::{ParentElement as _, Styled as _};
        gpui_kit::div().size_full().children(layer::draw(window, cx))
    }
}

fn hits(finder: &Entity<Finder>, cx: &mut VisualTestContext) -> Option<(Vec<String>, bool)> {
    finder.read_with(cx, |f, _| match f.answer() {
        Answer::Ready { hits, truncated } => Some((hits.iter().map(|h| format!("{}|{}", h.title, h.description)).collect(), *truncated)),
        _ => None,
    })
}

/// Types into the finder's field as a person does: what it held goes, then the words go in key by key.
fn type_in(text: &str, cx: &mut VisualTestContext) {
    let all = if cfg!(target_os = "macos") { "cmd-a backspace" } else { "ctrl-a backspace" };
    cx.simulate_keystrokes(all);
    cx.simulate_input(text);
}

#[gpui_kit::test]
fn the_finder_finds_a_file_by_name_and_closes_on_escape_not_on_a_press_inside(cx: &mut TestAppContext) {
    let Some(host) = Host::start() else { return };
    let (finder, cx) = finder_in(&host, Mode::Files, cx);
    type_in("zzzq", cx);
    until(cx, "no file matching", |cx| finder.read_with(cx, |f, cx| f.empty_line(cx) == words::FILES_NONE));
    type_in("plan", cx);
    until(cx, "the finder's file hits", |cx| hits(&finder, cx).is_some_and(|(h, _)| !h.is_empty()));
    let (found, _) = hits(&finder, cx).unwrap();
    assert_eq!(found.first().map(String::as_str), Some("plan.md|notes"), "the name match ranks first: {found:?}");
    assert!(cx.update(|_, cx| layer::is_open(finder::FINDER, cx)), "the finder is open");
    let foot = cx.debug_bounds("finder-foot").expect("the Open foot draws");
    cx.simulate_click(foot.center(), Modifiers::none());
    cx.run_until_parked();
    assert!(cx.update(|_, cx| layer::is_open(finder::FINDER, cx)), "a press inside the finder keeps it open");
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert!(!cx.update(|_, cx| layer::is_open(finder::FINDER, cx)), "Escape closes the finder");

    let shell = host.shell();
    let finder = cx.update(|window, cx| finder::open(Mode::Files, &shell, &host.workspace_id, window, cx));
    cx.run_until_parked();
    type_in("f1", cx);
    until(cx, "twenty hits or more", |cx| hits(&finder, cx).is_some_and(|(h, _)| h.len() >= 20));
    cx.run_until_parked();
    let (list, foot) = (cx.debug_bounds("finder-hits").expect("the hits draw"), cx.debug_bounds("finder-foot").expect("the foot draws"));
    assert!(list.bottom() <= foot.top(), "the list ends above the foot: list {list:?}, foot {foot:?}");
    let popup = cx.debug_bounds("file-finder-Find a file").expect("the finder's popup draws");
    assert!((popup.top() - px(80.)).abs() < px(1.), "the finder stands a tenth of the 800 px window down, as today's: {popup:?}");
    cx.simulate_click(gpui_kit::point(px(30.), px(700.)), Modifiers::none());
    cx.run_until_parked();
    assert!(!cx.update(|_, cx| layer::is_open(finder::FINDER, cx)), "a press on the scrim beside it closes the finder");
    println!("finder by name: {found:?}");
}

#[gpui_kit::test]
fn the_ssh_question_stands_on_the_dialogs_layer_and_closes_on_escape_or_the_scrim(cx: &mut TestAppContext) {
    let Some(host) = Host::start() else { return };
    cx.executor().allow_parking();
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
    });
    let empty = cx.new(|_| Blank);
    let (_, cx) = cx.add_window_view(|window, cx| Root::new(empty, window, cx));
    cx.simulate_resize(size(px(1000.), px(800.)));
    let ran = std::rc::Rc::new(std::cell::Cell::new(false));
    let ask = |cx: &mut VisualTestContext| {
        let ran = ran.clone();
        let again: ui::Run = std::rc::Rc::new(move |_, _| ran.set(true));
        cx.update(|window, cx| open_split::ask_ssh(host.shell(), again, window, cx));
        cx.run_until_parked();
    };
    ask(cx);
    assert!(cx.update(|_, cx| layer::is_open(open_split::SSH_DIALOG, cx)), "the question stands");
    let question = cx.debug_bounds("editor-ssh").expect("the question draws");
    cx.simulate_click(question.center(), Modifiers::none());
    cx.run_until_parked();
    assert!(cx.update(|_, cx| layer::is_open(open_split::SSH_DIALOG, cx)), "a press inside keeps it");
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert!(!cx.update(|_, cx| layer::is_open(open_split::SSH_DIALOG, cx)), "Escape closes it");
    ask(cx);
    cx.simulate_click(gpui_kit::point(px(30.), px(700.)), Modifiers::none());
    cx.run_until_parked();
    assert!(!cx.update(|_, cx| layer::is_open(open_split::SSH_DIALOG, cx)), "a press on the scrim closes it");
    assert!(!ran.get(), "closing it adds nothing and opens nothing");
}

#[gpui_kit::test]
fn the_finder_finds_lines_by_content_and_stops_at_five_hundred(cx: &mut TestAppContext) {
    let Some(host) = Host::start() else { return };
    let (finder, cx) = finder_in(&host, Mode::Text, cx);
    until(cx, "the workspace read", |cx| finder.read_with(cx, |f, cx| f.empty_line(cx) == words::TYPE_WORD));
    type_in("zzzq", cx);
    until(cx, "no line holding it", |cx| finder.read_with(cx, |f, cx| f.empty_line(cx) == words::TEXT_NONE));
    type_in("second step", cx);
    until(cx, "the finder's line hits", |cx| hits(&finder, cx).is_some_and(|(h, _)| !h.is_empty()));
    let (found, more) = hits(&finder, cx).unwrap();
    assert_eq!(found, ["second step|notes/plan.md:4"], "the one line holding the words");
    assert!(!more);
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    let asked = cx.update(|_, cx| cx.try_global::<files::Reveal>().cloned()).expect("Enter asks for the file");
    assert_eq!((asked.path, asked.line), (host.at("notes/plan.md"), Some(4)), "Enter opens the hit's file at its line");
    assert!(!cx.update(|_, cx| layer::is_open(finder::FINDER, cx)), "and closes the finder");
    let shell = host.shell();
    let finder = cx.update(|window, cx| finder::open(Mode::Text, &shell, &host.workspace_id, window, cx));
    cx.run_until_parked();
    type_in("needle", cx);
    until(cx, "the finder's needle hits", |cx| hits(&finder, cx).is_some_and(|(h, _)| h.len() > 1));
    let (found, more) = hits(&finder, cx).unwrap();
    assert_eq!(found.len(), 500, "the daemon's 500 hits");
    assert!(more, "the reply says more lines match");
    cx.run_until_parked();
    assert!(cx.debug_bounds("finder-more").is_some(), "the more-match note draws");
    println!("finder by content: {} hits, more: {more}", found.len());
}

#[gpui_kit::test]
fn the_file_tab_head_is_28_px_with_room_under_it_inline_and_its_crumbs_are_the_trees_11_px(cx: &mut TestAppContext) {
    let Some(host) = Host::start() else { return };
    let path = host.at("b.txt");
    let (screen, cx) = mount(&host, &format!("file:{path}"), cx);
    let file = screen.read_with(cx, |s, _| s.file().cloned().unwrap());
    until(cx, "the file read", |cx| file.read_with(cx, |f, _| matches!(f.read(), Read::Ready(_))));
    let crumb: &'static str = Box::leak(format!("crumb-{path}").into_boxed_str());
    let gaps = |cx: &mut VisualTestContext| {
        cx.run_until_parked();
        let head = cx.debug_bounds("file-head").expect("the head draws");
        let code = cx.debug_bounds("file-code").expect("the code draws");
        let crumb = cx.debug_bounds(crumb).expect("the file's crumb draws");
        (head.size.height, code.top() - head.bottom(), crumb.size.height)
    };
    assert_eq!(gaps(cx), (px(40.), px(8.), px(16.)), "as a sheet: h-10, then the code's own 8 px, crumbs 11 px on 16");
    cx.simulate_resize(size(px(1200.), px(700.)));
    assert_eq!(gaps(cx), (px(28.), px(20.), px(16.)), "inline: h-7 and mb-3, then the code's own 8 px");
    println!("file tab head: sheet and inline gaps as today's");
}

#[gpui_kit::test]
fn the_finders_first_hit_runs_on_enter_but_stays_unlit_until_a_key_steps(cx: &mut TestAppContext) {
    let Some(host) = Host::start() else { return };
    let (finder, cx) = finder_in(&host, Mode::Files, cx);
    type_in("a", cx);
    until(cx, "more than one hit", |cx| hits(&finder, cx).is_some_and(|(h, _)| h.len() > 1));
    cx.run_until_parked();
    assert_eq!(finder.read_with(cx, |f, _| f.lit()), None, "no hit is lit as the hits land");
    let (row, foot) = (cx.debug_bounds("finder-hit-0").expect("a hit draws"), cx.debug_bounds("finder-foot").expect("the foot draws"));
    assert_eq!((row.size.height, foot.size.height), (px(48.), px(40.)), "a hit's py-1.5 over 36 px; the foot's py-2.5 over 20 px");
    cx.simulate_keystrokes("down");
    assert_eq!(finder.read_with(cx, |f, _| f.lit()), Some(1), "Down steps on from the first hit, which Enter would run");
    cx.simulate_keystrokes("up");
    assert_eq!(finder.read_with(cx, |f, _| f.lit()), Some(0));
    type_in("al", cx);
    assert_eq!(finder.read_with(cx, |f, _| f.lit()), None, "typing clears the lit hit");
}

#[gpui_kit::test]
fn the_two_megabyte_note_stands_on_its_11_px_line_and_one_and_a_half_over_the_code(cx: &mut TestAppContext) {
    let Some(host) = Host::start() else { return };
    let (screen, cx) = mount(&host, "file:big.txt", cx);
    let file = screen.read_with(cx, |s, _| s.file().cloned().unwrap());
    until(cx, "the big file read", |cx| file.read_with(cx, |f, _| matches!(f.read(), Read::Ready(_))));
    cx.run_until_parked();
    let (note, code) = (cx.debug_bounds("file-truncated").expect("the note draws"), cx.debug_bounds("file-code").expect("the code draws"));
    assert_eq!((note.size.height, code.top() - note.bottom()), (px(28.5), px(8.)), "py-1.5 around 11 px at 1.5, then the code's 8 px");
}
