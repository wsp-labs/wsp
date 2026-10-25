// SPDX-License-Identifier: AGPL-3.0-only
//! The tools rung: Homebrew's taps and formulae, Nix home-manager, every global package manager's packages, and Go
//! binaries. Port of packages/collect/src/detect/tools.ts at a3b870a6e; the unit is apt_tools and its spec is
//! spec/apt_tools.md.
use std::sync::LazyLock;

use futures::future::join_all;
use indexmap::{IndexMap, IndexSet};
use regex::Regex;
use serde_json::Value;
use wsp_catalog::catalog::catalog_tool_for;
use wsp_catalog::mac_paths::MAC_BREW;
use wsp_catalog::road_modules::reads_row_road;
use wsp_wire::views::golden_image::tool_row_id;
use wsp_wire::wire::helpers::js_key_order;
use wsp_words::exit::HostError;
use wsp_words::js::{is_js_whitespace, js_trim, JS_WS};

use crate::brew_bottles::{linux_support, LINUX_BOTTLES};
use crate::detect::apt::apt_packages;
use crate::detect::common::{entry, exists, first_line, found, item, EntrySpec, RowOptions};
use crate::history::reader::try_json;
use crate::host::{expand, Host, Platform, RunOptions};
use crate::manifest::{Linux, ManifestEntry, RowDefault, Rung};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrewKind {
    Tap,
    Brew,
}

// tools.ts:10
// export interface BrewLine {
//   kind: "tap" | "brew";
//   name: string;
// }
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrewLine {
    pub kind: BrewKind,
    pub name: String,
}

// tools.ts:36
// export interface Pkg {
//   name: string;
//   version?: string;
//   /** The folder the package sits in, where the manager's listing names one. */
//   path?: string;
// }
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pkg {
    pub name: String,
    pub version: Option<String>,
    /// The folder the package sits in, where the manager's listing names one.
    pub path: Option<String>,
}

// tools.ts:124
// export interface GoModule {
//   path: string;
//   version: string;
// }
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoModule {
    pub path: String,
    pub version: String,
}

/// How npm and pnpm both list their globals (tools.ts:156).
pub const NPM_LS: [&str; 4] = ["ls", "-g", "--depth=0", "--json"];

/// Where bun keeps its global packages (tools.ts:176).
pub const BUN_GLOBALS: &str = "~/.bun/install/global/node_modules";

/// npm's own packages, never a row (tools.ts:55).
pub const NPM_OWN: [&str; 2] = ["npm", "corepack"];

/// Where another node once kept its globals; a laptop that moved to a new node still runs them from PATH (tools.ts:158).
const NPM_PREFIXES: [&str; 2] = [MAC_BREW, "/usr/local"];

static BREW_LINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(&format!(r#"^(tap|brew)[{JS_WS}]+"([^"]+)""#)).expect("BREW_LINE compiles"));

static NAME_VERSION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&format!(r"^([^{JS_WS}]+)[{JS_WS}]+v([^{JS_WS}]+?)(?::|[{JS_WS}]|$)")).expect("NAME_VERSION compiles"));

static BUN_LINE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(r"^[\x{{2502}}\x{{251c}}\x{{2514}}\x{{2500}}{JS_WS}]+(@?[^@{JS_WS}]+)@([^{JS_WS}]+)$")).expect("BUN_LINE compiles")
});

/// A command's argument list, as the host's `run` takes it.
pub fn argv(args: &[&str]) -> Vec<String> {
    args.iter().map(|a| a.to_string()).collect()
}

/// The Brewfile lines that name a tap or a formula; a cask or a Mac App Store app has no Linux build to bring.
pub fn parse_brewfile(text: &str) -> Vec<BrewLine> {
    text.split('\n')
        .filter_map(|line| {
            let m = BREW_LINE.captures(js_trim(line))?;
            let kind = if &m[1] == "tap" { BrewKind::Tap } else { BrewKind::Brew };
            Some(BrewLine { kind, name: m[2].to_string() })
        })
        .collect()
}

