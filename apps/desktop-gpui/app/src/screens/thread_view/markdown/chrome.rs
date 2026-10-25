// SPDX-License-Identifier: AGPL-3.0-only
//! A reply's code blocks and tables as `MarkdownCodeBlock` and `MarkdownTable` frame them: header, shell run, footer.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use gpui_kit::base::text::{CodeBlock, CodeBlockChrome, RenderedText, TableChrome, TableData};
use gpui_kit::base::{ScrollbarStyles, StyledExt as _};
use gpui_kit::component::menu::DropdownMenu;
use gpui_kit::component::{h_flex, v_flex, ActiveTheme, Icon, Sizable as _};
use gpui_kit::{
    div, prelude::*, px, relative, App, ClipboardItem, Entity, EntityId, FontFeatures, Hsla, SharedString, StyleRefinement, Window,
};

use super::{OnRun, RunAction, WORD_WRAP};
use crate::screens::thread_view::run::{self as run_box, LiveRun, RUN_BOX};
use crate::transcript::{block_at, Run};
use crate::ui::{ActivePalette, Glyph, Palette};

/// What a reply's code blocks need from the view: its source, to name a block and find its run, and Run.
pub(super) struct Reply {
    pub(super) view: EntityId,
    pub(super) text: RenderedText,
    pub(super) runs: Vec<Run>,
    pub(super) on_run: Option<OnRun>,
    pub(super) can_start: bool,
}

