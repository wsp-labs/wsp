// SPDX-License-Identifier: AGPL-3.0-only
//! Inline code naming a file, read by the web app's rules (`lib/markdownLinks.ts`) and drawn as a file chip.

use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use gpui_kit::base::text::{markdown_ast as mdast, InlineElement, InlineRenderContext, MarkdownNode, MarkdownParseContext, MarkdownPlugin};
use gpui_kit::component::{h_flex, ActiveTheme, Icon, Sizable as _};
use gpui_kit::{div, prelude::*, px, relative, App, FontWeight, Pixels, SharedString, Window};

use super::{fence_marker, open_file};
use crate::ui::{ActivePalette, Glyph};

/// A file a message names in inline code (`MarkdownFileLinkMeta` in lib/markdownLinks.ts): the path, the target with
/// its position, the path from the thread's folder when it is inside it, and the line and column it names.
#[derive(Clone, Debug, PartialEq)]
pub struct FileMeta {
    pub file_path: String,
    pub target: String,
    pub relative: Option<String>,
    pub basename: String,
    pub line: Option<u64>,
    pub column: Option<u64>,
}

impl FileMeta {
    /// What the Files pane opens: the path from the folder inside it, the host's own path outside it.
    pub fn panel_path(&self) -> Option<&str> {
        self.relative.as_deref().or_else(|| is_absolute(&self.file_path).then_some(self.file_path.as_str()))
    }
}

fn regex(cell: &'static OnceLock<regex::Regex>, pattern: &str) -> &'static regex::Regex {
    cell.get_or_init(|| regex::Regex::new(pattern).expect("a regex"))
}

fn drive(path: &str) -> bool {
    static R: OnceLock<regex::Regex> = OnceLock::new();
    regex(&R, r"^[A-Za-z]:[\\/]").is_match(path)
}

fn unc(path: &str) -> bool {
    path.starts_with("\\\\")
}

fn dotted(path: &str) -> bool {
    path.starts_with("~/") || path.starts_with("./") || path.starts_with("../")
}

fn position_suffix(path: &str) -> bool {
    static R: OnceLock<regex::Regex> = OnceLock::new();
    regex(&R, r":\d+(?::\d+)?$").is_match(path)
}

fn strip_position(path: &str) -> &str {
    static R: OnceLock<regex::Regex> = OnceLock::new();
    regex(&R, r":\d+(?::\d+)?$").find(path).map_or(path, |m| &path[..m.start()])
}

/// `isAbsolutePath` in terminal-links.ts.
fn is_absolute(path: &str) -> bool {
    path.starts_with('/') || drive(path) || unc(path)
}

/// `looksLikeHostname` in lib/markdownLinkParsing.ts.
fn looks_like_hostname(segment: &str, has_position: bool) -> bool {
    const GENERIC: [&str; 25] = [
        "com", "net", "org", "io", "dev", "app", "ai", "co", "edu", "gov", "mil", "info", "biz", "xyz", "me", "tv", "cc", "gg", "chat",
        "cloud", "site", "online", "tech", "store", "link",
    ];
    const COUNTRY: [&str; 31] = [
        "uk", "de", "fr", "nl", "se", "no", "fi", "dk", "pl", "ch", "at", "be", "es", "it", "pt", "eu", "us", "ca", "au", "nz", "jp", "kr",
        "cn", "br", "ru", "mx", "ie", "cz", "tr", "sg", "hk",
    ];
    static NUMERIC: OnceLock<regex::Regex> = OnceLock::new();
    if segment.starts_with('.') {
        return false;
    }
    let lowered = segment.to_lowercase();
    if lowered == "localhost" || regex(&NUMERIC, r"^\d+(?:\.\d+)+$").is_match(segment) {
        return true;
    }
    let labels: Vec<&str> = lowered.split('.').collect();
    let Some(last) = labels.last().filter(|_| labels.len() >= 2) else { return false };
    GENERIC.contains(last) || (!has_position && COUNTRY.contains(last))
}

/// `isConventionalFilePosition`: `Makefile:12`, an extensionless name the :line marks as a file.
fn conventional_position(path: &str) -> bool {
    const NAMES: [&str; 27] = [
        "Makefile",
        "makefile",
        "GNUmakefile",
        "Dockerfile",
        "Containerfile",
        "Justfile",
        "justfile",
        "Rakefile",
        "Gemfile",
        "Procfile",
        "Brewfile",
        "Caddyfile",
        "Vagrantfile",
        "Jenkinsfile",
        "Podfile",
        "Fastfile",
        "BUILD",
        "WORKSPACE",
        "LICENSE",
        "LICENCE",
        "COPYING",
        "NOTICE",
        "AUTHORS",
        "CONTRIBUTORS",
        "CHANGELOG",
        "README",
        "CODEOWNERS",
    ];
    static R: OnceLock<regex::Regex> = OnceLock::new();
    regex(&R, r"^[A-Za-z0-9_-]+(?::\d+){1,2}$").is_match(path) && NAMES.contains(&strip_position(path))
}

/// `inlineCodeFilePathCandidate`: inline code shaped like a path, which takes a path prefix, an extension or a :line.
fn inline_candidate(code: &str) -> Option<String> {
    static EXTENSION: OnceLock<regex::Regex> = OnceLock::new();
    let trimmed = code.trim();
    if trimmed.is_empty() || trimmed.contains(|c: char| c.is_whitespace() || c == '`') {
        return None;
    }
    let candidate = if drive(trimmed) || unc(trimmed) { trimmed.to_owned() } else { trimmed.replace('\\', "/") };
    let has_position = position_suffix(&candidate);
    if !has_position && !candidate.contains(['/', '\\']) {
        return None;
    }
    let explicit = dotted(&candidate) || candidate.starts_with('/') || drive(&candidate) || unc(&candidate);
    if !explicit {
        let bare = strip_position(&candidate);
        if looks_like_hostname(bare.split('/').next().unwrap_or(bare), has_position) {
            return None;
        }
        let basename = bare.trim_end_matches(['/', '\\']).rsplit(['/', '\\']).next().unwrap_or("");
        if !has_position && !regex(&EXTENSION, r"\.[A-Za-z0-9_-]+$").is_match(basename) {
            return None;
        }
    }
    Some(candidate)
}

/// `decodeURIComponent`, keeping the text as it stands where it does not decode.
pub(super) fn decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut ix = 0;
    while ix < bytes.len() {
        if bytes[ix] == b'%' {
            match text.get(ix + 1..ix + 3).and_then(|hex| u8::from_str_radix(hex, 16).ok()) {
                Some(byte) => {
                    out.push(byte);
                    ix += 3;
                    continue;
                }
                None => return text.to_owned(),
            }
        }
        out.push(bytes[ix]);
        ix += 1;
    }
    String::from_utf8(out).unwrap_or_else(|_| text.to_owned())
}

