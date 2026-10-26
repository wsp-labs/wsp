// SPDX-License-Identifier: AGPL-3.0-only
//! image-vault.test.ts: the image vault, what the seal takes off the builder, what an import lands, the one hash rule,
//! and the member rule the seal and the copy both read. The last test of the TS file (the guard between the catalog's
//! login paths and the pack's copy destinations) reads `copiedLoginDests` of golden-import.ts, which the pack owns and
//! the engine does not port, so it has no test here.
mod golden_roads_common;

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::io::Write;
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use base64::Engine as _;
use flate2::write::GzEncoder;
use flate2::Compression;
use golden_roads_common::run;
use sha2::{Digest, Sha256};
use wsp_catalog::roads::GUEST_HOME;
use wsp_engine::errors::{EngineError, Result as EngineResult, RoadError};
use wsp_engine::fetch::{Fetch, FetchRequest, FetchResponse};
use wsp_engine::image_vault::{
    export_image_vault, image_hash, import_image_vault, refuse_foreign_members, refused_member, vault_members, MemberKind, VaultMember,
};
use wsp_engine::machine::{ExecOptions, ExecResult, Machine, MachineKind, MachineLife, MachineState, RunOptions, SnapshotOptions};
use wsp_engine::vault::{tar_of, TarEntry};
use wsp_engine::vault::{ReadRoad, VaultOptions, EXEC_READ_CAP};
use wsp_wire::wire::machine_link::VaultRoad;

fn file(path: &str, mode: u32, content: &str) -> TarEntry {
    TarEntry::File { path: path.to_string(), mode, content: content.as_bytes().to_vec() }
}

fn tar_bytes() -> Vec<u8> {
    tar_of(&[file("root/.codex/auth.json", 0o600, "{}")]).unwrap()
}

#[derive(Default)]
struct GuestOpts {
    size: Option<u64>,
    probe_exit: Option<i64>,
    probe_err: Option<&'static str>,
    tar: Option<Vec<u8>>,
}

/// image-vault.test.ts:19 `guest`: a guest holding the paths named, whose tar writes an archive of them, and whose
/// signed URLs the fetch stub answers. `tar` plants another archive, which is what a builder that writes its own bytes
/// hands back.
struct Guest {
    present: Vec<String>,
    o: GuestOpts,
    archived: Vec<u8>,
    exec_cmds: RefCell<Vec<String>>,
    runs: RefCell<Vec<String>>,
    files: Rc<RefCell<HashMap<String, Vec<u8>>>>,
}

struct GuestFetch {
    files: Rc<RefCell<HashMap<String, Vec<u8>>>>,
    puts: RefCell<Vec<(String, Vec<u8>)>>,
    calls: Cell<u32>,
}

fn guest(present: &[&str], o: GuestOpts) -> (Arc<Guest>, Arc<GuestFetch>) {
    let archived = o.tar.clone().unwrap_or_else(|| {
        let entries: Vec<TarEntry> = present.iter().map(|p| file(p.trim_start_matches('/'), 0o600, "x")).collect();
        tar_of(&entries).unwrap()
    });
    let files: Rc<RefCell<HashMap<String, Vec<u8>>>> = Rc::new(RefCell::new(HashMap::new()));
    let g = Arc::new(Guest {
        present: present.iter().map(|p| p.to_string()).collect(),
        o,
        archived,
        exec_cmds: RefCell::new(Vec::new()),
        runs: RefCell::new(Vec::new()),
        files: files.clone(),
    });
    let fetch = Arc::new(GuestFetch { files, puts: RefCell::new(Vec::new()), calls: Cell::new(0) });
    (g, fetch)
}

/// The text between the first `'` after `prefix` and the next one: the path a `tar czf '<out>'`-style command names.
fn quoted_after<'a>(cmd: &'a str, prefix: &str) -> Option<&'a str> {
    let rest = cmd.strip_prefix(prefix)?;
    rest.split('\'').next()
}

