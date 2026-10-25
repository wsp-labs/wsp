// SPDX-License-Identifier: AGPL-3.0-only
//! The pure half of the MCP reader: whether a server definition runs on Linux, what mcp-remote it runs and the store
//! key it keeps a sign-in under, and the one reading of a definition's arguments that both its detail line and its
//! count of secrets come from. Port of packages/collect/src/detect/mcp.ts:1-290 at a3b870a6e; the spec is
//! spec/mcp_args.md. `detect::mcp` re-exports every item here, as the TS module exports them from one file.
use std::sync::LazyLock;

use fancy_regex::Regex;
use indexmap::IndexMap;
use md5::{Digest, Md5};
use wsp_catalog::mac_paths::MAC_ONLY;
use wsp_catalog::mcp::{McpServer, McpTransport};
use wsp_words::js::js_trim;

use crate::detect::shell_rc::{captures, group, is_match, is_secret_name, js_re};
use crate::host::tilde;

/// Directories whose binaries the machine finds on its own PATH by the same name: the catalog's MAC_BIN_DIRS, then
/// these three (mcp.ts:29).
pub const MCP_BIN_DIRS: [&str; 5] = ["/opt/homebrew/bin/", "/opt/homebrew/sbin/", "/usr/local/bin/", "/usr/bin/", "/bin/"];

/// A path segment long and token-like enough to be a key, as a JavaScript regex source (mcp.ts:169).
pub const KEYLIKE_SEGMENT: &str = r"^(?=.*\d)[A-Za-z0-9_=-]{24,}$";

/// Flags whose next argument is a secret whatever its name says (mcp.ts:127).
pub const HIDDEN_FLAGS: [&str; 3] = ["--header", "-H", "--static-oauth-client-info"];

/// The runners a definition names its own program after (mcp.ts:148).
pub const RUNNERS: [&str; 3] = ["npx", "uvx", "uv"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinuxFit {
    Ok { needs: String },
    No { reason: String },
}

/// What `mcp_remote_of` reads off a definition that runs mcp-remote (mcp.ts:82).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpRemote {
    pub url: String,
    /// `--header` and `-H` pairs, later ones of a name replacing the value in the first one's place.
    pub headers: IndexMap<String, String>,
    pub hash: String,
    pub client_credentials: bool,
    pub v1: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArgKind {
    Flag,
    Assign,
    Positional,
}

/// What one argument of a stdio definition comes to: the words the detail line shows for it, and how the count of
/// what the row carries names it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ArgRead {
    /// Nothing where the line drops the argument.
    pub show: Option<String>,
    /// Nothing where the argument carries no value of the person's.
    pub secret: Option<String>,
    /// What the value goes by where it is one: its variable, its flag or its place among the arguments.
    pub key: Option<String>,
    pub kind: Option<ArgKind>,
}

static ADDRESS_RE: LazyLock<Regex> = LazyLock::new(|| js_re(r"^[A-Za-z][A-Za-z0-9+.-]*://"));
static KEYLIKE_RE: LazyLock<Regex> = LazyLock::new(|| js_re(KEYLIKE_SEGMENT));
static MCP_REMOTE_RE: LazyLock<Regex> = LazyLock::new(|| js_re(r"^mcp-remote(@[^/]*)?$"));
static SEMVER_RE: LazyLock<Regex> = LazyLock::new(|| js_re(r"^(\d+)\.(\d+)\.(\d+)$"));
static FLAG_PATH_RE: LazyLock<Regex> = LazyLock::new(|| js_re(r"^--?[\w-]+="));
static FLAG_NAME_RE: LazyLock<Regex> = LazyLock::new(|| js_re(r"^(--?[A-Za-z][\w-]*)(=|$)"));
static ASSIGN_NAME_RE: LazyLock<Regex> = LazyLock::new(|| js_re(r"^([A-Za-z_]\w*)="));

/// `isMacOnly`: a path under `~/Library` or a macOS install location, with `~/` taken as home.
fn is_mac_only(p: &str, home: &str) -> bool {
    let abs = match p.strip_prefix("~/") {
        Some(rest) => format!("{home}/{rest}"),
        None => p.to_string(),
    };
    abs.starts_with(&format!("{home}/Library/")) || MAC_ONLY.iter().any(|prefix| abs.starts_with(prefix))
}

/// `--flag=/path` carries its path after the equals sign.
fn path_of(s: &str) -> String {
    match s.split_once('=') {
        Some((_, rest)) if is_match(&FLAG_PATH_RE, s) => rest.to_string(),
        _ => s.to_string(),
    }
}

