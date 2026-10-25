// SPDX-License-Identifier: AGPL-3.0-only
//! Ports packages/engine/test/tool-sizes.test.ts and brew-shared-row.test.ts. The four "tap formulae without a Linux
//! bottle" cases of tool-sizes (brewfileFor, toolInstallsFor of golden-import) are not here, and the brew-shared cases
//! run the shared step's scripts directly, without golden-tools' installTools.
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use indexmap::IndexMap;
use serde_json::{json, Value};

use wsp_catalog::catalog::{catalog_entry, size_bytes};
use wsp_engine::brew_shared::{brew_shared_check, brew_shared_deps};
use wsp_engine::golden_seam::{BrewFormula, BrewTable, RecipeEntry, ToolSource};
use wsp_engine::tool_sizes::{agent_size, assumed_size, brew_table, parse_brew_info, parse_du, source_of, tool_size, SizeRoad, ToolSize};

const MIB: u64 = 1024 * 1024;

fn formula(over: Value) -> Value {
    let mut base = json!({
        "tap": "homebrew/core",
        "dependencies": [],
        "build_dependencies": [],
        "requirements": [],
        "urls": { "stable": { "url": "https://example.org/src.tar.xz", "tag": null } },
        "versions": { "stable": "1.0", "bottle": true },
        "installed": [{ "version": "1.0", "runtime_dependencies": [] }],
    });
    base.as_object_mut().unwrap().extend(over.as_object().unwrap().clone());
    base
}

/// What `brew info --json=v2 --installed` says on a Mac with a few formulae and four tap formulae.
fn info() -> Value {
    json!({
        "formulae": [
            formula(json!({ "name": "ffmpeg", "full_name": "ffmpeg", "dependencies": ["x264", "openssl@3"], "installed": [{ "version": "8.1.2", "runtime_dependencies": [{ "full_name": "x264" }, { "full_name": "openssl@3" }, { "full_name": "ca-certificates" }] }] })),
            formula(json!({ "name": "x264", "full_name": "x264" })),
            formula(json!({ "name": "openssl@3", "full_name": "openssl@3", "dependencies": ["ca-certificates"] })),
            formula(json!({ "name": "ca-certificates", "full_name": "ca-certificates" })),
            formula(json!({ "name": "gh", "full_name": "gh", "build_dependencies": ["go"], "urls": { "stable": { "url": "https://github.com/cli/cli/archive/refs/tags/v2.100.0.tar.gz", "tag": null } } })),
            formula(json!({ "name": "llvm@21", "full_name": "llvm@21", "dependencies": ["zstd"] })),
            formula(json!({ "name": "zstd", "full_name": "zstd" })),
            formula(json!({ "name": "diskbloom", "full_name": "zingzy/tap/diskbloom", "tap": "zingzy/tap", "versions": { "stable": "0.1.0", "bottle": false }, "urls": { "stable": { "url": "https://github.com/Zingzy/diskbloom/releases/download/v0.1.0/diskbloom_0.1.0_darwin_arm64.tar.gz", "tag": null } } })),
            formula(json!({ "name": "sketchybar", "full_name": "felixkratz/formulae/sketchybar", "tap": "felixkratz/formulae", "versions": { "stable": "2.24.0", "bottle": false }, "urls": { "stable": { "url": "https://github.com/FelixKratz/SketchyBar/archive/refs/tags/v2.24.0.tar.gz", "tag": null } } })),
            formula(json!({ "name": "macthing", "full_name": "someone/tap/macthing", "tap": "someone/tap", "requirements": [{ "name": "macos", "version": "13" }], "urls": { "stable": { "url": "https://github.com/someone/macthing/archive/refs/tags/v1.tar.gz", "tag": null } } })),
            formula(json!({ "name": "gitonly", "full_name": "someone/tap/gitonly", "tap": "someone/tap", "urls": { "stable": { "url": "https://github.com/someone/gitonly.git", "tag": "v3.1.0" } } })),
            formula(json!({ "name": "elsewhere", "full_name": "someone/tap/elsewhere", "tap": "someone/tap", "urls": { "stable": { "url": "https://dl.example.org/elsewhere-1.tar.gz", "tag": null } } })),
        ],
        "casks": [],
    })
}

