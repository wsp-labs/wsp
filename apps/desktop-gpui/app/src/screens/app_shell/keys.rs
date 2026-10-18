// SPDX-License-Identifier: AGPL-3.0-only
//! The shell's keyboard shortcuts, ported from the web app's keybindingDefaults.ts and keybindingOverrides.ts: the
//! default chord table with its when clauses, the person's overrides over it, and both read into GPUI key bindings.
//!
//! A when clause becomes a key context predicate over the context names below, which the shell and the screens set on
//! their elements. A clause with no positive name is anchored on [`SHELL`], so every shell binding resolves at the
//! window root's depth and a binding on a deeper element (a text field's own, a terminal's) outranks it.

use std::collections::BTreeMap;
use std::rc::Rc;

use gpui_kit::{actions, Action, DummyKeyboardMapper, KeyBinding, KeyBindingContextPredicate, Keystroke, Unbind};

use crate::ui::Glyph;

/// The window root, always.
pub const SHELL: &str = "Shell";
/// On the root too, while a thread is on screen and Settings is closed.
pub const THREAD_OPEN: &str = "ThreadOpen";
/// On the root too, while the find bar is open over a thread and Settings is closed.
pub const THREAD_FIND: &str = "ThreadFind";
/// A focused terminal.
pub const TERMINAL: &str = "Terminal";
/// The browser pane, focused.
pub const PREVIEW: &str = "Preview";
/// Focus inside the right panel while it holds more than one tab.
pub const PANEL_TABS: &str = "PanelTabs";
/// On the root too, while Settings is open: Escape closes it there.
pub const SETTINGS_OPEN: &str = "SettingsOpen";

actions!(
    shell,
    [
        ToggleSidebar,
        ToggleTerminal,
        ToggleRightPanel,
        SplitTerminal,
        NewTerminal,
        ZoomInTerminal,
        ZoomOutTerminal,
        ResetTerminalZoom,
        TogglePreview,
        ToggleCommandPalette,
        QuickOpen,
        SearchFiles,
        ToggleSettings,
        NewThread,
        PreviousWorkspace,
        NextWorkspace,
        PreviousThread,
        NextThread,
        SettleThread,
        NextNeedsYou,
        OpenInEditor,
        FindInThread,
        FindOlder,
        FindNewer,
        NextPanelTab,
        PreviousPanelTab,
        ClosePanelTab,
        CloseSettings,
    ]
);

/// Jumps to the workspace in this sidebar slot, counted from 1.
#[derive(Clone, Debug, PartialEq, Eq, Action)]
#[action(namespace = shell, no_json)]
pub struct SelectWorkspace(pub u8);

/// One step along the sidebar's tasks, or back with a negative count, at once: what the palette's Next task runs,
/// where the chord's own action opens the switcher and waits for the key to come up.
#[derive(Clone, Debug, PartialEq, Eq, Action)]
#[action(namespace = shell, no_json)]
pub struct StepWorkspace(pub i8);

/// [`StepWorkspace`] over the threads of the task on screen.
#[derive(Clone, Debug, PartialEq, Eq, Action)]
#[action(namespace = shell, no_json)]
pub struct StepThread(pub i8);

/// One row of the chord table, spelled as the web writes it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rule<'a> {
    pub command: &'a str,
    pub chord: &'a str,
    pub when: Option<&'a str>,
}