/// The binary a command names: a path under a known bin directory or under ~/.local/bin is found by name on the machine.
fn binary_of(command: &str, home: &str) -> String {
    let abs = match command.strip_prefix("~/") {
        Some(rest) => format!("{home}/{rest}"),
        None => command.to_string(),
    };
    if !abs.starts_with('/') {
        return abs;
    }
    if MCP_BIN_DIRS.iter().any(|d| abs.starts_with(d)) || abs.starts_with(&format!("{home}/.local/bin/")) {
        return abs[abs.rfind('/').map_or(0, |p| p + 1)..].to_string();
    }
    tilde(home, &abs)
}

/// Whether a definition can run on the machine and what it needs there. A path under ~/Library or a macOS
/// install location has no Linux equivalent; home paths and Homebrew's prefix are rewritten on the machine.
pub fn linux_fit(server: &McpServer, home: &str) -> LinuxFit {
    match &server.transport {
        McpTransport::Http { .. } => LinuxFit::Ok { needs: "nothing to install".to_string() },
        McpTransport::Stdio { command, args, env, cwd, .. } => {
            if is_mac_only(command, home) {
                return LinuxFit::No { reason: format!("command {} is macOS-only, will not run", tilde(home, command)) };
            }
            let paths: Vec<String> = args.iter().map(|a| path_of(a)).chain(cwd.iter().cloned()).chain(env.values().cloned()).collect();
            for s in &paths {
                if is_mac_only(s, home) {
                    return LinuxFit::No { reason: format!("path {} is macOS-only, will not run", tilde(home, s)) };
                }
            }
            let bin = binary_of(command, home);
            match bin.as_str() {
                "npx" => LinuxFit::Ok { needs: "runs via npx".to_string() },
                "uvx" | "uv" => LinuxFit::Ok { needs: "needs uv, installed on the machine when missing".to_string() },
                _ => LinuxFit::Ok { needs: format!("needs {bin} on the machine") },
            }
        }
    }
}

fn is_url(s: &str) -> bool {
    s.starts_with("http://") || s.starts_with("https://")
}

/// `JSON.stringify` of one string: quotes, backslashes and control characters escaped as JavaScript escapes them.
fn json_string(s: &str) -> String {
    serde_json::to_string(s).expect("a string serializes")
}

/// `[JSON.stringify(o, sortedKeys)]` for a map with keys, its keys in UTF-16 order; nothing for an empty map.
fn sorted_json(o: &IndexMap<String, String>) -> Vec<String> {
    if o.is_empty() {
        return Vec::new();
    }
    let mut keys: Vec<&String> = o.keys().collect();
    keys.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
    let entries: Vec<String> = keys.iter().map(|k| format!("{}:{}", json_string(k), json_string(&o[k.as_str()]))).collect();
    vec![format!("{{{}}}", entries.join(","))]
}

/// Whether the mcp-remote a definition names keeps its sign-ins under mcp-remote-v1: unpinned, latest, and every version
/// from 0.3.2 on. An earlier one names that folder by its own version, and a range or another tag may land either side.
fn keeps_v1(spec: &str) -> bool {
    match spec.split('@').nth(1) {
        None | Some("latest") => true,
        Some(pin) => {
            let Some(caps) = captures(&SEMVER_RE, pin) else { return false };
            let v: Vec<f64> = (1..=3).map(|k| group(&caps, k).parse::<f64>().unwrap_or(f64::NAN)).collect();
            v[0] > 0.0 || v[1] > 3.0 || (v[1] == 3.0 && v[2] >= 2.0)
        }
    }
}

/// What a definition that runs mcp-remote hands it: the server's url, its `--header` pairs, the store key mcp-remote
/// derives as its getServerUrlHash does (md5 of the url, the `--resource`, the sorted `--authorize-param` pairs, the
/// sorted `--header` pairs and the `--client-metadata-url`, joined with `|`), whether `--client-credentials` has it
/// get its own token, where no person signs in, and whether its version keeps sign-ins under mcp-remote-v1. Undefined
/// when the definition does not run mcp-remote.
pub fn mcp_remote_of(args: &[String]) -> Option<McpRemote> {
    let at = args.iter().position(|a| is_match(&MCP_REMOTE_RE, a))?;
    let rest = &args[at + 1..];
    let url = rest.iter().find(|a| is_url(a))?.clone();
    let mut headers: IndexMap<String, String> = IndexMap::new();
    let mut params: IndexMap<String, String> = IndexMap::new();
    for (i, flag) in rest.iter().enumerate() {
        let Some(value) = rest.get(i + 1) else { continue };
        if flag == "--header" || flag == "-H" {
            if let Some(colon) = value.find(':').filter(|&c| c > 0) {
                let key = js_trim(&value[..colon]);
                if key != "__proto__" {
                    headers.insert(key.to_string(), js_trim(&value[colon + 1..]).to_string());
                }
            }
        } else if flag == "--authorize-param" {
            if let Some(eq) = value.find('=').filter(|&e| e > 0) {
                let key = js_trim(&value[..eq]);
                if key != "__proto__" {
                    params.insert(key.to_string(), js_trim(&value[eq + 1..]).to_string());
                }
            }
        }
    }
    let after = |flag: &str| rest.iter().position(|a| a == flag).and_then(|i| rest.get(i + 1)).map(|v| js_trim(v).to_string());
    let mut parts = vec![url.clone()];
    parts.extend(after("--resource"));
    parts.extend(sorted_json(&params));
    parts.extend(sorted_json(&headers));
    parts.extend(after("--client-metadata-url"));
    let hash = format!("{:x}", Md5::digest(parts.join("|").as_bytes()));
    Some(McpRemote { url, headers, hash, client_credentials: rest.iter().any(|a| a == "--client-credentials"), v1: keeps_v1(&args[at]) })
}

