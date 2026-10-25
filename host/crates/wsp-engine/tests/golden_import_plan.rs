// SPDX-License-Identifier: AGPL-3.0-only
//! Port of the plan half of packages/engine/test/golden-import.test.ts: brewfileFor, toolInstallsFor, what a step
//! shows, the catalog rows, tap formulae on the release road, the node a recipe's own rows bring, and the version a
//! step asks for. Every body under test is `todo!()` until the units land.
//!
//! Not ported: golden-import.test.ts:1209 ("a catalog tool comes off through the same module"), which tests
//! toolUninstall; and the last assertion of :905, `toolUninstall(row(tools/brew/jq))`, for the same reason.
#![allow(clippy::too_many_lines)]

use std::cmp::Ordering;
use std::fs;
use std::path::Path;
use std::process::Command;

use wsp_catalog::catalog::{base_note, catalog_entry, install_line};
use wsp_catalog::linux_casks::KUBECTL;
use wsp_catalog::pending::{Platform, ToolPin};
use wsp_catalog::road_modules::{
    road_step, BREW, BREW_PREFIX, BREW_REAL, BREW_REPO, HOMEBREW_COMMIT, HOMEBREW_TAG, LINUXBREW_HOME, LINUXBREW_SHIM,
};
use wsp_catalog::roads::{node_install_script, InstallRoad, RoadName, GUEST_HOME, LOCAL_BIN, NODE_RELEASES};
use wsp_engine::golden_seam::{
    brew_housekeeping, brewfile_for, path_line, tool_installs_for_rows, BrewFormula, BrewTable, PinRead, RecipeEntry, SkippedItem,
    ToolInstall, ToolSource, ToolsPlan, CATALOG_PREFIX, TOOLS_PATH, UNMEASURED_ROAD,
};
use wsp_engine::provision::{presence_tests, Versions};
use wsp_wire::types::daemon_contract::{HOMEBREW_PREFIX, TOOL_PREFIX};
use wsp_wire::types::shell_quote::shell_quote;
use wsp_wire::views::golden_image::{CustomLit, RecipeCustomRow};
use wsp_words::project_path::probe_path;
use wsp_words::semver::compare_versions;

// --- the TS file's helpers ---------------------------------------------------------------------------------------

/// golden-import.test.ts:65 `row`: the id as label, no paths, 0 bytes, `bring`, ticked.
fn row(rung: &str, id: &str) -> RecipeEntry {
    RecipeEntry {
        rung: rung.to_string(),
        id: id.to_string(),
        label: id.to_string(),
        paths: vec![],
        bytes: 0,
        default: "bring".to_string(),
        bring: Some(true),
        ..RecipeEntry::default()
    }
}

fn tools(id: &str) -> RecipeEntry {
    row("tools", id)
}

/// The overrides the TS spreads into `row({...})`, one method each.
trait Over: Sized {
    fn linux(self, v: &str) -> Self;
    fn label(self, v: &str) -> Self;
    fn version(self, v: &str) -> Self;
    fn paths(self, v: &[&str]) -> Self;
    fn unticked(self) -> Self;
    fn pin(self, tag: &str, sha256: &str) -> Self;
}

impl Over for RecipeEntry {
    fn linux(mut self, v: &str) -> Self {
        self.linux = Some(v.to_string());
        self
    }
    fn label(mut self, v: &str) -> Self {
        self.label = v.to_string();
        self
    }
    fn version(mut self, v: &str) -> Self {
        self.version = Some(v.to_string());
        self
    }
    fn paths(mut self, v: &[&str]) -> Self {
        self.paths = v.iter().map(|s| s.to_string()).collect();
        self
    }
    fn unticked(mut self) -> Self {
        self.bring = Some(false);
        self
    }
    fn pin(mut self, tag: &str, sha256: &str) -> Self {
        self.pin = Some(ToolPin { tag: tag.to_string(), sha256: Some(sha256.to_string()), arch: None, latest: None });
        self
    }
}

/// golden-import.test.ts:1119 `catalog`: a tools row under the catalog prefix, labelled by the entry's name.
fn catalog(id: &str) -> RecipeEntry {
    let name = catalog_entry(id).map(|e| e.name().to_string()).unwrap_or_else(|| id.to_string());
    tools(&format!("{CATALOG_PREFIX}{id}")).label(&name).linux("yes")
}

/// `toolInstallsFor(entries)` with every default.
fn plan(entries: &[RecipeEntry]) -> ToolsPlan {
    tool_installs_for_rows(entries, &BrewTable::new(), &[], TOOLS_PATH, None).unwrap()
}

fn plan_table(entries: &[RecipeEntry], table: &BrewTable) -> ToolsPlan {
    tool_installs_for_rows(entries, table, &[], TOOLS_PATH, None).unwrap()
}

/// golden-import.test.ts:75 `baseRows`: the row's id and name, and the note built for the computer read.
fn base_rows(plan: &ToolsPlan, platform: Platform) -> Vec<(String, String, String)> {
    plan.base.iter().map(|b| (b.id.clone(), b.name.clone(), base_note(b.entry, b.version.as_deref(), platform))).collect()
}

fn base3(rows: &[(&str, &str, &str)]) -> Vec<(String, String, String)> {
    rows.iter().map(|(a, b, c)| (a.to_string(), b.to_string(), c.to_string())).collect()
}

/// golden-import.test.ts:78 `homebrewStep`.
fn homebrew_step() -> String {
    plan(&[tools("tools/brew/gh").linux("yes")]).installs.into_iter().find(|i| i.id == "tools/homebrew").unwrap().cmd
}

/// `installs.map(i => [i.id, i.manager, i.after])`.
fn ima(installs: &[ToolInstall]) -> Vec<(&str, &str, Option<&str>)> {
    installs.iter().map(|i| (i.id.as_str(), i.manager.as_str(), i.after.as_deref())).collect()
}

/// `installs.map(i => [i.id, i.after])`.
fn ia(installs: &[ToolInstall]) -> Vec<(&str, Option<&str>)> {
    installs.iter().map(|i| (i.id.as_str(), i.after.as_deref())).collect()
}

fn ids(installs: &[ToolInstall]) -> Vec<&str> {
    installs.iter().map(|i| i.id.as_str()).collect()
}

fn get<'a>(t: &'a ToolsPlan, id: &str) -> &'a ToolInstall {
    t.installs.iter().find(|i| i.id == id).unwrap_or_else(|| panic!("no step {id}"))
}

fn cmd_of<'a>(t: &'a ToolsPlan, id: &str) -> &'a str {
    &get(t, id).cmd
}

fn skip(id: &str, note: &str) -> SkippedItem {
    SkippedItem { id: id.to_string(), note: note.to_string() }
}

fn formula(name: &str, full: &str, version: Option<&str>, source: Option<(&str, &str)>) -> BrewFormula {
    BrewFormula {
        name: name.to_string(),
        full_name: full.to_string(),
        deps: vec![],
        macos_only: false,
        version: version.map(str::to_string),
        source: source.map(|(repo, tag)| ToolSource { repo: repo.to_string(), tag: tag.to_string() }),
        ..BrewFormula::default()
    }
}

fn table(rows: Vec<BrewFormula>) -> BrewTable {
    rows.into_iter().map(|f| (f.full_name.clone(), f)).collect()
}

/// The regex `^?<parts joined by [\s\S]*>$?`, fixed strings in order.
fn re(s: &str, from_start: bool, parts: &[&str], to_end: bool) -> bool {
    let mut at = 0;
    for (n, part) in parts.iter().enumerate() {
        let last = n + 1 == parts.len();
        if last && to_end {
            return s.ends_with(part) && s.len() - part.len() >= at && (n > 0 || !from_start || s.starts_with(part));
        }
        match s[at..].find(part) {
            Some(i) if n > 0 || !from_start || i == 0 => at += i + part.len(),
            _ => return false,
        }
    }
    true
}

/// `/^export PATH=.*PNPM_HOME=/`.
fn on_tools_path(cmd: &str) -> bool {
    cmd.starts_with("export PATH=") && cmd.lines().next().is_some_and(|first| first["export PATH=".len()..].contains("PNPM_HOME="))
}

/// `/\|\s*(ba)?sh\b/`.
fn pipes_into_a_shell(cmd: &str) -> bool {
    let b = cmd.as_bytes();
    let word = |c: u8| c.is_ascii_alphanumeric() || c == b'_';
    cmd.match_indices('|').any(|(i, _)| {
        let mut j = i + 1;
        while j < b.len() && (b[j] as char).is_whitespace() {
            j += 1;
        }
        [&b"bash"[..], &b"sh"[..]].iter().any(|w| b[j..].starts_with(w) && !b.get(j + w.len()).is_some_and(|c| word(*c)))
    })
}

/// `/\n<line>$/m`: a line after the first.
fn has_later_line(cmd: &str, line: &str) -> bool {
    cmd.split('\n').skip(1).any(|l| l == line)
}

/// `/^<line>$/m`.
fn has_line(cmd: &str, line: &str) -> bool {
    cmd.split('\n').any(|l| l == line)
}

/// A version the road's own `version` field holds, as `installRoad as { version: string }`.
fn road_version(id: &str) -> String {
    let entry = catalog_entry(id).unwrap_or_else(|| panic!("no catalog entry {id}"));
    match entry.install_road() {
        InstallRoad::Npm { package, .. }
        | InstallRoad::Pnpm(package)
        | InstallRoad::Bun(package)
        | InstallRoad::Uv(package)
        | InstallRoad::Pipx(package)
        | InstallRoad::Cargo(package) => package.version.clone().unwrap_or_else(|| panic!("{id}: no version")),
        other => panic!("{id}: no package road: {other:?}"),
    }
}

fn node_release_22() -> wsp_catalog::roads::NodeRelease {
    NODE_RELEASES.iter().find(|(major, _)| *major == 22).unwrap().1
}