fn du_text() -> String {
    [
        "102400\t/opt/homebrew/Cellar/ffmpeg",
        "20480\t/opt/homebrew/Cellar/x264",
        "30720\t/opt/homebrew/Cellar/openssl@3",
        "1024\t/opt/homebrew/Cellar/ca-certificates",
        "51200\t/opt/homebrew/Cellar/gh",
        "2048000\t/opt/homebrew/Cellar/llvm@21",
        "5120\t/opt/homebrew/Cellar/zstd",
        "9800\t/opt/homebrew/Cellar/diskbloom",
        "1000\t/opt/homebrew/Cellar/sketchybar",
        "",
    ]
    .join("\n")
}

fn table() -> BrewTable {
    brew_table(&info(), &parse_du(&du_text(), None))
}

fn row(id: &str) -> RecipeEntry {
    RecipeEntry { rung: "tools".into(), id: id.into(), ..RecipeEntry::default() }
}

fn agent_row(id: &str) -> RecipeEntry {
    RecipeEntry { rung: "agents".into(), id: id.into(), ..RecipeEntry::default() }
}

fn brew(name: &str) -> RecipeEntry {
    row(&format!("tools/brew/{name}"))
}

fn formula_row(name: &str, deps: &[&str], bytes: Option<f64>) -> BrewFormula {
    BrewFormula {
        name: name.into(),
        full_name: name.into(),
        deps: deps.iter().map(|d| d.to_string()).collect(),
        bytes,
        version: None,
        macos_only: false,
        source: None,
    }
}

fn source(repo: &str, tag: &str) -> Option<ToolSource> {
    Some(ToolSource { repo: repo.into(), tag: tag.into() })
}

/// The catalog's measured size of an entry, read after the call under test.
fn measured_bytes(id: &str) -> f64 {
    size_bytes(catalog_entry(id).unwrap().size()).unwrap() as f64
}

fn measured(id: &str) -> ToolSize {
    ToolSize { bytes: measured_bytes(id), road: SizeRoad::Measured, deps: 0 }
}

// ---- the Mac's Homebrew as a table

// TS: packages/engine/test/tool-sizes.test.ts:55
#[test]
fn reads_name_dependencies_size_and_source_per_formula_a_du_line_without_a_formula_is_dropped() {
    let rows = parse_brew_info(&info());
    assert!(rows.iter().any(|f| f.full_name == "zingzy/tap/diskbloom"));
    let table = table();
    let ffmpeg = &table["ffmpeg"];
    // Direct and transitive runtime dependencies, each once.
    let mut deps = ffmpeg.deps.clone();
    deps.sort();
    assert_eq!(deps, vec!["ca-certificates".to_string(), "openssl@3".to_string(), "x264".to_string()]);
    assert_eq!(ffmpeg.bytes, Some(102400.0 * 1024.0));
    assert_eq!(ffmpeg.version, Some("8.1.2".to_string()));
    assert_eq!(table["gh"].source, source("cli/cli", "v2.100.0"));
    assert_eq!(table["zingzy/tap/diskbloom"].source, source("Zingzy/diskbloom", "v0.1.0"));
    assert_eq!(table["someone/tap/gitonly"].source, source("someone/gitonly", "v3.1.0"));
    assert_eq!(table["someone/tap/elsewhere"].source, None);
    assert_eq!(table["someone/tap/elsewhere"].bytes, None);
    assert!(table["someone/tap/macthing"].macos_only);
    assert!(table["felixkratz/formulae/sketchybar"].macos_only);
    assert!(!table["zingzy/tap/diskbloom"].macos_only);
    assert_eq!(parse_brew_info(&json!({ "nope": 1 })), Vec::<BrewFormula>::new());
    assert_eq!(parse_du("garbage\n\t\n12 /x", None), IndexMap::from([("x".to_string(), 12.0 * 1024.0)]));
}

// TS: packages/engine/test/tool-sizes.test.ts:75
#[test]
fn names_the_github_repository_and_tag_behind_a_formulas_url() {
    assert_eq!(source_of(Some("https://github.com/cli/cli/archive/refs/tags/v2.100.0.tar.gz"), None), source("cli/cli", "v2.100.0"));
    assert_eq!(source_of(Some("https://github.com/a/b/releases/download/1.2.3/b_1.2.3_linux.zip"), None), source("a/b", "1.2.3"));
    assert_eq!(source_of(Some("https://github.com/a/b.git"), Some("v9")), source("a/b", "v9"));
    assert_eq!(source_of(Some("https://github.com/a/b.git"), None), None);
    assert_eq!(source_of(Some("https://gitlab.com/a/b/-/archive/v1/b-v1.tar.gz"), None), None);
}