fn encode_uri_component(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

fn result(code: i64, stdout: &str, stderr: &str) -> ExecResult {
    ExecResult { exit_code: code.into(), stdout: stdout.to_string(), stderr: stderr.to_string() }
}

impl Guest {
    fn run_cmd(&self, cmd: &str) -> ExecResult {
        self.exec_cmds.borrow_mut().push(cmd.to_string());
        if cmd.starts_with("for p in ") {
            let mut out = self.present.join("\n");
            if !self.present.is_empty() {
                out.push('\n');
            }
            return result(self.o.probe_exit.unwrap_or(0), &out, self.o.probe_err.unwrap_or(""));
        }
        if let Some(path) = quoted_after(cmd, "tar czf '") {
            self.files.borrow_mut().insert(path.to_string(), self.archived.clone());
        }
        if let Some(path) = quoted_after(cmd, "wc -c < '") {
            let size = self.o.size.unwrap_or_else(|| self.files.borrow().get(path).map_or(0, |b| b.len() as u64));
            return result(0, &format!("{size}\n"), "");
        }
        if let Some(path) = quoted_after(cmd, "base64 < '") {
            let bytes = self.files.borrow().get(path).cloned().unwrap_or_default();
            return result(0, &base64::engine::general_purpose::STANDARD.encode(bytes), "");
        }
        result(0, "", "")
    }
    fn runs(&self) -> Vec<String> {
        self.runs.borrow().clone()
    }
    fn cmds(&self) -> Vec<String> {
        self.exec_cmds.borrow().clone()
    }
}

#[async_trait(?Send)]
impl Machine for Guest {
    fn id(&self) -> &str {
        "mg"
    }
    fn kind(&self) -> MachineKind {
        MachineKind::Sandbox
    }
    async fn exec(&self, cmd: &str, _opts: ExecOptions) -> EngineResult<ExecResult> {
        Ok(self.run_cmd(cmd))
    }
    async fn run(&self, script: &str, _opts: RunOptions) -> EngineResult<ExecResult> {
        self.runs.borrow_mut().push(script.to_string());
        Ok(self.run_cmd(script))
    }
    async fn snapshot(&self, _name: &str, _life: MachineLife, _opts: SnapshotOptions) -> EngineResult<String> {
        Ok("snap".to_string())
    }
    async fn pause(&self, _wait_ms: Option<u64>) -> EngineResult<()> {
        Ok(())
    }
    async fn resume(&self, _signal: Option<&wsp_engine::Cancel>) -> EngineResult<()> {
        Ok(())
    }
    async fn kill(&self) -> EngineResult<()> {
        Ok(())
    }
    async fn state(&self) -> EngineResult<MachineState> {
        Ok(MachineState::Running)
    }
    async fn download_url(&self, path: &str) -> EngineResult<String> {
        Ok(format!("https://signed.example/dl?path={}", encode_uri_component(path)))
    }
    async fn upload_url(&self, path: &str) -> EngineResult<String> {
        Ok(format!("https://signed.example/ul?path={}", encode_uri_component(path)))
    }
}

#[async_trait(?Send)]
impl Fetch for GuestFetch {
    async fn fetch(&self, req: FetchRequest) -> Result<FetchResponse, RoadError> {
        self.calls.set(self.calls.get() + 1);
        if req.method == "PUT" {
            self.puts.borrow_mut().push((req.url.clone(), req.body.clone().unwrap_or_default()));
            return Ok(FetchResponse { status: 200, headers: Vec::new(), body: Vec::new() });
        }
        let parsed = url::Url::parse(&req.url).expect("a url");
        let path = parsed.query_pairs().find(|(k, _)| k == "path").map(|(_, v)| v.to_string()).unwrap_or_default();
        let bytes = self.files.borrow().get(&path).cloned();
        Ok(match bytes {
            None => FetchResponse { status: 404, headers: Vec::new(), body: b"gone".to_vec() },
            Some(b) => FetchResponse { status: 200, headers: Vec::new(), body: b },
        })
    }
}

fn opts_with(fetch: &Arc<GuestFetch>) -> VaultOptions {
    VaultOptions { fetch: Some(fetch.clone()), ..VaultOptions::default() }
}

fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}

fn machine_of(g: &Arc<Guest>) -> Arc<dyn Machine> {
    g.clone()
}

