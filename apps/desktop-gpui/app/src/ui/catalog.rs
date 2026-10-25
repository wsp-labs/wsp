// SPDX-License-Identifier: AGPL-3.0-only
//! What the app draws of each agent, read from the catalog the web app draws it from (packages/catalog/src/agents, with
//! each mark's source and license beside it): its name, and its mark filled with its inks for the side the window
//! draws, else the ink of the text around it, as HarnessMark.tsx fills them.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

use gpui_kit::component::ActiveTheme;
use gpui_kit::{div, img, prelude::*, AnyElement, App, FontWeight, Hsla, Image, ImageFormat, Pixels, Rgba};

const AGENTS: [(&str, &str); 11] = [
    ("amp", include_str!("../../../../../packages/catalog/src/agents/amp.ts")),
    ("claude", include_str!("../../../../../packages/catalog/src/agents/claude.ts")),
    ("codex", include_str!("../../../../../packages/catalog/src/agents/codex.ts")),
    ("crush", include_str!("../../../../../packages/catalog/src/agents/crush.ts")),
    ("cursor", include_str!("../../../../../packages/catalog/src/agents/cursor.ts")),
    ("gemini", include_str!("../../../../../packages/catalog/src/agents/gemini.ts")),
    ("goose", include_str!("../../../../../packages/catalog/src/agents/goose.ts")),
    ("hermes", include_str!("../../../../../packages/catalog/src/agents/hermes.ts")),
    ("opencode", include_str!("../../../../../packages/catalog/src/agents/opencode.ts")),
    ("pi", include_str!("../../../../../packages/catalog/src/agents/pi.ts")),
    ("qwen", include_str!("../../../../../packages/catalog/src/agents/qwen.ts")),
];

fn source(agent: &str) -> Option<&'static str> {
    AGENTS.iter().find(|(id, _)| *id == agent).map(|(_, src)| *src)
}

/// `agentName`: the catalog's name for the agent, or the id itself for one the catalog does not list.
pub fn name(agent: &str) -> String {
    let named = source(agent).and_then(|src| {
        let at = src.find("\n  name: \"")? + "\n  name: \"".len();
        Some(&src[at..at + src[at..].find('"')?])
    });
    named.map_or_else(|| agent.to_owned(), str::to_owned)
}

/// A mark's svg and its inks, light then dark, as its catalog module writes them.
struct Mark {
    svg: &'static str,
    inks: Vec<(&'static str, &'static str)>,
}

fn read(src: &'static str) -> Option<Mark> {
    let mark = &src[src.find("mark: {")?..];
    let svg_at = mark.find("svg: `")? + "svg: `".len();
    let svg = &mark[svg_at..svg_at + mark[svg_at..].find('`')?];
    let head = &mark[..svg_at];
    let mut inks = Vec::new();
    let mut rest = head.find("inks: [").map_or("", |at| &head[at..]);
    while let Some(at) = rest.find("light: \"") {
        let light_at = at + "light: \"".len();
        let light = &rest[light_at..light_at + rest[light_at..].find('"')?];
        let dark_at = light_at + rest[light_at..].find("dark: \"")? + "dark: \"".len();
        let dark = &rest[dark_at..dark_at + rest[dark_at..].find('"')?];
        inks.push((light, dark));
        rest = &rest[dark_at..];
    }
    Some(Mark { svg, inks })
}

/// The mark's first ink, light and dark, where it has one; the Usage chart draws the agent's line in it.
pub fn ink(agent: &str) -> Option<(Rgba, Rgba)> {
    let (light, dark) = *read(source(agent)?)?.inks.first()?;
    Some((Rgba::try_from(light).ok()?, Rgba::try_from(dark).ok()?))
}