fn write_stub(path: &Path, script: &str) {
    use std::os::unix::fs::PermissionsExt;
    fs::write(path, script).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

fn temp(prefix: &str) -> tempfile::TempDir {
    tempfile::Builder::new().prefix(prefix).tempdir().unwrap()
}

/// `spawnSync("bash", ["-c", script])` on this process's environment.
fn bash_status(script: &str) -> Option<i32> {
    Command::new("/bin/bash").arg("-c").arg(script).output().unwrap().status.code()
}

/// `spawnSync("bash", ["-c", script], { env })`: the environment is exactly `env`.
fn bash_env(script: &str, env: &[(&str, &str)]) -> std::process::Output {
    Command::new("/bin/bash").arg("-c").arg(script).env_clear().envs(env.iter().copied()).output().unwrap()
}

/// `script.replace(/^export PATH=/, `export PATH=${stubs}:`)`.
fn stubbed(script: &str, stubs: &Path) -> String {
    match script.strip_prefix("export PATH=") {
        Some(rest) => format!("export PATH={}:{rest}", stubs.display()),
        None => script.to_string(),
    }
}

// --- brewfileFor -------------------------------------------------------------------------------------------------

// TS: packages/engine/test/golden-import.test.ts:766
// "lists ticked taps and formulae with a Linux bottle; unknown and macOS-only ones are noted"
#[test]
fn brewfile_for_lists_ticked_taps_and_formulae_with_a_linux_bottle_unknown_ones_noted() {
    let b = brewfile_for(
        &[
            tools("tools/brew-tap/zingzy/tap").linux("yes"),
            tools("tools/brew/gh").linux("yes"),
            tools("tools/brew/jq").linux("yes").unticked(),
            tools("tools/brew/zingzy/tap/diskbloom").linux("unknown"),
            tools("tools/brew/mas").linux("no"),
            tools("tools/brew/bat"),
        ],
        &BrewTable::new(),
    );
    assert_eq!(b.text, ["tap \"zingzy/tap\"", "brew \"gh\"", "brew \"bat\"", ""].join("\n"));
    assert_eq!(b.taps, vec!["zingzy/tap".to_string()]);
    assert_eq!(b.formulae, vec!["gh".to_string(), "bat".to_string()]);
    assert_eq!(
        b.skipped,
        vec![skip("tools/brew/zingzy/tap/diskbloom", "no Linux bottle known"), skip("tools/brew/mas", "no Linux bottle")]
    );
}

// TS: packages/engine/test/golden-import.test.ts:784
// "is empty when nothing Homebrew is ticked"
#[test]
fn brewfile_for_is_empty_when_nothing_homebrew_is_ticked() {
    assert_eq!(brewfile_for(&[tools("tools/npm/bun").label("bun@1.4.0").version("1.4.0")], &BrewTable::new()).text, "");
}

// --- toolInstallsFor ---------------------------------------------------------------------------------------------

// TS: packages/engine/test/golden-import.test.ts:790
// "Homebrew comes first (pinned clone, as its own user), then taps, then formulae, then each manager after its own install, with the row's pinned version"
#[test]
fn tool_installs_for_homebrew_first_then_taps_formulae_then_each_manager_after_its_own_install() {
    let t = plan(&[
        tools("tools/go/sqlc").label("sqlc").paths(&["github.com/sqlc-dev/sqlc/cmd/sqlc@v1.31.1"]).version("v1.31.1"),
        tools("tools/brew/gh").linux("yes"),
        tools("tools/brew-tap/zingzy/tap").linux("yes"),
        tools("tools/npm/bun").label("bun@1.4.0").version("1.4.0"),
        tools("tools/npm/@monid-ai/cli").label("@monid-ai/cli@0.3.1").version("0.3.1"),
        tools("tools/npm/pnpm").label("pnpm"),
        tools("tools/pnpm/turbo").label("turbo@2.5.0").version("2.5.0"),
        tools("tools/bun/eslint").label("eslint@9.0.0").version("9.0.0"),
        tools("tools/uv/ty").label("ty v0.0.56").version("0.0.56"),
        tools("tools/pipx/black").label("black 24.1.0").version("24.1.0"),
        tools("tools/cargo/bat").label("bat 0.24.0").version("0.24.0"),
        tools("tools/brew/mas").linux("no"),
        tools("tools/go/junk").label("junk (no module info)"),
    ]);
    assert_eq!(
        ima(&t.installs),
        vec![
            ("tools/homebrew", "brew", None),
            // Homebrew's own glibc then gcc, each its own brew process, before any formula (the 6.0.21 lock race).
            ("tools/brew-toolchain/glibc", "brew", Some("tools/homebrew")),
            ("tools/brew-toolchain/gcc", "brew", Some("tools/brew-toolchain/glibc")),
            ("tools/brew-tap/zingzy/tap", "brew", Some("tools/brew-toolchain/gcc")),
            // What two or more of the formulae (gh and the managers' pipx and go) share installs once, after the taps it may need.
            ("tools/brew-shared", "brew", Some("tools/brew-toolchain/gcc")),
            ("tools/brew/gh", "brew", Some("tools/brew-toolchain/gcc")),
            // Node is not on the floor, so the first row on the npm road brings it, once, ahead of them all.
            ("tools/manager/npm", "script", None),
            ("tools/npm/bun", "npm", Some("tools/manager/npm")),
            ("tools/npm/@monid-ai/cli", "npm", Some("tools/manager/npm")),
            ("tools/npm/pnpm", "npm", Some("tools/manager/npm")),
            // bun and pnpm came as npm globals, so their rows wait on those lines rather than on a formula; uv is the base's, so its rows wait on nothing.
            ("tools/pnpm/turbo", "pnpm", Some("tools/npm/pnpm")),
            ("tools/bun/eslint", "bun", Some("tools/npm/bun")),
            ("tools/uv/ty", "uv", None),
            ("tools/manager/pipx", "brew", Some("tools/brew-toolchain/gcc")),
            ("tools/pipx/black", "pipx", Some("tools/manager/pipx")),
            // cargo comes by rustup, which needs no Homebrew, so its step waits on nothing.
            ("tools/manager/cargo", "script", None),
            ("tools/cargo/bat", "cargo", Some("tools/manager/cargo")),
            ("tools/manager/go", "brew", Some("tools/brew-toolchain/gcc")),
            ("tools/go/sqlc", "go", Some("tools/manager/go")),
        ]
    );
    let cmd = |id: &str| cmd_of(&t, id).to_string();
    assert!(cmd("tools/homebrew")
        .contains(&format!("--branch {HOMEBREW_TAG} https://github.com/Homebrew/brew /home/linuxbrew/.linuxbrew/Homebrew")));
    assert!(cmd("tools/homebrew").contains(&format!("rev-parse HEAD)\" = \"{HOMEBREW_COMMIT}\"")));
    assert!(cmd("tools/homebrew").contains("useradd"));
    // The login-shell PATH file is written by the base stage on every golden, so the bootstrap no longer writes it.
    assert!(!cmd("tools/homebrew").contains("/etc/profile.d/wsp-golden.sh"));
    assert!(!cmd("tools/homebrew").contains("brew bundle"));
    assert!(cmd("tools/brew-toolchain/glibc").ends_with("brew install glibc'"));
    assert!(cmd("tools/brew-toolchain/gcc").ends_with("brew install gcc'"));
    assert!(re(&cmd("tools/brew-tap/zingzy/tap"), false, &["su -s /bin/bash linuxbrew -c '[ -d \"$PWD\" ]", "brew tap zingzy/tap'"], true));
    assert!(re(
        &cmd("tools/brew/gh"),
        false,
        &["su -s /bin/bash linuxbrew -c '[ -d \"$PWD\" ]", "HOMEBREW_NO_AUTO_UPDATE=1", "brew install gh'"],
        true
    ));
    let shared = cmd("tools/brew-shared");
    let head = "su -s /bin/bash linuxbrew -c '[ -d \"$PWD\" ] && [ -r \"$PWD\" ] || cd /home/linuxbrew\nexport HOMEBREW_NO_AUTO_UPDATE=1 ";
    let after_head = &shared[shared.find(head).expect("the su line with the readable-directory guard") + head.len()..];
    assert!(after_head.split('\n').next().unwrap().ends_with("NONINTERACTIVE=1 HOMEBREW_CURL_RETRIES=1") && after_head.contains('\n'));
    assert!(shared.contains(r#"brew deps --for-each '\''gh'\'' '\''pipx'\'' '\''go'\'')"#));
    // Homebrew's own toolchain is never in the shared set: it installed before, on request, and stays that way.
    assert!(shared.contains(r#"grep -vx -e '\'''\'' -e glibc -e gcc | sort | uniq -d"#));
    assert!(shared.contains("brew install $shared 2>\"$err\"; rc=$?"));
    // Installed as dependencies, so autoremove takes them with the formula that fails or leaves.
    assert!(shared.contains("brew tab --no-installed-on-request $shared || true\n"));
    assert!(shared.ends_with("exit $rc'"));
    // Homebrew cleans after each install; one recipe with it off left 2.6 GB of bottles on a 20 GB disk.
    for i in &t.installs {
        assert!(!i.cmd.contains("HOMEBREW_NO_INSTALL_CLEANUP"), "{}", i.id);
    }
    assert!(cmd("tools/npm/bun").ends_with("npm install -g bun@1.4.0"));
    assert!(cmd("tools/npm/@monid-ai/cli").ends_with("npm install -g @monid-ai/cli@0.3.1"));
    assert!(cmd("tools/pnpm/turbo").ends_with("pnpm add -g turbo@2.5.0"));
    assert!(cmd("tools/bun/eslint").ends_with("bun add -g eslint@9.0.0"));
    assert!(cmd("tools/uv/ty").ends_with("uv tool install ty==0.0.56"));
    assert!(cmd("tools/manager/pipx").ends_with("brew install pipx'"));
    assert!(cmd("tools/pipx/black").ends_with("pipx install black==24.1.0"));
    assert!(cmd("tools/manager/cargo")
        .contains("curl -o /tmp/rustup-init \"https://static.rust-lang.org/rustup/archive/1.29.1/$arch-unknown-linux-gnu/rustup-init\""));
    assert!(cmd("tools/manager/cargo")
        .ends_with("\n/tmp/rustup-init -y --no-modify-path --profile default --default-toolchain stable\nrm -f /tmp/rustup-init"));
    assert!(cmd("tools/cargo/bat").ends_with("cargo install bat --version 0.24.0 --locked"));
    assert!(cmd("tools/manager/go").ends_with("brew install go'"));
    assert!(cmd("tools/go/sqlc").ends_with("go install github.com/sqlc-dev/sqlc/cmd/sqlc@v1.31.1"));
    for i in &t.installs {
        assert!(on_tools_path(&i.cmd), "{}", i.id);
    }
    assert_eq!(t.skipped, vec![skip("tools/brew/mas", "no Linux bottle"), skip("tools/go/junk", "no module to install from")]);
    assert_eq!(base_rows(&t, Platform::Darwin), base3(&[]));
    assert_eq!(t.brewfile, ["tap \"zingzy/tap\"", "brew \"gh\"", ""].join("\n"));
}

// TS: packages/engine/test/golden-import.test.ts:875
// "a Go row's module rides in its first path (the collector's shape) and the version field pins it; an older recipe's label shape still works"
#[test]
fn tool_installs_for_a_go_rows_module_rides_in_its_first_path_and_the_version_field_pins_it() {
    let fresh = plan(&[tools("tools/go/gopls").label("gopls").paths(&["golang.org/x/tools/gopls@v0.16.2"]).version("v0.16.2")]);
    assert!(fresh.installs.last().unwrap().cmd.ends_with("go install golang.org/x/tools/gopls@v0.16.2"));
    let pinned = plan(&[tools("tools/go/gopls").label("gopls").paths(&["golang.org/x/tools/gopls@v0.16.2"]).version("v0.17.0")]);
    assert!(pinned.installs.last().unwrap().cmd.ends_with("gopls@v0.17.0"));
    let old = plan(&[tools("tools/go/gopls").label("gopls (golang.org/x/tools/gopls@v0.16.2)")]);
    assert!(old.installs.last().unwrap().cmd.ends_with("go install golang.org/x/tools/gopls@v0.16.2"));
    assert_eq!(
        plan(&[tools("tools/go/mystery").label("mystery (no module info)")]).skipped,
        vec![skip("tools/go/mystery", "no module to install from")]
    );
}

// TS: packages/engine/test/golden-import.test.ts:885
// "a row with no version installs the manager's latest; a manager already ticked as a formula is not installed twice, and takes the catalog's road for it"
#[test]
fn tool_installs_for_a_row_with_no_version_installs_the_managers_latest_no_formula_twice() {
    let t = plan(&[tools("tools/brew/bun").linux("yes"), tools("tools/bun/elysia").label("elysia"), tools("tools/uv/ty").label("ty")]);
    // The Mac's bun formula is the catalog's bun row, so it comes by the catalog's npm install and Homebrew stays off
    // the machine; that install runs on node, which the recipe ticked no row for, so the node step brings it.
    assert_eq!(
        ima(&t.installs),
        vec![
            ("tools/manager/npm", "script", None),
            ("tools/brew/bun", "npm", Some("tools/manager/npm")),
            ("tools/bun/elysia", "bun", Some("tools/brew/bun")),
            ("tools/uv/ty", "uv", None),
        ]
    );
    assert!(t.installs[1].cmd.ends_with("\nnpm install -g bun"));
    assert_eq!(t.brewfile, "");
    assert!(cmd_of(&t, "tools/bun/elysia").ends_with("bun add -g elysia"));
    assert!(cmd_of(&t, "tools/uv/ty").ends_with("uv tool install ty"));
}

// TS: packages/engine/test/golden-import.test.ts:905
// "rows the base floor covers install nothing and are listed as the base's, whatever road the Mac had them by"
#[test]
fn tool_installs_for_rows_the_base_floor_covers_install_nothing_and_are_listed_as_the_bases() {
    // Homebrew's node is the current major and its python the current 3.x: the Mac's versions come from the brew table.
    let brew = table(vec![
        formula("node", "node", Some("24.1.0"), None),
        formula("python", "python", Some("3.14.0"), None),
        formula("python@3.12", "python@3.12", Some("3.12.7"), None),
    ]);
    let t = plan_table(
        &[
            tools("tools/brew/jq").linux("yes"),
            tools("tools/npm/pnpm").label("pnpm").version("10.0.0"),
            tools("tools/brew/python@3.12").linux("yes"),
            tools("tools/brew/node").linux("yes"),
            tools("tools/brew/python").linux("yes"),
            tools("tools/brew/uv").linux("yes"),
            tools("tools/cargo/ripgrep").label("ripgrep").version("14.1.0"),
            tools("tools/brew/python@3.14").linux("yes"),
        ],
        &brew,
    );
    assert_eq!(
        base_rows(&t, Platform::Darwin),
        base3(&[
            ("tools/brew/jq", "jq", "jq is part of the base"),
            ("tools/brew/python@3.12", "Python 3.12", "Python 3.12 is part of the base"),
            ("tools/brew/python", "Python 3.12", "Python 3.12 is part of the base; this Mac runs Python 3.14"),
            ("tools/brew/uv", "uv", "uv is part of the base"),
            ("tools/cargo/ripgrep", "ripgrep", "ripgrep is part of the base"),
        ])
    );
    // The plan names no computer: it carries the floor row and the version read here, so the same rows read as a
    // Linux computer's when that is what was read.
    assert!(base_rows(&t, Platform::Linux)
        .iter()
        .map(|r| r.2.as_str())
        .any(|n| n == "Python 3.12 is part of the base; this computer runs Python 3.14"));
    // Node and pnpm are off the floor: the Mac's node formula is the node the npm road runs on, by the catalog's
    // own script, and pnpm waits on it.
    assert_eq!(
        ima(&t.installs),
        vec![
            ("tools/homebrew", "brew", None),
            ("tools/brew-toolchain/glibc", "brew", Some("tools/homebrew")),
            ("tools/brew-toolchain/gcc", "brew", Some("tools/brew-toolchain/glibc")),
            ("tools/brew/python@3.14", "brew", Some("tools/brew-toolchain/gcc")),
            ("tools/brew/node", "script", None),
            ("tools/npm/pnpm", "npm", Some("tools/brew/node")),
        ]
    );
    assert_eq!(t.skipped, Vec::<SkippedItem>::new());
    assert_eq!(t.brewfile, "brew \"python@3.14\"\n");
    // Adapted: the TS ends with `toolUninstall(row(tools/brew/jq), new Map())` equal to
    // `{ note: "jq is part of the base and stays" }`; toolUninstall is not in this port.
}

// TS: packages/engine/test/golden-import.test.ts:947
// "one formula has nothing to share, so no shared step; two get one between the taps and the first formula"
#[test]
fn tool_installs_for_one_formula_has_nothing_to_share_two_get_one_between_the_taps_and_the_first() {
    let one = plan(&[tools("tools/brew/gh").linux("yes")]);
    assert_eq!(ids(&one.installs), vec!["tools/homebrew", "tools/brew-toolchain/glibc", "tools/brew-toolchain/gcc", "tools/brew/gh"]);
    let two =
        plan(&[tools("tools/brew/gh").linux("yes"), tools("tools/brew/yq").linux("yes"), tools("tools/brew-tap/zingzy/tap").linux("yes")]);
    assert_eq!(
        ids(&two.installs),
        vec![
            "tools/homebrew",
            "tools/brew-toolchain/glibc",
            "tools/brew-toolchain/gcc",
            "tools/brew-tap/zingzy/tap",
            "tools/brew-shared",
            "tools/brew/gh",
            "tools/brew/yq"
        ]
    );
    let shared = get(&two, "tools/brew-shared");
    assert_eq!(shared.label, "shared Homebrew dependencies");
    assert_eq!(shared.after.as_deref(), Some("tools/brew-toolchain/gcc"));
    assert!(shared.cmd.contains(r#"brew deps --for-each '\''gh'\'' '\''yq'\'')"#));
}

// TS: packages/engine/test/golden-import.test.ts:956
// "the housekeeping after the loop is autoremove then a full cleanup, each as linuxbrew with the tools PATH"
#[test]
fn the_housekeeping_after_the_loop_is_autoremove_then_a_full_cleanup_each_as_linuxbrew() {
    let housekeeping = brew_housekeeping(TOOLS_PATH);
    assert_eq!(housekeeping.len(), 2);
    // /^export PATH=\/root\/\.local\/bin:.*\nsu -s \/bin\/bash linuxbrew -c '\[ -d "\$PWD" \][\s\S]*brew autoremove'$/
    let (first_line, rest) = housekeeping[0].split_once('\n').expect("two lines or more");
    assert!(first_line.starts_with("export PATH=/root/.local/bin:"));
    assert!(re(rest, true, &["su -s /bin/bash linuxbrew -c '[ -d \"$PWD\" ]", "brew autoremove'"], true));
    assert!(re(&housekeeping[1], false, &["\nsu -s /bin/bash linuxbrew -c '[ -d \"$PWD\" ]", "brew cleanup -s --prune=all'"], true));
}

// TS: packages/engine/test/golden-import.test.ts:963
// "puts the one brew a PATH reaches in the directory Homebrew's own line prepends, and Homebrew's brew where no PATH goes"
#[test]
fn puts_the_one_brew_a_path_reaches_in_the_directory_homebrews_own_line_prepends() {
    // A Mac's `eval "$(brew shellenv)"` arrives rewritten to this prefix and prepends its bin directory, so a shim
    // anywhere else ends up behind it; the only brew on any directory of the tools PATH is the shim.
    let reached: Vec<&str> =
        TOOLS_PATH.split(':').filter(|d| BREW.starts_with(&format!("{d}/")) || BREW_REAL.starts_with(&format!("{d}/"))).collect();
    assert_eq!(reached, vec![format!("{BREW_PREFIX}/bin")]);
}

// TS: packages/engine/test/golden-import.test.ts:969
// "installs the shim as the brew on PATH, root's own file, and links Homebrew's brew for it to run"
#[test]
fn installs_the_shim_as_the_brew_on_path_roots_own_file_and_links_homebrews_brew() {
    let cmd = homebrew_step();
    let brew: &str = &BREW;
    let brew_real: &str = &BREW_REAL;
    let shim: &str = &LINUXBREW_SHIM;
    assert!(cmd.contains(&format!("ln -sfn ../Homebrew/bin/brew {brew_real}")));
    assert!(!cmd.contains(&format!("ln -sfn ../Homebrew/bin/brew {brew}")));
    assert!(cmd.contains(&format!("rm -f {brew}")));
    assert!(cmd.contains(&format!("printf '%s\\n' {} > {brew}", shell_quote(shim))));
    assert!(cmd.contains(&format!("chmod 0755 {brew}")));
    // Written after the chown, so the file root runs with root's own rights is not the linuxbrew user's to rewrite.
    assert!(cmd.find(&format!("> {brew}")).unwrap() > cmd.find("chown -R linuxbrew:linuxbrew").unwrap());
    // Outside the first install, so a computer set up behind an older shim gets this one, and the step reads present
    // only where the file is this one.
    assert!(cmd.find(&format!("> {brew}")).unwrap() > cmd.find("\nfi\n").unwrap());
    let t = plan(&[tools("tools/brew/gh").linux("yes")]);
    assert_eq!(get(&t, "tools/homebrew").present.as_deref(), Some(format!("[ \"$(cat {brew})\" = {} ]", shell_quote(shim)).as_str()));
    // A person's own brew reads the folder they are in; one linuxbrew cannot reach (root's home on a box) would stop
    // Homebrew before it starts, so the shim moves off it and only off it, taking the Brewfile along on fd 9.
    assert!(shim.contains(&format!("su -s /bin/bash linuxbrew -c '[ -d \"$PWD\" ] && [ -r \"$PWD\" ] || {{ cd {LINUXBREW_HOME}; ")));
}

// TS: packages/engine/test/golden-import.test.ts:987
// "costs a line and not the step when the upstream branch will not fetch: the shim still lands"
#[test]
fn costs_a_line_and_not_the_step_when_the_upstream_branch_will_not_fetch_the_shim_still_lands() {
    // The generated step run for real, at a prefix of its own, with every command that would touch this machine
    // stubbed and the one fetch failing the way a dead network fails it. What it pins is that the rows waiting on
    // this step still get their Homebrew: the shim is written, the link is made, and the smoke at the end passes.
    let tmp = temp("wsp-brew-");
    let dir = tmp.path().to_str().unwrap().to_string();
    let stub = tmp.path().join("stub");
    fs::create_dir(&stub).unwrap();
    write_stub(
        &stub.join("git"),
        &format!(
            "#!/bin/sh\ncase \" $* \" in\n  *\" clone \"*) mkdir -p {bin} && printf '%s\\n' '#!/bin/sh' 'echo Homebrew' > {brew} && chmod 0755 {brew} ;;\n  *\"rev-parse HEAD\"*) echo {HOMEBREW_COMMIT} ;;\n  *\" fetch \"*) echo \"fatal: unable to access github.com\" >&2; exit 128 ;;\nesac\nexit 0\n",
            bin = shell_quote(&format!("{dir}/Homebrew/bin")),
            brew = shell_quote(&format!("{dir}/Homebrew/bin/brew")),
        ),
    );
    for name in ["apt-get", "useradd", "chown"] {
        write_stub(&stub.join(name), "#!/bin/sh\nexit 0\n");
    }
    write_stub(
        &stub.join("su"),
        "#!/bin/sh\nshell=/bin/sh\nwhile [ $# -gt 0 ]; do case \"$1\" in -s) shell=\"$2\"; shift 2 ;; -c) script=\"$2\"; shift 2 ;; --) shift; break ;; *) shift ;; esac; done\nexec \"$shell\" -c \"$script\" \"$@\"\n",
    );
    let script = homebrew_step().replace(BREW_PREFIX, &dir).replacen("export PATH=", &format!("export PATH={}:", stub.display()), 1);
    // The chown line names /home/linuxbrew whatever the prefix is, so the stub PATH is what keeps this run off it.
    assert!(script.contains(&format!("export PATH={}:", stub.display())));
    let run = Command::new("bash").arg("-c").arg(&script).output().unwrap();
    assert_eq!(
        (run.status.code(), String::from_utf8_lossy(&run.stdout).trim().to_string()),
        (Some(0), "origin/main did not fetch: brew update reads the branch in full".to_string())
    );
    assert_eq!(fs::read_to_string(tmp.path().join("bin/brew")).unwrap(), format!("{}\n", LINUXBREW_SHIM.replace(BREW_PREFIX, &dir)));
    assert!(fs::read_to_string(tmp.path().join("libexec/brew")).unwrap().contains("echo Homebrew"));
}

// TS: packages/engine/test/golden-import.test.ts:1010
// "leaves the checkout the origin remote a plain clone has, so brew update finds a branch to read"
#[test]
fn leaves_the_checkout_the_origin_remote_a_plain_clone_has_so_brew_update_finds_a_branch() {
    let repo: &str = &BREW_REPO;
    assert!(homebrew_step().contains(&format!("git -C {repo} remote set-branches origin '*'")));
    assert!(homebrew_step().contains(&format!("git -C {repo} fetch -q --depth 1 origin main")));
}

// TS: packages/engine/test/golden-import.test.ts:1015
// "a manager needed only for its rows brings Homebrew along when a formula is the only road to it"
#[test]
fn a_manager_needed_only_for_its_rows_brings_homebrew_along_when_a_formula_is_the_only_road() {
    let t = plan(&[tools("tools/pipx/black").label("black 24.1.0").version("24.1.0")]);
    assert_eq!(
        ids(&t.installs),
        vec!["tools/homebrew", "tools/brew-toolchain/glibc", "tools/brew-toolchain/gcc", "tools/manager/pipx", "tools/pipx/black"]
    );
}

// TS: packages/engine/test/golden-import.test.ts:1020
// "cargo's rows bring the catalog's rustup instead of Homebrew, and wait on it"
#[test]
fn cargos_rows_bring_the_catalogs_rustup_instead_of_homebrew_and_wait_on_it() {
    let t = plan(&[tools("tools/cargo/bat").label("bat 0.24.0").version("0.24.0")]);
    assert_eq!(ima(&t.installs), vec![("tools/manager/cargo", "script", None), ("tools/cargo/bat", "cargo", Some("tools/manager/cargo"))]);
    assert_eq!(t.installs[0].label, "Rust with cargo");
    assert_eq!(t.installs[0].bin.as_deref(), Some("cargo"));
    assert!(t.installs[0].cmd.contains("static.rust-lang.org/rustup/archive/1.29.1/"));
    assert_eq!(t.brewfile, "");
}

/// The body of the TS loop over `["rust", "rustup"]`, the second the name Homebrew recommends: either one is the
/// catalog's row.
fn macs_rust_formula(name: &str) {
    let t = plan(&[tools(&format!("tools/brew/{name}")).linux("yes"), tools("tools/cargo/bat").label("bat 0.24.0").version("0.24.0")]);
    let formula_id = format!("tools/brew/{name}");
    assert_eq!(ima(&t.installs), vec![(formula_id.as_str(), "script", None), ("tools/cargo/bat", "cargo", Some(formula_id.as_str()))]);
    assert!(t.installs[0].cmd.contains("static.rust-lang.org/rustup/archive/1.29.1/"));
    assert!(!t.installs[0].cmd.contains("brew install"));
    assert_eq!(t.brewfile, "");
    // With no cargo row behind it the formula still comes by the catalog's road: one rust on the machine, never the bottle.
    let alone = plan(&[tools(&format!("tools/brew/{name}")).linux("yes")]);
    let pairs: Vec<(&str, &str)> = alone.installs.iter().map(|i| (i.id.as_str(), i.manager.as_str())).collect();
    assert_eq!(pairs, vec![(formula_id.as_str(), "script")]);
}

// TS: packages/engine/test/golden-import.test.ts:1033
// "a Mac's rust formula is the catalog's rust row, and Homebrew's rust never installs beside it"
#[test]
fn a_macs_rust_formula_is_the_catalogs_rust_row_and_homebrews_rust_never_installs_beside_it() {
    macs_rust_formula("rust");
}

// TS: packages/engine/test/golden-import.test.ts:1033
// "a Mac's rustup formula is the catalog's rust row, and Homebrew's rustup never installs beside it"
#[test]
fn a_macs_rustup_formula_is_the_catalogs_rust_row_and_homebrews_rustup_never_installs_beside_it() {
    macs_rust_formula("rustup");
}

// TS: packages/engine/test/golden-import.test.ts:1050
// "a manager the catalog carries installs by the catalog's road: bun's rows wait on an npm install of bun, and no Homebrew comes along"
#[test]
fn a_manager_the_catalog_carries_installs_by_the_catalogs_road_bun_rows_wait_on_an_npm_install() {
    let t = plan(&[tools("tools/bun/eslint").label("eslint 9.0.0").version("9.0.0")]);
    // That install is itself an npm global, so the step that brings bun waits on the node step, which nothing else asked for.
    assert_eq!(
        ima(&t.installs),
        vec![
            ("tools/manager/npm", "script", None),
            ("tools/manager/bun", "npm", Some("tools/manager/npm")),
            ("tools/bun/eslint", "bun", Some("tools/manager/bun")),
        ]
    );
    assert!(t.installs[1].cmd.ends_with("\nnpm install -g bun"));
    assert_eq!(t.brewfile, "");
}

// TS: packages/engine/test/golden-import.test.ts:1062
// "no Homebrew step when no formula, tap or manager needs it; unticked rows install nothing"
#[test]
fn no_homebrew_step_when_no_formula_tap_or_manager_needs_it_unticked_rows_install_nothing() {
    let t = plan(&[tools("tools/npm/bun").label("bun@1.4.0").version("1.4.0"), tools("tools/brew/gh").linux("yes").unticked()]);
    assert_eq!(ids(&t.installs), vec!["tools/manager/npm", "tools/npm/bun"]);
    assert_eq!(t.brewfile, "");
}

// TS: packages/engine/test/golden-import.test.ts:1068
// "never pipes a download into a shell"
#[test]
fn never_pipes_a_download_into_a_shell() {
    let t = plan(&[
        tools("tools/brew/gh").linux("yes"),
        tools("tools/uv/ty").label("ty 0.0.56").version("0.0.56"),
        tools("tools/cargo/rg").label("rg"),
        tools("tools/pnpm/x").label("x"),
    ]);
    assert!(!t.installs.is_empty());
    for i in &t.installs {
        assert!(!pipes_into_a_shell(&i.cmd), "{}", i.id);
    }
}

// --- what a step shows while it runs -----------------------------------------------------------------------------

// TS: packages/engine/test/golden-import.test.ts:1080
// "is the manager's own line for a package, the brew line without its su, where a release comes from, and a custom row's lines as typed"
#[test]
fn what_a_step_shows_is_the_managers_own_line_the_brew_line_without_su_where_a_release_comes_from() {
    let custom = [RecipeCustomRow {
        kind: CustomLit,
        id: "just".to_string(),
        name: "just".to_string(),
        install: vec!["brew install just".to_string(), "just --version".to_string()],
        check: "command -v just".to_string(),
        manager: None,
        size: None,
        why: "added by hand".to_string(),
    }];
    let t = tool_installs_for_rows(
        &[
            tools("tools/brew/gh").linux("yes"),
            tools("tools/brew/yq").linux("yes"),
            tools("tools/brew-tap/zingzy/tap").linux("yes"),
            tools("tools/npm/bun").label("bun@1.4.0").version("1.4.0"),
            tools("tools/pipx/black").label("black 24.1.0").version("24.1.0"),
            tools(&format!("{CATALOG_PREFIX}tmux")).label("tmux").linux("yes"),
            tools(&format!("{CATALOG_PREFIX}gh")).label("GitHub CLI").linux("yes"),
        ],
        &BrewTable::new(),
        &custom,
        TOOLS_PATH,
        None,
    )
    .unwrap();
    // The node step shows its own script, one line, as the floor showed it before node left.
    let node_shown = get(&t, "tools/manager/npm").shown.clone().unwrap();
    assert!(!node_shown.contains('\n'));
    assert!(node_shown.contains(&format!("https://nodejs.org/dist/v{}/", node_release_22().version)));
    let shown: std::collections::BTreeMap<&str, Option<&str>> =
        t.installs.iter().filter(|i| i.id != "tools/manager/npm").map(|i| (i.id.as_str(), i.shown.as_deref())).collect();
    let expected: std::collections::BTreeMap<&str, Option<&str>> = [
        ("tools/homebrew", "git clone github.com/Homebrew/brew at 6.0.21"),
        ("tools/brew-toolchain/glibc", "brew install glibc"),
        ("tools/brew-toolchain/gcc", "brew install gcc"),
        ("tools/brew-tap/zingzy/tap", "brew tap zingzy/tap"),
        ("tools/brew-shared", "brew install the dependencies gh, yq, pipx share"),
        ("tools/brew/gh", "brew install gh"),
        ("tools/brew/yq", "brew install yq"),
        ("tools/npm/bun", "npm install -g bun@1.4.0"),
        ("tools/manager/pipx", "brew install pipx"),
        ("tools/pipx/black", "pipx install black==24.1.0"),
        ("tools/apt-index", "apt-get update"),
        ("tools/catalog/tmux", "apt-get install tmux"),
        ("tools/catalog/gh", "the v2.101.0 release of github.com/cli/cli"),
        ("tools/custom/just", "brew install just; just --version"),
    ]
    .into_iter()
    .map(|(id, line)| (id, Some(line)))
    .collect();
    assert_eq!(shown, expected);
}

// --- catalog rows ------------------------------------------------------------------------------------------------

// TS: packages/engine/test/golden-import.test.ts:1121
// "a catalog script row waits on what the catalog says it runs on top of: the apt index by its one step, a floor row on nothing"
#[test]
fn a_catalog_script_row_waits_on_what_the_catalog_says_it_runs_on_top_of() {
    let t = plan(&[catalog("swift"), catalog("playwright"), catalog("yarn"), catalog("shellcheck")]);
    assert_eq!(
        ima(&t.installs),
        vec![
            ("tools/apt-index", "apt", None),
            ("tools/catalog/swift", "script", Some("tools/apt-index")),
            // Playwright's npm road and yarn's script both name node, which is off the floor: one step brings it for both.
            ("tools/manager/npm", "script", None),
            ("tools/catalog/playwright", "script", Some("tools/manager/npm")),
            ("tools/catalog/yarn", "script", Some("tools/manager/npm")),
            ("tools/catalog/shellcheck", "apt", Some("tools/apt-index")),
        ]
    );
    assert_eq!(t.installs.iter().filter(|i| i.id == "tools/manager/npm").count(), 1);
    assert_eq!(t.installs.iter().filter(|i| i.id == "tools/apt-index").count(), 1);
}

// TS: packages/engine/test/golden-import.test.ts:1136
// "a ticked catalog tool this computer has no row for installs by its catalog road, named for its command; a road no golden build has run is noted"
#[test]
fn a_ticked_catalog_tool_this_computer_has_no_row_for_installs_by_its_catalog_road() {
    let t = plan(&[catalog("gh"), catalog("wrangler"), catalog("ffmpeg"), catalog("kubectl"), catalog("gcloud"), catalog("tmux")]);
    let rows: Vec<(&str, &str, Option<&str>, Option<&str>, Option<&str>)> =
        t.installs.iter().map(|i| (i.id.as_str(), i.manager.as_str(), i.after.as_deref(), i.bin.as_deref(), i.note.as_deref())).collect();
    assert_eq!(
        rows,
        vec![
            ("tools/manager/npm", "script", None, Some("node"), None),
            ("tools/catalog/wrangler", "npm", Some("tools/manager/npm"), Some("wrangler"), Some(UNMEASURED_ROAD)),
            ("tools/catalog/gh", "release", None, Some("gh"), Some(UNMEASURED_ROAD)),
            ("tools/apt-index", "apt", None, None, None),
            ("tools/catalog/ffmpeg", "apt", Some("tools/apt-index"), Some("ffmpeg"), Some(UNMEASURED_ROAD)),
            ("tools/catalog/kubectl", "vendor", None, Some("kubectl"), Some(UNMEASURED_ROAD)),
            ("tools/catalog/gcloud", "vendor", None, Some("gcloud"), None),
            ("tools/catalog/tmux", "apt", Some("tools/apt-index"), Some("tmux"), Some(UNMEASURED_ROAD)),
        ]
    );
    let cmd = |id: &str| cmd_of(&t, id).to_string();
    for i in &t.installs {
        assert!(on_tools_path(&i.cmd), "{}", i.id);
    }
    assert!(cmd("tools/catalog/wrangler").ends_with("\nnpm install -g wrangler"));
    assert!(cmd("tools/catalog/gh").contains("releases/download/v2.101.0/"));
    assert!(cmd("tools/catalog/gh").contains("name='gh'"));
    assert!(cmd("tools/apt-index").ends_with("\nexport DEBIAN_FRONTEND=noninteractive\napt-get update -qq"));
    assert!(cmd("tools/catalog/ffmpeg").ends_with("\napt-get install -y -qq ffmpeg"));
    assert_eq!(cmd("tools/catalog/kubectl"), format!("{}\n{}", path_line(TOOLS_PATH, None), KUBECTL.install));
    assert!(!t.installs.iter().any(|i| i.id == "tools/homebrew"));
    assert_eq!(t.skipped, Vec::<SkippedItem>::new());
    assert!(t.base.is_empty());
}

// TS: packages/engine/test/golden-import.test.ts:1161
// "carries its road's own presence read and the directories that road links into, so the job and the doctor read one rule"
#[test]
fn carries_its_roads_own_presence_read_and_the_directories_that_road_links_into() {
    // A formula row: the presence read is the prefix's link, and the check after the install stays brew's own list,
    // so the read that decides whether to install runs no brew at all and the check that proves one still does.
    let t = plan(&[tools("tools/brew/gh").linux("yes"), tools("tools/brew/yq").linux("yes")]);
    let formulae: Vec<&ToolInstall> = t.installs.iter().filter(|i| i.id.starts_with("tools/brew/")).collect();
    assert_eq!(formulae.iter().map(|i| i.id.as_str()).collect::<Vec<_>>(), vec!["tools/brew/gh", "tools/brew/yq"]);
    let brew_bins = vec![format!("{HOMEBREW_PREFIX}/bin"), format!("{HOMEBREW_PREFIX}/sbin"), "/usr/local/bin".to_string()];
    for step in formulae {
        assert_eq!(
            step.present.as_deref(),
            Some(format!("test -e {HOMEBREW_PREFIX}/opt/{}", &step.id["tools/brew/".len()..]).as_str()),
            "{}",
            step.id
        );
        assert!(step.check.as_deref().unwrap().contains("list --versions"), "{}", step.id);
        assert_eq!(step.bins.as_ref(), Some(&brew_bins), "{}", step.id);
    }
    // A brew tick the catalog covers is planned on the catalog's own road, which is node's installer: no presence
    // read of its own, since the command it puts on PATH is the read, and the directories are that script's.
    let covered_plan = plan(&[tools("tools/brew/node").linux("yes")]);
    let covered = get(&covered_plan, "tools/brew/node");
    assert_eq!(covered.manager, "script");
    assert_eq!(covered.bin.as_deref(), Some("node"));
    assert_eq!(covered.present, None);
    assert_eq!(covered.bins, Some(vec!["/usr/local/bin".to_string()]));
    // And a catalog row planned on the brew road carries brew's read and brew's directories, wherever it is planned.
    let go_plan = plan(&[catalog("go")]);
    let go = go_plan.installs.last().unwrap();
    assert_eq!(go.id, "tools/catalog/go");
    assert_eq!(go.present.as_deref(), Some(format!("test -e {HOMEBREW_PREFIX}/opt/go").as_str()));
    assert_eq!(go.bins, Some(brew_bins));
}

// TS: packages/engine/test/golden-import.test.ts:1181
// "a catalog tool on the Homebrew road brings Homebrew and its toolchain along, waits on them, and shares dependencies with the formulae ticked; a road a golden build has run carries no note"
#[test]
fn a_catalog_tool_on_the_homebrew_road_brings_homebrew_and_its_toolchain_along_and_waits_on_them() {
    let t = plan(&[catalog("go"), tools("tools/brew/yq").linux("yes")]);
    assert_eq!(
        ia(&t.installs),
        vec![
            ("tools/homebrew", None),
            ("tools/brew-toolchain/glibc", Some("tools/homebrew")),
            ("tools/brew-toolchain/gcc", Some("tools/brew-toolchain/glibc")),
            ("tools/brew-shared", Some("tools/brew-toolchain/gcc")),
            ("tools/brew/yq", Some("tools/brew-toolchain/gcc")),
            ("tools/catalog/go", Some("tools/brew-toolchain/gcc")),
        ]
    );
    assert!(cmd_of(&t, "tools/brew-shared").contains(r#"brew deps --for-each '\''yq'\'' '\''go'\'')"#));
    let last = t.installs.last().unwrap();
    assert_eq!(
        (last.id.as_str(), last.label.as_str(), last.manager.as_str(), last.bin.as_deref()),
        ("tools/catalog/go", "Go", "brew", Some("go"))
    );
    assert!(last.cmd.ends_with("brew install go'"));
    assert_eq!(last.note, None);
    // Homebrew's own bootstrap comes along for a catalog formula alone, as it does for a manager's.
    assert_eq!(
        ids(&plan(&[catalog("java")]).installs),
        vec!["tools/homebrew", "tools/brew-toolchain/glibc", "tools/brew-toolchain/gcc", "tools/catalog/java"]
    );
    // A catalog row on the script road brings none of it: Rust's rustup install is the whole step.
    let rust = plan(&[catalog("rust")]);
    let pairs: Vec<(&str, &str)> = rust.installs.iter().map(|i| (i.id.as_str(), i.manager.as_str())).collect();
    assert_eq!(pairs, vec![("tools/catalog/rust", "script")]);
}

// TS: packages/engine/test/golden-import.test.ts:1201
// "a catalog tool the floor carries is the base's, an id the catalog does not know is skipped, and a row this computer has keeps its own road at the laptop's version"
#[test]
fn a_catalog_tool_the_floor_carries_is_the_bases_an_unknown_id_is_skipped_a_row_keeps_its_own_road() {
    let t = plan(&[catalog("git"), catalog("nothing").label("nothing"), tools("tools/npm/wrangler").label("wrangler").version("4.1.0")]);
    assert_eq!(base_rows(&t, Platform::Darwin), base3(&[("tools/catalog/git", "git", "git is part of the base")]));
    assert_eq!(t.skipped, vec![skip("tools/catalog/nothing", "not in the catalog")]);
    let rows: Vec<(&str, &str, Option<&str>)> = t.installs.iter().map(|i| (i.id.as_str(), i.manager.as_str(), i.note.as_deref())).collect();
    assert_eq!(rows, vec![("tools/manager/npm", "script", None), ("tools/npm/wrangler", "npm", None)]);
    assert!(t.installs[1].cmd.ends_with("\nnpm install -g wrangler@4.1.0"));
}

// TS: packages/engine/test/golden-import.test.ts:1219
// "a catalog go row beside go rows is the manager's step: go installs once, the go rows wait on the catalog row, and no manager step is planned"
#[test]
fn a_catalog_go_row_beside_go_rows_is_the_managers_step_go_installs_once() {
    let t = plan(&[catalog("go"), tools("tools/go/gopls").label("gopls").paths(&["golang.org/x/tools/gopls@v0.16.2"])]);
    assert_eq!(
        ia(&t.installs),
        vec![
            ("tools/homebrew", None),
            ("tools/brew-toolchain/glibc", Some("tools/homebrew")),
            ("tools/brew-toolchain/gcc", Some("tools/brew-toolchain/glibc")),
            ("tools/catalog/go", Some("tools/brew-toolchain/gcc")),
            ("tools/go/gopls", Some("tools/catalog/go")),
        ]
    );
    assert_eq!(t.installs.iter().filter(|i| i.cmd.ends_with("brew install go'")).count(), 1);
    assert!(t.installs.last().unwrap().cmd.ends_with("\ngo install golang.org/x/tools/gopls@v0.16.2"));
}

// TS: packages/engine/test/golden-import.test.ts:1232
// "a catalog row's version is the install's where the road pins one; where the road cannot, the note says what it installed instead"
#[test]
fn a_catalog_rows_version_is_the_installs_where_the_road_pins_one_else_the_note_says_what_it_installed() {
    let t = plan(&[catalog("wrangler").version("4.1.0"), catalog("gh").version("v2.86.0"), catalog("tmux").version("3.5a")]);
    assert!(get(&t, "tools/catalog/wrangler").cmd.ends_with("\nnpm install -g wrangler@4.1.0"));
    assert_eq!(get(&t, "tools/catalog/wrangler").note.as_deref(), Some(UNMEASURED_ROAD));
    // A release row installs the catalog's pinned tag whatever the Mac runs, and the note says which one it got.
    assert_eq!(
        get(&t, "tools/catalog/gh").note.as_deref(),
        Some(format!("{UNMEASURED_ROAD}; asked v2.86.0, installed at the catalog's pinned v2.101.0").as_str())
    );
    assert!(get(&t, "tools/catalog/gh").cmd.contains("releases/download/v2.101.0/"));
    assert!(get(&t, "tools/catalog/tmux").cmd.ends_with("\napt-get install -y -qq tmux"));
    assert_eq!(
        get(&t, "tools/catalog/tmux").note.as_deref(),
        Some(format!("{UNMEASURED_ROAD}; 3.5a asked, installed by apt at its current version").as_str())
    );
}

// TS: packages/engine/test/golden-import.test.ts:1244
// "the catalog's pin wins over an older version on any road a row names the tool by"
#[test]
fn the_catalogs_pin_wins_over_an_older_version_on_any_road_a_row_names_the_tool_by() {
    let pnpm = road_version("pnpm");
    let t = plan(&[
        tools("tools/npm/pnpm").label("pnpm").version("10.34.5"),
        tools("tools/brew/pnpm").label("pnpm").version("10.34.5").linux("yes"),
        catalog("pnpm").version("10.34.5"),
        tools("tools/npm/wrangler").label("wrangler").version("4.1.0"),
    ]);
    for id in ["tools/npm/pnpm", "tools/brew/pnpm", "tools/catalog/pnpm"] {
        let step = get(&t, id);
        assert!(has_later_line(&step.cmd, &format!("npm install -g pnpm@{pnpm}")), "{id}");
        assert_eq!(step.asks.as_deref(), Some(pnpm.as_str()), "{id}");
        assert!(step.note.as_deref().unwrap().contains(&format!("asked 10.34.5, installed at the catalog's pinned {pnpm}")), "{id}");
    }
    // A tool the catalog does not pin keeps the version the row asked for.
    assert!(has_line(&get(&t, "tools/npm/wrangler").cmd, "npm install -g wrangler@4.1.0"));
    assert_eq!(get(&t, "tools/npm/wrangler").note, None);
}

// TS: packages/engine/test/golden-import.test.ts:1264
// "a row asking a newer version than the catalog pins keeps its own: the pin is a floor, never a downgrade"
#[test]
fn a_row_asking_a_newer_version_than_the_catalog_pins_keeps_its_own_the_pin_is_a_floor() {
    let pinned = road_version("agent-browser");
    assert_eq!(compare_versions("0.40.0", &pinned), Ordering::Greater);
    let t = plan(&[tools("tools/npm/agent-browser").label("agent-browser").version("0.40.0"), catalog("agent-browser").version("0.40.0")]);
    for id in ["tools/npm/agent-browser", "tools/catalog/agent-browser"] {
        let step = get(&t, id);
        assert!(has_line(&step.cmd, "npm install -g agent-browser@0.40.0"), "{id}");
        assert!(!step.note.clone().unwrap_or_default().contains("pinned"), "{id}");
    }
}

// TS: packages/engine/test/golden-import.test.ts:1275
// "the pin installs only where the computer's Node takes it: on an older Node the row keeps its own version, and the presence read holds the row to that one"
#[test]
fn the_pin_installs_only_where_the_computers_node_takes_it_older_node_keeps_the_rows_own_version() {
    let tmp = temp("wsp-pin-node-");
    let stubs = tmp.path().join("stubs");
    fs::create_dir(&stubs).unwrap();
    let log = tmp.path().join("npm.log");
    write_stub(&stubs.join("npm"), &format!("#!/bin/sh\necho \"$*\" >> {}\n", shell_quote(log.to_str().unwrap())));
    let node = |version: &str| write_stub(&stubs.join("node"), &format!("#!/bin/sh\necho v{version}\n"));
    let sh = |script: &str| bash_status(&stubbed(script, &stubs));
    let pin = road_version("pnpm");
    // A box's plan, on the order and under the prefix its job runs with, for each road a row names pnpm by.
    let t = tool_installs_for_rows(
        &[
            tools("tools/npm/pnpm").label("pnpm").version("10.34.5"),
            tools("tools/brew/pnpm").label("pnpm").version("10.34.5").linux("yes"),
            catalog("pnpm").version("10.34.5"),
        ],
        &BrewTable::new(),
        &[],
        &probe_path(GUEST_HOME),
        Some(TOOL_PREFIX),
    )
    .unwrap();
    let steps: Vec<&ToolInstall> = t.installs.iter().filter(|i| i.label == "pnpm").collect();
    assert_eq!(steps.len(), 3);
    for step in steps {
        assert!(
            step.note.as_deref().unwrap().contains(&format!(
                "asked 10.34.5, installed at the catalog's pinned {pin}, or 10.34.5 where the computer's Node is older than 22.13, which pnpm {pin} needs"
            )),
            "{}",
            step.id
        );
        for (version, installs) in [("20.20.2", "10.34.5"), ("22.12.0", "10.34.5"), ("22.13.0", pin.as_str()), ("24.18.1", pin.as_str())] {
            node(version);
            let _ = fs::remove_file(&log);
            assert_eq!(sh(&step.cmd), Some(0), "{} on Node {version}", step.id);
            assert_eq!(fs::read_to_string(&log).unwrap(), format!("install -g pnpm@{installs}\n"), "{} on Node {version}", step.id);
            // Present where the version on the computer is the one this Node takes, and stale otherwise.
            for have in ["10.34.5", pin.as_str()] {
                let with_read =
                    ToolInstall { pin: Some(PinRead { read: Some(format!("echo {have}")), ..step.pin.clone().unwrap() }), ..step.clone() };
                let read = presence_tests(&with_read, Versions::Asked).last().unwrap().clone();
                assert_eq!(
                    sh(&format!("export PATH=/usr/bin:/bin\n{read}")),
                    Some(if have == installs { 0 } else { 1 }),
                    "{} with {have} on Node {version}",
                    step.id
                );
            }
        }
        // A node that answers no version takes nothing newer than the row asked.
        write_stub(&stubs.join("node"), "#!/bin/sh\nexit 127\n");
        let _ = fs::remove_file(&log);
        sh(&step.cmd);
        assert_eq!(fs::read_to_string(&log).unwrap(), "install -g pnpm@10.34.5\n");
    }
}

// TS: packages/engine/test/golden-import.test.ts:1314
// "a box's npm step is main's one install line: nothing is linked, npm's folder is never read, and a prefix and a command planted in the shared home change nothing root runs"
#[test]
fn a_boxs_npm_step_is_mains_one_install_line_nothing_is_linked_and_a_planted_home_changes_nothing() {
    let tmp = temp("wsp-box-npm-");
    let t = tool_installs_for_rows(
        &[tools("tools/npm/wrangler").label("wrangler").version("4.1.0")],
        &BrewTable::new(),
        &[],
        &probe_path(GUEST_HOME),
        Some(TOOL_PREFIX),
    )
    .unwrap();
    let step = get(&t, "tools/npm/wrangler");
    assert_eq!(step.cmd, format!("{}\nnpm install -g wrangler@4.1.0", path_line(&probe_path(GUEST_HOME), Some(TOOL_PREFIX))));
    // The home every workspace on the box writes: an npmrc moving npm's prefix into it, and a command planted there
    // under a name root runs, linked into a package folder npm is about to replace.
    let home = tmp.path().join("home");
    fs::create_dir_all(home.join(".p").join("bin")).unwrap();
    fs::create_dir_all(home.join(".p").join("lib").join("node_modules").join("wrangler")).unwrap();
    Command::new("sh")
        .args([
            "-c",
            "printf 'prefix=%s\\n' \"$1/.p\" > \"$1/.npmrc\"; ln -s \"$1/.p/lib/node_modules/wrangler/planted\" \"$1/.p/bin/git\"",
            "sh",
            home.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    let stubs = tmp.path().join("stubs");
    fs::create_dir(&stubs).unwrap();
    let log = tmp.path().join("npm.log");
    // npm as root's job meets it: it takes the prefix the home's npmrc names, and says every call it got.
    write_stub(
        &stubs.join("npm"),
        &format!(
            "#!/bin/sh\necho \"$*\" >> {}\np=\"$(sed -n 's/^prefix=//p' \"$HOME/.npmrc\")\"\n[ \"$1 $2\" = \"install -g\" ] && mkdir -p \"$p/lib/node_modules/wrangler\"\nexit 0\n",
            shell_quote(log.to_str().unwrap())
        ),
    );
    let ls = || String::from_utf8_lossy(&Command::new("ls").args(["-la", LOCAL_BIN]).output().unwrap().stdout).into_owned();
    let before = ls();
    let res = bash_env(&stubbed(&step.cmd, &stubs), &[("HOME", home.to_str().unwrap())]);
    assert_eq!(res.status.code(), Some(0));
    // One call, the install: the step's cost is npm's alone, however many commands npm's folder holds.
    assert_eq!(fs::read_to_string(&log).unwrap(), "install -g wrangler@4.1.0\n");
    assert_eq!(ls(), before);
    // The planted name answers nowhere on the order a workspace and the job run on.
    let found = bash_env(&format!("export PATH={}; command -v git", probe_path(GUEST_HOME)), &[("HOME", home.to_str().unwrap())]);
    let git = String::from_utf8_lossy(&found.stdout).trim().to_string();
    assert!(!git.starts_with(home.to_str().unwrap()));
}

// TS: packages/engine/test/golden-import.test.ts:1342
// "Playwright's installer reaches the command npm put in its own folder where that folder is on no PATH, and never one under the shared home"
#[test]
fn playwrights_installer_reaches_the_command_npm_put_in_its_own_folder_never_one_under_the_shared_home() {
    let tmp = temp("wsp-playwright-");
    let stubs = tmp.path().join("stubs");
    let prefix = tmp.path().join("nvm");
    fs::create_dir(&stubs).unwrap();
    fs::create_dir_all(prefix.join("bin")).unwrap();
    let log = tmp.path().join("playwright.log");
    write_stub(
        &stubs.join("npm"),
        &format!("#!/bin/sh\n[ \"$1 $2\" = \"prefix -g\" ] && echo \"${{STUB_PREFIX:-{}}}\"\nexit 0\n", prefix.display()),
    );
    write_stub(&prefix.join("bin").join("playwright"), &format!("#!/bin/sh\necho \"$*\" >> {}\n", shell_quote(log.to_str().unwrap())));
    let script = match catalog_entry("playwright").unwrap().install_road() {
        InstallRoad::Script { script, .. } => script.clone(),
        other => panic!("playwright is not on the script road: {other:?}"),
    };
    let run = |env: &[(&str, &str)]| bash_env(&format!("export PATH={}:/usr/bin:/bin\n{script}", stubs.display()), env).status.code();
    assert_eq!(run(&[]), Some(0));
    assert_eq!(fs::read_to_string(&log).unwrap(), "install --with-deps chromium\n");
    // A prefix under the home a box shares with every workspace is not looked in: the installer is not found.
    fs::remove_file(&log).unwrap();
    assert_eq!(run(&[("STUB_PREFIX", &format!("{GUEST_HOME}/.p"))]), Some(127));
}

// TS: packages/engine/test/golden-import.test.ts:1361
// "a catalog row installs the artifact the catalog pinned and checks the sum beside it, whatever a recipe recorded"
#[test]
fn a_catalog_row_installs_the_artifact_the_catalog_pinned_and_checks_the_sum_beside_it() {
    let t = plan(&[catalog("gh").pin("v2.86.0", &"d".repeat(64)), catalog("kubectl").pin("v1.37.0", &"e".repeat(64))]);
    let gh = cmd_of(&t, "tools/catalog/gh");
    assert!(gh.contains("releases/download/v2.101.0/"));
    assert!(!gh.contains("api.github.com"));
    // The recorded sum of an older tag decides nothing: the catalog's own sum is checked before the unpack.
    assert!(!gh.contains(&format!("[ \"$sum\" = '{}' ]", "d".repeat(64))));
    assert!(gh.contains("echo \"$sha  $tmp/$asset\" | sha256sum -c - >/dev/null"));
    assert_eq!(cmd_of(&t, "tools/catalog/kubectl"), format!("{}\n{}", path_line(TOOLS_PATH, None), KUBECTL.install));
}

// --- tap formulae on the release road ----------------------------------------------------------------------------

fn diskbloom() -> BrewTable {
    table(vec![formula("diskbloom", "zingzy/tap/diskbloom", None, Some(("Zingzy/diskbloom", "v0.1.0")))])
}

// TS: packages/engine/test/golden-import.test.ts:1375
// "a tap formula on the road names its binary too"
#[test]
fn a_tap_formula_on_the_road_names_its_binary_too() {
    let t = plan_table(&[tools("tools/brew/zingzy/tap/diskbloom").label("diskbloom").linux("unknown")], &diskbloom());
    let step = t.installs.last().unwrap();
    assert_eq!(step.id, "tools/brew/zingzy/tap/diskbloom");
    assert_eq!(step.manager, "release");
    assert_eq!(step.bin.as_deref(), Some("diskbloom"));
    assert_eq!(step.after, None);
    assert!(step.cmd.contains("https://api.github.com/repos/Zingzy/diskbloom/releases/tags/v0.1.0"));
    assert!(step.cmd.contains("install -m 0755 \"$bin\" \"/usr/local/bin/$name\""));
    assert!(step.cmd.contains("GOBIN=/usr/local/bin go install 'github.com/Zingzy/diskbloom@v0.1.0'"));
    assert!(!step.cmd.contains("mv '/usr/local/bin/"));
    assert!(!step.cmd.contains("[ \"$sum\" ="));
}

// TS: packages/engine/test/golden-import.test.ts:1388
// "a pin for the same tag is checked; one for another tag is not"
#[test]
fn a_pin_for_the_same_tag_is_checked_one_for_another_tag_is_not() {
    let brew = diskbloom();
    let road = |tag: &str, sha: &str| {
        plan_table(&[tools("tools/brew/zingzy/tap/diskbloom").label("diskbloom").linux("unknown").pin(tag, sha)], &brew)
            .installs
            .last()
            .unwrap()
            .cmd
            .clone()
    };
    assert!(road("v0.1.0", &"c".repeat(64)).contains(&format!("[ \"$sum\" = '{}' ]", "c".repeat(64))));
    assert!(!road("v0.0.9", &"c".repeat(64)).contains("[ \"$sum\" ="));
}

// TS: packages/engine/test/golden-import.test.ts:1395
// "the go fallback lands under the row's command: a module whose last element is another name is moved there; a /vN module suffix is not the name"
#[test]
fn the_go_fallback_lands_under_the_rows_command_a_module_suffix_v_n_is_not_the_name() {
    let spoo = table(vec![formula("spoo", "spoo-me/tap/spoo", None, Some(("spoo-me/spoo-cli", "v0.4.1")))]);
    let t = plan_table(&[tools("tools/brew/spoo-me/tap/spoo").label("spoo").linux("unknown")], &spoo);
    let tap = t.installs.last().unwrap();
    assert_eq!(tap.manager, "release");
    assert_eq!(tap.bin.as_deref(), Some("spoo"));
    assert!(tap.cmd.contains("GOBIN=/usr/local/bin go install 'github.com/spoo-me/spoo-cli@v0.4.1'\n  mv '/usr/local/bin/spoo-cli' \"/usr/local/bin/$name\"\n  echo \"WSP_ROAD go \""));
    let v2 = table(vec![formula("spoo", "spoo-me/tap/spoo", None, Some(("spoo-me/spoo/v2", "v2.0.0")))]);
    let t2 = plan_table(&[tools("tools/brew/spoo-me/tap/spoo").label("spoo").linux("unknown")], &v2);
    assert!(!t2.installs.last().unwrap().cmd.contains("mv '/usr/local/bin/"));
}

// --- the node a recipe's own rows bring --------------------------------------------------------------------------

const NODE_STEP: &str = "tools/manager/npm";

/// golden-import.test.ts:1421 `tool`: a ticked tools row labelled by the last segment of its id.
fn tool(id: &str) -> RecipeEntry {
    tools(id).label(&id[id.rfind('/').unwrap() + 1..])
}

/// golden-import.test.ts:1422 `plan(...ids)`: each step's id and what it waits on.
fn node_plan(rows: &[&str]) -> Vec<(String, Option<String>)> {
    let entries: Vec<RecipeEntry> = rows.iter().map(|id| tool(id)).collect();
    plan(&entries).installs.into_iter().map(|t| (t.id, t.after)).collect()
}

fn waits(rows: &[(&str, Option<&str>)]) -> Vec<(String, Option<String>)> {
    rows.iter().map(|(id, after)| (id.to_string(), after.map(str::to_string))).collect()
}

// TS: packages/engine/test/golden-import.test.ts:1425
// "runs once, ahead of every row whose road runs on it, whichever road asked"
#[test]
fn the_node_step_runs_once_ahead_of_every_row_whose_road_runs_on_it_whichever_road_asked() {
    assert_eq!(node_plan(&["tools/npm/pnpm"]), waits(&[(NODE_STEP, None), ("tools/npm/pnpm", Some(NODE_STEP))]));
    // Two rows on the road, and a row whose own road is another that runs on node: still one node step, first.
    assert_eq!(
        node_plan(&["tools/npm/pnpm", "tools/npm/prettier", "tools/catalog/yarn"]),
        waits(&[
            (NODE_STEP, None),
            ("tools/npm/pnpm", Some(NODE_STEP)),
            ("tools/npm/prettier", Some(NODE_STEP)),
            ("tools/catalog/yarn", Some(NODE_STEP)),
        ])
    );
    // bun's own install is an npm global, so the step that brings bun waits on node as its rows wait on bun.
    assert_eq!(
        node_plan(&["tools/bun/eslint"]),
        waits(&[(NODE_STEP, None), ("tools/manager/bun", Some(NODE_STEP)), ("tools/bun/eslint", Some("tools/manager/bun"))])
    );
}

// TS: packages/engine/test/golden-import.test.ts:1442
// "is the recipe's own node row when it ticked one, so the person's tick is the step and nothing installs node twice"
#[test]
fn the_node_step_is_the_recipes_own_node_row_when_it_ticked_one_and_nothing_installs_node_twice() {
    assert_eq!(
        node_plan(&["tools/catalog/node", "tools/npm/pnpm"]),
        waits(&[("tools/catalog/node", None), ("tools/npm/pnpm", Some("tools/catalog/node"))])
    );
    // The row that waits on node comes after the node row in the recipe, so the wait is read while that row is
    // planned: what stands for npm is settled before the first row, or the node row is planned twice.
    let one = waits(&[("tools/catalog/node", None), ("tools/catalog/yarn", Some("tools/catalog/node"))]);
    assert_eq!(node_plan(&["tools/catalog/node", "tools/catalog/yarn"]), one);
    assert_eq!(node_plan(&["tools/catalog/yarn", "tools/catalog/node"]), one);
    assert_eq!(
        node_plan(&["tools/catalog/node", "tools/catalog/playwright"]),
        vec![one[0].clone(), ("tools/catalog/playwright".to_string(), Some("tools/catalog/node".to_string()))]
    );
    // This Mac's own node formula is the catalog's node row, and the same holds for it.
    assert_eq!(
        node_plan(&["tools/brew/node", "tools/catalog/yarn"]),
        waits(&[("tools/brew/node", None), ("tools/catalog/yarn", Some("tools/brew/node"))])
    );
}

// TS: packages/engine/test/golden-import.test.ts:1457
// "is not there at all for a recipe whose rows need no node"
#[test]
fn the_node_step_is_not_there_at_all_for_a_recipe_whose_rows_need_no_node() {
    let got: Vec<String> = node_plan(&["tools/catalog/gh", "tools/uv/ruff"]).into_iter().map(|(id, _)| id).collect();
    assert_eq!(got, vec!["tools/uv/ruff".to_string(), "tools/catalog/gh".to_string()]);
}

// TS: packages/engine/test/golden-import.test.ts:1461
// "is the catalog's own script: the node step and a node row install the same pinned release"
#[test]
fn the_node_step_is_the_catalogs_own_script_the_node_step_and_a_node_row_install_the_same_release() {
    let t = plan(&[tool("tools/npm/pnpm")]);
    let step = get(&t, NODE_STEP);
    assert_eq!(step.label, "Node 22 with npm");
    assert_eq!(step.bin.as_deref(), Some("node"));
    assert_eq!(step.manager, "script");
    assert!(step.cmd.contains(&node_install_script(22, &node_release_22())));
}

// TS: packages/engine/test/golden-import.test.ts:1469
// "keeps a node the machine already has: the script prints NODE_KEPT and downloads nothing"
#[test]
fn keeps_a_node_the_machine_already_has_the_script_prints_node_kept_and_downloads_nothing() {
    let tmp = temp("wsp-node-step-");
    let dir = tmp.path();
    write_stub(&dir.join("node"), "#!/bin/sh\necho v22.23.2\n");
    write_stub(&dir.join("curl"), "#!/bin/sh\necho \"curl ran\" >&2\nexit 1\n");
    let mut lines = road_step(RoadName::Script).env;
    lines.push(node_install_script(22, &node_release_22()));
    let script = lines.join("\n");
    let res = bash_env(&script, &[("HOME", dir.to_str().unwrap()), ("PATH", &format!("{}:/usr/bin:/bin", dir.display()))]);
    assert_eq!(res.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&res.stdout).contains("NODE_KEPT v22.23.2"));
    assert!(!String::from_utf8_lossy(&res.stderr).contains("curl ran"));
}

// TS: packages/engine/test/golden-import.test.ts:1481
// "installs the row's own Node over a newer one the machine has: Node 22 with npm never leaves Node 24 standing"
#[test]
fn installs_the_rows_own_node_over_a_newer_one_the_machine_has_never_leaves_node_24_standing() {
    let tmp = temp("wsp-node-step-");
    let dir = tmp.path();
    write_stub(&dir.join("node"), "#!/bin/sh\necho v24.18.1\n");
    write_stub(&dir.join("curl"), "#!/bin/sh\necho \"curl ran $*\" >&2\nexit 1\n");
    // A guest's arch, not the machine running the test: a Mac answers arm64, which no Linux guest reads.
    write_stub(&dir.join("uname"), "#!/bin/sh\necho x86_64\n");
    let entry = catalog_entry("node").unwrap();
    assert_eq!(entry.name(), "Node 22 with npm");
    let mut lines = road_step(RoadName::Script).env;
    lines.push(install_line(entry).unwrap());
    let script = lines.join("\n");
    let res = bash_env(&script, &[("HOME", dir.to_str().unwrap()), ("PATH", &format!("{}:/usr/bin:/bin", dir.display()))]);
    assert!(!String::from_utf8_lossy(&res.stdout).contains("NODE_KEPT"));
    let stderr = String::from_utf8_lossy(&res.stderr).into_owned();
    let url = format!("https://nodejs.org/dist/v{}/", node_release_22().version);
    assert!(
        stderr.lines().any(|l| l.find("curl ran ").is_some_and(|i| l[i + "curl ran ".len()..].contains(&url))),
        "stderr was {stderr:?}"
    );
}

// --- the version a step asks for ---------------------------------------------------------------------------------

// TS: packages/engine/test/golden-import.test.ts:1681
// "is the row's own where its road installs at one, and nothing where the road installs what its source serves"
#[test]
fn the_version_a_step_asks_for_is_the_rows_own_where_its_road_installs_at_one() {
    let t = plan(&[tools("tools/npm/bun").label("bun").version("1.4.0"), tools(&format!("{CATALOG_PREFIX}tmux")).label("tmux")]);
    let asks = |id: &str| get(&t, id).asks.clone();
    assert_eq!(asks("tools/npm/bun").as_deref(), Some("1.4.0"));
    // apt installs the distribution's own package: the line names no version, so the step asks for none, whatever
    // the row says this computer runs.
    assert!(matches!(catalog_entry("tmux").unwrap().install_road(), InstallRoad::Apt { .. }));
    assert_eq!(asks(&format!("{CATALOG_PREFIX}tmux")), None);
}