// image-vault.test.ts:59
#[test]
fn archives_only_the_paths_the_machine_has_counts_them_and_hashes_the_tar_it_brought_down() {
    let here = format!("{GUEST_HOME}/.codex/auth.json");
    let gone = format!("{GUEST_HOME}/.aws");
    let (g, fetch) = guest(&[&here], GuestOpts::default());
    let vault = run(export_image_vault(&*g, &[here.clone(), gone], &opts_with(&fetch))).unwrap();
    assert_eq!(vault.paths, 1);
    assert_eq!(vault.held, vec![here]);
    assert_eq!(vault.tar, g.archived);
    assert_eq!(vault.sha256, hex::encode(Sha256::digest(&g.archived)));
    let cmds = g.cmds();
    let tar = cmds.iter().find(|c| c.starts_with("tar czf")).expect("a tar command");
    assert!(tar.contains("'root/.codex/auth.json'"), "{tar}");
    assert!(!tar.contains(".aws"), "{tar}");
}

// image-vault.test.ts:73
#[test]
fn a_machine_whose_provider_mints_no_download_url_hands_the_archive_back_through_one_command() {
    let here = format!("{GUEST_HOME}/.codex/auth.json");
    let (g, fetch) = guest(&[&here], GuestOpts::default());
    let opts = VaultOptions { read_road: ReadRoad::Exec, ..VaultOptions::default() };
    let vault = run(export_image_vault(&*g, std::slice::from_ref(&here), &opts)).unwrap();
    assert_eq!(vault.tar, g.archived);
    assert_eq!(vault.paths, 1);
    assert!(g.cmds().iter().any(|c| c.starts_with("base64 < ")));
    assert_eq!(fetch.calls.get(), 0, "no fetch");
}

// image-vault.test.ts:83
#[test]
fn an_archive_too_big_for_that_road_is_refused_on_its_size_before_any_of_it_is_read_back() {
    let here = format!("{GUEST_HOME}/.aws");
    let (g, _fetch) = guest(&[&here], GuestOpts { size: Some(EXEC_READ_CAP + 1), ..GuestOpts::default() });
    let opts = VaultOptions { read_road: ReadRoad::Exec, ..VaultOptions::default() };
    let err = run(export_image_vault(&*g, std::slice::from_ref(&here), &opts)).err().expect("a refusal");
    assert!(matches!(err, EngineError::VaultTooLarge { .. }), "{err:?}");
    assert!(!g.cmds().iter().any(|c| c.starts_with("base64 < ")));
}

// image-vault.test.ts:90
#[test]
fn a_probe_the_machine_would_not_run_is_a_failure_not_an_empty_answer_nothing_is_archived_and_nothing_is_sealed() {
    let (g, fetch) = guest(&[], GuestOpts { probe_exit: Some(127), probe_err: Some("bash: for: command not found"), ..GuestOpts::default() });
    let err = run(export_image_vault(&*g, &[format!("{GUEST_HOME}/.codex/auth.json")], &opts_with(&fetch))).err().expect("a failure");
    assert!(err.to_string().contains("would not say which"), "{err}");
    assert!(!g.cmds().iter().any(|c| c.starts_with("tar czf")));
}

// image-vault.test.ts:96
#[test]
fn a_machine_holding_none_of_them_archives_nothing_and_says_so() {
    let (g, fetch) = guest(&[], GuestOpts::default());
    let vault = run(export_image_vault(&*g, &[format!("{GUEST_HOME}/.aws")], &opts_with(&fetch))).unwrap();
    assert_eq!(vault.paths, 0);
    assert!(g.cmds().iter().any(|c| c.contains("--no-recursion")));
}

// image-vault.test.ts:103
#[test]
fn lands_the_archive_over_the_root_in_one_upload_merging_into_what_the_pack_already_wrote() {
    let (g, fetch) = guest(&[], GuestOpts::default());
    run(import_image_vault(machine_of(&g), &tar_bytes(), &opts_with(&fetch))).unwrap();
    assert_eq!(fetch.puts.borrow().len(), 1);
    let runs = g.runs();
    let script = runs.iter().find(|r| r.contains("tar xzf")).expect("an extract");
    assert!(script.contains("tar xzf - -C '/' --no-same-owner"), "{script}");
    assert!(!script.contains("--recursive-unlink"), "{script}");
}

// image-vault.test.ts:112
#[test]
fn the_hash_is_one_rule_stable_moved_by_the_recipe_moved_by_the_vault_and_a_record_with_no_vault_is_its_own() {
    assert_eq!(image_hash("r1", Some("v1")), image_hash("r1", Some("v1")));
    assert_ne!(image_hash("r1", Some("v1")), image_hash("r2", Some("v1")));
    assert_ne!(image_hash("r1", Some("v1")), image_hash("r1", Some("v2")));
    assert_ne!(image_hash("r1", None), image_hash("r1", Some(&"0".repeat(64))));
    assert_eq!(image_hash("r1", None).len(), 64);
}