/// The store key mcp-remote derives for a definition that runs it; undefined when the definition does not run it.
pub fn mcp_remote_hash(args: &[String]) -> Option<String> {
    mcp_remote_of(args).map(|r| r.hash)
}

/// isSecretName's words plus the ones a file-valued variable tends to carry.
pub fn secret_named(name: &str) -> bool {
    is_secret_name(name) || name.to_uppercase().split('_').any(|w| matches!(w, "CREDENTIAL" | "CREDENTIALS" | "AUTH" | "CERT"))
}

/// A flag whose value is never shown: one that hides it whatever its name says, or a secret-shaped name.
pub fn hides_value(flag: &str) -> bool {
    HIDDEN_FLAGS.contains(&flag) || secret_named(&flag.replace('-', "_"))
}

/// `shownUrl`: the host and path of a url as WHATWG `URL` gives them, the url itself when it does not parse.
pub(crate) fn shown_url(url: &str) -> String {
    let Ok(u) = url::Url::parse(url) else { return url.to_string() };
    let host = u.host_str().unwrap_or("");
    let host = match u.port() {
        Some(port) => format!("{host}:{port}"),
        None => host.to_string(),
    };
    if u.path() == "/" {
        host
    } else {
        format!("{host}{}", u.path())
    }
}

/// `(<utf-8 bytes> B)`.
fn size(v: &str) -> String {
    format!("({} B)", v.len())
}

/// U+180E in a host: the WHATWG `URL` of V8 refuses it, where the `url` crate's older IDNA table maps it away.
fn host_holds_mongolian_vowel_separator(s: &str) -> bool {
    let Some((_, rest)) = s.split_once("//") else { return false };
    rest.split(['/', '?', '#', '\\']).next().is_some_and(|authority| authority.contains('\u{180e}'))
}

/// An address that carries nothing of the person's: no login, no query and no path segment shaped like a key. One that
/// carries any of them is a value, since asking costs a question and not asking can carry a key.
fn plain_address(s: &str) -> bool {
    if host_holds_mongolian_vowel_separator(s) {
        return false;
    }
    let Ok(u) = url::Url::parse(s) else { return false };
    u.username().is_empty()
        && u.password().is_none_or(str::is_empty)
        && u.query().is_none_or(str::is_empty)
        && !u.path().split('/').any(|seg| is_match(&KEYLIKE_RE, seg))
}

/// An `API_KEY=value` argument wherever it stands, a bare one or a flag's value: under a secret's name, or holding an
/// address that is not plain, the name is shown and the value hidden and counted, so a variable set behind `-e` reads
/// as the variable it is; any other is shown whole. Nothing for any other argument.
fn assign_read(arg: &str) -> Option<ArgRead> {
    let name = captures(&ASSIGN_NAME_RE, arg).map(|caps| group(&caps, 1))?;
    let value = arg.split_once('=').map_or("", |(_, v)| v);
    if secret_named(&name) || (is_match(&ADDRESS_RE, value) && !plain_address(value)) {
        Some(ArgRead {
            show: Some(format!("{name}=…")),
            secret: Some(format!("arg {name} {}", size(value))),
            key: Some(name),
            kind: Some(ArgKind::Assign),
        })
    } else {
        Some(ArgRead { show: Some(arg.to_string()), ..ArgRead::default() })
    }
}

/// A positional argument that is the definition's own word and no value of the person's: a path, or a plain address.
pub fn plain_arg(arg: &str) -> bool {
    arg.starts_with('/') || arg.starts_with("~/") || (is_match(&ADDRESS_RE, arg) && plain_address(arg))
}