/// Each drawn mark by agent or brand, side and text ink.
type Drawn = HashMap<(&'static str, bool, u32), Option<Arc<Image>>>;

thread_local! {
    /// One image per agent, side and text ink: the catalog's agents in each theme at most.
    static DRAWN: RefCell<Drawn> = RefCell::new(HashMap::new());
    /// The same for the brands' marks.
    static BRANDS_DRAWN: RefCell<Drawn> = RefCell::new(HashMap::new());
}

/// A mark as an svg filled with its inks for one side, else the text's, `0xrrggbb`.
fn fill(mark: &Mark, dark: bool, text: u32) -> Arc<Image> {
    let text_hex = format!("#{text:06x}");
    let ink = |n: usize| mark.inks.get(n).map(|(light, d)| if dark { *d } else { *light });
    let first = ink(0).unwrap_or(text_hex.as_str());
    let mut svg = mark.svg.replacen("<svg ", &format!("<svg xmlns=\"http://www.w3.org/2000/svg\" fill=\"{first}\" "), 1);
    for n in 0..mark.inks.len() {
        svg = svg.replace(&format!("var(--ink-{n})"), ink(n).unwrap_or(first));
    }
    svg = svg.replace("currentColor", first);
    Arc::new(Image::from_bytes(ImageFormat::Svg, svg.into_bytes()))
}

fn rgb(text: Hsla) -> u32 {
    let text = Rgba::from(text);
    let hex = |c: f32| (c * 255.).round() as u32;
    hex(text.r) << 16 | hex(text.g) << 8 | hex(text.b)
}

/// The agent's mark for one side, in its inks or else `text`'s, or none where the catalog draws none and the
/// initials stand in.
pub fn image(harness: &str, dark: bool, text: Hsla) -> Option<Arc<Image>> {
    let (id, src) = AGENTS.iter().find(|(id, _)| *id == harness)?;
    let text = rgb(text);
    DRAWN.with(|drawn| drawn.borrow_mut().entry((id, dark, text)).or_insert_with(|| Some(fill(&read(src)?, dark, text))).clone())
}

// ---- The brands' marks (packages/catalog/src/marks), which a tool server's row leads with. ----

const BRANDS: [&str; 4] = [
    include_str!("../../../../../packages/catalog/src/marks/node.ts"),
    include_str!("../../../../../packages/catalog/src/marks/github.ts"),
    include_str!("../../../../../packages/catalog/src/marks/cloudflare.ts"),
    include_str!("../../../../../packages/catalog/src/marks/linear.ts"),
];

/// `BrandMark`: its id, its mark, and the addresses and packages it claims.
struct Brand {
    id: &'static str,
    mark: Mark,
    hosts: Vec<&'static str>,
    packages: Vec<&'static str>,
}

/// The quoted words of one list field, `hosts: ["a", "b"]`.
fn words(src: &'static str, field: &str) -> Vec<&'static str> {
    let Some(at) = src.find(&format!("\n  {field}: [")) else { return Vec::new() };
    let list = &src[at..];
    let list = &list[..list.find(']').unwrap_or(list.len())];
    list.split('"').skip(1).step_by(2).collect()
}

fn brand(src: &'static str) -> Option<Brand> {
    let quoted = |key: &str, from: &'static str| -> Option<&'static str> {
        let at = from.find(&format!("{key}: \""))? + key.len() + 3;
        Some(&from[at..at + from[at..].find('"')?])
    };
    let svg_at = src.find("svg: `")? + "svg: `".len();
    let svg = &src[svg_at..svg_at + src[svg_at..].find('`')?];
    let inks = match src.find("\n  inks: [") {
        Some(at) => {
            let list = &src[at..at + src[at..].find(']')?];
            list.split('{').skip(1).filter_map(|one| Some((quoted("light", one)?, quoted("dark", one)?))).collect()
        }
        None => Vec::new(),
    };
    Some(Brand { id: quoted("id", src)?, mark: Mark { svg, inks }, hosts: words(src, "hosts"), packages: words(src, "packages") })
}

/// Every brand, read once.
fn brands() -> &'static [Brand] {
    static BRANDS_READ: std::sync::OnceLock<Vec<Brand>> = std::sync::OnceLock::new();
    BRANDS_READ.get_or_init(|| BRANDS.iter().filter_map(|src| brand(src)).collect())
}

/// `hostUnder`: the domain itself or a host under it, its port and a root dot aside.
fn host_under(host: &str, domain: &str) -> bool {
    let bare = host.rsplit_once(':').filter(|(_, port)| port.chars().all(|c| c.is_ascii_digit())).map_or(host, |(h, _)| h);
    let bare = bare.trim_end_matches('.').to_lowercase();
    bare == domain || bare.ends_with(&format!(".{domain}"))
}

/// The host of a word that is an http address.
fn host_of(word: &str) -> Option<&str> {
    let lower = word.get(..8).unwrap_or(word).to_ascii_lowercase();
    let rest = word.get(
        if lower.starts_with("https://") {
            8
        } else if lower.starts_with("http://") {
            7
        } else {
            return None;
        }..,
    )?;
    let authority = &rest[..rest.find(['/', '?', '#']).unwrap_or(rest.len())];
    let host = authority.rsplit_once('@').map_or(authority, |(_, h)| h);
    (!host.is_empty()).then_some(host)
}

/// The package a word of a command line names, without a version after it: `@scope/name@1.2` is `@scope/name`.
fn package_of(word: &str) -> &str {
    let word = match word.rfind('@') {
        Some(at) if at > 0 && !word[at..].contains('/') => &word[..at],
        _ => word,
    };
    match word.rfind(':') {
        Some(at) if !word[at..].contains('/') => &word[..at],
        _ => word,
    }
}

/// `serverMark`: the brand a tool server is reached at, by the host of its address, or the package or address its
/// command runs.
pub fn server_mark(reach: &str, stdio: bool) -> Option<&'static str> {
    let brands = brands();
    let by_host = |host: &str| brands.iter().find(|b| b.hosts.iter().any(|d| host_under(host, d))).map(|b| b.id);
    if !stdio {
        return by_host(reach);
    }
    reach.split_whitespace().find_map(|word| match host_of(word) {
        Some(host) => by_host(host),
        None => brands.iter().find(|b| b.packages.contains(&package_of(word))).map(|b| b.id),
    })
}