/// `MarkdownCodeBlock`: the header row across the block, the code, and under a shell block its run.
pub(super) fn chrome(reply: &Reply, block: &CodeBlock, window: &mut Window, cx: &mut App) -> CodeBlockChrome {
    let p = cx.palette().clone();
    let start = block.span.map_or(0, |span| span.start);
    let source = reply.text.source();
    let info = fence_info(source, start);
    let language = fence_language(info, block.lang());
    let title = fence_title(info);
    let code = block.code();
    let wrapped = window.use_keyed_state(block_key(CODE_WRAP, reply.view, start), cx, |_, _| WORD_WRAP);
    let copied = window.use_keyed_state(block_key(CODE_COPIED, reply.view, start), cx, |_, _| false);
    let said = window.use_keyed_state(block_key(CODE_REFUSED, reply.view, start), cx, |_, _| None::<SharedString>);
    let (wrap, done) = (*wrapped.read(cx), *copied.read(cx));
    let index = || block_at(source, source[..start.min(source.len())].encode_utf16().count());
    let command = runnable_command(&language, &code);
    let run = command.as_ref().and_then(|_| index()).and_then(|ix| reply.runs.iter().find(|run| run.block == ix));
    let mono = cx.theme().mono_font_family.clone();

    let label = h_flex()
        .min_w_0()
        .gap(px(5.6))
        .font_family(mono)
        .text_size(px(11.))
        .line_height(relative(1.375))
        .when_some(title.clone(), |el, _| el.child(Icon::new(Glyph::File).with_size(px(14.)).flex_none()))
        .child(div().min_w_0().truncate().child(title.unwrap_or(language)));
    let mut actions = h_flex().flex_none().gap(px(2.));
    let runnable = command.is_some() && reply.on_run.is_some();
    if let (Some(command), Some(on_run), true) = (command, reply.on_run.clone(), reply.can_start) {
        let running = run.is_some_and(|run| run.state == "running");
        let ix = index();
        let said = said.clone();
        let words = if run.is_some() { run_box::words::RUN_AGAIN } else { run_box::words::RUN };
        actions = actions.child(
            ghost_button("run", Glyph::Play, false, cx)
                .debug_selector(move || if running { "code-run-held" } else { "code-run" }.into())
                .tooltip(crate::ui::tip(words))
                .when(running, |el| el.opacity(0.64).cursor_default())
                .when(!running, |el| {
                    el.on_click(move |_, window, cx| {
                        if let Some(block) = ix {
                            on_run(RunAction::Start { block, command: command.clone(), said: said.clone() }, window, cx);
                        }
                    })
                }),
        );
    }
    actions = actions
        .child(
            ghost_button("wrap", Glyph::TextWrap, wrap, cx)
                .debug_selector(move || if wrap { "code-wrapped" } else { "code-unwrapped" }.into())
                .tooltip(crate::ui::tip(if wrap { "Disable line wrap" } else { "Wrap lines" }))
                .on_click(move |_, _, cx| {
                    wrapped.update(cx, |wrapped, cx| {
                        *wrapped = !*wrapped;
                        cx.notify();
                    })
                }),
        )
        .child(
            ghost_button("copy", if done { Glyph::Check } else { Glyph::Copy }, false, cx)
                .debug_selector(move || if done { "code-copied" } else { "code-copy" }.into())
                .tooltip(crate::ui::tip(if done { "Copied" } else { "Copy code" }))
                .on_click(move |_, _, cx| {
                    cx.write_to_clipboard(ClipboardItem::new_string(code.to_string()));
                    copied.update(cx, |copied, cx| {
                        *copied = true;
                        cx.notify();
                        cx.spawn(async move |copied, cx| {
                            cx.background_executor().timer(Duration::from_millis(1200)).await;
                            copied.update(cx, |copied, cx| {
                                *copied = false;
                                cx.notify();
                            })
                        })
                        .detach();
                    });
                }),
        );
    let header = h_flex()
        .w_full()
        .justify_between()
        .gap(px(8.))
        .pt(px(6.))
        .pr(px(6.))
        .pl(px(12.))
        .text_color(p.foreground.opacity(0.72))
        .child(label)
        .child(actions);

    let thumb = p.border.opacity(0.78);
    let chrome = CodeBlockChrome::new().header(header).wrap(wrap).scrollbar(
        px(7.),
        ScrollbarStyles::default()
            .track(|t| t.width(px(7.)).bg(gpui_kit::transparent_black()).border_color(gpui_kit::transparent_black()))
            .thumb(|t| t.width(px(7.)).inset(px(0.)).radius(px(3.5)).bg(thumb))
            .thumb_hover(|t| t.width(px(7.)).inset(px(0.)).radius(px(3.5)).bg(thumb))
            .thumb_active(|t| t.width(px(7.)).inset(px(0.)).radius(px(3.5)).bg(thumb)),
    );
    let refused = said.read(cx).clone();
    let run = match (run, reply.on_run.clone()) {
        (Some(run), Some(on_run)) if run.state == "running" => {
            let live =
                window.use_keyed_state(block_key(CODE_LIVE, reply.view, start), cx, |_, cx| LiveRun::new(run.clone(), on_run.clone(), cx));
            live.update(cx, |live, cx| live.set(run, on_run, cx));
            Some(live.into_any_element())
        }
        (Some(run), Some(on_run)) => Some(run_view(run, on_run, &p, cx).into_any_element()),
        _ => None,
    };
    if !runnable || (run.is_none() && refused.is_none()) {
        return chrome;
    }
    chrome.footer(v_flex().w_full().children(run).when_some(refused, |el, refused| {
        el.child(
            div()
                .debug_selector(|| "run-refusal".into())
                .px(px(12.))
                .py(px(6.))
                .text_size(px(12.))
                .line_height(px(16.))
                .text_color(p.error_foreground)
                .child(refused),
        )
    }))
}

/// The kinds of element state a view's blocks keep.
const CODE_WRAP: u32 = 1;
const CODE_COPIED: u32 = 2;
const TABLE_EXPANDED: u32 = 3;
const TABLE_COPIED: u32 = 4;
const CODE_REFUSED: u32 = 5;
const CODE_LIVE: u32 = 6;

/// The element state of one of a view's blocks, which starts at byte `start` of its source, named without allocating.
fn block_key(kind: u32, view: EntityId, start: usize) -> gpui_kit::ElementId {
    let mut id = [0u8; 20];
    id[..4].copy_from_slice(&kind.to_le_bytes());
    id[4..12].copy_from_slice(&view.as_u64().to_le_bytes());
    id[12..].copy_from_slice(&(start as u64).to_le_bytes());
    gpui_kit::ElementId::OpaqueId(id)
}