/// The taps and formulae this computer's Homebrew has, as `brew bundle dump` writes them. A plain dump also runs a
/// lister for casks, App Store apps, editor extensions and every other manager's packages, none of which a reader here
/// keeps; and `bundle` is a command Homebrew runs `brew update` before once its last one is a day old. Measured on a
/// Mac with 103 formulae: 3.3 s plain, 0.7 s like this.
pub async fn brewfile_here(host: &Host) -> Vec<BrewLine> {
    let opts = RunOptions { env: Some(IndexMap::from([("HOMEBREW_NO_AUTO_UPDATE".to_string(), "1".to_string())])), ..RunOptions::default() };
    let out = host.exec.run("brew", &argv(&["bundle", "dump", "--file=-", "--formula", "--taps", "--no-restart"]), opts).await;
    parse_brewfile(&out.unwrap_or_default())
}

/// The object entries of a parsed JSON object, in JavaScript's `Object.entries` order.
fn js_entries(map: &serde_json::Map<String, Value>) -> Vec<(String, Value)> {
    let mut entries: IndexMap<String, Value> = map.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
    js_key_order(&mut entries);
    entries.into_iter().collect()
}

pub fn parse_npm_globals(text: &str) -> Vec<Pkg> {
    let Some(data) = try_json(text) else { return Vec::new() };
    let Some(deps) = data.get("dependencies").and_then(Value::as_object) else { return Vec::new() };
    js_entries(deps)
        .into_iter()
        .filter(|(name, _)| !NPM_OWN.contains(&name.as_str()))
        .map(|(name, info)| Pkg { version: info.get("version").and_then(Value::as_str).map(str::to_string), name, path: None })
        .collect()
}

pub fn parse_pipx_list(text: &str) -> Vec<Pkg> {
    let Some(data) = try_json(text) else { return Vec::new() };
    let Some(venvs) = data.get("venvs").and_then(Value::as_object) else { return Vec::new() };
    js_entries(venvs)
        .into_iter()
        .map(|(name, venv)| {
            let version = venv
                .get("metadata")
                .and_then(|m| m.get("main_package"))
                .and_then(|m| m.get("package_version"))
                .and_then(Value::as_str)
                .map(str::to_string);
            Pkg { name, version, path: None }
        })
        .collect()
}

/// Top-level lines of `uv tool list` and `cargo install --list`: `name v1.2.3` with optional trailing decoration.
fn parse_name_version_lines(text: &str) -> Vec<Pkg> {
    let mut out = Vec::new();
    for line in text.split('\n') {
        if line.is_empty() || line.starts_with(|c: char| c == '-' || is_js_whitespace(c)) {
            continue;
        }
        let Some(m) = NAME_VERSION.captures(line) else { continue };
        out.push(Pkg { name: m[1].to_string(), version: Some(m[2].to_string()), path: None });
    }
    out
}

/// `uv tool list`'s top-level lines (TS `parseUvToolList = parseNameVersionLines`).
pub fn parse_uv_tool_list(text: &str) -> Vec<Pkg> {
    parse_name_version_lines(text)
}

/// `cargo install --list`'s top-level lines (TS `parseCargoInstalls = parseNameVersionLines`, tools.ts:95).
pub fn parse_cargo_installs(text: &str) -> Vec<Pkg> {
    parse_name_version_lines(text)
}

/// `pnpm ls -g --json`: one object per global dir, each with a dependencies map.
pub fn parse_pnpm_globals(text: &str) -> Vec<Pkg> {
    let Some(Value::Array(dirs)) = try_json(text) else { return Vec::new() };
    let mut out = Vec::new();
    for dir in &dirs {
        let Some(deps) = dir.get("dependencies").and_then(Value::as_object) else { continue };
        for (name, info) in js_entries(deps) {
            let version = info.get("version").and_then(Value::as_str).map(str::to_string);
            let path = info.get("path").and_then(Value::as_str).map(str::to_string);
            out.push(Pkg { name, version, path });
        }
    }
    out
}