/// Each one-colour mark by agent and ink.
type Mono = HashMap<(&'static str, u32), Option<Arc<Image>>>;

thread_local! {
    /// One one-colour mark per agent and ink.
    static MONO: RefCell<Mono> = RefCell::new(HashMap::new());
}

/// The agent's mark in `ink` alone, its shape masked out of its own inks, gradients and fills, as onboarding draws
/// six vendors' marks as one set; none where the catalog draws no mark.
pub fn mono(harness: &str, ink: Hsla) -> Option<Arc<Image>> {
    use base64::Engine as _;
    let (id, _) = AGENTS.iter().find(|(id, _)| *id == harness)?;
    let c = Rgba::from(ink);
    let byte = |v: f32| (v.clamp(0., 1.) * 255.).round() as u32;
    let key = byte(c.r) << 24 | byte(c.g) << 16 | byte(c.b) << 8 | byte(c.a);
    MONO.with(|mono| {
        mono.borrow_mut()
            .entry((id, key))
            .or_insert_with(|| {
                let mark = image(id, false, ink)?;
                let data = base64::engine::general_purpose::STANDARD.encode(&mark.bytes);
                let svg = format!(
                    r##"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="16" height="16" viewBox="0 0 16 16"><defs><filter id="m"><feFlood flood-color="#{:06x}" flood-opacity="{}"/><feComposite in2="SourceAlpha" operator="in"/></filter></defs><image width="16" height="16" filter="url(#m)" xlink:href="data:image/svg+xml;base64,{data}"/></svg>"##,
                    key >> 8,
                    c.a
                );
                Some(Arc::new(Image::from_bytes(ImageFormat::Svg, svg.into_bytes())))
            })
            .clone()
    })
}

/// `MarkSvg` of a brand at `size`, in its inks or else `ink`, the text's around it.
pub fn brand_mark(id: &str, size: Pixels, ink: Hsla, cx: &App) -> Option<AnyElement> {
    let dark = cx.theme().is_dark();
    let text = rgb(ink);
    let brand = brands().iter().find(|b| b.id == id)?;
    let image = BRANDS_DRAWN
        .with(|drawn| drawn.borrow_mut().entry((brand.id, dark, text)).or_insert_with(|| Some(fill(&brand.mark, dark, text))).clone())?;
    Some(img(image).size(size).flex_none().into_any_element())
}

// ---- The editors' marks (packages/catalog/src/marks/editors), which the editor select draws. ----

/// `EDITOR_MARKS` in order, each module beside the one its svg is written in: Cursor's is the agent's own.
const EDITORS: [(&str, &str); 14] = [
    (
        include_str!("../../../../../packages/catalog/src/marks/editors/vscode.ts"),
        include_str!("../../../../../packages/catalog/src/marks/editors/vscode.ts"),
    ),
    (
        include_str!("../../../../../packages/catalog/src/marks/editors/cursor.ts"),
        include_str!("../../../../../packages/catalog/src/agents/cursor.ts"),
    ),
    (
        include_str!("../../../../../packages/catalog/src/marks/editors/vscode-insiders.ts"),
        include_str!("../../../../../packages/catalog/src/marks/editors/vscode-insiders.ts"),
    ),
    (
        include_str!("../../../../../packages/catalog/src/marks/editors/zed.ts"),
        include_str!("../../../../../packages/catalog/src/marks/editors/zed.ts"),
    ),
    (
        include_str!("../../../../../packages/catalog/src/marks/editors/idea.ts"),
        include_str!("../../../../../packages/catalog/src/marks/editors/idea.ts"),
    ),
    (
        include_str!("../../../../../packages/catalog/src/marks/editors/webstorm.ts"),
        include_str!("../../../../../packages/catalog/src/marks/editors/webstorm.ts"),
    ),
    (
        include_str!("../../../../../packages/catalog/src/marks/editors/pycharm.ts"),
        include_str!("../../../../../packages/catalog/src/marks/editors/pycharm.ts"),
    ),
    (
        include_str!("../../../../../packages/catalog/src/marks/editors/goland.ts"),
        include_str!("../../../../../packages/catalog/src/marks/editors/goland.ts"),
    ),
    (
        include_str!("../../../../../packages/catalog/src/marks/editors/clion.ts"),
        include_str!("../../../../../packages/catalog/src/marks/editors/clion.ts"),
    ),
    (
        include_str!("../../../../../packages/catalog/src/marks/editors/phpstorm.ts"),
        include_str!("../../../../../packages/catalog/src/marks/editors/phpstorm.ts"),
    ),
    (
        include_str!("../../../../../packages/catalog/src/marks/editors/rubymine.ts"),
        include_str!("../../../../../packages/catalog/src/marks/editors/rubymine.ts"),
    ),
    (
        include_str!("../../../../../packages/catalog/src/marks/editors/rider.ts"),
        include_str!("../../../../../packages/catalog/src/marks/editors/rider.ts"),
    ),
    (
        include_str!("../../../../../packages/catalog/src/marks/editors/jetbrains.ts"),
        include_str!("../../../../../packages/catalog/src/marks/editors/jetbrains.ts"),
    ),
    (
        include_str!("../../../../../packages/catalog/src/marks/editors/finder.ts"),
        include_str!("../../../../../packages/catalog/src/marks/editors/finder.ts"),
    ),
];

/// `EditorMark`: the editors it marks, by the id the host opens them by, and its svg.
struct EditorMark {
    editors: Vec<&'static str>,
    mark: Mark,
}

fn editor(module: &'static str, drawn: &'static str) -> Option<EditorMark> {
    let list = &module[module.find("editors: [")?..];
    let list = &list[..list.find(']')?];
    let svg_at = drawn.find("svg: `")? + "svg: `".len();
    let svg = &drawn[svg_at..svg_at + drawn[svg_at..].find('`')?];
    Some(EditorMark { editors: list.split('"').skip(1).step_by(2).collect(), mark: Mark { svg, inks: Vec::new() } })
}

fn editors() -> &'static [EditorMark] {
    static EDITORS_READ: std::sync::OnceLock<Vec<EditorMark>> = std::sync::OnceLock::new();
    EDITORS_READ.get_or_init(|| EDITORS.iter().filter_map(|(module, drawn)| editor(module, drawn)).collect())
}