// Tar header blocks written by hand: the archives a hostile builder would write cannot come out of `tar_of`, which
// writes well-formed ustar members alone.
const BLOCK: usize = 512;

#[derive(Default)]
struct H {
    name: &'static str,
    kind: Option<&'static str>,
    size: Option<usize>,
    target: Option<&'static str>,
    magic: Option<&'static [u8]>,
    checksum: Option<&'static [u8]>,
    size_field: Option<[u8; 12]>,
}

fn put(h: &mut [u8], at: usize, len: usize, bytes: &[u8]) {
    let n = bytes.len().min(len);
    h[at..at + n].copy_from_slice(&bytes[..n]);
}

/// image-vault.test.ts:142 `header`.
fn header(o: H) -> Vec<u8> {
    let mut h = vec![0u8; BLOCK];
    put(&mut h, 0, 100, o.name.as_bytes());
    put(&mut h, 100, 8, b"0000600\0");
    put(&mut h, 108, 8, b"0000000\0");
    put(&mut h, 116, 8, b"0000000\0");
    match o.size_field {
        Some(field) => h[124..136].copy_from_slice(&field),
        None => put(&mut h, 124, 12, format!("{:0>11o}\0", o.size.unwrap_or(0)).as_bytes()),
    }
    put(&mut h, 136, 12, b"14000000000\0");
    put(&mut h, 148, 8, b"        ");
    put(&mut h, 156, 1, o.kind.unwrap_or("0").as_bytes());
    if let Some(target) = o.target {
        put(&mut h, 157, 100, target.as_bytes());
    }
    put(&mut h, 257, 6, o.magic.unwrap_or(b"ustar\0"));
    put(&mut h, 263, 2, b"00");
    let sum: u32 = h.iter().map(|b| *b as u32).sum();
    match o.checksum {
        Some(c) => put(&mut h, 148, 8, c),
        None => put(&mut h, 148, 8, format!("{sum:0>6o}\0 ").as_bytes()),
    }
    h
}

fn filled(text: &str) -> Vec<u8> {
    let n = text.len().div_ceil(BLOCK).max(1) * BLOCK;
    let mut b = vec![0u8; n];
    b[..text.len()].copy_from_slice(text.as_bytes());
    b
}

fn gzip(bytes: &[u8]) -> Vec<u8> {
    let mut enc = GzEncoder::new(Vec::new(), Compression::default());
    enc.write_all(bytes).unwrap();
    enc.finish().unwrap()
}

fn archive(blocks: &[Vec<u8>]) -> Vec<u8> {
    let mut raw: Vec<u8> = blocks.concat();
    raw.extend(vec![0u8; BLOCK * 2]);
    gzip(&raw)
}

fn held() -> Vec<String> {
    strings(&["/root/.codex/auth.json", "/root/.config/gh"])
}

fn names_of(members: &[VaultMember]) -> Vec<String> {
    members.iter().map(|m| m.name.clone()).collect()
}

fn refusal(road: VaultRoad, tar: &[u8], held: &[String]) -> String {
    refuse_foreign_members(road, tar, held).err().expect("a refusal").to_string()
}

fn members_refusal(tar: &[u8]) -> String {
    vault_members(VaultRoad::Import, tar).err().expect("a refusal").to_string()
}

fn member(name: &str, kind: MemberKind, target: Option<&str>) -> VaultMember {
    VaultMember { name: name.to_string(), kind, target: target.map(str::to_string) }
}

// image-vault.test.ts:165
#[test]
fn an_archive_whose_members_are_all_under_the_asked_paths_reads_them_and_lands_as_today() {
    let tar = tar_of(&[
        file("root/.codex/auth.json", 0o600, "{}"),
        TarEntry::Dir { path: "root/.config/gh".into(), mode: 0o700 },
        file("root/.config/gh/hosts.yml", 0o600, "x"),
        TarEntry::Link { path: "root/.config/gh/link".into(), target: "hosts.yml".into() },
    ])
    .unwrap();
    assert_eq!(
        names_of(&vault_members(VaultRoad::Import, &tar).unwrap()),
        strings(&["root/.codex/auth.json", "root/.config/gh", "root/.config/gh/hosts.yml", "root/.config/gh/link"])
    );
    refuse_foreign_members(VaultRoad::Import, &tar, &held()).expect("no refusal");
    let (g, fetch) = guest(&[], GuestOpts::default());
    run(import_image_vault(machine_of(&g), &tar, &opts_with(&fetch))).unwrap();
    assert_eq!(fetch.puts.borrow().len(), 1);
}