/// `bun pm ls -g`: a header naming the global dir, then tree lines of `name@version`.
pub fn parse_bun_globals(text: &str) -> Vec<Pkg> {
    let mut out = Vec::new();
    for line in text.split('\n') {
        let Some(m) = BUN_LINE.captures(line) else { continue };
        out.push(Pkg { name: m[1].to_string(), version: Some(m[2].to_string()), path: None });
    }
    out
}

/// `go version -m <binary>`: the `path` line is the install target, the `mod` line carries the version.
pub fn parse_go_version_m(text: &str) -> Option<GoModule> {
    let mut path = None;
    let mut version = None;
    for line in text.split('\n') {
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.get(1) == Some(&"path") {
            if let Some(p) = cols.get(2) {
                path = Some(p.to_string());
            }
        }
        if cols.get(1) == Some(&"mod") {
            if let Some(v) = cols.get(3) {
                version = Some(v.to_string());
            }
        }
    }
    Some(GoModule { path: path?, version: version? })
}

/// A formula the laptop already runs on Linux needs no snapshot to vouch for it. A formula nothing vouches for starts
/// unticked without a reason, so the row stays open to a tick that tries it.
fn formula_row(host: &Host, name: &str) -> ManifestEntry {
    let linux = if matches!(host.platform, Platform::Linux) { Linux::Yes } else { linux_support(name, &LINUX_BOTTLES) };
    let id = tool_row_id("brew", name);
    let opt = RowOptions { group: Some("Homebrew".to_string()), linux: Some(linux), ..RowOptions::default() };
    match linux {
        Linux::No => item(Rung::Tools, &id, name, RowOptions { default: Some(RowDefault::Skip), reason: Some("no Linux bottle".to_string()), ..opt }),
        Linux::Unknown => item(Rung::Tools, &id, name, RowOptions { default: Some(RowDefault::Skip), ..opt }),
        Linux::Yes => item(Rung::Tools, &id, name, opt),
    }
}

async fn brew_rows(host: &Host) -> Vec<ManifestEntry> {
    if !host.exec.which("brew").await {
        return Vec::new();
    }
    let mut rows = Vec::new();
    for l in brewfile_here(host).await {
        rows.push(match l.kind {
            BrewKind::Tap => item(
                Rung::Tools,
                &format!("tools/brew-tap/{}", l.name),
                &l.name,
                RowOptions { group: Some("Homebrew taps".to_string()), linux: Some(Linux::Yes), ..RowOptions::default() },
            ),
            BrewKind::Brew => formula_row(host, &l.name),
        });
    }
    rows
}

async fn go_rows(host: &Host) -> Vec<ManifestEntry> {
    if !host.exec.which("go").await {
        return Vec::new();
    }
    let bin = format!("{}/go/bin", host.home);
    let mut rows = Vec::new();
    for name in host.fs.list(&bin).await {
        let id = tool_row_id("go", &name);
        let path = format!("{bin}/{name}");
        let out = host.exec.run("go", &argv(&["version", "-m", &path]), RunOptions::default()).await;
        let opt = RowOptions { group: Some("Go binaries".to_string()), linux: Some(Linux::Yes), ..RowOptions::default() };
        rows.push(match parse_go_version_m(&out.unwrap_or_default()) {
            None => item(
                Rung::Tools,
                &id,
                &format!("{name} (no module info)"),
                RowOptions { default: Some(RowDefault::Skip), ..opt },
            ),
            // The module path rides in paths so the detail pane shows it first; bytes 0 says there is nothing to upload.
            Some(m) => entry(EntrySpec {
                rung: Rung::Tools,
                id,
                label: name,
                paths: vec![format!("{}@{}", m.path, m.version)],
                bytes: 0,
                opt: RowOptions { version: Some(m.version), ..opt },
            }),
        });
    }
    rows
}