/// `EndedRun`: an exited run's output, how it ended, and Send output to the agent; a moved run's Open the tab.
fn run_view(run: &Run, on_run: OnRun, p: &Palette, cx: &App) -> impl IntoElement {
    let mono = cx.theme().mono_font_family.clone();
    // A `<pre>` draws no line after its text's last newline.
    let output = run
        .output
        .as_deref()
        .filter(|output| run.state == "exited" && !output.is_empty())
        .map(|output| SharedString::from(output.strip_suffix('\n').unwrap_or(output).to_owned()));
    let mut tabular = StyleRefinement::default();
    tabular.text.font_features = Some(FontFeatures(Arc::new(vec![("tnum".into(), 1)])));
    let presses = h_flex()
        .gap(px(4.))
        .when(run.state == "exited", |el| {
            let (message, on_run) = (run_output_message(run), on_run.clone());
            el.child(run_box::run_button("run-send", run_box::words::SEND, move |_, window, cx| {
                on_run(RunAction::Send { message: message.clone() }, window, cx)
            }))
        })
        .when(run.state == "moved", |el| {
            let run = run.clone();
            el.child(run_box::run_button("run-open", run_box::words::OPEN_TAB, move |_, window, cx| {
                on_run(RunAction::Open(run.clone()), window, cx)
            }))
        });
    v_flex()
        .w_full()
        .debug_selector(|| "code-run-output".into())
        .when_some(output, |el, output| {
            el.child(
                div()
                    .id("run-output")
                    .w_full()
                    .max_h(RUN_BOX)
                    .overflow_y_scroll()
                    .bg(p.terminal_background)
                    .px(px(12.))
                    .py(px(8.))
                    .font_family(mono)
                    .text_size(px(12.))
                    .line_height(px(18.))
                    .text_color(p.foreground)
                    .refine_style(&tabular)
                    .child(output),
            )
        })
        .child(run_box::footer(run.words().into(), cx).child(presses))
}

/// `runOutputMessage` in packages/protocol/src/run-block.ts: the command, how it ended and the last 60 lines it printed.
pub(super) fn run_output_message(run: &Run) -> String {
    let ended = if run.state == "exited" && run.signal.is_none() {
        format!("it exited {}", run.exit_code.unwrap_or(0))
    } else {
        run.words().to_lowercase()
    };
    let output = run.output.as_deref().unwrap_or("");
    let command = &run.command;
    if output.trim().is_empty() {
        return format!("I ran `{command}` and {ended}. It printed nothing.");
    }
    let lines: Vec<&str> = output.split('\n').collect();
    let tail = lines[lines.len().saturating_sub(60)..].join("\n");
    let mut longest = 0;
    let mut current = 0;
    for c in tail.chars() {
        current = if c == '`' { current + 1 } else { 0 };
        longest = longest.max(current);
    }
    let fence = "`".repeat((longest + 1).max(3));
    format!("I ran `{command}` and {ended}. The last lines it printed:\n\n{fence}text\n{tail}\n{fence}")
}

/// The fence line's info string: what follows the backticks or tildes on the line holding byte `at`.
fn fence_info(source: &str, at: usize) -> &str {
    let at = at.min(source.len());
    let line_start = source[..at].rfind('\n').map_or(0, |ix| ix + 1);
    let line_end = source[at..].find('\n').map_or(source.len(), |ix| at + ix);
    let line = source[line_start..line_end].trim_start_matches([' ', '\t', '>']);
    let fence = line.chars().next().filter(|c| *c == '`' || *c == '~');
    fence.map_or("", |c| line.trim_start_matches(c).trim())
}

/// `extractFenceLanguage`: the info string's first word, `text` when there is none, and `ini` for `gitignore`.
pub(super) fn fence_language(info: &str, parsed: Option<SharedString>) -> SharedString {
    let word = info.split_whitespace().next().map(str::to_owned).or_else(|| parsed.map(|lang| lang.to_string()));
    match word.as_deref() {
        None | Some("") => "text".into(),
        Some("gitignore") => "ini".into(),
        Some(word) => SharedString::from(word.to_owned()),
    }
}