thread_local! {
    /// The editors' marks, by the first editor each marks.
    static EDITORS_DRAWN: RefCell<Drawn> = RefCell::new(HashMap::new());
}

/// `EditorGlyph`: the editor's mark at `size` in `ink`, the text's around it; none for an editor no mark claims, where
/// the web keeps the mark's square empty.
pub fn editor_mark(editor: &str, size: Pixels, ink: Hsla, cx: &App) -> Option<AnyElement> {
    let found = editors().iter().find(|e| e.editors.contains(&editor))?;
    let (dark, text) = (cx.theme().is_dark(), rgb(ink));
    let image = EDITORS_DRAWN.with(|drawn| {
        drawn.borrow_mut().entry((found.editors[0], dark, text)).or_insert_with(|| Some(fill(&found.mark, dark, text))).clone()
    })?;
    Some(img(image).size(size).flex_none().into_any_element())
}

/// HarnessMark.tsx `harnessInitials`: two letters of one word, or the first letters of two.
pub fn initials(label: &str) -> String {
    let words: Vec<&str> = label.split_whitespace().collect();
    match words.as_slice() {
        [] => String::new(),
        [one] => one.chars().take(2).collect::<String>().to_uppercase(),
        more => more.iter().take(2).filter_map(|w| w.chars().next()).collect::<String>().to_uppercase(),
    }
}