// image-vault.test.ts:179
#[test]
fn a_member_outside_the_asked_paths_a_name_that_walks_out_a_link_out_and_a_device_node_each_refuse_the_whole_archive() {
    let cases: Vec<(&str, Vec<u8>, &str)> = vec![
        ("etc/cron.d/x", tar_of(&[file("etc/cron.d/x", 0o644, "* * * * * root sh")]).unwrap(), "etc/cron.d/x"),
        ("root/../etc/x", tar_of(&[file("root/../etc/x", 0o644, "x")]).unwrap(), "walks out"),
        ("a symbolic link out", tar_of(&[TarEntry::Link { path: "root/.config/gh".into(), target: "/etc".into() }]).unwrap(), "points at /etc"),
        (
            "a hard link out of a held folder",
            archive(&[header(H { name: "root/.config/gh/x", kind: Some("1"), target: Some("etc/shadow"), ..H::default() })]),
            "points at etc/shadow",
        ),
        ("a device node", archive(&[header(H { name: "root/.codex/auth.json", kind: Some("3"), ..H::default() })]), "type flag is 3"),
    ];
    for (what, tar, reads) in cases {
        let (g, fetch) = guest(&[], GuestOpts::default());
        let import = refusal(VaultRoad::Import, &tar, &held());
        assert!(import.contains(reads), "{what}: {import}");
        assert!(import.contains("nothing of it was imported"), "{what}: {import}");
        let seal = refusal(VaultRoad::Seal, &tar, &held());
        assert!(seal.contains("the seal is refused and no version is recorded"), "{what}: {seal}");
        assert!(fetch.puts.borrow().is_empty(), "{what}");
        assert!(g.cmds().is_empty(), "{what}");
    }
}

// image-vault.test.ts:197
#[test]
fn a_pax_size_is_honoured_so_a_header_planted_inside_a_members_data_is_data_and_the_member_behind_it_is_read() {
    let tar = archive(&[
        header(H { name: "root/.codex/auth.json", kind: Some("x"), size: Some(13), ..H::default() }),
        filled("13 size=1024\n"),
        header(H { name: "root/.codex/auth.json", size: Some(512), ..H::default() }),
        filled("the first block of the member's data"),
        header(H { name: "root/.config/gh/hosts.yml", size: Some(512), ..H::default() }),
        header(H { name: "etc/cron.d/x", ..H::default() }),
    ]);
    assert!(refusal(VaultRoad::Import, &tar, &held()).contains("etc/cron.d/x"));
}

// image-vault.test.ts:209
#[test]
fn a_pax_path_and_linkpath_are_what_the_member_is_judged_by() {
    let named = archive(&[
        header(H { name: "root/.codex/auth.json", kind: Some("x"), size: Some(25), ..H::default() }),
        filled("25 path=etc/cron.d/x\n"),
        header(H { name: "root/.codex/auth.json", ..H::default() }),
    ]);
    assert!(refusal(VaultRoad::Import, &named, &held()).contains("etc/cron.d/x"));
    let pointed = archive(&[
        header(H { name: "root/.codex/auth.json", kind: Some("x"), size: Some(26), ..H::default() }),
        filled("26 linkpath=/etc/shadow\n"),
        header(H { name: "root/.codex/auth.json", kind: Some("2"), target: Some("auth.json"), ..H::default() }),
    ]);
    assert!(refusal(VaultRoad::Import, &pointed, &held()).contains("points at /etc/shadow"));
}