const fn rule(chord: &'static str, command: &'static str, when: Option<&'static str>) -> Rule<'static> {
    Rule { command, chord, when }
}

/// DEFAULT_KEYBINDINGS in keybindingDefaults.ts, row for row; a test reads that file and fails when the two part.
pub const DEFAULTS: &[Rule<'static>] = &[
    rule("mod+shift+b", "sidebar.toggle", None),
    rule("mod+b", "sidebar.toggle", Some("!terminalOwnsMod")),
    rule("mod+j", "terminal.toggle", None),
    rule("mod+alt+b", "rightPanel.toggle", None),
    rule("mod+d", "terminal.split", Some("terminalFocus")),
    rule("mod+n", "terminal.new", Some("terminalFocus")),
    rule("mod+=", "terminal.zoomIn", Some("terminalFocus")),
    rule("mod+shift+=", "terminal.zoomIn", Some("terminalFocus")),
    rule("mod+-", "terminal.zoomOut", Some("terminalFocus")),
    rule("mod+0", "terminal.zoomReset", Some("terminalFocus")),
    rule("mod+shift+j", "preview.toggle", None),
    rule("mod+shift+p", "commandPalette.toggle", None),
    rule("mod+k", "commandPalette.toggle", Some("!terminalOwnsMod")),
    rule("mod+p", "files.quickOpen", Some("!terminalOwnsMod")),
    rule("mod+shift+f", "files.search", None),
    rule("mod+,", "settings.toggle", None),
    rule("mod+t", "chat.new", Some("!terminalFocus")),
    rule("mod+alt+arrowleft", "workspace.previous", Some("!terminalFocus")),
    rule("mod+alt+arrowright", "workspace.next", Some("!terminalFocus")),
    rule("mod+alt+arrowup", "thread.previous", Some("!terminalFocus")),
    rule("mod+alt+arrowdown", "thread.next", Some("!terminalFocus")),
    rule("mod+shift+e", "thread.settle", Some("!terminalFocus")),
    rule("mod+alt+u", "thread.nextNeedsYou", Some("!terminalFocus")),
    rule("mod+o", "editor.open", Some("!terminalOwnsMod")),
    rule("mod+f", "thread.find", Some("threadOpen && !terminalFocus && !previewFocus")),
    rule("mod+g", "thread.findOlder", Some("threadFindOpen && !terminalFocus")),
    rule("f3", "thread.findOlder", Some("threadFindOpen && !terminalFocus")),
    rule("mod+shift+g", "thread.findNewer", Some("threadFindOpen && !terminalFocus")),
    rule("shift+f3", "thread.findNewer", Some("threadFindOpen && !terminalFocus")),
    rule("ctrl+tab", "workspace.next", Some("!terminalFocus && !panelTabsFocus")),
    rule("ctrl+shift+tab", "workspace.previous", Some("!terminalFocus && !panelTabsFocus")),
    rule("ctrl+tab", "rightPanel.nextTab", Some("panelTabsFocus")),
    rule("ctrl+shift+tab", "rightPanel.previousTab", Some("panelTabsFocus")),
    rule("mod+w", "rightPanel.closeTab", Some("previewFocus")),
    rule("mod+1", "workspace.select.1", None),
    rule("mod+2", "workspace.select.2", None),
    rule("mod+3", "workspace.select.3", None),
    rule("mod+4", "workspace.select.4", None),
    rule("mod+5", "workspace.select.5", None),
    rule("mod+6", "workspace.select.6", None),
    rule("mod+7", "workspace.select.7", None),
    rule("mod+8", "workspace.select.8", None),
    rule("mod+9", "workspace.select.9", None),
];

/// The action a command id runs, or None for an id this shell does not dispatch.
pub fn action(command: &str) -> Option<Box<dyn Action>> {
    if let Some(slot) = command.strip_prefix("workspace.select.") {
        return Some(Box::new(SelectWorkspace(slot.parse().ok()?)));
    }
    Some(match command {
        "sidebar.toggle" => Box::new(ToggleSidebar),
        "terminal.toggle" => Box::new(ToggleTerminal),
        "rightPanel.toggle" => Box::new(ToggleRightPanel),
        "terminal.split" => Box::new(SplitTerminal),
        "terminal.new" => Box::new(NewTerminal),
        "terminal.zoomIn" => Box::new(ZoomInTerminal),
        "terminal.zoomOut" => Box::new(ZoomOutTerminal),
        "terminal.zoomReset" => Box::new(ResetTerminalZoom),
        "preview.toggle" => Box::new(TogglePreview),
        "commandPalette.toggle" => Box::new(ToggleCommandPalette),
        "files.quickOpen" => Box::new(QuickOpen),
        "files.search" => Box::new(SearchFiles),
        "settings.toggle" => Box::new(ToggleSettings),
        "chat.new" => Box::new(NewThread),
        "workspace.previous" => Box::new(PreviousWorkspace),
        "workspace.next" => Box::new(NextWorkspace),
        "thread.previous" => Box::new(PreviousThread),
        "thread.next" => Box::new(NextThread),
        "thread.settle" => Box::new(SettleThread),
        "thread.nextNeedsYou" => Box::new(NextNeedsYou),
        "editor.open" => Box::new(OpenInEditor),
        "thread.find" => Box::new(FindInThread),
        "thread.findOlder" => Box::new(FindOlder),
        "thread.findNewer" => Box::new(FindNewer),
        "rightPanel.nextTab" => Box::new(NextPanelTab),
        "rightPanel.previousTab" => Box::new(PreviousPanelTab),
        "rightPanel.closeTab" => Box::new(ClosePanelTab),
        _ => return None,
    })
}

/// A web chord (`mod+shift+b`) as a GPUI keystroke (`cmd-shift-b`), or None where the web parser refuses it or GPUI
/// would read it as another key. `mod` is Command on macOS and Control elsewhere, `meta` is Command or Super.
pub fn keystroke(chord: &str, mac: bool) -> Option<String> {
    let lower = chord.to_lowercase();
    let mut tokens: Vec<&str> = lower.split('+').map(str::trim).collect();
    let trailing = tokens.iter().rev().take_while(|token| token.is_empty()).count();
    tokens.truncate(tokens.len() - trailing);
    if trailing > 0 {
        tokens.push("+");
    }
    if tokens.iter().any(|token| token.is_empty()) {
        return None;
    }
    let (mut platform, mut ctrl, mut alt, mut shift, mut key) = (false, false, false, false, None);
    for token in tokens {
        match token {
            "cmd" | "meta" => platform = true,
            "ctrl" | "control" => ctrl = true,
            "shift" => shift = true,
            "alt" | "option" => alt = true,
            "mod" if mac => platform = true,
            "mod" => ctrl = true,
            _ if key.is_some() => return None,
            other => key = Some(other),
        }
    }
    let key = match key? {
        "esc" => "escape",
        "arrowup" => "up",
        "arrowdown" => "down",
        "arrowleft" => "left",
        "arrowright" => "right",
        other => other,
    };
    let text = format!(
        "{}{}{}{}{key}",
        if platform && mac {
            "cmd-"
        } else if platform {
            "super-"
        } else {
            ""
        },
        if ctrl { "ctrl-" } else { "" },
        if alt { "alt-" } else { "" },
        if shift { "shift-" } else { "" },
    );
    let read = Keystroke::parse(&text).ok()?;
    (read.key == key && read.key_char.is_none()).then_some(text)
}

/// A when clause as a GPUI context predicate over the names above. `terminalOwnsMod` is a focused terminal off macOS,
/// where mod is Control and the shell reads it. Panics on a clause outside the table's grammar (names joined by `&&`,
/// each maybe negated); the tests run every default through it.
pub fn predicate(when: Option<&str>, mac: bool) -> String {
    let (mut held, mut barred) = (Vec::new(), Vec::new());
    for term in when.into_iter().flat_map(|when| when.split("&&")).map(str::trim) {
        let (negated, name) = term.strip_prefix('!').map_or((false, term), |name| (true, name.trim()));
        let context = match name {
            "terminalOwnsMod" if mac && negated => continue,
            "terminalOwnsMod" if !mac => TERMINAL,
            "terminalFocus" => TERMINAL,
            "previewFocus" => PREVIEW,
            "panelTabsFocus" => PANEL_TABS,
            "threadOpen" => THREAD_OPEN,
            "threadFindOpen" => THREAD_FIND,
            _ => panic!("no key context for `{term}`"),
        };
        if negated {
            barred.push(context)
        } else {
            held.push(context)
        }
    }
    if held.is_empty() {
        held.push(SHELL);
    }
    // GPUI checks every name a conjunction holds on one element, so only the root's names can share one.
    let on_root = |context: &&str| [SHELL, THREAD_OPEN, THREAD_FIND].contains(context);
    assert!(held.len() == 1 || held.iter().all(on_root), "`{when:?}` needs names on two elements");
    let mut text = held.join(" && ");
    for context in barred {
        text.push_str(" && !");
        text.push_str(context);
    }
    text
}

/// rulesWith from keybindingOverrides.ts: an override for a command replaces all of that command's defaults and takes
/// the when of its default on the same chord, else of its first default. An override for a command the table does not
/// hold, or whose chord does not read, is left out; a None is no override.
pub fn rules_with<'a>(mac: bool, overrides: &'a BTreeMap<String, Option<String>>) -> Vec<Rule<'a>> {
    let moved: Vec<(&str, &str)> = overrides
        .iter()
        .filter_map(|(command, chord)| Some((command.as_str(), chord.as_deref()?)))
        .filter(|(command, chord)| DEFAULTS.iter().any(|rule| rule.command == *command) && keystroke(chord, mac).is_some())
        .collect();
    let kept = DEFAULTS.iter().copied().filter(|rule| !moved.iter().any(|(command, _)| *command == rule.command));
    let added = moved.iter().map(|&(command, chord)| {
        let on = keystroke(chord, mac);
        let defaults = || DEFAULTS.iter().filter(move |rule| rule.command == command);
        let when = defaults().find(|rule| keystroke(rule.chord, mac) == on).or_else(|| defaults().next()).and_then(|rule| rule.when);
        Rule { command, chord, when }
    });
    kept.chain(added).collect()
}