/// `resolveMarkdownFileLinkTarget` for an inline code candidate: a path the folder reads relative ones against.
fn link_target(href: &str, cwd: Option<&str>) -> Option<String> {
    static SCHEME: OnceLock<regex::Regex> = OnceLock::new();
    static RELATIVE_PATH: OnceLock<regex::Regex> = OnceLock::new();
    static RELATIVE_NAME: OnceLock<regex::Regex> = OnceLock::new();
    static HASH_LINE: OnceLock<regex::Regex> = OnceLock::new();
    static POSITION_ONLY: OnceLock<regex::Regex> = OnceLock::new();
    let raw = href.trim();
    if raw.is_empty() || raw.starts_with('#') || raw.starts_with("//") {
        return None;
    }
    let (with_search, hash) = raw.split_at(raw.find('#').unwrap_or(raw.len()));
    let path = &with_search[..with_search.find('?').unwrap_or(with_search.len())];
    let path = decode(path.trim());
    let path = if path.len() > 3 && path.starts_with('/') && drive(&path[1..]) { path[1..].to_owned() } else { path };
    let hash = decode(hash.trim());
    if path.is_empty() {
        return None;
    }
    if !drive(&path) && !unc(&path) {
        if let Some(found) = regex(&SCHEME, r"^([A-Za-z][A-Za-z0-9+.-]*):(.*)$").captures(&path) {
            let rest = found.get(2).map_or("", |m| m.as_str());
            if rest.starts_with("//") || !regex(&POSITION_ONLY, r"^\d+(?::\d+)?$").is_match(rest) {
                return None;
            }
        }
    }
    let likely = if drive(&path) || unc(&path) || dotted(&path) {
        true
    } else if path.starts_with('/') {
        const ROOTS: [&str; 24] = [
            "/Users/",
            "/home/",
            "/tmp/",
            "/var/",
            "/etc/",
            "/opt/",
            "/mnt/",
            "/Volumes/",
            "/private/",
            "/root/",
            "/usr/",
            "/bin/",
            "/sbin/",
            "/lib/",
            "/lib64/",
            "/srv/",
            "/dev/",
            "/proc/",
            "/sys/",
            "/run/",
            "/boot/",
            "/media/",
            "/workspace/",
            "/workspaces/",
        ];
        let basename = &path[path.rfind('/').map_or(0, |at| at + 1)..];
        ROOTS.iter().any(|root| path.starts_with(root))
            || position_suffix(&path)
            || basename.rfind('.').is_some_and(|at| {
                at + 1 < basename.len() && basename[at + 1..].chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
            })
    } else {
        regex(&RELATIVE_PATH, r"^(?:[A-Za-z0-9._-]+(?: +[A-Za-z0-9._-]+)*/)+[A-Za-z0-9._-]+(?: +[A-Za-z0-9._-]+)*(?::\d+){0,2}$")
            .is_match(&path)
            || regex(&RELATIVE_NAME, r"^[A-Za-z0-9._-]+(?: +[A-Za-z0-9._-]+)*\.[A-Za-z0-9_-]+(?::\d+){0,2}$").is_match(&path)
    };
    if !likely {
        return None;
    }
    let path = match regex(&HASH_LINE, r"(?i)^#L(\d+)(?:C(\d+))?$").captures(&hash).filter(|_| !position_suffix(&path)) {
        Some(found) => format!("{path}:{}{}", &found[1], found.get(2).map_or(String::new(), |c| format!(":{}", c.as_str()))),
        None => path,
    };
    if !dotted(&path) && is_absolute(&path) {
        return Some(path);
    }
    cwd.map(|cwd| in_folder(&path, cwd))
}

