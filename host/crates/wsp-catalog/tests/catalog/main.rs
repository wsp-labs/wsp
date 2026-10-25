// SPDX-License-Identifier: AGPL-3.0-only
// Ports packages/catalog/test/catalog.test.ts.
use std::cell::RefCell;
use std::collections::HashMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::OnceLock;

use futures::executor::block_on;
use indexmap::IndexMap;
use regex::Regex;
use serde_json::{json, Value};
use wsp_catalog::catalog::{
    agent_name, base_entry_for, base_floor, base_note, catalog, catalog_agents, catalog_entry, catalog_id_of_row, catalog_tool_for,
    catalog_tool_for_dependency, default_agent, guest_env, install_after, install_line, install_shown, login_row, login_rows, size_bytes,
    size_method_words, smoke_of, AgentEntry, Brings, CatalogEntry, EntryKind, Evidence, InstallShown, LatestSource, RoadEvidence, Size,
    SizeMethod, ToolEntry, UsageLog, HISTORY_FORMATS,
};
use wsp_catalog::catalog_data::agent_modules;
use wsp_catalog::git_hosts::{runs_threads, THREAD_AGENTS};
use wsp_catalog::linux_casks::{linux_casks, CURSOR, GCLOUD, KUBECTL};
use wsp_catalog::release_pins::RELEASE_PINS;
use wsp_catalog::road_modules::{
    fixes_version, install_homes, reads_row_road, road_module, road_module_named, road_step, RoadNote, ToolRow, APT_INDEX, APT_UPDATE,
    BREW_ENV, BREW_PREFIX, CURL_NET, HOMEBREW_STEP, NET_READ_S, NET_RETRIES,
};
use wsp_catalog::roads::{
    node_install_script, node_release, pin_check_line, standing_pin, unpinned, version_of, InstallRoad, PackageRoad, ReleaseAsset,
    ReleaseAssets, RoadName, CLAUDE_CODE, CLAUDE_CONFIG_DIR, CLAUDE_INSTALL, DOCKER_INSTALL, GOLDEN_SETUP, GUEST_HOME, LOCAL_BIN,
    PLAYWRIGHT_VERSION, ROADS, UV_INSTALL, YARN_VERSION,
};
use wsp_catalog::signin::{
    has_login, keys_id_of, keys_row_of, login_id_of, mints_token, no_sign_in, KeyFiles, LoginSource, NoSignIn, SignIn, SignInKind,
};
use wsp_catalog::signin_rows::SIGN_IN_ROWS;
use wsp_catalog::skills::{PluginRoad, SeedReader};
use wsp_wire::types::agents_report::AgentSignInKind;
use wsp_wire::types::daemon_contract::HOMEBREW_HOME;
use wsp_wire::types::daemon_contract::HOMEBREW_PREFIX;
use wsp_wire::types::daemon_contract::PNPM_HOME;
use wsp_wire::types::daemon_contract::TOOL_PREFIX;
use wsp_wire::views::golden_image::ToolPin;
use wsp_wire::views::golden_image::ToolPinArch as Arch;
use wsp_words::words::computer::Platform;
use wsp_words::words::image::pin_mismatch_line;

const WORKSPACE_OVERLAID: [&str; 5] = ["/usr", "/etc", "/opt", "/var", "/srv"];
const SHARED_TOOL_ROOTS: [&str; 1] = [HOMEBREW_HOME];
const AGENT_STORE_ENVS: [&str; 2] = ["CLAUDE_CONFIG_DIR", "CODEX_HOME"];
const AGENT_BINS: [&str; 11] = ["amp", "claude", "codex", "crush", "cursor-agent", "gemini", "goose", "hermes", "opencode", "pi", "qwen"];

fn tools_path() -> String {
    format!(
        "/root/.local/bin:/usr/local/sbin:/usr/local/bin:{HOMEBREW_PREFIX}/bin:{HOMEBREW_PREFIX}/sbin:/root/go/bin:/root/.cargo/bin:{PNPM_HOME}:/root/.bun/bin:/usr/sbin:/usr/bin:/sbin:/bin"
    )
}