/// A rule as bound: its command and when are the table's own, its chord may be the person's.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bound {
    pub command: &'static str,
    pub chord: String,
    pub when: Option<&'static str>,
}

/// [`rules_with`], held apart from the overrides it was read from.
pub fn bound(mac: bool, overrides: &BTreeMap<String, Option<String>>) -> Vec<Bound> {
    rules_with(mac, overrides)
        .into_iter()
        .filter_map(|rule| {
            let own = DEFAULTS.iter().find(|d| d.command == rule.command && d.when == rule.when)?;
            Some(Bound { command: own.command, chord: rule.chord.to_owned(), when: own.when })
        })
        .collect()
}

/// TERMINAL_PASSES in keybindings.ts: the commands a focused terminal hands the app on a Control chord.
const TERMINAL_PASSES: &[&str] = &[
    "terminal.toggle",
    "commandPalette.toggle",
    "rightPanel.nextTab",
    "rightPanel.previousTab",
    "settings.toggle",
    "sidebar.toggle",
    "preview.toggle",
    "files.search",
    "terminal.zoomIn",
    "terminal.zoomOut",
    "terminal.zoomReset",
];

/// isTerminalAppShortcut in keybindings.ts: a Command or Super chord leaves a focused terminal, a Control chord only
/// for a command it passes and off the letters SHELL_KEEPS names.
fn leaves_terminal(command: &str, typed: &Keystroke) -> bool {
    let m = &typed.modifiers;
    let passes = TERMINAL_PASSES.contains(&command) || command.starts_with("workspace.select.");
    let kept = ["c", "d", "r", "k", "l", "a", "e", "w", "u", "z", "m", "i", "h", "["].contains(&typed.key.as_str()) && !m.shift && !m.alt;
    m.platform || (m.control && passes && !kept)
}

fn binding(mac: bool, rule: &Bound, action: Box<dyn Action>) -> Option<KeyBinding> {
    let keystroke = keystroke(&rule.chord, mac)?;
    let typed = Keystroke::parse(&keystroke).ok()?;
    let mut when = predicate(rule.when, mac);
    if !leaves_terminal(rule.command, &typed) {
        when.push_str(&format!(" && !{TERMINAL}"));
    }
    // KeybindingDispatcher.tsx leaves a key with neither mod nor Control, an F key aside, to the focused field.
    let function = typed.key.strip_prefix('f').is_some_and(|n| n.parse::<u8>().is_ok());
    if !(typed.modifiers.control || typed.modifiers.platform || function) {
        when.push_str(" && !Input");
    }
    let context = KeyBindingContextPredicate::parse(&when).ok()?;
    KeyBinding::load(&keystroke, action, Some(Rc::new(context)), false, None, &DummyKeyboardMapper).ok()
}

/// The key bindings for `rules`, in their order.
pub fn bindings_of(mac: bool, rules: &[Bound]) -> Vec<KeyBinding> {
    rules.iter().filter_map(|rule| binding(mac, rule, action(rule.command)?)).collect()
}

/// What takes `rules` back out of the keymap: GPUI's `Unbind` of each rule's action on its chord and context, which
/// disables that binding and leaves any bound after it alone.
pub fn unbindings(mac: bool, rules: &[Bound]) -> Vec<KeyBinding> {
    rules
        .iter()
        .filter_map(|rule| {
            let name = action(rule.command)?.name();
            binding(mac, rule, Box::new(Unbind(name.into())))
        })
        .collect()
}

/// The key bindings for a platform with the person's overrides in place, in [`rules_with`]'s order: GPUI lets a later
/// binding win over an earlier one at the same context depth, as the web lets the last matching rule win.
pub fn bindings(mac: bool, overrides: &BTreeMap<String, Option<String>>) -> Vec<KeyBinding> {
    bindings_of(mac, &bound(mac, overrides))
}