/// `extractFenceTitle`: a `title=`, `file=` or `filename=` in the fence's meta, else its first word shaped like a file name.
pub(super) fn fence_title(info: &str) -> Option<SharedString> {
    let meta = info.split_once(char::is_whitespace).map(|(_, meta)| meta.trim())?;
    thread_local! {
        /// Each fence meta read, so a block drawn again runs no regex; cleared past 256.
        static READ: RefCell<HashMap<String, Option<SharedString>>> = RefCell::new(HashMap::new());
    }
    if let Some(title) = READ.with(|read| read.borrow().get(meta).cloned()) {
        return title;
    }
    let title = fence_meta_title(meta);
    READ.with(|read| {
        let mut read = read.borrow_mut();
        if read.len() >= 256 {
            read.clear();
        }
        read.insert(meta.to_owned(), title.clone());
    });
    title
}

fn fence_meta_title(meta: &str) -> Option<SharedString> {
    static ATTR: OnceLock<regex::Regex> = OnceLock::new();
    static FILE: OnceLock<regex::Regex> = OnceLock::new();
    let attr =
        ATTR.get_or_init(|| regex::Regex::new(r#"(?i)(?:^|\s)(?:title|file(?:name)?)=(?:"([^"]+)"|'([^']+)'|(\S+))"#).expect("a regex"));
    if let Some(found) = attr.captures(meta) {
        return (1..=3).find_map(|ix| found.get(ix)).map(|m| SharedString::from(m.as_str().to_owned()));
    }
    let file = FILE.get_or_init(|| regex::Regex::new(r"^[\w@][\w@./-]*\.[A-Za-z0-9]+$").expect("a regex"));
    meta.split_whitespace().find(|word| file.is_match(word)).map(|word| SharedString::from(word.to_owned()))
}

/// `runnableCommand` in packages/protocol/src/run-block.ts: a shell fence's command; a console block's `$ ` lines.
pub(super) fn runnable_command(language: &str, code: &str) -> Option<String> {
    let label = language.trim().to_lowercase();
    if !["sh", "bash", "zsh", "shell", "console"].contains(&label.as_str()) {
        return None;
    }
    let text = code.replace("\r\n", "\n");
    let hidden = |c: char| {
        (c.is_control() && c != '\t' && c != '\n')
            || matches!(c, '\u{ad}' | '\u{61c}' | '\u{200b}'..='\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2060}'..='\u{2064}' | '\u{2066}'..='\u{2069}' | '\u{feff}')
    };
    if text.chars().any(hidden) {
        return None;
    }
    let prompted: Vec<&str> =
        if label == "console" { text.split('\n').filter_map(|line| line.strip_prefix("$ ")).collect() } else { Vec::new() };
    let command = if prompted.is_empty() { text.trim().to_owned() } else { prompted.join("\n").trim().to_owned() };
    (!command.is_empty()).then_some(command)
}

/// `MarkdownTable`'s footer: the expand toggle on the left, copy on the right; collapsed, cells keep to one line. Copy
/// opens "Copy as Markdown" and "Copy as CSV", then reads "Copied" with a check for 1.2 s.
pub(super) fn table_chrome(view: EntityId, ground: Option<Hsla>, table: &Arc<TableData>, window: &mut Window, cx: &mut App) -> TableChrome {
    let start = table.span.as_ref().map_or(0, |span| span.start);
    let expanded = window.use_keyed_state(block_key(TABLE_EXPANDED, view, start), cx, |_, _| WORD_WRAP);
    let copied = window.use_keyed_state(block_key(TABLE_COPIED, view, start), cx, |_, _| false);
    let (open, done) = (*expanded.read(cx), *copied.read(cx));
    let copy = |text: fn(&TableData) -> String| -> crate::ui::act::Run {
        let (copied, table) = (copied.clone(), table.clone());
        Rc::new(move |window, cx| {
            cx.write_to_clipboard(ClipboardItem::new_string(text(&table)));
            flash(&copied, Duration::from_millis(1200), window, cx);
        })
    };
    let act = |title: &'static str, run| crate::ui::MenuAct {
        group: 0,
        glyph: None,
        title: title.into(),
        destructive: false,
        held: false,
        why: None,
        keys: None,
        run,
    };
    let acts = Rc::new(vec![act("Copy as Markdown", copy(|table| table.markdown.clone())), act("Copy as CSV", copy(table_csv))]);
    let footer = h_flex()
        .justify_between()
        .child(
            ghost_button("expand", if open { Glyph::Minimize2 } else { Glyph::Maximize2 }, open, cx)
                .debug_selector(move || if open { "table-expanded" } else { "table-collapsed" }.into())
                .tooltip(crate::ui::tip(if open { "Collapse table cells" } else { "Expand table cells" }))
                .on_click(move |_, _, cx| {
                    expanded.update(cx, |expanded, cx| {
                        *expanded = !*expanded;
                        cx.notify();
                    })
                }),
        )
        .child(
            crate::ui::menu::Trigger(
                ghost_button("copy", if done { Glyph::Check } else { Glyph::Copy }, false, cx)
                    .debug_selector(move || if done { "table-copied" } else { "table-copy" }.into())
                    .tooltip(crate::ui::tip(if done { "Copied" } else { "Copy table" })),
            )
            .dropdown_menu_with_anchor(gpui_kit::Anchor::TopRight, move |menu, _, cx| {
                crate::ui::menu_of(menu, &acts, cx.palette().error_foreground, cx.palette().muted_foreground)
            }),
        );
    // The web's `mt-0.5`; the `scrollFade` mask's edges, into the surface the table stands on.
    let chrome = TableChrome::new().footer_gap(px(2.)).footer(footer);
    let chrome = match ground {
        Some(ground) => chrome.scroll_fade(ground),
        None => chrome,
    };
    if open {
        chrome
    } else {
        chrome.cell(StyleRefinement::default().whitespace_nowrap())
    }
}

/// Sets `flag` and clears it after `hold`, as a copy's check does.
fn flash(flag: &Entity<bool>, hold: Duration, window: &mut Window, cx: &mut App) {
    flag.update(cx, |flag, cx| {
        *flag = true;
        cx.notify();
    });
    let flag = flag.clone();
    window
        .spawn(cx, async move |cx| {
            cx.background_executor().timer(hold).await;
            flag.update(cx, |flag, cx| {
                *flag = false;
                cx.notify();
            });
        })
        .detach();
}

/// `serializeTableElementToCsv`: each row's cells, runs of space as one, quoted where a quote, comma or newline is in one.
fn table_csv(table: &TableData) -> String {
    #[cfg(test)]
    CSV_BUILT.with(|built| built.set(built.get() + 1));
    let cell = |value: &String| {
        let normal = value.split_whitespace().collect::<Vec<_>>().join(" ");
        if normal.contains(['"', ',', '\n']) {
            format!("\"{}\"", normal.replace('"', "\"\""))
        } else {
            normal
        }
    };
    std::iter::once(&table.headers)
        .chain(table.rows.iter())
        .filter(|row| !row.is_empty())
        .map(|row| row.iter().map(cell).collect::<Vec<_>>().join(","))
        .collect::<Vec<_>>()
        .join("\n")
}

/// The ghost `icon-xs` chrome action: 24 px, a 12 px muted glyph, the accent under the pointer, and a pressed look.
pub(crate) fn ghost_button(id: &'static str, glyph: Glyph, pressed: bool, cx: &App) -> gpui_kit::Stateful<gpui_kit::Div> {
    let p = cx.palette();
    h_flex()
        .id(id)
        .flex_none()
        .justify_center()
        .size(px(24.))
        .rounded(px(8.))
        .border_1()
        .border_color(gpui_kit::transparent_black())
        .cursor_pointer()
        .when(pressed, |el| el.bg(p.foreground.opacity(0.08)))
        .when(!pressed, |el| el.hover(|s| s.bg(p.accent)))
        .child(Icon::new(glyph).with_size(px(12.)).text_color(if pressed { p.foreground } else { p.muted_foreground }))
}

#[cfg(test)]
thread_local! {
    /// How many tables' CSV this test thread built.
    pub(super) static CSV_BUILT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}
