// SPDX-License-Identifier: AGPL-3.0-only
//! A project's image on the host: only the one PNG shape the window makes is taken, each rule broken is refused in two
//! halves, what is kept is IHDR, IDAT and IEND alone under the state folder by its SHA-256, and the file goes once no
//! project and no recipe names it. Ported from packages/runtime/test/project-icons.test.ts at 65f6b89c7: the two
//! `keptProjectIcon` cases whole, the three whole-host cases at the area's seam. Every PNG and every sentence is in
//! tests/fixtures/images.json, which oracle/images.oracle.ts wrote by running the TS checks on each file.
#[path = "support/base.rs"]
mod base;

use std::cell::RefCell;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use async_trait::async_trait;
use base::{run, TestIds};
use futures::channel::oneshot;
use indexmap::{IndexMap, IndexSet};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use wsp_account::projects::images::{kept_project_icon, node_base64, project_icons, project_icons_area, ProjectIconsArea, ProjectIconsCtx};
use wsp_core::ids::Ids;
use wsp_core::types::wiring::{RecipeShelf, ResolvedRecipe, ShelvedRecipe};
use wsp_wire::types::recipe_file::RecipeFile;
use wsp_wire::views::preferences::Preferences;
use wsp_words::exit::{HostError, Kind};

const AGAIN: &str = "Pick it again; if it is refused again, pick another file.";

fn fixture() -> Value {
    let text = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/images.json")).unwrap();
    serde_json::from_str(&text).unwrap()
}

fn case(name: &str) -> Value {
    fixture()["cases"].as_array().unwrap().iter().find(|c| c["name"] == name).unwrap_or_else(|| panic!("no case {name}")).clone()
}

fn file_of(c: &Value) -> Vec<u8> {
    hex::decode(c["file"].as_str().unwrap()).unwrap()
}

fn types_of(file: &[u8]) -> Vec<String> {
    let mut at = 8;
    let mut types = vec![];
    while at < file.len() {
        let len = u32::from_be_bytes(file[at..at + 4].try_into().unwrap()) as usize;
        types.push(String::from_utf8_lossy(&file[at + 4..at + 8]).into_owned());
        at += 12 + len;
    }
    types
}