const STUB_RUNNER_SH: &str = r##"#!/bin/sh
body="${0%/*}/.${0##*/}.stub"
[ -f "$body" ] || { echo "stub-runner: no script at $body" >&2; exit 127; }
IFS= read -r first < "$body"
case $first in "#!"*) exec ${first#??} "$body" "$@" ;; esac
exec /bin/sh "$body" "$@"
"##;

const SHA256SUM_STUB: &str = r#"#!/bin/sh
hash() {
  if [ -x /usr/bin/sha256sum ]; then /usr/bin/sha256sum "$1"; else /usr/bin/shasum -a 256 "$1"; fi | cut -d' ' -f1
}
if [ "$1" = -c ]; then
  bad=0
  while IFS= read -r line; do
    [ -n "$line" ] || continue
    want="${line%%  *}"
    f="${line#*  }"
    if [ "$(hash "$f")" = "$want" ]; then echo "$f: OK"; else echo "$f: FAILED"; bad=$((bad + 1)); fi
  done
  [ "$bad" -eq 0 ] || { echo "sha256sum: WARNING: $bad computed checksum did NOT match" >&2; exit 1; }
  exit 0
fi
echo "$(hash "$1")  $1"
"#;

fn stub_runner() -> &'static Path {
    static RUNNER: OnceLock<PathBuf> = OnceLock::new();
    RUNNER.get_or_init(|| {
        let dir = std::env::temp_dir().join(format!("wsp-catalog-stub-runner-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("stub-runner.sh");
        fs::write(&path, STUB_RUNNER_SH).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o555)).unwrap();
        loop {
            match Command::new(&path).output() {
                Err(e) if e.raw_os_error() == Some(26) => std::thread::sleep(std::time::Duration::from_millis(20)),
                _ => break,
            }
        }
        path
    })
}

fn write_stub(path: &Path, script: &str) {
    let name = path.file_name().unwrap().to_str().unwrap();
    fs::write(path.parent().unwrap().join(format!(".{name}.stub")), script).unwrap();
    let _ = fs::remove_file(path);
    std::os::unix::fs::symlink(stub_runner(), path).unwrap();
}

fn sha256_hex(path: &Path) -> String {
    let out = if Path::new("/usr/bin/sha256sum").exists() {
        Command::new("/usr/bin/sha256sum").arg(path).output().unwrap()
    } else {
        Command::new("/usr/bin/shasum").args(["-a", "256"]).arg(path).output().unwrap()
    };
    String::from_utf8(out.stdout).unwrap().split_whitespace().next().unwrap().to_string()
}

fn stdout_of(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn stderr_of(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

fn run_install(dir: &Path, script: &str) -> Output {
    let tools = dir.join("tools");
    fs::create_dir(&tools).unwrap();
    write_stub(&tools.join("sha256sum"), SHA256SUM_STUB);
    Command::new("bash")
        .args(["--noprofile", "--norc", "-c", script])
        .env_clear()
        .env("PATH", format!("{}:/usr/bin:/bin", tools.display()))
        .env("HOME", dir)
        .output()
        .unwrap()
}

fn re(pattern: &str) -> Regex {
    Regex::new(pattern).unwrap()
}

fn entry(id: &str) -> &'static CatalogEntry {
    catalog_entry(id).unwrap_or_else(|| panic!("no catalog entry {id}"))
}

fn agent(id: &str) -> &'static AgentEntry {
    entry(id).as_agent().unwrap_or_else(|| panic!("{id} is no agent"))
}

fn tool(id: &str) -> &'static ToolEntry {
    entry(id).as_tool().unwrap_or_else(|| panic!("{id} is no tool"))
}

fn line_of(e: &CatalogEntry) -> String {
    install_line(e).unwrap()
}

fn source_of(e: &CatalogEntry) -> &Evidence {
    match e {
        CatalogEntry::Agent(a) => &a.source,
        CatalogEntry::Tool(t) => &t.source,
    }
}

fn tool_id(pkg: &str) -> Option<&'static str> {
    catalog_tool_for(pkg).map(|t| t.id.as_str())
}