// image-vault.test.ts:216
#[test]
fn a_gnu_long_name_is_honoured_for_the_header_behind_it_and_a_long_name_beside_a_long_link_target_is_one_member() {
    let long = format!("etc/{}/x", "d".repeat(120));
    let tar = archive(&[
        header(H { name: "././@LongLink", kind: Some("L"), size: Some(long.len() + 1), ..H::default() }),
        filled(&format!("{long}\0")),
        header(H { name: "root/.codex/auth.json", ..H::default() }),
    ]);
    assert!(refusal(VaultRoad::Import, &tar, &held()).contains(&long));
    let target = format!("/etc/{}/secret", "e".repeat(120));
    let both = archive(&[
        header(H { name: "././@LongLink", kind: Some("K"), size: Some(target.len() + 1), ..H::default() }),
        filled(&format!("{target}\0")),
        header(H { name: "././@LongLink", kind: Some("L"), size: Some(long.len() + 1), ..H::default() }),
        filled(&format!("{long}\0")),
        header(H { name: "root/.config/gh/link", kind: Some("2"), target: Some("hosts.yml"), ..H::default() }),
    ]);
    assert!(refusal(VaultRoad::Import, &both, &held()).contains(&long));
    let twice = archive(&[
        header(H { name: "x", kind: Some("L"), size: Some(6), ..H::default() }),
        filled("etc/x\0"),
        header(H { name: "x", kind: Some("L"), size: Some(6), ..H::default() }),
        filled("etc/y\0"),
        header(H { name: "root/.codex/auth.json", ..H::default() }),
    ]);
    assert!(members_refusal(&twice).contains("two long name entries of the same kind"));
}

// image-vault.test.ts:233
#[test]
fn a_header_that_fails_its_own_checksum_is_refused_and_the_members_behind_it_are_never_read() {
    let tar = archive(&[
        header(H { name: "root/.codex/auth.json", checksum: Some(b"000000\0 "), ..H::default() }),
        header(H { name: "etc/cron.d/x", ..H::default() }),
    ]);
    let said = members_refusal(&tar);
    assert!(said.contains("fails its own checksum"), "{said}");
    assert!(!said.contains("etc/cron.d/x"), "{said}");
}

// image-vault.test.ts:239
#[test]
fn a_base_256_numeric_a_global_pax_record_an_unknown_type_flag_and_a_foreign_magic_are_each_a_refusal() {
    let mut base256 = [0u8; 12];
    base256[0] = 0x80;
    base256[11] = 1;
    assert!(members_refusal(&archive(&[header(H { name: "root/.codex/auth.json", size_field: Some(base256), ..H::default() })])).contains("base-256"));
    assert!(members_refusal(&archive(&[header(H { name: "PaxHeaders/g", kind: Some("g"), size: Some(10), ..H::default() }), filled("10 x=y\n")]))
        .contains("type flag is g"));
    assert!(members_refusal(&archive(&[header(H { name: "root/.codex/auth.json", kind: Some("7"), ..H::default() })])).contains("type flag is 7"));
    assert!(members_refusal(&archive(&[header(H { name: "root/.codex/auth.json", magic: Some(b"wsp\0\0\0"), ..H::default() })]))
        .contains("neither a ustar nor a GNU header"));
}

// image-vault.test.ts:249
#[test]
fn a_pax_key_this_reading_does_not_read_and_a_pax_record_beside_a_long_name_for_one_member_are_refusals() {
    assert!(members_refusal(&archive(&[
        header(H { name: "x", kind: Some("x"), size: Some(30), ..H::default() }),
        filled("30 SCHILY.xattr.user.x=y\n"),
        header(H { name: "root/.codex/auth.json", ..H::default() }),
    ]))
    .contains("SCHILY.xattr.user.x"));
    let both = archive(&[
        header(H { name: "x", kind: Some("x"), size: Some(25), ..H::default() }),
        filled("25 path=etc/cron.d/x\n"),
        header(H { name: "@LongLink", kind: Some("L"), size: Some(5), ..H::default() }),
        filled("etc\0"),
        header(H { name: "root/.codex/auth.json", ..H::default() }),
    ]);
    assert!(members_refusal(&both).contains("both a pax record and a long name"));
}