/// `HarnessMark`: the agent's mark at `size`, or its initials where it has none; a mark with no inks of its own, and
/// the initials, take `ink`, the text's around them.
pub fn mark(harness: &str, label: &str, size: Pixels, ink: Hsla, cx: &App) -> AnyElement {
    match image(harness, cx.theme().is_dark(), ink) {
        Some(image) => img(image).size(size).flex_none().into_any_element(),
        None => div()
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(size)
            .text_size(size * 0.7)
            .font_family(cx.theme().mono_font_family.clone())
            .font_weight(FontWeight::EXTRA_BOLD)
            .text_color(ink)
            .child(initials(label))
            .into_any_element(),
    }
}

/// The catalog's sign-in rows (`SIGN_IN_ROWS`) and its tools, for the sign-in an agent or a CLI takes.
const SIGN_INS: &str = include_str!("../../../../../packages/catalog/src/signin.ts");
const TOOLS: &str = include_str!("../../../../../packages/catalog/src/catalog.ts");

/// A string field at the top level of a block of the catalog's source: `    <field>: "<value>",`.
fn field<'a>(block: &'a str, indent: &str, name: &str) -> Option<&'a str> {
    let head = format!("\n{indent}{name}: \"");
    let at = block.find(&head)? + head.len();
    Some(&block[at..at + block[at..].find('"')?])
}

/// `planBrand`: the brand an agent's plan is sold under, where the catalog names one.
pub fn plan_brand(agent: &str) -> Option<&'static str> {
    field(source(agent)?, "  ", "planBrand")
}

/// An agent's sign-in as its catalog row writes it: how it signs in (`signInRoadOf`) and the line that mints its token.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SignIn {
    /// The agents report's road: device, code, key, token, terminal or none.
    pub road: &'static str,
    /// `mint`: the line a token sign-in runs to make its token.
    pub mint: Option<&'static str>,
}

/// The row of `SIGN_IN_ROWS` an agent's `signIn` names, none for one with no sign-in or an id the catalog lacks.
fn sign_in_row(agent: &str) -> Option<&'static str> {
    let src = source(agent)?;
    let at = src.find("\n  signIn: SIGN_IN_ROWS.")? + "\n  signIn: SIGN_IN_ROWS.".len();
    let row = &src[at..at + src[at..].find(|c: char| !c.is_alphanumeric() && c != '_')?];
    let rows = &SIGN_INS[SIGN_INS.find("export const SIGN_IN_ROWS = {")?..];
    let at = rows.find(&format!("\n  {row}: {{"))?;
    let block = &rows[at + 1..];
    Some(&block[..block.find("\n  },").unwrap_or(block.len())])
}