fn base_id(pkg: &str) -> Option<&'static str> {
    base_entry_for(pkg).map(|t| t.id.as_str())
}

fn s(text: &str) -> Option<String> {
    Some(text.to_string())
}

fn pin_of(tag: &str, sha256: &str) -> ToolPin {
    ToolPin { tag: tag.to_string(), sha256: Some(sha256.to_string()), arch: None, latest: None }
}

fn tag_pin(tag: &str) -> ToolPin {
    ToolPin { tag: tag.to_string(), sha256: None, arch: None, latest: None }
}

fn package_road(package: &str, version: Option<&str>, pin: Option<ToolPin>) -> PackageRoad {
    PackageRoad { package: package.to_string(), version: version.map(str::to_string), pin }
}

fn npm_road(package: &str, version: Option<&str>, pin: Option<ToolPin>) -> InstallRoad {
    InstallRoad::Npm { package: package_road(package, version, pin), ignore_scripts: None }
}

fn release_road(repo: &str, version: Option<&str>, pin: Option<ToolPin>) -> InstallRoad {
    InstallRoad::Release { repo: Some(repo.to_string()), version: version.map(str::to_string), assets: None, pin, go: None }
}

fn go_road(module: Option<&str>, version: Option<&str>, pin: Option<ToolPin>) -> InstallRoad {
    InstallRoad::Go { module: module.map(str::to_string), version: version.map(str::to_string), pin }
}

fn script_road(script: &str, version: Option<&str>) -> InstallRoad {
    InstallRoad::Script { script: script.to_string(), version: version.map(str::to_string), pin: None, bins: None }
}

fn sample_road(name: RoadName) -> InstallRoad {
    match name {
        RoadName::Brew => InstallRoad::Brew { formula: "go".to_string(), pin: None },
        RoadName::Npm => npm_road("wrangler", None, None),
        RoadName::Pnpm => InstallRoad::Pnpm(package_road("wrangler", None, None)),
        RoadName::Bun => InstallRoad::Bun(package_road("wrangler", None, None)),
        RoadName::Uv => InstallRoad::Uv(package_road("ruff", None, None)),
        RoadName::Pipx => InstallRoad::Pipx(package_road("black", None, None)),
        RoadName::Cargo => InstallRoad::Cargo(package_road("bat", None, None)),
        RoadName::Go => go_road(Some("golang.org/x/tools/gopls"), None, None),
        RoadName::Release => release_road("cli/cli", Some("v2.86.0"), None),
        RoadName::Vendor => InstallRoad::Vendor { cask: GCLOUD.clone(), version: None, pin: None },
        RoadName::Apt => InstallRoad::Apt { packages: vec!["tmux".to_string()], pin: None },
        RoadName::Script => script_road("x", None),
    }
}

fn install(road: &InstallRoad, bin: &str) -> Result<String, RoadNote> {
    road_module(road).install(road, bin, None)
}

fn uninstall(road: &InstallRoad, bin: &str) -> Result<String, RoadNote> {
    road_module(road).uninstall(road, bin, None)
}

fn ok(text: &str) -> Result<String, RoadNote> {
    Ok(text.to_string())
}

fn note(text: &str) -> Result<String, RoadNote> {
    Err(RoadNote(text.to_string()))
}

fn tool_row(name: &str, version: Option<&str>) -> ToolRow {
    ToolRow { name: name.to_string(), version: version.map(str::to_string), paths: vec![], label: name.to_string() }
}

fn sentences(text: &str) -> Vec<String> {
    let mut out = vec![];
    let mut current = String::new();
    for word in text.split(' ') {
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(word);
        if word.ends_with('.') {
            out.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

struct SeedFiles {
    files: HashMap<String, String>,
    reads: RefCell<Vec<String>>,
}

impl SeedReader for SeedFiles {
    async fn read(&self, rel: &str) -> Option<String> {
        self.reads.borrow_mut().push(rel.to_string());
        self.files.get(rel).cloned()
    }
}

struct SeedText(&'static str);

impl SeedReader for SeedText {
    async fn read(&self, _rel: &str) -> Option<String> {
        Some(self.0.to_string())
    }
}

mod part_1;
mod part_2;
