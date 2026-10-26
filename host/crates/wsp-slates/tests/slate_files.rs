// SPDX-License-Identifier: AGPL-3.0-only
//! Tests of src/slate_files.rs, from the TS in packages/runtime/src/slate-files.ts. Spec: spec/slate_files.md.
//! Every expected value was printed by the TS under node (probes pfiles.ts, pf2.ts, pf3.ts, pf4.ts), or is asserted in
//! packages/runtime/test/slate-box.test.ts:698-1149.

use std::cell::{Cell, RefCell};
use std::fs;
use std::os::unix::fs::{symlink, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;

use futures::future::LocalBoxFuture;
use futures::FutureExt;
use indexmap::IndexMap;
use serde_json::{json, Value};

use wsp_slates::pending::slate::{SlateDoc, SlateRunDecl};
use wsp_slates::slate_files::{
    always_refused, box_pins, cannot_pin, files_read, keyed, paths_named, pin_here, with_files, words_at, write_slate_files, BoxPins,
    FsHashReply, HashOn, HashRead, Pin, PinAsk,
};

/// The first 16 hex of the sha256 of "echo one".
const H: &str = "5d103558eaead111";
const H32: &str = "0123456789abcdef0123456789abcdef";

const P_OUTSIDE: &str = "is outside the thread's folder";
const P_LINK: &str = "is a link out of the thread's folder";
const P_PAST: &str = "is past what one command can pin (32 files, each up to 4 MiB)";
const P_SPECIAL: &str = "is not a regular file";
const P_BUILT: &str = "is a script wsp cannot read by name (a space, an escape or a $ in it)";

fn decl(v: Value) -> SlateRunDecl {
    serde_json::from_value(v).unwrap()
}

fn c(cmd: &str) -> Value {
    json!({"kind": "cmd", "cmd": cmd})
}

fn cx(cmd: &str, extra: Value) -> Value {
    let mut v = c(cmd);
    for (k, x) in extra.as_object().unwrap() {
        v.as_object_mut().unwrap().insert(k.clone(), x.clone());
    }
    v
}

/// A stored document holding these runs and files, the rest of the document's required keys empty.
fn doc(mut v: Value) -> SlateDoc {
    let o = v.as_object_mut().unwrap();
    for (k, empty) in [
        ("schema", json!(2)),
        ("root", json!("root")),
        ("values", json!({})),
        ("derived", json!({})),
        ("runs", json!({})),
        ("reactions", json!([])),
        ("pieces", json!({})),
    ] {
        o.entry(k).or_insert(empty);
    }
    serde_json::from_value(v).unwrap()
}

fn pin_from(v: Value) -> Pin {
    serde_json::from_value(v).unwrap()
}

fn pv(pin: &Pin) -> Value {
    serde_json::to_value(pin).unwrap()
}

fn keyed_v(pin: &Pin) -> Value {
    serde_json::to_value(keyed(pin)).unwrap()
}

fn strs(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

fn mode(path: &Path) -> u32 {
    fs::symlink_metadata(path).unwrap().permissions().mode() & 0o7777
}

/// A thread's folder `f` inside a canonical temp root, holding only what a case puts there.
struct Fx {
    _tmp: tempfile::TempDir,
    root: PathBuf,
    folder: PathBuf,
}

impl Fx {
    fn new() -> Fx {
        let tmp = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(tmp.path()).unwrap();
        let folder = root.join("f");
        fs::create_dir(&folder).unwrap();
        Fx { _tmp: tmp, root, folder }
    }

    fn file(&self, name: &str, text: &str) {
        let path = self.folder.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    fn sparse(&self, name: &str, len: u64) {
        fs::File::create(self.folder.join(name)).unwrap().set_len(len).unwrap();
    }

    fn away(&self) {
        fs::write(self.root.join("away.sh"), "echo away").unwrap();
    }

    fn link(&self, name: &str) {
        self.away();
        symlink(self.root.join("away.sh"), self.folder.join(name)).unwrap();
    }

    fn fifo(&self, name: &str) {
        let made = Command::new("mkfifo").arg(self.folder.join(name)).status().unwrap();
        assert!(made.success());
    }

    fn stats(&self) {
        self.file("stats.sh", "echo one");
    }

    fn pin(&self, d: Value) -> Pin {
        pin_here(Some(self.folder.to_str().unwrap()), &decl(d))
    }

    fn pin_cmd(&self, cmd: &str) -> Pin {
        self.pin(c(cmd))
    }

    /// The pin binds exactly these scripts and leaves nothing unpinned, and says nothing against an Always.
    fn expect_bound(&self, d: Value, scripts: &[(&str, &str)]) {
        let pin = self.pin(d.clone());
        let map: serde_json::Map<String, Value> = scripts.iter().map(|(k, v)| (k.to_string(), json!(v))).collect();
        assert_eq!(pv(&pin), json!({"scripts": map, "unpinned": []}), "{d}");
        assert_eq!(keyed_v(&pin), Value::Object(map), "{d}");
        assert_eq!(cannot_pin(&pin), None, "{d}");
        assert_eq!(always_refused("run", &pin), None, "{d}");
    }

    /// The pin binds nothing and leaves nothing unpinned.
    fn expect_clear(&self, d: Value) {
        let pin = self.pin(d.clone());
        assert_eq!(pv(&pin), json!({"unpinned": []}), "{d}");
        assert_eq!(keyed(&pin), None, "{d}");
        assert_eq!(cannot_pin(&pin), None, "{d}");
        assert_eq!(always_refused("run", &pin), None, "{d}");
    }

    /// The pin binds nothing and leaves these (path, why, phrase) unpinned, in this order, with the words the sheet and a
    /// refused Always say.
    fn expect_unpinned(&self, d: Value, items: &[(&str, &str, &str)]) {
        let pin = self.pin(d.clone());
        let list: Vec<Value> = items.iter().map(|(p, w, _)| json!({"path": p, "why": w})).collect();
        assert_eq!(pv(&pin), json!({"unpinned": list}), "{d}");
        let map: serde_json::Map<String, Value> = items.iter().map(|(p, w, _)| (p.to_string(), json!(w))).collect();
        assert_eq!(keyed_v(&pin), Value::Object(map), "{d}");
        let said = items.iter().map(|(p, _, ph)| format!("{p} {ph}")).collect::<Vec<_>>().join(" and ");
        let it = if items.len() == 1 { "it" } else { "them" };
        assert_eq!(cannot_pin(&pin), Some(format!("{said}, so wsp cannot pin {it} and it asks every time")), "{d}");
        assert_eq!(always_refused("run", &pin), Some(format!("$run names a script wsp cannot pin to hold an Always to: {said}.")), "{d}");
    }

    /// The unpinned (path, why) pairs a command leaves.
    fn unpinned_of(&self, cmd: &str) -> Vec<(String, String)> {
        let v = pv(&self.pin_cmd(cmd));
        v["unpinned"]
            .as_array()
            .unwrap()
            .iter()
            .map(|u| (u["path"].as_str().unwrap().to_string(), u["why"].as_str().unwrap().to_string()))
            .collect()
    }
}

fn outside_of(paths: &[&str]) -> Vec<(String, String)> {
    paths.iter().map(|p| (p.to_string(), "outside".to_string())).collect()
}

fn doc4() -> SlateDoc {
    doc(json!({"runs": {}, "files": {"a.sh": "A", "b.py": "B", "lib.sh": "L", "x.y.z": "X"}}))
}

fn all4() -> Vec<(&'static str, &'static str)> {
    vec![("a.sh", "A"), ("b.py", "B"), ("lib.sh", "L"), ("x.y.z", "X")]
}

fn last_key(v: &Value) -> String {
    v.as_object().unwrap().keys().next_back().unwrap().clone()
}

/// What `with_files` answers for a declaration: the files read (in order) and the JSON the key and sheet take, which is
/// the declaration with `files` last when any are read, and the declaration alone when none are.
fn expect_read(doc: Option<&SlateDoc>, d: Value, expected: Option<&[(&str, &str)]>) {
    let decl = decl(d.clone());
    let w = with_files(doc, &decl);
    assert_eq!(w.decl, decl, "{d}");
    let direct = files_read(doc, &decl);
    assert_eq!(direct, w.files, "{d}");
    let got: Option<Vec<(String, String)>> = w.files.as_ref().map(|f| f.iter().map(|(k, v)| (k.clone(), v.clone())).collect());
    let want: Option<Vec<(String, String)>> = expected.map(|e| e.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect());
    assert_eq!(got, want, "{d}");
    let mut want_json = serde_json::to_value(&decl).unwrap();
    if let Some(e) = expected {
        let files: serde_json::Map<String, Value> = e.iter().map(|(k, v)| (k.to_string(), json!(v))).collect();
        want_json.as_object_mut().unwrap().insert("files".into(), Value::Object(files));
        assert_eq!(last_key(&w.to_json()), "files", "{d}");
    }
    assert_eq!(w.to_json(), want_json, "{d}");
}

// with_files, rules 1 and 2

// TS withFiles-names: a name read when bounded by start, end or a character outside [A-Za-z0-9._-].
#[test]
fn with_files_reads_named_files_bounded_by_non_name_characters() {
    let d = doc4();
    expect_read(Some(&d), c("bash a.sh; python3 ./b.py; cat mylib.sh"), Some(&[("a.sh", "A"), ("b.py", "B")]));
    let w = with_files(Some(&d), &decl(c("bash a.sh; python3 ./b.py; cat mylib.sh")));
    assert_eq!(
        serde_json::to_string(&w.to_json()).unwrap(),
        r#"{"kind":"cmd","cmd":"bash a.sh; python3 ./b.py; cat mylib.sh","files":{"a.sh":"A","b.py":"B"}}"#
    );
    expect_read(Some(&d), c("cat a.sh,b.py"), Some(&[("a.sh", "A"), ("b.py", "B")]));
    expect_read(Some(&d), c("a.sh"), Some(&[("a.sh", "A")]));
    expect_read(Some(&d), c("echo 'a.sh'"), Some(&[("a.sh", "A")]));
    expect_read(Some(&d), c("cat dir/a.sh"), Some(&[("a.sh", "A")]));
    expect_read(Some(&d), c("cat a.sh\nb.py"), Some(&[("a.sh", "A"), ("b.py", "B")]));
    // the files come back in the document's order, not the order the command names them
    expect_read(Some(&d), c("cat b.py a.sh"), Some(&[("a.sh", "A"), ("b.py", "B")]));
    for none in ["xa.sh", "cat a.sh.bak", "cat -a.sh", "cat _a.sh", "cat a.sh_", "cat lib.sh.", "cat A.SH"] {
        expect_read(Some(&d), c(none), None);
    }
}

// TS withFiles-dir and withFiles-brace: naming $SLATE_DIR reads every file.
#[test]
fn with_files_reads_every_file_where_slate_dir_is_named() {
    let d = doc4();
    for said in ["sh \"$SLATE_DIR/a.sh\"", "sh ${SLATE_DIR}/a.sh", "echo $SLATE_DIR", "echo ${SLATE_DIR"] {
        expect_read(Some(&d), c(said), Some(&all4()));
    }
    let w = with_files(Some(&d), &decl(c("sh \"$SLATE_DIR/a.sh\"")));
    assert_eq!(
        serde_json::to_string(&w.to_json()).unwrap(),
        r#"{"kind":"cmd","cmd":"sh \"$SLATE_DIR/a.sh\"","files":{"a.sh":"A","b.py":"B","lib.sh":"L","x.y.z":"X"}}"#
    );
    expect_read(Some(&d), cx("echo hi", json!({"then": "sh $SLATE_DIR/q"})), Some(&all4()));
    // the name must end at a word boundary
    expect_read(Some(&d), c("echo $SLATE_DIRECTORY"), None);
    expect_read(Some(&d), c("echo $SLATE_DIR_x"), None);
}

// TS withFiles-none: a cmd naming no file comes back unchanged.
#[test]
fn with_files_leaves_a_decl_that_reads_nothing_unchanged() {
    let d = doc4();
    let hi = decl(c("echo hi"));
    let w = with_files(Some(&d), &hi);
    assert_eq!(w.decl, hi);
    assert_eq!(w.files, None);
    assert_eq!(w.to_json(), json!({"kind": "cmd", "cmd": "echo hi"}));
    assert_eq!(files_read(Some(&d), &hi), None);
    // a document with no files, or an empty set of them, reads nothing
    let none = doc(json!({"runs": {}}));
    expect_read(Some(&none), c("sh a.sh"), None);
    let empty = doc(json!({"runs": {}, "files": {}}));
    expect_read(Some(&empty), c("sh a.sh"), None);
    assert_eq!(with_files(Some(&none), &decl(c("sh a.sh"))).to_json(), json!({"kind": "cmd", "cmd": "sh a.sh"}));
}

// TS withFiles-dot: only the dot in a name is escaped, so x.y.z does not match x_y_z.
#[test]
fn with_files_matches_dots_literally() {
    let d = doc4();
    expect_read(Some(&d), c("cat x_y_z"), None);
    expect_read(Some(&d), c("cat x_y_z; cat x.y.z"), Some(&[("x.y.z", "X")]));
    let dotted = doc(json!({"runs": {}, "files": {"a.b": "AB"}}));
    expect_read(Some(&dotted), c("cat aXb"), None);
    expect_read(Some(&dotted), c("cat a.b"), Some(&[("a.b", "AB")]));
}

// TS withFiles-tool-then: a tool's or resource's `then` is read, its other fields are not; a cmd keeps its other keys.
#[test]
fn with_files_reads_a_tool_runs_then() {
    let d = doc4();
    let tool = json!({"kind": "tool", "server": "s", "tool": "t", "then": "sh lib.sh"});
    expect_read(Some(&d), tool, Some(&[("lib.sh", "L")]));
    let w = with_files(Some(&d), &decl(json!({"kind": "tool", "server": "s", "tool": "t", "then": "sh lib.sh"})));
    assert_eq!(w.to_json(), json!({"kind": "tool", "server": "s", "tool": "t", "then": "sh lib.sh", "files": {"lib.sh": "L"}}));
    assert_eq!(last_key(&w.to_json()), "files");
    expect_read(Some(&d), json!({"kind": "resource", "server": "s", "uri": "u", "then": "sh lib.sh"}), Some(&[("lib.sh", "L")]));
    expect_read(Some(&d), json!({"kind": "tool", "server": "s", "tool": "t", "then": "sh \"${SLATE_DIR}/q\""}), Some(&all4()));
    expect_read(Some(&d), json!({"kind": "tool", "server": "s", "tool": "lib.sh"}), None);
    expect_read(Some(&d), cx("echo hi", json!({"then": "sh lib.sh"})), Some(&[("lib.sh", "L")]));
    expect_read(Some(&d), cx("echo hi", json!({"cwd": "x", "on": "m", "then": "sh b.py"})), Some(&[("b.py", "B")]));
}

// TS withFiles-null-doc: no document reads nothing, even where $SLATE_DIR is named.
#[test]
fn with_files_with_no_doc_reads_nothing() {
    let said = decl(c("sh \"$SLATE_DIR/a.sh\""));
    assert_eq!(files_read(None, &said), None);
    let w = with_files(None, &said);
    assert_eq!(w.decl, said);
    assert_eq!(w.files, None);
    assert_eq!(w.to_json(), json!({"kind": "cmd", "cmd": "sh \"$SLATE_DIR/a.sh\""}));
    expect_read(None, c("bash a.sh"), None);
}

// write_slate_files, rule 3

// TS wsf probe and writeSlateFiles: drop what is not named, replace links and folders, 0600 files, 0700 folder made.
#[test]
fn write_slate_files_keeps_only_the_documents_files_mode_0600_and_never_follows_a_link() {
    unsafe { libc::umask(0o022) };
    let tmp = tempfile::tempdir().unwrap();
    let root = fs::canonicalize(tmp.path()).unwrap();
    let files =
        |pairs: &[(&str, &str)]| -> IndexMap<String, String> { pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect() };

    let dir = root.join("slate");
    assert_eq!(write_slate_files(&dir, &files(&[("a.sh", "A")])).unwrap(), dir);
    assert_eq!(mode(&dir), 0o700);
    assert_eq!(fs::read_to_string(dir.join("a.sh")).unwrap(), "A");

    fs::create_dir(dir.join("junk")).unwrap();
    fs::write(dir.join("junk").join("inner"), "i").unwrap();
    fs::write(dir.join("old"), "o").unwrap();
    fs::create_dir(dir.join("c.sh")).unwrap();
    fs::write(dir.join("c.sh").join("inner"), "i").unwrap();
    fs::write(dir.join("d.sh"), "old").unwrap();
    fs::set_permissions(dir.join("d.sh"), fs::Permissions::from_mode(0o644)).unwrap();
    let target = root.join("target");
    fs::write(&target, "T").unwrap();
    symlink(&target, dir.join("b.sh")).unwrap();
    let outdir = root.join("outdir");
    fs::create_dir(&outdir).unwrap();
    fs::write(outdir.join("child"), "keep").unwrap();
    symlink(&outdir, dir.join("e.sh")).unwrap();

    let second = files(&[("a.sh", "A2"), ("b.sh", "B"), ("c.sh", "C"), ("d.sh", "D")]);
    assert_eq!(write_slate_files(&dir, &second).unwrap(), dir);

    let mut names: Vec<String> = fs::read_dir(&dir).unwrap().map(|e| e.unwrap().file_name().into_string().unwrap()).collect();
    names.sort();
    assert_eq!(names, strs(&["a.sh", "b.sh", "c.sh", "d.sh"]));
    for (name, text) in [("a.sh", "A2"), ("b.sh", "B"), ("c.sh", "C"), ("d.sh", "D")] {
        let meta = fs::symlink_metadata(dir.join(name)).unwrap();
        assert!(meta.is_file(), "{name} is a regular file, not a link or folder");
        assert_eq!(mode(&dir.join(name)), 0o600, "{name}");
        assert_eq!(fs::read_to_string(dir.join(name)).unwrap(), text, "{name}");
    }
    assert_eq!(mode(&dir), 0o700);
    assert_eq!(fs::read_to_string(&target).unwrap(), "T");
    assert_eq!(fs::read_to_string(outdir.join("child")).unwrap(), "keep");

    // a folder that exists keeps its mode
    let there = root.join("there");
    fs::create_dir(&there).unwrap();
    fs::set_permissions(&there, fs::Permissions::from_mode(0o755)).unwrap();
    write_slate_files(&there, &files(&[("a.sh", "A")])).unwrap();
    assert_eq!(mode(&there), 0o755);

    // folders on the way are made, an empty document empties the folder
    let deep = root.join("deep").join("slate");
    assert_eq!(write_slate_files(&deep, &files(&[("a.sh", "A")])).unwrap(), deep);
    assert!(deep.is_dir());
    write_slate_files(&dir, &files(&[])).unwrap();
    assert_eq!(fs::read_dir(&dir).unwrap().count(), 0);

    // an error goes to the caller
    assert!(write_slate_files(&target.join("inside"), &files(&[("a.sh", "A")])).is_err());
}

// paths_named, rule 4

// TS pathsNamed (pfiles.ts line 1): split on whitespace, quotes, ; | & ( ) < > = , and a backtick; order and repeats kept.
#[test]
fn paths_named_splits_and_filters_as_the_ts() {
    let said = "cat /proc/loadavg; /usr/bin/env true; tail -n 1 ../notes.txt; tail -c 3 big.log; bash lib/../ok.sh; \
        .venv/bin/python3.12 stats.py; bash -c 'echo two words'; python3 -c 'print(1)'; echo $HOME; cat ~/.profile; \
        bash $SLATE_DIR/x.sh";
    let d = decl(cx(said, json!({"then": "jq . a.json"})));
    assert_eq!(
        paths_named(&d),
        strs(&[
            "/proc/loadavg",
            "/usr/bin/env",
            "../notes.txt",
            "big.log",
            "lib/../ok.sh",
            ".venv/bin/python3.12",
            "stats.py",
            "~/.profile",
            ".",
            "a.json"
        ])
    );
    assert_eq!(paths_named(&decl(cx("echo hi", json!({"then": "jq . a.json"})))), strs(&[".", "a.json"]));
    assert_eq!(paths_named(&decl(c("cat a.txt a.txt"))), strs(&["a.txt", "a.txt"]));
    assert_eq!(paths_named(&decl(c("sort <in.txt >out.txt 2>&1; echo `cat a/b`,c.d"))), strs(&["in.txt", "out.txt", "a/b", "c.d"]));
    assert_eq!(paths_named(&decl(c("(a.b)|c.d&e.f;g.h"))), strs(&["a.b", "c.d", "e.f", "g.h"]));
    assert_eq!(paths_named(&decl(c("echo hi"))), Vec::<String>::new());
}

// TS pathsNamed-url and the filter: no word starting with -, holding $ or ://, and each holds a . or a /.
#[test]
fn paths_named_drops_flags_urls_and_dollar_words() {
    assert_eq!(paths_named(&decl(c("curl https://a.b/c.json -o out.json --flag=x.y"))), strs(&["out.json", "x.y"]));
    assert_eq!(paths_named(&decl(c("curl http://a.b/c ftp://x.y/z file.txt"))), strs(&["file.txt"]));
    assert_eq!(paths_named(&decl(c("echo -a.b $HOME/x a=b/c\td.e\nf.g 'h i.j' \"k/l\""))), strs(&["b/c", "d.e", "f.g", "i.j", "k/l"]));
    assert_eq!(paths_named(&decl(c("x --y=z.w -p/q a$b.c ~/r ./ ../ . .. /"))), strs(&["z.w", "~/r", "./", "../", ".", "..", "/"]));
    assert_eq!(paths_named(&decl(c("df -h; sleep 0.5; echo plain words"))), strs(&["0.5"]));
}

// the fake daemon

type Asked = Rc<RefCell<Vec<(String, Vec<String>)>>>;

fn reply(files: Value) -> FsHashReply {
    serde_json::from_value(json!({"files": files, "outside": [], "past": [], "special": []})).unwrap()
}

fn reply_lists(files: Value, outside: &[&str], past: &[&str], special: &[&str]) -> FsHashReply {
    serde_json::from_value(json!({"files": files, "outside": outside, "past": past, "special": special})).unwrap()
}

type HashOnFor = Box<dyn Fn(&str) -> Option<HashOn>>;

fn daemon(f: impl Fn(String, Vec<String>) -> LocalBoxFuture<'static, Result<HashRead, String>> + 'static) -> HashOnFor {
    let on: HashOn = Rc::new(f);
    Box::new(move |_| Some(on.clone()))
}

/// A daemon that records what it is asked and answers `r`.
fn answering(asked: &Asked, r: FsHashReply) -> HashOnFor {
    let asked = asked.clone();
    daemon(move |root, paths| {
        asked.borrow_mut().push((root, paths));
        let r = r.clone();
        async move { Ok(r) }.boxed_local()
    })
}

fn ask(folder: Option<&str>, d: Value) -> PinAsk {
    PinAsk { folder: folder.map(str::to_string), decl: decl(d) }
}

fn acme(cmd: &str) -> PinAsk {
    ask(Some("/root/acme"), c(cmd))
}

fn pin_at(pins: &BoxPins, thread: &str, folder: Option<&str>, d: Value) -> Value {
    pv(&pins.pin(thread, folder, &decl(d)))
}

fn unread(path: &str) -> Value {
    json!({"unpinned": [{"path": path, "why": "unread"}]})
}

fn sorted(asked: &Asked) -> Vec<(String, Vec<String>)> {
    let mut v = asked.borrow().clone();
    v.sort();
    v
}

// words_at and the box reading, rules 5 and 15

// TS asked-root, asked: the daemon is asked the folder the command runs in (posix, ways up folded) joined to each word.
#[tokio::test(flavor = "current_thread")]
async fn box_pins_joins_relative_words_to_the_folder_and_cwd_as_posix() {
    let asked: Asked = Default::default();
    let pins = box_pins(answering(&asked, reply(json!({}))), None);
    let cases: Vec<(&str, Value, Vec<&str>)> = vec![
        ("/root/acme", c("bash stats.sh"), vec!["/root/acme/stats.sh"]),
        ("/", cx("bash a.sh /etc/x.sh", json!({"cwd": "/opt"})), vec!["/opt/a.sh", "/etc/x.sh"]),
        ("/root/acme", cx("bash a.sh", json!({"cwd": "../other/./x"})), vec!["/root/other/x/a.sh"]),
        ("/root/acme", c("bash ./x.sh ../y.sh /abs/z.sh"), vec!["/root/acme/./x.sh", "/root/acme/../y.sh", "/abs/z.sh"]),
        ("/", c("bash a.sh"), vec!["/a.sh"]),
        ("/", cx("bash a.sh", json!({"cwd": "x/y"})), vec!["/x/y/a.sh"]),
        ("/root/acme", cx("bash a.sh", json!({"cwd": "/srv//data/../app"})), vec!["/srv/app/a.sh"]),
        ("/root/acme", cx("bash a.sh", json!({"cwd": ".."})), vec!["/root/a.sh"]),
        ("/root/acme", cx("bash a.sh", json!({"cwd": "../../../.."})), vec!["/a.sh"]),
        ("/root/acme", cx("bash a.sh", json!({"cwd": "x", "then": "bash ./b.sh"})), vec!["/root/acme/x/a.sh", "/root/acme/x/./b.sh"]),
    ];
    for (folder, d, want) in &cases {
        for there in [true, false] {
            assert_eq!(words_at(folder, &decl(d.clone()), there), strs(want), "{folder} {d} there={there}");
        }
        asked.borrow_mut().clear();
        pins.read("t", &[ask(Some(*folder), d.clone())], false).await;
        assert_eq!(*asked.borrow(), vec![(folder.to_string(), strs(want))], "{folder} {d}");
    }
}

// TS asked-many and boxpin-many: 257 paths there, the 257th is `paths`, only the first 256 are sent.
#[tokio::test(flavor = "current_thread")]
async fn box_pins_sends_at_most_256_paths_and_marks_the_rest_paths() {
    let asked: Asked = Default::default();
    let pins = box_pins(answering(&asked, reply(json!({}))), None);
    let dirs: Vec<String> = (0..256).map(|n| format!("d{n}/")).collect();
    let many = format!("true {}; bash ./evil.sh", dirs.join(" "));
    pins.read("t", &[acme(&many)], false).await;
    let want: Vec<String> = (0..256).map(|n| format!("/root/acme/d{n}/")).collect();
    assert_eq!(*asked.borrow(), vec![("/root/acme".to_string(), want)]);
    let pin = pins.pin("t", Some("/root/acme"), &decl(c(&many)));
    assert_eq!(pv(&pin), json!({"unpinned": [{"path": "./evil.sh", "why": "paths"}]}));
    assert_eq!(keyed_v(&pin), json!({"./evil.sh": "paths"}));
    assert_eq!(
        cannot_pin(&pin),
        Some("./evil.sh is past the 256 paths one command can name, so wsp cannot pin it and it asks every time".into())
    );
    assert_eq!(
        always_refused("run", &pin),
        Some("$run names a script wsp cannot pin to hold an Always to: ./evil.sh is past the 256 paths one command can name.".into())
    );
}

// pin_here, rules 6 to 9 and 13

// TS pinHere: a script named in the folder is bound by its hash, however the command spells the run.
#[test]
fn pin_here_binds_scripts_by_hash() {
    let fx = Fx::new();
    fx.stats();
    for said in [
        "bash stats.sh",
        "bash lib/../stats.sh",
        "./stats.sh",
        "/usr/bin/env bash stats.sh",
        "X=1 bash stats.sh",
        "sudo -E bash stats.sh",
        "echo stats.sh",
    ] {
        fx.expect_bound(c(said), &[("stats.sh", H)]);
    }
    fx.expect_bound(cx("echo hi", json!({"then": "bash stats.sh"})), &[("stats.sh", H)]);
    // a missing file is not hashed and not unpinned
    fx.expect_clear(c("bash nothere.sh"));
}

// TS pinHere: link, outside twice, absolute, home, special, built three ways, past by size.
#[test]
fn pin_here_marks_each_unpinnable_script_with_its_why() {
    let case = |setup: &dyn Fn(&Fx), d: Value, items: &[(&str, &str, &str)]| {
        let fx = Fx::new();
        setup(&fx);
        fx.expect_unpinned(d, items);
    };
    case(&|fx| fx.link("out.sh"), c("bash ./out.sh"), &[("./out.sh", "link", P_LINK)]);
    case(&|fx| fx.away(), c("bash ../away.sh; bash ../away.sh"), &[("../away.sh", "outside", P_OUTSIDE)]);
    case(&|_| {}, c("python3 /opt/acme/stats.py"), &[("/opt/acme/stats.py", "outside", P_OUTSIDE)]);
    case(&|_| {}, c("bash ~/away.sh"), &[("~/away.sh", "outside", P_OUTSIDE)]);
    case(&|fx| fx.fifo("evil.sh"), c("bash ./evil.sh"), &[("./evil.sh", "special", P_SPECIAL)]);
    case(&|fx| fx.fifo("evil.sh"), c("bash './my evil.sh'"), &[("'./my evil.sh'", "built", P_BUILT), ("evil.sh", "special", P_SPECIAL)]);
    case(&|fx| fx.fifo("evil.sh"), c("bash my\\ evil.sh"), &[("my\\ evil.sh", "built", P_BUILT), ("evil.sh", "special", P_SPECIAL)]);
    case(&|_| {}, c("bash \"$PWD/../away.sh\""), &[("\"$PWD/../away.sh\"", "built", P_BUILT)]);
    case(&|fx| fx.sparse("big.sh", 4 * 1024 * 1024 + 1), c("bash ./big.sh"), &[("./big.sh", "past", P_PAST)]);
    case(&|fx| fx.sparse("big.sh", 4 * 1024 * 1024 + 1), c("cat /proc/loadavg; tail -c 3 big.sh"), &[("big.sh", "past", P_PAST)]);
    case(&|fx| fx.away(), cx("echo hi", json!({"then": "bash ../a.sh"})), &[("../a.sh", "outside", P_OUTSIDE)]);

    // exactly 4 MiB is not past the cap
    let fx = Fx::new();
    fx.sparse("exact.sh", 4 * 1024 * 1024);
    fx.expect_bound(c("bash exact.sh"), &[("exact.sh", "bb9f8df61474d25e")]);

    // a program name in the command's runner position counts as run, however it is spelled
    let fx = Fx::new();
    for (said, paths) in [
        ("nohup ../tool", vec!["../tool"]),
        ("node ../tool", vec!["../tool"]),
        ("../tool", vec!["../tool"]),
        ("FOO=1 ../tool", vec!["../tool"]),
        ("env FOO=1 ../tool", vec!["../tool"]),
        ("python3.12 ../tool", vec!["../tool"]),
        ("time -p ../tool", vec!["../tool"]),
        ("sudo ../tool", vec!["../tool"]),
        ("sudo env bash ../tool", vec!["../tool"]),
        ("bash -x ../tool", vec!["../tool"]),
        ("command ../tool", vec!["../tool"]),
        ("cd ..; ../tool", vec!["../tool"]),
        ("bash -e ../a.sh", vec!["../a.sh"]),
        ("node -e ../a.js", vec!["../a.js"]),
        ("node --eval ../a.js", vec!["../a.js"]),
        ("source ../env.sh", vec!["../env.sh"]),
        (". ../env.sh", vec!["../env.sh"]),
        ("../a.sh && ../b.sh", vec!["../a.sh", "../b.sh"]),
    ] {
        assert_eq!(fx.unpinned_of(said), outside_of(&paths), "{said}");
    }
}

// TS pinHere: tools, inline code and a big file only read leave nothing unpinned; the slate-box.test.ts Always case.
#[test]
fn pin_here_leaves_tools_inline_code_and_reads_bound() {
    let fx = Fx::new();
    for said in ["df -h /", "sleep 0.5", "bash -c 'echo two words'", "python3 -m http.server", "cat ../notes.txt"] {
        fx.expect_clear(c(said));
    }
    // run as a program only when relative; an absolute program is a tool of the computer
    for said in ["./tool", "/opt/tool ../x", "python3 -m ../mod", "echo ../tool"] {
        assert_eq!(fx.unpinned_of(said), vec![], "{said}");
    }

    // a big file the command only reads (not a script) is neither bound nor unpinned
    let big = Fx::new();
    big.sparse("big.log", 4 * 1024 * 1024 + 1);
    big.expect_clear(c("tail -c 3 big.log"));
    big.expect_clear(c("cat /proc/loadavg; tail -c 3 big.log"));

    // slate-box.test.ts: tools and interpreters outside the folder, a big file only read, a script inside spelled with
    // .., inline code and the slate's own files keep an Always
    let fx = Fx::new();
    fs::create_dir_all(fx.folder.join(".venv").join("bin")).unwrap();
    symlink("/usr/bin/env", fx.folder.join(".venv").join("bin").join("python3.12")).unwrap();
    fx.file("stats.py", "print('one')");
    fx.file("ok.sh", "true");
    fs::create_dir(fx.folder.join("lib")).unwrap();
    fx.sparse("big.log", 4 * 1024 * 1024 + 1);
    let said = "cat /proc/loadavg; /usr/bin/env true; tail -n 1 ../notes.txt; tail -c 3 big.log; bash lib/../ok.sh; \
        .venv/bin/python3.12 stats.py; bash -c 'echo two words'; python3 -c 'print(1)'; echo $HOME; cat ~/.profile; \
        bash $SLATE_DIR/x.sh";
    fx.expect_bound(c(said), &[("ok.sh", "b5bea41b6c623f7c"), ("stats.py", "a947cf982e4cfdac")]);
}

// TS pinHere-cwd: the command's cwd is where its words are joined.
#[test]
fn pin_here_follows_cwd() {
    let fx = Fx::new();
    fx.stats();
    fx.file("scripts/stats.sh", "echo one");
    fx.expect_bound(c("bash stats.sh"), &[("stats.sh", H)]);
    for cwd in ["scripts", "./scripts/../scripts/", fx.folder.join("scripts").to_str().unwrap()] {
        fx.expect_bound(cx("bash stats.sh", json!({"cwd": cwd})), &[("scripts/stats.sh", H)]);
    }
}

// TS pinHere-undef, pinHere-tool, pinHere-missing-folder: nothing to pin leaves nothing unpinned.
#[test]
fn pin_here_with_no_folder_or_a_missing_folder_pins_nothing() {
    let nothing = json!({"unpinned": []});
    assert_eq!(pv(&pin_here(None, &decl(c("bash stats.sh")))), nothing);
    assert_eq!(pv(&pin_here(Some("/nonexistent/x"), &decl(c("bash stats.sh")))), nothing);
    assert_eq!(
        pv(&pin_here(Some("/nonexistent/x"), &decl(c("bash ../away.sh ./x.sh")))),
        json!({"unpinned": [{"path": "../away.sh", "why": "outside"}]})
    );
    let fx = Fx::new();
    fx.stats();
    let folder = fx.folder.to_str().unwrap();
    let tool = decl(json!({"kind": "tool", "server": "s", "tool": "t"}));
    assert_eq!(pv(&pin_here(Some(folder), &tool)), nothing);
    let tool_then = decl(json!({"kind": "tool", "server": "s", "tool": "t", "then": "bash stats.sh"}));
    assert_eq!(pv(&pin_here(Some(folder), &tool_then)), nothing);
    let resource = decl(json!({"kind": "resource", "server": "s", "uri": "u"}));
    assert_eq!(pv(&pin_here(Some(folder), &resource)), nothing);
    assert_eq!(keyed(&pin_here(None, &decl(c("bash stats.sh")))), None);
}

// SCRIPT, rule 8

// TS SCRIPT: the suffixes in any case; a script that lands outside the folder is outside, any other name is not looked at.
#[test]
fn script_suffixes_are_case_insensitive() {
    let fx = Fx::new();
    for s in [
        "sh", "bash", "zsh", "fish", "ksh", "py", "js", "mjs", "cjs", "ts", "mts", "cts", "jsx", "tsx", "rb", "pl", "pm", "php", "lua",
        "r", "jl", "ps1", "awk", "tcl", "SH", "Py", "PS1", "R", "Tcl", "tSx",
    ] {
        let path = format!("../x.{s}");
        fx.expect_unpinned(c(&format!("cat {path}")), &[(path.as_str(), "outside", P_OUTSIDE)]);
    }
    for not in ["../x.txt", "../x.shx", "../x.sh.bak", "../xsh", "../x.c", "../x.json", "../x.rs", "../x.sh/"] {
        fx.expect_clear(c(&format!("cat {not}")));
    }
}

// keyed, cannot_pin, always_refused, rules 10 to 12

// TS pfiles.ts "two", "none", "phrase", "mix", "three": the words a sheet and a refused Always say.
#[test]
fn keyed_cannot_pin_and_always_refused_say_the_ts_words() {
    let two = pin_from(
        json!({"scripts": {"a.sh": "1"}, "unpinned": [{"path": "../a.sh", "why": "outside"}, {"path": "b.sh", "why": "special"}]}),
    );
    assert_eq!(keyed_v(&two), json!({"a.sh": "1", "../a.sh": "outside", "b.sh": "special"}));
    let order: Vec<String> = keyed(&two).unwrap().keys().cloned().collect();
    assert_eq!(order, strs(&["a.sh", "../a.sh", "b.sh"]));
    assert_eq!(
        cannot_pin(&two),
        Some("../a.sh is outside the thread's folder and b.sh is not a regular file, so wsp cannot pin them and it asks every time".into())
    );
    assert_eq!(
        always_refused("r", &two),
        Some(
            "$r names a script wsp cannot pin to hold an Always to: ../a.sh is outside the thread's folder and b.sh is not a regular file."
                .into()
        )
    );

    let none = pin_from(json!({"unpinned": []}));
    assert_eq!(keyed(&none), None);
    assert_eq!(cannot_pin(&none), None);
    assert_eq!(always_refused("r", &none), None);
    let scripts_only = pin_from(json!({"scripts": {"a": "1"}, "unpinned": []}));
    assert_eq!(keyed_v(&scripts_only), json!({"a": "1"}));
    assert_eq!(cannot_pin(&scripts_only), None);
    assert_eq!(always_refused("r", &scripts_only), None);

    let phrases = [
        ("outside", "p.sh is outside the thread's folder"),
        ("link", "p.sh is a link out of the thread's folder"),
        ("past", "p.sh is past what one command can pin (32 files, each up to 4 MiB)"),
        ("special", "p.sh is not a regular file"),
        ("built", "p.sh is a script wsp cannot read by name (a space, an escape or a $ in it)"),
        ("paths", "p.sh is past the 256 paths one command can name"),
    ];
    for (why, said) in phrases {
        let pin = pin_from(json!({"unpinned": [{"path": "p.sh", "why": why}]}));
        assert_eq!(keyed_v(&pin), json!({"p.sh": why}), "{why}");
        assert_eq!(cannot_pin(&pin), Some(format!("{said}, so wsp cannot pin it and it asks every time")), "{why}");
        assert_eq!(always_refused("r", &pin), Some(format!("$r names a script wsp cannot pin to hold an Always to: {said}.")), "{why}");
    }

    let mix = pin_from(json!({"unpinned": [{"path": "u.sh", "why": "unread"}, {"path": "o.sh", "why": "outside"}]}));
    assert_eq!(keyed_v(&mix), json!({"u.sh": "unread", "o.sh": "outside"}));
    assert_eq!(cannot_pin(&mix), Some("o.sh is outside the thread's folder, so wsp cannot pin them and it asks every time".into()));
    assert_eq!(
        always_refused("r", &mix),
        Some("$r names a script wsp cannot pin to hold an Always to: o.sh is outside the thread's folder.".into())
    );

    let three = pin_from(json!({"unpinned": [{"path": "a", "why": "built"}, {"path": "b", "why": "link"}, {"path": "c", "why": "past"}]}));
    assert_eq!(keyed_v(&three), json!({"a": "built", "b": "link", "c": "past"}));
    assert_eq!(
        cannot_pin(&three),
        Some("a is a script wsp cannot read by name (a space, an escape or a $ in it) and b is a link out of the thread's folder and c is past what one command can pin (32 files, each up to 4 MiB), so wsp cannot pin them and it asks every time".into())
    );
    assert_eq!(
        always_refused("r", &three),
        Some("$r names a script wsp cannot pin to hold an Always to: a is a script wsp cannot read by name (a space, an escape or a $ in it) and b is a link out of the thread's folder and c is past what one command can pin (32 files, each up to 4 MiB).".into())
    );
}

// TS pfiles.ts "unread": a pin with only unread entries has no sheet line, and the Always is refused for the computer.
#[test]
fn always_refused_for_an_unread_pin_names_the_computer_could_not_hash() {
    let one = pin_from(unread("stats.sh"));
    assert_eq!(keyed_v(&one), json!({"stats.sh": "unread"}));
    assert_eq!(cannot_pin(&one), None);
    assert_eq!(
        always_refused("stats", &one),
        Some(
            "$stats runs on the thread's machine and names stats.sh there, which its computer could not hash to hold an Always to.".into()
        )
    );

    let two = pin_from(json!({"unpinned": [{"path": "u.sh", "why": "unread"}, {"path": "v.sh", "why": "unread"}]}));
    assert_eq!(keyed_v(&two), json!({"u.sh": "unread", "v.sh": "unread"}));
    assert_eq!(cannot_pin(&two), None);
    assert_eq!(
        always_refused("r", &two),
        Some("$r runs on the thread's machine and names u.sh, v.sh there, which its computer could not hash to hold an Always to.".into())
    );

    let with_scripts = pin_from(json!({"scripts": {"a.sh": "1"}, "unpinned": [{"path": "u.sh", "why": "unread"}]}));
    assert_eq!(keyed_v(&with_scripts), json!({"a.sh": "1", "u.sh": "unread"}));
    assert_eq!(cannot_pin(&with_scripts), None);
    assert_eq!(
        always_refused("r", &with_scripts),
        Some("$r runs on the thread's machine and names u.sh there, which its computer could not hash to hold an Always to.".into())
    );
}

// pin_here, rule 13

// TS readHere: FS_HASH_FILES_MAX is 32, a repeat is not counted twice, any named regular file takes a place.
#[test]
fn pin_here_past_32_files_marks_the_rest_past() {
    let fx = Fx::new();
    let names: Vec<String> = (0..34).map(|n| format!("f{n:02}.sh")).collect();
    for name in &names {
        fx.file(name, "echo one");
    }
    fx.file("data.txt", "echo one");

    let said = format!("bash {} data.txt", names.join(" "));
    let pin = fx.pin_cmd(&said);
    let scripts: serde_json::Map<String, Value> = names[..32].iter().map(|n| (n.clone(), json!(H))).collect();
    assert_eq!(pv(&pin), json!({"scripts": scripts, "unpinned": [{"path": "f32.sh", "why": "past"}, {"path": "f33.sh", "why": "past"}]}));
    let key_order: Vec<String> = keyed(&pin).unwrap().keys().cloned().collect();
    assert_eq!(key_order, names);
    assert_eq!(cannot_pin(&pin), Some(format!("f32.sh {P_PAST} and f33.sh {P_PAST}, so wsp cannot pin them and it asks every time")));
    assert_eq!(
        always_refused("run", &pin),
        Some(format!("$run names a script wsp cannot pin to hold an Always to: f32.sh {P_PAST} and f33.sh {P_PAST}."))
    );

    // exactly 32, and a repeat of one of them, fits
    let exact = format!("bash {} {}", names[..32].join(" "), names[0]);
    fx.expect_bound(c(&exact), &names[..32].iter().map(|n| (n.as_str(), H)).collect::<Vec<_>>());

    // a file that is not a script takes a place as well: data.txt first pushes f31 and f32 past
    let first = format!("cat data.txt {} f32.sh", names[..32].join(" "));
    let pin = fx.pin_cmd(&first);
    let mut scripts: serde_json::Map<String, Value> = serde_json::Map::new();
    scripts.insert("data.txt".into(), json!(H));
    for n in &names[..31] {
        scripts.insert(n.clone(), json!(H));
    }
    assert_eq!(pv(&pin), json!({"scripts": scripts, "unpinned": [{"path": "f31.sh", "why": "past"}, {"path": "f32.sh", "why": "past"}]}));
}

// box_pins, rule 15

// TS asked, boxpin, boxpin-noread, boxpin-cwd, flags and link probes: one ask per key, hashes cut to 16, a reply's lists.
#[tokio::test(flavor = "current_thread")]
async fn box_pins_reads_once_per_key_and_cuts_hashes_to_16() {
    let asked: Asked = Default::default();
    let pins = box_pins(answering(&asked, reply(json!({"stats.sh": H32}))), None);
    let tool_then = json!({"kind": "tool", "server": "s", "tool": "t", "then": "x.sh"});
    pins.read(
        "t",
        &[
            acme("bash stats.sh"),
            acme("bash stats.sh"),
            acme("df -h"),
            ask(None, c("bash x.sh")),
            ask(Some("/root/acme"), tool_then),
            ask(Some("/root/acme"), cx("bash stats.sh", json!({"cwd": "scripts"}))),
        ],
        false,
    )
    .await;
    assert_eq!(
        sorted(&asked),
        vec![
            ("/root/acme".to_string(), strs(&["/root/acme/scripts/stats.sh"])),
            ("/root/acme".to_string(), strs(&["/root/acme/stats.sh"])),
        ]
    );
    let bound = json!({"scripts": {"stats.sh": "0123456789abcdef"}, "unpinned": []});
    assert_eq!(pin_at(&pins, "t", Some("/root/acme"), c("bash stats.sh")), bound);
    assert_eq!(pin_at(&pins, "t", Some("/root/acme"), cx("bash stats.sh", json!({"cwd": "scripts"}))), bound);
    assert_eq!(pin_at(&pins, "t", Some("/root/acme"), c("bash other.sh")), unread("other.sh"));
    assert_eq!(pin_at(&pins, "t", Some("/root/other"), c("bash stats.sh")), unread("stats.sh"));
    assert_eq!(pin_at(&pins, "t2", Some("/root/acme"), c("bash stats.sh")), unread("stats.sh"));
    // a command that names no path needs no reading, nor does one with no folder or one that is not a cmd
    let nothing = json!({"unpinned": []});
    assert_eq!(pin_at(&pins, "t", Some("/root/acme"), c("df -h")), nothing);
    assert_eq!(pin_at(&pins, "t", None, c("bash stats.sh")), nothing);
    assert_eq!(pin_at(&pins, "t", Some("/root/acme"), json!({"kind": "tool", "server": "s", "tool": "t"})), nothing);
    // the pin is the one a pin_of with the reading gives: keyed and spoken the same
    let pin = pins.pin("t", Some("/root/acme"), &decl(c("bash stats.sh")));
    assert_eq!(keyed_v(&pin), json!({"stats.sh": "0123456789abcdef"}));
    assert_eq!(cannot_pin(&pin), None);

    // a reply's lists become the whys of the scripts they name, in the order the command names them
    let lists = reply_lists(json!({"a.sh": H32}), &["/root/acme/../x.sh"], &["/root/acme/big.sh"], &["/root/acme/fifo.sh"]);
    let said = "bash a.sh /root/acme/../x.sh big.sh fifo.sh link.sh data.txt";
    let on_lists = box_pins(
        daemon(move |_, _| {
            let r = lists.clone();
            async move { Ok(r) }.boxed_local()
        }),
        None,
    );
    on_lists.read("t", &[acme(said)], false).await;
    assert_eq!(
        pin_at(&on_lists, "t", Some("/root/acme"), c(said)),
        json!({
            "scripts": {"a.sh": "0123456789abcdef"},
            "unpinned": [
                {"path": "/root/acme/../x.sh", "why": "outside"},
                {"path": "big.sh", "why": "past"},
                {"path": "fifo.sh", "why": "special"},
            ]
        })
    );
    let link = reply_lists(json!({}), &["/root/acme/link.sh", "/root/acme/data.txt"], &[], &[]);
    let on_link = box_pins(
        daemon(move |_, _| {
            let r = link.clone();
            async move { Ok(r) }.boxed_local()
        }),
        None,
    );
    on_link.read("t", &[acme("bash link.sh data.txt")], false).await;
    assert_eq!(
        pin_at(&on_link, "t", Some("/root/acme"), c("bash link.sh data.txt")),
        json!({"unpinned": [{"path": "link.sh", "why": "link"}]})
    );
}

// TS slate-box.test.ts:1132: no answer within the wait leaves the command unread; a napping computer is asked nothing.
#[tokio::test(flavor = "current_thread")]
async fn box_pins_leaves_a_command_unpinned_where_the_daemon_does_not_answer_in_time_and_asks_nothing_asleep() {
    let calls = Rc::new(Cell::new(0u32));
    let looked = Rc::new(Cell::new(0u32));
    let on: HashOn = {
        let calls = calls.clone();
        Rc::new(move |_, _| {
            calls.set(calls.get() + 1);
            futures::future::pending::<Result<HashRead, String>>().boxed_local()
        })
    };
    let pins = box_pins(
        {
            let looked = looked.clone();
            Box::new(move |_| {
                looked.set(looked.get() + 1);
                Some(on.clone())
            })
        },
        Some(20),
    );
    let stats = c("bash stats.sh");
    let unpinned = |d: Value| -> Vec<String> {
        pins.pin("t1", Some("/root/acme"), &decl(d))
            .unpinned
            .iter()
            .map(|u| serde_json::to_value(u).unwrap()["path"].as_str().unwrap().to_string())
            .collect()
    };
    pins.read("t1", &[acme("bash stats.sh")], false).await;
    assert_eq!(unpinned(stats.clone()), strs(&["stats.sh"]));
    assert_eq!(pin_at(&pins, "t1", Some("/root/acme"), stats.clone()), unread("stats.sh"));
    assert_eq!((calls.get(), looked.get()), (1, 1));
    pins.read("t1", &[acme("bash stats.sh")], true).await;
    assert_eq!((calls.get(), looked.get()), (1, 1));
    assert_eq!(pin_at(&pins, "t1", Some("/root/acme"), stats), unread("stats.sh"));
    assert_eq!(unpinned(c("df -h")), Vec::<String>::new());
}

// TS boxPins `at` counter: a read that finishes after a newer one has finished does not replace it.
#[tokio::test(flavor = "current_thread")]
async fn box_pins_an_older_read_finishing_late_does_not_replace_a_newer() {
    let calls = Rc::new(Cell::new(0u8));
    let (tx, rx) = tokio::sync::oneshot::channel::<HashRead>();
    let rx = Rc::new(RefCell::new(Some(rx)));
    let on: HashOn = {
        let calls = calls.clone();
        Rc::new(move |_, _| {
            let n = calls.get();
            calls.set(n + 1);
            if n == 0 {
                let rx = rx.borrow_mut().take().unwrap();
                async move { rx.await.map_err(|e| e.to_string()) }.boxed_local()
            } else {
                let letter = (b'a' + n) as char;
                async move { Ok(reply(json!({"stats.sh": letter.to_string().repeat(32)}))) }.boxed_local()
            }
        })
    };
    let pins = box_pins(Box::new(move |_| Some(on.clone())), None);
    let asks = [acme("bash stats.sh")];
    let second_done = Cell::new(false);
    futures::join!(
        pins.read("t", &asks, false),
        async {
            pins.read("t", &asks, false).await;
            second_done.set(true);
        },
        async {
            while !second_done.get() {
                tokio::task::yield_now().await;
            }
            tx.send(reply(json!({"stats.sh": "a".repeat(32)}))).unwrap();
        },
    );
    assert_eq!(calls.get(), 2);
    assert_eq!(
        pin_at(&pins, "t", Some("/root/acme"), c("bash stats.sh")),
        json!({"scripts": {"stats.sh": "b".repeat(16)}, "unpinned": []})
    );
    // a read that starts later does replace it
    pins.read("t", &asks, false).await;
    assert_eq!(
        pin_at(&pins, "t", Some("/root/acme"), c("bash stats.sh")),
        json!({"scripts": {"stats.sh": "c".repeat(16)}, "unpinned": []})
    );
}

// TS boxPins read: no ask deletes the thread's reading, drop deletes it, an asleep read replaces it, threads are apart.
#[tokio::test(flavor = "current_thread")]
async fn box_pins_with_no_asks_drops_the_reading() {
    let asked: Asked = Default::default();
    let pins = box_pins(answering(&asked, reply(json!({"stats.sh": H32}))), None);
    let bound = json!({"scripts": {"stats.sh": "0123456789abcdef"}, "unpinned": []});
    let at = |pins: &BoxPins, thread: &str| pin_at(pins, thread, Some("/root/acme"), c("bash stats.sh"));

    pins.read("t", &[acme("bash stats.sh")], false).await;
    assert_eq!(at(&pins, "t"), bound);
    // commands naming no path, no folder, or none at all ask nothing and clear the reading
    pins.read("t", &[acme("df -h")], false).await;
    assert_eq!(asked.borrow().len(), 1);
    assert_eq!(at(&pins, "t"), unread("stats.sh"));
    pins.read("t", &[acme("bash stats.sh")], false).await;
    assert_eq!(at(&pins, "t"), bound);
    pins.read("t", &[ask(None, c("bash stats.sh"))], false).await;
    assert_eq!(at(&pins, "t"), unread("stats.sh"));
    pins.read("t", &[acme("bash stats.sh")], false).await;
    pins.read("t", &[], false).await;
    assert_eq!(at(&pins, "t"), unread("stats.sh"));
    assert_eq!(asked.borrow().len(), 3);

    // drop clears one thread's reading and leaves the others
    pins.read("t", &[acme("bash stats.sh")], false).await;
    pins.read("t2", &[acme("bash stats.sh")], false).await;
    pins.drop_thread("t");
    assert_eq!(at(&pins, "t"), unread("stats.sh"));
    assert_eq!(at(&pins, "t2"), bound);

    // an asleep read asks nothing and replaces the held reading with an empty one
    let before = asked.borrow().len();
    pins.read("t2", &[acme("bash stats.sh")], true).await;
    assert_eq!(asked.borrow().len(), before);
    assert_eq!(at(&pins, "t2"), unread("stats.sh"));
}

// TS boxPins read: an ask that throws, a thread with no daemon, or a daemon that fails one of two asks.
#[tokio::test(flavor = "current_thread")]
async fn box_pins_a_failed_ask_reads_unread() {
    let failing = box_pins(daemon(|_, _| async { Err("a daemon too old to hash".to_string()) }.boxed_local()), None);
    failing.read("t", &[acme("bash stats.sh")], false).await;
    assert_eq!(pin_at(&failing, "t", Some("/root/acme"), c("bash stats.sh")), unread("stats.sh"));
    let pin = failing.pin("t", Some("/root/acme"), &decl(c("bash stats.sh")));
    assert_eq!(keyed_v(&pin), json!({"stats.sh": "unread"}));
    assert_eq!(cannot_pin(&pin), None);

    let on: HashOn = Rc::new(|_, _| async { Ok(reply(json!({"stats.sh": H32}))) }.boxed_local());
    let nobody = box_pins(Box::new(move |thread| if thread == "t2" { None } else { Some(on.clone()) }), None);
    nobody.read("t1", &[acme("bash stats.sh")], false).await;
    nobody.read("t2", &[acme("bash stats.sh")], false).await;
    assert_eq!(
        pin_at(&nobody, "t1", Some("/root/acme"), c("bash stats.sh")),
        json!({"scripts": {"stats.sh": "0123456789abcdef"}, "unpinned": []})
    );
    assert_eq!(pin_at(&nobody, "t2", Some("/root/acme"), c("bash stats.sh")), unread("stats.sh"));

    // one ask failing leaves the others read
    let mixed = box_pins(
        daemon(|_, paths| {
            if paths.iter().any(|p| p.contains("bad")) {
                async { Err("old".to_string()) }.boxed_local()
            } else {
                async { Ok(reply(json!({"good.sh": H32}))) }.boxed_local()
            }
        }),
        None,
    );
    mixed.read("t", &[acme("bash good.sh"), acme("bash bad.sh")], false).await;
    assert_eq!(
        pin_at(&mixed, "t", Some("/root/acme"), c("bash good.sh")),
        json!({"scripts": {"good.sh": "0123456789abcdef"}, "unpinned": []})
    );
    assert_eq!(pin_at(&mixed, "t", Some("/root/acme"), c("bash bad.sh")), unread("bad.sh"));

    // no reading at all: a command with no path needs none, one that names a path is unread
    let never = box_pins(Box::new(|_| None), None);
    assert_eq!(pin_at(&never, "t", Some("/root/acme"), c("df -h")), json!({"unpinned": []}));
    assert_eq!(pin_at(&never, "t", Some("/root/acme"), c("bash a.sh")), unread("a.sh"));
}