/// The web's label for a chord (`formatShortcutLabel` in keybindings.ts): ⌃⌥⇧⌘ glued before the key on a Mac, the
/// modifiers' names joined by `+` elsewhere.
pub fn label(chord: &str, mac: bool) -> Option<String> {
    let typed = Keystroke::parse(&keystroke(chord, mac)?).ok()?;
    let m = &typed.modifiers;
    let key = match typed.key.as_str() {
        "space" => "Space".to_owned(),
        "escape" => "Esc".to_owned(),
        k if k.chars().count() == 1 => k.to_uppercase(),
        k => {
            let mut chars = k.chars();
            chars.next().map(|c| c.to_uppercase().chain(chars).collect()).unwrap_or_default()
        }
    };
    if mac {
        let marks = [(m.control, "⌃"), (m.alt, "⌥"), (m.shift, "⇧"), (m.platform, "⌘")];
        return Some(marks.iter().filter(|(on, _)| *on).map(|(_, mark)| *mark).chain([key.as_str()]).collect());
    }
    let names = [(m.control, "Ctrl"), (m.alt, "Alt"), (m.shift, "Shift"), (m.platform, "Meta")];
    Some(names.iter().filter(|(on, _)| *on).map(|(_, name)| *name).chain([key.as_str()]).collect::<Vec<_>>().join("+"))
}

/// `shortcutLabelForCommand` as a palette row reads it: the last rule bound for `command` whose when clause holds with
/// nothing focused, skipping a chord a later rule took.
pub fn shortcut_label(rules: &[Bound], command: &str, mac: bool) -> Option<String> {
    let mut claimed: Vec<String> = Vec::new();
    for rule in rules.iter().rev() {
        let at_rest = rule.when.is_none_or(|when| when.split("&&").all(|term| term.trim().starts_with('!')));
        let Some(typed) = keystroke(&rule.chord, mac).filter(|_| at_rest) else { continue };
        if claimed.contains(&typed) {
            continue;
        }
        if rule.command == command {
            return label(&rule.chord, mac);
        }
        claimed.push(typed);
    }
    None
}

/// Why a palette row is held, in the words it says under its title (`paletteItems.tsx`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Held {
    NotConnected,
    NoProject,
    /// The page of projects with none to pick, or no host to make a thread on.
    NoPick,
    OneTask,
    OneThread,
    /// No task on screen, so no threads to walk.
    NoThreads,
    /// The task on screen, by name, holds nothing to step to.
    NothingToStep(String),
    NoTask,
    NoProjectOpen,
}

impl Held {
    pub fn words(&self) -> String {
        match self {
            Held::NotConnected => "Not connected to the runtime".to_owned(),
            Held::NoProject => "Add a project first".to_owned(),
            Held::NoPick => "Pick the project first".to_owned(),
            Held::OneTask => "Only one task".to_owned(),
            Held::OneThread => "Only one thread".to_owned(),
            Held::NoThreads => "No threads to walk".to_owned(),
            Held::NothingToStep(task) => format!("Nothing to step to on {task}; a walk stays inside one task"),
            Held::NoTask => "Select a task first".to_owned(),
            Held::NoProjectOpen => "Open a project first".to_owned(),
        }
    }
}

/// What a command needs to run, which the palette reads off the window to hold the row or leave it out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Need {
    Nothing,
    /// A host to make a thread on and a project to make it in: [`Held::NotConnected`], then [`Held::NoProject`].
    Create,
    /// The same, said as the page's own line: [`Held::NoPick`].
    Pick,
    /// Two tasks to step between: [`Held::OneTask`].
    Tasks,
    /// Two threads in the task on screen: [`Held::OneThread`], [`Held::NoThreads`] or [`Held::NothingToStep`].
    Walk,
    /// A task on screen: [`Held::NoTask`].
    Task,
    /// A project on screen: [`Held::NoProjectOpen`].
    Project,
    /// A thread on screen; with none the row is not listed at all.
    Thread,
}

/// What a row says under its title while it is not held.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Says {
    Nothing,
    Words(&'static str),
    /// The task on screen's name.
    Task,
    /// The project on screen's name.
    Project,
}

/// What running a command does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Run {
    /// The chord command's own action, dispatched to the shell.
    Command(&'static str),
    /// One step along the tasks at once; never the chord's own action, which opens the switcher and waits for the
    /// key to come up.
    StepWorkspace(i8),
    StepThread(i8),
    /// A screen by its route.
    Screen(&'static str),
    /// An address, opened in the system's browser.
    Url(&'static str),
    /// New thread: the composer on its project, or the page of projects where it asks for one.
    NewThread,
    /// Opens the page of projects New thread picks from.
    PickProject,
    AddProject,
    /// Add a computer: the web opens it over Settings' Computers page.
    AddComputer,
    CopyMarkdown,
    ResumeConversation,
}

/// The list a command's row stands in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
    Actions,
    /// The last row of the page of projects.
    Projects,
}

/// One command as the palette lists it (`paletteItems.tsx`): its row's words and glyph, what it needs, the chord its
/// row names and what it runs. The chordless commands sit here too, so the palette has one table.
#[derive(Clone, Copy, Debug)]
pub struct Command {
    pub id: &'static str,
    pub title: &'static str,
    pub terms: &'static [&'static str],
    pub glyph: Glyph,
    pub group: Group,
    pub says: Says,
    pub need: Need,
    /// The palette stays open after the run, as New thread's does while it asks for the project.
    pub keep_open: bool,
    /// The command in [`DEFAULTS`] whose chord the row names.
    pub chord: Option<&'static str>,
    pub run: Run,
}