/// The sign-in an agent takes, as the catalog says it: none where its row is `NO_SIGN_IN`.
pub fn sign_in(agent: &str) -> Option<SignIn> {
    source(agent)?;
    let Some(block) = sign_in_row(agent) else { return Some(SignIn { road: "none", mint: None }) };
    let kind = field(block, "    ", "kind")?;
    let asks = block.contains("person:");
    let road = match kind {
        _ if asks => "terminal",
        "token" => "token",
        "key" => "key",
        "none" => "none",
        "device" => "device",
        _ => match field(block, "    ", "headless") {
            Some("device") => "device",
            _ => "code",
        },
    };
    Some(SignIn { road, mint: field(block, "    ", "mint") })
}

/// `keyEnvOf`: the variable an agent's API key goes under, where its sign-in row names one.
pub fn key_env(agent: &str) -> Option<&'static str> {
    field(sign_in_row(agent)?, "    ", "keyEnv")
}

/// `signInWaysOf`: the ways an agent that mints its token on the computer running the host signs in on one you own:
/// the token, the key where it takes one, and its own login there where it has one. None for every other agent.
pub fn sign_in_ways(agent: &str) -> Vec<&'static str> {
    let Some(block) = sign_in_row(agent).filter(|b| field(b, "    ", "kind") == Some("token")) else { return Vec::new() };
    let mut ways = vec!["token"];
    if key_env(agent).is_some() {
        ways.push("key");
    }
    if block.contains("\n    there: {") {
        ways.push("machine");
    }
    ways
}

/// Whether the login a token agent runs on a computer you own hands its code back to paste (`loginThere`'s road).
pub fn there_pastes(agent: &str) -> bool {
    sign_in_row(agent).and_then(|b| b.lines().find(|l| l.starts_with("    there: {"))).is_some_and(|l| l.contains("headless: \"code\""))
}