/// `splitPathAndPosition`: a trailing `:line` or `:line:column`.
fn split_position(value: &str) -> (&str, Option<&str>, Option<&str>) {
    fn tail(text: &str) -> Option<(&str, &str)> {
        let (head, digits) = text.rsplit_once(':')?;
        (!digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit())).then_some((head, digits))
    }
    let Some((head, last)) = tail(value) else { return (value, None, None) };
    match tail(head) {
        Some((path, line)) => (path, Some(line), Some(last)),
        None => (head, Some(last), None),
    }
}

/// `resolvePathLinkTarget`: `~/` from the home the folder sits in, a relative path joined to the folder.
fn in_folder(raw: &str, cwd: &str) -> String {
    static HOME: OnceLock<regex::Regex> = OnceLock::new();
    let (path, line, column) = split_position(raw);
    let windows = drive(cwd) || cwd.contains(":\\");
    let join = |base: &str, next: &str| {
        let base = base.trim_end_matches(['/', '\\']);
        if windows {
            format!("{base}\\{}", next.replace('/', "\\"))
        } else {
            format!("{base}/{}", next.trim_start_matches('/'))
        }
    };
    let resolved = if let Some(rest) = path.strip_prefix("~/") {
        let home = regex(&HOME, r"^(/Users/[^/]+|/home/[^/]+|[A-Za-z]:\\Users\\[^\\]+)").find(cwd).map(|m| m.as_str());
        home.map_or_else(|| path.to_owned(), |home| join(home, rest))
    } else if !is_absolute(path) {
        join(cwd, path)
    } else {
        path.to_owned()
    };
    match (line, column) {
        (Some(line), Some(column)) => format!("{resolved}:{line}:{column}"),
        (Some(line), None) => format!("{resolved}:{line}"),
        _ => resolved,
    }
}

/// What a chip opens in Files (`panelPath` through `openNamedFile`): the whole path with `.` and `..` walked, for a file
/// in the folder or named whole.
pub(super) fn chip_path(meta: &FileMeta) -> Option<String> {
    meta.panel_path().map(|_| crate::screens::terminal::links::resolve_path(&meta.file_path, "/"))
}

/// `buildFileLinkMetaFromTarget`.
fn file_meta(target: String, cwd: Option<&str>) -> FileMeta {
    let (path, line, column) = split_position(&target);
    let path = path.to_owned();
    let relative = cwd.and_then(|root| {
        let normal = |p: &str| {
            let p = p.replace('\\', "/");
            if p.len() > 3 && p.starts_with('/') && drive(&p[1..]) {
                p[1..].to_owned()
            } else {
                p
            }
        };
        let (path, root) = (normal(&path), normal(root));
        let root = root.trim_end_matches('/');
        // The web slices at the root's length in UTF-16 units; a case-folded match may differ from it in bytes.
        let skip = root.chars().count() + 1;
        path.to_lowercase().starts_with(&format!("{}/", root.to_lowercase())).then(|| path.chars().skip(skip).collect())
    });
    let trimmed = path.trim_end_matches(['/', '\\']);
    let trimmed = if trimmed.is_empty() { path.as_str() } else { trimmed };
    let basename = trimmed.rsplit(['/', '\\']).next().unwrap_or(trimmed).to_owned();
    let (line, column) = (line.and_then(|l| l.parse().ok()), column.and_then(|c| c.parse().ok()));
    FileMeta { file_path: path, target: target.clone(), relative, basename, line, column }
}