const fn command(id: &'static str, title: &'static str, terms: &'static [&'static str], glyph: Glyph, run: Run) -> Command {
    Command { id, title, terms, glyph, group: Group::Actions, says: Says::Nothing, need: Need::Nothing, keep_open: false, chord: None, run }
}

const fn says(mut c: Command, says: Says) -> Command {
    c.says = says;
    c
}

const fn needs(mut c: Command, need: Need) -> Command {
    c.need = need;
    c
}

const fn chord(mut c: Command, chord: &'static str) -> Command {
    c.chord = Some(chord);
    c
}

/// wsp's repository, `REPO` in `packages/protocol/src/bundles.mjs`.
pub const REPO: &str = "https://github.com/wsp-labs/wsp";

/// The palette's commands in the order its Actions list them. The open thread's moves and the open task's own acts
/// join the list before Next task.
pub const COMMANDS: &[Command] = &[
    needs(
        says(
            command("action:new-thread", "New thread", &["new thread", "new chat", "new task", "create task"], Glyph::Plus, Run::NewThread),
            Says::Words("One piece of work on one project"),
        ),
        Need::Create,
    ),
    needs(
        says(
            command(
                "page:new-thread",
                "New thread in...",
                &["new thread in", "new thread on", "pick a project", "new chat in"],
                Glyph::FolderOpen,
                Run::PickProject,
            ),
            Says::Words("Pick the project first"),
        ),
        Need::Pick,
    ),
    needs(
        says(
            command(
                "action:resume-conversation",
                "Resume a conversation",
                &["resume a conversation", "pick up a conversation", "claude code", "codex"],
                Glyph::RotateCcwClock,
                Run::ResumeConversation,
            ),
            Says::Project,
        ),
        Need::Project,
    ),
    says(
        command(
            "action:add-project",
            "Add a project",
            &["add a project", "project", "folder", "repository", "clone", "record a project"],
            Glyph::FolderPlus,
            Run::AddProject,
        ),
        Says::Words("One folder or repository address on one computer"),
    ),
    needs(
        says(
            command(
                "action:copy-markdown",
                "Copy as Markdown",
                &["copy", "markdown", "export thread", "copy chat"],
                Glyph::FileText,
                Run::CopyMarkdown,
            ),
            Says::Words("The thread on screen, as the host holds it"),
        ),
        Need::Thread,
    ),
    chord(
        needs(
            command(
                "action:next-workspace",
                "Next task",
                &["next task", "switch task", "cycle tasks"],
                Glyph::ArrowDown,
                Run::StepWorkspace(1),
            ),
            Need::Tasks,
        ),
        "workspace.next",
    ),
    chord(
        needs(
            command(
                "action:previous-workspace",
                "Previous task",
                &["previous task", "switch task", "cycle tasks"],
                Glyph::ArrowUp,
                Run::StepWorkspace(-1),
            ),
            Need::Tasks,
        ),
        "workspace.previous",
    ),
    chord(
        needs(
            command(
                "action:next-thread",
                "Next thread",
                &["next thread", "switch thread", "cycle threads"],
                Glyph::ChevronDown,
                Run::StepThread(1),
            ),
            Need::Walk,
        ),
        "thread.next",
    ),
    chord(
        needs(
            command(
                "action:previous-thread",
                "Previous thread",
                &["previous thread", "switch thread", "cycle threads"],
                Glyph::ChevronUp,
                Run::StepThread(-1),
            ),
            Need::Walk,
        ),
        "thread.previous",
    ),
    chord(
        command(
            "action:toggle-sidebar",
            "Toggle sidebar",
            &["toggle sidebar", "hide sidebar", "show sidebar"],
            Glyph::PanelLeft,
            Run::Command("sidebar.toggle"),
        ),
        "sidebar.toggle",
    ),
    says(
        command(
            "action:add-computer",
            "Add a computer",
            &["add a computer", "join", "box", "another mac", "laptop", "linux", "ssh"],
            Glyph::Monitor,
            Run::AddComputer,
        ),
        Says::Words("Computers in Settings"),
    ),
    chord(
        says(
            command(
                "action:settings",
                "Settings",
                &[
                    "settings",
                    "preferences",
                    "computers",
                    "projects",
                    "devices",
                    "account",
                    "privacy",
                    "keybindings",
                    "appearance",
                    "theme",
                    "sidebar width",
                    "terminal size",
                ],
                Glyph::Settings,
                Run::Screen("settings"),
            ),
            Says::Words(
                "General, Appearance, Agents, Computers, Recipes, Projects, Usage, Activity, Keybindings, Privacy, Devices, Account",
            ),
        ),
        "settings.toggle",
    ),
    says(
        command(
            "action:activity",
            "Activity",
            &["activity", "task manager", "processes", "memory", "cpu", "power", "ports", "end a process"],
            Glyph::Activity,
            Run::Screen("settings:activity"),
        ),
        Says::Words("Every thread's processes, memory and CPU in Settings"),
    ),
    says(
        command(
            "action:github",
            "wsp on GitHub",
            &["github", "source", "repo", "about", "licence", "license"],
            Glyph::Github,
            Run::Url(REPO),
        ),
        Says::Words("github.com/wsp-labs/wsp"),
    ),
    says(
        command(
            "action:docs",
            "wsp docs",
            &["docs", "documentation", "help", "manual"],
            Glyph::BookOpen,
            Run::Url("https://usewsp.com/docs"),
        ),
        Says::Words("usewsp.com/docs"),
    ),
    says(
        command(
            "action:report-bug",
            "Report a bug",
            &["bug", "issue", "report", "feedback", "help"],
            Glyph::Bug,
            Run::Url("https://github.com/wsp-labs/wsp/issues/new"),
        ),
        Says::Words("github.com/wsp-labs/wsp/issues/new"),
    ),
    chord(
        needs(
            says(
                command(
                    "action:toggle-right-panel",
                    "Toggle right panel",
                    &["toggle right panel", "hide panel", "show panel"],
                    Glyph::PanelRight,
                    Run::Command("rightPanel.toggle"),
                ),
                Says::Task,
            ),
            Need::Task,
        ),
        "rightPanel.toggle",
    ),
    Command {
        group: Group::Projects,
        ..command(
            "action:new-project",
            "Add a project",
            &["new project", "add a project", "folder", "repository", "clone"],
            Glyph::Plus,
            Run::AddProject,
        )
    },
];