async fn nix_rows(host: &Host) -> Vec<ManifestEntry> {
    if !exists(host, "~/.config/home-manager").await && !exists(host, "~/.config/nixpkgs/home.nix").await {
        return Vec::new();
    }
    let paths = ["~/.config/home-manager".to_string(), "~/.config/nixpkgs/home.nix".to_string()];
    let f = found(host, &paths).await;
    vec![entry(EntrySpec {
        rung: Rung::Tools,
        id: "tools/nix-home-manager".to_string(),
        label: "Nix home-manager config".to_string(),
        paths: f.paths,
        bytes: f.bytes,
        opt: RowOptions { linux: Some(Linux::Yes), ..RowOptions::default() },
    })]
}

/// npm's own prefix, then every other prefix on this laptop that holds globals; a name in both is the own prefix's.
async fn npm_globals(host: &Host) -> Vec<Pkg> {
    let own = first_line(host.exec.run("npm", &argv(&["prefix", "-g"]), RunOptions::default()).await.as_deref());
    let mut out: IndexMap<String, Pkg> = IndexMap::new();
    for p in parse_npm_globals(&host.exec.run("npm", &argv(&NPM_LS), RunOptions::default()).await.unwrap_or_default()) {
        out.entry(p.name.clone()).or_insert(p);
    }
    for prefix in NPM_PREFIXES {
        if own.as_deref() == Some(prefix) || !exists(host, &format!("{prefix}/lib/node_modules")).await {
            continue;
        }
        let mut args = argv(&NPM_LS);
        args.extend(argv(&["--prefix", prefix]));
        for p in parse_npm_globals(&host.exec.run("npm", &args, RunOptions::default()).await.unwrap_or_default()) {
            out.entry(p.name.clone()).or_insert(p);
        }
    }
    out.into_values().collect()
}

/// How a global manager says what it has: one listing this reader parses, or a reader of its own where one listing is
/// not the whole story (npm's other prefixes, apt's rule over what a person chose).
enum Listing {
    Args(&'static [&'static str], fn(&str) -> Vec<Pkg>),
    Npm,
    Apt,
}

/// A language or system manager whose global packages are rows (tools.ts:147).
struct GlobalManager {
    id: &'static str,
    bin: &'static str,
    group: &'static str,
    /// Between a package and its version in the row's label; a manager whose packages carry no version needs none.
    sep: Option<&'static str>,
    listing: Listing,
}

const GLOBALS: [GlobalManager; 7] = [
    GlobalManager { id: "npm", bin: "npm", group: "npm globals", sep: Some("@"), listing: Listing::Npm },
    GlobalManager { id: "pnpm", bin: "pnpm", group: "pnpm globals", sep: Some("@"), listing: Listing::Args(&NPM_LS, parse_pnpm_globals) },
    GlobalManager { id: "bun", bin: "bun", group: "bun globals", sep: Some("@"), listing: Listing::Args(&["pm", "ls", "-g"], parse_bun_globals) },
    GlobalManager { id: "pipx", bin: "pipx", group: "pipx", sep: Some(" "), listing: Listing::Args(&["list", "--json"], parse_pipx_list) },
    GlobalManager { id: "uv", bin: "uv", group: "uv tools", sep: Some(" "), listing: Listing::Args(&["tool", "list"], parse_uv_tool_list) },
    GlobalManager { id: "cargo", bin: "cargo", group: "cargo installs", sep: Some(" "), listing: Listing::Args(&["install", "--list"], parse_cargo_installs) },
    // apt-mark is what says which packages a person asked for; dpkg alone cannot tell those from the base image's own.
    GlobalManager { id: "apt", bin: "apt-mark", group: "apt packages", sep: None, listing: Listing::Apt },
];

fn versioned(p: &Pkg, sep: Option<&str>) -> String {
    match (&p.version, sep) {
        (Some(v), Some(s)) => format!("{}{s}{v}", p.name),
        _ => p.name.clone(),
    }
}