// image-vault.test.ts:255
#[test]
fn two_gzip_members_concatenated_read_as_one_archive_and_bytes_behind_the_last_that_are_no_member_are_a_refusal() {
    let blocks: Vec<u8> = [header(H { name: "root/.codex/auth.json", ..H::default() }), header(H { name: "etc/cron.d/x", ..H::default() }), vec![0u8; BLOCK * 2]].concat();
    let split: Vec<u8> = [gzip(&blocks[..BLOCK]), gzip(&blocks[BLOCK..])].concat();
    assert_eq!(names_of(&vault_members(VaultRoad::Import, &split).unwrap()), strings(&["root/.codex/auth.json", "etc/cron.d/x"]));
    let trailing: Vec<u8> = [archive(&[header(H { name: "root/.codex/auth.json", ..H::default() })]), b"trailing".to_vec()].concat();
    let said = members_refusal(&trailing);
    assert!(said.contains("do not decompress"), "{said}");
    // node's gunzipSync: trailing garbage reads "incorrect header check", a cut stream "unexpected end of file"
    assert!(said.contains("(incorrect header check)"), "{said}");
}

// image-vault.test.ts:262
#[test]
fn a_member_whose_data_runs_past_the_end_of_the_archive_is_a_refusal_as_is_an_archive_that_never_ends() {
    assert!(members_refusal(&archive(&[header(H { name: "root/.codex/auth.json", size: Some(4096), ..H::default() })])).contains("runs past the end"));
    assert!(members_refusal(&gzip(&header(H { name: "root/.codex/auth.json", ..H::default() }))).contains("without the two zero blocks"));
}

// image-vault.test.ts:267
#[test]
fn the_rule_reads_a_folder_and_a_link_that_stay_inside_and_a_relative_link_that_climbs_out() {
    let held = held();
    assert_eq!(refused_member(&member("root/.config/gh/", MemberKind::Dir, None), &held), None);
    assert_eq!(refused_member(&member("./root/.config/gh/x", MemberKind::File, None), &held), None);
    assert_eq!(refused_member(&member("root/.config/gh/x", MemberKind::Symlink, Some("../gh/hosts.yml")), &held), None);
    let out = refused_member(&member("root/.config/gh/x", MemberKind::Symlink, Some("../../.ssh/id_ed25519")), &held).expect("a refusal");
    assert!(out.contains("points at"), "{out}");
}

// image-vault.test.ts:274
#[test]
fn a_hard_links_target_is_an_archive_name_rooted_where_tar_extracts_never_a_path_from_the_links_own_folder() {
    let held = held();
    let said = |m: VaultMember| refused_member(&m, &held).expect("a refusal");
    assert!(said(member("root/.config/gh/x", MemberKind::Hardlink, Some("etc/shadow"))).contains("points at etc/shadow"));
    assert!(said(member("root/.config/gh/x", MemberKind::Hardlink, Some("/etc/shadow"))).contains("points at /etc/shadow"));
    assert_eq!(refused_member(&member("root/.config/gh/x", MemberKind::Hardlink, Some("root/.config/gh/hosts.yml")), &held), None);
    assert_eq!(refused_member(&member("root/.config/gh/x", MemberKind::Symlink, Some("hosts.yml")), &held), None);
    let tar = archive(&[header(H { name: "root/.config/gh/x", kind: Some("1"), target: Some("etc/shadow"), ..H::default() })]);
    assert!(refusal(VaultRoad::Import, &tar, &strings(&["/root/.config/gh"])).contains("points at etc/shadow"));
}

// image-vault.test.ts:290
#[test]
fn a_builder_that_answers_a_member_outside_the_asked_paths_fails_the_seal_with_the_member_named() {
    let here = format!("{GUEST_HOME}/.codex/auth.json");
    let planted = tar_of(&[file("root/.codex/auth.json", 0o600, "{}"), file("etc/cron.d/x", 0o644, "x")]).unwrap();
    let (g, fetch) = guest(&[&here], GuestOpts { tar: Some(planted), ..GuestOpts::default() });
    let err = run(export_image_vault(&*g, std::slice::from_ref(&here), &opts_with(&fetch))).err().expect("a refusal");
    assert!(err.to_string().contains("etc/cron.d/x"), "{err}");
}

// image-vault.test.ts:296
#[test]
fn a_builder_that_answers_exactly_the_asked_paths_seals_with_the_list_it_was_asked_for() {
    let here = format!("{GUEST_HOME}/.codex/auth.json");
    let (g, fetch) = guest(&[&here], GuestOpts::default());
    let vault = run(export_image_vault(&*g, &[here.clone(), format!("{GUEST_HOME}/.aws")], &opts_with(&fetch))).unwrap();
    assert_eq!(vault.held, vec![here]);
}