/// The one reading of a definition's arguments, for the line a person reads and for what the row carries alike.
/// npx's own yes switch is dropped and takes no value. A dashed flag's next argument is that flag's value, read
/// like any other argument and shown unless the flag is secret-named or hides its value whatever its name says.
/// `--` ends the flags. A path or a plain address past the program is shown; every other argument there is a value
/// this definition does not name, so it is hidden and counted, and the row takes the copy answer.
/// One read per argument the loop visits, in order: a flag's value read with its flag adds a second entry.
pub fn read_args(command: &str, args: &[String], home: &str) -> Vec<ArgRead> {
    let shown = |a: &str| if is_url(a) { format!("({})", shown_url(a)) } else { tilde(home, a) };
    let mut out: Vec<ArgRead> = Vec::new();
    let mut program = RUNNERS.contains(&binary_of(command, home).as_str());
    let mut flags = true;
    let mut i = 0;
    while i < args.len() {
        let a = args[i].as_str();
        i += 1;
        if flags && (a == "-y" || a == "--yes") {
            out.push(ArgRead::default());
            continue;
        }
        if flags && a == "--" {
            flags = false;
            out.push(ArgRead { show: Some(a.to_string()), ..ArgRead::default() });
            continue;
        }
        if flags && a.starts_with('-') {
            let flag = captures(&FLAG_NAME_RE, a).map(|caps| group(&caps, 1));
            let hides = flag.as_deref().is_some_and(hides_value);
            if let Some((head, value)) = a.split_once('=') {
                if let (true, Some(flag)) = (hides, flag.as_ref()) {
                    out.push(ArgRead {
                        show: Some(format!("{flag}=…")),
                        secret: Some(format!("flag {flag} {}", size(value))),
                        key: Some(flag.clone()),
                        kind: Some(ArgKind::Flag),
                    });
                } else {
                    match assign_read(value) {
                        Some(assigned) => {
                            out.push(ArgRead { show: Some(format!("{head}={}", assigned.show.clone().unwrap_or_default())), ..assigned })
                        }
                        None => out.push(ArgRead { show: Some(shown(a)), ..ArgRead::default() }),
                    }
                }
                continue;
            }
            out.push(ArgRead { show: Some(a.to_string()), ..ArgRead::default() });
            if let Some(value) = args.get(i).filter(|v| !v.starts_with('-')) {
                i += 1;
                if hides {
                    out.push(ArgRead {
                        show: Some("…".to_string()),
                        secret: Some(format!("flag {a} {}", size(value))),
                        key: Some(a.to_string()),
                        kind: Some(ArgKind::Flag),
                    });
                } else {
                    out.push(assign_read(value).unwrap_or_else(|| ArgRead { show: Some(shown(value)), ..ArgRead::default() }));
                }
            }
            continue;
        }
        if let Some(assigned) = assign_read(a) {
            out.push(assigned);
            continue;
        }
        if program || plain_arg(a) {
            program = false;
            out.push(ArgRead { show: Some(shown(a)), ..ArgRead::default() });
            continue;
        }
        out.push(ArgRead {
            show: Some("…".to_string()),
            secret: Some(format!("arg {i} {}", size(a))),
            key: Some(format!("argument {i}")),
            kind: Some(ArgKind::Positional),
        });
    }
    out
}

/// A stdio definition's command and its first arguments as a person reads them, every value of theirs hidden.
pub fn stdio_line(command: &str, args: &[String], home: &str) -> String {
    let words: Vec<String> = read_args(command, args, home).into_iter().filter_map(|r| r.show).collect();
    let mut parts = vec![tilde(home, command)];
    if words.len() > 5 {
        parts.extend(words[..5].iter().cloned());
        parts.push("…".to_string());
    } else {
        parts.extend(words);
    }
    parts.join(" ")
}

/// `~`-relative, once each, in first-seen order: arguments, then cwd, then env values in their order.
pub fn home_paths(server: &McpServer, home: &str) -> Vec<String> {
    let McpTransport::Stdio { args, env, cwd, .. } = &server.transport else { return Vec::new() };
    let home_slash = format!("{home}/");
    let local_bin = format!("{home}/.local/bin/");
    let mut out: Vec<String> = Vec::new();
    let sources = args.iter().map(|a| path_of(a)).chain(cwd.iter().cloned()).chain(env.values().cloned());
    for s in sources {
        let abs = match s.strip_prefix("~/") {
            Some(rest) => format!("{home}/{rest}"),
            None => s.clone(),
        };
        if !abs.starts_with(&home_slash) || abs.starts_with(&local_bin) {
            continue;
        }
        let shown = tilde(home, &abs);
        if !out.contains(&shown) {
            out.push(shown);
        }
    }
    out
}