/// Whether the catalog's CLI `id` signs in: its entry names a row of `SIGN_IN_ROWS` whose kind is not `none`.
pub fn tool_signs_in(id: &str) -> bool {
    let needle = format!("id: \"{id}\",");
    let Some(line) = TOOLS.lines().find(|l| l.contains(&needle)) else { return false };
    let Some(at) = line.find("signIn: SIGN_IN_ROWS.") else { return false };
    let row = &line[at + "signIn: SIGN_IN_ROWS.".len()..];
    let row = &row[..row.find(|c: char| !c.is_alphanumeric() && c != '_').unwrap_or(row.len())];
    let Some(rows) = SIGN_INS.find("export const SIGN_IN_ROWS = {").map(|at| &SIGN_INS[at..]) else { return false };
    let Some(block) = rows.find(&format!("\n  {row}: {{")).map(|at| &rows[at..]) else { return false };
    block.find("kind: \"").is_some_and(|k| !block[k + "kind: \"".len()..].starts_with("none\""))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_catalogs_names_marks_and_inks_read() {
        let claude = read(AGENTS[1].1).expect("claude's mark");
        assert!(claude.svg.starts_with("<svg viewBox=\"0 0 256 257\">"));
        assert_eq!(claude.inks, [("#c4643f", "#d97757")]);
        assert_eq!(read(source("pi").unwrap()).unwrap().inks.len(), 3);
        for (id, src) in AGENTS {
            assert_eq!(read(src).is_some(), id != "crush", "{id}: the catalog draws a mark for every agent but Crush");
        }
        assert!(image("claude", true, gpui_kit::white()).is_some());
        assert!(image("codex", true, gpui_kit::white()).is_some(), "a mark with no inks takes the text's");
        assert!(image("nobody", true, gpui_kit::white()).is_none());
        assert_eq!(initials("Claude Code"), "CC");
        assert_eq!(initials("codex"), "CO");
        let names: Vec<String> = ["claude", "codex", "opencode", "hermes", "gemini", "crush", "someone"].map(name).into();
        assert_eq!(names, ["Claude Code", "Codex", "OpenCode", "Hermes Agent", "Gemini CLI", "Crush", "someone"]);
        assert_eq!(ink("claude"), Some((Rgba::try_from("#c4643f").unwrap(), Rgba::try_from("#d97757").unwrap())));
        assert_eq!(ink("codex"), None);
    }

    #[test]
    fn the_catalogs_sign_ins_read() {
        assert_eq!(plan_brand("codex"), Some("ChatGPT"));
        assert_eq!(plan_brand("claude"), Some("Claude"));
        assert_eq!(plan_brand("opencode"), None);
        assert_eq!(sign_in("claude"), Some(SignIn { road: "token", mint: Some("claude setup-token") }));
        assert_eq!(sign_in("crush"), Some(SignIn { road: "none", mint: None }));
        assert_eq!(sign_in("nobody"), None);
        assert!(tool_signs_in("gh") && !tool_signs_in("jq"));
        assert!(!tool_signs_in("op") && !tool_signs_in("kubectl"), "a row whose kind is none signs nothing in");
        assert_eq!(sign_in_ways("claude"), ["token", "key", "machine"]);
        assert!(sign_in_ways("codex").is_empty(), "only a token agent has ways");
        assert_eq!(key_env("claude"), Some("ANTHROPIC_API_KEY"));
        assert!(there_pastes("claude"));
    }

    #[test]
    fn every_editor_mark_of_the_catalog_reads_in_its_order() {
        let index = include_str!("../../../../../packages/catalog/src/marks/index.ts");
        let listed = &index[index.find("export const EDITOR_MARKS").unwrap()..];
        let listed = &listed[listed.find("= [").unwrap() + 3..];
        let listed = &listed[..listed.find(']').unwrap()];
        let ids: Vec<String> = listed.split(',').map(|n| n.trim().to_lowercase().replace('_', "-")).collect();
        let module_id = |module: &str| {
            let at = module.find("id: \"").unwrap() + 5;
            module[at..at + module[at..].find('"').unwrap()].to_owned()
        };
        assert_eq!(EDITORS.map(|(module, _)| module_id(module)).to_vec(), ids, "one module per mark the catalog lists, in its order");
        assert_eq!(editors().len(), EDITORS.len(), "every module reads");
        assert_eq!(editors()[0].editors, ["vscode"]);
        let rustrover = editors().iter().find(|e| e.editors.contains(&"rustrover")).expect("JetBrains' mark claims RustRover");
        assert!(rustrover.mark.svg.starts_with("<svg viewBox=\"0 0 24 24\">"));
        assert_eq!(editors()[1].mark.svg, read(AGENTS[4].1).unwrap().svg, "Cursor the editor wears Cursor the agent's mark");
    }

    #[test]
    fn a_servers_brand_is_read_off_its_address_or_its_command() {
        let github = brand(BRANDS[1]).expect("github's mark");
        assert!(github.mark.svg.starts_with("<svg viewBox=\"0 0 24 24\">"));
        assert_eq!(github.hosts, ["github.com", "githubcopilot.com"]);
        assert_eq!(brand(BRANDS[0]).unwrap().mark.inks, [("#579348", "#5fa04e")], "an ink written dark first reads light first");
        assert_eq!(server_mark("api.githubcopilot.com", false), Some("github"));
        assert_eq!(server_mark("MCP.Linear.app:443", false), Some("linear"));
        assert_eq!(server_mark("notlinear.app", false), None);
        assert_eq!(server_mark("npx -y @modelcontextprotocol/server-github@1.2.0", true), Some("github"));
        assert_eq!(server_mark("docker run ghcr.io/github/github-mcp-server:latest", true), Some("github"));
        assert_eq!(server_mark("npx mcp-remote https://mcp.cloudflare.com/sse", true), Some("cloudflare"));
        assert_eq!(server_mark("node ./server.js", true), None, "node the runtime claims no server");
        assert_eq!(package_of("@scope/name@1.2"), "@scope/name");
        assert_eq!(package_of("@scope/name"), "@scope/name");
    }
}