/// `resolveInlineCodeFileLinkMeta`: the file inline code names, read against the thread's folder.
pub fn inline_file(code: &str, cwd: Option<&str>) -> Option<FileMeta> {
    let candidate = inline_candidate(code)?;
    if let Some(target) = link_target(&candidate, cwd) {
        return Some(file_meta(target, cwd));
    }
    let cwd = cwd.filter(|_| conventional_position(&candidate))?;
    Some(file_meta(in_folder(&candidate, cwd), Some(cwd)))
}

/// `extractInlineCodeSpans`: each inline code span outside fenced code, trimmed.
fn inline_spans(text: &str) -> Vec<&str> {
    static SPAN: OnceLock<regex::Regex> = OnceLock::new();
    let span = regex(&SPAN, r"`([^`\n]+)`");
    let mut spans = Vec::new();
    let mut fence: Option<(char, usize)> = None;
    for line in text.lines() {
        match (fence, fence_marker(line)) {
            (None, Some(marker)) => fence = Some(marker),
            (Some((c, n)), Some((d, m))) if c == d && m >= n => fence = None,
            (Some(_), _) => {}
            (None, None) => {
                spans.extend(span.captures_iter(line).filter_map(|c| c.get(1)).map(|m| m.as_str().trim()).filter(|s| !s.is_empty()))
            }
        }
    }
    spans
}

/// `buildFileLinkParentSuffixByPath`: for files of one name, the parent folders that tell them apart, at least two.
pub(super) fn parent_suffixes(paths: &[String]) -> HashMap<String, String> {
    let mut groups: HashMap<String, Vec<String>> = HashMap::new();
    for path in paths {
        let normal = path.replace('\\', "/");
        let Some(base) = normal.split('/').rfind(|s| !s.is_empty()).map(str::to_owned) else { continue };
        let group = groups.entry(base).or_default();
        if !group.contains(&normal) {
            group.push(normal);
        }
    }
    let mut out = HashMap::new();
    for group in groups.values().filter(|g| g.len() >= 2) {
        let parents: Vec<Vec<&str>> = group
            .iter()
            .map(|p| {
                let segments: Vec<&str> = p.split('/').filter(|s| !s.is_empty()).collect();
                segments[..segments.len().saturating_sub(1)].to_vec()
            })
            .collect();
        for (ix, path) in group.iter().enumerate() {
            let segments = &parents[ix];
            if segments.is_empty() {
                continue;
            }
            let tail = |s: &[&str], depth: usize| s[s.len().saturating_sub(depth)..].join("/");
            let depth = (1..=segments.len())
                .find(|&depth| {
                    let mine = tail(segments, depth);
                    !parents.iter().enumerate().any(|(other, theirs)| other != ix && tail(theirs, depth) == mine)
                })
                .unwrap_or(segments.len());
            out.insert(path.clone(), tail(segments, segments.len().min(depth.max(2))));
        }
    }
    out
}

/// Inline code naming a file, drawn as today's file chip (`MarkdownFileLink`): the file glyph, the name, the folder that
/// tells two files of one name apart and the line, the whole path on its hover; a press opens it in Files.
/// A message's source and its chips' parent folders.
type Suffixes = (String, Arc<HashMap<String, String>>);

pub struct FileChips {
    cwd: Option<String>,
    /// The message's chips' parent folders, by the source they were read off, so each node does not read it again.
    suffixes: std::sync::Mutex<Option<Suffixes>>,
}

impl FileChips {
    pub fn new(cwd: Option<&str>) -> Self {
        FileChips { cwd: cwd.map(str::to_owned), suffixes: std::sync::Mutex::new(None) }
    }

    fn suffixes(&self, source: &str) -> Arc<HashMap<String, String>> {
        let mut held = self.suffixes.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((_, suffixes)) = held.as_ref().filter(|(read, _)| read == source) {
            return suffixes.clone();
        }
        let cwd = self.cwd.as_deref();
        let paths: Vec<String> = inline_spans(source).into_iter().filter_map(|span| inline_file(span, cwd)).map(|m| m.file_path).collect();
        let suffixes = Arc::new(parent_suffixes(&paths));
        *held = Some((source.to_owned(), suffixes.clone()));
        suffixes
    }
}