fn sha(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn good() -> Vec<u8> {
    file_of(&case("good rgba"))
}

fn kept_hash(name: &str) -> String {
    case(name)["sha256"].as_str().unwrap().to_owned()
}

mod the_png_the_host_keeps_for_a_projects_image {
    use super::*;

    #[test]
    fn takes_a_128_px_rgba_or_rgb_png_and_keeps_its_header_pixels_and_end_alone() {
        let sent = file_of(&case("text and phys dropped"));
        assert_eq!(types_of(&sent), vec!["IHDR", "tEXt", "pHYs", "IDAT", "IEND"]);
        let kept = kept_project_icon(&sent).unwrap();
        assert_eq!(types_of(&kept), vec!["IHDR", "IDAT", "IEND"]);
        assert_eq!(hex::encode(&kept), case("text and phys dropped")["kept"].as_str().unwrap());
        assert!(!kept.windows(9).any(|w| w == b"Acme Road"));
        let rgb = kept_project_icon(&file_of(&case("good rgb"))).unwrap();
        assert_eq!(types_of(&rgb), vec!["IHDR", "IDAT", "IEND"]);
        assert_eq!(hex::encode(&rgb), case("good rgb")["kept"].as_str().unwrap());
        // Two IDAT chunks around another chunk: both kept whole, in order, the other dropped.
        let two = kept_project_icon(&file_of(&case("two idat chunks around another"))).unwrap();
        assert_eq!(types_of(&two), vec!["IHDR", "IDAT", "IDAT", "IEND"]);
        assert_eq!(hex::encode(&two), case("two idat chunks around another")["kept"].as_str().unwrap());
    }

    #[test]
    fn refuses_each_file_that_breaks_a_rule_naming_the_rule_and_asking_again() {
        let mut refused = 0;
        for c in fixture()["cases"].as_array().unwrap() {
            let Some(said) = c["said"].as_str() else { continue };
            let e = kept_project_icon(&file_of(c)).expect_err(c["name"].as_str().unwrap());
            assert_eq!(e.message, said, "{}", c["name"]);
            assert!(e.message.starts_with("The host did not take this image: "), "{}", c["name"]);
            assert_eq!(e.fix.as_deref(), Some(AGAIN), "{}", c["name"]);
            assert_eq!(e.kind, Some(Kind::Usage), "{}", c["name"]);
            refused += 1;
        }
        assert_eq!(refused, 36);
    }

    #[test]
    fn the_checks_run_in_the_ts_order() {
        // Size before depth, a checksum before a missing header, the signature before anything.
        for (name, why) in [
            ("16 bits and 64 px", "it is 64 by 64 pixels, not 128 by 128"),
            ("checksum before a missing header", "its tEXt chunk fails its checksum"),
            ("signature alone", "it does not open with its header"),
            ("seven bytes", "it is not a PNG"),
            ("rgba pixels under an rgb header", "its pixels inflate past the 49280 bytes 128 px holds"),
        ] {
            let e = kept_project_icon(&file_of(&case(name))).unwrap_err();
            assert!(e.message.contains(why), "{name}: {}", e.message);
        }
    }

    #[test]
    fn inflate_cap_is_one_byte_past_the_rows() {
        let one = kept_project_icon(&file_of(&case("pixels one past"))).unwrap_err();
        assert_eq!(one.message, case("pixels one past")["said"].as_str().unwrap());
        assert!(one.message.contains("inflate to 65665 bytes, not 65664"));
        let two = kept_project_icon(&file_of(&case("pixels two past"))).unwrap_err();
        assert!(two.message.contains("inflate past the 65664 bytes 128 px holds"));
        let bomb = kept_project_icon(&file_of(&case("a deflate bomb"))).unwrap_err();
        assert_eq!(bomb.message, two.message);
    }

    #[test]
    fn exactly_81920_bytes_passes_the_size_check() {
        let at = file_of(&case("exactly 81920 bytes"));
        assert_eq!(at.len(), 81920);
        assert_eq!(hex::encode(kept_project_icon(&at).unwrap()), case("exactly 81920 bytes")["kept"].as_str().unwrap());
        let past = file_of(&case("81921 bytes"));
        assert_eq!(past.len(), 81921);
        assert!(kept_project_icon(&past).unwrap_err().message.contains("it is 81921 bytes, over the 81920 an image may be"));
    }

    #[test]
    fn bytes_after_iend_are_ignored() {
        let base = kept_project_icon(&good()).unwrap();
        for name in ["bytes after iend", "a chunk after iend"] {
            assert_eq!(kept_project_icon(&file_of(&case(name))).unwrap(), base, "{name}");
        }
        let filters = kept_project_icon(&file_of(&case("filter 4 on every row"))).unwrap();
        assert_eq!(sha(&filters), kept_hash("filter 4 on every row"));
    }

    #[test]
    fn set_decodes_base64_as_node_does() {
        for b in fixture()["base64"].as_array().unwrap() {
            assert_eq!(hex::encode(node_base64(b["text"].as_str().unwrap())), b["hex"].as_str().unwrap(), "{}", b["text"]);
        }
    }
}

fn icons_dir() -> (tempfile::TempDir, PathBuf) {
    let root = tempfile::tempdir().unwrap();
    let dir = root.path().join("state").join("project-icons");
    (root, dir)
}

fn listing(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> =
        std::fs::read_dir(dir).map(|r| r.map(|e| e.unwrap().file_name().into_string().unwrap()).collect()).unwrap_or_default();
    names.sort();
    names
}

mod the_file_store {
    use super::*;

    #[test]
    fn keep_writes_the_hash_file_at_0600_under_a_0700_folder_and_leaves_no_part() {
        let (_root, dir) = icons_dir();
        let d = dir.clone();
        let icons = project_icons(Rc::new(move || d.clone()), TestIds::new());
        let hash = icons.keep(&file_of(&case("text and phys dropped"))).unwrap();
        assert_eq!(hash, kept_hash("text and phys dropped"));
        let file = dir.join(format!("{hash}.png"));
        assert_eq!(std::fs::metadata(&file).unwrap().permissions().mode() & 0o777, 0o600);
        assert_eq!(std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777, 0o700);
        assert_eq!(sha(&std::fs::read(&file).unwrap()), hash);
        assert_eq!(listing(&dir), vec![format!("{hash}.png")]);
    }

    #[test]
    fn read_answers_each_hash_in_order_from_disk_null_for_a_bad_hash_or_missing_file() {
        let (_root, dir) = icons_dir();
        let d = dir.clone();
        let icons = project_icons(Rc::new(move || d.clone()), TestIds::new());
        let hash = icons.keep(&good()).unwrap();
        let other = "0".repeat(64);
        let read = icons.read(&[other.clone(), hash.clone(), "../etc/passwd".to_owned(), hash.clone()]);
        assert_eq!(read.keys().cloned().collect::<Vec<_>>(), vec![other.clone(), hash.clone(), "../etc/passwd".to_owned()]);
        assert_eq!(read[&other], None);
        assert_eq!(read["../etc/passwd"], None);
        let url = read[&hash].clone().unwrap();
        use base64::Engine;
        let bytes = base64::engine::general_purpose::STANDARD.decode(url.strip_prefix("data:image/png;base64,").unwrap()).unwrap();
        assert_eq!(bytes, std::fs::read(dir.join(format!("{hash}.png"))).unwrap());
        // Read from disk at each ask: a file gone by hand reads as none at the next.
        std::fs::remove_file(dir.join(format!("{hash}.png"))).unwrap();
        assert_eq!(icons.read(std::slice::from_ref(&hash))[&hash], None);
    }

    #[test]
    fn holds_reads_the_listing_now() {
        let (_root, dir) = icons_dir();
        let d = dir.clone();
        let icons = project_icons(Rc::new(move || d.clone()), TestIds::new());
        assert!(!icons.holds(&"a".repeat(64)), "a folder not made yet holds nothing");
        let hash = icons.keep(&good()).unwrap();
        assert!(icons.holds(&hash));
        std::fs::remove_file(dir.join(format!("{hash}.png"))).unwrap();
        assert!(!icons.holds(&hash));
    }

    #[test]
    fn sweep_deletes_unnamed_hashes_and_every_part_and_leaves_other_names() {
        let (_root, dir) = icons_dir();
        let d = dir.clone();
        let icons = project_icons(Rc::new(move || d.clone()), TestIds::new());
        let a = icons.keep(&good()).unwrap();
        let b = icons.keep(&file_of(&case("good rgb"))).unwrap();
        std::fs::write(dir.join(format!(".{a}.deadbeef.part")), b"cut").unwrap();
        std::fs::write(dir.join("notes.txt"), b"mine").unwrap();
        std::fs::write(dir.join("ABC.png"), b"not a hash").unwrap();
        icons.sweep(&IndexSet::from([a.clone()]));
        let mut want = vec![format!("{a}.png"), "ABC.png".to_owned(), "notes.txt".to_owned()];
        want.sort();
        assert_eq!(listing(&dir), want);
        assert!(!dir.join(format!("{b}.png")).exists());
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The area over a fake context

fn prefs(icons: Value, looks: Value) -> Preferences {
    serde_json::from_value(json!({"theme":"dark","lightTheme":"paper","darkTheme":"graphite","sidebarMode":"list","terminalSize":"app","terminalZoom":{},"access":{},"projectLook":looks,"projectIcon":icons,"computerLook":{},"serverIcons":true,"agentVersions":true,"usageLogs":true,"productUsage":true,"keepAwake":true,"transparency":true,"projectOrder":[],"keybindings":{},"appFont":"","codeFont":"","agentDefaults":{},"projectDefaults":{},"sendWith":"enter","midTurn":"queue","notifyNeeds":"notify-sound","notifyDone":"notify","planAlerts":true,"settleAfter":"2h","askDelete":true,"onQuit":"ask","newThreadIn":"current","labs":false}))
        .expect("preferences read")
}

fn icons_json(p: &Preferences) -> Value {
    serde_json::to_value(&p.project_icon).unwrap()
}

struct Shelf {
    listed: RefCell<Result<Vec<ShelvedRecipe>, String>>,
}

#[async_trait(?Send)]
impl RecipeShelf for Shelf {
    async fn list(&self) -> Result<Vec<ShelvedRecipe>, HostError> {
        self.listed.borrow().clone().map_err(|m| base::plain(&m))
    }
    async fn read(&self, _word: &str) -> Result<ShelvedRecipe, HostError> {
        unimplemented!()
    }
    async fn get(&self, _word: &str) -> Result<(ShelvedRecipe, String), HostError> {
        unimplemented!()
    }
    async fn save(&self, _file: Value) -> Result<ShelvedRecipe, HostError> {
        unimplemented!()
    }
    async fn remove(&self, _word: &str) -> Result<ShelvedRecipe, HostError> {
        unimplemented!()
    }
    async fn options(&self, _folders: Option<Vec<(String, String)>>) -> Result<Value, HostError> {
        unimplemented!()
    }
    async fn resolve(&self, _slug: &str) -> Result<ResolvedRecipe, HostError> {
        unimplemented!()
    }
    async fn resolve_file(&self, _file: RecipeFile) -> Result<ResolvedRecipe, HostError> {
        unimplemented!()
    }
}

fn recipe_naming(image: &str) -> ShelvedRecipe {
    let file: RecipeFile =
        serde_json::from_value(json!({ "name": "Builders", "agents": {}, "mcp": {}, "clis": {}, "skills": {}, "plugins": {},
        "folders": { "lab": { "from": "~/lab", "image": image, "keep": [] } }, "configs": {} }))
        .expect("a recipe reads");
    ShelvedRecipe { slug: "builders".to_owned(), file, saved_at: None }
}

/// The context the image area reads, over a temp state folder.
struct FakeIcons {
    state: PathBuf,
    state_path: bool,
    held: RefCell<IndexSet<String>>,
    prefs: RefCell<Preferences>,
    shelf: Option<Rc<Shelf>>,
    listener: RefCell<Option<Rc<dyn Fn()>>>,
    ids: Rc<TestIds>,
    log: RefCell<Vec<String>>,
    /// Held changes: each `change_preferences` waits on the next gate where one is queued.
    gates: RefCell<Vec<oneshot::Receiver<()>>>,
}

#[async_trait(?Send)]
impl ProjectIconsCtx for FakeIcons {
    fn state_folder(&self) -> Result<PathBuf, HostError> {
        Ok(self.state.clone())
    }
    fn has_state_path(&self) -> bool {
        self.state_path
    }
    async fn ready(&self) -> Result<(), HostError> {
        self.log.borrow_mut().push("ready".to_owned());
        Ok(())
    }
    fn project_held(&self, project_id: &str) -> bool {
        self.held.borrow().contains(project_id)
    }
    async fn preferences(&self) -> Result<Preferences, HostError> {
        Ok(self.prefs.borrow().clone())
    }
    async fn change_preferences(&self, change: Box<dyn FnOnce(Preferences) -> Preferences>) -> Result<Preferences, HostError> {
        self.log.borrow_mut().push("change start".to_owned());
        let gate = if self.gates.borrow().is_empty() { None } else { Some(self.gates.borrow_mut().remove(0)) };
        if let Some(g) = gate {
            let _ = g.await;
        }
        let next = change(self.prefs.borrow().clone());
        *self.prefs.borrow_mut() = next.clone();
        self.log.borrow_mut().push("change end".to_owned());
        Ok(next)
    }
    fn recipes(&self) -> Option<Rc<dyn RecipeShelf>> {
        self.shelf.clone().map(|s| s as Rc<dyn RecipeShelf>)
    }
    fn on_recipes_changed(&self, listener: Rc<dyn Fn()>) {
        *self.listener.borrow_mut() = Some(listener);
    }
    fn ids(&self) -> Rc<dyn Ids> {
        self.ids.clone()
    }
}

struct Host {
    _root: tempfile::TempDir,
    ctx: Rc<FakeIcons>,
    area: Rc<ProjectIconsArea>,
    dir: PathBuf,
}

fn host(shelf: Option<Result<Vec<ShelvedRecipe>, String>>, state_path: bool) -> Host {
    let root = tempfile::tempdir().unwrap();
    let state = root.path().join("state");
    std::fs::create_dir_all(&state).unwrap();
    let ctx = Rc::new(FakeIcons {
        state: state.clone(),
        state_path,
        held: RefCell::new(IndexSet::from(["pr_lab".to_owned(), "pr_site".to_owned()])),
        prefs: RefCell::new(prefs(json!({}), json!({}))),
        shelf: shelf.map(|l| Rc::new(Shelf { listed: RefCell::new(l) })),
        listener: RefCell::new(None),
        ids: TestIds::new(),
        log: RefCell::new(vec![]),
        gates: RefCell::new(vec![]),
    });
    let area = project_icons_area(ctx.clone());
    Host { _root: root, ctx, area, dir: state.join("project-icons") }
}

fn b64(bytes: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

mod a_projects_image_over_the_area {
    use super::*;

    #[test]
    fn set_keeps_the_file_names_it_last_and_answers_its_hash() {
        run(async {
            let h = host(Some(Ok(vec![])), true);
            *h.ctx.prefs.borrow_mut() = prefs(json!({ "pr_lab": "a".repeat(64), "pr_site": "b".repeat(64) }), json!({}));
            let sent = file_of(&case("text and phys dropped"));
            let image = h.area.set("pr_lab", Some(b64(&sent))).await.unwrap();
            assert_eq!(image.as_deref(), Some(kept_hash("text and phys dropped").as_str()));
            // The re-set project's key moves to the end; the other keeps its place.
            assert_eq!(icons_json(&h.ctx.prefs.borrow()), json!({ "pr_site": "b".repeat(64), "pr_lab": image.clone().unwrap() }));
            assert_eq!(
                serde_json::to_string(&h.ctx.prefs.borrow().project_icon).unwrap(),
                format!(r#"{{"pr_site":"{}","pr_lab":"{}"}}"#, "b".repeat(64), image.clone().unwrap())
            );
            assert_eq!(listing(&h.dir), vec![format!("{}.png", image.unwrap())]);
            assert_eq!(*h.ctx.log.borrow(), vec!["ready", "change start", "change end"]);
        });
    }

    #[test]
    fn set_refuses_a_project_not_held_and_writes_nothing() {
        run(async {
            let h = host(Some(Ok(vec![])), true);
            let e = h.area.set("pr_gone", Some(b64(&good()))).await.unwrap_err();
            assert_eq!(e.message, "no project \"pr_gone\"");
            assert_eq!(e.kind, Some(Kind::Usage));
            assert_eq!(icons_json(&h.ctx.prefs.borrow()), json!({}));
            assert!(listing(&h.dir).is_empty());
            assert_eq!(*h.ctx.log.borrow(), vec!["ready"]);
        });
    }

    #[test]
    fn a_refused_png_names_nothing_and_keeps_nothing() {
        run(async {
            let h = host(Some(Ok(vec![])), true);
            let first = h.area.set("pr_lab", Some(b64(&good()))).await.unwrap().unwrap();
            let e = h.area.set("pr_site", Some(b64(&file_of(&case("64 px"))))).await.unwrap_err();
            assert!(e.message.contains("64 by 64"));
            assert_eq!(icons_json(&h.ctx.prefs.borrow()), json!({ "pr_lab": first.clone() }));
            assert_eq!(listing(&h.dir), vec![format!("{first}.png")]);
        });
    }

    #[test]
    fn set_null_takes_the_image_off_and_deletes_the_file_nothing_names() {
        run(async {
            let h = host(Some(Ok(vec![])), true);
            let shared = file_of(&case("good rgba"));
            let image = h.area.set("pr_lab", Some(b64(&shared))).await.unwrap().unwrap();
            h.area.set("pr_site", Some(b64(&shared))).await.unwrap();
            assert_eq!(h.area.set("pr_lab", None).await.unwrap(), None);
            assert_eq!(icons_json(&h.ctx.prefs.borrow()), json!({ "pr_site": image.clone() }));
            assert!(h.dir.join(format!("{image}.png")).exists(), "the other project still names the file");
            h.area.set("pr_site", None).await.unwrap();
            assert_eq!(icons_json(&h.ctx.prefs.borrow()), json!({}));
            assert!(listing(&h.dir).is_empty());
        });
    }

    #[test]
    fn read_with_no_state_path_answers_null_for_each() {
        run(async {
            let h = host(None, false);
            let read = h.area.read(vec!["b".repeat(64), "a".repeat(64)]).await.unwrap();
            assert_eq!(serde_json::to_value(&read).unwrap(), json!({ "b".repeat(64): null, "a".repeat(64): null }));
            assert_eq!(read.keys().next().unwrap(), &"b".repeat(64));
        });
    }

    #[test]
    fn forget_drops_the_look_and_the_image_and_deletes_the_file() {
        run(async {
            let h = host(Some(Ok(vec![])), true);
            let image = h.area.set("pr_site", Some(b64(&good()))).await.unwrap().unwrap();
            let mut p = h.ctx.prefs.borrow().clone();
            p = prefs(
                icons_json(&p),
                json!({ "pr_lab": { "icon": "rocket", "hue": "blue" }, "pr_site": { "icon": "rocket", "hue": "teal" } }),
            );
            *h.ctx.prefs.borrow_mut() = p;
            h.area.forget("pr_site").await.unwrap();
            let after = h.ctx.prefs.borrow().clone();
            assert_eq!(serde_json::to_value(&after.project_look).unwrap(), json!({ "pr_lab": { "icon": "rocket", "hue": "blue" } }));
            assert_eq!(icons_json(&after), json!({}));
            assert!(!h.dir.join(format!("{image}.png")).exists());
        });
    }

    #[test]
    fn wear_names_a_held_hash_and_ignores_one_not_held() {
        run(async {
            let h = host(Some(Ok(vec![])), true);
            let d = h.dir.clone();
            let kept = project_icons(Rc::new(move || d.clone()), TestIds::new()).keep(&good()).unwrap();
            h.area.wear("pr_lab", Some("c".repeat(64))).await.unwrap();
            h.area.wear("pr_lab", None).await.unwrap();
            assert_eq!(icons_json(&h.ctx.prefs.borrow()), json!({}));
            assert!(h.ctx.log.borrow().is_empty(), "nothing named, nothing changed");
            h.area.wear("pr_lab", Some(kept.clone())).await.unwrap();
            assert_eq!(icons_json(&h.ctx.prefs.borrow()), json!({ "pr_lab": kept }));
            let none = host(Some(Ok(vec![])), false);
            none.area.wear("pr_lab", Some(kept)).await.unwrap();
            assert_eq!(icons_json(&none.ctx.prefs.borrow()), json!({}), "no state path wears nothing");
        });
    }

    #[test]
    fn sweep_keeps_a_file_a_recipe_folder_names() {
        run(async {
            let named = kept_hash("good rgb");
            let h = host(Some(Ok(vec![recipe_naming(&named)])), true);
            h.area.set("pr_lab", Some(b64(&file_of(&case("good rgb"))))).await.unwrap();
            h.area.set("pr_lab", None).await.unwrap();
            assert_eq!(listing(&h.dir), vec![format!("{named}.png")]);
        });
    }

    #[test]
    fn an_unreadable_recipe_shelf_deletes_nothing() {
        run(async {
            let h = host(Some(Err("the shelf is locked".to_owned())), true);
            let image = h.area.set("pr_lab", Some(b64(&good()))).await.unwrap().unwrap();
            h.area.set("pr_lab", None).await.unwrap();
            assert_eq!(listing(&h.dir), vec![format!("{image}.png")]);
            assert_eq!(icons_json(&h.ctx.prefs.borrow()), json!({}));
        });
    }

    #[test]
    fn no_state_path_sweeps_no_files() {
        run(async {
            let h = host(Some(Ok(vec![])), false);
            std::fs::create_dir_all(&h.dir).unwrap();
            std::fs::write(h.dir.join(format!("{}.png", "d".repeat(64))), b"x").unwrap();
            h.area.sweep().await.unwrap();
            assert_eq!(listing(&h.dir), vec![format!("{}.png", "d".repeat(64))]);
        });
    }

    #[test]
    fn a_recipes_changed_event_sweeps_in_turn() {
        run(async {
            let h = host(Some(Ok(vec![])), true);
            std::fs::create_dir_all(&h.dir).unwrap();
            std::fs::write(h.dir.join(format!("{}.png", "e".repeat(64))), b"x").unwrap();
            let listener = h.ctx.listener.borrow().clone().expect("the area listens for recipes.changed");
            listener();
            // A wear queued after the event runs after its sweep.
            h.area.wear("pr_lab", None).await.unwrap();
            assert!(listing(&h.dir).is_empty());
        });
    }

    #[test]
    fn set_forget_and_wear_run_one_at_a_time_in_call_order() {
        run(async {
            let h = host(Some(Ok(vec![])), true);
            let (tx, rx) = oneshot::channel();
            h.ctx.gates.borrow_mut().push(rx);
            let area = h.area.clone();
            let png = b64(&good());
            let first = tokio::task::spawn_local(async move { area.set("pr_lab", Some(png)).await });
            let area = h.area.clone();
            let second = tokio::task::spawn_local(async move { area.forget("pr_site").await });
            tokio::task::yield_now().await;
            tokio::task::yield_now().await;
            assert_eq!(*h.ctx.log.borrow(), vec!["ready", "change start"], "the forget waits for the set's turn");
            tx.send(()).unwrap();
            first.await.unwrap().unwrap();
            second.await.unwrap().unwrap();
            assert_eq!(*h.ctx.log.borrow(), vec!["ready", "change start", "change end", "change start", "change end"]);
        });
    }

    #[test]
    fn a_failed_set_does_not_stop_the_next() {
        run(async {
            let h = host(Some(Ok(vec![])), true);
            assert!(h.area.set("pr_gone", Some(b64(&good()))).await.is_err());
            let image = h.area.set("pr_lab", Some(b64(&good()))).await.unwrap();
            assert_eq!(icons_json(&h.ctx.prefs.borrow()), json!({ "pr_lab": image.unwrap() }));
        });
    }
}

#[allow(dead_code)]
fn unused(_: IndexMap<String, String>) {}