// ---- toolSize

// TS: packages/engine/test/tool-sizes.test.ts:85
#[test]
fn a_formulas_number_is_its_dependency_closure_read_from_this_macs_homebrew() {
    let s = tool_size(&brew("ffmpeg"), &table()).unwrap();
    assert_eq!(s.bytes, ((102400 + 20480 + 30720 + 1024) * 1024) as f64);
    assert_eq!(s.deps, 3);
    assert_eq!(s.road, SizeRoad::Mac);
}

// TS: packages/engine/test/tool-sizes.test.ts:92
#[test]
fn a_formula_the_catalog_carries_takes_the_catalogs_measured_closure_once_whatever_this_macs_table_says() {
    let alone = tool_size(&brew("go"), &BrewTable::new());
    assert_eq!(alone, Some(measured("go")));
    // The catalog's number already holds the Linux runtime dependencies, so the Mac's are not added on top.
    let mut with_go = table();
    with_go.insert("go".into(), formula_row("go", &["zstd"], Some(1.0)));
    assert_eq!(tool_size(&brew("go"), &with_go), Some(measured("go")));
    // A formula the catalog does not carry is sized from this Mac alone, with its closure; without a table it has no size.
    assert_eq!(
        tool_size(&brew("llvm@21"), &table()),
        Some(ToolSize { bytes: ((2048000 + 5120) * 1024) as f64, road: SizeRoad::Mac, deps: 1 })
    );
    assert_eq!(tool_size(&brew("llvm@21"), &BrewTable::new()), None);
}

// TS: packages/engine/test/tool-sizes.test.ts:103
#[test]
fn a_formula_nothing_measured_or_read_has_no_size_taps_are_nothing_a_managers_global_takes_the_catalogs_number() {
    assert_eq!(tool_size(&brew("gh"), &BrewTable::new()), None);
    assert_eq!(tool_size(&row("tools/brew-tap/zingzy/tap"), &table()), None);
    assert_eq!(tool_size(&row("tools/npm/bun"), &table()), Some(measured("bun")));
    assert_eq!(tool_size(&row("tools/npm/typescript"), &table()), Some(measured("typescript")));
    assert_eq!(tool_size(&row("tools/uv/ruff"), &table()), Some(measured("ruff")));
    assert_eq!(tool_size(&row("tools/npm/left-pad"), &table()), None);
}

// TS: packages/engine/test/tool-sizes.test.ts:112
#[test]
fn homebrews_own_toolchain_is_left_out_of_a_closure_it_is_the_groups_own_line() {
    let mut with_gcc = table();
    with_gcc.insert("needs-gcc".into(), formula_row("needs-gcc", &["gcc", "zstd"], Some((10 * MIB) as f64)));
    assert_eq!(
        tool_size(&brew("needs-gcc"), &with_gcc),
        Some(ToolSize { bytes: (10 * MIB + 5120 * 1024) as f64, road: SizeRoad::Mac, deps: 1 })
    );
}

// ---- agentSize

// TS: packages/engine/test/tool-sizes.test.ts:119
#[test]
fn measured_install_sizes_by_agent_an_agent_nobody_measured_has_none() {
    assert_eq!(agent_size(&agent_row("agents/opencode")), Some(673 * MIB));
    assert_eq!(agent_size(&agent_row("agents/claude")), Some(208 * MIB));
    assert_eq!(agent_size(&agent_row("agents/aider")), None);
}

// ---- assumedSize

// TS: packages/engine/test/tool-sizes.test.ts:127
#[test]
fn a_row_nothing_measured_counts_at_the_default_for_its_kind_and_says_which_kind() {
    let said = |e: &RecipeEntry| {
        let a = assumed_size(e);
        (a.bytes, a.kind.words())
    };
    assert_eq!(said(&row("tools/go/gopls")), (500 * MIB, "a go install"));
    assert_eq!(said(&row("tools/uv/ty")), (100 * MIB, "a uv tool"));
    assert_eq!(said(&row("tools/npm/left-pad")), (50 * MIB, "an npm global"));
    assert_eq!(said(&row("tools/pnpm/left-pad")), (50 * MIB, "an npm global"));
    assert_eq!(said(&row("tools/cargo/ripgrep")), (100 * MIB, "an install"));
    assert_eq!(said(&row("tools/brew/nobody/tap/mystery")), (100 * MIB, "an install"));
    assert_eq!(said(&agent_row("agents/aider")), (350 * MIB, "an agent"));
}