/// A chip as it parsed: the file and its label's parts.
struct Chip {
    meta: FileMeta,
    label: SharedString,
}

/// `fileChipLabel`: the name, the folder that tells it apart, `L12:C3`.
fn chip_label(meta: &FileMeta, suffixes: &HashMap<String, String>) -> String {
    let mut parts = vec![meta.basename.clone()];
    if let Some(suffix) = suffixes.get(&meta.file_path.replace('\\', "/")).filter(|s| !s.is_empty()) {
        parts.push(suffix.clone());
    }
    if let Some(line) = meta.line.filter(|l| *l > 0) {
        parts.push(match meta.column.filter(|c| *c > 0) {
            Some(column) => format!("L{line}:C{column}"),
            None => format!("L{line}"),
        });
    }
    parts.join(" ")
}

const CHIP: &str = "wsp-file-chip";

impl MarkdownPlugin for FileChips {
    fn name(&self) -> &str {
        CHIP
    }

    fn parse(&self, node: &mdast::Node, cx: &MarkdownParseContext<'_>) -> Option<MarkdownNode> {
        let mdast::Node::InlineCode(code) = node else { return None };
        let meta = inline_file(&code.value, self.cwd.as_deref())?;
        let label: SharedString = chip_label(&meta, &self.suffixes(cx.source())).into();
        let markdown = cx.node_source(node).map_or_else(|| format!("`{}`", code.value), str::to_owned);
        Some(MarkdownNode::new(CHIP, Chip { meta, label: label.clone() }).text(label).markdown(markdown))
    }

    fn render_inline(
        &self,
        node: &MarkdownNode,
        context: &InlineRenderContext,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<InlineElement> {
        let chip = node.data::<Chip>()?;
        let p = cx.palette().clone();
        let font = window.text_system().resolve_font(&context.text_style().font());
        let x_height = window.text_system().x_height(font, context.font_size());
        let panel = chip_path(&chip.meta);
        let line = chip.meta.line;
        let target = SharedString::from(chip.meta.target.clone());
        let element = h_flex()
            .id(SharedString::from(format!("file-chip:{}", chip.meta.target)))
            .debug_selector(|| "file-chip".into())
            .flex_none()
            .h(CHIP_HEIGHT)
            .gap(px(12. * 0.33))
            .px(px(6.))
            .rounded(px(6.))
            .border_1()
            .border_color(p.border.opacity(0.7))
            .bg(p.accent.opacity(0.4))
            .text_size(px(12.))
            .line_height(px(15.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(p.foreground)
            .child(Icon::new(Glyph::File).with_size(px(12. * 1.17)).flex_none().opacity(0.85))
            .child(div().min_w_0().truncate().child(chip.label.clone()))
            .tooltip(path_tip(target))
            .when_some(panel, |el, path| {
                el.cursor_pointer().hover(|s| s.bg(p.accent.opacity(0.7))).on_click(move |_, window, cx| {
                    cx.stop_propagation();
                    open_file(&path, line, window, cx);
                })
            });
        // `vertical-align: middle`: the chip's middle half an x-height over the line's baseline.
        Some(InlineElement::new(element).with_baseline(CHIP_HEIGHT / 2. + x_height / 2.))
    }
}

/// `h-[1.41em]` at the chip's 12 px.
const CHIP_HEIGHT: Pixels = px(12. * 1.41);

/// A chip's tooltip: the whole path.
fn path_tip(path: SharedString) -> impl Fn(&mut Window, &mut App) -> gpui_kit::AnyView + 'static {
    move |window, cx| {
        let (path, mono) = (path.clone(), cx.theme().mono_font_family.clone());
        crate::ui::tooltip_view(move |_| path_text(path.clone(), mono.clone()), window, cx)
    }
}

/// The path in a chip's tooltip, `font-mono text-[11px] leading-tight`.
pub fn path_text(path: SharedString, mono: SharedString) -> gpui_kit::Stateful<gpui_kit::Div> {
    div()
        .id("file-chip-tip")
        .debug_selector(|| "file-chip-tip".into())
        .font_family(mono)
        .text_size(px(11.))
        .line_height(relative(1.25))
        .whitespace_nowrap()
        .child(path)
}