/// Each row installs on the image: a language manager fetches that package's own Linux build, and apt is the image's
/// own package manager. A manager whose rows the build reads no road off (apt) files none for a tool the catalog
/// carries: that row would stand in for the catalog's, whose own road is what installs it there.
async fn global_rows(host: &Host, g: &GlobalManager) -> Result<Vec<ManifestEntry>, HostError> {
    if !host.exec.which(g.bin).await {
        return Ok(Vec::new());
    }
    let all = match g.listing {
        Listing::Args(args, parse) => parse(&host.exec.run(g.bin, &argv(args), RunOptions::default()).await.unwrap_or_default()),
        Listing::Npm => npm_globals(host).await,
        Listing::Apt => apt_packages(host).await?,
    };
    let pkgs: Vec<Pkg> = if reads_row_road(g.id) { all } else { all.into_iter().filter(|p| catalog_tool_for(&p.name).is_none()).collect() };
    Ok(pkgs
        .into_iter()
        .map(|p| {
            let opt = RowOptions { group: Some(g.group.to_string()), linux: Some(Linux::Yes), version: p.version.clone(), ..RowOptions::default() };
            item(Rung::Tools, &tool_row_id(g.id, &p.name), &versioned(&p, g.sep), opt)
        })
        .collect())
}

/// The returned function gives a package's candidate folders: each root's `<root>/<name>`, then pnpm's listed paths.
pub async fn js_global_folders(host: &Host) -> Box<dyn Fn(&str) -> Vec<String>> {
    let npm_args = argv(&["root", "-g"]);
    let pnpm_args = argv(&NPM_LS);
    let (npm, pnpm) = futures::join!(
        host.exec.run("npm", &npm_args, RunOptions::default()),
        host.exec.run("pnpm", &pnpm_args, RunOptions::default()),
    );
    let mut roots: Vec<String> = Vec::new();
    let candidates = first_line(npm.as_deref())
        .into_iter()
        .chain(NPM_PREFIXES.iter().map(|p| format!("{p}/lib/node_modules")))
        .chain(std::iter::once(expand(&host.home, BUN_GLOBALS)));
    for root in candidates {
        if !roots.contains(&root) {
            roots.push(root);
        }
    }
    let mut listed: IndexMap<String, Vec<String>> = IndexMap::new();
    for p in parse_pnpm_globals(&pnpm.unwrap_or_default()) {
        if let Some(path) = p.path {
            listed.entry(p.name).or_default().push(path);
        }
    }
    Box::new(move |name: &str| {
        let mut out: Vec<String> = roots.iter().map(|root| format!("{root}/{name}")).collect();
        out.extend(listed.get(name).cloned().unwrap_or_default());
        out
    })
}

/// The commands a global package puts on PATH, off its package.json in any of its folders: each key of an object
/// `bin`, or for a single one the name past its scope, as npm links it.
pub async fn package_commands(host: &Host, name: &str, folders: &[String]) -> Vec<String> {
    let texts = join_all(folders.iter().map(|folder| async move { host.fs.read_text(&format!("{folder}/package.json")).await })).await;
    let mut out: IndexSet<String> = IndexSet::new();
    for text in texts {
        let Some(data) = try_json(text.as_deref().unwrap_or("")) else { continue };
        match data.get("bin") {
            Some(Value::String(_)) => {
                out.insert(name.rsplit('/').next().unwrap_or(name).to_string());
            }
            Some(Value::Object(bin)) => {
                for (key, _) in js_entries(bin) {
                    out.insert(key);
                }
            }
            _ => {}
        }
    }
    out.into_iter().collect()
}

/// Every manager is asked at once; the rows keep brew, nix, the GLOBALS in table order, then go.
/// apt's list read failing part way fails the whole read, as the TS `Promise.all` rejects (coordinator's ruling).
pub async fn detect_tools(host: &Host) -> Result<Vec<ManifestEntry>, HostError> {
    let globals = join_all(GLOBALS.iter().map(|g| global_rows(host, g)));
    let (brew, nix, globals, go) = futures::join!(brew_rows(host), nix_rows(host), globals, go_rows(host));
    let mut rows = brew;
    rows.extend(nix);
    for g in globals {
        rows.extend(g?);
    }
    rows.extend(go);
    Ok(rows)
}