// ---- brew-shared-row: the shared step's scripts, run in this machine's bash with su and brew stubbed

/// Every awk this computer has: Debian's mawk and gawk on Linux, the BSD one on a Mac.
fn awks() -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = Vec::new();
    for a in ["/usr/bin/mawk", "/usr/bin/gawk", "/usr/bin/awk"] {
        if let Ok(real) = std::fs::canonicalize(a) {
            if !found.contains(&real) {
                found.push(real);
            }
        }
    }
    found
}

fn stub(path: &Path, script: &str) {
    std::fs::write(path, script).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

/// The hetzner box of the report: of the dependencies zig and cargo-zigbuild share, llvm@21 is not installed. `fails`
/// is what the stub brew's install does: fail on llvm@21 the way a child process failure reads, or exit 0.
fn box_dir(fails: bool, awk: &Path) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::os::unix::fs::symlink(awk, dir.path().join("awk")).unwrap();
    stub(
        &dir.path().join("su"),
        "#!/bin/sh\nshell=/bin/sh\nwhile [ $# -gt 0 ]; do case \"$1\" in -s) shell=\"$2\"; shift 2 ;; -c) script=\"$2\"; shift 2 ;; --) shift; break ;; *) shift ;; esac; done\nexec \"$shell\" -c \"$script\" \"$@\"\n",
    );
    let install = if fails {
        r#"echo "==> Pouring z3"; echo "Error: An exception occurred within a child process:" >&2; echo "  BuildError: llvm@21 failed to pour" >&2; exit 1"#
    } else {
        "exit 0"
    };
    stub(
        &dir.path().join("brew"),
        &format!(
            "#!/bin/sh\ncase \"$1\" in\n  deps) printf '%s\\n' 'zig: llvm@21 z3 zstd' 'cargo-zigbuild: llvm@21 z3 zstd zig' 'gh:' ;;\n  list) printf '%s\\n' 'z3 4.15.3' 'zstd 1.5.7'; exit 1 ;;\n  install) {install} ;;\nesac\nexit 0\n"
        ),
    );
    dir
}

/// The script's own run: exit code, stdout, stderr.
fn run_plan_script(cmd: &str, dir: &Path) -> (i32, String, String) {
    let at = dir.to_str().unwrap();
    let real_brew = wsp_catalog::road_modules::BREW.as_str();
    let text = cmd.replace(real_brew, &format!("{at}/brew")).replace("export PATH=", &format!("export PATH={at}:"));
    let out = Command::new("bash").args(["-c", &text]).env("PATH", format!("{at}:/usr/bin:/bin")).output().unwrap();
    (out.status.code().unwrap_or(1), String::from_utf8_lossy(&out.stdout).into_owned(), String::from_utf8_lossy(&out.stderr).into_owned())
}

fn plan_formulae() -> Vec<String> {
    ["zig", "cargo-zigbuild", "gh"].iter().map(|s| s.to_string()).collect()
}

fn last_line(text: &str) -> String {
    text.trim_end().lines().last().unwrap_or("").to_string()
}

// TS: packages/engine/test/brew-shared-row.test.ts:49
#[test]
fn a_failed_shared_check_names_the_formula_that_is_missing_and_the_recipes_tools_that_need_it_never_its_shell() {
    let cmd = brew_shared_check(&plan_formulae());
    for awk in awks() {
        let dir = box_dir(false, &awk);
        let (code, stdout, stderr) = run_plan_script(&cmd, dir.path());
        assert_eq!(code, 1, "awk {}: stderr {stderr}", awk.display());
        assert_eq!(last_line(&stdout), "llvm@21 is not installed; zig and cargo-zigbuild need it.", "awk {}", awk.display());
    }
}

// TS: packages/engine/test/brew-shared-row.test.ts:60
#[test]
fn a_failed_shared_install_names_the_formula_it_left_behind_and_the_last_line_brew_printed_for_it() {
    let cmd = brew_shared_deps(&plan_formulae());
    let awk = awks().remove(0);
    let dir = box_dir(true, &awk);
    let (code, _stdout, stderr) = run_plan_script(&cmd, dir.path());
    assert_eq!(code, 1);
    assert_eq!(last_line(&stderr), "Error: llvm@21 did not install: BuildError: llvm@21 failed to pour");
}