/// The command whose row has the id `id`.
pub fn command_of(id: &str) -> Option<&'static Command> {
    COMMANDS.iter().find(|c| c.id == id)
}

/// The action a command's run dispatches to the shell, where it is one.
pub fn run_action(run: Run) -> Option<Box<dyn Action>> {
    match run {
        Run::Command(command) => action(command),
        Run::StepWorkspace(by) => Some(Box::new(StepWorkspace(by))),
        Run::StepThread(by) => Some(Box::new(StepThread(by))),
        Run::NewThread => Some(Box::new(NewThread)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui_kit::{KeyContext, Keymap};

    fn overrides(entries: &[(&str, Option<&str>)]) -> BTreeMap<String, Option<String>> {
        entries.iter().map(|(command, chord)| (command.to_string(), chord.map(str::to_owned))).collect()
    }

    fn fired(keymap: &Keymap, typed: &str, contexts: &[&str]) -> Option<String> {
        let stack: Vec<KeyContext> = contexts.iter().map(|context| KeyContext::parse(context).unwrap()).collect();
        let (found, _) = keymap.bindings_for_input(&[Keystroke::parse(typed).unwrap()], &stack);
        found.first().map(|binding| binding.action().name().trim_start_matches("shell::").to_owned())
    }

    #[test]
    fn every_default_binds_on_both_platforms() {
        for mac in [true, false] {
            for rule in DEFAULTS {
                assert!(keystroke(rule.chord, mac).is_some(), "{rule:?} mac={mac}");
                assert!(action(rule.command).is_some(), "{rule:?}");
            }
            assert_eq!(bindings(mac, &BTreeMap::new()).len(), DEFAULTS.len(), "mac={mac}");
        }
    }

    #[test]
    fn reads_chords_per_platform() {
        let cases = [
            ("mod+shift+b", "cmd-shift-b", "ctrl-shift-b"),
            ("mod+alt+arrowleft", "cmd-alt-left", "ctrl-alt-left"),
            ("ctrl+tab", "ctrl-tab", "ctrl-tab"),
            ("ctrl+shift+tab", "ctrl-shift-tab", "ctrl-shift-tab"),
            ("mod+=", "cmd-=", "ctrl-="),
            ("mod+-", "cmd--", "ctrl--"),
            ("mod+,", "cmd-,", "ctrl-,"),
            ("shift+f3", "shift-f3", "shift-f3"),
            ("mod+1", "cmd-1", "ctrl-1"),
            ("meta+k", "cmd-k", "super-k"),
            ("Control+Option+Esc", "ctrl-alt-escape", "ctrl-alt-escape"),
            ("mod++", "cmd-+", "ctrl-+"),
            ("cmd + space", "cmd-space", "super-space"),
        ];
        for (chord, mac, other) in cases {
            assert_eq!(keystroke(chord, true).as_deref(), Some(mac), "{chord} on mac");
            assert_eq!(keystroke(chord, false).as_deref(), Some(other), "{chord} off mac");
        }
        let mac = Keystroke::parse(&keystroke("mod+b", true).unwrap()).unwrap().modifiers;
        let other = Keystroke::parse(&keystroke("mod+b", false).unwrap()).unwrap().modifiers;
        assert!(mac.platform && !mac.control);
        assert!(other.control && !other.platform);
    }

    #[test]
    fn refuses_chords_that_do_not_read() {
        for chord in ["mod", "mod+shift", "mod+a+b", "mod++b", "mod+fn", "mod+a-b"] {
            assert_eq!(keystroke(chord, false), None, "{chord}");
        }
    }

    #[test]
    fn turns_each_when_clause_into_a_predicate() {
        let cases = [
            (None, "Shell", "Shell"),
            (Some("!terminalOwnsMod"), "Shell", "Shell && !Terminal"),
            (Some("terminalFocus"), "Terminal", "Terminal"),
            (Some("!terminalFocus"), "Shell && !Terminal", "Shell && !Terminal"),
            (
                Some("threadOpen && !terminalFocus && !previewFocus"),
                "ThreadOpen && !Terminal && !Preview",
                "ThreadOpen && !Terminal && !Preview",
            ),
            (Some("threadFindOpen && !terminalFocus"), "ThreadFind && !Terminal", "ThreadFind && !Terminal"),
            (Some("!terminalFocus && !panelTabsFocus"), "Shell && !Terminal && !PanelTabs", "Shell && !Terminal && !PanelTabs"),
            (Some("panelTabsFocus"), "PanelTabs", "PanelTabs"),
            (Some("previewFocus"), "Preview", "Preview"),
        ];
        for (when, mac, other) in cases {
            assert_eq!(predicate(when, true), mac, "{when:?} on mac");
            assert_eq!(predicate(when, false), other, "{when:?} off mac");
            assert!(KeyBindingContextPredicate::parse(mac).is_ok() && KeyBindingContextPredicate::parse(other).is_ok());
        }
        for rule in DEFAULTS {
            assert!(cases.iter().any(|(when, _, _)| *when == rule.when), "no predicate case for {:?}", rule.when);
        }
    }

    #[test]
    fn resolves_chords_in_the_focus_they_name() {
        let other = Keymap::new(bindings(false, &BTreeMap::new()));
        assert_eq!(fired(&other, "ctrl-tab", &["Shell"]).as_deref(), Some("NextWorkspace"));
        assert_eq!(fired(&other, "ctrl-tab", &["Shell", "PanelTabs"]).as_deref(), Some("NextPanelTab"));
        assert_eq!(fired(&other, "ctrl-tab", &["Shell", "PanelTabs", "Terminal"]).as_deref(), Some("NextPanelTab"));
        assert_eq!(fired(&other, "ctrl-tab", &["Shell", "Terminal"]), None);
        assert_eq!(fired(&other, "ctrl-b", &["Shell"]).as_deref(), Some("ToggleSidebar"));
        assert_eq!(fired(&other, "ctrl-b", &["Shell", "Terminal"]), None);
        assert_eq!(fired(&other, "ctrl-shift-b", &["Shell", "Terminal"]).as_deref(), Some("ToggleSidebar"));
        assert_eq!(fired(&other, "ctrl-f", &["Shell"]), None);
        assert_eq!(fired(&other, "ctrl-f", &["Shell ThreadOpen"]).as_deref(), Some("FindInThread"));
        assert_eq!(fired(&other, "ctrl-f", &["Shell ThreadOpen", "Preview"]), None);
        assert_eq!(fired(&other, "shift-f3", &["Shell ThreadFind"]).as_deref(), Some("FindNewer"));
        let mac = Keymap::new(bindings(true, &BTreeMap::new()));
        assert_eq!(fired(&mac, "cmd-b", &["Shell", "Terminal"]).as_deref(), Some("ToggleSidebar"));
        assert_eq!(fired(&mac, "ctrl-b", &["Shell"]), None);
        let stack = [KeyContext::parse("Shell").unwrap()];
        let (found, _) = mac.bindings_for_input(&[Keystroke::parse("cmd-3").unwrap()], &stack);
        assert!(found[0].action().partial_eq(&SelectWorkspace(3)));
    }

    #[test]
    fn an_override_replaces_every_default_of_its_command() {
        let moved = overrides(&[("sidebar.toggle", Some("mod+shift+s"))]);
        let rules = rules_with(false, &moved);
        let kept: Vec<Rule> = DEFAULTS.iter().copied().filter(|rule| rule.command != "sidebar.toggle").collect();
        assert_eq!(rules[..kept.len()], kept[..]);
        assert_eq!(rules[kept.len()..], [Rule { command: "sidebar.toggle", chord: "mod+shift+s", when: None }]);
    }

    #[test]
    fn an_override_takes_the_when_of_its_default_on_that_chord() {
        let back = overrides(&[("sidebar.toggle", Some("ctrl+b"))]);
        assert_eq!(rules_with(false, &back).last().unwrap().when, Some("!terminalOwnsMod"));
        assert_eq!(rules_with(true, &back).last().unwrap().when, None);
        let off = overrides(&[("thread.findOlder", Some("f4")), ("terminal.zoomIn", Some("mod+i"))]);
        let rules = rules_with(false, &off);
        let when_of = |command: &str| rules.iter().find(|rule| rule.command == command).unwrap().when;
        assert_eq!(when_of("thread.findOlder"), Some("threadFindOpen && !terminalFocus"));
        assert_eq!(when_of("terminal.zoomIn"), Some("terminalFocus"));
    }

    #[test]
    fn drops_unknown_commands_bad_chords_and_nulls() {
        for moved in [
            overrides(&[("nope.toggle", Some("mod+y"))]),
            overrides(&[("workspace.select.10", Some("mod+y"))]),
            overrides(&[("sidebar.toggle", Some("mod+a+b"))]),
            overrides(&[("sidebar.toggle", None)]),
        ] {
            for mac in [true, false] {
                assert_eq!(rules_with(mac, &moved), DEFAULTS, "{moved:?}");
            }
        }
    }

    #[test]
    fn a_later_override_wins_its_chord_at_the_same_depth() {
        let keymap =
            Keymap::new(bindings(false, &overrides(&[("files.search", Some("mod+b")), ("workspace.select.3", Some("mod+shift+3"))])));
        assert_eq!(fired(&keymap, "ctrl-b", &["Shell"]).as_deref(), Some("SearchFiles"));
        assert_eq!(fired(&keymap, "ctrl-3", &["Shell"]), None);
        assert_eq!(fired(&keymap, "ctrl-shift-3", &["Shell"]).as_deref(), Some("SelectWorkspace"));
    }

    #[test]
    fn a_deeper_context_outranks_a_later_override() {
        let keymap = Keymap::new(bindings(true, &overrides(&[("files.search", Some("mod+d"))])));
        assert_eq!(fired(&keymap, "cmd-d", &["Shell"]).as_deref(), Some("SearchFiles"));
        assert_eq!(fired(&keymap, "cmd-d", &["Shell", "Terminal"]).as_deref(), Some("SplitTerminal"));
    }

    #[test]
    fn a_focused_terminal_keeps_the_control_chords_keybindings_ts_leaves_it() {
        let other = Keymap::new(bindings(false, &BTreeMap::new()));
        for typed in ["ctrl-d", "ctrl-n", "ctrl-alt-b"] {
            assert_eq!(fired(&other, typed, &["Shell", "Terminal"]), None, "{typed}");
        }
        assert_eq!(fired(&other, "ctrl-j", &["Shell", "Terminal"]).as_deref(), Some("ToggleTerminal"));
    }

    #[test]
    fn a_chord_without_mod_or_control_is_the_focused_fields() {
        let keymap = Keymap::new(bindings(false, &overrides(&[("chat.new", Some("shift+t"))])));
        assert_eq!(fired(&keymap, "shift-t", &["Shell"]).as_deref(), Some("NewThread"));
        assert_eq!(fired(&keymap, "shift-t", &["Shell", "Input"]), None);
    }

    #[test]
    fn a_rows_label_is_the_last_chord_bound_at_rest() {
        let rules = bound(false, &BTreeMap::new());
        let mac_rules = bound(true, &BTreeMap::new());
        let cases = [
            ("workspace.next", "Ctrl+Tab", "⌃Tab"),
            ("thread.next", "Ctrl+Alt+Down", "⌥⌘Down"),
            ("sidebar.toggle", "Ctrl+B", "⌘B"),
            ("rightPanel.toggle", "Ctrl+Alt+B", "⌥⌘B"),
            ("settings.toggle", "Ctrl+,", "⌘,"),
            ("workspace.select.3", "Ctrl+3", "⌘3"),
            ("commandPalette.toggle", "Ctrl+K", "⌘K"),
        ];
        for (command, other, mac) in cases {
            assert_eq!(shortcut_label(&rules, command, false).as_deref(), Some(other), "{command}");
            assert_eq!(shortcut_label(&mac_rules, command, true).as_deref(), Some(mac), "{command} on mac");
        }
        assert_eq!(shortcut_label(&rules, "thread.find", false), None, "a chord bound only with a thread open names no row");
        let moved = bound(false, &overrides(&[("files.search", Some("mod+b"))]));
        assert_eq!(shortcut_label(&moved, "sidebar.toggle", false).as_deref(), Some("Ctrl+Shift+B"), "a later rule took mod+b");
        assert_eq!(label("mod+alt+shift+arrowleft", true).as_deref(), Some("⌥⇧⌘Left"));
        assert_eq!(label("shift+f3", false).as_deref(), Some("Shift+F3"));
    }

    #[test]
    fn the_palettes_steps_go_at_once_and_never_open_the_switcher() {
        for (id, step, chord) in [
            ("action:next-workspace", Run::StepWorkspace(1), "workspace.next"),
            ("action:previous-workspace", Run::StepWorkspace(-1), "workspace.previous"),
            ("action:next-thread", Run::StepThread(1), "thread.next"),
            ("action:previous-thread", Run::StepThread(-1), "thread.previous"),
        ] {
            let command = command_of(id).unwrap();
            assert_eq!((command.run, command.chord), (step, Some(chord)), "{id}");
            let dispatched = run_action(command.run).unwrap();
            assert!(!dispatched.partial_eq(action(chord).unwrap().as_ref()), "{id} dispatches the step, not the chord's own action");
        }
        assert!(run_action(Run::StepWorkspace(1)).unwrap().partial_eq(&StepWorkspace(1)));
        assert!(run_action(Run::StepThread(-1)).unwrap().partial_eq(&StepThread(-1)));
        let ids: Vec<&str> = COMMANDS.iter().map(|c| c.id).collect();
        let unique: std::collections::BTreeSet<&str> = ids.iter().copied().collect();
        assert_eq!(unique.len(), ids.len(), "one row per id");
        for command in COMMANDS {
            if let Some(chord) = command.chord {
                assert!(DEFAULTS.iter().any(|r| r.command == chord), "{} names a chord the table binds", command.id);
            }
        }
    }

    fn quoted<'a>(line: &'a str, field: &str) -> Option<&'a str> {
        let start = line.find(&format!("{field}: \""))? + field.len() + 3;
        Some(&line[start..start + line[start..].find('"')?])
    }

    #[test]
    fn matches_the_web_table() {
        let web = concat!(env!("CARGO_MANIFEST_DIR"), "/../../web/src");
        let defaults = std::fs::read_to_string(format!("{web}/keybindingDefaults.ts")).unwrap();
        let types = std::fs::read_to_string(format!("{web}/keybindingTypes.ts")).unwrap();
        let slots = types.split("WORKSPACE_SELECT_SLOTS = [").nth(1).and_then(|rest| rest.split(']').next()).unwrap();
        let body = defaults.split("DEFAULT_KEYBINDINGS: ReadonlyArray<KeybindingRule> = [").nth(1).unwrap().split("\n];").next().unwrap();
        let mut web_rules: Vec<(String, String, Option<String>)> = Vec::new();
        for line in body.lines().map(str::trim).filter(|line| !line.is_empty() && !line.starts_with("//")) {
            if line.starts_with("...WORKSPACE_SELECT_SLOTS.map(") {
                assert!(line.contains("key: `mod+${slot}`, command: workspaceSelectCommand(slot)"), "{line}");
                for slot in slots.split(',').map(str::trim) {
                    web_rules.push((format!("workspace.select.{slot}"), format!("mod+{slot}"), None));
                }
                continue;
            }
            let rules: Vec<&str> = line.split('{').filter(|rule| rule.contains("key:")).collect();
            assert!(!rules.is_empty(), "unread line in keybindingDefaults.ts: {line}");
            for rule in rules {
                let (key, command) = (quoted(rule, "key"), quoted(rule, "command"));
                let (Some(key), Some(command)) = (key, command) else { panic!("unread rule in keybindingDefaults.ts: {rule}") };
                web_rules.push((command.to_owned(), key.to_owned(), quoted(rule, "when").map(str::to_owned)));
            }
        }
        let ours: Vec<(String, String, Option<String>)> =
            DEFAULTS.iter().map(|rule| (rule.command.to_owned(), rule.chord.to_owned(), rule.when.map(str::to_owned))).collect();
        for (at, (web_rule, our_rule)) in web_rules.iter().zip(&ours).enumerate() {
            assert_eq!(our_rule, web_rule, "row {at}");
        }
        assert_eq!(ours.len(), web_rules.len());
    }
}
